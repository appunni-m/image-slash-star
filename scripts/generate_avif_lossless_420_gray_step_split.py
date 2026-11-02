#!/usr/bin/env python3
"""Generate a lossless 4:2:0 following-leaf chroma-skip parity fixture."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path

from PIL import Image
import yaml

from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
ASSET = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "portable_lossless_420_gray_step_split_16x8.avif"
)
EXPECTED_SHA256 = "387d206379f144e7904d4aaa2636c7b37e084109f5600b852734d5af003bbd3b"
SIZE = (16, 8)


def source_pixels() -> bytes:
    """Make two gray Square8 leaves with a luma-only step at their boundary."""
    return bytes(
        component
        for _y in range(SIZE[1])
        for x in range(SIZE[0])
        for component in ((64 if x < 8 else 128),) * 3
    )


def encode() -> bytes:
    """Encode the pinned source through Pillow's AVIF plugin."""
    image = Image.frombytes("RGB", SIZE, source_pixels())
    output = BytesIO()
    image.save(
        output,
        format="AVIF",
        quality=100,
        speed=8,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
    )
    return output.getvalue()


def verify_oracle(data: bytes) -> None:
    """Require Pillow to reconstruct every source pixel exactly."""
    with Image.open(BytesIO(data)) as image:
        image.load()
        if image.mode != "RGB" or image.size != SIZE:
            raise RuntimeError("Pillow changed the fixture's mode or dimensions")
        if image.tobytes() != source_pixels():
            raise RuntimeError("Pillow output differs from the grayscale step source")


def generate(*, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)

    data = encode()
    if data != encode():
        raise RuntimeError("AVIF fixture encoding is not deterministic")
    actual_sha256 = sha256(data).hexdigest()
    if actual_sha256 != EXPECTED_SHA256:
        raise RuntimeError(f"fixture hash changed: {actual_sha256}")
    verify_oracle(data)

    if check:
        if not ASSET.is_file() or ASSET.read_bytes() != data:
            raise RuntimeError(f"generated fixture is missing or stale: {ASSET}")
    elif ASSET.exists():
        if ASSET.read_bytes() != data:
            raise RuntimeError(f"refusing to replace a different fixture: {ASSET}")
    else:
        ASSET.write_bytes(data)

    print(f"{ASSET}: {len(data)} bytes, sha256={actual_sha256}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify the fixture")
    args = parser.parse_args()
    generate(check=args.check)


if __name__ == "__main__":
    main()
