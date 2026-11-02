#!/usr/bin/env python3
"""Generate a pinned 10-bit I422 super-resolution/SGR AVIF sequence."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, _avif, features

from generate_avif_10bit_lossless_inter_fixture import (
    normalize_sequence_timestamps,
    sequence_timestamp_fields,
    validate_timestamp_repeatability,
)
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET = ROOT / "tests" / "fixtures" / "input" / "images" / "avif" / (
    "animated_lossy_inter_422_superres_sgr_10bit_160x56.avif"
)
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
EXPECTED_ASSET_SHA256 = "60748f33355d8f8b54f48942e319c1791e75c75c94da55a1568fd6679edf578a"
EXPECTED_Y4M_SHA256 = (
    "38d38ec2544747c5d09d8a68548619fe5e8ec55417a5b6eb84e3ba263d0c1395",
    "896a665decff46a6912b5a0fc283ea310e0de67b52bc5fe9c003756fdb1f124f",
)
EXPECTED_FRAME_SHA256 = (
    "69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed",
    "8b3079d8e82b1eda72b002a51cc3dfafc9e38344dc6a262d4e3c5e8c9493e940",
)
ADVANCED_OPTIONS = (
    ("superres-mode", "1"),
    ("superres-denominator", "10"),
    ("superres-kf-denominator", "8"),
    ("enable-restoration", "1"),
    ("enable-cdef", "0"),
    ("loopfilter-control", "0"),
    ("enable-global-motion", "0"),
    ("enable-warped-motion", "0"),
    ("enable-masked-comp", "0"),
    ("enable-dist-wtd-comp", "0"),
    ("enable-onesided-comp", "0"),
    ("enable-interintra-comp", "0"),
    ("enable-diff-wtd-comp", "0"),
    ("enable-interinter-wedge", "0"),
    ("enable-interintra-wedge", "0"),
    ("enable-obmc", "0"),
    ("enable-ref-frame-mvs", "0"),
    ("enable-intrabc", "0"),
    ("aq-mode", "0"),
    ("deltaq-mode", "0"),
    ("enable-chroma-deltaq", "0"),
    ("enable-tx-size-search", "0"),
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of bytes."""
    return hashlib.sha256(data).hexdigest()


