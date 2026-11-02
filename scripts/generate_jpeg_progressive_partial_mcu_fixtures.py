#!/usr/bin/env python3
"""Generate varied RGB sources for progressive JPEG partial-MCU parity."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as pillow_version


ROOT = Path(__file__).resolve().parents[1]
PIXELS_SHA256 = "c29b38b57a3d52956a666ba8fda6055f8139f262209e671b414a9f991583bda2"
SOURCES = (
    (63, 65, "d58e4febf4ef7583141a0f3361191105a4a13f96ec39c140f5b5502ca10efa12"),
    (65, 63, "8bf78a951adcc00bcc38382199ee932cf219b78050a7e36284d10e21cc2802b3"),
)


def source_pixels(width: int, height: int) -> bytes:
    """Use the production JPEG benchmark's deterministic xorshift32 samples."""
    state = 0x12345678
    pixels = bytearray(width * height * 3)
    for index in range(len(pixels)):
        state ^= (state << 13) & 0xFFFFFFFF
        state ^= state >> 17
        state ^= (state << 5) & 0xFFFFFFFF
        pixels[index] = state & 0xFF
    result = bytes(pixels)
    if hashlib.sha256(result).hexdigest() != PIXELS_SHA256:
        raise RuntimeError("progressive partial-MCU source pixels changed")
    return result


def generate(check: bool) -> None:
    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    for width, height, expected_sha256 in SOURCES:
        pixels = source_pixels(width, height)
        output = BytesIO()
        with Image.frombytes("RGB", (width, height), pixels) as image:
            image.save(output, format="PNG", compress_level=9)
        data = output.getvalue()
        if hashlib.sha256(data).hexdigest() != expected_sha256:
            raise RuntimeError(f"progressive partial-MCU {width}x{height} PNG changed")
        with Image.open(BytesIO(data)) as image:
            image.load()
            if image.mode != "RGB" or image.size != (width, height) or image.tobytes() != pixels:
                raise RuntimeError("PNG did not preserve the complete RGB source")
        path = ROOT / "tests/fixtures/input/images/png" / (
            f"jpeg_progressive_partial_mcu_source_{width}x{height}.png"
        )
        if check:
            if not path.is_file() or path.read_bytes() != data:
                raise RuntimeError(f"fixture is stale or missing: {path.relative_to(ROOT)}")
        else:
            if path.exists() and path.read_bytes() != data:
                raise RuntimeError(f"refusing to replace a different fixture: {path.relative_to(ROOT)}")
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        print(f"{'Checked' if check else 'Generated'} {path.relative_to(ROOT)}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    arguments = parser.parse_args()
    generate(arguments.check)


if __name__ == "__main__":
    main()
