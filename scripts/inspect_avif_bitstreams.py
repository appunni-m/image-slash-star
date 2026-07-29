#!/usr/bin/env python3
"""Report AVIF item extents and track samples from committed fixture bytes.

This is an independent ISO-BMFF boundary oracle for the portable AVIF parser.
It intentionally does not import Pillow, call libavif, or reuse Rust output.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).parent.parent
DEFAULT_INPUT = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
ALPHA_URNS = {
    b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha",
    b"urn:mpeg:hevc:2015:auxid:1",
}


@dataclass(frozen=True)
class Box:
    kind: bytes
    start: int
    size: int
    header_size: int

    @property
    def payload_start(self) -> int:
        return self.start + self.header_size

    @property
    def end(self) -> int:
        return self.start + self.size


class Reader:
    def __init__(self, data: bytes, start: int, end: int):
        if start < 0 or start > end or end > len(data):
            raise ValueError("reader bounds exceed input")
        self.data = data
        self.offset = start
        self.end = end

    def take(self, size: int) -> bytes:
        end = self.offset + size
        if size < 0 or end > self.end:
            raise ValueError("truncated field")
        value = self.data[self.offset:end]
        self.offset = end
        return value

    def uint(self, size: int) -> int:
        if size == 0:
            return 0
        if size not in (1, 2, 4, 8):
            raise ValueError(f"unsupported integer width: {size}")
        return int.from_bytes(self.take(size), "big")

    def u8(self) -> int:
        return self.uint(1)

    def u16(self) -> int:
        return self.uint(2)

    def u32(self) -> int:
        return self.uint(4)

    def u64(self) -> int:
        return self.uint(8)

    def c_string(self) -> bytes:
        terminator = self.data.find(b"\0", self.offset, self.end)
        if terminator < 0:
            raise ValueError("unterminated string")
        value = self.data[self.offset:terminator]
        self.offset = terminator + 1
        return value


def parse_boxes(data: bytes, start: int, end: int) -> list[Box]:
    boxes = []
    offset = start
    while offset < end:
        if end - offset < 8:
            raise ValueError(f"truncated box header at {offset}")
        size = struct.unpack_from(">I", data, offset)[0]
        kind = data[offset + 4 : offset + 8]
        header_size = 8
        if size == 1:
            if end - offset < 16:
                raise ValueError(f"truncated large box header at {offset}")
            size = struct.unpack_from(">Q", data, offset + 8)[0]
            header_size = 16
        if kind == b"uuid":
            header_size += 16
        if size == 0:
            size = end - offset
        if size < header_size or size > end - offset:
            raise ValueError(f"invalid {display_fourcc(kind)} box size at {offset}")
        boxes.append(Box(kind, offset, size, header_size))
        offset += size
    return boxes


def children(data: bytes, parent: Box, prefix: int = 0) -> list[Box]:
    return parse_boxes(data, parent.payload_start + prefix, parent.end)


def unique_box(boxes: list[Box], kind: bytes, required: bool = True) -> Box | None:
    matches = [box for box in boxes if box.kind == kind]
    if len(matches) > 1:
        raise ValueError(f"duplicate {display_fourcc(kind)} box")
    if required and not matches:
        raise ValueError(f"missing {display_fourcc(kind)} box")
    return matches[0] if matches else None


def full_box_reader(data: bytes, box: Box) -> tuple[int, int, Reader]:
    reader = Reader(data, box.payload_start, box.end)
    raw = reader.u32()
    return raw >> 24, raw & 0x00FF_FFFF, reader


def display_fourcc(value: bytes) -> str:
    return value.decode("latin-1")


def hash_spans(data: bytes, spans: list[tuple[int, int]]) -> str:
    digest = hashlib.sha256()
    for start, end in spans:
        digest.update(data[start:end])
    return digest.hexdigest()


def parse_ftyp(data: bytes, box: Box) -> dict[str, object]:
    reader = Reader(data, box.payload_start, box.end)
    major = reader.take(4)
    minor = reader.u32()
    compatible = []
    while reader.offset < reader.end:
        compatible.append(reader.take(4))
    return {
        "major": display_fourcc(major),
        "minor": minor,
        "compatible": [display_fourcc(brand) for brand in compatible],
    }


def parse_pitm(data: bytes, box: Box) -> int:
    version, _, reader = full_box_reader(data, box)
    item_id = reader.u16() if version == 0 else reader.u32()
    if item_id == 0:
        raise ValueError("zero primary item ID")
    return item_id


def parse_infe(data: bytes, box: Box) -> tuple[int, bytes]:
    version, _, reader = full_box_reader(data, box)
    if version == 2:
        item_id = reader.u16()
    elif version == 3:
        item_id = reader.u32()
    else:
        raise ValueError(f"unsupported infe version {version}")
    if item_id == 0:
        raise ValueError("zero item ID")
    reader.u16()
    kind = reader.take(4)
    reader.c_string()
    if kind == b"mime":
        reader.c_string()
    return item_id, kind


def parse_iinf(data: bytes, box: Box) -> dict[int, bytes]:
    version, _, reader = full_box_reader(data, box)
    if version == 0:
        entry_count = reader.u16()
    elif version == 1:
        entry_count = reader.u32()
    else:
        raise ValueError(f"unsupported iinf version {version}")
    entries = parse_boxes(data, reader.offset, reader.end)
    if len(entries) != entry_count or any(entry.kind != b"infe" for entry in entries):
        raise ValueError("iinf entry count/type mismatch")
    result = {}
    for entry in entries:
        item_id, kind = parse_infe(data, entry)
        if item_id in result:
            raise ValueError("duplicate infe item ID")
        result[item_id] = kind
    return result


def parse_iref(data: bytes, box: Box) -> list[tuple[bytes, int, int]]:
    version, _, reader = full_box_reader(data, box)
    if version > 1:
        return []
    references = []
    for child in parse_boxes(data, reader.offset, reader.end):
        values = Reader(data, child.payload_start, child.end)
        from_id = values.u16() if version == 0 else values.u32()
        count = values.u16()
        for _ in range(count):
            to_id = values.u16() if version == 0 else values.u32()
            if from_id == 0 or to_id == 0:
                raise ValueError("zero iref item ID")
            references.append((child.kind, from_id, to_id))
        if values.offset != values.end:
            raise ValueError("trailing iref bytes")
    return references


def parse_iprp_properties(
    data: bytes, box: Box
) -> tuple[set[int], dict[int, dict[str, object]]]:
    iprp_children = children(data, box)
    ipco = unique_box(iprp_children, b"ipco")
    ipma = unique_box(iprp_children, b"ipma")
    assert ipco is not None and ipma is not None

    alpha_properties = set()
    av1c_properties = {}
    for index, prop in enumerate(children(data, ipco), start=1):
        if prop.kind == b"av1C":
            payload = data[prop.payload_start : prop.end]
            av1c_properties[index] = {
                "offset": prop.payload_start,
                "length": len(payload),
                "sha256": hashlib.sha256(payload).hexdigest(),
                "hex": payload.hex(),
            }
        if prop.kind not in (b"auxC", b"auxi"):
            continue
        version, _, reader = full_box_reader(data, prop)
        if version != 0:
            raise ValueError("unsupported auxiliary property version")
        if reader.c_string() in ALPHA_URNS:
            alpha_properties.add(index)

    version, flags, reader = full_box_reader(data, ipma)
    wide = flags & 1
    entry_count = reader.u32()
    result = set()
    configs = {}
    previous_id = 0
    for _ in range(entry_count):
        item_id = reader.u16() if version == 0 else reader.u32()
        if item_id <= previous_id:
            raise ValueError("ipma IDs are not strictly increasing")
        previous_id = item_id
        for _ in range(reader.u8()):
            raw = reader.u16() if wide else reader.u8()
            index = raw & (0x7FFF if wide else 0x7F)
            if index in alpha_properties:
                result.add(item_id)
            if index in av1c_properties:
                if item_id in configs:
                    raise ValueError("multiple av1C properties for one item")
                configs[item_id] = av1c_properties[index]
    if reader.offset != reader.end:
        raise ValueError("trailing ipma bytes")
    return result, configs


def parse_iloc(
    data: bytes, box: Box, idat: Box | None
) -> dict[int, tuple[str, list[tuple[int, int]]]]:
    version, _, reader = full_box_reader(data, box)
    if version > 2:
        raise ValueError(f"unsupported iloc version {version}")
    sizes = reader.u16()
    offset_size = sizes >> 12
    length_size = (sizes >> 8) & 0xF
    base_offset_size = (sizes >> 4) & 0xF
    index_size = sizes & 0xF if version in (1, 2) else 0
    for size in (offset_size, length_size, base_offset_size, index_size):
        if size not in (0, 4, 8):
            raise ValueError(f"unsupported iloc integer width {size}")

    item_count = reader.u16() if version < 2 else reader.u32()
    locations = {}
    for _ in range(item_count):
        item_id = reader.u16() if version < 2 else reader.u32()
        if item_id == 0 or item_id in locations:
            raise ValueError("zero or duplicate iloc item ID")
        method = 0
        if version in (1, 2):
            construction = reader.u16()
            if construction >> 4:
                raise ValueError("nonzero iloc reserved bits")
            method = construction & 0xF
        if method not in (0, 1):
            raise ValueError(f"unsupported iloc construction method {method}")
        if reader.u16() != 0:
            raise ValueError("external iloc data reference")
        base_offset = reader.uint(base_offset_size)
        extents = []
        for _ in range(reader.u16()):
            if index_size:
                reader.uint(index_size)
            extent_offset = reader.uint(offset_size)
            extent_length = reader.uint(length_size)
            relative = base_offset + extent_offset
            if method == 0:
                start = relative
            else:
                if idat is None:
                    raise ValueError("idat-backed item without idat")
                start = idat.payload_start + relative
            end = start + extent_length
            limit = len(data) if method == 0 else idat.end
            if start < 0 or start > end or end > limit:
                raise ValueError("item extent exceeds source bytes")
            extents.append((start, end))
        locations[item_id] = ("file" if method == 0 else "idat", extents)
    if reader.offset != reader.end:
        raise ValueError("trailing iloc bytes")
    return locations


def item_record(
    data: bytes,
    item_id: int,
    item_types: dict[int, bytes],
    locations: dict[int, tuple[str, list[tuple[int, int]]]],
    configs: dict[int, dict[str, object]],
) -> dict[str, object]:
    construction, spans = locations[item_id]
    return {
        "item_id": item_id,
        "type": display_fourcc(item_types[item_id]),
        "construction": construction,
        "spans": [
            {"offset": start, "length": end - start} for start, end in spans
        ],
        "length": sum(end - start for start, end in spans),
        "sha256": hash_spans(data, spans),
        "av1c": configs.get(item_id),
    }


def parse_items(data: bytes, meta: Box) -> dict[str, object]:
    boxes = children(data, meta, prefix=4)
    pitm = unique_box(boxes, b"pitm")
    iinf = unique_box(boxes, b"iinf")
    iprp = unique_box(boxes, b"iprp")
    iloc = unique_box(boxes, b"iloc")
    idat = unique_box(boxes, b"idat", required=False)
    iref = unique_box(boxes, b"iref", required=False)
    assert pitm is not None and iinf is not None and iprp is not None and iloc is not None

    primary = parse_pitm(data, pitm)
    item_types = parse_iinf(data, iinf)
    references = parse_iref(data, iref) if iref is not None else []
    alpha_items, configs = parse_iprp_properties(data, iprp)
    locations = parse_iloc(data, iloc, idat)

    if primary not in item_types or primary not in locations:
        raise ValueError("primary item lacks type or location")
    if item_types[primary] == b"av01":
        color_ids = [primary]
    elif item_types[primary] == b"grid":
        color_ids = [
            to_id
            for kind, from_id, to_id in references
            if kind == b"dimg" and from_id == primary
        ]
        if not color_ids:
            raise ValueError("grid primary item has no dimg children")
    else:
        raise ValueError("primary item is not AV1 or grid")

    alpha_ids = []
    for color_id in color_ids:
        matches = [
            from_id
            for kind, from_id, to_id in references
            if kind == b"auxl" and to_id == color_id and from_id in alpha_items
        ]
        if len(matches) > 1:
            raise ValueError("multiple alpha items target one color item")
        alpha_ids.extend(matches)

    records = {
        item_id: item_record(data, item_id, item_types, locations, configs)
        for item_id in dict.fromkeys(color_ids + alpha_ids)
    }
    return {
        "primary_item_id": primary,
        "color": [records[item_id] for item_id in color_ids],
        "alpha": [records[item_id] for item_id in alpha_ids],
    }


def parse_tkhd(data: bytes, box: Box) -> int:
    version, _, reader = full_box_reader(data, box)
    if version == 0:
        reader.take(8)
        return reader.u32()
    if version == 1:
        reader.take(16)
        return reader.u32()
    raise ValueError(f"unsupported tkhd version {version}")


def parse_mdhd(data: bytes, box: Box) -> int:
    version, _, reader = full_box_reader(data, box)
    if version == 0:
        reader.take(8)
        timescale = reader.u32()
    elif version == 1:
        reader.take(16)
        timescale = reader.u32()
    else:
        raise ValueError(f"unsupported mdhd version {version}")
    if timescale == 0:
        raise ValueError("zero media timescale")
    return timescale


def parse_hdlr(data: bytes, box: Box) -> bytes:
    version, _, reader = full_box_reader(data, box)
    if version != 0 or reader.u32() != 0:
        raise ValueError("invalid handler")
    return reader.take(4)


def parse_tref(data: bytes, box: Box | None) -> int | None:
    if box is None:
        return None
    auxl = unique_box(children(data, box), b"auxl", required=False)
    if auxl is None:
        return None
    reader = Reader(data, auxl.payload_start, auxl.end)
    value = reader.u32()
    if value == 0:
        raise ValueError("zero track reference")
    return value


def parse_u32_table(data: bytes, box: Box, width: int) -> list[int]:
    version, _, reader = full_box_reader(data, box)
    if version != 0:
        raise ValueError(f"unsupported {display_fourcc(box.kind)} version")
    values = [reader.uint(width) for _ in range(reader.u32())]
    if reader.offset != reader.end:
        raise ValueError(f"trailing {display_fourcc(box.kind)} bytes")
    return values


def parse_stsc(data: bytes, box: Box) -> list[tuple[int, int, int]]:
    version, _, reader = full_box_reader(data, box)
    if version != 0:
        raise ValueError("unsupported stsc version")
    entries = []
    for _ in range(reader.u32()):
        entry = (reader.u32(), reader.u32(), reader.u32())
        if not entries and entry[0] != 1:
            raise ValueError("stsc does not start at chunk one")
        if entries and entry[0] <= entries[-1][0]:
            raise ValueError("stsc chunks are not strictly increasing")
        entries.append(entry)
    if reader.offset != reader.end:
        raise ValueError("trailing stsc bytes")
    return entries


def parse_stsz(data: bytes, box: Box) -> list[int]:
    version, _, reader = full_box_reader(data, box)
    if version != 0:
        raise ValueError("unsupported stsz version")
    common_size = reader.u32()
    count = reader.u32()
    sizes = [common_size] * count if common_size else [reader.u32() for _ in range(count)]
    if reader.offset != reader.end:
        raise ValueError("trailing stsz bytes")
    if any(size == 0 for size in sizes):
        raise ValueError("zero AV1 sample size")
    return sizes


def parse_stts(data: bytes, box: Box | None) -> list[tuple[int, int]]:
    if box is None:
        return []
    version, _, reader = full_box_reader(data, box)
    if version != 0:
        raise ValueError("unsupported stts version")
    entries = [(reader.u32(), reader.u32()) for _ in range(reader.u32())]
    if reader.offset != reader.end:
        raise ValueError("trailing stts bytes")
    return entries


def sample_durations(entries: list[tuple[int, int]], count: int) -> list[int]:
    if not entries:
        return [1] * count
    durations = []
    for sample_count, delta in entries:
        durations.extend([delta] * min(sample_count, count - len(durations)))
        if len(durations) == count:
            break
    if len(durations) < count:
        durations.extend([entries[-1][1]] * (count - len(durations)))
    return durations


def parse_stsd_config(data: bytes, box: Box) -> dict[str, object] | None:
    version, _, reader = full_box_reader(data, box)
    if version not in (0, 1):
        raise ValueError("unsupported stsd version")
    for sample in parse_boxes(data, reader.offset + 4, reader.end):
        if sample.kind != b"av01":
            continue
        properties = parse_boxes(data, sample.payload_start + 78, sample.end)
        av1c = unique_box(properties, b"av1C", required=False)
        if av1c is not None:
            payload = data[av1c.payload_start : av1c.end]
            return {
                "offset": av1c.payload_start,
                "length": len(payload),
                "sha256": hashlib.sha256(payload).hexdigest(),
                "hex": payload.hex(),
            }
    return None


def parse_track(data: bytes, box: Box) -> dict[str, object]:
    track_boxes = children(data, box)
    tkhd = unique_box(track_boxes, b"tkhd")
    tref = unique_box(track_boxes, b"tref", required=False)
    mdia = unique_box(track_boxes, b"mdia")
    assert tkhd is not None and mdia is not None

    mdia_boxes = children(data, mdia)
    mdhd = unique_box(mdia_boxes, b"mdhd")
    hdlr = unique_box(mdia_boxes, b"hdlr")
    minf = unique_box(mdia_boxes, b"minf")
    assert mdhd is not None and hdlr is not None and minf is not None
    stbl = unique_box(children(data, minf), b"stbl")
    assert stbl is not None
    table = children(data, stbl)

    stco = unique_box(table, b"stco", required=False)
    co64 = unique_box(table, b"co64", required=False)
    if (stco is None) == (co64 is None):
        raise ValueError("track must have exactly one stco/co64")
    offsets = parse_u32_table(data, stco, 4) if stco else parse_u32_table(data, co64, 8)
    stsc = unique_box(table, b"stsc")
    stsz = unique_box(table, b"stsz")
    stsd = unique_box(table, b"stsd")
    stss = unique_box(table, b"stss", required=False)
    stts = unique_box(table, b"stts", required=False)
    assert stsc is not None and stsz is not None and stsd is not None

    mappings = parse_stsc(data, stsc)
    sizes = parse_stsz(data, stsz)
    sync_numbers = set(parse_u32_table(data, stss, 4)) if stss else set()
    durations = sample_durations(parse_stts(data, stts), len(sizes))
    samples = []
    sample_index = 0
    for chunk_index, chunk_offset in enumerate(offsets, start=1):
        applicable = [entry for entry in mappings if entry[0] <= chunk_index]
        if not applicable or applicable[-1][1] == 0:
            raise ValueError("chunk has no samples")
        sample_offset = chunk_offset
        for _ in range(applicable[-1][1]):
            if sample_index >= len(sizes):
                raise ValueError("sample table has fewer sizes than samples")
            size = sizes[sample_index]
            end = sample_offset + size
            if end > len(data):
                raise ValueError("track sample exceeds input")
            samples.append(
                {
                    "index": sample_index,
                    "offset": sample_offset,
                    "length": size,
                    "sync": sample_index == 0 or sample_index + 1 in sync_numbers,
                    "duration": durations[sample_index],
                    "sha256": hashlib.sha256(data[sample_offset:end]).hexdigest(),
                }
            )
            sample_offset = end
            sample_index += 1
    if sample_index != len(sizes):
        raise ValueError("sample table has more sizes than chunk samples")

    return {
        "track_id": parse_tkhd(data, tkhd),
        "handler": display_fourcc(parse_hdlr(data, hdlr)),
        "aux_for_track_id": parse_tref(data, tref),
        "timescale": parse_mdhd(data, mdhd),
        "av1c": parse_stsd_config(data, stsd),
        "samples": samples,
    }


def inspect(path: Path) -> dict[str, object]:
    data = path.read_bytes()
    top = parse_boxes(data, 0, len(data))
    ftyp = unique_box(top, b"ftyp")
    meta = unique_box(top, b"meta", required=False)
    moov = unique_box(top, b"moov", required=False)
    assert ftyp is not None
    result: dict[str, object] = {
        "file": path.name,
        "length": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
        "ftyp": parse_ftyp(data, ftyp),
    }
    if meta is not None:
        result["items"] = parse_items(data, meta)
    if moov is not None:
        tracks = [parse_track(data, box) for box in children(data, moov) if box.kind == b"trak"]
        result["tracks"] = tracks
        main = next(
            (track for track in tracks if track["handler"] in ("pict", "vide")),
            None,
        )
        if main is not None:
            result["color_track_id"] = main["track_id"]
            result["alpha_track_id"] = next(
                (
                    track["track_id"]
                    for track in tracks
                    if track["handler"] == "auxv"
                    and track["aux_for_track_id"] == main["track_id"]
                ),
                None,
            )
    return result


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "paths",
        nargs="*",
        type=Path,
        help="AVIF files; defaults to every committed AVIF fixture",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    paths = args.paths or sorted(DEFAULT_INPUT.glob("*.avif"))
    reports = []
    for path in paths:
        try:
            reports.append(inspect(path))
        except ValueError as error:
            reports.append({"file": path.name, "error": str(error)})
    print(json.dumps(reports, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
