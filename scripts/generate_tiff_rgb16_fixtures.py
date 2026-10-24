#!/usr/bin/env python3
"""Generate and verify RGB16 TIFF fixtures for strips and tiled images."""

from __future__ import annotations

import argparse
import hashlib
import struct
import sys
import zlib
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUTS = {
    "little": ROOT / "tests/fixtures/input/images/tiff/rgb16_le_uncompressed_strips.tiff",
    "big": ROOT / "tests/fixtures/input/images/tiff/rgb16_be_uncompressed_strips.tiff",
}
ZERO_COUNT_SAMPLE_FORMAT_OUTPUT = (
    ROOT / "tests/fixtures/input/images/tiff/rgb16_be_zero_count_sample_format.tiff"
)
SIGNED_SAMPLE_FORMAT_OUTPUT = (
    ROOT / "tests/fixtures/input/images/tiff/rgb16_signed_sample_format_unsupported.tiff"
)
MIXED_SAMPLE_FORMAT_OUTPUT = (
    ROOT / "tests/fixtures/input/images/tiff/rgb16_mixed_sample_format_unsupported.tiff"
)
EXTRA_SAMPLES_OUTPUT = (
    ROOT / "tests/fixtures/input/images/tiff/rgb16_extra_sample_unsupported.tiff"
)
DEFLATE_CONTIGUOUS_OUTPUT = (
    ROOT / "tests/fixtures/input/images/tiff/rgb16_le_deflate_contiguous_strips.tiff"
)
DEFLATE_PREDICTOR2_OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/tiff/rgb16_le_deflate_predictor2_contiguous_strips.tiff"
)
FILL_ORDER2_OUTPUT = (
    ROOT / "tests/fixtures/input/images/tiff/rgb16_deflate_fill_order_2_unsupported.tiff"
)
UNCOMPRESSED_PREDICTOR2_OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/tiff/rgb16_le_uncompressed_predictor2_strips.tiff"
)
UNCOMPRESSED_PREDICTOR3_OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/tiff/rgb16_le_uncompressed_predictor3_strips.tiff"
)
EXPECTED_SHA256 = {
    "little": "aebc7d664d848fe2d4e5054c4926ea6b38b99665d2d31bd6c86419aa07b7874b",
    "big": "9b91240255da2d849d12be62e59c3d868721112253ed46f1284578597567b0e3",
}
EXPECTED_UNCOMPRESSED_PREDICTOR2_SHA256 = (
    "238b7058ca4ed7222ae9388482aaf39330181e190a3fbe6b80719e9ee48ff58a"
)
EXPECTED_UNCOMPRESSED_PREDICTOR3_SHA256 = (
    "1528a4d7a519f6a2f1b24c0f88cdee98b566d813533621cdf6971fd8f4aedec9"
)
EXPECTED_ZERO_COUNT_SAMPLE_FORMAT_SHA256 = (
    "1787315279c7e9290fc895aa1eefeb6c73dc33e00199209648bf67285cd016a8"
)
EXPECTED_SIGNED_SAMPLE_FORMAT_SHA256 = (
    "acf7ac21e19739826a6f4783d36150ff139dd70a632b6223f55cee23f3fac142"
)
EXPECTED_MIXED_SAMPLE_FORMAT_SHA256 = (
    "e6c535b10127e845fa911f375f7e88bfefa4a39a0dd9f1bff6dcfb95b3649b55"
)
EXPECTED_EXTRA_SAMPLES_SHA256 = (
    "086d099af708225e89e39ab59ab2e62576636da3e3083f3bb606bdffd4a2975a"
)
EXPECTED_DEFLATE_CONTIGUOUS_SHA256 = (
    "b3a35c9135e45daf9016229f803eaade5923b506830b491cd001c9db1899e369"
)
EXPECTED_DEFLATE_PREDICTOR2_SHA256 = (
    "37fc7bcedd0f550626fab36b6cfa772f925aa33e1b1bebb4c4f3e832fe4c978e"
)
EXPECTED_FILL_ORDER2_SHA256 = (
    "36a010a79dfeacec61408c2535b19eb7bdedbca460057604c407c2929d35dcbb"
)
SAMPLES = (
    0x1234,
    0x5678,
    0x9ABC,
    0xABCD,
    0x00FF,
    0xFF00,
    0x0080,
    0x80FF,
    0xFF01,
    0xFFFF,
    0x0100,
    0x7F80,
)
EXPECTED_PIXELS = bytes.fromhex("12569aab00ff0080ffff017f")
WIDTH = 2
HEIGHT = 2
ROWS_PER_STRIP = 1
SAMPLES_PER_STRIP = WIDTH * ROWS_PER_STRIP * 3
PLANAR_TILED_OUTPUT = (
    ROOT / "tests/fixtures/input/images/tiff/rgb16_separate_planar_tiled_deflate.tiff"
)
CONTIGUOUS_TILED_OUTPUT = (
    ROOT / "tests/fixtures/input/images/tiff/rgb16_contiguous_tiled_deflate_edges.tiff"
)
CONTIGUOUS_TILED_MISSING_COUNTS_OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/tiff/rgb16_contiguous_tiled_deflate_without_byte_counts.tiff"
)
PLANAR_TILED_WIDTH = 17
PLANAR_TILED_HEIGHT = 17
PLANAR_TILED_TILE_SIZE = 16
EXPECTED_PLANAR_TILED_SHA256 = (
    "0cee08795dbfcb70e86040432e8ead2e371a8bfa40675fe4a35fc0231f7d7e86"
)
EXPECTED_CONTIGUOUS_TILED_SHA256 = (
    "3b29709ca7a616ada00ca7ca800378fed328f7353d6574ecb8c30181145d2031"
)
EXPECTED_CONTIGUOUS_TILED_MISSING_COUNTS_SHA256 = (
    "ed0b0e058f8fb8a7113c498f8a7a063d7f2ff28f9e45aa4bb33b63019cf86a7f"
)


