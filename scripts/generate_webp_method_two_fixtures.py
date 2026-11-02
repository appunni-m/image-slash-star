#!/usr/bin/env python3
"""Generate input-only RGB PNGs for WebP method-two mode and skip decisions."""

from __future__ import annotations

import argparse
import hashlib
import struct
from pathlib import Path

from generate_webp_dimension_limit_fixture import PNG_SIGNATURE, png_chunk, stored_zlib


ROOT = Path(__file__).resolve().parent.parent
SPECS = (
    (
        "neutral_16x16",
        16,
        16,
        "cdb1eb5bdbc366ffdeb23f20ffb9baf2c8afb6c33b6080bfdfe52223dd4aa2d6",
        "f04d702e124c5ffe01e333f5916ed71240939824817a229c58a9582bb47b225e",
    ),
    (
        "neutral_17x19",
        17,
        19,
        "8adb08c96b955f3045ed5670c7c339fafdfe84d87313667af4887bc05fe79130",
        "4e102738c3b04e356d58aacbbd2649fa112bdf582058f7a3124ce0d8014307e6",
    ),
    (
        "sparse_64x64",
        64,
        64,
        "f60fc9a29cfc17b46d1d41b2a9444f57814923cf8e672f4b38381c59150f031a",
        "f0f0ca8e9c4d4dc3e5e94c4bb7537fff310e51b070948368ca6bf57e3ff7e9a4",
    ),
)


def generate(name: str, width: int, height: int) -> tuple[bytes, bytes]:
    """Construct source samples and a complete stored-DEFLATE PNG."""
    pixels = bytearray()
    for y in range(height):
        for x in range(width):
            rgb = (130, 130, 130)
            if name == "sparse_64x64" and x >= 48 and y >= 48:
                rgb = (
                    (31 * x + 17 * y) & 255,
                    (7 * x + 13 * y) & 255,
                    (19 * x + 23 * y) & 255,
                )
            pixels.extend(rgb)
    row_bytes = width * 3
    scanlines = b"".join(
        b"\x00" + pixels[y * row_bytes : (y + 1) * row_bytes]
        for y in range(height)
    )
    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    png = (
        PNG_SIGNATURE
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", stored_zlib(scanlines))
        + png_chunk(b"IEND", b"")
    )
    return bytes(pixels), png


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--check", action="store_true", help="verify without writing")
    action.add_argument("--write", action="store_true", help="write source PNGs")
    args = parser.parse_args()
    for name, width, height, pixels_sha256, source_sha256 in SPECS:
        pixels, png = generate(name, width, height)
        if hashlib.sha256(pixels).hexdigest() != pixels_sha256:
            raise RuntimeError(f"method-two source pixels changed: {name}")
        if hashlib.sha256(png).hexdigest() != source_sha256:
            raise RuntimeError(f"method-two source PNG changed: {name}")
        path = ROOT / "tests/fixtures/input/images/png" / f"webp_skip_{name}.png"
        if args.check:
            if not path.is_file() or path.read_bytes() != png:
                raise RuntimeError(f"stale or missing fixture: {path.relative_to(ROOT)}")
        else:
            if path.exists() and path.read_bytes() != png:
                raise RuntimeError(f"refusing to replace a different fixture: {path}")
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(png)
        print(f"{'Verified' if args.check else 'Wrote'} {path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
