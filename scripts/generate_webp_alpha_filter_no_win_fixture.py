#!/usr/bin/env python3
"""Generate a WebP alpha fixture that separates filter sampling from output cost."""

from __future__ import annotations

import argparse
import hashlib
import io
import random
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "tests/fixtures/input/images/png/webp_rgba_alpha_filter_no_win_512x512.png"
WIDTH = 512
HEIGHT = 512
SEED = 0xA17A
EXPECTED_SOURCE_SHA256 = "343008603074288cbdad755679e6e6f0c78e9cb8b96b73cffcb7b316f7229210"


def render_png() -> bytes:
    """Build a seeded 32-color alpha plane with favorable sampled horizontal residuals."""
    from PIL import Image, __version__ as pillow_version

    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    rng = random.Random(SEED)
    palette = rng.sample(range(256), 32)
    alpha = [palette[rng.randrange(len(palette))] for _ in range(WIDTH * HEIGHT)]
    for y in range(0, HEIGHT, 2):
        row = y * WIDTH
        for x in range(2, WIDTH, 2):
            alpha[row + x] = alpha[row + x - 1]

    rgba = bytes(
        channel
        for value in alpha
        for channel in (128, 128, 128, value)
    )
    image = Image.frombytes("RGBA", (WIDTH, HEIGHT), rgba)
    stream = io.BytesIO()
    image.save(stream, format="PNG", compress_level=9, optimize=False)
    return stream.getvalue()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check", action="store_true", help="verify the committed fixture bytes"
    )
    args = parser.parse_args()
    if args.check:
        if not OUTPUT.is_file():
            print(f"stale WebP alpha filter fixture: {OUTPUT}", file=sys.stderr)
            return 1
        actual_sha256 = hashlib.sha256(OUTPUT.read_bytes()).hexdigest()
        if actual_sha256 != EXPECTED_SOURCE_SHA256:
            print(f"stale WebP alpha filter fixture: {OUTPUT}", file=sys.stderr)
            return 1
        print(f"Verified {OUTPUT} (sha256={actual_sha256})")
        return 0

    expected = render_png()
    if hashlib.sha256(expected).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("generated WebP alpha filter PNG differs from its pinned digest")
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(expected)
    print(f"Generated {OUTPUT} (sha256={hashlib.sha256(expected).hexdigest()})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
