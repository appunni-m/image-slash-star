#!/usr/bin/env python3
"""Generate complete AVIF parity fixtures for item-information parsing.

The wrong-child fixture changes the four-byte box type of the sole ``iinf``
child. The version-1 fixture widens the entry count from 16 to 32 bits while
preserving the complete still image and its file-backed item extent.
"""

from __future__ import annotations

import argparse
import hashlib
import struct
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURE_DIR = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = FIXTURE_DIR / "baseline.avif"
OUTPUT = FIXTURE_DIR / "iinf_child_not_infe.avif"
VERSION_1_OUTPUT = FIXTURE_DIR / "iinf_version_1_entry_count.avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
FIXTURE_SHA256 = "ee95c49866ddcf2bf1fd4f2f3a0e4c1d2c768bb5ccbf8f675555691298225d76"
VERSION_1_FIXTURE_SHA256 = (
    "4d5ef8f6f7e8934c2f7ab0ba4e7d6af52b23f250c6f52a78eca6323f97bfea1a"
)
EXPECTED_PILLOW_ERROR = "cannot identify image file <bytes>"


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


def iinf_child_type_offset(source: bytes) -> int:
    """Validate the pinned one-entry layout and return its child type offset."""
    top_level = box_ranges(source, 0, len(source))
    if [box[0] for box in top_level] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("baseline AVIF top-level box layout changed")

    _, _, meta_payload, meta_end = top_level[1]
    if (
        meta_end - meta_payload < 4
        or source[meta_payload : meta_payload + 4] != bytes(4)
    ):
        raise RuntimeError("baseline AVIF meta must use a version-zero FullBox")

    meta_children = box_ranges(source, meta_payload + 4, meta_end)
    iinf_boxes = [box for box in meta_children if box[0] == b"iinf"]
    if len(iinf_boxes) != 1:
        raise RuntimeError("baseline AVIF meta must contain one iinf box")

    _, _, iinf_payload, iinf_end = iinf_boxes[0]
    if (
        iinf_end - iinf_payload < 6
        or source[iinf_payload : iinf_payload + 4] != bytes(4)
    ):
        raise RuntimeError("baseline AVIF iinf must use version zero")
    entry_count = struct.unpack_from(">H", source, iinf_payload + 4)[0]
    if entry_count != 1:
        raise RuntimeError("baseline AVIF iinf must declare exactly one entry")

    children = box_ranges(source, iinf_payload + 6, iinf_end)
    if len(children) != entry_count or children[0][0] != b"infe":
        raise RuntimeError("baseline AVIF iinf must contain one infe child")
    _, child_start, _, child_end = children[0]
    if child_end - child_start < 8:
        raise RuntimeError("baseline AVIF infe child header is truncated")
    return child_start + 4


def build_fixture(source: bytes) -> bytes:
    """Change only the checked ``infe`` fourcc to ``free`` and pin the result."""
    if sha256(source) != SOURCE_SHA256:
        raise RuntimeError("baseline AVIF source differs from its pinned hash")

    type_offset = iinf_child_type_offset(source)
    if source[type_offset : type_offset + 4] != b"infe":
        raise RuntimeError("baseline AVIF iinf child is no longer infe")

    fixture = bytearray(source)
    fixture[type_offset : type_offset + 4] = b"free"
    result = bytes(fixture)
    changed_offsets = [
        offset
        for offset, (before, after) in enumerate(zip(source, result))
        if before != after
    ]
    if (
        len(result) != len(source)
        or source[:type_offset] != result[:type_offset]
        or source[type_offset : type_offset + 4] != b"infe"
        or result[type_offset : type_offset + 4] != b"free"
        or source[type_offset + 4 :] != result[type_offset + 4 :]
        or changed_offsets != list(range(type_offset, type_offset + 3))
    ):
        raise RuntimeError("AVIF iinf mutation changed bytes outside its child type")
    if sha256(result) != FIXTURE_SHA256:
        raise RuntimeError("AVIF wrong-child-kind fixture differs from its pinned hash")
    return result


