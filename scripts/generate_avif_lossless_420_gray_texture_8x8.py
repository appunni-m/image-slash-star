#!/usr/bin/env python3
"""Generate an 8x8 lossless I420 grayscale-texture parity fixture."""

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
ASSET = ROOT / "tests/fixtures/input/images/avif/portable_lossless_420_gray_texture_8x8.avif"
EXPECTED_ASSET_SHA256 = "4d4caeb830d6775b0c82172b60cacae07b0945a419cecb44680394f4337eef65"
EXPECTED_PIXELS_SHA256 = "6596205a5d46da184e167b8f3b5eb0aa586091b0e83f7368b79f3992a4786242"
SIZE = (8, 8)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def sha256(data: bytes) -> str:
    return sha256_digest(data).hexdigest()


def source_pixels() -> bytes:
    """Build a deterministic luma texture with neutral 4:2:0 chroma."""
    return bytes(
        level
        for y in range(SIZE[1])
        for x in range(SIZE[0])
        for level in (((x * 29 + y * 43 + x * y * 11 + 17) & 0xFF,) * 3)
    )


def encode_once() -> bytes:
    """Encode the source with the pinned Pillow/libavif AV1 implementation."""
    image = Image.frombytes("RGB", SIZE, source_pixels())
    output = BytesIO()
    image.save(
        output,
        format="AVIF",
        quality=100,
        speed=6,
        max_threads=1,
        subsampling="4:2:0",
        range="full",
        autotiling=False,
    )
    return output.getvalue()


def verify_oracle(data: bytes) -> None:
    """Require exact Pillow pixels and an all-lossless 8-bit I420 key frame."""
    with Image.open(BytesIO(data)) as image:
        image.load()
        if image.format != "AVIF" or image.mode != "RGB" or image.size != SIZE:
            raise RuntimeError("Pillow changed the fixture format, mode, or dimensions")
        if sha256(image.tobytes()) != EXPECTED_PIXELS_SHA256:
            raise RuntimeError("Pillow output pixels differ from the grayscale source")

    staging = ROOT / "target" / "oracle-staging"
    staging.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="avif-gray-texture-", dir=staging) as directory:
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
        header.get("bit_depth") != 8
        or header.get("monochrome") is not False
        or header.get("subsampling_x") != 1
        or header.get("subsampling_y") != 1
        or header.get("color_range") != 1
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence must declare full-range 8-bit I420 coding")
    if not frame_headers or any(
        header.get("all_lossless") is not True
        or header.get("frame_type") != "key"
        or (header.get("frame_width"), header.get("frame_height")) != SIZE
        for header in frame_headers
    ):
        raise RuntimeError("AV1 frame must be an all-lossless 8x8 key frame")


def generate(*, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow/libavif/libaom stack required, found {observed!r}")

    data = encode_once()
    if encode_once() != data:
        raise RuntimeError("repeated AVIF encodes differ")
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
    generate(check=parser.parse_args().check)


if __name__ == "__main__":
    main()
