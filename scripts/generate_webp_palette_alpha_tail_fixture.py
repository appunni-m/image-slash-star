#!/usr/bin/env python3
"""Generate an RGBA input for VP8L palette mode with transparent black."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "tests/fixtures/input/images/png/webp_lossless_palette_alpha_tail_64x64.png"
WIDTH = 64
HEIGHT = 64
PALETTE = ((0, 0, 0, 0),) + tuple(
    (red, 0, 0, 255) for red in (*range(128, 249, 8), 255)
)
EXPECTED_PIXELS_SHA256 = "50f10bdd5a8fe762eeb87d2326daf4b27c699a504136c2ad74013068352b6f8e"
EXPECTED_SOURCE_SHA256 = "b749e47aa0f306c0c42c21e9d55ad6ddb36dfcfb419be5c3785f9bf8ce79f936"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Return one PNG chunk with a deterministic CRC."""
    checksum = zlib.crc32(kind + payload) & 0xFFFF_FFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def stored_zlib(data: bytes) -> bytes:
    """Wrap scanlines in deterministic uncompressed DEFLATE blocks."""
    result = bytearray(b"\x78\x01")
    for offset in range(0, len(data), 65_535):
        block = data[offset : offset + 65_535]
        final = offset + len(block) == len(data)
        result.append(int(final))
        result.extend(struct.pack("<HH", len(block), len(block) ^ 0xFFFF))
        result.extend(block)
    result.extend(struct.pack(">I", zlib.adler32(data) & 0xFFFF_FFFF))
    return bytes(result)


def pixel_bytes() -> bytes:
    """Build an irregular repeated palette with transparent black and mixed red deltas."""
    pixels = bytearray()
    state = 0x7A11_C0DE
    for _ in range(WIDTH * HEIGHT):
        state ^= (state << 13) & 0xFFFF_FFFF
        state ^= state >> 17
        state ^= (state << 5) & 0xFFFF_FFFF
        palette_index = ((state >> 16) ^ state) % len(PALETTE)
        pixels.extend(PALETTE[palette_index])
    result = bytes(pixels)
    if hashlib.sha256(result).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("WebP palette-tail pixel source differs from its pinned digest")
    return result


def fixture_bytes() -> bytes:
    """Encode the deterministic RGBA stimulus as a complete PNG source."""
    pixels = pixel_bytes()
    scanlines = b"".join(
        b"\x00" + pixels[row * WIDTH * 4 : (row + 1) * WIDTH * 4]
        for row in range(HEIGHT)
    )
    header = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 6, 0, 0, 0)
    fixture = (
        PNG_SIGNATURE
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", stored_zlib(scanlines))
        + png_chunk(b"IEND", b"")
    )
    if hashlib.sha256(fixture).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("WebP palette-tail PNG differs from its pinned digest")
    return fixture


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group()
    action.add_argument("--check", action="store_true", help="verify without writing")
    action.add_argument("--write", action="store_true", help="write the source PNG")
    arguments = parser.parse_args()
    fixture = fixture_bytes()

    if arguments.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != fixture:
            raise SystemExit(f"generated fixture is stale or missing: {OUTPUT.relative_to(ROOT)}")
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
