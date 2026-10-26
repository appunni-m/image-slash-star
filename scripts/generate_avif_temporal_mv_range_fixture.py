#!/usr/bin/env python3
"""Generate pinned AVIF sequences with out-of-range temporal motion vectors."""

from __future__ import annotations

import argparse
import hashlib
import random
from dataclasses import dataclass
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features
from PIL import __version__ as PILLOW_VERSION


ROOT = Path(__file__).resolve().parent.parent
INPUT_DIR = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SHIFT = 600
RUN_SIZE = 16


@dataclass(frozen=True)
class Fixture:
    filename: str
    width: int
    height: int
    axis: str
    motion_sign: int
    file_sha256: str
    frame_sha256: tuple[str, str, str]


FIXTURES = (
    Fixture(
        filename="animated_motion_temporal_mv_range_2048x128.avif",
        width=2048,
        height=128,
        axis="x",
        motion_sign=-1,
        file_sha256="294468c2498c8fa494bfcc2e701a944e90f15ec4b5f08eb64fa34f907e5a6a1a",
        frame_sha256=(
            "74e4725826effa7eb52b0734b54b6ca26dccef3a9e37bd6009aea5b76280ecba",
            "fc1c3c55e8475f979786e7e28ca273bbfb245ce7a5c3e798f648132a1da25d6c",
            "74e4725826effa7eb52b0734b54b6ca26dccef3a9e37bd6009aea5b76280ecba",
        ),
    ),
    Fixture(
        filename="animated_motion_temporal_mv_range_positive_x_2048x128.avif",
        width=2048,
        height=128,
        axis="x",
        motion_sign=1,
        file_sha256="f802a724db9dea26e0d71abb7d123c9338449e77008282874f58580c0b2163d1",
        frame_sha256=(
            "74e4725826effa7eb52b0734b54b6ca26dccef3a9e37bd6009aea5b76280ecba",
            "72c3c05f8c8634fbd2a92e5c76fb1b4cb0581067fbb2f644733a3e7650c56f57",
            "74e4725826effa7eb52b0734b54b6ca26dccef3a9e37bd6009aea5b76280ecba",
        ),
    ),
    Fixture(
        filename="animated_motion_temporal_mv_range_negative_y_128x2048.avif",
        width=128,
        height=2048,
        axis="y",
        motion_sign=-1,
        file_sha256="e32c9ff32e87922be280d26fef6d8c92635e84d2918df7c23f3f8ec3087b051c",
        frame_sha256=(
            "ab7ab98b4053d82805629f676d0aa6436469cdc1018818ecee6bfdc3330527b2",
            "9aed2a62e634a182b4ced10d4421fc1e3745e40ee8bdfa16f4559237816a1f1c",
            "ab7ab98b4053d82805629f676d0aa6436469cdc1018818ecee6bfdc3330527b2",
        ),
    ),
    Fixture(
        filename="animated_motion_temporal_mv_range_positive_y_128x2048.avif",
        width=128,
        height=2048,
        axis="y",
        motion_sign=1,
        file_sha256="307a001ecd6e10c4b840a3d0e0b30b24a6a90ac1c8f5f5ea1b8d9e209b1c3333",
        frame_sha256=(
            "ab7ab98b4053d82805629f676d0aa6436469cdc1018818ecee6bfdc3330527b2",
            "1f91ab54d21f29418c199e2ce32ae69a088757f924d40d15ac2cccb7bb06625e",
            "ab7ab98b4053d82805629f676d0aa6436469cdc1018818ecee6bfdc3330527b2",
        ),
    ),
)


def normalize_sequence_timestamps(encoded: bytes) -> bytes:
    """Zero version-one BMFF timestamps to make Pillow output reproducible."""
    normalized = bytearray(encoded)
    for box_type in (b"mvhd", b"tkhd", b"mdhd"):
        offset = normalized.find(box_type)
        if offset < 4 or normalized[offset + 4] != 1:
            raise RuntimeError(f"expected a version-one {box_type!r} box")
        if normalized.find(box_type, offset + 4) != -1:
            raise RuntimeError(f"found duplicate {box_type!r} boxes")
        normalized[offset + 8 : offset + 24] = bytes(16)
    return bytes(normalized)


