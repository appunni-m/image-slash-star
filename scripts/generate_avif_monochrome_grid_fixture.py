#!/usr/bin/env python3
"""Generate and verify a lossless monochrome AVIF grid with a clipped last cell.

The fixture uses the pinned valid grid.avif metadata layout, two distinct
64x64 monochrome-plus-alpha AVIF cell items encoded by Pillow, and a 73x64
canvas that exposes nine columns from the right-hand cell.
"""

from __future__ import annotations

import hashlib
import io
import struct
from pathlib import Path

import yaml
from PIL import Image, __version__ as PILLOW_VERSION, features

import generate_decode_refs as oracle
from avif_bmff_boxes import pack_box
from inspect_avif_bitstreams import (
    children,
    full_box_reader,
    parse_boxes,
    parse_iinf,
    parse_iloc,
    parse_items,
    parse_iref,
    parse_pitm,
    unique_box,
)

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/avif/grid.avif"
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/avif/portable_lossless_monochrome_grid_73x64.avif"
)
MANIFEST = ROOT / "manifest.yaml"

EXPECTED_SOURCE_SHA256 = "bae56368b348b1d847e2bfb662522599f0c63dfe62fb68826c9e42a300ff405d"
EXPECTED_FIXTURE_SHA256 = "3e69167291d846470673b297e9a30e51d7027e182918b0a8c1d95f24719e9828"
EXPECTED_PIXELS_SHA256 = "a553e8c265e29b0644101cff0da8cb54ac351ac4541b86d095ad960445cea987"
CELL_SIZE = (64, 64)
GRID_SIZE = (73, 64)
GRID_DESCRIPTOR = bytes.fromhex("0000000100490040")
MONO_AV1C = bytes.fromhex("81001c00")
CELL_ORDER = (1, 5, 6, 2, 3)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def cell_pixels(variant: int) -> bytes:
    """Return deterministic grayscale RGBA samples for one AV1 grid cell."""
    pixels = bytearray()
    for y in range(CELL_SIZE[1]):
        for x in range(CELL_SIZE[0]):
            luma = (x * 13 + y * 27 + ((x * y + variant * 91) % 113)) % 256
            alpha = 160 + ((x * 3 + y * 5 + variant * 29) % 96)
            pixels.extend((luma, luma, luma, alpha))
    return bytes(pixels)


def encode_cell(variant: int) -> tuple[bytes, bytes, bytes]:
    """Encode one lossless 4:0:0 color item and auxiliary alpha item."""
    pixels = cell_pixels(variant)
    image = Image.frombytes("RGBA", CELL_SIZE, pixels)

    def encode() -> bytes:
        output = io.BytesIO()
        image.save(
            output,
            format="AVIF",
            quality=100,
            speed=6,
            max_threads=1,
            subsampling="4:0:0",
            range="full",
            autotiling=False,
        )
        return output.getvalue()

    first = encode()
    second = encode()
    if first != second:
        raise RuntimeError(f"AVIF grid cell {variant} encoding is not deterministic")

    with Image.open(io.BytesIO(first)) as check:
        check.verify()
    with Image.open(io.BytesIO(first)) as check:
        check.load()
        if check.format != "AVIF" or check.mode != "RGBA":
            raise RuntimeError(f"AVIF grid cell {variant} has an unexpected Pillow mode")
        if check.size != CELL_SIZE or check.tobytes() != pixels:
            raise RuntimeError(f"AVIF grid cell {variant} does not roundtrip exactly")

    top = parse_boxes(first, 0, len(first))
    meta = unique_box(top, b"meta")
    if meta is None:
        raise RuntimeError(f"AVIF grid cell {variant} is missing meta")
    meta_boxes = children(first, meta, prefix=4)
    pitm = unique_box(meta_boxes, b"pitm")
    iinf = unique_box(meta_boxes, b"iinf")
    iloc = unique_box(meta_boxes, b"iloc")
    iref = unique_box(meta_boxes, b"iref")
    iprp = unique_box(meta_boxes, b"iprp")
    if any(box is None for box in (pitm, iinf, iloc, iref, iprp)):
        raise RuntimeError(f"AVIF grid cell {variant} metadata boxes are incomplete")

    primary = parse_pitm(first, pitm)
    item_types = parse_iinf(first, iinf)
    references = parse_iref(first, iref)
    if item_types.get(primary) != b"av01":
        raise RuntimeError(f"AVIF grid cell {variant} primary item is not AV1")
    alpha_ids = [
        from_id
        for kind, from_id, to_id in references
        if kind == b"auxl" and to_id == primary
    ]
    if len(alpha_ids) != 1 or item_types.get(alpha_ids[0]) != b"av01":
        raise RuntimeError(f"AVIF grid cell {variant} lacks one AV1 alpha item")

    _, codec_configs = parse_ipma_properties(first, iprp)
    for item_id in (primary, alpha_ids[0]):
        config = codec_configs.get(item_id)
        if config is None or config["hex"] != MONO_AV1C.hex():
            raise RuntimeError(f"AVIF grid cell {variant} is not configured as monochrome")

    locations = parse_iloc(first, iloc, idat=None)

    def item_payload(item_id: int) -> bytes:
        construction, spans = locations[item_id]
        if construction != "file" or len(spans) != 1:
            raise RuntimeError(f"AVIF grid cell {variant} item {item_id} is not one file extent")
        start, end = spans[0]
        payload = first[start:end]
        if not payload:
            raise RuntimeError(f"AVIF grid cell {variant} item {item_id} is empty")
        return payload

    color_payload = item_payload(primary)
    alpha_payload = item_payload(alpha_ids[0])
    return color_payload, alpha_payload, pixels


