#!/usr/bin/env python3
"""Generate pinned lossy 4:2:0 four-Square8 AVIF fixtures."""

from __future__ import annotations

import argparse
import hashlib
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
AVIFENC_VERSION = "Version: 1.4.1 (aom [enc]:3.13.2)"


def quadrant_pixels() -> bytes:
    """Return the fixed RGB source with a distinct lower-right 8x8 quadrant."""

    base = (17, 91, 203)
    replacement = (22, 96, 208)
    return bytes(
        component
        for y in range(16)
        for x in range(16)
        for component in (replacement if x >= 8 and y >= 8 else base)
    )


def gradient_pixels() -> bytes:
    """Return the smooth RGB gradient that disables screen-content tools."""

    return bytes(
        component
        for y in range(16)
        for x in range(16)
        for component in (24 + 5 * x + 2 * y, 40 + 3 * y + x, 180 - 2 * x - 3 * y)
    )


@dataclass(frozen=True)
class FixtureProfile:
    """Bind a reproducible source to its independent encoder/Pillow checks."""

    filename: str
    pixels: Callable[[], bytes]
    fixture_sha256: str
    pillow_rgb_sha256: str
    allow_screen_content_tools: bool


PROFILES = {
    "quadrant": FixtureProfile(
        "coverage_lossy_420_square8_four_leaves_01.avif",
        quadrant_pixels,
        "c0465a00209870571f58be71cb122d5c42ae8d19e91ae001d8f3b706e7205255",
        "a8e0fdcf9fc9fde209db6dbb71c23dae9a25996b6cef24808e64e03ff5e38e64",
        True,
    ),
    "gradient": FixtureProfile(
        "portable_lossy_420_square8_gradient.avif",
        gradient_pixels,
        "8e7e7bcdfc9cd34e88e49c0b18b0e010529d62d3c526266d286cf32746f41671",
        "82bc76a521907851c6cd20dace4a458d98387a5786b1e9551add7c0cec14d3f3",
        False,
    ),
}


def inspect_av1(path: Path, profile: FixtureProfile) -> None:
    """Check the AV1 profile and lossy 4:2:0 frame controls."""

    from inspect_av1_obus import inspect

    report = inspect(path)
    headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    frames = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    if len(headers) != 1 or len(frames) != 1:
        raise RuntimeError("expected one AV1 sequence and frame header")

    sequence = headers[0]
    sequence_fields = (
        sequence["profile"],
        sequence["bit_depth"],
        sequence["color_primaries"],
        sequence["transfer_characteristics"],
        sequence["matrix_coefficients"],
        sequence["color_range"],
        sequence["subsampling_x"],
        sequence["subsampling_y"],
    )
    if sequence_fields != (0, 8, 1, 13, 6, 1, 1, 1):
        raise RuntimeError(f"unexpected AV1 sequence parameters: {sequence_fields!r}")

    frame = frames[0]
    quantization = frame["quantization"]
    cdef = frame["cdef"]
    loop_filter = frame["loop_filter"]
    if not (
        frame["frame_width"] == 16
        and frame["frame_height"] == 16
        and frame["allow_screen_content_tools"] is profile.allow_screen_content_tools
        and frame["allow_intrabc"] is False
        and frame["all_lossless"] is False
        and frame["delta_q"] == {"present": True, "resolution_log2": 0}
        and quantization["base"] == 4
        and quantization["using_matrix"]
        and quantization["matrix_y"] == 10
        and quantization["matrix_u"] == 10
        and quantization["matrix_v"] == 10
        and cdef["bits"] == 0
        and cdef["y_strengths"] == [0]
        and cdef["uv_strengths"] == [0]
        and loop_filter["level_y"] == [0, 0]
        and loop_filter["level_u"] == 0
        and loop_filter["level_v"] == 0
    ):
        raise RuntimeError("AV1 frame parameters do not match the lossy 4:2:0 case")


def validate_pillow(path: Path, profile: FixtureProfile) -> None:
    """Check Pillow's observable decode mode, dimensions, and RGB bytes."""

    with Image.open(path) as image:
        if image.format != "AVIF" or image.size != (16, 16) or image.mode != "RGB":
            raise RuntimeError("Pillow opened the AVIF with unexpected image info")
        image.verify()

    with Image.open(path) as image:
        image.load()
        if image.mode != "RGB" or image.size != (16, 16):
            raise RuntimeError("Pillow decoded the AVIF with unexpected shape")
        digest = hashlib.sha256(image.tobytes()).hexdigest()
    if digest != profile.pillow_rgb_sha256:
        raise RuntimeError(f"Pillow RGB bytes differ: {digest}")


def encode(avifenc: Path, source_path: Path, output_path: Path) -> None:
    """Encode one candidate with the pinned AOM partition controls."""

    subprocess.run(
        [
            str(avifenc),
            "--depth",
            "8",
            "--yuv",
            "420",
            "--qcolor",
            "99",
            "--speed",
            "8",
            "--jobs",
            "1",
            "--advanced",
            "min-partition-size=8",
            "--advanced",
            "max-partition-size=8",
            str(source_path),
            str(output_path),
        ],
        check=True,
        capture_output=True,
        text=True,
    )


def generate(avifenc: Path, output: Path, profile: FixtureProfile) -> None:
    """Generate and validate the deterministic AVIF fixture."""

    avifenc = avifenc.expanduser().resolve()
    if not avifenc.is_file():
        raise RuntimeError(f"avifenc does not exist: {avifenc}")
    version = subprocess.run(
        [str(avifenc), "--version"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    version_line = version[0] if version else ""
    if version_line != AVIFENC_VERSION:
        raise RuntimeError(
            f"expected pinned avifenc {AVIFENC_VERSION!r}, found {version_line!r}"
        )

    with tempfile.TemporaryDirectory(prefix="avif-lossy-420-square8-") as temporary:
        directory = Path(temporary)
        source_path = directory / "source.png"
        first_path = directory / "first.avif"
        second_path = directory / "second.avif"
        Image.frombytes("RGB", (16, 16), profile.pixels()).save(
            source_path, format="PNG"
        )
        encode(avifenc, source_path, first_path)
        encode(avifenc, source_path, second_path)
        encoded = first_path.read_bytes()
        digest = hashlib.sha256(encoded).hexdigest()
        if encoded != second_path.read_bytes() or digest != profile.fixture_sha256:
            raise RuntimeError(f"generated AVIF hash differs: {digest}")
        inspect_av1(first_path, profile)
        validate_pillow(first_path, profile)

        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(first_path, output)
    print(f"Wrote {output} ({len(encoded)} bytes, sha256 {profile.fixture_sha256})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--avifenc",
        type=Path,
        required=True,
        help="avifenc built with libavif 1.4.1 and libaom 3.13.2",
    )
    parser.add_argument("--profile", choices=PROFILES, default="quadrant")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    profile = PROFILES[args.profile]
    output = args.output or ROOT / "tests/fixtures/input/images/avif" / profile.filename
    generate(args.avifenc, output.expanduser().resolve(), profile)


if __name__ == "__main__":
    main()
