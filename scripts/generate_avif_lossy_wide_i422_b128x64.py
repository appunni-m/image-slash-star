#!/usr/bin/env python3
"""Generate the pinned lossy I422 B128x64 inter-block AVIF fixture."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import tempfile
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
    / "animated_lossy_wide_i422_b128x64.avif"
)
EXPECTED_FILE_SHA256 = "dbfb8c364abaddaeab4894511ff28f42fa9512226a27af097da35975a4ee5dbc"
EXPECTED_FRAME_SHA256 = (
    "180e2a96e64100f8d42892dcfd2a19b6d16329d325a527811fbeee5008682c65",
    "8cd9f587054f3e1d17152aa6b8c2b743c3f9d75061a2d165bd9d333156133ada",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


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


def check_pinned_oracle() -> None:
    """Require the repository's pinned Pillow and AVIF runtime."""
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow AVIF oracle required; found {observed!r}")


def frame_hashes(encoded: bytes) -> tuple[str, ...]:
    """Load and hash both exact Pillow RGB frames."""
    with Image.open(BytesIO(encoded)) as image:
        if image.format != "AVIF" or image.mode != "RGB" or image.size != (128, 128):
            raise RuntimeError("Pillow AVIF metadata differs from the pinned fixture")
        if image.n_frames != len(EXPECTED_FRAME_SHA256):
            raise RuntimeError(f"expected two frames, found {image.n_frames}")
        hashes = []
        for frame_index in range(image.n_frames):
            image.seek(frame_index)
            image.load()
            pixels = image.convert("RGB").tobytes()
            hashes.append(hashlib.sha256(pixels).hexdigest())
    return tuple(hashes)


def encode_twice(encoder: Path) -> bytes:
    """Run the fixture-only AOM encoder twice and require stable bytes."""
    with tempfile.TemporaryDirectory(prefix="avif-i422-b128x64-") as directory:
        paths = [Path(directory) / f"encoded-{index}.avif" for index in range(2)]
        for path in paths:
            subprocess.run([str(encoder), str(path)], check=True, cwd=ROOT)
        encoded = [normalize_sequence_timestamps(path.read_bytes()) for path in paths]
    if encoded[0] != encoded[1]:
        raise RuntimeError("I422 B128x64 AVIF fixture is not deterministic")
    return encoded[0]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()
    check_pinned_oracle()

    encoder = args.encoder.resolve()
    if not encoder.is_file():
        raise RuntimeError(f"fixture encoder does not exist: {encoder}")
    encoded = encode_twice(encoder)
    file_hash = hashlib.sha256(encoded).hexdigest()
    if file_hash != EXPECTED_FILE_SHA256:
        raise RuntimeError(f"fixture hash changed: {file_hash}")
    hashes = frame_hashes(encoded)
    if hashes != EXPECTED_FRAME_SHA256:
        raise RuntimeError(f"Pillow frame hashes changed: {hashes!r}")

    output = args.output.resolve()
    if output.exists() and output.read_bytes() != encoded:
        raise RuntimeError(f"refusing to replace differing fixture: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded)
    print(f"Wrote {output} ({len(encoded)} bytes, sha256={file_hash})")


if __name__ == "__main__":
    main()
