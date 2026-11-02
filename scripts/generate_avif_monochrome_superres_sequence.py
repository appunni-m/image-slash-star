#!/usr/bin/env python3
"""Generate a two-frame monochrome AVIF with active AV1 super-resolution."""

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
from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "animated_monochrome_superres_sequence_17x16.avif"
)
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
SOURCE_SHA256 = (
    "5f2b1cf6d7a2ce66f3f815cffc3965f03c6788cd61503d56aeb43bb75a5e18dd",
    "f7cabd5caee02bdaa0665901496a5c472ae35ea08b72d1aea41f8c8342e05f44",
)
ASSET_SHA256 = "d7018788a6c11276df9721e556e726542baa7c6195acb3d42f8404ed4d2b0310"
FRAME_SHA256 = (
    "4b73e39df990e89fe831ee0205ceb76fb185752ed0e5e37971ae29c2b6edcf78",
    "050d031e65c147f66dbdce374365f639585ad869a6e9003786d0e4a314216386",
)
ADVANCED_OPTIONS = (
    ("superres-mode", "1"),
    ("superres-denominator", "9"),
    ("superres-kf-denominator", "9"),
    ("min-partition-size", "16"),
    ("max-partition-size", "16"),
    ("enable-tx-size-search", "0"),
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
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def y4m_frame(frame_index: int) -> bytes:
    """Build a deterministic full-range 17x16 monochrome Y4M frame."""
    if frame_index not in (0, 1):
        raise ValueError("the pinned sequence contains exactly two frames")
    pixels = bytes(
        (x * 13 + y * 7 + ((x * y) & 31) + frame_index * (x * 5 + y * 3 + 11))
        & 0xFF
        for y in range(16)
        for x in range(17)
    )
    header = b"YUV4MPEG2 W17 H16 F30:1 Ip A1:1 Cmono XCOLORRANGE=FULL\nFRAME\n"
    return header + pixels


def encode_command(avifenc: Path, source0: Path, source1: Path, output: Path) -> list[str]:
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
    report = inspect(path)
    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    expected_sequence = {
        "bit_depth": 8,
        "max_width": 17,
        "max_height": 16,
        "monochrome": True,
        "subsampling_x": 1,
        "subsampling_y": 1,
        "enable_superres": True,
    }
    if not sequence_headers or any(
        any(header.get(key) != value for key, value in expected_sequence.items())
        for header in sequence_headers
    ):
        raise RuntimeError("sequence header differs from the pinned monochrome profile")

    samples = sorted(
        (sample for sample in report["samples"] if sample.get("role") == "track_pict"),
        key=lambda sample: sample["identity"]["sample"],
    )
    if len(samples) != 2:
        raise RuntimeError("AVIF must contain exactly two picture-track samples")

    headers = []
    for index, sample in enumerate(samples):
        frame_headers = [
            obu["frame_header"] for obu in sample["obus"] if "frame_header" in obu
        ]
        if len(frame_headers) != 1:
            raise RuntimeError("each picture sample must contain one frame header")
        frame = frame_headers[0]
        expected_frame = {
            "frame_type": "key" if index == 0 else "inter",
            "frame_width": 16,
            "frame_height": 16,
            "upscaled_width": 17,
            "superres_enabled": True,
            "superres_denominator": 9,
            "all_lossless": False,
        }
        if any(frame.get(key) != value for key, value in expected_frame.items()):
            raise RuntimeError(f"frame {index} differs from the pinned super-resolution profile")
        if frame.get("restoration", {}).get("types") != [2, 0, 0]:
            raise RuntimeError(f"frame {index} lacks the pinned monochrome SGR restoration profile")
        tiling = frame.get("tiling", {})
        if tiling.get("columns") != 1 or tiling.get("rows") != 1:
            raise RuntimeError("each monochrome frame must use a single tile")
        tile_count = sum(
            len(obu["tile_group"]["tiles"])
            for obu in sample["obus"]
            if "tile_group" in obu
        )
        if tile_count != 1:
            raise RuntimeError("each frame must contain one complete tile group")
        headers.append(frame)

    return {
        "sequence_header": expected_sequence,
        "frames": [
            {
                "type": header["frame_type"],
                "coded_size": [header["frame_width"], header["frame_height"]],
                "upscaled_width": header["upscaled_width"],
                "superres_denominator": header["superres_denominator"],
            }
            for header in headers
        ],
    }


def validate_pillow(path: Path) -> list[dict[str, object]]:
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"Pillow oracle differs from pin: {observed!r}")

    frames = []
    with Image.open(path) as image:
        if image.size != (17, 16) or image.mode != "RGB" or image.n_frames != 2:
            raise RuntimeError("Pillow must decode two 17x16 RGB frames")
        for index, expected_hash in enumerate(FRAME_SHA256):
            image.seek(index)
            image.load()
            pixels = image.tobytes()
            duration = image.info.get("duration")
            digest = sha256(pixels)
            if len(pixels) != 17 * 16 * 3 or digest != expected_hash or duration != 33:
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
        if digest != ASSET_SHA256:
            raise RuntimeError(f"fixture differs from its file hash pin: {digest}")
        print(
            json.dumps(
                {
                    "asset": str(output.relative_to(ROOT))
                    if output.is_relative_to(ROOT)
                    else "staging",
                    "bytes": output.stat().st_size,
                    "sha256": digest,
                    "syntax": validate_bitstream(output),
                    "frames": validate_pillow(output),
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
    version = run([str(avifenc), "--version"])
    if "1.4.1" not in version or "3.13.2" not in version:
        raise RuntimeError("avifenc must use libavif 1.4.1 and libaom 3.13.2")

    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix=".avif-monochrome-superres-sequence-",
        dir=STAGING_ROOT,
    ) as temporary:
        work = Path(temporary)
        sources = []
        for index, expected_hash in enumerate(SOURCE_SHA256):
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
        if asset_hash != ASSET_SHA256:
            raise RuntimeError(f"normalized AVIF differs from its file hash pin: {asset_hash}")
        if output.exists() and output.read_bytes() != normalized_runs[0]:
            raise RuntimeError(f"refusing to replace a different fixture at {output}")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(normalized_runs[0])
        timestamp_fields = [
            {"box": kind.decode("ascii"), "offsets": [start, end]}
            for kind, start, end in sequence_timestamp_fields(raw_runs[0])
        ]

    print(
        json.dumps(
            {
                "asset": str(output.relative_to(ROOT)) if output.is_relative_to(ROOT) else "staging",
                "bytes": len(normalized_runs[0]),
                "sha256": asset_hash,
                "frames": pillow_reports[0],
                "syntax": syntax_reports[0],
                "timestamp_fields": timestamp_fields,
                "avifenc": str(avifenc),
                "version": version.strip(),
            },
            indent=2,
        )
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--avifenc",
        type=Path,
        help="Pinned libavif/libaom avifenc executable; required unless --check is used",
    )
    parser.add_argument("--output", type=Path, help="Optional output below target/oracle-staging")
    parser.add_argument("--check", action="store_true", help="verify the retained fixture")
    args = parser.parse_args()
    if args.check and args.avifenc is not None:
        parser.error("--check cannot be combined with --avifenc")
    try:
        generate(args)
    except (OSError, RuntimeError, subprocess.SubprocessError, ValueError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
