#!/usr/bin/env python3
"""Generate an I422 mode-select sequence with a repeated 64x64 region."""

from __future__ import annotations

import argparse
import hashlib
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features
from PIL import __version__ as PILLOW_VERSION

from generate_avif_inter_422_compound_direct_grid_fixture import (
    make_frame,
    normalize_timestamps,
)
from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parent.parent
INPUT_DIR = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
FILENAME = "animated_lossy_inter_422_direct_grid_mode_select_mixed_b128x64.avif"
FILE_SHA256 = "7aee9a35ab56e6209dbc11b2feccd8d636edc4c6b7b448d128c66832d6da198e"
FRAME_SHA256 = (
    "54b01010b300655b2bbf26316e8d58b2b71022f3bdd3a0fb90329e8078829f33",
    "67c6f4f457203099e76a6b2737d2b571be975baa0a5ed3c81403370ed31084ca",
    "79a68f04adb0f38511b6e67da73b14e577c7857dc9b7501d820b20c0d90daf74",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def make_frames() -> tuple[Image.Image, Image.Image, Image.Image]:
    """Build two reference frames and a half-updated final frame."""
    first = Image.new("RGB", (128, 64))
    first.paste(make_frame(1096), (0, 0))
    first.paste(make_frame(1097), (64, 0))

    second = Image.new("RGB", (128, 64))
    second.paste(make_frame(2096), (0, 0))
    second.paste(make_frame(2097), (64, 0))

    final = Image.new("RGB", (128, 64))
    final.paste(
        Image.blend(first.crop((0, 0, 64, 64)), second.crop((0, 0, 64, 64)), 0.5),
        (0, 0),
    )
    final.paste(second.crop((64, 0, 128, 64)), (64, 0))
    return first, second, final


def encode_fixture() -> bytes:
    """Encode deterministic I422 blocks with selectable transform sizes."""
    frames = make_frames()
    encoded_runs = []
    for _ in range(2):
        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=list(frames[1:]),
            duration=[100] * len(frames),
            loop=0,
            quality=75,
            speed=4,
            max_threads=1,
            subsampling="4:2:2",
            autotiling=False,
            codec="aom",
            advanced={
                "min-partition-size": "64",
                "max-partition-size": "64",
                "aq-mode": "0",
                "deltaq-mode": "0",
                "enable-cdef": "0",
                "enable-restoration": "0",
                "loopfilter-control": "0",
                "enable-interintra-comp": "0",
                "enable-warped-motion": "0",
                "enable-global-motion": "0",
            },
        )
        encoded_runs.append(normalize_timestamps(output.getvalue()))
    if encoded_runs[0] != encoded_runs[1]:
        raise RuntimeError("repeated AVIF encodes differ")
    return encoded_runs[0]


def verify_pillow_frames(encoded: bytes) -> None:
    """Require exact pinned Pillow metadata and decoded RGB frames."""
    frame_hashes = []
    with Image.open(BytesIO(encoded)) as sequence:
        if (
            sequence.format != "AVIF"
            or sequence.mode != "RGB"
            or sequence.size != (128, 64)
            or sequence.n_frames != len(FRAME_SHA256)
        ):
            raise RuntimeError("Pillow AVIF metadata differs from the pinned fixture")
        for frame_index in range(sequence.n_frames):
            sequence.seek(frame_index)
            sequence.load()
            frame_hashes.append(
                hashlib.sha256(sequence.convert("RGB").tobytes()).hexdigest()
            )
    if tuple(frame_hashes) != FRAME_SHA256:
        raise RuntimeError(f"Pillow frame hashes changed: {tuple(frame_hashes)!r}")


def verify_av1_headers(encoded: bytes) -> None:
    """Require an I422 TX_MODE_SELECT inter frame with positive qindex."""
    with tempfile.TemporaryDirectory(prefix="avif-i422-mode-select-skip-") as directory:
        fixture = Path(directory) / FILENAME
        fixture.write_bytes(encoded)
        report = inspect(fixture)

    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if not sequence_headers or any(
        header.get("profile") != 2
        or header.get("bit_depth") != 8
        or header.get("monochrome")
        or header.get("subsampling_x") != 1
        or header.get("subsampling_y") != 0
        for header in sequence_headers
    ):
        raise RuntimeError("the AV1 sequence must use 8-bit I422 profile 2")

    final_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if obu.get("frame_header", {}).get("order_hint") == 2
    ]
    if len(final_headers) != 1:
        raise RuntimeError("expected one final frame header with order hint two")
    header = final_headers[0]
    if (
        header.get("frame_type") != "inter"
        or header.get("frame_width") != 128
        or header.get("frame_height") != 64
        or header.get("transform_mode") != "select"
        or header.get("all_lossless") is not False
        or header.get("reference_mode") != "single"
        or header.get("reference_indices") != [1, 0, 0, 0, 0, 0, 0]
        or header.get("quantization", {}).get("base", 0) <= 0
    ):
        raise RuntimeError("final inter frame differs from the TX_MODE_SELECT profile")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the checked-in fixture without writing it",
    )
    args = parser.parse_args()

    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"the pinned Pillow AVIF oracle is required; found {observed!r}")

    encoded = encode_fixture()
    if hashlib.sha256(encoded).hexdigest() != FILE_SHA256:
        raise RuntimeError("the AVIF sequence differs from its pinned hash")
    verify_pillow_frames(encoded)
    verify_av1_headers(encoded)

    output = INPUT_DIR / FILENAME
    if args.check:
        if not output.is_file() or output.read_bytes() != encoded:
            raise RuntimeError(f"the checked-in {FILENAME} is stale")
        print(f"Verified {output.relative_to(ROOT)} ({len(encoded)} bytes)")
        return

    if output.exists() and output.read_bytes() != encoded:
        raise RuntimeError(f"refusing to replace a differing fixture: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded)
    print(f"Wrote {output.relative_to(ROOT)} ({len(encoded)} bytes)")


if __name__ == "__main__":
    main()
