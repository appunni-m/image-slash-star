#!/usr/bin/env python3
"""Generate a full-file AVIS fixture without its required leading ftyp box."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "animated_sequence_only_no_meta.avif"
OUTPUT = ASSETS / "error_animated_missing_leading_ftyp.avif"
SOURCE_SHA256 = "c861761e45518018150dc52302da7ff5015d1f259ff2c1309eea94eaf8176fdb"
OUTPUT_SHA256 = "d45842074621fd7572ef6331e44fd97691dfd74584e0a58ec198cf60524f8020"


def fixture_bytes() -> bytes:
    """Change only the first complete box type, preserving all file extents."""
    source = SOURCE.read_bytes()
    source_digest = hashlib.sha256(source).hexdigest()
    if source_digest != SOURCE_SHA256:
        raise RuntimeError(f"pinned AVIS source hash changed: {source_digest}")
    if len(source) < 8 or source[4:8] != b"ftyp":
        raise RuntimeError("pinned AVIS no longer begins with its ftyp box")

    candidate = source[:4] + b"free" + source[8:]
    candidate_digest = hashlib.sha256(candidate).hexdigest()
    if candidate_digest != OUTPUT_SHA256:
        raise RuntimeError(f"generated fixture hash changed: {candidate_digest}")
    return candidate


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    candidate = fixture_bytes()
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != candidate:
            raise RuntimeError(f"{OUTPUT.relative_to(ROOT)} differs from its recipe")
    else:
        OUTPUT.write_bytes(candidate)


if __name__ == "__main__":
    main()