def build_fixture(byte_order: str, *, predictor: int = 1) -> bytes:
    """Build a classic TIFF with two contiguous, uncompressed RGB16 strips."""
    if byte_order not in {"little", "big"}:
        raise ValueError(f"unsupported byte order {byte_order!r}")
    if predictor not in (1, 2, 3):
        raise ValueError(f"unsupported TIFF predictor {predictor}")
    if predictor != 1 and byte_order != "little":
        raise ValueError("only little-endian uncompressed predictor fixtures are pinned")
    endian = "<" if byte_order == "little" else ">"
    marker = b"II" if byte_order == "little" else b"MM"
    explicit_sample_format = byte_order == "big"

    def pack(code: str, *values: int) -> bytes:
        return struct.pack(endian + code, *values)

    ifd_offset = 8
    entry_count = 9 + int(explicit_sample_format) + int(predictor != 1)
    ifd_end = ifd_offset + 2 + entry_count * 12 + 4
    bits_offset = ifd_end
    sample_formats_offset = bits_offset + 3 * 2
    strip_offsets_offset = sample_formats_offset + (3 * 2 if explicit_sample_format else 0)
    strip_offsets_offset = (strip_offsets_offset + 3) & ~3
    strip_byte_counts_offset = strip_offsets_offset + 2 * 4
    first_strip_offset = strip_byte_counts_offset + 2 * 4
    strip_byte_count = SAMPLES_PER_STRIP * 2
    second_strip_offset = first_strip_offset + strip_byte_count

    def entry(tag: int, field_type: int, count: int, value: bytes) -> bytes:
        if len(value) > 4:
            raise ValueError(f"tag {tag} does not fit its inline TIFF field")
        return pack("HHI", tag, field_type, count) + value.ljust(4, b"\x00")

    entries = [
        entry(256, 4, 1, pack("I", WIDTH)),
        entry(257, 4, 1, pack("I", HEIGHT)),
        entry(258, 3, 3, pack("I", bits_offset)),
        entry(259, 3, 1, pack("H", 1)),
        entry(262, 3, 1, pack("H", 2)),
        entry(273, 4, 2, pack("I", strip_offsets_offset)),
        entry(277, 3, 1, pack("H", 3)),
        entry(278, 4, 1, pack("I", ROWS_PER_STRIP)),
        entry(279, 4, 2, pack("I", strip_byte_counts_offset)),
    ]
    if predictor != 1:
        entries.append(entry(317, 3, 1, pack("H", predictor)))
    if explicit_sample_format:
        entries.append(entry(339, 3, 3, pack("I", sample_formats_offset)))
    directory = pack("H", entry_count) + b"".join(entries) + pack("I", 0)
    extra_values = pack("HHH", 16, 16, 16)
    if explicit_sample_format:
        extra_values += pack("HHH", 1, 1, 1)
    extra_values += bytes(strip_offsets_offset - bits_offset - len(extra_values))
    strip_arrays = pack(
        "II", first_strip_offset, second_strip_offset
    ) + pack("II", strip_byte_count, strip_byte_count)
    raster = pack("12H", *SAMPLES)

    encoded = marker + pack("HI", 42, ifd_offset) + directory + extra_values + strip_arrays + raster
    expected_length = second_strip_offset + strip_byte_count
    if len(encoded) != expected_length:
        raise RuntimeError(
            f"generated TIFF is {len(encoded)} bytes; expected {expected_length}"
        )
    digest = hashlib.sha256(encoded).hexdigest()
    expected_digest = {
        1: EXPECTED_SHA256[byte_order],
        2: EXPECTED_UNCOMPRESSED_PREDICTOR2_SHA256,
        3: EXPECTED_UNCOMPRESSED_PREDICTOR3_SHA256,
    }[predictor]
    if digest != expected_digest:
        raise RuntimeError(
            f"{byte_order}-endian TIFF predictor {predictor} SHA-256 mismatch: "
            f"expected {expected_digest}, found {digest}"
        )
    return encoded


def build_zero_count_sample_format_fixture() -> bytes:
    """Build big-endian RGB16 with a present, zero-count SampleFormat tag."""
    encoded = bytearray(build_fixture("big"))
    sample_format_entry_offset = 8 + 2 + 9 * 12
    tag, field_type, count = struct.unpack_from(">HHI", encoded, sample_format_entry_offset)
    if (tag, field_type, count) != (339, 3, 3):
        raise RuntimeError("big-endian RGB16 fixture no longer has the expected SampleFormat entry")

    # A zero count makes the inline value field irrelevant; clear it as well to
    # keep the mutated fixture deterministic and avoid a dangling array offset.
    encoded[sample_format_entry_offset + 4 : sample_format_entry_offset + 12] = bytes(8)
    digest = hashlib.sha256(encoded).hexdigest()
    if digest != EXPECTED_ZERO_COUNT_SAMPLE_FORMAT_SHA256:
        raise RuntimeError(
            "zero-count SampleFormat TIFF SHA-256 mismatch: "
            f"expected {EXPECTED_ZERO_COUNT_SAMPLE_FORMAT_SHA256}, found {digest}"
        )
    return bytes(encoded)


def build_sample_format_fixture(
    sample_formats: tuple[int, int, int], expected_digest: str, label: str
) -> bytes:
    """Build big-endian RGB16 with pinned per-channel SampleFormat values."""
    encoded = bytearray(build_fixture("big"))
    sample_format_entry_offset = 8 + 2 + 9 * 12
    tag, field_type, count = struct.unpack_from(">HHI", encoded, sample_format_entry_offset)
    if (tag, field_type, count) != (339, 3, 3):
        raise RuntimeError("big-endian RGB16 fixture lacks its expected SampleFormat entry")
    sample_formats_offset = struct.unpack_from(">I", encoded, sample_format_entry_offset + 8)[0]
    struct.pack_into(">HHH", encoded, sample_formats_offset, *sample_formats)

    digest = hashlib.sha256(encoded).hexdigest()
    if digest != expected_digest:
        raise RuntimeError(
            f"{label} SampleFormat TIFF SHA-256 mismatch: "
            f"expected {expected_digest}, found {digest}"
        )
    return bytes(encoded)


def build_signed_sample_format_fixture() -> bytes:
    """Build RGB16 with signed samples, which Pillow rejects during opening."""
    return build_sample_format_fixture(
        (2, 2, 2), EXPECTED_SIGNED_SAMPLE_FORMAT_SHA256, "signed"
    )


def build_mixed_sample_format_fixture() -> bytes:
    """Build RGB16 with mixed signed and unsigned channels."""
    return build_sample_format_fixture(
        (1, 2, 1), EXPECTED_MIXED_SAMPLE_FORMAT_SHA256, "mixed"
    )


def build_extra_samples_fixture() -> bytes:
    """Build RGB16 that declares an extra sample without a fourth channel."""
    encoded = bytearray(build_fixture("big"))
    extra_samples_entry_offset = 8 + 2 + 9 * 12
    tag, field_type, count = struct.unpack_from(">HHI", encoded, extra_samples_entry_offset)
    if (tag, field_type, count) != (339, 3, 3):
        raise RuntimeError("big-endian RGB16 fixture lacks its expected SampleFormat entry")
    struct.pack_into(">HHI", encoded, extra_samples_entry_offset, 338, 3, 1)
    struct.pack_into(">HHH", encoded, extra_samples_entry_offset + 8, 0, 0, 0)

    digest = hashlib.sha256(encoded).hexdigest()
    if digest != EXPECTED_EXTRA_SAMPLES_SHA256:
        raise RuntimeError(
            "RGB16 ExtraSamples TIFF SHA-256 mismatch: "
            f"expected {EXPECTED_EXTRA_SAMPLES_SHA256}, found {digest}"
        )
    return bytes(encoded)


