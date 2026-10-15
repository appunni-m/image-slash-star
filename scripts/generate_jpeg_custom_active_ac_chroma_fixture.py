#!/usr/bin/env python3
"""Generate a baseline JPEG with a custom AC table selected for chroma scans.

The fixture changes two unused, same-length symbols in the active table 1 of
the pinned 4:4:4 baseline image. Pillow therefore returns the exact baseline
pixels while the decoder must reject the standard-chroma table fast path for
both Cb and Cr.
"""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as PILLOW_VERSION


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/jpeg/baseline_444.jpg"
OUTPUT = ROOT / "tests/fixtures/input/images/jpeg/baseline_444_custom_ac_chroma_values.jpg"
EXPECTED_SOURCE_SHA256 = "60dc8ca9db232378a58f6c45321303775dc0b0a8762dd5bfad553cefe390621a"
EXPECTED_OUTPUT_SHA256 = "157f83e6e9a3f0d41dde66adf35dc156d1efaa1acfd77c64a83c782214059799"
EXPECTED_PILLOW_RGB_SHA256 = "2288f8f3cb6dafaa523538760165f2a4634b9a269e03175b8fbc284158dbb94a"
EXPECTED_AC_CHROMA_COUNTS = bytes((
    0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 119,
))
AC_CHROMA_TABLE_INFO = 0x11
SWAP_INDICES = (43, 44)
SWAP_SYMBOLS = (0x17, 0x18)


def find_selected_chroma_ac_table(data: bytes) -> tuple[int, bytes, bytes]:
    """Return the symbol offset, counts, and values of the selected AC table 1."""
    position = 2
    table = None

    while position < len(data):
        if data[position] != 0xFF:
            raise ValueError(f"expected JPEG marker at offset {position}")

        marker_start = position
        while position < len(data) and data[position] == 0xFF:
            position += 1
        if position >= len(data):
            raise ValueError("truncated marker before first scan")

        marker = data[position]
        position += 1
        if marker in (0xD8, 0xD9, 0x01) or 0xD0 <= marker <= 0xD7:
            if marker == 0xD9:
                break
            continue

        length_end = position + 2
        if length_end > len(data):
            raise ValueError("truncated marker length before first scan")
        segment_length = int.from_bytes(data[position:length_end], "big")
        if segment_length < 2:
            raise ValueError(f"invalid marker length {segment_length}")
        segment_end = position + segment_length
        if segment_end > len(data):
            raise ValueError("truncated marker payload before first scan")

        if marker == 0xC4:
            cursor = length_end
            while cursor < segment_end:
                table_info = data[cursor]
                cursor += 1
                counts_end = cursor + 16
                if counts_end > segment_end:
                    raise ValueError("truncated DHT code counts")
                counts = data[cursor:counts_end]
                symbol_start = counts_end
                symbol_end = symbol_start + sum(counts)
                if symbol_end > segment_end:
                    raise ValueError("truncated DHT symbols")
                values = data[symbol_start:symbol_end]

                if table_info == AC_CHROMA_TABLE_INFO:
                    if table is not None:
                        raise ValueError("source defines AC Huffman table 1 more than once")
                    table = (symbol_start, counts, values)
                cursor = symbol_end

            if cursor != segment_end:
                raise ValueError("DHT segment contains trailing bytes")

        if marker == 0xDA:
            if table is None:
                raise ValueError("source has no AC Huffman table 1 before its first scan")

            payload_start = length_end
            scan_components = data[payload_start]
            expected_payload_size = 1 + 2 * scan_components + 3
            if segment_end - payload_start != expected_payload_size:
                raise ValueError("invalid SOS component-selector payload")

            selectors: dict[int, int] = {}
            cursor = payload_start + 1
            for _ in range(scan_components):
                component_id = data[cursor]
                selector = data[cursor + 1]
                if component_id in selectors:
                    raise ValueError("SOS repeats a component identifier")
                selectors[component_id] = selector
                cursor += 2

            for component_id in (2, 3):
                selector = selectors.get(component_id)
                if selector is None or selector & 0x0F != 1:
                    raise ValueError(
                        f"first SOS does not select AC table 1 for component {component_id}"
                    )
            return table

        position = segment_end

    raise ValueError("source has no first scan marker")


def verify_pillow_pixels(encoded: bytes) -> bytes:
    """Decode a complete JPEG with the pinned Pillow oracle and return RGB bytes."""
    with Image.open(BytesIO(encoded)) as image:
        image.load()
        if image.format != "JPEG" or image.mode != "RGB" or image.size != (128, 128):
            raise RuntimeError(
                "Pillow returned an unexpected JPEG result: "
                f"format={image.format!r}, mode={image.mode!r}, size={image.size!r}"
            )
        return image.convert("RGB").tobytes()


def generate_fixture() -> bytes:
    """Change two unused 16-bit symbols in the scan-selected chroma AC table."""
    if PILLOW_VERSION != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PILLOW_VERSION}")

    source = SOURCE.read_bytes()
    source_sha256 = hashlib.sha256(source).hexdigest()
    if source_sha256 != EXPECTED_SOURCE_SHA256:
        raise RuntimeError(
            f"baseline_444.jpg SHA-256 mismatch: expected {EXPECTED_SOURCE_SHA256}, "
            f"found {source_sha256}"
        )

    symbol_start, counts, values = find_selected_chroma_ac_table(source)
    if counts != EXPECTED_AC_CHROMA_COUNTS or len(values) != 162:
        raise RuntimeError("selected AC table 1 differs from the pinned standard chroma counts")
    if sum(counts[:15]) != SWAP_INDICES[0]:
        raise RuntimeError("the pinned symbol pair is no longer in the 16-bit code-length group")
    actual_symbols = tuple(values[index] for index in SWAP_INDICES)
    if actual_symbols != SWAP_SYMBOLS:
        raise RuntimeError(
            f"pinned AC symbol pair mismatch: expected {SWAP_SYMBOLS!r}, "
            f"found {actual_symbols!r}"
        )

    encoded = bytearray(source)
    first, second = SWAP_INDICES
    encoded[symbol_start + first], encoded[symbol_start + second] = (
        encoded[symbol_start + second],
        encoded[symbol_start + first],
    )
    encoded_bytes = bytes(encoded)

    output_sha256 = hashlib.sha256(encoded_bytes).hexdigest()
    if output_sha256 != EXPECTED_OUTPUT_SHA256:
        raise RuntimeError(
            f"generated JPEG SHA-256 mismatch: expected {EXPECTED_OUTPUT_SHA256}, "
            f"found {output_sha256}"
        )

    baseline_pixels = verify_pillow_pixels(source)
    candidate_pixels = verify_pillow_pixels(encoded_bytes)
    if candidate_pixels != baseline_pixels:
        raise RuntimeError("custom active AC table changed Pillow's decoded RGB pixels")
    pixel_sha256 = hashlib.sha256(candidate_pixels).hexdigest()
    if pixel_sha256 != EXPECTED_PILLOW_RGB_SHA256:
        raise RuntimeError(
            "Pillow RGB pixel SHA-256 mismatch: "
            f"expected {EXPECTED_PILLOW_RGB_SHA256}, found {pixel_sha256}"
        )
    return encoded_bytes


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
    print(
        f"Wrote {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes, "
        f"SHA-256 {hashlib.sha256(encoded).hexdigest()})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
