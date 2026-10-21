#!/usr/bin/env python3
"""Generate and verify the deterministic 9x9 RGB16 Adam7 PNG fixture."""

from __future__ import annotations

import argparse
import hashlib
import struct
import zlib
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/fixtures/input/images/png/adam7_rgb16.png"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
WIDTH = 9
HEIGHT = 9
ADAM7_PASSES = (
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
)
EXPECTED_SOURCE_SHA256 = "243f253a436c595237e54fddb58f7b5b9fa5fbf07a18b35531399a821cb7a5dc"
EXPECTED_PIXELS_SHA256 = "fdd488e078dac07ca3813a65dce0146ee1b9fc2f451301d579356f95f512920e"


def samples() -> list[tuple[int, int, int]]:
    """Reuse the RGB16 channel pattern used by ``rgb16.png``."""
    return [
        (
            (x * 8191 + y * 257) & 0xFFFF,
            (x * 1021 + y * 4093) & 0xFFFF,
            (x * 509 + y * 1237) & 0xFFFF,
        )
        for y in range(HEIGHT)
        for x in range(WIDTH)
    ]


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Serialize a PNG chunk with a length and CRC for its payload."""
    if len(kind) != 4 or len(payload) > 0xFFFF_FFFF:
        raise ValueError("PNG chunk kind or payload exceeds its format limit")
    return (
        struct.pack(">I", len(payload))
        + kind
        + payload
        + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFF_FFFF)
    )


def build_fixture() -> bytes:
    """Encode all seven Adam7 passes with unfiltered 16-bit RGB rows."""
    pixels = samples()
    if len(pixels) != WIDTH * HEIGHT:
        raise RuntimeError("RGB16 sample count does not match the 9x9 raster")

    scanlines = bytearray()
    for x_start, y_start, x_step, y_step in ADAM7_PASSES:
        for y in range(y_start, HEIGHT, y_step):
            scanlines.append(0)
            for x in range(x_start, WIDTH, x_step):
                scanlines.extend(struct.pack(">HHH", *pixels[y * WIDTH + x]))

    ihdr = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 16, 2, 0, 0, 1)
    encoded = (
        PNG_SIGNATURE
        + png_chunk(b"IHDR", ihdr)
        + png_chunk(b"IDAT", zlib.compress(scanlines, level=6))
        + png_chunk(b"IEND", b"")
    )
    digest = hashlib.sha256(encoded).hexdigest()
    if EXPECTED_SOURCE_SHA256 and digest != EXPECTED_SOURCE_SHA256:
        raise RuntimeError(
            f"PNG source SHA-256 mismatch: expected {EXPECTED_SOURCE_SHA256}, found {digest}"
        )
    return encoded


def verify_pillow(data: bytes) -> None:
    """Check exact decoded mode, dimensions, and pixels with Pillow 12.2.0."""
    try:
        import PIL
        from PIL import Image
    except ImportError as error:
        raise RuntimeError("Pillow 12.2.0 is required for --verify-pillow") from error

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PIL.__version__}")

    with Image.open(BytesIO(data)) as image:
        image.verify()

    with Image.open(BytesIO(data)) as image:
        image.load()
        if image.format != "PNG" or image.mode != "RGB" or image.size != (WIDTH, HEIGHT):
            raise RuntimeError("Pillow decoded unexpected RGB16 Adam7 PNG metadata")
        digest = hashlib.sha256(image.tobytes()).hexdigest()
        if EXPECTED_PIXELS_SHA256 and digest != EXPECTED_PIXELS_SHA256:
            raise RuntimeError(
                "Pillow decoded pixels differ from their pinned hash: "
                f"expected {EXPECTED_PIXELS_SHA256}, found {digest}"
            )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    parser.add_argument(
        "--verify-pillow",
        action="store_true",
        help="verify the pinned Pillow 12.2.0 mode and pixels",
    )
    arguments = parser.parse_args()

    fixture = build_fixture()
    if arguments.verify_pillow or not arguments.check:
        verify_pillow(fixture)

    if arguments.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != fixture:
            raise RuntimeError(f"fixture is stale or missing: {OUTPUT.relative_to(ROOT)}")
        print(f"Verified {OUTPUT.relative_to(ROOT)}")
        return 0

    if OUTPUT.exists() and OUTPUT.read_bytes() != fixture:
        raise RuntimeError(f"refusing to replace different fixture: {OUTPUT.relative_to(ROOT)}")
    OUTPUT.write_bytes(fixture)
    print(f"Wrote {OUTPUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
