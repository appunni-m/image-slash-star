#!/usr/bin/env python3
"""Generate or verify a VP8L row-mismatch early-stop parity input."""

from __future__ import annotations

import argparse
import binascii
import hashlib
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/png/webp_lossless_sampling_row_mismatch_8x16383.png"
)
WIDTH = 8
HEIGHT = 16_383
EXPECTED_SHA256 = "262b8efe57c954caf1068ea6e63969fbc205b667e85ce4788cc34aa5a75098d6"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Return one PNG chunk with its length and CRC fields."""
    checksum = binascii.crc32(kind + payload) & 0xFFFF_FFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def stored_zlib(data: bytes) -> bytes:
    """Wrap bytes in deterministic uncompressed DEFLATE blocks."""
    result = bytearray(b"\x78\x01")
    for offset in range(0, len(data), 65_535):
        block = data[offset : offset + 65_535]
        final = offset + len(block) == len(data)
        length = len(block)
        result.append(int(final))
        result.extend(struct.pack("<HH", length, length ^ 0xFFFF))
        result.extend(block)
    result.extend(struct.pack(">I", zlib.adler32(data) & 0xFFFF_FFFF))
    return bytes(result)


def fixture_bytes() -> bytes:
    """Build paired tiles with distinct red/green histogram populations."""
    scanlines = bytearray()
    for y in range(HEIGHT):
        tile_y = y // 8
        band = tile_y // 2
        green_group = band & 1
        multiplier = 37 if band % 2 == 0 else 5
        tile_high = tile_y >> 8
        tile_low = tile_y & 0xFF
        row = bytearray()
        for x in range(WIDTH):
            local_pixel = (y % 8) * WIDTH + x
            permuted = (local_pixel * multiplier + tile_y * 13) & 0x3F
            if green_group:
                pixel = (tile_high, 64 + permuted, tile_low)
            else:
                pixel = (64 + permuted, tile_high, tile_low)
            row.extend(pixel)
        scanlines.extend(b"\x00" + row)

    ihdr = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 2, 0, 0, 0)
    return (
        PNG_SIGNATURE
        + png_chunk(b"IHDR", ihdr)
        + png_chunk(b"IDAT", stored_zlib(bytes(scanlines)))
        + png_chunk(b"IEND", b"")
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the checked-in source fixture without writing it",
    )
    args = parser.parse_args()

    expected = fixture_bytes()
    digest = hashlib.sha256(expected).hexdigest()
    if EXPECTED_SHA256 and digest != EXPECTED_SHA256:
        raise SystemExit(f"generated source digest changed: {digest}")

    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != expected:
            raise SystemExit(f"fixture is stale or missing: {OUTPUT.relative_to(ROOT)}")
        print(f"verified {OUTPUT.relative_to(ROOT)} sha256={digest}")
        return 0

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(expected)
    print(f"wrote {OUTPUT.relative_to(ROOT)} sha256={digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
