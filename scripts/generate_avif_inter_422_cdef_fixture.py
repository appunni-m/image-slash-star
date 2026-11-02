#!/usr/bin/env python3
"""Generate a pinned 16x16 I422 inter-frame CDEF AVIF parity fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
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
ASSET = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "animated_lossy_inter_422_cdef_8bit_16x16.avif"
)
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
EXPECTED_SOURCE_SHA256 = (
    "c44d3e40821a1d15129c3565ef5fdc82e314e9e5ed032739aba88869353b7f62",
    "cc186cd3d5d73a42ebafa2e524c92f01d2b3d1ddca26dd8a91a66ddb617ae297",
)
EXPECTED_ASSET_SHA256 = "84446e8a38a5ad9a742f6e4274a0b8982f07fd4ceb778787b10abe7dcc420c28"
EXPECTED_FRAME_SHA256 = (
    "bbd410464f0aec96998d578a91ee8a9ab166bcb05e6f4a15107992798a6ba542",
    "09caf9a6b16ae693926681d7c98f167a1fb0d934fba1e01df7f95e264c624150",
)
ADVANCED_OPTIONS = (
    ("min-partition-size", "16"),
    ("max-partition-size", "16"),
    ("enable-tx-size-search", "0"),
    ("enable-restoration", "1"),
    ("enable-cdef", "1"),
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
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def y4m_frame(frame_index: int) -> bytes:
    """Build one deterministic, full-range 8-bit planar I422 frame."""

    luma = bytearray()
    chroma_u = bytearray()
    chroma_v = bytearray()
    for y in range(16):
        for x in range(16):
            sample = 48 + ((x * 37 + y * 53 + x * y * 11) % 160)
            if frame_index and 4 <= x < 12 and 3 <= y < 13:
                sample = 235 - sample
            luma.append(sample)
    for y in range(16):
        for x in range(8):
            chroma_u.append(36 + ((x * 67 + y * 31 + x * y * 7 + frame_index * 9) % 184))
            chroma_v.append(40 + ((x * 29 + y * 71 + x * y * 5 + frame_index * 13) % 176))

    header = b"YUV4MPEG2 W16 H16 F30:1 Ip A1:1 C422 XCOLORRANGE=FULL\nFRAME\n"
    return header + bytes(luma + chroma_u + chroma_v)


def encode_command(
    avifenc: Path,
    source0: Path,
    source1: Path,
    output: Path,
) -> list[str]:
    command = [
        str(avifenc),
        "-q",
        "45",
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


def run(command: list[str]) -> str:
    result = subprocess.run(
        command,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return result.stdout


def validate_bitstream(path: Path) -> dict[str, object]:
    report = inspect_obus(path)
    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    expected_sequence = {
        "profile": 2,
        "bit_depth": 8,
        "max_width": 16,
        "max_height": 16,
        "monochrome": False,
        "subsampling_x": 1,
        "subsampling_y": 0,
        "enable_superres": False,
        "enable_cdef": True,
        "enable_restoration": True,
    }
    if not sequence_headers or any(
        any(header.get(key) != value for key, value in expected_sequence.items())
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence header differs from the 8-bit I422 profile")

    samples = sorted(
        (sample for sample in report["samples"] if sample.get("role") == "track_pict"),
        key=lambda sample: sample["identity"]["sample"],
    )
    if len(samples) != 2:
        raise RuntimeError("AVIF must contain exactly two picture-track samples")

    frame_headers = []
    for sample in samples:
        headers = [obu["frame_header"] for obu in sample["obus"] if "frame_header" in obu]
        if len(headers) != 1:
            raise RuntimeError("each AV1 picture sample must contain one frame header")
        frame_headers.append(headers[0])

    key_frame, inter_frame = frame_headers
    if (
        key_frame.get("frame_type") != "key"
        or inter_frame.get("frame_type") != "inter"
        or any(
            header.get(key) != value
            for header in frame_headers
            for key, value in (
                ("frame_width", 16),
                ("frame_height", 16),
                ("upscaled_width", 16),
                ("superres_enabled", False),
                ("all_lossless", False),
            )
        )
        or inter_frame.get("transform_mode") != "largest"
        or inter_frame.get("quantization", {}).get("base") != 140
        or inter_frame.get("quantization", {}).get("using_matrix") is not False
        or inter_frame.get("cdef", {}).get("bits") != 0
        or not any(
            inter_frame.get("cdef", {}).get(plane, [0])
            for plane in ("y_strengths", "uv_strengths")
        )
        or inter_frame.get("restoration", {}).get("types") != [0, 0, 0]
        or inter_frame.get("loop_filter", {}).get("level_y") != [0, 0]
        or inter_frame.get("loop_filter", {}).get("level_u") != 0
        or inter_frame.get("loop_filter", {}).get("level_v") != 0
        or inter_frame.get("delta_q", {}).get("present") is not False
        or inter_frame.get("delta_loop_filter", {}).get("present") is not False
        or inter_frame.get("skip_mode_enabled") is not False
        or inter_frame.get("reference_mode") != "single"
        or inter_frame.get("use_ref_frame_mvs") is not False
        or inter_frame.get("segmentation", {}).get("enabled") is not False
    ):
        raise RuntimeError("inter frame differs from the public I422 CDEF profile")

    return {
        "sequence_header": expected_sequence,
        "key_frame": key_frame["frame_type"],
        "inter_frame": {
            "size": [inter_frame["frame_width"], inter_frame["frame_height"]],
            "transform_mode": inter_frame["transform_mode"],
            "base_qindex": inter_frame["quantization"]["base"],
            "cdef": inter_frame["cdef"],
            "restoration_types": inter_frame["restoration"]["types"],
        },
    }


def validate_pillow(path: Path) -> list[dict[str, object]]:
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"Pillow oracle differs from pin: {observed!r}")

    frames = []
    with Image.open(path) as image:
        if image.size != (16, 16) or image.mode != "RGB" or image.n_frames != 2:
            raise RuntimeError("Pillow must decode two 16x16 RGB frames")
        for index, expected_hash in enumerate(EXPECTED_FRAME_SHA256):
            image.seek(index)
            image.load()
            pixels = image.tobytes()
            digest = sha256(pixels)
            duration = image.info.get("duration")
            if len(pixels) != 16 * 16 * 3 or digest != expected_hash or duration != 33:
                raise RuntimeError(f"Pillow frame {index} differs from its pinned reference")
            frames.append(
                {
                    "index": index,
                    "size": list(image.size),
                    "mode": image.mode,
                    "duration_ms": duration,
                    "bytes": len(pixels),
                    "sha256": digest,
                }
            )
    return frames


def generate(args: argparse.Namespace) -> None:
    output = args.output.expanduser().resolve() if args.output is not None else ASSET.resolve()
    if output != ASSET.resolve() and STAGING_ROOT.resolve() not in output.parents:
        raise RuntimeError("output must be the fixture path or below target/oracle-staging")
    if args.check:
        if not output.is_file():
            raise RuntimeError(f"fixture does not exist: {output}")
        digest = sha256(output.read_bytes())
        if digest != EXPECTED_ASSET_SHA256:
            raise RuntimeError(f"fixture differs from its file hash pin: {digest}")
        report = {
            "asset": str(output.relative_to(ROOT)) if output.is_relative_to(ROOT) else "staging",
            "sha256": digest,
            "bytes": output.stat().st_size,
            "syntax": validate_bitstream(output),
            "frames": validate_pillow(output),
            "verified": True,
        }
        print(json.dumps(report, indent=2))
        return
    if args.avifenc is None:
        raise RuntimeError("--avifenc is required unless --check is used")
    avifenc = args.avifenc.expanduser().resolve()
    if not avifenc.is_file():
        raise RuntimeError(f"avifenc does not exist: {avifenc}")
    version = run([str(avifenc), "--version"])
    if "1.4.1" not in version or "3.13.2" not in version:
        raise RuntimeError("avifenc must use libavif 1.4.1 and libaom 3.13.2")

    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".avif-inter-i422-cdef-", dir=STAGING_ROOT) as temporary:
        work = Path(temporary)
        sources = []
        for index, expected_hash in enumerate(EXPECTED_SOURCE_SHA256):
            data = y4m_frame(index)
            if sha256(data) != expected_hash:
                raise RuntimeError(f"Y4M source {index} differs from its input hash pin")
            source = work / f"frame-{index}.y4m"
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

        if output.exists() and output.read_bytes() != normalized_runs[0]:
            raise RuntimeError(f"refusing to replace a different fixture at {output}")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(normalized_runs[0])

        report = {
            "asset": str(output.relative_to(ROOT)) if output.is_relative_to(ROOT) else "staging",
            "bytes": len(normalized_runs[0]),
            "sha256": asset_hash,
            "frames": pillow_reports[0],
            "syntax": syntax_reports[0],
            "timestamp_fields": [
                {"box": kind.decode("ascii"), "offsets": [start, end]}
                for kind, start, end in sequence_timestamp_fields(raw_runs[0])
            ],
        }

    print(json.dumps({"asset": report, "avifenc": str(avifenc), "version": version.strip()}, indent=2))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--avifenc",
        type=Path,
        help="Pinned libavif/libaom avifenc executable; required unless --check is used",
    )
    parser.add_argument("--output", type=Path, help="Optional staging output path")
    parser.add_argument("--check", action="store_true", help="verify the retained fixture")
    args = parser.parse_args()
    try:
        generate(args)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
