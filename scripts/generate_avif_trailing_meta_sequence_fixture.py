#!/usr/bin/env python3
"""Append handler-only AVIS metadata after the complete track media."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from generate_avif_sequence_selection_fixtures import shift_track_chunk_offsets
from inspect_avif_bitstreams import parse_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "animated_sequence_only_no_meta.avif"
OUTPUT = ASSETS / "animated_sequence_only_trailing_meta_after_mdat.avif"
INTERSTITIAL_OUTPUT = ASSETS / "animated_sequence_only_interstitial_meta_before_moov.avif"
SOURCE_SHA256 = "c861761e45518018150dc52302da7ff5015d1f259ff2c1309eea94eaf8176fdb"
OUTPUT_SHA256 = "a58d67aaece0bd6502b9f3bb63467e22de4452d4d99d9fb21d088d371f0e0c1f"
INTERSTITIAL_OUTPUT_SHA256 = "2c60d420f540730712e700640cd01aa8d71657029ed4873321137eecf159fc59"


def box(kind: bytes, payload: bytes) -> bytes:
    """Encode one bounded ISO-BMFF box with a 32-bit size."""
    if len(kind) != 4:
        raise ValueError("box type must contain exactly four bytes")
    size = 8 + len(payload)
    if size > 0xFFFF_FFFF:
        raise ValueError("box exceeds the 32-bit size field")
    return size.to_bytes(4, "big") + kind + payload


def fixture_bytes() -> bytes:
    """Append a meta box containing only the required pict handler."""
    source = SOURCE.read_bytes()
    source_digest = hashlib.sha256(source).hexdigest()
    if source_digest != SOURCE_SHA256:
        raise RuntimeError(f"pinned AVIS source hash changed: {source_digest}")

    source_boxes = parse_boxes(source, 0, len(source))
    if (
        not source_boxes
        or source_boxes[0].kind != b"ftyp"
        or source_boxes[-1].kind != b"mdat"
        or source_boxes[-1].end != len(source)
    ):
        raise RuntimeError("pinned AVIS source no longer ends with its complete mdat")

    handler = b"\0" * 8 + b"pict" + b"\0" * 12 + b"\0"
    meta = box(b"meta", b"\0" * 4 + box(b"hdlr", handler))
    candidate = source + meta
    candidate_digest = hashlib.sha256(candidate).hexdigest()
    if candidate_digest != OUTPUT_SHA256:
        raise RuntimeError(f"generated fixture hash changed: {candidate_digest}")
    return candidate


def interstitial_fixture_bytes() -> bytes:
    """Place ignorable AVIS metadata after mdat and before its movie box."""
    source = SOURCE.read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned AVIS source hash changed")

    source_boxes = parse_boxes(source, 0, len(source))
    if [box.kind for box in source_boxes] != [b"ftyp", b"free", b"moov", b"mdat"]:
        raise RuntimeError("pinned AVIS source no longer has its expected top-level layout")

    appended = fixture_bytes()
    appended_boxes = parse_boxes(appended, 0, len(appended))
    if appended_boxes[-1].kind != b"meta":
        raise RuntimeError("appended AVIS fixture no longer ends with meta")
    meta = appended[appended_boxes[-1].start : appended_boxes[-1].end]
    ftyp, free, moov, mdat = source_boxes
    candidate = bytearray(
        source[ftyp.start : ftyp.end]
        + source[free.start : free.end]
        + source[mdat.start : mdat.end]
        + meta
        + source[moov.start : moov.end]
    )
    shift_track_chunk_offsets(candidate, -moov.size)

    result = bytes(candidate)
    result_digest = hashlib.sha256(result).hexdigest()
    if result_digest != INTERSTITIAL_OUTPUT_SHA256:
        raise RuntimeError(f"generated interstitial fixture hash changed: {result_digest}")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    outputs = (
        (OUTPUT, fixture_bytes()),
        (INTERSTITIAL_OUTPUT, interstitial_fixture_bytes()),
    )
    if args.check:
        for output, candidate in outputs:
            if not output.is_file() or output.read_bytes() != candidate:
                raise RuntimeError(f"{output.relative_to(ROOT)} differs from its recipe")
    else:
        for output, candidate in outputs:
            output.write_bytes(candidate)


if __name__ == "__main__":
    main()
