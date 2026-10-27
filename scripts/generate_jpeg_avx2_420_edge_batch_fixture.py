#!/usr/bin/env python3
"""Generate the RGB parity input for the JPEG AVX2 4:2:0 edge batch."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/fixtures/input/images/png/jpeg_avx2_420_edge_batch_113x17.png"
WIDTH = 113
HEIGHT = 17
EXPECTED_PIXELS_SHA256 = "03a763007ddc3a6bd612ddfe4df7ebdb354e7cc494aa62b3e04825ae3c71e729"
EXPECTED_SOURCE_SHA256 = "fc952ca09c46170552b0a265db8825dc752ddedad70e834724dbfc7d5f03242d"


def source_pixels() -> bytes:
    """Build a deterministic RGB pattern with content on every edge."""
    pixels = bytearray()
    for y in range(HEIGHT):
        for x in range(WIDTH):
            pixels.extend(
                (
                    (x * 37 + y * 11 + (x * y) % 17) & 0xFF,
                    (x * 13 + y * 53 + (x * y) % 29) & 0xFF,
                    (x * 97 + y * 7 + (x * y) % 31) & 0xFF,
                )
            )
    result = bytes(pixels)
    if hashlib.sha256(result).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("AVX2 edge-batch RGB pixels differ from their pinned digest")
    return result


def generate_fixture() -> bytes:
    """Encode the pinned RGB samples as a Pillow 12.2.0 PNG."""
    from PIL import Image, __version__ as pillow_version

    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    image = Image.frombytes("RGB", (WIDTH, HEIGHT), source_pixels())
    output = BytesIO()
    image.save(output, format="PNG", compress_level=6)
    fixture = output.getvalue()
    if hashlib.sha256(fixture).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("AVX2 edge-batch source PNG differs from its pinned digest")
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
            raise RuntimeError(f"fixture is stale or missing: {OUTPUT.relative_to(ROOT)}")
        print(f"Verified {OUTPUT.relative_to(ROOT)}")
        return 0

    if OUTPUT.exists() and OUTPUT.read_bytes() != fixture:
        raise RuntimeError(f"refusing to replace a different fixture: {OUTPUT.relative_to(ROOT)}")
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(fixture)
    print(f"Wrote {OUTPUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
