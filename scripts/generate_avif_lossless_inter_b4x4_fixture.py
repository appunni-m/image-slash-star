#!/usr/bin/env python3
"""Generate a pinned all-lossless I420 sequence with 4x4 AV1 partitions."""

from __future__ import annotations

import argparse
from hashlib import sha256 as sha256_digest
from io import BytesIO
from pathlib import Path
import subprocess
import tempfile

from PIL import Image, __version__ as PILLOW_VERSION, _avif, features

from generate_avif_multitile_motion_fixture import normalize_timestamps
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_lossless_inter_420_b4x4_leaves_64x64.avif"
)
EXPECTED_ASSET_SHA256 = "8c588829bd49a125fee501c9cdcaa1e6f9c0e6f058084270eaa039688acc9748"
EXPECTED_FRAME_SHA256 = (
    "09655c56751ad6a6586d91c51aefc59d335a666f7c531611b7655e348dfba0e7",
    "46fc9a9783dbd4610a95ef439de9fe286b265d71a459be3835d56c9d90cfa2b5",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def check_oracle() -> None:
    """Require the exact Pillow AVIF encoder used by repository fixtures."""
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow/libavif/libaom stack required, found {observed!r}")


def encode_once(encoder: Path) -> bytes:
    """Run the pinned fixture encoder and normalize movie timestamps."""
    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="avif-lossless-inter-b4x4-", dir=STAGING_ROOT
    ) as directory:
        raw_path = Path(directory) / "encoded.avif"
        subprocess.run([str(encoder), str(raw_path)], check=True, cwd=ROOT)
        return normalize_timestamps(raw_path.read_bytes())


def validate_pixels(encoded: bytes) -> None:
    """Require the exact pinned Pillow metadata, durations, and frame pixels."""
    with Image.open(BytesIO(encoded)) as image:
        if (
            image.format != "AVIF"
            or image.mode != "RGB"
            or image.size != (64, 64)
            or image.n_frames != len(EXPECTED_FRAME_SHA256)
            or not image.is_animated
        ):
            raise RuntimeError("Pillow AVIF metadata differs from the pinned sequence")
        for frame_index, expected_hash in enumerate(EXPECTED_FRAME_SHA256):
            image.seek(frame_index)
            image.load()
            if (
                image.info.get("duration") != 100
                or sha256_digest(image.tobytes()).hexdigest() != expected_hash
            ):
                raise RuntimeError(f"Pillow frame {frame_index} differs from its reference")


def validate_frame_headers(encoded: bytes) -> None:
    """Require one all-lossless key frame followed by one all-lossless inter frame."""
    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="avif-lossless-inter-b4x4-", dir=STAGING_ROOT
    ) as directory:
        path = Path(directory) / "fixture.avif"
        path.write_bytes(encoded)
        report = inspect_obus(path)

    samples = [sample for sample in report["samples"] if sample.get("role") == "track_pict"]
    if len(samples) != 2:
        raise RuntimeError("AVIF must contain exactly two picture-track samples")
    headers = [
        obu["frame_header"]
        for sample in samples
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    if (
        len(headers) != 2
        or [header.get("frame_type") for header in headers] != ["key", "inter"]
        or any(header.get("all_lossless") is not True for header in headers)
        or any(
            (header.get("frame_width"), header.get("frame_height")) != (64, 64)
            for header in headers
        )
    ):
        raise RuntimeError("AV1 frames differ from the pinned all-lossless sequence")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, help="pinned fixture-only AVIF encoder")
    parser.add_argument("--output", type=Path, default=ASSET)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the fixture against its pinned hash, Pillow output, and AV1 headers",
    )
    args = parser.parse_args()

    check_oracle()
    output = args.output.resolve()
    if output != ASSET.resolve() and STAGING_ROOT.resolve() not in output.parents:
        raise RuntimeError("output must be the fixture path or a staging path")
    if args.check:
        if not output.is_file():
            raise RuntimeError(f"fixture does not exist: {output}")
        encoded = output.read_bytes()
    else:
        if args.encoder is None:
            parser.error("--encoder is required when generating a fixture")
        encoder = args.encoder.resolve()
        if not encoder.is_file():
            raise RuntimeError(f"fixture encoder does not exist: {encoder}")
        encoded = encode_once(encoder)
        if encode_once(encoder) != encoded:
            raise RuntimeError("repeated AVIF encodes differ after timestamp normalization")
    asset_hash = sha256_digest(encoded).hexdigest()
    if asset_hash != EXPECTED_ASSET_SHA256:
        raise RuntimeError(f"fixture hash changed: {asset_hash}")
    validate_pixels(encoded)
    validate_frame_headers(encoded)

    if args.check:
        if output.read_bytes() != encoded:
            raise RuntimeError(f"fixture differs from deterministic output: {output}")
        print(f"Verified {output} ({len(encoded)} bytes, sha256={asset_hash})")
        return
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary_output = output.with_name(f".{output.name}.tmp")
    try:
        temporary_output.write_bytes(encoded)
        temporary_output.replace(output)
    finally:
        temporary_output.unlink(missing_ok=True)
    print(f"Wrote {output} ({len(encoded)} bytes, sha256={asset_hash})")


if __name__ == "__main__":
    main()