def parse_ipma_properties(
    data: bytes, iprp: object
) -> tuple[set[int], dict[int, dict[str, object]]]:
    """Import the property-association parser through the shared inspector."""
    from inspect_avif_bitstreams import parse_iprp_properties

    return parse_iprp_properties(data, iprp)


def template_properties(template: bytes, meta: object) -> list[object]:
    meta_children = children(template, meta, prefix=4)
    iprp = unique_box(meta_children, b"iprp")
    if iprp is None:
        raise RuntimeError("source AVIF grid is missing iprp")
    ipco = unique_box(children(template, iprp), b"ipco")
    if ipco is None:
        raise RuntimeError("source AVIF grid is missing ipco")
    properties = children(template, ipco)
    if [prop.kind for prop in properties] != [
        b"ispe",
        b"pixi",
        b"colr",
        b"ispe",
        b"av1C",
        b"pixi",
        b"auxC",
        b"av1C",
    ]:
        raise RuntimeError("source AVIF grid item properties differ from the pinned layout")
    return properties


def rewrite_ipma(template: bytes, patched: bytearray, ipma: object) -> None:
    version, flags, reader = full_box_reader(template, ipma)
    if version != 0 or flags != 0:
        raise RuntimeError("source AVIF grid ipma differs from the pinned version-zero layout")
    entry_count = reader.u32()
    if entry_count != 5:
        raise RuntimeError("source AVIF grid ipma does not have five entries")

    changed = set()
    for _ in range(entry_count):
        item_id = reader.u16()
        association_count = reader.u8()
        for association_index in range(association_count):
            association_offset = reader.offset
            raw = reader.u8()
            if item_id in (1, 2, 3) and association_index == 1:
                if raw != 2:
                    raise RuntimeError("source AVIF grid pixi association differs from the pin")
                patched[association_offset] = 6
                changed.add(item_id)
    if reader.offset != reader.end or changed != {1, 2, 3}:
        raise RuntimeError("source AVIF grid property associations are incomplete")


def rewrite_iloc(
    template: bytes,
    patched: bytearray,
    iloc: object,
    locations: dict[int, tuple[str, list[tuple[int, int]]]],
    item_offsets: dict[int, int],
    item_lengths: dict[int, int],
) -> None:
    version, flags, reader = full_box_reader(template, iloc)
    if version != 0 or flags != 0:
        raise RuntimeError("source AVIF grid iloc differs from the pinned version-zero layout")
    sizes = reader.u16()
    item_count = reader.u16()
    if sizes != 0x4400 or item_count != len(CELL_ORDER):
        raise RuntimeError("source AVIF grid iloc uses an unexpected extent layout")

    seen = set()
    for _ in range(item_count):
        item_id = reader.u16()
        data_reference_index = reader.u16()
        extent_count = reader.u16()
        if (
            item_id not in locations
            or locations[item_id][0] != "file"
            or extent_count != 1
            or data_reference_index != 0
        ):
            raise RuntimeError(f"source AVIF grid iloc item {item_id} is unsupported")
        offset_position = reader.offset
        reader.u32()
        length_position = reader.offset
        reader.u32()
        start, end = locations[item_id][1][0]
        if end <= start:
            raise RuntimeError(f"source AVIF grid item {item_id} has an empty extent")
        if item_id not in item_offsets or item_offsets[item_id] < 0:
            raise RuntimeError(f"rebuilt AVIF grid item {item_id} has no valid offset")
        struct.pack_into(">I", patched, offset_position, item_offsets[item_id])
        struct.pack_into(">I", patched, length_position, item_lengths[item_id])
        seen.add(item_id)
    if reader.offset != reader.end or seen != set(CELL_ORDER):
        raise RuntimeError("source AVIF grid iloc entries are incomplete")


