#!/usr/bin/env python3
"""Generate a complete AVIF whose declared iinf child has the wrong kind.

The fixture changes only the four-byte box type of the sole ``iinf`` child in
the pinned still image. Pillow 12.2.0 rejects the complete file during open.
"""

from __future__ import annotations

import argparse
import hashlib
import struct
from io import BytesIO
from pathlib import Path

from PIL import (
    Image,
    UnidentifiedImageError,
    __version__ as PILLOW_VERSION,
    _avif,
    features,
)

from generate_decode_refs import stable_error_message


ROOT = Path(__file__).resolve().parent.parent
FIXTURE_DIR = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = FIXTURE_DIR / "baseline.avif"
OUTPUT = FIXTURE_DIR / "iinf_child_not_infe.avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
FIXTURE_SHA256 = "ee95c49866ddcf2bf1fd4f2f3a0e4c1d2c768bb5ccbf8f675555691298225d76"
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


def verify_pillow(fixture: bytes) -> None:
    """Check the exact pinned Pillow and libavif public open failure."""
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


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the tracked fixture without writing it",
    )
    args = parser.parse_args()

    fixture = build_fixture(SOURCE.read_bytes())
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_bytes() != fixture:
            raise RuntimeError(
                f"tracked fixture does not match the pinned mutation: {OUTPUT}"
            )
        print(f"Verified existing fixture: {OUTPUT}")
        return

    verify_pillow(fixture)
    if OUTPUT.exists():
        if OUTPUT.read_bytes() != fixture:
            raise RuntimeError(f"refusing to replace a different fixture at {OUTPUT}")
        print(f"Verified existing fixture: {OUTPUT}")
        return
    OUTPUT.write_bytes(fixture)
    print(f"Wrote verified fixture: {OUTPUT}")


if __name__ == "__main__":
    main()
