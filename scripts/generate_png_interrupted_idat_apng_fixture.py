#!/usr/bin/env python3
"""Generate and verify an APNG whose default IDAT run is interrupted."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path
import struct
import zlib


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/png/apng_default_image.png"
OUTPUT = ROOT / "tests/fixtures/input/images/png/apng_default_image_interrupted_idat.png"
EXPECTED_SOURCE_SHA256 = "56ec5797461a6fbb62349d1d8846c61f34a5abb50627046cdf0b6f59fd331960"
EXPECTED_FIXTURE_SHA256 = "6bd28537198e97b9fd61f8b27b7dedc28f130935f951317043bc4fa46b3228cb"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
SEPARATOR_KIND = b"vpAg"
SEPARATOR_PAYLOAD = b"separator"
PINNED_CHUNKS = (
    b"IHDR",
    b"acTL",
    b"IDAT",
    b"fcTL",
    b"fdAT",
    b"fcTL",
    b"fdAT",
    b"IEND",
)


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
    checksum = zlib.crc32(kind + payload) & 0xFFFF_FFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def make_fixture(source: bytes) -> bytes:
    """Split the default image zlib stream around a valid ancillary chunk."""
    if sha256(source).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("APNG default-image source differs from its pinned hash")

    chunks = read_chunks(source)
    if tuple(kind for kind, _ in chunks) != PINNED_CHUNKS:
        raise RuntimeError("source APNG differs from the pinned default-image layout")
    compressed = chunks[2][1]
    if len(compressed) < 2:
        raise RuntimeError("default image IDAT is too short to split")

    fixture = PNG_SIGNATURE + b"".join(
        (
            png_chunk(chunks[0][0], chunks[0][1]),
            png_chunk(chunks[1][0], chunks[1][1]),
            png_chunk(b"IDAT", compressed[:1]),
            png_chunk(SEPARATOR_KIND, SEPARATOR_PAYLOAD),
            png_chunk(b"IDAT", compressed[1:]),
            *(png_chunk(kind, payload) for kind, payload in chunks[3:]),
        )
    )
    fixture_chunks = read_chunks(fixture)
    expected_layout = (
        b"IHDR",
        b"acTL",
        b"IDAT",
        SEPARATOR_KIND,
        b"IDAT",
        b"fcTL",
        b"fdAT",
        b"fcTL",
        b"fdAT",
        b"IEND",
    )
    if tuple(kind for kind, _ in fixture_chunks) != expected_layout:
        raise RuntimeError("fixture does not interrupt the default IDAT run")
    if fixture_chunks[2][1] + fixture_chunks[4][1] != compressed:
        raise RuntimeError("fixture changed the source APNG default-image zlib stream")
    if sha256(fixture).hexdigest() != EXPECTED_FIXTURE_SHA256:
        raise RuntimeError("generated interrupted-IDAT APNG differs from its pinned hash")
    return fixture


def verify_pillow(data: bytes) -> None:
    """Check structural verification and exact load failures with Pillow."""
    import PIL
    from PIL import Image

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PIL.__version__}")

    with Image.open(BytesIO(data)) as image:
        if image.format != "PNG" or image.mode != "RGBA" or image.size != (4, 4):
            raise RuntimeError("Pillow returned unexpected APNG default-image metadata")
        if image.n_frames != 3:
            raise RuntimeError(f"Pillow returned an unexpected APNG frame count: {image.n_frames}")
        image.verify()

    for frame in range(3):
        try:
            with Image.open(BytesIO(data)) as image:
                image.seek(frame)
                image.load()
        except OSError as error:
            if str(error) != "image file is truncated (0 bytes not processed)":
                raise RuntimeError(f"Pillow returned an unexpected frame error: {error}") from error
        else:
            raise RuntimeError(f"Pillow unexpectedly loaded interrupted-IDAT frame {frame}")


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
        help="verify the pinned Pillow 12.2.0 behavior",
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
