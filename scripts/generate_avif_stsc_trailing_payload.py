#!/usr/bin/env python3
"""Generate a valid animated AVIF whose stsc box has one trailing byte."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct

from avif_bmff_boxes import read_boxes, rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "animated_stsc_two_chunk_mapping.avif"
OUTPUT = ASSETS / "animated_stsc_trailing_payload.avif"
SOURCE_SHA256 = "117a8b2d61e01d9e9a5d911313524509c4b99d527b108e552a0b6e27d2a8500c"
OUTPUT_SHA256 = "afd53fee7738e9b4fe280bb0ff1a19f938cdaad326977abf2cb4266c8e376e4b"
STSC_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stsc")
STCO_PATH = STSC_PATH[:-1] + (b"stco",)
ILOC_PATH = (b"meta", b"iloc")
SOURCE_STSC = bytes.fromhex(
    "0000000000000002000000010000000200000001000000020000000300000001"
)
SOURCE_STCO = bytes.fromhex("00000000000000020000040f000004a7")
SOURCE_ILOC = bytes.fromhex("00000000440000010001000000010000040f00000027")


def candidate_bytes() -> bytes:
    """Append one ignored stsc byte and rebase file-relative sample offsets."""
    source = SOURCE.read_bytes()
    source_sha256 = hashlib.sha256(source).hexdigest()
    if source_sha256 != SOURCE_SHA256:
        raise RuntimeError(f"two-chunk source hash changed: {source_sha256}")

    changed = {"stsc": 0, "stco": 0, "iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == STSC_PATH:
            changed["stsc"] += 1
            if payload != SOURCE_STSC:
                raise RuntimeError("pinned stsc mapping changed during rewrite")
            return payload + b"\0"
        if path == STCO_PATH:
            changed["stco"] += 1
            if payload != SOURCE_STCO:
                raise RuntimeError("pinned stco mapping changed during rewrite")
            result = bytearray(payload)
            count = struct.unpack_from(">I", result, 4)[0]
            for index in range(count):
                offset = 8 + index * 4
                struct.pack_into(">I", result, offset, struct.unpack_from(">I", result, offset)[0] + 1)
            return bytes(result)
        if path == ILOC_PATH:
            changed["iloc"] += 1
            if payload != SOURCE_ILOC:
                raise RuntimeError("pinned iloc extent changed during rewrite")
            extent_offset = struct.unpack_from(">I", payload, 14)[0]
            return payload[:14] + struct.pack(">I", extent_offset + 1) + payload[18:]
        return payload

    candidate = rewrite_boxes(source, transform)
    if changed != {"stsc": 1, "stco": 1, "iloc": 1}:
        raise RuntimeError(f"unexpected sample-offset rewrite counts: {changed}")
    if len(candidate) != len(source) + 1:
        raise RuntimeError("one-byte stsc growth did not produce a one-byte file growth")

    candidate_top = read_boxes(candidate, 0, len(candidate))
    source_top = read_boxes(source, 0, len(source))
    source_mdat = [payload for kind, _, payload in source_top if kind == b"mdat"]
    candidate_mdat = [payload for kind, _, payload in candidate_top if kind == b"mdat"]
    if len(source_mdat) != 1 or candidate_mdat != source_mdat:
        raise RuntimeError("stsc trailing-byte mutation changed encoded samples")

    candidate_sha256 = hashlib.sha256(candidate).hexdigest()
    if candidate_sha256 != OUTPUT_SHA256:
        raise RuntimeError(f"generated fixture hash changed: {candidate_sha256}")
    return candidate


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    candidate = candidate_bytes()

    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_bytes() != candidate:
            raise RuntimeError(f"{OUTPUT.relative_to(ROOT)} differs from its recipe")
    else:
        OUTPUT.write_bytes(candidate)


if __name__ == "__main__":
    main()