def build_deflate_contiguous_fixture(
    *, predictor: int = 1, fill_order: int = 1
) -> bytes:
    """Build little-endian RGB16 with two independently Deflate-coded strips."""
    if predictor not in (1, 2):
        raise ValueError(f"unsupported TIFF horizontal predictor {predictor}")
    if fill_order not in (1, 2):
        raise ValueError(f"unsupported TIFF fill order {fill_order}")
    if predictor == 2 and fill_order == 2:
        raise ValueError("combined Predictor 2 and FillOrder 2 fixture is not pinned")

    def pack(code: str, *values: int) -> bytes:
        return struct.pack("<" + code, *values)

    raw_strips = []
    for row in range(HEIGHT):
        start = row * SAMPLES_PER_STRIP
        samples = SAMPLES[start : start + SAMPLES_PER_STRIP]
        if predictor == 2:
            residuals = list(samples)
            for index in range(3, len(samples)):
                residuals[index] = (samples[index] - samples[index - 3]) & 0xFFFF
            samples = residuals
        raw_strips.append(pack(f"{SAMPLES_PER_STRIP}H", *samples))
    compressed_strips = [zlib.compress(strip, level=9) for strip in raw_strips]

    ifd_offset = 8
    entry_count = 9 + int(predictor == 2) + int(fill_order == 2)
    ifd_end = ifd_offset + 2 + entry_count * 12 + 4
    bits_offset = ifd_end
    strip_offsets_offset = (bits_offset + 3 * 2 + 3) & ~3
    strip_byte_counts_offset = strip_offsets_offset + 2 * 4
    first_strip_offset = strip_byte_counts_offset + 2 * 4
    strip_offsets = (
        first_strip_offset,
        first_strip_offset + len(compressed_strips[0]),
    )

    def entry(tag: int, field_type: int, count: int, value: bytes) -> bytes:
        if len(value) > 4:
            raise ValueError(f"tag {tag} does not fit its inline TIFF field")
        return pack("HHI", tag, field_type, count) + value.ljust(4, b"\x00")

    entries = [
        entry(256, 4, 1, pack("I", WIDTH)),
        entry(257, 4, 1, pack("I", HEIGHT)),
        entry(258, 3, 3, pack("I", bits_offset)),
        entry(259, 3, 1, pack("H", 8)),
        entry(262, 3, 1, pack("H", 2)),
    ]
    if fill_order == 2:
        entries.append(entry(266, 3, 1, pack("H", fill_order)))
    entries.extend(
        [
            entry(273, 4, 2, pack("I", strip_offsets_offset)),
            entry(277, 3, 1, pack("H", 3)),
            entry(278, 4, 1, pack("I", ROWS_PER_STRIP)),
            entry(279, 4, 2, pack("I", strip_byte_counts_offset)),
        ]
    )
    if predictor == 2:
        entries.append(entry(317, 3, 1, pack("H", predictor)))
    directory = pack("H", entry_count) + b"".join(entries) + pack("I", 0)
    extra_values = pack("HHH", 16, 16, 16)
    strip_arrays = pack("II", *strip_offsets) + pack(
        "II", *(len(strip) for strip in compressed_strips)
    )
    encoded = bytearray(b"II*\x00" + pack("I", ifd_offset) + directory + extra_values)
    encoded.extend(b"\x00" * (strip_offsets_offset - len(encoded)))
    encoded.extend(strip_arrays)
    encoded.extend(b"".join(compressed_strips))

    expected_length = strip_offsets[-1] + len(compressed_strips[-1])
    if len(encoded) != expected_length:
        raise RuntimeError(
            f"Deflate-contiguous TIFF is {len(encoded)} bytes; expected {expected_length}"
        )
    digest = hashlib.sha256(encoded).hexdigest()
    expected_digest = {
        (1, 1): EXPECTED_DEFLATE_CONTIGUOUS_SHA256,
        (2, 1): EXPECTED_DEFLATE_PREDICTOR2_SHA256,
        (1, 2): EXPECTED_FILL_ORDER2_SHA256,
    }[(predictor, fill_order)]
    if digest != expected_digest:
        raise RuntimeError(
            f"Deflate-contiguous TIFF predictor {predictor}, FillOrder {fill_order} "
            "SHA-256 mismatch: "
            f"expected {expected_digest}, found {digest}"
        )
    return bytes(encoded)


