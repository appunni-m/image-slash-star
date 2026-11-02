#!/usr/bin/env python3
"""Generate a lossless CMYK source with varied K samples and odd JPEG edges."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as pillow_version


ROOT = Path(__file__).resolve().parents[1]
SOURCES = (
    (17, 17,
     "6e69f4c9a572f1c4a778685c9e10b7ddc9b60d5b3651057acbe43d5f6a603fda",
     "d15b9f35cf22f3198e8c2988f3ff2465ba2cd7f347ce41a0b1d58f4ae7e79364"),
    (1, 9,
     "b0470cfa5718a49bf056d2b62b9ac331735550b0cdad6812a02a7d3d3e0666c0",
     "8f1cbd5ccd5044b678ae8c9a0610caf5b87ad7df754921b114b8f86a7bfe2161"),
    (3, 9,
     "410b1bccad7fd37b76fa14facace884930cb5adf14b43b41aacf9b3cad46e6f9",
     "e8f0fba0d1c522bed80d24172b43192a0279bed4e466908cada12fab63d486b0"),
)


def source_pixels(width: int, height: int, expected_sha256: str) -> bytes:
    """Generate the production JPEG benchmark's xorshift32 CMYK samples."""
    state = 0x12345678
    pixels = bytearray(width * height * 4)
    for index in range(len(pixels)):
        state ^= (state << 13) & 0xFFFFFFFF
        state ^= state >> 17
        state ^= (state << 5) & 0xFFFFFFFF
        pixels[index] = state & 0xFF
    result = bytes(pixels)
    if hashlib.sha256(result).hexdigest() != expected_sha256:
        raise RuntimeError("CMYK subsampling source pixels changed")
    if len(set(result[3::4])) < 2:
        raise RuntimeError("the K channel must exercise filtering")
    return result


def generate(check: bool) -> None:
    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    for width, height, pixels_sha256, source_sha256 in SOURCES:
        pixels = source_pixels(width, height, pixels_sha256)
        output = BytesIO()
        with Image.frombytes("CMYK", (width, height), pixels) as image:
            image.save(output, format="TIFF", compression="raw")
        data = output.getvalue()
        if hashlib.sha256(data).hexdigest() != source_sha256:
            raise RuntimeError("CMYK subsampling source TIFF changed")
        with Image.open(BytesIO(data)) as image:
            image.load()
            if image.mode != "CMYK" or image.size != (width, height) or image.tobytes() != pixels:
                raise RuntimeError("TIFF did not preserve the complete CMYK source")
        destination = ROOT / "tests/fixtures/input/images/tiff" / (
            f"jpeg_cmyk_subsampling_source_{width}x{height}.tiff"
        )
        if check:
            if not destination.is_file() or destination.read_bytes() != data:
                raise RuntimeError(f"fixture is stale or missing: {destination.relative_to(ROOT)}")
        else:
            if destination.exists() and destination.read_bytes() != data:
                raise RuntimeError(f"refusing to replace a different fixture: {destination.relative_to(ROOT)}")
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
        print(f"{'Checked' if check else 'Generated'} {destination.relative_to(ROOT)}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    args = parser.parse_args()
    generate(args.check)


if __name__ == "__main__":
    main()
