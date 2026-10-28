#!/usr/bin/env python3
"""Generate a pinned lossless AVIF sequence with a wide inter grid."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path
import tempfile

from PIL import Image, _avif, features

from generate_avif_multitile_motion_fixture import normalize_timestamps
from inspect_av1_obus import inspect as inspect_av1


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "animated_lossless_inter_420_wide_grid_b128x128.avif"
)
SIZE = (128, 128)
EXPECTED_FILE_SHA256 = "e44c5a53a1cf185a29b4a6570836c82078d1cd2e1d0d0963e69ebf1028f7d47f"
EXPECTED_FRAME_SHA256 = (
    "ea586891ce5f68415430eb8eff7bde1c4b9a676fdc5f42430a437d84d73e050c",
    "49d23c28e069e0b5c3e3e02648cda1ae2ae802ab83271a2e590a3ec4f105a9c9",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
ADVANCED_OPTIONS = {
    "sb-size": "128",
    "min-partition-size": "128",
    "max-partition-size": "128",
    "aq-mode": "0",
    "deltaq-mode": "0",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "loopfilter-control": "0",
    "enable-warped-motion": "0",
    "enable-global-motion": "0",
    "enable-interintra-comp": "0",
}


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def source_frame(shift: int) -> Image.Image:
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            source_x = max(0, x - shift)
            value = (source_x * 73 + y * 151 + source_x * y * 13) & 0xFF
            pixels.extend((value, (value * 3 + 17) & 0xFF, (value * 7 + 31) & 0xFF))
    return Image.frombytes("RGB", SIZE, bytes(pixels))


def check_oracle() -> None:
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow/libavif/libaom stack required, found {observed!r}")


def encode_fixture() -> bytes:
    frames = (source_frame(0), source_frame(1))
    output = BytesIO()
    frames[0].save(
        output,
        format="AVIF",
        save_all=True,
        append_images=[frames[1]],
        duration=[100, 100],
        loop=0,
        quality=100,
        speed=0,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        codec="aom",
        advanced=ADVANCED_OPTIONS,
    )
    return normalize_timestamps(output.getvalue())


def validate_fixture(encoded: bytes) -> None:
    file_sha256 = sha256(encoded)
    if file_sha256 != EXPECTED_FILE_SHA256:
        raise RuntimeError(f"fixture hash changed: {file_sha256}")

    with tempfile.TemporaryDirectory(prefix="avif-lossless-wide-grid-") as directory:
        path = Path(directory) / "fixture.avif"
        path.write_bytes(encoded)
        report = inspect_av1(path)

    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if not sequence_headers or any(
        header.get("use_128x128_superblock") is not True for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence does not use 128x128 superblocks")

    frame_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    if not any(header.get("frame_type") == "inter" for header in frame_headers):
        raise RuntimeError("AV1 sequence has no inter frame")
    if any(header.get("all_lossless") is not True for header in frame_headers):
        raise RuntimeError("AV1 sequence contains a frame that is not all-lossless")

    frame_hashes = []
    with Image.open(BytesIO(encoded)) as image:
        if (
            image.format != "AVIF"
            or image.mode != "RGB"
            or image.size != SIZE
            or image.n_frames != 2
            or not image.is_animated
        ):
            raise RuntimeError("Pillow AVIF metadata differs from the pinned sequence")
        for frame_index in range(image.n_frames):
            image.seek(frame_index)
            image.load()
            if image.info.get("duration") != 100:
                raise RuntimeError("Pillow frame duration differs from the pinned sequence")
            frame_hashes.append(sha256(image.convert("RGB").tobytes()))
    if tuple(frame_hashes) != EXPECTED_FRAME_SHA256:
        raise RuntimeError(f"Pillow frame hashes changed: {tuple(frame_hashes)!r}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify the committed fixture")
    args = parser.parse_args()

    check_oracle()
    encoded = encode_fixture()
    validate_fixture(encoded)

    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != encoded:
            raise RuntimeError(f"fixture differs from deterministic output: {OUTPUT}")
        print(f"Verified {OUTPUT} (sha256={EXPECTED_FILE_SHA256})")
        return

    if OUTPUT.exists() and OUTPUT.read_bytes() != encoded:
        raise RuntimeError(f"refusing to replace differing fixture: {OUTPUT}")
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    temporary = OUTPUT.with_name(f".{OUTPUT.name}.tmp")
    try:
        temporary.write_bytes(encoded)
        temporary.replace(OUTPUT)
    finally:
        temporary.unlink(missing_ok=True)
    print(f"Wrote {OUTPUT} ({len(encoded)} bytes, sha256={EXPECTED_FILE_SHA256})")


if __name__ == "__main__":
    main()
