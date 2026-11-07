#!/usr/bin/env python3
"""Generate the deterministic grayscale VP8 header-budget parity source."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/png/webp_vp8_header_budget_noise_gray_4096x3072.png"
)
WIDTH = 4_096
HEIGHT = 3_072
SEED = 0x5EED_2026
EXPECTED_PIXELS_SHA256 = "5c16d025c0764c693e8733e816e8f1e4888a726d58c8907f318cbc3249498b71"
EXPECTED_SOURCE_SHA256 = "f2821e63e4de32ca6113807da7ba2742c047610a72084368ea46e84c15668b39"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
DEFLATE_BLOCK_BYTES = 65_535


def source_pixels() -> bytes:
    """Build pseudo-random grayscale samples with a pinned xorshift stream."""
    state = SEED
    pixels = bytearray(WIDTH * HEIGHT)
    for index in range(len(pixels)):
        state ^= (state << 13) & 0xFFFF_FFFF
        state ^= state >> 17
        state ^= (state << 5) & 0xFFFF_FFFF
        state &= 0xFFFF_FFFF
        pixels[index] = state >> 24

    result = bytes(pixels)
    if hashlib.sha256(result).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("VP8 header-budget pixels differ from their pinned digest")
    return result


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Serialize one PNG chunk with a CRC over its type and payload."""
    checksum = zlib.crc32(kind + payload) & 0xFFFF_FFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def zlib_stored(data: bytes) -> bytes:
    """Wrap stored DEFLATE blocks in a deterministic zlib stream."""
    stream = bytearray((0x78, 0x01))
    for offset in range(0, len(data), DEFLATE_BLOCK_BYTES):
        block = data[offset : offset + DEFLATE_BLOCK_BYTES]
        final = int(offset + len(block) == len(data))
        stream.append(final)
        stream.extend(struct.pack("<HH", len(block), len(block) ^ 0xFFFF))
        stream.extend(block)
    stream.extend(struct.pack(">I", zlib.adler32(data) & 0xFFFF_FFFF))
    return bytes(stream)


def generate_fixture() -> bytes:
    """Build a grayscale PNG using platform-independent stored DEFLATE blocks."""
    pixels = source_pixels()
    row_bytes = WIDTH + 1
    scanlines = bytearray(row_bytes * HEIGHT)
    for row in range(HEIGHT):
        destination = row * row_bytes
        source = row * WIDTH
        scanlines[destination + 1 : destination + row_bytes] = pixels[
            source : source + WIDTH
        ]

    header = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 0, 0, 0, 0)
    fixture = (
        PNG_SIGNATURE
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", zlib_stored(scanlines))
        + png_chunk(b"IEND", b"")
    )
    if hashlib.sha256(fixture).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("VP8 header-budget PNG differs from its pinned digest")
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
            raise RuntimeError(
                f"generated fixture is stale or missing: {OUTPUT.relative_to(ROOT)}"
            )
        print(f"Verified {OUTPUT.relative_to(ROOT)}")
        return 0

    if OUTPUT.exists() and OUTPUT.read_bytes() != fixture:
        raise RuntimeError(
            f"refusing to replace a different fixture at {OUTPUT.relative_to(ROOT)}"
        )
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(fixture)
    print(f"Wrote {OUTPUT.relative_to(ROOT)}")
    print(f"source_sha256: {hashlib.sha256(fixture).hexdigest()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
