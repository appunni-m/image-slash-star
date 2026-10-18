#!/usr/bin/env python3
"""Generate and verify an APNG with a mismatched pre-IDAT fdAT sequence."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/png/apng_empty_fdat_before_idat.png"
OUTPUT = ROOT / "tests/fixtures/input/images/png/apng_fdat_sequence_mismatch_before_idat.png"
EXPECTED_SOURCE_SHA256 = "108354b38dbd4325334a4a236910930d14a99fe1d6bc7110c8bbad2e80d01ab7"
EXPECTED_FIXTURE_SHA256 = "f25e81a9a1bf35ae695e8ef4145f6218dd42918382e7d58d21632cbab2d17e9b"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
EXPECTED_CHUNK_KINDS = (
    b"IHDR",
    b"acTL",
    b"fcTL",
    b"fdAT",
    b"IDAT",
    b"fcTL",
    b"fdAT",
    b"IEND",
)
EXPECTED_SOURCE_SEQUENCE = (0, 1, 2, 3)
EXPECTED_FIXTURE_SEQUENCE = (0, 2, 2, 3)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_chunks(data: bytes) -> list[tuple[bytes, bytes, int, int]]:
    """Return complete CRC-valid PNG chunks and their byte bounds."""
    if not data.startswith(PNG_SIGNATURE):
        raise RuntimeError("source PNG has an invalid signature")

    chunks = []
    position = len(PNG_SIGNATURE)
    while position < len(data):
        if len(data) - position < 12:
            raise RuntimeError("source PNG contains a truncated chunk")
        payload_length = struct.unpack_from(">I", data, position)[0]
        payload_start = position + 8
        payload_end = payload_start + payload_length
        chunk_end = payload_end + 4
        if chunk_end > len(data):
            raise RuntimeError("source PNG chunk exceeds the file bounds")

        kind = data[position + 4 : payload_start]
        payload = data[payload_start:payload_end]
        expected_crc = struct.unpack_from(">I", data, payload_end)[0]
        actual_crc = zlib.crc32(kind + payload) & 0xFFFF_FFFF
        if expected_crc != actual_crc:
            raise RuntimeError(f"source PNG {kind!r} chunk has an invalid CRC")

        chunks.append((kind, payload, position, chunk_end))
        position = chunk_end
        if kind == b"IEND":
            break

    if position != len(data) or not chunks or chunks[-1][0] != b"IEND":
        raise RuntimeError("source PNG is incomplete or contains trailing bytes")
    return chunks


def sequence_numbers(chunks: list[tuple[bytes, bytes, int, int]]) -> tuple[int, ...]:
    """Return APNG sequence numbers in their on-file order."""
    numbers = []
    for kind, payload, _, _ in chunks:
        if kind not in (b"fcTL", b"fdAT"):
            continue
        if len(payload) < 4:
            raise RuntimeError(f"source APNG {kind!r} chunk has no sequence number")
        numbers.append(struct.unpack_from(">I", payload)[0])
    return tuple(numbers)


def make_fixture(source: bytes) -> bytes:
    """Change the empty pre-IDAT fdAT sequence from one to two."""
    if sha256(source) != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("empty pre-IDAT fdAT source differs from its pinned hash")

    chunks = read_chunks(source)
    if tuple(kind for kind, _, _, _ in chunks) != EXPECTED_CHUNK_KINDS:
        raise RuntimeError("source APNG differs from the pinned empty-fdAT chunk layout")
    expected_lengths = {
        b"IHDR": 13,
        b"acTL": 8,
        b"fcTL": 26,
        b"IEND": 0,
    }
    for kind, payload, _, _ in chunks:
        expected_length = expected_lengths.get(kind)
        if expected_length is not None and len(payload) != expected_length:
            raise RuntimeError(f"source APNG {kind!r} chunk has an unexpected length")
        if kind == b"fdAT" and len(payload) < 4:
            raise RuntimeError("source APNG fdAT chunk has no sequence number")
    if sequence_numbers(chunks) != EXPECTED_SOURCE_SEQUENCE:
        raise RuntimeError("source APNG sequence numbers differ from the pinned fixture")

    idat_index = next(
        index for index, (kind, _, _, _) in enumerate(chunks) if kind == b"IDAT"
    )
    pre_idat_fdat = [
        (index, payload, offset)
        for index, (kind, payload, offset, _) in enumerate(chunks)
        if kind == b"fdAT" and index < idat_index
    ]
    if len(pre_idat_fdat) != 1:
        raise RuntimeError("source APNG must have exactly one pre-IDAT fdAT chunk")

    fdat_index, fdat_payload, fdat_offset = pre_idat_fdat[0]
    if fdat_index != idat_index - 1 or len(fdat_payload) != 4:
        raise RuntimeError("pre-IDAT fdAT must be empty and immediately precede IDAT")
    if struct.unpack(">I", fdat_payload)[0] != 1:
        raise RuntimeError("pre-IDAT fdAT sequence must be 1 before mutation")

    payload_start = fdat_offset + 8
    payload_end = payload_start + len(fdat_payload)
    changed_payload = struct.pack(">I", 2)
    fixture = bytearray(source)
    fixture[payload_start:payload_end] = changed_payload
    fixture[payload_end : payload_end + 4] = struct.pack(
        ">I", zlib.crc32(b"fdAT" + changed_payload) & 0xFFFF_FFFF
    )
    result = bytes(fixture)

    result_chunks = read_chunks(result)
    if tuple(kind for kind, _, _, _ in result_chunks) != EXPECTED_CHUNK_KINDS:
        raise RuntimeError("generated APNG chunk layout changed unexpectedly")
    if sequence_numbers(result_chunks) != EXPECTED_FIXTURE_SEQUENCE:
        raise RuntimeError("generated APNG sequence numbers differ from the expected mismatch")
    if result[:payload_start] != source[:payload_start] or result[payload_end + 4 :] != source[
        payload_end + 4 :
    ]:
        raise RuntimeError("generated APNG changed bytes outside the fdAT sequence and CRC")
    return result


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
        raise RuntimeError("pre-IDAT fdAT sequence-mismatch fixture differs from its pinned hash")

    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_bytes() != fixture:
            raise RuntimeError(f"pre-IDAT fdAT sequence-mismatch fixture is stale at {OUTPUT}")
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
