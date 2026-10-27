#!/usr/bin/env python3
"""Generate a larger VP8L cost-manager repeated-fragment parity input."""

from __future__ import annotations

import argparse
import hashlib
import random
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/png/webp_cost_manager_deep_fragment_stream_2048x1024.png"
)
WIDTH = 2_048
HEIGHT = 1_024
COLOR_COUNT = 2_048
SEED = 0xC057_1027
EXPECTED_PIXELS_SHA256 = "2b64a163aab6f678e14c5ba405b6109bcb36abe37897b8cadd0bc1b1b9932c3b"
EXPECTED_SOURCE_SHA256 = "c3b7842b11dd2e67ce401cb095a354fd49cdb15ea1c934ca90bddca022995d16"
COPY_DISTANCES = (
    1,
    2,
    3,
    5,
    8,
    13,
    21,
    34,
    55,
    89,
    144,
    233,
    377,
    610,
    987,
    1_597,
    2_584,
    4_093,
    8_191,
    16_381,
    32_749,
)
COPY_LENGTHS = (
    10,
    12,
    16,
    21,
    29,
    40,
    57,
    81,
    113,
    157,
    223,
    317,
    449,
    631,
    887,
    1_249,
    1_759,
    2_477,
    3_481,
    4_095,
)


def source_pixels() -> bytes:
    """Build high-cardinality literals interleaved with overlapping copies."""
    generator = random.Random(SEED)
    palette = [
        bytes(
            (
                generator.randrange(256),
                generator.randrange(256),
                generator.randrange(256),
            )
        )
        for _ in range(COLOR_COUNT)
    ]
    pixels = bytearray()

    def append_literals(count: int) -> None:
        for _ in range(count):
            pixels.extend(palette[generator.randrange(len(palette))])

    pixel_count = WIDTH * HEIGHT
    append_literals(8_192)
    fragment = 0
    while len(pixels) // 3 < pixel_count:
        remaining = pixel_count - len(pixels) // 3
        phase = fragment % 29
        if phase == 0:
            append_literals(min(remaining, generator.randint(128, 384)))
        elif phase in (1, 2, 3):
            append_literals(min(remaining, generator.randint(1, 12)))
        else:
            distance = min(generator.choice(COPY_DISTANCES), len(pixels) // 3)
            length = min(remaining, generator.choice(COPY_LENGTHS))
            for _ in range(length):
                source = (len(pixels) // 3 - distance) * 3
                pixels.extend(pixels[source : source + 3])

        fragment += 1

    result = bytes(pixels)
    if hashlib.sha256(result).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("deep-fragment pixels differ from their pinned digest")
    return result


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
        raise RuntimeError("deep-fragment source PNG differs from its pinned digest")
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
