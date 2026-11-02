#!/usr/bin/env python3
"""Generate AVIF sequences for difference-weighted compound parity."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path
import random
from tempfile import TemporaryDirectory

from PIL import Image, _avif, features
from PIL import __version__ as PILLOW_VERSION

from generate_avif_multitile_motion_fixture import normalize_timestamps
from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parent.parent
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
WIDTH = 64
HEIGHT = 64
FRAME_SHA256 = (
    "9b773524f37b7bf706c019b56533a3c4e443926c6d5bd08f95ab94f8b20f469f",
    "9b83c9c2c8e4f56ff07b60222fdef357bbf62995f6f56d7bb226ef9e7a27094a",
)
VARIANTS = {
    "i420": {
        "asset": "animated_lossy_interinter_diffwtd_i420_b16x16_64x64.avif",
        "sha256": "cfd5adb4e0c97a9c3d206f2de0b0a8ce195fd7aa4f69e3b937129cf581986ae0",
        "subsampling": "4:2:0",
        "mask_inverted": False,
        "frame_sha256": "665b90f0aa1d8af4ccf4e3e62c4b8ecc5bfb699b8baf1fec74e5efe5fa98a949",
    },
    "i420-inverted": {
        "asset": "animated_lossy_interinter_diffwtd_i420_inverted_b16x16_64x64.avif",
        "sha256": "23c64470703c11c0ecfedfd906cc7b5098db1cc4b707676464c8c8064f91dd45",
        "subsampling": "4:2:0",
        "mask_inverted": True,
        "frame_sha256": "2a483932643ae4a354cc7613a7731194815f77e998993f4bc35b7e7ada11006a",
    },
    "i422": {
        "asset": "animated_lossy_interinter_diffwtd_i422_b16x16_64x64.avif",
        "sha256": "fad2e3b5264d0ec4d58dd53beea98c15462fbf36fd02e0a82a3f21447e291962",
        "subsampling": "4:2:2",
        "mask_inverted": False,
        "frame_sha256": "665b90f0aa1d8af4ccf4e3e62c4b8ecc5bfb699b8baf1fec74e5efe5fa98a949",
    },
    "i444": {
        "asset": "animated_lossy_interinter_diffwtd_i444_b16x16_64x64.avif",
        "sha256": "abafc8fd5f727e83dbef669a3e43e57b283e780e0274a55f540da9070eae837b",
        "subsampling": "4:4:4",
        "mask_inverted": False,
        "frame_sha256": "665b90f0aa1d8af4ccf4e3e62c4b8ecc5bfb699b8baf1fec74e5efe5fa98a949",
    },
}


def check_pinned_oracle() -> None:
    """Require the exact Pillow and AVIF codec versions used for fixtures."""
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned AVIF oracle required; found {observed!r}")


def make_frames(mask_inverted: bool) -> tuple[Image.Image, Image.Image, Image.Image]:
    """Create seeded grayscale references and their AV1 diff-weighted blend."""
    references = []
    for seed in (1096, 2096):
        rng = random.Random(seed)
        gray = bytes(rng.randrange(256) for _ in range(WIDTH * HEIGHT))
        rgb = bytes(channel for value in gray for channel in (value, value, value))
        references.append(Image.frombytes("RGB", (WIDTH, HEIGHT), rgb))

    first, second = references
    first_pixels = first.tobytes()
    second_pixels = second.tobytes()
    blended = bytearray(len(first_pixels))
    for offset in range(0, len(blended), 3):
        first_value = first_pixels[offset]
        second_value = second_pixels[offset]
        mask = min(64, 38 + abs(first_value - second_value) // 16)
        if mask_inverted:
            mask = 64 - mask
        value = (first_value * (64 - mask) + second_value * mask + 32) // 64
        blended[offset : offset + 3] = bytes((value, value, value))
    final = Image.frombytes("RGB", (WIDTH, HEIGHT), bytes(blended))
    return first, second, final


def encode_fixture(variant: dict[str, object]) -> bytes:
    """Encode a quality-100 sequence with compound-selectable references."""
    first, second, final = make_frames(bool(variant["mask_inverted"]))
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
            "enable-diff-wtd-comp": "1",
            "enable-interinter-wedge": "0",
        },
    )
    return normalize_timestamps(output.getvalue())


def pillow_frame_hashes(encoded: bytes) -> tuple[str, ...]:
    """Hash exact Pillow RGB frames and verify public sequence metadata."""
    with Image.open(BytesIO(encoded)) as sequence:
        if sequence.format != "AVIF" or sequence.mode != "RGB":
            raise RuntimeError("Pillow AVIF format or mode differs from the fixture")
        if sequence.size != (WIDTH, HEIGHT) or sequence.n_frames != 3:
            raise RuntimeError("expected a three-frame 64x64 AVIF sequence")
        hashes = []
        for frame_index in range(sequence.n_frames):
            sequence.seek(frame_index)
            sequence.load()
            if sequence.info.get("duration") != 100:
                raise RuntimeError("Pillow AVIF frame duration differs from the fixture")
            hashes.append(hashlib.sha256(sequence.convert("RGB").tobytes()).hexdigest())
    return tuple(hashes)


def verify_av1_headers(encoded: bytes, variant: dict[str, object]) -> None:
    """Require masked compound and both reference lanes in the final frame."""
    with TemporaryDirectory() as temporary_directory:
        fixture = Path(temporary_directory) / "interinter_diffwtd.avif"
        fixture.write_bytes(encoded)
        report = inspect(fixture)

    expected_subsampling = {
        "subsampling_x": variant["subsampling"] != "4:4:4",
        "subsampling_y": variant["subsampling"] == "4:2:0",
    }
    expected_profile = {"4:2:0": 0, "4:2:2": 2, "4:4:4": 1}[
        str(variant["subsampling"])
    ]
    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if not sequence_headers or any(
        header.get("bit_depth") != 8
        or header.get("monochrome")
        or header.get("profile") != expected_profile
        or header.get("subsampling_x") != expected_subsampling["subsampling_x"]
        or header.get("subsampling_y") != expected_subsampling["subsampling_y"]
        or not header.get("enable_masked_compound")
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence header differs from the selected color variant")

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
        raise RuntimeError("difference-weighted compound AVIF is not deterministic")
    file_hash = hashlib.sha256(encoded).hexdigest()
    if file_hash != variant["sha256"]:
        raise RuntimeError(f"fixture hash changed: {file_hash}")
    frame_hashes = pillow_frame_hashes(encoded)
    expected_frame_hashes = (*FRAME_SHA256, variant["frame_sha256"])
    if frame_hashes != expected_frame_hashes:
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
