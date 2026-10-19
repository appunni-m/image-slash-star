#!/usr/bin/env python3
"""Generate and verify little- and big-endian uncompressed RGB16 TIFF strips."""

from __future__ import annotations

import argparse
import hashlib
import struct
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUTS = {
    "little": ROOT / "tests/fixtures/input/images/tiff/rgb16_le_uncompressed_strips.tiff",
    "big": ROOT / "tests/fixtures/input/images/tiff/rgb16_be_uncompressed_strips.tiff",
}
EXPECTED_SHA256 = {
    "little": "aebc7d664d848fe2d4e5054c4926ea6b38b99665d2d31bd6c86419aa07b7874b",
    "big": "9b91240255da2d849d12be62e59c3d868721112253ed46f1284578597567b0e3",
}
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


def verify_pillow_reference(fixtures: dict[str, bytes]) -> None:
    """Check exact public pixels and storage tags against the pinned Pillow oracle."""
    import yaml
    from PIL import Image

    sys_path = ROOT / "scripts"
    import sys

    sys.path.insert(0, str(sys_path))
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


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify committed fixture bytes without rewriting them",
    )
    args = parser.parse_args()
    fixtures = {byte_order: build_fixture(byte_order) for byte_order in OUTPUTS}
    if args.check:
        for byte_order, output in OUTPUTS.items():
            if not output.is_file() or output.read_bytes() != fixtures[byte_order]:
                raise RuntimeError(f"{output.relative_to(ROOT)} differs from generated bytes")
            print(f"Fixture matches: {output.relative_to(ROOT)}")
        return 0

    verify_pillow_reference(fixtures)
    for byte_order, output in OUTPUTS.items():
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(fixtures[byte_order])
        print(
            f"Wrote {output.relative_to(ROOT)} ({len(fixtures[byte_order])} bytes, "
            f"SHA-256 {hashlib.sha256(fixtures[byte_order]).hexdigest()})"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
