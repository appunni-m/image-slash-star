#!/usr/bin/env python3
"""Generate the pinned lossy 10-bit I420 B128x128 AVIF sequence fixture."""

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
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "animated_lossy_wide_i420_10bit_b128x128.avif"
)
EXPECTED_FILE_SHA256 = "4167f64f7b7336c0f16cbac49379cae49671a988f80b2ba066fdec9fe54c32d4"
EXPECTED_FRAME_SHA256 = (
    "cdb0804d645ce6fb16c0a497db1c6e7214026b0df508f56353044a7d2a49a903",
    "21860611f25c648fc5b0fd7085786097164175954615d3b0a0a7f022797f0272",
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
    """Require the pinned Pillow AVIF decoder and runtime codec versions."""
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow AVIF oracle required; found {observed!r}")


def encode_twice(encoder: Path) -> bytes:
    """Encode twice and require identical bytes after timestamp normalization."""
    with tempfile.TemporaryDirectory(prefix="avif-i420-b128x128-10bit-") as directory:
        paths = [Path(directory) / f"encoded-{index}.avif" for index in range(2)]
        for path in paths:
            subprocess.run([str(encoder), str(path)], check=True, cwd=ROOT)
        encoded = [normalize_sequence_timestamps(path.read_bytes()) for path in paths]
    if encoded[0] != encoded[1]:
        raise RuntimeError("lossy 10-bit I420 B128x128 AVIF fixture is not deterministic")
    return encoded[0]


def validate_pixels(encoded: bytes) -> None:
    """Require exact pinned Pillow metadata and RGB frames."""
    with Image.open(BytesIO(encoded)) as image:
        if image.format != "AVIF" or image.mode != "RGB" or image.size != (128, 128):
            raise RuntimeError("Pillow AVIF metadata differs from the pinned fixture")
        if image.n_frames != len(EXPECTED_FRAME_SHA256):
            raise RuntimeError(f"expected two frames, found {image.n_frames}")
        for frame_index, expected_hash in enumerate(EXPECTED_FRAME_SHA256):
            image.seek(frame_index)
            image.load()
            pixels = image.convert("RGB").tobytes()
            if (
                len(pixels) != 128 * 128 * 3
                or sha256(pixels) != expected_hash
                or image.info.get("duration") != 100
            ):
                raise RuntimeError(f"Pillow frame {frame_index} differs from its pinned reference")


def validate_frame_headers(encoded: bytes) -> None:
    """Require the pinned lossy 10-bit I420 key/inter restoration profile."""
    with tempfile.TemporaryDirectory(prefix="avif-i420-b128x128-10bit-inspect-") as directory:
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
        or sequence.get("profile") != 0
        or sequence.get("bit_depth") != 10
        or sequence.get("monochrome") is not False
        or sequence.get("subsampling_x") != 1
        or sequence.get("subsampling_y") != 1
        or sequence.get("color_range") != 1
        or sequence.get("use_128x128_superblock") is not True
    ):
        raise RuntimeError("sequence header differs from the pinned full-range 10-bit I420 profile")

    key, inter = frame_headers
    if (
        key.get("frame_type") != "key"
        or inter.get("frame_type") != "inter"
        or key.get("frame_width") != 128
        or key.get("frame_height") != 128
        or inter.get("frame_width") != 128
        or inter.get("frame_height") != 128
        or key.get("transform_mode") != "largest"
        or inter.get("transform_mode") != "largest"
        or key.get("all_lossless") is not False
        or inter.get("all_lossless") is not False
        or key.get("restoration", {}).get("types") != [3, 3, 3]
        or inter.get("restoration", {}).get("types") != [3, 3, 3]
        or key.get("restoration", {}).get("unit_size_log2") != [8, 8]
        or inter.get("restoration", {}).get("unit_size_log2") != [8, 8]
        or key.get("cdef", {}).get("bits") != 0
        or inter.get("cdef", {}).get("bits") != 0
        or key.get("loop_filter", {}).get("level_y") != [0, 0]
        or inter.get("loop_filter", {}).get("level_y") != [0, 0]
        or key.get("quantization", {}).get("base", 0) <= 0
        or inter.get("quantization", {}).get("base", 0) <= 0
    ):
        raise RuntimeError("frame headers differ from the lossy 10-bit B128x128 profile")


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
    else:
        if args.encoder is None:
            parser.error("--encoder is required when generating a fixture")
        encoder = args.encoder.resolve()
        if not encoder.is_file():
            raise RuntimeError(f"fixture encoder does not exist: {encoder}")
        encoded = encode_twice(encoder)

    file_hash = sha256(encoded)
    if file_hash != EXPECTED_FILE_SHA256:
        raise RuntimeError(f"fixture hash changed: {file_hash}")
    validate_pixels(encoded)
    validate_frame_headers(encoded)

    if not args.check:
        if output.exists() and output.read_bytes() != encoded:
            raise RuntimeError(f"refusing to replace differing fixture: {output}")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(encoded)
    print(f"Verified {output} ({len(encoded)} bytes, sha256={file_hash})")


if __name__ == "__main__":
    main()
