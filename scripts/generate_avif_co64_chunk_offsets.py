#!/usr/bin/env python3
"""Generate an AVIF sequence with its track chunk-offset table widened to co64.

The mutation is intentionally limited to the pinned animated sequence: it
converts ``moov/trak/mdia/minf/stbl/stco`` to ``co64``, rebases the chunk and
primary-item offsets for the four-byte insertion, and rebuilds the affected
box sizes. The mdat/sample bytes are preserved verbatim.
"""

from __future__ import annotations

import hashlib
from pathlib import Path
import struct


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "animated.avif"
OUTPUT = ASSETS / "animated_co64_chunk_offsets.avif"
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
FIXTURE_SHA256 = "589f8836e3e1fbe9f8524353b5bc56cc615927dc15848c1e4fb671c467c04021"

TRACK_STCO_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stco")
TRACK_CO64_PATH = TRACK_STCO_PATH[:-1] + (b"co64",)
PRIMARY_ILOC_PATH = (b"meta", b"iloc")
CONTAINERS = {b"moov", b"trak", b"mdia", b"minf", b"stbl"}
SIZE_REBUILD_PATHS = (
    (b"moov",),
    (b"moov", b"trak"),
    (b"moov", b"trak", b"mdia"),
    (b"moov", b"trak", b"mdia", b"minf"),
    (b"moov", b"trak", b"mdia", b"minf", b"stbl"),
)
BOX_INSERTION_BYTES = 4  # one 32-bit offset becomes one 64-bit offset
SOURCE_CHUNK_OFFSET = 0x3FF


def parse_boxes(data: bytes) -> list[tuple[bytes, int, bytes]]:
    """Parse a bounded sequence of standard-size BMFF boxes."""
    boxes: list[tuple[bytes, int, bytes]] = []
    offset = 0
    while offset < len(data):
        if len(data) - offset < 8:
            raise RuntimeError(f"truncated BMFF box header at byte {offset}")
        size = struct.unpack_from(">I", data, offset)[0]
        kind = data[offset + 4 : offset + 8]
        if size < 8 or size > len(data) - offset:
            raise RuntimeError(f"invalid {kind!r} box size {size} at byte {offset}")
        boxes.append((kind, size, data[offset + 8 : offset + size]))
        offset += size
    if offset != len(data):
        raise RuntimeError("BMFF boxes do not exactly cover their parent")
    return boxes


def pack_box(kind: bytes, payload: bytes) -> bytes:
    """Pack a standard-size BMFF box, rejecting sizes that need an extended header."""
    if len(kind) != 4:
        raise ValueError("BMFF box types must contain exactly four bytes")
    size = len(payload) + 8
    if size > 0xFFFF_FFFF:
        raise RuntimeError(f"{kind!r} box no longer fits a standard-size header")
    return struct.pack(">I", size) + kind + payload


def rewrite_boxes(data: bytes, path: tuple[bytes, ...] = ()) -> bytes:
    """Rebuild containers and apply only the pinned stco and iloc mutations."""
    output: list[bytes] = []
    for kind, _, payload in parse_boxes(data):
        child_path = path + (kind,)
        if kind == b"meta":
            if payload[:4] != bytes(4):
                raise RuntimeError("the pinned top-level meta must be a version-zero FullBox")
            payload = payload[:4] + rewrite_boxes(payload[4:], child_path)
        elif kind in CONTAINERS:
            payload = rewrite_boxes(payload, child_path)

        if child_path == TRACK_STCO_PATH:
            if kind != b"stco" or len(payload) < 8 or payload[:4] != bytes(4):
                raise RuntimeError("the target track stco FullBox changed")
            count = struct.unpack_from(">I", payload, 4)[0]
            if count != 1 or len(payload) != 8 + 4 * count:
                raise RuntimeError("the target track must contain exactly one 32-bit chunk offset")
            offsets = list(struct.unpack_from(f">{count}I", payload, 8))
            if offsets != [SOURCE_CHUNK_OFFSET]:
                raise RuntimeError("the pinned source chunk offset changed")
            widened = b"".join(
                struct.pack(">Q", offset + BOX_INSERTION_BYTES) for offset in offsets
            )
            kind = b"co64"
            payload = payload[:8] + widened
        elif child_path == PRIMARY_ILOC_PATH:
            expected = bytes.fromhex(
                "0000000044000001000100000001000003ff00000027"
            )
            if payload != expected:
                raise RuntimeError("the pinned top-level meta iloc primary extent changed")
            old_extent_offset = struct.unpack_from(">I", payload, 14)[0]
            if old_extent_offset != SOURCE_CHUNK_OFFSET:
                raise RuntimeError("the primary-item extent no longer points to the source mdat")
            payload = (
                payload[:14]
                + struct.pack(">I", old_extent_offset + BOX_INSERTION_BYTES)
                + payload[18:]
            )

        output.append(pack_box(kind, payload))
    return b"".join(output)


def find_box(data: bytes, path: tuple[bytes, ...]) -> tuple[int, bytes]:
    """Return the unique box size and payload at a path from the file root."""
    if not path:
        raise ValueError("box path cannot be empty")
    candidates = [box for box in parse_boxes(data) if box[0] == path[0]]
    if len(candidates) != 1:
        raise RuntimeError(f"expected one {path[0]!r} box at {path}, found {len(candidates)}")
    _, size, payload = candidates[0]
    if len(path) == 1:
        return size, payload
    if path[0] == b"meta":
        if payload[:4] != bytes(4):
            raise RuntimeError("the pinned meta must be a version-zero FullBox")
        child_data = payload[4:]
    elif path[0] in CONTAINERS:
        child_data = payload
    else:
        raise RuntimeError(f"cannot descend through non-container box {path[0]!r}")
    return find_box(child_data, path[1:])


