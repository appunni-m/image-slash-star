#!/usr/bin/env python3
"""Mutate later-page TIFF BitsPerSample fields for sequence parity cases."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path
import struct


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tests/fixtures/input/images/tiff/multipage.tiff"
OUTPUTS = {
    "later_ifd_mixed_bits.tiff": "mixed",
    "later_ifd_empty_bits.tiff": "empty",
}
EXPECTED_SOURCE_SHA256 = (
    "6a7559bf68841a5f5b9f98f19e64205b2033784be3c92a765c224a91da2b81de"
)
EXPECTED_OUTPUT_SHA256 = {
    "later_ifd_mixed_bits.tiff": "e403d7283ffbce1891e93f79000ab41ed71a2f6cd2912302b5d0f47c8b8ef649",
    "later_ifd_empty_bits.tiff": "41e519c3bdaed68e0a4cea7a800a0969d0841560c042f15805d1625c29270be4",
}
ENTRY_SIZE = 12
EXPECTED_PILLOW_ERROR = ("builtins.SyntaxError", "unknown pixel mode")


def digest(data: bytes) -> str:
    return sha256(data).hexdigest()


def parse_directories(data: bytes) -> tuple[str, list[tuple[int, int]]]:
    """Return byte order and complete classic-TIFF IFD extents."""
    if len(data) < 8 or data[:2] not in (b"II", b"MM"):
        raise RuntimeError("source TIFF must have a complete classic header")
    endian = "<" if data[:2] == b"II" else ">"
    if struct.unpack_from(endian + "H", data, 2)[0] != 42:
        raise RuntimeError("source TIFF must use classic TIFF magic")

    offset = struct.unpack_from(endian + "I", data, 4)[0]
    directories = []
    seen = set()
    while offset:
        if offset in seen or offset < 8 or offset + 2 > len(data):
            raise RuntimeError("source TIFF has a cyclic or out-of-bounds IFD")
        seen.add(offset)
        count = struct.unpack_from(endian + "H", data, offset)[0]
        next_offset_pos = offset + 2 + count * ENTRY_SIZE
        if next_offset_pos + 4 > len(data):
            raise RuntimeError("source TIFF has a truncated IFD")
        directories.append((offset, count))
        offset = struct.unpack_from(endian + "I", data, next_offset_pos)[0]

    if len(directories) != 2:
        raise RuntimeError(f"source TIFF has {len(directories)} pages, expected two")
    return endian, directories


def bits_per_sample_entry(
    data: bytes | bytearray, endian: str, directory: tuple[int, int]
) -> tuple[int, int, int]:
    """Return the BitsPerSample entry offset, value count, and value offset."""
    directory_offset, entry_count = directory
    for index in range(entry_count):
        entry_offset = directory_offset + 2 + index * ENTRY_SIZE
        tag, field_type, value_count = struct.unpack_from(
            endian + "HHI", data, entry_offset
        )
        if tag != 258:
            continue
        if field_type != 3:
            raise RuntimeError("source BitsPerSample field must contain SHORT values")
        value_offset = (
            entry_offset + 8
            if value_count * 2 <= 4
            else struct.unpack_from(endian + "I", data, entry_offset + 8)[0]
        )
        return entry_offset, value_count, value_offset
    raise RuntimeError("source IFD has no BitsPerSample field")


def generate_fixtures() -> dict[str, bytes]:
    source = SOURCE.read_bytes()
    source_sha256 = digest(source)
    if source_sha256 != EXPECTED_SOURCE_SHA256:
        raise RuntimeError(
            f"source TIFF SHA-256 mismatch: expected {EXPECTED_SOURCE_SHA256}, "
            f"found {source_sha256}"
        )

    endian, directories = parse_directories(source)
    _, first_count, first_values = bits_per_sample_entry(source, endian, directories[0])
    if first_count != 3 or struct.unpack_from(endian + "3H", source, first_values) != (
        8,
        8,
        8,
    ):
        raise RuntimeError("source first page must retain three 8-bit RGB samples")

    second_entry, second_count, second_values = bits_per_sample_entry(
        source, endian, directories[1]
    )
    if second_count != 3 or struct.unpack_from(endian + "3H", source, second_values) != (
        8,
        8,
        8,
    ):
        raise RuntimeError("source second page must retain three 8-bit RGB samples")

    generated = {}
    for name, variant in OUTPUTS.items():
        encoded = bytearray(source)
        if variant == "mixed":
            struct.pack_into(endian + "3H", encoded, second_values, 8, 16, 8)
        else:
            struct.pack_into(endian + "I", encoded, second_entry + 4, 0)
            struct.pack_into(endian + "I", encoded, second_entry + 8, 0)
        output = bytes(encoded)
        output_sha256 = digest(output)
        if output_sha256 != EXPECTED_OUTPUT_SHA256[name]:
            raise RuntimeError(
                f"{name} SHA-256 mismatch: expected {EXPECTED_OUTPUT_SHA256[name]}, "
                f"found {output_sha256}"
            )
        generated[name] = output
    return generated


def verify_pillow_reference(fixtures: dict[str, bytes]) -> None:
    """Pin page-one pixels and later-page enumeration errors to Pillow 12.2.0."""
    import PIL
    from PIL import Image

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PIL.__version__}")

    with Image.open(SOURCE) as source_image:
        if source_image.format != "TIFF" or source_image.n_frames != 2:
            raise RuntimeError("source TIFF no longer has two Pillow-readable pages")
        source_image.load()
        source_mode = source_image.mode
        source_size = source_image.size
        source_pixels = source_image.tobytes()

    if source_mode != "RGB" or source_size != (9, 7):
        raise RuntimeError("source TIFF first-page mode or dimensions changed")

    for name, encoded in fixtures.items():
        with Image.open(BytesIO(encoded)) as image:
            if (
                image.format != "TIFF"
                or image.mode != source_mode
                or image.size != source_size
            ):
                raise RuntimeError(f"{name} changed Pillow's first-page metadata")
            image.load()
            if image.tobytes() != source_pixels:
                raise RuntimeError(f"{name} changed Pillow's first-page pixels")
            try:
                image.n_frames
            except Exception as error:
                observed = (
                    f"{type(error).__module__}.{type(error).__name__}",
                    str(error),
                )
                if observed != EXPECTED_PILLOW_ERROR:
                    raise RuntimeError(
                        f"{name} changed Pillow's sequence error: {observed!r}"
                    ) from error
            else:
                raise RuntimeError(f"{name} unexpectedly enumerates every Pillow page")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify committed fixtures without rewriting them",
    )
    args = parser.parse_args()

    fixtures = generate_fixtures()
    if args.check:
        for name, encoded in fixtures.items():
            path = SOURCE.parent / name
            if not path.is_file() or path.read_bytes() != encoded:
                raise RuntimeError(f"{path.relative_to(ROOT)} differs from generated bytes")
        print(f"Fixtures match: {', '.join(OUTPUTS)}")
        return 0

    verify_pillow_reference(fixtures)
    for name, encoded in fixtures.items():
        path = SOURCE.parent / name
        path.write_bytes(encoded)
        print(f"Wrote {path.relative_to(ROOT)} ({len(encoded)} bytes, sha256={digest(encoded)})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
