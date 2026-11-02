#!/usr/bin/env python3
"""Generate an AVIF sequence that selects inter-inter wedge compound blocks."""

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
FRAME_SHA256 = (
    "5a4efdd0b6a5bff4c1a9b588ef0a07fafd02fda98d50af0c1d58dfc9984eadde",
    "5eac488adf821625c106a15b1f12cfe035c0af2c3350ba5dd28a1615df293b72",
    "14bf97edaf7c65ee203f6c3c4c0e07b0ee459d380a2baa535f2b8a6fa98ab289",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
WIDTH = 64
HEIGHT = 64
VARIANTS = {
    "i420": {
        "asset": "animated_lossy_interinter_wedge_i420_b16x16_64x64.avif",
        "file_sha256": "48defb0aff134ef32ed8c2e0ade85619f7c0dc1d48bc2473b7b8a67b0e54ffed",
        "profile": 0,
        "subsampling": "4:2:0",
        "subsampling_x": 1,
        "subsampling_y": 1,
    },
    "i422": {
        "asset": "animated_lossy_interinter_wedge_i422_b16x16_64x64.avif",
        "file_sha256": "c8b77442e52d68206e6f4100a940a7e85a8deae6677371712ba6b0c760b565b6",
        "profile": 2,
        "subsampling": "4:2:2",
        "subsampling_x": 1,
        "subsampling_y": 0,
    },
    "i444": {
        "asset": "animated_lossy_interinter_wedge_i444_b16x16_64x64.avif",
        "file_sha256": "9525cb01d99c78fe0b479a37da45228ac168c468f4759e90b4cc3d37c3e51e66",
        "profile": 1,
        "subsampling": "4:4:4",
        "subsampling_x": 0,
        "subsampling_y": 0,
    },
}


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


def make_frames() -> tuple[Image.Image, Image.Image, Image.Image]:
    """Create grayscale references and wedge predictions in each B16x16 block."""
    first_rng = random.Random(1096)
    second_rng = random.Random(2096)
    first_gray = bytes(first_rng.randrange(256) for _ in range(WIDTH * HEIGHT))
    second_gray = bytes(second_rng.randrange(256) for _ in range(WIDTH * HEIGHT))
    first_pixels = bytes(
        channel for value in first_gray for channel in (value, value, value)
    )
    second_pixels = bytes(
        channel for value in second_gray for channel in (value, value, value)
    )
    first = Image.frombytes("RGB", (WIDTH, HEIGHT), first_pixels)
    second = Image.frombytes("RGB", (WIDTH, HEIGHT), second_pixels)

    # Use the 16x16 horizontal wedge mask with a rising second-reference weight.
    mask_by_row = [0, 2, 7, 21, 43, 57, 62, 64] + [64] * 8
    final_pixels = bytearray(WIDTH * HEIGHT * 3)
    for block_y in range(0, HEIGHT, 16):
        for block_x in range(0, WIDTH, 16):
            for y, mask in enumerate(mask_by_row):
                for x in range(16):
                    offset = ((block_y + y) * WIDTH + block_x + x) * 3
                    value = (
                        first_pixels[offset] * (64 - mask)
                        + second_pixels[offset] * mask
                        + 32
                    ) // 64
                    final_pixels[offset : offset + 3] = bytes((value, value, value))
    return first, second, Image.frombytes("RGB", (WIDTH, HEIGHT), bytes(final_pixels))


def encode_fixture(variant: dict[str, object]) -> bytes:
    """Encode a non-lossless AV1 sequence with repeated inter-inter wedges."""
    first, second, final = make_frames()
    output = BytesIO()
    first.save(
        output,
        format="AVIF",
        save_all=True,
        append_images=[second, final],
        duration=[100, 100, 100],
        loop=0,
        quality=99,
        speed=0,
        max_threads=1,
        subsampling=str(variant["subsampling"]),
        autotiling=False,
        codec="aom",
        advanced={
            "sb-size": "64",
            "min-partition-size": "16",
            "max-partition-size": "16",
            "aq-mode": "0",
            "deltaq-mode": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "enable-warped-motion": "0",
            "enable-interintra-comp": "0",
            "enable-global-motion": "0",
            "enable-masked-comp": "1",
            "enable-diff-wtd-comp": "0",
            "enable-interinter-wedge": "1",
        },
    )
    return normalize_sequence_timestamps(output.getvalue())


def pillow_frame_hashes(encoded: bytes) -> tuple[str, ...]:
    """Hash exact Pillow RGB frames and verify the public sequence metadata."""
    with Image.open(BytesIO(encoded)) as sequence:
        if sequence.format != "AVIF" or sequence.mode != "RGB":
            raise RuntimeError("Pillow AVIF format or mode differs from the fixture")
        if sequence.size != (WIDTH, HEIGHT) or sequence.n_frames != 3:
            raise RuntimeError("expected a three-frame 64x64 AVIF sequence")
        hashes = []
        for frame_index in range(sequence.n_frames):
            sequence.seek(frame_index)
            sequence.load()
            hashes.append(hashlib.sha256(sequence.convert("RGB").tobytes()).hexdigest())
    return tuple(hashes)


def verify_av1_headers(encoded: bytes, variant: dict[str, object]) -> None:
    """Require masked compound and two-reference inter syntax in the final frame."""
    with TemporaryDirectory() as temporary_directory:
        fixture = Path(temporary_directory) / "interinter_wedge.avif"
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
        or header.get("profile") != variant["profile"]
        or header.get("subsampling_x") != variant["subsampling_x"]
        or header.get("subsampling_y") != variant["subsampling_y"]
        or not header.get("enable_masked_compound")
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence header differs from the selected wedge layout")

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
    parser.add_argument("--variant", choices=sorted(VARIANTS), default="i420")
    parser.add_argument("--output", type=Path)
    parser.add_argument(
        "--check",
        action="store_true",
        help="require the fixture to match the deterministic pinned encoding",
    )
    args = parser.parse_args()
    check_pinned_oracle()

    variant = VARIANTS[args.variant]
    encoded = encode_fixture(variant)
    if encoded != encode_fixture(variant):
        raise RuntimeError("inter-inter wedge AVIF is not deterministic")
    file_hash = hashlib.sha256(encoded).hexdigest()
    if file_hash != variant["file_sha256"]:
        raise RuntimeError(f"fixture hash changed: {file_hash}")
    frame_hashes = pillow_frame_hashes(encoded)
    if frame_hashes != FRAME_SHA256:
        raise RuntimeError(f"Pillow RGB frame hashes changed: {frame_hashes!r}")
    verify_av1_headers(encoded, variant)

    output = (
        args.output
        or ROOT / "tests/fixtures/input/images/avif" / str(variant["asset"])
    ).resolve()
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
