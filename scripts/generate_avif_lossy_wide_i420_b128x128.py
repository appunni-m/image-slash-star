#!/usr/bin/env python3
"""Generate the pinned lossy I420 B128x128 AVIF sequence fixture."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features
from PIL import __version__ as PILLOW_VERSION


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "animated_lossy_wide_i420_b128x128.avif"
)
EXPECTED_FILE_SHA256 = "a8cdebb85ccc240bbab9adde924e13b13d82e76d22600beb8511e62b58ae8bdd"
EXPECTED_FRAME_SHA256 = (
    "749f2b8d76d244d35532b235f536cddaa6d8d5f2e3da48576c1604cfb6af40c6",
    "1d55a4266d45ef6171dd1753c1033c04c190b5e11bf66ee84dac2f56cbaab7bc",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def normalize_sequence_timestamps(encoded: bytes) -> bytes:
    """Zero version-one BMFF timestamps for repeatable file bytes."""
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
    """Require the Pillow, libavif, dav1d, and libaom versions used to pin it."""
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned AVIF oracle required; found {observed!r}")


def make_frames() -> tuple[Image.Image, Image.Image]:
    """Build a tiled color key frame and a shifted, high-signal inter frame."""
    width = height = 256
    palette = (
        (35, 65, 205),
        (225, 54, 32),
        (28, 192, 83),
        (178, 136, 24),
    )
    tile = bytearray(64 * 64 * 3)
    for y in range(64):
        for x in range(64):
            color = palette[(x // 8 + 3 * (y // 8)) & 3]
            grain = ((x * 13 + y * 7) % 17) - 8
            offset = (y * 64 + x) * 3
            tile[offset : offset + 3] = bytes(
                max(0, min(255, channel + grain)) for channel in color
            )

    base = bytearray(width * height * 3)
    for y in range(height):
        for x in range(width):
            source = ((y % 64) * 64 + (x % 64)) * 3
            destination = (y * width + x) * 3
            base[destination : destination + 3] = tile[source : source + 3]

    frames = []
    for marker_x in (12, 28):
        frame = bytearray(base)
        for block_y in (0, 128):
            for block_x in (0, 128):
                for y in range(block_y + 12, block_y + 44):
                    for x in range(block_x + marker_x, block_x + marker_x + 32):
                        offset = (y * width + x) * 3
                        frame[offset : offset + 3] = bytes((244, 230, 24))
        frames.append(Image.frombytes("RGB", (width, height), bytes(frame)))
    return frames[0], frames[1]


def encode_fixture() -> bytes:
    """Encode both I420 frames with fixed 128-pixel partitions."""
    first, second = make_frames()
    output = BytesIO()
    first.save(
        output,
        format="AVIF",
        save_all=True,
        append_images=[second],
        duration=[100, 100],
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
        },
    )
    return normalize_sequence_timestamps(output.getvalue())


def pillow_frame_hashes(encoded: bytes) -> tuple[str, ...]:
    """Hash both exact Pillow RGB frames and validate their public metadata."""
    with Image.open(BytesIO(encoded)) as sequence:
        if sequence.format != "AVIF" or sequence.mode != "RGB":
            raise RuntimeError("Pillow AVIF format or mode differs from the fixture")
        if sequence.size != (256, 256):
            raise RuntimeError(f"expected 256x256 frames, found {sequence.size}")
        if sequence.n_frames != len(EXPECTED_FRAME_SHA256):
            raise RuntimeError(f"expected two frames, found {sequence.n_frames}")
        hashes = []
        for frame_index in range(sequence.n_frames):
            sequence.seek(frame_index)
            sequence.load()
            hashes.append(hashlib.sha256(sequence.convert("RGB").tobytes()).hexdigest())
    return tuple(hashes)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="require the committed fixture to match the deterministic encoding",
    )
    args = parser.parse_args()
    check_pinned_oracle()

    encoded = encode_fixture()
    if encoded != encode_fixture():
        raise RuntimeError("wide I420 AVIF fixture is not deterministic")
    file_hash = hashlib.sha256(encoded).hexdigest()
    if file_hash != EXPECTED_FILE_SHA256:
        raise RuntimeError(f"fixture hash changed: {file_hash}")
    frame_hashes = pillow_frame_hashes(encoded)
    if frame_hashes != EXPECTED_FRAME_SHA256:
        raise RuntimeError(f"Pillow RGB frame hashes changed: {frame_hashes!r}")

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
