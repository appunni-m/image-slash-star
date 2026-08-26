#!/usr/bin/env python3
"""Search a fixed corpus for the AV1 Horizontal32x8 filter-intra CDF arm.

The campaign is deliberately bounded and input-driven.  It creates exactly
one hundred deterministic 32x32 RGB candidates in ten families, encodes each
candidate twice with the pinned Pillow/libavif/libaom oracle, and decodes the
color item twice through the independent scalar dav1d diagnostic build.
Candidates qualify only when the oracle exposes a Horizontal32x8 luma leaf,
codes DC luma, and selects the false ``use_filter_intra`` sentinel.  The
false decision proves the CDF-index-9 parser branch without claiming
filter-intra reconstruction.  No repository Rust code is invoked.

Generated AVIF files are temporary unless ``--retain-dir`` is supplied.  The
JSON report records every trace-derived predicate, rejection reason, hash,
and pinned provenance value.
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


SIZE = (32, 32)
SUBSAMPLING = "4:2:0"
TARGET_FILTER_CDF_INDEX = 9
FAMILY_NAMES = (
    "F01_known_h4_ripple",
    "F02_shifted_h4_ripple",
    "F03_horizontal_bands",
    "F04_banded_saw",
    "F05_banded_ramp",
    "F06_banded_texture",
    "F07_banded_chroma",
    "F08_banded_checker",
    "F09_banded_low_contrast",
    "F10_banded_mixed",
)
ADVANCED = {
    "min-partition-size": "8",
    "max-partition-size": "32",
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
YMODE_PATTERN = re.compile(r"^Post-ymode\[(?P<mode>\d+)\]")
UVMODE_PATTERN = re.compile(r"^Post-uvmode\[(?P<mode>\d+)\]")
YANGLE_PATTERN = re.compile(r"^Post-yangle-symbol\[(?P<symbol>\d+)\]")
UVANGLE_PATTERN = re.compile(r"^Post-uvangle-symbol\[(?P<symbol>\d+)\]")


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of ``data``."""

    return hashlib.sha256(data).hexdigest()


def clamp(value: int) -> int:
    """Clamp one generated RGB component to an 8-bit value."""

    return max(0, min(255, value))


def deterministic_noise(seed: int, count: int) -> list[int]:
    """Return a reproducible bounded integer stream for one family."""

    state = random.Random(seed)
    return [state.randrange(-12, 13) for _ in range(count)]


