#!/usr/bin/env python3
"""Generate the tiny RGBA source for VP8L predictor-grid parity."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "tests/fixtures/input/images/png/webp_tiny_predictor_grid_8x3.png"
WIDTH = 8
HEIGHT = 3
EXPECTED_PIXELS_SHA256 = "580e7bc47e167835f14407e6fddc86eef7579f15841df488b1698c7f91232609"
EXPECTED_SOURCE_SHA256 = "6dd52bc01af49b9cf5949e83094117653646a442d323e45b52d2b086ea528e8d"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def source_pixels() -> bytes:
    """Build 24 distinct RGBA colors in an eight-by-three image."""
    pixels = bytes(
        channel
        for index in range(WIDTH * HEIGHT)
        for channel in (
            (index * 37 + 11) & 0xFF,
            (index * 73 + 23) & 0xFF,
            (index * 109 + 47) & 0xFF,
            64 + index * 8,
        )
    )
    if len({pixels[offset : offset + 4] for offset in range(0, len(pixels), 4)}) != 24:
        raise RuntimeError("tiny predictor-grid source must contain 24 colors")
    if hashlib.sha256(pixels).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("tiny predictor-grid pixels differ from their pinned digest")
    return pixels


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Serialize a PNG chunk with a CRC over its type and payload."""
    checksum = zlib.crc32(kind + payload) & 0xFFFF_FFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def generate_fixture() -> bytes:
    """Build a deterministic RGBA PNG without external dependencies."""
    pixels = source_pixels()
    row_bytes = WIDTH * 4
    scanlines = b"".join(
        b"\x00" + pixels[row * row_bytes : (row + 1) * row_bytes]
        for row in range(HEIGHT)
    )
    compressed = (
        b"\x78\x01"
        + b"\x01"
        + struct.pack("<HH", len(scanlines), len(scanlines) ^ 0xFFFF)
        + scanlines
        + struct.pack(">I", zlib.adler32(scanlines) & 0xFFFF_FFFF)
    )
    header = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 6, 0, 0, 0)
    fixture = (
        PNG_SIGNATURE
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", compressed)
        + png_chunk(b"IEND", b"")
    )
    if hashlib.sha256(fixture).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("tiny predictor-grid PNG differs from its pinned digest")
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