def build_planar_tiled_fixture() -> bytes:
    """Build an edge-clipped RGB16 TIFF with Deflate tiles in separate planes."""
    width = PLANAR_TILED_WIDTH
    height = PLANAR_TILED_HEIGHT
    tile_size = PLANAR_TILED_TILE_SIZE
    endian = "<"

    def pack(code: str, *values: int) -> bytes:
        return struct.pack(endian + code, *values)

    def sample(x: int, y: int, channel: int) -> int:
        high_bytes = (x + 1, y + 33, x + y + 65)
        low_byte = (x * 13 + y * 17 + channel * 29 + 1) & 0xFF
        return (high_bytes[channel] << 8) | low_byte

    compressed_tiles = []
    tiles_across = (width + tile_size - 1) // tile_size
    tiles_down = (height + tile_size - 1) // tile_size
    for channel in range(3):
        for tile_y in range(tiles_down):
            for tile_x in range(tiles_across):
                tile_samples = []
                for local_y in range(tile_size):
                    y = tile_y * tile_size + local_y
                    for local_x in range(tile_size):
                        x = tile_x * tile_size + local_x
                        tile_samples.append(
                            sample(x, y, channel) if x < width and y < height else 0
                        )
                raw_tile = pack(f"{len(tile_samples)}H", *tile_samples)
                compressed_tiles.append(zlib.compress(raw_tile, level=9))

    entry_count = 13
    ifd_offset = 8
    ifd_end = ifd_offset + 2 + entry_count * 12 + 4
    bits_offset = ifd_end
    sample_format_offset = bits_offset + 3 * 2
    tile_offsets_offset = (sample_format_offset + 3 * 2 + 3) & ~3
    tile_byte_counts_offset = tile_offsets_offset + len(compressed_tiles) * 4
    tile_data_offset = tile_byte_counts_offset + len(compressed_tiles) * 4

    tile_offsets = []
    cursor = tile_data_offset
    for compressed_tile in compressed_tiles:
        tile_offsets.append(cursor)
        cursor += len(compressed_tile)

    def entry(tag: int, field_type: int, count: int, value: bytes) -> bytes:
        if len(value) > 4:
            raise ValueError(f"tag {tag} does not fit its inline TIFF field")
        return pack("HHI", tag, field_type, count) + value.ljust(4, b"\x00")

    entries = [
        entry(256, 4, 1, pack("I", width)),
        entry(257, 4, 1, pack("I", height)),
        entry(258, 3, 3, pack("I", bits_offset)),
        entry(259, 3, 1, pack("H", 8)),
        entry(262, 3, 1, pack("H", 2)),
        entry(277, 3, 1, pack("H", 3)),
        entry(284, 3, 1, pack("H", 2)),
        entry(317, 3, 1, pack("H", 1)),
        entry(322, 4, 1, pack("I", tile_size)),
        entry(323, 4, 1, pack("I", tile_size)),
        entry(324, 4, len(tile_offsets), pack("I", tile_offsets_offset)),
        entry(325, 4, len(compressed_tiles), pack("I", tile_byte_counts_offset)),
        entry(339, 3, 3, pack("I", sample_format_offset)),
    ]
    directory = pack("H", entry_count) + b"".join(entries) + pack("I", 0)
    encoded = bytearray(b"II*\x00" + pack("I", ifd_offset) + directory)
    encoded.extend(pack("HHH", 16, 16, 16))
    encoded.extend(pack("HHH", 1, 1, 1))
    encoded.extend(b"\x00" * (tile_offsets_offset - len(encoded)))
    encoded.extend(pack(f"{len(tile_offsets)}I", *tile_offsets))
    encoded.extend(pack(f"{len(compressed_tiles)}I", *(len(tile) for tile in compressed_tiles)))
    for compressed_tile in compressed_tiles:
        encoded.extend(compressed_tile)

    expected_length = cursor
    if len(encoded) != expected_length:
        raise RuntimeError(
            f"generated TIFF is {len(encoded)} bytes; expected {expected_length}"
        )
    digest = hashlib.sha256(encoded).hexdigest()
    if digest != EXPECTED_PLANAR_TILED_SHA256:
        raise RuntimeError(
            "separate-planar tiled TIFF SHA-256 mismatch: "
            f"expected {EXPECTED_PLANAR_TILED_SHA256}, found {digest}"
        )
    return bytes(encoded)


def build_contiguous_tiled_fixture() -> bytes:
    """Build edge-clipped RGB16 Deflate tiles with contiguous samples."""
    width = PLANAR_TILED_WIDTH
    height = PLANAR_TILED_HEIGHT
    tile_size = PLANAR_TILED_TILE_SIZE
    tile_count_x = (width + tile_size - 1) // tile_size
    tile_count_y = (height + tile_size - 1) // tile_size

    def pack(code: str, *values: int) -> bytes:
        return struct.pack("<" + code, *values)

    def sample(x: int, y: int, channel: int) -> int:
        high_bytes = (x + 1, y + 33, x + y + 65)
        low_byte = (x * 13 + y * 17 + channel * 29 + 1) & 0xFF
        return (high_bytes[channel] << 8) | low_byte

    compressed_tiles = []
    for tile_y in range(tile_count_y):
        for tile_x in range(tile_count_x):
            tile_samples = []
            for local_y in range(tile_size):
                y = tile_y * tile_size + local_y
                for local_x in range(tile_size):
                    x = tile_x * tile_size + local_x
                    for channel in range(3):
                        tile_samples.append(
                            sample(x, y, channel) if x < width and y < height else 0
                        )
            raw_tile = pack(f"{len(tile_samples)}H", *tile_samples)
            compressed_tiles.append(zlib.compress(raw_tile, level=9))

    entry_count = 13
    ifd_offset = 8
    ifd_end = ifd_offset + 2 + entry_count * 12 + 4
    bits_offset = ifd_end
    sample_format_offset = bits_offset + 3 * 2
    tile_offsets_offset = (sample_format_offset + 3 * 2 + 3) & ~3
    tile_byte_counts_offset = tile_offsets_offset + len(compressed_tiles) * 4
    tile_data_offset = tile_byte_counts_offset + len(compressed_tiles) * 4

    tile_offsets = []
    cursor = tile_data_offset
    for compressed_tile in compressed_tiles:
        tile_offsets.append(cursor)
        cursor += len(compressed_tile)

    def entry(tag: int, field_type: int, count: int, value: bytes) -> bytes:
        if len(value) > 4:
            raise ValueError(f"tag {tag} does not fit its inline TIFF field")
        return pack("HHI", tag, field_type, count) + value.ljust(4, b"\x00")

    entries = [
        entry(256, 4, 1, pack("I", width)),
        entry(257, 4, 1, pack("I", height)),
        entry(258, 3, 3, pack("I", bits_offset)),
        entry(259, 3, 1, pack("H", 8)),
        entry(262, 3, 1, pack("H", 2)),
        entry(277, 3, 1, pack("H", 3)),
        entry(284, 3, 1, pack("H", 1)),
        entry(317, 3, 1, pack("H", 1)),
        entry(322, 4, 1, pack("I", tile_size)),
        entry(323, 4, 1, pack("I", tile_size)),
        entry(324, 4, len(tile_offsets), pack("I", tile_offsets_offset)),
        entry(325, 4, len(compressed_tiles), pack("I", tile_byte_counts_offset)),
        entry(339, 3, 3, pack("I", sample_format_offset)),
    ]
    directory = pack("H", entry_count) + b"".join(entries) + pack("I", 0)
    encoded = bytearray(b"II*\x00" + pack("I", ifd_offset) + directory)
    encoded.extend(pack("HHH", 16, 16, 16))
    encoded.extend(pack("HHH", 1, 1, 1))
    encoded.extend(b"\x00" * (tile_offsets_offset - len(encoded)))
    encoded.extend(pack(f"{len(tile_offsets)}I", *tile_offsets))
    encoded.extend(pack(f"{len(compressed_tiles)}I", *(len(tile) for tile in compressed_tiles)))
    for compressed_tile in compressed_tiles:
        encoded.extend(compressed_tile)

    if len(encoded) != cursor:
        raise RuntimeError(f"generated TIFF is {len(encoded)} bytes; expected {cursor}")
    digest = hashlib.sha256(encoded).hexdigest()
    if digest != EXPECTED_CONTIGUOUS_TILED_SHA256:
        raise RuntimeError(
            "contiguous-tiled TIFF SHA-256 mismatch: "
            f"expected {EXPECTED_CONTIGUOUS_TILED_SHA256}, found {digest}"
        )
    return bytes(encoded)


