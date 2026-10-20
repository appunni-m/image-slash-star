#!/usr/bin/env python3
"""Generate the RGB PNG source for the WebP VP8L run-cap parity case."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "tests/fixtures/input/images/png/webp_lossless_run_cap_4356x1.png"
WIDTH = 4_356
RUN_PIXELS = 4_099
TAIL_COLORS = 257
RUN_COLOR = (255, 255, 255)
EXPECTED_PIXELS_SHA256 = "3206f3f36cf23cf01f6a3f2b696cb407eee7c80c81c05a0373154b4a165a6016"
EXPECTED_SOURCE_SHA256 = "038d4a47bb2b7f22b62eaac823a897c74607b20ee0ab6999e30b5dc84f6a1a93"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Serialize a PNG chunk with a CRC over its type and payload."""
    checksum = zlib.crc32(kind + payload) & 0xFFFF_FFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def source_pixels() -> bytes:
    """Build 4,099 equal pixels followed by 257 distinct RGB values."""
    tail = bytes(
        component
        for index in range(TAIL_COLORS)
        for component in (index & 0xFF, index >> 8, (index * 73) & 0xFF)
    )
    pixels = bytes(RUN_COLOR) * RUN_PIXELS + tail
    if len(pixels) != WIDTH * 3:
        raise RuntimeError("run-cap source pixels do not match the 4,356-by-1 layout")
    if pixels[: RUN_PIXELS * 3] != bytes(RUN_COLOR) * RUN_PIXELS:
        raise RuntimeError("run-cap source prefix is not a single repeated RGB color")
    if pixels[RUN_PIXELS * 3 : RUN_PIXELS * 3 + 3] == bytes(RUN_COLOR):
        raise RuntimeError("run-cap source tail continues the repeated prefix")
    tail_pixels = {
        tail[offset : offset + 3] for offset in range(0, len(tail), 3)
    }
    if len(tail_pixels) != TAIL_COLORS:
        raise RuntimeError("run-cap source tail does not contain 257 distinct colors")
    if hashlib.sha256(pixels).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("run-cap source pixels differ from their pinned digest")
    return pixels


def stored_zlib(data: bytes) -> bytes:
    """Wrap one deterministic uncompressed DEFLATE block in a zlib stream."""
    if len(data) > 0xFFFF:
        raise ValueError("PNG scanline exceeds the single-block DEFLATE limit")
    length = len(data)
    block = b"\x01" + struct.pack("<HH", length, length ^ 0xFFFF) + data
    return b"\x78\x01" + block + struct.pack(">I", zlib.adler32(data) & 0xFFFF_FFFF)


def generate_fixture() -> bytes:
    """Build a deterministic, dependency-free PNG byte stream."""
    pixels = source_pixels()
    scanline = b"\x00" + pixels
    header = struct.pack(">IIBBBBB", WIDTH, 1, 8, 2, 0, 0, 0)
    encoded = (
        PNG_SIGNATURE
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", stored_zlib(scanline))
        + png_chunk(b"IEND", b"")
    )
    if hashlib.sha256(encoded).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("run-cap source PNG differs from its pinned digest")
    return encoded


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
