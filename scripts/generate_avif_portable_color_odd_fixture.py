#!/usr/bin/env python3
"""Generate an odd-sized lossless I420 AVIF parity fixture."""

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
ASSET_DIR = ROOT / "tests/fixtures/input/images/avif"
CASES = (
    (
        "portable_lossless_i420_odd_17x17.avif",
        (17, 17),
        "8b8ed2a65adb2dce764a2786b3d5c4332df59a00b0ba7e1dc3efc8a1e9036d1c",
        "5d83f59fc460b40ac4c154f4e2048344876fd4d7e13e9bd2123629693f4d4833",
    ),
    (
        "portable_lossless_i420_aligned_width_odd_height_16x17.avif",
        (16, 17),
        "fd52d7f651bd18556811b6f2c4200302454011f37e013f15bdfcbcddf1983904",
        "8ca59242477321cc92e2a718117d851c1666cec17e109fa130550e3caab62504",
    ),
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def sha256(data: bytes) -> str:
    return sha256_digest(data).hexdigest()


def source_pixels(size: tuple[int, int]) -> bytes:
    """Build fixed RGB samples with independent horizontal and vertical ramps."""
    return bytes(
        (x * 37 + y * 61 + channel * 89 + x * y * 3) & 0xFF
        for y in range(size[1])
        for x in range(size[0])
        for channel in range(3)
    )


def encode_once(size: tuple[int, int]) -> bytes:
    """Encode the deterministic RGB source with the pinned AV1 oracle."""
    image = Image.frombytes("RGB", size, source_pixels(size))
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


def verify_oracle(data: bytes, size: tuple[int, int], pixels_sha256: str) -> None:
    """Require exact Pillow output and an odd-sized lossless I420 key frame."""
    with Image.open(BytesIO(data)) as image:
        image.load()
        if image.format != "AVIF" or image.mode != "RGB" or image.size != size:
            raise RuntimeError("Pillow changed the odd-sized color fixture metadata")
        if sha256(image.tobytes()) != pixels_sha256:
            raise RuntimeError("Pillow output pixels changed for the color fixture")

    with tempfile.TemporaryDirectory(prefix="avif-color-odd-") as directory:
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
        or (header.get("frame_width"), header.get("frame_height")) != size
        for header in frame_headers
    ):
        raise RuntimeError("AV1 frame must be a lossless 17x17 key frame")


def generate(*, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    observed_oracle = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed_oracle != PINNED_ORACLE:
        raise RuntimeError(
            f"pinned Pillow/libavif/libaom stack required, found {observed_oracle!r}"
        )

    for asset_name, size, expected_asset_sha256, expected_pixels_sha256 in CASES:
        asset = ASSET_DIR / asset_name
        data = encode_once(size)
        if data != encode_once(size):
            raise RuntimeError(f"independent color AVIF encodes differ: {asset_name}")
        actual_sha256 = sha256(data)
        if actual_sha256 != expected_asset_sha256:
            raise RuntimeError(f"fixture hash changed for {asset_name}: {actual_sha256}")
        verify_oracle(data, size, expected_pixels_sha256)

        if check:
            if not asset.is_file() or asset.read_bytes() != data:
                raise RuntimeError(f"generated fixture is missing or stale: {asset}")
        elif asset.exists():
            if asset.read_bytes() != data:
                raise RuntimeError(f"refusing to replace a different fixture: {asset}")
        else:
            asset.write_bytes(data)

        print(f"{asset}: {len(data)} bytes, sha256={actual_sha256}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify the fixture")
    generate(check=parser.parse_args().check)


if __name__ == "__main__":
    main()
