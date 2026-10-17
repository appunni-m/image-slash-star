#!/usr/bin/env python3
"""Generate and verify the pre-IDAT duplicate-acTL PNG parity fixture."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from io import BytesIO
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/png/apng_rgba_controls.png"
OUTPUT = ROOT / "tests/fixtures/input/images/png/apng_duplicate_actl_before_idat.png"
EXPECTED_SOURCE_SHA256 = "90692df361ecb38985c898c08af1a65337aca3dccc77e92ca0b7be55d409bbd8"
EXPECTED_FIXTURE_SHA256 = "18b489f5cea0c7e3e58550afcc2ce169a12d377d5f7e357b1de2714ab5c67022"
EXPECTED_PIXELS_SHA256 = "21b7dac1be9dacf604fd4475304d962dc50a83b6b9623340f59de886d398b8c6"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_chunks(data: bytes) -> list[tuple[bytes, bytes]]:
    """Return all chunks after validating bounds, CRCs, and the IEND boundary."""
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
    """Serialize one PNG chunk with its checked length and CRC."""
    if len(kind) != 4 or len(payload) > 0xFFFF_FFFF:
        raise RuntimeError("PNG chunk kind or payload exceeds its format limit")
    return (
        struct.pack(">I", len(payload))
        + kind
        + payload
        + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFF_FFFF)
    )


def make_fixture(source: bytes) -> bytes:
    """Duplicate the pinned APNG control chunk before its first frame data."""
    if sha256(source) != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("APNG controls source differs from its pinned hash")

    chunks = read_chunks(source)
    actl_indexes = [index for index, (kind, _) in enumerate(chunks) if kind == b"acTL"]
    idat_indexes = [index for index, (kind, _) in enumerate(chunks) if kind == b"IDAT"]
    if len(actl_indexes) != 1 or not idat_indexes:
        raise RuntimeError("source must have one acTL and at least one IDAT chunk")
    actl_index = actl_indexes[0]
    if actl_index >= idat_indexes[0] or len(chunks[actl_index][1]) != 8:
        raise RuntimeError("source acTL must be an eight-byte control before IDAT")

    duplicated = list(chunks)
    duplicated.insert(actl_index + 1, chunks[actl_index])
    fixture = PNG_SIGNATURE + b"".join(
        png_chunk(kind, payload) for kind, payload in duplicated
    )
    fixture_chunks = read_chunks(fixture)
    fixture_actl_indexes = [
        index for index, (kind, _) in enumerate(fixture_chunks) if kind == b"acTL"
    ]
    fixture_idat_index = next(
        index for index, (kind, _) in enumerate(fixture_chunks) if kind == b"IDAT"
    )
    if (
        len(fixture_actl_indexes) != 2
        or fixture_actl_indexes[1] != fixture_actl_indexes[0] + 1
        or fixture_actl_indexes[1] >= fixture_idat_index
    ):
        raise RuntimeError("fixture must contain adjacent duplicate acTL chunks before IDAT")
    if fixture_chunks[fixture_actl_indexes[0]][1] != fixture_chunks[
        fixture_actl_indexes[1]
    ][1]:
        raise RuntimeError("duplicate acTL payload differs from the source control")
    if sha256(fixture) != EXPECTED_FIXTURE_SHA256:
        raise RuntimeError("generated APNG fixture differs from its pinned hash")
    return fixture


def verify_pillow(data: bytes, source: bytes) -> None:
    """Verify Pillow's default-image fallback and exact first-frame pixels."""
    try:
        import PIL
        from PIL import Image
    except ImportError as error:
        raise RuntimeError("Pillow 12.2.0 is required for --verify-pillow") from error

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PIL.__version__}")

    with Image.open(BytesIO(source)) as original:
        original.seek(0)
        original.load()
        expected_pixels = original.tobytes()

    with Image.open(BytesIO(data)) as fallback:
        fallback.load()
        if (
            fallback.format != "PNG"
            or fallback.mode != "RGBA"
            or fallback.size != (4, 4)
            or fallback.n_frames != 1
            or sha256(fallback.tobytes()) != EXPECTED_PIXELS_SHA256
            or fallback.tobytes() != expected_pixels
        ):
            raise RuntimeError("Pillow did not preserve the static default image fallback")


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
        help="verify the pinned Pillow 12.2.0 default-image fallback",
    )
    arguments = parser.parse_args()

    source = SOURCE.read_bytes()
    fixture = make_fixture(source)
    if arguments.verify_pillow:
        verify_pillow(fixture, source)

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
