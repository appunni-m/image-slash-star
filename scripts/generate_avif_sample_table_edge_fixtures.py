#!/usr/bin/env python3
"""Generate AVIF parity fixtures for sample-table edge conditions."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "animated_stsc_two_chunk_mapping.avif"
SOURCE_SHA256 = "117a8b2d61e01d9e9a5d911313524509c4b99d527b108e552a0b6e27d2a8500c"
STSC_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stsc")
STCO_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stco")
STBL_PATH = STSC_PATH[:-1]
ILOC_PATH = (b"meta", b"iloc")
SOURCE_ILOC = bytes.fromhex("00000000440000010001000000010000040f00000027")
SOURCE_STSC = bytes.fromhex(
    "0000000000000002000000010000000200000001000000020000000300000001"
)
SOURCE_STCO = struct.pack(">II2I", 0, 2, 0x40F, 0x4A7)
MUTATIONS = (
    (
        "animated_stsc_nonzero_version.avif",
        "stsc_version_one",
        "d8778ff338b70039d77390547f95423a70e44e31eb53f70d6d56202911c56296",
    ),
    (
        "animated_stsc_empty_table.avif",
        "stsc_empty",
        "000ebcbdf17df55f3c1c2da392869f982250cd7d77ec149b3f9219dc25803180",
    ),
    (
        "animated_stsc_zero_samples_per_chunk.avif",
        "stsc_zero_samples_per_chunk",
        "89df914cacdf68e8ec3c1053fc8d0503d79a361d405312957db2f3265d1e2213",
    ),
    (
        "animated_stco_empty_offsets.avif",
        "stco_empty",
        "6ddb173daf11f397e82abb5fd46760ab8f24a46a66ecaf66877fdf93340fde2e",
    ),
    (
        "animated_stco_nonzero_version.avif",
        "stco_version_one",
        "e0a2ce3fbe6306871a2b1db1165010adea9d658cf049e987d3f231db093351d1",
    ),
    (
        "animated_stco_trailing_payload.avif",
        "stco_trailing_payload",
        "e773e124d5c3954d8fd0ca5b7fde7f6ad4d50b198dddc20b86b45a1cc70e2d15",
    ),
    (
        "animated_stsc_zero_description_index_first.avif",
        "stsc_description_index_zero",
        "c6f3ff4272a7ade6f51cd77a3c643fb7301e416e4e5ebbd75e4fca4397c5db64",
    ),
)


def box_payload_at_path(data: bytes, path: tuple[bytes, ...]) -> bytes:
    """Return one nested box payload, validating each parent lookup."""
    for index, kind in enumerate(path):
        matches = [
            payload
            for child_kind, _, payload in read_boxes(data, 0, len(data))
            if child_kind == kind
        ]
        if len(matches) != 1:
            raise RuntimeError(f"expected one {kind!r} box at depth {index}, found {len(matches)}")
        data = matches[0]
    return data


def candidate_bytes(mutation: str, expected_sha256: str) -> bytes:
    """Apply one pinned table mutation while preserving the AV1 samples."""
    source = SOURCE.read_bytes()
    source_sha256 = hashlib.sha256(source).hexdigest()
    if source_sha256 != SOURCE_SHA256:
        raise RuntimeError(f"two-chunk source hash changed: {source_sha256}")
    if box_payload_at_path(source, STSC_PATH) != SOURCE_STSC:
        raise RuntimeError("pinned two-chunk stsc mapping changed")
    if box_payload_at_path(source, STCO_PATH) != SOURCE_STCO:
        raise RuntimeError("pinned two-chunk stco mapping changed")

    stco_mutations = {"stco_empty", "stco_version_one", "stco_trailing_payload"}
    changed = {"stsc": 0, "stco": 0, "stbl": 0, "iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == STSC_PATH and mutation not in stco_mutations:
            changed["stsc"] += 1
            if payload != SOURCE_STSC:
                raise RuntimeError("pinned stsc mapping changed during rewrite")
            if mutation == "stsc_version_one":
                result = bytearray(payload)
                result[0] = 1
                return bytes(result)
            if mutation == "stsc_empty":
                return payload[:4] + bytes(4)
            if mutation in {"stsc_zero_samples_per_chunk", "stsc_description_index_zero"}:
                result = bytearray(payload)
                field_offset = 12 if mutation == "stsc_zero_samples_per_chunk" else 16
                struct.pack_into(">I", result, field_offset, 0)
                return bytes(result)
        if path == STCO_PATH and mutation in stco_mutations:
            changed["stco"] += 1
            if payload != SOURCE_STCO:
                raise RuntimeError("pinned stco mapping changed during rewrite")
            if mutation == "stco_version_one":
                result = bytearray(payload)
                result[0] = 1
                return bytes(result)
            if mutation == "stco_trailing_payload":
                result = bytearray(payload)
                count = struct.unpack_from(">I", result, 4)[0]
                for index in range(count):
                    field_offset = 8 + index * 4
                    value = struct.unpack_from(">I", result, field_offset)[0]
                    struct.pack_into(">I", result, field_offset, value + 1)
                return bytes(result) + b"\0"
            return payload[:4] + bytes(4)
        if path == ILOC_PATH and mutation == "stco_trailing_payload":
            changed["iloc"] += 1
            if payload != SOURCE_ILOC:
                raise RuntimeError("pinned iloc extent changed during rewrite")
            extent_offset = struct.unpack_from(">I", payload, 14)[0]
            return payload[:14] + struct.pack(">I", extent_offset + 1) + payload[18:]
        if path == STBL_PATH and mutation in {"stsc_empty", "stco_empty"}:
            changed["stbl"] += 1
            growth = 24 if mutation == "stsc_empty" else 8
            return payload + pack_box(b"free", bytes(growth - 8))
        return payload

    candidate = rewrite_boxes(source, transform)
    expected_changes = {
        "stsc": int(mutation not in stco_mutations),
        "stco": int(mutation in stco_mutations),
        "stbl": int(mutation in {"stsc_empty", "stco_empty"}),
        "iloc": int(mutation == "stco_trailing_payload"),
    }
    if changed != expected_changes:
        raise RuntimeError(f"unexpected box mutation counts: {changed}")
    expected_growth = int(mutation == "stco_trailing_payload")
    if len(candidate) != len(source) + expected_growth:
        raise RuntimeError("sample-table mutation changed the file length")

    if mutation == "stsc_version_one":
        expected_stsc = bytearray(SOURCE_STSC)
        expected_stsc[0] = 1
    elif mutation == "stsc_empty":
        expected_stsc = SOURCE_STSC[:4] + bytes(4)
    elif mutation == "stsc_zero_samples_per_chunk":
        expected_stsc = bytearray(SOURCE_STSC)
        struct.pack_into(">I", expected_stsc, 12, 0)
    elif mutation == "stsc_description_index_zero":
        expected_stsc = bytearray(SOURCE_STSC)
        struct.pack_into(">I", expected_stsc, 16, 0)
    else:
        expected_stsc = SOURCE_STSC
    if box_payload_at_path(candidate, STSC_PATH) != expected_stsc:
        raise RuntimeError("generated stsc payload differs from the requested mutation")

    if mutation == "stco_empty":
        expected_stco = SOURCE_STCO[:4] + bytes(4)
    elif mutation == "stco_version_one":
        expected_stco = bytearray(SOURCE_STCO)
        expected_stco[0] = 1
    elif mutation == "stco_trailing_payload":
        expected_stco = bytearray(SOURCE_STCO)
        count = struct.unpack_from(">I", expected_stco, 4)[0]
        for index in range(count):
            field_offset = 8 + index * 4
            value = struct.unpack_from(">I", expected_stco, field_offset)[0]
            struct.pack_into(">I", expected_stco, field_offset, value + 1)
        expected_stco.extend(b"\0")
    else:
        expected_stco = SOURCE_STCO
    if box_payload_at_path(candidate, STCO_PATH) != expected_stco:
        raise RuntimeError("generated stco payload differs from the requested mutation")

    source_top = read_boxes(source, 0, len(source))
    candidate_top = read_boxes(candidate, 0, len(candidate))
    source_mdat = [payload for kind, _, payload in source_top if kind == b"mdat"]
    candidate_mdat = [payload for kind, _, payload in candidate_top if kind == b"mdat"]
    if len(source_mdat) != 1 or candidate_mdat != source_mdat:
        raise RuntimeError("sample-table mutation changed encoded media samples")

    candidate_sha256 = hashlib.sha256(candidate).hexdigest()
    if candidate_sha256 != expected_sha256:
        raise RuntimeError(f"generated fixture hash changed: {candidate_sha256}")
    return candidate


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    for filename, mutation, expected_sha256 in MUTATIONS:
        output = ASSETS / filename
        candidate = candidate_bytes(mutation, expected_sha256)
        if args.check:
            if not output.exists() or output.read_bytes() != candidate:
                raise RuntimeError(f"{output.relative_to(ROOT)} differs from its recipe")
        else:
            output.write_bytes(candidate)


if __name__ == "__main__":
    main()
