#!/usr/bin/env python3
"""Generate the pinned wide-I444 mode-2 TX64-to-TX16 AVIF sequence."""

from __future__ import annotations

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
    / "animated_lossy_wide_i444_mode2_deep16_b128x128.avif"
)
EXPECTED_FILE_SHA256 = (
    "a806f4585aa11699bc831fff1b02b7f1eacbd2089a88a2578bb6933afbcbdaf0"
)
EXPECTED_FRAME_SHA256 = (
    "83ab53863cf746efe2611c6b521cfbdefba4f5325cf123ab451c385035b394ef",
    "e5c2a5b0de8905d2cc43d1054cf9117dcd68f0b6e6779406623112e237c0f109",
    "4128dcf262a77d95244bf4b54fa676848832fecb16756d9f4e67406bc0d71e61",
    "d48b9cc5c307ace33f47c22f7399b0bef2fd8104a3f698b97a1c172b9e7d9e55",
    "113800554714e0e047b5667fcef549080c6e66213562f4940f04af794a12acf6",
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


def make_frames() -> tuple[Image.Image, ...]:
    """Build a gray key frame followed by four increasing checker periods."""
    width = height = 256
    frames = [Image.new("RGB", (width, height), (128, 128, 128))]
    for period in (2, 4, 8, 16):
        pixels = bytearray(width * height * 3)
        for y in range(height):
            for x in range(width):
                value = 255 if ((x // period + y // period) & 1) else 0
                offset = (y * width + x) * 3
                pixels[offset : offset + 3] = bytes((value, value, value))
        frames.append(Image.frombytes("RGB", (width, height), bytes(pixels)))
    return tuple(frames)


def encode_fixture() -> bytes:
    frames = make_frames()
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
        subsampling="4:4:4",
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
    with Image.open(BytesIO(encoded)) as sequence:
        if sequence.n_frames != len(EXPECTED_FRAME_SHA256):
            raise RuntimeError(
                f"expected five frames, found {sequence.n_frames}"
            )
        hashes = []
        for frame_index in range(sequence.n_frames):
            sequence.seek(frame_index)
            sequence.load()
            hashes.append(hashlib.sha256(sequence.tobytes()).hexdigest())
    return tuple(hashes)


def main() -> None:
    if (
        PILLOW_VERSION != "12.2.0"
        or features.version("avif") != "1.4.1"
        or _avif.codec_versions()
        != "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
    ):
        raise RuntimeError("the pinned Pillow 12.2.0 AVIF oracle is required")

    encoded = encode_fixture()
    if encoded != encode_fixture():
        raise RuntimeError("mode-2 deep16 AVIF fixture is not deterministic")
    if hashlib.sha256(encoded).hexdigest() != EXPECTED_FILE_SHA256:
        raise RuntimeError("mode-2 deep16 AVIF fixture differs from its pinned hash")
    if pillow_frame_hashes(encoded) != EXPECTED_FRAME_SHA256:
        raise RuntimeError("mode-2 deep16 AVIF Pillow frames differ from their pins")

    OUTPUT.write_bytes(encoded)
    print(f"Wrote {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes)")


if __name__ == "__main__":
    main()
