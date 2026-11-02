#!/usr/bin/env python3
"""Generate a color AVIF parity fixture with tiled super-resolution."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import shutil
import subprocess
import tempfile

from PIL import Image, _avif, features

from inspect_av1_obus import inspect as inspect_av1
from inspect_avif_bitstreams import inspect as inspect_avif


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "coverage_color_multitile_superres_8bit_256x64.avif"
)
REFERENCE = (
    ROOT
    / "tests"
    / "fixtures"
    / "outputs"
    / "raws"
    / "Decode.avif_coverage_color_multitile_superres_8bit_256x64_avif.bin"
)

WIDTH = 256
HEIGHT = 64
CODED_WIDTH = 228
SUPERRES_DENOMINATOR = 9
AVIFENC_VERSION = "Version: 1.4.1 (aom [enc]:3.13.2)"
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
SOURCE_SHA256 = "62d2716fec633745791370ae10ee2e642c063a3cde509d8ce0b2bb79039102e3"
ASSET_SHA256 = "5a1c05d8f225ef1c73f73010089ab0d5ddf7e495620f117673321a248e71b044"
PIXEL_SHA256 = "42703be2059382a9c3dce70c4795ea5e085fcd87e30ca361d9bfc443213273f3"

ADVANCED_OPTIONS = (
    ("sb-size", "64"),
    ("min-partition-size", "16"),
    ("max-partition-size", "64"),
    ("superres-mode", "1"),
    ("superres-denominator", str(SUPERRES_DENOMINATOR)),
    ("superres-kf-denominator", str(SUPERRES_DENOMINATOR)),
    ("enable-restoration", "0"),
    ("enable-cdef", "0"),
    ("aq-mode", "0"),
    ("deltaq-mode", "0"),
)


def sha256(data: bytes) -> str:
    """Return a lowercase SHA-256 digest."""
    return hashlib.sha256(data).hexdigest()


def source_y4m() -> bytes:
    """Build the fixed full-range 8-bit I420 encoder input."""
    planes = []
    for plane_width, plane_height, phase, scale in (
        (WIDTH, HEIGHT, 0, 1),
        (WIDTH // 2, HEIGHT // 2, 37, 2),
        (WIDTH // 2, HEIGHT // 2, 91, 2),
    ):
        pixels = bytes(
            (
                96
                + (
                    (
                        x * 13
                        + y * 29
                        + ((x * y + phase) % 31) * 7
                    )
                    // scale
                )
                % 128
            )
            for y in range(plane_height)
            for x in range(plane_width)
        )
        planes.append(pixels)

    header = (
        f"YUV4MPEG2 W{WIDTH} H{HEIGHT} F30:1 Ip A1:1 C420jpeg "
        "XCOLORRANGE=FULL\nFRAME\n"
    )
    return header.encode("ascii") + b"".join(planes)


def encode_command(avifenc: Path, source: Path, output: Path) -> list[str]:
    """Build the deterministic two-column AV1 super-resolution command."""
    command = [
        str(avifenc),
        "-q",
        "45",
        "-s",
        "0",
        "-j",
        "1",
        "--tilecolslog2",
        "1",
        "--tilerowslog2",
        "0",
    ]
    for name, value in ADVANCED_OPTIONS:
        command.extend(("--advanced", f"{name}={value}"))
    command.extend((str(source), str(output)))
    return command


def run(command: list[str]) -> str:
    """Run a fixture-generation command and preserve its diagnostics."""
    result = subprocess.run(
        command,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return result.stdout


def validate_pillow(path: Path) -> bytes:
    """Return the exact RGB bytes produced by the pinned Pillow oracle."""
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"Pillow oracle differs from pin: {observed!r}")

    with Image.open(path) as image:
        if image.format != "AVIF" or image.size != (WIDTH, HEIGHT) or image.mode != "RGB":
            raise RuntimeError("Pillow opened AVIF with unexpected image properties")
        if image.n_frames != 1:
            raise RuntimeError("fixture must decode as one still image")
        image.verify()

    with Image.open(path) as image:
        image.load()
        pixels = image.tobytes()
    if len(pixels) != WIDTH * HEIGHT * 3 or sha256(pixels) != PIXEL_SHA256:
        raise RuntimeError("Pillow RGB bytes differ from the pinned exact reference")
    return pixels


def validate_bitstream(path: Path) -> dict[str, object]:
    """Require the color, tile, and super-resolution syntax under test."""
    container = inspect_avif(path)
    color_items = container["items"]["color"]
    if len(color_items) != 1 or color_items[0]["type"] != "av01":
        raise RuntimeError("AVIF must contain exactly one AV1 color item")
    if container["items"]["alpha"]:
        raise RuntimeError("AVIF must not contain an alpha item")

    report = inspect_av1(path)
    if len(report["samples"]) != 1:
        raise RuntimeError("AVIF must contain one still AV1 sample")
    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    frame_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    if len(sequence_headers) != 1 or len(frame_headers) != 1:
        raise RuntimeError("AV1 sample must contain one sequence and frame header")

    sequence = sequence_headers[0]
    expected_sequence = {
        "bit_depth": 8,
        "max_width": WIDTH,
        "max_height": HEIGHT,
        "monochrome": False,
        "subsampling_x": 1,
        "subsampling_y": 1,
        "enable_superres": True,
        "enable_cdef": False,
        "enable_restoration": False,
    }
    if any(sequence.get(key) != value for key, value in expected_sequence.items()):
        raise RuntimeError("AV1 sequence header differs from the color superres profile")

    frame = frame_headers[0]
    tiling = frame.get("tiling", {})
    expected_frame = {
        "frame_type": "key",
        "frame_width": CODED_WIDTH,
        "frame_height": HEIGHT,
        "upscaled_width": WIDTH,
        "superres_enabled": True,
        "superres_denominator": SUPERRES_DENOMINATOR,
    }
    if any(frame.get(key) != value for key, value in expected_frame.items()):
        raise RuntimeError("AV1 frame header differs from the color superres profile")
    if (tiling.get("columns"), tiling.get("rows")) != (2, 1):
        raise RuntimeError("AV1 frame must use two tile columns and one tile row")

    return {
        "coded_size": [CODED_WIDTH, HEIGHT],
        "upscaled_size": [WIDTH, HEIGHT],
        "tile_columns": tiling["columns"],
        "tile_rows": tiling["rows"],
        "superres_denominator": frame["superres_denominator"],
    }


def validate_existing(asset: Path = ASSET, reference: Path = REFERENCE) -> None:
    """Verify the retained fixture, AV1 syntax, and pinned Pillow reference."""
    asset_bytes = asset.read_bytes()
    if sha256(asset_bytes) != ASSET_SHA256:
        raise RuntimeError("committed AVIF asset differs from its pinned SHA-256")
    validate_bitstream(asset)
    pixels = validate_pillow(asset)
    if reference.read_bytes() != pixels or sha256(pixels) != PIXEL_SHA256:
        raise RuntimeError("committed RGB reference differs from pinned Pillow output")


def generate(avifenc: Path, asset: Path = ASSET, reference: Path = REFERENCE) -> None:
    """Encode twice, verify deterministic bytes, and write the parity fixture."""
    avifenc = avifenc.expanduser().resolve()
    if not avifenc.is_file():
        raise RuntimeError(f"avifenc does not exist: {avifenc}")
    if run([str(avifenc), "--version"]).splitlines()[0] != AVIFENC_VERSION:
        raise RuntimeError("avifenc must use libavif 1.4.1 and libaom 3.13.2")

    source_bytes = source_y4m()
    if sha256(source_bytes) != SOURCE_SHA256:
        raise RuntimeError("generated Y4M source differs from its pinned SHA-256")

    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="avif-color-multitile-superres-", dir=STAGING_ROOT
    ) as temporary:
        work = Path(temporary)
        source = work / "source.y4m"
        first = work / "first.avif"
        second = work / "second.avif"
        source.write_bytes(source_bytes)

        run(encode_command(avifenc, source, first))
        run(encode_command(avifenc, source, second))
        asset_bytes = first.read_bytes()
        if asset_bytes != second.read_bytes() or sha256(asset_bytes) != ASSET_SHA256:
            raise RuntimeError("libaom output differs from the pinned AVIF bytes")

        syntax = validate_bitstream(first)
        pixels = validate_pillow(first)
        asset.parent.mkdir(parents=True, exist_ok=True)
        reference.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(first, asset)
        reference.write_bytes(pixels)

    validate_existing(asset, reference)
    print(
        {
            "asset": str(asset),
            "asset_sha256": ASSET_SHA256,
            "reference": str(reference),
            "reference_sha256": PIXEL_SHA256,
            "syntax": syntax,
        }
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--avifenc",
        type=Path,
        help="avifenc built from libavif 1.4.1 and libaom 3.13.2",
    )
    parser.add_argument("--check", action="store_true", help="validate committed outputs only")
    args = parser.parse_args()

    if args.check:
        if args.avifenc is not None:
            parser.error("--check cannot be combined with --avifenc")
        validate_existing()
        print(f"Verified {ASSET} and {REFERENCE}")
    elif args.avifenc is None:
        parser.error("--avifenc is required unless --check is used")
    else:
        generate(args.avifenc)


if __name__ == "__main__":
    main()
