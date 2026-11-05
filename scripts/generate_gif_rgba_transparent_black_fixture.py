#!/usr/bin/env python3
"""Generate a one-pixel transparent-black RGBA PNG for GIF palette parity."""

from __future__ import annotations

import argparse
import binascii
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "tests/fixtures/input/images/png/gif_rgba_transparent_black.png"
PIXEL = bytes((0, 0, 0, 0))


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    data = kind + payload
    checksum = binascii.crc32(data) & 0xFFFF_FFFF
    return struct.pack(">I", len(payload)) + data + struct.pack(">I", checksum)


def fixture_bytes() -> bytes:
    scanline = b"\x00" + PIXEL
    length = len(scanline)
    stored_block = b"\x01" + struct.pack("<HH", length, length ^ 0xFFFF) + scanline
    zlib_stream = b"\x78\x01" + stored_block + struct.pack(">I", zlib.adler32(scanline))
    header = struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", zlib_stream)
        + png_chunk(b"IEND", b"")
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify the committed fixture")
    args = parser.parse_args()

    expected = fixture_bytes()
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != expected:
            parser.error(f"{OUTPUT.relative_to(ROOT)} is stale; regenerate without --check")
        print(f"Verified {OUTPUT.relative_to(ROOT)}")
        return 0

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(expected)
    print(f"Generated {OUTPUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
