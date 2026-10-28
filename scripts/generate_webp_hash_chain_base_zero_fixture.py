#!/usr/bin/env python3
"""Generate RGB input for VP8L's token-aware base-zero chain exit."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "tests/fixtures/input/images/png/webp_lossless_hash_chain_base_zero_300x1.png"
WIDTH = 300
HEIGHT = 1
REPEATED_COLOR = 0xFF_00_00_00
THIRD_COLOR = 0xFF_AF_AC_39
THIRD_RESIDUAL = 0x00_03_AC_8D
TAIL_COLORS = 297
HASH_MULTIPLIER_HI = 0xC6A4_A793
HASH_MULTIPLIER_LO = 0x5BD1_E996
HASH_BITS = 18
EXPECTED_PIXEL_SHA256 = "b2f504f4bbd8cb4e284a5e0bf6f7179092c2ae7476769b2394eca0558f993769"
EXPECTED_SOURCE_SHA256 = "28dd1994d851eab95406c6c9bde9b04169e8fc96ac42af84a200c9593c45bbfd"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def pair_hash(first: int, second: int) -> int:
    """Mirror the VP8L pair hash for adjacent ARGB pixels."""
    key = (second * HASH_MULTIPLIER_HI + first * HASH_MULTIPLIER_LO) & 0xFFFF_FFFF
    return key >> (32 - HASH_BITS)


def subtract_green(color: int) -> int:
    """Mirror method-zero's green subtraction before predictor residuals."""
    green = (color >> 8) & 0xFF
    red = (((color >> 16) & 0xFF) - green) & 0xFF
    blue = ((color & 0xFF) - green) & 0xFF
    return (color & 0xFF00_FF00) | (red << 16) | blue


def subtract_pixels(color: int, prediction: int) -> int:
    """Mirror VP8L's wrapping per-channel predictor residual."""
    return sum(
        (((color >> shift) - (prediction >> shift)) & 0xFF) << shift
        for shift in (24, 16, 8, 0)
    )


def source_pixels() -> bytes:
    """Build an overlapping hash collision and bypass palette encoding."""
    colors = [REPEATED_COLOR, REPEATED_COLOR, THIRD_COLOR] + [
        0xFF00_0000
        | ((index & 0xFF) << 16)
        | ((index >> 8) << 8)
        | ((index * 73) & 0xFF)
        for index in range(TAIL_COLORS)
    ]
    if len(colors) != WIDTH * HEIGHT:
        raise RuntimeError("base-zero source colors do not match the 300-by-1 layout")
    if colors[0] != colors[1] or colors[2] in colors[:2] or colors[3] == colors[2]:
        raise RuntimeError("the overlapping pair prefix must not enter run insertion")
    if len(set(colors)) <= 256:
        raise RuntimeError("base-zero source must bypass VP8L palette encoding")
    transformed = [subtract_green(color) for color in colors[:3]]
    residuals = [
        subtract_pixels(transformed[0], 0xFF00_0000),
        subtract_pixels(transformed[1], transformed[0]),
        subtract_pixels(transformed[2], transformed[1]),
    ]
    if residuals != [0, 0, THIRD_RESIDUAL]:
        raise RuntimeError("method-zero's initial predictor residuals changed")
    if pair_hash(residuals[0], residuals[1]) != pair_hash(residuals[1], residuals[2]):
        raise RuntimeError("the overlapping leading pairs must share a VP8L hash")

    pixels = bytes(
        component
        for color in colors
        for component in ((color >> 16) & 0xFF, (color >> 8) & 0xFF, color & 0xFF)
    )
    if hashlib.sha256(pixels).hexdigest() != EXPECTED_PIXEL_SHA256:
        raise RuntimeError("base-zero source pixels differ from their pinned digest")
    return pixels


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Serialize one PNG chunk with its CRC."""
    checksum = zlib.crc32(kind + payload) & 0xFFFF_FFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def stored_zlib(data: bytes) -> bytes:
    """Wrap scanlines in one deterministic stored DEFLATE block."""
    if len(data) > 0xFFFF:
        raise ValueError("PNG scanline exceeds the single-block DEFLATE limit")
    length = len(data)
    block = b"\x01" + struct.pack("<HH", length, length ^ 0xFFFF) + data
    return b"\x78\x01" + block + struct.pack(">I", zlib.adler32(data) & 0xFFFF_FFFF)


def generate_fixture() -> bytes:
    """Build a deterministic 300-by-1 RGB PNG without external dependencies."""
    pixels = source_pixels()
    scanline = b"\x00" + pixels
    header = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 2, 0, 0, 0)
    fixture = (
        PNG_SIGNATURE
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", stored_zlib(scanline))
        + png_chunk(b"IEND", b"")
    )
    if hashlib.sha256(fixture).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("base-zero source PNG differs from its pinned digest")
    return fixture


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group()
    action.add_argument("--check", action="store_true", help="verify without writing")
    action.add_argument("--write", action="store_true", help="write the source PNG")
    arguments = parser.parse_args()
    fixture = generate_fixture()

    if arguments.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != fixture:
            raise RuntimeError(f"generated fixture is stale or missing: {OUTPUT.relative_to(ROOT)}")
        print(f"Verified {OUTPUT.relative_to(ROOT)}")
        return 0

    if OUTPUT.exists() and OUTPUT.read_bytes() != fixture:
        raise RuntimeError(f"refusing to replace a different fixture at {OUTPUT.relative_to(ROOT)}")
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(fixture)
    print(f"Wrote {OUTPUT.relative_to(ROOT)}")
    print(f"source_sha256: {hashlib.sha256(fixture).hexdigest()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