def top_level_boxes(data: bytes) -> list[tuple[bytes, int, bytes]]:
    boxes = parse_boxes(data)
    if [kind for kind, _, _ in boxes] != [b"ftyp", b"meta", b"moov", b"mdat"]:
        raise RuntimeError("the pinned AVIF must have top-level ftyp, meta, moov and mdat boxes")
    return boxes


def mdat_payload_start(boxes: list[tuple[bytes, int, bytes]]) -> int:
    offset = 0
    for kind, size, _ in boxes:
        if kind == b"mdat":
            return offset + 8
        offset += size
    raise RuntimeError("the pinned AVIF has no top-level mdat")


def candidate_bytes() -> bytes:
    source = SOURCE.read_bytes()
    source_hash = hashlib.sha256(source).hexdigest()
    if source_hash != SOURCE_SHA256:
        raise RuntimeError(f"animated source hash changed: {source_hash}")
    source_boxes = top_level_boxes(source)
    if len(source_boxes[2][2]) < 8:
        raise RuntimeError("the pinned moov box is unexpectedly short")
    _, stco_payload = find_box(source, TRACK_STCO_PATH)
    if stco_payload != bytes.fromhex("0000000000000001000003ff"):
        raise RuntimeError("the pinned track chunk-offset table changed")
    source_iloc_size, _ = find_box(source, PRIMARY_ILOC_PATH)
    source_mdat = [payload for kind, _, payload in source_boxes if kind == b"mdat"]
    if len(source_mdat) != 1 or mdat_payload_start(source_boxes) != SOURCE_CHUNK_OFFSET:
        raise RuntimeError("the source chunk offset must identify the top-level mdat payload")

    candidate = rewrite_boxes(source)
    candidate_hash = hashlib.sha256(candidate).hexdigest()
    if candidate_hash != FIXTURE_SHA256:
        raise RuntimeError(f"generated fixture hash changed: {candidate_hash}")
    candidate_boxes = top_level_boxes(candidate)

    co64_size, co64_payload = find_box(candidate, TRACK_CO64_PATH)
    stco_size, _ = find_box(source, TRACK_STCO_PATH)
    stbl_payload = find_box(candidate, TRACK_CO64_PATH[:-1])[1]
    if any(kind == b"stco" for kind, _, _ in parse_boxes(stbl_payload)):
        raise RuntimeError("the target stbl must not retain its old stco box")
    if co64_size != stco_size + BOX_INSERTION_BYTES:
        raise RuntimeError("co64 must be four bytes larger than the source stco")
    if co64_payload != bytes.fromhex("00000000000000010000000000000403"):
        raise RuntimeError("generated co64 table must contain the rebased u64 offset 0x403")
    if find_box(candidate, PRIMARY_ILOC_PATH)[1] != bytes.fromhex(
        "00000000440000010001000000010000040300000027"
    ):
        raise RuntimeError("generated iloc primary-item extent must be rebased to 0x403")
    if find_box(candidate, PRIMARY_ILOC_PATH)[0] != source_iloc_size:
        raise RuntimeError("rebasing the iloc extent must not change meta box size")

    source_sizes = {path: find_box(source, path)[0] for path in SIZE_REBUILD_PATHS}
    candidate_sizes = {path: find_box(candidate, path)[0] for path in SIZE_REBUILD_PATHS}
    if any(
        candidate_sizes[path] != source_sizes[path] + BOX_INSERTION_BYTES
        for path in SIZE_REBUILD_PATHS
    ):
        raise RuntimeError("co64 insertion must add four bytes to each ancestor box size")
    source_mdat_start = mdat_payload_start(source_boxes)
    candidate_mdat_start = mdat_payload_start(candidate_boxes)
    if candidate_mdat_start != source_mdat_start + BOX_INSERTION_BYTES:
        raise RuntimeError("co64 insertion must move the mdat payload by exactly four bytes")
    if candidate_mdat_start != struct.unpack_from(">Q", co64_payload, 8)[0]:
        raise RuntimeError("generated co64 offset does not identify the mdat payload")
    if candidate_mdat_start != struct.unpack_from(">I", find_box(candidate, PRIMARY_ILOC_PATH)[1], 14)[0]:
        raise RuntimeError("generated iloc extent does not identify the mdat payload")

    candidate_mdat = [payload for kind, _, payload in candidate_boxes if kind == b"mdat"]
    if candidate_mdat != source_mdat:
        raise RuntimeError("co64 mutation changed mdat/sample payload bytes")
    if candidate_boxes[0] != source_boxes[0]:
        raise RuntimeError("co64 mutation changed the ftyp box")
    return candidate


def main() -> None:
    candidate = candidate_bytes()
    if OUTPUT.exists():
        if OUTPUT.read_bytes() != candidate:
            raise RuntimeError(f"refusing to replace a different fixture at {OUTPUT}")
    else:
        OUTPUT.write_bytes(candidate)
    print(
        f"{OUTPUT}: {len(candidate)} bytes, "
        f"sha256={hashlib.sha256(candidate).hexdigest()}"
    )


if __name__ == "__main__":
    main()
