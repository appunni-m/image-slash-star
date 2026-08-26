#!/usr/bin/env python3
"""Search a fixed corpus for a rectangular AV1 chroma leaf.

The campaign is deliberately bounded and input-driven. It creates exactly one
hundred deterministic 16x16 RGB candidates, encodes each twice through the
pinned Pillow/libavif/libaom oracle, and classifies two independent traces from
the pinned scalar dav1d executable. Generated files are temporary unless
``--retain-dir`` is supplied; no repository Rust code is invoked. The
historical ``diagonal157`` target is the default; ``origin_vertical`` searches
the newly specified origin-Vertical case using the same corpus. The separate
``following_paeth`` target searches coded UV mode 12 on the following leaf. The
``following_horizontal`` target searches the dependency-ready Horizontal case
with semantic UV angle symbols recorded by the instrumented oracle.
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


SIZE = (16, 16)
SUBSAMPLING = "4:2:0"
ADVANCED = {
    "min-partition-size": "8",
    "max-partition-size": "16",
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
PAETH_ADVANCED = {
    **ADVANCED,
    "enable-paeth-intra": "1",
    "enable-directional-intra": "0",
}
FAMILY_NAMES = (
    "rect_d157_saw_52",
    "rect_d157_saw_73",
    "rect_d157_saw_94",
    "rect_d157_step_52",
    "rect_d157_step_73",
    "rect_d157_right_bias",
    "rect_d157_edge_ramp",
    "rect_d157_luma_partition",
    "rect_d157_dual_ac",
    "rect_d157_mirror",
)
PAETH_FAMILY_NAMES = (
    "paeth_one_row_bands",
    "paeth_two_row_bands",
    "paeth_opposed_ramps",
    "paeth_four_row_saw",
    "paeth_triangle_rows",
    "paeth_single_step",
    "paeth_two_step",
    "paeth_row_impulse",
    "paeth_plane_phase",
    "paeth_luma_texture",
)
HORIZONTAL_FAMILY_NAMES = (
    "horizontal_control_d157",
    "horizontal_row_saw",
    "horizontal_row_step",
    "horizontal_row_triangle",
    "horizontal_row_bands",
    "horizontal_row_ramp",
    "horizontal_row_impulse",
    "horizontal_row_opposed",
    "horizontal_row_phase",
    "horizontal_row_mirror",
)
TARGET_DESCRIPTIONS = {
    "diagonal157": (
        "two side-by-side Vertical8x16 leaves with following right UV mode 6 "
        "(Diagonal157), R4x8 U/V, DctAdst, and non-empty AC"
    ),
    "origin_vertical": (
        "two side-by-side Vertical8x16 leaves with origin UV mode 1 "
        "(Vertical), right UV mode 5 (Diagonal113), ADST-DCT R4x8 U/V, "
        "non-empty AC on both leaves, and non-palette luma"
    ),
    "following_paeth": (
        "two side-by-side Vertical8x16 leaves with origin UV mode 0 (DC), "
        "following UV mode 12 (Paeth), one R4x8 U/V pair per leaf, "
        "following ADST-ADST chroma, non-empty AC, and nonconstant origin "
        "chroma right edges"
    ),
    "following_horizontal": (
        "two side-by-side Vertical8x16 leaves with origin UV mode 0 (DC), "
        "following UV mode 2 (Horizontal), one Dct-Dct and one Dct-Adst "
        "R4x8 U/V pair, non-empty AC, varying origin chroma right edges, "
        "and a recorded valid following UV angle symbol"
    ),
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
LUMA_MODE_PATTERN = re.compile(r"^Post-ymode\[(?P<mode>\d+)\]")
ANGLE_PATTERN = re.compile(r"^Post-uvangle-symbol\[(?P<symbol>\d+)\]")
CHROMA_PATTERN = re.compile(
    r"^Post-uv-cf-blk\[pl=(?P<plane>\d+),tx=(?P<tx>\d+),"
    r"txtp=(?P<txtp>-?\d+),eob=(?P<eob>-?\d+)\]"
    r"(?:.*cbx4=(?P<cbx4>\d+).*)?$"
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of one byte sequence."""

    return hashlib.sha256(data).hexdigest()


