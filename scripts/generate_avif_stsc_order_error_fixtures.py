#!/usr/bin/env python3
"""Generate AVIF parity fixtures for invalid sample-to-chunk ordering."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct

from avif_bmff_boxes import read_boxes, rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "animated_stsc_two_chunk_mapping.avif"
SOURCE_SHA256 = "117a8b2d61e01d9e9a5d911313524509c4b99d527b108e552a0b6e27d2a8500c"
STSC_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stsc")
SOURCE_STSC = bytes.fromhex(
    "0000000000000002000000010000000200000001000000020000000300000001"
)
MUTATIONS = (
    (
        "animated_stsc_first_chunk_not_one.avif",
        8,
        2,
        "9b54a6a24989c3ca9feaffb1779ced9407a9cad3f864415d0742e9def89eb46a",
    ),
    (
        "animated_stsc_non_increasing_chunk.avif",
        20,
        1,
        "a6032faac42c6646befcb10470541495702600e98d70fc56dcde3c7fde787f73",
    ),
)


def box_payload_at_path(data: bytes, path: tuple[bytes, ...]) -> bytes:
    """Return one nested box payload, validating each parent lookup."""
    for index, kind in enumerate(path):
        matches = [payload for child_kind, _, payload in read_boxes(data, 0, len(data)) if child_kind == kind]
        if len(matches) != 1:
            raise RuntimeError(f"expected one {kind!r} box at depth {index}, found {len(matches)}")
        data = matches[0]
        if kind == b"meta" and index + 1 < len(path):
            if data[:4] != bytes(4):
                raise RuntimeError("pinned meta must be a version-zero FullBox")
            data = data[4:]
    return data


def candidate_bytes(offset: int, value: int, expected_sha256: str) -> bytes:
    """Mutate one pinned first-chunk field while preserving every sample byte."""
    source = SOURCE.read_bytes()
    source_sha256 = hashlib.sha256(source).hexdigest()
    if source_sha256 != SOURCE_SHA256:
        raise RuntimeError(f"two-chunk source hash changed: {source_sha256}")
    if box_payload_at_path(source, STSC_PATH) != SOURCE_STSC:
        raise RuntimeError("pinned two-chunk stsc mapping changed")

    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path != STSC_PATH:
            return payload
        changed += 1
        if payload != SOURCE_STSC:
            raise RuntimeError("pinned stsc mapping changed during rewrite")
        result = bytearray(payload)
        struct.pack_into(">I", result, offset, value)
        return bytes(result)

    candidate = rewrite_boxes(source, transform)
    if changed != 1:
        raise RuntimeError(f"expected to mutate one stsc box, found {changed}")
    if len(candidate) != len(source):
        raise RuntimeError("same-size stsc mutation changed the file length")

    expected_stsc = bytearray(SOURCE_STSC)
    struct.pack_into(">I", expected_stsc, offset, value)
    if box_payload_at_path(candidate, STSC_PATH) != expected_stsc:
        raise RuntimeError("generated stsc mapping differs from the requested mutation")

    source_mdat = [payload for kind, _, payload in read_boxes(source, 0, len(source)) if kind == b"mdat"]
    candidate_mdat = [payload for kind, _, payload in read_boxes(candidate, 0, len(candidate)) if kind == b"mdat"]
    if len(source_mdat) != 1 or candidate_mdat != source_mdat:
        raise RuntimeError("stsc mutation changed encoded media samples")

    candidate_sha256 = hashlib.sha256(candidate).hexdigest()
    if candidate_sha256 != expected_sha256:
        raise RuntimeError(f"generated fixture hash changed: {candidate_sha256}")
    return candidate


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    for filename, offset, value, expected_sha256 in MUTATIONS:
        output = ASSETS / filename
        candidate = candidate_bytes(offset, value, expected_sha256)
        if args.check:
            if not output.exists() or output.read_bytes() != candidate:
                raise RuntimeError(f"{output.relative_to(ROOT)} differs from its deterministic recipe")
        else:
            output.write_bytes(candidate)


if __name__ == "__main__":
    main()
