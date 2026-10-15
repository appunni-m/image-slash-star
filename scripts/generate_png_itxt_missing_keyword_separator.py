#!/usr/bin/env python3
"""Generate and verify a PNG iTXt chunk without a keyword separator."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/png/1x1.png"
OUTPUT = ROOT / "tests/fixtures/input/images/png/itxt_missing_keyword_separator.png"
EXPECTED_SOURCE_SHA256 = "611033f4cc0aa9e3348e69eba089499bc6287d57b53832079e64f42564a4e876"
EXPECTED_FIXTURE_SHA256 = "ef5663b10dd1bef8481a1f73bee3283038fc41a3bd928cd0ba7945654efb98cd"
ITXT_PAYLOAD = bytes.fromhex(
    "6b6579776f7264789cf30caf5674757656cb2eb52a04327c1c7472ab013e030602"
)
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_chunks(data: bytes) -> list[tuple[bytes, bytes, int, int]]:
    """Return complete chunks after validating their bounds and CRCs."""
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


def make_fixture(source: bytes) -> bytes:
    """Insert a zero-free iTXt payload before IDAT in the pinned source PNG."""
    if sha256(source) != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("1x1 PNG source differs from its pinned hash")
    chunks = read_chunks(source)
    if [kind for kind, _, _, _ in chunks] != [b"IHDR", b"IDAT", b"IEND"]:
        raise RuntimeError("source PNG differs from the pinned one-pixel layout")
    if chunks[0][1] != bytes.fromhex("00000001000000010802000000"):
        raise RuntimeError("source PNG is not the pinned 1x1 RGB8 image")
    if b"\0" in ITXT_PAYLOAD:
        raise RuntimeError("pinned iTXt payload contains a keyword separator")

    idat_offset = chunks[1][2]
    return source[:idat_offset] + png_chunk(b"iTXt", ITXT_PAYLOAD) + source[idat_offset:]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the generated fixture without writing it",
    )
    args = parser.parse_args()

    fixture = make_fixture(SOURCE.read_bytes())
    if sha256(fixture) != EXPECTED_FIXTURE_SHA256:
        raise RuntimeError("missing-separator iTXt fixture differs from its pinned hash")

    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_bytes() != fixture:
            raise RuntimeError(f"missing-separator iTXt fixture is stale at {OUTPUT}")
        print(f"Verified existing fixture: {OUTPUT}")
        return

    if OUTPUT.exists():
        if OUTPUT.read_bytes() != fixture:
            raise RuntimeError(f"refusing to replace a different fixture at {OUTPUT}")
        print(f"Verified existing fixture: {OUTPUT}")
        return

    OUTPUT.write_bytes(fixture)
    print(f"Wrote verified fixture: {OUTPUT}")


if __name__ == "__main__":
    main()
