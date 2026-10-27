#!/usr/bin/env python3
"""Generate a two-color PNG for the VP8L box-chain match-length cap."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "tests/fixtures/input/images/png/webp_box_chain_max_match_8196x3.png"
PACKED_WIDTH = 4_098
WIDTH = PACKED_WIDTH * 2
HEIGHT = 3
COLORS = (
    bytes((255, 0, 0)),
    bytes((0, 255, 0)),
    bytes((0, 0, 255)),
    bytes((255, 255, 0)),
    bytes((255, 0, 255)),
    bytes((0, 255, 255)),
)
GROUP_PATTERN = ((0, 1), (2, 3), (4, 5), (0, 2), (1, 4), (3, 5))
EXPECTED_PIXELS_SHA256 = "809865969461d792ec552fbe8040f3f59abc39fa4f4da222d3b715d058f2dfe6"
EXPECTED_SOURCE_SHA256 = "c07464a1965f2fd029193f05cb0f1da90740d046570b30a66bd86dd2d2ea5df3"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Serialize a PNG chunk with a CRC over its type and payload."""
    checksum = zlib.crc32(kind + payload) & 0xFFFF_FFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def source_pixels() -> bytes:
    """Build three rows whose packed palette stream has a long box-chain match."""
    groups = [GROUP_PATTERN[index % len(GROUP_PATTERN)] for index in range(PACKED_WIDTH)]
    groups.append(GROUP_PATTERN[4])
    match_distance = PACKED_WIDTH + 1
    while len(groups) < PACKED_WIDTH * HEIGHT:
        groups.append(groups[len(groups) - match_distance])
    pixels = bytes(
        component
        for first, second in groups
        for color_index in (first, second)
        for component in COLORS[color_index]
    )
    if len(pixels) != WIDTH * HEIGHT * 3:
        raise RuntimeError("box-chain source pixels do not match the 8,196-by-3 layout")
    if len({pixels[offset : offset + 3] for offset in range(0, len(pixels), 3)}) != len(COLORS):
        raise RuntimeError("box-chain source must contain exactly six palette colors")
    if hashlib.sha256(pixels).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("box-chain source pixels differ from their pinned digest")
    return pixels


def stored_zlib(data: bytes) -> bytes:
    """Wrap deterministic uncompressed DEFLATE blocks in a zlib stream."""
    blocks = []
    for start in range(0, len(data), 0xFFFF):
        block = data[start : start + 0xFFFF]
        final = start + len(block) == len(data)
        header = b"\x01" if final else b"\x00"
        blocks.append(header + struct.pack("<HH", len(block), len(block) ^ 0xFFFF) + block)
    return b"\x78\x01" + b"".join(blocks) + struct.pack(">I", zlib.adler32(data) & 0xFFFF_FFFF)


def generate_fixture() -> bytes:
    """Build the deterministic PNG input without external dependencies."""
    pixels = source_pixels()
    row_bytes = WIDTH * 3
    scanline = b"".join(
        b"\x00" + pixels[row * row_bytes : (row + 1) * row_bytes]
        for row in range(HEIGHT)
    )
    header = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 2, 0, 0, 0)
    fixture = (
        PNG_SIGNATURE
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", stored_zlib(scanline))
        + png_chunk(b"IEND", b"")
    )
    if hashlib.sha256(fixture).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("box-chain source PNG differs from its pinned digest")
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
