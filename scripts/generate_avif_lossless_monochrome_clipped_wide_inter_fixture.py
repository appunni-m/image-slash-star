#!/usr/bin/env python3
"""Generate or verify a clipped, wide lossless monochrome AVIF sequence."""

from __future__ import annotations

from hashlib import sha256 as sha256_digest
from io import BytesIO
from pathlib import Path
import tempfile

from PIL import Image, __version__ as PILLOW_VERSION, _avif, features

from generate_avif_multitile_motion_fixture import normalize_timestamps
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
ASSET = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_lossless_inter_monochrome_clipped_b128x128_124x128.avif"
)
SIZE = (124, 128)
EXPECTED_ASSET_SHA256 = "7553404e35f1583e5bb2e785acb4908de5a4bbfa8d888054b163cc8da32391e8"
EXPECTED_FRAME_SHA256 = (
    "f503c9af04a4099c03bedb1461014aa8e4ef8668506fd169c523d7a862e22b87",
    "6717767046bca15b0c19c8ef51ad4abd60dbc6d1effacd520dd2398f2d572f1a",
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
    return sha256_digest(data).hexdigest()


def source_frame(shift: int) -> Image.Image:
    """Build a deterministic monochrome frame with a horizontal translation."""
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            source_x = max(0, x - shift)
            pixels.append((source_x * 73 + y * 151 + source_x * y * 13) & 0xFF)
    return Image.frombytes("L", SIZE, bytes(pixels))


def check_oracle() -> None:
    """Require the exact Pillow and AVIF oracle versions used for the fixture."""
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow/libavif/libaom stack required, found {observed!r}")


def encode_once() -> bytes:
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
        subsampling="4:0:0",
        autotiling=False,
        codec="aom",
        advanced=ADVANCED_OPTIONS,
    )
    return normalize_timestamps(output.getvalue())


def encode_fixture() -> bytes:
    """Encode twice and require byte-identical timestamp-normalized output."""
    first = encode_once()
    second = encode_once()
    if first != second:
        raise RuntimeError("independent monochrome AVIF encodes differ")
    return first


def validate_fixture(encoded: bytes) -> dict[str, object]:
    asset_hash = sha256(encoded)
    if asset_hash != EXPECTED_ASSET_SHA256:
        raise RuntimeError(f"fixture hash changed: {asset_hash}")

    with tempfile.TemporaryDirectory(prefix="avif-lossless-monochrome-wide-") as directory:
        path = Path(directory) / "fixture.avif"
        path.write_bytes(encoded)
        report = inspect_obus(path)

    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if not sequence_headers or any(
        header.get("monochrome") is not True
        or header.get("use_128x128_superblock") is not True
        or (header.get("max_width"), header.get("max_height")) != SIZE
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence must be monochrome with 128x128 superblocks")

    frame_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    if not any(header.get("frame_type") == "inter" for header in frame_headers):
        raise RuntimeError("AV1 sequence has no inter frame")
    if any(
        header.get("all_lossless") is not True
        or (header.get("frame_width"), header.get("frame_height")) != SIZE
        for header in frame_headers
    ):
        raise RuntimeError("AV1 frames must be lossless and retain the clipped 124x128 extent")

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
            frame_hashes.append(sha256(image.tobytes()))
    if tuple(frame_hashes) != EXPECTED_FRAME_SHA256:
        raise RuntimeError(f"Pillow RGB frame hashes changed: {tuple(frame_hashes)!r}")

    return {"asset_bytes": len(encoded), "asset_sha256": asset_hash, "frames": frame_hashes}


def main() -> None:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify the committed fixture")
    args = parser.parse_args()
    check_oracle()
    encoded = encode_fixture()
    observation = validate_fixture(encoded)

    if args.check:
        if not ASSET.is_file() or ASSET.read_bytes() != encoded:
            raise RuntimeError(f"fixture differs from deterministic output: {ASSET}")
        print(f"Verified {ASSET}: {observation}")
        return

    if ASSET.exists() and ASSET.read_bytes() != encoded:
        raise RuntimeError(f"refusing to replace differing fixture: {ASSET}")
    ASSET.parent.mkdir(parents=True, exist_ok=True)
    temporary = ASSET.with_name(f".{ASSET.name}.tmp")
    try:
        temporary.write_bytes(encoded)
        temporary.replace(ASSET)
    finally:
        temporary.unlink(missing_ok=True)
    print(f"Wrote {ASSET}: {observation}")


if __name__ == "__main__":
    main()
