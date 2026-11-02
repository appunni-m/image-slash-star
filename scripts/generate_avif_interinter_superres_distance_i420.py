#!/usr/bin/env python3
"""Generate a pinned AV1 scaled-reference compound-prediction AVIF sequence."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from io import BytesIO
from pathlib import Path
from tempfile import TemporaryDirectory

from PIL import Image, _avif, features
from PIL import __version__ as PILLOW_VERSION

from generate_avif_interinter_distance_i420 import (
    PINNED_ORACLE,
    make_frames,
    normalize_timestamps,
)
from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
FILENAME = "animated_lossy_interinter_superres_distance_i420_b16x16_64x64.avif"
ASSET = ROOT / "tests" / "fixtures" / "input" / "images" / "avif" / FILENAME
EXPECTED_ASSET_SHA256 = "bf6049e6ed687af3367ec31f43517f5cbcd5633171d7dd381b6eb7a6ae9f36ad"
EXPECTED_FRAME_SHA256 = (
    "73cf96ff575c2dfe69d55c11ba275e02fbabd3228299a6cc21722ef46748fbeb",
    "53e3dcfb1e5a811b3fe0ca8be8a5aad6d0c6f5d2f09f0d94e0f466d9320c3cc8",
    "c1989819133a337ef040c1eacc3f0f5cca5851a98edf39f8b1fd63dca9a52dde",
)
WIDTH = 64
HEIGHT = 64
ADVANCED_OPTIONS = (
    ("superres-mode", "1"),
    ("superres-denominator", "10"),
    ("superres-kf-denominator", "8"),
    ("enable-dist-wtd-comp", "1"),
    ("enable-masked-comp", "0"),
    ("enable-interintra-comp", "0"),
    ("enable-global-motion", "0"),
    ("enable-warped-motion", "0"),
    ("enable-cdef", "0"),
    ("enable-restoration", "0"),
    ("loopfilter-control", "0"),
    ("sb-size", "64"),
    ("min-partition-size", "16"),
    ("max-partition-size", "16"),
    ("aq-mode", "0"),
    ("deltaq-mode", "0"),
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of fixture bytes."""
    return hashlib.sha256(data).hexdigest()


