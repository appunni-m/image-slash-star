#!/usr/bin/env python3
"""Generate a full-file AVIF with a size-zero top-level ``skip`` box."""

from __future__ import annotations

import argparse
import hashlib
import struct
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "baseline.avif"
OUTPUT = ASSETS / "size_zero_top_level_skip.avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
OUTPUT_SHA256 = "02a2791f3c182f175019224d62d658ed1c5b7654bb15eed98840b38628e81584"


def fixture_bytes() -> bytes:
    """Append a size-zero skip box and payload after the complete still."""
    source = SOURCE.read_bytes()
    source_digest = hashlib.sha256(source).hexdigest()
    if source_digest != SOURCE_SHA256:
        raise RuntimeError(f"pinned AVIF source hash changed: {source_digest}")
    if len(source) < 12 or source[4:8] != b"ftyp":
        raise RuntimeError("pinned AVIF no longer begins with its ftyp box")

    candidate = source + struct.pack(">I4s", 0, b"skip") + b"trailing data through EOF"
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
