#!/usr/bin/env python3
"""Generate the pinned AVIF witness for single-reference extended MV search."""

from __future__ import annotations

import argparse
import hashlib
import random
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from generate_avif_10bit_lossless_inter_fixture import normalize_sequence_timestamps
from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parent.parent
FILENAME = "animated_lossy_inter_420_single_extended_ref_128x128.avif"
OUTPUT = ROOT / "tests/fixtures/input/images/avif" / FILENAME
FILE_SHA256 = "6fe9058db08a06e520922eb2d63d2dc1111d842e1f8d26e023acd560fbdde271"
FRAME_SHA256 = (
    "959739d6be415dc39d76b26ca3f0772484657826d95210cf2655a42737d39175",
    "c8dde6303bfbf7e1768f6e342955c1402e3dd810d7f7fa54ae97e0f9fde7d289",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
WIDTH = 128
HEIGHT = 128


def check_pinned_oracle() -> None:
    """Require the Pillow AVIF encoder used to pin this fixture."""
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned AVIF oracle required; found {observed!r}")


def make_frames() -> list[Image.Image]:
    """Make a mostly unchanged frame pair with one deterministic noisy patch."""
    reference = bytearray(WIDTH * HEIGHT * 3)
    for y in range(HEIGHT):
        for x in range(WIDTH):
            offset = (y * WIDTH + x) * 3
            if y < 64:
                cell = (x // 16) * 17 + (y // 16) * 31
                value = 30 + ((x * 3 + y * 5 + cell) % 80)
            else:
                value = 96
            reference[offset : offset + 3] = bytes((value, value, value))

    changed = bytearray(reference)
    random_source = random.Random(9917)
    for y in range(48, 64):
        for x in range(16):
            offset = (y * WIDTH + x) * 3
            value = random_source.randrange(256)
            changed[offset : offset + 3] = bytes((value, value, value))

    return [
        Image.frombytes("RGB", (WIDTH, HEIGHT), bytes(frame))
        for frame in (reference, changed)
    ]


def encode_once() -> bytes:
    """Encode the two I420 frames with the fixture's pinned AOM settings."""
    frames = make_frames()
    output = BytesIO()
    frames[0].save(
        output,
        format="AVIF",
        save_all=True,
        append_images=frames[1:],
        duration=[100, 100],
        loop=0,
        quality=60,
        speed=0,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        codec="aom",
        advanced={
            "sb-size": "64",
            "min-partition-size": "8",
            "max-partition-size": "64",
            "aq-mode": "0",
            "deltaq-mode": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "enable-global-motion": "0",
            "enable-warped-motion": "0",
        },
    )
    return normalize_sequence_timestamps(output.getvalue())


def encode_fixture() -> bytes:
    """Require byte-identical repeat encodes after normalizing BMFF clocks."""
    first = encode_once()
    second = encode_once()
    if first != second:
        raise RuntimeError("single-reference extended-MV AVIF is not deterministic")
    return first


def validate_pillow_frames(encoded: bytes) -> None:
    """Check exact pinned Pillow metadata and pixels for both frames."""
    with Image.open(BytesIO(encoded)) as sequence:
        if (
            sequence.format != "AVIF"
            or sequence.mode != "RGB"
            or sequence.size != (WIDTH, HEIGHT)
            or sequence.n_frames != len(FRAME_SHA256)
        ):
            raise RuntimeError("Pillow AVIF metadata differs from the pinned fixture")
        for index, expected_hash in enumerate(FRAME_SHA256):
            sequence.seek(index)
            sequence.load()
            pixels = sequence.convert("RGB").tobytes()
            if (
                len(pixels) != WIDTH * HEIGHT * 3
                or hashlib.sha256(pixels).hexdigest() != expected_hash
                or sequence.info.get("duration") != 100
            ):
                raise RuntimeError(f"Pillow frame {index} differs from its pinned reference")


def validate_av1_headers(encoded: bytes) -> None:
    """Require an 8-bit I420 key/inter sequence with single-reference syntax."""
    staging = ROOT / "target" / "oracle-staging"
    staging.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="avif-single-extended-ref-", dir=staging) as directory:
        fixture = Path(directory) / FILENAME
        fixture.write_bytes(encoded)
        report = inspect(fixture)

    samples = sorted(
        (sample for sample in report["samples"] if sample.get("role") == "track_pict"),
        key=lambda sample: sample["identity"]["sample"],
    )
    if len(samples) != 2:
        raise RuntimeError("AVIF must contain exactly two picture-track samples")

    sequence_headers = [
        obu["sequence_header"]
        for sample in samples
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if not sequence_headers:
        raise RuntimeError("AV1 picture samples must declare a sequence header")
    sequence = sequence_headers[0]
    if (
        any(header != sequence for header in sequence_headers[1:])
        or sequence.get("profile") != 0
        or sequence.get("bit_depth") != 8
        or sequence.get("monochrome") is not False
        or sequence.get("subsampling_x") != 1
        or sequence.get("subsampling_y") != 1
    ):
        raise RuntimeError("sequence header differs from the pinned 8-bit I420 profile")

    frame_headers = []
    for sample in samples:
        headers = [obu["frame_header"] for obu in sample["obus"] if "frame_header" in obu]
        if len(headers) != 1:
            raise RuntimeError("each AV1 track sample must contain one frame header")
        frame_headers.extend(headers)
    key, inter = frame_headers
    if (
        key.get("frame_type") != "key"
        or inter.get("frame_type") != "inter"
        or inter.get("frame_width") != WIDTH
        or inter.get("frame_height") != HEIGHT
        or inter.get("reference_mode") != "single"
        or inter.get("all_lossless") is not False
        or inter.get("quantization", {}).get("base", 0) <= 0
    ):
        raise RuntimeError("inter frame differs from the pinned lossy single-reference profile")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the checked-in fixture without writing it",
    )
    args = parser.parse_args()
    check_pinned_oracle()

    output = args.output.resolve()
    if args.check:
        if not output.is_file():
            raise RuntimeError(f"fixture does not exist: {output}")
        encoded = output.read_bytes()
        if normalize_sequence_timestamps(encoded) != encoded:
            raise RuntimeError("fixture BMFF timestamps are not normalized")
    else:
        encoded = encode_fixture()
        if output.exists() and output.read_bytes() != encoded:
            raise RuntimeError(f"refusing to replace a differing fixture: {output}")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(encoded)

    file_hash = hashlib.sha256(encoded).hexdigest()
    if file_hash != FILE_SHA256:
        raise RuntimeError(f"fixture hash changed: {file_hash}")
    validate_pillow_frames(encoded)
    validate_av1_headers(encoded)
    print(f"Verified {output} ({len(encoded)} bytes, sha256={file_hash})")


if __name__ == "__main__":
    main()
