#!/usr/bin/env python3
"""Search a fixed corpus for a right-hand Square8 Diagonal113 AVIF leaf.

The search is deliberately bounded and input-driven. It creates exactly one
hundred deterministic 16x8 RGB candidates, encodes each twice through the
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


SIZE = (16, 8)
SUBSAMPLING = "4:2:0"
ADVANCED = {
    "min-partition-size": "8",
    "max-partition-size": "8",
    "use-intra-dct-only": "0",
    "enable-filter-intra": "0",
    "enable-intra-edge-filter": "0",
    "enable-smooth-intra": "0",
    "enable-paeth-intra": "0",
    "enable-directional-intra": "1",
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


def chroma_pattern(seed: int, kind: int) -> bytes:
    """Generate deterministic chroma-oriented RGB families."""

    state = random.Random(seed)
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if kind == 1:
                phase = (x + y + seed % 7) % 16
                color = (24 + 15 * phase, 180 - 9 * phase, 230 - 11 * phase)
            elif kind == 2:
                phase = (3 * x - 5 * y + seed) % 32
                color = (45 + 6 * phase, 220 - 5 * phase, 35 + 7 * phase)
            elif kind == 3:
                phase = (x - y + seed) % 8
                color = ((30, 210, 220), (210, 50, 35))[phase < 4]
            elif kind == 4:
                phase = (x + y + seed) % 8
                color = ((235, 55, 45), (25, 210, 225))[phase < 4]
            elif kind == 5:
                phase = (5 * x + 2 * y + seed) % 24
                color = (100 + 7 * phase, 80 + 5 * phase, 240 - 8 * phase)
            elif kind == 6:
                phase = (7 * x - 3 * y + seed) % 20
                color = (230 - 8 * phase, 35 + 10 * phase, 50 + 5 * phase)
            elif kind == 7:
                value = 50 + ((11 * x + 17 * y + seed) % 120)
                color = (value + 50, value - 20, 250 - value // 2)
            elif kind == 8:
                block = ((x // 2) + (y // 2) + seed) % 4
                color = ((230, 40, 40), (40, 220, 60), (40, 70, 230), (220, 190, 35))[block]
            else:
                value = state.randrange(256)
                color = (value + 30 * x - 12 * y, 180 + 8 * y - 17 * x + value // 8, 60 + 13 * x + value // 5)
            pixels.extend(clamp(component) for component in color)
    return bytes(pixels)


def candidates() -> list[dict[str, object]]:
    """Return ten deterministic families with ten cases each."""

    result: list[dict[str, object]] = []
    result.extend(
        {
            "id": f"f01_rgb_noise_{index:02d}",
            "family": "F01_rgb_noise",
            "seed": 211 + index,
            "pixels": rgb_noise(211 + index),
            "quality": 76,
            "speed": 0,
        }
        for index in range(10)
    )
    families = (
        "F02_diagonal_chroma_ramp",
        "F03_hue_ramp",
        "F04_diagonal_two_color",
        "F05_antidiagonal_two_color",
        "F06_blue_ramp",
        "F07_red_ramp",
        "F08_luma_chroma",
        "F09_mosaic",
        "F10_smooth_noise",
    )
    for kind, family in enumerate(families, 1):
        for index in range(10):
            seed = 300 + 10 * kind + index
            result.append(
                {
                    "id": f"{family.lower()}_{index:02d}",
                    "family": family,
                    "seed": seed,
                    "pixels": chroma_pattern(seed, kind),
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


def classify(blocks: list[dict[str, int]], groups: list[list[str]]) -> dict[str, object]:
    """Apply exact predicates for the right-hand Square8 mode-5 leaf."""

    root = next(
        (
            block
            for block in blocks
            if block["level"] == 3 and block["x"] == 0 and block["y"] == 0
        ),
        None,
    )
    right = groups[1] if len(groups) == 2 else []
    uv_modes = [
        int(line.split("[", 1)[1].split("]", 1)[0])
        for line in right
        if line.startswith("Post-uvmode[")
    ]
    luma_payloads = []
    chroma_payloads = []
    for line in right:
        if match := LUMA_PATTERN.match(line):
            luma_payloads.append({name: int(value) for name, value in match.groupdict().items()})
        if match := CHROMA_PATTERN.match(line):
            chroma_payloads.append({name: int(value) for name, value in match.groupdict().items()})
    predicates = {
        "visible_split_root": root is not None and root["partition"] == 3,
        "two_visible_square8_groups": len(groups) == 2
        and all(block["level"] == 4 for block in blocks[1:3]),
        "right_uv_mode_5": uv_modes == [5],
        "right_square8_luma": len(luma_payloads) >= 1
        and all(payload["tx"] == 0 for payload in luma_payloads),
        "right_adst_dct_chroma": len(chroma_payloads) == 2
        and {payload["plane"] for payload in chroma_payloads} == {0, 1}
        and all(payload["tx"] == 0 and payload["txtp"] == 1 for payload in chroma_payloads),
        "right_chroma_nonempty": any(payload["eob"] >= 0 for payload in chroma_payloads),
    }
    return {
        "root_partition": root,
        "group_count": len(groups),
        "right_uv_modes": uv_modes,
        "right_luma_payloads": luma_payloads,
        "right_chroma_payloads": chroma_payloads,
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
) -> dict[str, object]:
    """Encode, independently decode, and classify one candidate."""

    pixels = candidate["pixels"]
    if not isinstance(pixels, bytes):
        raise TypeError("candidate pixels must be bytes")
    quality = int(candidate["quality"])
    speed = int(candidate["speed"])
    encoded = encode(pixels, quality, speed)
    if encoded != encode(pixels, quality, speed):
        raise RuntimeError(f"nondeterministic encoding for {candidate['id']}")
    path = work / f"{candidate['id']}.avif"
    path.write_bytes(encoded)
    if retain_dir is not None:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path.name).write_bytes(encoded)
    item, _ = extract_color_item(path)
    item_path = work / f"{candidate['id']}.obu"
    yuv_path = work / f"{candidate['id']}.yuv"
    item_path.write_bytes(item)
    result = run(
        [
            str(executable),
            "--input",
            str(item_path),
            "--demuxer",
            "section5",
            "--output",
            str(yuv_path),
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
    return {
        "id": candidate["id"],
        "family": candidate["family"],
        "seed": candidate["seed"],
        "quality": quality,
        "speed": speed,
        "input_rgb_sha256": sha256(pixels),
        "encoded_file_sha256": sha256(encoded),
        "encoded_item_sha256": sha256(item),
        "encoded_item_length": len(item),
        "entropy_operation_count": entropy_count,
        "partition_blocks": blocks,
        **classify(blocks, groups),
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
    args = parser.parse_args()
    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")
    with tempfile.TemporaryDirectory(prefix="image-star-avif-diagonal113-") as name:
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
            decode_candidate(executable, environment, work, candidate, args.retain_dir)
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
            "candidate_count": len(reports),
            "target": "visible right-hand Square8 leaf with coded UV mode 5 (Diagonal113), ADST_DCT chroma transform, and non-skipped chroma residual",
            "families": [
                "F01_rgb_noise",
                "F02_diagonal_chroma_ramp",
                "F03_hue_ramp",
                "F04_diagonal_two_color",
                "F05_antidiagonal_two_color",
                "F06_blue_ramp",
                "F07_red_ramp",
                "F08_luma_chroma",
                "F09_mosaic",
                "F10_smooth_noise",
            ],
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "by_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in (
                    "visible_split_root",
                    "two_visible_square8_groups",
                    "right_uv_mode_5",
                    "right_square8_luma",
                    "right_adst_dct_chroma",
                    "right_chroma_nonempty",
                )
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic Diagonal113 traces: {args.output}")


if __name__ == "__main__":
    main()
