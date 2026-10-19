#!/usr/bin/env python3
"""Duplicate the animated AVIF track's sample-to-chunk box verbatim.

The input mutation preserves all encoded samples and rebases the primary-item
extent and sample chunk offset for the additional 28-byte ``stsc`` box.
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "animated.avif"
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
FIXTURES = {
    "identical": (
        "animated_duplicate_stsc.avif",
        "56b417e69e63d2d56be80d3e58a71b8043a4134c7e8f283b2ed5e088357e43d0",
        5,
    ),
    "override_four": (
        "animated_duplicate_stsc_override_four.avif",
        "3343bc98df777765f8c79b1c180832af10262503a187af3a4206894547949589",
        4,
    ),
}

STBL_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl")
STSC_PATH = STBL_PATH + (b"stsc",)
STCO_PATH = STBL_PATH + (b"stco",)
ILOC_PATH = (b"meta", b"iloc")
SIZE_PATHS = tuple(STSC_PATH[:index] for index in range(1, len(STSC_PATH)))
TABLE_GROWTH_BYTES = 28
EXPECTED_STSC_PAYLOAD = bytes.fromhex("0000000000000001000000010000000500000001")
EXPECTED_STCO_PAYLOAD = bytes.fromhex("0000000000000001000003ff")
EXPECTED_ILOC_PAYLOAD = bytes.fromhex("0000000044000001000100000001000003ff00000027")


def boxes_at_path(data: bytes, path: tuple[bytes, ...]) -> tuple[bytes, bytes]:
    """Return the unique complete box and payload for a nested box path."""
    if not path:
        raise ValueError("box path cannot be empty")
    matches = [box for box in read_boxes(data, 0, len(data)) if box[0] == path[0]]
    if len(matches) != 1:
        raise RuntimeError(f"expected one {path[0]!r} box, found {len(matches)}")
    kind, complete, payload = matches[0]
    if len(path) == 1:
        return complete, payload
    if kind == b"meta":
        if payload[:4] != bytes(4):
            raise RuntimeError("pinned meta must be a version-zero FullBox")
        children = payload[4:]
    else:
        children = payload
    return boxes_at_path(children, path[1:])


def mdat_payload(data: bytes) -> tuple[int, bytes]:
    """Return the final ``mdat`` payload offset and bytes."""
    boxes = read_boxes(data, 0, len(data))
    if [kind for kind, _, _ in boxes] != [b"ftyp", b"meta", b"moov", b"mdat"]:
        raise RuntimeError("pinned AVIF must contain ftyp, meta, moov, and mdat")
    offset = sum(len(complete) for _, complete, _ in boxes[:3])
    payload = boxes[3][2]
    return offset + 8, payload


def transform(path: tuple[bytes, ...], payload: bytes, samples_per_chunk: int) -> bytes:
    """Append a second identical stsc box and rebase absolute data offsets."""
    if path == STBL_PATH:
        children = read_boxes(payload, 0, len(payload))
        matches = [box for box in children if box[0] == b"stsc"]
        if len(matches) != 1 or matches[0][2] != EXPECTED_STSC_PAYLOAD:
            raise RuntimeError("pinned stbl must contain one canonical stsc box")
        duplicate_payload = bytearray(matches[0][2])
        struct.pack_into(">I", duplicate_payload, 12, samples_per_chunk)
        return payload + pack_box(b"stsc", bytes(duplicate_payload))
    if path == STCO_PATH:
        if payload != EXPECTED_STCO_PAYLOAD:
            raise RuntimeError("pinned stco must contain the original mdat offset")
        return payload[:-4] + struct.pack(
            ">I", struct.unpack_from(">I", payload, len(payload) - 4)[0] + TABLE_GROWTH_BYTES
        )
    if path == ILOC_PATH:
        if payload != EXPECTED_ILOC_PAYLOAD:
            raise RuntimeError("pinned primary-item iloc extent changed")
        original_offset = struct.unpack_from(">I", payload, 14)[0]
        return payload[:14] + struct.pack(">I", original_offset + TABLE_GROWTH_BYTES) + payload[18:]
    return payload


def candidate_bytes(samples_per_chunk: int, fixture_sha256: str) -> bytes:
    """Build and validate the deterministic duplicate-box fixture."""
    source = SOURCE.read_bytes()
    source_hash = hashlib.sha256(source).hexdigest()
    if source_hash != SOURCE_SHA256:
        raise RuntimeError(f"animated source hash changed: {source_hash}")

    source_mdat_offset, source_mdat = mdat_payload(source)
    source_stbl = boxes_at_path(source, STBL_PATH)[1]
    stsc_boxes = [box for box in read_boxes(source_stbl, 0, len(source_stbl)) if box[0] == b"stsc"]
    if source_mdat_offset != 0x3FF or stsc_boxes != [
        (b"stsc", pack_box(b"stsc", EXPECTED_STSC_PAYLOAD), EXPECTED_STSC_PAYLOAD)
    ]:
        raise RuntimeError("pinned animated source no longer has one expected stsc box")
    source_stco = boxes_at_path(source, STCO_PATH)[1]
    source_iloc = boxes_at_path(source, ILOC_PATH)[1]
    if source_stco != EXPECTED_STCO_PAYLOAD or source_iloc != EXPECTED_ILOC_PAYLOAD:
        raise RuntimeError("pinned AVIF offsets differ from the expected source layout")

    candidate = rewrite_boxes(
        source, lambda path, payload: transform(path, payload, samples_per_chunk)
    )
    candidate_hash = hashlib.sha256(candidate).hexdigest()
    if candidate_hash != fixture_sha256:
        raise RuntimeError(f"generated fixture hash changed: {candidate_hash}")

    candidate_mdat_offset, candidate_mdat = mdat_payload(candidate)
    if candidate_mdat != source_mdat:
        raise RuntimeError("duplicating stsc changed encoded sample bytes")
    if candidate_mdat_offset != source_mdat_offset + TABLE_GROWTH_BYTES:
        raise RuntimeError("sample-table growth must move mdat by exactly 28 bytes")

    candidate_stbl = boxes_at_path(candidate, STBL_PATH)[1]
    candidate_stsc = [
        box for box in read_boxes(candidate_stbl, 0, len(candidate_stbl))
        if box[0] == b"stsc"
    ]
    expected_stsc = (b"stsc", pack_box(b"stsc", EXPECTED_STSC_PAYLOAD), EXPECTED_STSC_PAYLOAD)
    duplicate_payload = bytearray(EXPECTED_STSC_PAYLOAD)
    struct.pack_into(">I", duplicate_payload, 12, samples_per_chunk)
    expected_duplicate = (
        b"stsc",
        pack_box(b"stsc", bytes(duplicate_payload)),
        bytes(duplicate_payload),
    )
    if candidate_stsc != [expected_stsc, expected_duplicate]:
        raise RuntimeError("generated stbl must contain the expected duplicate stsc box")

    expected_offset = candidate_mdat_offset
    if boxes_at_path(candidate, STCO_PATH)[1] != bytes.fromhex("00000000000000010000041b"):
        raise RuntimeError("generated stco offset must point to the mdat payload")
    candidate_iloc = boxes_at_path(candidate, ILOC_PATH)[1]
    expected_iloc = (
        EXPECTED_ILOC_PAYLOAD[:14]
        + struct.pack(">I", expected_offset)
        + EXPECTED_ILOC_PAYLOAD[18:]
    )
    if candidate_iloc != expected_iloc:
        raise RuntimeError("generated iloc extent must point to the mdat payload")

    for path in SIZE_PATHS:
        original_size = struct.unpack_from(">I", boxes_at_path(source, path)[0])[0]
        candidate_size = struct.unpack_from(">I", boxes_at_path(candidate, path)[0])[0]
        if candidate_size != original_size + TABLE_GROWTH_BYTES:
            raise RuntimeError(f"sample-table ancestor size changed unexpectedly at {path}")

    return candidate


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--variant",
        choices=sorted(FIXTURES),
        default="identical",
        help="select an identical duplicate or a later table that overrides the sample count",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the generated fixture without creating or replacing it",
    )
    args = parser.parse_args()
    filename, fixture_sha256, samples_per_chunk = FIXTURES[args.variant]
    output = ASSETS / filename
    candidate = candidate_bytes(samples_per_chunk, fixture_sha256)
    if output.exists():
        if output.read_bytes() != candidate:
            raise RuntimeError(f"refusing to replace a different fixture at {output}")
    elif args.check:
        raise RuntimeError(f"fixture is missing: {output}")
    else:
        output.write_bytes(candidate)
    print(f"{output}: {len(candidate)} bytes, sha256={hashlib.sha256(candidate).hexdigest()}")


if __name__ == "__main__":
    main()
