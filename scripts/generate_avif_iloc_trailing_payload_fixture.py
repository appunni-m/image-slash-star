#!/usr/bin/env python3
"""Generate a Pillow-compatible AVIF with trailing bytes in its iloc box."""

from __future__ import annotations

import argparse
import hashlib
import struct
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURE_DIR = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = FIXTURE_DIR / "baseline.avif"
OUTPUT = FIXTURE_DIR / "iloc_trailing_payload_ignored.avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
FIXTURE_SHA256 = "d62c4cf7da0e986cf90225f63160fb4ff8bd851cf0d2a0c2957bc48ef7ce4504"
ILOC_PREFIX = bytes.fromhex("0000000044000001000100000001")
EXPECTED_FILE_EXTENT_OFFSET = 0x11A
TRAILING_BYTE = b"\xff"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def box_ranges(
    data: bytes, start: int, end: int
) -> list[tuple[bytes, int, int, int]]:
    """Return ``(kind, box_start, payload_start, box_end)`` for bounded boxes."""
    if not 0 <= start <= end <= len(data):
        raise RuntimeError("AVIF box range is outside its parent")

    boxes = []
    position = start
    while position < end:
        remaining = end - position
        if remaining < 8:
            raise RuntimeError("AVIF box header is truncated")
        size, kind = struct.unpack_from(">I4s", data, position)
        header_size = 8
        if size == 1:
            if remaining < 16:
                raise RuntimeError("AVIF large-box header is truncated")
            size = struct.unpack_from(">Q", data, position + 8)[0]
            header_size = 16
        elif size == 0:
            size = remaining
        if size < header_size or size > remaining:
            raise RuntimeError(f"AVIF {kind!r} box size is invalid")
        box_end = position + size
        boxes.append((kind, position, position + header_size, box_end))
        position = box_end
    if position != end:
        raise RuntimeError("AVIF boxes do not fill their parent")
    return boxes


def pack_boxes(boxes: list[tuple[bytes, bytes]]) -> bytes:
    """Serialize standard-size boxes while refreshing their 32-bit lengths."""
    packed = bytearray()
    for kind, payload in boxes:
        size = len(payload) + 8
        if len(kind) != 4 or size > 0xFFFF_FFFF:
            raise RuntimeError("AVIF fixture box cannot use a 32-bit header")
        packed.extend(struct.pack(">I4s", size, kind))
        packed.extend(payload)
    return bytes(packed)


def build_fixture(source: bytes) -> bytes:
    """Append an iloc tail byte and rebase its file-backed extent by one."""
    if sha256(source) != SOURCE_SHA256:
        raise RuntimeError("baseline AVIF source differs from its pinned hash")

    top_level = box_ranges(source, 0, len(source))
    if [box[0] for box in top_level] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("baseline AVIF top-level box layout changed")
    _, _, meta_payload_start, meta_end = top_level[1]
    if source[meta_payload_start : meta_payload_start + 4] != bytes(4):
        raise RuntimeError("baseline AVIF meta must use a version-zero FullBox")

    meta_ranges = box_ranges(source, meta_payload_start + 4, meta_end)
    meta_children = [
        (kind, source[payload_start:box_end])
        for kind, _, payload_start, box_end in meta_ranges
    ]
    iloc_indexes = [
        index for index, (kind, _) in enumerate(meta_children) if kind == b"iloc"
    ]
    if len(iloc_indexes) != 1:
        raise RuntimeError("baseline AVIF meta must contain one iloc box")

    iloc_index = iloc_indexes[0]
    iloc = bytearray(meta_children[iloc_index][1])
    if len(iloc) != 22 or iloc[:14] != ILOC_PREFIX:
        raise RuntimeError("baseline AVIF iloc is not one 32-bit file-backed extent")
    extent_offset = struct.unpack_from(">I", iloc, 14)[0]
    extent_length = struct.unpack_from(">I", iloc, 18)[0]
    if extent_offset != EXPECTED_FILE_EXTENT_OFFSET or extent_length == 0:
        raise RuntimeError("baseline AVIF iloc extent differs from the pinned layout")

    # The appended byte grows meta before mdat, shifting this absolute extent.
    struct.pack_into(">I", iloc, 14, extent_offset + len(TRAILING_BYTE))
    iloc.extend(TRAILING_BYTE)
    meta_children[iloc_index] = (b"iloc", bytes(iloc))

    meta_payload = source[meta_payload_start : meta_payload_start + 4]
    root_children = [
        (kind, source[payload_start:box_end])
        for kind, _, payload_start, box_end in top_level
    ]
    root_children[1] = (b"meta", meta_payload + pack_boxes(meta_children))
    fixture = pack_boxes(root_children)
    if len(fixture) != len(source) + len(TRAILING_BYTE):
        raise RuntimeError("AVIF trailing-payload fixture changed an unexpected size")
    if sha256(fixture) != FIXTURE_SHA256:
        raise RuntimeError("AVIF trailing-payload fixture differs from its pinned hash")
    return fixture


def verify_pillow(source: bytes, fixture: bytes) -> None:
    """Require pinned Pillow to verify and decode the exact baseline result."""
    from PIL import Image, __version__ as PILLOW_VERSION, _avif, features

    if PILLOW_VERSION != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PILLOW_VERSION}")
    avif_version = features.version("avif")
    if avif_version != "1.4.1":
        raise RuntimeError(f"libavif 1.4.1 is required, found {avif_version}")
    codec_versions = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codec_versions:
            raise RuntimeError(
                f"AVIF oracle requires {expected}, found {codec_versions}"
            )

    with Image.open(BytesIO(fixture)) as image:
        image.verify()

    def public_result(data: bytes) -> tuple[object, ...]:
        with Image.open(BytesIO(data)) as image:
            image.load()
            return (
                image.format,
                image.mode,
                image.size,
                image.info.copy(),
                image.tobytes(),
            )

    if public_result(fixture) != public_result(source):
        raise RuntimeError("trailing-payload Pillow result differs from the baseline")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the tracked fixture without writing it",
    )
    args = parser.parse_args()

    source = SOURCE.read_bytes()
    fixture = build_fixture(source)
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_bytes() != fixture:
            raise RuntimeError(
                f"tracked fixture does not match the pinned mutation: {OUTPUT}"
            )
        print(f"Verified existing fixture: {OUTPUT}")
        return

    verify_pillow(source, fixture)
    if OUTPUT.exists():
        if OUTPUT.read_bytes() != fixture:
            raise RuntimeError(f"refusing to replace a different fixture at {OUTPUT}")
        print(f"Verified existing fixture: {OUTPUT}")
    else:
        OUTPUT.write_bytes(fixture)
        print(f"Wrote verified fixture: {OUTPUT}")


if __name__ == "__main__":
    main()
