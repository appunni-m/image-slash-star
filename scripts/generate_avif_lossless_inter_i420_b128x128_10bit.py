#!/usr/bin/env python3
"""Generate or verify a pinned 10-bit I420 lossless B128 AVIF sequence."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import tempfile
from pathlib import Path

from PIL import __version__ as PILLOW_VERSION, _avif, features

from generate_avif_10bit_lossless_inter_fixture import (
    PINNED_PILLOW,
    normalize_sequence_timestamps,
    pillow_frames,
    validate_bitstream,
    validate_timestamp_repeatability,
)
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_lossless_inter_420_b128x128_10bit.avif"
)
EXPECTED_ASSET_SHA256 = "a1056a8b40154eab74c1571cce2c39832e18a37de4b8bd1a3f808c6df231e851"
EXPECTED_FRAME_SHA256 = (
    "f5272a49a30f79b7cfc18b388fb26bd1e2a153718bb196df5880c2bc7da852b9",
    "9c2fd1e299098be30bc4664475977dbf13d9f870c30830cc5bb9314de762d3f8",
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_oracle() -> None:
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"Pillow/native version differs from pin: {observed!r}")


def validate_128x128_superblock(path: Path) -> None:
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


def validate_fixture(path: Path, encoded: bytes) -> dict[str, object]:
    file_hash = sha256(encoded)
    if file_hash != EXPECTED_ASSET_SHA256:
        raise RuntimeError(f"fixture hash changed: {file_hash}")
    obu_sample_hashes = validate_bitstream(path, 128, 128, 10, monochrome=False)
    validate_128x128_superblock(path)
    frames = pillow_frames(path, 128, 128, EXPECTED_FRAME_SHA256)
    return {
        "asset_bytes": len(encoded),
        "asset_sha256": file_hash,
        "obu_sample_hashes": [
            {"sample": sample, "sha256": digest}
            for sample, digest in obu_sample_hashes
        ],
        "frames": frames,
    }


def encode_fixture(encoder: Path) -> bytes:
    with tempfile.TemporaryDirectory(prefix="avif-lossless-i420-b128x128-") as directory:
        raw_paths = [Path(directory) / f"encoded-{index}.avif" for index in range(2)]
        normalized_paths = [
            Path(directory) / f"normalized-{index}.avif" for index in range(2)
        ]
        raw_runs = []
        normalized_runs = []
        observations = []
        for raw_path, normalized_path in zip(raw_paths, normalized_paths, strict=True):
            subprocess.run([str(encoder), str(raw_path)], check=True, cwd=ROOT)
            raw = raw_path.read_bytes()
            normalized = normalize_sequence_timestamps(raw)
            normalized_path.write_bytes(normalized)
            raw_runs.append(raw)
            normalized_runs.append(normalized)
            observations.append(validate_fixture(normalized_path, normalized))

        validate_timestamp_repeatability(raw_runs[0], raw_runs[1])
        if normalized_runs[0] != normalized_runs[1]:
            raise RuntimeError("independent normalized B128 AVIF encodes differ")
        if observations[0] != observations[1]:
            raise RuntimeError("B128 AV1 or Pillow observations differ between encodes")
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

    validate_oracle()
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

    with tempfile.TemporaryDirectory(prefix="avif-b128x128-check-") as directory:
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