def texture_bytes(fixture: Fixture) -> bytes:
    """Build a seeded texture with distinct 16-pixel runs on the motion axis."""
    random_source = random.Random(0xA71)
    group_count = (fixture.width if fixture.axis == "x" else fixture.height) // RUN_SIZE
    colors = [
        bytes(random_source.randrange(256) for _ in range(3))
        for _ in range(group_count)
    ]
    if fixture.axis == "x":
        row = b"".join(color * RUN_SIZE for color in colors)
        return row * fixture.height
    if fixture.axis == "y":
        return b"".join((color * fixture.width) * RUN_SIZE for color in colors)
    raise ValueError(f"unsupported motion axis {fixture.axis!r}")


def translated_frame(fixture: Fixture, texture: bytes) -> Image.Image:
    """Translate the base texture opposite the stored motion-vector sign."""
    width, height = fixture.width, fixture.height
    displacement = -fixture.motion_sign * SHIFT
    pixels = bytearray(width * height * 3)
    if fixture.axis == "x":
        copy_width = width - abs(displacement)
        source_x = max(0, -displacement)
        destination_x = max(0, displacement)
        row_bytes = copy_width * 3
        for y in range(height):
            source_start = (y * width + source_x) * 3
            destination_start = (y * width + destination_x) * 3
            pixels[destination_start : destination_start + row_bytes] = texture[
                source_start : source_start + row_bytes
            ]
    else:
        copy_height = height - abs(displacement)
        source_y = max(0, -displacement)
        destination_y = max(0, displacement)
        row_bytes = width * 3
        copy_bytes = copy_height * row_bytes
        source_start = source_y * row_bytes
        destination_start = destination_y * row_bytes
        pixels[destination_start : destination_start + copy_bytes] = texture[
            source_start : source_start + copy_bytes
        ]
    return Image.frombytes("RGB", (width, height), bytes(pixels))


def make_frames(fixture: Fixture) -> tuple[Image.Image, ...]:
    """Create the base, translated, and restored texture frames."""
    texture = texture_bytes(fixture)
    base = Image.frombytes("RGB", (fixture.width, fixture.height), texture)
    return (base, translated_frame(fixture, texture), base.copy())


def encode_fixture(fixture: Fixture) -> bytes:
    frames = make_frames(fixture)
    output = BytesIO()
    frames[0].save(
        output,
        format="AVIF",
        save_all=True,
        append_images=list(frames[1:]),
        duration=[100] * len(frames),
        loop=0,
        quality=95,
        speed=0,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        codec="aom",
        advanced={
            "aq-mode": "0",
            "deltaq-mode": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "enable-warped-motion": "0",
            "enable-global-motion": "0",
        },
    )
    return normalize_sequence_timestamps(output.getvalue())


def pillow_frame_hashes(encoded: bytes, expected_count: int) -> tuple[str, ...]:
    with Image.open(BytesIO(encoded)) as sequence:
        if sequence.n_frames != expected_count:
            raise RuntimeError(
                f"expected {expected_count} frames, found {sequence.n_frames}"
            )
        hashes = []
        for frame_index in range(sequence.n_frames):
            sequence.seek(frame_index)
            sequence.load()
            hashes.append(hashlib.sha256(sequence.tobytes()).hexdigest())
    return tuple(hashes)


def verify_fixture(fixture: Fixture, check: bool) -> None:
    output = INPUT_DIR / fixture.filename
    encoded = encode_fixture(fixture)
    if encoded != encode_fixture(fixture):
        raise RuntimeError(f"{fixture.filename} is not deterministic")
    if hashlib.sha256(encoded).hexdigest() != fixture.file_sha256:
        raise RuntimeError(f"{fixture.filename} differs from its pinned hash")
    if pillow_frame_hashes(encoded, len(fixture.frame_sha256)) != fixture.frame_sha256:
        raise RuntimeError(f"{fixture.filename} Pillow frames differ from pins")

    if check:
        if not output.is_file() or output.read_bytes() != encoded:
            raise RuntimeError(f"the checked-in {fixture.filename} is stale")
        print(f"Verified {output.relative_to(ROOT)} ({len(encoded)} bytes)")
        return

    output.write_bytes(encoded)
    print(f"Wrote {output.relative_to(ROOT)} ({len(encoded)} bytes)")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify existing fixtures without writing them",
    )
    args = parser.parse_args()

    if (
        PILLOW_VERSION != "12.2.0"
        or features.version("avif") != "1.4.1"
        or _avif.codec_versions()
        != "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
    ):
        raise RuntimeError("the pinned Pillow 12.2.0 AVIF oracle is required")

    for fixture in FIXTURES:
        verify_fixture(fixture, args.check)


if __name__ == "__main__":
    main()
