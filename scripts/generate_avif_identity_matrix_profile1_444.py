#!/usr/bin/env python3
"""Generate a deterministic AVIF sequence using the profile-1 identity matrix."""

from __future__ import annotations

import argparse
import hashlib
import io
from pathlib import Path
import sys

import PIL
from PIL import Image, _avif, features

from avif_bmff_boxes import normalize_creation_timestamps


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_identity_matrix_profile1_444.avif"
)
EXPECTED_SHA256 = "4535db4548ddf82d23c72ef970b01f1632f4db4c9f7ddfd392bf0d5d011bdd4a"
EXPECTED_CODECS = "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
EXPECTED_SEQUENCE = (1, 8, 1, 13, 0, True, False, False)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def input_bytes() -> bytes:
    require(PIL.__version__ == "12.2.0", "Pillow oracle version changed")
    require(features.version("avif") == "1.4.1", "libavif version changed")
    require(
        _avif.codec_versions() == EXPECTED_CODECS,
        "AVIF decoder or encoder codec version changed",
    )

    frames = []
    for frame_index in range(5):
        image = Image.new("RGB", (64, 64))
        pixels = image.load()
        for y in range(64):
            for x in range(64):
                pixels[x, y] = (
                    (x * 7 + frame_index * 23) & 0xFF,
                    (y * 5 + frame_index * 41) & 0xFF,
                    ((x + y) * 3 + frame_index * 67) & 0xFF,
                )
        frames.append(image)

    encoded = io.BytesIO()
    frames[0].save(
        encoded,
        format="AVIF",
        save_all=True,
        append_images=frames[1:],
        duration=[33] * len(frames),
        quality=100,
        speed=8,
        max_threads=1,
        codec="aom",
        subsampling="4:4:4",
        advanced={
            "color-primaries": "1",
            "transfer-characteristics": "13",
            "matrix-coefficients": "0",
        },
    )
    data = normalize_creation_timestamps(encoded.getvalue())
    digest = hashlib.sha256(data).hexdigest()
    require(digest == EXPECTED_SHA256, f"generated AVIF hash changed: {digest}")
    validate_output(data)
    return data


def validate_output(data: bytes) -> None:
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
        "generated AVIF does not use the valid profile-1 identity matrix",
    )

    with Image.open(io.BytesIO(data)) as image:
        require(image.format == "AVIF", "Pillow did not identify generated AVIF")
        require(
            image.size == (64, 64) and image.n_frames == 5,
            "generated AVIF canvas changed",
        )
        for frame_index in range(image.n_frames):
            image.seek(frame_index)
            image.load()
            require(image.mode == "RGB", "generated AVIF frame is not RGB")
            require(
                image.info.get("duration") == 33,
                "generated frame duration changed",
            )


def inspect_from_bytes(data: bytes) -> dict[str, object]:
    from tempfile import NamedTemporaryFile

    from inspect_av1_obus import inspect

    with NamedTemporaryFile(suffix=".avif") as handle:
        handle.write(data)
        handle.flush()
        return inspect(Path(handle.name))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    data = input_bytes()
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != data:
            raise SystemExit(
                f"{OUTPUT.relative_to(ROOT)} differs from its deterministic encoder input"
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
