#!/usr/bin/env python3
"""Generate an AVIF sequence declared with the still-image major brand."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
FIXTURE_DIR = ROOT / "tests/fixtures/input/images/avif"
SOURCE = FIXTURE_DIR / "animated_motion_chroma.avif"
OUTPUT = FIXTURE_DIR / "animated_sequence_still_major_brand.avif"
SOURCE_SHA256 = "473469335e58cff3c442df51b4bbdca00b9467e8f352319812f50cf789748535"


def generated_fixture() -> bytes:
    """Change only the pinned sequence file's ftyp major brand."""
    source = SOURCE.read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("animated_motion_chroma source differs from its pinned input")
    if len(source) < 16 or source[4:8] != b"ftyp" or source[8:12] != b"avis":
        raise RuntimeError("pinned AVIF sequence no longer starts with an avis ftyp box")
    box_size = int.from_bytes(source[:4], "big")
    if box_size < 16 or box_size > len(source):
        raise RuntimeError("pinned AVIF ftyp box has an invalid extent")

    result = bytearray(source)
    result[8:12] = b"avif"
    return bytes(result)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify the generated fixture")
    args = parser.parse_args()

    expected = generated_fixture()
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != expected:
            raise RuntimeError(f"generated AVIF fixture is stale: {OUTPUT.relative_to(ROOT)}")
        return 0

    OUTPUT.write_bytes(expected)
    print(f"wrote {OUTPUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
