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
DEFLATE_CONTIGUOUS_OUTPUT = (
    ROOT / "tests/fixtures/input/images/tiff/rgb16_le_deflate_contiguous_strips.tiff"
)
EXPECTED_SHA256 = {
    "little": "aebc7d664d848fe2d4e5054c4926ea6b38b99665d2d31bd6c86419aa07b7874b",
    "big": "9b91240255da2d849d12be62e59c3d868721112253ed46f1284578597567b0e3",
}
EXPECTED_ZERO_COUNT_SAMPLE_FORMAT_SHA256 = (
    "1787315279c7e9290fc895aa1eefeb6c73dc33e00199209648bf67285cd016a8"
)
EXPECTED_DEFLATE_CONTIGUOUS_SHA256 = (
    "b3a35c9135e45daf9016229f803eaade5923b506830b491cd001c9db1899e369"
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
PLANAR_TILED_WIDTH = 17
PLANAR_TILED_HEIGHT = 17
PLANAR_TILED_TILE_SIZE = 16
EXPECTED_PLANAR_TILED_SHA256 = (
    "0cee08795dbfcb70e86040432e8ead2e371a8bfa40675fe4a35fc0231f7d7e86"
)


def build_fixture(byte_order: str) -> bytes:
    """Build a classic TIFF with two contiguous, uncompressed RGB16 strips."""
    if byte_order not in {"little", "big"}:
        raise ValueError(f"unsupported byte order {byte_order!r}")
    endian = "<" if byte_order == "little" else ">"
    marker = b"II" if byte_order == "little" else b"MM"
    explicit_sample_format = byte_order == "big"

    def pack(code: str, *values: int) -> bytes:
        return struct.pack(endian + code, *values)

    ifd_offset = 8
    entry_count = 9 + int(explicit_sample_format)
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
    if digest != EXPECTED_SHA256[byte_order]:
        raise RuntimeError(
            f"{byte_order}-endian TIFF SHA-256 mismatch: "
            f"expected {EXPECTED_SHA256[byte_order]}, found {digest}"
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


def build_deflate_contiguous_fixture() -> bytes:
    """Build little-endian RGB16 with two independently Deflate-coded strips."""
    def pack(code: str, *values: int) -> bytes:
        return struct.pack("<" + code, *values)

    raw_strips = []
    for row in range(HEIGHT):
        start = row * SAMPLES_PER_STRIP
        samples = SAMPLES[start : start + SAMPLES_PER_STRIP]
        raw_strips.append(pack(f"{SAMPLES_PER_STRIP}H", *samples))
    compressed_strips = [zlib.compress(strip, level=9) for strip in raw_strips]

    ifd_offset = 8
    entry_count = 9
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
        entry(273, 4, 2, pack("I", strip_offsets_offset)),
        entry(277, 3, 1, pack("H", 3)),
        entry(278, 4, 1, pack("I", ROWS_PER_STRIP)),
        entry(279, 4, 2, pack("I", strip_byte_counts_offset)),
    ]
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
    if digest != EXPECTED_DEFLATE_CONTIGUOUS_SHA256:
        raise RuntimeError(
            "Deflate-contiguous TIFF SHA-256 mismatch: "
            f"expected {EXPECTED_DEFLATE_CONTIGUOUS_SHA256}, found {digest}"
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


def verify_deflate_contiguous_pillow_reference(encoded: bytes) -> None:
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
        if (
            tuple(tags.get(258, ())) != (16, 16, 16)
            or tags.get(259) != 8
            or tags.get(262) != 2
            or tuple(tags.get(273, ())) != (144, 164)
            or tags.get(277) != 3
            or tags.get(278) != ROWS_PER_STRIP
            or tuple(tags.get(279, ())) != (20, 20)
            or tags.get(284) is not None
            or tags.get(317) is not None
            or tags.get(339) is not None
        ):
            raise RuntimeError("Pillow TIFF tags differ from the Deflate-contiguous contract")
    with Image.open(BytesIO(encoded)) as image:
        image.verify()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify committed fixture bytes without rewriting them",
    )
    args = parser.parse_args()
    fixtures = {byte_order: build_fixture(byte_order) for byte_order in OUTPUTS}
    zero_count_sample_format = build_zero_count_sample_format_fixture()
    deflate_contiguous = build_deflate_contiguous_fixture()
    planar_tiled = build_planar_tiled_fixture()
    if args.check:
        for byte_order, output in OUTPUTS.items():
            if not output.is_file() or output.read_bytes() != fixtures[byte_order]:
                raise RuntimeError(f"{output.relative_to(ROOT)} differs from generated bytes")
            print(f"Fixture matches: {output.relative_to(ROOT)}")
        if (
            not ZERO_COUNT_SAMPLE_FORMAT_OUTPUT.is_file()
            or ZERO_COUNT_SAMPLE_FORMAT_OUTPUT.read_bytes() != zero_count_sample_format
        ):
            raise RuntimeError(
                f"{ZERO_COUNT_SAMPLE_FORMAT_OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {ZERO_COUNT_SAMPLE_FORMAT_OUTPUT.relative_to(ROOT)}")
        if (
            not PLANAR_TILED_OUTPUT.is_file()
            or PLANAR_TILED_OUTPUT.read_bytes() != planar_tiled
        ):
            raise RuntimeError(
                f"{PLANAR_TILED_OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {PLANAR_TILED_OUTPUT.relative_to(ROOT)}")
        if (
            not DEFLATE_CONTIGUOUS_OUTPUT.is_file()
            or DEFLATE_CONTIGUOUS_OUTPUT.read_bytes() != deflate_contiguous
        ):
            raise RuntimeError(
                f"{DEFLATE_CONTIGUOUS_OUTPUT.relative_to(ROOT)} differs from generated bytes"
            )
        print(f"Fixture matches: {DEFLATE_CONTIGUOUS_OUTPUT.relative_to(ROOT)}")
        return 0

    verify_pillow_reference(fixtures)
    verify_zero_count_sample_format_pillow_reference(zero_count_sample_format)
    verify_deflate_contiguous_pillow_reference(deflate_contiguous)
    verify_planar_tiled_pillow_reference(planar_tiled)
    for byte_order, output in OUTPUTS.items():
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(fixtures[byte_order])
        print(
            f"Wrote {output.relative_to(ROOT)} ({len(fixtures[byte_order])} bytes, "
            f"SHA-256 {hashlib.sha256(fixtures[byte_order]).hexdigest()})"
        )
    ZERO_COUNT_SAMPLE_FORMAT_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    ZERO_COUNT_SAMPLE_FORMAT_OUTPUT.write_bytes(zero_count_sample_format)
    print(
        f"Wrote {ZERO_COUNT_SAMPLE_FORMAT_OUTPUT.relative_to(ROOT)} "
        f"({len(zero_count_sample_format)} bytes, "
        f"SHA-256 {hashlib.sha256(zero_count_sample_format).hexdigest()})"
    )
    PLANAR_TILED_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    PLANAR_TILED_OUTPUT.write_bytes(planar_tiled)
    print(
        f"Wrote {PLANAR_TILED_OUTPUT.relative_to(ROOT)} ({len(planar_tiled)} bytes, "
        f"SHA-256 {hashlib.sha256(planar_tiled).hexdigest()})"
    )
    DEFLATE_CONTIGUOUS_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    DEFLATE_CONTIGUOUS_OUTPUT.write_bytes(deflate_contiguous)
    print(
        f"Wrote {DEFLATE_CONTIGUOUS_OUTPUT.relative_to(ROOT)} "
        f"({len(deflate_contiguous)} bytes, "
        f"SHA-256 {hashlib.sha256(deflate_contiguous).hexdigest()})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
