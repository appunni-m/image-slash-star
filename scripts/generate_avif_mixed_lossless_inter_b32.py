#!/usr/bin/env python3
"""Generate a pinned mixed-lossless AVIF inter sequence with a full or clipped B32 leaf."""

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
from inspect_avif_bitstreams import children, inspect as inspect_container
from inspect_avif_bitstreams import parse_boxes, unique_box


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "animated_mixed_lossless_inter_b32_i420_64x64.avif"
)
CLIPPED_ASSET = ASSET.with_name("animated_mixed_lossless_inter_b32_i420_60x60.avif")
RESIDUAL_ASSET = ASSET.with_name(
    "animated_mixed_lossless_inter_b32_i420_residual_64x64.avif"
)
REFERENCE_FRAME_TEMPLATE = (
    ROOT
    / "tests"
    / "fixtures"
    / "outputs"
    / "raws"
    / "Decode.avif_animated_mixed_lossless_inter_b32_i420_64x64_avif_frame_{index}.bin"
)
CLIPPED_REFERENCE_FRAME_TEMPLATE = REFERENCE_FRAME_TEMPLATE.with_name(
    "Decode.avif_animated_mixed_lossless_inter_b32_i420_60x60_avif_frame_{index}.bin"
)
RESIDUAL_REFERENCE_FRAME_TEMPLATE = REFERENCE_FRAME_TEMPLATE.with_name(
    "Decode.avif_animated_mixed_lossless_inter_b32_i420_residual_64x64_avif_frame_{index}.bin"
)
REFERENCE = REFERENCE_FRAME_TEMPLATE.with_name(
    REFERENCE_FRAME_TEMPLATE.name.format(index=0)
)
CLIPPED_REFERENCE = CLIPPED_REFERENCE_FRAME_TEMPLATE.with_name(
    CLIPPED_REFERENCE_FRAME_TEMPLATE.name.format(index=0)
)
RESIDUAL_REFERENCE = RESIDUAL_REFERENCE_FRAME_TEMPLATE.with_name(
    RESIDUAL_REFERENCE_FRAME_TEMPLATE.name.format(index=0)
)
RECEIPT = STAGING_ROOT / "mixed_lossless_inter_b32_i420_64x64.receipt.json"
CLIPPED_RECEIPT = STAGING_ROOT / "mixed_lossless_inter_b32_i420_60x60.receipt.json"
RESIDUAL_RECEIPT = STAGING_ROOT / "mixed_lossless_inter_b32_i420_residual_64x64.receipt.json"
DEFAULT_ENCODER = STAGING_ROOT / "encode-avif-mixed-lossless-inter-b32"

LIBAVIF_COMMIT = "6543b22b5bc706c53f038a16fe515f921556d9b3"
LIBAOM_COMMIT = "ad44980d7f3c7a2605c25d51ea96946949000841"
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
EXPECTED_FILE_SHA256 = "ecb244cfa33b1737f09246cf87ad20dbd8156108dc9b93da76a8bbe885007257"
EXPECTED_PIXEL_SHA256 = "31b29f375f838c9c6249d51fa2e30273fc82168bd6e212f8d5a62da1642adfa5"
EXPECTED_FRAME_SHA256 = (
    "90ce523631e9fd385bcae414317ca29e655e9fa26a022e09d50795e71b32d634",
    "aa3d0199115d1734da9065059f33f648e4f90151fbce35dc3cd670a5f6b479f4",
)
RESIDUAL_EXPECTED_FILE_SHA256 = "58552db6049a5220936acccbb483fd0366224744e7a2f81058a099b4016d5d36"
RESIDUAL_EXPECTED_PIXEL_SHA256 = "1dd668726550968fd54f7d92264a3a3a15c0aaec7ab866277cead24e2637402d"
RESIDUAL_EXPECTED_FRAME_SHA256 = (
    "90ce523631e9fd385bcae414317ca29e655e9fa26a022e09d50795e71b32d634",
    "f99f8c1c96218f4dde446f18003ec53ae2c35e57a4f1e1b46ce406a2dbe6ef61",
)
RESIDUAL_EXPECTED_FILE_BYTES = 1087
CLIPPED_EXPECTED_FILE_SHA256 = "618e40d92c2dd80005f841b5af4907e8f328512960621c6f1eb7e267c24b23ce"
CLIPPED_EXPECTED_PIXEL_SHA256 = "31f32dc03ca4a9cbffe09bf77929c32931d460a97c0dceb236b9254c1284671f"
CLIPPED_EXPECTED_FRAME_SHA256 = (
    "a3b4b27575bef66a5eb058529aee5cac545d4d43a9bace07421cea066b4d59b8",
    "6b6f92c82d4962e2f59502cd76f454527f3a213fafa10130620e601812aa918c",
)
EXPECTED_FILE_BYTES = 1083


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
        / "libavif-mixed-lossless-inter-b32.patch",
        ROOT
        / "scripts"
        / "avif_fixture_oracle"
        / "libaom-mixed-lossless-clipped-b32.patch",
        ROOT
        / "scripts"
        / "avif_fixture_oracle"
        / "libaom-mixed-lossless-inter-b32.patch",
    )
    return {
        path.relative_to(ROOT).as_posix(): sha256(path.read_bytes())
        for path in patch_paths
    }


