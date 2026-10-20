#!/usr/bin/env python3
"""Promote the alpha.avif auxl relationship to the version-1 iref encoding.

The fixture widens the ``iref`` item identifiers from 16 to 32 bits while
preserving the same direct alpha relationship and AV1 payloads. The larger
metadata box shifts both file-backed ``iloc`` extents by four bytes.
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/avif"
SOURCE = ASSETS / "alpha.avif"
SOURCE_SHA256 = "b19f57d9421bbd3d0b0706c8fe79cef802aebf106afefa6bddde9de1a07509c9"
FIXTURE = ASSETS / "iref_version_1_alpha_auxl.avif"
FIXTURE_SHA256 = "b6f3b78c98b33b32b679d07a91b3f3030aba7c49def8bc8bad8b64243eb2c710"
SOURCE_IREF = bytes.fromhex("000000000000000e6175786c000200010001")
SOURCE_ILOC = bytes.fromhex(
    "0000000044000002000100000001000002d700001652000200000001000001c90000010e"
)
IREF_DELTA = 4


def box_payload(
    boxes: list[tuple[bytes, bytes, bytes]], kind: bytes, context: str
) -> bytes:
    matches = [payload for box_kind, _, payload in boxes if box_kind == kind]
    if len(matches) != 1:
        raise RuntimeError(f"expected exactly one {kind!r} box in {context}")
    return matches[0]


def bump_file_extents(payload: bytes) -> tuple[bytes, list[tuple[int, int]]]:
    """Relocate every pinned file-backed item extent after metadata growth."""
    if payload != SOURCE_ILOC:
        raise RuntimeError("alpha item locations differ from their pinned layout")
    if payload[0] != 0 or payload[4:6] != b"\x44\x00":
        raise RuntimeError("alpha item locations no longer use the pinned version-0 layout")

    result = bytearray(payload)
    position = 8
    item_count = int.from_bytes(payload[6:8], "big")
    extents: list[tuple[int, int]] = []
    for _ in range(item_count):
        item_id = int.from_bytes(payload[position : position + 2], "big")
        position += 2
        data_reference_index = int.from_bytes(payload[position : position + 2], "big")
        position += 2
        extent_count = int.from_bytes(payload[position : position + 2], "big")
        position += 2
        if data_reference_index != 0 or extent_count != 1:
            raise RuntimeError("alpha item locations are not the pinned file-backed extents")
        source_offset = int.from_bytes(payload[position : position + 4], "big")
        position += 4
        extent_length = int.from_bytes(payload[position : position + 4], "big")
        position += 4
        result[position - 8 : position - 4] = (source_offset + IREF_DELTA).to_bytes(4, "big")
        extents.append((source_offset, extent_length))
        if item_id not in (1, 2):
            raise RuntimeError(f"unexpected alpha fixture item ID {item_id}")

    if position != len(payload) or len(extents) != 2:
        raise RuntimeError("alpha item-location records have unexpected trailing data")
    return bytes(result), extents


def candidate_bytes() -> bytes:
    source = SOURCE.read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("AVIF alpha source differs from its pin")

    source_top = read_boxes(source, 0, len(source))
    source_meta = box_payload(source_top, b"meta", "alpha source")
    if source_meta[:4] != bytes(4):
        raise RuntimeError("alpha meta FullBox differs from its pinned version-0 layout")
    source_meta_children = read_boxes(source_meta, 4, len(source_meta))
    if box_payload(source_meta_children, b"iref", "alpha source meta") != SOURCE_IREF:
        raise RuntimeError("alpha auxiliary reference differs from its pinned layout")
    relocated_iloc, original_extents = bump_file_extents(
        box_payload(source_meta_children, b"iloc", "alpha source meta")
    )

    changed = {"iref": 0, "iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == (b"meta", b"iref"):
            if payload != SOURCE_IREF:
                raise RuntimeError("alpha auxiliary reference differs from its pin")
            changed["iref"] += 1
            version_one_auxl = struct.pack(">IHI", 2, 1, 1)
            return b"\x01\x00\x00\x00" + pack_box(b"auxl", version_one_auxl)
        if path == (b"meta", b"iloc"):
            changed["iloc"] += 1
            return bump_file_extents(payload)[0]
        return payload

    result = rewrite_boxes(source, transform)
    if changed != {"iref": 1, "iloc": 1}:
        raise RuntimeError("expected to update one iref and one iloc box")
    if len(result) != len(source) + IREF_DELTA:
        raise RuntimeError("AVIF metadata mutation changed an unexpected byte count")
    if hashlib.sha256(result).hexdigest() != FIXTURE_SHA256:
        raise RuntimeError("mutated AVIF differs from its pinned output hash")

    result_top = read_boxes(result, 0, len(result))
    result_meta = box_payload(result_top, b"meta", "mutated file")
    result_meta_children = read_boxes(result_meta, 4, len(result_meta))
    result_iref = box_payload(result_meta_children, b"iref", "mutated meta")
    if result_iref[:4] != b"\x01\x00\x00\x00":
        raise RuntimeError("mutated iref does not use version 1 with zero flags")
    references = read_boxes(result_iref, 4, len(result_iref))
    if len(references) != 1 or references[0][0] != b"auxl":
        raise RuntimeError("mutated iref must contain exactly one auxl reference")
    if references[0][2] != struct.pack(">IHI", 2, 1, 1):
        raise RuntimeError("mutated auxl reference does not preserve the 2-to-1 relationship")

    source_mdat = box_payload(source_top, b"mdat", "alpha source")
    result_mdat = box_payload(result_top, b"mdat", "mutated file")
    if result_mdat != source_mdat:
        raise RuntimeError("AVIF mutation changed encoded AV1 samples")

    result_iloc = box_payload(result_meta_children, b"iloc", "mutated meta")
    if result_iloc != relocated_iloc:
        raise RuntimeError("mutated item locations do not match the relocated source extents")
    relocated_extents = [
        (offset + IREF_DELTA, length) for offset, length in original_extents
    ]
    if len(original_extents) != len(relocated_extents):
        raise RuntimeError("mutated item extent count differs from the source")
    for (source_offset, extent_length), (result_offset, _) in zip(
        original_extents, relocated_extents
    ):
        if source[source_offset : source_offset + extent_length] != result[
            result_offset : result_offset + extent_length
        ]:
            raise RuntimeError("relocated item extent no longer selects the original payload")

    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify fixture bytes without writing files",
    )
    args = parser.parse_args()

    expected = candidate_bytes()
    if args.check:
        if not FIXTURE.is_file() or FIXTURE.read_bytes() != expected:
            raise RuntimeError(f"{FIXTURE.name} differs from the maintained recipe")
    elif FIXTURE.exists():
        if FIXTURE.read_bytes() != expected:
            raise RuntimeError(
                f"{FIXTURE.name} already exists with different bytes; refusing to overwrite"
            )
    else:
        FIXTURE.write_bytes(expected)
    print(
        f"{FIXTURE.name}: {len(expected)} bytes, "
        f"sha256={hashlib.sha256(expected).hexdigest()}"
    )


if __name__ == "__main__":
    main()
