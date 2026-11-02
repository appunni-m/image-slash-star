#!/usr/bin/env python3
"""Generate a pinned AVIF sequence for AV1 distance-compound parity."""

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
FILENAME = "animated_lossy_interinter_distance_i420_b16x16_64x64.avif"
OUTPUT = ROOT / "tests/fixtures/input/images/avif" / FILENAME
FILE_SHA256 = "7abfec6d36ae819aaea9a9dc814c33c538488ba87362cd174b5cd62dfcaaab13"
FRAME_SHA256 = (
    "9b773524f37b7bf706c019b56533a3c4e443926c6d5bd08f95ab94f8b20f469f",
    "9b83c9c2c8e4f56ff07b60222fdef357bbf62995f6f56d7bb226ef9e7a27094a",
    "8f7596cf080f1ea94a4d736d667d51a652a19cb5e3b865dbde84247756fb1ff4",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
WIDTH = 64
HEIGHT = 64


def normalize_timestamps(encoded: bytes) -> bytes:
    """Zero version-one movie and track timestamps."""
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
    """Require the pinned Pillow and AVIF codec versions."""
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned AVIF oracle required; found {observed!r}")


def make_frames() -> tuple[Image.Image, Image.Image, Image.Image]:
    """Make two seeded references and their 11:5 distance-weighted blend."""
    references = []
    for seed in (1096, 2096):
        random_source = random.Random(seed)
        gray = bytes(random_source.randrange(256) for _ in range(WIDTH * HEIGHT))
        references.append(bytes(channel for value in gray for channel in (value,) * 3))

    first, second = references
    target = bytes(
        (near * 11 + far * 5 + 8) // 16 for far, near in zip(first, second)
    )
    return (
        Image.frombytes("RGB", (WIDTH, HEIGHT), first),
        Image.frombytes("RGB", (WIDTH, HEIGHT), second),
        Image.frombytes("RGB", (WIDTH, HEIGHT), target),
    )


def encode_fixture() -> bytes:
    """Encode a compound-selectable sequence with distance weighting enabled."""
    first, second, final = make_frames()
    output = BytesIO()
    first.save(
        output,
        format="AVIF",
        save_all=True,
        append_images=[second, final],
        duration=[100, 100, 100],
        loop=0,
        quality=100,
        speed=0,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        codec="aom",
        advanced={
            "enable-dist-wtd-comp": "1",
            "enable-masked-comp": "0",
            "enable-interintra-comp": "0",
            "enable-global-motion": "0",
            "enable-warped-motion": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "sb-size": "64",
            "min-partition-size": "16",
            "max-partition-size": "16",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
    )
    return normalize_timestamps(output.getvalue())


def pillow_frame_hashes(encoded: bytes) -> tuple[str, ...]:
    """Check the exact three-frame Pillow RGB result and timing."""
    with Image.open(BytesIO(encoded)) as sequence:
        if sequence.format != "AVIF" or sequence.mode != "RGB":
            raise RuntimeError("Pillow AVIF format or mode differs from the fixture")
        if sequence.size != (WIDTH, HEIGHT) or sequence.n_frames != len(FRAME_SHA256):
            raise RuntimeError("Pillow AVIF dimensions or frame count differ")
        hashes = []
        for index in range(sequence.n_frames):
            sequence.seek(index)
            sequence.load()
            if sequence.info.get("duration") != 100:
                raise RuntimeError("Pillow AVIF frame duration differs")
            hashes.append(hashlib.sha256(sequence.convert("RGB").tobytes()).hexdigest())
    return tuple(hashes)


def verify_av1_headers(encoded: bytes) -> None:
    """Require order hints, distance-compound syntax, and both references."""
    with TemporaryDirectory(
        prefix="avif-distance-compound-",
        dir=ROOT / "target" / "oracle-staging",
    ) as temporary_directory:
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
        not header.get("enable_order_hint")
        or not header.get("enable_jnt_comp")
        or header.get("enable_masked_compound")
        or header.get("enable_warped_motion")
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence does not enable distance-only compound syntax")

    final_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if obu.get("frame_header", {}).get("order_hint") == 2
    ]
    if len(final_headers) != 1:
        raise RuntimeError("expected one final inter frame header with order hint two")
    header = final_headers[0]
    if (
        header.get("frame_type") != "inter"
        or header.get("reference_mode") != "select"
        or header.get("reference_indices") != [1, 0, 0, 0, 0, 0, 0]
    ):
        raise RuntimeError("the final frame does not expose both compound references")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the checked-in input without writing it",
    )
    args = parser.parse_args()
    check_pinned_oracle()

    encoded = encode_fixture()
    if encoded != encode_fixture():
        raise RuntimeError("distance-compound AVIF is not deterministic")
    file_hash = hashlib.sha256(encoded).hexdigest()
    if file_hash != FILE_SHA256:
        raise RuntimeError(f"fixture hash changed: {file_hash}")
    frame_hashes = pillow_frame_hashes(encoded)
    if frame_hashes != FRAME_SHA256:
        raise RuntimeError(f"Pillow RGB frame hashes changed: {frame_hashes!r}")
    verify_av1_headers(encoded)

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
