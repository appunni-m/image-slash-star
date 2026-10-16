#!/usr/bin/env python3
"""Split the pinned animated AVIF track across two sample-table chunks.

The mutation adds a second ``stsc`` mapping and a second ``stco`` offset while
preserving every encoded sample byte. The primary-item extent and chunk offsets
are rebased for the sixteen-byte increase in the track sample table.
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
OUTPUT = ASSETS / "animated_stsc_two_chunk_mapping.avif"
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
FIXTURE_SHA256 = "117a8b2d61e01d9e9a5d911313524509c4b99d527b108e552a0b6e27d2a8500c"

STSC_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stsc")
STCO_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stco")
STSZ_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stsz")
ILOC_PATH = (b"meta", b"iloc")
SIZE_PATHS = tuple(STSC_PATH[:index] for index in range(1, len(STSC_PATH)))
TABLE_GROWTH_BYTES = 16
SOURCE_MDAT_PAYLOAD_OFFSET = 0x3FF
SAMPLES_PER_CHUNK = (2, 3)


def boxes_at_path(data: bytes, path: tuple[bytes, ...]) -> tuple[bytes, bytes]:
    """Return the unique box bytes and payload for a path below the file root."""
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
        child_data = payload[4:]
    else:
        child_data = payload
    return boxes_at_path(child_data, path[1:])


def mdat_payload(data: bytes) -> tuple[int, bytes]:
    """Return the single top-level mdat payload offset and bytes."""
    boxes = read_boxes(data, 0, len(data))
    if [kind for kind, _, _ in boxes] != [b"ftyp", b"meta", b"moov", b"mdat"]:
        raise RuntimeError("pinned AVIF must contain ftyp, meta, moov, and mdat")
    offset = sum(len(complete) for kind, complete, _ in boxes[:3])
    payload = boxes[3][2]
    if boxes[3][0] != b"mdat":
        raise RuntimeError("pinned AVIF mdat is not the final top-level box")
    return offset + 8, payload


def sample_sizes(data: bytes) -> tuple[int, ...]:
    """Parse and validate the pinned variable-size sample table."""
    _, payload = boxes_at_path(data, STSZ_PATH)
    if len(payload) < 12 or payload[:4] != bytes(4):
        raise RuntimeError("pinned stsz must be a version-zero FullBox")
    uniform_size, count = struct.unpack_from(">II", payload, 4)
    if uniform_size != 0 or len(payload) != 12 + count * 4:
        raise RuntimeError("pinned stsz must contain a complete variable-size table")
    sizes = struct.unpack_from(f">{count}I", payload, 12)
    if sizes != (39, 113, 5, 30, 25):
        raise RuntimeError("pinned animated AVIF sample sizes changed")
    return sizes


def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
    """Apply the two-chunk mapping and rebase the primary-item extent."""
    if path == STSC_PATH:
        if payload != bytes.fromhex("0000000000000001000000010000000500000001"):
            raise RuntimeError("pinned stsc must contain one five-sample mapping")
        return bytes.fromhex(
            "0000000000000002000000010000000200000001000000020000000300000001"
        )
    if path == STCO_PATH:
        if payload != bytes.fromhex("0000000000000001000003ff"):
            raise RuntimeError("pinned stco must contain the original single chunk")
        return struct.pack(">II2I", 0, 2, 0x40F, 0x4A7)
    if path == ILOC_PATH:
        expected = bytes.fromhex("0000000044000001000100000001000003ff00000027")
        if payload != expected:
            raise RuntimeError("pinned primary-item iloc extent changed")
        return payload[:14] + struct.pack(">I", 0x40F) + payload[18:]
    return payload


def candidate_bytes() -> bytes:
    """Build and validate the deterministically pinned two-chunk fixture."""
    source = SOURCE.read_bytes()
    source_hash = hashlib.sha256(source).hexdigest()
    if source_hash != SOURCE_SHA256:
        raise RuntimeError(f"animated source hash changed: {source_hash}")

    source_mdat_offset, source_mdat = mdat_payload(source)
    sizes = sample_sizes(source)
    if source_mdat_offset != SOURCE_MDAT_PAYLOAD_OFFSET or sum(sizes) != len(source_mdat):
        raise RuntimeError("pinned samples must exactly fill the mdat payload")
    first_chunk_bytes = sum(sizes[: SAMPLES_PER_CHUNK[0]])
    if SAMPLES_PER_CHUNK != (2, 3) or first_chunk_bytes != 152:
        raise RuntimeError("pinned sample split no longer defines the two expected chunks")

    candidate = rewrite_boxes(source, transform)
    candidate_hash = hashlib.sha256(candidate).hexdigest()
    if candidate_hash != FIXTURE_SHA256:
        raise RuntimeError(f"generated fixture hash changed: {candidate_hash}")

    candidate_mdat_offset, candidate_mdat = mdat_payload(candidate)
    if candidate_mdat != source_mdat:
        raise RuntimeError("two-chunk mutation changed encoded sample bytes")
    if candidate_mdat_offset != source_mdat_offset + TABLE_GROWTH_BYTES:
        raise RuntimeError("sample-table growth must move mdat by exactly sixteen bytes")

    stsc_box, stsc_payload = boxes_at_path(candidate, STSC_PATH)
    if stsc_box != pack_box(b"stsc", stsc_payload):
        raise RuntimeError("generated stsc box does not use its canonical size")
    if stsc_payload != bytes.fromhex(
        "0000000000000002000000010000000200000001000000020000000300000001"
    ):
        raise RuntimeError("generated stsc rows do not map chunks 1–2 and 3–5")

    stco_box, stco_payload = boxes_at_path(candidate, STCO_PATH)
    expected_offsets = (candidate_mdat_offset, candidate_mdat_offset + first_chunk_bytes)
    if len(stco_payload) != 16 or stco_payload[:8] != bytes.fromhex("0000000000000002"):
        raise RuntimeError("generated stco must contain exactly two 32-bit offsets")
    if struct.unpack_from(">2I", stco_payload, 8) != expected_offsets:
        raise RuntimeError("generated stco offsets must point to each contiguous chunk")
    if stco_box != pack_box(b"stco", stco_payload):
        raise RuntimeError("generated stco box does not use its canonical size")

    iloc_box, iloc_payload = boxes_at_path(candidate, ILOC_PATH)
    if iloc_payload != bytes.fromhex("00000000440000010001000000010000040f00000027"):
        raise RuntimeError("generated iloc primary extent must point to the first chunk")
    if len(iloc_box) != len(boxes_at_path(source, ILOC_PATH)[0]):
        raise RuntimeError("rebasing the primary-item extent must not change meta size")

    for path in SIZE_PATHS:
        original_size = struct.unpack_from(">I", boxes_at_path(source, path)[0])[0]
        candidate_size = struct.unpack_from(">I", boxes_at_path(candidate, path)[0])[0]
        if candidate_size != original_size + TABLE_GROWTH_BYTES:
            raise RuntimeError(f"sample-table ancestor size changed unexpectedly at {path}")

    return candidate


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the generated fixture without creating or replacing it",
    )
    args = parser.parse_args()

    candidate = candidate_bytes()
    if OUTPUT.exists():
        if OUTPUT.read_bytes() != candidate:
            raise RuntimeError(f"refusing to replace a different fixture at {OUTPUT}")
    elif args.check:
        raise RuntimeError(f"fixture is missing: {OUTPUT}")
    else:
        OUTPUT.write_bytes(candidate)
    print(f"{OUTPUT}: {len(candidate)} bytes, sha256={hashlib.sha256(candidate).hexdigest()}")


if __name__ == "__main__":
    main()