def assemble_fixture(
    template: bytes,
    cells: tuple[tuple[bytes, bytes, bytes], tuple[bytes, bytes, bytes]],
    *,
    grid_size: tuple[int, int] = GRID_SIZE,
    grid_descriptor: bytes = GRID_DESCRIPTOR,
) -> bytes:
    top = parse_boxes(template, 0, len(template))
    if [box.kind for box in top] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("source AVIF grid top-level box layout differs from the pin")
    ftyp, meta, mdat = top
    meta_children = children(template, meta, prefix=4)
    pitm = unique_box(meta_children, b"pitm")
    iinf = unique_box(meta_children, b"iinf")
    iloc = unique_box(meta_children, b"iloc")
    iref = unique_box(meta_children, b"iref")
    iprp = unique_box(meta_children, b"iprp")
    if any(box is None for box in (pitm, iinf, iloc, iref, iprp)):
        raise RuntimeError("source AVIF grid metadata boxes are incomplete")

    primary = parse_pitm(template, pitm)
    item_types = parse_iinf(template, iinf)
    references = parse_iref(template, iref)
    template_records = parse_items(template, meta)
    if (
        primary != 1
        or item_types != {1: b"grid", 2: b"av01", 3: b"av01", 5: b"av01", 6: b"av01"}
        or references
        != [
            (b"dimg", 1, 2),
            (b"dimg", 1, 3),
            (b"auxl", 5, 2),
            (b"auxl", 6, 3),
        ]
        or [item["item_id"] for item in template_records["color"]] != [2, 3]
        or [item["item_id"] for item in template_records["alpha"]] != [5, 6]
    ):
        raise RuntimeError("source AVIF grid item relationships differ from the pinned layout")

    locations = parse_iloc(template, iloc, idat=None)
    descriptor_location = locations.get(1)
    if (
        descriptor_location is None
        or descriptor_location[0] != "file"
        or len(descriptor_location[1]) != 1
        or template[
            descriptor_location[1][0][0] : descriptor_location[1][0][1]
        ]
        != bytes.fromhex("0000010000500050")
    ):
        raise RuntimeError("source AVIF grid descriptor differs from the pinned 2x1 layout")

    properties = template_properties(template, meta)
    if len(template[properties[0].payload_start : properties[0].end]) != 12:
        raise RuntimeError("source AVIF grid ispe property has an unexpected size")
    if len(template[properties[3].payload_start : properties[3].end]) != 12:
        raise RuntimeError("source AVIF cell ispe property has an unexpected size")
    if template[properties[4].payload_start : properties[4].end] != bytes.fromhex("81200000"):
        raise RuntimeError("source AVIF color av1C property differs from the pinned layout")
    if template[properties[5].payload_start : properties[5].end] != bytes.fromhex(
        "000000000108"
    ):
        raise RuntimeError("source AVIF one-channel pixi property differs from the pinned layout")

    patched = bytearray(template)
    struct.pack_into(">III", patched, properties[0].payload_start, 0, *grid_size)
    struct.pack_into(">III", patched, properties[3].payload_start, 0, *CELL_SIZE)
    patched[
        properties[4].payload_start : properties[4].end
    ] = MONO_AV1C
    rewrite_ipma(template, patched, iprp_associations(template, iprp))
    cells_by_item = {
        2: cells[0][0],
        3: cells[1][0],
        5: cells[0][1],
        6: cells[1][1],
    }
    payloads = {
        1: grid_descriptor,
        **cells_by_item,
    }
    mdat_payload = b"".join(payloads[item_id] for item_id in CELL_ORDER)
    mdat_payload_start = mdat.payload_start
    starts = {}
    cursor = mdat_payload_start
    for item_id in CELL_ORDER:
        starts[item_id] = cursor
        cursor += len(payloads[item_id])
    if cursor - mdat_payload_start != len(mdat_payload):
        raise RuntimeError("rebuilt AVIF mdat payload length is inconsistent")
    iloc_payload_lengths = {item_id: len(payloads[item_id]) for item_id in CELL_ORDER}
    rewrite_iloc(
        template, patched, iloc, locations, starts, iloc_payload_lengths
    )
    rewritten_meta = bytes(patched[meta.start : meta.end])
    if len(rewritten_meta) != meta.size:
        raise RuntimeError("AVIF metadata changed size while rebasing item locations")
    rewritten_mdat = pack_box(b"mdat", mdat_payload)
    result = bytes(patched[: ftyp.end]) + rewritten_meta + rewritten_mdat
    if result[: meta.start] != template[: meta.start]:
        raise RuntimeError("AVIF template file-type box changed unexpectedly")
    verify_fixture_structure(
        result, payloads, grid_size=grid_size, grid_descriptor=grid_descriptor
    )
    return result