def clamp(value: int) -> int:
    """Clamp one generated RGB component to the 8-bit range."""

    return max(0, min(255, value))


def yuv_to_rgb(y: int, u: int, v: int) -> tuple[int, int, int]:
    """Convert a bounded synthetic BT.601-like YUV sample to RGB."""

    du = u - 128
    dv = v - 128
    return (
        clamp(y + (358 * dv + 128) // 256),
        clamp(y - (88 * du + 183 * dv + 128) // 256),
        clamp(y + (453 * du + 128) // 256),
    )


def chroma_deltas(family: int, index: int, cx: int, cy: int) -> tuple[int, int]:
    """Return deterministic U/V deltas for one 4:2:0 chroma sample."""

    seed = 1000 + 10 * family + index
    phase = (11 * index + 7 * family + 3) % 32
    amplitude = 16 + (index % 5) * 3
    if family in (0, 3, 5, 6, 7, 8):
        coordinate = 5 * cx - 2 * cy + phase
    elif family in (1, 4):
        coordinate = 7 * cx - 3 * cy + phase
    elif family == 2:
        coordinate = 9 * cx - 4 * cy + phase
    else:
        coordinate = 2 * cx - 5 * cy + phase
    wrapped = coordinate % 32
    wave = wrapped - 16
    if family in (3, 4):
        chroma = amplitude if wrapped >= 16 else -amplitude
    else:
        chroma = (wave * amplitude) // 16
    if family == 5 and cx >= 4:
        chroma *= 2
    if family == 6:
        chroma += (3 * cy + phase) % 7 - 3
    if family == 5:
        return (
            chroma + ((37 * cx + 19 * cy + seed) % 121) - 60,
            chroma + ((23 * cx + 47 * cy + 3 * seed) % 121) - 60,
        )
    if family == 7:
        return (
            ((17 * cx + 31 * cy + seed) % 121) - 60,
            ((29 * cx + 13 * cy + 2 * seed) % 121) - 60,
        )
    if family == 8:
        chroma += ((3 * cx + 5 * cy + seed) % 7) - 3
    if family in (1, 4, 9):
        return chroma, -chroma
    if family == 9:
        return -chroma, chroma
    return chroma, chroma


def paeth_chroma_deltas(family: int, index: int, cx: int, cy: int) -> tuple[int, int]:
    """Return row-shaped U/V deltas that can select coded Paeth."""

    amplitude = 24 + 4 * (index % 5)
    phase = (index + 2 * family) % 8
    epsilon = 2 + index % 4
    row = cy + phase
    if family == 0:
        base = amplitude if row % 2 == 0 else -amplitude
    elif family == 1:
        base = amplitude if (row // 2) % 2 == 0 else -amplitude
    elif family == 2:
        base = (row - 3) * (amplitude // 3)
    elif family == 3:
        base = (row % 4 - 1) * (amplitude // 2)
    elif family == 4:
        distance = abs((row % 8) - 4)
        base = (4 - distance) * (amplitude // 4)
    elif family == 5:
        base = amplitude if row >= 4 + index % 3 else -amplitude
    elif family == 6:
        base = amplitude if row in {2 + index % 2, 6 + index % 2} else -amplitude // 2
    elif family == 7:
        base = amplitude if row == 3 + index % 3 else -amplitude // 3
    elif family == 8:
        base = amplitude if (row + phase) % 3 == 0 else -amplitude // 2
    else:
        base = ((row * 3 + phase) % 9 - 4) * (amplitude // 4)
    horizontal = (cx - 3) * epsilon
    if family in {2, 4, 8}:
        return base + horizontal, -base + (cx + cy + index) % 3 - 1
    if family in {5, 6}:
        return base + horizontal, base - horizontal
    return base + horizontal, base + ((2 * cx + index) % 5) - 2


def horizontal_chroma_deltas(family: int, index: int, cx: int, cy: int) -> tuple[int, int]:
    """Return row-shaped U/V deltas for a following Horizontal predictor."""

    if family == 0:
        # Keep one exact historical family as a reproducibility control.
        return chroma_deltas(0, index, cx, cy)
    amplitude = 18 + (index % 5) * 3
    phase = (3 * index + 5 * family) % 8
    row = cy + phase
    if family == 1:
        base = amplitude if row % 2 == 0 else -amplitude
    elif family == 2:
        base = (row % 4 - 1) * (amplitude // 2)
    elif family == 3:
        base = (row - 3) * (amplitude // 3)
    elif family == 4:
        base = (4 - abs((row % 8) - 4)) * (amplitude // 4)
    elif family == 5:
        base = amplitude if row >= 4 + index % 3 else -amplitude
    elif family == 6:
        base = amplitude if row in {2 + index % 2, 6 + index % 2} else -amplitude // 2
    elif family == 7:
        base = amplitude if row == 3 + index % 3 else -amplitude // 3
    elif family == 8:
        base = ((row * 3 + phase) % 9 - 4) * (amplitude // 4)
    else:
        base = amplitude if (row + phase) % 3 == 0 else -amplitude // 2
    # Make the right leaf row-shaped while retaining a small AC signal in both
    # leaves. The left half is deliberately quieter so DC remains reachable.
    scale = 1 if cx < 4 else 2
    ripple = ((cx - 3) * (1 + family % 3)) if cx >= 4 else (cx - 3)
    u = scale * base + ripple
    if family in {2, 4, 8}:
        v = -scale * base + ((cx + cy + index) % 3) - 1
    elif family in {5, 6}:
        v = scale * base - ripple
    elif family == 9:
        v = -scale * base
        u, v = v, u
    else:
        v = scale * base + ((2 * cx + index) % 5) - 2
    return u, v


def candidate_pixels(family: int, index: int, target: str) -> bytes:
    """Create one deterministic 16x16 RGB candidate from a YUV-shaped field."""

    seed = 1000 + 10 * family + index
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            cx, cy = x // 2, y // 2
            if target == "following_paeth":
                u_delta, v_delta = paeth_chroma_deltas(family, index, cx, cy)
            elif target == "following_horizontal":
                u_delta, v_delta = horizontal_chroma_deltas(family, index, cx, cy)
            else:
                u_delta, v_delta = chroma_deltas(family, index, cx, cy)
            luma = 128
            if target == "following_paeth":
                if family in (3, 6, 8, 9):
                    luma += ((5 * x + 3 * y + seed) % 9) - 4
                if x >= 8:
                    luma += 9 if (x // 2 + y // 2 + seed) % 2 else -9
            elif target == "following_horizontal":
                # Keep luma on the proven DC-only topology so chroma mode
                # selection is the only changing variable in this campaign.
                luma = 128
            elif family in (5, 7, 8):
                luma += 14 if x >= 8 and ((x // 2 + y // 2 + seed) % 2) else 0
                if family == 5:
                    luma += ((3 * (x // 2) + 5 * (y // 2) + seed) % 3) - 1
                if family == 8:
                    luma += ((7 * x + 11 * y + seed) % 17) - 8
            elif family == 9:
                luma += ((x // 2 + 3 * (y // 2) + seed) % 13) - 6
            if family == 6 and x >= 8:
                luma += 8
            pixels.extend(yuv_to_rgb(luma, 128 + u_delta, 128 + v_delta))
    return bytes(pixels)


def candidates(target: str) -> list[dict[str, object]]:
    """Return exactly ten deterministic families with ten cases each."""

    result = []
    if target == "following_paeth":
        family_names = PAETH_FAMILY_NAMES
        prefix = "PTH"
    elif target == "following_horizontal":
        family_names = HORIZONTAL_FAMILY_NAMES
        prefix = "HOR"
    else:
        family_names = FAMILY_NAMES
        prefix = "R157"
    for family, family_name in enumerate(family_names):
        for index in range(10):
            result.append(
                {
                    "id": f"{prefix}-F{family + 1:02d}-N{index:02d}",
                    "family": family_name,
                    "family_index": family,
                    "candidate_index": index,
                    "seed": 1000 + 10 * family + index,
                    "pixels": candidate_pixels(family, index, target),
                    "quality": 76,
                    "speed": 0,
                }
            )
    if len(result) != 100:
        raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
    return result


def encode(pixels: bytes, quality: int, speed: int, target: str) -> bytes:
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
        advanced=PAETH_ADVANCED if target == "following_paeth" else ADVANCED,
    )
    return output.getvalue()


def trace(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    item: bytes,
    stem: str,
    ordinal: int,
) -> tuple[str, list[dict[str, int]], list[list[str]], int, bytes]:
    """Trace one color item with the independent scalar dav1d executable."""

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


def classify(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    left_edge_observable: bool,
    yuv: bytes,
    target: str,
) -> dict[str, object]:
    """Apply exact predicates for one of the supported rectangular targets."""

    root = next(
        (
            block
            for block in blocks
            if (
                block["x"] == 0
                and block["y"] == 0
                and block["level"] == 3
                and block["context"] == 0
                and block["partition"] == 2
            )
        ),
        None,
    )
    luma_groups = []
    chroma_groups = []
    uv_modes = []
    y_modes = []
    uv_angle_symbols = []
    for group in groups:
        luma_payloads = []
        chroma_payloads = []
        for line in group:
            if line.startswith("Post-uvmode["):
                uv_modes.append(int(line.split("[", 1)[1].split("]", 1)[0]))
            if match := LUMA_MODE_PATTERN.match(line):
                y_modes.append(int(match["mode"]))
            if match := ANGLE_PATTERN.match(line):
                uv_angle_symbols.append(int(match["symbol"]))
            if match := LUMA_PATTERN.match(line):
                luma_payloads.append({name: int(value) for name, value in match.groupdict().items()})
            if match := CHROMA_PATTERN.match(line):
                chroma_payloads.append(
                    {
                        name: int(value)
                        for name, value in match.groupdict().items()
                        if value is not None
                    }
                )
        luma_groups.append(luma_payloads)
        chroma_groups.append(chroma_payloads)
    right_luma = luma_groups[1] if len(luma_groups) == 2 else []
    right_chroma = chroma_groups[1] if len(chroma_groups) == 2 else []
    left_chroma = chroma_groups[0] if chroma_groups else []

    def is_vertical8x16_luma(payloads: list[dict[str, int]]) -> bool:
        """Accept the unsplit or legal two-TX8x8 luma form of Vertical8x16."""

        return (len(payloads) == 1 and payloads[0]["tx"] == 7) or (
            len(payloads) == 2 and all(payload["tx"] == 1 for payload in payloads)
        )

    common_predicates = {
        "vertical_split_root": root is not None,
        "two_visible_leaf_groups": len(groups) == 2,
        "both_vertical8x16_luma": (
            len(luma_groups) == 2
            and all(is_vertical8x16_luma(payloads) for payloads in luma_groups)
        ),
        "left_edge_observable": left_edge_observable,
        "no_filter_intra": not any(
            line.startswith("Post-filterintramode[")
            for group in groups
            for line in group
        ),
    }
    no_palette = not any(
        line.startswith(("Post-y_pal[", "Post-pal[", "Post-y-pal-indices", "y-pal-pred"))
        for group in groups
        for line in group
    ) and not any(
        line.startswith(("Post-uv_pal[", "Post-uv-pal-indices", "uv-pal-pred"))
        for group in groups
        for line in group
    )

    def is_r4x8(
        payloads: list[dict[str, int]], txtp: int, cbx4: int | None = None
    ) -> bool:
        """Accept exactly one non-empty U/V R4x8 coefficient pair."""

        return (
            len(payloads) == 2
            and {payload["plane"] for payload in payloads} == {0, 1}
            and all(payload["tx"] == 5 for payload in payloads)
            and all(payload["txtp"] == txtp for payload in payloads)
            and (cbx4 is None or all(payload.get("cbx4") == cbx4 for payload in payloads))
        )

    def has_nonempty_ac(payloads: list[dict[str, int]]) -> bool:
        """Require non-empty AC on both chroma planes."""

        return len(payloads) == 2 and all(payload["eob"] >= 1 for payload in payloads)

    def origin_right_chroma_edge(offset: int) -> list[int]:
        """Read the origin leaf's rightmost 4:2:0 chroma column from dav1d YUV."""

        if len(yuv) < 384:
            return []
        return [yuv[offset + row * 8 + 3] for row in range(8)]

    origin_u_edge = origin_right_chroma_edge(256)
    origin_v_edge = origin_right_chroma_edge(320)

    if target == "origin_vertical":
        target_predicates = {
            "no_palette": no_palette,
            "origin_uv_mode_1": len(uv_modes) == 2 and uv_modes[0] == 1,
            "right_uv_mode_5": len(uv_modes) == 2 and uv_modes[1] == 5,
            "left_chroma_r4x8_adst_dct": is_r4x8(left_chroma, 1),
            "right_chroma_r4x8_adst_dct": is_r4x8(right_chroma, 1),
            "left_chroma_nonempty_ac": has_nonempty_ac(left_chroma),
            "right_chroma_nonempty_ac": has_nonempty_ac(right_chroma),
        }
    elif target == "diagonal157":
        target_predicates = {
            "right_uv_mode_6": len(uv_modes) == 2 and uv_modes[1] == 6,
            "right_chroma_r4x8": is_r4x8(right_chroma, 2),
            "right_chroma_nonempty_ac": has_nonempty_ac(right_chroma),
        }
    elif target == "following_paeth":
        target_predicates = {
            "no_palette": no_palette,
            "no_luma_paeth": len(y_modes) == 2 and all(mode != 12 for mode in y_modes),
            "origin_uv_mode_0": len(uv_modes) == 2 and uv_modes[0] == 0,
            "following_uv_mode_12": len(uv_modes) == 2 and uv_modes[1] == 12,
            "left_chroma_r4x8_dct_dct": is_r4x8(left_chroma, 0, cbx4=0),
            "following_chroma_r4x8_adst_adst": is_r4x8(right_chroma, 3, cbx4=1),
            "left_chroma_nonempty_ac": has_nonempty_ac(left_chroma),
            "following_chroma_nonempty_ac": has_nonempty_ac(right_chroma),
            "origin_u_right_edge_varies": len(set(origin_u_edge)) > 1,
            "origin_v_right_edge_varies": len(set(origin_v_edge)) > 1,
        }
    elif target == "following_horizontal":
        following_angle_valid = len(uv_angle_symbols) == 1 and all(
            0 <= symbol <= 6 for symbol in uv_angle_symbols
        )
        target_predicates = {
            "no_palette": no_palette,
            "luma_modes_dc_only": len(y_modes) == 2 and all(mode == 0 for mode in y_modes),
            "origin_uv_mode_0": len(uv_modes) == 2 and uv_modes[0] == 0,
            "following_uv_mode_2": len(uv_modes) == 2 and uv_modes[1] == 2,
            "origin_chroma_r4x8_dct_dct": is_r4x8(left_chroma, 0, cbx4=0),
            "following_chroma_r4x8_dct_adst": is_r4x8(right_chroma, 2, cbx4=1),
            "origin_chroma_nonempty_ac": has_nonempty_ac(left_chroma),
            "following_chroma_nonempty_ac": has_nonempty_ac(right_chroma),
            "origin_u_right_edge_varies": len(set(origin_u_edge)) > 1,
            "origin_v_right_edge_varies": len(set(origin_v_edge)) > 1,
            "following_uv_angle_recorded": len(uv_angle_symbols) == 1,
            "following_uv_angle_valid": following_angle_valid,
        }
    else:
        raise ValueError(f"unknown rectangular campaign target: {target}")
    predicates = {**common_predicates, **target_predicates}
    return {
        "root_partition": root,
        "group_count": len(groups),
        "uv_modes": uv_modes,
        "following_uv_angle_symbols": uv_angle_symbols,
        "following_uv_angle_deltas": [symbol - 3 for symbol in uv_angle_symbols],
        "following_uv_angles": [180 + 3 * (symbol - 3) for symbol in uv_angle_symbols],
        "left_luma_payloads": luma_groups[0] if luma_groups else [],
        "right_luma_payloads": right_luma,
        "left_chroma_payloads": chroma_groups[0] if chroma_groups else [],
        "right_chroma_payloads": right_chroma,
        "luma_modes": y_modes,
        "origin_u_right_edge": origin_u_edge,
        "origin_v_right_edge": origin_v_edge,
        "target": target,
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
    target: str,
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
    trace_a, blocks_a, groups_a, entropy_a, yuv_a = trace(
        executable, environment, work, item_a, str(candidate["id"]), 1
    )
    trace_b, blocks_b, groups_b, entropy_b, yuv_b = trace(
        executable, environment, work, item_b, str(candidate["id"]), 2
    )
    left_edge = any(
        delta != 0
        for cy in range(8)
        for delta in (
            paeth_chroma_deltas(
                int(candidate["family_index"]), int(candidate["candidate_index"]), 3, cy
            )
            if target == "following_paeth"
            else horizontal_chroma_deltas(
                int(candidate["family_index"]), int(candidate["candidate_index"]), 3, cy
            )
            if target == "following_horizontal"
            else chroma_deltas(
                int(candidate["family_index"]), int(candidate["candidate_index"]), 3, cy
            )
        )
    )
    classification = classify(blocks_a, groups_a, left_edge, yuv_a, target)
    classification["predicates"].update(
        {
            "double_encode_equal": encoded_a == encoded_b,
            "double_trace_equal": (
                trace_a == trace_b
                and blocks_a == blocks_b
                and groups_a == groups_b
                and yuv_a == yuv_b
            ),
        }
    )
    classification["rejection_reasons"] = [
        name for name, passed in classification["predicates"].items() if not passed
    ]
    classification["qualifies"] = all(classification["predicates"].values())
    portable_color = portable_color_reference(path_a)
    report = {
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
        "entropy_operation_count": entropy_a,
        "entropy_operation_count_second": entropy_b,
        "yuv_sha256": sha256(yuv_a),
        "yuv_sha256_second": sha256(yuv_b),
        "yuv_size": len(yuv_a),
        "partition_blocks": blocks_a,
        "partition_blocks_second": blocks_b,
        "trace_sha256": sha256(trace_a.encode()),
        "trace_sha256_second": sha256(trace_b.encode()),
        "portable_color": portable_color,
        **classification,
    }
    if report["qualifies"] and retain_dir is not None:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path_a.name).write_bytes(encoded_a)
    return report


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
    parser.add_argument("--target", choices=sorted(TARGET_DESCRIPTIONS), default="diagonal157")
    args = parser.parse_args()
    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")
    with tempfile.TemporaryDirectory(prefix=f"image-star-avif-rectangular-{args.target}-") as name:
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
                include_block_angles=args.target == "following_horizontal",
            )
        else:
            executable = args.dav1d.resolve()
            environment = {}
        version_result = run([str(executable), "--version"], env=environment)
        version = (version_result.stdout + version_result.stderr).strip()
        if not version.startswith("1.5.3-0-gb546257"):
            raise RuntimeError(f"unexpected dav1d executable version: {version}")
        reports = [
            decode_candidate(executable, environment, work, candidate, args.retain_dir, args.target)
            for candidate in candidates(args.target)
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
            "advanced": PAETH_ADVANCED if args.target == "following_paeth" else ADVANCED,
        },
        "search": {
            "candidate_count": len(reports),
            "seed_formula": "1000 + 10*family_index + candidate_index",
            "target_id": args.target,
            "target": TARGET_DESCRIPTIONS[args.target],
            "families": list(
                PAETH_FAMILY_NAMES
                if args.target == "following_paeth"
                else HORIZONTAL_FAMILY_NAMES
                if args.target == "following_horizontal"
                else FAMILY_NAMES
            ),
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "by_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in sorted(reports[0]["predicates"])
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic {args.target} traces: {args.output}")


if __name__ == "__main__":
    main()
