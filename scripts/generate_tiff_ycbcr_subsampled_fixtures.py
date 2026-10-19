#!/usr/bin/env python3
"""Generate Pillow-parity fixtures for subsampled TIFF YCbCr LZW strips."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from math import isnan
from pathlib import Path
import struct

import yaml

from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DIRECTORY = ROOT / "tests" / "fixtures" / "input" / "images" / "tiff"
PINNED_PILLOW = "12.2.0"
REFERENCE_PIXELS = bytes.fromhex(
    "004000004000002f000b61003da4000b72003d93000b6100001e003d8232000c276fa2bd3d82320b50003d708b0b3e59"
)
REFERENCE_PIXEL_SHA256 = sha256(REFERENCE_PIXELS).hexdigest()

REFERENCE_CANDIDATE = bytes.fromhex(
    "49 49 2a 00 1e 00 00 00 "
    "80 00 00 06 41 94 0c 00 32 82 42 20 70 43 21 90 00 96 82 44 20 20 "
    "0c 00 "
    "00 01 03 00 01 00 00 00 04 00 00 00 "
    "01 01 03 00 01 00 00 00 04 00 00 00 "
    "02 01 03 00 03 00 00 00 b4 00 00 00 "
    "03 01 03 00 01 00 00 00 05 00 00 00 "
    "06 01 03 00 01 00 00 00 06 00 00 00 "
    "11 01 04 00 01 00 00 00 08 00 00 00 "
    "15 01 03 00 01 00 00 00 03 00 00 00 "
    "16 01 03 00 01 00 00 00 04 00 00 00 "
    "17 01 04 00 01 00 00 00 16 00 00 00 "
    "1c 01 03 00 01 00 00 00 01 00 00 00 "
    "12 02 03 00 02 00 00 00 02 00 02 00 "
    "13 02 03 00 01 00 00 00 01 00 00 00 "
    "00 00 00 00 08 00 08 00 08 00"
)

ODD_EDGE_CLUMPS = (
    (80, 96, 112, 128, 90, 180),
    (144, 160, 176, 192, 130, 30),
    (208, 224, 240, 0, 64, 200),
    (16, 32, 48, 64, 192, 70),
)

TWO_STRIP_CLUMPS = (
    (20, 40, 60, 80, 70, 160),
    (100, 120, 140, 160, 180, 30),
    (180, 200, 220, 240, 100, 220),
    (4, 24, 44, 64, 210, 50),
)

BASE_CLUMPS = (
    (0, 0, 100, 50, 0, 100),
    (0, 50, 100, 50, 50, 100),
    (0, 100, 100, 50, 100, 100),
    (0, 150, 100, 50, 150, 100),
)


def lzw_blocks(
    width: int,
    height: int,
    rows_per_strip: int,
    clumps: tuple[tuple[int, ...], ...],
    *,
    subsampling: tuple[int, int] = (2, 2),
) -> list[bytes]:
    """Encode one TIFF LZW stream for each strip of complete clump rows."""
    if width <= 0 or height <= 0:
        raise ValueError("YCbCr image dimensions must be positive")
    horizontal, vertical = subsampling
    if horizontal <= 0 or vertical <= 0:
        raise ValueError("YCbCr subsampling factors must be positive")
    if rows_per_strip <= 0 or rows_per_strip % vertical != 0:
        raise ValueError("YCbCr RowsPerStrip must align to subsampled rows")
    clump_columns = (width + horizontal - 1) // horizontal
    clump_rows = (height + vertical - 1) // vertical
    clump_rows_per_strip = rows_per_strip // vertical
    samples_per_clump = horizontal * vertical + 2
    if len(clumps) != clump_columns * clump_rows:
        raise ValueError("YCbCr clump count does not match image geometry")
    if any(
        len(clump) != samples_per_clump
        or any(not 0 <= sample <= 255 for sample in clump)
        for clump in clumps
    ):
        raise ValueError(
            f"each YCbCr clump must contain {samples_per_clump} 8-bit samples"
        )

    blocks = []
    for first_row in range(0, clump_rows, clump_rows_per_strip):
        start = first_row * clump_columns
        end = min(first_row + clump_rows_per_strip, clump_rows) * clump_columns
        samples = bytes(value for clump in clumps[start:end] for value in clump)
        blocks.append(pack_lzw_codes_with_growth([256, *samples, 257]))
    return blocks


def deterministic_factor_clumps(
    width: int, height: int, subsampling: tuple[int, int]
) -> tuple[tuple[int, ...], ...]:
    """Build varied luma/chroma clumps for accepted tag 530 factor pairs."""
    horizontal, vertical = subsampling
    clump_columns = (width + horizontal - 1) // horizontal
    clump_rows = (height + vertical - 1) // vertical
    return tuple(
        tuple(
            [
                (17 + row * 31 + column * 19 + sample * 13) % 256
                for sample in range(horizontal * vertical)
            ]
            + [73 + row * 17 + column * 9, 181 - row * 11 - column * 7]
        )
        for row in range(clump_rows)
        for column in range(clump_columns)
    )


def pack_lzw_codes_with_growth(codes: list[int]) -> bytes:
    """Pack TIFF LZW codes with the format's early-change code widths."""
    fields = []
    code_width = 9
    next_code = 258
    has_previous = False
    for code in codes:
        fields.append((code, code_width))
        if code == 256:
            code_width = 9
            next_code = 258
            has_previous = False
        elif code == 257:
            break
        elif not has_previous:
            has_previous = True
        elif next_code < 4096:
            next_code += 1
            if code_width < 12 and next_code == (1 << code_width) - 1:
                code_width += 1

    bits = "".join(f"{code:0{width}b}" for code, width in fields)
    bits += "0" * (-len(bits) % 8)
    return int(bits, 2).to_bytes(len(bits) // 8, "big")


def build_tiff(
    width: int,
    height: int,
    rows_per_strip: int,
    clumps: tuple[tuple[int, ...], ...],
    *,
    subsampling: tuple[int, int] | None,
    positioning: int | None,
    coefficients: tuple[tuple[int, int], ...] | None = None,
    reference_black_white: tuple[tuple[int, int], ...] | None = None,
) -> bytes:
    """Write a compact contiguous YCbCr TIFF with independent LZW strips."""
    storage_subsampling = (2, 2) if subsampling is None else subsampling
    blocks = lzw_blocks(
        width,
        height,
        rows_per_strip,
        clumps,
        subsampling=storage_subsampling,
    )
    entries = [
        (256, 4, [width]),
        (257, 4, [height]),
        (258, 3, [8, 8, 8]),
        (259, 3, [5]),
        (262, 3, [6]),
        (273, 4, [0] * len(blocks)),
        (277, 3, [3]),
        (278, 4, [rows_per_strip]),
        (279, 4, [len(block) for block in blocks]),
        (284, 3, [1]),
    ]
    if subsampling is not None:
        entries.append((530, 3, list(subsampling)))
    if positioning is not None:
        entries.append((531, 3, [positioning]))
    if coefficients is not None:
        if len(coefficients) != 3:
            raise ValueError("YCbCr coefficients must contain three rationals")
        entries.append((529, 5, [value for pair in coefficients for value in pair]))
    if reference_black_white is not None:
        if len(reference_black_white) != 6:
            raise ValueError("YCbCr reference black and white must contain six rationals")
        entries.append((532, 5, [value for pair in reference_black_white for value in pair]))
    entries.sort()

    def pack_values(field_type: int, values: list[int]) -> bytes:
        scalar_type = "H" if field_type == 3 else "I"
        return struct.pack("<" + scalar_type * len(values), *values)

    def value_count(field_type: int, values: list[int]) -> int:
        return len(values) // 2 if field_type == 5 else len(values)

    cursor = 8 + 2 + 12 * len(entries) + 4
    buffers: dict[int, tuple[int, bytes]] = {}
    for tag, field_type, values in entries:
        packed = pack_values(field_type, values)
        if len(packed) > 4:
            cursor += cursor % 2
            buffers[tag] = (cursor, packed)
            cursor += len(packed)

    block_offsets = []
    for block in blocks:
        cursor += cursor % 2
        block_offsets.append(cursor)
        cursor += len(block)
    if len(block_offsets) > 1:
        offsets_position, _ = buffers[273]
        buffers[273] = (
            offsets_position,
            struct.pack("<" + "I" * len(block_offsets), *block_offsets),
        )

    output = bytearray(b"II*\0" + struct.pack("<I", 8) + struct.pack("<H", len(entries)))
    for tag, field_type, values in entries:
        if tag == 273:
            values = block_offsets
        packed = pack_values(field_type, values)
        value = (
            struct.pack("<I", buffers[tag][0])
            if len(packed) > 4
            else packed.ljust(4, b"\0")
        )
        output.extend(struct.pack("<HHI", tag, field_type, value_count(field_type, values)) + value)
    output.extend(bytes(4))
    for offset, buffer in sorted(buffers.values()):
        output.extend(bytes(offset - len(output)))
        output.extend(buffer)
    for block in blocks:
        if len(output) % 2:
            output.append(0)
        output.extend(block)
    return bytes(output)


def build_full_resolution_lzw_tiff(
    width: int,
    height: int,
    pixels: bytes,
    *,
    coefficients: tuple[tuple[int, int], ...] | None = None,
    reference_black_white: tuple[tuple[int, int], ...] | None = None,
) -> bytes:
    """Create a deterministic contiguous 1-by-1 YCbCr LZW source."""
    if width <= 0 or height <= 0 or len(pixels) != width * height * 3:
        raise ValueError("full-resolution YCbCr samples do not match image geometry")
    block = pack_lzw_codes_with_growth([256, *pixels, 257])
    entries = [
        (256, 4, [width]),
        (257, 4, [height]),
        (258, 3, [8, 8, 8]),
        (259, 3, [5]),
        (262, 3, [6]),
        (273, 4, [0]),
        (277, 3, [3]),
        (278, 4, [height]),
        (279, 4, [len(block)]),
        (284, 3, [1]),
        (530, 3, [1, 1]),
    ]
    if coefficients is not None:
        if len(coefficients) != 3:
            raise ValueError("YCbCr coefficients must contain three rationals")
        entries.append((529, 5, [value for pair in coefficients for value in pair]))
    if reference_black_white is not None:
        if len(reference_black_white) != 6:
            raise ValueError("YCbCr reference black and white must contain six rationals")
        entries.append(
            (532, 5, [value for pair in reference_black_white for value in pair])
        )
    entries.sort()
    cursor = 8 + 2 + 12 * len(entries) + 4
    buffers: dict[int, tuple[int, bytes]] = {}
    for tag, field_type, values in entries:
        packed = struct.pack(
            "<" + ("H" if field_type == 3 else "I") * len(values), *values
        )
        if len(packed) > 4:
            cursor += cursor % 2
            buffers[tag] = (cursor, packed)
            cursor += len(packed)

    cursor += cursor % 2
    block_offset = cursor
    output = bytearray(b"II*\0" + struct.pack("<I", 8) + struct.pack("<H", len(entries)))
    for tag, field_type, values in entries:
        if tag == 273:
            values = [block_offset]
        packed = struct.pack(
            "<" + ("H" if field_type == 3 else "I") * len(values), *values
        )
        value = (
            struct.pack("<I", buffers[tag][0])
            if len(packed) > 4
            else packed.ljust(4, b"\0")
        )
        value_count = len(values) // 2 if field_type == 5 else len(values)
        output.extend(struct.pack("<HHI", tag, field_type, value_count) + value)
    output.extend(bytes(4))
    for offset, buffer in sorted(buffers.values()):
        output.extend(bytes(offset - len(output)))
        output.extend(buffer)
    output.extend(bytes(block_offset - len(output)))
    output.extend(block)
    return bytes(output)


def build_separate_full_resolution_lzw_tiff(
    width: int, height: int, pixels: bytes
) -> bytes:
    """Create a full-resolution YCbCr TIFF with three LZW-compressed planes."""
    if width <= 0 or height <= 0 or len(pixels) != width * height * 3:
        raise ValueError("separate YCbCr samples do not match image geometry")

    planes = [bytes(pixels[channel::3]) for channel in range(3)]
    blocks = [pack_lzw_codes_with_growth([256, *plane, 257]) for plane in planes]
    entries = [
        (256, 4, [width]),
        (257, 4, [height]),
        (258, 3, [8, 8, 8]),
        (259, 3, [5]),
        (262, 3, [6]),
        (273, 4, [0, 0, 0]),
        (277, 3, [3]),
        (278, 4, [height]),
        (279, 4, [len(block) for block in blocks]),
        (284, 3, [2]),
        (530, 3, [1, 1]),
        (531, 3, [1]),
    ]
    entries.sort()

    cursor = 8 + 2 + 12 * len(entries) + 4
    buffers: dict[int, tuple[int, bytes]] = {}
    for tag, field_type, values in entries:
        scalar_type = "H" if field_type == 3 else "I"
        packed = struct.pack("<" + scalar_type * len(values), *values)
        if len(packed) > 4:
            cursor += cursor % 2
            buffers[tag] = (cursor, packed)
            cursor += len(packed)

    block_offsets = []
    for block in blocks:
        cursor += cursor % 2
        block_offsets.append(cursor)
        cursor += len(block)
    offsets_position, _ = buffers[273]
    buffers[273] = (offsets_position, struct.pack("<III", *block_offsets))

    output = bytearray(b"II*\0" + struct.pack("<I", 8) + struct.pack("<H", len(entries)))
    for tag, field_type, values in entries:
        if tag == 273:
            values = block_offsets
        scalar_type = "H" if field_type == 3 else "I"
        packed = struct.pack("<" + scalar_type * len(values), *values)
        value = (
            struct.pack("<I", buffers[tag][0])
            if len(packed) > 4
            else packed.ljust(4, b"\0")
        )
        output.extend(struct.pack("<HHI", tag, field_type, len(values)) + value)
    output.extend(bytes(4))
    for offset, buffer in sorted(buffers.values()):
        output.extend(bytes(offset - len(output)))
        output.extend(buffer)
    for block in blocks:
        if len(output) % 2:
            output.append(0)
        output.extend(block)
    return bytes(output)


def mutate_ifd_tag(
    data: bytes, tag: int, *, field_type: int | None = None, count: int | None = None
) -> bytes:
    """Change a generated little-endian TIFF tag type or count in place."""
    if data[:4] != b"II*\0":
        raise ValueError("tag mutations require a generated little-endian TIFF")
    output = bytearray(data)
    ifd_offset = struct.unpack_from("<I", output, 4)[0]
    entry_count = struct.unpack_from("<H", output, ifd_offset)[0]
    for index in range(entry_count):
        entry_offset = ifd_offset + 2 + 12 * index
        entry_tag = struct.unpack_from("<H", output, entry_offset)[0]
        if entry_tag != tag:
            continue
        if field_type is not None:
            struct.pack_into("<H", output, entry_offset + 2, field_type)
        if count is not None:
            struct.pack_into("<I", output, entry_offset + 4, count)
        return bytes(output)
    raise ValueError(f"generated TIFF does not contain tag {tag}")


def set_ifd_subsampling_values(data: bytes, field_type: int, values: tuple[int, ...]) -> bytes:
    """Set tag 530 to supported integer or ASCII values."""
    scalar_type = {1: "B", 2: "B", 3: "H", 4: "I", 6: "b", 8: "h", 9: "i"}.get(
        field_type
    )
    if scalar_type is None:
        raise ValueError("unsupported YCbCr subsampling fixture field type")
    packed = struct.pack("<" + scalar_type * len(values), *values)
    output = bytearray(data)
    if output[:4] != b"II*\0":
        raise ValueError("subsampling mutations require a generated little-endian TIFF")
    ifd_offset = struct.unpack_from("<I", output, 4)[0]
    entry_count = struct.unpack_from("<H", output, ifd_offset)[0]
    for index in range(entry_count):
        entry_offset = ifd_offset + 2 + 12 * index
        if struct.unpack_from("<H", output, entry_offset)[0] != 530:
            continue
        struct.pack_into("<H", output, entry_offset + 2, field_type)
        struct.pack_into("<I", output, entry_offset + 4, len(values))
        if len(packed) <= 4:
            output[entry_offset + 8 : entry_offset + 12] = packed.ljust(4, b"\0")
        else:
            payload_offset = len(output) + len(output) % 2
            if payload_offset > len(output):
                output.append(0)
            output.extend(packed)
            struct.pack_into("<I", output, entry_offset + 8, payload_offset)
        return bytes(output)
    raise ValueError("generated TIFF does not contain tag 530")


def set_ifd_float_values(
    data: bytes, tag: int, field_type: int, values: tuple[float, ...]
) -> bytes:
    """Set an existing YCbCr FLOAT or DOUBLE payload to actual IEEE values."""
    scalar_type = {11: "f", 12: "d"}.get(field_type)
    expected_count = {529: 3, 532: 6}.get(tag)
    if scalar_type is None or expected_count is None or len(values) != expected_count:
        raise ValueError("invalid YCbCr floating-value fixture")
    packed = struct.pack("<" + scalar_type * len(values), *values)
    output = bytearray(data)
    if output[:4] != b"II*\0":
        raise ValueError("floating-value mutations require a generated little-endian TIFF")
    ifd_offset = struct.unpack_from("<I", output, 4)[0]
    entry_count = struct.unpack_from("<H", output, ifd_offset)[0]
    for index in range(entry_count):
        entry_offset = ifd_offset + 2 + 12 * index
        if struct.unpack_from("<H", output, entry_offset)[0] != tag:
            continue
        payload_offset = struct.unpack_from("<I", output, entry_offset + 8)[0]
        payload_end = payload_offset + len(packed)
        if payload_end > len(output):
            raise ValueError("YCbCr floating-value payload exceeds the generated TIFF")
        struct.pack_into("<HI", output, entry_offset + 2, field_type, len(values))
        output[payload_offset:payload_end] = packed
        return bytes(output)
    raise ValueError(f"generated TIFF does not contain tag {tag}")


def float32_values(values: tuple[float, ...]) -> tuple[float, ...]:
    """Return the exact Python values Pillow exposes for a TIFF FLOAT array."""
    return tuple(struct.unpack("<f", struct.pack("<f", value))[0] for value in values)


def set_ifd_short_value(data: bytes, tag: int, value: int) -> bytes:
    """Replace one inline SHORT value in a generated little-endian TIFF."""
    if data[:4] != b"II*\0":
        raise ValueError("SHORT mutations require a generated little-endian TIFF")
    output = bytearray(data)
    ifd_offset = struct.unpack_from("<I", output, 4)[0]
    entry_count = struct.unpack_from("<H", output, ifd_offset)[0]
    for index in range(entry_count):
        entry_offset = ifd_offset + 2 + 12 * index
        if struct.unpack_from("<H", output, entry_offset)[0] != tag:
            continue
        field_type = struct.unpack_from("<H", output, entry_offset + 2)[0]
        count = struct.unpack_from("<I", output, entry_offset + 4)[0]
        if field_type != 3 or count != 1:
            raise ValueError(f"tag {tag} is not one inline SHORT value")
        struct.pack_into("<H", output, entry_offset + 8, value)
        return bytes(output)
    raise ValueError(f"generated TIFF does not contain tag {tag}")


def set_ifd_srational_values(
    data: bytes, tag: int, values: tuple[tuple[int, int], ...]
) -> bytes:
    """Store signed rational words in an existing external TIFF tag payload."""
    if data[:4] != b"II*\0":
        raise ValueError("rational mutations require a generated little-endian TIFF")
    output = bytearray(data)
    ifd_offset = struct.unpack_from("<I", output, 4)[0]
    entry_count = struct.unpack_from("<H", output, ifd_offset)[0]
    for index in range(entry_count):
        entry_offset = ifd_offset + 2 + 12 * index
        if struct.unpack_from("<H", output, entry_offset)[0] != tag:
            continue
        count = struct.unpack_from("<I", output, entry_offset + 4)[0]
        if count != len(values):
            raise ValueError(f"tag {tag} does not have the expected rational count")
        value_offset = struct.unpack_from("<I", output, entry_offset + 8)[0]
        payload = b"".join(struct.pack("<ii", numerator, denominator) for numerator, denominator in values)
        output[entry_offset + 2 : entry_offset + 4] = struct.pack("<H", 10)
        output[value_offset : value_offset + len(payload)] = payload
        return bytes(output)
    raise ValueError(f"generated TIFF does not contain tag {tag}")


def fixtures() -> dict[
    str, tuple[bytes, tuple[int, int], str | None, dict[int, object], bytes | None]
]:
    """Return deterministic inputs and the pinned Pillow result metadata."""
    one_by_two_clumps = tuple(
        (
            (31 + 37 * (2 * clump_row) + 23 * x) % 256,
            (31 + 37 * (2 * clump_row + 1) + 23 * x) % 256,
            90 + 11 * x + 17 * clump_row,
            180 - 13 * x - 9 * clump_row,
        )
        for clump_row in range(2)
        for x in range(4)
    )
    one_by_two = build_tiff(
        4,
        4,
        4,
        one_by_two_clumps,
        subsampling=(1, 2),
        positioning=1,
    )
    one_by_two_odd_height = build_tiff(
        4,
        3,
        4,
        one_by_two_clumps,
        subsampling=(1, 2),
        positioning=1,
    )
    custom_coefficients_one_by_two_odd_height = build_tiff(
        4,
        3,
        4,
        one_by_two_clumps,
        subsampling=(1, 2),
        positioning=1,
        coefficients=((2126, 10000), (7152, 10000), (722, 10000)),
    )
    odd_edge = build_tiff(
        3,
        3,
        4,
        ODD_EDGE_CLUMPS,
        subsampling=None,
        positioning=None,
    )
    two_strips = build_tiff(
        4,
        4,
        2,
        TWO_STRIP_CLUMPS,
        subsampling=(2, 2),
        positioning=2,
    )
    custom_coefficients = build_tiff(
        4,
        4,
        4,
        BASE_CLUMPS,
        subsampling=(2, 2),
        positioning=1,
        coefficients=((2126, 10000), (7152, 10000), (722, 10000)),
    )
    zero_green_coefficient = build_tiff(
        4,
        4,
        4,
        BASE_CLUMPS,
        subsampling=(2, 2),
        positioning=1,
        coefficients=((299, 1000), (0, 1000), (114, 1000)),
    )
    zero_red_coefficient_denominator = build_tiff(
        4,
        4,
        4,
        BASE_CLUMPS,
        subsampling=(2, 2),
        positioning=1,
        coefficients=((299, 0), (587, 1000), (114, 1000)),
    )
    zero_green_coefficient_denominator = build_tiff(
        4,
        4,
        4,
        BASE_CLUMPS,
        subsampling=(2, 2),
        positioning=1,
        coefficients=((299, 1000), (587, 0), (114, 1000)),
    )
    zero_blue_coefficient_denominator = build_tiff(
        4,
        4,
        4,
        BASE_CLUMPS,
        subsampling=(2, 2),
        positioning=1,
        coefficients=((299, 1000), (587, 1000), (114, 0)),
    )
    zero_reference_white_denominator = build_tiff(
        4,
        4,
        4,
        BASE_CLUMPS,
        subsampling=(2, 2),
        positioning=1,
        reference_black_white=((0, 1), (255, 0), (128, 1), (255, 1), (128, 1), (255, 1)),
    )
    accepted_reference_limit = build_tiff(
        4,
        4,
        4,
        BASE_CLUMPS,
        subsampling=(2, 2),
        positioning=1,
        reference_black_white=(
            (0, 1),
            (4_294_967_166, 2),
            (128, 1),
            (255, 1),
            (128, 1),
            (255, 1),
        ),
    )
    rejected_reference_limit = build_tiff(
        4,
        4,
        4,
        BASE_CLUMPS,
        subsampling=(2, 2),
        positioning=1,
        reference_black_white=(
            (0, 1),
            (4_294_967_168, 2),
            (128, 1),
            (255, 1),
            (128, 1),
            (255, 1),
        ),
    )
    custom_reference_black_white = build_tiff(
        4,
        4,
        4,
        BASE_CLUMPS,
        subsampling=(2, 2),
        positioning=1,
        reference_black_white=((16, 1), (235, 1), (128, 1), (240, 1), (128, 1), (240, 1)),
    )
    custom_coefficients_and_reference_black_white = build_tiff(
        4,
        4,
        4,
        BASE_CLUMPS,
        subsampling=(2, 2),
        positioning=1,
        coefficients=((2126, 10000), (7152, 10000), (722, 10000)),
        reference_black_white=((16, 1), (235, 1), (128, 1), (240, 1), (128, 1), (240, 1)),
    )
    custom_coefficients_only = build_tiff(
        4,
        4,
        4,
        BASE_CLUMPS,
        subsampling=(2, 2),
        positioning=1,
        coefficients=((2126, 10000), (7152, 10000), (722, 10000)),
    )
    equivalent_default_rational = build_tiff(
        4,
        4,
        4,
        BASE_CLUMPS,
        subsampling=(2, 2),
        positioning=1,
        coefficients=((598, 2000), (1174, 2000), (228, 2000)),
        reference_black_white=(
            (0, 2),
            (510, 2),
            (256, 2),
            (510, 2),
            (256, 2),
            (510, 2),
        ),
    )
    custom_reference_only = build_tiff(
        4,
        4,
        4,
        BASE_CLUMPS,
        subsampling=(2, 2),
        positioning=1,
        reference_black_white=((16, 1), (235, 1), (128, 1), (240, 1), (128, 1), (240, 1)),
    )
    float_coefficients = (0.2126, 0.7152, 0.0722)
    float_reference_black_white = (16.0, 235.0, 128.0, 240.0, 128.0, 240.0)
    coefficients_float_type = set_ifd_float_values(
        custom_coefficients_only, 529, 11, float_coefficients
    )
    coefficients_double_values = set_ifd_float_values(
        custom_coefficients_only, 529, 12, float_coefficients
    )
    coefficients_double_infinity = set_ifd_float_values(
        custom_coefficients_only, 529, 12, (float("inf"), 0.7152, 0.0722)
    )
    reference_float_type = set_ifd_float_values(
        custom_reference_only, 532, 11, float_reference_black_white
    )
    reference_double_type = set_ifd_float_values(
        custom_reference_only, 532, 12, float_reference_black_white
    )
    reference_double_below_limit = set_ifd_float_values(
        custom_reference_only,
        532,
        12,
        (16.0, 2_147_483_583.0, 128.0, 240.0, 128.0, 240.0),
    )
    reference_double_at_limit = set_ifd_float_values(
        custom_reference_only,
        532,
        12,
        (16.0, 2_147_483_584.0, 128.0, 240.0, 128.0, 240.0),
    )
    coefficients_float_nan = set_ifd_float_values(
        custom_coefficients_only, 529, 11, (float("nan"), 0.7152, 0.0722)
    )
    coefficients_double_nan = set_ifd_float_values(
        custom_coefficients_only, 529, 12, (float("nan"), 0.7152, 0.0722)
    )
    reference_float_non_finite = set_ifd_float_values(
        custom_reference_only, 532, 11, (16.0, float("inf"), 128.0, 240.0, 128.0, 240.0)
    )
    reference_double_non_finite = set_ifd_float_values(
        custom_reference_only, 532, 12, (16.0, float("inf"), 128.0, 240.0, 128.0, 240.0)
    )
    coefficients_short_type = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 529, field_type=3
    )
    coefficients_ascii_type = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 529, field_type=2
    )
    coefficients_undefined_type = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 529, field_type=7
    )
    coefficients_long_type = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 529, field_type=4
    )
    coefficients_srational_type = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 529, field_type=10
    )
    reference_short_type = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 532, field_type=3
    )
    reference_ascii_type = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 532, field_type=2
    )
    reference_undefined_type = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 532, field_type=7
    )
    reference_long_type = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 532, field_type=4
    )
    reference_srational_type = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 532, field_type=10
    )
    negative_coefficients_srational = set_ifd_srational_values(
        custom_coefficients_and_reference_black_white,
        529,
        ((-2126, -10000), (-7152, -10000), (-722, -10000)),
    )
    negative_reference_srational = set_ifd_srational_values(
        custom_coefficients_and_reference_black_white,
        532,
        ((-16, -1), (-235, -1), (-128, -1), (-240, -1), (-128, -1), (-240, -1)),
    )
    coefficients_double_type = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 529, field_type=12
    )
    coefficients_short_count = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 529, count=2
    )
    coefficients_long_count = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 529, count=4
    )
    reference_short_count = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 532, count=5
    )
    reference_long_count = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 532, count=7
    )
    positioning_invalid_value = set_ifd_short_value(
        custom_coefficients_and_reference_black_white, 531, 0
    )
    positioning_long_type = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 531, field_type=4
    )
    positioning_zero_count = mutate_ifd_tag(
        custom_coefficients_and_reference_black_white, 531, count=0
    )
    full_resolution_pixels = bytes(
        value
        for y in range(4)
        for x in range(4)
        for value in (30 + 40 * x + 20 * y, 96 + 9 * x + 13 * y, 160 - 7 * x + 5 * y)
    )
    full_resolution = build_full_resolution_lzw_tiff(4, 4, full_resolution_pixels)
    full_resolution_separate = build_separate_full_resolution_lzw_tiff(
        4, 4, full_resolution_pixels
    )
    full_resolution_custom = build_full_resolution_lzw_tiff(
        4,
        4,
        full_resolution_pixels,
        coefficients=((2126, 10000), (7152, 10000), (722, 10000)),
        reference_black_white=(
            (16, 1),
            (235, 1),
            (128, 1),
            (240, 1),
            (128, 1),
            (240, 1),
        ),
    )
    checkpoint_clumps = tuple(
        (
            (index * 17 + 11) % 256,
            (index * 29 + 23) % 256,
            (index * 37 + 47) % 256,
            (index * 53 + 71) % 256,
            (index * 11 + 80) % 256,
            (index * 19 + 128) % 256,
        )
        for index in range(16 * 16)
    )
    checkpoint = build_tiff(
        32,
        32,
        32,
        checkpoint_clumps,
        subsampling=(2, 2),
        positioning=1,
    )
    factor_fixtures = {}
    for horizontal, vertical in ((2, 1), (4, 1), (4, 2), (4, 4)):
        subsampling = (horizontal, vertical)
        width = 8
        height = 8
        factor_fixtures[f"ycbcr_subsampled_lzw_{horizontal}x{vertical}.tiff"] = (
            build_tiff(
                width,
                height,
                height,
                deterministic_factor_clumps(width, height, subsampling),
                subsampling=subsampling,
                positioning=1,
            ),
            (width, height),
            {
                (2, 1): "919d55463c5f83d6ed6cd44fe05914879871d8daec7f8345113b0071339f5bd1",
                (4, 1): "61d626f6056a0386c908194af851d84da9c56cbf55d22428d42fabf014df2787",
                (4, 2): "ce4ab52f7dfd14008940466767f2a397e9b2177fc00d292794be0c5d44ee74bd",
                (4, 4): "73a2e7a7ea192ec490b4cedc7690286af5182a8fc141752db6b75303ee8174c3",
            }[subsampling],
            {530: subsampling, 531: 1},
            None,
        )

    custom_4x2 = build_tiff(
        8,
        8,
        8,
        deterministic_factor_clumps(8, 8, (4, 2)),
        subsampling=(4, 2),
        positioning=1,
        coefficients=((2126, 10000), (7152, 10000), (722, 10000)),
    )
    factor_fixtures["ycbcr_custom_coefficients_lzw_4x2.tiff"] = (
        custom_4x2,
        (8, 8),
        "6df0546698f326496000568126d81424f292fad152199d9894142d95f703c824",
        {
            529: (0.2126, 0.7152, 0.0722),
            530: (4, 2),
            531: 1,
        },
        bytes.fromhex(
            "640200710f007e1c008b29006c1700792400863100933e00983600a54300"
            "b25000bf5d06a04b03ad5810ba651dc7722a7223007f30008c3d03994a10"
            "7a380d87451a945227a15f34a6571db3642ac07137cd7e44ae6c41bb794e"
            "c8865bd593688044288d51359a5e42a76b4f88594c956659a27366af8073"
            "b4785cc18569ce9276db9f83bc8d80c99a8dd6a79ae3b4a78d65679a7274"
            "a77f81b48c8e957a8aa28797af94a4bca1b1c1999bcea6a8dbb3b5e8c0c2"
            "c9aebed6bbcbe3c8d8f0d5e5"
        ),
    )

    odd_edge_width = 5
    odd_edge_height = 5
    odd_edge_subsampling = (4, 4)
    odd_edge_4x4 = build_tiff(
        odd_edge_width,
        odd_edge_height,
        4,
        deterministic_factor_clumps(
            odd_edge_width, odd_edge_height, odd_edge_subsampling
        ),
        subsampling=odd_edge_subsampling,
        positioning=1,
    )
    custom_odd_edge_4x4 = build_tiff(
        odd_edge_width,
        odd_edge_height,
        4,
        deterministic_factor_clumps(
            odd_edge_width, odd_edge_height, odd_edge_subsampling
        ),
        subsampling=odd_edge_subsampling,
        positioning=1,
        coefficients=((2126, 10000), (7152, 10000), (722, 10000)),
    )

    byte_subsampling = set_ifd_subsampling_values(REFERENCE_CANDIDATE, 1, (2, 2))
    long_subsampling = set_ifd_subsampling_values(REFERENCE_CANDIDATE, 4, (2, 2))
    short_count_subsampling = set_ifd_subsampling_values(REFERENCE_CANDIDATE, 3, (2,))
    unsupported_pair_subsampling = set_ifd_subsampling_values(
        REFERENCE_CANDIDATE, 3, (3, 1)
    )
    zero_pair_subsampling = set_ifd_subsampling_values(REFERENCE_CANDIDATE, 3, (0, 0))
    signed_byte_subsampling = set_ifd_subsampling_values(one_by_two, 6, (1, 2))
    signed_short_subsampling = set_ifd_subsampling_values(one_by_two, 8, (1, 2))
    signed_long_subsampling = set_ifd_subsampling_values(one_by_two, 9, (1, 2))
    signed_short_negative_subsampling = set_ifd_subsampling_values(
        REFERENCE_CANDIDATE, 8, (-1, 2)
    )
    signed_short_negative_vertical_subsampling = set_ifd_subsampling_values(
        REFERENCE_CANDIDATE, 8, (1, -2)
    )
    ascii_subsampling = set_ifd_subsampling_values(REFERENCE_CANDIDATE, 2, (49, 49))
    short_count_three_subsampling = set_ifd_subsampling_values(
        REFERENCE_CANDIDATE, 3, (1, 2, 2)
    )

    return {
        "ycbcr_subsampled_lzw_2x2.tiff": (
            REFERENCE_CANDIDATE,
            (4, 4),
            REFERENCE_PIXEL_SHA256,
            {530: (2, 2), 531: 1},
            REFERENCE_PIXELS,
        ),
        "ycbcr_equivalent_default_rational_lzw_2x2.tiff": (
            equivalent_default_rational,
            (4, 4),
            REFERENCE_PIXEL_SHA256,
            {
                529: (0.299, 0.587, 0.114),
                530: (2, 2),
                531: 1,
                532: (0.0, 255.0, 128.0, 255.0, 128.0, 255.0),
            },
            REFERENCE_PIXELS,
        ),
        "ycbcr_subsampling_byte_type_lzw_2x2.tiff": (
            byte_subsampling,
            (4, 4),
            REFERENCE_PIXEL_SHA256,
            {530: b"\x02\x02", 531: 1},
            REFERENCE_PIXELS,
        ),
        "ycbcr_subsampling_long_type_lzw_2x2.tiff": (
            long_subsampling,
            (4, 4),
            REFERENCE_PIXEL_SHA256,
            {530: (2, 2), 531: 1},
            REFERENCE_PIXELS,
        ),
        "ycbcr_subsampling_short_count_lzw_2x2.tiff": (
            short_count_subsampling,
            (4, 4),
            REFERENCE_PIXEL_SHA256,
            {530: (2,), 531: 1},
            REFERENCE_PIXELS,
        ),
        "ycbcr_subsampling_unsupported_pair_lzw_2x2.tiff": (
            unsupported_pair_subsampling,
            (4, 4),
            None,
            {530: (3, 1), 531: 1},
            None,
        ),
        "ycbcr_subsampling_zero_pair_lzw_2x2.tiff": (
            zero_pair_subsampling,
            (4, 4),
            None,
            {530: (0, 0), 531: 1},
            None,
        ),
        "ycbcr_subsampling_signed_byte_type_lzw_1x2.tiff": (
            signed_byte_subsampling,
            (4, 4),
            "1ddadfe41e070962f7991dda8e8d26efc8c43a7352227b45154387a9991c5fb6",
            {530: (1, 2), 531: 1},
            None,
        ),
        "ycbcr_subsampling_signed_short_type_lzw_1x2.tiff": (
            signed_short_subsampling,
            (4, 4),
            "1ddadfe41e070962f7991dda8e8d26efc8c43a7352227b45154387a9991c5fb6",
            {530: (1, 2), 531: 1},
            None,
        ),
        "ycbcr_subsampling_signed_long_type_lzw_1x2.tiff": (
            signed_long_subsampling,
            (4, 4),
            "1ddadfe41e070962f7991dda8e8d26efc8c43a7352227b45154387a9991c5fb6",
            {530: (1, 2), 531: 1},
            None,
        ),
        "ycbcr_subsampling_signed_short_negative_lzw_2x2.tiff": (
            signed_short_negative_subsampling,
            (4, 4),
            REFERENCE_PIXEL_SHA256,
            {530: (-1, 2), 531: 1},
            REFERENCE_PIXELS,
        ),
        "ycbcr_subsampling_signed_short_negative_vertical_lzw_2x2.tiff": (
            signed_short_negative_vertical_subsampling,
            (4, 4),
            REFERENCE_PIXEL_SHA256,
            {530: (1, -2), 531: 1},
            REFERENCE_PIXELS,
        ),
        "ycbcr_subsampling_ascii_type_lzw_2x2.tiff": (
            ascii_subsampling,
            (4, 4),
            REFERENCE_PIXEL_SHA256,
            {530: ("11",), 531: 1},
            REFERENCE_PIXELS,
        ),
        "ycbcr_subsampling_short_count_three_lzw_2x2.tiff": (
            short_count_three_subsampling,
            (4, 4),
            REFERENCE_PIXEL_SHA256,
            {530: (1, 2, 2), 531: 1},
            REFERENCE_PIXELS,
        ),
        "ycbcr_subsampled_lzw_1x2.tiff": (
            one_by_two,
            (4, 4),
            "1ddadfe41e070962f7991dda8e8d26efc8c43a7352227b45154387a9991c5fb6",
            {530: (1, 2), 531: 1},
            bytes.fromhex(
                "6807006d2306714031765c5b8d2c0192482b9665569b8180"
                "a55244aa6e6eaf8b99b4a7c3ca7769cf9393d4b0bed9cce8"
            ),
        ),
        "ycbcr_subsampled_lzw_1x2_odd_height.tiff": (
            one_by_two_odd_height,
            (4, 3),
            "c71479e1cfff0dd7c76fbe5df2c61e80c75405df52c1aa2413e2f4d08a99f1fb",
            {530: (1, 2), 531: 1},
            bytes.fromhex(
                "6807006d2306714031765c5b8d2c0192482b9665569b8180"
                "a55244aa6e6eaf8b99b4a7c3"
            ),
        ),
        "ycbcr_custom_coefficients_lzw_1x2_odd_height.tiff": (
            custom_coefficients_one_by_two_odd_height,
            (4, 3),
            "b9c6456db4389b82894e338cbec478835eb4eab02f3664289164f74c4f3ecba7",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (1, 2),
                531: 1,
            },
            bytes.fromhex(
                "710e0073290476442f785f5b963300984e299b69549d8480"
                "ad5942af746db28f99b4aac4"
            ),
        ),
        "ycbcr_subsampled_lzw_2x2_odd_edge.tiff": (
            odd_edge,
            (3, 3),
            "027f5f35676c24314a2fefac2bb3fb6e8b06b144a96fd3f55b5c4b057473e673",
            {},
            None,
        ),
        "ycbcr_subsampled_lzw_2x2_two_strips.tiff": (
            two_strips,
            (4, 4),
            "7a2b02a725a8988a4e716873f80fe8290c26e274536c6716b4474c31b8379df4",
            {530: (2, 2), 531: 2},
            None,
        ),
        "ycbcr_custom_coefficients_lzw_2x2.tiff": (
            custom_coefficients,
            (4, 4),
            "fa0742fcea742c1e1ee51b4eedc289afe3dfe18e491ecb5cde9d669c2d97119c",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: 1,
            },
            bytes.fromhex(
                "002500002500001c00064e00388900065700388000064e000012003876300009"
                "296a9fbf387630064400386d8d063b5b"
            ),
        ),
        "ycbcr_zero_green_coefficient_lzw_2x2.tiff": (
            zero_green_coefficient,
            (4, 4),
            None,
            {529: (0.299, 0.0, 0.114), 530: (2, 2), 531: 1},
            None,
        ),
        "ycbcr_zero_red_coefficient_denominator_lzw_2x2.tiff": (
            zero_red_coefficient_denominator,
            (4, 4),
            "242cbd3dfa89e9b4db704510b08b023c859d94eb2cbb30a61283a2c7c562c745",
            {529: (float("nan"), 0.587, 0.114), 530: (2, 2), 531: 1},
            bytes.fromhex(
                "002c00002c00001b00004d002c9000005e002c7f00004d00000a002c6e32"
                "0000275e8ebd2c6e32003c002c5c8b002a59"
            ),
        ),
        "ycbcr_zero_green_coefficient_denominator_lzw_2x2.tiff": (
            zero_green_coefficient_denominator,
            (4, 4),
            None,
            {529: (0.299, float("nan"), 0.114), 530: (2, 2), 531: 1},
            None,
        ),
        "ycbcr_zero_blue_coefficient_denominator_lzw_2x2.tiff": (
            zero_blue_coefficient_denominator,
            (4, 4),
            "8daae45c1be1e6af6702226e09e3451f21beee4abe1c39b2e5bac7d97c9819ff",
            {529: (0.299, 0.587, float("nan")), 530: (2, 2), 531: 1},
            bytes.fromhex(
                "0014000014000014000b46003d78000b46003d78000b46000014003d78"
                "2c00142c6faac23d782c0b46003d78900b465e"
            ),
        ),
        "ycbcr_zero_reference_white_denominator_lzw_2x2.tiff": (
            zero_reference_white_denominator,
            (4, 4),
            "7d507c6654601cd33a509a65fe760da82cb0268e17edc9938430096342e783cf",
            {
                530: (2, 2),
                531: 1,
                532: (0.0, float("nan"), 128.0, 255.0, 128.0, 255.0),
            },
            bytes.fromhex(
                "004000004000002f00ffffffffffffffffffffffffffffff001e00ffffff"
                "000c27ffffffffffffffffffffffffffffff"
            ),
        ),
        "ycbcr_reference_limit_accepted_lzw_2x2.tiff": (
            accepted_reference_limit,
            (4, 4),
            "2e7fe6d7279561e878df78111c9267d4b2476a4235b9a259b657453a12bcedbb",
            {
                530: (2, 2),
                531: 1,
                532: (0.0, 2_147_483_583.0, 128.0, 255.0, 128.0, 255.0),
            },
            bytes.fromhex(
                "004000004000002f00002f00004000004000002f00002f00001e00001e"
                "00000c27000c27001e00001e00000c27000c27"
            ),
        ),
        "ycbcr_reference_limit_rejected_lzw_2x2.tiff": (
            rejected_reference_limit,
            (4, 4),
            None,
            {
                530: (2, 2),
                531: 1,
                532: (0.0, 2_147_483_584.0, 128.0, 255.0, 128.0, 255.0),
            },
            None,
        ),
        "ycbcr_coefficients_short_type_lzw_2x2.tiff": (
            coefficients_short_type,
            (4, 4),
            None,
            {
                529: (2126, 0, 10000),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            None,
        ),
        "ycbcr_coefficients_ascii_type_lzw_2x2.tiff": (
            coefficients_ascii_type,
            (4, 4),
            "20ef7069dad35f4368a43e8a17d5106a274997632f16de33917740f30355f392",
            {
                529: ("\xbc\0",),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            bytes.fromhex(
                "003600003600002200005b0036a900006f00369500005b00000f0036822a"
                "00001971aac736822a004800366f8c003552"
            ),
        ),
        "ycbcr_coefficients_undefined_type_lzw_2x2.tiff": (
            coefficients_undefined_type,
            (4, 4),
            "20ef7069dad35f4368a43e8a17d5106a274997632f16de33917740f30355f392",
            {
                529: (b"\xbc\0\0",),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            bytes.fromhex(
                "003600003600002200005b0036a900006f00369500005b00000f0036822a"
                "00001971aac736822a004800366f8c003552"
            ),
        ),
        "ycbcr_coefficients_long_type_lzw_2x2.tiff": (
            coefficients_long_type,
            (4, 4),
            "26a0bd17bf5db1e79f2318b475d0f50c41cea1bbead7a8a259005c80143a4258",
            {
                529: (2126, 10000, 7152),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            bytes.fromhex(
                "000000000000000000272727616161272727616161272727000000616161"
                "0000009c9c9c616161272727616161272727"
            ),
        ),
        "ycbcr_coefficients_srational_type_lzw_2x2.tiff": (
            coefficients_srational_type,
            (4, 4),
            "cc0b9abbf0f9741f8dd977611938d485be949664bb386c1b23a38abd1b52d66b",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            bytes.fromhex(
                "001800001800000d00004600308b00005100308000004600000200307527"
                "00001b6ba6c9307527003b00306b8e003154"
            ),
        ),
        "ycbcr_reference_short_type_lzw_2x2.tiff": (
            reference_short_type,
            (4, 4),
            "19d878fb6b31b50fd31b819d4dde624ead2dec8fe1a1f43c640e86851e78ea41",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: 1,
                532: (16, 0, 1, 0, 235, 0),
            },
            bytes.fromhex(
                "ffc6ffffc6ffffff0000c10000000000000000000000c100ffff00000000"
                "ffff0000000000000000c10000000000c100"
            ),
        ),
        "ycbcr_reference_ascii_type_lzw_2x2.tiff": (
            reference_ascii_type,
            (4, 4),
            "fa0742fcea742c1e1ee51b4eedc289afe3dfe18e491ecb5cde9d669c2d97119c",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: 1,
                532: ("\x10\0\0\0\x01",),
            },
            bytes.fromhex(
                "002500002500001c00064e00388900065700388000064e00001200387630"
                "0009296a9fbf387630064400386d8d063b5b"
            ),
        ),
        "ycbcr_reference_undefined_type_lzw_2x2.tiff": (
            reference_undefined_type,
            (4, 4),
            "fa0742fcea742c1e1ee51b4eedc289afe3dfe18e491ecb5cde9d669c2d97119c",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: 1,
                532: (b"\x10\0\0\0\x01\0",),
            },
            bytes.fromhex(
                "002500002500001c00064e00388900065700388000064e00001200387630"
                "0009296a9fbf387630064400386d8d063b5b"
            ),
        ),
        "ycbcr_reference_long_type_lzw_2x2.tiff": (
            reference_long_type,
            (4, 4),
            "d75efff0f5b11dc1218d2ce36bd4de44cca7fd9ce70182acb0d68399817b1de0",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: 1,
                532: (16, 1, 235, 1, 128, 1),
            },
            bytes.fromhex(
                "ffebffffebfffff0ff000000000000000000000000000000fff5ff000000"
                "fffaff000000000000000000000000000000"
            ),
        ),
        "ycbcr_reference_srational_type_lzw_2x2.tiff": (
            reference_srational_type,
            (4, 4),
            "cc0b9abbf0f9741f8dd977611938d485be949664bb386c1b23a38abd1b52d66b",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            bytes.fromhex(
                "001800001800000d00004600308b00005100308000004600000200307527"
                "00001b6ba6c9307527003b00306b8e003154"
            ),
        ),
        "ycbcr_coefficients_negative_srational_lzw_2x2.tiff": (
            negative_coefficients_srational,
            (4, 4),
            "ffdbff41c9e5ce395a6ec446662ac52de202b58f6d6f845fe2547feba54c5c0e",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            bytes.fromhex(
                "001e00001e00001200004b00239100005700238500004b00000700237a23"
                "00001e5eaacc237a23004000236f91003557"
            ),
        ),
        "ycbcr_reference_negative_srational_lzw_2x2.tiff": (
            negative_reference_srational,
            (4, 4),
            "9363c239bddbb3e8c174ff45e557466ff5e8b72964d93d5ac26424aa64c7a8ad",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            bytes.fromhex(
                "ff0000ff0000ff00ffff00ffff0000ff0000ff00ffff00ffff00ffff00"
                "ffff00ffff00ffff00ffff00ffff00ffff00ff"
            ),
        ),
        "ycbcr_coefficients_double_type_lzw_2x2.tiff": (
            coefficients_double_type,
            (4, 4),
            None,
            {
                529: (
                    2.1219957910703e-310,
                    2.12199579131863e-310,
                    2.12199579100094e-310,
                ),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            None,
        ),
        "ycbcr_coefficients_float_type_lzw_2x2.tiff": (
            coefficients_float_type,
            (4, 4),
            "fa0742fcea742c1e1ee51b4eedc289afe3dfe18e491ecb5cde9d669c2d97119c",
            {
                529: float32_values(float_coefficients),
                530: (2, 2),
                531: 1,
            },
            bytes.fromhex(
                "002500002500001c00064e00388900065700388000064e000012003876300009"
                "296a9fbf387630064400386d8d063b5b"
            ),
        ),
        "ycbcr_coefficients_double_values_lzw_2x2.tiff": (
            coefficients_double_values,
            (4, 4),
            "fa0742fcea742c1e1ee51b4eedc289afe3dfe18e491ecb5cde9d669c2d97119c",
            {529: float_coefficients, 530: (2, 2), 531: 1},
            bytes.fromhex(
                "002500002500001c00064e00388900065700388000064e000012003876300009"
                "296a9fbf387630064400386d8d063b5b"
            ),
        ),
        "ycbcr_coefficients_double_infinity_lzw_2x2.tiff": (
            coefficients_double_infinity,
            (4, 4),
            "dd618e329d17bac3db6f66f654b2b7a3a08fdd47255f8848e0f2081ac1e979cc",
            {529: (float("inf"), 0.7152, 0.0722), 530: (2, 2), 531: 1},
            bytes.fromhex(
                "001800001800000f00324100647c00324a00647300324100000500646930"
                "0000299692bf64693032370064608d322e5b"
            ),
        ),
        "ycbcr_reference_float_type_lzw_2x2.tiff": (
            reference_float_type,
            (4, 4),
            "20ef7069dad35f4368a43e8a17d5106a274997632f16de33917740f30355f392",
            {530: (2, 2), 531: 1, 532: float_reference_black_white},
            bytes.fromhex(
                "003600003600002200005b0036a900006f00369500005b00000f0036822a"
                "00001971aac736822a004800366f8c003552"
            ),
        ),
        "ycbcr_reference_double_type_lzw_2x2.tiff": (
            reference_double_type,
            (4, 4),
            "20ef7069dad35f4368a43e8a17d5106a274997632f16de33917740f30355f392",
            {530: (2, 2), 531: 1, 532: float_reference_black_white},
            bytes.fromhex(
                "003600003600002200005b0036a900006f00369500005b00000f0036822a"
                "00001971aac736822a004800366f8c003552"
            ),
        ),
        "ycbcr_reference_double_below_limit_lzw_2x2.tiff": (
            reference_double_below_limit,
            (4, 4),
            "092d2582801c2796d0e5bd3d0cad1ccf194f68cd71f2b75660ff8c7cb259941f",
            {
                530: (2, 2),
                531: 1,
                532: (16.0, 2_147_483_583.0, 128.0, 240.0, 128.0, 240.0),
            },
            bytes.fromhex(
                "004800004800003400003400004800004800003400003400002100002100"
                "000e2b000e2b002100002100000e2b000e2b"
            ),
        ),
        "ycbcr_reference_double_at_limit_lzw_2x2.tiff": (
            reference_double_at_limit,
            (4, 4),
            None,
            {
                530: (2, 2),
                531: 1,
                532: (16.0, 2_147_483_584.0, 128.0, 240.0, 128.0, 240.0),
            },
            None,
        ),
        "ycbcr_coefficients_float_nan_lzw_2x2.tiff": (
            coefficients_float_nan,
            (4, 4),
            None,
            {
                529: float32_values((float("nan"), 0.7152, 0.0722)),
                530: (2, 2),
                531: 1,
            },
            None,
        ),
        "ycbcr_coefficients_double_nan_lzw_2x2.tiff": (
            coefficients_double_nan,
            (4, 4),
            None,
            {
                529: (float("nan"), 0.7152, 0.0722),
                530: (2, 2),
                531: 1,
            },
            None,
        ),
        "ycbcr_reference_float_infinity_lzw_2x2.tiff": (
            reference_float_non_finite,
            (4, 4),
            None,
            {
                530: (2, 2),
                531: 1,
                532: (16.0, float("inf"), 128.0, 240.0, 128.0, 240.0),
            },
            None,
        ),
        "ycbcr_reference_double_infinity_lzw_2x2.tiff": (
            reference_double_non_finite,
            (4, 4),
            None,
            {
                530: (2, 2),
                531: 1,
                532: (16.0, float("inf"), 128.0, 240.0, 128.0, 240.0),
            },
            None,
        ),
        "ycbcr_coefficients_short_count_lzw_2x2.tiff": (
            coefficients_short_count,
            (4, 4),
            "20ef7069dad35f4368a43e8a17d5106a274997632f16de33917740f30355f392",
            {
                529: (0.2126, 0.7152),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            bytes.fromhex(
                "003600003600002200005b0036a900006f00369500005b00000f0036822a"
                "00001971aac736822a004800366f8c003552"
            ),
        ),
        "ycbcr_coefficients_long_count_lzw_2x2.tiff": (
            coefficients_long_count,
            (4, 4),
            "20ef7069dad35f4368a43e8a17d5106a274997632f16de33917740f30355f392",
            {
                529: (0.2126, 0.7152, 0.0722, 16.0),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            bytes.fromhex(
                "003600003600002200005b0036a900006f00369500005b00000f0036822a"
                "00001971aac736822a004800366f8c003552"
            ),
        ),
        "ycbcr_reference_short_count_lzw_2x2.tiff": (
            reference_short_count,
            (4, 4),
            "fa0742fcea742c1e1ee51b4eedc289afe3dfe18e491ecb5cde9d669c2d97119c",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0),
            },
            bytes.fromhex(
                "002500002500001c00064e00388900065700388000064e00001200387630"
                "0009296a9fbf387630064400386d8d063b5b"
            ),
        ),
        "ycbcr_reference_long_count_lzw_2x2.tiff": (
            reference_long_count,
            (4, 4),
            "fa0742fcea742c1e1ee51b4eedc289afe3dfe18e491ecb5cde9d669c2d97119c",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0, 0.02999970797919751),
            },
            bytes.fromhex(
                "002500002500001c00064e00388900065700388000064e00001200387630"
                "0009296a9fbf387630064400386d8d063b5b"
            ),
        ),
        "ycbcr_positioning_zero_value_lzw_2x2.tiff": (
            positioning_invalid_value,
            (4, 4),
            "cc0b9abbf0f9741f8dd977611938d485be949664bb386c1b23a38abd1b52d66b",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: 0,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            None,
        ),
        "ycbcr_positioning_long_type_lzw_2x2.tiff": (
            positioning_long_type,
            (4, 4),
            "cc0b9abbf0f9741f8dd977611938d485be949664bb386c1b23a38abd1b52d66b",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            None,
        ),
        "ycbcr_positioning_zero_count_lzw_2x2.tiff": (
            positioning_zero_count,
            (4, 4),
            "cc0b9abbf0f9741f8dd977611938d485be949664bb386c1b23a38abd1b52d66b",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: None,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            None,
        ),
        "ycbcr_custom_reference_black_white_lzw_2x2.tiff": (
            custom_reference_black_white,
            (4, 4),
            "20ef7069dad35f4368a43e8a17d5106a274997632f16de33917740f30355f392",
            {
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            bytes.fromhex(
                "003600003600002200005b0036a900006f00369500005b00000f0036822a"
                "00001971aac736822a004800366f8c003552"
            ),
        ),
        "ycbcr_custom_coefficients_and_reference_black_white_lzw_2x2.tiff": (
            custom_coefficients_and_reference_black_white,
            (4, 4),
            "cc0b9abbf0f9741f8dd977611938d485be949664bb386c1b23a38abd1b52d66b",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (2, 2),
                531: 1,
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            bytes.fromhex(
                "001800001800000d00004600308b00005100308000004600000200307527"
                "00001b6ba6c9307527003b00306b8e003154"
            ),
        ),
        "ycbcr_full_resolution_lzw_1x1.tiff": (
            full_resolution,
            (4, 4),
            "a9ef8ac10e78a041a3b442685d6a85cc153d136972866ebd10fb03804bd13a7e",
            {530: (1, 1)},
            None,
        ),
        "ycbcr_separate_full_resolution_lzw_1x1.tiff": (
            full_resolution_separate,
            (4, 4),
            "a9ef8ac10e78a041a3b442685d6a85cc153d136972866ebd10fb03804bd13a7e",
            {284: 2, 530: (1, 1), 531: 1},
            None,
        ),
        "ycbcr_custom_tags_full_resolution_lzw_1x1.tiff": (
            full_resolution_custom,
            (4, 4),
            "23e5d86280e8ff4053b81cea57ba76715e5b21fd4fc56ee029936ed889d7d711",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (1, 1),
                532: (16.0, 235.0, 128.0, 240.0, 128.0, 240.0),
            },
            bytes.fromhex(
                "4906006a360e8c6651af97936818008c4842ad7882cfa9c4882933aa5a73cd8b"
                "b4eebbf5a93c63cb6ca5ed9ce7ffccff"
            ),
        ),
        "ycbcr_subsampled_lzw_2x2_checkpoint_32x32.tiff": (
            checkpoint,
            (32, 32),
            "c5fc0bdafed814b597605fb7d00ce7510c60e1b45e774d0c71c4850ec39b4ded",
            {530: (2, 2), 531: 1},
            None,
        ),
        "ycbcr_subsampled_lzw_4x4_odd_edge.tiff": (
            odd_edge_4x4,
            (odd_edge_width, odd_edge_height),
            "18133f36a5dfa0595cf17ec4ceee5c79ef983e4b4d4f89456e76e0ba641bf177",
            {530: (4, 4), 531: 1},
            bytes.fromhex(
                "5b0000680b007518008225006413008f32009c3f00a94c00b6590b98"
                "4706c36618d07325dd8032ea8d3fcc7b3af79a4cffa759ffb466ffc"
                "173ffaf6e6b1f00782c00853907924614743410"
            ),
        ),
        "ycbcr_custom_coefficients_lzw_4x4_odd_edge.tiff": (
            custom_odd_edge_4x4,
            (odd_edge_width, odd_edge_height),
            "3a1f7bcac62ca5a45bef741bab0a4f2969a8f94d2cee233c8ef9ac23902a6506",
            {
                529: (0.2126, 0.7152, 0.0722),
                530: (4, 4),
                531: 1,
            },
            bytes.fromhex(
                "640200710f007e1c008b29006c1700983600a54300b25000bf5d06a04b"
                "03cc6a13d97720e6842df3913ad47f37ff9e47ffab54ffb861ffc56eff"
                "b36b7223007f30008c3d03994a107a380d"
            ),
        ),
        **factor_fixtures,
    }


