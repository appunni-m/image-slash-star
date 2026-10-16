#!/usr/bin/env python3
"""Generate Pillow error-parity TIFF fixtures from active FillOrder=1 inputs."""

from __future__ import annotations

import argparse
import hashlib
import struct
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tests/fixtures/input/images/tiff/tiled_palette4_unaligned_edge.tiff"
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/tiff/tiled_palette4_fillorder2_unaligned_error.tiff"
)
STRIP_SOURCE = ROOT / "tests/fixtures/input/images/tiff/palette4.tiff"
STRIP_OUTPUT = ROOT / "tests/fixtures/input/images/tiff/palette4_fillorder2_strip_error.tiff"
EXPECTED_SOURCE_SHA256 = (
    "3b51369c66d32d87e988fc7922b3a47dfcfcaff9cee035e843df7112c9226e9b"
)
EXPECTED_OUTPUT_SHA256 = (
    "54e20e9873b64062800c6ba886b8583b14376fae74c40c133b317b02118c5094"
)
EXPECTED_STRIP_SOURCE_SHA256 = (
    "2e2ea5a76c3235dbe41825a8791805292dd94c660661a8e97e66e3fd8fe7590d"
)
EXPECTED_STRIP_OUTPUT_SHA256 = (
    "10698d4758ae4ce8b18614377e817da971115286f4a766daf49efe289164cdf8"
)
FILL_ORDER_TAG = 266
TILE_OFFSETS_TAG = 324
TILE_BYTE_COUNTS_TAG = 325
STRIP_OFFSETS_TAG = 273
STRIP_BYTE_COUNTS_TAG = 279
TYPE_SIZES = {1: 1, 2: 1, 3: 2, 4: 4, 5: 8, 6: 1, 7: 1, 8: 2, 9: 4, 10: 8, 11: 4, 12: 8}


def parse_layout(data: bytes) -> tuple[int, int, int, dict[int, tuple[int, int, int]]]:
    """Return the first classic TIFF IFD and its tag entry positions."""
    if len(data) < 8 or data[:4] != b"II*\x00":
        raise ValueError("fixture source must be little-endian classic TIFF")
    ifd_offset = struct.unpack_from("<I", data, 4)[0]
    if ifd_offset + 2 > len(data):
        raise ValueError("fixture IFD count is truncated")
    entry_count = struct.unpack_from("<H", data, ifd_offset)[0]
    ifd_end = ifd_offset + 2 + entry_count * 12 + 4
    if ifd_end > len(data):
        raise ValueError("fixture IFD is truncated")

    entries = {}
    tags = []
    for index in range(entry_count):
        position = ifd_offset + 2 + index * 12
        tag, field_type, count = struct.unpack_from("<HHI", data, position)
        if tag in entries:
            raise ValueError(f"fixture repeats TIFF tag {tag}")
        if field_type not in TYPE_SIZES:
            raise ValueError(f"fixture uses unsupported TIFF field type {field_type}")
        entries[tag] = (position, field_type, count)
        tags.append(tag)
    if tags != sorted(tags):
        raise ValueError("fixture IFD tags are not sorted")
    if struct.unpack_from("<I", data, ifd_end - 4)[0] != 0:
        raise ValueError("fixture must contain one TIFF directory")
    return ifd_offset, entry_count, ifd_end, entries


def entry_values(
    data: bytes | bytearray, entry: tuple[int, int, int]
) -> tuple[int, ...]:
    """Read integer values from the numeric TIFF fields used by this fixture."""
    position, field_type, count = entry
    format_code = {1: "B", 3: "H", 4: "I"}.get(field_type)
    if format_code is None:
        raise ValueError(f"fixture field type {field_type} is not an integer")
    byte_count = TYPE_SIZES[field_type] * count
    payload_offset = (
        position + 8
        if byte_count <= 4
        else struct.unpack_from("<I", data, position + 8)[0]
    )
    payload_end = payload_offset + byte_count
    if payload_offset < 0 or payload_end > len(data):
        raise ValueError("fixture TIFF value extends beyond the file")
    return tuple(
        struct.unpack_from(f"<{format_code}", data, payload_offset + index * TYPE_SIZES[field_type])[0]
        for index in range(count)
    )


def reverse_bits(byte: int) -> int:
    """Reverse the bit order in one byte, as required by TIFF FillOrder 2."""
    return int(f"{byte:08b}"[::-1], 2)


