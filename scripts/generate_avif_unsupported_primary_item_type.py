#!/usr/bin/env python3
"""Generate a grid AVIF whose primary item is declared as Exif."""

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
    parse_pitm,
    unique_box,
)


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "grid.avif"
OUTPUT = ASSETS / "error_unsupported_primary_item_type.avif"
SOURCE_SHA256 = "bae56368b348b1d847e2bfb662522599f0c63dfe62fb68826c9e42a300ff405d"
OUTPUT_SHA256 = "9285db1aa6df2d36c4d02fa8f2efedeac2f3ca48fc5f6aec64488893c3e3b9b8"
PILLOW_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
EXPECTED_ERROR = "Failed to decode image: Missing or empty image item"
EXPECTED_ITEMS = {1: b"grid", 2: b"av01", 3: b"av01", 5: b"av01", 6: b"av01"}
EXPECTED_REFERENCES = [
    (b"dimg", 1, 2),
    (b"dimg", 1, 3),
    (b"auxl", 5, 2),
    (b"auxl", 6, 3),
]


def fixture_bytes() -> bytes:
    """Change only the primary item's declared type, preserving every extent."""
    source = SOURCE.read_bytes()
    source_digest = sha256(source).hexdigest()
    if source_digest != SOURCE_SHA256:
        raise RuntimeError(f"pinned grid source hash changed: {source_digest}")

    top_level = parse_boxes(source, 0, len(source))
    meta = unique_box(top_level, b"meta")
    if meta is None:
        raise RuntimeError("pinned grid source has no meta box")
    meta_children = children(source, meta, prefix=4)
    pitm = unique_box(meta_children, b"pitm")
    iinf = unique_box(meta_children, b"iinf")
    iref = unique_box(meta_children, b"iref")
    if pitm is None or iinf is None or iref is None:
        raise RuntimeError("pinned grid source lacks primary item metadata")
    if parse_iinf(source, iinf) != EXPECTED_ITEMS:
        raise RuntimeError("pinned grid source item types changed")
    if parse_iref(source, iref) != EXPECTED_REFERENCES:
        raise RuntimeError("pinned grid source item references changed")

    primary_item_id = parse_pitm(source, pitm)
    version, _, reader = full_box_reader(source, iinf)
    if version == 0:
        entry_count = reader.u16()
    elif version == 1:
        entry_count = reader.u32()
    else:
        raise RuntimeError(f"unsupported pinned iinf version {version}")
    entries = parse_boxes(source, reader.offset, iinf.end, count=entry_count)

    primary_entry = None
    for entry in entries:
        if entry.kind != b"infe":
            raise RuntimeError("pinned iinf contains a non-infe entry")
        infe_version, _, _ = full_box_reader(source, entry)
        item_id, kind = parse_infe(source, entry)
        if item_id != primary_item_id:
            continue
        if infe_version != 2 or kind != b"grid":
            raise RuntimeError("pinned primary item declaration changed")
        primary_entry = entry
        break

    if primary_entry is None:
        raise RuntimeError("pinned primary item information is missing")

    item_type_offset = primary_entry.payload_start + 8
    candidate = bytearray(source)
    candidate[item_type_offset : item_type_offset + 4] = b"Exif"
    result = bytes(candidate)
    result_digest = sha256(result).hexdigest()
    if result_digest != OUTPUT_SHA256:
        raise RuntimeError(f"generated fixture hash changed: {result_digest}")
    return result


def verify_pillow(candidate: bytes) -> None:
    """Pin the complete-file Pillow result for the unsupported primary item."""
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
        raise RuntimeError("Pillow unexpectedly accepted the unsupported primary item")


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
