#!/usr/bin/env python3
"""Generate a complete AVIF with a file-backed item extent past EOF.

The fixture keeps the pinned baseline file and every BMFF box unchanged except
for increasing the primary item's declared ``iloc`` extent length by one byte.
Pillow can inspect and verify the file, then reports truncated data on decode.
"""

from __future__ import annotations

import argparse
import hashlib
import struct
from io import BytesIO
from pathlib import Path

import yaml
from PIL import Image, ImageSequence

from generate_avif_zero_width_clap_fixture import pack_box, read_boxes
from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests" / "fixtures" / "input" / "images" / "avif" / "baseline.avif"
OUTPUT_DIR = SOURCE.parent
FILENAME = "error_iloc_file_extent_out_of_bounds.avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
FIXTURE_SHA256 = "764f4205c7b60acd296e37e9d2e45968c66aafd4fc84102366d67e9903d237ad"
BASELINE_ILOC_PREFIX = bytes.fromhex("0000000044000001000100000001")
DECODE_ERROR = "Failed to decode frame 0: Truncated data"


def sha256(data: bytes) -> str:
    """Return the hexadecimal SHA-256 digest for one fixture input."""
    return hashlib.sha256(data).hexdigest()


def extend_primary_extent(source: bytes) -> bytes:
    """Return the baseline AVIF with its primary file extent one byte too long."""
    if sha256(source) != SOURCE_SHA256:
        raise RuntimeError("baseline AVIF differs from its pinned source hash")

    top_level = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in top_level] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("baseline AVIF top-level box layout changed")
    ftyp, meta, mdat = (complete for _, complete, _ in top_level)
    children = read_boxes(meta, 12, len(meta))
    locations = [payload for kind, _, payload in children if kind == b"iloc"]
    coded_item = mdat[8:]
    expected_offset = len(ftyp) + len(meta) + 8
    expected_location = BASELINE_ILOC_PREFIX + struct.pack(
        ">II", expected_offset, len(coded_item)
    )
    if len(locations) != 1 or locations[0] != expected_location:
        raise RuntimeError("baseline AVIF primary-item extent changed")
    if expected_offset + len(coded_item) != len(source):
        raise RuntimeError("baseline AVIF item no longer ends exactly at EOF")

    location = locations[0]
    extent_length = struct.unpack_from(">I", location, len(location) - 4)[0]
    mutated_location = location[:-4] + struct.pack(">I", extent_length + 1)
    rebuilt_children = [
        pack_box(b"iloc", mutated_location) if kind == b"iloc" else complete
        for kind, complete, _ in children
    ]
    candidate = ftyp + pack_box(b"meta", meta[8:12] + b"".join(rebuilt_children)) + mdat
    if sha256(candidate) != FIXTURE_SHA256:
        raise RuntimeError("mutated AVIF differs from its pinned fixture hash")
    return candidate


def verify_pillow(candidate: bytes) -> None:
    """Check the exact pinned Pillow open, inspect, verify, and decode results."""
    with Image.open(BytesIO(candidate)) as image:
        header = (
            image.format,
            image.mode,
            image.size,
            image.n_frames,
            image.is_animated,
            dict(image.info),
        )
        if header != ("AVIF", "RGB", (128, 128), 1, False, {}):
            raise RuntimeError(f"Pillow inspection changed: {header!r}")

    with Image.open(BytesIO(candidate)) as image:
        image.verify()

    for operation in ("decode", "decode_sequence"):
        try:
            with Image.open(BytesIO(candidate)) as image:
                if operation == "decode":
                    image.load()
                    image.tobytes()
                else:
                    for frame in ImageSequence.Iterator(image):
                        frame.load()
                        frame.tobytes()
        except SyntaxError as error:
            if str(error) != DECODE_ERROR:
                raise RuntimeError(
                    f"Pillow {operation} diagnostic changed: {error!r}"
                ) from error
        else:
            raise RuntimeError(f"Pillow unexpectedly completed {operation}")


def generate(source_path: Path = SOURCE, output_dir: Path = OUTPUT_DIR) -> Path:
    """Verify and write the pinned file-backed extent fixture."""
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_bytes())
    verify_primary_oracle(manifest)
    candidate = extend_primary_extent(source_path.read_bytes())
    verify_pillow(candidate)

    output_dir.mkdir(parents=True, exist_ok=True)
    path = output_dir / FILENAME
    if path.exists() and path.read_bytes() != candidate:
        raise RuntimeError(f"refusing to replace a different fixture at {path}")
    if not path.exists():
        path.write_bytes(candidate)
    print(f"Verified AVIF file-backed extent fixture: {path}")
    return path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=SOURCE)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT_DIR)
    arguments = parser.parse_args()
    generate(arguments.source, arguments.output_dir)


if __name__ == "__main__":
    main()