def y4m_frame(image: Image.Image) -> bytes:
    """Serialize one grayscale RGB source as a full-range I420 Y4M frame."""
    if image.mode != "RGB" or image.size != (WIDTH, HEIGHT):
        raise RuntimeError("compound fixture source dimensions or mode changed")

    pixels = image.tobytes()
    red = pixels[0::3]
    green = pixels[1::3]
    blue = pixels[2::3]
    if red != green or red != blue:
        raise RuntimeError("compound fixture source is no longer grayscale")

    header = b"YUV4MPEG2 W64 H64 F30:1 Ip A1:1 C420jpeg XCOLORRANGE=FULL\nFRAME\n"
    chroma = bytes([128]) * (WIDTH * HEIGHT // 4)
    return header + red + chroma + chroma


def encode_command(avifenc: Path, sources: tuple[Path, ...], output: Path) -> list[str]:
    """Build the pinned deterministic super-resolution/compound command."""
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
    for index, source in enumerate(sources):
        if index:
            command.extend(("--duration", "1"))
        command.append(str(source))
    command.append(str(output))
    return command


def run(command: list[str]) -> str:
    """Run an oracle command and return its combined output."""
    completed = subprocess.run(
        command,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return completed.stdout


def validate_bitstream(path: Path) -> dict[str, object]:
    """Require scaled inter frames and selectable compound references."""
    report = inspect(path)
    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if not sequence_headers or any(
        header.get("bit_depth") != 8
        or header.get("max_width") != WIDTH
        or header.get("max_height") != HEIGHT
        or header.get("monochrome")
        or header.get("subsampling_x") != 1
        or header.get("subsampling_y") != 1
        or not header.get("enable_superres")
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence differs from the 8-bit I420 super-resolution profile")

    pictures = sorted(
        (sample for sample in report["samples"] if sample.get("role") == "track_pict"),
        key=lambda sample: sample["identity"]["sample"],
    )
    if len(pictures) != 3:
        raise RuntimeError("AVIF must contain exactly three picture-track samples")

    frame_headers = []
    for sample in pictures:
        headers = [obu["frame_header"] for obu in sample["obus"] if "frame_header" in obu]
        if len(headers) != 1:
            raise RuntimeError("each AV1 picture sample must contain one frame header")
        frame_headers.append(headers[0])

    key, first_inter, compound_inter = frame_headers
    if (
        key.get("frame_type") != "key"
        or key.get("frame_width") != WIDTH
        or key.get("upscaled_width") != WIDTH
        or first_inter.get("frame_type") != "inter"
        or first_inter.get("frame_width") != 51
        or first_inter.get("upscaled_width") != WIDTH
        or first_inter.get("superres_enabled") is not True
        or first_inter.get("superres_denominator") != 10
        or first_inter.get("reference_mode") != "single"
        or compound_inter.get("frame_type") != "inter"
        or compound_inter.get("frame_width") != 51
        or compound_inter.get("upscaled_width") != WIDTH
        or compound_inter.get("superres_enabled") is not True
        or compound_inter.get("superres_denominator") != 10
        or compound_inter.get("reference_mode") != "select"
        or compound_inter.get("reference_indices") != [1, 0, 0, 0, 0, 0, 0]
    ):
        raise RuntimeError("AV1 frames differ from the scaled distance-compound profile")

    return {
        "sequence_header": {
            "bit_depth": 8,
            "max_size": [WIDTH, HEIGHT],
            "subsampling": [2, 2],
            "enable_superres": True,
        },
        "frames": [
            {
                "type": header["frame_type"],
                "coded_size": [header["frame_width"], header["frame_height"]],
                "upscaled_width": header["upscaled_width"],
                "superres_denominator": header.get("superres_denominator"),
                "reference_mode": header.get("reference_mode"),
                "reference_indices": header.get("reference_indices"),
            }
            for header in frame_headers
        ],
    }


def validate_pillow(encoded: bytes) -> list[dict[str, object]]:
    """Check exact pinned Pillow output for all three rendered frames."""
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow oracle required; found {observed!r}")

    frames = []
    with Image.open(BytesIO(encoded)) as sequence:
        if (
            sequence.format != "AVIF"
            or sequence.mode != "RGB"
            or sequence.size != (WIDTH, HEIGHT)
            or sequence.n_frames != len(EXPECTED_FRAME_SHA256)
        ):
            raise RuntimeError("Pillow AVIF dimensions, mode, or frame count differ")

        for index, expected_hash in enumerate(EXPECTED_FRAME_SHA256):
            sequence.seek(index)
            sequence.load()
            pixels = sequence.tobytes()
            digest = sha256(pixels)
            duration = sequence.info.get("duration")
            if len(pixels) != WIDTH * HEIGHT * 3 or digest != expected_hash or duration != 33:
                raise RuntimeError(f"Pillow frame {index} differs from its pinned reference")
            frames.append(
                {
                    "index": index,
                    "duration_ms": duration,
                    "bytes": len(pixels),
                    "sha256": digest,
                }
            )
    return frames


def check_pinned_encoder(avifenc: Path) -> str:
    """Require libavif 1.4.1 and libaom 3.13.2."""
    if not avifenc.is_file():
        raise RuntimeError(f"avifenc does not exist: {avifenc}")
    version = run([str(avifenc), "--version"])
    if "1.4.1" not in version or "3.13.2" not in version:
        raise RuntimeError(f"pinned libavif 1.4.1/libaom 3.13.2 avifenc required: {version.strip()}")
    return version


def generate(args: argparse.Namespace) -> None:
    """Encode twice, validate parity evidence, then check or write the fixture."""
    avifenc = args.avifenc.expanduser().resolve()
    version = check_pinned_encoder(avifenc)
    STAGING_ROOT.mkdir(parents=True, exist_ok=True)

    normalized_runs = []
    syntax_reports = []
    pillow_reports = []
    with TemporaryDirectory(prefix="avif-scaled-distance-compound-", dir=STAGING_ROOT) as temporary:
        work = Path(temporary)
        sources = []
        for index, image in enumerate(make_frames()):
            source = work / f"frame-{index}.y4m"
            source.write_bytes(y4m_frame(image))
            sources.append(source)

        for index in range(2):
            raw_path = work / f"candidate-{index}.avif"
            normalized_path = work / f"candidate-{index}-normalized.avif"
            run(encode_command(avifenc, tuple(sources), raw_path))
            normalized = normalize_timestamps(raw_path.read_bytes())
            normalized_path.write_bytes(normalized)
            normalized_runs.append(normalized)
            syntax_reports.append(validate_bitstream(normalized_path))
            pillow_reports.append(validate_pillow(normalized))

    if normalized_runs[0] != normalized_runs[1]:
        raise RuntimeError("independent normalized AVIF encodes differ")
    if syntax_reports[0] != syntax_reports[1] or pillow_reports[0] != pillow_reports[1]:
        raise RuntimeError("independent AV1 syntax or Pillow pixels differ")

    encoded = normalized_runs[0]
    digest = sha256(encoded)
    if digest != EXPECTED_ASSET_SHA256:
        raise RuntimeError(f"normalized AVIF differs from its file hash pin: {digest}")

    output = args.output.expanduser().resolve() if args.output else ASSET.resolve()
    if output != ASSET.resolve() and STAGING_ROOT.resolve() not in output.parents:
        raise RuntimeError("output must be the fixture path or below target/oracle-staging")
    if args.check:
        if not output.is_file() or output.read_bytes() != encoded:
            raise RuntimeError(f"fixture differs from deterministic output: {output}")
    else:
        if output.exists() and output.read_bytes() != encoded:
            raise RuntimeError(f"refusing to replace a different fixture at {output}")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(encoded)

    print(
        json.dumps(
            {
                "asset": str(output.relative_to(ROOT)) if output.is_relative_to(ROOT) else "staging",
                "bytes": len(encoded),
                "sha256": digest,
                "frames": pillow_reports[0],
                "syntax": syntax_reports[0],
                "avifenc": str(avifenc),
                "version": version.strip(),
            },
            indent=2,
        )
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--avifenc", type=Path, required=True, help="Pinned patched avifenc executable")
    parser.add_argument("--output", type=Path, help="Optional output below target/oracle-staging")
    parser.add_argument("--check", action="store_true", help="Verify the checked-in input without writing it")
    args = parser.parse_args()
    try:
        generate(args)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
