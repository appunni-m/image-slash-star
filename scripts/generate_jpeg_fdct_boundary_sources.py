#!/usr/bin/env python3
"""Generate lossless sources for JPEG FDCT sample bounds and block tails."""
from __future__ import annotations

import argparse
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as pillow_version

ROOT = Path(__file__).resolve().parents[1]
SOURCES = (
    ("gray_zero", "L", 32, 8),
    ("gray_full", "L", 32, 8),
    ("rgb_checker", "RGB", 32, 8),
    ("rgb_checker_tail", "RGB", 33, 9),
    ("rgb_stripes", "RGB", 64, 16),
    ("cmyk_checker", "CMYK", 32, 8),
)


def source_pixels(name: str, mode: str, width: int, height: int) -> bytes:
    """Construct only input samples, independently of any JPEG output."""
    if name == "gray_zero":
        return bytes(width * height)
    if name == "gray_full":
        return bytes([255]) * (width * height)
    return bytes(
        255 if (x + (0 if name == "rgb_stripes" else y) + channel) % 2 else 0
        for y in range(height)
        for x in range(width)
        for channel in range(len(mode))
    )


def generate(check: bool) -> None:
    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    for name, mode, width, height in SOURCES:
        pixels = source_pixels(name, mode, width, height)
        fmt, extension = ("TIFF", "tiff") if mode == "CMYK" else ("PNG", "png")
        output = BytesIO()
        with Image.frombytes(mode, (width, height), pixels) as image:
            if fmt == "TIFF":
                image.save(output, format=fmt, compression="raw")
            else:
                image.save(output, format=fmt, compress_level=0)
        data = output.getvalue()
        with Image.open(BytesIO(data)) as image:
            image.load()
            if (image.mode, image.size, image.tobytes()) != (mode, (width, height), pixels):
                raise RuntimeError(f"source did not preserve samples: {name}")
        destination = ROOT / f"tests/fixtures/input/images/{extension}" / (
            f"jpeg_fdct_{name}_{width}x{height}.{extension}"
        )
        if check:
            if not destination.is_file() or destination.read_bytes() != data:
                raise RuntimeError(f"stale or missing fixture: {destination.relative_to(ROOT)}")
        else:
            if destination.exists() and destination.read_bytes() != data:
                raise RuntimeError(f"refusing to replace different fixture: {destination.relative_to(ROOT)}")
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
        print(f"{'Checked' if check else 'Generated'} {destination.relative_to(ROOT)}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    generate(parser.parse_args().check)


if __name__ == "__main__":
    main()
