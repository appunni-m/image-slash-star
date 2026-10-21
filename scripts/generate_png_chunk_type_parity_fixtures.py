#!/usr/bin/env python3
"""Generate PNG fixtures that pin Pillow's chunk-type byte behavior."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from pathlib import Path

from generate_test_assets import png_chunks, rebuild_png


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/png/1x1.png"
OUTPUT_DIRECTORY = SOURCE.parent
EXPECTED_SOURCE_SHA256 = "611033f4cc0aa9e3348e69eba089499bc6287d57b53832079e64f42564a4e876"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
FIXTURES = (
    (
        "invalid_chunk_type_non_letter.png",
        ((b"a@Cd", b"non-letter type"),),
        "edac414911843c23dac2cba7f194ed34fc43addfd40877440e29b676dce55ae8",
    ),
    (
        "chunk_type_digit_underscore.png",
        ((b"a1cd", b"numeric type"), (b"a_cd", b"underscore type")),
        "92f4b333a2dc484ceae684e89dac4cac8d3c11c96c03e287b0e2a0e4a71785de",
    ),
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_chunk_crcs(data: bytes, chunks: list[tuple[bytes, bytes]]) -> None:
    """Check each serialized chunk's CRC and the end of the PNG stream."""
    position = len(PNG_SIGNATURE)
    for kind, payload in chunks:
        crc_start = position + 8 + len(payload)
        expected_crc = zlib.crc32(kind + payload) & 0xFFFF_FFFF
        actual_crc = struct.unpack_from(">I", data, crc_start)[0]
        if actual_crc != expected_crc:
            raise RuntimeError(f"generated {kind!r} chunk has an invalid CRC")
        position = crc_start + 4

    if position != len(data):
        raise RuntimeError("generated PNG contains bytes after IEND")


def make_fixture(
    source: bytes, inserted_chunks: tuple[tuple[bytes, bytes], ...], expected_sha256: str
) -> bytes:
    """Insert chunk controls before IDAT and recompute every chunk CRC."""
    if sha256(source) != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("base 1x1 PNG differs from its pinned source hash")

    chunks = png_chunks(source)
    if not chunks or chunks[0][0] != b"IHDR":
        raise RuntimeError("base PNG must begin with IHDR")
    if len(chunks) < 3 or chunks[1][0] != b"IDAT":
        raise RuntimeError("base PNG must contain one IDAT immediately after IHDR")

    fixture_chunks = list(chunks)
    fixture_chunks[1:1] = inserted_chunks
    fixture = rebuild_png(fixture_chunks)
    observed_chunks = png_chunks(fixture)
    if observed_chunks != fixture_chunks:
        raise RuntimeError("generated PNG differs from its expected chunk sequence")
    verify_chunk_crcs(fixture, observed_chunks)
    if sha256(fixture) != expected_sha256:
        raise RuntimeError("generated PNG differs from its pinned fixture hash")
    return fixture


def generate_fixtures(*, check: bool) -> None:
    source = SOURCE.read_bytes()
    for filename, inserted_chunks, expected_sha256 in FIXTURES:
        fixture = make_fixture(source, inserted_chunks, expected_sha256)
        output = OUTPUT_DIRECTORY / filename
        if check:
            if not output.is_file() or output.read_bytes() != fixture:
                raise RuntimeError(f"fixture is stale or missing: {output.relative_to(ROOT)}")
            print(f"Verified {output.relative_to(ROOT)}")
            continue
        if output.exists() and output.read_bytes() != fixture:
            raise RuntimeError(f"refusing to replace different fixture: {output.relative_to(ROOT)}")
        output.write_bytes(fixture)
        print(f"Wrote {output.relative_to(ROOT)}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    arguments = parser.parse_args()
    generate_fixtures(check=arguments.check)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
