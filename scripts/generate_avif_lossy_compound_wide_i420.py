#!/usr/bin/env python3
"""Generate a pinned lossy I420 AVIF sequence with wide compound blocks."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path
import random
from tempfile import TemporaryDirectory

from PIL import Image, _avif, features
from PIL import __version__ as PILLOW_VERSION

from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parent.parent
OUTPUTS = {
    "mixed": ROOT
    / "tests/fixtures/input/images/avif/animated_lossy_compound_wide_i420_b128x128.avif",
    "unsplit": ROOT
    / "tests/fixtures/input/images/avif/animated_lossy_compound_wide_i420_unsplit_b128x128.avif",
}
EXPECTED_FILE_SHA256 = {
    "mixed": "d0491ed004f7e6d722ce55b7cbeaa6db1898073a9868b306ef412f89693b7f71",
    "unsplit": "8fed3a09dd26a5baf9aa52918f479ee8f83ebfe49642f8b1c021a86fca9abe35",
}
EXPECTED_FRAME_SHA256 = {
    "mixed": (
        "5197ee67def6e725e34453c3c6f343c1d197feae3a781cb89f9668b68aa780ac",
        "258efcda969af7507f5db5eabb70f01d39a1c641739e9442e7dd341468fefcbf",
        "09fbba2634aec5a52c6e9083d2691ea935f448fa9ba848a82a7f939b6f952bf7",
    ),
    "unsplit": (
        "653cdbf8e6dd54fbdb54cd7a2bdb51b178e8086b45aceb26faf34d76a2dec51d",
        "1b83dab831e91958ef270989a9f7d84e94e05413b103df63df73fff7ddaa78bd",
        "e0e0d384a435e7e440e93a27e5eba7a49d9aaf4f5dcd497070f4274b8e04a0cf",
    ),
}
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
WIDTH = 256
HEIGHT = 256


def normalize_sequence_timestamps(encoded: bytes) -> bytes:
    """Zero version-one BMFF timestamps for reproducible fixture bytes."""
    normalized = bytearray(encoded)
    for box_type in (b"mvhd", b"tkhd", b"mdhd"):
        offset = normalized.find(box_type)
        if offset < 4 or normalized[offset + 4] != 1:
            raise RuntimeError(f"expected a version-one {box_type!r} box")
        if normalized.find(box_type, offset + 4) != -1:
            raise RuntimeError(f"found duplicate {box_type!r} boxes")
        end = offset + 24
        if end > len(normalized):
            raise RuntimeError(f"truncated {box_type!r} box")
        normalized[offset + 8 : end] = bytes(16)
    return bytes(normalized)


def check_pinned_oracle() -> None:
    """Require the exact Pillow and AVIF codec versions used for the fixture."""
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned AVIF oracle required; found {observed!r}")


def make_noise_frame(seed: int) -> Image.Image:
    """Build a seeded full-range RGB frame without external assets."""
    source = random.Random(seed)
    pixels = bytes(
        source.randrange(256) for _ in range(WIDTH * HEIGHT * 3)
    )
    return Image.frombytes("RGB", (WIDTH, HEIGHT), pixels)


def make_frames(*, mixed_topology: bool) -> tuple[Image.Image, Image.Image, Image.Image]:
    """Create two references and either a residual or exact-average final frame."""
    first = make_noise_frame(1096)
    second = make_noise_frame(2096)
    final_pixels = bytearray(Image.blend(first, second, 0.5).tobytes())

    if mixed_topology:
        # Each 128x128 region has a visible residual while the rest matches
        # the average of both references, encouraging a mixed transform tree.
        for block_y in (0, 128):
            for block_x in (0, 128):
                for y in range(block_y + 24, block_y + 56):
                    for x in range(block_x + 24, block_x + 56):
                        offset = (y * WIDTH + x) * 3
                        final_pixels[offset : offset + 3] = bytes((248, 32, 192))

    final = Image.frombytes("RGB", (WIDTH, HEIGHT), bytes(final_pixels))
    return first, second, final


def encode_fixture(*, mixed_topology: bool) -> bytes:
    """Encode a three-frame I420 sequence with 128-pixel inter partitions."""
    first, second, final = make_frames(mixed_topology=mixed_topology)
    output = BytesIO()
    first.save(
        output,
        format="AVIF",
        save_all=True,
        append_images=[second, final],
        duration=[100, 100, 100],
        loop=0,
        quality=80,
        speed=0,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        codec="aom",
        advanced={
            "sb-size": "128",
            "min-partition-size": "128",
            "max-partition-size": "128",
            "aq-mode": "0",
            "deltaq-mode": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "enable-warped-motion": "0",
            "enable-interintra-comp": "0",
            "enable-global-motion": "0",
        },
    )
    return normalize_sequence_timestamps(output.getvalue())


def pillow_frame_hashes(encoded: bytes, expected_frames: int) -> tuple[str, ...]:
    """Hash exact Pillow RGB frames and verify the public sequence metadata."""
    with Image.open(BytesIO(encoded)) as sequence:
        if sequence.format != "AVIF" or sequence.mode != "RGB":
            raise RuntimeError("Pillow AVIF format or mode differs from the fixture")
        if sequence.size != (WIDTH, HEIGHT):
            raise RuntimeError(f"expected 256x256 frames, found {sequence.size}")
        if sequence.n_frames != expected_frames:
            raise RuntimeError(f"expected three frames, found {sequence.n_frames}")
        hashes = []
        for frame_index in range(sequence.n_frames):
            sequence.seek(frame_index)
            sequence.load()
            hashes.append(hashlib.sha256(sequence.convert("RGB").tobytes()).hexdigest())
    return tuple(hashes)


def verify_av1_headers(encoded: bytes) -> None:
    """Require 8-bit I420 and a compound-selectable final 256x256 frame."""
    with TemporaryDirectory() as temporary_directory:
        fixture = Path(temporary_directory) / "compound_wide_i420.avif"
        fixture.write_bytes(encoded)
        report = inspect(fixture)

    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if not sequence_headers or any(
        header.get("bit_depth") != 8
        or header.get("monochrome")
        or header.get("profile") != 0
        or header.get("subsampling_x") != 1
        or header.get("subsampling_y") != 1
        for header in sequence_headers
    ):
        raise RuntimeError("expected the AV1 sequence to use 8-bit I420 profile 0")

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
        or header.get("frame_width") != WIDTH
        or header.get("frame_height") != HEIGHT
        or header.get("reference_mode") != "select"
        or header.get("reference_indices") != [1, 0, 0, 0, 0, 0, 0]
    ):
        raise RuntimeError("the final frame does not expose both compound references")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    parser.add_argument(
        "--variant",
        choices=tuple(OUTPUTS),
        default="mixed",
        help="select a mixed-residual or exact-average final frame",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="require the fixture to match the deterministic pinned encoding",
    )
    args = parser.parse_args()
    check_pinned_oracle()

    mixed_topology = args.variant == "mixed"
    encoded = encode_fixture(mixed_topology=mixed_topology)
    if encoded != encode_fixture(mixed_topology=mixed_topology):
        raise RuntimeError(f"{args.variant} compound wide I420 AVIF is not deterministic")
    file_hash = hashlib.sha256(encoded).hexdigest()
    if file_hash != EXPECTED_FILE_SHA256[args.variant]:
        raise RuntimeError(f"fixture hash changed: {file_hash}")
    frame_hashes = pillow_frame_hashes(encoded, len(EXPECTED_FRAME_SHA256[args.variant]))
    if frame_hashes != EXPECTED_FRAME_SHA256[args.variant]:
        raise RuntimeError(f"Pillow RGB frame hashes changed: {frame_hashes!r}")
    verify_av1_headers(encoded)

    output = (args.output or OUTPUTS[args.variant]).resolve()
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
