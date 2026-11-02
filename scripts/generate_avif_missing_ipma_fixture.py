#!/usr/bin/env python3
"""Generate a malformed AVIF without an item-property association box."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "baseline.avif"
OUTPUT = ASSETS / "error_iprp_missing_ipma.avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
OUTPUT_SHA256 = "b73571771c5418bf14f1a70989e0af0ed98aa59ee834b6b04c8dece86ff77392"
IPRP_PATH = (b"meta", b"iprp")
META_PATH = (b"meta",)


def unique_payload(boxes: list[tuple[bytes, bytes, bytes]], kind: bytes) -> bytes:
    """Return one child payload and reject ambiguous parent layouts."""
    matches = [payload for child_kind, _, payload in boxes if child_kind == kind]
    if len(matches) != 1:
        raise RuntimeError(f"expected exactly one {kind!r} box, found {len(matches)}")
    return matches[0]


def candidate_bytes() -> bytes:
    """Remove ipma from iprp and preserve meta/file size with an ignored free box."""
    source = SOURCE.read_bytes()
    source_sha256 = hashlib.sha256(source).hexdigest()
    if source_sha256 != SOURCE_SHA256:
        raise RuntimeError(f"baseline AVIF hash changed: {source_sha256}")

    source_top = read_boxes(source, 0, len(source))
    source_meta = unique_payload(source_top, b"meta")
    if source_meta[:4] != bytes(4):
        raise RuntimeError("baseline meta must be a version-zero FullBox")
    source_meta_children = read_boxes(source_meta, 4, len(source_meta))
    source_iprp = unique_payload(source_meta_children, b"iprp")
    source_iprp_children = read_boxes(source_iprp, 0, len(source_iprp))
    if [kind for kind, _, _ in source_iprp_children] != [b"ipco", b"ipma"]:
        raise RuntimeError("baseline iprp child order differs from its pin")

    changed = {"iprp": 0, "meta": 0}
    free_box: bytes | None = None

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal free_box
        if path == IPRP_PATH:
            changed["iprp"] += 1
            children = read_boxes(payload, 0, len(payload))
            if [kind for kind, _, _ in children] != [b"ipco", b"ipma"]:
                raise RuntimeError("baseline iprp child order changed during rewrite")
            free_box = pack_box(b"free", bytes(len(children[1][1]) - 8))
            return children[0][1]
        if path == META_PATH:
            changed["meta"] += 1
            if free_box is None:
                raise RuntimeError("ipma removal did not prepare its size-preserving box")
            return payload + free_box
        return payload

    candidate = rewrite_boxes(source, transform)
    if changed != {"iprp": 1, "meta": 1}:
        raise RuntimeError(f"unexpected metadata mutation counts: {changed}")
    if len(candidate) != len(source):
        raise RuntimeError("ipma removal changed the file length")

    candidate_top = read_boxes(candidate, 0, len(candidate))
    candidate_meta = unique_payload(candidate_top, b"meta")
    candidate_meta_children = read_boxes(candidate_meta, 4, len(candidate_meta))
    candidate_iprp = unique_payload(candidate_meta_children, b"iprp")
    candidate_iprp_children = read_boxes(candidate_iprp, 0, len(candidate_iprp))
    if [kind for kind, _, _ in candidate_iprp_children] != [b"ipco"]:
        raise RuntimeError("generated iprp still contains an association box")
    if candidate_meta_children[-1][0] != b"free":
        raise RuntimeError("size-preserving free box is not the final meta child")

    source_mdat = [payload for kind, _, payload in source_top if kind == b"mdat"]
    candidate_mdat = [payload for kind, _, payload in candidate_top if kind == b"mdat"]
    if len(source_mdat) != 1 or candidate_mdat != source_mdat:
        raise RuntimeError("ipma removal changed encoded AV1 samples")

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
            raise RuntimeError(f"{OUTPUT.relative_to(ROOT)} differs from its deterministic recipe")
    else:
        OUTPUT.write_bytes(candidate)


if __name__ == "__main__":
    main()
