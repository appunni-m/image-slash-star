#!/usr/bin/env python3
"""Generate the grayscale source for progressive JPEG correction-buffer parity."""

from __future__ import annotations

import argparse
import hashlib
import io
from pathlib import Path

import yaml
from PIL import Image

from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parents[1]
SOURCE_SIZE = (256, 256)
SOURCE_SHA256 = "a50cef6ddd8ace094ab3db04f0d205abbd8d6a70b2b618d93fb03e494c47eca9"
SOURCE_PATH = Path("png") / "jpeg_progressive_correction_source_256x256.png"


def source_pixels() -> bytes:
    """Repeat an eight-level grayscale row to produce dense progressive scans."""
    row = bytes(range(0, 256, 32)) * 32
    return row * SOURCE_SIZE[1]


def source_png() -> bytes:
    pixels = source_pixels()
    if len(pixels) != SOURCE_SIZE[0] * SOURCE_SIZE[1]:
        raise RuntimeError("progressive correction source pixel count changed")

    with Image.frombytes("L", SOURCE_SIZE, pixels) as image:
        buffer = io.BytesIO()
        image.save(buffer, format="PNG", compress_level=9)
    data = buffer.getvalue()
    if hashlib.sha256(data).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("progressive correction source PNG differs from its pinned hash")

    with Image.open(io.BytesIO(data)) as decoded:
        decoded.load()
        if decoded.mode != "L" or decoded.size != SOURCE_SIZE or decoded.tobytes() != pixels:
            raise RuntimeError("progressive correction PNG changed its source pixels")
    return data


def generate(output: Path, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text())
    verify_primary_oracle(manifest)

    destination = output / SOURCE_PATH
    data = source_png()
    if check:
        if not destination.is_file() or destination.read_bytes() != data:
            raise RuntimeError("complete generated input differs: " + str(destination))
    else:
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
    print(f"{'Checked' if check else 'Generated'} {destination}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "tests/fixtures/input/images")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    generate(args.output.resolve(), args.check)


if __name__ == "__main__":
    main()