def run_encoder(
    encoder: Path, output: Path, canvas_size: int, *, residual_roi: bool = False
) -> str:
    command = [str(encoder), str(output), str(canvas_size)]
    if residual_roi:
        command.append("--residual-roi")
    result = subprocess.run(
        command,
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
        "AVIF_FIXTURE_INTER_LEAF frame_number=1 frame_type=1 row=8 col=8 "
        "width=32 height=32 partition=0 segment=1 qindex=0",
    )
    for line in required_trace:
        if line not in result.stdout:
            raise RuntimeError(
                "pinned encoder did not prove the frame-one inter-coded B32 leaf "
                f"({line!r}):\n{result.stdout}"
            )
    return result.stdout


def sequence_timestamp_fields(data: bytes) -> list[tuple[bytes, int, int]]:
    root_boxes = parse_boxes(data, 0, len(data))
    movie = unique_box(root_boxes, b"moov")
    if movie is None:
        raise RuntimeError("sequence fixture has no movie box")
    movie_children = children(data, movie)
    timestamp_boxes = [(b"mvhd", unique_box(movie_children, b"mvhd"))]
    for track in (box for box in movie_children if box.kind == b"trak"):
        track_children = children(data, track)
        media = unique_box(track_children, b"mdia")
        if media is None:
            raise RuntimeError("sequence track has no media box")
        timestamp_boxes.extend(
            (
                (b"tkhd", unique_box(track_children, b"tkhd")),
                (b"mdhd", unique_box(children(data, media), b"mdhd")),
            )
        )
    if len(timestamp_boxes) != 3 or any(box is None for _, box in timestamp_boxes):
        raise RuntimeError("fixture must have exactly one movie and one track timestamp set")

    fields = []
    for box_kind, box in timestamp_boxes:
        if box is None or box.payload_start >= box.end or data[box.payload_start] != 1:
            raise RuntimeError(f"fixture lacks a version-one {box_kind!r} box")
        start = box.payload_start + 4
        end = start + 16
        if end > box.end:
            raise RuntimeError(f"truncated creation/modification fields in {box_kind!r}")
        fields.append((box_kind, start, end))
    return fields


def normalize_sequence_timestamps(data: bytes) -> bytes:
    normalized = bytearray(data)
    for _, start, end in sequence_timestamp_fields(data):
        normalized[start:end] = bytes(end - start)
    return bytes(normalized)


def validate_repeatability(first: bytes, second: bytes) -> None:
    if len(first) != len(second):
        raise RuntimeError("native AVIF generation runs have different file lengths")
    timestamp_fields = sequence_timestamp_fields(first)
    differing_offsets = [
        index for index in range(len(first)) if first[index] != second[index]
    ]
    outside_timestamps = [
        offset
        for offset in differing_offsets
        if not any(start <= offset < end for _, start, end in timestamp_fields)
    ]
    if outside_timestamps:
        raise RuntimeError(
            "native AVIF runs differ outside creation/modification timestamp fields: "
            + ", ".join(str(offset) for offset in outside_timestamps[:8])
        )


