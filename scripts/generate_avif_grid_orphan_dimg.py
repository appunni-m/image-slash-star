#!/usr/bin/env python3
"""Generate a color grid with an orphan ``dimg`` reference."""

from __future__ import annotations

import argparse
import hashlib
import struct
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
OUTPUT = ASSETS / "grid_orphan_dimg_reference.avif"
SOURCE_SHA256 = "bae56368b348b1d847e2bfb662522599f0c63dfe62fb68826c9e42a300ff405d"
OUTPUT_SHA256 = "8e168a723ea15a1a3f50032cb7b427e131929c9f7b0b56198abdd3a281a0ca80"
PIXELS_SHA256 = "f9a11555c837abeb321bdedd737638cc4a62c1784ad58f9b699fd49d3288c019"
PILLOW_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
SOURCE_REFERENCES = [
    (b"dimg", 1, 2),
    (b"dimg", 1, 3),
    (b"auxl", 5, 2),
    (b"auxl", 6, 3),
]
OUTPUT_REFERENCES = [
    (b"dimg", 1, 2),
    (b"dimg", 1, 3),
    (b"dimg", 5, 6),
]


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def fixture_bytes() -> bytes:
    """Add an orphan ``dimg`` edge while preserving the RGB grid and its extents."""
    source = SOURCE.read_bytes()
    if sha256(source) != SOURCE_SHA256:
        raise RuntimeError("pinned grid source bytes changed")

    meta = unique_box(parse_boxes(source, 0, len(source)), b"meta")
    if meta is None:
        raise RuntimeError("pinned grid source has no meta box")
    metadata = children(source, meta, prefix=4)
    iref = unique_box(metadata, b"iref")
    iloc = unique_box(metadata, b"iloc")
    if (
        iref is None
        or iloc is None
        or source[iref.payload_start : iref.payload_start + 4] != bytes(4)
    ):
        raise RuntimeError("pinned grid item references are not version zero")
    if parse_iref(source, iref) != SOURCE_REFERENCES:
        raise RuntimeError("pinned grid item-reference layout changed")

    grid_reference = next(
        (box for box in children(source, iref, prefix=4) if box.kind == b"dimg"),
        None,
    )
    if grid_reference is None:
        raise RuntimeError("pinned color grid has no dimg reference")
    orphan_reference = pack_box(b"dimg", struct.pack(">HHH", 5, 1, 6))
    payload = source[iref.payload_start : iref.payload_start + 4]
    payload += source[grid_reference.start : grid_reference.end]
    payload += orphan_reference
    rebuilt_iref = pack_box(b"iref", payload)
    padding_size = iref.end - iref.start - len(rebuilt_iref)
    if padding_size < 8:
        raise RuntimeError("pinned iref has no room for a sibling padding box")
    candidate = (
        source[: iref.start]
        + rebuilt_iref
        + pack_box(b"free", bytes(padding_size - 8))
        + source[iref.end :]
    )
    if len(candidate) != len(source) or sha256(candidate) != OUTPUT_SHA256:
        raise RuntimeError("generated orphan-dimg fixture differs from its pin")

    candidate_meta = unique_box(parse_boxes(candidate, 0, len(candidate)), b"meta")
    if candidate_meta is None:
        raise RuntimeError("generated grid has no meta box")
    candidate_iref = unique_box(children(candidate, candidate_meta, prefix=4), b"iref")
    if candidate_iref is None or parse_iref(candidate, candidate_iref) != OUTPUT_REFERENCES:
        raise RuntimeError("generated grid item references differ from their pin")
    for construction, spans in parse_iloc(source, iloc, idat=None).values():
        if construction != "file":
            raise RuntimeError("pinned grid item is not stored in the file")
        for start, end in spans:
            if candidate[start:end] != source[start:end]:
                raise RuntimeError("orphan dimg changed an existing item payload")
    return candidate


def verify_pillow(candidate: bytes) -> None:
    """Pin Pillow's accepted RGB output and prove its pixels match the source."""
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PILLOW_ORACLE:
        raise RuntimeError(f"Pillow oracle differs from pin: {observed!r}")

    with Image.open(BytesIO(candidate)) as image:
        image.verify()
    with Image.open(BytesIO(candidate)) as image:
        image.load()
        pixels = image.tobytes()
        if (image.format, image.mode, image.size, image.n_frames) != (
            "AVIF",
            "RGB",
            (80, 80),
            1,
        ) or sha256(pixels) != PIXELS_SHA256:
            raise RuntimeError("Pillow orphan-dimg output differs from its pixel pin")

    with Image.open(SOURCE) as image:
        image.load()
        if image.convert("RGB").tobytes() != pixels:
            raise RuntimeError("orphan dimg reference changed the color-grid pixels")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    candidate = fixture_bytes()
    verify_pillow(candidate)
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != candidate:
            raise RuntimeError("committed orphan-dimg fixture differs from its recipe")
    else:
        if OUTPUT.exists() and OUTPUT.read_bytes() != candidate:
            raise RuntimeError(f"refusing to replace differing fixture: {OUTPUT}")
        OUTPUT.write_bytes(candidate)
    print(f"Verified {OUTPUT.relative_to(ROOT)} ({len(candidate)} bytes)")


if __name__ == "__main__":
    main()