def verify_pillow_reference(
    name: str,
    data: bytes,
    expected_size: tuple[int, int],
    expected_pixel_sha256: str | None,
    expected_tags: dict[int, object],
    expected_pixels: bytes | None,
) -> None:
    """Check the fixture's declared YCbCr storage and exact Pillow pixels."""
    from PIL import Image, __version__ as pillow_version

    if pillow_version != PINNED_PILLOW:
        raise RuntimeError(f"Pillow {PINNED_PILLOW} is required, found {pillow_version}")
    with Image.open(BytesIO(data)) as verification_image:
        verification_image.verify()
    with Image.open(BytesIO(data)) as image:
        if image.size != expected_size:
            raise RuntimeError(f"{name}: unexpected Pillow dimensions {image.size}")
        tags = image.tag_v2
        actual_tags = {
            259: tags.get(259, 1),
            262: tags.get(262, 1),
            277: tags.get(277, 1),
            284: tags.get(284, 1),
            530: tags.get(530),
            531: tags.get(531),
            529: tags.get(529),
            532: tags.get(532),
        }
        expected = {
            259: 5,
            262: 6,
            277: 3,
            284: expected_tags.get(284, 1),
            530: expected_tags.get(530),
            531: expected_tags.get(531),
            529: expected_tags.get(529),
            532: expected_tags.get(532),
        }
        def tag_value_matches(actual: object, expected_value: object) -> bool:
            if isinstance(expected_value, float) and isnan(expected_value):
                if isinstance(actual, (int, float)):
                    return isnan(float(actual))
                return getattr(actual, "denominator", None) == 0
            if isinstance(expected_value, tuple) and isinstance(actual, (tuple, list)):
                return len(actual) == len(expected_value) and all(
                    tag_value_matches(actual_value, expected_item)
                    for actual_value, expected_item in zip(actual, expected_value)
                )
            return actual == expected_value

        if any(
            not tag_value_matches(actual_tags[tag], expected_value)
            for tag, expected_value in expected.items()
        ):
            raise RuntimeError(f"{name}: TIFF tags differ: {actual_tags!r}")
        if expected_pixel_sha256 is None:
            try:
                image.load()
            except OSError as error:
                if type(error) is not OSError or str(error) != "decoder error -2":
                    raise RuntimeError(
                        f"{name}: unexpected Pillow load error "
                        f"({type(error).__name__}: {error})"
                    ) from error
            else:
                raise RuntimeError(f"{name}: Pillow unexpectedly decoded invalid coefficients")
            return

        image.load()
        pixels = image.tobytes()
        actual_pixel_sha256 = sha256(pixels).hexdigest()
        if (
            image.mode != "RGB"
            or actual_pixel_sha256 != expected_pixel_sha256
            or (expected_pixels is not None and pixels != expected_pixels)
        ):
            raise RuntimeError(
                f"{name}: Pillow RGB reference changed "
                f"(mode={image.mode}, sha256={actual_pixel_sha256})"
            )


