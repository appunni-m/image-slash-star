#!/usr/bin/env python3
"""Add essential primary-item imir axes and trailing-byte AVIF fixtures.

The mutation changes only item metadata. It preserves the encoded AV1 samples
and rebases the primary item extent after the larger metadata box.
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/avif"
SOURCE = ASSETS / "baseline.avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
FIXTURES = {
    0: (
        "primary_item_imir_top_bottom.avif",
        "b129f71ceda1490258d8fbdbec9885ab5ea29a82e9a453aa000290e366e8f3df",
    ),
    1: (
        "primary_item_imir_left_right.avif",
        "5535e699843b31a97921316661e3cacb4826383638f88f0462a55da5aafe5d5d",
    ),
}
TRAILING_FIXTURE = (
    "primary_item_imir_trailing_byte.avif",
    "891bd77c35c719432f2632f87c0f5633aa91dacdc3c068cf99d5834c8c5fc9f1",
)
ILOC_SOURCE = bytes.fromhex("00000000440000010001000000010000011a00000aeb")
IPMA_SOURCE = bytes.fromhex("000000000000000100010401028304")
SOURCE_OFFSET = 0x11A
EXTENT_LENGTH = 0xAEB


def box_payload(
    boxes: list[tuple[bytes, bytes, bytes]], kind: bytes, context: str
) -> bytes:
    matches = [payload for box_kind, _, payload in boxes if box_kind == kind]
    if len(matches) != 1:
        raise RuntimeError(f"expected exactly one {kind!r} box in {context}")
    return matches[0]


def candidate_bytes(axis: int, *, trailing_byte: bool = False) -> bytes:
    source = SOURCE.read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("AVIF primary-item source differs from its pin")

    mirror_payload = bytes((axis,)) + (b"\xff" if trailing_byte else b"")
    mirror = pack_box(b"imir", mirror_payload)
    expected_box_length = 10 if trailing_byte else 9
    if len(mirror) != expected_box_length:
        raise RuntimeError("AVIF imir property has an unexpected box length")
    delta = len(mirror) + 1
    changed = {b"ipco": 0, b"ipma": 0, b"iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        kind = path[-1]
        if path == (b"meta", b"iprp", b"ipco"):
            properties = read_boxes(payload, 0, len(payload))
            if [property_kind for property_kind, _, _ in properties] != [
                b"ispe",
                b"pixi",
                b"av1C",
                b"colr",
            ]:
                raise RuntimeError("primary-item properties differ from the pinned layout")
            changed[kind] += 1
            return payload + mirror
        if path == (b"meta", b"iprp", b"ipma"):
            if payload != IPMA_SOURCE:
                raise RuntimeError("primary-item property associations differ from their pin")
            changed[kind] += 1
            return payload[:10] + b"\x05" + payload[11:] + b"\x85"
        if path == (b"meta", b"iloc"):
            if payload != ILOC_SOURCE:
                raise RuntimeError("primary-item extent differs from its pin")
            changed[kind] += 1
            new_offset = SOURCE_OFFSET + delta
            return payload[:14] + struct.pack(">I", new_offset) + payload[18:]
        return payload

    result = rewrite_boxes(source, transform)
    if changed != dict.fromkeys(changed, 1):
        raise RuntimeError("expected to update one ipco, ipma, and iloc box")
    if len(result) != len(source) + delta:
        raise RuntimeError("AVIF metadata mutation changed an unexpected byte count")

    source_top = read_boxes(source, 0, len(source))
    result_top = read_boxes(result, 0, len(result))
    source_media = box_payload(source_top, b"mdat", "source")
    result_media = box_payload(result_top, b"mdat", "mutated file")
    if result_media != source_media:
        raise RuntimeError("AVIF mutation changed encoded AV1 samples")

    meta = box_payload(result_top, b"meta", "mutated file")
    meta_children = read_boxes(meta, 4, len(meta))
    iloc = box_payload(meta_children, b"iloc", "meta")
    if iloc != ILOC_SOURCE[:14] + struct.pack(">I", SOURCE_OFFSET + delta) + ILOC_SOURCE[18:]:
        raise RuntimeError("mutated primary-item extent differs from the pinned layout")
    iprp = box_payload(meta_children, b"iprp", "meta")
    iprp_children = read_boxes(iprp, 0, len(iprp))
    ipco = box_payload(iprp_children, b"ipco", "iprp")
    properties = read_boxes(ipco, 0, len(ipco))
    if [property_kind for property_kind, _, _ in properties] != [
        b"ispe",
        b"pixi",
        b"av1C",
        b"colr",
        b"imir",
    ] or properties[-1][2] != mirror_payload:
        raise RuntimeError("mutated imir property has an unexpected index or axis")
    ipma = box_payload(iprp_children, b"ipma", "iprp")
    if ipma != IPMA_SOURCE[:10] + b"\x05" + IPMA_SOURCE[11:] + b"\x85":
        raise RuntimeError("mutated primary item does not associate essential property 5")
    extent_offset = int.from_bytes(iloc[14:18], "big")
    extent_length = int.from_bytes(iloc[18:22], "big")
    if (
        extent_offset != SOURCE_OFFSET + delta
        or extent_length != len(result_media)
        or result[extent_offset : extent_offset + extent_length] != result_media
    ):
        raise RuntimeError("primary-item extent no longer selects the unchanged mdat payload")

    expected_hash = TRAILING_FIXTURE[1] if trailing_byte else FIXTURES[axis][1]
    if hashlib.sha256(result).hexdigest() != expected_hash:
        raise RuntimeError("mutated AVIF differs from its pinned output hash")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify fixture bytes without writing files",
    )
    args = parser.parse_args()

    for axis, (filename, _) in FIXTURES.items():
        output = ASSETS / filename
        expected = candidate_bytes(axis)
        if args.check:
            if not output.is_file() or output.read_bytes() != expected:
                raise RuntimeError(f"{filename} differs from the maintained recipe")
        elif output.exists():
            if output.read_bytes() != expected:
                raise RuntimeError(
                    f"{filename} already exists with different bytes; refusing to overwrite"
                )
        else:
            output.write_bytes(expected)
        print(f"{filename}: {len(expected)} bytes, sha256={hashlib.sha256(expected).hexdigest()}")

    trailing_filename, _ = TRAILING_FIXTURE
    trailing_output = ASSETS / trailing_filename
    trailing_expected = candidate_bytes(0, trailing_byte=True)
    if args.check:
        if not trailing_output.is_file() or trailing_output.read_bytes() != trailing_expected:
            raise RuntimeError(f"{trailing_filename} differs from the maintained recipe")
    elif trailing_output.exists():
        if trailing_output.read_bytes() != trailing_expected:
            raise RuntimeError(
                f"{trailing_filename} already exists with different bytes; refusing to overwrite"
            )
    else:
        trailing_output.write_bytes(trailing_expected)
    print(
        f"{trailing_filename}: {len(trailing_expected)} bytes, "
        f"sha256={hashlib.sha256(trailing_expected).hexdigest()}"
    )


if __name__ == "__main__":
    main()
