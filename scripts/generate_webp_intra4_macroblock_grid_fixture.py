#!/usr/bin/env python3
"""Generate a checkerboard of 4x4 gray tiles for VP8 method-zero intra4."""

from __future__ import annotations

import argparse
import hashlib
import io
from pathlib import Path

from PIL import Image
import yaml

from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "png"
    / "webp_intra4_macroblock_grid_32x32.png"
)
WIDTH = 32
HEIGHT = 32
TILE_SIZE = 4
EXPECTED_PIXELS_SHA256 = "51a0e15254819a4e60f587345b2fb9078c5d3bdd44c89fcb1f5672555d030c3a"
EXPECTED_INPUT_SHA256 = "6b8ff21ec037d123f4efd9a53623565f2aed6cb775ff66c83bc2fb1fb9413259"


def fixture_pixels() -> bytes:
    """Alternate black and white 4x4 tiles within each VP8 macroblock."""
    pixels = bytearray()
    for y in range(HEIGHT):
        for x in range(WIDTH):
            value = 255 if ((x // TILE_SIZE + y // TILE_SIZE) & 1) else 0
            pixels.extend((value, value, value))

    result = bytes(pixels)
    if hashlib.sha256(result).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("WebP intra4 grid pixels differ from their pinned digest")
    return result


def fixture_bytes() -> bytes:
    """Encode the deterministic RGB stimulus as a complete PNG source."""
    image = Image.frombytes("RGB", (WIDTH, HEIGHT), fixture_pixels())
    output = io.BytesIO()
    image.save(output, format="PNG", compress_level=9)
    result = output.getvalue()
    if hashlib.sha256(result).hexdigest() != EXPECTED_INPUT_SHA256:
        raise RuntimeError("WebP intra4 grid PNG differs from its pinned digest")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group()
    action.add_argument("--check", action="store_true", help="verify without writing")
    action.add_argument("--write", action="store_true", help="write the source PNG")
    arguments = parser.parse_args()

    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    data = fixture_bytes()
    if arguments.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != data:
            raise RuntimeError(f"generated fixture is stale or missing: {OUTPUT.relative_to(ROOT)}")
        print(f"Verified {OUTPUT.relative_to(ROOT)}")
        return 0

    if OUTPUT.exists() and OUTPUT.read_bytes() != data:
        raise RuntimeError(f"refusing to replace a different fixture at {OUTPUT.relative_to(ROOT)}")
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(data)
    print(f"Wrote {OUTPUT.relative_to(ROOT)}")
    print(f"source_sha256: {hashlib.sha256(data).hexdigest()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
