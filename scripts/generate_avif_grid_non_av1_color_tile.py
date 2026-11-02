#!/usr/bin/env python3
"""Generate a grid AVIF whose first color tile is declared as Exif."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, __version__ as PILLOW_VERSION, features

from inspect_avif_bitstreams import (
    children,
    full_box_reader,
    parse_boxes,
    parse_iinf,
    parse_infe,
    parse_iref,
    unique_box,
)


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "grid.avif"
OUTPUT = ASSETS / "error_grid_non_av1_color_tile.avif"
SOURCE_SHA256 = "bae56368b348b1d847e2bfb662522599f0c63dfe62fb68826c9e42a300ff405d"
OUTPUT_SHA256 = "1721f82f4ea1f128a97041d867fc1a1c1c0a69663ef9e12d26b02e90b5285803"
PILLOW_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
EXPECTED_ERROR = "Failed to decode image: Invalid image grid"
EXPECTED_ITEMS = {1: b"grid", 2: b"av01", 3: b"av01", 5: b"av01", 6: b"av01"}
EXPECTED_REFERENCES = [
    (b"dimg", 1, 2),
    (b"dimg", 1, 3),
    (b"auxl", 5, 2),
    (b"auxl", 6, 3),
]


def fixture_bytes() -> bytes:
    """Change only item 2's declared type, preserving all box extents and AV1 data."""
    source = SOURCE.read_bytes()
    source_digest = sha256(source).hexdigest()
    if source_digest != SOURCE_SHA256:
        raise RuntimeError(f"pinned grid source hash changed: {source_digest}")

    top_level = parse_boxes(source, 0, len(source))
    meta = unique_box(top_level, b"meta")
    if meta is None:
        raise RuntimeError("pinned grid source has no meta box")
    meta_children = children(source, meta, prefix=4)
    iinf = unique_box(meta_children, b"iinf")
    iref = unique_box(meta_children, b"iref")
    if iinf is None or iref is None:
        raise RuntimeError("pinned grid source lacks item information or references")
    if parse_iinf(source, iinf) != EXPECTED_ITEMS:
        raise RuntimeError("pinned grid source item types changed")
    if parse_iref(source, iref) != EXPECTED_REFERENCES:
        raise RuntimeError("pinned grid source item references changed")

    version, _, reader = full_box_reader(source, iinf)
    if version == 0:
        entry_count = reader.u16()
    elif version == 1:
        entry_count = reader.u32()
    else:
        raise RuntimeError(f"unsupported pinned iinf version {version}")
    entries = parse_boxes(source, reader.offset, iinf.end, count=entry_count)

    item_type_offset = None
    for entry in entries:
        if entry.kind != b"infe":
            raise RuntimeError("pinned iinf contains a non-infe entry")
        infe_version, _, _ = full_box_reader(source, entry)
        item_id, kind = parse_infe(source, entry)
        if item_id != 2:
            continue
        if infe_version != 2 or kind != b"av01":
            raise RuntimeError("pinned first color tile infe declaration changed")
        item_type_offset = entry.payload_start + 8
        break

    if item_type_offset is None:
        raise RuntimeError("pinned first color tile item is missing")

    candidate = bytearray(source)
    candidate[item_type_offset : item_type_offset + 4] = b"Exif"
    result = bytes(candidate)
    result_digest = sha256(result).hexdigest()
    if result_digest != OUTPUT_SHA256:
        raise RuntimeError(f"generated fixture hash changed: {result_digest}")
    return result


def verify_pillow(candidate: bytes) -> None:
    """Pin the complete-file Pillow result for the malformed grid."""
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PILLOW_ORACLE:
        raise RuntimeError(f"Pillow oracle differs from pin: {observed!r}")

    try:
        with Image.open(BytesIO(candidate)) as image:
            image.verify()
    except RuntimeError as error:
        if str(error) != EXPECTED_ERROR:
            raise RuntimeError(f"Pillow error differs from pin: {error}") from error
    else:
        raise RuntimeError("Pillow unexpectedly accepted the non-AV1 grid tile")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    candidate = fixture_bytes()
    verify_pillow(candidate)
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != candidate:
            raise RuntimeError(f"{OUTPUT.relative_to(ROOT)} differs from its recipe")
        return

    OUTPUT.write_bytes(candidate)


if __name__ == "__main__":
    main()
