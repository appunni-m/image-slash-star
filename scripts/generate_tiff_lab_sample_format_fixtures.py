#!/usr/bin/env python3
"""Generate pinned CIELAB TIFF SampleFormat edge-case fixtures."""

from __future__ import annotations

import argparse
import hashlib
import struct
from io import BytesIO
from pathlib import Path

from PIL import Image, UnidentifiedImageError, __version__ as PILLOW_VERSION, features


ROOT = Path(__file__).resolve().parents[1]
INPUT_DIR = ROOT / "tests/fixtures/input/images/tiff"
SOURCE_PATH = INPUT_DIR / "lab.tiff"
EXPECTED_PILLOW = ("12.2.0", "4.7.1")
EXPECTED_SOURCE_SHA256 = (
    "305ca36742bba9d7d07303482ef68515bce77c4159d5811c4d2931506a024671"
)
EXPECTED_PIXEL_SHA256 = (
    "403fa5475429b8200e15e60e01ffdb4f2499732bf9538e2d98e78bc1a1d0e64e"
)
SIZE = (3, 2)
IFD_TAGS = (256, 257, 258, 259, 262, 273, 277, 278, 279, 284)
FIXTURES = (
    (
        "lab_zero_count_sample_format.tiff",
        (),
        None,
        "LAB",
        170,
        "9876b5725c0d2465f75e1c504f577ff7eb28d7b1073a3a0a07a63e251133da0e",
    ),
    (
        "lab_signed_sample_format.tiff",
        (2,),
        None,
        None,
        170,
        "56afa52419d724980ed1e51a033ae347a165ad8d63135d7181d46f2a771fc4a0",
    ),
    (
        "lab_three_unsigned_sample_formats.tiff",
        (1, 1, 1),
        None,
        "LAB",
        176,
        "6fcd36de4da573de27821679dd915b9b5837e580193deb06db7f5c8a5a7bbf51",
    ),
    (
        "lab_mixed_sample_formats.tiff",
        (1, 2, 1),
        None,
        None,
        176,
        "a4eceb6dffd2304a459b9a7cfa0ce31c808c8f6b9f7b859309ec73df914327b2",
    ),
    (
        "lab_extra_sample.tiff",
        None,
        2,
        None,
        170,
        "fb57bc962b2bb098ca85acff272cf845bc3393b6598886d70ecbb758d384d888",
    ),
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest for bytes."""
    return hashlib.sha256(data).hexdigest()


def source_pixels() -> bytes:
    """Load and validate the pinned Pillow CIELAB source pixels."""
    observed_pillow = (PILLOW_VERSION, features.version("libtiff"))
    if observed_pillow != EXPECTED_PILLOW:
        raise RuntimeError(
            f"Pillow/libtiff {EXPECTED_PILLOW!r} is required, found {observed_pillow!r}"
        )

    source = SOURCE_PATH.read_bytes()
    if sha256(source) != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("the source CIELAB TIFF differs from its pinned bytes")

    with Image.open(BytesIO(source)) as image:
        if (
            image.format != "TIFF"
            or image.mode != "LAB"
            or image.size != SIZE
            or image.tag_v2.get(262) != 8
        ):
            raise RuntimeError("the source fixture must be a 3×2 CIELAB TIFF")
        image.load()
        pixels = image.tobytes()

    if sha256(pixels) != EXPECTED_PIXEL_SHA256:
        raise RuntimeError("the source fixture differs from its pinned CIELAB samples")
    return pixels


def insert_sample_tags(
    source: bytes,
    sample_formats: tuple[int, ...] | None,
    extra_sample: int | None,
) -> bytes:
    """Insert TIFF SampleFormat/ExtraSamples tags and relocate source values."""
    if source[:2] != b"II" or struct.unpack_from("<H", source, 2)[0] != 42:
        raise RuntimeError(
            "the pinned CIELAB source must be little-endian classic TIFF"
        )

    ifd_offset = struct.unpack_from("<I", source, 4)[0]
    old_count = struct.unpack_from("<H", source, ifd_offset)[0]
    if old_count != len(IFD_TAGS):
        raise RuntimeError("the source IFD has an unexpected number of entries")

    old_ifd_end = ifd_offset + 2 + old_count * 12 + 4
    next_ifd_start = ifd_offset + 2 + old_count * 12
    added_tags = int(sample_formats is not None) + int(extra_sample is not None)
    if added_tags == 0:
        raise RuntimeError("at least one new TIFF tag is required")
    relocation = added_tags * 12
    entries: list[tuple[int, bytes]] = []
    for index in range(old_count):
        entry_offset = ifd_offset + 2 + index * 12
        entry = bytearray(source[entry_offset : entry_offset + 12])
        tag, field_type, field_count = struct.unpack_from("<HHI", entry)
        if tag != IFD_TAGS[index] or tag == 339:
            raise RuntimeError("the source IFD tags differ from the pinned layout")

        if tag == 258:
            value_offset = struct.unpack_from("<I", entry, 8)[0]
            if (field_type, field_count, value_offset) != (3, 3, old_ifd_end):
                raise RuntimeError(
                    "the source BitsPerSample field differs from the pinned layout"
                )
            struct.pack_into("<I", entry, 8, value_offset + relocation)
        elif tag == 273:
            strip_offset = struct.unpack_from("<I", entry, 8)[0]
            if (field_type, field_count, strip_offset) != (4, 1, old_ifd_end + 6):
                raise RuntimeError(
                    "the source StripOffsets field differs from the pinned layout"
                )
            struct.pack_into("<I", entry, 8, strip_offset + relocation)

        entries.append((tag, bytes(entry)))

    if source[next_ifd_start:old_ifd_end] != b"\0\0\0\0":
        raise RuntimeError("the source unexpectedly contains a subsequent IFD")

    appended_values = b""
    if sample_formats is not None:
        sample_format = bytearray(12)
        count = len(sample_formats)
        struct.pack_into("<HHI", sample_format, 0, 339, 3, count)
        if count <= 2:
            for index, value in enumerate(sample_formats):
                if not 0 <= value <= 0xFFFF:
                    raise RuntimeError("SampleFormat values must fit in SHORT fields")
                struct.pack_into("<H", sample_format, 8 + index * 2, value)
        elif count == 3:
            if any(not 0 <= value <= 0xFFFF for value in sample_formats):
                raise RuntimeError("SampleFormat values must fit in SHORT fields")
            struct.pack_into("<I", sample_format, 8, len(source) + relocation)
            appended_values = b"".join(
                struct.pack("<H", value) for value in sample_formats
            )
        else:
            raise RuntimeError("only zero, one, two, or three values are supported")
        entries.append((339, bytes(sample_format)))

    if extra_sample is not None:
        if not 0 <= extra_sample <= 0xFFFF:
            raise RuntimeError("ExtraSamples values must fit in SHORT fields")
        extra = bytearray(12)
        struct.pack_into("<HHI", extra, 0, 338, 3, 1)
        struct.pack_into("<H", extra, 8, extra_sample)
        entries.append((338, bytes(extra)))

    rewritten = bytearray(source[:ifd_offset])
    rewritten.extend(struct.pack("<H", old_count + added_tags))
    for _, entry in sorted(entries):
        rewritten.extend(entry)
    rewritten.extend(source[next_ifd_start:old_ifd_end])
    rewritten.extend(source[old_ifd_end:])
    rewritten.extend(appended_values)
    return bytes(rewritten)


def verify_fixture(
    data: bytes,
    filename: str,
    sample_formats: tuple[int, ...] | None,
    extra_sample: int | None,
    expected_mode: str | None,
    pixels: bytes,
) -> None:
    """Verify the exact file bytes and pinned Pillow behavior."""
    expected = next(fixture for fixture in FIXTURES if fixture[0] == filename)
    if len(data) != expected[4] or sha256(data) != expected[5]:
        raise RuntimeError(f"{filename} differs from its pinned complete-file bytes")

    if expected_mode is not None:
        with Image.open(BytesIO(data)) as image:
            if (
                image.format != "TIFF"
                or image.mode != expected_mode
                or image.size != SIZE
                or image.tag_v2.get(262) != 8
                or image.tag_v2.get(338) != extra_sample
            ):
                raise RuntimeError("SampleFormat changed the Pillow CIELAB identity")
            observed_formats = image.tag_v2.get(339)
            if sample_formats and observed_formats != sample_formats:
                raise RuntimeError("Pillow exposed unexpected SampleFormat values")
            if sample_formats == () and observed_formats is not None:
                raise RuntimeError("Pillow exposed values for zero-count SampleFormat")
            image.load()
            if image.tobytes() != pixels:
                raise RuntimeError("SampleFormat changed the CIELAB samples")
        return

    try:
        Image.open(BytesIO(data))
    except UnidentifiedImageError as error:
        expected_prefix = "cannot identify image file <_io.BytesIO object at 0x"
        if not str(error).startswith(expected_prefix):
            raise RuntimeError(
                "SampleFormat edge case produced an unexpected Pillow error"
            ) from error
        return
    raise RuntimeError(
        "Pillow unexpectedly accepted a malformed SampleFormat CIELAB TIFF"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    source = SOURCE_PATH.read_bytes()
    pixels = source_pixels()

    for filename, sample_formats, extra_sample, mode, _, _ in FIXTURES:
        fixture = insert_sample_tags(source, sample_formats, extra_sample)
        verify_fixture(
            fixture, filename, sample_formats, extra_sample, mode, pixels
        )
        output = INPUT_DIR / filename
        if args.check:
            if not output.is_file() or output.read_bytes() != fixture:
                raise RuntimeError(f"fixture is missing or stale: {output}")
            action = "Verified"
        else:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(fixture)
            action = "Wrote"
        print(f"{action} {output} ({len(fixture)} bytes, SHA-256 {sha256(fixture)})")


if __name__ == "__main__":
    main()
