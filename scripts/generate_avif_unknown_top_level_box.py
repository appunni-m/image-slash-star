#!/usr/bin/env python3
"""Generate a still AVIF followed by a valid unknown top-level box."""

from __future__ import annotations

import argparse
from hashlib import sha256
import struct
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "portable_lossless_420_color_texture_8x8.avif"
OUTPUT = ASSETS / "unknown_top_level_box_ignored.avif"
SOURCE_SHA256 = "eba2565967dbab65b88d58c6855ee920d06f69ef39ed610306cff3e322beb0a2"
OUTPUT_SHA256 = "ebd1e037026559f6a2f7dc12271c94a7440b2c06c79843eed5380afb92072d64"
PIXEL_SHA256 = "8099fa1157400c228f24ce0e41a6f9c59cc73c8be02953024a92761dba0db709"
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def pillow_rgb(data: bytes) -> bytes:
    """Decode a still image through the pinned Pillow/libavif oracle."""
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"Pillow oracle differs from pin: {observed!r}")

    with Image.open(BytesIO(data)) as image:
        if image.format != "AVIF" or image.mode != "RGB" or image.size != (8, 8):
            raise RuntimeError("Pillow opened AVIF with unexpected image properties")
        if image.n_frames != 1:
            raise RuntimeError("fixture must decode as one still image")
        image.verify()

    with Image.open(BytesIO(data)) as image:
        image.load()
        pixels = image.convert("RGB").tobytes()
    if sha256(pixels).hexdigest() != PIXEL_SHA256:
        raise RuntimeError("Pillow RGB bytes differ from the pinned exact reference")
    return pixels


def fixture_bytes() -> bytes:
    """Append one bounded, unknown box after the complete AVIF still."""
    source = SOURCE.read_bytes()
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned lossless AVIF source hash changed")

    offset = 0
    kinds = []
    while offset < len(source):
        if len(source) - offset < 8:
            raise RuntimeError("pinned source has a truncated top-level box")
        size = int.from_bytes(source[offset : offset + 4], "big")
        if size < 8 or size > len(source) - offset:
            raise RuntimeError("pinned source has an invalid top-level box size")
        kinds.append(source[offset + 4 : offset + 8])
        offset += size
    if kinds != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError(f"pinned source top-level boxes changed: {kinds!r}")

    unknown_box = struct.pack(">I4s", 12, b"zunk") + b"TEST"
    candidate = source + unknown_box
    if sha256(candidate).hexdigest() != OUTPUT_SHA256:
        raise RuntimeError("generated unknown-box fixture hash changed")
    if pillow_rgb(source) != pillow_rgb(candidate):
        raise RuntimeError("unknown top-level box changed Pillow's decoded pixels")
    return candidate


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    candidate = fixture_bytes()
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != candidate:
            raise RuntimeError(f"{OUTPUT.relative_to(ROOT)} differs from its recipe")
        print(f"Verified {OUTPUT.relative_to(ROOT)}")
    else:
        OUTPUT.write_bytes(candidate)
        print(f"Wrote {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
