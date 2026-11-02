#!/usr/bin/env python3
"""Insert a repeated 0xFF before a stuffed byte in a small JPEG scan."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tests/fixtures/input/images/jpeg/baseline_444_three_sequential_scans.jpg"
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/jpeg/baseline_444_three_scans_entropy_marker_fill_stuffed_byte.jpg"
)
EXPECTED_SOURCE_SHA256 = "b9e3e277c0d05b2df169804236d5f9b72d3a1c8beda33e845f9c6ffd436f424b"
EXPECTED_OUTPUT_SHA256 = "0c37482aa7b181f42b6152e68ca811566070fc6fe752f9b64d0f4549ccb43dc6"
EXPECTED_PILLOW_RGB_SHA256 = "0d4dccc69dc0ee4ae956be8c624186ddb2b5f3d411671361056c51a7ea7f9e66"
STUFFED_FF = b"\xff\x00"


def scan_ranges(data: bytes) -> list[tuple[int, int]]:
    """Return entropy ranges for each SOS in a complete JPEG stream."""
    ranges = []
    pos = 2
    while pos < len(data):
        while pos < len(data) and data[pos] != 0xFF:
            pos += 1
        if pos >= len(data):
            break

        while pos < len(data) and data[pos] == 0xFF:
            pos += 1
        if pos >= len(data):
            raise RuntimeError("JPEG ends in a truncated marker")

        marker = data[pos]
        pos += 1
        if marker == 0xD9:
            break
        if marker in (0xD8, 0x01) or 0xD0 <= marker <= 0xD7:
            continue
        if pos + 2 > len(data):
            raise RuntimeError("JPEG marker has no complete segment length")

        segment_length = int.from_bytes(data[pos : pos + 2], "big")
        if segment_length < 2 or pos + segment_length > len(data):
            raise RuntimeError("JPEG marker segment has an invalid length")
        if marker != 0xDA:
            pos += segment_length
            continue

        entropy_start = pos + segment_length
        cursor = entropy_start
        while cursor < len(data):
            marker_start = data.find(b"\xff", cursor)
            if marker_start < 0:
                raise RuntimeError("JPEG scan has no following marker")
            code_pos = marker_start + 1
            while code_pos < len(data) and data[code_pos] == 0xFF:
                code_pos += 1
            if code_pos >= len(data):
                raise RuntimeError("JPEG scan ends in a truncated marker")
            marker_code = data[code_pos]
            if marker_code == 0x00 or 0xD0 <= marker_code <= 0xD7:
                cursor = code_pos + 1
                continue
            ranges.append((entropy_start, marker_start))
            pos = marker_start
            break
        else:
            raise RuntimeError("JPEG scan has no following marker")

    return ranges


def generate_fixture() -> bytes:
    """Insert one 0xFF before the first stuffed pair in the second scan."""
    source = SOURCE.read_bytes()
    source_sha256 = sha256(source).hexdigest()
    if source_sha256 != EXPECTED_SOURCE_SHA256:
        raise RuntimeError(
            f"source JPEG SHA-256 mismatch: expected {EXPECTED_SOURCE_SHA256}, "
            f"found {source_sha256}"
        )

    scans = scan_ranges(source)
    if len(scans) != 3:
        raise RuntimeError(f"source JPEG must contain exactly three scans, found {len(scans)}")
    scan_start, scan_end = scans[1]
    stuffed_ff = source.find(STUFFED_FF, scan_start, scan_end)
    if stuffed_ff < 0:
        raise RuntimeError("second JPEG scan has no stuffed FF00 byte")

    encoded = source[:stuffed_ff] + b"\xff" + source[stuffed_ff:]
    output_sha256 = sha256(encoded).hexdigest()
    if output_sha256 != EXPECTED_OUTPUT_SHA256:
        raise RuntimeError(
            f"generated JPEG SHA-256 mismatch: expected {EXPECTED_OUTPUT_SHA256}, "
            f"found {output_sha256}"
        )
    return encoded


def verify_pillow_reference(encoded: bytes) -> None:
    """Confirm pinned Pillow accepts the file and returns its exact pixels."""
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

    pixel_sha256 = sha256(pixels).hexdigest()
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
