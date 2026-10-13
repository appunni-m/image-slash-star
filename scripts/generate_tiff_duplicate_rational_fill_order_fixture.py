#!/usr/bin/env python3
"""Generate a TIFF with duplicate rational FillOrder tags."""

from __future__ import annotations

import argparse
import hashlib
import struct
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = (
    ROOT
    / "tests/fixtures/input/images/tiff/miniswhite_1bit_fillorder_rational1.tiff"
)
OUTPUT = ROOT / "tests/fixtures/input/images/tiff" / (
    "miniswhite_1bit_fillorder_duplicate_rational_last_wins.tiff"
)
EXPECTED_SOURCE_SHA256 = (
    "04c5d443ec6b9f8d0db910f650f66d68ecad91bb8ad1778efe7669ef993969f2"
)
EXPECTED_OUTPUT_SHA256 = (
    "aedff731bf569efab03f53c48a900054a50b16622aadf4488dbdb457eff96b1d"
)
FILL_ORDER_TAG = 266
STRIP_OFFSETS_TAG = 273


def tiff_layout(data: bytes) -> tuple[str, int, list[bytes], bytes]:
    """Read a classic TIFF IFD and its unchanged trailing payload."""
    if len(data) < 8 or data[:2] not in (b"II", b"MM"):
        raise ValueError("source is not a complete classic TIFF header")
    endian = "<" if data[:2] == b"II" else ">"
    if struct.unpack_from(f"{endian}H", data, 2)[0] != 42:
        raise ValueError("source is not classic TIFF version 42")
    ifd_offset = struct.unpack_from(f"{endian}I", data, 4)[0]
    count = struct.unpack_from(f"{endian}H", data, ifd_offset)[0]
    entries_start = ifd_offset + 2
    next_offset_position = entries_start + count * 12
    payload_start = next_offset_position + 4
    if payload_start > len(data):
        raise ValueError("source IFD extends beyond the complete TIFF file")
    entries = [
        data[position : position + 12]
        for position in range(entries_start, next_offset_position, 12)
    ]
    return endian, ifd_offset, entries, data[payload_start:]


def entry_fields(entry: bytes, endian: str) -> tuple[int, int, int, int]:
    tag, field_type, count = struct.unpack_from(f"{endian}HHI", entry)
    value = struct.unpack_from(f"{endian}I", entry, 8)[0]
    return tag, field_type, count, value


def rational_value(data: bytes, entry: bytes, endian: str) -> tuple[int, int]:
    _, field_type, count, offset = entry_fields(entry, endian)
    if field_type != 5 or count != 1 or offset + 8 > len(data):
        raise ValueError("FillOrder entry must contain one in-bounds RATIONAL value")
    return struct.unpack_from(f"{endian}II", data, offset)


def set_entry_value(entry: bytes, value: int, endian: str) -> bytes:
    updated = bytearray(entry)
    struct.pack_into(f"{endian}I", updated, 8, value)
    return bytes(updated)


def bit_reverse(data: bytes) -> bytes:
    """Reverse packed samples in each byte for TIFF FillOrder 2."""
    return bytes(int(f"{value:08b}"[::-1], 2) for value in data)


