#!/usr/bin/env python3
"""Generate AVIF parity fixtures for sample-description edge conditions."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct

from avif_bmff_boxes import read_boxes, rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "animated_sequence_only_no_meta.avif"
SOURCE_SHA256 = "c861761e45518018150dc52302da7ff5015d1f259ff2c1309eea94eaf8176fdb"
STSD_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stsd")
SOURCE_STSD = bytes.fromhex(
    "0000000000000001000000856176303100000000000000010000000000000000"
    "00000000000000000096009600480000004800000000000000010a414f4d2043"
    "6f64696e670000000000000000000000000000000000000000000018ffff0000"
    "000c6176314381000c0000000013636f6c726e636c780001000d000680000000"
    "1063637374000000007c000000"
)
SOURCE_STCO = bytes.fromhex("0000000000000001000003ff")
STCO_PATH = STSD_PATH[:-1] + (b"stco",)
FIXTURES = (
    (
        "animated_stsd_trailing_payload.avif",
        "trailing_payload",
        "153ead664b729f1854c738ddd2e771e18c7488457f7716d8516df626eb2543ef",
    ),
    (
        "error_animated_stsd_nonzero_version.avif",
        "nonzero_version",
        "aeace2695e5ce994805cc52b298cbed8f4354f025ff58431da591b83956a790b",
    ),
    (
        "error_animated_stsd_unknown_sample_entry.avif",
        "unknown_sample_entry",
        "48318a79ccba0b03ec54fb991dc9125d6a79a2fba64a34fb710d97c36343c402",
    ),
    (
        "error_animated_stsd_short_av01_entry.avif",
        "short_av01_entry",
        "2fd1bb9882767e930ab717a2e8c970de48643788b11546af73cf91a09582af91",
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
            raise RuntimeError(
                f"expected one {kind!r} box at depth {index}, found {len(matches)}"
            )
        data = matches[0]
    return data


def adjusted_stsd(mutation: str) -> bytes:
    """Return the pinned sample-description mutation."""
    if mutation == "trailing_payload":
        return SOURCE_STSD + b"\0"
    result = bytearray(SOURCE_STSD)
    if mutation == "nonzero_version":
        result[0] = 2
    elif mutation == "unknown_sample_entry":
        result[12:16] = b"free"
    elif mutation == "short_av01_entry":
        struct.pack_into(">I", result, 8, 8)
    else:
        raise RuntimeError(f"unknown stsd mutation: {mutation}")
    return bytes(result)


def adjusted_stco(file_delta: int) -> bytes:
    """Rebase every 32-bit chunk offset after the stsd box grows."""
    result = bytearray(SOURCE_STCO)
    count = struct.unpack_from(">I", result, 4)[0]
    if len(result) != 8 + count * 4:
        raise RuntimeError("pinned stco table length differs from its entry count")
    for index in range(count):
        offset = 8 + index * 4
        current = struct.unpack_from(">I", result, offset)[0]
        adjusted = current + file_delta
        if not 0 <= adjusted <= 0xFFFF_FFFF:
            raise RuntimeError("stco offset cannot be rebased without overflow")
        struct.pack_into(">I", result, offset, adjusted)
    return bytes(result)


def candidate_bytes(mutation: str, expected_sha256: str) -> bytes:
    """Apply one sample-description mutation while preserving media bytes."""
    file_delta = int(mutation == "trailing_payload")
    source = SOURCE.read_bytes()
    source_sha256 = hashlib.sha256(source).hexdigest()
    if source_sha256 != SOURCE_SHA256:
        raise RuntimeError(f"pinned AVIS-only source hash changed: {source_sha256}")
    if box_payload_at_path(source, STSD_PATH) != SOURCE_STSD:
        raise RuntimeError("pinned track sample-description table changed")
    if box_payload_at_path(source, STCO_PATH) != SOURCE_STCO:
        raise RuntimeError("pinned track chunk-offset table changed")

    changed = {"stsd": 0, "stco": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == STSD_PATH:
            changed["stsd"] += 1
            if payload != SOURCE_STSD:
                raise RuntimeError("pinned stsd table changed during rewrite")
            return adjusted_stsd(mutation)
        if path == STCO_PATH:
            changed["stco"] += 1
            if payload != SOURCE_STCO:
                raise RuntimeError("pinned stco table changed during rewrite")
            return adjusted_stco(file_delta)
        return payload

    candidate = rewrite_boxes(source, transform)
    if changed != {"stsd": 1, "stco": 1}:
        raise RuntimeError(f"unexpected sample-description rewrite counts: {changed}")
    if len(candidate) != len(source) + file_delta:
        raise RuntimeError("sample-description mutation produced an unexpected file size")
    if box_payload_at_path(candidate, STSD_PATH) != adjusted_stsd(mutation):
        raise RuntimeError("generated stsd payload differs from the requested mutation")
    if box_payload_at_path(candidate, STCO_PATH) != adjusted_stco(file_delta):
        raise RuntimeError("generated stco offsets were not rebased")

    source_mdat = [
        payload for kind, _, payload in read_boxes(source, 0, len(source)) if kind == b"mdat"
    ]
    candidate_mdat = [
        payload
        for kind, _, payload in read_boxes(candidate, 0, len(candidate))
        if kind == b"mdat"
    ]
    if len(source_mdat) != 1 or candidate_mdat != source_mdat:
        raise RuntimeError("sample-description mutation changed encoded media samples")

    candidate_sha256 = hashlib.sha256(candidate).hexdigest()
    if candidate_sha256 != expected_sha256:
        raise RuntimeError(f"generated fixture hash changed: {candidate_sha256}")
    return candidate


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    for filename, mutation, expected_sha256 in FIXTURES:
        output = ASSETS / filename
        candidate = candidate_bytes(mutation, expected_sha256)
        if args.check:
            if not output.exists() or output.read_bytes() != candidate:
                raise RuntimeError(f"{output.relative_to(ROOT)} differs from its recipe")
        else:
            output.write_bytes(candidate)


if __name__ == "__main__":
    main()
