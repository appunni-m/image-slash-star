#!/usr/bin/env python3
"""Search a bounded vertical-following AVIF Diagonal67 corpus.

The campaign creates exactly one hundred deterministic 8x16 RGB candidates in
ten named families. Each candidate is encoded twice with the pinned
Pillow/libavif/libaom oracle and decoded twice through an independently
instrumented scalar dav1d build. The default chroma target qualifies a clipped
16x16 split with a vertically following bottom Square8 leaf, coded chroma
Diagonal67 mode 8, ADST-DCT 4x4 U/V transforms, and the required non-empty
residuals. The luma target qualifies the same topology with coded luma
Diagonal67 mode 8, a genuine Zone-1 edge, and skipped chroma. Repository Rust
is never invoked during the search.
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


SIZE = (8, 16)
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
LUMA_ADVANCED = {
    **ADVANCED,
    "use-intra-dct-only": "1",
}
FAMILY_NAMES = (
    "positive_diagonal_ramp",
    "negative_diagonal_ramp",
    "two_level_step",
    "four_level_staircase",
    "low_frequency_wave",
    "sawtooth_rows",
    "uv_antiphase",
    "u_dominant",
    "v_dominant",
    "dual_plane_dither",
)

BLOCK_PATTERN = re.compile(
    r"^poc=(?P<poc>-?\d+),y=(?P<y>-?\d+),x=(?P<x>-?\d+),"
    r"bl=(?P<level>\d+),ctx=(?P<context>\d+),bp=(?P<partition>\d+): "
    r"r=(?P<range>\d+)$"
)
LUMA_MODE_PATTERN = re.compile(r"^Post-ymode\[(?P<mode>\d+)\]")
UV_MODE_PATTERN = re.compile(r"^Post-uvmode\[(?P<mode>\d+)\]")
LUMA_ANGLE_PATTERN = re.compile(r"^Post-yangle-symbol\[(?P<symbol>\d+)\]")
UV_ANGLE_PATTERN = re.compile(r"^Post-uvangle-symbol\[(?P<symbol>\d+)\]")
LUMA_PATTERN = re.compile(
    r"^Post-y-cf-blk\[tx=(?P<tx>\d+),txtp=(?P<txtp>-?\d+),"
    r"eob=(?P<eob>-?\d+)\]"
)
CHROMA_PATTERN = re.compile(
    r"^Post-uv-cf-blk\[pl=(?P<plane>\d+),tx=(?P<tx>\d+),"
    r"txtp=(?P<txtp>-?\d+),eob=(?P<eob>-?\d+)\]"
)
CHROMA_LOCATION_PATTERN = re.compile(r"cbx4=(?P<cbx4>\d+)")


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of ``data``."""

    return hashlib.sha256(data).hexdigest()


def clamp(value: int) -> int:
    """Clamp one generated component to the eight-bit range."""

    return max(0, min(255, value))


