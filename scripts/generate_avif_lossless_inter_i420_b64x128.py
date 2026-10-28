#!/usr/bin/env python3
"""Generate or verify a pinned 8-bit lossless I420 B64x128 sequence."""

from __future__ import annotations

import argparse
from hashlib import sha256 as sha256_digest
import subprocess
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as PILLOW_VERSION, _avif, features

from generate_avif_multitile_motion_fixture import normalize_timestamps
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_lossless_inter_420_vertical_b64x128.avif"
)
EXPECTED_ASSET_SHA256 = "9ed8dd60feedcb8480bb8d6de7f8ea2ec23583da923e916e28a128c3746ca650"
EXPECTED_FRAME_SHA256 = (
    "184f26a78f3ccd248c492097a9efda76e13e755c6f83eeb5ec8c063ba845f0ac",
    "017090fd5800195e619cccee1cc806952d7d57c395865a0ae6bd2e8e89bf29c4",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def check_oracle() -> None:
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow/libavif/libaom stack required, found {observed!r}")


def validate_fixture(path: Path, encoded: bytes) -> dict[str, object]:
    asset_hash = sha256_digest(encoded).hexdigest()
    if asset_hash != EXPECTED_ASSET_SHA256:
        raise RuntimeError(f"fixture hash changed: {asset_hash}")

    report = inspect_obus(path)
    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if not sequence_headers or any(
        header.get("use_128x128_superblock") is not True
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence must use 128x128 superblocks")

    frame_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    if not any(header.get("frame_type") == "inter" for header in frame_headers):
        raise RuntimeError("AV1 sequence has no inter frame")
    if any(header.get("all_lossless") is not True for header in frame_headers):
        raise RuntimeError("AV1 sequence contains a frame that is not all-lossless")

    frame_hashes = []
    with Image.open(BytesIO(encoded)) as image:
        if (
            image.format != "AVIF"
            or image.mode != "RGB"
            or image.size != (128, 128)
            or image.n_frames != len(EXPECTED_FRAME_SHA256)
            or not image.is_animated
        ):
            raise RuntimeError("Pillow AVIF metadata differs from the pinned sequence")
        for frame_index in range(image.n_frames):
            image.seek(frame_index)
            image.load()
            if image.info.get("duration") != 100:
                raise RuntimeError("Pillow frame duration differs from the pinned sequence")
            frame_hashes.append(sha256_digest(image.convert("RGB").tobytes()).hexdigest())
    if tuple(frame_hashes) != EXPECTED_FRAME_SHA256:
        raise RuntimeError(f"Pillow frame hashes changed: {tuple(frame_hashes)!r}")

    return {
        "asset_bytes": len(encoded),
        "asset_sha256": asset_hash,
        "frame_hashes": frame_hashes,
    }


def encode_fixture(encoder: Path) -> bytes:
    with tempfile.TemporaryDirectory(prefix="avif-lossless-i420-b64x128-") as directory:
        normalized_runs = []
        observations = []
        for index in range(2):
            raw_path = Path(directory) / f"encoded-{index}.avif"
            normalized_path = Path(directory) / f"normalized-{index}.avif"
            subprocess.run([str(encoder), str(raw_path)], check=True, cwd=ROOT)
            raw = raw_path.read_bytes()
            normalized = normalize_timestamps(raw)
            normalized_path.write_bytes(normalized)
            normalized_runs.append(normalized)
            observations.append(validate_fixture(normalized_path, normalized))
        if normalized_runs[0] != normalized_runs[1]:
            raise RuntimeError("independent normalized B64x128 AVIF encodes differ")
        if observations[0] != observations[1]:
            raise RuntimeError("B64x128 AV1 or Pillow observations differ between encodes")
        return normalized_runs[0]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, help="fixture-only pinned AVIF encoder")
    parser.add_argument("--output", type=Path, help="override the default output path")
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed asset without rebuilding the fixture encoder",
    )
    args = parser.parse_args()

    check_oracle()
    output = (args.output or ASSET).resolve()
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
        encoded = encode_fixture(encoder)

    with tempfile.TemporaryDirectory(prefix="avif-b64x128-check-") as directory:
        check_path = Path(directory) / "fixture.avif"
        check_path.write_bytes(encoded)
        observation = validate_fixture(check_path, encoded)

    if not args.check:
        if output.exists() and output.read_bytes() != encoded:
            raise RuntimeError(f"refusing to replace differing fixture: {output}")
        output.parent.mkdir(parents=True, exist_ok=True)
        temporary_output = output.with_name(f".{output.name}.tmp")
        try:
            temporary_output.write_bytes(encoded)
            temporary_output.replace(output)
        finally:
            temporary_output.unlink(missing_ok=True)

    print(f"Verified {output}: {observation}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        raise SystemExit(f"error: {error}") from error
