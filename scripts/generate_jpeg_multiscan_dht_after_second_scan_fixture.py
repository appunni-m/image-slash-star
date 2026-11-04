#!/usr/bin/env python3
"""Repeat JPEG Huffman tables between the second and third baseline scans."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tests/fixtures/input/images/jpeg/baseline_444_three_sequential_scans.jpg"
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/jpeg/baseline_444_three_scans_dht_after_second_scan.jpg"
)
EXPECTED_SOURCE_SHA256 = "b9e3e277c0d05b2df169804236d5f9b72d3a1c8beda33e845f9c6ffd436f424b"
EXPECTED_OUTPUT_SHA256 = "8fa86b39601dc652d6a6a7f65c3e15d48568d7498071b1f1fd390f87787fc353"
EXPECTED_PILLOW_RGB_SHA256 = "0d4dccc69dc0ee4ae956be8c624186ddb2b5f3d411671361056c51a7ea7f9e66"
SOI = b"\xff\xd8"
EOI = 0xD9
DHT = 0xC4
SOS = 0xDA


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def insertion_point_and_header_dhts(data: bytes) -> tuple[int, list[bytes]]:
    """Locate the third scan and collect DHT segments from the JPEG header."""
    if not data.startswith(SOI):
        raise RuntimeError("source JPEG is missing its SOI marker")

    pos = len(SOI)
    scan_count = 0
    header_dhts: list[bytes] = []

    while pos < len(data):
        marker_start = pos
        if data[pos] != 0xFF:
            raise RuntimeError(f"expected JPEG marker at byte {pos}")
        while pos < len(data) and data[pos] == 0xFF:
            pos += 1
        if pos >= len(data):
            break

        marker = data[pos]
        pos += 1
        if marker == EOI:
            break
        if marker == 0x00:
            raise RuntimeError("unexpected stuffed byte outside JPEG entropy data")
        if marker in {0xD8, 0x01} or 0xD0 <= marker <= 0xD7:
            continue
        if pos + 2 > len(data):
            raise RuntimeError("truncated JPEG segment length")

        segment_length = int.from_bytes(data[pos : pos + 2], "big")
        segment_end = pos + segment_length
        if segment_length < 2 or segment_end > len(data):
            raise RuntimeError("invalid JPEG segment extent")

        if marker == DHT and scan_count == 0:
            header_dhts.append(data[marker_start:segment_end])

        if marker != SOS:
            pos = segment_end
            continue

        scan_count += 1
        if scan_count == 3:
            return marker_start, header_dhts

        # The next unstuffed, non-restart marker terminates this entropy scan.
        pos = segment_end
        while pos < len(data):
            if data[pos] != 0xFF:
                pos += 1
                continue

            next_marker_start = pos
            marker_code_pos = pos + 1
            while marker_code_pos < len(data) and data[marker_code_pos] == 0xFF:
                marker_code_pos += 1
            if marker_code_pos >= len(data):
                raise RuntimeError("truncated marker after JPEG entropy data")

            code = data[marker_code_pos]
            if code == 0x00 or 0xD0 <= code <= 0xD7:
                pos = marker_code_pos + 1
                continue
            pos = next_marker_start
            break

    raise RuntimeError("source JPEG does not contain three complete scans")


def generate_fixture() -> bytes:
    """Insert the original, equivalent Huffman tables before the third scan."""
    source = SOURCE.read_bytes()
    source_sha256 = sha256(source)
    if source_sha256 != EXPECTED_SOURCE_SHA256:
        raise RuntimeError(
            f"source JPEG SHA-256 mismatch: expected {EXPECTED_SOURCE_SHA256}, "
            f"found {source_sha256}"
        )

    third_scan_start, header_dhts = insertion_point_and_header_dhts(source)
    if len(header_dhts) != 4:
        raise RuntimeError(f"expected four header DHT segments, found {len(header_dhts)}")

    output = (
        source[:third_scan_start]
        + b"".join(header_dhts)
        + source[third_scan_start:]
    )
    output_sha256 = sha256(output)
    if output_sha256 != EXPECTED_OUTPUT_SHA256:
        raise RuntimeError(
            f"generated JPEG SHA-256 mismatch: expected {EXPECTED_OUTPUT_SHA256}, "
            f"found {output_sha256}"
        )
    return output


def verify_pillow_reference(encoded: bytes) -> None:
    """Confirm Pillow decodes the repeated tables to the pinned RGB pixels."""
    import PIL
    from PIL import Image

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PIL.__version__}")

    with Image.open(BytesIO(encoded)) as image:
        image.verify()
    with Image.open(BytesIO(encoded)) as image:
        image.load()
        if image.format != "JPEG" or image.mode != "RGB" or image.size != (8, 8):
            raise RuntimeError(
                "Pillow returned an unexpected JPEG result: "
                f"format={image.format!r}, mode={image.mode!r}, size={image.size!r}"
            )
        pixels = image.tobytes()

    pixel_sha256 = sha256(pixels)
    if pixel_sha256 != EXPECTED_PILLOW_RGB_SHA256:
        raise RuntimeError(
            f"Pillow RGB SHA-256 mismatch: expected {EXPECTED_PILLOW_RGB_SHA256}, "
            f"found {pixel_sha256}"
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
            raise RuntimeError(f"{OUTPUT.relative_to(ROOT)} differs from generated bytes")
        print(f"Fixture matches: {OUTPUT.relative_to(ROOT)}")
        return 0

    verify_pillow_reference(encoded)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(encoded)
    print(f"Wrote {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
