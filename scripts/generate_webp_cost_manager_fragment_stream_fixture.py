#!/usr/bin/env python3
"""Generate a high-cardinality VP8L cost-manager parity input."""

from __future__ import annotations

import argparse
import hashlib
import random
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/png/webp_cost_manager_fragment_stream_1024x1024.png"
)
WIDTH = 1_024
HEIGHT = 1_024
COLOR_COUNT = 4_096
SEED = 0x5EED_500
EXPECTED_PIXELS_SHA256 = "5afff9a4bc1b81d0f066a1f75fd2284cb1405cd28abfa0ffa01c17451e83146a"
EXPECTED_SOURCE_SHA256 = "a435566d1b22d8405e38ff5588213b5492050a180633586a3d67c673879aef40"
COPY_DISTANCES = (1, 2, 3, 4, 7, 13, 31, 67, 127, 251, 509, 1_021, 2_047, 4_093, 8_191, 16_381, 32_749)
COPY_LENGTHS = (8, 11, 16, 23, 31, 47, 64, 95, 127, 191, 255, 383, 511, 767, 1_023, 1_535, 2_047, 3_071, 4_095)


def source_pixels() -> bytes:
    """Build seeded literals interleaved with varied, overlapping copy fragments."""
    generator = random.Random(SEED)
    palette = [
        bytes((generator.randrange(256), generator.randrange(256), generator.randrange(256)))
        for _ in range(COLOR_COUNT)
    ]
    pixels = bytearray()

    def append_literals(count: int) -> None:
        for _ in range(count):
            pixels.extend(palette[generator.randrange(COLOR_COUNT)])

    pixel_count = WIDTH * HEIGHT
    append_literals(8_192)
    fragment = 0
    while len(pixels) // 3 < pixel_count:
        remaining = pixel_count - len(pixels) // 3
        if fragment % 7 == 0:
            append_literals(min(remaining, generator.randint(4, 64)))
        else:
            distance = min(generator.choice(COPY_DISTANCES), len(pixels) // 3)
            length = min(remaining, generator.choice(COPY_LENGTHS))
            for _ in range(length):
                source = (len(pixels) // 3 - distance) * 3
                pixels.extend(pixels[source : source + 3])
        fragment += 1

    result = bytes(pixels)
    if hashlib.sha256(result).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("fragment-stream pixels differ from their pinned digest")
    return result


def generate_fixture() -> bytes:
    """Encode the deterministic RGB stream as the pinned PNG source asset."""
    from PIL import Image, __version__ as pillow_version

    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    image = Image.frombytes("RGB", (WIDTH, HEIGHT), source_pixels())
    output = BytesIO()
    image.save(output, format="PNG", compress_level=6)
    fixture = output.getvalue()
    if hashlib.sha256(fixture).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("fragment-stream source PNG differs from its pinned digest")
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
