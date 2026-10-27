#!/usr/bin/env python3
"""Generate a pinned monochrome sequence with qindex-zero TX_MODE_SELECT leaves."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as PILLOW_VERSION, _avif, features

from generate_avif_10bit_lossless_inter_fixture import normalize_sequence_timestamps
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
ASSET = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_lossy_inter_monochrome_b16x64_qindex0_mode_select_64x64.avif"
)
EXPECTED_ASSET_SHA256 = "972330af1b5c8bd691dae557ccb6e70532959c52009f5e840aca73989975bf3c"
EXPECTED_FRAME_SHA256 = (
    "e7b00031b2a3ae66798ba32c8b803100073cba5b4d1b36dcbcc60fe1ae322bee",
    "4dd1322b796175c60b7ad53a5901d9b2955aea0af871d697992e000e7659b7c6",
)
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_oracle() -> None:
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"Pillow/native version differs from pin: {observed!r}")


def encode_fixture(encoder: Path) -> bytes:
    with tempfile.TemporaryDirectory(prefix="avif-mono-b16x64-qindex0-") as directory:
        encoded_paths = [Path(directory) / f"encoded-{index}.avif" for index in range(2)]
        for path in encoded_paths:
            subprocess.run([str(encoder), str(path)], check=True, cwd=ROOT)
        encoded = [normalize_sequence_timestamps(path.read_bytes()) for path in encoded_paths]
    if encoded[0] != encoded[1]:
        raise RuntimeError("qindex-zero monochrome TX_MODE_SELECT fixture is not deterministic")
    return encoded[0]


def validate_frames(data: bytes) -> None:
    with Image.open(BytesIO(data)) as image:
        if image.format != "AVIF" or image.size != (64, 64) or image.mode != "RGB":
            raise RuntimeError("Pillow AVIF metadata differs from the pinned fixture")
        if image.n_frames != len(EXPECTED_FRAME_SHA256):
            raise RuntimeError("Pillow AVIF frame count differs from the pinned fixture")
        for index, expected_hash in enumerate(EXPECTED_FRAME_SHA256):
            image.seek(index)
            image.load()
            pixels = image.tobytes()
            if (
                len(pixels) != 64 * 64 * 3
                or sha256(pixels) != expected_hash
                or image.info.get("duration") != 100
            ):
                raise RuntimeError(f"Pillow frame {index} differs from its pinned reference")


def validate_frame_headers(data: bytes) -> None:
    with tempfile.TemporaryDirectory(prefix="avif-mono-b16x64-qindex0-header-") as directory:
        path = Path(directory) / "fixture.avif"
        path.write_bytes(data)
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
        or sequence.get("profile") != 0
        or sequence.get("bit_depth") != 8
        or sequence.get("monochrome") is not True
    ):
        raise RuntimeError("sequence header differs from the pinned 8-bit monochrome profile")

    key, inter = frame_headers
    key_quantization = key.get("quantization", {})
    quantization = inter.get("quantization", {})
    if (
        key.get("frame_type") != "key"
        or key.get("all_lossless") is not True
        or key_quantization.get("base") != 0
        or key_quantization.get("y_dc_delta") != 0
        or inter.get("frame_type") != "inter"
        or inter.get("frame_width") != 64
        or inter.get("frame_height") != 64
        or inter.get("transform_mode") != "select"
        or inter.get("all_lossless") is not False
        or quantization.get("base") != 0
        or quantization.get("y_dc_delta") != 2
        or any(
            quantization.get(name) != 0
            for name in ("u_dc_delta", "u_ac_delta", "v_dc_delta", "v_ac_delta")
        )
    ):
        raise RuntimeError(
            "fixture must have a lossless qindex-zero key and a non-lossless "
            "qindex-zero TX_MODE_SELECT inter frame"
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

    validate_oracle()
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
    validate_frames(encoded)
    validate_frame_headers(encoded)
    print(f"Verified {output} ({len(encoded)} bytes, sha256={file_hash})")


if __name__ == "__main__":
    main()
