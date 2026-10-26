#!/usr/bin/env python3
"""Generate a deterministic lossy I422 sequence with vertical inter splits."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "animated_lossy_inter_422_vertical_split_b32x64.avif"
)
EXPECTED_FILE_SHA256 = "0eb45159064b3f1c30919d0062afa19056415f76be203278cd3ddd7174e8a798"
EXPECTED_FRAME_SHA256 = (
    "f1aab4fdf740e5726556337b0287ec940079cc48a4f1995d5f2b8fb5fbc9348c",
    "c72b8082dac958d7aeda2637782f8226e7238711a7e967959100d25ad270e39d",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def check_pinned_oracle() -> None:
    """Require the exact Pillow AVIF encoder that produced this fixture."""
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow AVIF oracle required; found {observed!r}")


def make_frame(seed: int) -> Image.Image:
    """Build one deterministic 32x64 RGB frame with vertical texture runs."""
    pixels = bytearray()
    for y in range(64):
        for x in range(32):
            half = x // 16
            base = (x // 2 * 37 + y * 19 + half * 83 + seed) & 255
            pixels.extend((base, (base * 3 + x * 5) & 255, (base * 5 + y * 7) & 255))
    return Image.frombytes("RGB", (32, 64), bytes(pixels))


def normalize_sequence_timestamps(encoded: bytes) -> bytes:
    """Zero version-one BMFF creation and modification timestamps."""
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


def encode_fixture() -> bytes:
    """Encode the fixed sequence and verify its exact Pillow RGB frames."""
    frames = [make_frame(1096), make_frame(2096)]
    encoded_sequences = []
    for _ in range(2):
        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=[frames[1]],
            duration=[100, 100],
            loop=0,
            quality=95,
            speed=0,
            max_threads=1,
            subsampling="4:2:2",
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
                "enable-interintra-comp": "0",
                "enable-warped-motion": "0",
                "enable-global-motion": "0",
            },
        )
        encoded_sequences.append(normalize_sequence_timestamps(output.getvalue()))
    if encoded_sequences[0] != encoded_sequences[1]:
        raise RuntimeError("repeated AVIF encodes differ")

    encoded = encoded_sequences[0]
    digest = hashlib.sha256(encoded).hexdigest()
    if digest != EXPECTED_FILE_SHA256:
        raise RuntimeError(f"fixture hash changed: {digest}")

    frame_hashes = []
    with Image.open(BytesIO(encoded)) as decoded:
        if decoded.format != "AVIF" or decoded.mode != "RGB" or decoded.size != (32, 64):
            raise RuntimeError("Pillow AVIF metadata differs from the pinned fixture")
        if decoded.n_frames != len(EXPECTED_FRAME_SHA256):
            raise RuntimeError(f"expected two frames, found {decoded.n_frames}")
        for frame_index in range(decoded.n_frames):
            decoded.seek(frame_index)
            decoded.load()
            frame_hashes.append(
                hashlib.sha256(decoded.convert("RGB").tobytes()).hexdigest()
            )
    if tuple(frame_hashes) != EXPECTED_FRAME_SHA256:
        raise RuntimeError(f"Pillow frame hashes changed: {tuple(frame_hashes)!r}")
    return encoded


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing the fixture")
    args = parser.parse_args()
    check_pinned_oracle()
    encoded = encode_fixture()

    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != encoded:
            raise RuntimeError(f"fixture differs from deterministic output: {OUTPUT}")
        print(f"Verified {OUTPUT} (sha256={EXPECTED_FILE_SHA256})")
        return

    if OUTPUT.exists() and OUTPUT.read_bytes() != encoded:
        raise RuntimeError(f"refusing to replace differing fixture: {OUTPUT}")
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(encoded)
    print(f"Wrote {OUTPUT} ({len(encoded)} bytes, sha256={EXPECTED_FILE_SHA256})")


if __name__ == "__main__":
    main()
