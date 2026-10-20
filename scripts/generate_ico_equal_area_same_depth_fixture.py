#!/usr/bin/env python3
"""Generate equal-area, equal-depth ICO entries to pin stable first-entry ties."""

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
ASSET_PATH = Path("ico") / "equal_area_same_depth.ico"
EXPECTED_INPUT_SHA256 = "971907370044e3d15908854cba640abe0566ef55e15ac39beb4a7c835203cb36"
EXPECTED_PIXELS = bytes.fromhex("00ff00ff")
EXPECTED_PIXELS_SHA256 = "7a7bf454c5f3cb1b9d9a20f81417f98d976fe3b3dd52c1b9968f02e89e7e8a2f"


def dib_entry(bgr_pixel: bytes) -> bytes:
    """Build one complete 1x1 bottom-up 24-bit DIB with an AND row."""
    header = struct.pack(
        "<IiiHHIIiiII",
        40,
        1,
        2,
        1,
        24,
        0,
        4,
        0,
        0,
        0,
        0,
    )
    return header + bgr_pixel + b"\x00" + bytes(4)


def fixture_bytes() -> bytes:
    """Place green then red equal-area entries at the same declared depth."""
    payloads = (
        dib_entry(bytes((0, 255, 0))),
        dib_entry(bytes((0, 0, 255))),
    )
    directory_offset = 6 + len(payloads) * 16
    directory = []
    payload_offset = directory_offset
    for payload in payloads:
        directory.append(
            struct.pack(
                "<BBBBHHII",
                1,
                1,
                0,
                0,
                1,
                24,
                len(payload),
                payload_offset,
            )
        )
        payload_offset += len(payload)
    return struct.pack("<HHH", 0, 1, len(payloads)) + b"".join(directory) + b"".join(payloads)


def verify_pillow(data: bytes) -> None:
    """Require Pillow to keep the first of two equal-area, equal-depth entries."""
    if hashlib.sha256(data).hexdigest() != EXPECTED_INPUT_SHA256:
        raise RuntimeError("equal-area same-depth ICO fixture differs from the pinned input")
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
