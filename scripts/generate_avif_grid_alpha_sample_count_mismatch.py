#!/usr/bin/env python3
"""Generate a grid AVIF with one color cell missing its alpha relationship."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, __version__ as PILLOW_VERSION, features

from inspect_avif_bitstreams import children, parse_boxes, parse_iref, unique_box


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "grid.avif"
OUTPUT = ASSETS / "error_grid_alpha_sample_count_mismatch.avif"
SOURCE_SHA256 = "bae56368b348b1d847e2bfb662522599f0c63dfe62fb68826c9e42a300ff405d"
OUTPUT_SHA256 = "e63f9dfda938353e27c05834e4b736fd855bbdb273d9b8c2d661348e710e3e50"
PILLOW_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
EXPECTED_ERROR = "Failed to decode image: Invalid image grid"
EXPECTED_REFERENCES = [
    (b"dimg", 1, 2),
    (b"dimg", 1, 3),
    (b"auxl", 5, 2),
    (b"auxl", 6, 3),
]


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def fixture_bytes() -> bytes:
    """Repoint the second alpha cell to the first color cell without resizing."""
    source = SOURCE.read_bytes()
    source_digest = sha256(source)
    if source_digest != SOURCE_SHA256:
        raise RuntimeError(f"pinned grid source hash changed: {source_digest}")

    top_level = parse_boxes(source, 0, len(source))
    meta = unique_box(top_level, b"meta")
    if meta is None:
        raise RuntimeError("pinned grid source has no meta box")
    iref = unique_box(children(source, meta, prefix=4), b"iref")
    if iref is None or parse_iref(source, iref) != EXPECTED_REFERENCES:
        raise RuntimeError("pinned grid item-reference layout changed")

    alpha_reference = next(
        (
            child
            for child in children(source, iref, prefix=4)
            if child.kind == b"auxl"
            and source[child.payload_start : child.end] == bytes.fromhex("000600010003")
        ),
        None,
    )
    if alpha_reference is None:
        raise RuntimeError("pinned second alpha-cell reference changed")

    candidate = bytearray(source)
    candidate[alpha_reference.end - 2 : alpha_reference.end] = b"\x00\x02"
    result = bytes(candidate)
    result_digest = sha256(result)
    if result_digest != OUTPUT_SHA256:
        raise RuntimeError(f"generated fixture hash changed: {result_digest}")
    return result


def verify_pillow(candidate: bytes) -> None:
    """Pin the oracle and its exact error for the complete candidate file."""
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
        raise RuntimeError("Pillow unexpectedly accepted the incomplete alpha grid")


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
