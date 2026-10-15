#!/usr/bin/env python3
"""Create and verify a Pillow-tolerated PNG zTXt chunk without a separator."""

from __future__ import annotations

import hashlib
import struct
import zlib
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as PILLOW_VERSION


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/png/1x1.png"
OUTPUT = ROOT / "tests/fixtures/input/images/png/ztxt_missing_keyword_separator.png"
EXPECTED_SOURCE_SHA256 = "611033f4cc0aa9e3348e69eba089499bc6287d57b53832079e64f42564a4e876"
EXPECTED_FIXTURE_SHA256 = "5d58b8f1d999ec34d587dc6ead66ded839aa8de614f0bbaafe4225c193c8908a"
EXPECTED_PIXELS_SHA256 = "25ff297dd78c82468376d6e2c93b672b49c01fb125a2812b94f8f9693a51b67e"
TEXT_ZLIB_STREAM = bytes.fromhex(
    "789cf30caf5674757656cb2eb52a04327c1c7472ab013e030602"
)
TEXT_VALUE = b"IW{!ECC&ku:q!ECL@,m{"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_chunks(data: bytes) -> list[tuple[bytes, bytes, int, int]]:
    """Return complete PNG chunks after validating their bounds and CRCs."""
    if not data.startswith(PNG_SIGNATURE):
        raise RuntimeError("source PNG has an invalid signature")

    chunks = []
    position = len(PNG_SIGNATURE)
    while position < len(data):
        if len(data) - position < 12:
            raise RuntimeError("source PNG contains a truncated chunk")
        payload_length = struct.unpack_from(">I", data, position)[0]
        payload_start = position + 8
        chunk_end = payload_start + payload_length + 4
        if chunk_end > len(data):
            raise RuntimeError("source PNG chunk exceeds the file bounds")

        kind = data[position + 4 : payload_start]
        payload = data[payload_start : payload_start + payload_length]
        expected_crc = struct.unpack_from(">I", data, payload_start + payload_length)[0]
        actual_crc = zlib.crc32(kind + payload) & 0xFFFF_FFFF
        if expected_crc != actual_crc:
            raise RuntimeError(f"source PNG {kind!r} chunk has an invalid CRC")

        chunks.append((kind, payload, position, chunk_end))
        position = chunk_end
        if kind == b"IEND":
            break

    if position != len(data):
        raise RuntimeError("source PNG has data after IEND")
    return chunks


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Serialize one PNG chunk with its CRC."""
    if len(kind) != 4:
        raise RuntimeError("PNG chunk type must contain exactly four bytes")
    return (
        struct.pack(">I", len(payload))
        + kind
        + payload
        + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFF_FFFF)
    )


def add_ztxt_without_separator(source: bytes) -> bytes:
    """Insert compressed text whose payload has no NUL keyword separator."""
    chunks = read_chunks(source)
    if [kind for kind, _, _, _ in chunks] != [b"IHDR", b"IDAT", b"IEND"]:
        raise RuntimeError("source PNG differs from the pinned one-pixel layout")
    header = chunks[0][1]
    if header != bytes.fromhex("00000001000000010802000000"):
        raise RuntimeError("source PNG is not the pinned 1x1 RGB8 image")
    if zlib.decompress(TEXT_ZLIB_STREAM) != TEXT_VALUE:
        raise RuntimeError("pinned zTXt data is not a valid text zlib stream")

    payload = b"keyword" + TEXT_ZLIB_STREAM
    if b"\0" in payload:
        raise RuntimeError("pinned zTXt payload contains a misleading NUL separator")
    idat_offset = chunks[1][2]
    return source[:idat_offset] + png_chunk(b"zTXt", payload) + source[idat_offset:]


def verify_pillow(data: bytes) -> None:
    if PILLOW_VERSION != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PILLOW_VERSION}")

    with Image.open(BytesIO(data)) as image:
        image.verify()
    with Image.open(BytesIO(data)) as image:
        image.load()
        if image.format != "PNG" or image.mode != "RGB" or image.size != (1, 1):
            raise RuntimeError("Pillow decoded unexpected PNG image metadata")
        if sha256(image.tobytes()) != EXPECTED_PIXELS_SHA256:
            raise RuntimeError("Pillow decoded pixels differ from the 1x1 baseline")


def main() -> None:
    source = SOURCE.read_bytes()
    if sha256(source) != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("1x1 PNG source differs from its pinned hash")

    fixture = add_ztxt_without_separator(source)
    if sha256(fixture) != EXPECTED_FIXTURE_SHA256:
        raise RuntimeError("missing-separator zTXt fixture differs from its pinned hash")
    verify_pillow(fixture)

    if OUTPUT.exists():
        if OUTPUT.read_bytes() != fixture:
            raise RuntimeError(f"refusing to replace a different fixture at {OUTPUT}")
        print(f"Verified existing fixture: {OUTPUT}")
        return

    OUTPUT.write_bytes(fixture)
    print(f"Wrote verified fixture: {OUTPUT}")


if __name__ == "__main__":
    main()
