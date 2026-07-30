#!/usr/bin/env python3
"""Append deterministic bytes to a single-item AVIF frame OBU.

This diagnostic helper updates the primary ``iloc`` extent, enclosing ``mdat``
box, and final AV1 OBU payload length together. It is used only to create
longer entropy-input prefixes for pinned-oracle mutation sweeps.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from inspect_avif_bitstreams import (
    children,
    full_box_reader,
    parse_boxes,
    parse_pitm,
    unique_box,
)


def read_leb128(data: bytes, offset: int, end: int) -> tuple[int, int]:
    """Read one bounded unsigned LEB128 value."""

    value = 0
    for index in range(8):
        if offset >= end:
            raise RuntimeError("truncated OBU length")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << (index * 7)
        if byte < 0x80:
            return value, offset
    raise RuntimeError("oversized OBU length")


def final_obu_length_field(sample: bytes) -> tuple[int, int, int]:
    """Return the final OBU size-field offset, width, and payload length."""

    offset = 0
    final: tuple[int, int, int] | None = None
    while offset < len(sample):
        header = sample[offset]
        offset += 1
        if header & 0x80 or header & 0x01:
            raise RuntimeError("invalid OBU header")
        if header & 0x04:
            if offset >= len(sample):
                raise RuntimeError("truncated OBU extension")
            offset += 1
        if not header & 0x02:
            raise RuntimeError("OBU must carry an explicit size field")
        size_offset = offset
        payload_length, payload_start = read_leb128(sample, offset, len(sample))
        end = payload_start + payload_length
        if end > len(sample):
            raise RuntimeError("OBU payload exceeds AV1 item")
        final = (size_offset, payload_start - size_offset, payload_length)
        offset = end
    if final is None:
        raise RuntimeError("AV1 item contains no OBU")
    return final


def primary_extent(data: bytes) -> tuple[int, int, int, int]:
    """Return primary extent start, length, length-field offset, and width."""

    top = parse_boxes(data, 0, len(data))
    meta = unique_box(top, b"meta")
    assert meta is not None
    meta_children = children(data, meta, prefix=4)
    pitm = unique_box(meta_children, b"pitm")
    iloc = unique_box(meta_children, b"iloc")
    assert pitm is not None and iloc is not None
    primary_item_id = parse_pitm(data, pitm)
    version, _, reader = full_box_reader(data, iloc)
    if version > 2:
        raise RuntimeError(f"unsupported iloc version {version}")
    sizes = reader.u16()
    offset_size = sizes >> 12
    length_size = (sizes >> 8) & 0xF
    base_offset_size = (sizes >> 4) & 0xF
    index_size = sizes & 0xF if version in (1, 2) else 0
    for size in (offset_size, length_size, base_offset_size, index_size):
        if size not in (0, 4, 8):
            raise RuntimeError(f"unsupported iloc field width {size}")
    item_count = reader.u16() if version < 2 else reader.u32()
    selected: tuple[int, int, int, int] | None = None
    for _ in range(item_count):
        item_id = reader.u16() if version < 2 else reader.u32()
        method = 0
        if version in (1, 2):
            method = reader.u16() & 0xF
        if method != 0 or reader.u16() != 0:
            raise RuntimeError("suffix helper requires file-backed iloc extents")
        base_offset = reader.uint(base_offset_size)
        extent_count = reader.u16()
        if item_id == primary_item_id and extent_count != 1:
            raise RuntimeError("primary item must have one extent")
        for _ in range(extent_count):
            if index_size:
                reader.uint(index_size)
            extent_offset = reader.uint(offset_size)
            length_field_offset = reader.offset
            extent_length = reader.uint(length_size)
            if item_id == primary_item_id:
                selected = (
                    base_offset + extent_offset,
                    extent_length,
                    length_field_offset,
                    length_size,
                )
    if selected is None:
        raise RuntimeError("primary item has no iloc extent")
    return selected


def main() -> None:
    """Append the requested suffix and update all enclosing lengths."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--count", type=int, required=True)
    parser.add_argument("--fill", type=lambda value: int(value, 0), default=0)
    args = parser.parse_args()
    if not 1 <= args.count <= 32:
        parser.error("--count must be in 1..=32")
    if not 0 <= args.fill <= 255:
        parser.error("--fill must be a byte")

    source = args.input.read_bytes()
    item_offset, item_length, length_field_offset, length_size = primary_extent(source)
    if item_offset + item_length != len(source):
        raise RuntimeError("primary AV1 item must end at end-of-file")
    sample = source[item_offset:]
    obu_length_offset, obu_length_width, obu_payload_length = final_obu_length_field(
        sample
    )
    if obu_length_width != 1 or obu_payload_length + args.count >= 128:
        raise RuntimeError("suffix helper requires a one-byte final OBU length")

    top = parse_boxes(source, 0, len(source))
    mdat = unique_box(top, b"mdat")
    assert mdat is not None
    if (
        mdat.header_size != 8
        or mdat.payload_start != item_offset
        or mdat.end != len(source)
    ):
        raise RuntimeError("primary item must be the complete final 32-bit mdat")

    output = bytearray(source)
    output[length_field_offset : length_field_offset + length_size] = (
        item_length + args.count
    ).to_bytes(length_size, "big")
    output[mdat.start : mdat.start + 4] = (mdat.size + args.count).to_bytes(4, "big")
    output[item_offset + obu_length_offset] = obu_payload_length + args.count
    output.extend([args.fill] * args.count)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(output)
    report = {
        "input": args.input.name,
        "input_sha256": hashlib.sha256(source).hexdigest(),
        "output": args.output.name,
        "output_sha256": hashlib.sha256(output).hexdigest(),
        "item_offset": item_offset,
        "old_item_length": item_length,
        "new_item_length": item_length + args.count,
        "final_obu_length_offset": item_offset + obu_length_offset,
        "old_final_obu_payload_length": obu_payload_length,
        "new_final_obu_payload_length": obu_payload_length + args.count,
        "suffix": [args.fill] * args.count,
    }
    print(json.dumps(report, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
