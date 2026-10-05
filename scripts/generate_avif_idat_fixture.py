#!/usr/bin/env python3
"""Generate a complete AVIF whose primary item uses nonzero idat offsets.

The deterministic input mutation keeps the AV1 item bytes and properties from
the pinned still, then selects an idat extent through iloc construction method
one. Live pinned Pillow observations validate the complete image independently.
"""

from __future__ import annotations

import argparse
import hashlib
import struct
from pathlib import Path

import yaml

from generate_avif_config_disagreement_fixtures import pillow_snapshot
from generate_avif_zero_width_clap_fixture import pack_box, read_boxes
from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT_DIR = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
DEFAULT_SOURCE = DEFAULT_OUTPUT_DIR / "baseline.avif"
FILENAME = "iloc_idat_primary_item.avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
BASE_OFFSET = 8
EXTENT_OFFSET = 8
IDAT_PADDING = b"\xff" * (BASE_OFFSET + EXTENT_OFFSET)


def move_primary_item_to_idat(source: bytes) -> bytes:
    """Relocate the pinned primary item while retaining all existing boxes."""
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("baseline AVIF source differs from its pinned hash")
    top = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in top] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("baseline AVIF top-level layout changed")
    ftyp, meta, mdat = (complete for _, complete, _ in top)
    coded_item = mdat[8:]
    children = read_boxes(meta, 12, len(meta))
    locations = [payload for kind, _, payload in children if kind == b"iloc"]
    if (
        meta[8:12] != bytes(4)
        or len(locations) != 1
        or any(kind == b"idat" for kind, _, _ in children)
        or locations[0] != bytes.fromhex("0000000044000001000100000001")
        + struct.pack(">II", len(ftyp) + len(meta) + 8, len(coded_item))
    ):
        raise RuntimeError("baseline AVIF primary-item extent changed")

    # iloc v1: four-byte offset, length and base offset; no extent index.
    # Item 1 uses construction method 1 and data_reference_index zero.
    location = bytes.fromhex("0100000044400001000100010000") + struct.pack(
        ">IHII", BASE_OFFSET, 1, EXTENT_OFFSET, len(coded_item)
    )
    rebuilt = [
        pack_box(b"iloc", location) if kind == b"iloc" else complete
        for kind, complete, _ in children
    ]
    rebuilt.append(pack_box(b"idat", IDAT_PADDING + coded_item))
    # The original mdat remains unchanged and unreferenced. No absolute file
    # extent remains to rebase when the meta box grows.
    return ftyp + pack_box(b"meta", meta[8:12] + b"".join(rebuilt)) + mdat


def generate(
    output_dir: Path = DEFAULT_OUTPUT_DIR, source_path: Path = DEFAULT_SOURCE
) -> Path:
    """Generate or verify the idat fixture for the normal asset runner."""
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_bytes())
    verify_primary_oracle(manifest)
    source = source_path.read_bytes()
    candidate = move_primary_item_to_idat(source)
    if pillow_snapshot(candidate) != pillow_snapshot(source):
        raise RuntimeError("AVIF idat mutation changed Pillow's public image result")
    path = output_dir / FILENAME
    if path.exists() and path.read_bytes() != candidate:
        raise RuntimeError(f"refusing to replace a different fixture at {path}")
    output_dir.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        path.write_bytes(candidate)
    print(f"Verified AVIF idat fixture: {path}")
    return path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    args = parser.parse_args()
    generate(args.output_dir, args.source)


if __name__ == "__main__":
    main()
