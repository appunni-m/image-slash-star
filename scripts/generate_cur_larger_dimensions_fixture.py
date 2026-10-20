#!/usr/bin/env python3
"""Generate a CUR fixture proving selection requires both dimensions to grow."""

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
ASSET_PATH = Path("ico") / "larger_dimensions.cur"
EXPECTED_INPUT_SHA256 = "9119f0d91fa3a5a5da28904cbbfb10a6ae4eb4eb43ea446881308749a564ff40"
EXPECTED_PIXELS = bytes.fromhex("ff0000" * 4)
EXPECTED_PIXELS_SHA256 = "fcf067f6323bb42b8292a565a8f826ec5fdb1b142b7a69bf7f7721f0d5d46ef8"


def dib_entry(width: int, height: int, bgr_pixel: bytes) -> bytes:
    """Build a complete bottom-up 24-bit DIB and its one-bit AND mask."""
    xor_stride = (width * 3 + 3) & ~3
    xor_row = bgr_pixel * width + bytes(xor_stride - width * 3)
    and_stride = ((width + 31) // 32) * 4
    header = struct.pack(
        "<IiiHHIIiiII",
        40,
        width,
        height * 2,
        1,
        24,
        0,
        len(xor_row) * height,
        0,
        0,
        0,
        0,
    )
    return header + xor_row * height + bytes(and_stride * height)


def fixture_bytes() -> bytes:
    """Place 1x1 green first and 2x2 red second with in-bounds hotspots."""
    entries = (
        (1, 1, 0, 0, dib_entry(1, 1, bytes((0, 255, 0)))),
        (2, 2, 1, 1, dib_entry(2, 2, bytes((0, 0, 255)))),
    )
    directory_offset = 6 + len(entries) * 16
    directory = []
    payload_offset = directory_offset
    for width, height, x_hotspot, y_hotspot, payload in entries:
        directory.append(
            struct.pack(
                "<BBBBHHII",
                width,
                height,
                0,
                0,
                x_hotspot,
                y_hotspot,
                len(payload),
                payload_offset,
            )
        )
        payload_offset += len(payload)
    return (
        struct.pack("<HHH", 0, 2, len(entries))
        + b"".join(directory)
        + b"".join(entry[4] for entry in entries)
    )


def verify_pillow(data: bytes) -> None:
    """Require Pillow to select and decode the later 2x2 CUR entry."""
    if hashlib.sha256(data).hexdigest() != EXPECTED_INPUT_SHA256:
        raise RuntimeError("larger-dimensions CUR fixture differs from the pinned input")
    with Image.open(io.BytesIO(data)) as image:
        image.load()
        if image.format != "CUR" or image.mode != "RGB" or image.size != (2, 2):
            raise RuntimeError(
                f"Pillow returned unexpected CUR metadata: "
                f"{image.format!r} {image.mode!r} {image.size!r}"
            )
        pixels = image.tobytes()
    if pixels != EXPECTED_PIXELS:
        raise RuntimeError(f"Pillow selected unexpected pixels: {pixels.hex()}")
    if hashlib.sha256(pixels).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("Pillow pixels differ from the pinned RGB result")


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
