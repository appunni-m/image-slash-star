#!/usr/bin/env python3
"""Generate a WebP source whose alpha scan spans one checkpoint interval."""

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
    / "webp_segment_alpha_full_span_48x48.png"
)
WIDTH = 48
HEIGHT = 48
CHECKER_SIZE = 16
CHECKER_LOW = 88
CHECKER_HIGH = 173
EXPECTED_PIXELS_SHA256 = "cdd3f15fdf71cbd01937af97efb50745988548238cc086c345c0c4c1b9343e86"
EXPECTED_INPUT_SHA256 = "596f26ec405bc7d3f8751c140bcbce1d434af0d9495694afeea3b4407a4a53a1"


def fixture_pixels() -> bytes:
    """Align a mixed-texture frame's segment alpha range to 64 scan values."""
    image = Image.new("RGB", (WIDTH, HEIGHT), (128, 128, 128))
    pixels = image.load()
    for y in range(CHECKER_SIZE):
        for x in range(CHECKER_SIZE):
            value = CHECKER_HIGH if (x + y) & 1 else CHECKER_LOW
            pixels[x, y] = (value, value, value)

    result = image.tobytes()
    if hashlib.sha256(result).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("WebP segment-alpha pixels differ from their pinned digest")
    return result


def fixture_bytes() -> bytes:
    """Encode the deterministic RGB stimulus as a complete PNG source."""
    image = Image.frombytes("RGB", (WIDTH, HEIGHT), fixture_pixels())
    output = io.BytesIO()
    image.save(output, format="PNG", compress_level=9)
    result = output.getvalue()
    if hashlib.sha256(result).hexdigest() != EXPECTED_INPUT_SHA256:
        raise RuntimeError("WebP segment-alpha PNG differs from its pinned digest")
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
