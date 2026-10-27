#!/usr/bin/env python3
"""Generate or verify the VP8L sampled-map copy-checkpoint parity input."""

from __future__ import annotations

import argparse
import binascii
import hashlib
import random
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/png/webp_lossless_sampling_copy_checkpoint.png"
)
TILE_COLUMNS = 2_047
TILE_SIZE = 8
WIDTH = TILE_COLUMNS * TILE_SIZE
HEIGHT = TILE_SIZE
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
EXPECTED_SHA256 = "cded007438a8422352d2ff0caa0032487c139d0e594347af511409884cea32b6"


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
    """Build paired VP8L tile histograms with over 256 distinct colors."""
    permutations = []
    for tile_x in range(TILE_COLUMNS):
        pair = tile_x // 2
        values = random.Random(pair * 1_009 + 0x5EED).sample(range(256), 64)
        permutation = random.Random(tile_x * 1_009 + 0x5EED).sample(values, 64)
        permutations.append(permutation)

    scanlines = bytearray()
    for y in range(HEIGHT):
        row = bytearray()
        for tile_x in range(TILE_COLUMNS):
            green_group = (tile_x // 2) & 1
            for local_x in range(TILE_SIZE):
                value = permutations[tile_x][y * TILE_SIZE + local_x]
                pixel = (0, value, 0) if green_group else (value, 0, 0)
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
    action = parser.add_mutually_exclusive_group()
    action.add_argument("--check", action="store_true", help="verify without writing")
    action.add_argument("--write", action="store_true", help="write the source PNG")
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

    if OUTPUT.exists() and OUTPUT.read_bytes() != expected:
        raise SystemExit(f"refusing to replace a different fixture at {OUTPUT.relative_to(ROOT)}")
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(expected)
    print(f"wrote {OUTPUT.relative_to(ROOT)} sha256={digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