def generate_fixture() -> bytes:
    """Append a last-wins FillOrder 2 tag and reverse the packed strip."""
    source = SOURCE.read_bytes()
    source_sha256 = hashlib.sha256(source).hexdigest()
    if source_sha256 != EXPECTED_SOURCE_SHA256:
        raise RuntimeError(
            f"source SHA-256 mismatch: expected {EXPECTED_SOURCE_SHA256}, "
            f"found {source_sha256}"
        )

    endian, ifd_offset, source_entries, trailing_data = tiff_layout(source)
    fill_entries = [
        entry
        for entry in source_entries
        if entry_fields(entry, endian)[0] == FILL_ORDER_TAG
    ]
    strip_entries = [
        entry
        for entry in source_entries
        if entry_fields(entry, endian)[0] == STRIP_OFFSETS_TAG
    ]
    strip_count_entries = [
        entry for entry in source_entries if entry_fields(entry, endian)[0] == 279
    ]
    if (
        len(fill_entries) != 1
        or len(strip_entries) != 1
        or len(strip_count_entries) != 1
    ):
        raise ValueError("source must have one FillOrder tag and one strip")
    first_fill_entry = fill_entries[0]
    if rational_value(source, first_fill_entry, endian) != (1, 1):
        raise ValueError("source FillOrder rational must be 1/1")

    strip_offset = entry_fields(strip_entries[0], endian)[3]
    strip_byte_count = entry_fields(strip_count_entries[0], endian)[3]
    if strip_offset + strip_byte_count > len(source):
        raise ValueError("source strip extends beyond the TIFF file")

    shift = 12
    updated_entries = []
    duplicate_entry_index = None
    for entry in source_entries:
        tag, field_type, count, value = entry_fields(entry, endian)
        updated = entry
        if tag == FILL_ORDER_TAG:
            if (field_type, count) != (5, 1):
                raise ValueError("source FillOrder tag must be a single RATIONAL")
            updated = set_entry_value(entry, value + shift, endian)
        elif tag == STRIP_OFFSETS_TAG:
            if (field_type, count) != (4, 1):
                raise ValueError("source strip offset must be one inline LONG")
            updated = set_entry_value(entry, value + shift, endian)
        updated_entries.append(updated)
        if tag == FILL_ORDER_TAG:
            duplicate_entry_index = len(updated_entries)
            updated_entries.append(entry)

    if duplicate_entry_index is None:
        raise ValueError("source FillOrder entry disappeared during generation")

    ifd_count = len(source_entries)
    old_next_offset_position = ifd_offset + 2 + ifd_count * 12
    next_ifd_offset = source[old_next_offset_position : old_next_offset_position + 4]
    prefix = source[:ifd_offset]

    def serialize(entries: list[bytes]) -> bytearray:
        return bytearray(
            prefix
            + struct.pack(f"{endian}H", ifd_count + 1)
            + b"".join(entries)
            + next_ifd_offset
            + trailing_data
        )

    output = serialize(updated_entries)
    rational_offset = (len(output) + 3) & ~3
    duplicate = bytearray(updated_entries[duplicate_entry_index])
    struct.pack_into(f"{endian}I", duplicate, 8, rational_offset)
    updated_entries[duplicate_entry_index] = bytes(duplicate)
    output = serialize(updated_entries)
    output.extend(bytes(rational_offset - len(output)))
    output.extend(struct.pack(f"{endian}II", 2, 1))

    shifted_strip_offset = strip_offset + shift
    shifted_strip_end = shifted_strip_offset + strip_byte_count
    output[shifted_strip_offset:shifted_strip_end] = bit_reverse(
        bytes(output[shifted_strip_offset:shifted_strip_end])
    )

    encoded = bytes(output)
    verify_structure(encoded, source)
    output_sha256 = hashlib.sha256(encoded).hexdigest()
    if output_sha256 != EXPECTED_OUTPUT_SHA256:
        raise RuntimeError(
            f"generated TIFF SHA-256 mismatch: expected {EXPECTED_OUTPUT_SHA256}, "
            f"found {output_sha256}"
        )
    return encoded


def verify_structure(encoded: bytes, source: bytes) -> None:
    endian, _, entries, _ = tiff_layout(encoded)
    duplicate_values = [
        rational_value(encoded, entry, endian)
        for entry in entries
        if entry_fields(entry, endian)[0] == FILL_ORDER_TAG
    ]
    if duplicate_values != [(1, 1), (2, 1)]:
        raise RuntimeError(
            "duplicate FillOrder values are "
            f"{duplicate_values}, expected [(1, 1), (2, 1)]"
        )

    source_endian, _, source_entries, _ = tiff_layout(source)
    source_strip_offset = next(
        entry_fields(entry, source_endian)[3]
        for entry in source_entries
        if entry_fields(entry, source_endian)[0] == STRIP_OFFSETS_TAG
    )
    source_strip_byte_count = next(
        entry_fields(entry, source_endian)[3]
        for entry in source_entries
        if entry_fields(entry, source_endian)[0] == 279
    )
    strip_offset = next(
        entry_fields(entry, endian)[3]
        for entry in entries
        if entry_fields(entry, endian)[0] == STRIP_OFFSETS_TAG
    )
    strip = encoded[strip_offset : strip_offset + source_strip_byte_count]
    expected_strip = bit_reverse(
        source[source_strip_offset : source_strip_offset + source_strip_byte_count]
    )
    if strip != expected_strip:
        raise RuntimeError(
            "FillOrder 2 strip bytes do not reverse the original packed samples"
        )


def verify_pillow_reference(encoded: bytes) -> None:
    """Confirm Pillow uses the later duplicate tag and preserves source pixels."""
    from PIL import Image, __version__ as pillow_version

    if pillow_version != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {pillow_version}")
    with Image.open(BytesIO(SOURCE.read_bytes())) as source_image:
        source_image.load()
        expected = (
            source_image.format,
            source_image.mode,
            source_image.size,
            source_image.tobytes(),
        )
    with Image.open(BytesIO(encoded)) as image:
        image.load()
        fill_order = image.tag_v2.get(FILL_ORDER_TAG)
        if float(fill_order) != 2.0:
            raise RuntimeError(
                f"Pillow did not select the later FillOrder value: {fill_order!r}"
            )
        actual = (image.format, image.mode, image.size, image.tobytes())
    if actual != expected:
        raise RuntimeError(
            "Pillow duplicate-FillOrder pixels differ from the source image"
        )


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
            raise RuntimeError(
                f"{OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {OUTPUT.relative_to(ROOT)}")
        return 0

    verify_pillow_reference(encoded)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(encoded)
    print(
        f"Wrote {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes, "
        f"SHA-256 {hashlib.sha256(encoded).hexdigest()})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