def generate_fixtures(directory: Path = DEFAULT_DIRECTORY, *, check: bool = False) -> None:
    """Generate fixtures with Pillow or check pinned input bytes without it."""
    if not check:
        verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    inputs = fixtures()
    for name, (data, size, pixel_hash, tags, expected_pixels) in inputs.items():
        expected_input_hash = {
            "ycbcr_subsampled_lzw_2x2.tiff": "d9c3e4b7111658f3b86fdb1f94bf04e03aebe7cb4fbd01a98708ebfd27e65a6c",
            "ycbcr_equivalent_default_rational_lzw_2x2.tiff": "06a8a28ba0ebe1359d107ca2d7c442eb02dcc27cba8b85b06c16eaa610e7fa65",
            "ycbcr_subsampling_byte_type_lzw_2x2.tiff": "be68207f391e87874d5b40af16571c888d588424db45cc06e693387cf6ca89ab",
            "ycbcr_subsampling_long_type_lzw_2x2.tiff": "a3bc8d6110c272f34180e6e6231f88aa229ce5c87ee026464e6996a4523631cd",
            "ycbcr_subsampling_short_count_lzw_2x2.tiff": "f46aa199d9253f5dda799917e7223cab8005858df402ff8f5dc3916ecb254f5b",
            "ycbcr_subsampling_unsupported_pair_lzw_2x2.tiff": "afc8480db4452d713f4e5d10198c3bb0e35a981c9e8b2689456cff9eff5aeee8",
            "ycbcr_subsampling_zero_pair_lzw_2x2.tiff": "0c4d64f3d62266d6ba9fccbac32f4e2189991e8c211ac65998c2e33663a74fc8",
            "ycbcr_subsampling_signed_byte_type_lzw_1x2.tiff": "c080016f25b312985ac17df0c313806b6126c227ff6d2e56ea95994c5908dead",
            "ycbcr_subsampling_signed_short_type_lzw_1x2.tiff": "dfe209d2861f63f8b393079059aadf8d5b3840916d58d3200a56510d5acbdd3b",
            "ycbcr_subsampling_signed_long_type_lzw_1x2.tiff": "5fa78a9777e5e7e4b21f2d433352a66b7a3974d19aaa6a96af5a3ed01b99a08f",
            "ycbcr_subsampling_signed_short_negative_lzw_2x2.tiff": "d6cf543c98afe9fc083cd7d5fd3edfa4ebcc8eee212d4458e139187b1a63ad9a",
            "ycbcr_subsampling_signed_short_negative_vertical_lzw_2x2.tiff": "f5d7516b4d712a0b70df26f0bfc2db851beeeb0890c42b971073a358a30d155d",
            "ycbcr_subsampling_ascii_type_lzw_2x2.tiff": "dedbfec3b02eacedb00f936c11a2f00af510cd875b23b2b2b546db5225f8a808",
            "ycbcr_subsampling_short_count_three_lzw_2x2.tiff": "701521f18c666bc757bd68835e0bd6f90316ceb1e1b549a90ec63545fe3bfd26",
            "ycbcr_subsampled_lzw_1x2.tiff": "d659443b0f3eb3c6de349a45d3e007e482b6727c4682f82545df0ac5b49971c9",
            "ycbcr_subsampled_lzw_1x2_odd_height.tiff": "00013d30c2155603f592722acf82b52154eb031774453023612d93a8983e2185",
            "ycbcr_custom_coefficients_lzw_1x2_odd_height.tiff": "2555bdcefad89e0c7de7c7259f3f815a69981fbb21cc0457386bb4d2be05ca35",
            "ycbcr_subsampled_lzw_2x2_odd_edge.tiff": "67df0dce7b4cab6af8d43f258f7eb1b77ba119e096ad242083baec83ebd585e9",
            "ycbcr_subsampled_lzw_2x2_two_strips.tiff": "1dd542a0a6e22d922f66e0f19e92421dd15fd1108f726af38810a11a6bf13487",
            "ycbcr_custom_coefficients_lzw_2x2.tiff": "543fce746df657636828a74104092da002d1f3907d325b487945623d061f44b7",
            "ycbcr_zero_green_coefficient_lzw_2x2.tiff": "ff382d3f6afa3fb5f14fba10bb2df80d1aaa820e68f75883de097387bfa047d4",
            "ycbcr_zero_red_coefficient_denominator_lzw_2x2.tiff": "046eff0e1e9ededa25698a2e7e8b27f92685ff9b5daa474a1b20ef23d0466ea8",
            "ycbcr_zero_green_coefficient_denominator_lzw_2x2.tiff": "19e12a145a43997c05ff85632661514a88dfd2e96491b8895cd265fc8fc3ec3f",
            "ycbcr_zero_blue_coefficient_denominator_lzw_2x2.tiff": "ad9a10a013248f0d8369d7885da9711835853a3c0bee1ebc02e44db2cae798b3",
            "ycbcr_zero_reference_white_denominator_lzw_2x2.tiff": "b7e2979a563d4c2680da936e49f43d77b9685f89e7421f24c8ad0134c48b4d76",
            "ycbcr_reference_limit_accepted_lzw_2x2.tiff": "f6666b8bde65045252de34833702e767c42e88c9e8f1235ad22231529f611402",
            "ycbcr_reference_limit_rejected_lzw_2x2.tiff": "ffef9e28dfe7b6bc3d91b47993cd3696ecf6dcebc8a1391c0d7f650f4610f655",
            "ycbcr_coefficients_short_type_lzw_2x2.tiff": "f7438184eef0098cf58df7bd679613710738331ebfc4c0d57a7fe21eff680a41",
            "ycbcr_coefficients_ascii_type_lzw_2x2.tiff": "d36675d549f16bf745e88302f6cab7986a962180b38e54c13a5db2a9284aee2a",
            "ycbcr_coefficients_undefined_type_lzw_2x2.tiff": "11628766f85ea5fe0fa92471cea69c545fe9dc4808732939b527eef0cdb0c989",
            "ycbcr_coefficients_long_type_lzw_2x2.tiff": "dd66f2e9d3efd927ca21528770322f947eb47b55b9ccd48902449b4ad2570991",
            "ycbcr_coefficients_srational_type_lzw_2x2.tiff": "42a51687d68eb2abcdca68333afffc15028c8a076a42f9cbf392d9a0d0bf1923",
            "ycbcr_reference_short_type_lzw_2x2.tiff": "823d6faf5e938a69b22c7a9d4096cdfbe7fa2bc201ed80555105c92582cb0678",
            "ycbcr_reference_ascii_type_lzw_2x2.tiff": "5aa054faf7f5540b22c2cffeb0101556aa4f0711a8ff96d893d8953aeab4f68f",
            "ycbcr_reference_undefined_type_lzw_2x2.tiff": "ad28be2769b4c12802f4efe3f70e537827224bfe21c6bba924462504870f7768",
            "ycbcr_reference_long_type_lzw_2x2.tiff": "e09b7787f8ed2ee5b975633e23f1019c537a6fcd8815931cabec3edd00ffa5be",
            "ycbcr_reference_srational_type_lzw_2x2.tiff": "1eea7554aef894c4e2f470b7091047fb8aaaa6e10b2db11e7404e21b4b09cefb",
            "ycbcr_coefficients_negative_srational_lzw_2x2.tiff": "1f8ef67d9b888d251f862e9a9e94036c42e51188e3cc1befc201ea22882d6b49",
            "ycbcr_reference_negative_srational_lzw_2x2.tiff": "91501422161e6b325c724836acfb0dbeb45baf415af9420d5b007fe672f476fb",
            "ycbcr_coefficients_double_type_lzw_2x2.tiff": "146ece179798e6c975a3ceceef83d029c24d3608f244a5f3c9282e62fccdd318",
            "ycbcr_coefficients_float_type_lzw_2x2.tiff": "8013c341878b41988870bd9da47b30fd0cb8d9d37bec2d520796b86faf8bc8f9",
            "ycbcr_coefficients_double_values_lzw_2x2.tiff": "86021e46c9e268d38077bb0ce37dace13d792c73530195e40dffe31688f091b9",
            "ycbcr_coefficients_double_infinity_lzw_2x2.tiff": "798aa5745ea7146883324287482b5d01cb29940796075677d5103e435f5a4857",
            "ycbcr_reference_float_type_lzw_2x2.tiff": "4fedf6f18aba1a16e3b4be43b2ccdb4fd2e1ce560c11ee638f82d38dad5021c2",
            "ycbcr_reference_double_type_lzw_2x2.tiff": "44360eed61346ece234b4d905f43a0087ee2af40396fbcf092c006ea5ab9d7aa",
            "ycbcr_reference_double_below_limit_lzw_2x2.tiff": "cce30fcae9daeb4fda7d0d5f4e888798667e781c1175f2e0db6aa6a8836a40cd",
            "ycbcr_reference_double_at_limit_lzw_2x2.tiff": "1dbbe503d82d2ed6b2e8af411e3059ca5ff250644f5796e0ddd497b342e1b264",
            "ycbcr_coefficients_float_nan_lzw_2x2.tiff": "b5528738ef256e3b2bbbe7fee599132d0c351bc30e27e79983e4842d26ddb31b",
            "ycbcr_coefficients_double_nan_lzw_2x2.tiff": "94b0daf8bfefd5110240870628e67459b534217939026e91e6013ecab064aecb",
            "ycbcr_reference_float_infinity_lzw_2x2.tiff": "1e4bf6a92a8c6ecde82d1b7ac2c729ed10ba6043b9a3bbee2bd5fb4a5b61c2da",
            "ycbcr_reference_double_infinity_lzw_2x2.tiff": "9252570e68a6fe82e71ad202d45a3164b5d49543cc18d192e605d20ab00b63d2",
            "ycbcr_coefficients_short_count_lzw_2x2.tiff": "6bc1a1a005f0599bb4e9aab5895c9318df8687b755384134bd10df1352b0b7ed",
            "ycbcr_coefficients_long_count_lzw_2x2.tiff": "9b3d9fe558a5d730ee9f7d50020d7b69e193458148c10c676421215dca76de2d",
            "ycbcr_reference_short_count_lzw_2x2.tiff": "30721b73b9f1df772294e0ecc23ae05187ef1d49ad89ebe1b90ba7ecaf32dcba",
            "ycbcr_reference_long_count_lzw_2x2.tiff": "27b16ce9cfc5aa25ee36d7ada3344e3fec7cacb2d0cdc793f8d5cf12cea29fe4",
            "ycbcr_custom_reference_black_white_lzw_2x2.tiff": "fc1dd50550aa040f323c221e107fb8edd30171e7889e86a7ae8b7244c0361122",
            "ycbcr_custom_coefficients_and_reference_black_white_lzw_2x2.tiff": "c72ecf1c02ee1a18c9a7f41ae8917ce0668508de2555aee6172e486eb6103bb2",
            "ycbcr_full_resolution_lzw_1x1.tiff": "7a90e875697e13f98b23500bf9b4c9d5b83064b572daa102784bb84503cc167a",
            "ycbcr_separate_full_resolution_lzw_1x1.tiff": "49a0f6c148ab883d78a99fef5493eae9116a9e052113bf7b45409daa1d46c57c",
            "ycbcr_custom_tags_full_resolution_lzw_1x1.tiff": "ec170a210ddaf4c27f3cc69810a68ee666f57eb77001c860b8558583011e8248",
            "ycbcr_subsampled_lzw_2x2_checkpoint_32x32.tiff": "655b9b0172e44433b222cc93acdd2e14ca296e22f373e5f073e2e24d33b5111f",
            "ycbcr_subsampled_lzw_2x1.tiff": "abc595f9245db5d77d61f3ca19894bcba716db5e9841a258f08afcebec29fc35",
            "ycbcr_subsampled_lzw_4x1.tiff": "dda6b978a0f36882c6f380bb505f89d4801cacea4f298f23a3b27f570d06f657",
            "ycbcr_subsampled_lzw_4x2.tiff": "61de5caf036d78fdc559e1607b60590f1b2be45e7d887794768b1d45b6c8977f",
            "ycbcr_subsampled_lzw_4x4.tiff": "d9e6f0dc5d95750791aca652af5669e5c1f74bd597c778e791cd0fad5ceb8f13",
            "ycbcr_subsampled_lzw_4x4_odd_edge.tiff": "d77892013772aa86644a89d45471d05efab08b997517c93c575fdbb266dbe5b3",
            "ycbcr_custom_coefficients_lzw_4x2.tiff": "54fe5be273b743f33a510d177f3eade1d075281dd4d67714a0a2fd993aa024c6",
            "ycbcr_custom_coefficients_lzw_4x4_odd_edge.tiff": "425396287e96b1c7a710934e58a5cf1e9b5ac1530d3532ec56a84bbc13f42867",
            "ycbcr_positioning_zero_value_lzw_2x2.tiff": "126b986668c66582533c5d590c02579d076cf7a2b02905ad97488026279bc228",
            "ycbcr_positioning_long_type_lzw_2x2.tiff": "ff83ab481e010441fb73adac938b1632976a8083f18af792623a3495d54d8a00",
            "ycbcr_positioning_zero_count_lzw_2x2.tiff": "35cc88ce341e6e843891488c3f4d3eb0686ae0dbd1b430903df56c1d179f0562",
        }[name]
        actual_input_hash = sha256(data).hexdigest()
        if actual_input_hash != expected_input_hash:
            raise RuntimeError(
                f"{name}: input SHA-256 changed "
                f"(expected {expected_input_hash}, found {actual_input_hash})"
            )
        if not check:
            verify_pillow_reference(name, data, size, pixel_hash, tags, expected_pixels)
    if check:
        for name, (data, _, _, _, _) in inputs.items():
            destination = directory / name
            if not destination.is_file() or destination.read_bytes() != data:
                raise RuntimeError(f"{destination}: deterministic YCbCr fixture differs")
        return

    directory.mkdir(parents=True, exist_ok=True)
    for name, (data, _, _, _, _) in inputs.items():
        destination = directory / name
        destination.write_bytes(data)


def main() -> None:
    """Expose fixture generation and deterministic input verification."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-directory", type=Path, default=DEFAULT_DIRECTORY)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    generate_fixtures(arguments.output_directory, check=arguments.check)
    action = "Checked" if arguments.check else "Generated"
    print(f"{action} {len(fixtures())} TIFF YCbCr fixtures")


if __name__ == "__main__":
    main()
