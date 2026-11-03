#!/usr/bin/env python3
"""Generate a pinned all-lossless I444 sequence with fixed 64x64 partitions."""

from __future__ import annotations

import argparse
from hashlib import sha256 as sha256_digest
from io import BytesIO
from pathlib import Path
import tempfile

from PIL import Image, __version__ as PILLOW_VERSION, _avif, features

from generate_avif_multitile_motion_fixture import normalize_timestamps
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET = (
    ROOT
    / "tests/fixtures/input/images/avif/"
    / "animated_lossless_inter_i444_fixed_b64x64_64x64.avif"
)
EXPECTED_ASSET_SHA256 = "89000cefae8e2c8c1a933be59cb42ee6749747cb62909c21378d54be54d5d0ff"
EXPECTED_FRAME_SHA256 = (
    "378928f592dffdaaa85e46041b599f3f7e3ffa3be25519f27a70380f6458fb60",
    "0ee6a79e722b28dc067fc5418d4a87d924c1770eec746df226123d0f20f60097",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def check_oracle() -> None:
    """Require the exact Pillow AVIF encoder used by repository fixtures."""
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow/libavif/libaom stack required, found {observed!r}")


def make_frame(seed: int) -> Image.Image:
    """Build textured RGB planes so both chroma channels have residuals."""
    pixels = bytearray()
    for y in range(64):
        for x in range(64):
            pixels.extend(
                (
                    (x * 37 + y * 19 + x * y * 3 + seed) & 255,
                    (x * 11 + y * 53 + (x ^ y) * 7 + 3 * seed) & 255,
                    (x * 7 + y * 23 + x * y * 5 + 5 * seed) & 255,
                )
            )
    return Image.frombytes("RGB", (64, 64), bytes(pixels))


def encode_once() -> bytes:
    """Encode and normalize a fixed-partition, two-frame I444 sequence."""
    output = BytesIO()
    make_frame(1).save(
        output,
        format="AVIF",
        save_all=True,
        append_images=[make_frame(2)],
        duration=[100, 100],
        loop=0,
        quality=100,
        speed=0,
        max_threads=1,
        subsampling="4:4:4",
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
    return normalize_timestamps(output.getvalue())


def validate_pixels(encoded: bytes) -> None:
    """Require pinned Pillow frame metadata and exact RGB pixels."""
    with Image.open(BytesIO(encoded)) as image:
        if (
            image.format != "AVIF"
            or image.mode != "RGB"
            or image.size != (64, 64)
            or image.n_frames != len(EXPECTED_FRAME_SHA256)
            or not image.is_animated
        ):
            raise RuntimeError("Pillow AVIF metadata differs from the pinned sequence")
        for frame_index, expected_hash in enumerate(EXPECTED_FRAME_SHA256):
            image.seek(frame_index)
            image.load()
            if (
                image.info.get("duration") != 100
                or sha256_digest(image.tobytes()).hexdigest() != expected_hash
            ):
                raise RuntimeError(f"Pillow frame {frame_index} differs from its reference")


def validate_frame_headers(encoded: bytes) -> None:
    """Require an 8-bit I444 key/inter sequence whose frames are lossless."""
    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="avif-i444-lossless-b64x64-", dir=STAGING_ROOT
    ) as directory:
        path = Path(directory) / "fixture.avif"
        path.write_bytes(encoded)
        report = inspect_obus(path)

    samples = sorted(
        (sample for sample in report["samples"] if sample.get("role") == "track_pict"),
        key=lambda sample: sample["identity"]["sample"],
    )
    if len(samples) != 2:
        raise RuntimeError("AVIF must contain exactly two picture-track samples")
    sequence_headers = []
    frame_headers = []
    for sample in samples:
        sequence_headers.extend(
            obu["sequence_header"]
            for obu in sample["obus"]
            if "sequence_header" in obu
        )
        headers = [obu["frame_header"] for obu in sample["obus"] if "frame_header" in obu]
        if len(headers) != 1:
            raise RuntimeError("each AV1 picture sample must contain one frame header")
        frame_headers.extend(headers)
    if not sequence_headers:
        raise RuntimeError("AV1 picture samples must declare a sequence header")

    sequence = sequence_headers[0]
    if (
        any(header != sequence for header in sequence_headers[1:])
        or sequence.get("profile") != 1
        or sequence.get("bit_depth") != 8
        or sequence.get("monochrome") is not False
        or sequence.get("subsampling_x") != 0
        or sequence.get("subsampling_y") != 0
    ):
        raise RuntimeError("sequence header differs from the pinned 8-bit I444 profile")
    if (
        [header.get("frame_type") for header in frame_headers] != ["key", "inter"]
        or any(header.get("all_lossless") is not True for header in frame_headers)
        or any(header.get("quantization", {}).get("base") != 0 for header in frame_headers)
        or any(
            (header.get("frame_width"), header.get("frame_height")) != (64, 64)
            for header in frame_headers
        )
    ):
        raise RuntimeError("AV1 frames differ from the pinned all-lossless sequence")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ASSET)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the fixture against its pinned hash, Pillow pixels, and AV1 headers",
    )
    args = parser.parse_args()

    check_oracle()
    output = args.output.resolve()
    if output != ASSET.resolve() and STAGING_ROOT.resolve() not in output.parents:
        raise RuntimeError("output must be the fixture path or a staging path")
    if args.check:
        if not output.is_file():
            raise RuntimeError(f"fixture does not exist: {output}")
        encoded = output.read_bytes()
    else:
        encoded = encode_once()
        if encode_once() != encoded:
            raise RuntimeError("repeated AVIF encodes differ after timestamp normalization")
    asset_hash = sha256_digest(encoded).hexdigest()
    if asset_hash != EXPECTED_ASSET_SHA256:
        raise RuntimeError(f"fixture hash changed: {asset_hash}")
    validate_pixels(encoded)
    validate_frame_headers(encoded)

    if args.check:
        if output.read_bytes() != encoded:
            raise RuntimeError(f"fixture differs from deterministic output: {output}")
        print(f"Verified {output} ({len(encoded)} bytes, sha256={asset_hash})")
        return

    output.parent.mkdir(parents=True, exist_ok=True)
    temporary_output = output.with_name(f".{output.name}.tmp")
    try:
        temporary_output.write_bytes(encoded)
        temporary_output.replace(output)
    finally:
        temporary_output.unlink(missing_ok=True)
    print(f"Wrote {output} ({len(encoded)} bytes, sha256={asset_hash})")


if __name__ == "__main__":
    main()
