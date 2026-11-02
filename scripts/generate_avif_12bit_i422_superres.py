#!/usr/bin/env python3
"""Generate a pinned 12-bit I422 super-resolution AVIF sequence."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import subprocess
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from generate_avif_10bit_lossless_inter_fixture import normalize_sequence_timestamps
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET = ROOT / "tests" / "fixtures" / "input" / "images" / "avif" / (
    "animated_lossy_inter_422_superres_12bit_160x56.avif"
)
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
EXPECTED_ASSET_SHA256 = "565ad0b866eb0fd25b37c5b23e0c3dba8ab791312e782cf4d58fd80a92684db9"
EXPECTED_Y4M_SHA256 = (
    "0fff8b68b497e3023c676fc7d6b0b47b18457e97065d76aa051fde6c73555805",
    "97e9f7193265d7ef519a5423e28e9f4f562a265072fbaf1cce189c55d6c6b379",
)
EXPECTED_FRAME_SHA256 = (
    "69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed",
    "3acbd991ecc3cdbea09be468ef346e16f3413c9766e2cc6ebb8690b8e2f69ab6",
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
    """Build one deterministic full-range 12-bit planar 4:2:2 Y4M frame."""
    planes = []
    for plane, width in ((0, 160), (1, 80), (2, 80)):
        samples = []
        for y in range(56):
            for x in range(width):
                luma_x = x if plane == 0 else x * 2
                positive = ((luma_x // 8 + y // 8) & 1) != 0
                if frame_index == 0:
                    sample = 2048
                else:
                    sample = 2304 if positive ^ (plane == 2) else 1792
                samples.append(sample)
        planes.append(struct.pack(f"<{len(samples)}H", *samples))
    header = b"YUV4MPEG2 W160 H56 F30:1 Ip A1:1 C422p12 XCOLORRANGE=FULL\nFRAME\n"
    return header + b"".join(planes)


def encode_command(avifenc: Path, source0: Path, source1: Path, output: Path) -> list[str]:
    """Build the fixed pinned libavif/libaom encoder invocation."""
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


def validate_bitstream(path: Path) -> dict[str, object]:
    """Require 12-bit I422 super-resolution with restoration disabled."""
    report = inspect_obus(path)
    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    expected_sequence = {
        "profile": 2,
        "bit_depth": 12,
        "max_width": 160,
        "max_height": 56,
        "monochrome": False,
        "subsampling_x": 1,
        "subsampling_y": 0,
        "enable_superres": True,
        "enable_restoration": False,
        "color_primaries": 1,
        "transfer_characteristics": 13,
        "matrix_coefficients": 6,
        "color_range": 1,
    }
    if not sequence_headers or any(
        any(header.get(key) != value for key, value in expected_sequence.items())
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence header differs from the 12-bit I422 profile")

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
        or inter.get("quantization", {}).get("base", 0) <= 0
        or inter.get("restoration", {}).get("types") != [0, 0, 0]
    ):
        raise RuntimeError("inter frame differs from the pinned 12-bit I422 profile")
    return {
        "sequence_header": expected_sequence,
        "inter_frame": {
            "coded_size": [inter["frame_width"], inter["frame_height"]],
            "upscaled_width": inter["upscaled_width"],
            "superres_denominator": inter["superres_denominator"],
            "transform_mode": inter["transform_mode"],
            "restoration_types": inter["restoration"]["types"],
            "base_qindex": inter["quantization"]["base"],
        },
    }


def validate_pillow(path: Path) -> list[dict[str, object]]:
    """Require exact pinned Pillow metadata and RGB frame pixels."""
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
            if len(pixels) != 160 * 56 * 3 or digest != expected_hash:
                raise RuntimeError(f"Pillow frame {index} differs from its reference")
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
    """Encode twice, compare pinned output, and write or verify the fixture."""
    output = args.output.expanduser().resolve() if args.output else ASSET.resolve()
    if output != ASSET.resolve() and STAGING_ROOT.resolve() not in output.parents:
        raise RuntimeError("output must be the fixture path or a target/oracle-staging path")
    if args.check:
        if not output.is_file():
            raise RuntimeError(f"fixture does not exist: {output}")
        encoded = output.read_bytes()
        digest = sha256(encoded)
        if digest != EXPECTED_ASSET_SHA256:
            raise RuntimeError(f"fixture differs from its file hash pin: {digest}")
        print(
            json.dumps(
                {
                    "asset": str(output.relative_to(ROOT))
                    if output.is_relative_to(ROOT)
                    else "staging",
                    "bytes": len(encoded),
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
        prefix=".avif-hi12-i422-superres-", dir=STAGING_ROOT
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

        encoded_runs = []
        for index in range(2):
            candidate = work / f"candidate-{index}.avif"
            subprocess.run(
                encode_command(avifenc, sources[0], sources[1], candidate),
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
            )
            encoded_runs.append(normalize_sequence_timestamps(candidate.read_bytes()))
        if encoded_runs[0] != encoded_runs[1]:
            raise RuntimeError("independent normalized AVIF encodes differ")
        encoded = encoded_runs[0]
        digest = sha256(encoded)
        if digest != EXPECTED_ASSET_SHA256:
            raise RuntimeError(f"normalized AVIF differs from its file hash pin: {digest}")

        temporary_output = output.with_name(f".{output.name}.tmp")
        try:
            temporary_output.write_bytes(encoded)
            validate_pillow(temporary_output)
            validate_bitstream(temporary_output)
            output.parent.mkdir(parents=True, exist_ok=True)
            if output.exists() and output.read_bytes() != encoded:
                raise RuntimeError(f"refusing to replace differing fixture: {output}")
            temporary_output.replace(output)
        finally:
            temporary_output.unlink(missing_ok=True)

    print(
        json.dumps(
            {
                "asset": str(output.relative_to(ROOT)) if output.is_relative_to(ROOT) else "staging",
                "bytes": len(encoded),
                "sha256": digest,
                "frames": validate_pillow(output),
                "syntax": validate_bitstream(output),
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
