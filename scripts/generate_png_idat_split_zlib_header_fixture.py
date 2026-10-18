#!/usr/bin/env python3
"""Generate and verify a PNG whose zlib header spans adjacent IDAT chunks."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
import struct
from pathlib import Path
import zlib


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/png/1x1.png"
OUTPUT = ROOT / "tests/fixtures/input/images/png/idat_split_zlib_header_byte_1.png"
EXPECTED_SOURCE_SHA256 = "611033f4cc0aa9e3348e69eba089499bc6287d57b53832079e64f42564a4e876"
EXPECTED_FIXTURE_SHA256 = "0026074bebac404e7b9f28f3b55497abae304baa4f9398093f5cf1fec9ae4f52"
EXPECTED_PIXELS = bytes.fromhex("800000")
EXPECTED_PIXELS_SHA256 = "25ff297dd78c82468376d6e2c93b672b49c01fb125a2812b94f8f9693a51b67e"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
PINNED_IHDR = bytes.fromhex("00000001000000010802000000")


def read_chunks(data: bytes) -> list[tuple[bytes, bytes]]:
    """Read a complete PNG after checking every chunk boundary and CRC."""
    if not data.startswith(PNG_SIGNATURE):
        raise RuntimeError("PNG has an invalid signature")

    chunks = []
    position = len(PNG_SIGNATURE)
    while position < len(data):
        if len(data) - position < 12:
            raise RuntimeError("PNG contains a truncated chunk")
        payload_length = struct.unpack_from(">I", data, position)[0]
        payload_start = position + 8
        chunk_end = payload_start + payload_length + 4
        if chunk_end > len(data):
            raise RuntimeError("PNG chunk exceeds the file bounds")

        kind = data[position + 4 : payload_start]
        payload = data[payload_start : payload_start + payload_length]
        expected_crc = struct.unpack_from(">I", data, payload_start + payload_length)[0]
        if zlib.crc32(kind + payload) & 0xFFFF_FFFF != expected_crc:
            raise RuntimeError(f"PNG {kind!r} chunk has an invalid CRC")

        chunks.append((kind, payload))
        position = chunk_end
        if kind == b"IEND":
            break

    if position != len(data) or not chunks or chunks[-1][0] != b"IEND":
        raise RuntimeError("PNG is incomplete or contains trailing bytes")
    return chunks


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Serialize one PNG chunk with its CRC."""
    if len(kind) != 4 or len(payload) > 0xFFFF_FFFF:
        raise RuntimeError("PNG chunk kind or payload exceeds its format limit")
    return (
        struct.pack(">I", len(payload))
        + kind
        + payload
        + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFF_FFFF)
    )


def make_fixture(source: bytes) -> bytes:
    """Split the pinned 1x1 PNG's zlib stream after its first byte."""
    if sha256(source).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("1x1 PNG source differs from its pinned hash")

    chunks = read_chunks(source)
    if [kind for kind, _ in chunks] != [b"IHDR", b"IDAT", b"IEND"]:
        raise RuntimeError("source PNG differs from the pinned one-pixel layout")
    if chunks[0][1] != PINNED_IHDR:
        raise RuntimeError("source PNG is not the pinned 1x1 RGB8 image")
    compressed = chunks[1][1]
    if len(compressed) < 2:
        raise RuntimeError("source PNG IDAT is too short to split its zlib header")

    fixture = PNG_SIGNATURE + b"".join(
        (
            png_chunk(b"IHDR", chunks[0][1]),
            png_chunk(b"IDAT", compressed[:1]),
            png_chunk(b"IDAT", compressed[1:]),
            png_chunk(b"IEND", chunks[2][1]),
        )
    )
    fixture_chunks = read_chunks(fixture)
    if [kind for kind, _ in fixture_chunks] != [b"IHDR", b"IDAT", b"IDAT", b"IEND"]:
        raise RuntimeError("fixture must split image data across two adjacent IDAT chunks")
    if fixture_chunks[1][1] + fixture_chunks[2][1] != compressed:
        raise RuntimeError("fixture changed the source PNG zlib stream")
    if sha256(fixture).hexdigest() != EXPECTED_FIXTURE_SHA256:
        raise RuntimeError("generated split-IDAT PNG differs from its pinned hash")
    return fixture


def verify_pillow(data: bytes) -> None:
    """Check exact decoded pixels using the pinned Pillow oracle."""
    import PIL
    from PIL import Image

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PIL.__version__}")

    with Image.open(BytesIO(data)) as image:
        image.verify()

    with Image.open(BytesIO(data)) as image:
        image.load()
        pixels = image.tobytes()
        if (
            image.format != "PNG"
            or image.mode != "RGB"
            or image.size != (1, 1)
            or image.n_frames != 1
            or pixels != EXPECTED_PIXELS
            or sha256(pixels).hexdigest() != EXPECTED_PIXELS_SHA256
        ):
            raise RuntimeError("Pillow returned unexpected split-IDAT PNG pixels")


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
        help="verify the pinned Pillow 12.2.0 pixels",
    )
    arguments = parser.parse_args()

    fixture = make_fixture(SOURCE.read_bytes())
    if arguments.verify_pillow or not arguments.check:
        verify_pillow(fixture)

    if arguments.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != fixture:
            raise RuntimeError(f"generated fixture is stale or missing: {OUTPUT.relative_to(ROOT)}")
        print(f"Verified {OUTPUT.relative_to(ROOT)}")
        return 0

    if OUTPUT.exists() and OUTPUT.read_bytes() != fixture:
        raise RuntimeError(f"refusing to replace a different fixture at {OUTPUT.relative_to(ROOT)}")
    OUTPUT.write_bytes(fixture)
    print(f"Wrote {OUTPUT.relative_to(ROOT)} ({len(fixture)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
