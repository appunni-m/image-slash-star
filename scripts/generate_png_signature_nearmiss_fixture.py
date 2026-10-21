#!/usr/bin/env python3
"""Generate the full-file PNG fixture with a one-byte signature near miss."""

from __future__ import annotations

import argparse
from hashlib import sha256
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/png/1x1.png"
OUTPUT = ROOT / "tests/fixtures/input/images/png/near_miss_signature.png"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
EXPECTED_SOURCE_SHA256 = "611033f4cc0aa9e3348e69eba089499bc6287d57b53832079e64f42564a4e876"
EXPECTED_FIXTURE_SHA256 = "a3b94311d64b39503ea188d69a3717b2e99cbfd768ce979866000d9e52b809b2"


def make_fixture(source: bytes) -> bytes:
    """Change only the last PNG signature byte in the pinned 1x1 file."""
    if sha256(source).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("1x1 PNG source differs from its pinned hash")
    if not source.startswith(PNG_SIGNATURE):
        raise RuntimeError("source does not have the complete PNG signature")

    fixture = bytearray(source)
    fixture[7] ^= 0x01
    if fixture[:7] != source[:7] or fixture[8:] != source[8:]:
        raise RuntimeError("near-miss fixture must change only signature byte 7")
    if fixture[7] == source[7]:
        raise RuntimeError("near-miss fixture did not change signature byte 7")

    encoded = bytes(fixture)
    if sha256(encoded).hexdigest() != EXPECTED_FIXTURE_SHA256:
        raise RuntimeError("generated PNG near-miss differs from its pinned hash")
    return encoded


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    arguments = parser.parse_args()

    fixture = make_fixture(SOURCE.read_bytes())
    if arguments.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != fixture:
            raise RuntimeError(f"fixture is stale or missing: {OUTPUT.relative_to(ROOT)}")
        print(f"Verified {OUTPUT.relative_to(ROOT)}")
        return 0

    if OUTPUT.exists() and OUTPUT.read_bytes() != fixture:
        raise RuntimeError(f"refusing to replace different fixture: {OUTPUT.relative_to(ROOT)}")
    OUTPUT.write_bytes(fixture)
    print(f"Wrote {OUTPUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
