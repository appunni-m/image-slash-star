#!/usr/bin/env python3
"""Generate a pinned 8-bit monochrome, two-tile CDEF parity fixture."""

from __future__ import annotations

import argparse
import hashlib
import random
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, _avif, features

from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET_ROOT = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
REFERENCE_ROOT = ROOT / "tests" / "fixtures" / "outputs" / "raws"
AVIFENC_VERSION = "Version: 1.4.1 (aom [enc]:3.13.2)"
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
WIDTH = 128
HEIGHT = 64
SOURCE_SHA256 = "b3f7d0ce2a014d088a7a17a09d196ce2828a796712290c41261fa9b6b5711cae"


@dataclass(frozen=True)
class FixtureProfile:
    """Pinned AV1 syntax, encoder settings, and Pillow pixels for one filter profile."""

    variant: str
    stem: str
    quality: int
    speed: int
    advanced_options: tuple[tuple[str, str], ...]
    transform_mode: str
    quantization_matrix: bool
    cdef_damping: int
    cdef_y_strengths: tuple[int, ...]
    loop_filter_y: tuple[int, int]
    loop_filter_sharpness: int
    asset_sha256: str
    rgb_sha256: str


PROFILES = {
    profile.variant: profile
    for profile in (
        FixtureProfile(
            variant="cdef",
            stem="coverage_monochrome_multitile_cdef_8bit_128x64",
            quality=40,
            speed=0,
            advanced_options=(
                ("sb-size", "64"),
                ("min-partition-size", "16"),
                ("max-partition-size", "64"),
                ("enable-tx-size-search", "0"),
                ("enable-qm", "0"),
                ("enable-restoration", "0"),
                ("enable-cdef", "1"),
                ("loopfilter-control", "0"),
                ("aq-mode", "0"),
                ("deltaq-mode", "0"),
            ),
            transform_mode="largest",
            quantization_matrix=False,
            cdef_damping=6,
            cdef_y_strengths=(61,),
            loop_filter_y=(0, 0),
            loop_filter_sharpness=0,
            asset_sha256="ea6c16c86c07414bfa3f71f1132af6494c4d7f6cb5015d5b98bfc94e09e5d73b",
            rgb_sha256="0d9fa687322831cf61ae6cd8b2cba87ee78bbb071641aada0f2879667bebd3b8",
        ),
        FixtureProfile(
            variant="loop_cdef",
            stem="coverage_monochrome_multitile_loop_cdef_8bit_128x64",
            quality=75,
            speed=6,
            advanced_options=(
                ("sb-size", "64"),
                ("min-partition-size", "16"),
                ("max-partition-size", "64"),
                ("enable-restoration", "0"),
                ("enable-cdef", "1"),
                ("aq-mode", "0"),
                ("deltaq-mode", "0"),
            ),
            transform_mode="select",
            quantization_matrix=True,
            cdef_damping=5,
            cdef_y_strengths=(0,),
            loop_filter_y=(4, 4),
            loop_filter_sharpness=7,
            asset_sha256="f81ad82e033b3d36cd1c92ab97e296a1b4c3ea5308cd12c919e748a8304ce282",
            rgb_sha256="2b228a77b35dad05b440b250ee3eee71d858a8561dde92a71634e8cccfffbb57",
        ),
        FixtureProfile(
            variant="loop_cdef_active",
            stem="coverage_monochrome_multitile_loop_cdef_active_8bit_128x64",
            quality=20,
            speed=6,
            advanced_options=(
                ("sb-size", "64"),
                ("min-partition-size", "16"),
                ("max-partition-size", "64"),
                ("enable-restoration", "0"),
                ("enable-cdef", "1"),
            ),
            transform_mode="largest",
            quantization_matrix=True,
            cdef_damping=3,
            cdef_y_strengths=(44,),
            loop_filter_y=(56, 56),
            loop_filter_sharpness=0,
            asset_sha256="229af13b8ae6f1e69ce316e74c42d2d225d3ce479bececd45c5ff2059502d763",
            rgb_sha256="04fe662bfb299b49c6f8117c02278c3b94a80c18f388ca2c5f7370732af460e5",
        ),
    )
}


