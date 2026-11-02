#!/usr/bin/env python3
"""Generate the pinned lossy I422 sequence with small inter partitions."""

from __future__ import annotations

import argparse
import hashlib
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "animated_lossy_inter_422_small_partitions_32x64.avif"
)
EXPECTED_FILE_SHA256 = "f150bd51312eee0a9e7dccc57b3d26fc08899c400d82565b839d704e98c6c029"
EXPECTED_FRAME_SHA256 = (
    "5eed454eb226d4b477be8d1f072e6661232361c868ebe3ad15e6a663e79b6105",
    "d9eb51dd91740c02a7327a20ce7c33647f2a5503f7b60cb664cbe39e949c65c1",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of bytes."""
    return hashlib.sha256(data).hexdigest()


def check_pinned_oracle() -> None:
    """Require the exact Pillow AVIF encoder used for this fixture."""
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow AVIF oracle required; found {observed!r}")


def make_frame(seed: int) -> Image.Image:
    """Build a deterministic 32x64 RGB frame with vertical texture runs."""
    pixels = bytearray()
    for y in range(64):
        for x in range(32):
            half = x // 16
            base = (x // 2 * 37 + y * 19 + half * 83 + seed) & 255
            pixels.extend((base, (base * 3 + x * 5) & 255, (base * 5 + y * 7) & 255))
    return Image.frombytes("RGB", (32, 64), bytes(pixels))


def normalize_sequence_timestamps(encoded: bytes) -> bytes:
    """Zero the version-one BMFF creation and modification timestamps."""
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


def encode_once() -> bytes:
    """Encode one fixed two-frame I422 sequence with bounded partitions."""
    output = BytesIO()
    make_frame(1096).save(
        output,
        format="AVIF",
        save_all=True,
        append_images=[make_frame(2096)],
        duration=[100, 100],
        loop=0,
        quality=70,
        speed=0,
        max_threads=1,
        subsampling="4:2:2",
        autotiling=False,
        codec="aom",
        advanced={
            "min-partition-size": "8",
            "max-partition-size": "16",
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
    return normalize_sequence_timestamps(output.getvalue())


def validate_pixels(encoded: bytes) -> None:
    """Require the exact pinned Pillow metadata and both RGB frames."""
    with Image.open(BytesIO(encoded)) as image:
        if image.format != "AVIF" or image.mode != "RGB" or image.size != (32, 64):
            raise RuntimeError("Pillow AVIF metadata differs from the pinned fixture")
        if image.n_frames != len(EXPECTED_FRAME_SHA256):
            raise RuntimeError(f"expected two frames, found {image.n_frames}")
        for frame_index, expected_hash in enumerate(EXPECTED_FRAME_SHA256):
            image.seek(frame_index)
            image.load()
            pixels = image.tobytes()
            if (
                len(pixels) != 32 * 64 * 3
                or sha256(pixels) != expected_hash
                or image.info.get("duration") != 100
            ):
                raise RuntimeError(f"Pillow frame {frame_index} differs from its reference")


def validate_frame_headers(encoded: bytes) -> None:
    """Require an 8-bit I422 key/inter sequence with lossy TX_MODE_SELECT."""
    staging = ROOT / "target" / "oracle-staging"
    staging.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="avif-i422-small-partitions-", dir=staging) as directory:
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
            obu["sequence_header"] for obu in sample["obus"] if "sequence_header" in obu
        )
        headers = [obu["frame_header"] for obu in sample["obus"] if "frame_header" in obu]
        if len(headers) != 1:
            raise RuntimeError("each AV1 picture sample must contain exactly one frame header")
        frame_headers.extend(headers)
    if not sequence_headers:
        raise RuntimeError("AV1 picture samples must declare a sequence header")
    sequence = sequence_headers[0]
    if (
        any(header != sequence for header in sequence_headers[1:])
        or sequence.get("profile") != 2
        or sequence.get("bit_depth") != 8
        or sequence.get("monochrome") is not False
        or sequence.get("subsampling_x") != 1
        or sequence.get("subsampling_y") != 0
    ):
        raise RuntimeError("sequence header differs from the pinned 8-bit I422 profile")
    key, inter = frame_headers
    if (
        key.get("frame_type") != "key"
        or inter.get("frame_type") != "inter"
        or inter.get("frame_width") != 32
        or inter.get("frame_height") != 64
        or inter.get("transform_mode") != "select"
        or inter.get("all_lossless") is not False
        or inter.get("quantization", {}).get("base", 0) <= 0
    ):
        raise RuntimeError("inter frame differs from the pinned lossy I422 profile")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed fixture against a fresh pinned Pillow encode",
    )
    args = parser.parse_args()
    check_pinned_oracle()
    encoded = encode_once()
    if encode_once() != encoded:
        raise RuntimeError("repeated AVIF encodes differ")
    digest = sha256(encoded)
    if digest != EXPECTED_FILE_SHA256:
        raise RuntimeError(f"fixture hash changed: {digest}")
    validate_pixels(encoded)
    validate_frame_headers(encoded)

    output = args.output.resolve()
    if args.check:
        if not output.is_file() or output.read_bytes() != encoded:
            raise RuntimeError(f"fixture differs from deterministic output: {output}")
        print(f"Verified {output} ({len(encoded)} bytes, sha256={digest})")
        return
    if output.exists() and output.read_bytes() != encoded:
        raise RuntimeError(f"refusing to replace differing fixture: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded)
    print(f"Wrote {output} ({len(encoded)} bytes, sha256={digest})")


if __name__ == "__main__":
    main()