def iprp_associations(template: bytes, iprp: object) -> object:
    associations = unique_box(children(template, iprp), b"ipma")
    if associations is None:
        raise RuntimeError("source AVIF grid is missing ipma")
    return associations


def verify_fixture_structure(
    data: bytes,
    expected_payloads: dict[int, bytes],
    *,
    grid_size: tuple[int, int] = GRID_SIZE,
    grid_descriptor: bytes = GRID_DESCRIPTOR,
) -> None:
    top = parse_boxes(data, 0, len(data))
    if [box.kind for box in top] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("generated AVIF grid top-level boxes are invalid")
    meta = unique_box(top, b"meta")
    if meta is None:
        raise RuntimeError("generated AVIF grid is missing meta")
    meta_children = children(data, meta, prefix=4)
    pitm = unique_box(meta_children, b"pitm")
    iloc = unique_box(meta_children, b"iloc")
    iref = unique_box(meta_children, b"iref")
    iprp = unique_box(meta_children, b"iprp")
    if any(box is None for box in (pitm, iloc, iref, iprp)):
        raise RuntimeError("generated AVIF grid metadata boxes are incomplete")
    ipco = unique_box(children(data, iprp), b"ipco")
    if ipco is None:
        raise RuntimeError("generated AVIF grid is missing its property table")
    properties = children(data, ipco)
    if not properties or properties[0].kind != b"ispe":
        raise RuntimeError("generated AVIF grid is missing its spatial extent")
    width, height = struct.unpack_from(">II", data, properties[0].payload_start + 4)
    if (width, height) != grid_size:
        raise RuntimeError("generated AVIF grid spatial extent differs from its descriptor")
    if parse_pitm(data, pitm) != 1:
        raise RuntimeError("generated AVIF grid has an unexpected primary item")
    if parse_iref(data, iref) != [
        (b"dimg", 1, 2),
        (b"dimg", 1, 3),
        (b"auxl", 5, 2),
        (b"auxl", 6, 3),
    ]:
        raise RuntimeError("generated AVIF grid references differ from the pinned layout")

    locations = parse_iloc(data, iloc, idat=None)
    if locations.get(1) is None or locations[1][0] != "file":
        raise RuntimeError("generated AVIF grid descriptor is not file-backed")
    if [data[start:end] for start, end in locations[1][1]] != [grid_descriptor]:
        raise RuntimeError("generated AVIF grid payload differs from its pinned descriptor")
    for item_id, expected_payload in expected_payloads.items():
        location = locations.get(item_id)
        if location is None or location[0] != "file" or len(location[1]) != 1:
            raise RuntimeError(f"generated AVIF item {item_id} has invalid file extents")
        start, end = location[1][0]
        if data[start:end] != expected_payload:
            raise RuntimeError(f"generated AVIF item {item_id} extent has the wrong payload")
    records = parse_items(data, meta)
    if [item["item_id"] for item in records["color"]] != [2, 3]:
        raise RuntimeError("generated AVIF grid lacks two color cells")
    if [item["item_id"] for item in records["alpha"]] != [5, 6]:
        raise RuntimeError("generated AVIF grid lacks two alpha cells")