def build_contiguous_tiled_missing_byte_counts_fixture() -> bytes:
    """Remove TileByteCounts from the pinned RGB16 Deflate tile fixture."""
    encoded = build_contiguous_tiled_fixture()
    ifd_offset = struct.unpack_from("<I", encoded, 4)[0]
    entry_count = struct.unpack_from("<H", encoded, ifd_offset)[0]
    entries = []
    removed_count = 0
    for index in range(entry_count):
        entry_start = ifd_offset + 2 + 12 * index
        entry = encoded[entry_start : entry_start + 12]
        if struct.unpack_from("<H", entry)[0] == 325:
            removed_count += 1
        else:
            entries.append(entry)
    if removed_count != 1:
        raise RuntimeError(f"expected one TileByteCounts entry, found {removed_count}")

    result = bytearray(encoded)
    struct.pack_into("<H", result, ifd_offset, len(entries))
    for index, entry in enumerate(entries):
        entry_start = ifd_offset + 2 + 12 * index
        result[entry_start : entry_start + 12] = entry
    old_next_offset = struct.unpack_from("<I", encoded, ifd_offset + 2 + 12 * entry_count)[0]
    struct.pack_into("<I", result, ifd_offset + 2 + 12 * len(entries), old_next_offset)

    digest = hashlib.sha256(result).hexdigest()
    if digest != EXPECTED_CONTIGUOUS_TILED_MISSING_COUNTS_SHA256:
        raise RuntimeError(
            "contiguous-tiled missing-count TIFF SHA-256 mismatch: "
            f"expected {EXPECTED_CONTIGUOUS_TILED_MISSING_COUNTS_SHA256}, found {digest}"
        )
    return bytes(result)


def verify_pillow_reference(fixtures: dict[str, bytes]) -> None:
    """Check exact public pixels and storage tags against the pinned Pillow oracle."""
    import yaml
    from PIL import Image

    sys.path.insert(0, str(ROOT / "scripts"))
    from generate_decode_refs import verify_primary_oracle

    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    for byte_order, encoded in fixtures.items():
        with Image.open(BytesIO(encoded)) as image:
            if (image.format, image.mode, image.size) != ("TIFF", "RGB", (WIDTH, HEIGHT)):
                raise RuntimeError(
                    f"{byte_order}-endian TIFF opened with unexpected Pillow metadata"
                )
            image.load()
            if image.tobytes() != EXPECTED_PIXELS:
                raise RuntimeError(
                    f"{byte_order}-endian TIFF pixels differ from the exact oracle bytes"
                )
            if (
                tuple(image.tag_v2.get(258, ())) != (16, 16, 16)
                or tuple(image.tag_v2.get(339, ()))
                != ((1, 1, 1) if byte_order == "big" else ())
                or tuple(image.tag_v2.get(273, ()))
                != (
                    (164, 164 + SAMPLES_PER_STRIP * 2)
                    if byte_order == "big"
                    else (144, 144 + SAMPLES_PER_STRIP * 2)
                )
                or tuple(image.tag_v2.get(279, ()))
                != (strip_count := SAMPLES_PER_STRIP * 2, strip_count)
                or image.tag_v2.get(278) != ROWS_PER_STRIP
            ):
                raise RuntimeError(
                    f"{byte_order}-endian TIFF strip tags differ from the fixture contract"
                )
        with Image.open(BytesIO(encoded)) as image:
            image.verify()


def verify_uncompressed_predictor_pillow_reference(encoded: bytes, predictor: int) -> None:
    """Check Pillow preserves RGB16 pixels for a tagged uncompressed predictor."""
    import yaml
    from PIL import Image

    sys.path.insert(0, str(ROOT / "scripts"))
    from generate_decode_refs import verify_primary_oracle

    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    with Image.open(BytesIO(encoded)) as image:
        if (image.format, image.mode, image.size) != ("TIFF", "RGB", (WIDTH, HEIGHT)):
            raise RuntimeError(
                f"Pillow opened the uncompressed Predictor {predictor} TIFF with unexpected metadata"
            )
        image.load()
        if image.tobytes() != EXPECTED_PIXELS:
            raise RuntimeError(
                f"Pillow uncompressed Predictor {predictor} TIFF pixels differ from expected RGB bytes"
            )
        tags = image.tag_v2
        if (
            tuple(tags.get(258, ())) != (16, 16, 16)
            or tags.get(259) != 1
            or tuple(tags.get(273, ())) != (156, 168)
            or tags.get(277) != 3
            or tags.get(278) != ROWS_PER_STRIP
            or tuple(tags.get(279, ())) != (12, 12)
            or tags.get(284) is not None
            or tags.get(317) != predictor
            or tags.get(339) is not None
        ):
            raise RuntimeError(
                f"Pillow TIFF tags differ from the uncompressed Predictor {predictor} contract"
            )
    with Image.open(BytesIO(encoded)) as image:
        image.verify()


def verify_zero_count_sample_format_pillow_reference(encoded: bytes) -> None:
    """Check Pillow's absent-tag interpretation and exact RGB16 output."""
    import yaml
    from PIL import Image

    sys.path.insert(0, str(ROOT / "scripts"))
    from generate_decode_refs import verify_primary_oracle

    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    with Image.open(BytesIO(encoded)) as image:
        if (image.format, image.mode, image.size) != ("TIFF", "RGB", (WIDTH, HEIGHT)):
            raise RuntimeError("Pillow opened zero-count SampleFormat TIFF with unexpected metadata")
        image.load()
        if image.tobytes() != EXPECTED_PIXELS:
            raise RuntimeError("Pillow zero-count SampleFormat TIFF pixels differ from expected RGB bytes")
        if 339 in image.tag_v2:
            raise RuntimeError("Pillow retained the zero-count SampleFormat tag")
    with Image.open(BytesIO(encoded)) as image:
        image.verify()


def verify_pillow_rejects_on_open(encoded: bytes, description: str) -> None:
    """Check Pillow rejects a fixture while opening it."""
    import yaml
    from PIL import Image, UnidentifiedImageError

    sys.path.insert(0, str(ROOT / "scripts"))
    from generate_decode_refs import verify_primary_oracle

    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    try:
        with Image.open(BytesIO(encoded)):
            pass
    except UnidentifiedImageError:
        return
    except Exception as error:
        raise RuntimeError(f"Pillow returned an unexpected error for {description}") from error
    raise RuntimeError(f"Pillow unexpectedly accepted {description}")


