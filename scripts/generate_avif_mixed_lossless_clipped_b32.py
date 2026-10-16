#!/usr/bin/env python3
"""Generate the pinned mixed-segment AVIF with a clipped lossless B32 leaf."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path
from typing import Any

from PIL import Image, _avif, features
from PIL import __version__ as PILLOW_VERSION

from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "mixed_lossless_clipped_b32_i420_60x60.avif"
)
REFERENCE = (
    ROOT
    / "tests"
    / "fixtures"
    / "outputs"
    / "raws"
    / "Decode.avif_mixed_lossless_clipped_b32_i420_60x60_avif.bin"
)
RECEIPT = STAGING_ROOT / "mixed_lossless_clipped_b32_i420_60x60.receipt.json"
DEFAULT_ENCODER = STAGING_ROOT / "encode-avif-mixed-lossless-clipped-b32"

LIBAVIF_COMMIT = "6543b22b5bc706c53f038a16fe515f921556d9b3"
LIBAOM_COMMIT = "ad44980d7f3c7a2605c25d51ea96946949000841"
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
EXPECTED_FILE_SHA256 = "ea8d484e1cde6819d1c98a64ff58db1bec7937fd929a835cf9c13c2da8430d1b"
EXPECTED_PIXEL_SHA256 = "a3b4b27575bef66a5eb058529aee5cac545d4d43a9bace07421cea066b4d59b8"
EXPECTED_FILE_BYTES = 321
EXPECTED_PIXEL_BYTES = 60 * 60 * 3


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def patch_receipts() -> dict[str, str]:
    patch_paths = (
        ROOT / "scripts" / "avif_fixture_oracle" / "libavif-av1-fixture-controls.patch",
        ROOT
        / "scripts"
        / "avif_fixture_oracle"
        / "libavif-mixed-lossless-clipped-b32.patch",
        ROOT
        / "scripts"
        / "avif_fixture_oracle"
        / "libaom-mixed-lossless-clipped-b32.patch",
    )
    return {
        path.relative_to(ROOT).as_posix(): sha256(path.read_bytes())
        for path in patch_paths
    }


def run_encoder(encoder: Path, output: Path) -> str:
    result = subprocess.run(
        [str(encoder), str(output)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"pinned encoder failed with status {result.returncode}:\n{result.stdout}"
        )

    required_trace = (
        "AVIF_FIXTURE_NONRD row=8 col=8 width=32 height=32 partition=0",
        "AVIF_FIXTURE_LEAF row=8 col=8 width=32 height=32 segment=1 qindex=0",
    )
    for line in required_trace:
        if line not in result.stdout:
            raise RuntimeError(
                f"pinned encoder did not prove the clipped lossless B32 leaf "
                f"({line!r}):\n{result.stdout}"
            )
    return result.stdout


def validate_bitstream(path: Path) -> dict[str, Any]:
    report = inspect_obus(path)
    samples = report.get("samples")
    if not isinstance(samples, list) or len(samples) != 1:
        raise RuntimeError("AVIF must contain one color sample")
    sample = samples[0]
    if sample.get("role") != "item_color":
        raise RuntimeError("AVIF sample must be the primary color item")

    sequence_headers = [
        obu["sequence_header"]
        for obu in sample.get("obus", [])
        if "sequence_header" in obu
    ]
    frame_headers = [
        obu["frame_header"]
        for obu in sample.get("obus", [])
        if "frame_header" in obu
    ]
    if len(sequence_headers) != 1 or len(frame_headers) != 1:
        raise RuntimeError("AVIF must contain one sequence and frame header")

    sequence = sequence_headers[0]
    expected_sequence = {
        "bit_depth": 8,
        "max_width": 60,
        "max_height": 60,
        "monochrome": False,
        "subsampling_x": 1,
        "subsampling_y": 1,
        "color_primaries": 1,
        "transfer_characteristics": 13,
        "matrix_coefficients": 6,
        "color_range": 1,
    }
    if any(sequence.get(key) != value for key, value in expected_sequence.items()):
        raise RuntimeError("AV1 sequence header differs from the pinned full-range I420")

    frame = frame_headers[0]
    if (
        frame.get("frame_type") != "key"
        or frame.get("frame_width") != 60
        or frame.get("frame_height") != 60
        or frame.get("all_lossless") is not False
        or frame.get("transform_mode") != "largest"
        or frame.get("quantization", {}).get("base") != 36
        or frame.get("segmentation", {}).get("enabled") is not True
        or frame.get("segmentation", {}).get("update_map") is not True
        or frame.get("segment_qindex") != [36, 0, 36, 36, 36, 36, 36, 36]
        or frame.get("segment_lossless")
        != [False, True, False, False, False, False, False, False]
        or frame.get("segmentation", {}).get("segments", [{} for _ in range(2)])[1].get(
            "delta_q"
        )
        != -36
    ):
        raise RuntimeError(
            "AV1 frame does not prove mixed qindex-36/qindex-0 lossless segments "
            "with transform mode 1"
        )

    return {
        "sequence_header": {
            key: sequence.get(key)
            for key in expected_sequence
        },
        "frame_header": {
            "frame_type": frame["frame_type"],
            "frame_size": [frame["frame_width"], frame["frame_height"]],
            "all_lossless": frame["all_lossless"],
            "transform_mode": frame["transform_mode"],
            "base_qindex": frame["quantization"]["base"],
            "segment_qindex": frame["segment_qindex"],
            "segment_lossless": frame["segment_lossless"],
            "segment_1_delta_q": frame["segmentation"]["segments"][1]["delta_q"],
        },
        "clipped_b32": {
            "mi_origin": [8, 8],
            "pixel_origin": [8 * 4, 8 * 4],
            "block_size": [32, 32],
            "visible_extent": [60 - 8 * 4, 60 - 8 * 4],
            "segment": 1,
            "qindex": 0,
        },
    }


def validate_pillow(path: Path) -> bytes:
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"pinned Pillow AVIF oracle required; found {observed!r}")

    with Image.open(path) as image:
        if (
            image.format != "AVIF"
            or image.size != (60, 60)
            or image.mode != "RGB"
            or image.n_frames != 1
            or image.is_animated
        ):
            raise RuntimeError(
                "Pillow must decode one non-animated 60x60 RGB AVIF frame"
            )
        image.load()
        pixels = image.tobytes()
    if len(pixels) != EXPECTED_PIXEL_BYTES:
        raise RuntimeError(f"expected {EXPECTED_PIXEL_BYTES} Pillow bytes, found {len(pixels)}")
    if sha256(pixels) != EXPECTED_PIXEL_SHA256:
        raise RuntimeError("pinned Pillow RGB pixels differ from their SHA-256")
    return pixels


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, default=DEFAULT_ENCODER)
    parser.add_argument("--output", type=Path, default=ASSET)
    parser.add_argument("--reference-output", type=Path, default=REFERENCE)
    parser.add_argument("--receipt-output", type=Path, default=RECEIPT)
    args = parser.parse_args()
    encoder = args.encoder.resolve()
    if not encoder.is_file():
        raise RuntimeError(f"fixture encoder does not exist: {encoder}")

    with tempfile.TemporaryDirectory(prefix="avif-mixed-lossless-b32-") as temporary:
        temporary_root = Path(temporary)
        first_path = temporary_root / "first.avif"
        second_path = temporary_root / "second.avif"
        first_trace = run_encoder(encoder, first_path)
        second_trace = run_encoder(encoder, second_path)
        first_bytes = first_path.read_bytes()
        second_bytes = second_path.read_bytes()
        if first_bytes != second_bytes:
            raise RuntimeError("mixed lossless clipped-B32 AVIF encoding is not deterministic")
        first_geometry_trace = [
            line for line in first_trace.splitlines() if line.startswith("AVIF_FIXTURE_")
        ]
        second_geometry_trace = [
            line for line in second_trace.splitlines() if line.startswith("AVIF_FIXTURE_")
        ]
        if first_geometry_trace != second_geometry_trace:
            raise RuntimeError("mixed lossless clipped-B32 encoder geometry is not deterministic")
        if len(first_bytes) != EXPECTED_FILE_BYTES or sha256(first_bytes) != EXPECTED_FILE_SHA256:
            raise RuntimeError("encoded AVIF differs from the pinned file hash")

        bitstream = validate_bitstream(first_path)
        pixels = validate_pillow(first_path)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.reference_output.parent.mkdir(parents=True, exist_ok=True)
    args.receipt_output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(first_bytes)
    args.reference_output.write_bytes(pixels)
    observed_pillow = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    receipt = {
        "libavif_commit": LIBAVIF_COMMIT,
        "libaom_commit": LIBAOM_COMMIT,
        "fixture_patch_sha256": patch_receipts(),
        "encoder_path": encoder.as_posix(),
        "encoder_binary_sha256": sha256(encoder.read_bytes()),
        "encoded_bytes": len(first_bytes),
        "encoded_sha256": sha256(first_bytes),
        "reference_bytes": len(pixels),
        "reference_mode": "Rgb8",
        "reference_size": [60, 60],
        "reference_sha256": sha256(pixels),
        "pillow": {
            "version": observed_pillow[0],
            "libavif": observed_pillow[1],
            "codec_versions": observed_pillow[2],
        },
        "bitstream": bitstream,
        "encoder_geometry_trace": first_geometry_trace,
        "repeat_geometry_trace_matches": True,
    }
    args.receipt_output.write_text(json.dumps(receipt, indent=2) + "\n")

    print(f"Wrote {args.output} ({len(first_bytes)} bytes, SHA-256 {sha256(first_bytes)})")
    print(
        f"Wrote {args.reference_output} ({len(pixels)} bytes, "
        f"SHA-256 {sha256(pixels)})"
    )
    print(f"Wrote oracle receipt {args.receipt_output}")
    print(f"Proved clipped B32 visible extent {bitstream['clipped_b32']['visible_extent']}")


if __name__ == "__main__":
    main()