def profile_paths(profile: FixtureProfile) -> tuple[Path, Path]:
    """Return the AVIF input and exact RGB reference paths for a profile."""

    return (
        ASSET_ROOT / f"{profile.stem}.avif",
        REFERENCE_ROOT / f"Decode.avif_{profile.stem}_avif.bin",
    )


def sha256(data: bytes) -> str:
    """Return a lowercase SHA-256 digest."""

    return hashlib.sha256(data).hexdigest()


def y4m_source() -> bytes:
    """Build deterministic grayscale input for the 128x64 Cmono profile."""

    generator = random.Random(0xCDEF)
    pixels = bytes(
        96 + ((x * 13 + y * 29 + generator.randrange(32)) % 128)
        for y in range(HEIGHT)
        for x in range(WIDTH)
    )
    header = (
        f"YUV4MPEG2 W{WIDTH} H{HEIGHT} F30:1 Ip A1:1 Cmono XCOLORRANGE=FULL\n"
        "FRAME\n"
    ).encode("ascii")
    return header + pixels


def encode_command(
    avifenc: Path,
    source: Path,
    output: Path,
    profile: FixtureProfile,
) -> list[str]:
    """Return pinned libaom settings for a two-column tile profile."""

    command = [
        str(avifenc),
        "-q",
        str(profile.quality),
        "-s",
        str(profile.speed),
        "-j",
        "1",
        "--tilerowslog2",
        "0",
        "--tilecolslog2",
        "1",
    ]
    for name, value in profile.advanced_options:
        command.extend(("--advanced", f"{name}={value}"))
    command.extend((str(source), str(output)))
    return command


