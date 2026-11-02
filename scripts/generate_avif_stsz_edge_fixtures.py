#!/usr/bin/env python3
"""Generate AVIF parity fixtures for stsz edge conditions."""

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
FIXTURES = (
    (
        "animated_stsz_trailing_payload.avif",
        "trailing_payload",
        "ec5869776d517f36dcbd9d19eb05e8a87dd709b3651a2b2ca00a826cf546f8c3",
    ),
    (
        "error_animated_stsz_empty_table.avif",
        "empty_table",
        "00a12f15bb8a108afd79d4a8c3a38f03e2a00180b513c32ecc3c18a2544deffd",
    ),
    (
        "error_animated_stsz_zero_entry.avif",
        "zero_entry",
        "6bf0b1208f1043014565adbfa368a46ac60462faaea837da36fe240604fd6b72",
    ),
    (
        "error_animated_stsz_nonzero_version.avif",
        "nonzero_version",
        "b4f1467cfb5dd7aa833de3e1fa54009540debaf53d12c01c901ed57bbcb14a22",
    ),
)
TRACK_SOURCE = ASSETS / "animated_sequence_only_no_meta.avif"
TRACK_SOURCE_SHA256 = "c861761e45518018150dc52302da7ff5015d1f259ff2c1309eea94eaf8176fdb"
TRACK_SOURCE_STCO = bytes.fromhex("0000000000000001000003ff")
TRACK_FIXTURE = (
    "error_animated_sequence_only_stsz_common_size_short_samples.avif",
    "fe2666ad9cf9fe32d6b4a35d64f1b9f1a101ae9a65e8055fab6cd62bc63e827c",
)
TRACK_TRUNCATED_TAIL_FIXTURE = (
    "error_animated_sequence_only_stsz_common_size_short_samples_truncated_tail.avif",
    "704aa89ebb2866a5c6b457a28bd2e85317c0b5bb77bf40e151f7e9ab0618bda2",
)
STSZ_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stsz")
STCO_PATH = STSZ_PATH[:-1] + (b"stco",)
ILOC_PATH = (b"meta", b"iloc")
SOURCE_STSZ = bytes.fromhex(
    "0000000000000000000000050000002700000071000000050000001e00000019"
)
SOURCE_STCO = bytes.fromhex("00000000000000020000040f000004a7")
SOURCE_ILOC = bytes.fromhex("00000000440000010001000000010000040f00000027")


def box_payload_at_path(data: bytes, path: tuple[bytes, ...]) -> bytes:
    """Return one nested payload, accounting for the meta FullBox prefix."""
    parent = None
    for index, kind in enumerate(path):
        if parent == b"meta":
            data = data[4:]
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
        parent = kind
    return data


def rebased_offset(current: int, delta: int, label: str) -> int:
    """Return a checked file-relative offset after a file-size change."""
    adjusted = current + delta
    if not 0 <= adjusted <= 0xFFFF_FFFF:
        raise RuntimeError(f"{label} cannot be rebased without overflow")
    return adjusted


def adjust_u32(payload: bytes, offset: int, delta: int, label: str) -> bytes:
    """Adjust a pinned unsigned 32-bit file-relative offset safely."""
    current = struct.unpack_from(">I", payload, offset)[0]
    adjusted = rebased_offset(current, delta, label)
    return payload[:offset] + struct.pack(">I", adjusted) + payload[offset + 4 :]


def adjust_chunk_offsets(payload: bytes, delta: int) -> bytes:
    """Rebase every declared 32-bit chunk offset by the file-size change."""
    result = bytearray(payload)
    count = struct.unpack_from(">I", result, 4)[0]
    if len(result) != 8 + count * 4:
        raise RuntimeError("pinned stco table length differs from its entry count")
    for index in range(count):
        offset = 8 + index * 4
        current = struct.unpack_from(">I", result, offset)[0]
        adjusted = rebased_offset(current, delta, "stco offset")
        struct.pack_into(">I", result, offset, adjusted)
    return bytes(result)


