#!/usr/bin/env python3
"""Generate a pinned I420 animation using the reduced inter-transform alphabet."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image

from generate_avif_10bit_lossless_inter_fixture import normalize_sequence_timestamps
from generate_avif_inter_i420_b16x64_mode_select import (
    validate_frame_headers,
    validate_oracle,
)
from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parent.parent
ASSET = (
    ROOT / "tests/fixtures/input/images/avif"
    / "animated_lossy_inter_420_b16x64_reduced_transform_set_64x64.avif"
)
EXPECTED_SHA256 = "3a52bdb0573571d5b9d6c5f79cbf4f41816fa84e4ec052034a9e418b9797511e"
EXPECTED_FRAMES = (
    "c7c4037bad1feda75d7692d103bc1f8982ed34e0e0ed6e2cbb391f626b59c3d4",
    "32863c207860b8ba57ecb2e8adefa3c6a50ccbc1c0139c85a775e7db4b7b5ff3",
)


def validate(encoded: bytes) -> None:
    """Verify complete bytes, reduced-set headers, timing, and live Pillow pixels."""
    if hashlib.sha256(encoded).hexdigest() != EXPECTED_SHA256:
        raise RuntimeError("reduced-transform-set AVIF bytes differ from their pin")
    validate_frame_headers(encoded)
    staging = ROOT / "target/oracle-staging"
    staging.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="reduced-tx-headers-", dir=staging) as directory:
        path = Path(directory) / "fixture.avif"
        path.write_bytes(encoded)
        samples = [
            sample for sample in inspect(path)["samples"]
            if sample.get("role") == "track_pict"
        ]
    headers = [
        obu["frame_header"] for sample in samples for obu in sample["obus"]
        if "frame_header" in obu
    ]
    if len(headers) != 2 or any(
        header.get("reduced_transform_set") is not True for header in headers
    ):
        raise RuntimeError("both picture frames must signal the reduced transform set")
    with Image.open(BytesIO(encoded)) as image:
        if (image.format, image.size, image.mode, image.n_frames) != (
            "AVIF", (64, 64), "RGB", 2
        ):
            raise RuntimeError("Pillow metadata differs from the pinned sequence")
        for index, expected in enumerate(EXPECTED_FRAMES):
            image.seek(index)
            image.load()
            pixels = image.tobytes()
            if (
                len(pixels) != 64 * 64 * 3
                or hashlib.sha256(pixels).hexdigest() != expected
                or image.info.get("duration") != 100
            ):
                raise RuntimeError(f"Pillow frame {index} differs from its pinned reference")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, help="pinned fixture-only B16x64 encoder")
    parser.add_argument("--check", action="store_true", help="verify the committed asset")
    args = parser.parse_args()
    validate_oracle()
    if args.check:
        encoded = ASSET.read_bytes()
    else:
        if args.encoder is None:
            parser.error("--encoder is required when generating a fixture")
        encoder = args.encoder.resolve()
        staging = ROOT / "target/oracle-staging"
        staging.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="reduced-tx-encode-", dir=staging) as directory:
            outputs = []
            for index in range(2):
                path = Path(directory) / f"encoded-{index}.avif"
                subprocess.run(
                    [str(encoder), "--reduced-transform-set", str(path)],
                    check=True, cwd=ROOT,
                )
                outputs.append(normalize_sequence_timestamps(path.read_bytes()))
        if outputs[0] != outputs[1]:
            raise RuntimeError("reduced-transform-set fixture is not repeatable")
        encoded = outputs[0]
    validate(encoded)
    if not args.check:
        if ASSET.exists() and ASSET.read_bytes() != encoded:
            raise RuntimeError(f"refusing to replace differing fixture: {ASSET}")
        ASSET.write_bytes(encoded)
    print(f"Verified {ASSET} ({len(encoded)} bytes, sha256={EXPECTED_SHA256})")


if __name__ == "__main__":
    main()
