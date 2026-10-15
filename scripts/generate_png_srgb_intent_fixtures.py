#!/usr/bin/env python3
"""Generate and verify PNG fixtures for valid sRGB rendering intents."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/png/srgb.png"
OUTPUT_DIRECTORY = SOURCE.parent
EXPECTED_SOURCE_SHA256 = "838049a2022bb51c4e53c6955c7e859b67e421d00bb0775dc216d1478baaaf03"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
FIXTURES = (
    (
        "srgb_relative_colorimetric.png",
        1,
        "fa535021c526b667fd18e260cc59ccde76c8ed81490b76a18e8d9e3c5a169255",
    ),
    (
        "srgb_saturation.png",
        2,
        "23091532c4a987f861682cae621707cca8194ea6808730818c723ce2c1cb4ca5",
    ),
    (
        "srgb_absolute_colorimetric.png",
        3,
        "dafb11f02b1cb229c82dca025b8d1e9158d6714ce7cc55c0141468eff57e75d7",
    ),
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_chunks(data: bytes) -> list[tuple[bytes, bytes]]:
    """Read a complete PNG, validating every chunk boundary and CRC."""
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

        chunks.append((kind, payload))
        position = chunk_end
        if kind == b"IEND":
            break

    if position != len(data) or not chunks or chunks[-1][0] != b"IEND":
        raise RuntimeError("source PNG is incomplete or contains trailing bytes")
    return chunks


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Serialize one PNG chunk with a recomputed CRC."""
    return (
        struct.pack(">I", len(payload))
        + kind
        + payload
        + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFF_FFFF)
    )


def with_srgb_intent(source: bytes, intent: int) -> bytes:
    """Change only the single-byte sRGB intent in the pinned PNG."""
    chunks = read_chunks(source)
    srgb_indexes = [
        index for index, (kind, _) in enumerate(chunks) if kind == b"sRGB"
    ]
    if len(srgb_indexes) != 1:
        raise RuntimeError("source PNG must contain exactly one sRGB chunk")
    srgb_index = srgb_indexes[0]
    if chunks[srgb_index][1] != b"\0":
        raise RuntimeError("source PNG sRGB intent must be the pinned value 0")

    chunks[srgb_index] = (b"sRGB", bytes((intent,)))
    return PNG_SIGNATURE + b"".join(
        png_chunk(kind, payload) for kind, payload in chunks
    )


def generate_fixtures(*, check: bool) -> None:
    source = SOURCE.read_bytes()
    if sha256(source) != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("base sRGB PNG differs from the pinned source hash")

    source_chunks = read_chunks(source)
    for filename, intent, expected_sha256 in FIXTURES:
        fixture = with_srgb_intent(source, intent)
        if sha256(fixture) != expected_sha256:
            raise RuntimeError(f"generated {filename} differs from its pinned hash")

        fixture_chunks = read_chunks(fixture)
        expected_chunks = list(source_chunks)
        srgb_index = next(
            index
            for index, (kind, _) in enumerate(expected_chunks)
            if kind == b"sRGB"
        )
        expected_chunks[srgb_index] = (b"sRGB", bytes((intent,)))
        if fixture_chunks != expected_chunks:
            raise RuntimeError(f"generated {filename} changed unrelated PNG chunks")

        output = OUTPUT_DIRECTORY / filename
        if check:
            if not output.is_file() or output.read_bytes() != fixture:
                raise RuntimeError(f"generated fixture is stale or missing: {output}")
            print(f"Verified {output.relative_to(ROOT)}")
        elif output.exists():
            if output.read_bytes() != fixture:
                raise RuntimeError(f"refusing to replace a different fixture at {output}")
            print(f"Verified existing fixture: {output.relative_to(ROOT)}")
        else:
            output.write_bytes(fixture)
            print(f"Wrote {output.relative_to(ROOT)}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify generated files without writing them",
    )
    arguments = parser.parse_args()
    generate_fixtures(check=arguments.check)


if __name__ == "__main__":
    main()
