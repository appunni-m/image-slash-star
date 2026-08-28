#!/usr/bin/env python3
"""Search a fixed corpus for an origin Vertical8x16 filter-intra AVIF leaf.

The search is deliberately bounded and input-driven. It creates exactly one
hundred deterministic 8x16 RGB candidates, encodes each twice through the
pinned Pillow/libavif/libaom oracle, and classifies an independently
instrumented scalar dav1d trace. Generated files are temporary unless
``--retain-dir`` is supplied; no repository Rust code is invoked.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import re
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from generate_av1_reconstruction_refs import (
    DAV1D_COMMIT,
    build_dav1d,
    extract_color_item,
    resolve_tool,
    run,
    verify_source,
)


SIZE = (8, 16)
EXPECTED_YUV_BYTES = SIZE[0] * SIZE[1] + 2 * (SIZE[0] // 2) * (SIZE[1] // 2)
SUBSAMPLING = "4:2:0"
LAYOUT_UNSPLIT = "unsplit-tx8x16"
LAYOUT_TX4X4_GRID = "tx4x4-grid-2x4"
ADVANCED = {
    "min-partition-size": "8",
    "max-partition-size": "16",
    "use-intra-dct-only": "1",
    "enable-filter-intra": "1",
    "enable-intra-edge-filter": "0",
    "enable-smooth-intra": "0",
    "enable-paeth-intra": "0",
    "enable-directional-intra": "0",
    "enable-cfl-intra": "0",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "loopfilter-control": "0",
    "aq-mode": "0",
    "deltaq-mode": "0",
}
BLOCK_PATTERN = re.compile(
    r"^poc=(?P<poc>-?\d+),y=(?P<y>-?\d+),x=(?P<x>-?\d+),"
    r"bl=(?P<level>\d+),ctx=(?P<context>\d+),bp=(?P<partition>\d+): "
    r"r=(?P<range>\d+)$"
)
FILTER_PATTERN = re.compile(
    r"^Post-filterintramode\[(?P<y_mode>\d+)/(?P<filter_mode>\d+)\]"
)
LUMA_PATTERN = re.compile(
    r"^Post-y-cf-blk\[tx=(?P<tx>\d+),txtp=(?P<txtp>-?\d+),"
    r"eob=(?P<eob>-?\d+)\]"
)
CHROMA_PATTERN = re.compile(
    r"^Post-uv-cf-blk\[pl=(?P<plane>\d+),tx=(?P<tx>\d+),"
    r"txtp=(?P<txtp>-?\d+),eob=(?P<eob>-?\d+)\]"
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of ``data``."""

    return hashlib.sha256(data).hexdigest()


def clamp(value: int) -> int:
    """Clamp one generated RGB component to a byte."""

    return max(0, min(255, value))


def rgb_noise(seed: int) -> bytes:
    """Generate deterministic RGB noise."""

    state = random.Random(seed)
    return bytes(state.randrange(256) for _ in range(SIZE[0] * SIZE[1] * 3))


def gray_noise(seed: int) -> bytes:
    """Generate deterministic grayscale noise represented as RGB."""

    state = random.Random(seed)
    return bytes(
        value
        for _ in range(SIZE[0] * SIZE[1])
        for value in (state.randrange(256),) * 3
    )


def color_ramp(seed: int) -> bytes:
    """Generate a chromatic two-dimensional ramp."""

    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            base = 24 + ((5 * x + 9 * y + seed) % 192)
            pixels.extend((clamp(base + 24), base, clamp(base - 24)))
    return bytes(pixels)


