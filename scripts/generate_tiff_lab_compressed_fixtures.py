#!/usr/bin/env python3
"""Generate compressed CIELAB TIFF fixtures from the pinned Pillow source."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as PILLOW_VERSION, features


ROOT = Path(__file__).resolve().parents[1]
INPUT_DIR = ROOT / "tests/fixtures/input/images/tiff"
SOURCE_PATH = INPUT_DIR / "lab.tiff"
SIZE = (3, 2)
EXPECTED_PILLOW = ("12.2.0", "4.7.1")
EXPECTED_PIXEL_SHA256 = "403fa5475429b8200e15e60e01ffdb4f2499732bf9538e2d98e78bc1a1d0e64e"
FIXTURES = (
    (
        "lab_lzw.tiff",
        "tiff_lzw",
        5,
        162,
        "4ab3a1dd27b1c8fc9a0579deec08bae2620d3b5349e7b05893ce78cca629b6dc",
    ),
    (
        "lab_deflate.tiff",
        "tiff_adobe_deflate",
        8,
        166,
        "ec537b7f3473b0d5e0a6194f68a94ff02c1d89cf4c9edc9cb1c24f4953ed759c",
    ),
    (
        "lab_packbits.tiff",
        "packbits",
        32773,
        160,
        "3d55dcb8ba69106acd622636959a299fb16fa9060d784a6d3633a80b113c0088",
    ),
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest for fixture bytes."""
    return hashlib.sha256(data).hexdigest()


def source_image() -> tuple[Image.Image, bytes]:
    """Load and validate the pinned CIELAB oracle image."""
    observed_pillow = (PILLOW_VERSION, features.version("libtiff"))
    if observed_pillow != EXPECTED_PILLOW:
        raise RuntimeError(
            f"Pillow/libtiff {EXPECTED_PILLOW!r} is required, found {observed_pillow!r}"
        )

    with Image.open(SOURCE_PATH) as image:
        if (
            image.format != "TIFF"
            or image.mode != "LAB"
            or image.size != SIZE
            or image.tag_v2.get(262) != 8
        ):
            raise RuntimeError("the source fixture must be a 3×2 CIELAB TIFF")
        image.load()
        pixels = image.tobytes()

    if sha256(pixels) != EXPECTED_PIXEL_SHA256:
        raise RuntimeError("the source fixture differs from its pinned CIELAB samples")
    return Image.frombytes("LAB", SIZE, pixels), pixels


def build_fixture(
    source: Image.Image,
    pixels: bytes,
    compression: str,
    compression_tag: int,
    expected_bytes: int,
    expected_sha256: str,
) -> bytes:
    """Encode and verify one complete compressed TIFF through Pillow."""
    encoded = BytesIO()
    source.save(encoded, format="TIFF", compression=compression)
    fixture = encoded.getvalue()
    if len(fixture) != expected_bytes or sha256(fixture) != expected_sha256:
        raise RuntimeError(
            f"{compression} fixture differs from its pinned complete-file bytes"
        )

    with Image.open(BytesIO(fixture)) as image:
        if (
            image.format != "TIFF"
            or image.mode != "LAB"
            or image.size != SIZE
            or image.tag_v2.get(259) != compression_tag
            or image.tag_v2.get(262) != 8
        ):
            raise RuntimeError("fixture lost its CIELAB mode or compression tags")
        image.load()
        if image.tobytes() != pixels:
            raise RuntimeError("Pillow-decoded samples differ from the source CIELAB bytes")
    return fixture


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    source, pixels = source_image()

    for filename, compression, tag, expected_bytes, expected_sha256 in FIXTURES:
        fixture = build_fixture(
            source, pixels, compression, tag, expected_bytes, expected_sha256
        )
        output = INPUT_DIR / filename
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
