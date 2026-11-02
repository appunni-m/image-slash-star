#!/usr/bin/env python3
"""Generate a pinned lossy I444 AVIF sequence with inter B8x16 leaves."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from generate_avif_10bit_lossless_inter_fixture import normalize_sequence_timestamps
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_lossy_inter_i444_b8x16_mode_select_64x64.avif"
)
EXPECTED_FILE_SHA256 = "0187f7634e7ffb21019961fbb47091c3c2deaaccd88a04c043a7ffe1b05b8f93"
EXPECTED_FRAME_SHA256 = (
    "88c35ff901b357eb27dd0b079351ce61ea088d2d8bd1ab4fb9a215f3fc6bf930",
    "24c6d0c2cbe9558c7ce24811c04af120ac2e6cc811e2bb0b89377b699185d157",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of bytes."""
    return hashlib.sha256(data).hexdigest()


def check_pinned_oracle() -> None:
    """Require the repository's pinned Pillow and AVIF runtime."""
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow AVIF oracle required; found {observed!r}")


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
            pixels = image.tobytes()
            if (
                len(pixels) != 64 * 64 * 3
                or sha256(pixels) != expected_hash
                or image.info.get("duration") != 100
            ):
                raise RuntimeError(f"Pillow frame {frame_index} differs from its pinned reference")


def validate_frame_headers(encoded: bytes) -> None:
    """Require an 8-bit I444 key/inter sequence with TX_MODE_SELECT."""
    with tempfile.TemporaryDirectory(prefix="avif-i444-b8x16-64x64-") as directory:
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
            raise RuntimeError("each AV1 picture sample must contain exactly one frame header")
        frame_headers.extend(headers)
    if not sequence_headers:
        raise RuntimeError("AV1 picture samples must declare a sequence header")
    sequence = sequence_headers[0]
    if (
        any(header != sequence for header in sequence_headers[1:])
        or sequence.get("profile") != 1
        or sequence.get("bit_depth") != 8
        or sequence.get("monochrome") is not False
        or sequence.get("subsampling_x") != 0
        or sequence.get("subsampling_y") != 0
    ):
        raise RuntimeError("sequence header differs from the pinned 8-bit I444 profile")
    key, inter = frame_headers
    if (
        key.get("frame_type") != "key"
        or inter.get("frame_type") != "inter"
        or inter.get("frame_width") != 64
        or inter.get("frame_height") != 64
        or inter.get("transform_mode") != "select"
        or inter.get("all_lossless") is not False
        or inter.get("quantization", {}).get("base", 0) <= 0
    ):
        raise RuntimeError("inter frame differs from the lossy I444 TX_MODE_SELECT profile")


def encode_twice(encoder: Path) -> bytes:
    """Encode twice and require stable AVIF bytes after timestamp normalization."""
    with tempfile.TemporaryDirectory(prefix="avif-i444-b8x16-") as directory:
        paths = [Path(directory) / f"encoded-{index}.avif" for index in range(2)]
        for path in paths:
            subprocess.run([str(encoder), str(path)], check=True, cwd=ROOT)
        encoded = [normalize_sequence_timestamps(path.read_bytes()) for path in paths]
    if encoded[0] != encoded[1]:
        raise RuntimeError("I444 B8x16 AVIF fixture is not deterministic")
    return encoded[0]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, help="fixture-only pinned AVIF encoder")
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed fixture without rebuilding the fixture encoder",
    )
    args = parser.parse_args()
    check_pinned_oracle()

    output = args.output.resolve()
    if args.check:
        if not output.is_file():
            raise RuntimeError(f"fixture does not exist: {output}")
        encoded = output.read_bytes()
        if normalize_sequence_timestamps(encoded) != encoded:
            raise RuntimeError("fixture BMFF timestamps are not normalized")
    else:
        if args.encoder is None:
            parser.error("--encoder is required when generating a fixture")
        encoder = args.encoder.resolve()
        if not encoder.is_file():
            raise RuntimeError(f"fixture encoder does not exist: {encoder}")
        encoded = encode_twice(encoder)
        if output.exists() and output.read_bytes() != encoded:
            raise RuntimeError(f"refusing to replace differing fixture: {output}")

    file_hash = sha256(encoded)
    if file_hash != EXPECTED_FILE_SHA256:
        raise RuntimeError(f"fixture hash changed: {file_hash}")
    validate_pixels(encoded)
    validate_frame_headers(encoded)
    output.parent.mkdir(parents=True, exist_ok=True)
    if not output.exists():
        output.write_bytes(encoded)
    print(f"Verified {output} ({len(encoded)} bytes, sha256={file_hash})")


if __name__ == "__main__":
    main()
