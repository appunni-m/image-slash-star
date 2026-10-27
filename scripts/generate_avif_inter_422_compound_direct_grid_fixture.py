#!/usr/bin/env python3
"""Generate a deterministic compound-reference I422 direct chroma-grid sequence."""

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
FILENAME = "animated_lossy_inter_422_compound_direct_grid_b64x64.avif"
FILE_SHA256 = "5eef65a8499170972eb9edf607844b39292721f4e53ebbcdc4d61e78f2d7c51e"
FRAME_SHA256 = (
    "d76bd94bf21860dbc544ab16baa0331d8b9ccafaa71e5a25157c4aebe4abc792",
    "e57adc48bcd4957a13dce5c57c22c890362a9b64ab8c9d2566c152d18d991248",
    "3f73b16723e66215f7f265d635095ff763cccca27fcb977e70d93076b8107d65",
)


def make_frame(seed: int) -> Image.Image:
    """Build a deterministic 64x64 textured I422 reference frame."""
    random_source = random.Random(seed)
    pixels = bytearray()
    for y in range(64):
        for x in range(64):
            half = x // 32
            base = (x // 2 * 37 + y * 19 + half * 83 + seed) & 255
            base = max(0, min(255, base + random_source.randrange(-24, 25)))
            pixels.extend((base, (base * 3 + x * 5) & 255, (base * 5 + y * 7) & 255))
    return Image.frombytes("RGB", (64, 64), bytes(pixels))


def normalize_timestamps(encoded: bytes) -> bytes:
    """Zero version-one BMFF timestamps so repeated encodes are byte-stable."""
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


def make_frames() -> tuple[Image.Image, Image.Image, Image.Image]:
    """Create two references and a final frame that blends both references."""
    first = make_frame(1096)
    second = make_frame(2096)
    return first, second, Image.blend(first, second, 0.5)


def encode_fixture() -> bytes:
    """Encode a deterministic three-frame 8-bit I422 compound sequence."""
    frames = make_frames()
    encoded_sequences = []
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
            speed=0,
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
        encoded_sequences.append(normalize_timestamps(output.getvalue()))
    if encoded_sequences[0] != encoded_sequences[1]:
        raise RuntimeError("repeated AVIF encodes differ")
    return encoded_sequences[0]


def verify_pillow_frames(encoded: bytes) -> None:
    """Require exact pinned Pillow pixels for every sequence frame."""
    frame_hashes = []
    with Image.open(BytesIO(encoded)) as sequence:
        if sequence.format != "AVIF" or sequence.mode != "RGB" or sequence.size != (64, 64):
            raise RuntimeError("Pillow AVIF metadata differs from the pinned fixture")
        if sequence.n_frames != len(FRAME_SHA256):
            raise RuntimeError(f"expected three frames, found {sequence.n_frames}")
        for frame_index in range(sequence.n_frames):
            sequence.seek(frame_index)
            sequence.load()
            frame_hashes.append(
                hashlib.sha256(sequence.convert("RGB").tobytes()).hexdigest()
            )
    if tuple(frame_hashes) != FRAME_SHA256:
        raise RuntimeError(f"Pillow frame hashes changed: {tuple(frame_hashes)!r}")


def verify_av1_headers(encoded: bytes) -> None:
    """Require I422 syntax and selectable compound references on the final frame."""
    with TemporaryDirectory() as temporary_directory:
        fixture = Path(temporary_directory) / FILENAME
        fixture.write_bytes(encoded)
        report = inspect(fixture)

    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if not sequence_headers or any(
        header.get("monochrome")
        or header.get("subsampling_x") != 1
        or header.get("subsampling_y") != 0
        or header.get("profile") != 2
        for header in sequence_headers
    ):
        raise RuntimeError("expected the AV1 sequence to use 8-bit 4:2:2 profile 2")

    final_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if obu.get("frame_header", {}).get("order_hint") == 2
    ]
    if len(final_headers) != 1:
        raise RuntimeError("expected one final frame header with order hint two")
    header = final_headers[0]
    if header.get("transform_mode") != "largest":
        raise RuntimeError("the final inter frame must use largest-transform mode")
    if header.get("frame_width") != 64 or header.get("frame_height") != 64:
        raise RuntimeError("the final inter frame must cover the complete 64x64 block")
    if header.get("reference_mode") != "select":
        raise RuntimeError("the final inter frame does not allow compound selection")
    if header.get("reference_indices") != [1, 0, 0, 0, 0, 0, 0]:
        raise RuntimeError("the final inter frame does not expose both prior references")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the checked-in fixture without writing it",
    )
    args = parser.parse_args()

    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    expected = (
        "12.2.0",
        "1.4.1",
        "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
    )
    if observed != expected:
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