def run(command: list[str]) -> str:
    """Run a fixture tool and include diagnostics on failure."""

    result = subprocess.run(
        command,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return result.stdout


def validate_bitstream(path: Path, profile: FixtureProfile) -> dict[str, object]:
    """Require the exact public monochrome multi-tile filter syntax profile."""

    report = inspect_obus(path)
    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    expected_sequence = {
        "bit_depth": 8,
        "max_width": WIDTH,
        "max_height": HEIGHT,
        "monochrome": True,
        "subsampling_x": True,
        "subsampling_y": True,
        "enable_superres": False,
        "enable_cdef": True,
        "enable_restoration": False,
    }
    if not sequence_headers or any(
        any(header.get(key) != value for key, value in expected_sequence.items())
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence header differs from the monochrome CDEF profile")

    samples = report["samples"]
    frame_headers = [
        obu["frame_header"]
        for sample in samples
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    if len(samples) != 1 or len(frame_headers) != 1:
        raise RuntimeError("AVIF must contain one still AV1 picture")

    frame = frame_headers[0]
    quantization = frame.get("quantization", {})
    tiling = frame.get("tiling", {})
    cdef = frame.get("cdef", {})
    loop_filter = frame.get("loop_filter", {})
    restoration = frame.get("restoration", {})
    if (
        frame.get("frame_type") != "key"
        or frame.get("frame_width") != WIDTH
        or frame.get("frame_height") != HEIGHT
        or frame.get("upscaled_width") != WIDTH
        or frame.get("superres_enabled") is not False
        or frame.get("all_lossless") is not False
        or frame.get("transform_mode") != profile.transform_mode
        or quantization.get("base", 0) == 0
        or quantization.get("using_matrix") is not profile.quantization_matrix
        or (tiling.get("columns"), tiling.get("rows")) != (2, 1)
        or cdef.get("bits") != 0
        or cdef.get("damping") != profile.cdef_damping
        or cdef.get("y_strengths") != list(profile.cdef_y_strengths)
        or cdef.get("uv_strengths") != []
        or loop_filter.get("level_y") != list(profile.loop_filter_y)
        or loop_filter.get("level_u") != 0
        or loop_filter.get("level_v") != 0
        or loop_filter.get("sharpness") != profile.loop_filter_sharpness
        or restoration.get("types") != [0, 0, 0]
        or frame.get("delta_q", {}).get("present") is not False
        or frame.get("delta_loop_filter", {}).get("present") is not False
        or frame.get("segmentation", {}).get("enabled") is not False
    ):
        raise RuntimeError("AV1 frame differs from the monochrome multi-tile CDEF profile")

    return {
        "sequence_header": expected_sequence,
        "frame_type": frame["frame_type"],
        "tile_columns": tiling["columns"],
        "tile_rows": tiling["rows"],
        "cdef_y_strengths": cdef["y_strengths"],
        "base_qindex": quantization["base"],
    }


def validate_pillow(path: Path, profile: FixtureProfile) -> bytes:
    """Return exact RGB bytes from the pinned Pillow/libavif oracle."""

    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"Pillow oracle differs from pin: {observed!r}")

    with Image.open(path) as image:
        if image.format != "AVIF" or image.size != (WIDTH, HEIGHT) or image.mode != "RGB":
            raise RuntimeError("Pillow opened AVIF with unexpected image properties")
        image.verify()

    with Image.open(path) as image:
        image.load()
        pixels = image.tobytes()
    if len(pixels) != WIDTH * HEIGHT * 3 or sha256(pixels) != profile.rgb_sha256:
        raise RuntimeError("Pillow RGB bytes differ from the pinned monochrome reference")
    return pixels


def validate_existing(asset: Path, reference: Path, profile: FixtureProfile) -> None:
    """Verify fixture bytes, AV1 syntax, and the pinned Pillow reference."""

    if sha256(asset.read_bytes()) != profile.asset_sha256:
        raise RuntimeError("committed AVIF asset differs from its pinned SHA-256")
    validate_bitstream(asset, profile)
    if sha256(reference.read_bytes()) != profile.rgb_sha256:
        raise RuntimeError("committed RGB reference differs from its pinned Pillow hash")


def generate(
    avifenc: Path,
    asset: Path,
    reference: Path,
    profile: FixtureProfile,
) -> None:
    """Encode twice, validate independent pixels, and write the parity fixture."""

    avifenc = avifenc.expanduser().resolve()
    if not avifenc.is_file():
        raise RuntimeError(f"avifenc does not exist: {avifenc}")
    version = run([str(avifenc), "--version"])
    if version.splitlines()[0] != AVIFENC_VERSION:
        raise RuntimeError("avifenc must use libavif 1.4.1 and libaom 3.13.2")

    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix=f"avif-monochrome-multitile-{profile.variant}-", dir=STAGING_ROOT
    ) as temporary:
        work = Path(temporary)
        source = work / "source.y4m"
        first = work / "first.avif"
        second = work / "second.avif"
        source_bytes = y4m_source()
        if sha256(source_bytes) != SOURCE_SHA256:
            raise RuntimeError("generated Y4M source differs from its pinned SHA-256")
        source.write_bytes(source_bytes)

        run(encode_command(avifenc, source, first, profile))
        run(encode_command(avifenc, source, second, profile))
        asset_bytes = first.read_bytes()
        if (
            asset_bytes != second.read_bytes()
            or sha256(asset_bytes) != profile.asset_sha256
        ):
            raise RuntimeError("libaom output differs from the pinned AVIF bytes")

        syntax = validate_bitstream(first, profile)
        pixels = validate_pillow(first, profile)
        asset.parent.mkdir(parents=True, exist_ok=True)
        reference.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(first, asset)
        reference.write_bytes(pixels)

    print(
        {
            "asset": str(asset),
        "asset_sha256": profile.asset_sha256,
            "reference": str(reference),
        "reference_sha256": profile.rgb_sha256,
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
    parser.add_argument("--variant", choices=sorted(PROFILES), default="cdef")
    parser.add_argument("--asset", type=Path)
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--check", action="store_true", help="validate committed outputs only")
    args = parser.parse_args()
    profile = PROFILES[args.variant]
    default_asset, default_reference = profile_paths(profile)
    asset = args.asset or default_asset
    reference = args.reference or default_reference

    if args.check:
        if args.avifenc is not None:
            parser.error("--check cannot be combined with --avifenc")
        validate_existing(asset, reference, profile)
        print(f"Verified {asset} and {reference}")
    elif args.avifenc is None:
        parser.error("--avifenc is required unless --check is used")
    else:
        generate(args.avifenc, asset, reference, profile)


if __name__ == "__main__":
    main()
