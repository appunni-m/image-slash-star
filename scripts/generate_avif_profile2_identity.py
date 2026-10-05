#!/usr/bin/env python3
"""Generate the pinned profile-2 12-bit identity-CICP AVIF parity fixture."""

from __future__ import annotations

import argparse
import hashlib
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/avif/profile2_identity_12bit_444.avif"
)
AVIFENC_VERSION = "Version: 1.4.1 (aom [enc]:v3.13.2)"
FIXTURE_SHA256 = "1ce2aae5539074e3a65092f185c4db34537462d637859d4fd85449bb15557931"
PILLOW_RGB_SHA256 = "45114693c3cde135ae810d49fbfb8bcf54204da4b9bf11fa3094b5e177686343"


def source_pixels() -> bytes:
    pixels = bytearray(16 * 16 * 3)
    offset = 0
    for y in range(16):
        for x in range(16):
            pixels[offset : offset + 3] = bytes(
                ((x * 17 + y * 3) & 0xFF, (y * 19 + x * 5) & 0xFF, (x * 11 + y * 13) & 0xFF)
            )
            offset += 3
    return bytes(pixels)


def inspect_sequence(path: Path) -> None:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from inspect_av1_obus import inspect

    report = inspect(path)
    headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if obu["name"] == "sequence_header"
    ]
    if len(headers) != 1:
        raise RuntimeError("expected one AV1 sequence header in identity fixture")
    header = headers[0]
    actual = (
        header["profile"],
        header["bit_depth"],
        header["color_primaries"],
        header["transfer_characteristics"],
        header["matrix_coefficients"],
        header["color_range"],
        header["subsampling_x"],
        header["subsampling_y"],
    )
    expected = (2, 12, 1, 13, 0, 1, 0, 0)
    if actual != expected:
        raise RuntimeError(f"identity fixture sequence differs: {actual!r}")


def validate_pillow(path: Path) -> None:
    with Image.open(path) as image:
        if image.format != "AVIF" or image.size != (16, 16) or image.mode != "RGB":
            raise RuntimeError("Pillow opened identity fixture with unexpected image info")
        image.verify()

    with Image.open(path) as image:
        image.load()
        if image.mode != "RGB" or image.size != (16, 16):
            raise RuntimeError("Pillow decoded identity fixture with unexpected shape")
        digest = hashlib.sha256(image.tobytes()).hexdigest()
    if digest != PILLOW_RGB_SHA256:
        raise RuntimeError(f"Pillow RGB bytes differ: {digest}")


def generate(avifenc: Path, output: Path) -> None:
    avifenc = avifenc.expanduser().resolve()
    if not avifenc.is_file():
        raise RuntimeError(f"avifenc does not exist: {avifenc}")
    version = subprocess.run(
        [str(avifenc), "--version"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    version_line = version.splitlines()[0] if version else ""
    if version_line != AVIFENC_VERSION:
        raise RuntimeError(f"expected pinned avifenc {AVIFENC_VERSION!r}, found {version!r}")

    with tempfile.TemporaryDirectory(prefix="avif-profile2-identity-") as temporary:
        directory = Path(temporary)
        source_path = directory / "source.png"
        encoded_path = directory / "profile2_identity_12bit_444.avif"
        Image.frombytes("RGB", (16, 16), source_pixels()).save(source_path, format="PNG")
        subprocess.run(
            [
                str(avifenc),
                "--depth",
                "12",
                "--yuv",
                "444",
                "--qcolor",
                "100",
                "--cicp",
                "1/13/0",
                "--range",
                "full",
                "--speed",
                "8",
                "--jobs",
                "1",
                str(source_path),
                str(encoded_path),
            ],
            check=True,
        )
        encoded = encoded_path.read_bytes()
        digest = hashlib.sha256(encoded).hexdigest()
        if digest != FIXTURE_SHA256:
            raise RuntimeError(f"generated AVIF hash differs: {digest}")
        inspect_sequence(encoded_path)
        validate_pillow(encoded_path)

        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(encoded_path, output)
    print(f"Wrote {output} ({len(encoded)} bytes, sha256 {FIXTURE_SHA256})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--avifenc",
        type=Path,
        required=True,
        help="avifenc built with libavif 1.4.1 and libaom 3.13.2",
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    generate(args.avifenc, args.output.expanduser().resolve())


if __name__ == "__main__":
    main()