def build_version_1_fixture(source: bytes) -> bytes:
    """Widen the sole ``iinf`` entry count and keep its ``mdat`` extent valid."""
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
    iinf_indexes = [
        index for index, (kind, _) in enumerate(meta_children) if kind == b"iinf"
    ]
    iloc_indexes = [
        index for index, (kind, _) in enumerate(meta_children) if kind == b"iloc"
    ]
    if len(iinf_indexes) != 1 or len(iloc_indexes) != 1:
        raise RuntimeError("baseline AVIF meta must contain one iinf and one iloc")

    iinf_index = iinf_indexes[0]
    iinf = meta_children[iinf_index][1]
    if len(iinf) < 6 or iinf[:4] != bytes(4):
        raise RuntimeError("baseline AVIF iinf must use a version-zero FullBox")
    if struct.unpack_from(">H", iinf, 4)[0] != 1:
        raise RuntimeError("baseline AVIF iinf must declare exactly one entry")
    if [box[0] for box in box_ranges(iinf, 6, len(iinf))] != [b"infe"]:
        raise RuntimeError("baseline AVIF iinf must contain one infe child")
    meta_children[iinf_index] = (
        b"iinf",
        b"\x01" + iinf[1:4] + struct.pack(">I", 1) + iinf[6:],
    )

    iloc_index = iloc_indexes[0]
    iloc = bytearray(meta_children[iloc_index][1])
    expected_prefix = bytes.fromhex("0000000044000001000100000001")
    if len(iloc) != 22 or iloc[:14] != expected_prefix:
        raise RuntimeError("baseline AVIF iloc is not one 32-bit file-backed extent")
    old_extent = struct.unpack_from(">I", iloc, 14)[0]
    struct.pack_into(">I", iloc, 14, old_extent + 2)
    meta_children[iloc_index] = (b"iloc", bytes(iloc))

    meta_payload = source[meta_payload_start : meta_payload_start + 4]
    root_children = [
        (kind, source[payload_start:box_end])
        for kind, _, payload_start, box_end in top_level
    ]
    root_children[1] = (b"meta", meta_payload + pack_boxes(meta_children))
    result = pack_boxes(root_children)
    if len(result) != len(source) + 2:
        raise RuntimeError(
            "version-1 iinf fixture changed an unexpected number of bytes"
        )
    if sha256(result) != VERSION_1_FIXTURE_SHA256:
        raise RuntimeError("AVIF version-1 iinf fixture differs from its pinned hash")
    return result


def verify_pillow(fixture: bytes) -> None:
    """Check the exact pinned Pillow and libavif public open failure."""
    from PIL import (
        Image,
        UnidentifiedImageError,
        __version__ as PILLOW_VERSION,
        _avif,
        features,
    )

    from generate_decode_refs import stable_error_message

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

    try:
        image = Image.open(BytesIO(fixture))
    except UnidentifiedImageError as error:
        if stable_error_message(str(error)) != EXPECTED_PILLOW_ERROR:
            raise RuntimeError(
                "Pillow wrong-child error differs from its pinned result"
            ) from error
        return
    image.close()
    raise RuntimeError("Pillow unexpectedly accepted an iinf child other than infe")


def verify_version_1_pillow(source: bytes, fixture: bytes) -> None:
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
        raise RuntimeError("version-1 iinf Pillow result differs from the baseline")


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
    version_1_fixture = build_version_1_fixture(source)
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_bytes() != fixture:
            raise RuntimeError(
                f"tracked fixture does not match the pinned mutation: {OUTPUT}"
            )
        if (
            not VERSION_1_OUTPUT.exists()
            or VERSION_1_OUTPUT.read_bytes() != version_1_fixture
        ):
            raise RuntimeError(
                "tracked fixture does not match the pinned mutation: "
                f"{VERSION_1_OUTPUT}"
            )
        print(f"Verified existing fixtures: {OUTPUT}, {VERSION_1_OUTPUT}")
        return

    verify_pillow(fixture)
    verify_version_1_pillow(source, version_1_fixture)
    for output, expected in (
        (OUTPUT, fixture),
        (VERSION_1_OUTPUT, version_1_fixture),
    ):
        if output.exists():
            if output.read_bytes() != expected:
                raise RuntimeError(
                    f"refusing to replace a different fixture at {output}"
                )
            print(f"Verified existing fixture: {output}")
        else:
            output.write_bytes(expected)
            print(f"Wrote verified fixture: {output}")


if __name__ == "__main__":
    main()
