#!/usr/bin/env python3
"""Generate varied repeated rows for WebP lossless backward-reference parity."""

from __future__ import annotations

import argparse
import hashlib
import random
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/png/webp_cost_manager_shifted_rows_2048x512.png"
)
WIDTH = 2_048
HEIGHT = 512
COLOR_COUNT = 127
SEED = 0xC057
EXPECTED_PIXELS_SHA256 = "36bb2d35f0575d6c684883f834f7aabb5f39cedf5edeb9f0eacc4e5b1bed7cac"
EXPECTED_SOURCE_SHA256 = "c190d493dc2f746a76d1b73be5aa7232ab2fddf4fbd496d54fc7691cc93a303f"


def source_pixels() -> bytes:
    """Build distinct-color rows shifted through one deterministic base row."""
    rng = random.Random(SEED)
    palette = [
        (rng.randrange(256), rng.randrange(256), rng.randrange(256))
        for _ in range(COLOR_COUNT)
    ]
    base = bytes(
        channel
        for _ in range(WIDTH)
        for channel in palette[rng.randrange(COLOR_COUNT)]
    )
    rows = []
    for y in range(HEIGHT):
        shift = (y * 37 + y * y * 29) % WIDTH
        cut = shift * 3
        rows.append(base[cut:] + base[:cut])
    pixels = b"".join(rows)
    if hashlib.sha256(pixels).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("shifted-row pixels differ from their pinned digest")
    return pixels


def generate_fixture() -> bytes:
    """Encode the deterministic RGB pixels as the pinned PNG source asset."""
    from PIL import Image, __version__ as pillow_version

    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    image = Image.frombytes("RGB", (WIDTH, HEIGHT), source_pixels())
    output = BytesIO()
    image.save(output, format="PNG", compress_level=6)
    fixture = output.getvalue()
    if hashlib.sha256(fixture).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("shifted-row source PNG differs from its pinned digest")
    return fixture


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group()
    action.add_argument("--check", action="store_true", help="verify without writing")
    action.add_argument("--write", action="store_true", help="write the source PNG")
    arguments = parser.parse_args()

    if arguments.check:
        if not OUTPUT.is_file():
            raise RuntimeError(
                f"generated fixture is stale or missing: {OUTPUT.relative_to(ROOT)}"
            )
        actual_sha256 = hashlib.sha256(OUTPUT.read_bytes()).hexdigest()
        if actual_sha256 != EXPECTED_SOURCE_SHA256:
            raise RuntimeError(
                f"generated fixture is stale or missing: {OUTPUT.relative_to(ROOT)}"
            )
        print(f"Verified {OUTPUT.relative_to(ROOT)}")
        return 0

    fixture = generate_fixture()
    if OUTPUT.exists() and OUTPUT.read_bytes() != fixture:
        raise RuntimeError(
            f"refusing to replace a different fixture at {OUTPUT.relative_to(ROOT)}"
        )
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(fixture)
    print(f"Wrote {OUTPUT.relative_to(ROOT)}")
    print(f"source_sha256: {hashlib.sha256(fixture).hexdigest()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
