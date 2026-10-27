#!/usr/bin/env python3
"""Generate the repeated high-cardinality WebP lossless parity source."""

from __future__ import annotations

import argparse
import hashlib
import random
from io import BytesIO
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "tests/fixtures/input/images/png/webp_singleton_copy_group_1024x1024.png"
WIDTH = 1_024
HEIGHT = 1_024
BASE_PIXEL_COUNT = 4_096
SEED = 0xC057500
EXPECTED_PIXELS_SHA256 = "a29bae79270897e1a851e3a69c84a3dc125b0b2170c583cc9775fa94d6a7d36f"
EXPECTED_SOURCE_SHA256 = "f23d13f12432aa352e89f2be02398a25ef3870ff4e185ac12b3932b5f7e197ba"


def source_pixels() -> bytes:
    """Build 4,096 deterministic RGB colors and repeat them across the image."""
    generator = random.Random(SEED)
    palette = [
        tuple(generator.randrange(256) for _ in range(3))
        for _ in range(BASE_PIXEL_COUNT)
    ]
    if len(set(palette)) != BASE_PIXEL_COUNT:
        raise RuntimeError("singleton copy-group palette must contain 4,096 colors")
    base = bytes(channel for color in palette for channel in color)
    pixels = base * (WIDTH * HEIGHT // BASE_PIXEL_COUNT)
    if hashlib.sha256(pixels).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("singleton copy-group pixels differ from their pinned digest")
    return pixels


def generate_fixture() -> bytes:
    """Encode the pinned RGB pattern as a Pillow 12.2.0 PNG."""
    from PIL import Image, __version__ as pillow_version

    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    image = Image.frombytes("RGB", (WIDTH, HEIGHT), source_pixels())
    output = BytesIO()
    image.save(output, format="PNG", compress_level=6)
    fixture = output.getvalue()
    if hashlib.sha256(fixture).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("singleton copy-group PNG differs from its pinned digest")
    return fixture


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group()
    action.add_argument("--check", action="store_true", help="verify without writing")
    action.add_argument("--write", action="store_true", help="write the source PNG")
    arguments = parser.parse_args()
    fixture = generate_fixture()

    if arguments.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != fixture:
            raise RuntimeError(f"generated fixture is stale or missing: {OUTPUT.relative_to(ROOT)}")
        print(f"Verified {OUTPUT.relative_to(ROOT)}")
        return 0

    if OUTPUT.exists() and OUTPUT.read_bytes() != fixture:
        raise RuntimeError(f"refusing to replace a different fixture at {OUTPUT.relative_to(ROOT)}")
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(fixture)
    print(f"Wrote {OUTPUT.relative_to(ROOT)}")
    print(f"source_sha256: {hashlib.sha256(fixture).hexdigest()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
