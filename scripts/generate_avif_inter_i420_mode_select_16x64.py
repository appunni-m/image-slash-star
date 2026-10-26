#!/usr/bin/env python3
"""Generate the pinned 16x64 lossy I420 AVIF sequence with TX_MODE_SELECT."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path
import tempfile

from PIL import Image, __version__ as PILLOW_VERSION, _avif, features

from generate_avif_10bit_lossless_inter_fixture import normalize_sequence_timestamps
from generate_avif_inter_i420_mode_largest_16x64 import PINNED_PILLOW, source_frames
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
ASSET = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_lossy_inter_420_mode_select_16x64.avif"
)
EXPECTED_ASSET_SHA256 = "6de7ecf1d5ed5eab61c60fab3899cdc328cf3416ddcc1338bc7823412e64ddd2"
EXPECTED_FRAME_SHA256 = (
    "e214dae742032d39e41f85f61009b8f62d8b79cb428a8a472ca66d4825068e49",
    "911a1abd8f394876eb1a3ea6b58dbc4e90a40b41aa260513dcdc4ce3e9e47b00",
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encode_fixture() -> bytes:
    frames = source_frames()
    output = BytesIO()
    frames[0].save(
        output,
        format="AVIF",
        save_all=True,
        append_images=frames[1:],
        duration=[100, 100],
        loop=0,
        quality=99,
        speed=8,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        codec="aom",
        advanced={
            "min-partition-size": "16",
            "max-partition-size": "64",
            "aq-mode": "0",
            "deltaq-mode": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
        },
    )
    return normalize_sequence_timestamps(output.getvalue())


def validate_oracle() -> None:
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"Pillow/native version differs from pin: {observed!r}")


def validate_frames(data: bytes) -> None:
    with Image.open(BytesIO(data)) as image:
        if image.size != (16, 64) or image.mode != "RGB" or image.n_frames != 2:
            raise RuntimeError("Pillow must decode exactly two 16x64 RGB frames")
        for index, expected_hash in enumerate(EXPECTED_FRAME_SHA256):
            image.seek(index)
            image.load()
            pixels = image.tobytes()
            if (
                len(pixels) != 16 * 64 * 3
                or sha256(pixels) != expected_hash
                or image.info.get("duration") != 100
            ):
                raise RuntimeError(f"Pillow frame {index} differs from its pinned reference")


def validate_frame_headers(data: bytes) -> None:
    with tempfile.TemporaryDirectory(prefix="avif-i420-mode-select-16x64-") as temporary:
        path = Path(temporary) / "fixture.avif"
        path.write_bytes(data)
        report = inspect_obus(path)
    samples = sorted(
        (sample for sample in report["samples"] if sample.get("role") == "track_pict"),
        key=lambda sample: sample["identity"]["sample"],
    )
    if len(samples) != 2:
        raise RuntimeError("AVIF must contain exactly two picture-track samples")
    headers = []
    sequence_headers = []
    for sample in samples:
        sequence_headers.extend(
            obu["sequence_header"] for obu in sample["obus"] if "sequence_header" in obu
        )
        frame_headers = [
            obu["frame_header"] for obu in sample["obus"] if "frame_header" in obu
        ]
        if len(frame_headers) != 1:
            raise RuntimeError("each AV1 picture sample must contain one frame header")
        headers.append(frame_headers[0])
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
    key, inter = headers
    if (
        key.get("frame_type") != "key"
        or inter.get("frame_type") != "inter"
        or inter.get("frame_width") != 16
        or inter.get("frame_height") != 64
        or inter.get("transform_mode") != "select"
        or inter.get("all_lossless") is not False
        or inter.get("quantization", {}).get("base") != 9
    ):
        raise RuntimeError("inter frame differs from the lossy I420 TX_MODE_SELECT profile")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed asset against the deterministic encoder output",
    )
    args = parser.parse_args()

    validate_oracle()
    data = encode_fixture()
    if sha256(data) != EXPECTED_ASSET_SHA256:
        raise RuntimeError("generated AVIF differs from its pinned input hash")
    validate_frames(data)
    validate_frame_headers(data)

    if args.check:
        if not ASSET.is_file() or ASSET.read_bytes() != data:
            raise RuntimeError("committed AVIF differs from its deterministic generator")
    else:
        ASSET.parent.mkdir(parents=True, exist_ok=True)
        ASSET.write_bytes(data)
    print(f"Verified {ASSET.relative_to(ROOT)} ({len(data)} bytes, sha256={sha256(data)})")


if __name__ == "__main__":
    main()