def verify_planar_tiled_pillow_reference(encoded: bytes) -> None:
    """Check the pinned Pillow pixels and TIFF layout for the tiled fixture."""
    import yaml
    from PIL import Image

    sys.path.insert(0, str(ROOT / "scripts"))
    from generate_decode_refs import verify_primary_oracle

    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    expected_pixels = bytes(
        value
        for y in range(PLANAR_TILED_HEIGHT)
        for x in range(PLANAR_TILED_WIDTH)
        for value in (x + 1, y + 33, x + y + 65)
    )
    with Image.open(BytesIO(encoded)) as image:
        if (
            image.format,
            image.mode,
            image.size,
        ) != ("TIFF", "RGB", (PLANAR_TILED_WIDTH, PLANAR_TILED_HEIGHT)):
            raise RuntimeError("Pillow opened the separate-planar TIFF with unexpected metadata")
        image.load()
        if image.tobytes() != expected_pixels:
            raise RuntimeError("Pillow separate-planar TIFF pixels differ from expected RGB bytes")
        tags = image.tag_v2
        if (
            tuple(tags.get(258, ())) != (16, 16, 16)
            or tags.get(259) != 8
            or tags.get(284) != 2
            or tags.get(317) != 1
            or tags.get(322) != PLANAR_TILED_TILE_SIZE
            or tags.get(323) != PLANAR_TILED_TILE_SIZE
            or tuple(tags.get(339, ())) != (1, 1, 1)
            or len(tuple(tags.get(324, ()))) != 12
            or len(tuple(tags.get(325, ()))) != 12
        ):
            raise RuntimeError("Pillow TIFF tags differ from the separate-planar tile contract")
    with Image.open(BytesIO(encoded)) as image:
        image.verify()


def verify_contiguous_tiled_pillow_reference(encoded: bytes) -> None:
    """Check exact Pillow output for contiguous RGB16 Deflate tiles."""
    import yaml
    from PIL import Image

    sys.path.insert(0, str(ROOT / "scripts"))
    from generate_decode_refs import verify_primary_oracle

    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    expected_pixels = bytes(
        value
        for y in range(PLANAR_TILED_HEIGHT)
        for x in range(PLANAR_TILED_WIDTH)
        for value in (x + 1, y + 33, x + y + 65)
    )
    with Image.open(BytesIO(encoded)) as image:
        if (
            image.format,
            image.mode,
            image.size,
        ) != ("TIFF", "RGB", (PLANAR_TILED_WIDTH, PLANAR_TILED_HEIGHT)):
            raise RuntimeError("Pillow opened the contiguous-tiled TIFF with unexpected metadata")
        image.load()
        if image.tobytes() != expected_pixels:
            raise RuntimeError("Pillow contiguous-tiled RGB16 pixels differ from expected bytes")
        tags = image.tag_v2
        if (
            tuple(tags.get(258, ())) != (16, 16, 16)
            or tags.get(259) != 8
            or tags.get(277) != 3
            or tags.get(284) != 1
            or tags.get(317) != 1
            or tags.get(322) != PLANAR_TILED_TILE_SIZE
            or tags.get(323) != PLANAR_TILED_TILE_SIZE
            or tuple(tags.get(339, ())) != (1, 1, 1)
            or len(tuple(tags.get(324, ()))) != 4
            or len(tuple(tags.get(325, ()))) != 4
        ):
            raise RuntimeError("Pillow TIFF tags differ from the contiguous-tiled RGB16 contract")
    with Image.open(BytesIO(encoded)) as image:
        image.verify()


def verify_contiguous_tiled_missing_counts_pillow_reference(encoded: bytes) -> None:
    """Check Pillow opens/verifies the directory but rejects decoding missing counts."""
    import yaml
    from PIL import Image

    sys.path.insert(0, str(ROOT / "scripts"))
    from generate_decode_refs import verify_primary_oracle

    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    with Image.open(BytesIO(encoded)) as image:
        if (
            image.format,
            image.mode,
            image.size,
            image.tag_v2.get(259),
            image.tag_v2.get(324) is not None,
            image.tag_v2.get(325),
        ) != ("TIFF", "RGB", (PLANAR_TILED_WIDTH, PLANAR_TILED_HEIGHT), 8, True, None):
            raise RuntimeError("Pillow opened the missing-count TIFF with unexpected metadata")
        image.verify()
    try:
        with Image.open(BytesIO(encoded)) as image:
            image.load()
    except OSError as error:
        if str(error) != "decoder error -2":
            raise RuntimeError("Pillow returned an unexpected missing-count tile error") from error
    else:
        raise RuntimeError("Pillow unexpectedly decoded compressed tiles without byte counts")


def verify_deflate_contiguous_pillow_reference(encoded: bytes, predictor: int = 1) -> None:
    """Check pinned Pillow pixels and tags for contiguous compressed RGB16 strips."""
    import yaml
    from PIL import Image

    sys.path.insert(0, str(ROOT / "scripts"))
    from generate_decode_refs import verify_primary_oracle

    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    with Image.open(BytesIO(encoded)) as image:
        if (image.format, image.mode, image.size) != ("TIFF", "RGB", (WIDTH, HEIGHT)):
            raise RuntimeError("Pillow opened the Deflate-contiguous TIFF with unexpected metadata")
        image.load()
        if image.tobytes() != EXPECTED_PIXELS:
            raise RuntimeError("Pillow Deflate-contiguous TIFF pixels differ from expected RGB bytes")
        tags = image.tag_v2
        offsets = tuple(tags.get(273, ()))
        byte_counts = tuple(tags.get(279, ()))
        if (
            tuple(tags.get(258, ())) != (16, 16, 16)
            or tags.get(259) != 8
            or tags.get(262) != 2
            or tags.get(277) != 3
            or tags.get(278) != ROWS_PER_STRIP
            or len(offsets) != 2
            or len(byte_counts) != 2
            or tags.get(284) is not None
            or tags.get(317) != (None if predictor == 1 else predictor)
            or tags.get(339) is not None
        ):
            raise RuntimeError("Pillow TIFF tags differ from the Deflate-contiguous contract")
        if predictor == 1 and (offsets != (144, 164) or byte_counts != (20, 20)):
            raise RuntimeError("Pillow TIFF strip tags differ from the predictor-1 contract")
    with Image.open(BytesIO(encoded)) as image:
        image.verify()


