#!/usr/bin/env python3
"""Generate a deterministic profile-2 12-bit identity-matrix AVIF sequence."""

from __future__ import annotations

import argparse
import hashlib
import io
from pathlib import Path
import subprocess
import sys
import tempfile

import PIL
from PIL import Image, _avif, features

from avif_bmff_boxes import normalize_creation_timestamps


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_identity_matrix_profile2_12bit_444.avif"
)
EXPECTED_SHA256 = "d0079f47a1f7709a373191d2248d42daec16d01132b6aaf1cdb1d7dc613fb1da"
EXPECTED_CODECS = "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
EXPECTED_AVIFENC_VERSION = "Version: 1.4.1 (aom [enc]:3.13.2)"
EXPECTED_SEQUENCE = (2, 12, 1, 13, 0, True, False, False)
FRAME_SIZE = (64, 64)
FRAME_MODE = "RGB"
FRAME_COUNT = 5
FRAME_DURATION_MS = 33


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def frame_pixels(frame_index: int) -> bytes:
    pixels = bytearray(FRAME_SIZE[0] * FRAME_SIZE[1] * 3)
    offset = 0
    for y in range(FRAME_SIZE[1]):
        for x in range(FRAME_SIZE[0]):
            pixels[offset : offset + 3] = bytes(
                (
                    (x * 7 + frame_index * 23) & 0xFF,
                    (y * 5 + frame_index * 41) & 0xFF,
                    ((x + y) * 3 + frame_index * 67) & 0xFF,
                )
            )
            offset += 3
    return bytes(pixels)


def encode(avifenc_path: Path) -> bytes:
    require(PIL.__version__ == "12.2.0", "Pillow source-image version changed")
    require(features.version("avif") == "1.4.1", "libavif version changed")
    require(
        _avif.codec_versions() == EXPECTED_CODECS,
        "AVIF decoder or encoder codec version changed",
    )

    avifenc = avifenc_path.expanduser().resolve()
    require(avifenc.is_file(), f"avifenc does not exist: {avifenc}")
    version = subprocess.run(
        [str(avifenc), "--version"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    require(
        version.splitlines()[0] == EXPECTED_AVIFENC_VERSION,
        f"expected pinned avifenc, found {version!r}",
    )

    with tempfile.TemporaryDirectory(prefix="avif-profile2-identity-sequence-") as temporary:
        directory = Path(temporary)
        inputs = []
        for frame_index in range(FRAME_COUNT):
            path = directory / f"frame_{frame_index}.png"
            Image.frombytes(FRAME_MODE, FRAME_SIZE, frame_pixels(frame_index)).save(
                path, format="PNG"
            )
            inputs.append(path)

        output = directory / "identity_profile2_12bit_444.avif"
        command = [
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
            "--timescale",
            "1000",
            "--duration",
            str(FRAME_DURATION_MS),
            "--creation-time",
            "0",
            "--modification-time",
            "0",
            "--repetition-count",
            "infinite",
            *(str(path) for path in inputs),
            str(output),
        ]
        result = subprocess.run(command, capture_output=True, text=True)
        require(
            result.returncode == 0,
            f"pinned avifenc failed: {result.stderr.strip() or result.stdout.strip()}",
        )
        return normalize_creation_timestamps(output.read_bytes())


def inspect_from_bytes(data: bytes) -> dict[str, object]:
    from tempfile import NamedTemporaryFile

    from inspect_av1_obus import inspect

    with NamedTemporaryFile(suffix=".avif") as handle:
        handle.write(data)
        handle.flush()
        return inspect(Path(handle.name))


def validate_output(data: bytes) -> None:
    digest = hashlib.sha256(data).hexdigest()
    require(digest == EXPECTED_SHA256, f"generated AVIF hash changed: {digest}")

    report = inspect_from_bytes(data)
    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample.get("obus", [])
        if "sequence_header" in obu
    ]
    require(
        len(sequence_headers) == 2,
        "generated AVIF must contain two repeated AV1 sequence headers",
    )
    require(
        all(
            not sequence["reduced_still_picture_header"]
            and (
                sequence["profile"],
                sequence["bit_depth"],
                sequence["color_primaries"],
                sequence["transfer_characteristics"],
                sequence["matrix_coefficients"],
                bool(sequence["color_range"]),
                bool(sequence["subsampling_x"]),
                bool(sequence["subsampling_y"]),
            )
            == EXPECTED_SEQUENCE
            for sequence in sequence_headers
        ),
        "generated AVIF does not use a valid profile-2 identity matrix",
    )

    with Image.open(io.BytesIO(data)) as image:
        require(image.format == "AVIF", "Pillow did not identify generated AVIF")
        require(
            image.size == FRAME_SIZE and image.n_frames == FRAME_COUNT,
            "generated AVIF canvas or frame count changed",
        )
        for frame_index in range(image.n_frames):
            image.seek(frame_index)
            image.load()
            require(image.mode == FRAME_MODE, "generated AVIF frame is not RGB")
            require(
                image.info.get("duration") == FRAME_DURATION_MS,
                "generated frame duration changed",
            )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--avifenc",
        type=Path,
        help="pinned libavif 1.4.1 avifenc used to reproduce the fixture",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.check and args.avifenc is None:
        if not OUTPUT.is_file():
            raise SystemExit(f"missing fixture: {OUTPUT.relative_to(ROOT)}")
        data = OUTPUT.read_bytes()
    else:
        if args.avifenc is None:
            parser.error("--avifenc is required when generating or reproducing the fixture")
        data = encode(args.avifenc)

    validate_output(data)
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != data:
            raise SystemExit(
                f"{OUTPUT.relative_to(ROOT)} differs from the deterministic fixture"
            )
        action = "verified"
    else:
        if OUTPUT.exists() and OUTPUT.read_bytes() != data:
            raise SystemExit(f"refusing to replace different fixture at {OUTPUT}")
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_bytes(data)
        action = "wrote"
    print(f"{action} {OUTPUT.relative_to(ROOT)} sha256={EXPECTED_SHA256}")


if __name__ == "__main__":
    main()
