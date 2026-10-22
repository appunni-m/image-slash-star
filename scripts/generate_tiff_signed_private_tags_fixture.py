#!/usr/bin/env python3
"""Add ignored private signed-value tags to a pinned grayscale TIFF."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path
import struct


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tests/fixtures/input/images/tiff/gray.tiff"
OUTPUT = ROOT / "tests/fixtures/input/images/tiff/gray_signed_private_tags.tiff"
EXPECTED_SOURCE_SHA256 = "e4295c2265ed3731f8b6b68e24595f44a4b7e3d088a3bb99e0a332b938444921"
EXPECTED_OUTPUT_SHA256 = "5e7b1dfc00abf38207e020dc3b39d6fe37fb32e77c7b5382227590057bc63c38"
EXPECTED_PIXELS_SHA256 = "1928fc10cb7c056f9c5de7ee534445e876fac9fd1ea050bc3c8b6eb29371c707"
PRIVATE_TAGS = ((65000, 6, 0xFF), (65001, 8, 0xFFFF), (65002, 9, 0xFFFFFFFF))
SOURCE_TAGS = (256, 257, 258, 259, 262, 273, 278, 279, 284)
ENTRY_SIZE = 12


def digest(data: bytes) -> str:
    return sha256(data).hexdigest()


def generate_fixture() -> bytes:
    source = SOURCE.read_bytes()
    source_sha256 = digest(source)
    if source_sha256 != EXPECTED_SOURCE_SHA256:
        raise RuntimeError(
            f"source TIFF SHA-256 mismatch: expected {EXPECTED_SOURCE_SHA256}, "
            f"found {source_sha256}"
        )
    if not source.startswith(b"II*\x00"):
        raise RuntimeError("source TIFF must have a little-endian classic header")

    ifd_offset = struct.unpack_from("<I", source, 4)[0]
    if ifd_offset != 8:
        raise RuntimeError(f"source TIFF IFD moved from byte 8 to {ifd_offset}")
    entry_count = struct.unpack_from("<H", source, ifd_offset)[0]
    entries_start = ifd_offset + 2
    entries_end = entries_start + ENTRY_SIZE * entry_count
    entries: list[bytes] = []
    tags: list[int] = []
    strip_offset: int | None = None
    for index in range(entry_count):
        entry_offset = entries_start + ENTRY_SIZE * index
        tag, field_type, value_count, value = struct.unpack_from(
            "<HHII", source, entry_offset
        )
        tags.append(tag)
        if tag == 273:
            if field_type != 4 or value_count != 1 or strip_offset is not None:
                raise RuntimeError("source TIFF must contain one inline LONG StripOffsets value")
            strip_offset = value + ENTRY_SIZE * len(PRIVATE_TAGS)
            value = strip_offset
        entries.append(struct.pack("<HHII", tag, field_type, value_count, value))

    if tuple(tags) != SOURCE_TAGS:
        raise RuntimeError(f"source TIFF directory tags changed: {tuple(tags)!r}")
    if strip_offset is None:
        raise RuntimeError("source TIFF has no StripOffsets entry")
    if strip_offset != entries_end + 4 + ENTRY_SIZE * len(PRIVATE_TAGS):
        raise RuntimeError("source TIFF strip is not the payload following its first IFD")
    if struct.unpack_from("<I", source, entries_end)[0] != 0:
        raise RuntimeError("source TIFF must contain one directory")

    private_entries = b"".join(
        struct.pack("<HHII", tag, field_type, 1, inline_value)
        for tag, field_type, inline_value in PRIVATE_TAGS
    )
    encoded = (
        source[:ifd_offset]
        + struct.pack("<H", entry_count + len(PRIVATE_TAGS))
        + b"".join(entries)
        + private_entries
        + b"\x00\x00\x00\x00"
        + source[entries_end + 4 :]
    )
    output_sha256 = digest(encoded)
    if output_sha256 != EXPECTED_OUTPUT_SHA256:
        raise RuntimeError(
            f"generated TIFF SHA-256 mismatch: expected {EXPECTED_OUTPUT_SHA256}, "
            f"found {output_sha256}"
        )
    return encoded


def verify_pillow_reference(encoded: bytes) -> None:
    """Confirm Pillow 12.2.0 reads all signed tags and keeps the source pixels."""
    import PIL
    from PIL import Image

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PIL.__version__}")

    with Image.open(SOURCE) as source_image:
        source_image.load()
        source_pixels = source_image.tobytes()
    if digest(source_pixels) != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("source grayscale pixels differ from their pinned hash")

    with Image.open(BytesIO(encoded)) as image:
        image.verify()
    with Image.open(BytesIO(encoded)) as image:
        image.load()
        if image.format != "TIFF" or image.mode != "L" or image.size != (128, 128):
            raise RuntimeError(
                "Pillow returned an unexpected TIFF result: "
                f"format={image.format!r}, mode={image.mode!r}, size={image.size!r}"
            )
        if any(image.tag_v2.get(tag) != -1 for tag, _, _ in PRIVATE_TAGS):
            raise RuntimeError("Pillow did not interpret all private signed-value tags as -1")
        pixels = image.tobytes()

    if pixels != source_pixels or digest(pixels) != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("private signed-value tags changed Pillow's grayscale pixels")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed fixture without rewriting it",
    )
    args = parser.parse_args()

    encoded = generate_fixture()
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != encoded:
            raise RuntimeError(f"{OUTPUT.relative_to(ROOT)} differs from generated bytes")
        print(f"Fixture matches: {OUTPUT.relative_to(ROOT)}")
        return 0

    verify_pillow_reference(encoded)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(encoded)
    print(f"Wrote {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
