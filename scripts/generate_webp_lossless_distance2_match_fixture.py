#!/usr/bin/env python3
"""Generate the alternating RGB row for a VP8L distance-two match case."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/png/webp_lossless_distance2_max_match_tail_8451x1.png"
)
RUN_PIXELS = 8_194
TAIL_COLORS = 257
WIDTH = RUN_PIXELS + TAIL_COLORS
FIRST_COLOR = bytes((240, 20, 30))
SECOND_COLOR = bytes((12, 210, 40))
EXPECTED_PREFIX_SHA256 = "483b34e11794cda5df1aace83130e85bb4404843bc55ecd543c091288975f70b"
EXPECTED_PIXELS_SHA256 = "5834138628a72670296997b5593616138423457d4bb6a3b575cd41fba47d8bf6"
EXPECTED_SOURCE_SHA256 = "242459784becc93c3faf0d20fe68fa0a5979ec529d404574c22b7964842e1784"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Serialize a PNG chunk with a CRC over its type and payload."""
    checksum = zlib.crc32(kind + payload) & 0xFFFF_FFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def source_pixels() -> bytes:
    """Build a repeating prefix and distinct-color tail for VP8L search."""
    prefix = (FIRST_COLOR + SECOND_COLOR) * (RUN_PIXELS // 2)
    if hashlib.sha256(prefix).hexdigest() != EXPECTED_PREFIX_SHA256:
        raise RuntimeError("distance-two source prefix differs from its pinned digest")
    tail = bytes(
        component
        for index in range(TAIL_COLORS)
        for component in (index & 0xFF, index >> 8, (index * 73) & 0xFF)
    )
    pixels = prefix + tail
    if len(pixels) != WIDTH * 3:
        raise RuntimeError("distance-two source pixels do not match the 8,451-by-1 layout")
    if prefix[:12] != (FIRST_COLOR + SECOND_COLOR) * 2:
        raise RuntimeError("distance-two source prefix is not alternating")
    if tail[:3] == prefix[-3:]:
        raise RuntimeError("distance-two source tail continues the alternating prefix")
    tail_pixels = {tail[offset : offset + 3] for offset in range(0, len(tail), 3)}
    if len(tail_pixels) != TAIL_COLORS:
        raise RuntimeError("distance-two source tail does not contain 257 distinct colors")
    if hashlib.sha256(pixels).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("distance-two source pixels differ from their pinned digest")
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
    scanline = b"\x00" + source_pixels()
    header = struct.pack(">IIBBBBB", WIDTH, 1, 8, 2, 0, 0, 0)
    encoded = (
        PNG_SIGNATURE
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", stored_zlib(scanline))
        + png_chunk(b"IEND", b"")
    )
    if hashlib.sha256(encoded).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("distance-two source PNG differs from its pinned digest")
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
