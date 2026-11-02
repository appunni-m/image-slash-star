#!/usr/bin/env python3
"""Generate an RGB AVIF grid whose auxiliary alpha items are unreferenced."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, __version__ as PILLOW_VERSION, features

from avif_bmff_boxes import pack_box
from inspect_avif_bitstreams import (
    children,
    parse_boxes,
    parse_iloc,
    parse_iref,
    unique_box,
)


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/avif"
SOURCE = ASSETS / "grid.avif"
OUTPUT = ASSETS / "grid_unreferenced_alpha_items.avif"
SOURCE_SHA256 = "bae56368b348b1d847e2bfb662522599f0c63dfe62fb68826c9e42a300ff405d"
OUTPUT_SHA256 = "4f687d744f2afefcf00d387b5c21607d6e52922593b249e9dcf194a1da13926b"
PIXELS_SHA256 = "f9a11555c837abeb321bdedd737638cc4a62c1784ad58f9b699fd49d3288c019"
PILLOW_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def fixture_bytes() -> bytes:
    """Remove alpha relationships, retaining the grid and every item payload."""
    source = SOURCE.read_bytes()
    if sha256(source) != SOURCE_SHA256:
        raise RuntimeError("pinned grid source bytes changed")
    meta = unique_box(parse_boxes(source, 0, len(source)), b"meta")
    if meta is None:
        raise RuntimeError("pinned grid source has no meta box")
    metadata = children(source, meta, prefix=4)
    iref = unique_box(metadata, b"iref")
    iloc = unique_box(metadata, b"iloc")
    if iref is None or iloc is None or parse_iref(source, iref) != [
        (b"dimg", 1, 2), (b"dimg", 1, 3),
        (b"auxl", 5, 2), (b"auxl", 6, 3),
    ]:
        raise RuntimeError("pinned grid item-reference layout changed")
    references = children(source, iref, prefix=4)
    grid_references = [box for box in references if box.kind == b"dimg"]
    if len(grid_references) != 1:
        raise RuntimeError("pinned grid must have one two-cell reference box")
    payload = source[iref.payload_start : iref.payload_start + 4]
    payload += b"".join(source[box.start : box.end] for box in grid_references)
    rebuilt = pack_box(b"iref", payload)
    padding_size = iref.end - iref.start - len(rebuilt)
    if padding_size < 8:
        raise RuntimeError("removed alpha references cannot hold a free box")
    # A sibling padding box preserves every absolute item extent. Alpha items
    # remain well-formed but have no relationship to either color cell.
    rebuilt += pack_box(b"free", bytes(padding_size - 8))
    candidate = source[: iref.start] + rebuilt + source[iref.end :]
    if sha256(candidate) != OUTPUT_SHA256:
        raise RuntimeError("generated RGB grid bytes differ from their pin")
    for construction, spans in parse_iloc(source, iloc, idat=None).values():
        if construction != "file":
            raise RuntimeError("pinned grid item is not stored in the file")
        for start, end in spans:
            if candidate[start:end] != source[start:end]:
                raise RuntimeError("RGB grid changed an existing item payload")
    return candidate


def verify_pillow(candidate: bytes) -> None:
    """Verify the complete RGB grid and its exact pinned Pillow pixels."""
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PILLOW_ORACLE:
        raise RuntimeError(f"Pillow oracle differs from pin: {observed!r}")
    with Image.open(BytesIO(candidate)) as image:
        image.verify()
    with Image.open(BytesIO(candidate)) as image:
        image.load()
        pixels = image.tobytes()
        if (image.format, image.mode, image.size, image.n_frames) != (
            "AVIF", "RGB", (80, 80), 1
        ) or len(pixels) != 80 * 80 * 3 or sha256(pixels) != PIXELS_SHA256:
            raise RuntimeError("Pillow RGB grid pixels or metadata differ from their pin")
    with Image.open(SOURCE) as source:
        source.load()
        if source.convert("RGB").tobytes() != pixels:
            raise RuntimeError("RGB grid changed the source color channels")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    candidate = fixture_bytes()
    verify_pillow(candidate)
    if args.check:
        if OUTPUT.read_bytes() != candidate:
            raise RuntimeError("committed RGB grid differs from its recipe")
    else:
        if OUTPUT.exists() and OUTPUT.read_bytes() != candidate:
            raise RuntimeError(f"refusing to replace differing fixture: {OUTPUT}")
        OUTPUT.write_bytes(candidate)
    print(f"Verified {OUTPUT.relative_to(ROOT)} ({len(candidate)} bytes)")


if __name__ == "__main__":
    main()
