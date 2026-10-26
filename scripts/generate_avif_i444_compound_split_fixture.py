#!/usr/bin/env python3
"""Generate a pinned I444 AVIF sequence with selectable compound references."""

from __future__ import annotations

import argparse
import hashlib
import random
from io import BytesIO
from pathlib import Path
from tempfile import TemporaryDirectory

from PIL import Image, _avif, features
from PIL import __version__ as PILLOW_VERSION

from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parent.parent
INPUT_DIR = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
FILENAME = "animated_lossy_inter_i444_compound_split_b16x32_mode2.avif"
FILE_SHA256 = "bb43388411119ac10f7990ecb8523e8b62f85227b501e3402b962b5c2b624714"
FRAME_SHA256 = (
    "d3826d80137dc7de8d45f708eef5135912063502f90918884b53e80531e2e5e9",
    "f6211237ef43734ec8fd3ade696c45138242d106e22ab501e3f7379efc7d5227",
    "69ee4d446b34653b7f5bcabd8aae04785a0876f71973ac2ed06ba5310f256975",
)


def normalize_timestamps(encoded: bytes) -> bytes:
    """Zero version-one BMFF timestamps so the encoded fixture stays stable."""
    normalized = bytearray(encoded)
    for box_type in (b"mvhd", b"tkhd", b"mdhd"):
        offset = normalized.find(box_type)
        if offset < 4 or normalized[offset + 4] != 1:
            raise RuntimeError(f"expected a version-one {box_type!r} box")
        if normalized.find(box_type, offset + 4) != -1:
            raise RuntimeError(f"found duplicate {box_type!r} boxes")
        normalized[offset + 8 : offset + 24] = bytes(16)
    return bytes(normalized)


def make_frames() -> tuple[Image.Image, Image.Image, Image.Image]:
    """Create a B16x32 split candidate and a final blend of both references."""
    frames = []
    for frame_index in range(2):
        random_source = random.Random(490 + frame_index)
        pixels = bytearray()
        for y in range(32):
            for x in range(32):
                value = 220 if ((x // 8 + y // 8) & 1) else 30
                value += random_source.randint(-8, 8)
                if frame_index and (x - 2) % 32 < 4 and 8 <= y < 24:
                    value += 80 if (x + y) & 1 else -55
                value = max(0, min(255, value))
                pixels.extend((value, value, value))
        frames.append(Image.frombytes("RGB", (32, 32), bytes(pixels)))

    blended = Image.blend(frames[0], frames[1], 0.5)
    return frames[0], frames[1], blended


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
        quality=75,
        speed=0,
        max_threads=1,
        subsampling="4:4:4",
        autotiling=False,
        codec="aom",
        advanced={
            "min-partition-size": "16",
            "max-partition-size": "32",
            "aq-mode": "0",
            "deltaq-mode": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
        },
    )
    return normalize_timestamps(output.getvalue())


def decoded_frame_hashes(encoded: bytes) -> tuple[str, ...]:
    with Image.open(BytesIO(encoded)) as sequence:
        if sequence.n_frames != len(FRAME_SHA256):
            raise RuntimeError(f"expected three frames, found {sequence.n_frames}")
        hashes = []
        for frame_index in range(sequence.n_frames):
            sequence.seek(frame_index)
            sequence.load()
            hashes.append(hashlib.sha256(sequence.tobytes()).hexdigest())
    return tuple(hashes)


def check_selectable_references(encoded: bytes) -> None:
    """Require the final inter header to expose both earlier frame references."""
    with TemporaryDirectory() as temporary_directory:
        fixture_path = Path(temporary_directory) / "compound.avif"
        fixture_path.write_bytes(encoded)
        report = inspect(fixture_path)

    final_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if obu.get("frame_header", {}).get("order_hint") == 2
    ]
    if len(final_headers) != 1:
        raise RuntimeError("expected one final frame header with order hint two")
    header = final_headers[0]
    if header.get("reference_mode") != "select":
        raise RuntimeError("the final inter frame does not allow compound selection")
    if header.get("reference_indices") != [1, 0, 0, 0, 0, 0, 0]:
        raise RuntimeError("the final inter frame does not expose both prior frames")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the checked-in fixture without writing it",
    )
    args = parser.parse_args()

    if (
        PILLOW_VERSION != "12.2.0"
        or features.version("avif") != "1.4.1"
        or _avif.codec_versions()
        != "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
    ):
        raise RuntimeError("the pinned Pillow 12.2.0 AVIF oracle is required")

    output = INPUT_DIR / FILENAME
    encoded = encode_fixture()
    if encoded != encode_fixture():
        raise RuntimeError("the AVIF sequence is not deterministic")
    if hashlib.sha256(encoded).hexdigest() != FILE_SHA256:
        raise RuntimeError("the AVIF sequence differs from its pinned hash")
    if decoded_frame_hashes(encoded) != FRAME_SHA256:
        raise RuntimeError("Pillow-decoded frames differ from their pinned hashes")
    check_selectable_references(encoded)

    if args.check:
        if not output.is_file() or output.read_bytes() != encoded:
            raise RuntimeError(f"the checked-in {FILENAME} is stale")
        print(f"Verified {output.relative_to(ROOT)} ({len(encoded)} bytes)")
    else:
        output.write_bytes(encoded)
        print(f"Wrote {output.relative_to(ROOT)} ({len(encoded)} bytes)")


if __name__ == "__main__":
    main()