def generate_fixture() -> bytes:
    """Insert FillOrder=2 and relocate each external value and tile."""
    source = SOURCE.read_bytes()
    source_digest = hashlib.sha256(source).hexdigest()
    if source_digest != EXPECTED_SOURCE_SHA256:
        raise RuntimeError(
            f"source SHA-256 mismatch: expected {EXPECTED_SOURCE_SHA256}, "
            f"found {source_digest}"
        )

    ifd_offset, entry_count, ifd_end, source_entries = parse_layout(source)
    required_tags = {256, 257, 258, 259, 262, 277, 284, 320, 322, 323, 324, 325}
    if set(source_entries) != required_tags:
        raise ValueError("source TIFF metadata differs from the 4-bit palette tile fixture")
    if FILL_ORDER_TAG in source_entries:
        raise ValueError("source TIFF already declares FillOrder")
    expected_values = {
        256: (17,),
        257: (1,),
        258: (4,),
        259: (1,),
        262: (3,),
        277: (1,),
        284: (1,),
        322: (9,),
        323: (16,),
    }
    if any(entry_values(source, source_entries[tag]) != values for tag, values in expected_values.items()):
        raise ValueError("source TIFF is not the expected 17x1 unaligned palette tile")

    source_offsets = entry_values(source, source_entries[TILE_OFFSETS_TAG])
    source_counts = entry_values(source, source_entries[TILE_BYTE_COUNTS_TAG])
    if not source_offsets or len(source_offsets) != len(source_counts):
        raise ValueError("source TIFF tile offsets and byte counts do not match")
    if any(
        offset < ifd_end or offset + count > len(source)
        for offset, count in zip(source_offsets, source_counts)
    ):
        raise ValueError("source TIFF tile payload is out of bounds")

    insertion_index = next(
        index
        for index, tag in enumerate(source_entries)
        if tag > FILL_ORDER_TAG
    )
    insertion_offset = ifd_offset + 2 + insertion_index * 12
    fill_order_entry = struct.pack("<HHIHH", FILL_ORDER_TAG, 3, 1, 2, 0)
    output = bytearray(
        source[:insertion_offset] + fill_order_entry + source[insertion_offset:]
    )
    struct.pack_into("<H", output, ifd_offset, entry_count + 1)

    shifted_entries = {
        tag: (position + (12 if position >= insertion_offset else 0), field_type, count)
        for tag, (position, field_type, count) in source_entries.items()
    }
    # Every value stored after the IFD moves with the inserted directory entry.
    for tag, (position, field_type, count) in shifted_entries.items():
        if TYPE_SIZES[field_type] * count > 4:
            old_offset = struct.unpack_from("<I", output, position + 8)[0]
            if old_offset < ifd_end:
                raise ValueError(f"TIFF tag {tag} points inside the source IFD")
            struct.pack_into("<I", output, position + 8, old_offset + 12)

    tile_offset_entry = shifted_entries[TILE_OFFSETS_TAG]
    tile_offsets_data = struct.unpack_from("<I", output, tile_offset_entry[0] + 8)[0]
    for index, source_offset in enumerate(source_offsets):
        struct.pack_into("<I", output, tile_offsets_data + index * 4, source_offset + 12)

    output_offsets = tuple(source_offset + 12 for source_offset in source_offsets)

    for output_offset, byte_count in zip(output_offsets, source_counts):
        tile_end = output_offset + byte_count
        output[output_offset:tile_end] = bytes(
            reverse_bits(byte) for byte in output[output_offset:tile_end]
        )

    encoded = bytes(output)
    verify_structure(encoded, source)
    output_digest = hashlib.sha256(encoded).hexdigest()
    if output_digest != EXPECTED_OUTPUT_SHA256:
        raise RuntimeError(
            f"generated TIFF SHA-256 mismatch: expected {EXPECTED_OUTPUT_SHA256}, "
            f"found {output_digest}"
        )
    return encoded


