#!/usr/bin/env python3
"""Oversubscribe DC0 in a progressive JPEG while retaining the other tables."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
INPUT_DIR = ROOT / "tests/fixtures/input/images/jpeg"
SOURCE = INPUT_DIR / "progressive_gray.jpg"
OUTPUT = INPUT_DIR / "progressive_oversubscribed_dc_huffman.jpg"
SOURCE_SHA256 = "13ef0fb89cd1541f93e1762ef707adbb9725c9eea44d146fa240ffcfbe128c93"
OUTPUT_SHA256 = "883582bb9b9d43c06f950dca99782441350fa25262ad30247a0d87d2d7bff1b3"
BROKEN_DATA_MESSAGE = "broken data stream when reading image file"
OVERSUBSCRIBED_DC0 = bytes((0, 3, *([0] * 15), 0, 1, 2))


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def replace_dc0_table(payload: bytes) -> bytes:
    """Replace DC0 in a DHT payload and preserve all its sibling tables."""
    rewritten = bytearray()
    position = 0
    replaced = False
    while position < len(payload):
        if len(payload) - position < 17:
            raise ValueError("truncated JPEG Huffman table header")
        table_info = payload[position]
        symbol_count = sum(payload[position + 1 : position + 17])
        record_end = position + 17 + symbol_count
        if record_end > len(payload):
            raise ValueError("truncated JPEG Huffman table symbols")
        if table_info == 0 and not replaced:
            rewritten.extend(OVERSUBSCRIBED_DC0)
            replaced = True
        else:
            rewritten.extend(payload[position:record_end])
        position = record_end
    if not replaced:
        raise ValueError("progressive JPEG has no DC0 Huffman table")
    return bytes(rewritten)


def generate_fixture() -> bytes:
    """Replace the first pre-scan DC0 DHT with three one-bit codes."""
    source = SOURCE.read_bytes()
    source_sha256 = sha256(source)
    if source_sha256 != SOURCE_SHA256:
        raise RuntimeError(
            f"source JPEG SHA-256 mismatch: expected {SOURCE_SHA256}, found {source_sha256}"
        )
    if not source.startswith(b"\xff\xd8"):
        raise RuntimeError("source JPEG does not start with SOI")

    position = 2
    while position < len(source):
        marker_start = position
        if source[position] != 0xFF:
            raise RuntimeError("unexpected non-marker byte before progressive scan")
        while position < len(source) and source[position] == 0xFF:
            position += 1
        if position >= len(source):
            raise RuntimeError("truncated JPEG marker")
        marker_code = source[position]
        position += 1
        if marker_code == 0xDA:
            break
        if marker_code in (0xD8, 0xD9) or 0xD0 <= marker_code <= 0xD7:
            continue
        if position + 2 > len(source):
            raise RuntimeError("truncated JPEG marker length")
        segment_length = int.from_bytes(source[position : position + 2], "big")
        if segment_length < 2:
            raise RuntimeError("invalid JPEG marker length")
        segment_end = position + segment_length
        if segment_end > len(source):
            raise RuntimeError("truncated JPEG marker segment")
        if marker_code == 0xC4:
            payload = replace_dc0_table(source[position + 2 : segment_end])
            encoded = (
                source[:marker_start]
                + b"\xff\xc4"
                + (len(payload) + 2).to_bytes(2, "big")
                + payload
                + source[segment_end:]
            )
            output_sha256 = sha256(encoded)
            if output_sha256 != OUTPUT_SHA256:
                raise RuntimeError(
                    f"generated JPEG SHA-256 mismatch: expected {OUTPUT_SHA256}, "
                    f"found {output_sha256}"
                )
            return encoded
        position = segment_end
    raise RuntimeError("progressive JPEG has no pre-scan DHT segment")


def verify_pillow_reference(encoded: bytes) -> None:
    """Pin successful structural verification and the malformed decode result."""
    import PIL
    from PIL import Image

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PIL.__version__}")
    with Image.open(BytesIO(encoded)) as image:
        image.verify()
    try:
        with Image.open(BytesIO(encoded)) as image:
            image.load()
    except OSError as error:
        if str(error) != BROKEN_DATA_MESSAGE:
            raise RuntimeError(f"unexpected Pillow decode error: {error}") from error
    else:
        raise RuntimeError("Pillow unexpectedly decoded the oversubscribed DHT")


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

    verify_pillow_reference(encoded)
    OUTPUT.write_bytes(encoded)
    print(f"Wrote {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