def h4_pattern(family: int, index: int) -> bytes:
    """Generate one horizontal-band RGB candidate.

    Each family keeps the strong four horizontal bands that produced the
    existing H4 witness, while changing only bounded in-band luma/chroma
    detail.  The first family/index pair is the declarative reproduction of
    the committed ``coverage_r32x8_h4_ripple_01`` input pattern.
    """

    seed = 7000 + 10 * family + index
    phase = (7 * family + 3 * index) % 31
    bases = (
        (48, 104, 160, 216),
        (40, 96, 152, 208),
        (32, 88, 144, 200),
        (56, 112, 168, 224),
        (24, 80, 136, 192),
        (64, 120, 176, 232),
        (48, 112, 176, 224),
        (36, 100, 164, 228),
        (72, 112, 152, 192),
        (44, 100, 156, 212),
    )[family]
    noise = deterministic_noise(seed, SIZE[0] * SIZE[1])
    pixels = bytearray()
    for y in range(SIZE[1]):
        band = min(3, y // 8)
        for x in range(SIZE[0]):
            sample = noise[y * SIZE[0] + x]
            base = bases[band]
            if family == 0:
                # This is the exact generator shape used by the committed
                # H4 witness when family=index=0 (phase is zero there).
                ripple = ((13 * x + 17 * y + x * y + phase) % 31) - 15
                red_delta = 8 if (x + y) % 3 else -8
                components = (
                    clamp(base + ripple + red_delta),
                    clamp(base + ripple),
                    clamp(base - ripple),
                )
            elif family == 1:
                ripple = ((11 * x + 19 * y + x * y + phase) % 29) - 14
                red_delta, green_delta, blue_delta = 10, sample // 2, -ripple
            elif family == 2:
                ripple = ((5 * x + 3 * y + phase) % 17) - 8
                red_delta, green_delta, blue_delta = ripple, 0, -ripple
            elif family == 3:
                ripple = ((x * x + 7 * y + phase) % 23) - 11
                red_delta, green_delta, blue_delta = sample, ripple, -sample
            elif family == 4:
                ripple = ((9 * x + 2 * y + phase) % 19) - 9
                red_delta, green_delta, blue_delta = ripple, ripple // 2, -ripple
            elif family == 5:
                ripple = sample + ((3 * x + y + phase) % 7) - 3
                red_delta, green_delta, blue_delta = ripple, 0, -ripple
            elif family == 6:
                ripple = ((7 * x + 5 * y + phase) % 21) - 10
                chroma = ((x * 5 + y * 3 + seed) % 17) - 8
                red_delta, green_delta, blue_delta = ripple + chroma, -chroma, -ripple
            elif family == 7:
                checker = 9 if ((x // 2) + (y // 2) + index) % 2 else -9
                ripple = checker
                red_delta, green_delta, blue_delta = checker, sample // 3, -checker
            elif family == 8:
                ripple = ((3 * x + 2 * y + phase) % 11) - 5
                red_delta, green_delta, blue_delta = ripple, 0, -ripple
            else:
                ripple = ((13 * x + 17 * y + x * y + phase) % 31) - 15
                red_delta, green_delta, blue_delta = ripple + sample // 2, sample, -ripple
            if family != 0:
                components = (
                    clamp(base + ripple + red_delta),
                    clamp(base + sample // 3 + green_delta),
                    clamp(base - ripple + blue_delta),
                )
            pixels.extend(components)
    return bytes(pixels)


def candidates() -> list[dict[str, object]]:
    """Return exactly ten deterministic families with ten cases each."""

    result = []
    for family, family_name in enumerate(FAMILY_NAMES):
        for index in range(10):
            result.append(
                {
                    "id": f"H32-F{family + 1:02d}-N{index:02d}",
                    "family": family_name,
                    "family_index": family,
                    "candidate_index": index,
                    "seed": 7000 + 10 * family + index,
                    "pixels": h4_pattern(family, index),
                    "quality": 76,
                    "speed": 0,
                }
            )
    if len(result) != 100:
        raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
    if {candidate["family"] for candidate in result} != set(FAMILY_NAMES):
        raise AssertionError("candidate corpus must contain all ten named families")
    if any(
        sum(candidate["family"] == family for candidate in result) != 10
        for family in FAMILY_NAMES
    ):
        raise AssertionError("each campaign family must contain exactly ten cases")
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
    """Parse partition blocks, leaf groups, and contiguous scalar entropy."""

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
    return blocks, groups, len(entropy)


def parse_group(group: list[str]) -> dict[str, object]:
    """Extract modes, filter decisions, and transform payloads from a leaf."""

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
    y_angles = [
        int(match["symbol"])
        for line in group
        if (match := YANGLE_PATTERN.match(line)) is not None
    ]
    uv_angles = [
        int(match["symbol"])
        for line in group
        if (match := UVANGLE_PATTERN.match(line)) is not None
    ]
    filter_decisions = []
    luma_payloads = []
    chroma_payloads = []
    for line in group:
        if match := FILTER_PATTERN.match(line):
            filter_decisions.append(
                {
                    "y_mode": int(match["y_mode"]),
                    "filter_mode": int(match["filter_mode"]),
                }
            )
        if match := LUMA_PATTERN.match(line):
            luma_payloads.append(
                {name: int(value) for name, value in match.groupdict().items()}
            )
        if match := CHROMA_PATTERN.match(line):
            chroma_payloads.append(
                {name: int(value) for name, value in match.groupdict().items()}
            )
    return {
        "y_modes": y_modes,
        "uv_modes": uv_modes,
        "y_angle_symbols": y_angles,
        "uv_angle_symbols": uv_angles,
        "filter_decisions": filter_decisions,
        "luma_payloads": luma_payloads,
        "chroma_payloads": chroma_payloads,
    }


def classify(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
) -> dict[str, object]:
    """Apply exact predicates for the H32x8 false-filter witness."""

    parsed = [parse_group(group) for group in groups]
    root = next(
        (
            block
            for block in blocks
            if block["level"] == 2 and block["x"] == 0 and block["y"] == 0
        ),
        None,
    )
    y_modes = [mode for group in parsed for mode in group["y_modes"]]
    uv_modes = [mode for group in parsed for mode in group["uv_modes"]]
    filter_decisions = [
        decision for group in parsed for decision in group["filter_decisions"]
    ]
    y_angle_symbols = [
        symbol for group in parsed for symbol in group["y_angle_symbols"]
    ]
    uv_angle_symbols = [
        symbol for group in parsed for symbol in group["uv_angle_symbols"]
    ]
    luma_groups = [group["luma_payloads"] for group in parsed]
    chroma_groups = [group["chroma_payloads"] for group in parsed]
    forbidden_prefixes = (
        "Post-y_pal[",
        "Post-pal[",
        "Post-y-pal-indices",
        "y-pal-pred",
        "Post-uv_pal[",
        "Post-uv-pal-indices",
        "uv-pal-pred",
    )
    no_palette = not any(
        line.startswith(forbidden_prefixes)
        for group in groups
        for line in group
    )

    def is_h32x8_luma(payloads: list[dict[str, int]]) -> bool:
        return (
            len(payloads) == 1
            and payloads[0]["tx"] == 16
            and payloads[0]["txtp"] == 0
            and payloads[0]["eob"] >= 0
        )

    def is_h16x4_chroma(payloads: list[dict[str, int]]) -> bool:
        return (
            len(payloads) == 2
            and {payload["plane"] for payload in payloads} == {0, 1}
            and all(
                payload["tx"] == 14 and payload["txtp"] == 0
                for payload in payloads
            )
        )

    y_plane_length = SIZE[0] * SIZE[1]
    y_plane = yuv[:y_plane_length]
    predicates = {
        "frame_is_32x32_8bit_420": (
            portable_color.get("width") == SIZE[0]
            and portable_color.get("height") == SIZE[1]
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "root_is_partition_h4": (
            root is not None
            and root["partition"] == 8
            and root["level"] == 2
            and root["x"] == 0
            and root["y"] == 0
        ),
        "four_h32x8_terminal_groups": len(groups) == 4,
        "all_terminal_luma_modes_are_dc": (
            len(groups) == 4 and y_modes == [0, 0, 0, 0]
        ),
        "all_filter_decisions_are_false_sentinel": (
            len(groups) == 4
            and filter_decisions == [{"y_mode": 0, "filter_mode": 0}] * 4
        ),
        "no_palette_syntax": no_palette,
        "no_luma_or_chroma_angle_symbols": (
            y_angle_symbols == [] and uv_angle_symbols == []
        ),
        "all_terminal_chroma_modes_are_dc": (
            len(groups) == 4 and uv_modes == [0, 0, 0, 0]
        ),
        "all_luma_transforms_are_h32x8_dct": (
            len(luma_groups) == 4 and all(is_h32x8_luma(payloads) for payloads in luma_groups)
        ),
        "all_chroma_transforms_are_h16x4_dct": (
            len(chroma_groups) == 4 and all(is_h16x4_chroma(payloads) for payloads in chroma_groups)
        ),
        "at_least_one_nonempty_luma_residual": any(
            payload["eob"] >= 1
            for payloads in luma_groups
            for payload in payloads
        ),
        "visible_luma_varies": len(y_plane) == y_plane_length and len(set(y_plane)) > 1,
    }
    return {
        "target": "h32x8_filter_intra_cdf9_false",
        "target_filter_cdf_index": TARGET_FILTER_CDF_INDEX,
        "root_partition": root,
        "group_count": len(groups),
        "y_modes": y_modes,
        "uv_modes": uv_modes,
        "filter_decisions": filter_decisions,
        "y_angle_symbols": y_angle_symbols,
        "uv_angle_symbols": uv_angle_symbols,
        "luma_payloads": luma_groups,
        "chroma_payloads": chroma_groups,
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
    if retain_dir is not None:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path.name).write_bytes(encoded)
    item, _ = extract_color_item(path)
    second_item, _ = extract_color_item(second_path)
    if item != second_item:
        raise RuntimeError(f"nondeterministic color item for {candidate['id']}")
    portable_color = portable_color_reference(path)
    item_path = work / f"{candidate['id']}.obu"
    item_path.write_bytes(item)

    def trace_once(ordinal: int) -> tuple[str, bytes]:
        yuv_path = work / f"{candidate['id']}-{ordinal}.yuv"
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
        return result.stdout, yuv_path.read_bytes()

    trace_a, yuv_a = trace_once(1)
    trace_b, yuv_b = trace_once(2)
    if trace_a != trace_b or yuv_a != yuv_b:
        raise RuntimeError(f"nondeterministic dav1d trace or YUV for {candidate['id']}")
    blocks, groups, entropy_count = parse_trace(trace_a)
    with Image.open(BytesIO(encoded)) as decoded:
        pillow_rgb = decoded.convert("RGB").tobytes()
    with Image.open(BytesIO(encoded_second)) as decoded:
        pillow_rgb_second = decoded.convert("RGB").tobytes()
    if pillow_rgb != pillow_rgb_second:
        raise RuntimeError(f"nondeterministic Pillow RGB decode for {candidate['id']}")
    y_length = SIZE[0] * SIZE[1]
    chroma_length = (SIZE[0] // 2) * (SIZE[1] // 2)
    expected_yuv_length = y_length + 2 * chroma_length
    if len(yuv_a) != expected_yuv_length:
        raise RuntimeError(
            f"unexpected 4:2:0 YUV length for {candidate['id']}: {len(yuv_a)}"
        )
    y_plane = yuv_a[:y_length]
    u_plane = yuv_a[y_length : y_length + chroma_length]
    v_plane = yuv_a[y_length + chroma_length :]
    return {
        "id": candidate["id"],
        "family": candidate["family"],
        "family_index": candidate["family_index"],
        "candidate_index": candidate["candidate_index"],
        "seed": candidate["seed"],
        "quality": quality,
        "speed": speed,
        "input_rgb_sha256": sha256(pixels),
        "encoded_file_sha256": sha256(encoded),
        "encoded_file_second_sha256": sha256(encoded_second),
        "encoded_item_sha256": sha256(item),
        "encoded_item_second_sha256": sha256(second_item),
        "encoded_item_length": len(item),
        "pillow_rgb_sha256": sha256(pillow_rgb),
        "pillow_rgb_second_sha256": sha256(pillow_rgb_second),
        "dav1d_trace_sha256": sha256(trace_a.encode()),
        "dav1d_trace_second_sha256": sha256(trace_b.encode()),
        "decoded_yuv_sha256": sha256(yuv_a),
        "decoded_y_plane_sha256": sha256(y_plane),
        "decoded_u_plane_sha256": sha256(u_plane),
        "decoded_v_plane_sha256": sha256(v_plane),
        "entropy_operation_count": entropy_count,
        "partition_blocks": blocks,
        **classify(blocks, groups, yuv_a, portable_color),
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

    with tempfile.TemporaryDirectory(prefix="image-star-avif-h32x8-filter-") as name:
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

    rejection_reasons = (
        "frame_is_32x32_8bit_420",
        "root_is_partition_h4",
        "four_h32x8_terminal_groups",
        "all_terminal_luma_modes_are_dc",
        "all_filter_decisions_are_false_sentinel",
        "no_palette_syntax",
        "no_luma_or_chroma_angle_symbols",
        "all_terminal_chroma_modes_are_dc",
        "all_luma_transforms_are_h32x8_dct",
        "all_chroma_transforms_are_h16x4_dct",
        "at_least_one_nonempty_luma_residual",
        "visible_luma_varies",
    )
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
            "quality": 76,
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
            "target_id": "h32x8_filter_intra_cdf9_false",
            "target": (
                "32x32 4:2:0 PARTITION_H4 frame with four terminal "
                "Horizontal32x8 profile; every coded luma leaf is DC, the "
                "derived use_filter_intra CDF index is 9, and the decision is false"
            ),
            "filter_cdf_index": TARGET_FILTER_CDF_INDEX,
            "families": list(FAMILY_NAMES),
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "qualified_by_family": {
                family: sum(
                    report["qualifies"] and report["family"] == family
                    for report in reports
                )
                for family in FAMILY_NAMES
            },
            "by_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in rejection_reasons
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic H32x8 filter-intra traces: {args.output}")


if __name__ == "__main__":
    main()
