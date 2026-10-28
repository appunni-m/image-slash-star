#!/usr/bin/env python3
"""Generate RGB input for VP8L's token-aware method-zero window cutoff."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/png/webp_lossless_hash_chain_window_cutoff_512x32.png"
)
WIDTH = 512
HEIGHT = 32
WINDOW_SIZE = WIDTH << 4
EARLIER_PAIR = 1_024
TARGET_PAIR = 9_217
FIRST_PIXEL = 0xFF12_3456
EARLIER_SECOND_PIXEL = 0xFFAB_CDEF
TARGET_SECOND_PIXEL = 0xFF75_DCA0
HASH_MULTIPLIER_HI = 0xC6A4_A793
HASH_MULTIPLIER_LO = 0x5BD1_E996
HASH_BITS = 18
EXPECTED_PIXEL_SHA256 = "bc702acdd3795ca917a47945d886cc49a7fe21cc1ca0e1529d449ceb1b274e8c"
EXPECTED_SOURCE_SHA256 = "1e5c6be227246e38464ba17396ec6fe1d3a305fbbbd71dbc4b39f54025d7e9b0"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def pair_hash(first: int, second: int) -> int:
    """Mirror VP8L's 18-bit pair hash to validate the chosen coordinates."""
    key = (
        second * HASH_MULTIPLIER_HI + first * HASH_MULTIPLIER_LO
    ) & 0xFFFF_FFFF
    return key >> (32 - HASH_BITS)


def source_pixels() -> bytes:
    """Build over 256 colors with one same-hash pair just outside the window."""
    pixel_count = WIDTH * HEIGHT
    colors = [
        0xFF00_0000 | ((index * 0x9E37_79 + 0x01_23_45) & 0x00FF_FFFF)
        for index in range(pixel_count)
    ]
    colors[EARLIER_PAIR] = FIRST_PIXEL
    colors[EARLIER_PAIR + 1] = EARLIER_SECOND_PIXEL
    colors[TARGET_PAIR] = FIRST_PIXEL
    colors[TARGET_PAIR + 1] = TARGET_SECOND_PIXEL

    expected_hash = pair_hash(FIRST_PIXEL, EARLIER_SECOND_PIXEL)
    if pair_hash(FIRST_PIXEL, TARGET_SECOND_PIXEL) != expected_hash:
        raise RuntimeError("selected pair colors no longer share a VP8L hash")
    if TARGET_PAIR - EARLIER_PAIR != WINDOW_SIZE + 1:
        raise RuntimeError("selected pairs must straddle the quality-25 search window")

    history = [
        position
        for position in range(TARGET_PAIR)
        if pair_hash(colors[position], colors[position + 1]) == expected_hash
    ]
    if history != [EARLIER_PAIR]:
        raise RuntimeError(f"unexpected in-window pair-hash candidates: {history}")
    if len(set(colors)) <= 256:
        raise RuntimeError("window-cutoff source must bypass VP8L palette encoding")

    pixels = bytes(
        component
        for color in colors
        for component in ((color >> 16) & 0xFF, (color >> 8) & 0xFF, color & 0xFF)
    )
    if hashlib.sha256(pixels).hexdigest() != EXPECTED_PIXEL_SHA256:
        raise RuntimeError("window-cutoff source pixels differ from their pinned digest")
    return pixels


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Serialize a PNG chunk with its CRC."""
    checksum = zlib.crc32(kind + payload) & 0xFFFF_FFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def stored_zlib(data: bytes) -> bytes:
    """Wrap scanlines in deterministic stored DEFLATE blocks."""
    blocks = []
    for start in range(0, len(data), 0xFFFF):
        block = data[start : start + 0xFFFF]
        final = start + len(block) == len(data)
        blocks.append(
            (b"\x01" if final else b"\x00")
            + struct.pack("<HH", len(block), len(block) ^ 0xFFFF)
            + block
        )
    return b"\x78\x01" + b"".join(blocks) + struct.pack(">I", zlib.adler32(data) & 0xFFFF_FFFF)


def generate_fixture() -> bytes:
    """Build a deterministic 512-by-32 RGB PNG without external dependencies."""
    pixels = source_pixels()
    row_bytes = WIDTH * 3
    scanlines = b"".join(
        b"\x00" + pixels[row * row_bytes : (row + 1) * row_bytes]
        for row in range(HEIGHT)
    )
    header = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 2, 0, 0, 0)
    fixture = (
        PNG_SIGNATURE
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", stored_zlib(scanlines))
        + png_chunk(b"IEND", b"")
    )
    if hashlib.sha256(fixture).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("window-cutoff source PNG differs from its pinned digest")
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