def verify_fill_order2_pillow_reference(encoded: bytes) -> None:
    """Check Pillow rejects FillOrder 2 for this complete compressed RGB16 TIFF."""
    import yaml
    from PIL import Image, UnidentifiedImageError

    sys.path.insert(0, str(ROOT / "scripts"))
    from generate_decode_refs import verify_primary_oracle

    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    try:
        with Image.open(BytesIO(encoded)) as image:
            if image.format != "TIFF" or image.tag_v2.get(266) != 2:
                raise RuntimeError("Pillow opened FillOrder 2 TIFF with unexpected metadata")
    except UnidentifiedImageError:
        return
    except Exception as error:
        raise RuntimeError("Pillow returned an unexpected FillOrder 2 TIFF error") from error
    raise RuntimeError("Pillow unexpectedly accepted RGB16 Deflate TIFF with FillOrder 2")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify committed fixture bytes without rewriting them",
    )
    args = parser.parse_args()
    fixtures = {byte_order: build_fixture(byte_order) for byte_order in OUTPUTS}
    uncompressed_predictor2 = build_fixture("little", predictor=2)
    uncompressed_predictor3 = build_fixture("little", predictor=3)
    zero_count_sample_format = build_zero_count_sample_format_fixture()
    signed_sample_format = build_signed_sample_format_fixture()
    mixed_sample_format = build_mixed_sample_format_fixture()
    extra_samples = build_extra_samples_fixture()
    deflate_contiguous = build_deflate_contiguous_fixture()
    deflate_predictor2 = build_deflate_contiguous_fixture(predictor=2)
    fill_order2 = build_deflate_contiguous_fixture(fill_order=2)
    planar_tiled = build_planar_tiled_fixture()
    contiguous_tiled = build_contiguous_tiled_fixture()
    contiguous_tiled_missing_counts = build_contiguous_tiled_missing_byte_counts_fixture()
    if args.check:
        for byte_order, output in OUTPUTS.items():
            if not output.is_file() or output.read_bytes() != fixtures[byte_order]:
                raise RuntimeError(f"{output.relative_to(ROOT)} differs from generated bytes")
            print(f"Fixture matches: {output.relative_to(ROOT)}")
        if (
            not UNCOMPRESSED_PREDICTOR2_OUTPUT.is_file()
            or UNCOMPRESSED_PREDICTOR2_OUTPUT.read_bytes() != uncompressed_predictor2
        ):
            raise RuntimeError(
                f"{UNCOMPRESSED_PREDICTOR2_OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {UNCOMPRESSED_PREDICTOR2_OUTPUT.relative_to(ROOT)}")
        if (
            not UNCOMPRESSED_PREDICTOR3_OUTPUT.is_file()
            or UNCOMPRESSED_PREDICTOR3_OUTPUT.read_bytes() != uncompressed_predictor3
        ):
            raise RuntimeError(
                f"{UNCOMPRESSED_PREDICTOR3_OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {UNCOMPRESSED_PREDICTOR3_OUTPUT.relative_to(ROOT)}")
        if (
            not ZERO_COUNT_SAMPLE_FORMAT_OUTPUT.is_file()
            or ZERO_COUNT_SAMPLE_FORMAT_OUTPUT.read_bytes() != zero_count_sample_format
        ):
            raise RuntimeError(
                f"{ZERO_COUNT_SAMPLE_FORMAT_OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {ZERO_COUNT_SAMPLE_FORMAT_OUTPUT.relative_to(ROOT)}")
        if (
            not SIGNED_SAMPLE_FORMAT_OUTPUT.is_file()
            or SIGNED_SAMPLE_FORMAT_OUTPUT.read_bytes() != signed_sample_format
        ):
            raise RuntimeError(
                f"{SIGNED_SAMPLE_FORMAT_OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {SIGNED_SAMPLE_FORMAT_OUTPUT.relative_to(ROOT)}")
        if (
            not MIXED_SAMPLE_FORMAT_OUTPUT.is_file()
            or MIXED_SAMPLE_FORMAT_OUTPUT.read_bytes() != mixed_sample_format
        ):
            raise RuntimeError(
                f"{MIXED_SAMPLE_FORMAT_OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {MIXED_SAMPLE_FORMAT_OUTPUT.relative_to(ROOT)}")
        if (
            not EXTRA_SAMPLES_OUTPUT.is_file()
            or EXTRA_SAMPLES_OUTPUT.read_bytes() != extra_samples
        ):
            raise RuntimeError(
                f"{EXTRA_SAMPLES_OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {EXTRA_SAMPLES_OUTPUT.relative_to(ROOT)}")
        if (
            not PLANAR_TILED_OUTPUT.is_file()
            or PLANAR_TILED_OUTPUT.read_bytes() != planar_tiled
        ):
            raise RuntimeError(
                f"{PLANAR_TILED_OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {PLANAR_TILED_OUTPUT.relative_to(ROOT)}")
        if (
            not CONTIGUOUS_TILED_OUTPUT.is_file()
            or CONTIGUOUS_TILED_OUTPUT.read_bytes() != contiguous_tiled
        ):
            raise RuntimeError(
                f"{CONTIGUOUS_TILED_OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {CONTIGUOUS_TILED_OUTPUT.relative_to(ROOT)}")
        if (
            not CONTIGUOUS_TILED_MISSING_COUNTS_OUTPUT.is_file()
            or CONTIGUOUS_TILED_MISSING_COUNTS_OUTPUT.read_bytes()
            != contiguous_tiled_missing_counts
        ):
            raise RuntimeError(
                f"{CONTIGUOUS_TILED_MISSING_COUNTS_OUTPUT.relative_to(ROOT)} "
                "differs from generated bytes"
            )
        print(
            "Fixture matches: "
            f"{CONTIGUOUS_TILED_MISSING_COUNTS_OUTPUT.relative_to(ROOT)}"
        )
        if (
            not DEFLATE_CONTIGUOUS_OUTPUT.is_file()
            or DEFLATE_CONTIGUOUS_OUTPUT.read_bytes() != deflate_contiguous
        ):
            raise RuntimeError(
                f"{DEFLATE_CONTIGUOUS_OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {DEFLATE_CONTIGUOUS_OUTPUT.relative_to(ROOT)}")
        if (
            not DEFLATE_PREDICTOR2_OUTPUT.is_file()
            or DEFLATE_PREDICTOR2_OUTPUT.read_bytes() != deflate_predictor2
        ):
            raise RuntimeError(
                f"{DEFLATE_PREDICTOR2_OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {DEFLATE_PREDICTOR2_OUTPUT.relative_to(ROOT)}")
        if not FILL_ORDER2_OUTPUT.is_file() or FILL_ORDER2_OUTPUT.read_bytes() != fill_order2:
            raise RuntimeError(
                f"{FILL_ORDER2_OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {FILL_ORDER2_OUTPUT.relative_to(ROOT)}")
        return 0

    verify_pillow_reference(fixtures)
    verify_uncompressed_predictor_pillow_reference(uncompressed_predictor2, predictor=2)
    verify_uncompressed_predictor_pillow_reference(uncompressed_predictor3, predictor=3)
    verify_zero_count_sample_format_pillow_reference(zero_count_sample_format)
    verify_pillow_rejects_on_open(signed_sample_format, "signed RGB16 SampleFormat values")
    verify_pillow_rejects_on_open(mixed_sample_format, "mixed RGB16 SampleFormat values")
    verify_pillow_rejects_on_open(
        extra_samples, "an RGB16 TIFF declaring an extra sample without a fourth channel"
    )
    verify_deflate_contiguous_pillow_reference(deflate_contiguous)
    verify_deflate_contiguous_pillow_reference(deflate_predictor2, predictor=2)
    verify_fill_order2_pillow_reference(fill_order2)
    verify_planar_tiled_pillow_reference(planar_tiled)
    verify_contiguous_tiled_pillow_reference(contiguous_tiled)
    verify_contiguous_tiled_missing_counts_pillow_reference(contiguous_tiled_missing_counts)
    for byte_order, output in OUTPUTS.items():
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(fixtures[byte_order])
        print(
            f"Wrote {output.relative_to(ROOT)} ({len(fixtures[byte_order])} bytes, "
            f"SHA-256 {hashlib.sha256(fixtures[byte_order]).hexdigest()})"
        )
    UNCOMPRESSED_PREDICTOR2_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    UNCOMPRESSED_PREDICTOR2_OUTPUT.write_bytes(uncompressed_predictor2)
    print(
        f"Wrote {UNCOMPRESSED_PREDICTOR2_OUTPUT.relative_to(ROOT)} "
        f"({len(uncompressed_predictor2)} bytes, "
        f"SHA-256 {hashlib.sha256(uncompressed_predictor2).hexdigest()})"
    )
    UNCOMPRESSED_PREDICTOR3_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    UNCOMPRESSED_PREDICTOR3_OUTPUT.write_bytes(uncompressed_predictor3)
    print(
        f"Wrote {UNCOMPRESSED_PREDICTOR3_OUTPUT.relative_to(ROOT)} "
        f"({len(uncompressed_predictor3)} bytes, "
        f"SHA-256 {hashlib.sha256(uncompressed_predictor3).hexdigest()})"
    )
    ZERO_COUNT_SAMPLE_FORMAT_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    ZERO_COUNT_SAMPLE_FORMAT_OUTPUT.write_bytes(zero_count_sample_format)
    print(
        f"Wrote {ZERO_COUNT_SAMPLE_FORMAT_OUTPUT.relative_to(ROOT)} "
        f"({len(zero_count_sample_format)} bytes, "
        f"SHA-256 {hashlib.sha256(zero_count_sample_format).hexdigest()})"
    )
    SIGNED_SAMPLE_FORMAT_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    SIGNED_SAMPLE_FORMAT_OUTPUT.write_bytes(signed_sample_format)
    print(
        f"Wrote {SIGNED_SAMPLE_FORMAT_OUTPUT.relative_to(ROOT)} "
        f"({len(signed_sample_format)} bytes, "
        f"SHA-256 {hashlib.sha256(signed_sample_format).hexdigest()})"
    )
    MIXED_SAMPLE_FORMAT_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    MIXED_SAMPLE_FORMAT_OUTPUT.write_bytes(mixed_sample_format)
    print(
        f"Wrote {MIXED_SAMPLE_FORMAT_OUTPUT.relative_to(ROOT)} "
        f"({len(mixed_sample_format)} bytes, "
        f"SHA-256 {hashlib.sha256(mixed_sample_format).hexdigest()})"
    )
    EXTRA_SAMPLES_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    EXTRA_SAMPLES_OUTPUT.write_bytes(extra_samples)
    print(
        f"Wrote {EXTRA_SAMPLES_OUTPUT.relative_to(ROOT)} "
        f"({len(extra_samples)} bytes, SHA-256 {hashlib.sha256(extra_samples).hexdigest()})"
    )
    PLANAR_TILED_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    PLANAR_TILED_OUTPUT.write_bytes(planar_tiled)
    print(
        f"Wrote {PLANAR_TILED_OUTPUT.relative_to(ROOT)} ({len(planar_tiled)} bytes, "
        f"SHA-256 {hashlib.sha256(planar_tiled).hexdigest()})"
    )
    CONTIGUOUS_TILED_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    CONTIGUOUS_TILED_OUTPUT.write_bytes(contiguous_tiled)
    print(
        f"Wrote {CONTIGUOUS_TILED_OUTPUT.relative_to(ROOT)} "
        f"({len(contiguous_tiled)} bytes, "
        f"SHA-256 {hashlib.sha256(contiguous_tiled).hexdigest()})"
    )
    CONTIGUOUS_TILED_MISSING_COUNTS_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    CONTIGUOUS_TILED_MISSING_COUNTS_OUTPUT.write_bytes(contiguous_tiled_missing_counts)
    print(
        f"Wrote {CONTIGUOUS_TILED_MISSING_COUNTS_OUTPUT.relative_to(ROOT)} "
        f"({len(contiguous_tiled_missing_counts)} bytes, "
        f"SHA-256 {hashlib.sha256(contiguous_tiled_missing_counts).hexdigest()})"
    )
    DEFLATE_CONTIGUOUS_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    DEFLATE_CONTIGUOUS_OUTPUT.write_bytes(deflate_contiguous)
    print(
        f"Wrote {DEFLATE_CONTIGUOUS_OUTPUT.relative_to(ROOT)} "
        f"({len(deflate_contiguous)} bytes, "
        f"SHA-256 {hashlib.sha256(deflate_contiguous).hexdigest()})"
    )
    DEFLATE_PREDICTOR2_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    DEFLATE_PREDICTOR2_OUTPUT.write_bytes(deflate_predictor2)
    print(
        f"Wrote {DEFLATE_PREDICTOR2_OUTPUT.relative_to(ROOT)} "
        f"({len(deflate_predictor2)} bytes, "
        f"SHA-256 {hashlib.sha256(deflate_predictor2).hexdigest()})"
    )
    FILL_ORDER2_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    FILL_ORDER2_OUTPUT.write_bytes(fill_order2)
    print(
        f"Wrote {FILL_ORDER2_OUTPUT.relative_to(ROOT)} "
        f"({len(fill_order2)} bytes, SHA-256 {hashlib.sha256(fill_order2).hexdigest()})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
