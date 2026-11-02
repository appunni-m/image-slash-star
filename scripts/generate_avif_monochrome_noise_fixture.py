#!/usr/bin/env python3
"""Generate a high-entropy lossless monochrome AVIF parity fixture."""

from __future__ import annotations

import argparse
from hashlib import sha256 as sha256_digest
from io import BytesIO
from pathlib import Path
import tempfile

from PIL import Image, __version__ as PILLOW_VERSION, _avif, features
import yaml

from generate_decode_refs import verify_primary_oracle
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
ASSET = (
    ROOT
    / "tests/fixtures/input/images/avif/portable_lossless_monochrome_noise_64x64.avif"
)
SIZE = (64, 64)
EXPECTED_ASSET_SHA256 = "508ad1f423c0bfa66c3983fb4dd198017cca9d5c2fc426be4b87d037b0a0dd28"
EXPECTED_PIXELS_SHA256 = "3dc9c981fa8cc6536af6f278e7e5f1a738d649c03c4de9a9a3c0a778ce09797e"
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def sha256(data: bytes) -> str:
    return sha256_digest(data).hexdigest()


def source_pixels() -> bytes:
    """Build fixed high-entropy luma samples with a 32-bit xorshift stream."""
    state = 0x6D6F6E6F
    pixels = bytearray()
    for _ in range(SIZE[0] * SIZE[1]):
        state ^= (state << 13) & 0xFFFFFFFF
        state ^= state >> 17
        state ^= (state << 5) & 0xFFFFFFFF
        state &= 0xFFFFFFFF
        pixels.append(state >> 24)
    return bytes(pixels)


def encode_once() -> bytes:
    """Encode the deterministic grayscale source with the pinned AV1 oracle."""
    image = Image.frombytes("L", SIZE, source_pixels())
    output = BytesIO()
    image.save(
        output,
        format="AVIF",
        quality=100,
        speed=6,
        max_threads=1,
        subsampling="4:0:0",
        range="full",
        autotiling=False,
    )
    return output.getvalue()


def verify_oracle(data: bytes) -> None:
    """Require exact Pillow pixels and monochrome all-lossless AV1 syntax."""
    with Image.open(BytesIO(data)) as image:
        image.load()
        if image.format != "AVIF" or image.mode != "RGB" or image.size != SIZE:
            raise RuntimeError("Pillow changed the monochrome fixture's output metadata")
        if image.convert("L").tobytes() != source_pixels():
            raise RuntimeError("Pillow output differs from the exact grayscale source")

    with tempfile.TemporaryDirectory(prefix="avif-monochrome-noise-") as directory:
        path = Path(directory) / "fixture.avif"
        path.write_bytes(data)
        report = inspect_obus(path)

    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    frame_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    if not sequence_headers or any(
        header.get("monochrome") is not True for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence must declare monochrome coding")
    if not frame_headers or any(
        header.get("all_lossless") is not True
        or (header.get("frame_width"), header.get("frame_height")) != SIZE
        for header in frame_headers
    ):
        raise RuntimeError("AV1 key frame must be all-lossless at 64x64")


def generate(*, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    observed_oracle = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed_oracle != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow/libavif/libaom stack required, found {observed_oracle!r}")

    pixels = source_pixels()
    if sha256(pixels) != EXPECTED_PIXELS_SHA256:
        raise RuntimeError("deterministic monochrome source pixels changed")
    data = encode_once()
    if data != encode_once():
        raise RuntimeError("independent monochrome AVIF encodes differ")
    actual_sha256 = sha256(data)
    if actual_sha256 != EXPECTED_ASSET_SHA256:
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
