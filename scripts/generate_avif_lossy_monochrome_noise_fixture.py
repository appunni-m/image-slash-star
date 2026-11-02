#!/usr/bin/env python3
"""Generate a CDEF-enabled lossy monochrome AVIF parity fixture."""

from __future__ import annotations

import argparse
from hashlib import sha256 as sha256_digest
from io import BytesIO
from pathlib import Path
import tempfile

from PIL import Image, __version__ as PILLOW_VERSION, _avif, features
import yaml

from generate_avif_monochrome_noise_fixture import SIZE, source_pixels
from generate_decode_refs import verify_primary_oracle
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
ASSET = (
    ROOT
    / "tests/fixtures/input/images/avif/portable_lossy_monochrome_noise_64x64.avif"
)
EXPECTED_ASSET_SHA256 = "e6bdd362737c7bdbbabd407bf162a80d3d54a4584ec323e2d96e18c4a4d26155"
EXPECTED_PIXELS_SHA256 = "ef5456d3204a3ac83ce0e6505113eb3190ad16930a8952fa555864bb247250cf"
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def sha256(data: bytes) -> str:
    return sha256_digest(data).hexdigest()


def encode_once() -> bytes:
    """Encode fixed high-entropy monochrome samples with CDEF enabled."""
    image = Image.frombytes("L", SIZE, source_pixels())
    output = BytesIO()
    image.save(
        output,
        format="AVIF",
        quality=70,
        speed=6,
        max_threads=1,
        subsampling="4:0:0",
        range="full",
        autotiling=False,
    )
    return output.getvalue()


def verify_oracle(data: bytes) -> None:
    """Pin Pillow pixels and require monochrome lossy AV1 with CDEF enabled."""
    with Image.open(BytesIO(data)) as image:
        image.load()
        if image.format != "AVIF" or image.mode != "RGB" or image.size != SIZE:
            raise RuntimeError("Pillow changed the lossy monochrome fixture metadata")
        if sha256(image.tobytes()) != EXPECTED_PIXELS_SHA256:
            raise RuntimeError("Pillow pixels differ from the pinned lossy reference")

    with tempfile.TemporaryDirectory(prefix="avif-lossy-monochrome-noise-") as directory:
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
        header.get("monochrome") is not True or header.get("enable_cdef") is not True
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence must declare monochrome coding with CDEF enabled")
    if not frame_headers or any(
        header.get("all_lossless") is not False
        or header.get("frame_type") != "key"
        or (header.get("frame_width"), header.get("frame_height")) != SIZE
        for header in frame_headers
    ):
        raise RuntimeError("AV1 frame must be a lossy 64x64 key frame")


def generate(*, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    observed_oracle = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed_oracle != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow/libavif/libaom stack required, found {observed_oracle!r}")

    data = encode_once()
    if data != encode_once():
        raise RuntimeError("independent lossy monochrome AVIF encodes differ")
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