def y4m_frame(frame_index: int) -> bytes:
    """Return one deterministic full-range 10-bit planar 4:2:2 Y4M frame."""
    planes = []
    for plane, width in ((0, 160), (1, 80), (2, 80)):
        samples = []
        for y in range(56):
            for x in range(width):
                luma_x = x if plane == 0 else x * 2
                positive = ((luma_x // 8 + y // 8) & 1) != 0
                if frame_index == 0:
                    sample = 512
                else:
                    sample = 576 if positive ^ (plane == 2) else 448
                samples.append(sample)
        planes.append(struct.pack(f"<{len(samples)}H", *samples))
    header = b"YUV4MPEG2 W160 H56 F30:1 Ip A1:1 C422p10 XCOLORRANGE=FULL\nFRAME\n"
    return header + b"".join(planes)


def encode_command(avifenc: Path, source0: Path, source1: Path, output: Path) -> list[str]:
    command = [
        str(avifenc),
        "-q",
        "60",
        "-s",
        "0",
        "-j",
        "1",
        "--timescale",
        "30",
        "--duration",
        "1",
        "--keyframe",
        "60",
        "--tilecolslog2",
        "0",
        "--tilerowslog2",
        "0",
    ]
    for key, value in ADVANCED_OPTIONS:
        command.extend(("--advanced", f"{key}={value}"))
    command.extend((str(source0), "--duration", "1", str(source1), str(output)))
    return command


def run(command: list[str]) -> bytes:
    completed = subprocess.run(
        command,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return completed.stdout.encode("utf-8")


def validate_bitstream(path: Path) -> dict[str, object]:
    """Require 10-bit I422 super-resolution and active SGR on the inter frame."""
    report = inspect_obus(path)
    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    expected_sequence = {
        "profile": 2,
        "bit_depth": 10,
        "max_width": 160,
        "max_height": 56,
        "monochrome": False,
        "subsampling_x": 1,
        "subsampling_y": 0,
        "enable_superres": True,
        "enable_restoration": True,
        "color_primaries": 1,
        "transfer_characteristics": 13,
        "matrix_coefficients": 6,
        "color_range": 1,
    }
    if not sequence_headers or any(
        any(header.get(key) != value for key, value in expected_sequence.items())
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence header differs from the 10-bit I422 profile")

    samples = sorted(
        (sample for sample in report["samples"] if sample.get("role") == "track_pict"),
        key=lambda sample: sample["identity"]["sample"],
    )
    if len(samples) != 2:
        raise RuntimeError("AVIF must contain exactly two picture-track samples")
    headers = []
    for sample in samples:
        frame_headers = [
            obu["frame_header"] for obu in sample["obus"] if "frame_header" in obu
        ]
        if len(frame_headers) != 1:
            raise RuntimeError("each AV1 picture sample must contain one frame header")
        headers.append(frame_headers[0])

    key, inter = headers
    if (
        key.get("frame_type") != "key"
        or inter.get("frame_type") != "inter"
        or inter.get("frame_width") != 128
        or inter.get("frame_height") != 56
        or inter.get("upscaled_width") != 160
        or inter.get("superres_enabled") is not True
        or inter.get("superres_denominator") != 10
        or inter.get("all_lossless") is not False
        or inter.get("transform_mode") != "largest"
        or inter.get("quantization", {}).get("base") != 100
        or inter.get("quantization", {}).get("using_matrix") is not False
        or inter.get("restoration", {}).get("types") != [3, 3, 3]
        or inter.get("restoration", {}).get("unit_size_log2") != [8, 8]
        or inter.get("cdef", {}).get("bits") != 0
        or inter.get("loop_filter", {}).get("level_y") != [0, 0]
        or inter.get("loop_filter", {}).get("level_u") != 0
        or inter.get("loop_filter", {}).get("level_v") != 0
        or inter.get("delta_q", {}).get("present") is not False
        or inter.get("delta_loop_filter", {}).get("present") is not False
        or inter.get("skip_mode_enabled") is not False
        or inter.get("reference_mode") != "single"
        or inter.get("motion_mode_switchable") is not False
        or inter.get("use_ref_frame_mvs") is not False
        or inter.get("allow_warped_motion") is not False
        or inter.get("segmentation", {}).get("enabled") is not False
    ):
        raise RuntimeError("inter frame differs from the pinned I422 SGR profile")
    return {
        "sequence_header": expected_sequence,
        "inter_frame": {
            "coded_size": [inter["frame_width"], inter["frame_height"]],
            "upscaled_width": inter["upscaled_width"],
            "superres_denominator": inter["superres_denominator"],
            "transform_mode": inter["transform_mode"],
            "restoration_types": inter["restoration"]["types"],
            "restoration_unit_size_log2": inter["restoration"]["unit_size_log2"],
            "base_qindex": inter["quantization"]["base"],
        },
    }


def validate_pillow(path: Path) -> list[dict[str, object]]:
    """Require exact Pillow frame metadata and RGB pixels."""
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"Pillow oracle differs from pin: {observed!r}")
    frames = []
    with Image.open(path) as image:
        if image.size != (160, 56) or image.mode != "RGB" or image.n_frames != 2:
            raise RuntimeError("Pillow must decode two 160x56 RGB frames")
        for index, expected_hash in enumerate(EXPECTED_FRAME_SHA256):
            image.seek(index)
            image.load()
            pixels = image.tobytes()
            digest = sha256(pixels)
            if (
                len(pixels) != 160 * 56 * 3
                or digest != expected_hash
                or image.info.get("duration") != 33
            ):
                raise RuntimeError(f"Pillow frame {index} differs from its pinned reference")
            frames.append(
                {
                    "index": index,
                    "size": list(image.size),
                    "mode": image.mode,
                    "duration_ms": image.info.get("duration"),
                    "bytes": len(pixels),
                    "sha256": digest,
                }
            )
    return frames


def generate(args: argparse.Namespace) -> None:
    """Encode twice, compare the pinned output, and write or verify the fixture."""
    output = args.output.expanduser().resolve() if args.output else ASSET.resolve()
    if output != ASSET.resolve() and STAGING_ROOT.resolve() not in output.parents:
        raise RuntimeError("output must be the fixture path or a target/oracle-staging path")
    if args.check:
        if not output.is_file():
            raise RuntimeError(f"fixture does not exist: {output}")
        digest = sha256(output.read_bytes())
        if digest != EXPECTED_ASSET_SHA256:
            raise RuntimeError(f"fixture differs from its file hash pin: {digest}")
        print(
            json.dumps(
                {
                    "asset": str(output.relative_to(ROOT))
                    if output.is_relative_to(ROOT)
                    else "staging",
                    "bytes": output.stat().st_size,
                    "sha256": digest,
                    "frames": validate_pillow(output),
                    "syntax": validate_bitstream(output),
                    "verified": True,
                },
                indent=2,
            )
        )
        return
    if args.avifenc is None:
        raise RuntimeError("--avifenc is required unless --check is used")
    avifenc = args.avifenc.expanduser().resolve()
    if not avifenc.is_file():
        raise RuntimeError(f"avifenc does not exist: {avifenc}")
    version = subprocess.run(
        [str(avifenc), "--version"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    ).stdout
    if "1.4.1" not in version or "3.13.2" not in version:
        raise RuntimeError(f"avifenc must be libavif 1.4.1/libaom 3.13.2: {version.strip()}")

    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix=".avif-hi10-i422-superres-", dir=STAGING_ROOT
    ) as temporary:
        work = Path(temporary)
        sources = []
        for index in range(2):
            source = work / f"frame-{index}.y4m"
            data = y4m_frame(index)
            if sha256(data) != EXPECTED_Y4M_SHA256[index]:
                raise RuntimeError(f"Y4M source {index} differs from its input hash pin")
            source.write_bytes(data)
            sources.append(source)

        raw_runs = []
        normalized_runs = []
        syntax_reports = []
        pillow_reports = []
        for index in range(2):
            raw_path = work / f"candidate-{index}.avif"
            normalized_path = work / f"candidate-{index}-normalized.avif"
            run(encode_command(avifenc, sources[0], sources[1], raw_path))
            raw = raw_path.read_bytes()
            normalized = normalize_sequence_timestamps(raw)
            normalized_path.write_bytes(normalized)
            raw_runs.append(raw)
            normalized_runs.append(normalized)
            syntax_reports.append(validate_bitstream(normalized_path))
            pillow_reports.append(validate_pillow(normalized_path))

        validate_timestamp_repeatability(raw_runs[0], raw_runs[1])
        if normalized_runs[0] != normalized_runs[1]:
            raise RuntimeError("independent normalized AVIF encodes differ")
        if syntax_reports[0] != syntax_reports[1] or pillow_reports[0] != pillow_reports[1]:
            raise RuntimeError("independent AV1 syntax or Pillow pixels differ")
        asset_hash = sha256(normalized_runs[0])
        if asset_hash != EXPECTED_ASSET_SHA256:
            raise RuntimeError(f"normalized AVIF differs from its file hash pin: {asset_hash}")

        output.parent.mkdir(parents=True, exist_ok=True)
        temporary_output = output.with_name(f".{output.name}.tmp")
        try:
            temporary_output.write_bytes(normalized_runs[0])
            temporary_output.replace(output)
        finally:
            temporary_output.unlink(missing_ok=True)

    print(
        json.dumps(
            {
                "asset": str(output.relative_to(ROOT)) if output.is_relative_to(ROOT) else "staging",
                "bytes": len(normalized_runs[0]),
                "sha256": asset_hash,
                "frames": pillow_reports[0],
                "syntax": syntax_reports[0],
                "timestamp_fields": [
                    {"box": kind.decode("ascii"), "offsets": [start, end]}
                    for kind, start, end in sequence_timestamp_fields(raw_runs[0])
                ],
                "avifenc": str(avifenc),
                "version": version.strip(),
            },
            indent=2,
        )
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--avifenc", type=Path, help="Pinned libavif/libaom avifenc")
    parser.add_argument("--output", type=Path, help="Optional staging output path")
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the retained fixture without rebuilding the fixture encoder",
    )
    args = parser.parse_args()
    try:
        generate(args)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