def validate_bitstream(path: Path, canvas_size: int) -> dict[str, Any]:
    container = inspect_container(path)
    if container["ftyp"]["major"] != "avis":
        raise RuntimeError("fixture does not declare an AVIF sequence")
    tracks = [track for track in container["tracks"] if track["handler"] == "pict"]
    if len(tracks) != 1 or len(tracks[0]["samples"]) != 2:
        raise RuntimeError("fixture must contain one two-sample picture track")
    samples = tracks[0]["samples"]
    if [sample["sync"] for sample in samples] != [True, False]:
        raise RuntimeError("fixture must contain a key sample followed by an inter sample")

    report = inspect_obus(path)
    picture_samples = [
        sample
        for sample in report["samples"]
        if sample["role"] == "track_pict"
        and sample["identity"]["track_id"] == tracks[0]["track_id"]
    ]
    picture_samples.sort(key=lambda sample: sample["identity"]["sample"])
    if [sample["identity"]["sample"] for sample in picture_samples] != [0, 1]:
        raise RuntimeError("AV1 OBU report does not contain exactly two picture samples")

    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    expected_sequence = {
        "bit_depth": 8,
        "max_width": canvas_size,
        "max_height": canvas_size,
        "monochrome": False,
        "subsampling_x": 1,
        "subsampling_y": 1,
        "color_primaries": 1,
        "transfer_characteristics": 13,
        "matrix_coefficients": 6,
        "color_range": 1,
    }
    if not sequence_headers or any(
        any(header.get(key) != value for key, value in expected_sequence.items())
        for header in sequence_headers
    ):
        raise RuntimeError("sequence header differs from the pinned full-range I420 shape")

    frame_headers = []
    frame_reports = []
    for index, sample in enumerate(picture_samples):
        expected_type = ("key", "inter")[index]
        headers = [obu["frame_header"] for obu in sample["obus"] if "frame_header" in obu]
        if len(headers) != 1:
            raise RuntimeError("each picture sample must contain one AV1 frame header")
        frame = headers[0]
        quantization = frame.get("quantization", {})
        segmentation = frame.get("segmentation", {})
        segment_qindex = frame.get("segment_qindex", [])
        segment_lossless = frame.get("segment_lossless", [])
        if expected_type == "inter" and segmentation.get("temporal") is not False:
            raise RuntimeError("inter frame must explicitly update its segment map")
        if (
            frame.get("frame_type") != expected_type
            or frame.get("frame_width") != canvas_size
            or frame.get("frame_height") != canvas_size
            or frame.get("all_lossless") is not False
            or frame.get("transform_mode") not in ("largest", "select")
            or quantization.get("base", 0) <= 0
            or segmentation.get("enabled") is not True
            or len(segment_qindex) != 8
            or segment_qindex[1] != 0
            or segment_qindex[0] <= 0
            or segment_qindex[2] != segment_qindex[0]
            or segment_lossless[1] is not True
            or segment_lossless[0] is not False
            or segment_lossless[2] is not False
            or segmentation.get("segments", [{}, {}])[1].get("delta_q")
            != -quantization.get("base")
        ):
            raise RuntimeError(
                f"frame {expected_type} lacks the mixed lossy/lossless segment contract"
            )
        frame_headers.append(frame)
        frame_reports.append(
            {
                "frame_type": frame["frame_type"],
                "frame_size": [frame["frame_width"], frame["frame_height"]],
                "all_lossless": frame["all_lossless"],
                "transform_mode": frame["transform_mode"],
                "transform_mode_value": (
                    1 if frame["transform_mode"] == "largest" else 2
                ),
                "base_qindex": quantization["base"],
                "segment_qindex": segment_qindex,
                "segment_lossless": segment_lossless,
                "segment_map_temporal": segmentation.get("temporal"),
                "segment_1_delta_q": segmentation["segments"][1]["delta_q"],
            }
        )

    return {
        "sequence_header": {key: sequence_headers[0].get(key) for key in expected_sequence},
        "frames": frame_reports,
        "inter_b32": {
            "frame_index": 1,
            "mi_origin": [8, 8],
            "pixel_origin": [32, 32],
            "block_size": [32, 32],
            "visible_extent": [canvas_size - 32, canvas_size - 32],
            "segment": 1,
            "qindex": 0,
        },
    }


