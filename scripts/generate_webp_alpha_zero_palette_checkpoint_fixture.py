#!/usr/bin/env python3
"""Generate a transparent 121-value alpha palette with mixed delta signs."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "tests/fixtures/input/images/png/webp_rgba_alpha_zero_palette_gap_512x512.png"
WIDTH = 512
HEIGHT = 512
EXPECTED_PIXELS_SHA256 = "7c8bee506bf44fffd787780a3324fe9090a06062e486d4552246cb0680752dd2"
EXPECTED_SOURCE_SHA256 = "0d5a554fec44c2de7f6e04f7c03f7c4fa9da67fb5acbc6cbd392a91cedd3d168"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Return one PNG chunk with its deterministic CRC."""
    checksum = zlib.crc32(kind + payload) & 0xFFFF_FFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def stored_zlib(data: bytes) -> bytes:
    """Wrap the scanline in one deterministic uncompressed DEFLATE block."""
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
    """Build the repeated alpha-gap pattern with its lowest value transparent."""
    pixels = bytearray()
    for _y in range(HEIGHT):
        for x in range(WIDTH):
            residue = x % 121
            alpha = (
                0
                if residue == 0
                else residue + 1
                if residue < 100
                else 230 + residue - 100
            )
            pixels.extend((128, 128, 128, alpha))
    pixels = bytes(pixels)
    if hashlib.sha256(pixels).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("WebP alpha palette pixel source differs from its pinned digest")
    return pixels


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
        raise RuntimeError("WebP alpha palette PNG differs from its pinned digest")
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
