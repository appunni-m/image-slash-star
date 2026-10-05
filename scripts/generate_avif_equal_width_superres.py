#!/usr/bin/env python3
"""Generate pinned AVIF super-resolution fixtures for 4:2:0, 4:2:2, and 4:4:4."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, _avif, features

from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET_ROOT = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


@dataclass(frozen=True)
class FixtureProfile:
    chroma_format: str
    asset_name: str
    width: int
    height: int
    superres_denominator: int
    source_sha256: str
    asset_sha256: str
    pixels_sha256: str
    subsampling_x: int
    subsampling_y: int
    advanced_options: tuple[tuple[str, str], ...]
    cicp: tuple[int, int, int] | None = None
    full_range: bool | None = None


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
NO_RESTORATION_OPTIONS = tuple(
    (key, "0" if key == "enable-restoration" else value)
    for key, value in ADVANCED_OPTIONS
)
PROFILES = {
    "420": FixtureProfile(
        chroma_format="420jpeg",
        asset_name="superres_equal_width_16x16.avif",
        width=16,
        height=16,
        superres_denominator=9,
        source_sha256="2e2f418407e97791ab8a72a73878dc9b0a4e446f666bd51f13526b535f5f5706",
        asset_sha256="2de74d720f8863be43049a3df776337c1cde494122a57093ffad532cea19a004",
        pixels_sha256="d5b4e270a08e4f03c6de84f4488420658a3ab62b00a954aeca14d50f58bb0eef",
        subsampling_x=1,
        subsampling_y=1,
        advanced_options=ADVANCED_OPTIONS,
    ),
    "444": FixtureProfile(
        chroma_format="444",
        asset_name="superres_equal_width_i444_16x16.avif",
        width=16,
        height=16,
        superres_denominator=9,
        source_sha256="0ed9bb725d8f4eb3ae33998b97f064ef9486683156befa28e3c572f652d42788",
        asset_sha256="cd02b86f213b96f9d58bec3781f8855f325aa7b4c867a6fbf76f5802886420a4",
        pixels_sha256="9cebc72915c26c647a920455c639b606976a1bbc0221ff9bf10cc0c9441864f9",
        subsampling_x=0,
        subsampling_y=0,
        advanced_options=(*ADVANCED_OPTIONS, ("enable-qm", "0")),
    ),
    "444-actual": FixtureProfile(
        chroma_format="444",
        asset_name="superres_actual_upscaled_i444_32x16.avif",
        width=32,
        height=16,
        superres_denominator=9,
        source_sha256="657cca877bc538eb54f26d53d4c3ede01ed2d584ac14ff9ce67543fd5c36dd9e",
        asset_sha256="c976ff5f3a4669557d6dc45780743ab462f1766de732c7914d5094663e323948",
        pixels_sha256="b0e67526aef49d7eef555bf385e706db90d5c6e03228a62259692e6cbb8a84df",
        subsampling_x=0,
        subsampling_y=0,
        advanced_options=(*ADVANCED_OPTIONS, ("enable-qm", "0")),
    ),
    "422-actual": FixtureProfile(
        chroma_format="422",
        asset_name="superres_actual_upscaled_i422_33x17.avif",
        width=33,
        height=17,
        superres_denominator=9,
        source_sha256="25fb156933cb769fb94ddb7c596ccaeefbf1272a5b6269deebf2b6b3551f5356",
        asset_sha256="9095580af62e5003fcb8f68d62da1b6b0cbb6205578a2828d05a87ee2f367c1f",
        pixels_sha256="25b0da5647a3c80a6ab366db9a8e800e4b166ebc0d5a95f1c64dc62f8d795c20",
        subsampling_x=1,
        subsampling_y=0,
        advanced_options=NO_RESTORATION_OPTIONS,
        cicp=(1, 13, 6),
        full_range=True,
    ),
}


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def source_y4m(profile: FixtureProfile) -> bytes:
    """Build the deterministic full-range 8-bit source for a chroma profile."""
    width = profile.width
    height = profile.height
    luma = bytes(
        (x * 13 + y * 7 + ((x * y) & 31)) & 0xFF
        for y in range(height)
        for x in range(width)
    )
    chroma_width = (width + (1 << profile.subsampling_x) - 1) >> profile.subsampling_x
    chroma_height = (height + (1 << profile.subsampling_y) - 1) >> profile.subsampling_y
    chroma_u = bytes(
        (80 + x * 9 + y * 3) & 0xFF
        for y in range(chroma_height)
        for x in range(chroma_width)
    )
    chroma_v = bytes(
        (176 - x * 5 + y * 7) & 0xFF
        for y in range(chroma_height)
        for x in range(chroma_width)
    )
    header = (
        f"YUV4MPEG2 W{width} H{height} F30:1 Ip A1:1 C{profile.chroma_format} "
        "XCOLORRANGE=FULL\nFRAME\n"
    ).encode("ascii")
    return header + luma + chroma_u + chroma_v


def encode_command(
    avifenc: Path,
    source: Path,
    output: Path,
    profile: FixtureProfile,
) -> list[str]:
    command = [
        str(avifenc),
        "-q",
        "45",
        "-s",
        "0",
        "-j",
        "1",
        "--tilecolslog2",
        "0",
        "--tilerowslog2",
        "0",
    ]
    for key, value in profile.advanced_options:
        command.extend(("--advanced", f"{key}={value}"))
    if profile.cicp is not None:
        command.extend(("--cicp", "/".join(str(value) for value in profile.cicp)))
    if profile.full_range is not None:
        command.extend(("--range", "full" if profile.full_range else "limited"))
    command.extend((str(source), str(output)))
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


def validate_bitstream(path: Path, profile: FixtureProfile) -> None:
    report = inspect_obus(path)
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
        raise RuntimeError("AVIF must contain one AV1 sequence and frame header")

    sequence = sequence_headers[0]
    expected_sequence = {
        "bit_depth": 8,
        "max_width": profile.width,
        "max_height": profile.height,
        "monochrome": False,
        "subsampling_x": profile.subsampling_x,
        "subsampling_y": profile.subsampling_y,
        "enable_superres": True,
    }
    if profile.cicp is not None:
        expected_sequence.update(
            zip(
                ("color_primaries", "transfer_characteristics", "matrix_coefficients"),
                profile.cicp,
                strict=True,
            )
        )
    if profile.full_range is not None:
        expected_sequence["color_range"] = int(profile.full_range)
    if any(sequence.get(key) != value for key, value in expected_sequence.items()):
        raise RuntimeError("AV1 sequence header differs from its 8-bit chroma profile")

    frame = frame_headers[0]
    coded_width = max(
        (profile.width * 8 + profile.superres_denominator // 2)
        // profile.superres_denominator,
        min(16, profile.width),
    )
    expected_frame = {
        "frame_type": "key",
        "frame_width": coded_width,
        "frame_height": profile.height,
        "upscaled_width": profile.width,
        "superres_enabled": True,
        "superres_denominator": profile.superres_denominator,
        "all_lossless": False,
    }
    if any(frame.get(key) != value for key, value in expected_frame.items()):
        raise RuntimeError("AV1 frame header differs from the requested super-resolution geometry")


def validate_pillow(path: Path, profile: FixtureProfile) -> None:
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"Pillow oracle differs from pin: {observed!r}")

    with Image.open(path) as image:
        image.load()
        pixels = image.tobytes()
        if (
            image.format != "AVIF"
            or image.size != (profile.width, profile.height)
            or image.mode != "RGB"
            or image.n_frames != 1
            or len(pixels) != profile.width * profile.height * 3
            or sha256(pixels) != profile.pixels_sha256
        ):
            raise RuntimeError("Pillow pixels differ from the pinned AVIF still reference")


def generate(avifenc: Path, profile: FixtureProfile) -> None:
    avifenc = avifenc.expanduser().resolve()
    if not avifenc.is_file():
        raise RuntimeError(f"avifenc does not exist: {avifenc}")
    version = run([str(avifenc), "--version"])
    if "1.4.1" not in version or "3.13.2" not in version:
        raise RuntimeError("avifenc must use libavif 1.4.1 and libaom 3.13.2")

    source_data = source_y4m(profile)
    if sha256(source_data) != profile.source_sha256:
        raise RuntimeError("generated Y4M source differs from its input hash pin")

    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix=".avif-equal-width-superres-",
        dir=STAGING_ROOT,
    ) as temporary:
        work = Path(temporary)
        source = work / "source.y4m"
        source.write_bytes(source_data)

        encoded_runs = []
        for index in range(2):
            candidate = work / f"candidate-{index}.avif"
            run(encode_command(avifenc, source, candidate, profile))
            data = candidate.read_bytes()
            if sha256(data) != profile.asset_sha256:
                raise RuntimeError(f"encoded AVIF differs from its hash pin: {sha256(data)}")
            validate_bitstream(candidate, profile)
            validate_pillow(candidate, profile)
            encoded_runs.append(data)

        if encoded_runs[0] != encoded_runs[1]:
            raise RuntimeError("independent equal-width AVIF encodes differ")

        asset = ASSET_ROOT / profile.asset_name
        asset.parent.mkdir(parents=True, exist_ok=True)
        asset.write_bytes(encoded_runs[0])
    print(f"Wrote {asset} ({len(encoded_runs[0])} bytes, sha256 {profile.asset_sha256})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--avifenc",
        type=Path,
        required=True,
        help="avifenc built with pinned libavif 1.4.1 and libaom 3.13.2",
    )
    parser.add_argument(
        "--chroma",
        choices=sorted(PROFILES),
        default="420",
        help="YUV chroma format for the fixture (default: 420)",
    )
    arguments = parser.parse_args()
    generate(arguments.avifenc, PROFILES[arguments.chroma])


if __name__ == "__main__":
    main()