def generate_strip_fixture() -> bytes:
    """Insert FillOrder=2 and relocate every external value and strip."""
    source = STRIP_SOURCE.read_bytes()
    source_digest = hashlib.sha256(source).hexdigest()
    if source_digest != EXPECTED_STRIP_SOURCE_SHA256:
        raise RuntimeError(
            f"strip source SHA-256 mismatch: expected {EXPECTED_STRIP_SOURCE_SHA256}, "
            f"found {source_digest}"
        )

    ifd_offset, entry_count, ifd_end, source_entries = parse_layout(source)
    required_tags = {256, 257, 258, 259, 262, 273, 277, 278, 279, 320}
    if set(source_entries) != required_tags:
        raise ValueError("strip source metadata differs from the 4-bit palette fixture")
    if FILL_ORDER_TAG in source_entries:
        raise ValueError("strip source already declares FillOrder")
    expected_values = {
        256: (17,),
        257: (13,),
        258: (4,),
        259: (1,),
        262: (3,),
        277: (1,),
        278: (13,),
    }
    if any(
        entry_values(source, source_entries[tag]) != values
        for tag, values in expected_values.items()
    ):
        raise ValueError("strip source TIFF is not the expected 17x13 palette image")

    source_offsets = entry_values(source, source_entries[STRIP_OFFSETS_TAG])
    source_counts = entry_values(source, source_entries[STRIP_BYTE_COUNTS_TAG])
    if not source_offsets or len(source_offsets) != len(source_counts):
        raise ValueError("source TIFF strip offsets and byte counts do not match")
    if any(
        offset < ifd_end or offset + count > len(source)
        for offset, count in zip(source_offsets, source_counts)
    ):
        raise ValueError("source TIFF strip payload is out of bounds")

    insertion_index = next(
        index for index, tag in enumerate(source_entries) if tag > FILL_ORDER_TAG
    )
    insertion_offset = ifd_offset + 2 + insertion_index * 12
    fill_order_entry = struct.pack("<HHIHH", FILL_ORDER_TAG, 3, 1, 2, 0)
    output = bytearray(
        source[:insertion_offset] + fill_order_entry + source[insertion_offset:]
    )
    struct.pack_into("<H", output, ifd_offset, entry_count + 1)

    shifted_entries = {
        tag: (position + (12 if position >= insertion_offset else 0), field_type, count)
        for tag, (position, field_type, count) in source_entries.items()
    }
    for tag, (position, field_type, count) in shifted_entries.items():
        if TYPE_SIZES[field_type] * count > 4:
            old_offset = struct.unpack_from("<I", output, position + 8)[0]
            if old_offset < ifd_end:
                raise ValueError(f"TIFF tag {tag} points inside the source IFD")
            struct.pack_into("<I", output, position + 8, old_offset + 12)

    strip_offsets_entry = shifted_entries[STRIP_OFFSETS_TAG]
    if TYPE_SIZES[strip_offsets_entry[1]] * strip_offsets_entry[2] > 4:
        strip_offsets_data = struct.unpack_from(
            "<I", output, strip_offsets_entry[0] + 8
        )[0]
    else:
        strip_offsets_data = strip_offsets_entry[0] + 8
    for index, source_offset in enumerate(source_offsets):
        struct.pack_into("<I", output, strip_offsets_data + index * 4, source_offset + 12)

    output_offsets = tuple(source_offset + 12 for source_offset in source_offsets)
    for output_offset, byte_count in zip(output_offsets, source_counts):
        payload_end = output_offset + byte_count
        output[output_offset:payload_end] = bytes(
            reverse_bits(byte) for byte in output[output_offset:payload_end]
        )

    encoded = bytes(output)
    verify_structure(
        encoded,
        source,
        offsets_tag=STRIP_OFFSETS_TAG,
        byte_counts_tag=STRIP_BYTE_COUNTS_TAG,
    )
    output_digest = hashlib.sha256(encoded).hexdigest()
    if output_digest != EXPECTED_STRIP_OUTPUT_SHA256:
        raise RuntimeError(
            f"generated strip TIFF SHA-256 mismatch: expected "
            f"{EXPECTED_STRIP_OUTPUT_SHA256}, found {output_digest}"
        )
    return encoded


