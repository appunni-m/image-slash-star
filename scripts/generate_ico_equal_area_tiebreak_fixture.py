#!/usr/bin/env python3
"""Generate the equal-area ICO entries used to pin Pillow's depth tie-break."""

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
ASSET_PATH = Path("ico") / "equal_area_bit_depth_tiebreak.ico"
EXPECTED_INPUT_SHA256 = "99f2da93f172c14fb4dd2fe78b6c25e2f5194fd57cb9fc7f798f485241443a82"
EXPECTED_PIXELS = bytes.fromhex("ff0000ff")
EXPECTED_PIXELS_SHA256 = "34aaa746c25a0f105c4316bbb1f009aa359f49582656ee97d73c58132d563423"


def dib_entry(bit_count: int, xor_row: bytes) -> bytes:
    """Build one complete 1x1 bottom-up DIB, including its 1-bit AND row."""
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
    return header + xor_row + bytes(4)


def fixture_bytes() -> bytes:
    """Place opaque green 32-bit first and opaque red 24-bit second."""
    payloads = (
        dib_entry(32, bytes((0, 255, 0, 255))),
        dib_entry(24, bytes((0, 0, 255, 0))),
    )
    directory_offset = 6 + 2 * 16
    entries = []
    payload_offset = directory_offset
    for bit_count, payload in zip((32, 24), payloads, strict=True):
        entries.append(
            struct.pack(
                "<BBBBHHII",
                1,
                1,
                0,
                0,
                1,
                bit_count,
                len(payload),
                payload_offset,
            )
        )
        payload_offset += len(payload)
    return struct.pack("<HHH", 0, 1, 2) + b"".join(entries) + b"".join(payloads)


def verify_pillow(data: bytes) -> None:
    """Require the pinned public ICO loader to select the later red entry."""
    if hashlib.sha256(data).hexdigest() != EXPECTED_INPUT_SHA256:
        raise RuntimeError("equal-area ICO fixture bytes differ from the pinned input")
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
        raise RuntimeError("Pillow pixel bytes differ from the pinned RGBA result")


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
