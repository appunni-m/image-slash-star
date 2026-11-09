#!/usr/bin/env python3
"""Generate a pinned 16x8 lossy 4:2:0 bounded-partition AVIF fixture."""

from __future__ import annotations

import argparse
import hashlib
import tempfile
from io import BytesIO
from pathlib import Path

import PIL
from PIL import Image, _avif, features

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "coverage_lossy_420_bounded_partition_16x8_01.avif"
)
PILLOW_VERSION = "12.2.0"
AVIF_VERSION = "1.4.1"
CODEC_VERSIONS = ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2")
FIXTURE_SHA256 = "b02d5f24285af413750836f1b1ac019f6830b914fe1ac4772dcfb264ee5f891f"
PIXELS_SHA256 = "f5028deeedbdbd9fd8bd2234c1f9356b0b1cb349bc7a668e4a21f08fe6b65aed"
SIZE = (16, 8)


def source_image() -> Image.Image:
    """Build the split-contrast source image from fixed RGB samples."""

    pixels = bytes(
        channel
        for _y in range(SIZE[1])
        for x in range(SIZE[0])
        for channel in ((17, 91, 203) if x < 8 else (22, 96, 208))
    )
    return Image.frombytes("RGB", SIZE, pixels)


def encode() -> bytes:
    output = BytesIO()
    source_image().save(
        output,
        format="AVIF",
        quality=99,
        speed=8,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        advanced={"min-partition-size": "8", "max-partition-size": "8"},
    )
    return output.getvalue()


def validate(encoded: bytes) -> None:
    if hashlib.sha256(encoded).hexdigest() != FIXTURE_SHA256:
        raise RuntimeError("generated bounded-partition AVIF differs from its pinned hash")

    with Image.open(BytesIO(encoded)) as decoded:
        decoded.load()
        if decoded.format != "AVIF" or decoded.size != SIZE or decoded.mode != "RGB":
            raise RuntimeError("Pillow decoded the bounded-partition AVIF unexpectedly")
        if hashlib.sha256(decoded.tobytes()).hexdigest() != PIXELS_SHA256:
            raise RuntimeError("Pillow pixels differ from the pinned RGB reference")

    from inspect_av1_obus import inspect

    with tempfile.TemporaryDirectory(prefix="avif-lossy-420-bounded-partition-") as temp:
        candidate = Path(temp) / "candidate.avif"
        candidate.write_bytes(encoded)
        report = inspect(candidate)

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
        raise RuntimeError("expected one AV1 sequence header and frame header")

    sequence = sequence_headers[0]
    if (
        sequence["bit_depth"],
        sequence["subsampling_x"],
        sequence["subsampling_y"],
    ) != (8, True, True):
        raise RuntimeError("AV1 sequence is not 8-bit 4:2:0")

    frame = frame_headers[0]
    quantization = frame["quantization"]
    if not (
        frame["frame_width"] == SIZE[0]
        and frame["frame_height"] == SIZE[1]
        and frame["all_lossless"] is False
        and frame["delta_q"] == {"present": True, "resolution_log2": 0}
        and quantization["base"] == 4
        and quantization["using_matrix"]
        and quantization["matrix_y"] == 10
        and quantization["matrix_u"] == 10
        and quantization["matrix_v"] == 10
        and frame["cdef"]["bits"] == 0
        and frame["loop_filter"]["level_y"] == [0, 0]
        and frame["loop_filter"]["level_u"] == 0
        and frame["loop_filter"]["level_v"] == 0
        and frame["restoration"]["types"] == [0, 0, 0]
    ):
        raise RuntimeError("AV1 frame parameters differ from the bounded-partition case")


def generate(output: Path, check: bool) -> None:
    if PIL.__version__ != PILLOW_VERSION:
        raise RuntimeError(f"Pillow {PILLOW_VERSION} is required, found {PIL.__version__}")
    if features.version("avif") != AVIF_VERSION:
        raise RuntimeError(f"libavif {AVIF_VERSION} is required")
    codec_versions = _avif.codec_versions()
    for version in CODEC_VERSIONS:
        if version not in codec_versions:
            raise RuntimeError(f"AVIF fixture requires {version}: {codec_versions}")

    encoded = encode()
    if encoded != encode():
        raise RuntimeError("bounded-partition AVIF encoding is not deterministic")
    validate(encoded)

    if check:
        if not output.is_file() or output.read_bytes() != encoded:
            raise RuntimeError(f"checked-in AVIF fixture differs: {output}")
        action = "Verified"
    else:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(encoded)
        action = "Wrote"
    print(f"{action} {output} ({len(encoded)} bytes, sha256 {FIXTURE_SHA256})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    generate(args.output.expanduser().resolve(), args.check)


if __name__ == "__main__":
    main()