def expected_pixels(
    cell0: bytes,
    cell1: bytes,
    *,
    grid_size: tuple[int, int] = GRID_SIZE,
    orientation: str = "horizontal",
) -> bytes:
    """Compose two RGBA cells into a grid clipped on its final edge."""
    row_bytes = CELL_SIZE[0] * 4
    if orientation == "horizontal":
        if (
            grid_size[1] != CELL_SIZE[1]
            or grid_size[0] <= CELL_SIZE[0]
            or grid_size[0] > 2 * CELL_SIZE[0]
        ):
            raise ValueError("horizontal grid dimensions do not fit two cell columns")
        visible_right_bytes = (grid_size[0] - CELL_SIZE[0]) * 4
        result = bytearray()
        for y in range(grid_size[1]):
            start = y * row_bytes
            result.extend(cell0[start : start + row_bytes])
            result.extend(cell1[start : start + visible_right_bytes])
        return bytes(result)
    if orientation == "vertical":
        if (
            grid_size[0] != CELL_SIZE[0]
            or grid_size[1] <= CELL_SIZE[1]
            or grid_size[1] > 2 * CELL_SIZE[1]
        ):
            raise ValueError("vertical grid dimensions do not fit two cell rows")
        visible_bottom_bytes = (grid_size[1] - CELL_SIZE[1]) * row_bytes
        return cell0 + cell1[:visible_bottom_bytes]
    raise ValueError(f"unsupported two-cell grid orientation: {orientation}")


def verify_fixture_pixels(
    data: bytes,
    cell0: bytes,
    cell1: bytes,
    *,
    grid_size: tuple[int, int] = GRID_SIZE,
    expected_hash: str = EXPECTED_PIXELS_SHA256,
    orientation: str = "horizontal",
) -> bytes:
    with Image.open(io.BytesIO(data)) as image:
        image.verify()
    with Image.open(io.BytesIO(data)) as image:
        image.load()
        if image.format != "AVIF" or image.mode != "RGBA" or image.size != grid_size:
            raise RuntimeError("Pillow decoded an unexpected monochrome AVIF grid descriptor")
        actual = image.tobytes()

    expected = expected_pixels(
        cell0, cell1, grid_size=grid_size, orientation=orientation
    )
    if actual != expected:
        raise RuntimeError("Pillow output differs from the exact lossless cell pixels")
    if sha256(actual) != expected_hash:
        raise RuntimeError("Pillow pixels differ from the pinned output reference")
    return actual


def main() -> None:
    manifest = yaml.safe_load(MANIFEST.read_text())
    oracle.verify_primary_oracle(manifest)
    if PILLOW_VERSION != "12.2.0" or features.version("avif") != "1.4.1":
        raise RuntimeError("AVIF grid generation requires the pinned Pillow oracle")

    template = SOURCE.read_bytes()
    if sha256(template) != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("source grid.avif differs from the pinned libavif fixture")

    encoded_cells = (encode_cell(0), encode_cell(1))
    fixture = assemble_fixture(template, encoded_cells)
    pixels = verify_fixture_pixels(
        fixture, encoded_cells[0][2], encoded_cells[1][2]
    )
    fixture_hash = sha256(fixture)
    if fixture_hash != EXPECTED_FIXTURE_SHA256:
        raise RuntimeError(
            "exact Pillow pixels passed, but the fixture bytes differ from the "
            "prototype container hash cross-check: " + fixture_hash
        )

    if OUTPUT.exists():
        if OUTPUT.read_bytes() != fixture:
            raise RuntimeError(f"refusing to replace different AVIF fixture at {OUTPUT}")
        print(f"Verified existing fixture: {OUTPUT}")
    else:
        OUTPUT.write_bytes(fixture)
        print(f"Wrote fixture: {OUTPUT}")

    print(
        f"grid=2x1 cells=64x64 canvas={GRID_SIZE[0]}x{GRID_SIZE[1]} "
        f"descriptor={GRID_DESCRIPTOR.hex()}"
    )
    print(f"asset_sha256={sha256(fixture)}")
    print(f"pixels={len(pixels)} pillow_rgba_sha256={sha256(pixels)}")


if __name__ == "__main__":
    main()
