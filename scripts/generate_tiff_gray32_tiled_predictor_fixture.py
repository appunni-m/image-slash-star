#!/usr/bin/env python3
"""Generate and verify a signed big-endian Gray32 Predictor 2 TIFF fixture."""

from __future__ import annotations

import argparse
import hashlib
import struct
import sys
import zlib
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/tiff/gray32_signed_be_deflate_predictor_contiguous_tiled.tiff"
)
WIDTH = 17
HEIGHT = 17
TILE_SIZE = 16
U32_MASK = (1 << 32) - 1
EXPECTED_SHA256 = "76c3020ca1ee787d4c0b077499039c29d111637d75c2e66f419b0e64a77784cb"


def sample(x: int, y: int) -> int:
    """Return varied signed samples, including values around the signed limits."""
    if (x, y) == (15, 15):
        return -(1 << 31)
    if (x, y) == (16, 15):
        return (1 << 31) - 1
    if (x, y) == (15, 16):
        return -1
    if (x, y) == (16, 16):
        return 0

    unsigned = (
        0x1020_3040 + x * 0x0123_4567 + y * 0x0765_4321 + x * y * 0x0001_0203
    ) & U32_MASK
    if (x + y) % 2:
        unsigned ^= 1 << 31
    return unsigned if unsigned < 1 << 31 else unsigned - (1 << 32)


def build_fixture() -> bytes:
    """Build a classic big-endian TIFF with four complete contiguous tiles."""

    def pack(code: str, *values: int) -> bytes:
        return struct.pack(">" + code, *values)

    def entry(tag: int, field_type: int, count: int, value: bytes) -> bytes:
        if len(value) > 4:
            raise ValueError(f"tag {tag} does not fit its inline TIFF field")
        return pack("HHI", tag, field_type, count) + value.ljust(4, b"\x00")

    tile_samples = TILE_SIZE * TILE_SIZE
    compressed_tiles = []
    tiles_across = (WIDTH + TILE_SIZE - 1) // TILE_SIZE
    tiles_down = (HEIGHT + TILE_SIZE - 1) // TILE_SIZE
    for tile_y in range(tiles_down):
        for tile_x in range(tiles_across):
            differences = []
            for local_y in range(TILE_SIZE):
                y = tile_y * TILE_SIZE + local_y
                previous = 0
                for local_x in range(TILE_SIZE):
                    x = tile_x * TILE_SIZE + local_x
                    value = sample(x, y) if x < WIDTH and y < HEIGHT else 0
                    value_bits = value & U32_MASK
                    differences.append((value_bits - previous) & U32_MASK)
                    previous = value_bits
            raw_tile = pack(f"{tile_samples}I", *differences)
            compressed_tiles.append(zlib.compress(raw_tile, level=9))

    entries_count = 13
    ifd_offset = 8
    ifd_end = ifd_offset + 2 + entries_count * 12 + 4
    tile_offsets_offset = (ifd_end + 3) & ~3
    tile_byte_counts_offset = tile_offsets_offset + len(compressed_tiles) * 4
    tile_data_offset = tile_byte_counts_offset + len(compressed_tiles) * 4

    tile_offsets = []
    cursor = tile_data_offset
    for compressed_tile in compressed_tiles:
        tile_offsets.append(cursor)
        cursor += len(compressed_tile)

    entries = [
        entry(256, 4, 1, pack("I", WIDTH)),
        entry(257, 4, 1, pack("I", HEIGHT)),
        entry(258, 3, 1, pack("H", 32)),
        entry(259, 3, 1, pack("H", 8)),
        entry(262, 3, 1, pack("H", 1)),
        entry(277, 3, 1, pack("H", 1)),
        entry(284, 3, 1, pack("H", 1)),
        entry(317, 3, 1, pack("H", 2)),
        entry(322, 4, 1, pack("I", TILE_SIZE)),
        entry(323, 4, 1, pack("I", TILE_SIZE)),
        entry(324, 4, len(tile_offsets), pack("I", tile_offsets_offset)),
        entry(325, 4, len(compressed_tiles), pack("I", tile_byte_counts_offset)),
        entry(339, 3, 1, pack("H", 2)),
    ]
    directory = pack("H", entries_count) + b"".join(entries) + pack("I", 0)
    encoded = bytearray(b"MM" + pack("HI", 42, ifd_offset) + directory)
    encoded.extend(b"\x00" * (tile_offsets_offset - len(encoded)))
    encoded.extend(pack(f"{len(tile_offsets)}I", *tile_offsets))
    encoded.extend(
        pack(f"{len(compressed_tiles)}I", *(len(tile) for tile in compressed_tiles))
    )
    encoded.extend(b"".join(compressed_tiles))

    if len(encoded) != cursor:
        raise RuntimeError(f"generated TIFF is {len(encoded)} bytes; expected {cursor}")
    fixture = bytes(encoded)
    digest = hashlib.sha256(fixture).hexdigest()
    if EXPECTED_SHA256 is not None and digest != EXPECTED_SHA256:
        raise RuntimeError(
            f"Gray32 tiled TIFF SHA-256 mismatch: expected {EXPECTED_SHA256}, found {digest}"
        )
    return fixture


def expected_pixels() -> bytes:
    # Pillow TIFF mode I preserves these decoded 32-bit samples in source byte order.
    return b"".join(
        struct.pack(">i", sample(x, y))
        for y in range(HEIGHT)
        for x in range(WIDTH)
    )


def verify_pillow_reference(encoded: bytes) -> None:
    """Check exact Pillow mode, pixels, and TIFF tags using the pinned oracle."""
    import yaml
    from PIL import Image

    sys.path.insert(0, str(ROOT / "scripts"))
    from generate_decode_refs import verify_primary_oracle

    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    with Image.open(BytesIO(encoded)) as image:
        if (image.format, image.mode, image.size) != ("TIFF", "I", (WIDTH, HEIGHT)):
            raise RuntimeError("Pillow opened the Gray32 TIFF with unexpected metadata")
        image.load()
        if image.tobytes() != expected_pixels():
            raise RuntimeError("Pillow Gray32 tiled pixels differ from the expected bytes")
        tags = image.tag_v2
        if (
            tuple(tags.get(258, ())) != (32,)
            or tags.get(259) != 8
            or tags.get(262) != 1
            or tags.get(277) != 1
            or tags.get(284) != 1
            or tags.get(317) != 2
            or tags.get(322) != TILE_SIZE
            or tags.get(323) != TILE_SIZE
            or tuple(tags.get(339, ())) != (2,)
            or len(tuple(tags.get(324, ()))) != 4
            or len(tuple(tags.get(325, ()))) != 4
        ):
            raise RuntimeError("Pillow TIFF tags differ from the signed Gray32 tile contract")
    with Image.open(BytesIO(encoded)) as image:
        image.verify()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify committed fixture bytes without rewriting them",
    )
    args = parser.parse_args()
    encoded = build_fixture()

    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != encoded:
            raise RuntimeError(f"{OUTPUT.relative_to(ROOT)} differs from generated bytes")
        print(f"Fixture matches: {OUTPUT.relative_to(ROOT)}")
        return 0

    verify_pillow_reference(encoded)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(encoded)
    digest = hashlib.sha256(encoded).hexdigest()
    print(f"Wrote {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes, SHA-256 {digest})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
