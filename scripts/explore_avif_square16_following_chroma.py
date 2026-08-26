#!/usr/bin/env python3
"""Search bounded following-Square16 4:2:0 chroma reconstruction inputs.

The campaign generates one hundred deterministic 32x16 RGB candidates, encodes
each twice through the pinned Pillow/libavif/libaom oracle, and traces each
color item twice through scalar dav1d.  It promotes only a right-hand
Square16 leaf whose coded chroma mode is SmoothHorizontal, whose transform and
residual shape are explicit, and whose complete syntax/trace/YUV/RGB evidence
is deterministic.  Repository Rust is never invoked by the search.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from explore_avif_chroma_diagonal113 import parse_trace
from generate_av1_reconstruction_refs import (
    DAV1D_COMMIT,
    build_dav1d,
    extract_color_item,
    portable_color_reference,
    resolve_tool,
    run,
    verify_source,
)


SIZE = (32, 16)
SUBSAMPLING = "4:2:0"
QUALITY = 76
SPEED = 0
ADVANCED = {
    "min-partition-size": "16",
    "max-partition-size": "16",
    "use-intra-dct-only": "0",
    "enable-filter-intra": "0",
    "enable-intra-edge-filter": "0",
    "enable-smooth-intra": "1",
    "enable-paeth-intra": "0",
    "enable-directional-intra": "0",
    "enable-cfl-intra": "0",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "loopfilter-control": "0",
    "aq-mode": "0",
    "deltaq-mode": "0",
}
FAMILY_NAMES = (
    "positive_row_ramp",
    "negative_row_ramp",
    "two_level_step",
    "four_level_staircase",
    "vertical_saw",
    "low_frequency_ripple",
    "alternating_bands",
    "opposed_uv_rows",
    "edge_biased_rows",
    "asymmetric_texture",
)
BLOCK_PATTERN = re.compile(
    r"^poc=(?P<poc>-?\d+),y=(?P<y>-?\d+),x=(?P<x>-?\d+),"
    r"bl=(?P<level>\d+),ctx=(?P<context>\d+),bp=(?P<partition>\d+): "
    r"r=(?P<range>\d+)$"
)
LUMA_MODE_PATTERN = re.compile(r"^Post-ymode\[(?P<mode>\d+)\]")
UV_MODE_PATTERN = re.compile(r"^Post-uvmode\[(?P<mode>\d+)\]")
LUMA_PATTERN = re.compile(
    r"^Post-y-cf-blk\[tx=(?P<tx>\d+),txtp=(?P<txtp>-?\d+),"
    r"eob=(?P<eob>-?\d+)\]"
)
CHROMA_PATTERN = re.compile(
    r"^Post-uv-cf-blk\[pl=(?P<plane>\d+),tx=(?P<tx>\d+),"
    r"txtp=(?P<txtp>-?\d+),eob=(?P<eob>-?\d+)\].*cbx4=(?P<cbx4>\d+)"
)
ANGLE_PATTERN = re.compile(r"^Post-(?:y|uv)angle-symbol\[")


def sha256(data: bytes) -> str:
    """Return a lowercase SHA-256 digest."""

    return hashlib.sha256(data).hexdigest()


def clamp(value: int) -> int:
    """Clamp one generated component to the eight-bit range."""

    return max(0, min(255, value))


def yuv_to_rgb(y: int, u: int, v: int) -> tuple[int, int, int]:
    """Convert a bounded full-range synthetic YUV sample to RGB."""

    du = u - 128
    dv = v - 128
    return (
        clamp(y + (358 * dv + 128) // 256),
        clamp(y - (88 * du + 183 * dv + 128) // 256),
        clamp(y + (453 * du + 128) // 256),
    )


def row_signal(family: int, index: int, row: int) -> int:
    """Return a deterministic row signal for one chroma family."""

    phase = (3 * family + index) % 8
    if family == 0:
        return (row - 3) * (8 + index % 4)
    if family == 1:
        return -(row - 3) * (8 + index % 4)
    if family == 2:
        return 28 if row < 4 else -28
    if family == 3:
        return (row // 2 - 2) * 13
    if family == 4:
        return ((5 * row + phase) % 9 - 4) * (7 + index % 3)
    if family == 5:
        return (((row * 7 + phase) % 16) - 8) * (3 + index % 3)
    if family == 6:
        return (22 if (row + phase) % 2 == 0 else -22) + (index % 3 - 1) * 3
    if family == 7:
        return (row - 3) * (6 + index % 3)
    if family == 8:
        return (3 - abs(row - (2 + index % 4))) * (8 + index % 3)
    return (((11 * row + 5 * index + phase) % 17) - 8) * 3


def chroma_sample(family: int, index: int, cx: int, cy: int) -> tuple[int, int]:
    """Return U/V deltas with a smooth-horizontal continuation bias."""

    left = row_signal(family, index, cy)
    origin_first = row_signal(family, index, 0)
    if cx < 8:
        horizontal = (cx - 7) * (1 + family % 3)
        u = left + horizontal
        v = left - horizontal if family in (1, 4, 7) else left + horizontal // 2
    else:
        step = cx - 8
        continuation = left + ((origin_first - left) * step + 3) // 7
        ripple = ((cx + 2 * cy + index + family) % 3) - 1
        if family in (2, 6):
            ripple *= 2
        u = continuation + ripple
        v = continuation - ripple if family in (1, 4, 7) else continuation + ripple // 2
    return u, v


def candidate_pixels(family: int, index: int) -> bytes:
    """Create one deterministic RGB candidate from a synthetic YUV field."""

    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            cx, cy = x // 2, y // 2
            u_delta, v_delta = chroma_sample(family, index, cx, cy)
            luma = 128
            if family in (5, 9):
                luma += ((7 * x + 11 * y + index + family) % 7) - 3
            pixels.extend(yuv_to_rgb(luma, 128 + u_delta, 128 + v_delta))
    return bytes(pixels)


def candidates() -> list[dict[str, object]]:
    """Return ten deterministic families with ten cases each."""

    result = []
    for family, family_name in enumerate(FAMILY_NAMES):
        for index in range(10):
            result.append(
                {
                    "id": f"SF16-F{family + 1:02d}-N{index:02d}",
                    "family": family_name,
                    "family_index": family,
                    "candidate_index": index,
                    "seed": 8000 + 10 * family + index,
                    "pixels": candidate_pixels(family, index),
                    "quality": QUALITY,
                    "speed": SPEED,
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


def parse_group(group: list[str]) -> dict[str, object]:
    """Extract predictor modes, angles, and coefficient payloads."""

    y_modes = [
        int(match["mode"])
        for line in group
        if (match := LUMA_MODE_PATTERN.match(line)) is not None
    ]
    uv_modes = [
        int(match["mode"])
        for line in group
        if (match := UV_MODE_PATTERN.match(line)) is not None
    ]
    luma_payloads = []
    chroma_payloads = []
    for line in group:
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
        "luma_payloads": luma_payloads,
        "chroma_payloads": chroma_payloads,
        "angle_lines": [line for line in group if ANGLE_PATTERN.match(line)],
    }


def plane_edge(yuv: bytes, plane: int) -> list[int]:
    """Extract the origin leaf's right edge from a flat 4:2:0 YUV frame."""

    y_length = SIZE[0] * SIZE[1]
    chroma_length = (SIZE[0] // 2) * (SIZE[1] // 2)
    if plane == 0:
        offset, width, height, x = 0, SIZE[0], SIZE[1], 15
    elif plane == 1:
        offset, width, height, x = y_length, SIZE[0] // 2, SIZE[1] // 2, 7
    else:
        offset, width, height, x = y_length + chroma_length, SIZE[0] // 2, SIZE[1] // 2, 7
    return [yuv[offset + row * width + x] for row in range(height)]


def classify(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
) -> dict[str, object]:
    """Apply fixed predicates for the following SmoothHorizontal class."""

    parsed = [parse_group(group) for group in groups]
    shape = [
        (
            block["poc"],
            block["x"],
            block["y"],
            block["level"],
            block["context"],
            block["partition"],
        )
        for block in blocks
    ]
    y_modes = [mode for group in parsed for mode in group["y_modes"]]
    uv_modes = [mode for group in parsed for mode in group["uv_modes"]]
    y_payloads = [group["luma_payloads"] for group in parsed]
    chroma_payloads = [group["chroma_payloads"] for group in parsed]
    forbidden_prefixes = (
        "Post-filterintramode[",
        "Post-y_pal[",
        "Post-pal[",
        "Post-y-pal-indices",
        "y-pal-pred",
        "Post-uv_pal[",
        "Post-uv-pal-indices",
        "uv-pal-pred",
    )
    all_lines = [line for group in groups for line in group]
    u_edge = plane_edge(yuv, 1) if len(yuv) == 768 else []
    v_edge = plane_edge(yuv, 2) if len(yuv) == 768 else []

    def is_tx16x16(payloads: list[dict[str, int]]) -> bool:
        return (
            len(payloads) == 1
            and payloads[0]["tx"] == 2
            and payloads[0]["txtp"] == 0
            and payloads[0]["eob"] >= 1
        )

    def is_chroma_tx8x8(payloads: list[dict[str, int]], transform: int, cbx4: int) -> bool:
        return (
            len(payloads) == 2
            and {payload["plane"] for payload in payloads} == {0, 1}
            and all(
                payload["tx"] == 1
                and payload["txtp"] == transform
                and payload["eob"] >= 1
                and payload["cbx4"] == cbx4
                for payload in payloads
            )
        )

    predicates = {
        "exact_clipped_split_shape": shape == [
            (0, 0, 0, 2, 0, 3),
            (0, 0, 0, 3, 0, 0),
            (0, 4, 0, 3, 0, 0),
        ],
        "eight_bit_420_frame": (
            portable_color.get("width") == SIZE[0]
            and portable_color.get("height") == SIZE[1]
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "two_visible_square16_groups": len(groups) == 2,
        "both_luma_modes_dc": y_modes == [0, 0],
        "following_uv_mode_smooth_horizontal": uv_modes == [0, 11],
        "no_angle_symbols": not any(group["angle_lines"] for group in parsed),
        "no_palette_or_filter_intra": not any(
            line.startswith(forbidden_prefixes) for line in all_lines
        ),
        "both_luma_are_unsplit_tx16x16": (
            len(y_payloads) == 2 and all(is_tx16x16(payloads) for payloads in y_payloads)
        ),
        "left_chroma_is_dct_dct_tx8x8": (
            len(chroma_payloads) == 2 and is_chroma_tx8x8(chroma_payloads[0], 0, 0)
        ),
        "right_chroma_is_dct_adst_tx8x8": (
            len(chroma_payloads) == 2 and is_chroma_tx8x8(chroma_payloads[1], 2, 2)
        ),
        "right_chroma_has_ac": (
            len(chroma_payloads) == 2
            and all(payload["eob"] >= 1 for payload in chroma_payloads[1])
        ),
        "decoded_yuv_has_expected_size": len(yuv) == 768,
        "origin_u_edge_varies": len(u_edge) == 8 and len(set(u_edge)) > 1,
        "origin_v_edge_varies": len(v_edge) == 8 and len(set(v_edge)) > 1,
        "origin_chroma_edge_is_not_midpoint": (
            bool(u_edge) and bool(v_edge) and (u_edge[0] != 128 or v_edge[0] != 128)
        ),
    }
    return {
        "root_partition": blocks[0] if blocks else None,
        "partition_shape": shape,
        "group_count": len(groups),
        "y_modes": y_modes,
        "uv_modes": uv_modes,
        "origin_u_right_edge": u_edge,
        "origin_v_right_edge": v_edge,
        "left_luma_payloads": y_payloads[0] if y_payloads else [],
        "right_luma_payloads": y_payloads[1] if len(y_payloads) == 2 else [],
        "left_chroma_payloads": chroma_payloads[0] if chroma_payloads else [],
        "right_chroma_payloads": chroma_payloads[1] if len(chroma_payloads) == 2 else [],
        "predicates": predicates,
        "rejection_reasons": [name for name, passed in predicates.items() if not passed],
        "qualifies": all(predicates.values()),
    }


def trace_once(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    item: bytes,
    stem: str,
    ordinal: int,
) -> tuple[str, list[dict[str, int]], list[list[str]], int, bytes]:
    """Trace one color item through scalar dav1d."""

    item_path = work / f"{stem}-{ordinal}.obu"
    yuv_path = work / f"{stem}-{ordinal}.yuv"
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
    return result.stdout, blocks, groups, entropy_count, yuv_path.read_bytes()


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
    encoded_a = encode(pixels, int(candidate["quality"]), int(candidate["speed"]))
    encoded_b = encode(pixels, int(candidate["quality"]), int(candidate["speed"]))
    path_a = work / f"{candidate['id']}.avif"
    path_b = work / f"{candidate['id']}-second.avif"
    path_a.write_bytes(encoded_a)
    path_b.write_bytes(encoded_b)
    item_a, _ = extract_color_item(path_a)
    item_b, _ = extract_color_item(path_b)
    trace_a, blocks_a, groups_a, entropy_a, yuv_a = trace_once(
        executable, environment, work, item_a, str(candidate["id"]), 1
    )
    trace_b, blocks_b, groups_b, entropy_b, yuv_b = trace_once(
        executable, environment, work, item_b, str(candidate["id"]), 2
    )
    portable_color = portable_color_reference(path_a)
    with Image.open(BytesIO(encoded_a)) as decoded:
        pillow_rgb_a = decoded.convert("RGB").tobytes()
    with Image.open(BytesIO(encoded_b)) as decoded:
        pillow_rgb_b = decoded.convert("RGB").tobytes()
    classification = classify(blocks_a, groups_a, yuv_a, portable_color)
    classification["predicates"].update(
        {
            "double_encode_equal": encoded_a == encoded_b,
            "double_color_item_equal": item_a == item_b,
            "double_trace_equal": (
                trace_a == trace_b
                and blocks_a == blocks_b
                and groups_a == groups_b
                and yuv_a == yuv_b
            ),
            "double_pillow_rgb_equal": pillow_rgb_a == pillow_rgb_b,
        }
    )
    classification["rejection_reasons"] = [
        name for name, passed in classification["predicates"].items() if not passed
    ]
    classification["qualifies"] = all(classification["predicates"].values())
    if classification["qualifies"] and retain_dir is not None:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path_a.name).write_bytes(encoded_a)
    return {
        "id": candidate["id"],
        "family": candidate["family"],
        "seed": candidate["seed"],
        "quality": candidate["quality"],
        "speed": candidate["speed"],
        "input_rgb_sha256": sha256(pixels),
        "encoded_file_sha256": sha256(encoded_a),
        "encoded_file_sha256_second": sha256(encoded_b),
        "encoded_item_sha256": sha256(item_a),
        "encoded_item_sha256_second": sha256(item_b),
        "encoded_item_length": len(item_a),
        "pillow_rgb_sha256": sha256(pillow_rgb_a),
        "pillow_rgb_sha256_second": sha256(pillow_rgb_b),
        "dav1d_trace_sha256": sha256(trace_a.encode()),
        "dav1d_trace_sha256_second": sha256(trace_b.encode()),
        "decoded_yuv_sha256": sha256(yuv_a),
        "decoded_yuv_sha256_second": sha256(yuv_b),
        "decoded_y_plane_sha256": sha256(yuv_a[: SIZE[0] * SIZE[1]]),
        "decoded_u_plane_sha256": sha256(
            yuv_a[SIZE[0] * SIZE[1] : SIZE[0] * SIZE[1] + 128]
        ),
        "decoded_v_plane_sha256": sha256(yuv_a[-128:]),
        "entropy_operation_count": entropy_a,
        "entropy_operation_count_second": entropy_b,
        "partition_blocks": blocks_a,
        "partition_blocks_second": blocks_b,
        "portable_color": portable_color,
        **classification,
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
    with tempfile.TemporaryDirectory(prefix="image-star-avif-square16-following-") as name:
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
                broaden_vertical_following=False,
                broaden_horizontal_square16=True,
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
            "quality": QUALITY,
            "speed": SPEED,
            "max_threads": 1,
            "autotiling": False,
            "advanced": ADVANCED,
        },
        "search": {
            "candidate_count": len(reports),
            "seed_formula": "8000 + 10*family_index + candidate_index",
            "target_id": "following_square16_chroma_smooth_horizontal",
            "target": (
                "32x16 8-bit 4:2:0 clipped 32x32 root split with two visible "
                "Square16 leaves; following UV mode 11 SmoothHorizontal, "
                "DCT-ADST TX8x8 U/V, non-empty AC"
            ),
            "families": list(FAMILY_NAMES),
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "by_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in reports[0]["predicates"]
            },
        },
        "qualified_candidates": [report["id"] for report in reports if report["qualifies"]],
        "promoted_candidate": next(
            (report["id"] for report in reports if report["qualifies"]), None
        ),
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic following-Square16 traces: {args.output}")


if __name__ == "__main__":
    main()
