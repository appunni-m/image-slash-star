#!/usr/bin/env python3
"""Generate the Pillow 12.2.0 source for odd-width 1-bit PNG encoding."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from io import BytesIO
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "tests/fixtures/input/images/png/1bit_odd_width.png"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
WIDTH = 9
HEIGHT = 3
ROW_BYTES = (WIDTH + 7) // 8
PIXEL_ROWS = (b"\x59\x80", b"\xa6\x00", b"\x00\x80")
EXPECTED_PIXELS_SHA256 = "36e3c5d288415910fd696272299e53b64a80b66782e965c506b57e7dd2d8d203"
EXPECTED_SOURCE_SHA256 = "ab470446e941a51011e157e861e3f920b911686d3e592e1b7cb1d6a683bdd675"
PILLOW_ZLIB_STREAM = bytes.fromhex("789c638c546758c6c0c0d0000007a901a8")
PILLOW_FILTERED_SCANLINES = bytes.fromhex("01592700a600000080")


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Serialize a PNG chunk with a checked length and CRC."""
    if len(payload) > 0xFFFF_FFFF:
        raise ValueError("PNG chunk payload exceeds its 32-bit length field")
    checksum = zlib.crc32(kind + payload) & 0xFFFF_FFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def generate_fixture() -> bytes:
    """Reproduce Pillow's pinned PNG bytes without runtime Pillow dependencies."""
    if len(PIXEL_ROWS) != HEIGHT or any(len(row) != ROW_BYTES for row in PIXEL_ROWS):
        raise ValueError("pinned grayscale pixels must match the 9-by-3 row layout")

    pixels = b"".join(PIXEL_ROWS)
    pixel_sha256 = hashlib.sha256(pixels).hexdigest()
    if pixel_sha256 != EXPECTED_PIXELS_SHA256:
        raise RuntimeError(
            f"pinned pixel SHA-256 mismatch: expected {EXPECTED_PIXELS_SHA256}, "
            f"found {pixel_sha256}"
        )

    if zlib.decompress(PILLOW_ZLIB_STREAM) != PILLOW_FILTERED_SCANLINES:
        raise RuntimeError("pinned Pillow IDAT stream does not match its filtered rows")

    header = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 1, 0, 0, 0, 0)
    encoded = (
        PNG_SIGNATURE
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", PILLOW_ZLIB_STREAM)
        + png_chunk(b"IEND", b"")
    )
    source_sha256 = hashlib.sha256(encoded).hexdigest()
    if source_sha256 != EXPECTED_SOURCE_SHA256:
        raise RuntimeError(
            f"generated source SHA-256 mismatch: expected {EXPECTED_SOURCE_SHA256}, "
            f"found {source_sha256}"
        )
    return encoded


def verify_pillow(data: bytes) -> None:
    """Confirm the fixture matches a Pillow 12.2.0 save and pixel roundtrip."""
    try:
        import PIL
        from PIL import Image
    except ImportError as error:
        raise RuntimeError("Pillow 12.2.0 is required for --verify-pillow") from error

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PIL.__version__}")

    image = Image.new("1", (WIDTH, HEIGHT))
    image.frombytes(b"".join(PIXEL_ROWS))
    generated = BytesIO()
    image.save(generated, format="PNG")
    if generated.getvalue() != data:
        raise RuntimeError("fixture bytes differ from the pinned Pillow PNG save")

    with Image.open(BytesIO(data)) as decoded:
        decoded.load()
        if (
            decoded.format != "PNG"
            or decoded.mode != "1"
            or decoded.size != (WIDTH, HEIGHT)
        ):
            raise RuntimeError("Pillow decoded unexpected source PNG metadata")
        if hashlib.sha256(decoded.tobytes()).hexdigest() != EXPECTED_PIXELS_SHA256:
            raise RuntimeError("Pillow decoded pixels differ from the pinned 9-by-3 raster")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed fixture without writing it",
    )
    parser.add_argument(
        "--verify-pillow",
        action="store_true",
        help="verify the pinned Pillow 12.2.0 save and pixel roundtrip",
    )
    arguments = parser.parse_args()
    fixture = generate_fixture()

    if arguments.verify_pillow:
        verify_pillow(fixture)

    if arguments.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != fixture:
            raise RuntimeError(f"generated fixture is stale or missing: {OUTPUT.relative_to(ROOT)}")
        print(f"Verified {OUTPUT.relative_to(ROOT)}")
        return 0

    if OUTPUT.exists() and OUTPUT.read_bytes() != fixture:
        raise RuntimeError(f"refusing to replace a different fixture at {OUTPUT.relative_to(ROOT)}")
    OUTPUT.write_bytes(fixture)
    print(f"Wrote {OUTPUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
