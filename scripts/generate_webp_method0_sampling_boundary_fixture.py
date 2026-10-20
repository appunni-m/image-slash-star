#!/usr/bin/env python3
"""Generate the RGB PNG at WebP method 0's first 51-macroblock sample."""

from __future__ import annotations

import argparse
import hashlib
import io
from pathlib import Path

from PIL import Image
import yaml

from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tests" / "fixtures" / "input" / "images" / "png" / "rgb.png"
OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "png"
    / "webp_method0_sampling_boundary_204_macroblocks.png"
)
WIDTH = 192
HEIGHT = 272
MACROBLOCK_SIZE = 16
SAMPLED_MACROBLOCK_INDEX = 50
SOURCE_SHA256 = "fc88e54a55583ec05de9253f7fe2576f5431ae3be587b22c25087d7ba95d0fb8"
EXPECTED_PIXELS_SHA256 = "daf5681d8ffa7bb34be77c028a192a358d757cb1da3e2cb33ee932296335c69e"
EXPECTED_INPUT_SHA256 = "ed366fb0782fb533d08eb853fca3409488179a0d855ff6036b99cda0724b556f"


def fixture_pixels() -> bytes:
    """Tile the reference RGB pattern and texture only sampled macroblock 51."""
    source_bytes = SOURCE.read_bytes()
    if hashlib.sha256(source_bytes).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("WebP method-zero boundary source pattern changed")

    with Image.open(SOURCE) as source_image:
        source_image.load()
        if source_image.mode != "RGB" or source_image.size != (128, 128):
            raise RuntimeError("WebP method-zero boundary source is not RGB 128x128")
        source = source_image.copy()

    image = Image.new("RGB", (WIDTH, HEIGHT))
    pixels = image.load()
    source_pixels = source.load()
    for y in range(HEIGHT):
        for x in range(WIDTH):
            pixels[x, y] = source_pixels[x % source.width, y % source.height]

    macroblock_width = WIDTH // MACROBLOCK_SIZE
    row, column = divmod(SAMPLED_MACROBLOCK_INDEX, macroblock_width)
    left = column * MACROBLOCK_SIZE
    top = row * MACROBLOCK_SIZE
    for y in range(top, top + MACROBLOCK_SIZE):
        for x in range(left, left + MACROBLOCK_SIZE):
            value = 255 if ((x // 2 + y // 2) & 1) else 0
            pixels[x, y] = (value, 255 - value, (x * 7 + y * 3) & 0xFF)

    result = image.tobytes()
    if hashlib.sha256(result).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("WebP method-zero boundary pixels differ from their pinned digest")
    return result


def fixture_bytes() -> bytes:
    """Encode the deterministic RGB stimulus as a complete PNG source."""
    image = Image.frombytes("RGB", (WIDTH, HEIGHT), fixture_pixels())
    output = io.BytesIO()
    image.save(output, format="PNG", compress_level=9)
    result = output.getvalue()
    if hashlib.sha256(result).hexdigest() != EXPECTED_INPUT_SHA256:
        raise RuntimeError("WebP method-zero boundary PNG differs from its pinned digest")
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
