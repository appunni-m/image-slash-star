#!/usr/bin/env python3
"""Generate the pinned lossy AVIF mixed B16 transform-topology sequence."""

from __future__ import annotations

import hashlib
import random
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
    / "animated_lossy_b16_mixed_topology_inter_420_b16x16.avif"
)
EXPECTED_FILE_SHA256 = "3264482e2a50d80bd39be178b843fdfe9997e947d54b4466c745751837821d74"
EXPECTED_FRAME_SHA256 = (
    "7f3e5e4e65eca4390e9242558012bc9bdad133d7ac9f6aed53fa156a2288f73b",
    "7f3e5e4e65eca4390e9242558012bc9bdad133d7ac9f6aed53fa156a2288f73b",
    "e195092d99a776d7638ff75a30825447f6dffd85a04fa845750617ce04a98a79",
)
SEED = 31066


def normalize_sequence_timestamps(encoded: bytes) -> bytes:
    """Zero version-one BMFF timestamps to make Pillow output reproducible."""
    normalized = bytearray(encoded)
    for box_type in (b"mvhd", b"tkhd", b"mdhd"):
        offset = normalized.find(box_type)
        if offset < 4 or normalized[offset + 4] != 1:
            raise RuntimeError(f"expected a version-one {box_type!r} box")
        if normalized.find(box_type, offset + 4) != -1:
            raise RuntimeError(f"found duplicate version-one {box_type!r} boxes")
        normalized[offset + 8 : offset + 24] = bytes(16)
    return bytes(normalized)


def mixed_residual_frame() -> Image.Image:
    """Add sparse luma residuals to the top-right TX8 child of a gray B16."""
    pixels = bytearray(16 * 16 * 3)
    for y in range(16):
        for x in range(16):
            local_x = x % 8
            local_y = y % 8
            quadrant = (y // 8) * 2 + (x // 8)
            if quadrant == 1:
                frame_seed = SEED + 1
                rng = random.Random(
                    frame_seed * 1009 + quadrant * 97 + local_y * 17 + local_x
                )
                value = (
                    128 + rng.choice((-1, 1)) * 80
                    if (local_x + 3 * local_y) % 13 == 0
                    else 128
                )
                if (x + 3 * y + SEED) % 11 == 0:
                    value += 23 if (x + y) & 1 else -19
            else:
                value = 104 + ((3 * local_x + 5 * local_y + SEED + 1) % 49)
            value = max(8, min(247, value))
            offset = (y * 16 + x) * 3
            pixels[offset : offset + 3] = bytes((value, value, value))
    return Image.frombytes("RGB", (16, 16), bytes(pixels))


def encode_fixture() -> bytes:
    key_frame = Image.new("RGB", (16, 16), (128, 128, 128))
    frames = (key_frame, key_frame.copy(), mixed_residual_frame())
    output = BytesIO()
    frames[0].save(
        output,
        format="AVIF",
        save_all=True,
        append_images=list(frames[1:]),
        duration=[100] * len(frames),
        loop=0,
        quality=99,
        speed=0,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        codec="aom",
        advanced={"min-partition-size": "16", "max-partition-size": "16"},
    )
    return normalize_sequence_timestamps(output.getvalue())


def pillow_frame_hashes(encoded: bytes) -> tuple[str, ...]:
    hashes = []
    with Image.open(BytesIO(encoded)) as sequence:
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
        raise RuntimeError("mixed-topology AVIF fixture is not deterministic")
    if hashlib.sha256(encoded).hexdigest() != EXPECTED_FILE_SHA256:
        raise RuntimeError("mixed-topology AVIF fixture differs from its pinned hash")
    if pillow_frame_hashes(encoded) != EXPECTED_FRAME_SHA256:
        raise RuntimeError("mixed-topology AVIF Pillow frames differ from their pins")

    OUTPUT.write_bytes(encoded)
    print(f"Wrote {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes)")


if __name__ == "__main__":
    main()
