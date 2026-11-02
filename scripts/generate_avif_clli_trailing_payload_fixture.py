#!/usr/bin/env python3
"""Generate the pinned AVIF CLLI trailing-payload parity fixture."""

from __future__ import annotations

import argparse
from hashlib import sha256
from pathlib import Path
import struct

import yaml

from avif_bmff_boxes import pack_box, read_boxes
from generate_avif_config_disagreement_fixtures import pillow_snapshot
from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE_FILENAME = "hdr.avif"
OUTPUT_FILENAME = "clli_trailing_payload_ignored.avif"
SOURCE_SHA256 = "9980e58ddf718a923f1738c34aad1c72f8e5795ec07e68f1a5f9bd216ca19740"
FIXTURE_SHA256 = "7862991cf4424c784f22d4a27ef128949eea8ad927c8ad05eed23a6e9effd108"
ILOC_PREFIX = bytes.fromhex("0000000044000005")
CLLI_PAYLOAD = bytes.fromhex("01d60107")
TRAILING_BYTE = b"\x00"


def iloc_extents(payload: bytes) -> list[tuple[int, int, int]]:
    """Read the pinned version-zero, file-backed HDR item extents."""
    if not payload.startswith(ILOC_PREFIX):
        raise RuntimeError("pinned HDR iloc header changed")

    item_count = int.from_bytes(payload[6:8], "big")
    position = 8
    extents = []
    for _ in range(item_count):
        if position + 6 > len(payload):
            raise RuntimeError("pinned HDR iloc item record is truncated")
        item_id = int.from_bytes(payload[position : position + 2], "big")
        data_reference_index = int.from_bytes(payload[position + 2 : position + 4], "big")
        extent_count = int.from_bytes(payload[position + 4 : position + 6], "big")
        position += 6
        if data_reference_index != 0 or extent_count != 1:
            raise RuntimeError("pinned HDR iloc item is not one local file extent")
        if position + 8 > len(payload):
            raise RuntimeError("pinned HDR iloc extent record is truncated")
        offset = int.from_bytes(payload[position : position + 4], "big")
        length = int.from_bytes(payload[position + 4 : position + 8], "big")
        position += 8
        extents.append((item_id, offset, length))

    if position != len(payload) or len(extents) != 5:
        raise RuntimeError("pinned HDR iloc record count or size changed")
    return extents


def rebase_iloc(payload: bytes, amount: int) -> bytes:
    """Shift every absolute item extent after metadata grows by ``amount``."""
    extents = iloc_extents(payload)
    updated = bytearray(payload)
    position = 8
    for _, offset, _ in extents:
        updated_offset = offset + amount
        if updated_offset > 0xFFFF_FFFF:
            raise RuntimeError("rebased HDR iloc offset exceeds its field")
        struct.pack_into(">I", updated, position + 6, updated_offset)
        position += 14
    return bytes(updated)


def rebuild_ipco(payload: bytes) -> bytes:
    """Append one bounded byte to the unique CLLI property."""
    boxes = read_boxes(payload, 0, len(payload))
    rebuilt = []
    clli_count = 0
    for kind, _, child_payload in boxes:
        if kind == b"clli":
            clli_count += 1
            if child_payload != CLLI_PAYLOAD:
                raise RuntimeError("pinned HDR CLLI fields changed")
            child_payload += TRAILING_BYTE
        rebuilt.append(pack_box(kind, child_payload))
    if clli_count != 1:
        raise RuntimeError(f"expected one HDR CLLI property, found {clli_count}")
    return b"".join(rebuilt)


def rebuild_iprp(payload: bytes) -> bytes:
    """Rebuild the property container after the CLLI box grows."""
    boxes = read_boxes(payload, 0, len(payload))
    rebuilt = []
    ipco_count = 0
    for kind, _, child_payload in boxes:
        if kind == b"ipco":
            ipco_count += 1
            child_payload = rebuild_ipco(child_payload)
        rebuilt.append(pack_box(kind, child_payload))
    if ipco_count != 1:
        raise RuntimeError(f"expected one HDR ipco box, found {ipco_count}")
    return b"".join(rebuilt)


