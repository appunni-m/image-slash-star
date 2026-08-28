#!/usr/bin/env python3
"""Search fixed corpora for bounded Square8 AVIF reconstruction witnesses.

The search is deliberately bounded and input-driven. It creates exactly one
hundred deterministic 16x8 RGB candidates for the selected target, encodes
each twice through the pinned Pillow/libavif/libaom oracle, and classifies an
independently instrumented scalar dav1d trace. Generated files are temporary
unless ``--retain-dir`` is supplied; no repository Rust code is invoked. The
historical ``diagonal113`` target remains the default; the luma targets search
bounded right-hand Square8 predictor witnesses, and ``chroma_diagonal45``
searches a mode-3 chroma witness.
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


SIZE = (16, 8)
SUBSAMPLING = "4:2:0"
LUMA_DIAGONAL_FAMILY_NAMES = (
    "F01_diagonal_down_right_ramp",
    "F02_diagonal_down_right_step",
    "F03_diagonal_down_right_saw",
    "F04_diagonal_down_right_ripple",
    "F05_diagonal_down_right_bands",
    "F06_diagonal_down_right_checker",
    "F07_diagonal_down_right_edge",
    "F08_diagonal_down_right_mirror",
    "F09_diagonal_down_right_dual_ac",
    "F10_diagonal_down_right_mixed",
)
LUMA_SMOOTH_FAMILY_NAMES = (
    "F01_smooth_bilinear_ramp",
    "F02_smooth_bilinear_offset",
    "F03_smooth_bilinear_texture",
    "F04_smooth_vertical_ramp",
    "F05_smooth_vertical_offset",
    "F06_smooth_vertical_texture",
    "F07_smooth_horizontal_wave",
    "F08_smooth_horizontal_texture",
    "F09_smooth_horizontal_edges",
    "F10_smooth_mixed",
)
LUMA_DIAGONAL45_FAMILY_NAMES = (
    "F01_diagonal45_ramp",
    "F02_diagonal45_step",
    "F03_diagonal45_saw",
    "F04_diagonal45_ripple",
    "F05_diagonal45_bands",
    "F06_diagonal45_checker",
    "F07_diagonal45_edge",
    "F08_diagonal45_mirror",
    "F09_diagonal45_dual_ac",
    "F10_diagonal45_mixed",
)
LUMA_DIAGONAL67_FAMILY_NAMES = (
    "F01_diagonal67_vertical_ramp",
    "F02_diagonal67_vertical_step",
    "F03_diagonal67_vertical_saw",
    "F04_diagonal67_vertical_ripple",
    "F05_diagonal67_vertical_bands",
    "F06_diagonal67_vertical_checker",
    "F07_diagonal67_vertical_edge",
    "F08_diagonal67_vertical_mirror",
    "F09_diagonal67_vertical_dual_ac",
    "F10_diagonal67_vertical_mixed",
)
CHROMA_DIAGONAL45_FAMILY_NAMES = (
    "F01_descending_ramp",
    "F02_descending_step",
    "F03_descending_wave",
    "F04_descending_bands",
    "F05_descending_phase",
    "F06_rising_bands",
    "F07_steep_rising_wave",
    "F08_steep_descending_bands",
    "F09_wide_rising_phase",
    "F10_steep_descending_wave",
)
CHROMA_DIAGONAL45_PARAMETERS = (
    (1, -1, 0),
    (1, -1, 1),
    (1, -1, 2),
    (1, -1, 3),
    (1, -1, 4),
    (1, 2, 3),
    (5, 2, 2),
    (5, -2, 3),
    (7, 3, 4),
    (1, -8, 2),
)
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
LUMA_SMOOTH_ADVANCED = {
    **ADVANCED,
    "use-intra-dct-only": "1",
    "enable-smooth-intra": "1",
    "enable-directional-intra": "0",
}
LUMA_DIAGONAL45_ADVANCED = {
    **ADVANCED,
    "use-intra-dct-only": "1",
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
CHROMA_LOCATION_PATTERN = re.compile(r"cbx4=(?P<cbx4>\d+)")
YMODE_PATTERN = re.compile(r"^Post-ymode\[(?P<mode>\d+)\]")
YANGLE_PATTERN = re.compile(r"^Post-yangle-symbol\[(?P<symbol>\d+)\]")
UVANGLE_PATTERN = re.compile(r"^Post-uvangle-symbol\[(?P<symbol>\d+)\]")


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


def yuv_to_rgb(y: int, u: int, v: int) -> tuple[int, int, int]:
    """Convert one bounded synthetic full-range YUV sample to RGB."""

    du = u - 128
    dv = v - 128
    return (
        clamp(y + (358 * dv + 128) // 256),
        clamp(y - (88 * du + 183 * dv + 128) // 256),
        clamp(y + (453 * du + 128) // 256),
    )


def luma_diagonal_pattern(family: int, index: int) -> bytes:
    """Generate a low-chroma 135-degree luma continuation corpus."""

    seed = 4000 + 10 * family + index
    phase = (7 * family + 11 * index) % 16

    def left_signal(x: int, y: int) -> int:
        # The left leaf is intentionally quiet and non-directional. Its small
        # row-dependent edge variation makes the following leaf's spatial
        # context observable without forcing a directional left mode.
        if family in (0, 1):
            # High-frequency, uncorrelated detail encourages the encoder to
            # split the left residual into four TX4x4 payloads without giving
            # the DC predictor a coherent directional signal.
            return ((17 * x + 31 * y + phase + seed) % 33) - 16
        value = ((5 * x + 7 * y + phase + seed) % 13) - 6
        if family in (2, 5):
            block = ((x // 4) + (y // 4) + index) & 1
            value += 14 if block == 0 else -14
        return value

    def diagonal_signal(x: int, y: int) -> int:
        coordinate = x - y if family in (0, 1, 3, 5, 7, 9) else x + y
        wrapped = (coordinate * (1 + family % 3) + phase) % 32
        wave = wrapped - 16
        if family in (1, 4):
            return 24 if wrapped >= 16 else -24
        if family == 2:
            return wave * 2
        if family == 3:
            return wave + (((3 * x + 5 * y + seed) % 9) - 4)
        if family == 5:
            return (18 if (x + y + phase) % 4 < 2 else -18) + wave // 2
        if family == 6:
            return 12 + (wave if x >= 3 else -wave // 2)
        if family == 7:
            return -wave
        if family == 8:
            return wave * 2 + (((x + 2 * y + seed) % 7) - 3)
        if family == 9:
            return wave + (14 if (x + 2 * y + index) % 5 == 0 else -8)
        return wave * 2

    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            cx, cy = x // 2, y // 2
            edge = left_signal(7, y)
            if x < 8:
                luma = 128 + left_signal(x, y)
                chroma = ((3 * x + 5 * y + seed) % 7) - 3
            else:
                luma = 128 + edge + diagonal_signal(x - 8, y)
                chroma = ((13 * cx + 17 * cy + phase + seed) % 17) - 8
            # Keep chroma uncorrelated rather than directional so DC remains
            # the coded predictor, while the right leaf receives AC on both
            # planes at the pinned quality.
            scale = 1 if x < 8 else 2 + (index % 3)
            u_delta = scale * chroma + ((cx + 2 * cy + family) % 3) - 1
            v_delta = scale * chroma + ((2 * cx + cy + index) % 3) - 1
            pixels.extend(yuv_to_rgb(luma, 128 + u_delta, 128 + v_delta))
    return bytes(pixels)


def luma_smooth_pattern(family: int, index: int) -> bytes:
    """Generate deterministic low-chroma luma-smooth candidates.

    The first, fourth, and seventh families retain the three oracle probes
    that selected right-leaf modes 9, 10, and 11 respectively. The remaining
    families add bounded offsets and texture so the campaign tests reachability
    rather than promoting one repeated input three times.
    """

    seed = 5000 + 10 * family + index
    offset = index % 5
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if family == 0:
                # The index-zero case is the known right-mode-9 bilinear ramp.
                luma = (
                    32
                    + (191 * x // 15)
                    + ((191 * y // 7) // 2)
                    + offset
                )
            elif family == 1:
                luma = (
                    32
                    + (191 * x // 15)
                    + ((191 * y // 7) // 2)
                    + 2 * ((x + 2 * y + seed) % 5 - 2)
                )
            elif family == 2:
                luma = (
                    36
                    + (187 * x // 15)
                    + ((187 * y // 7) // 2)
                    + ((x // 2 + y + index) % 3 - 1)
                )
            elif family == 3:
                # The index-zero case is the known right-mode-10 vertical ramp.
                luma = (191 * x // 15) + (191 * y // 7) + offset
            elif family == 4:
                luma = (
                    8
                    + (183 * x // 15)
                    + (183 * y // 7)
                    + 2 * ((x + seed) % 5 - 2)
                )
            elif family == 5:
                luma = (
                    (191 * x // 15)
                    + (191 * y // 7)
                    + ((x + 3 * y + index) % 7 - 3)
                )
            elif family == 6:
                # The index-zero case is the known right-mode-11 horizontal
                # texture, which naturally selects a split TX4x4 payload.
                luma = 32 + ((11 * x + 2 * y + index) % 160)
            elif family == 7:
                luma = 40 + ((11 * x + 2 * y + seed) % 144)
            elif family == 8:
                luma = 32 + ((7 * x + 5 * y + seed) % 160)
            else:
                luma = 32 + ((11 * x + 2 * y + index) % 160)
                luma += ((x + y + seed) % 5) - 2
            pixels.extend(yuv_to_rgb(clamp(luma), 128, 128))
    return bytes(pixels)


def luma_diagonal45_pattern(family: int, index: int) -> bytes:
    """Generate deterministic low-slope 45-degree luma candidates."""

    bases = (120, 132, 136, 140, 144, 148, 152, 120, 136, 152)
    amplitudes = (0, 0, 0, 0, 0, 0, 0, 2, 2, 2)
    base = bases[family] + (index % 3)
    amplitude = amplitudes[family]

    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if x < 8:
                luma = base + amplitude * ((x + 2 * y + index) % 3 - 1)
            else:
                luma = base + (x - 8 + y) + amplitude * ((x + y + index) % 3 - 1)
            pixels.extend(yuv_to_rgb(clamp(luma), 128, 128))
    return bytes(pixels)


def luma_diagonal67_pattern(family: int, index: int) -> bytes:
    """Generate deterministic near-vertical luma candidates.

    The right leaf is on the frame's top edge, so the oracle's effective
    predictor for coded D67 syntax is vertical prediction from the first
    sample of the available left edge.  These families vary that edge and add
    bounded residual detail without introducing chroma structure.
    """

    bases = (44, 60, 76, 92, 108, 124, 140, 156, 172, 188)
    slopes = (13, 15, 17, 19, 21, 11, 23, 14, 18, 16)
    base = bases[family] + (index % 3)
    slope = slopes[family]
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if x < 8:
                luma = base + slope * y
                if family in (1, 5):
                    luma += 18 if (y // 2 + index) % 2 == 0 else -18
                elif family in (2, 8):
                    luma += ((3 * y + index + family) % 7) - 3
                elif family == 6:
                    luma += 22 if y >= 4 else -22
            else:
                right_y = y
                luma = base + slope * right_y
                if family in (0, 3, 7):
                    luma += (x - 8) // 2
                elif family in (1, 4):
                    luma += 16 if ((x - 8) + y + index) % 4 < 2 else -16
                elif family == 5:
                    luma += ((5 * (x - 8) + 3 * y + index) % 11) - 5
                elif family == 6:
                    luma += 24 if x >= 12 else -24
                elif family == 8:
                    luma += ((x - 8) + 2 * y + index) % 9 - 4
                elif family == 9:
                    luma += 12 if (x + y + index) % 3 == 0 else -8
            pixels.extend(yuv_to_rgb(clamp(luma), 128, 128))
    return bytes(pixels)


def chroma_diagonal45_pattern(family: int, index: int) -> bytes:
    """Generate deterministic chroma fields for a mode-3 witness search."""

    a, b, kind = CHROMA_DIAGONAL45_PARAMETERS[family]
    amplitude = 8 + 2 * (index % 8)
    phase = (index * 3 + a * 7 + b * 11) % 32
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            cx, cy = x // 2, y // 2
            if cx < 4:
                chroma = (a * cx + b * cy + phase) % 16 - 8
            else:
                value = a * cx + b * cy + phase
                if kind == 0:
                    chroma = value % 16 - 8
                elif kind == 1:
                    chroma = amplitude if value % 16 >= 8 else -amplitude
                elif kind == 2:
                    chroma = ((value % 32) - 16) * amplitude // 8
                elif kind == 3:
                    chroma = amplitude if (value // 4) % 2 == 0 else -amplitude
                else:
                    chroma = amplitude if value % 3 == 0 else -amplitude // 2
            # Opposed U/V signals make the chroma direction observable while
            # leaving luma at a stable DC level for this syntax witness.
            pixels.extend(yuv_to_rgb(128, 128 + chroma, 128 - chroma))
    return bytes(pixels)


def candidates(target: str = "diagonal113") -> list[dict[str, object]]:
    """Return ten deterministic families with ten cases each."""

    if target == "luma_diagonal_down_right":
        result = []
        for family, family_name in enumerate(LUMA_DIAGONAL_FAMILY_NAMES):
            for index in range(10):
                seed = 4000 + 10 * family + index
                result.append(
                    {
                        "id": f"LDR-F{family + 1:02d}-N{index:02d}",
                        "family": family_name,
                        "family_index": family,
                        "candidate_index": index,
                        "seed": seed,
                        "pixels": luma_diagonal_pattern(family, index),
                        "quality": 76,
                        "speed": 0,
                    }
                )
        if len(result) != 100:
            raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
        return result

    if target == "luma_smooth":
        result = []
        for family, family_name in enumerate(LUMA_SMOOTH_FAMILY_NAMES):
            for index in range(10):
                result.append(
                    {
                        "id": f"LS-F{family + 1:02d}-N{index:02d}",
                        "family": family_name,
                        "family_index": family,
                        "candidate_index": index,
                        "seed": 5000 + 10 * family + index,
                        "pixels": luma_smooth_pattern(family, index),
                        "quality": 76,
                        "speed": 0,
                    }
                )
        if len(result) != 100:
            raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
        return result

    if target == "luma_diagonal45":
        result = []
        for family, family_name in enumerate(LUMA_DIAGONAL45_FAMILY_NAMES):
            for index in range(10):
                result.append(
                    {
                        "id": f"LD45-F{family + 1:02d}-N{index:02d}",
                        "family": family_name,
                        "family_index": family,
                        "candidate_index": index,
                        "seed": 6000 + 10 * family + index,
                        "pixels": luma_diagonal45_pattern(family, index),
                        "quality": 76,
                        "speed": 0,
                    }
                )
        if len(result) != 100:
            raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
        return result

    if target == "luma_diagonal67":
        result = []
        for family, family_name in enumerate(LUMA_DIAGONAL67_FAMILY_NAMES):
            for index in range(10):
                result.append(
                    {
                        "id": f"LD67-F{family + 1:02d}-N{index:02d}",
                        "family": family_name,
                        "family_index": family,
                        "candidate_index": index,
                        "seed": 7000 + 10 * family + index,
                        "pixels": luma_diagonal67_pattern(family, index),
                        "quality": 76,
                        "speed": 0,
                    }
                )
        if len(result) != 100:
            raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
        return result

    if target == "chroma_diagonal45":
        result = []
        for family, family_name in enumerate(CHROMA_DIAGONAL45_FAMILY_NAMES):
            for index in range(10):
                result.append(
                    {
                        "id": f"CD45-F{family + 1:02d}-N{index:02d}",
                        "family": family_name,
                        "family_index": family,
                        "candidate_index": index,
                        "seed": 9000 + 10 * family + index,
                        "pixels": chroma_diagonal45_pattern(family, index),
                        "quality": 76,
                        "speed": 0,
                    }
                )
        if len(result) != 100:
            raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
        return result

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


def advanced_for_target(target: str) -> dict[str, str]:
    """Return the immutable pinned encoder controls for a campaign target."""

    if target == "luma_smooth":
        return LUMA_SMOOTH_ADVANCED
    if target == "luma_diagonal45":
        return LUMA_DIAGONAL45_ADVANCED
    if target == "luma_diagonal67":
        return LUMA_DIAGONAL45_ADVANCED
    return ADVANCED


def encode(
    pixels: bytes,
    quality: int,
    speed: int,
    target: str = "diagonal113",
) -> bytes:
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
        advanced=advanced_for_target(target),
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


def parse_group(group: list[str]) -> dict[str, object]:
    """Extract predictor modes, angles, and coefficient payloads from one leaf."""

    y_modes = [
        int(match["mode"])
        for line in group
        if (match := YMODE_PATTERN.match(line)) is not None
    ]
    y_angle_symbols = [
        int(match["symbol"])
        for line in group
        if (match := YANGLE_PATTERN.match(line)) is not None
    ]
    uv_modes = [
        int(line.split("[", 1)[1].split("]", 1)[0])
        for line in group
        if line.startswith("Post-uvmode[")
    ]
    luma_payloads = []
    chroma_payloads = []
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
        "y_angle_symbols": y_angle_symbols,
        "uv_modes": uv_modes,
        "luma_payloads": luma_payloads,
        "chroma_payloads": chroma_payloads,
    }


def classify_luma_diagonal_down_right(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
) -> dict[str, object]:
    """Apply exact predicates for the mode-4/135-degree Square8 witness."""

    parsed = [parse_group(group) for group in groups]
    y_modes = [mode for group in parsed for mode in group["y_modes"]]
    y_angle_symbols = [
        symbol for group in parsed for symbol in group["y_angle_symbols"]
    ]
    uv_modes = [mode for group in parsed for mode in group["uv_modes"]]
    uv_angle_symbols = [
        int(match["symbol"])
        for group in groups
        for line in group
        if (match := UVANGLE_PATTERN.match(line)) is not None
    ]
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
    expected_shape = [
        (0, 0, 0, 3, 0, 3),
        (0, 0, 0, 4, 0, 0),
        (0, 2, 0, 4, 0, 0),
    ]
    y_plane = yuv[: SIZE[0] * SIZE[1]]
    left_edge = [
        y_plane[row * SIZE[0] + 7]
        for row in range(SIZE[1])
    ] if len(y_plane) == SIZE[0] * SIZE[1] else []
    luma_groups = [group["luma_payloads"] for group in parsed]
    chroma_groups = [group["chroma_payloads"] for group in parsed]
    no_palette_or_filter = not any(
        line.startswith(
            (
                "Post-filterintramode[",
                "Post-y_pal[",
                "Post-pal[",
                "Post-y-pal-indices",
                "y-pal-pred",
                "Post-uv_pal[",
                "Post-uv-pal-indices",
                "uv-pal-pred",
            )
        )
        for group in groups
        for line in group
    )

    def is_tx4x4(payloads: list[dict[str, int]]) -> bool:
        return len(payloads) == 4 and all(payload["tx"] == 0 for payload in payloads)

    def is_chroma_tx4x4(payloads: list[dict[str, int]], cbx4: int) -> bool:
        return (
            len(payloads) == 2
            and {payload["plane"] for payload in payloads} == {0, 1}
            and all(
                payload["tx"] == 0
                and payload["txtp"] == 0
                and payload.get("cbx4") == cbx4
                for payload in payloads
            )
        )

    predicates = {
        "exact_visible_split_blocks": shape == expected_shape,
        "eight_bit_420_frame": (
            portable_color.get("width") == SIZE[0]
            and portable_color.get("height") == SIZE[1]
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "two_visible_square8_groups": len(groups) == 2,
        "luma_modes_dc_then_diagonal_down_right": y_modes == [0, 4],
        "luma_angle_symbol_is_zero_delta_135": (
            y_angle_symbols == [3]
        ),
        "uv_modes_dc_then_dc": uv_modes == [0, 0],
        "no_uv_angle_symbols": uv_angle_symbols == [],
        "no_palette_or_filter_intra": no_palette_or_filter,
        "both_luma_leaves_are_split_tx4x4": (
            len(luma_groups) == 2 and all(is_tx4x4(payloads) for payloads in luma_groups)
        ),
        "right_luma_has_ac": (
            len(luma_groups) == 2
            and any(payload["eob"] >= 1 for payload in luma_groups[1])
        ),
        "left_chroma_is_tx4x4_dct": (
            len(chroma_groups) == 2 and is_chroma_tx4x4(chroma_groups[0], 0)
        ),
        "right_chroma_is_tx4x4_dct": (
            len(chroma_groups) == 2 and is_chroma_tx4x4(chroma_groups[1], 1)
        ),
        "right_chroma_uv_have_ac": (
            len(chroma_groups) == 2
            and all(payload["eob"] >= 1 for payload in chroma_groups[1])
        ),
        "left_edge_varies": len(left_edge) == SIZE[1] and len(set(left_edge)) > 1,
    }
    return {
        "target": "luma_diagonal_down_right",
        "root_partition": blocks[0] if blocks else None,
        "group_count": len(groups),
        "y_modes": y_modes,
        "y_angle_symbols": y_angle_symbols,
        "y_angle_deltas": [symbol - 3 for symbol in y_angle_symbols],
        "y_angles": [135 + 3 * (symbol - 3) for symbol in y_angle_symbols],
        "uv_modes": uv_modes,
        "uv_angle_symbols": uv_angle_symbols,
        "left_edge": left_edge,
        "left_luma_payloads": luma_groups[0] if luma_groups else [],
        "right_luma_payloads": luma_groups[1] if len(luma_groups) == 2 else [],
        "left_chroma_payloads": chroma_groups[0] if chroma_groups else [],
        "right_chroma_payloads": chroma_groups[1] if len(chroma_groups) == 2 else [],
        "predicates": predicates,
        "rejection_reasons": [name for name, passed in predicates.items() if not passed],
        "qualifies": all(predicates.values()),
    }


def classify_luma_smooth(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
) -> dict[str, object]:
    """Apply exact predicates for the three right-leaf smooth modes."""

    parsed = [parse_group(group) for group in groups]
    y_modes = [mode for group in parsed for mode in group["y_modes"]]
    y_angle_symbols = [
        symbol for group in parsed for symbol in group["y_angle_symbols"]
    ]
    uv_modes = [mode for group in parsed for mode in group["uv_modes"]]
    uv_angle_symbols = [
        int(match["symbol"])
        for group in groups
        for line in group
        if (match := UVANGLE_PATTERN.match(line)) is not None
    ]
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
    expected_shape = [
        (0, 0, 0, 3, 0, 3),
        (0, 0, 0, 4, 0, 0),
        (0, 2, 0, 4, 0, 0),
    ]
    y_plane = yuv[: SIZE[0] * SIZE[1]]
    left_edge = [
        y_plane[row * SIZE[0] + 7]
        for row in range(SIZE[1])
    ] if len(y_plane) == SIZE[0] * SIZE[1] else []
    luma_groups = [group["luma_payloads"] for group in parsed]
    chroma_groups = [group["chroma_payloads"] for group in parsed]
    no_palette_or_filter = not any(
        line.startswith(
            (
                "Post-filterintramode[",
                "Post-y_pal[",
                "Post-pal[",
                "Post-y-pal-indices",
                "y-pal-pred",
                "Post-uv_pal[",
                "Post-uv-pal-indices",
                "uv-pal-pred",
            )
        )
        for group in groups
        for line in group
    )

    def is_tx8x8(payloads: list[dict[str, int]]) -> bool:
        return (
            len(payloads) == 1
            and payloads[0]["tx"] == 1
            and payloads[0]["txtp"] == 0
            and payloads[0]["eob"] >= 1
        )

    def is_tx4x4_split(payloads: list[dict[str, int]]) -> bool:
        return (
            len(payloads) == 4
            and all(
                payload["tx"] == 0
                and payload["txtp"] == 0
                and payload["eob"] >= 1
                for payload in payloads
            )
        )

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

    right_mode = y_modes[1] if len(y_modes) == 2 and y_modes[0] == 0 else None
    common_predicates = {
        "exact_visible_split_blocks": shape == expected_shape,
        "eight_bit_420_frame": (
            portable_color.get("width") == SIZE[0]
            and portable_color.get("height") == SIZE[1]
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "two_visible_square8_groups": len(groups) == 2,
        "one_origin_and_one_right_luma_mode": (
            len(y_modes) == 2 and y_modes[0] == 0
        ),
        "right_luma_mode_is_smooth": right_mode in (9, 10, 11),
        "no_luma_or_chroma_angle_symbols": (
            y_angle_symbols == [] and uv_angle_symbols == []
        ),
        "uv_modes_dc_then_dc": uv_modes == [0, 0],
        "no_palette_or_filter_intra": no_palette_or_filter,
        "origin_luma_is_unsplit_tx8x8": (
            len(luma_groups) == 2 and is_tx8x8(luma_groups[0])
        ),
        "left_chroma_is_dc_skipped": (
            len(chroma_groups) == 2 and is_skipped_chroma(chroma_groups[0], 0)
        ),
        "right_chroma_is_dc_skipped": (
            len(chroma_groups) == 2 and is_skipped_chroma(chroma_groups[1], 1)
        ),
        "left_edge_varies": len(left_edge) == SIZE[1] and len(set(left_edge)) > 1,
    }
    mode_predicates = {
        "9": (
            right_mode == 9
            and len(luma_groups) == 2
            and is_tx8x8(luma_groups[1])
        ),
        "10": (
            right_mode == 10
            and len(luma_groups) == 2
            and is_tx8x8(luma_groups[1])
        ),
        "11": (
            right_mode == 11
            and len(luma_groups) == 2
            and is_tx4x4_split(luma_groups[1])
        ),
    }
    predicates = {
        **common_predicates,
        "right_mode_9_has_unsplit_tx8x8": mode_predicates["9"],
        "right_mode_10_has_unsplit_tx8x8": mode_predicates["10"],
        "right_mode_11_has_split_tx4x4": mode_predicates["11"],
    }
    rejection_reasons = [
        name for name, passed in common_predicates.items() if not passed
    ]
    if right_mode in (9, 10, 11):
        mode_name = {
            9: "right_mode_9_has_unsplit_tx8x8",
            10: "right_mode_10_has_unsplit_tx8x8",
            11: "right_mode_11_has_split_tx4x4",
        }[right_mode]
        if not mode_predicates[str(right_mode)]:
            rejection_reasons.append(mode_name)
    else:
        rejection_reasons.append("right_mode_is_one_of_9_10_11")
    qualifies_mode = (
        right_mode
        if right_mode in (9, 10, 11)
        and all(common_predicates.values())
        and mode_predicates[str(right_mode)]
        else None
    )
    return {
        "target": "luma_smooth",
        "root_partition": blocks[0] if blocks else None,
        "group_count": len(groups),
        "y_modes": y_modes,
        "right_mode": right_mode,
        "uv_modes": uv_modes,
        "y_angle_symbols": y_angle_symbols,
        "uv_angle_symbols": uv_angle_symbols,
        "left_edge": left_edge,
        "left_luma_payloads": luma_groups[0] if luma_groups else [],
        "right_luma_payloads": luma_groups[1] if len(luma_groups) == 2 else [],
        "left_chroma_payloads": chroma_groups[0] if chroma_groups else [],
        "right_chroma_payloads": chroma_groups[1] if len(chroma_groups) == 2 else [],
        "mode_predicates": mode_predicates,
        "qualifies_mode": qualifies_mode,
        "predicates": predicates,
        "rejection_reasons": rejection_reasons,
        "qualifies": qualifies_mode is not None,
    }


def classify_luma_diagonal45(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
) -> dict[str, object]:
    """Apply exact predicates for an unsplit right-leaf 45-degree witness."""

    parsed = [parse_group(group) for group in groups]
    y_modes = [mode for group in parsed for mode in group["y_modes"]]
    y_angle_symbols = [
        symbol for group in parsed for symbol in group["y_angle_symbols"]
    ]
    uv_modes = [mode for group in parsed for mode in group["uv_modes"]]
    uv_angle_symbols = [
        int(match["symbol"])
        for group in groups
        for line in group
        if (match := UVANGLE_PATTERN.match(line)) is not None
    ]
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
    expected_shape = [
        (0, 0, 0, 3, 0, 3),
        (0, 0, 0, 4, 0, 0),
        (0, 2, 0, 4, 0, 0),
    ]
    y_plane = yuv[: SIZE[0] * SIZE[1]]
    left_edge = [
        y_plane[row * SIZE[0] + 7]
        for row in range(SIZE[1])
    ] if len(y_plane) == SIZE[0] * SIZE[1] else []
    luma_groups = [group["luma_payloads"] for group in parsed]
    chroma_groups = [group["chroma_payloads"] for group in parsed]
    no_palette_or_filter = not any(
        line.startswith(
            (
                "Post-filterintramode[",
                "Post-y_pal[",
                "Post-pal[",
                "Post-y-pal-indices",
                "y-pal-pred",
                "Post-uv_pal[",
                "Post-uv-pal-indices",
                "uv-pal-pred",
            )
        )
        for group in groups
        for line in group
    )

    def is_tx8x8(payloads: list[dict[str, int]]) -> bool:
        return (
            len(payloads) == 1
            and payloads[0]["tx"] == 1
            and payloads[0]["txtp"] == 0
            and payloads[0]["eob"] >= 0
        )

    def is_tx8x8_with_ac(payloads: list[dict[str, int]]) -> bool:
        return is_tx8x8(payloads) and payloads[0]["eob"] >= 1

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
        "exact_visible_split_blocks": shape == expected_shape,
        "eight_bit_420_frame": (
            portable_color.get("width") == SIZE[0]
            and portable_color.get("height") == SIZE[1]
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "two_visible_square8_groups": len(groups) == 2,
        "luma_modes_dc_then_diagonal45": y_modes == [0, 3],
        "right_luma_angle_symbol_is_zero_delta_45": (
            len(parsed) == 2
            and parsed[0]["y_angle_symbols"] == []
            and parsed[1]["y_angle_symbols"] == [3]
        ),
        "uv_modes_dc_then_dc": uv_modes == [0, 0],
        "no_uv_angle_symbols": uv_angle_symbols == [],
        "no_palette_or_filter_intra": no_palette_or_filter,
        "origin_luma_is_unsplit_tx8x8": (
            len(luma_groups) == 2 and is_tx8x8(luma_groups[0])
        ),
        "right_luma_is_unsplit_tx8x8": (
            len(luma_groups) == 2 and is_tx8x8_with_ac(luma_groups[1])
        ),
        "left_chroma_is_dc_skipped": (
            len(chroma_groups) == 2 and is_skipped_chroma(chroma_groups[0], 0)
        ),
        "right_chroma_is_dc_skipped": (
            len(chroma_groups) == 2 and is_skipped_chroma(chroma_groups[1], 1)
        ),
        "left_top_right_is_not_default": (
            len(left_edge) == SIZE[1] and left_edge[0] != 128
        ),
    }
    return {
        "target": "luma_diagonal45",
        "root_partition": blocks[0] if blocks else None,
        "group_count": len(groups),
        "y_modes": y_modes,
        "y_angle_symbols": y_angle_symbols,
        "y_angle_deltas": [symbol - 3 for symbol in y_angle_symbols],
        "y_angles": [45 + 3 * (symbol - 3) for symbol in y_angle_symbols],
        "uv_modes": uv_modes,
        "uv_angle_symbols": uv_angle_symbols,
        "left_edge": left_edge,
        "left_luma_payloads": luma_groups[0] if luma_groups else [],
        "right_luma_payloads": luma_groups[1] if len(luma_groups) == 2 else [],
        "left_chroma_payloads": chroma_groups[0] if chroma_groups else [],
        "right_chroma_payloads": chroma_groups[1] if len(chroma_groups) == 2 else [],
        "predicates": predicates,
        "rejection_reasons": [name for name, passed in predicates.items() if not passed],
        "qualifies": all(predicates.values()),
    }


def classify_luma_diagonal67(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
) -> dict[str, object]:
    """Apply exact predicates for a top-row D67 syntax witness.

    The right leaf has a left neighbor but no above neighbor.  dav1d therefore
    converts this coded near-vertical angle to the effective vertical mode and
    repeats the first left-edge sample across the synthetic top edge.
    """

    parsed = [parse_group(group) for group in groups]
    y_modes = [mode for group in parsed for mode in group["y_modes"]]
    y_angle_symbols = [
        symbol for group in parsed for symbol in group["y_angle_symbols"]
    ]
    uv_modes = [mode for group in parsed for mode in group["uv_modes"]]
    uv_angle_symbols = [
        int(match["symbol"])
        for group in groups
        for line in group
        if (match := UVANGLE_PATTERN.match(line)) is not None
    ]
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
    expected_shape = [
        (0, 0, 0, 3, 0, 3),
        (0, 0, 0, 4, 0, 0),
        (0, 2, 0, 4, 0, 0),
    ]
    y_plane = yuv[: SIZE[0] * SIZE[1]]
    left_edge = [
        y_plane[row * SIZE[0] + 7]
        for row in range(SIZE[1])
    ] if len(y_plane) == SIZE[0] * SIZE[1] else []
    luma_groups = [group["luma_payloads"] for group in parsed]
    chroma_groups = [group["chroma_payloads"] for group in parsed]
    no_palette_or_filter = not any(
        line.startswith(
            (
                "Post-filterintramode[",
                "Post-y_pal[",
                "Post-pal[",
                "Post-y-pal-indices",
                "y-pal-pred",
                "Post-uv_pal[",
                "Post-uv-pal-indices",
                "uv-pal-pred",
            )
        )
        for group in groups
        for line in group
    )

    def is_tx8x8(payloads: list[dict[str, int]], with_ac: bool) -> bool:
        return (
            len(payloads) == 1
            and payloads[0]["tx"] == 1
            and payloads[0]["txtp"] == 0
            and (payloads[0]["eob"] >= 1 if with_ac else payloads[0]["eob"] >= 0)
        )

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
        "exact_visible_split_blocks": shape == expected_shape,
        "eight_bit_420_frame": (
            portable_color.get("width") == SIZE[0]
            and portable_color.get("height") == SIZE[1]
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "two_visible_square8_groups": len(groups) == 2,
        "luma_modes_dc_then_diagonal67": y_modes == [0, 8],
        "right_luma_angle_symbol_is_zero_delta_67": (
            len(parsed) == 2
            and parsed[0]["y_angle_symbols"] == []
            and parsed[1]["y_angle_symbols"] == [3]
        ),
        "uv_modes_dc_then_dc": uv_modes == [0, 0],
        "no_uv_angle_symbols": uv_angle_symbols == [],
        "no_palette_or_filter_intra": no_palette_or_filter,
        "origin_luma_is_unsplit_tx8x8": (
            len(luma_groups) == 2 and is_tx8x8(luma_groups[0], False)
        ),
        "right_luma_is_unsplit_tx8x8_with_ac": (
            len(luma_groups) == 2 and is_tx8x8(luma_groups[1], True)
        ),
        "left_chroma_is_dc_skipped": (
            len(chroma_groups) == 2 and is_skipped_chroma(chroma_groups[0], 0)
        ),
        "right_chroma_is_dc_skipped": (
            len(chroma_groups) == 2 and is_skipped_chroma(chroma_groups[1], 1)
        ),
        "right_leaf_top_unavailable_left_available": shape == expected_shape,
        "left_edge_varies": len(left_edge) == SIZE[1] and len(set(left_edge)) > 1,
        "left_top_sample_is_not_midpoint": (
            len(left_edge) == SIZE[1] and left_edge[0] != 128
        ),
    }
    return {
        "target": "luma_diagonal67",
        "effective_predictor": "vertical_no_top_left_available",
        "root_partition": blocks[0] if blocks else None,
        "group_count": len(groups),
        "y_modes": y_modes,
        "y_angle_symbols": y_angle_symbols,
        "y_angle_deltas": [symbol - 3 for symbol in y_angle_symbols],
        "y_angles": [67 + 3 * (symbol - 3) for symbol in y_angle_symbols],
        "uv_modes": uv_modes,
        "uv_angle_symbols": uv_angle_symbols,
        "left_edge": left_edge,
        "left_luma_payloads": luma_groups[0] if luma_groups else [],
        "right_luma_payloads": luma_groups[1] if len(luma_groups) == 2 else [],
        "left_chroma_payloads": chroma_groups[0] if chroma_groups else [],
        "right_chroma_payloads": chroma_groups[1] if len(chroma_groups) == 2 else [],
        "predicates": predicates,
        "rejection_reasons": [name for name, passed in predicates.items() if not passed],
        "qualifies": all(predicates.values()),
    }


def classify_chroma_diagonal45(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
) -> dict[str, object]:
    """Apply exact predicates for the right-leaf chroma mode-3 witness."""

    parsed = [parse_group(group) for group in groups]
    y_modes = [mode for group in parsed for mode in group["y_modes"]]
    uv_modes = [mode for group in parsed for mode in group["uv_modes"]]
    uv_angle_symbols_by_group = [
        [
            int(match["symbol"])
            for line in group
            if (match := UVANGLE_PATTERN.match(line)) is not None
        ]
        for group in groups
    ]
    uv_angle_symbols = [symbol for group in uv_angle_symbols_by_group for symbol in group]
    right_uv_angle_symbols = (
        uv_angle_symbols_by_group[1] if len(uv_angle_symbols_by_group) == 2 else []
    )
    y_angle_symbols = [
        symbol for group in parsed for symbol in group["y_angle_symbols"]
    ]
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
    expected_shape = [
        (0, 0, 0, 3, 0, 3),
        (0, 0, 0, 4, 0, 0),
        (0, 2, 0, 4, 0, 0),
    ]
    luma_groups = [group["luma_payloads"] for group in parsed]
    chroma_groups = [group["chroma_payloads"] for group in parsed]
    no_palette_or_filter = not any(
        line.startswith(
            (
                "Post-filterintramode[",
                "Post-y_pal[",
                "Post-pal[",
                "Post-y-pal-indices",
                "y-pal-pred",
                "Post-uv_pal[",
                "Post-uv-pal-indices",
                "uv-pal-pred",
            )
        )
        for group in groups
        for line in group
    )

    def is_chroma_tx4x4(payloads: list[dict[str, int]], cbx4: int) -> bool:
        return (
            len(payloads) == 2
            and {payload["plane"] for payload in payloads} == {0, 1}
            and all(
                payload["tx"] == 0
                and payload["txtp"] == 0
                and payload["eob"] >= 0
                and payload.get("cbx4") == cbx4
                for payload in payloads
            )
        )

    predicates = {
        "exact_visible_split_blocks": shape == expected_shape,
        "eight_bit_420_frame": (
            portable_color.get("width") == SIZE[0]
            and portable_color.get("height") == SIZE[1]
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "two_visible_square8_groups": len(groups) == 2,
        "both_luma_modes_dc": y_modes == [0, 0],
        "no_luma_angle_symbols": y_angle_symbols == [],
        "uv_modes_dc_then_diagonal45": uv_modes == [0, 3],
        "right_uv_angle_symbol_is_plus_two_delta_51": right_uv_angle_symbols == [5],
        "no_palette_or_filter_intra": no_palette_or_filter,
        "left_chroma_is_tx4x4_dct": (
            len(chroma_groups) == 2 and is_chroma_tx4x4(chroma_groups[0], 0)
        ),
        "right_chroma_is_tx4x4_dct_nonempty": (
            len(chroma_groups) == 2 and is_chroma_tx4x4(chroma_groups[1], 1)
        ),
        "right_leaf_top_unavailable_left_available": (
            shape == expected_shape
            and blocks[1]["x"] == 0
            and blocks[1]["y"] == 0
            and blocks[2]["x"] == 2
            and blocks[2]["y"] == 0
        ),
    }
    return {
        "target": "chroma_diagonal45",
        "root_partition": blocks[0] if blocks else None,
        "group_count": len(groups),
        "y_modes": y_modes,
        "y_angle_symbols": y_angle_symbols,
        "uv_modes": uv_modes,
        "uv_angle_symbols": uv_angle_symbols,
        "uv_angle_deltas": [symbol - 3 for symbol in uv_angle_symbols],
        "uv_angles": [45 + 3 * (symbol - 3) for symbol in uv_angle_symbols],
        "uv_angle_symbols_by_group": uv_angle_symbols_by_group,
        "right_uv_angle_symbols": right_uv_angle_symbols,
        "right_uv_angle_deltas": [symbol - 3 for symbol in right_uv_angle_symbols],
        "right_uv_angles": [45 + 3 * (symbol - 3) for symbol in right_uv_angle_symbols],
        "qualifications": {
            "right_mode3_any_valid_angle": (
                uv_modes == [0, 3]
                and len(right_uv_angle_symbols) == 1
                and 0 <= right_uv_angle_symbols[0] <= 6
            ),
            "right_mode3_symbol3_delta0_angle45": (
                uv_modes == [0, 3] and right_uv_angle_symbols == [3]
            ),
            "right_mode3_symbol5_delta_plus2_angle51": (
                uv_modes == [0, 3] and right_uv_angle_symbols == [5]
            ),
        },
        "left_luma_payloads": luma_groups[0] if luma_groups else [],
        "right_luma_payloads": luma_groups[1] if len(luma_groups) == 2 else [],
        "left_chroma_payloads": chroma_groups[0] if chroma_groups else [],
        "right_chroma_payloads": chroma_groups[1] if len(chroma_groups) == 2 else [],
        "predicates": predicates,
        "rejection_reasons": [name for name, passed in predicates.items() if not passed],
        "qualifies": all(predicates.values()),
    }


def classify(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    target: str = "diagonal113",
    yuv: bytes = b"",
    portable_color: dict[str, object] | None = None,
) -> dict[str, object]:
    """Apply exact predicates for the selected bounded target."""

    if target == "luma_diagonal_down_right":
        if portable_color is None:
            raise ValueError("portable color metadata is required for the luma target")
        return classify_luma_diagonal_down_right(blocks, groups, yuv, portable_color)

    if target == "luma_smooth":
        if portable_color is None:
            raise ValueError("portable color metadata is required for the luma target")
        return classify_luma_smooth(blocks, groups, yuv, portable_color)

    if target == "luma_diagonal45":
        if portable_color is None:
            raise ValueError("portable color metadata is required for the luma target")
        return classify_luma_diagonal45(blocks, groups, yuv, portable_color)

    if target == "luma_diagonal67":
        if portable_color is None:
            raise ValueError("portable color metadata is required for the luma target")
        return classify_luma_diagonal67(blocks, groups, yuv, portable_color)

    if target == "chroma_diagonal45":
        if portable_color is None:
            raise ValueError("portable color metadata is required for the chroma target")
        return classify_chroma_diagonal45(blocks, groups, yuv, portable_color)

    if target != "diagonal113":
        raise ValueError(f"unknown Square8 campaign target: {target}")

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
    target: str,
) -> dict[str, object]:
    """Double-encode, double-trace, and classify one candidate."""

    pixels = candidate["pixels"]
    if not isinstance(pixels, bytes):
        raise TypeError("candidate pixels must be bytes")
    quality = int(candidate["quality"])
    speed = int(candidate["speed"])
    encoded = encode(pixels, quality, speed, target)
    encoded_second = encode(pixels, quality, speed, target)
    if encoded != encoded_second:
        raise RuntimeError(f"nondeterministic encoding for {candidate['id']}")
    path = work / f"{candidate['id']}.avif"
    path.write_bytes(encoded)
    if retain_dir is not None:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path.name).write_bytes(encoded)
    item, _ = extract_color_item(path)
    second_path = work / f"{candidate['id']}-second.avif"
    second_path.write_bytes(encoded_second)
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
    if len(yuv_a) != y_length + 2 * chroma_length:
        raise RuntimeError(
            f"unexpected 4:2:0 YUV length for {candidate['id']}: {len(yuv_a)}"
        )
    y_plane = yuv_a[:y_length]
    u_plane = yuv_a[y_length : y_length + chroma_length]
    v_plane = yuv_a[y_length + chroma_length :]
    return {
        "id": candidate["id"],
        "family": candidate["family"],
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
        **classify(blocks, groups, target, yuv_a, portable_color),
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
    parser.add_argument(
        "--target",
        choices=(
            "diagonal113",
            "luma_diagonal_down_right",
            "luma_smooth",
            "luma_diagonal45",
            "luma_diagonal67",
            "chroma_diagonal45",
        ),
        default="diagonal113",
    )
    args = parser.parse_args()
    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")
    with tempfile.TemporaryDirectory(prefix=f"image-star-avif-{args.target}-") as name:
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
                include_luma_angles=args.target
                in ("luma_diagonal_down_right", "luma_diagonal45", "luma_diagonal67"),
                include_block_angles=args.target == "chroma_diagonal45",
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
            )
            for candidate in candidates(args.target)
        ]
    if args.target == "luma_smooth":
        target_description = (
            "visible 16x8 right-hand Square8 leaf with luma mode 9 "
            "(Smooth), 10 (SmoothVertical), or 11 (SmoothHorizontal), "
            "DC chroma, and the observed mode-specific TX8x8/TX4x4 residual shape"
        )
        families = list(LUMA_SMOOTH_FAMILY_NAMES)
        rejection_reasons = (
            "exact_visible_split_blocks",
            "eight_bit_420_frame",
            "two_visible_square8_groups",
            "one_origin_and_one_right_luma_mode",
            "right_luma_mode_is_smooth",
            "no_luma_or_chroma_angle_symbols",
            "uv_modes_dc_then_dc",
            "no_palette_or_filter_intra",
            "origin_luma_is_unsplit_tx8x8",
            "left_chroma_is_dc_skipped",
            "right_chroma_is_dc_skipped",
            "left_edge_varies",
            "right_mode_9_has_unsplit_tx8x8",
            "right_mode_10_has_unsplit_tx8x8",
            "right_mode_11_has_split_tx4x4",
            "right_mode_is_one_of_9_10_11",
        )
    elif args.target == "luma_diagonal_down_right":
        target_description = (
            "visible 16x8 right-hand Square8 leaf with luma mode 4 "
            "(DiagonalDownRight), angle symbol 3 (135 degrees), DC chroma, "
            "split TX4x4 luma/chroma residuals, and right-hand AC"
        )
        families = list(LUMA_DIAGONAL_FAMILY_NAMES)
        rejection_reasons = (
            "exact_visible_split_blocks",
            "eight_bit_420_frame",
            "two_visible_square8_groups",
            "luma_modes_dc_then_diagonal_down_right",
            "luma_angle_symbol_is_zero_delta_135",
            "uv_modes_dc_then_dc",
            "no_uv_angle_symbols",
            "no_palette_or_filter_intra",
            "both_luma_leaves_are_split_tx4x4",
            "right_luma_has_ac",
            "left_chroma_is_tx4x4_dct",
            "right_chroma_is_tx4x4_dct",
            "right_chroma_uv_have_ac",
            "left_edge_varies",
        )
    elif args.target == "luma_diagonal45":
        target_description = (
            "visible 16x8 right-hand Square8 leaf with luma mode 3 "
            "(Diagonal45), angle symbol 3 (45 degrees), DC chroma, "
            "unsplit TX8x8 luma, and a non-default preceding top-right edge"
        )
        families = list(LUMA_DIAGONAL45_FAMILY_NAMES)
        rejection_reasons = (
            "exact_visible_split_blocks",
            "eight_bit_420_frame",
            "two_visible_square8_groups",
            "luma_modes_dc_then_diagonal45",
            "right_luma_angle_symbol_is_zero_delta_45",
            "uv_modes_dc_then_dc",
            "no_uv_angle_symbols",
            "no_palette_or_filter_intra",
            "origin_luma_is_unsplit_tx8x8",
            "right_luma_is_unsplit_tx8x8",
            "left_chroma_is_dc_skipped",
            "right_chroma_is_dc_skipped",
            "left_top_right_is_not_default",
        )
    elif args.target == "luma_diagonal67":
        target_description = (
            "visible 16x8 top-row right-hand Square8 leaf with luma mode 8 "
            "(Diagonal67), angle symbol 3 (67 degrees), effective vertical "
            "prediction from the available left edge, DC-skipped chroma, "
            "and unsplit TX8x8 luma with AC"
        )
        families = list(LUMA_DIAGONAL67_FAMILY_NAMES)
        rejection_reasons = (
            "exact_visible_split_blocks",
            "eight_bit_420_frame",
            "two_visible_square8_groups",
            "luma_modes_dc_then_diagonal67",
            "right_luma_angle_symbol_is_zero_delta_67",
            "uv_modes_dc_then_dc",
            "no_uv_angle_symbols",
            "no_palette_or_filter_intra",
            "origin_luma_is_unsplit_tx8x8",
            "right_luma_is_unsplit_tx8x8_with_ac",
            "left_chroma_is_dc_skipped",
            "right_chroma_is_dc_skipped",
            "right_leaf_top_unavailable_left_available",
            "left_edge_varies",
            "left_top_sample_is_not_midpoint",
        )
    elif args.target == "chroma_diagonal45":
        target_description = (
            "visible 16x8 right-hand Square8 leaf with coded chroma mode 3 "
            "(Diagonal45), angle symbol 5 (+2 steps, 51 degrees), TX4x4 DCT-DCT U/V "
            "residuals, DC luma, and top-unavailable/left-available edges"
        )
        families = list(CHROMA_DIAGONAL45_FAMILY_NAMES)
        rejection_reasons = (
            "exact_visible_split_blocks",
            "eight_bit_420_frame",
            "two_visible_square8_groups",
            "both_luma_modes_dc",
            "no_luma_angle_symbols",
            "uv_modes_dc_then_diagonal45",
            "right_uv_angle_symbol_is_plus_two_delta_51",
            "no_palette_or_filter_intra",
            "left_chroma_is_tx4x4_dct",
            "right_chroma_is_tx4x4_dct_nonempty",
            "right_leaf_top_unavailable_left_available",
        )
    else:
        target_description = (
            "visible right-hand Square8 leaf with coded UV mode 5 "
            "(Diagonal113), ADST_DCT chroma transform, and non-skipped "
            "chroma residual"
        )
        families = [
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
        ]
        rejection_reasons = (
            "visible_split_root",
            "two_visible_square8_groups",
            "right_uv_mode_5",
            "right_square8_luma",
            "right_adst_dct_chroma",
            "right_chroma_nonempty",
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
            "max_threads": 1,
            "autotiling": False,
            "advanced": advanced_for_target(args.target),
        },
        "search": {
            "candidate_count": len(reports),
            "target": target_description,
            "target_id": args.target,
            "families": families,
            "repository_rust_invoked": False,
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "by_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in rejection_reasons
            },
            **(
                {
                    "qualified_by_mode": {
                        str(mode): sum(
                            report.get("qualifies_mode") == mode
                            for report in reports
                        )
                        for mode in (9, 10, 11)
                    }
                }
                if args.target == "luma_smooth"
                else {}
            ),
        },
        **(
            {
                "qualified_candidates": [
                    report["id"] for report in reports if report["qualifies"]
                ],
                "promoted_candidates": {
                    str(mode): next(
                        (
                            report["id"]
                            for report in reports
                            if report.get("qualifies_mode") == mode
                        ),
                        None,
                    )
                    for mode in (9, 10, 11)
                },
            }
            if args.target == "luma_smooth"
            else {}
        ),
            **(
                {
                    "qualified_candidates": [
                        report["id"] for report in reports if report["qualifies"]
                    ]
                }
                if args.target in ("luma_diagonal45", "luma_diagonal67", "chroma_diagonal45")
                else {}
            ),
        **(
            {
                "qualification_counts": {
                    qualification: sum(
                        bool(report["qualifications"].get(qualification))
                        for report in reports
                    )
                    for qualification in (
                        "right_mode3_any_valid_angle",
                        "right_mode3_symbol3_delta0_angle45",
                        "right_mode3_symbol5_delta_plus2_angle51",
                    )
                }
            }
            if args.target == "chroma_diagonal45"
            else {}
        ),
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic {args.target} traces: {args.output}")


if __name__ == "__main__":
    main()
