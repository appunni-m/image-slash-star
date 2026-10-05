#!/usr/bin/env python3
"""Generate a baseline JPEG with an unused custom AC Huffman table.

The fixture is derived from the complete public baseline_444.jpg input. Its
added table has standard luma code counts but a changed symbol order, and no
scan selects table 2. Pillow must therefore produce the unchanged baseline
pixels while the Rust decoder constructs the generic custom-table fallback.
"""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as PILLOW_VERSION


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/jpeg/baseline_444.jpg"
OUTPUT = ROOT / "tests/fixtures/input/images/jpeg/baseline_444_unused_custom_ac_table.jpg"
EXPECTED_SOURCE_SHA256 = "60dc8ca9db232378a58f6c45321303775dc0b0a8762dd5bfad553cefe390621a"
EXPECTED_OUTPUT_SHA256 = "dafdfa78319a611ff5c88d9db8bd0c93b05ef0f9d528ca4e2d7da9147d958480"
EXPECTED_PILLOW_RGB_SHA256 = "2288f8f3cb6dafaa523538760165f2a4634b9a269e03175b8fbc284158dbb94a"
EXPECTED_AC_LUMA_COUNTS = bytes((
    0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 125,
))


def find_sos_and_ac_luma_table(data: bytearray) -> tuple[int, bytes, bytes]:
    """Read complete marker segments through SOS and locate standard AC table 0."""
    position = 2
    sos_position = None
    luma_counts = None
    luma_values = None

    while position < len(data):
        if data[position] != 0xFF:
            raise ValueError(f"expected JPEG marker at offset {position}")
        marker_start = position
        while position < len(data) and data[position] == 0xFF:
            position += 1
        if position >= len(data):
            raise ValueError("truncated marker before SOS")
        marker = data[position]
        position += 1
        if marker == 0xDA:
            sos_position = marker_start
            break
        if marker in (0xD8, 0xD9) or 0xD0 <= marker <= 0xD7:
            continue

        length_end = position + 2
        if length_end > len(data):
            raise ValueError("truncated marker length before SOS")
        segment_length = int.from_bytes(data[position:length_end], "big")
        if segment_length < 2:
            raise ValueError(f"invalid marker length {segment_length}")
        segment_end = position + segment_length
        if segment_end > len(data):
            raise ValueError("truncated marker payload before SOS")

        if marker == 0xC4:
            cursor = length_end
            while cursor < segment_end:
                table_info = data[cursor]
                cursor += 1
                counts_end = cursor + 16
                if counts_end > segment_end:
                    raise ValueError("truncated DHT code counts")
                counts = bytes(data[cursor:counts_end])
                cursor = counts_end
                value_end = cursor + sum(counts)
                if value_end > segment_end:
                    raise ValueError("truncated DHT symbols")
                values = bytes(data[cursor:value_end])
                cursor = value_end

                if table_info == 0x12:
                    raise ValueError("source already defines AC Huffman table 2")
                if table_info == 0x10:
                    if luma_counts is not None:
                        raise ValueError("source defines AC Huffman table 0 more than once")
                    luma_counts, luma_values = counts, values

        position = segment_end

    if sos_position is None:
        raise ValueError("source has no start-of-scan marker")
    if luma_counts is None or luma_values is None:
        raise ValueError("source has no AC Huffman table 0")
    if luma_counts != EXPECTED_AC_LUMA_COUNTS or len(luma_values) != 162:
        raise ValueError("source AC table 0 differs from the pinned baseline luma table")
    return sos_position, luma_counts, luma_values


def generate_fixture() -> bytes:
    """Insert an unused AC table whose counts match standard luma but values do not."""
    if PILLOW_VERSION != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PILLOW_VERSION}")

    source = SOURCE.read_bytes()
    source_sha256 = hashlib.sha256(source).hexdigest()
    if source_sha256 != EXPECTED_SOURCE_SHA256:
        raise RuntimeError(
            f"baseline_444.jpg SHA-256 mismatch: expected {EXPECTED_SOURCE_SHA256}, "
            f"found {source_sha256}"
        )

    sos_position, counts, original_values = find_sos_and_ac_luma_table(bytearray(source))
    custom_values = bytearray(original_values)
    offset = 0
    for count in counts:
        if count >= 2:
            custom_values[offset], custom_values[offset + 1] = (
                custom_values[offset + 1],
                custom_values[offset],
            )
            break
        offset += count
    else:
        raise RuntimeError("standard luma table has no same-length symbol pair")

    payload = bytes((0x12,)) + counts + bytes(custom_values)
    segment = b"\xff\xc4" + (len(payload) + 2).to_bytes(2, "big") + payload
    encoded = source[:sos_position] + segment + source[sos_position:]
    output_sha256 = hashlib.sha256(encoded).hexdigest()
    if output_sha256 != EXPECTED_OUTPUT_SHA256:
        raise RuntimeError(
            f"generated JPEG SHA-256 mismatch: expected {EXPECTED_OUTPUT_SHA256}, "
            f"found {output_sha256}"
        )

    with Image.open(BytesIO(encoded)) as image:
        image.load()
        if image.format != "JPEG" or image.mode != "RGB" or image.size != (128, 128):
            raise RuntimeError(
                "Pillow returned an unexpected JPEG result: "
                f"format={image.format!r}, mode={image.mode!r}, size={image.size!r}"
            )
        pixels = image.convert("RGB").tobytes()
    pixel_sha256 = hashlib.sha256(pixels).hexdigest()
    if pixel_sha256 != EXPECTED_PILLOW_RGB_SHA256:
        raise RuntimeError(
            "Pillow RGB pixel SHA-256 mismatch: "
            f"expected {EXPECTED_PILLOW_RGB_SHA256}, found {pixel_sha256}"
        )
    return encoded


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

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(encoded)
    print(f"Wrote {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
