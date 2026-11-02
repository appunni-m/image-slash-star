#!/usr/bin/env python3
"""Generate a pinned AVIF sequence for the distance-weight ratio edge."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path
from tempfile import TemporaryDirectory

from PIL import Image, _avif, features
from PIL import __version__ as PILLOW_VERSION

from generate_avif_interinter_distance_altref import truncate_track_samples
from generate_avif_interinter_distance_i420 import normalize_timestamps
from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parent.parent
FILENAME = "animated_lossy_interinter_distance_ratio_edge_i420_b16x16_64x64.avif"
OUTPUT = ROOT / "tests/fixtures/input/images/avif" / FILENAME
FILE_SHA256 = "abc38ca334493341926ff5f728ece919a1b71569cd8bfd4dec08f18d180811ba"
FRAME_SHA256 = (
    "ec6173a5461ba786931283814af599d5751f1f656cbfb189de6ff0aaa304508a",
    "b8df4280090e8dbe984b13baccc0600e066ad891fcd24dea4024dc92328e440b",
    "93a6c9e9b5d21b4548591915a6fabdf49cf85d41ab6257397cc399d4ee5a1477",
    "afa087bc42bbaa0f9ad5508973f9a7390184e4e5b810d998f08acefccd2b4432",
    "2203a98fd84adfbd3cbbfda33c94fcc2d33f0e1d6ebfa4e0ced17c5469ba54e5",
    "be3efffdbf5efe8638e56eb51305ca5c4ca5f52155e670c621bb9c8044c54846",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
WIDTH = 64
HEIGHT = 64
INPUT_FRAME_COUNT = 8
TRACK_SAMPLE_COUNT = 6


def check_pinned_oracle() -> None:
    """Require the pinned Pillow and AVIF codec versions."""
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned AVIF oracle required; found {observed!r}")


def checkerboard() -> bytes:
    """Return an 8x8-pixel checker pattern used by the two references."""
    return bytes(
        255 if ((x // 8 + y // 8) & 1) else 0
        for y in range(HEIGHT)
        for x in range(WIDTH)
    )


def gray_image(gray: bytes) -> Image.Image:
    """Expand one grayscale plane into the RGB input Pillow encodes."""
    if len(gray) != WIDTH * HEIGHT:
        raise ValueError("grayscale frame has an unexpected sample count")
    pixels = bytes(channel for value in gray for channel in (value, value, value))
    return Image.frombytes("RGB", (WIDTH, HEIGHT), pixels)


def make_frames() -> list[Image.Image]:
    """Make references at order hints 3 and 6 and their 5:11 blend at 5."""
    first = checkerboard()
    second = bytes(255 - value for value in first)
    blend = bytes((near * 5 + future * 11 + 8) // 16 for near, future in zip(first, second))
    planes = [
        bytes([32]) * (WIDTH * HEIGHT),
        bytes([64]) * (WIDTH * HEIGHT),
        bytes([80]) * (WIDTH * HEIGHT),
        first,
        bytes([128]) * (WIDTH * HEIGHT),
        blend,
        second,
        bytes([192]) * (WIDTH * HEIGHT),
    ]
    return [gray_image(plane) for plane in planes]


def encode_full_sequence(frames: list[Image.Image]) -> bytes:
    """Encode a deterministic altref sequence with distance compound enabled."""
    if len(frames) != INPUT_FRAME_COUNT:
        raise ValueError("distance-ratio sequence has an unexpected input frame count")
    output = BytesIO()
    frames[0].save(
        output,
        format="AVIF",
        save_all=True,
        append_images=frames[1:],
        duration=[100] * len(frames),
        loop=0,
        quality=70,
        speed=0,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        codec="aom",
        advanced={
            "auto-alt-ref": "1",
            "arnr-maxframes": "1",
            "arnr-strength": "0",
            "enable-dist-wtd-comp": "1",
            "enable-masked-comp": "0",
            "enable-interintra-comp": "0",
            "enable-global-motion": "0",
            "enable-warped-motion": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "sb-size": "64",
            "min-partition-size": "16",
            "max-partition-size": "16",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
    )
    return normalize_timestamps(output.getvalue())


def encode_fixture() -> bytes:
    """Encode the eight source frames and retain the six-sample sequence."""
    return truncate_track_samples(encode_full_sequence(make_frames()), TRACK_SAMPLE_COUNT)


def pillow_frame_hashes(encoded: bytes) -> tuple[str, ...]:
    """Check all six decoded RGB frames and their durations with Pillow."""
    with Image.open(BytesIO(encoded)) as sequence:
        if (
            sequence.format != "AVIF"
            or sequence.mode != "RGB"
            or sequence.size != (WIDTH, HEIGHT)
            or sequence.n_frames != TRACK_SAMPLE_COUNT
        ):
            raise RuntimeError("Pillow AVIF format, mode, dimensions, or frame count differ")
        hashes = []
        for index in range(sequence.n_frames):
            sequence.seek(index)
            sequence.load()
            if sequence.info.get("duration") != 100:
                raise RuntimeError("Pillow AVIF frame duration differs")
            hashes.append(hashlib.sha256(sequence.convert("RGB").tobytes()).hexdigest())
    return tuple(hashes)


def verify_av1_headers(encoded: bytes) -> None:
    """Require selectable distance-compound syntax in the final shown frame."""
    staging = ROOT / "target" / "oracle-staging"
    staging.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory(prefix="avif-distance-ratio-", dir=staging) as directory:
        fixture_path = Path(directory) / FILENAME
        fixture_path.write_bytes(encoded)
        report = inspect(fixture_path)

    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if not sequence_headers or any(
        not header.get("enable_order_hint")
        or not header.get("enable_jnt_comp")
        or header.get("enable_masked_compound")
        or header.get("enable_warped_motion")
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence does not enable distance-only compound syntax")

    final_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if obu.get("frame_header", {}).get("order_hint") == 5
    ]
    if len(final_headers) != 1:
        raise RuntimeError("expected one final inter frame header with order hint five")
    header = final_headers[0]
    if (
        header.get("frame_type") != "inter"
        or header.get("reference_mode") != "select"
        or header.get("reference_indices") != [5, 2, 4, 0, 3, 0, 1]
    ):
        raise RuntimeError("final frame does not expose the required compound references")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the checked-in input without writing it",
    )
    args = parser.parse_args()
    check_pinned_oracle()

    encoded = encode_fixture()
    if encoded != encode_fixture():
        raise RuntimeError("distance-ratio AVIF is not deterministic")
    file_hash = hashlib.sha256(encoded).hexdigest()
    if file_hash != FILE_SHA256:
        raise RuntimeError(f"fixture hash changed: {file_hash}")
    frame_hashes = pillow_frame_hashes(encoded)
    if frame_hashes != FRAME_SHA256:
        raise RuntimeError(f"Pillow RGB frame hashes changed: {frame_hashes!r}")
    verify_av1_headers(encoded)

    output = args.output.resolve()
    if args.check:
        if not output.is_file() or output.read_bytes() != encoded:
            raise RuntimeError(f"fixture differs from deterministic output: {output}")
    else:
        if output.exists() and output.read_bytes() != encoded:
            raise RuntimeError(f"refusing to replace differing fixture: {output}")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(encoded)
    print(f"Verified {output} ({len(encoded)} bytes, sha256={file_hash})")


if __name__ == "__main__":
    main()
