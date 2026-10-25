#!/usr/bin/env python3
"""Generate compressed grayscale TIFFs with FillOrder 2."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as PILLOW_VERSION, features

ROOT = Path(__file__).resolve().parents[1]
OUTPUT_DIR = ROOT / "tests/fixtures/input/images/tiff"
SIZE = (17, 13)
EXPECTED_PILLOW = ("12.2.0", "4.7.1")
EXPECTED_PIXEL_SHA256 = "b03d7423b9aaf79161be9a0f1a119cd3a814f4d81815f10b80aad8bdc2f5c4f4"
FIXTURES = (
    (
        "gray8_deflate_fillorder2.tiff",
        "tiff_adobe_deflate",
        8,
        366,
        "359ce018950a1959a1fc1f621e2313177817772bc42545346cd191b744d1e851",
    ),
    (
        "gray8_lzw_fillorder2.tiff",
        "tiff_lzw",
        5,
        384,
        "688b5355bd642d870c6d8450fb943ad189d9dbd7b9df834127bdbbc3f060dae7",
    ),
    (
        "gray8_packbits_fillorder2.tiff",
        "packbits",
        32773,
        368,
        "68b28cdf7af702becd02ccb328da7680593873563b51c14844c4305feef54924",
    ),
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def source_pixels() -> bytes:
    width, height = SIZE
    return bytes(
        (x * 17 + y * 29 + (x * y) % 37) & 0xFF
        for y in range(height)
        for x in range(width)
    )


def build_fixture(
    compression: str,
    tag_compression: int,
    expected_bytes: int,
    expected_sha256: str,
) -> bytes:
    observed_pillow = (PILLOW_VERSION, features.version("libtiff"))
    if observed_pillow != EXPECTED_PILLOW:
        raise RuntimeError(
            f"Pillow/libtiff {EXPECTED_PILLOW!r} is required, found {observed_pillow!r}"
        )

    expected_pixels = source_pixels()
    source = Image.frombytes("L", SIZE, expected_pixels)
    encoded = BytesIO()
    source.save(
        encoded,
        format="TIFF",
        compression=compression,
        tiffinfo={266: 2},
    )
    fixture = encoded.getvalue()
    if len(fixture) != expected_bytes or sha256(fixture) != expected_sha256:
        raise RuntimeError("generated TIFF differs from its pinned complete-file bytes")

    with Image.open(BytesIO(fixture)) as image:
        if (
            image.format != "TIFF"
            or image.mode != "L"
            or image.size != SIZE
            or image.tag_v2.get(259) != tag_compression
            or image.tag_v2.get(266) != 2
        ):
            raise RuntimeError("fixture does not retain its compression and FillOrder tags")
        image.load()
        pixels = image.tobytes()
    if pixels != expected_pixels or sha256(pixels) != EXPECTED_PIXEL_SHA256:
        raise RuntimeError("Pillow decoded pixels differ from the pinned grayscale source")
    return fixture


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    for filename, compression, tag_compression, expected_bytes, expected_sha256 in FIXTURES:
        fixture = build_fixture(
            compression, tag_compression, expected_bytes, expected_sha256
        )
        output = OUTPUT_DIR / filename
        if args.check:
            if not output.is_file() or output.read_bytes() != fixture:
                raise RuntimeError(f"fixture is missing or stale: {output}")
            action = "Verified"
        else:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(fixture)
            action = "Wrote"
        print(f"{action} {output} ({len(fixture)} bytes, SHA-256 {sha256(fixture)})")


if __name__ == "__main__":
    main()
