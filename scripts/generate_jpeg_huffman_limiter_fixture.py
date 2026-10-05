#!/usr/bin/env python3
"""Generate the grayscale source that forces JPEG Huffman length limiting."""

from __future__ import annotations

import hashlib
from fractions import Fraction
from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parent.parent
ASSET = ROOT / "tests" / "fixtures" / "input" / "images" / "png" / (
    "jpeg_huffman_length_limiter.png"
)
PINNED_PILLOW = "12.2.0"
EXPECTED_ASSET_SHA256 = (
    "21d7fefde68d0e4c9ee627c2222831fce271230e3365a8a921393bf37b554338"
)
EXPECTED_PIXELS_SHA256 = (
    "8c5f7f43d4a78548fe6c8054b4c6a74279da5d46afe367adbd95a1ec8da92a9c"
)
BLOCKS_WIDE = 313
BLOCKS_HIGH = 36
WIDTH = BLOCKS_WIDE * 8
HEIGHT = BLOCKS_HIGH * 8
FIXED_POINT_ONE = 1 << 30
ZIGZAG_TO_NATURAL = (
    0,
    1,
    8,
    16,
    9,
    2,
    3,
    10,
    17,
    24,
    32,
    25,
    18,
    11,
    4,
    5,
    12,
    19,
)
RARE_AC_COUNTS = (
    1,
    1,
    1,
    2,
    4,
    7,
    11,
    18,
    30,
    48,
    89,
    161,
    291,
    404,
    677,
    1072,
    2817,
)
# Q30 samples keep the source independent of platform libm rounding.
COSINE_BASIS_Q30 = (
    (
        1073741824,
        1073741824,
        1073741824,
        1073741824,
        1073741824,
        1073741824,
        1073741824,
        1073741824,
    ),
    (
        1053110176,
        892783698,
        596538995,
        209476638,
        -209476638,
        -596538995,
        -892783698,
        -1053110176,
    ),
    (
        992008094,
        410903207,
        -410903207,
        -992008094,
        -992008094,
        -410903207,
        410903207,
        992008094,
    ),
    (
        892783698,
        -209476638,
        -1053110176,
        -596538995,
        596538995,
        1053110176,
        209476638,
        -892783698,
    ),
    (
        759250125,
        -759250125,
        -759250125,
        759250125,
        759250125,
        -759250125,
        -759250125,
        759250125,
    ),
    (
        596538995,
        -1053110176,
        209476638,
        892783698,
        -892783698,
        -209476638,
        1053110176,
        -596538995,
    ),
    (
        410903207,
        -992008094,
        992008094,
        -410903207,
        -410903207,
        992008094,
        -992008094,
        410903207,
    ),
    (
        209476638,
        -596538995,
        892783698,
        -1053110176,
        1053110176,
        -892783698,
        596538995,
        -209476638,
    ),
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def source_pixels() -> bytes:
    """Place balanced AC frequencies in DCT basis blocks over a flat field."""
    block_count = BLOCKS_WIDE * BLOCKS_HIGH
    if sum(RARE_AC_COUNTS) * 2 != block_count:
        raise RuntimeError("rare AC and flat-block frequencies must be balanced")

    pixels = bytearray([128]) * (WIDTH * HEIGHT)
    block_index = 0
    for zigzag_index, frequency in enumerate(RARE_AC_COUNTS, start=1):
        natural_index = ZIGZAG_TO_NATURAL[zigzag_index]
        vertical_frequency, horizontal_frequency = divmod(natural_index, 8)
        for _ in range(frequency):
            block_x = block_index % BLOCKS_WIDE
            block_y = block_index // BLOCKS_WIDE
            for y in range(8):
                row = (block_y * 8 + y) * WIDTH + block_x * 8
                for x in range(8):
                    numerator = (
                        128 * FIXED_POINT_ONE * FIXED_POINT_ONE
                        + 116
                        * COSINE_BASIS_Q30[horizontal_frequency][x]
                        * COSINE_BASIS_Q30[vertical_frequency][y]
                    )
                    value = round(Fraction(numerator, FIXED_POINT_ONE * FIXED_POINT_ONE))
                    pixels[row + x] = min(255, max(0, value))
            block_index += 1

    if block_index * 2 != block_count:
        raise RuntimeError("generated AC blocks do not balance the remaining flat blocks")
    return bytes(pixels)


def generate() -> None:
    if Image.__version__ != PINNED_PILLOW:
        raise RuntimeError(f"Pillow version differs from pin: {Image.__version__}")

    pixels = source_pixels()
    if sha256(pixels) != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("generated grayscale pixels differ from their hash pin")

    image = Image.frombytes("L", (WIDTH, HEIGHT), pixels)
    ASSET.parent.mkdir(parents=True, exist_ok=True)
    image.save(ASSET, format="PNG", compress_level=9)
    asset_data = ASSET.read_bytes()
    if sha256(asset_data) != EXPECTED_ASSET_SHA256:
        raise RuntimeError("generated PNG differs from its hash pin")
    print(f"Wrote {ASSET} ({len(asset_data)} bytes, sha256 {EXPECTED_ASSET_SHA256})")


if __name__ == "__main__":
    generate()