def verify_structure(
    output: bytes,
    source: bytes,
    *,
    offsets_tag: int = TILE_OFFSETS_TAG,
    byte_counts_tag: int = TILE_BYTE_COUNTS_TAG,
) -> None:
    """Check ordered tags, relocated offsets, and every transformed payload."""
    source_ifd, source_count, source_end, source_entries = parse_layout(source)
    output_ifd, output_count, output_end, output_entries = parse_layout(output)
    if (
        output_ifd != source_ifd
        or output_count != source_count + 1
        or output_end != source_end + 12
        or len(output) != len(source) + 12
    ):
        raise RuntimeError("FillOrder fixture changed its IFD geometry unexpectedly")
    if list(output_entries) != sorted(output_entries):
        raise RuntimeError("FillOrder fixture tags are not sorted")
    if entry_values(output, output_entries[FILL_ORDER_TAG]) != (2,):
        raise RuntimeError("FillOrder fixture does not declare value 2")
    if output_entries[FILL_ORDER_TAG][1:] != (3, 1):
        raise RuntimeError("FillOrder fixture does not declare one SHORT FillOrder")

    for tag, source_entry in source_entries.items():
        output_entry = output_entries.get(tag)
        if output_entry is None or output_entry[1:] != source_entry[1:]:
            raise RuntimeError(f"FillOrder fixture changed the shape of TIFF tag {tag}")
        source_values = entry_values(source, source_entry)
        output_values = entry_values(output, output_entry)
        expected_values = (
            tuple(value + 12 for value in source_values)
            if tag == offsets_tag
            else source_values
        )
        if output_values != expected_values:
            raise RuntimeError(f"FillOrder fixture changed TIFF tag {tag} values")
        byte_count = TYPE_SIZES[source_entry[1]] * source_entry[2]
        if byte_count > 4:
            source_pointer = struct.unpack_from("<I", source, source_entry[0] + 8)[0]
            output_pointer = struct.unpack_from("<I", output, output_entry[0] + 8)[0]
            if output_pointer != source_pointer + 12:
                raise RuntimeError(f"FillOrder fixture did not relocate TIFF tag {tag}")

    source_offsets = entry_values(source, source_entries[offsets_tag])
    source_counts = entry_values(source, source_entries[byte_counts_tag])
    output_offsets = entry_values(output, output_entries[offsets_tag])
    if len(source_offsets) != len(source_counts) or len(output_offsets) != len(source_offsets):
        raise RuntimeError("FillOrder fixture payload offsets and byte counts do not match")
    for source_offset, output_offset, byte_count in zip(
        source_offsets, output_offsets, source_counts
    ):
        source_payload = source[source_offset : source_offset + byte_count]
        output_payload = output[output_offset : output_offset + byte_count]
        if output_payload != bytes(reverse_bits(byte) for byte in source_payload):
            raise RuntimeError("FillOrder fixture did not reverse every payload byte")


def verify_pillow_reference(encoded: bytes, expected_size: tuple[int, int]) -> None:
    """Confirm the pinned Pillow plugin opens and verifies, then rejects load."""
    from PIL import Image, __version__ as pillow_version

    if pillow_version != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {pillow_version}")
    with Image.open(BytesIO(encoded)) as image:
        if (image.format, image.mode, image.size, image.tag_v2.get(FILL_ORDER_TAG)) != (
            "TIFF",
            "P",
            expected_size,
            2,
        ):
            raise RuntimeError("Pillow did not open the expected TIFF palette image")
        image.verify()
    try:
        with Image.open(BytesIO(encoded)) as image:
            image.load()
    except ValueError as error:
        if str(error) != "unknown raw mode for given image mode":
            raise RuntimeError(f"Pillow raised an unexpected load error: {error}") from error
    else:
        raise RuntimeError("Pillow unexpectedly loads TIFF palette FillOrder 2")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed fixture without rewriting it",
    )
    args = parser.parse_args()
    fixtures = (
        (OUTPUT, generate_fixture(), (17, 1)),
        (STRIP_OUTPUT, generate_strip_fixture(), (17, 13)),
    )
    if args.check:
        for output, encoded, _ in fixtures:
            if not output.is_file() or output.read_bytes() != encoded:
                raise RuntimeError(f"{output.relative_to(ROOT)} differs from generated bytes")
            print(f"Fixture matches: {output.relative_to(ROOT)}")
        return 0

    for _, encoded, expected_size in fixtures:
        verify_pillow_reference(encoded, expected_size)
    for output, encoded, _ in fixtures:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(encoded)
        print(
            f"Wrote {output.relative_to(ROOT)} ({len(encoded)} bytes, "
            f"SHA-256 {hashlib.sha256(encoded).hexdigest()})"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
