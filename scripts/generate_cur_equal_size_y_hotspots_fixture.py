#!/usr/bin/env python3
"""Generate a CUR fixture proving equal-size entries keep directory order."""

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
ASSET_PATH = Path("ico") / "equal_size_y_hotspots.cur"
EXPECTED_INPUT_SHA256 = "49e7629bdf85ec5c1e27af798e430fd08650fdee5693ffdd3704ebe256b9a0e2"
HEIGHT = 11
EXPECTED_PIXELS = bytes.fromhex("00ff00" * HEIGHT)
EXPECTED_PIXELS_SHA256 = "13282cb75283192bda25b1e0f8688337bd77de15e28fbb9dd3c509b8775d0b80"


def dib_entry(bgr_pixel: bytes) -> bytes:
    """Build a complete 1x11, 24-bit bottom-up cursor DIB with its AND rows."""
    xor_row = bgr_pixel + b"\x00"
    header = struct.pack(
        "<IiiHHIIiiII",
        40,
        1,
        HEIGHT * 2,
        1,
        24,
        0,
        len(xor_row),
        0,
        0,
        0,
        0,
    )
    return header + xor_row * HEIGHT + bytes(4 * HEIGHT)


def fixture_bytes() -> bytes:
    """Place green first at y=10, then red at y=2, with equal 1x11 dimensions."""
    payloads = (
        dib_entry(bytes((0, 255, 0))),
        dib_entry(bytes((0, 0, 255))),
    )
    directory_offset = 6 + 2 * 16
    entries = []
    payload_offset = directory_offset
    for y_hotspot, payload in zip((10, 2), payloads, strict=True):
        entries.append(
            struct.pack(
                "<BBBBHHII",
                1,
                HEIGHT,
                0,
                0,
                0,
                y_hotspot,
                len(payload),
                payload_offset,
            )
        )
        payload_offset += len(payload)
    return struct.pack("<HHH", 0, 2, 2) + b"".join(entries) + b"".join(payloads)


def verify_pillow(data: bytes) -> None:
    """Require Pillow to keep and decode the first equal-size CUR entry."""
    if hashlib.sha256(data).hexdigest() != EXPECTED_INPUT_SHA256:
        raise RuntimeError("equal-size CUR fixture bytes differ from the pinned input")
    with Image.open(io.BytesIO(data)) as image:
        image.load()
        if image.format != "CUR" or image.mode != "RGB" or image.size != (1, HEIGHT):
            raise RuntimeError(
                f"Pillow returned unexpected CUR metadata: "
                f"{image.format!r} {image.mode!r} {image.size!r}"
            )
        pixels = image.tobytes()
    if pixels != EXPECTED_PIXELS:
        raise RuntimeError(f"Pillow selected unexpected pixels: {pixels.hex()}")
    if hashlib.sha256(pixels).hexdigest() != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("Pillow pixel bytes differ from the pinned RGB result")


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
