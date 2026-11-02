#!/usr/bin/env python3
"""Add an ignored hint track with a non-AV1 sample entry to an AVIS fixture."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "animated_sequence_only_no_meta.avif"
OUTPUT = ASSETS / "animated_unknown_sample_entry_ignored_hint_track.avif"
SOURCE_SHA256 = "c861761e45518018150dc52302da7ff5015d1f259ff2c1309eea94eaf8176fdb"
OUTPUT_SHA256 = "dcc55d191d0f5e9cbabcd1dfbc9119b2f8e712ec1cea33880873739ed9987a49"
STCO_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stco")


def children(payload: bytes) -> list[tuple[bytes, bytes]]:
    """Return direct children from a standard-sized box payload."""
    return [(kind, child_payload) for kind, _, child_payload in read_boxes(payload, 0, len(payload))]


def unique_child(payload: bytes, kind: bytes, parent: str) -> bytes:
    """Return one expected child payload or explain which pinned layout changed."""
    matches = [
        child_payload
        for child_kind, child_payload in children(payload)
        if child_kind == kind
    ]
    if len(matches) != 1:
        raise RuntimeError(f"pinned {parent} must contain exactly one {kind.decode()} box")
    return matches[0]


def track_identifier(payload: bytes) -> int:
    """Read a version-zero or version-one track ID from a validated tkhd."""
    if len(payload) < 24:
        raise RuntimeError("pinned AVIS track header is too short")
    version = payload[0]
    if version == 0:
        offset = 12
    elif version == 1:
        offset = 20
    else:
        raise RuntimeError(f"unsupported pinned track-header version: {version}")
    identifier = struct.unpack_from(">I", payload, offset)[0]
    if identifier == 0:
        raise RuntimeError("pinned AVIS track ID must be nonzero")
    return identifier


def make_hint_track(identifier: int, chunk_offset: int) -> bytes:
    """Build an ignored hint track with one bounded sample and an unknown entry."""
    tkhd = bytearray(84)
    struct.pack_into(">I", tkhd, 12, identifier)

    mdhd = bytearray(24)
    struct.pack_into(">I", mdhd, 12, 1000)

    full_box = bytes(4)
    hdlr = full_box + bytes(4) + b"hint" + bytes(12) + b"\0"
    stco = full_box + struct.pack(">II", 1, chunk_offset)
    stsc = full_box + struct.pack(">I", 1) + struct.pack(">III", 1, 1, 1)
    stsz = full_box + struct.pack(">II", 1, 1)
    stsd = full_box + struct.pack(">I", 1) + pack_box(b"zzzz", b"")

    sample_table = pack_box(
        b"stbl",
        pack_box(b"stco", stco)
        + pack_box(b"stsc", stsc)
        + pack_box(b"stsz", stsz)
        + pack_box(b"stsd", stsd),
    )
    media = pack_box(
        b"mdia",
        pack_box(b"mdhd", bytes(mdhd))
        + pack_box(b"hdlr", hdlr)
        + pack_box(b"minf", sample_table),
    )
    return pack_box(b"trak", pack_box(b"tkhd", bytes(tkhd)) + media)


def fixture_bytes() -> bytes:
    """Append one hint track and rebase the original media chunk offsets."""
    source = SOURCE.read_bytes()
    actual_source_hash = hashlib.sha256(source).hexdigest()
    if actual_source_hash != SOURCE_SHA256:
        raise RuntimeError(f"pinned AVIS source hash changed: {actual_source_hash}")

    top_level = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in top_level] != [b"ftyp", b"free", b"moov", b"mdat"]:
        raise RuntimeError("pinned AVIS top-level box layout changed")
    moov_payload = unique_child(source, b"moov", "source")
    tracks = [payload for kind, payload in children(moov_payload) if kind == b"trak"]
    if len(tracks) != 1:
        raise RuntimeError("pinned AVIS must contain exactly one track")
    track_children = children(tracks[0])
    headers = [payload for kind, payload in track_children if kind == b"tkhd"]
    if len(headers) != 1:
        raise RuntimeError("pinned AVIS must contain exactly one track header")
    identifier = track_identifier(headers[0]) + 1
    if identifier > 0xFFFF_FFFF:
        raise RuntimeError("no unused track identifier remains")

    media = next((payload for kind, _, payload in top_level if kind == b"mdat"), None)
    if media is None or not media:
        raise RuntimeError("pinned AVIS media data must contain a sample byte")
    media_box = unique_child(tracks[0], b"mdia", "track")
    media_info = unique_child(media_box, b"minf", "media box")
    sample_table = unique_child(media_info, b"stbl", "media-information box")
    offsets = unique_child(sample_table, b"stco", "sample table")
    if len(offsets) != 12:
        raise RuntimeError("pinned AVIS must contain one one-entry stco table")
    source_chunk_offset = struct.unpack_from(">I", offsets, 8)[0]

    added_track = make_hint_track(identifier, 0)
    growth = len(added_track)
    hint_chunk_offset = source_chunk_offset + growth
    if hint_chunk_offset > 0xFFFF_FFFF:
        raise RuntimeError("hint sample offset cannot be rebased without overflow")
    added_track = make_hint_track(identifier, hint_chunk_offset)
    if len(added_track) != growth:
        raise RuntimeError("hint-track size changed while rebasing its sample")
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path == STCO_PATH:
            changed += 1
            result = bytearray(payload)
            if len(result) < 8 or result[:4] != bytes(4):
                raise RuntimeError("pinned AVIS chunk-offset table header changed")
            count = struct.unpack_from(">I", result, 4)[0]
            if len(result) != 8 + count * 4:
                raise RuntimeError("pinned AVIS chunk-offset table length changed")
            for index in range(count):
                offset = 8 + index * 4
                current = struct.unpack_from(">I", result, offset)[0]
                adjusted = current + growth
                if adjusted > 0xFFFF_FFFF:
                    raise RuntimeError("chunk offset cannot be rebased without overflow")
                struct.pack_into(">I", result, offset, adjusted)
            return bytes(result)
        if path == (b"moov",):
            return payload + added_track
        return payload

    candidate = rewrite_boxes(source, transform)
    if changed != 1:
        raise RuntimeError(f"expected one original stco table, found {changed}")

    actual_output_hash = hashlib.sha256(candidate).hexdigest()
    if OUTPUT_SHA256 and actual_output_hash != OUTPUT_SHA256:
        raise RuntimeError(f"generated fixture hash changed: {actual_output_hash}")
    return candidate


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    candidate = fixture_bytes()
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != candidate:
            raise RuntimeError(f"{OUTPUT.relative_to(ROOT)} differs from its recipe")
    else:
        OUTPUT.write_bytes(candidate)


if __name__ == "__main__":
    main()