def adjusted_stsz(mutation: str) -> bytes:
    """Return the requested declared-size-table mutation."""
    if mutation == "trailing_payload":
        return SOURCE_STSZ + b"\0"
    if mutation == "empty_table":
        return SOURCE_STSZ[:8] + bytes(4)
    if mutation == "zero_entry":
        result = bytearray(SOURCE_STSZ)
        struct.pack_into(">I", result, 12, 0)
        return bytes(result)
    if mutation == "nonzero_version":
        return bytes((1,)) + SOURCE_STSZ[1:]
    raise RuntimeError(f"unknown stsz mutation: {mutation}")


def candidate_bytes(mutation: str, expected_sha256: str) -> bytes:
    """Apply one stsz mutation and rebase file-relative offsets if needed."""
    file_delta = {
        "trailing_payload": 1,
        "empty_table": -20,
        "zero_entry": 0,
        "nonzero_version": 0,
    }.get(mutation)
    if file_delta is None:
        raise RuntimeError(f"unknown stsz mutation: {mutation}")
    source = SOURCE.read_bytes()
    source_sha256 = hashlib.sha256(source).hexdigest()
    if source_sha256 != SOURCE_SHA256:
        raise RuntimeError(f"two-chunk source hash changed: {source_sha256}")
    if box_payload_at_path(source, STSZ_PATH) != SOURCE_STSZ:
        raise RuntimeError("pinned stsz table changed")
    if box_payload_at_path(source, STCO_PATH) != SOURCE_STCO:
        raise RuntimeError("pinned stco table changed")
    if box_payload_at_path(source, ILOC_PATH) != SOURCE_ILOC:
        raise RuntimeError("pinned iloc extent changed")

    changed = {"stsz": 0, "stco": 0, "iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == STSZ_PATH:
            changed["stsz"] += 1
            if payload != SOURCE_STSZ:
                raise RuntimeError("pinned stsz table changed during rewrite")
            return adjusted_stsz(mutation)
        if path == STCO_PATH:
            changed["stco"] += 1
            if payload != SOURCE_STCO:
                raise RuntimeError("pinned stco table changed during rewrite")
            return adjust_chunk_offsets(payload, file_delta)
        if path == ILOC_PATH:
            changed["iloc"] += 1
            if payload != SOURCE_ILOC:
                raise RuntimeError("pinned iloc extent changed during rewrite")
            return adjust_u32(payload, 14, file_delta, "iloc extent")
        return payload

    candidate = rewrite_boxes(source, transform)
    if changed != {"stsz": 1, "stco": 1, "iloc": 1}:
        raise RuntimeError(f"unexpected box mutation counts: {changed}")
    if len(candidate) != len(source) + file_delta:
        raise RuntimeError("stsz mutation produced an unexpected file-size change")
    expected_stsz = adjusted_stsz(mutation)
    if box_payload_at_path(candidate, STSZ_PATH) != expected_stsz:
        raise RuntimeError("generated stsz payload differs from the requested mutation")
    expected_stco = adjust_chunk_offsets(SOURCE_STCO, file_delta)
    if box_payload_at_path(candidate, STCO_PATH) != expected_stco:
        raise RuntimeError("generated stco offsets were not rebased")
    expected_iloc = adjust_u32(SOURCE_ILOC, 14, file_delta, "iloc extent")
    if box_payload_at_path(candidate, ILOC_PATH) != expected_iloc:
        raise RuntimeError("generated iloc extent was not rebased")

    source_mdat = [
        payload
        for kind, _, payload in read_boxes(source, 0, len(source))
        if kind == b"mdat"
    ]
    candidate_mdat = [
        payload
        for kind, _, payload in read_boxes(candidate, 0, len(candidate))
        if kind == b"mdat"
    ]
    if len(source_mdat) != 1 or candidate_mdat != source_mdat:
        raise RuntimeError("stsz trailing-byte mutation changed encoded samples")

    candidate_sha256 = hashlib.sha256(candidate).hexdigest()
    if candidate_sha256 != expected_sha256:
        raise RuntimeError(f"generated fixture hash changed: {candidate_sha256}")
    return candidate


def track_only_common_size_candidate(expected_sha256: str) -> bytes:
    """Exercise the common-size table using Pillow's sequence-only decode path."""
    source = TRACK_SOURCE.read_bytes()
    source_sha256 = hashlib.sha256(source).hexdigest()
    if source_sha256 != TRACK_SOURCE_SHA256:
        raise RuntimeError(f"track-only source hash changed: {source_sha256}")
    if box_payload_at_path(source, STSZ_PATH) != SOURCE_STSZ:
        raise RuntimeError("pinned track-only stsz table changed")
    if box_payload_at_path(source, STCO_PATH) != TRACK_SOURCE_STCO:
        raise RuntimeError("pinned track-only stco table changed")

    file_delta = -20
    expected_stsz = SOURCE_STSZ[:4] + struct.pack(">II", 1, 5)
    changed = {"stsz": 0, "stco": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == STSZ_PATH:
            changed["stsz"] += 1
            if payload != SOURCE_STSZ:
                raise RuntimeError("pinned track-only stsz changed during rewrite")
            return expected_stsz
        if path == STCO_PATH:
            changed["stco"] += 1
            if payload != TRACK_SOURCE_STCO:
                raise RuntimeError("pinned track-only stco changed during rewrite")
            return adjust_chunk_offsets(payload, file_delta)
        return payload

    candidate = rewrite_boxes(source, transform)
    if changed != {"stsz": 1, "stco": 1}:
        raise RuntimeError(f"unexpected track-only box mutation counts: {changed}")
    if len(candidate) != len(source) + file_delta:
        raise RuntimeError("track-only stsz mutation produced an unexpected file size")
    if box_payload_at_path(candidate, STSZ_PATH) != expected_stsz:
        raise RuntimeError("generated track-only stsz differs from the mutation")
    expected_stco = adjust_chunk_offsets(TRACK_SOURCE_STCO, file_delta)
    if box_payload_at_path(candidate, STCO_PATH) != expected_stco:
        raise RuntimeError("generated track-only stco offsets were not rebased")

    source_mdat = [
        payload
        for kind, _, payload in read_boxes(source, 0, len(source))
        if kind == b"mdat"
    ]
    candidate_mdat = [
        payload
        for kind, _, payload in read_boxes(candidate, 0, len(candidate))
        if kind == b"mdat"
    ]
    if len(source_mdat) != 1 or candidate_mdat != source_mdat:
        raise RuntimeError("track-only stsz mutation changed encoded samples")

    candidate_sha256 = hashlib.sha256(candidate).hexdigest()
    if candidate_sha256 != expected_sha256:
        raise RuntimeError(f"generated track-only fixture hash changed: {candidate_sha256}")
    return candidate


def track_truncated_tail_candidate(expected_sha256: str) -> bytes:
    """Append an incomplete top-level box header after the complete AVIS movie."""
    candidate = track_only_common_size_candidate(TRACK_FIXTURE[1]) + struct.pack(
        ">I", 16
    )
    candidate_sha256 = hashlib.sha256(candidate).hexdigest()
    if candidate_sha256 != expected_sha256:
        raise RuntimeError(
            f"generated truncated-tail fixture hash changed: {candidate_sha256}"
        )
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
    filename, expected_sha256 = TRACK_FIXTURE
    output = ASSETS / filename
    candidate = track_only_common_size_candidate(expected_sha256)
    if args.check:
        if not output.exists() or output.read_bytes() != candidate:
            raise RuntimeError(f"{output.relative_to(ROOT)} differs from its recipe")
    else:
        output.write_bytes(candidate)
    filename, expected_sha256 = TRACK_TRUNCATED_TAIL_FIXTURE
    output = ASSETS / filename
    candidate = track_truncated_tail_candidate(expected_sha256)
    if args.check:
        if not output.exists() or output.read_bytes() != candidate:
            raise RuntimeError(f"{output.relative_to(ROOT)} differs from its recipe")
    else:
        output.write_bytes(candidate)


if __name__ == "__main__":
    main()