def quadrant_pattern(seed: int) -> bytes:
    """Generate four contrasting quadrants with small ripples."""

    colors = ((32, 80, 160), (224, 64, 32), (48, 192, 80), (208, 192, 48))
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            color = colors[(x // 4) + 2 * (y // 8)]
            ripple = ((7 * x + 11 * y + seed) % 13) - 6
            pixels.extend(clamp(value + ripple) for value in color)
    return bytes(pixels)


def checker_pattern(seed: int) -> bytes:
    """Generate a high-contrast checkerboard with chroma variation."""

    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            luma = 208 if (x + y + seed) % 2 == 0 else 48
            pixels.extend((luma, clamp(luma + 17), clamp(luma - 17)))
    return bytes(pixels)


def split_noise(seed: int) -> bytes:
    """Generate a structured top half and noisy bottom half."""

    noise = rgb_noise(seed)
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if y < 8:
                base = 40 + ((3 * x + 7 * y) % 88)
                pixels.extend((clamp(base + 20), base, clamp(base - 20)))
            else:
                offset = ((y - 8) * SIZE[0] + x) * 3
                pixels.extend(noise[offset : offset + 3])
    return bytes(pixels)


def grid_pattern(seed: int) -> bytes:
    """Generate a 2x4 color grid with local ripples."""

    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            cell = (x // 2) + 4 * (y // 4)
            base = 24 + 29 * ((cell + seed) % 8)
            ripple = ((13 * x + 17 * y + seed) % 11) - 5
            pixels.extend(
                (clamp(base + ripple + 15), clamp(base + ripple), clamp(base + ripple - 15))
            )
    return bytes(pixels)


def diagonal_pattern(seed: int) -> bytes:
    """Generate a diagonal edge with two-dimensional texture."""

    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            base = 44 if x + y < 7 + seed % 3 else 212
            ripple = ((5 * x + 3 * y + seed) % 17) - 8
            pixels.extend((clamp(base + ripple + 12), clamp(base + ripple), clamp(base + ripple - 12)))
    return bytes(pixels)


def flat_with_noise(seed: int) -> bytes:
    """Generate a flat chromatic field with sparse noise."""

    state = random.Random(seed)
    pixels = bytearray()
    for _ in range(SIZE[0] * SIZE[1]):
        delta = state.randrange(-24, 25)
        pixels.extend((clamp(146 + delta), clamp(128 + delta), clamp(110 + delta)))
    return bytes(pixels)


def mosaic_pattern(seed: int) -> bytes:
    """Generate a coarse luma mosaic with chroma variation."""

    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            tile = (x // 2) + 4 * (y // 4)
            luma = (36, 92, 148, 204)[(tile + seed) % 4]
            chroma = ((x * 9 + y * 7 + seed) % 31) - 15
            pixels.extend((luma, clamp(128 + chroma), clamp(128 - chroma)))
    return bytes(pixels)


def candidates() -> list[dict[str, object]]:
    """Return ten deterministic families with ten cases each."""

    families = (
        ("F01_rgb_noise", rgb_noise, 101),
        ("F02_gray_noise", gray_noise, 201),
        ("F03_color_ramp", color_ramp, 301),
        ("F04_quadrants", quadrant_pattern, 401),
        ("F05_checker", checker_pattern, 501),
        ("F06_split_noise", split_noise, 601),
        ("F07_grid", grid_pattern, 701),
        ("F08_diagonal", diagonal_pattern, 801),
        ("F09_flat_noise", flat_with_noise, 901),
        ("F10_mosaic", mosaic_pattern, 1001),
    )
    result = []
    for family, generator, first_seed in families:
        for index in range(10):
            seed = first_seed + index
            result.append(
                {
                    "id": f"{family.lower()}_{index:02d}",
                    "family": family,
                    "seed": seed,
                    "pixels": generator(seed),
                    "quality": 76,
                    "speed": 0,
                }
            )
    if len(result) != 100:
        raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
    return result


def encode(pixels: bytes, quality: int, speed: int) -> bytes:
    """Encode one candidate with the pinned Pillow AVIF oracle."""

    output = BytesIO()
    Image.frombytes("RGB", SIZE, pixels).save(
        output,
        format="AVIF",
        quality=quality,
        speed=speed,
        max_threads=1,
        subsampling=SUBSAMPLING,
        autotiling=False,
        advanced=ADVANCED,
    )
    return output.getvalue()


def parse_trace(output: str) -> tuple[list[dict[str, int]], list[list[str]], int]:
    """Parse partition blocks, leaf groups, and the scalar entropy count."""

    lines = [line.rstrip() for line in output.splitlines() if line.strip()]
    entropy = [
        json.loads(line.removeprefix("@MSAC "))
        for line in lines
        if line.startswith("@MSAC ")
    ]
    if not entropy or entropy[0].get("operation") != "init":
        raise RuntimeError("missing scalar MSAC trace")
    if [operation["step"] for operation in entropy] != list(range(len(entropy))):
        raise RuntimeError("non-contiguous scalar MSAC trace")
    blocks = []
    for line in lines:
        if match := BLOCK_PATTERN.fullmatch(line):
            blocks.append({name: int(value) for name, value in match.groupdict().items()})
    groups: list[list[str]] = []
    for line in lines:
        if line.startswith("Post-skip["):
            groups.append([])
        if groups:
            groups[-1].append(line)
    if not blocks:
        raise RuntimeError("missing partition trace")
    return blocks, groups, len(entropy)


def classify(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    target_filter_mode: int,
    target_layout: str,
    yuv_bytes: int,
) -> dict[str, object]:
    """Apply exact predicates for one origin Vertical8x16 target layout."""

    root_blocks = [
        block
        for block in blocks
        if block["level"] == 3 and block["x"] == 0 and block["y"] == 0
    ]
    root = root_blocks[0] if len(root_blocks) == 1 else None
    leaf = groups[0] if len(groups) == 1 else []
    filter_modes = []
    luma_payloads = []
    chroma_payloads = []
    syntax_markers = []
    for line in leaf:
        if line.startswith(
            (
                "Post-skip[",
                "Post-cdef_idx[",
                "Post-ymode[",
                "Post-uvmode[",
                "Post-filterintramode[",
                "Post-tx[",
            )
        ):
            syntax_markers.append(line.split(":", 1)[0])
        if match := FILTER_PATTERN.match(line):
            filter_modes.append(
                {"y_mode": int(match["y_mode"]), "filter_mode": int(match["filter_mode"])}
            )
        if match := LUMA_PATTERN.match(line):
            luma_payloads.append({name: int(value) for name, value in match.groupdict().items()})
        if match := CHROMA_PATTERN.match(line):
            chroma_payloads.append({name: int(value) for name, value in match.groupdict().items()})
    expected_tx_marker = "Post-tx[7]" if target_layout == LAYOUT_UNSPLIT else "Post-tx[0]"
    expected_syntax_markers = [
        "Post-skip[0]",
        "Post-cdef_idx[0]",
        "Post-ymode[0]",
        "Post-uvmode[0]",
        f"Post-filterintramode[13/{target_filter_mode}]",
        expected_tx_marker,
    ]
    if target_layout == LAYOUT_UNSPLIT:
        luma_shape = len(luma_payloads) == 1 and luma_payloads[0]["tx"] == 7
    elif target_layout == LAYOUT_TX4X4_GRID:
        luma_shape = len(luma_payloads) == 8 and all(
            payload["tx"] == 0 for payload in luma_payloads
        )
    else:
        raise ValueError(f"unsupported target layout: {target_layout}")
    predicates = {
        "one_origin_vertical8x16_root": (
            len(blocks) == 1
            and root is not None
            and root["partition"] == 2
        ),
        "single_leaf_group": len(groups) == 1,
        "ordered_filter_intra_syntax": syntax_markers == expected_syntax_markers,
        "filter_intra_selected": (
            len(filter_modes) == 1
            and filter_modes[0]["y_mode"] == 13
            and filter_modes[0]["filter_mode"] == target_filter_mode
        ),
        "luma_layout": luma_shape,
        "luma_nonempty": bool(luma_payloads) and all(
            payload["eob"] > 0 for payload in luma_payloads
        ),
        "one_tx4x8_chroma_pair": (
            len(chroma_payloads) == 2
            and {item["plane"] for item in chroma_payloads} == {0, 1}
            and all(item["tx"] == 5 for item in chroma_payloads)
            and all(item["eob"] >= 0 for item in chroma_payloads)
        ),
        "full_yuv_output": yuv_bytes == EXPECTED_YUV_BYTES,
    }
    return {
        "root_partition": root,
        "group_count": len(groups),
        "target_filter_mode": target_filter_mode,
        "target_layout": target_layout,
        "syntax_markers": syntax_markers,
        "filter_modes": filter_modes,
        "luma_payloads": luma_payloads,
        "chroma_payloads": chroma_payloads,
        "yuv_bytes": yuv_bytes,
        "predicates": predicates,
        "rejection_reasons": [name for name, passed in predicates.items() if not passed],
        "qualifies": all(predicates.values()),
    }


def decode_candidate(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    candidate: dict[str, object],
    retain_dir: Path | None,
    target_filter_mode: int,
    target_layout: str,
) -> dict[str, object]:
    """Encode, independently decode, and classify one candidate."""

    pixels = candidate["pixels"]
    if not isinstance(pixels, bytes):
        raise TypeError("candidate pixels must be bytes")
    encoded = encode(pixels, int(candidate["quality"]), int(candidate["speed"]))
    encoded_second = encode(pixels, int(candidate["quality"]), int(candidate["speed"]))
    if encoded != encoded_second:
        raise RuntimeError(f"nondeterministic encoding for {candidate['id']}")
    path = work / f"{candidate['id']}.avif"
    path.write_bytes(encoded)
    if retain_dir is not None:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path.name).write_bytes(encoded)
    sample, _ = extract_color_item(path)
    sample_path = work / f"{candidate['id']}.obu"
    output_path = work / f"{candidate['id']}.yuv"
    sample_path.write_bytes(sample)
    result = run(
        [
            str(executable),
            "--input",
            str(sample_path),
            "--demuxer",
            "section5",
            "--output",
            str(output_path),
            "--muxer",
            "yuv",
            "--threads",
            "1",
            "--framedelay",
            "1",
            "--cpumask",
            "0",
            "--quiet",
        ],
        env=environment,
    )
    blocks, groups, entropy_count = parse_trace(result.stdout)
    yuv = output_path.read_bytes()
    yuv_bytes = len(yuv)
    return {
        "id": candidate["id"],
        "family": candidate["family"],
        "seed": candidate["seed"],
        "quality": candidate["quality"],
        "speed": candidate["speed"],
        "input_rgb_sha256": sha256(pixels),
        "encoded_file_sha256": sha256(encoded),
        "encoded_file_sha256_second": sha256(encoded_second),
        "encoded_item_sha256": sha256(sample),
        "encoded_item_length": len(sample),
        "decoded_yuv_sha256": sha256(yuv),
        "entropy_operation_count": entropy_count,
        "partition_blocks": blocks,
        "repository_rust_invoked": False,
        **classify(blocks, groups, target_filter_mode, target_layout, yuv_bytes),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument("--dav1d-source", type=Path)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--python-path", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--retain-dir", type=Path)
    parser.add_argument("--filter-mode", type=int, choices=range(5), default=2)
    parser.add_argument(
        "--target-layout",
        choices=(LAYOUT_UNSPLIT, LAYOUT_TX4X4_GRID),
        default=LAYOUT_UNSPLIT,
        help="exact luma payload topology to qualify",
    )
    args = parser.parse_args()
    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")
    with tempfile.TemporaryDirectory(prefix="image-star-avif-v8x16-filter-") as name:
        work = Path(name)
        if args.dav1d_source is not None:
            source = args.dav1d_source.resolve()
            verify_source(source)
            executable, environment = build_dav1d(
                source,
                work,
                resolve_tool(args.meson, "Meson"),
                resolve_tool(args.ninja, "Ninja"),
                args.python_path.resolve() if args.python_path else None,
            )
        else:
            executable = args.dav1d.resolve()
            environment = {}
        version_result = run([str(executable), "--version"], env=environment)
        version = (version_result.stdout + version_result.stderr).strip()
        if not version.startswith("1.5.3-0-gb546257"):
            raise RuntimeError(f"unexpected dav1d executable version: {version}")
        reports = [
            decode_candidate(
                executable,
                environment,
                work,
                candidate,
                args.retain_dir,
                args.filter_mode,
                args.target_layout,
            )
            for candidate in candidates()
        ]
    report = {
        "format_version": 1,
        "oracle": {
            "pillow": Image.__version__,
            "libavif": features.version("avif"),
            "codecs": codecs,
            "dav1d": version,
            "dav1d_commit": DAV1D_COMMIT,
        },
        "encoding": {
            "size": list(SIZE),
            "subsampling": SUBSAMPLING,
            "max_threads": 1,
            "autotiling": False,
            "advanced": ADVANCED,
        },
        "search": {
            "input_only": True,
            "candidate_count": len(reports),
            "family_count": 10,
            "candidates_per_family": 10,
            "repository_rust_invoked": False,
            "target_id": args.target_layout,
            "target": (
                "origin Vertical8x16 FILTER_PRED mode "
                f"{args.filter_mode} with "
                + (
                    "one TX8x16 luma payload"
                    if args.target_layout == LAYOUT_UNSPLIT
                    else "a 2x4 grid of eight TX4x4 luma payloads"
                )
                + " and TX4x8 U/V pair"
            ),
            "filter_mode": args.filter_mode,
            "target_layout": args.target_layout,
            "expected_yuv_bytes": EXPECTED_YUV_BYTES,
            "families": [
                "F01_rgb_noise",
                "F02_gray_noise",
                "F03_color_ramp",
                "F04_quadrants",
                "F05_checker",
                "F06_split_noise",
                "F07_grid",
                "F08_diagonal",
                "F09_flat_noise",
                "F10_mosaic",
            ],
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "by_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in (
                    "one_origin_vertical8x16_root",
                    "single_leaf_group",
                    "ordered_filter_intra_syntax",
                    "filter_intra_selected",
                    "luma_layout",
                    "luma_nonempty",
                    "one_tx4x8_chroma_pair",
                    "full_yuv_output",
                )
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic Vertical8x16 traces: {args.output}")


if __name__ == "__main__":
    main()
