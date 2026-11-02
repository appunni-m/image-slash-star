#!/usr/bin/env python3
"""Add duplicate primary-item transforms with Pillow's first-property behavior.

The fixture adds conflicting duplicate ``irot``, ``imir``, ``pasp``, and
``clap`` properties to the pinned baseline AVIF. It preserves the encoded
samples and rebases the file-backed primary-item extent after metadata growth.
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/avif"
SOURCE = ASSETS / "baseline.avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
OUTPUT = ASSETS / "primary_item_duplicate_transforms_first_wins.avif"
OUTPUT_SHA256 = "c6b9cfc406f9fd210aa2817d6311791ce27929ef0b0173c06e9b2945b82eeae7"
SOURCE_IPMA = bytes.fromhex("000000000000000100010401028304")
SOURCE_ILOC = bytes.fromhex("00000000440000010001000000010000011a00000aeb")


def box_payload(boxes: list[tuple[bytes, bytes, bytes]], kind: bytes, context: str) -> bytes:
    matches = [payload for box_kind, _, payload in boxes if box_kind == kind]
    if len(matches) != 1:
        raise RuntimeError(f"expected exactly one {kind!r} box in {context}")
    return matches[0]


def duplicate_properties() -> tuple[bytes, bytes]:
    """Return conflicting transform properties and their essential associations."""
    properties = (
        pack_box(b"irot", b"\x03"),
        pack_box(b"irot", b"\x01"),
        pack_box(b"imir", b"\x00"),
        pack_box(b"imir", b"\x01"),
        pack_box(b"pasp", struct.pack(">II", 4, 3)),
        pack_box(b"pasp", struct.pack(">II", 3, 4)),
        pack_box(b"clap", struct.pack(">IIIIiIiI", 128, 1, 128, 1, 0, 1, 0, 1)),
        pack_box(b"clap", struct.pack(">IIIIiIiI", 64, 1, 128, 1, 0, 1, 0, 1)),
    )
    associations = bytes(0x80 | property_index for property_index in range(5, 13))
    return b"".join(properties), associations


def candidate_bytes() -> bytes:
    source = SOURCE.read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("AVIF baseline source differs from its pin")

    source_top = read_boxes(source, 0, len(source))
    source_meta = box_payload(source_top, b"meta", "baseline source")
    if source_meta[:4] != bytes(4):
        raise RuntimeError("baseline meta FullBox differs from its pinned layout")
    source_meta_children = read_boxes(source_meta, 4, len(source_meta))
    source_iprp = box_payload(source_meta_children, b"iprp", "baseline meta")
    source_iprp_children = read_boxes(source_iprp, 0, len(source_iprp))
    source_ipco = box_payload(source_iprp_children, b"ipco", "baseline iprp")
    source_properties = read_boxes(source_ipco, 0, len(source_ipco))
    expected_properties = [b"ispe", b"pixi", b"av1C", b"colr"]
    if [kind for kind, _, _ in source_properties] != expected_properties:
        raise RuntimeError("baseline primary-item properties differ from their pinned order")

    source_ipma = box_payload(source_iprp_children, b"ipma", "baseline iprp")
    if source_ipma != SOURCE_IPMA:
        raise RuntimeError("baseline primary-item associations differ from their pin")
    source_iloc = box_payload(source_meta_children, b"iloc", "baseline meta")
    if source_iloc != SOURCE_ILOC:
        raise RuntimeError("baseline primary-item extent differs from its pin")

    properties, associations = duplicate_properties()
    delta = len(properties) + len(associations)
    changed = {b"ipco": 0, b"ipma": 0, b"iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == (b"meta", b"iprp", b"ipco"):
            changed[b"ipco"] += 1
            return payload + properties
        if path == (b"meta", b"iprp", b"ipma"):
            if payload != SOURCE_IPMA:
                raise RuntimeError("baseline primary-item associations differ from their pin")
            changed[b"ipma"] += 1
            return (
                payload[:10]
                + bytes((payload[10] + len(associations),))
                + payload[11:]
                + associations
            )
        if path == (b"meta", b"iloc"):
            if payload != SOURCE_ILOC:
                raise RuntimeError("baseline primary-item extent differs from its pin")
            changed[b"iloc"] += 1
            result = bytearray(payload)
            offset = struct.unpack_from(">I", payload, 14)[0]
            struct.pack_into(">I", result, 14, offset + delta)
            return bytes(result)
        return payload

    result = rewrite_boxes(source, transform)
    if changed != dict.fromkeys(changed, 1):
        raise RuntimeError("expected to update one ipco, ipma, and iloc box")
    if len(result) != len(source) + delta:
        raise RuntimeError("AVIF metadata mutation changed an unexpected byte count")

    result_top = read_boxes(result, 0, len(result))
    result_meta = box_payload(result_top, b"meta", "mutated file")
    result_meta_children = read_boxes(result_meta, 4, len(result_meta))
    result_iprp = box_payload(result_meta_children, b"iprp", "mutated meta")
    result_iprp_children = read_boxes(result_iprp, 0, len(result_iprp))
    result_ipco = box_payload(result_iprp_children, b"ipco", "mutated iprp")
    result_properties = read_boxes(result_ipco, 0, len(result_ipco))
    expected_kinds = expected_properties + [
        b"irot", b"irot", b"imir", b"imir", b"pasp", b"pasp", b"clap", b"clap"
    ]
    if [kind for kind, _, _ in result_properties] != expected_kinds:
        raise RuntimeError("mutated property table differs from its maintained recipe")

    result_ipma = box_payload(result_iprp_children, b"ipma", "mutated iprp")
    expected_ipma = (
        SOURCE_IPMA[:10]
        + bytes((SOURCE_IPMA[10] + len(associations),))
        + SOURCE_IPMA[11:]
        + associations
    )
    if result_ipma != expected_ipma:
        raise RuntimeError("mutated primary item does not associate all eight transforms")

    source_mdat = box_payload(source_top, b"mdat", "baseline source")
    result_mdat = box_payload(result_top, b"mdat", "mutated file")
    if result_mdat != source_mdat:
        raise RuntimeError("AVIF mutation changed encoded AV1 samples")

    result_iloc = box_payload(result_meta_children, b"iloc", "mutated meta")
    expected_iloc = bytearray(SOURCE_ILOC)
    original_offset = struct.unpack_from(">I", SOURCE_ILOC, 14)[0]
    struct.pack_into(">I", expected_iloc, 14, original_offset + delta)
    if result_iloc != expected_iloc:
        raise RuntimeError("mutated primary-item extent differs from its rebased pin")
    extent_offset = struct.unpack_from(">I", result_iloc, 14)[0]
    extent_length = struct.unpack_from(">I", result_iloc, 18)[0]
    if result[extent_offset : extent_offset + extent_length] != result_mdat:
        raise RuntimeError("primary-item extent no longer selects the unchanged mdat payload")

    if hashlib.sha256(result).hexdigest() != OUTPUT_SHA256:
        raise RuntimeError("mutated AVIF differs from its pinned output hash")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify fixture bytes without writing")
    args = parser.parse_args()

    expected = candidate_bytes()
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != expected:
            raise RuntimeError(f"{OUTPUT.name} differs from the maintained recipe")
    elif OUTPUT.exists():
        if OUTPUT.read_bytes() != expected:
            raise RuntimeError(f"{OUTPUT.name} exists with different bytes; refusing to overwrite")
    else:
        OUTPUT.write_bytes(expected)
    print(f"{OUTPUT}: {len(expected)} bytes, sha256={hashlib.sha256(expected).hexdigest()}")


if __name__ == "__main__":
    main()
