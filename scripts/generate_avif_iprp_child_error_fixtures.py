#!/usr/bin/env python3
"""Generate malformed AVIF fixtures for unknown and duplicate iprp children."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "baseline.avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
IPRP_PATH = (b"meta", b"iprp")
ILOC_PATH = (b"meta", b"iloc")
SOURCE_ILOC = bytes.fromhex("00000000440000010001000000010000011a00000aeb")
CASES = (
    (
        "error_iprp_unknown_child.avif",
        "unknown_child",
        "1d67de2af3d30be3ef552e461fb3a74c3badacd6c6fd84e1e8fb9937fa3d3580",
    ),
    (
        "error_iprp_duplicate_ipma.avif",
        "duplicate_ipma",
        "d6e09f8fcc13fd62817793463188f12b2583ec044a967368af9c977f4cf2b65f",
    ),
)


def unique_payload(boxes: list[tuple[bytes, bytes, bytes]], kind: bytes) -> bytes:
    """Return one child payload and reject ambiguous parent layouts."""
    matches = [payload for child_kind, _, payload in boxes if child_kind == kind]
    if len(matches) != 1:
        raise RuntimeError(f"expected one {kind!r} box, found {len(matches)}")
    return matches[0]


def bump_iloc_extent(payload: bytes, delta: int) -> bytes:
    """Rebase the pinned v0 single-extent primary item after metadata growth."""
    if payload != SOURCE_ILOC:
        raise RuntimeError("pinned iloc extent changed during rewrite")
    extent_offset = struct.unpack_from(">I", payload, 14)[0]
    return payload[:14] + struct.pack(">I", extent_offset + delta) + payload[18:]


def candidate_bytes(mutation: str, expected_sha256: str) -> bytes:
    """Apply one iprp child mutation while preserving image samples."""
    source = SOURCE.read_bytes()
    source_sha256 = hashlib.sha256(source).hexdigest()
    if source_sha256 != SOURCE_SHA256:
        raise RuntimeError(f"baseline AVIF hash changed: {source_sha256}")

    source_top = read_boxes(source, 0, len(source))
    source_meta = unique_payload(source_top, b"meta")
    source_meta_children = read_boxes(source_meta, 4, len(source_meta))
    source_iprp = unique_payload(source_meta_children, b"iprp")
    source_iprp_children = read_boxes(source_iprp, 0, len(source_iprp))
    if [kind for kind, _, _ in source_iprp_children] != [b"ipco", b"ipma"]:
        raise RuntimeError("baseline iprp child order differs from its pin")
    ipma_box = source_iprp_children[1][1]

    changed = {"iprp": 0, "iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == IPRP_PATH:
            changed["iprp"] += 1
            children = read_boxes(payload, 0, len(payload))
            if [kind for kind, _, _ in children] != [b"ipco", b"ipma"]:
                raise RuntimeError("baseline iprp child order changed during rewrite")
            if mutation == "unknown_child":
                return children[0][1] + pack_box(b"free", children[1][2])
            if mutation == "duplicate_ipma":
                return payload + ipma_box
        if path == ILOC_PATH and mutation == "duplicate_ipma":
            changed["iloc"] += 1
            return bump_iloc_extent(payload, len(ipma_box))
        return payload

    candidate = rewrite_boxes(source, transform)
    expected_changes = {"iprp": 1, "iloc": int(mutation == "duplicate_ipma")}
    if changed != expected_changes:
        raise RuntimeError(f"unexpected iprp rewrite counts: {changed}")

    candidate_top = read_boxes(candidate, 0, len(candidate))
    candidate_meta = unique_payload(candidate_top, b"meta")
    candidate_meta_children = read_boxes(candidate_meta, 4, len(candidate_meta))
    candidate_iprp = unique_payload(candidate_meta_children, b"iprp")
    candidate_iprp_children = read_boxes(candidate_iprp, 0, len(candidate_iprp))
    expected_kinds = (
        [b"ipco", b"free"]
        if mutation == "unknown_child"
        else [b"ipco", b"ipma", b"ipma"]
    )
    if [kind for kind, _, _ in candidate_iprp_children] != expected_kinds:
        raise RuntimeError("generated iprp children differ from the requested mutation")

    source_mdat = [payload for kind, _, payload in source_top if kind == b"mdat"]
    candidate_mdat = [payload for kind, _, payload in candidate_top if kind == b"mdat"]
    if len(source_mdat) != 1 or candidate_mdat != source_mdat:
        raise RuntimeError("iprp mutation changed encoded AV1 samples")

    candidate_sha256 = hashlib.sha256(candidate).hexdigest()
    if candidate_sha256 != expected_sha256:
        raise RuntimeError(f"generated fixture hash changed: {candidate_sha256}")
    return candidate


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    for filename, mutation, expected_sha256 in CASES:
        output = ASSETS / filename
        candidate = candidate_bytes(mutation, expected_sha256)
        if args.check:
            if not output.exists() or output.read_bytes() != candidate:
                raise RuntimeError(f"{output.relative_to(ROOT)} differs from its recipe")
        else:
            output.write_bytes(candidate)


if __name__ == "__main__":
    main()