def yuv_to_rgb(y: int, u: int, v: int) -> tuple[int, int, int]:
    """Convert one bounded full-range synthetic YUV sample to RGB."""

    du = u - 128
    dv = v - 128
    return (
        clamp(y + (358 * dv + 128) // 256),
        clamp(y - (88 * du + 183 * dv + 128) // 256),
        clamp(y + (453 * du + 128) // 256),
    )


def edge_signal(family: int, index: int, x: int, plane: int) -> int:
    """Return a varying four-sample top-leaf edge for one chroma plane."""

    phase = (7 * family + 11 * index + 3 * plane) % 16
    amplitude = 10 + (index % 5) * 2
    coordinate = x + phase
    if family == 0:
        value = (x - 1) * amplitude
    elif family == 1:
        value = (2 - x) * amplitude
    elif family == 2:
        value = amplitude if x < 2 else -amplitude
    elif family == 3:
        value = (x // 1 - 1) * (amplitude // 2 + 3)
    elif family == 4:
        value = (((coordinate * 3) % 11) - 5) * 3
    elif family == 5:
        value = (((coordinate * 5) % 13) - 6) * 3
    elif family == 6:
        value = (amplitude if (x + phase) % 2 == 0 else -amplitude)
    elif family == 7:
        value = (x - 1) * (amplitude + 2)
    elif family == 8:
        value = (2 - x) * (amplitude + 2)
    else:
        value = (((coordinate * 7 + index) % 15) - 7) * 2
    if plane == 1:
        value = -value if family in (1, 6, 8) else value + (x - 1)
    return value


def interpolate_edge(edge: list[int], position_q6: int) -> int:
    """Interpolate a top edge using a q6 position and a repeated final sample."""

    if not edge:
        raise ValueError("a top edge is required")
    if position_q6 <= 0:
        return edge[0]
    last = len(edge) - 1
    index = position_q6 // 64
    if index >= last:
        return edge[last]
    fraction = position_q6 % 64
    left = edge[index]
    right = edge[index + 1]
    return left + ((right - left) * fraction + 32) // 64


def chroma_sample(family: int, index: int, cx: int, cy: int) -> tuple[int, int]:
    """Build top-edge data and a Zone-1 angle-67 bottom continuation."""

    top_edges = [
        [edge_signal(family, index, x, plane) for x in range(4)]
        for plane in (0, 1)
    ]
    if cy < 4:
        # The last row is the reference edge. Earlier rows add a small
        # deterministic vertical residual without changing that edge.
        vertical = (cy - 3) * (1 + (family + index) % 3)
        u = top_edges[0][cx] + vertical
        v = top_edges[1][cx] - vertical
    else:
        local_y = cy - 4
        position_q6 = cx * 64 + (local_y + 1) * 27
        u = interpolate_edge(top_edges[0], position_q6)
        v = interpolate_edge(top_edges[1], position_q6)
        perturbation = ((3 * cx + 5 * local_y + 7 * family + index) % 5) - 2
        if family in (2, 5):
            perturbation *= 2
        if family == 6:
            u += 3 * perturbation
            v -= 3 * perturbation
        elif family == 7:
            u += 4 * perturbation
            v += 2 * perturbation
        elif family == 8:
            u += 2 * perturbation
            v -= 4 * perturbation
        else:
            u += 3 * perturbation
            v += 2 * perturbation
    return u, v


def luma_edge_value(family: int, index: int, x: int) -> int:
    """Return the varying bottom edge exposed by the top Square8 leaf."""

    del index
    slopes = (3, -3, 2, 3, 3, 6, -2, 4, -4, 3)
    return 80 + slopes[family] * x


def luma_residual(family: int, x: int, local_y: int) -> int:
    """Return a bounded residual that retains split 4x4 luma syntax."""

    if family == 0:
        return 16 if (x, local_y) == (1, 0) else 0
    if family == 1:
        return 5 if x < 4 and local_y < 4 and (x + local_y) % 2 == 0 else -5 if x < 4 and local_y < 4 else 0
    if family == 2:
        return 5 if x < 4 and local_y < 4 and (x + local_y) % 2 == 0 else -5 if x < 4 and local_y < 4 else 0
    if family == 3:
        return 8 * ((x + local_y) % 3 - 1) if x < 4 and local_y < 4 else 0
    if family == 4:
        return 4 if x < 4 and local_y < 4 and (x // 2 + local_y // 2) % 2 == 0 else -4 if x < 4 and local_y < 4 else 0
    if family == 5:
        return 5 if x < 4 and local_y < 4 and x % 2 == 0 else -5 if x < 4 and local_y < 4 else 0
    if family == 6:
        return 20 if x < 4 and local_y < 4 else 0
    if family == 7:
        return 10 if x < 4 and local_y < 4 else 0
    if family == 8:
        return 5 if x < 4 and local_y < 4 and x % 2 == 0 else -5 if x < 4 and local_y < 4 else 0
    return 5 if x < 4 and local_y < 4 and (x + local_y) % 2 == 0 else -5 if x < 4 and local_y < 4 else 0


def luma_sample(family: int, index: int, x: int, y: int, target: str) -> int:
    """Build a top-edge-continuous luma field for the selected target."""

    if y < 8:
        value = luma_edge_value(family, index, x) + (y - 7) * ((family + index) % 2)
    else:
        local_y = y - 8
        if target == "luma_diagonal67":
            # D67 is near vertical: each lower row advances only part of one
            # top-edge sample.  The encoder sees a real top edge here.
            source_x = min(7, x + (local_y + 1) // 2)
        else:
            source_x = min(7, x + local_y + 1)
        value = luma_edge_value(family, index, source_x)
        if target == "chroma_diagonal67":
            value += luma_residual(family, x, local_y)
        else:
            value += ((3 * x + 5 * local_y + 7 * family + index) % 7) - 3
    return value


def candidate_pixels(family: int, index: int, target: str) -> bytes:
    """Create one deterministic 8x16 RGB candidate from synthetic YUV."""

    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if target == "luma_diagonal67":
                u_delta = v_delta = 0
            else:
                u_delta, v_delta = chroma_sample(family, index, x // 2, y // 2)
            pixels.extend(
                yuv_to_rgb(
                    luma_sample(family, index, x, y, target),
                    128 + u_delta,
                    128 + v_delta,
                )
            )
    return bytes(pixels)


def candidates(target: str) -> list[dict[str, object]]:
    """Return exactly ten deterministic families with ten cases each."""

    result = []
    for family, family_name in enumerate(FAMILY_NAMES):
        for index in range(10):
            result.append(
                {
                    "id": f"D67V-F{family + 1:02d}-N{index:02d}",
                    "family": family_name,
                    "family_index": family,
                    "candidate_index": index,
                    "seed": 12000 + 10 * family + index,
                    "pixels": candidate_pixels(family, index, target),
                    "quality": 76,
                    "speed": 0,
                }
            )
    if len(result) != 100:
        raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
    return result


def encode(pixels: bytes, quality: int, speed: int, target: str) -> bytes:
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
        advanced=LUMA_ADVANCED if target == "luma_diagonal67" else ADVANCED,
    )
    return output.getvalue()


def parse_group(group: list[str]) -> dict[str, object]:
    """Extract modes, angle symbols, and coefficient payloads from a leaf."""

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
    y_angle_symbols = [
        int(match["symbol"])
        for line in group
        if (match := LUMA_ANGLE_PATTERN.match(line)) is not None
    ]
    uv_angle_symbols = [
        int(match["symbol"])
        for line in group
        if (match := UV_ANGLE_PATTERN.match(line)) is not None
    ]
    luma_payloads = []
    chroma_payloads = []
    dq_matrices = []
    for index, line in enumerate(group):
        if line != "dq":
            continue
        matrix = []
        for row in group[index + 1 :]:
            values = row.split()
            if not values:
                break
            try:
                matrix.extend(int(value) for value in values)
            except ValueError:
                break
        if matrix:
            dq_matrices.append(matrix)
    for line in group:
        if match := LUMA_PATTERN.match(line):
            luma_payloads.append(
                {name: int(value) for name, value in match.groupdict().items()}
            )
        if match := CHROMA_PATTERN.match(line):
            payload = {name: int(value) for name, value in match.groupdict().items()}
            if location := CHROMA_LOCATION_PATTERN.search(line):
                payload["cbx4"] = int(location["cbx4"])
            chroma_payloads.append(payload)
    return {
        "y_modes": y_modes,
        "uv_modes": uv_modes,
        "y_angle_symbols": y_angle_symbols,
        "uv_angle_symbols": uv_angle_symbols,
        "luma_payloads": luma_payloads,
        "chroma_payloads": chroma_payloads,
        "dq_matrices": dq_matrices,
    }


def top_edge(yuv: bytes, plane: int) -> list[int]:
    """Extract the top leaf's bottom chroma edge from packed 4:2:0 YUV."""

    y_length = SIZE[0] * SIZE[1]
    chroma_length = (SIZE[0] // 2) * (SIZE[1] // 2)
    if plane == 0:
        offset = y_length
    elif plane == 1:
        offset = y_length + chroma_length
    else:
        raise ValueError(f"unsupported plane {plane}")
    width = SIZE[0] // 2
    return [yuv[offset + 3 * width + x] for x in range(width)]


def classify_luma(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
    angle_symbol: int,
) -> dict[str, object]:
    """Apply exact predicates for one vertical-following luma D67 angle."""

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
    y_angles = [symbol for group in parsed for symbol in group["y_angle_symbols"]]
    uv_modes = [mode for group in parsed for mode in group["uv_modes"]]
    uv_angles = [symbol for group in parsed for symbol in group["uv_angle_symbols"]]
    y_payloads = [group["luma_payloads"] for group in parsed]
    dq_matrices = [group["dq_matrices"] for group in parsed]
    chroma_payloads = [group["chroma_payloads"] for group in parsed]
    all_lines = [line for group in groups for line in group]
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
    y_length = SIZE[0] * SIZE[1]
    top_luma_edge = (
        [yuv[7 * SIZE[0] + x] for x in range(SIZE[0])]
        if len(yuv) == 192
        else []
    )

    def is_tx8x8(payloads: list[dict[str, int]], with_ac: bool) -> bool:
        return (
            len(payloads) == 1
            and payloads[0]["tx"] == 1
            and payloads[0]["txtp"] == 0
            and (payloads[0]["eob"] >= 1 if with_ac else payloads[0]["eob"] >= 0)
        )

    def has_nonzero_ac(matrices: list[list[int]]) -> bool:
        return any(value != 0 for matrix in matrices for value in matrix[1:])

    def is_skipped_chroma(payloads: list[dict[str, int]], cbx4: int) -> bool:
        return (
            len(payloads) == 2
            and {payload["plane"] for payload in payloads} == {0, 1}
            and all(
                payload["tx"] == 0
                and payload["txtp"] == 0
                and payload["eob"] == -1
                and payload.get("cbx4") == cbx4
                for payload in payloads
            )
        )

    predicates = {
        "exact_vertical_split_shape": shape == [
            (0, 0, 0, 3, 0, 3),
            (0, 0, 0, 4, 0, 0),
            (0, 0, 2, 4, 0, 0),
        ],
        "eight_bit_420_frame": (
            portable_color.get("width") == SIZE[0]
            and portable_color.get("height") == SIZE[1]
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "two_visible_square8_groups": len(groups) == 2,
        "origin_luma_mode_dc": len(y_modes) == 2 and y_modes[0] == 0,
        "bottom_luma_mode_diagonal67": y_modes[1:2] == [8],
        "bottom_luma_angle_symbol": y_angles == [angle_symbol],
        "origin_luma_is_unsplit_tx8x8": (
            len(y_payloads) == 2 and is_tx8x8(y_payloads[0], False)
        ),
        "bottom_luma_is_unsplit_tx8x8_with_ac": (
            len(y_payloads) == 2
            and is_tx8x8(y_payloads[1], True)
            and len(dq_matrices) == 2
            and len(dq_matrices[1]) == 1
            and len(dq_matrices[1][0]) == 64
            and has_nonzero_ac(dq_matrices[1])
        ),
        "origin_and_bottom_chroma_are_dc_skipped": (
            len(chroma_payloads) == 2
            and is_skipped_chroma(chroma_payloads[0], 0)
            and is_skipped_chroma(chroma_payloads[1], 0)
        ),
        "no_unexpected_angle_or_tool_syntax": not any(
            line.startswith(forbidden_prefixes) for line in all_lines
        ),
        "decoded_yuv_has_expected_size": len(yuv) == y_length + 2 * (4 * 8),
        "origin_bottom_edge_varies": (
            len(top_luma_edge) == SIZE[0] and len(set(top_luma_edge)) > 1
        ),
        "origin_bottom_edge_is_not_midpoint": (
            len(top_luma_edge) == SIZE[0] and top_luma_edge[0] != 128
        ),
    }
    return {
        "target": f"vertical_following_luma_diagonal67_symbol_{angle_symbol}",
        "effective_predictor": "z1_top_available_left_unavailable",
        "root_partition": blocks[0] if blocks else None,
        "partition_shape": shape,
        "group_count": len(groups),
        "y_modes": y_modes,
        "y_angle_symbols": y_angles,
        "y_angle_deltas": [symbol - 3 for symbol in y_angles],
        "y_angles": [67 + 3 * (symbol - 3) for symbol in y_angles],
        "uv_modes": uv_modes,
        "uv_angle_symbols": uv_angles,
        "origin_bottom_luma_edge": top_luma_edge,
        "top_luma_payloads": y_payloads[0] if y_payloads else [],
        "bottom_luma_payloads": y_payloads[1] if len(y_payloads) == 2 else [],
        "bottom_luma_dq_matrix": (
            dq_matrices[1][0]
            if len(dq_matrices) == 2 and len(dq_matrices[1]) == 1
            else []
        ),
        "top_chroma_payloads": chroma_payloads[0] if chroma_payloads else [],
        "bottom_chroma_payloads": chroma_payloads[1] if len(chroma_payloads) == 2 else [],
        "predicates": predicates,
        "rejection_reasons": [name for name, passed in predicates.items() if not passed],
        "qualifies": all(predicates.values()),
    }


def classify(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
    target: str,
    angle_symbol: int,
) -> dict[str, object]:
    """Apply exact predicates for the vertical-following mode-8 class."""

    if target == "luma_diagonal67":
        return classify_luma(blocks, groups, yuv, portable_color, angle_symbol)

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
    y_angles = [symbol for group in parsed for symbol in group["y_angle_symbols"]]
    uv_angles = [symbol for group in parsed for symbol in group["uv_angle_symbols"]]
    y_payloads = [group["luma_payloads"] for group in parsed]
    chroma_payloads = [group["chroma_payloads"] for group in parsed]
    all_lines = [line for group in groups for line in group]
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
    u_edge = top_edge(yuv, 0) if len(yuv) == 192 else []
    v_edge = top_edge(yuv, 1) if len(yuv) == 192 else []

    def is_tx4x4(payloads: list[dict[str, int]], *, txtp: int | None = None) -> bool:
        return (
            len(payloads) == 4
            and all(payload["tx"] == 0 for payload in payloads)
            and (txtp is None or all(payload["txtp"] == txtp for payload in payloads))
        )

    def is_chroma_tx4x4(payloads: list[dict[str, int]], txtp: int) -> bool:
        return (
            len(payloads) == 2
            and {payload["plane"] for payload in payloads} == {0, 1}
            and all(
                payload["tx"] == 0
                and payload["txtp"] == txtp
                and payload.get("cbx4") == 0
                for payload in payloads
            )
        )

    bottom_y_modes = y_modes[1:2] if len(y_modes) >= 2 else []
    bottom_uv_modes = uv_modes[1:2] if len(uv_modes) >= 2 else []
    bottom_y_payloads = y_payloads[1] if len(y_payloads) == 2 else []
    bottom_chroma = chroma_payloads[1] if len(chroma_payloads) == 2 else []
    predicates = {
        "exact_vertical_split_shape": shape == [
            (0, 0, 0, 3, 0, 3),
            (0, 0, 0, 4, 0, 0),
            (0, 0, 2, 4, 0, 0),
        ],
        "eight_bit_420_frame": (
            portable_color.get("width") == SIZE[0]
            and portable_color.get("height") == SIZE[1]
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "two_visible_square8_groups": len(groups) == 2,
        "bottom_luma_mode_diagonal45": bottom_y_modes == [3],
        "bottom_luma_angle_is_valid": len(y_angles) == 1 and 0 <= y_angles[0] <= 5,
        "origin_uv_mode_dc": len(uv_modes) == 2 and uv_modes[0] == 0,
        "bottom_uv_mode_diagonal67": bottom_uv_modes == [8],
        "no_unexpected_angle_or_tool_syntax": not any(
            line.startswith(forbidden_prefixes) for line in all_lines
        ),
        "bottom_luma_is_split_tx4x4": is_tx4x4(bottom_y_payloads),
        "bottom_luma_has_ac": any(payload["eob"] >= 1 for payload in bottom_y_payloads),
        "origin_chroma_is_tx4x4_dct": (
            len(chroma_payloads) == 2 and is_chroma_tx4x4(chroma_payloads[0], 0)
        ),
        "bottom_chroma_is_tx4x4_adst_dct": is_chroma_tx4x4(bottom_chroma, 1),
        "bottom_chroma_has_ac_on_both_planes": (
            len(bottom_chroma) == 2 and all(payload["eob"] >= 1 for payload in bottom_chroma)
        ),
        "decoded_yuv_has_expected_size": len(yuv) == 192,
        "origin_u_bottom_edge_varies": len(u_edge) == 4 and len(set(u_edge)) > 1,
        "origin_v_bottom_edge_varies": len(v_edge) == 4 and len(set(v_edge)) > 1,
        "origin_chroma_edge_is_not_midpoint": (
            bool(u_edge) and bool(v_edge) and (u_edge[0] != 128 or v_edge[0] != 128)
        ),
    }
    return {
        "target": "vertical_following_chroma_diagonal67",
        "root_partition": blocks[0] if blocks else None,
        "partition_shape": shape,
        "group_count": len(groups),
        "y_modes": y_modes,
        "y_angle_symbols": y_angles,
        "y_angle_deltas": [symbol - 3 for symbol in y_angles],
        "y_angles": [45 + 3 * (symbol - 3) for symbol in y_angles],
        "uv_modes": uv_modes,
        "uv_angle_symbols": uv_angles,
        "uv_angle_deltas": [symbol - 3 for symbol in uv_angles],
        "uv_angles": [180 + 3 * (symbol - 3) for symbol in uv_angles],
        "origin_u_bottom_edge": u_edge,
        "origin_v_bottom_edge": v_edge,
        "top_luma_payloads": y_payloads[0] if y_payloads else [],
        "bottom_luma_payloads": bottom_y_payloads,
        "top_chroma_payloads": chroma_payloads[0] if chroma_payloads else [],
        "bottom_chroma_payloads": bottom_chroma,
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
    """Decode one color item through the independent scalar trace."""

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
    target: str,
    angle_symbol: int,
) -> dict[str, object]:
    """Double-encode, double-trace, and classify one candidate."""

    pixels = candidate["pixels"]
    if not isinstance(pixels, bytes):
        raise TypeError("candidate pixels must be bytes")
    quality = int(candidate["quality"])
    speed = int(candidate["speed"])
    encoded_a = encode(pixels, quality, speed, target)
    encoded_b = encode(pixels, quality, speed, target)
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
    classification = classify(
        blocks_a, groups_a, yuv_a, portable_color, target, angle_symbol
    )
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
    y_length = SIZE[0] * SIZE[1]
    chroma_length = (SIZE[0] // 2) * (SIZE[1] // 2)
    return {
        "id": candidate["id"],
        "family": candidate["family"],
        "seed": candidate["seed"],
        "quality": quality,
        "speed": speed,
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
        "decoded_y_plane_sha256": sha256(yuv_a[:y_length]),
        "decoded_u_plane_sha256": sha256(yuv_a[y_length : y_length + chroma_length]),
        "decoded_v_plane_sha256": sha256(yuv_a[y_length + chroma_length :]),
        "entropy_operation_count": entropy_a,
        "entropy_operation_count_second": entropy_b,
        "partition_blocks": blocks_a,
        "partition_blocks_second": blocks_b,
        "portable_color": portable_color,
        **classification,
    }


def main() -> None:
    """Run the deterministic oracle campaign and write its JSON report."""

    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument("--dav1d-source", type=Path)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--python-path", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--retain-dir", type=Path)
    parser.add_argument(
        "--target",
        choices=("chroma_diagonal67", "luma_diagonal67"),
        default="chroma_diagonal67",
    )
    parser.add_argument(
        "--luma-angle-symbol",
        type=int,
        choices=(2, 3, 4),
        default=3,
        help="D67 luma angle symbol to qualify (2=64 degrees, 3=67, 4=70)",
    )
    args = parser.parse_args()
    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")
    with tempfile.TemporaryDirectory(prefix="image-star-avif-diagonal67-vertical-") as name:
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
                include_block_angles=True,
                include_luma_angles=True,
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
                args.target,
                args.luma_angle_symbol,
            )
            for candidate in candidates(args.target)
        ]
    rejection_reasons = list(reports[0]["predicates"])
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
            "advanced": LUMA_ADVANCED if args.target == "luma_diagonal67" else ADVANCED,
        },
        "search": {
            "candidate_count": len(reports),
            "seed_formula": "12000 + 10*family_index + candidate_index",
            "target_id": (
                f"{args.target}_angle_symbol_{args.luma_angle_symbol}"
                if args.target == "luma_diagonal67"
                else args.target
            ),
            "target": (
                "8x16 8-bit 4:2:0 clipped 16x16 root split with top and bottom "
                "Square8 leaves; bottom following luma mode 8 Diagonal67 at "
                f"angle symbol {args.luma_angle_symbol} (resolved "
                f"{67 + 3 * (args.luma_angle_symbol - 3)} degrees), genuine Zone-1 top edge, unsplit TX8x8 "
                "luma with AC, and skipped chroma"
                if args.target == "luma_diagonal67"
                else (
                    "8x16 8-bit 4:2:0 clipped 16x16 root split with top and bottom "
                    "Square8 leaves; bottom following UV mode 8 Diagonal67, "
                    "ADST-DCT TX4x4 U/V, and non-empty AC"
                )
            ),
            "families": list(FAMILY_NAMES),
            "repository_rust_invoked": False,
        },
        "counts": {
            "qualified": sum(bool(item["qualifies"]) for item in reports),
            "by_rejection_reason": {
                reason: sum(reason in item["rejection_reasons"] for item in reports)
                for reason in rejection_reasons
            },
        },
        "qualified_candidates": [item["id"] for item in reports if item["qualifies"]],
        "promoted_candidate": next(
            (item["id"] for item in reports if item["qualifies"]), None
        ),
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic vertical Diagonal67 traces: {args.output}")


if __name__ == "__main__":
    main()
