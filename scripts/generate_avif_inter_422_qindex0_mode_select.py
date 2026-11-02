#!/usr/bin/env python3
"""Generate the pinned qindex-zero non-lossless I422 sequence."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image

from generate_avif_inter_422_small_partitions_fixture import (
    PINNED_ORACLE,
    check_pinned_oracle,
    normalize_sequence_timestamps,
)
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
ASSET = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_lossy_inter_422_qindex0_mode_select_64x64.avif"
)
EXPECTED_ASSET_SHA256 = "062cec9bf86cfa7e5d2130baec72884dec70d96d9621578f91388612d21a1fab"
EXPECTED_FRAME_SHA256 = (
    "09cd9be5690daa187a8f18453bc2bf203f416c10aa60e599e5590a3a6cb0e7a7",
    "efe6384013c5ba216282f444602c7ea6643f52ebf036649281b1257a56bd0616",
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of bytes."""
    return hashlib.sha256(data).hexdigest()


def encode_fixture(encoder: Path) -> bytes:
    """Run the fixture-only qindex-zero I422 encoder twice."""
    with tempfile.TemporaryDirectory(prefix="avif-i422-qindex0-") as directory:
        encoded_paths = [Path(directory) / f"encoded-{index}.avif" for index in range(2)]
        for path in encoded_paths:
            subprocess.run(
                [str(encoder), "--qindex-zero-nonlossless", str(path)],
                check=True,
                cwd=ROOT,
            )
        encoded = [normalize_sequence_timestamps(path.read_bytes()) for path in encoded_paths]
    if encoded[0] != encoded[1]:
        raise RuntimeError("repeated qindex-zero I422 encodes differ")
    return encoded[0]


def validate_pixels(encoded: bytes) -> None:
    """Require the exact pinned Pillow metadata and both RGB frames."""
    with Image.open(BytesIO(encoded)) as image:
        if image.format != "AVIF" or image.mode != "RGB" or image.size != (64, 64):
            raise RuntimeError("Pillow AVIF metadata differs from the pinned fixture")
        if image.n_frames != len(EXPECTED_FRAME_SHA256):
            raise RuntimeError(f"expected two frames, found {image.n_frames}")
        for frame_index, expected_hash in enumerate(EXPECTED_FRAME_SHA256):
            image.seek(frame_index)
            image.load()
            pixels = image.convert("RGB").tobytes()
            if (
                len(pixels) != 64 * 64 * 3
                or sha256(pixels) != expected_hash
                or image.info.get("duration") != 100
            ):
                raise RuntimeError(f"Pillow frame {frame_index} differs from its reference")


def validate_frame_headers(encoded: bytes) -> None:
    """Require an 8-bit I422 key/inter sequence with qindex-zero inter data."""
    with tempfile.TemporaryDirectory(prefix="avif-i422-qindex0-header-") as directory:
        path = Path(directory) / "fixture.avif"
        path.write_bytes(encoded)
        report = inspect_obus(path)

    samples = sorted(
        (sample for sample in report["samples"] if sample.get("role") == "track_pict"),
        key=lambda sample: sample["identity"]["sample"],
    )
    if len(samples) != 2:
        raise RuntimeError("AVIF must contain exactly two picture-track samples")

    sequence_headers = []
    frame_headers = []
    for sample in samples:
        sequence_headers.extend(
            obu["sequence_header"] for obu in sample["obus"] if "sequence_header" in obu
        )
        headers = [obu["frame_header"] for obu in sample["obus"] if "frame_header" in obu]
        if len(headers) != 1:
            raise RuntimeError("each AV1 picture sample must contain one frame header")
        frame_headers.extend(headers)

    if not sequence_headers:
        raise RuntimeError("AV1 picture samples must declare a sequence header")
    sequence = sequence_headers[0]
    if (
        any(header != sequence for header in sequence_headers[1:])
        or sequence.get("profile") != 2
        or sequence.get("bit_depth") != 8
        or sequence.get("monochrome") is not False
        or sequence.get("subsampling_x") != 1
        or sequence.get("subsampling_y") != 0
    ):
        raise RuntimeError("sequence header differs from the pinned 8-bit I422 profile")

    key, inter = frame_headers
    key_quantization = key.get("quantization", {})
    inter_quantization = inter.get("quantization", {})
    if (
        key.get("frame_type") != "key"
        or key.get("all_lossless") is not True
        or key_quantization.get("base") != 0
        or inter.get("frame_type") != "inter"
        or inter.get("frame_width") != 64
        or inter.get("frame_height") != 64
        or inter.get("transform_mode") != "select"
        or inter.get("all_lossless") is not False
        or inter_quantization.get("base") != 0
        or inter_quantization.get("y_dc_delta") != 2
        or any(
            inter_quantization.get(name) != 0
            for name in ("u_dc_delta", "u_ac_delta", "v_dc_delta", "v_ac_delta")
        )
    ):
        raise RuntimeError(
            "fixture must have a lossless key and a non-lossless qindex-zero "
            "TX_MODE_SELECT I422 inter frame"
        )


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

    check_pinned_oracle()
    output = (args.output or ASSET).resolve()
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
        if output.exists() and output.read_bytes() != encoded:
            raise RuntimeError(f"refusing to replace differing fixture: {output}")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(encoded)

    file_hash = sha256(encoded)
    if file_hash != EXPECTED_ASSET_SHA256:
        raise RuntimeError(f"fixture hash changed: {file_hash}")
    validate_pixels(encoded)
    validate_frame_headers(encoded)
    print(f"Verified {output} ({len(encoded)} bytes, sha256={file_hash})")


if __name__ == "__main__":
    main()
