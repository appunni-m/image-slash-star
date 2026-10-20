#!/usr/bin/env python3
"""Generate an ICO that pins Pillow's zero-bit-count depth fallbacks."""

from __future__ import annotations

import argparse
import hashlib
import io
import struct
from pathlib import Path

import yaml
from PIL import Image

from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parents[1]
ASSET_PATH = Path("ico") / "zero_bpp_depth_fallbacks.ico"
EXPECTED_INPUT_SHA256 = "6d7fa6c19913b0f124338fc4afb0c5e841730cf05e20e3b6dd88ccb6943b4651"
EXPECTED_PIXELS = bytes.fromhex("00ff00ff")
EXPECTED_PIXELS_SHA256 = "7a7bf454c5f3cb1b9d9a20f81417f98d976fe3b3dd52c1b9968f02e89e7e8a2f"


def dib_entry(bit_count: int, xor_row: bytes, palette: bytes = b"") -> bytes:
    """Build a complete 1x1 bottom-up DIB with an optional RGB palette."""
    header = struct.pack(
        "<IiiHHIIiiII",
        40,
        1,
        2,
        1,
        bit_count,
        0,
        len(xor_row),
        0,
        0,
        0,
        0,
    )
    return header + palette + xor_row + bytes(4)


def fixture_bytes() -> bytes:
    """Place 24-bit red, palette-derived 1-bit green, then fallback blue."""
    payloads = (
        dib_entry(24, bytes((0, 0, 255, 0))),
        dib_entry(
            1,
            bytes((0x80, 0, 0, 0)),
            palette=bytes((0, 0, 0, 0, 0, 255, 0, 0)),
        ),
        dib_entry(32, bytes((255, 0, 0, 255))),
    )
    palette_colors = (0, 2, 0)
    bit_counts = (24, 0, 0)
    directory_offset = 6 + len(payloads) * 16
    entries = []
    payload_offset = directory_offset
    for colors, bit_count, payload in zip(
        palette_colors, bit_counts, payloads, strict=True
    ):
        entries.append(
            struct.pack(
                "<BBBBHHII",
                1,
                1,
                colors,
                0,
                1,
                bit_count,
                len(payload),
                payload_offset,
            )
        )
        payload_offset += len(payload)
    return struct.pack("<HHH", 0, 1, len(payloads)) + b"".join(entries) + b"".join(payloads)


def verify_pillow(data: bytes) -> None:
    """Require Pillow to select the palette-derived 1-bit entry."""
    if hashlib.sha256(data).hexdigest() != EXPECTED_INPUT_SHA256:
        raise RuntimeError("zero-bit-count ICO fixture differs from the pinned input")
    with Image.open(io.BytesIO(data)) as image:
        image.load()
        if image.format != "ICO" or image.mode != "RGBA" or image.size != (1, 1):
            raise RuntimeError(
                f"Pillow returned unexpected ICO metadata: "
                f"{image.format!r} {image.mode!r} {image.size!r}"
            )
        pixels = image.tobytes()
    if pixels != EXPECTED_PIXELS:
        raise RuntimeError(f"Pillow selected unexpected pixels: {pixels.hex()}")
    if hashlib.sha256(pixels).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("Pillow pixels differ from the pinned RGBA result")


def generate(destination: Path, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    data = fixture_bytes()
    verify_pillow(data)

    if check:
        if not destination.is_file() or destination.read_bytes() != data:
            raise RuntimeError(f"complete generated input differs: {destination}")
    else:
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
    print(f"{'Checked' if check else 'Generated'} {destination}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "tests" / "fixtures" / "input" / "images",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    generate(args.output.resolve() / ASSET_PATH, args.check)


if __name__ == "__main__":
    main()
