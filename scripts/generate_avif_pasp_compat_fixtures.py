#!/usr/bin/env python3
"""Generate AVIF parity fixtures for Pillow's tolerant `pasp` handling."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "primary_item_irot_pasp_4x3.avif"
SOURCE_SHA256 = "2acc2e59572718a5fd1247aef7d18ab7666943533f068c221117df05378c4564"
SOURCE_ILOC = bytes.fromhex("00000000440000010001000000010000012e00000028")
SOURCE_PASP = struct.pack(">II", 4, 3)
SOURCE_MDAT_SHA256 = "00a0f82fcaac5a3ab8fe05d3abf467a3b11ae0e427ec64872bafa3be1066fc6d"

FIXTURES = (
    (
        "primary_item_pasp_zero_horizontal_spacing.avif",
        "zero_horizontal_spacing",
        0,
        3,
        False,
        "1d8ad709f18d1bed97ca2e44178a8b9580aeb79763ea9543d4bc3ef73d62d799",
    ),
    (
        "primary_item_pasp_zero_vertical_spacing.avif",
        "zero_vertical_spacing",
        4,
        0,
        False,
        "3f8eef88f63387e8de633f5783d56e3ca8ae6d0ccc5a397969377c2bda3f220a",
    ),
    (
        "primary_item_pasp_trailing_byte.avif",
        "trailing_byte",
        4,
        3,
        True,
        "e62a08cb8bcb753fcae35739316bfc5ee4c0943879f4d9f6d0ecf087dee2f5c7",
    ),
)


def candidate_bytes(mutation: str, h_spacing: int, v_spacing: int, trailing: bool) -> bytes:
    """Apply one bounded `pasp` mutation to the pinned primary-item fixture."""
    source = SOURCE.read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned AVIF `pasp` source hash changed")

    source_top = read_boxes(source, 0, len(source))
    source_mdat = [payload for kind, _, payload in source_top if kind == b"mdat"]
    if len(source_mdat) != 1 or hashlib.sha256(source_mdat[0]).hexdigest() != SOURCE_MDAT_SHA256:
        raise RuntimeError("pinned AVIF primary-item samples changed")

    changed = {"ipco": 0, "pasp": 0, "iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == (b"meta", b"iprp", b"ipco"):
            changed["ipco"] += 1
            properties = []
            for kind, _, child_payload in read_boxes(payload, 0, len(payload)):
                if kind == b"pasp":
                    changed["pasp"] += 1
                    if child_payload != SOURCE_PASP:
                        raise RuntimeError("source `pasp` payload differs from 4:3 pin")
                    child_payload = struct.pack(">II", h_spacing, v_spacing)
                    if trailing:
                        child_payload += b"\xff"
                properties.append(pack_box(kind, child_payload))
            return b"".join(properties)

        if trailing and path == (b"meta", b"iloc"):
            changed["iloc"] += 1
            if payload != SOURCE_ILOC:
                raise RuntimeError("source `iloc` differs from the pinned extent layout")
            result = bytearray(payload)
            extent_offset = struct.unpack_from(">I", result, 14)[0]
            struct.pack_into(">I", result, 14, extent_offset + 1)
            return bytes(result)

        return payload

    candidate = rewrite_boxes(source, transform)
    expected_changes = {"ipco": 1, "pasp": 1, "iloc": int(trailing)}
    if changed != expected_changes:
        raise RuntimeError(f"unexpected `pasp` rewrite counts for {mutation}: {changed}")
    if len(candidate) != len(source) + int(trailing):
        raise RuntimeError(f"`pasp` mutation changed the wrong number of bytes: {mutation}")

    candidate_top = read_boxes(candidate, 0, len(candidate))
    candidate_mdat = [payload for kind, _, payload in candidate_top if kind == b"mdat"]
    if candidate_mdat != source_mdat:
        raise RuntimeError("`pasp` mutation changed encoded media samples")
    candidate_hash = hashlib.sha256(candidate).hexdigest()
    expected_hash = next(item[5] for item in FIXTURES if item[1] == mutation)
    if candidate_hash != expected_hash:
        raise RuntimeError(f"generated fixture hash changed for {mutation}: {candidate_hash}")
    return candidate


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="verify fixture bytes without writing")
    args = parser.parse_args()

    for filename, mutation, h_spacing, v_spacing, trailing, expected_hash in FIXTURES:
        output = ASSETS / filename
        candidate = candidate_bytes(mutation, h_spacing, v_spacing, trailing)
        if hashlib.sha256(candidate).hexdigest() != expected_hash:
            raise RuntimeError(f"{filename} differs from its pinned hash")
        if args.check:
            if not output.is_file() or output.read_bytes() != candidate:
                raise RuntimeError(f"{output.relative_to(ROOT)} differs from its maintained recipe")
        elif output.exists():
            if output.read_bytes() != candidate:
                raise RuntimeError(f"{output} exists with different bytes; refusing to overwrite")
        else:
            output.write_bytes(candidate)
        print(f"{output}: {len(candidate)} bytes, sha256={expected_hash}")


if __name__ == "__main__":
    main()