def rebuild_meta(payload: bytes) -> bytes:
    """Rebuild HDR metadata and rebase file-backed item locations."""
    if payload[:4] != bytes(4):
        raise RuntimeError("pinned HDR meta FullBox header changed")
    boxes = read_boxes(payload, 4, len(payload))
    rebuilt = []
    counts = {b"iloc": 0, b"iprp": 0}
    for kind, _, child_payload in boxes:
        if kind in counts:
            counts[kind] += 1
        if kind == b"iloc":
            child_payload = rebase_iloc(child_payload, len(TRAILING_BYTE))
        elif kind == b"iprp":
            child_payload = rebuild_iprp(child_payload)
        rebuilt.append(pack_box(kind, child_payload))
    if counts != {b"iloc": 1, b"iprp": 1}:
        raise RuntimeError(f"pinned HDR meta layout changed: {counts}")
    return payload[:4] + b"".join(rebuilt)


def unique_box_payload(data: bytes, kind: bytes) -> bytes:
    """Return the payload of one unique top-level box."""
    matches = [payload for box_kind, _, payload in read_boxes(data, 0, len(data)) if box_kind == kind]
    if len(matches) != 1:
        raise RuntimeError(f"expected one top-level {kind!r} box, found {len(matches)}")
    return matches[0]


def metadata_iloc(data: bytes) -> bytes:
    """Return the unique iloc payload nested in the pinned meta box."""
    meta = unique_box_payload(data, b"meta")
    matches = [
        payload
        for kind, _, payload in read_boxes(meta, 4, len(meta))
        if kind == b"iloc"
    ]
    if len(matches) != 1:
        raise RuntimeError(f"expected one HDR iloc box, found {len(matches)}")
    return matches[0]


def candidate_bytes(source: bytes) -> bytes:
    """Grow CLLI and enclosing metadata while preserving every media extent."""
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned HDR source hash changed")

    top_level = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in top_level] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("pinned HDR top-level box layout changed")

    rebuilt = []
    for kind, _, payload in top_level:
        if kind == b"meta":
            payload = rebuild_meta(payload)
        rebuilt.append(pack_box(kind, payload))
    candidate = b"".join(rebuilt)
    if len(candidate) != len(source) + len(TRAILING_BYTE):
        raise RuntimeError("CLLI mutation changed an unexpected number of bytes")

    source_media = unique_box_payload(source, b"mdat")
    candidate_media = unique_box_payload(candidate, b"mdat")
    if source_media != candidate_media:
        raise RuntimeError("CLLI mutation changed the AV1 media payload")

    source_extents = iloc_extents(metadata_iloc(source))
    candidate_extents = iloc_extents(metadata_iloc(candidate))
    if len(source_extents) != len(candidate_extents):
        raise RuntimeError("CLLI mutation changed the item extent count")
    source_bytes = []
    candidate_extent_bytes = []
    for before, after in zip(source_extents, candidate_extents, strict=True):
        source_id, source_offset, source_length = before
        candidate_id, candidate_offset, candidate_length = after
        if (
            source_id != candidate_id
            or source_length != candidate_length
            or candidate_offset != source_offset + len(TRAILING_BYTE)
        ):
            raise RuntimeError("CLLI mutation did not rebase the pinned item extents")
        source_extent = source[source_offset : source_offset + source_length]
        candidate_extent = candidate[candidate_offset : candidate_offset + candidate_length]
        if len(source_extent) != source_length or len(candidate_extent) != candidate_length:
            raise RuntimeError("CLLI mutation produced an out-of-bounds item extent")
        source_bytes.append(source_extent)
        candidate_extent_bytes.append(candidate_extent)
    if source_bytes != candidate_extent_bytes:
        raise RuntimeError("CLLI mutation changed an AV1 item extent")

    actual_hash = sha256(candidate).hexdigest()
    if actual_hash != FIXTURE_SHA256:
        raise RuntimeError(f"{OUTPUT_FILENAME} hash changed: {actual_hash}")
    return candidate


def generate(*, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    source = (ASSETS / SOURCE_FILENAME).read_bytes()
    candidate = candidate_bytes(source)
    if pillow_snapshot(candidate) != pillow_snapshot(source):
        raise RuntimeError("Pillow observable HDR output changed after the CLLI mutation")

    output = ASSETS / OUTPUT_FILENAME
    if check:
        if not output.is_file() or output.read_bytes() != candidate:
            raise RuntimeError(f"generated fixture is missing or stale: {output}")
    elif output.exists():
        if output.read_bytes() != candidate:
            raise RuntimeError(f"refusing to replace a different fixture: {output}")
    else:
        output.write_bytes(candidate)
    print(f"{output}: {len(candidate)} bytes, sha256={FIXTURE_SHA256}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify the existing fixture")
    args = parser.parse_args()
    generate(check=args.check)


if __name__ == "__main__":
    main()
