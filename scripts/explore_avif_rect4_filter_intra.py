#!/usr/bin/env python3
"""Search the two AV1 rectangular filter-CDF arms through public AVIF input.

The campaign creates exactly one hundred deterministic 16x16 RGB candidates
in ten families.  Each family contains five horizontal-band candidates whose
target geometry is ``Horizontal16x4`` and five transposed vertical-band
candidates whose target geometry is ``Vertical4x16``.  The candidates are
encoded twice through the pinned Pillow/libavif/libaom oracle and decoded twice
through an independently instrumented scalar dav1d build.

The target is the geometry-specific ``use_filter_intra`` CDF-row selection in
the safe Rust decoder.  A false ``Post-filterintramode[0/0]`` decision is
intentional and sufficient: the CDF row is selected before the adaptive bool
is read.  This campaign does not claim filter-intra reconstruction.  The
repository Rust implementation is never invoked.
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
    portable_color_reference,
    resolve_tool,
    run,
    verify_source,
)


SIZE = (16, 16)
SUBSAMPLING = "4:2:0"
FAMILY_NAMES = (
    "F01_known_ramp",
    "F02_shifted_ramp",
    "F03_low_contrast_steps",
    "F04_high_contrast_steps",
    "F05_sawtooth",
    "F06_ripple",
    "F07_checker_modulation",
    "F08_sparse_noise",
    "F09_chroma_variation",
    "F10_mixed_texture",
)
ADVANCED = {
    "min-partition-size": "4",
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
RAMPS = (
    ((17, 91, 203), (32, 32, 32), (0, 255, 0), (127, 127, 127)),
    ((28, 86, 190), (42, 48, 54), (5, 224, 26), (138, 122, 112)),
    ((90, 112, 134), (104, 126, 148), (118, 140, 162), (132, 154, 176)),
    ((8, 32, 220), (220, 32, 8), (16, 220, 48), (220, 192, 12)),
    ((36, 76, 164), (52, 104, 184), (72, 132, 204), (96, 164, 224)),
    ((24, 104, 184), (60, 140, 220), (108, 176, 232), (156, 208, 244)),
    ((20, 80, 180), (220, 72, 24), (28, 192, 76), (224, 188, 32)),
    ((46, 90, 154), (68, 112, 176), (90, 134, 198), (112, 156, 220)),
    ((34, 128, 226), (80, 164, 214), (128, 196, 190), (176, 222, 164)),
    ((24, 64, 176), (48, 112, 208), (92, 156, 226), (144, 196, 236)),
)
BLOCK_PATTERN = re.compile(
    r"^poc=(?P<poc>-?\d+),y=(?P<y>-?\d+),x=(?P<x>-?\d+),"
    r"bl=(?P<level>\d+),ctx=(?P<context>\d+),bp=(?P<partition>\d+): "
    r"r=(?P<range>\d+)$"
)
FILTER_PATTERN = re.compile(
    r"^Post-filterintramode\[(?P<y_mode>\d+)/(?P<filter_mode>\d+)\]"
)
YMODE_PATTERN = re.compile(r"^Post-ymode\[(?P<mode>\d+)\]")
UVMODE_PATTERN = re.compile(r"^Post-uvmode\[(?P<mode>\d+)\]")
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
    """Clamp one generated component to an eight-bit sample."""

    return max(0, min(255, value))


def noise(seed: int, count: int) -> list[int]:
    """Return a deterministic bounded noise stream."""

    state = random.Random(seed)
    return [state.randrange(-12, 13) for _ in range(count)]


def candidate_pixels(family: int, index: int, orientation: str) -> bytes:
    """Create one deterministic RGB candidate for one target orientation."""

    ramp = RAMPS[family]
    seed = 4000 + family * 100 + index
    random_values = noise(seed, SIZE[0] * SIZE[1])
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            band_coordinate = y if orientation == "h16x4" else x
            other_coordinate = x if orientation == "h16x4" else y
            band = min(3, band_coordinate // 4)
            base = ramp[band]
            # Keep family zero as the exact known ramp that already emits the
            # two desired partition trees.  The remaining families vary the
            # same public raster without encoding any expected decoder result.
            if family == 0:
                components = base
            else:
                phase = (7 * family + 3 * index) % 19
                ripple = ((5 * other_coordinate + 3 * band_coordinate + phase) % 13) - 6
                sample = random_values[y * SIZE[0] + x]
                if family in (1, 2):
                    deltas = (ripple, ripple // 2, -ripple)
                elif family in (3, 4):
                    deltas = (sample + ripple, ripple, -sample)
                elif family == 5:
                    deltas = (ripple, -ripple, ripple // 2)
                elif family == 6:
                    checker = 8 if (other_coordinate // 2 + band_coordinate // 2) % 2 else -8
                    deltas = (checker + ripple, checker, -checker)
                elif family == 7:
                    deltas = (sample, sample // 2, -sample)
                elif family == 8:
                    chroma = ((11 * other_coordinate + 7 * band_coordinate + seed) % 17) - 8
                    deltas = (ripple + chroma, -chroma, -ripple)
                else:
                    deltas = (ripple + sample // 2, sample, -ripple)
                components = tuple(clamp(value + delta) for value, delta in zip(base, deltas))
            pixels.extend(components)
    return bytes(pixels)


def candidates() -> list[dict[str, object]]:
    """Return ten families with five cases per orientation."""

    result = []
    for family, family_name in enumerate(FAMILY_NAMES):
        for index in range(10):
            orientation = "h16x4" if index < 5 else "v4x16"
            result.append(
                {
                    "id": f"R4-{orientation.upper()}-F{family + 1:02d}-N{index:02d}",
                    "family": family_name,
                    "family_index": family,
                    "candidate_index": index,
                    "orientation": orientation,
                    "seed": 4000 + family * 100 + index,
                    "pixels": candidate_pixels(family, index, orientation),
                    "quality": 12,
                    "speed": 0,
                }
            )
    if len(result) != 100:
        raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
    if any(
        sum(candidate["family"] == family for candidate in result) != 10
        for family in FAMILY_NAMES
    ):
        raise AssertionError("each campaign family must contain exactly ten cases")
    if any(
        sum(candidate["orientation"] == orientation for candidate in result) != 50
        for orientation in ("h16x4", "v4x16")
    ):
        raise AssertionError("each rectangular orientation must contain 50 cases")
    return result


def encode(pixels: bytes, quality: int, speed: int) -> bytes:
    """Encode one candidate through the pinned Pillow AVIF oracle."""

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
    """Parse partition blocks, leaf groups, and the scalar entropy trace."""

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
    debug = [line for line in lines if not line.startswith("@MSAC ")]
    blocks = []
    for line in debug:
        if match := BLOCK_PATTERN.fullmatch(line):
            blocks.append({name: int(value) for name, value in match.groupdict().items()})
    groups: list[list[str]] = []
    for line in debug:
        if line.startswith("Post-skip["):
            groups.append([])
        if groups:
            groups[-1].append(line)
    if not blocks:
        raise RuntimeError("missing partition trace")
    if not groups:
        raise RuntimeError("missing leaf groups")
    return blocks, groups, len(entropy)


def parse_group(group: list[str]) -> dict[str, object]:
    """Extract public syntax states from one dav1d leaf group."""

    y_modes = [
        int(match["mode"])
        for line in group
        if (match := YMODE_PATTERN.match(line)) is not None
    ]
    uv_modes = [
        int(match["mode"])
        for line in group
        if (match := UVMODE_PATTERN.match(line)) is not None
    ]
    filters = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in group
        if (match := FILTER_PATTERN.match(line)) is not None
    ]
    luma = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in group
        if (match := LUMA_PATTERN.match(line)) is not None
    ]
    chroma = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in group
        if (match := CHROMA_PATTERN.match(line)) is not None
    ]
    return {
        "y_modes": y_modes,
        "uv_modes": uv_modes,
        "filter_decisions": filters,
        "luma_payloads": luma,
        "chroma_payloads": chroma,
    }


def classify(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
    orientation: str,
) -> dict[str, object]:
    """Apply exact trace predicates for both rectangular CDF rows."""

    parsed = [parse_group(group) for group in groups]
    expected_partition = 8 if orientation == "h16x4" else 9
    expected_luma_tx = 14 if orientation == "h16x4" else 13
    expected_chroma_tx = 6 if orientation == "h16x4" else 5
    root = next(
        (
            block
            for block in blocks
            if block["x"] == 0
            and block["y"] == 0
            and block["level"] == 3
            and block["context"] == 0
        ),
        None,
    )
    y_modes = [mode for group in parsed for mode in group["y_modes"]]
    uv_modes = [mode for group in parsed for mode in group["uv_modes"]]
    filters = [decision for group in parsed for decision in group["filter_decisions"]]
    luma_payloads = [payload for group in parsed for payload in group["luma_payloads"]]
    chroma_payloads = [payload for group in parsed for payload in group["chroma_payloads"]]
    no_luma_palette = not any(
        line.startswith(("Post-pal[pl=0", "Post-y-pal-indices", "y-pal-pred"))
        for group in groups
        for line in group
    )
    y_plane = yuv[: SIZE[0] * SIZE[1]]
    predicates = {
        "frame_is_16x16_8bit_420": (
            portable_color.get("width") == 16
            and portable_color.get("height") == 16
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "root_is_target_rectangular_partition": (
            root is not None
            and root["partition"] == expected_partition
            and root["level"] == 3
            and root["x"] == 0
            and root["y"] == 0
        ),
        "four_terminal_groups": len(groups) == 4,
        "all_terminal_luma_modes_are_dc": len(groups) == 4 and y_modes == [0] * 4,
        "all_filter_decisions_are_false_sentinel": (
            filters == [{"y_mode": 0, "filter_mode": 0}] * 4
        ),
        "no_luma_palette_syntax": no_luma_palette,
        "no_angle_syntax": not any(
            line.startswith("Post-" + suffix)
            for group in groups
            for line in group
            for suffix in ("yangle-symbol[", "uvangle-symbol[")
        ),
        "all_luma_payloads_are_target_rectangles": (
            len(luma_payloads) == 4
            and all(
                payload["tx"] == expected_luma_tx and payload["txtp"] == 0
                for payload in luma_payloads
            )
        ),
        "all_chroma_payloads_are_target_rectangles": (
            len(chroma_payloads) == 4
            and {payload["plane"] for payload in chroma_payloads} == {0, 1}
            and all(
                payload["tx"] == expected_chroma_tx and payload["txtp"] == 0
                for payload in chroma_payloads
            )
        ),
        "decoded_yuv_has_expected_size": len(yuv) == 384,
        "visible_luma_varies": len(y_plane) == 256 and len(set(y_plane)) > 1,
    }
    return {
        "target": "cdf14_h16x4_false" if orientation == "h16x4" else "cdf19_v4x16_false",
        "orientation": orientation,
        "target_filter_cdf_index": 14 if orientation == "h16x4" else 19,
        "root_partition": root,
        "group_count": len(groups),
        "y_modes": y_modes,
        "uv_modes": uv_modes,
        "filter_decisions": filters,
        "luma_payloads": luma_payloads,
        "chroma_payloads": chroma_payloads,
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
    """Double-encode, double-trace, and classify one candidate."""

    pixels = candidate["pixels"]
    if not isinstance(pixels, bytes):
        raise TypeError("candidate pixels must be bytes")
    quality = int(candidate["quality"])
    speed = int(candidate["speed"])
    encoded = encode(pixels, quality, speed)
    encoded_second = encode(pixels, quality, speed)
    if encoded != encoded_second:
        raise RuntimeError(f"nondeterministic encoding for {candidate['id']}")
    path = work / f"{candidate['id']}.avif"
    second_path = work / f"{candidate['id']}-second.avif"
    path.write_bytes(encoded)
    second_path.write_bytes(encoded_second)
    item, _ = extract_color_item(path)
    second_item, _ = extract_color_item(second_path)
    if item != second_item:
        raise RuntimeError(f"nondeterministic color item for {candidate['id']}")
    item_path = work / f"{candidate['id']}.obu"
    second_item_path = work / f"{candidate['id']}-second.obu"
    item_path.write_bytes(item)
    second_item_path.write_bytes(second_item)

    def trace_once(item_file: Path, ordinal: int) -> tuple[str, bytes]:
        yuv_path = work / f"{candidate['id']}-{ordinal}.yuv"
        result = run(
            [
                str(executable),
                "--input",
                str(item_file),
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
        return result.stdout, yuv_path.read_bytes()

    trace_a, yuv_a = trace_once(item_path, 1)
    trace_b, yuv_b = trace_once(second_item_path, 2)
    if trace_a != trace_b or yuv_a != yuv_b:
        raise RuntimeError(f"nondeterministic dav1d trace or YUV for {candidate['id']}")
    blocks, groups, entropy_count = parse_trace(trace_a)
    with Image.open(BytesIO(encoded)) as decoded:
        pillow_rgb = decoded.convert("RGB").tobytes()
    with Image.open(BytesIO(encoded_second)) as decoded:
        pillow_rgb_second = decoded.convert("RGB").tobytes()
    if pillow_rgb != pillow_rgb_second:
        raise RuntimeError(f"nondeterministic Pillow RGB decode for {candidate['id']}")
    portable_color = portable_color_reference(path)
    classification = classify(
        blocks,
        groups,
        yuv_a,
        portable_color,
        str(candidate["orientation"]),
    )
    if retain_dir is not None and classification["qualifies"]:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path.name).write_bytes(encoded)
    return {
        "id": candidate["id"],
        "family": candidate["family"],
        "family_index": candidate["family_index"],
        "candidate_index": candidate["candidate_index"],
        "orientation": candidate["orientation"],
        "seed": candidate["seed"],
        "quality": quality,
        "speed": speed,
        "input_rgb_sha256": sha256(pixels),
        "encoded_file_sha256": sha256(encoded),
        "encoded_file_sha256_second": sha256(encoded_second),
        "encoded_item_sha256": sha256(item),
        "encoded_item_sha256_second": sha256(second_item),
        "encoded_item_length": len(item),
        "pillow_rgb_sha256": sha256(pillow_rgb),
        "pillow_rgb_sha256_second": sha256(pillow_rgb_second),
        "dav1d_trace_sha256": sha256(trace_a.encode()),
        "dav1d_trace_sha256_second": sha256(trace_b.encode()),
        "decoded_yuv_sha256": sha256(yuv_a),
        "entropy_operation_count": entropy_count,
        "partition_blocks": blocks,
        "portable_color": portable_color,
        "repository_rust_invoked": False,
        **classification,
    }


def main() -> None:
    """Run the pinned 100-case input-only campaign."""

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

    with tempfile.TemporaryDirectory(prefix="image-star-avif-rect4-filter-") as name:
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

    rejection_reasons = sorted(
        reason for report in reports for reason in report["rejection_reasons"]
    )
    rejection_reasons = sorted(set(rejection_reasons))
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
            "quality": 12,
            "speed": 0,
            "max_threads": 1,
            "autotiling": False,
            "advanced": ADVANCED,
        },
        "search": {
            "input_only": True,
            "repository_rust_invoked": False,
            "candidate_count": len(reports),
            "family_count": len(FAMILY_NAMES),
            "candidates_per_family": 10,
            "candidates_per_orientation": {"h16x4": 50, "v4x16": 50},
            "seed_formula": "4000 + 100*family_index + candidate_index",
            "target_ids": ["cdf14_h16x4_false", "cdf19_v4x16_false"],
            "target": (
                "16x16 4:2:0 frames whose four DC luma terminals are either "
                "Horizontal16x4 with false filter CDF-index-14 decisions or "
                "Vertical4x16 with false filter CDF-index-19 decisions"
            ),
            "families": list(FAMILY_NAMES),
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "qualified_by_orientation": {
                orientation: sum(
                    report["qualifies"] and report["orientation"] == orientation
                    for report in reports
                )
                for orientation in ("h16x4", "v4x16")
            },
            "qualified_candidates": [
                report["id"] for report in reports if report["qualifies"]
            ],
            "by_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in rejection_reasons
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic rectangular filter-CDF traces: {args.output}")


if __name__ == "__main__":
    main()