def validate_pillow(
    path: Path,
    canvas_size: int,
    expected_pixel_sha256: str,
    expected_frame_sha256: tuple[str, str],
) -> tuple[list[bytes], list[dict[str, Any]]]:
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"pinned Pillow AVIF oracle required; found {observed!r}")

    pixels: list[bytes] = []
    frame_reports: list[dict[str, Any]] = []
    with Image.open(path) as image:
        if (
            image.format != "AVIF"
            or image.size != (canvas_size, canvas_size)
            or image.mode != "RGB"
            or image.n_frames != 2
        ):
            raise RuntimeError(
                f"Pillow must decode a two-frame {canvas_size}x{canvas_size} RGB AVIF sequence"
            )
        for index, expected_hash in enumerate(expected_frame_sha256):
            image.seek(index)
            image.load()
            frame = image.tobytes()
            digest = sha256(frame)
            duration = image.info.get("duration")
            if (
                len(frame) != canvas_size * canvas_size * 3
                or digest != expected_hash
                or duration != 100
            ):
                raise RuntimeError(f"pinned Pillow RGB pixels differ for frame {index}")
            pixels.append(frame)
            frame_reports.append(
                {
                    "index": index,
                    "mode": image.mode,
                    "size": list(image.size),
                    "duration_ms": duration,
                    "bytes": len(frame),
                    "sha256": digest,
                }
            )

    if sha256(b"".join(pixels)) != expected_pixel_sha256:
        raise RuntimeError("pinned Pillow combined frame pixels differ from their SHA-256")
    return pixels, frame_reports


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, default=DEFAULT_ENCODER)
    parser.add_argument("--canvas-size", type=int, choices=(60, 64), default=64)
    parser.add_argument(
        "--lossless-roi-residual",
        action="store_true",
        help="change all pixels in the qindex-zero B32 ROI between frames",
    )
    parser.add_argument("--output", type=Path)
    parser.add_argument("--reference-output", type=Path)
    parser.add_argument("--receipt-output", type=Path)
    args = parser.parse_args()
    residual_roi = args.lossless_roi_residual
    if residual_roi and args.canvas_size != 64:
        parser.error("--lossless-roi-residual requires --canvas-size 64")
    clipped = args.canvas_size == 60
    if residual_roi:
        expected_file_sha256 = RESIDUAL_EXPECTED_FILE_SHA256
        expected_pixel_sha256 = RESIDUAL_EXPECTED_PIXEL_SHA256
        expected_frame_sha256 = RESIDUAL_EXPECTED_FRAME_SHA256
        expected_file_bytes = RESIDUAL_EXPECTED_FILE_BYTES
        output_path = args.output or RESIDUAL_ASSET
        reference_output = args.reference_output or RESIDUAL_REFERENCE
        reference_frame_template = RESIDUAL_REFERENCE_FRAME_TEMPLATE
        receipt_output = args.receipt_output or RESIDUAL_RECEIPT
    else:
        expected_file_sha256 = (
            CLIPPED_EXPECTED_FILE_SHA256 if clipped else EXPECTED_FILE_SHA256
        )
        expected_pixel_sha256 = (
            CLIPPED_EXPECTED_PIXEL_SHA256 if clipped else EXPECTED_PIXEL_SHA256
        )
        expected_frame_sha256 = (
            CLIPPED_EXPECTED_FRAME_SHA256 if clipped else EXPECTED_FRAME_SHA256
        )
        expected_file_bytes = EXPECTED_FILE_BYTES
        output_path = args.output or (CLIPPED_ASSET if clipped else ASSET)
        reference_output = args.reference_output or (
            CLIPPED_REFERENCE if clipped else REFERENCE
        )
        reference_frame_template = (
            CLIPPED_REFERENCE_FRAME_TEMPLATE if clipped else REFERENCE_FRAME_TEMPLATE
        )
        receipt_output = args.receipt_output or (CLIPPED_RECEIPT if clipped else RECEIPT)
    encoder = args.encoder.resolve()
    if not encoder.is_file():
        raise RuntimeError(f"fixture encoder does not exist: {encoder}")

    with tempfile.TemporaryDirectory(prefix="avif-mixed-lossless-inter-b32-") as temporary:
        temporary_root = Path(temporary)
        first_path = temporary_root / "first.avif"
        second_path = temporary_root / "second.avif"
        first_trace = run_encoder(
            encoder, first_path, args.canvas_size, residual_roi=residual_roi
        )
        second_trace = run_encoder(
            encoder, second_path, args.canvas_size, residual_roi=residual_roi
        )
        first_raw = first_path.read_bytes()
        second_raw = second_path.read_bytes()
        validate_repeatability(first_raw, second_raw)
        normalized = normalize_sequence_timestamps(first_raw)
        if normalized != normalize_sequence_timestamps(second_raw):
            raise RuntimeError("mixed-lossless inter AVIF sequence is not deterministic")
        if len(normalized) != expected_file_bytes or sha256(normalized) != expected_file_sha256:
            raise RuntimeError("encoded AVIF differs from the pinned file hash")

        normalized_path = temporary_root / "normalized.avif"
        normalized_path.write_bytes(normalized)
        bitstream = validate_bitstream(normalized_path, args.canvas_size)
        pixels, frame_reports = validate_pillow(
            normalized_path,
            args.canvas_size,
            expected_pixel_sha256,
            expected_frame_sha256,
        )
        if residual_roi:
            changed_pixels = sum(
                pixels[0][(y * 64 + x) * 3 : (y * 64 + x) * 3 + 3]
                != pixels[1][(y * 64 + x) * 3 : (y * 64 + x) * 3 + 3]
                for y in range(32, 64)
                for x in range(32, 64)
            )
            if changed_pixels != 32 * 32:
                raise RuntimeError(
                    "the qindex-zero B32 ROI must change at every Pillow pixel"
                )
        else:
            changed_pixels = 0

    output_path.parent.mkdir(parents=True, exist_ok=True)
    reference_output.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_bytes(normalized)
    reference_output.write_bytes(pixels[0])
    for index, frame in enumerate(pixels):
        reference_frame_template.with_name(
            reference_frame_template.name.format(index=index)
        ).write_bytes(frame)

    observed_pillow = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    geometry_trace = [
        line
        for line in first_trace.splitlines()
        if line.startswith("AVIF_FIXTURE_")
    ]
    repeat_geometry_trace = [
        line
        for line in second_trace.splitlines()
        if line.startswith("AVIF_FIXTURE_")
    ]
    if geometry_trace != repeat_geometry_trace:
        raise RuntimeError("encoder geometry trace differs between repeat runs")

    receipt_output.parent.mkdir(parents=True, exist_ok=True)
    receipt = {
        "libavif_commit": LIBAVIF_COMMIT,
        "libaom_commit": LIBAOM_COMMIT,
        "fixture_patch_sha256": patch_receipts(),
        "encoder_path": encoder.as_posix(),
        "encoder_binary_sha256": sha256(encoder.read_bytes()),
        "encoded_bytes": len(normalized),
        "encoded_sha256": sha256(normalized),
        "reference_bytes": sum(len(frame) for frame in pixels),
        "reference_mode": "Rgb8",
        "reference_size": [args.canvas_size, args.canvas_size],
        "reference_sha256": sha256(b"".join(pixels)),
        "pillow_frames": frame_reports,
        "pillow": {
            "version": observed_pillow[0],
            "libavif": observed_pillow[1],
            "codec_versions": observed_pillow[2],
        },
        "bitstream": bitstream,
        "lossless_roi_residual_pixels": changed_pixels,
        "encoder_geometry_trace": geometry_trace,
        "repeat_geometry_trace_matches": True,
        "sequence_timestamps_normalized": ["mvhd", "tkhd", "mdhd"],
    }
    receipt_output.write_text(json.dumps(receipt, indent=2) + "\n")

    print(f"Wrote {output_path} ({len(normalized)} bytes, SHA-256 {sha256(normalized)})")
    print(
        f"Wrote first-frame reference {reference_output} "
        f"({len(pixels[0])} bytes, SHA-256 {sha256(pixels[0])})"
    )
    for frame in frame_reports:
        print(
            f"Pillow frame {frame['index']}: {frame['bytes']} bytes, "
            f"SHA-256 {frame['sha256']}"
        )
    print(f"Wrote oracle receipt {receipt_output}")
    print(f"Proved frame-one inter B32 at {bitstream['inter_b32']['pixel_origin']}")


if __name__ == "__main__":
    main()
