#!/usr/bin/env python3
"""Generate Pillow-verified monochrome AVIF grids with edge clipping."""

from __future__ import annotations

import argparse
import hashlib
import io
import struct
from pathlib import Path

import yaml
from PIL import Image, __version__ as PILLOW_VERSION, features

import generate_avif_monochrome_grid_fixture as grid
from avif_bmff_boxes import pack_box
from inspect_avif_bitstreams import (
    children,
    full_box_reader,
    parse_boxes,
    parse_iinf,
    parse_iloc,
    parse_iref,
    parse_items,
    parse_pitm,
    unique_box,
)

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/avif/grid.avif"
BOTTOM_CLIP_OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/avif/portable_lossless_monochrome_grid_bottom_clip_64x73.avif"
)
NO_ALPHA_OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/avif/portable_lossless_monochrome_grid_no_alpha_73x64.avif"
)
MANIFEST = ROOT / "manifest.yaml"

EXPECTED_SOURCE_SHA256 = "bae56368b348b1d847e2bfb662522599f0c63dfe62fb68826c9e42a300ff405d"
EXPECTED_BOTTOM_FIXTURE_SHA256 = "453945fc2524dabc77162d97896b21d8399a7171a909d19babb908603c0a4201"
EXPECTED_BOTTOM_PIXELS_SHA256 = "20b867857750340cae5d682bb6f5a162508395c242dfe331321cbff0bae90bb4"
EXPECTED_NO_ALPHA_FIXTURE_SHA256 = "a33a40230675d388722a13e45849205e24ff7f36486502907472c5f9ebd719d0"
EXPECTED_NO_ALPHA_PIXELS_SHA256 = "1b4a716a8f76555d37d08901f0582e195b2bdcedad46b00f781995d42e4cb1b6"

CELL_SIZE = (64, 64)
BOTTOM_GRID_SIZE = (64, 73)
BOTTOM_GRID_DESCRIPTOR = bytes.fromhex("0000010000400049")
NO_ALPHA_GRID_SIZE = (73, 64)
NO_ALPHA_GRID_DESCRIPTOR = bytes.fromhex("0000000100490040")
MONO_AV1C = bytes.fromhex("81001c00")
NO_ALPHA_ITEM_IDS = (1, 2, 3)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def luma_pixels(variant: int) -> bytes:
    """Return deterministic 8-bit luma samples for one grid cell."""
    return bytes(
        (x * 13 + y * 27 + ((x * y + variant * 91) % 113)) % 256
        for y in range(CELL_SIZE[1])
        for x in range(CELL_SIZE[0])
    )


def encode_luma_cell(variant: int) -> tuple[bytes, bytes]:
    """Encode a lossless monochrome cell and return its AV1 item payload."""
    pixels = luma_pixels(variant)
    image = Image.frombytes("L", CELL_SIZE, pixels)

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
    if first != encode():
        raise RuntimeError(f"no-alpha AVIF grid cell {variant} encoding is not deterministic")

    with Image.open(io.BytesIO(first)) as check:
        check.verify()
    with Image.open(io.BytesIO(first)) as check:
        check.load()
        if check.format != "AVIF" or check.size != CELL_SIZE:
            raise RuntimeError(f"no-alpha AVIF grid cell {variant} has an unexpected image")
        if check.convert("L").tobytes() != pixels:
            raise RuntimeError(f"no-alpha AVIF grid cell {variant} does not roundtrip exactly")

    top = parse_boxes(first, 0, len(first))
    meta = unique_box(top, b"meta")
    if meta is None:
        raise RuntimeError(f"no-alpha AVIF grid cell {variant} is missing meta")
    meta_children = children(first, meta, prefix=4)
    pitm = unique_box(meta_children, b"pitm")
    iinf = unique_box(meta_children, b"iinf")
    iloc = unique_box(meta_children, b"iloc")
    irefs = [box for box in meta_children if box.kind == b"iref"]
    if any(box is None for box in (pitm, iinf, iloc)) or len(irefs) > 1:
        raise RuntimeError(f"no-alpha AVIF grid cell {variant} metadata is incomplete")
    iref = irefs[0] if irefs else None

    primary = parse_pitm(first, pitm)
    if parse_iinf(first, iinf) != {primary: b"av01"}:
        raise RuntimeError(f"no-alpha AVIF grid cell {variant} has unexpected AV1 items")
    if iref is not None and parse_iref(first, iref):
        raise RuntimeError(f"no-alpha AVIF grid cell {variant} has item references")
    locations = parse_iloc(first, iloc, idat=None)
    location = locations.get(primary)
    if location is None or location[0] != "file" or len(location[1]) != 1:
        raise RuntimeError(f"no-alpha AVIF grid cell {variant} has an invalid AV1 extent")
    start, end = location[1][0]
    payload = first[start:end]
    if not payload:
        raise RuntimeError(f"no-alpha AVIF grid cell {variant} has an empty AV1 extent")
    return payload, pixels


def _box_bytes(data: bytes, box: object) -> bytes:
    return data[box.start : box.end]


def _full_box_payload(data: bytes, box: object) -> tuple[int, int, object]:
    version, flags, reader = full_box_reader(data, box)
    return version, flags, reader


def _item_info_id(data: bytes, item_info: object) -> int:
    version, _, reader = full_box_reader(data, item_info)
    if version not in (2, 3):
        raise RuntimeError("grid template item-info version differs from the pinned layout")
    return reader.u16() if version == 2 else reader.u32()


def _make_iinf(template: bytes, iinf: object) -> bytes:
    version, flags, reader = _full_box_payload(template, iinf)
    if version != 0 or flags != 0:
        raise RuntimeError("grid template iinf differs from the pinned version-zero layout")
    entry_count = reader.u16()
    entries = children(template, iinf, prefix=6)
    if len(entries) != entry_count:
        raise RuntimeError("grid template iinf item count is inconsistent")
    kept = [
        _box_bytes(template, entry)
        for entry in entries
        if _item_info_id(template, entry) in NO_ALPHA_ITEM_IDS
    ]
    if len(kept) != len(NO_ALPHA_ITEM_IDS):
        raise RuntimeError("grid template does not contain exactly three no-alpha items")
    payload = template[iinf.payload_start : iinf.payload_start + 4]
    payload += struct.pack(">H", len(kept)) + b"".join(kept)
    return pack_box(b"iinf", payload)


def _make_iref(template: bytes, iref: object) -> bytes:
    version, flags, _ = _full_box_payload(template, iref)
    if version != 0 or flags != 0:
        raise RuntimeError("grid template iref differs from the pinned version-zero layout")
    references = children(template, iref, prefix=4)
    kept = [box for box in references if box.kind == b"dimg"]
    if len(kept) != 1:
        raise RuntimeError("grid template does not contain two image-cell references")
    payload = template[iref.payload_start : iref.payload_start + 4]
    payload += b"".join(_box_bytes(template, box) for box in kept)
    return pack_box(b"iref", payload)


def _make_ipma(template: bytes, ipma: object) -> bytes:
    version, flags, reader = _full_box_payload(template, ipma)
    if version != 0 or flags != 0:
        raise RuntimeError("grid template ipma differs from the pinned version-zero layout")
    entry_count = reader.u32()
    entries = []
    seen = set()
    for _ in range(entry_count):
        start = reader.offset
        item_id = reader.u16()
        association_count = reader.u8()
        associations = bytearray(reader.u8() for _ in range(association_count))
        if item_id in NO_ALPHA_ITEM_IDS:
            if len(associations) < 2 or associations[1] != 2:
                raise RuntimeError("grid template monochrome pixi association differs from the pin")
            associations[1] = 6
            entries.append(
                struct.pack(">HB", item_id, association_count) + associations
            )
            seen.add(item_id)
        if reader.offset <= start:
            raise RuntimeError("grid template ipma entry did not advance")
    if reader.offset != reader.end or seen != set(NO_ALPHA_ITEM_IDS):
        raise RuntimeError("grid template ipma no-alpha entries are incomplete")
    payload = template[ipma.payload_start : ipma.payload_start + 4]
    payload += struct.pack(">I", len(entries)) + b"".join(entries)
    return pack_box(b"ipma", payload)


def _make_iprp(template: bytes, iprp: object, grid_size: tuple[int, int]) -> bytes:
    iprp_children = children(template, iprp)
    ipco = unique_box(iprp_children, b"ipco")
    ipma = unique_box(iprp_children, b"ipma")
    if ipco is None or ipma is None or len(iprp_children) != 2:
        raise RuntimeError("grid template iprp differs from the pinned two-box layout")
    properties = children(template, ipco)
    expected_kinds = [b"ispe", b"pixi", b"colr", b"ispe", b"av1C", b"pixi"]
    if [property_box.kind for property_box in properties[:6]] != expected_kinds:
        raise RuntimeError("grid template property table differs from the pinned layout")

    rewritten = []
    for index, property_box in enumerate(properties[:6]):
        content = bytearray(_box_bytes(template, property_box))
        payload_offset = property_box.payload_start - property_box.start
        if index == 0:
            struct.pack_into(">III", content, payload_offset, 0, *grid_size)
        elif index == 3:
            struct.pack_into(">III", content, payload_offset, 0, *CELL_SIZE)
        elif index == 4:
            content[payload_offset : property_box.end - property_box.start] = MONO_AV1C
        rewritten.append(bytes(content))

    new_ipco = pack_box(b"ipco", b"".join(rewritten))
    new_ipma = _make_ipma(template, ipma)
    return pack_box(b"iprp", new_ipco + new_ipma)


def _make_iloc(
    offsets: dict[int, int], lengths: dict[int, int]
) -> bytes:
    payload = bytes(4) + struct.pack(">HH", 0x4400, len(NO_ALPHA_ITEM_IDS))
    for item_id in NO_ALPHA_ITEM_IDS:
        payload += struct.pack(">HHHII", item_id, 0, 1, offsets[item_id], lengths[item_id])
    return pack_box(b"iloc", payload)


def assemble_no_alpha_fixture(
    template: bytes,
    cells: tuple[bytes, bytes],
    grid_size: tuple[int, int],
    descriptor: bytes,
) -> bytes:
    """Build a clean monochrome grid without auxiliary-alpha structures."""
    top = parse_boxes(template, 0, len(template))
    if [box.kind for box in top] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("source AVIF grid top-level box layout differs from the pin")
    ftyp, source_meta, _ = top
    meta_children = children(template, source_meta, prefix=4)
    pitm = unique_box(meta_children, b"pitm")
    iinf = unique_box(meta_children, b"iinf")
    iloc = unique_box(meta_children, b"iloc")
    iref = unique_box(meta_children, b"iref")
    iprp = unique_box(meta_children, b"iprp")
    if any(box is None for box in (pitm, iinf, iloc, iref, iprp)):
        raise RuntimeError("source AVIF grid metadata boxes are incomplete")
    if parse_pitm(template, pitm) != 1:
        raise RuntimeError("source AVIF grid primary item differs from the pin")

    references = parse_iref(template, iref)
    if references != [
        (b"dimg", 1, 2),
        (b"dimg", 1, 3),
        (b"auxl", 5, 2),
        (b"auxl", 6, 3),
    ]:
        raise RuntimeError("source AVIF grid references differ from the pinned layout")

    payloads = {1: descriptor, 2: cells[0], 3: cells[1]}
    mdat_payload = b"".join(payloads[item_id] for item_id in NO_ALPHA_ITEM_IDS)

    def make_meta(offsets: dict[int, int]) -> bytes:
        replacements = {
            b"iinf": _make_iinf(template, iinf),
            b"iloc": _make_iloc(
                offsets, {item_id: len(payloads[item_id]) for item_id in NO_ALPHA_ITEM_IDS}
            ),
            b"iref": _make_iref(template, iref),
            b"iprp": _make_iprp(template, iprp, grid_size),
        }
        child_bytes = []
        for box in meta_children:
            child_bytes.append(
                replacements[box.kind]
                if box.kind in replacements
                else _box_bytes(template, box)
            )
        payload = template[source_meta.payload_start : source_meta.payload_start + 4]
        payload += b"".join(child_bytes)
        return pack_box(b"meta", payload)

    placeholder_offsets = {item_id: 0 for item_id in NO_ALPHA_ITEM_IDS}
    placeholder_meta = make_meta(placeholder_offsets)
    mdat_payload_start = ftyp.size + len(placeholder_meta) + 8
    offsets = {}
    cursor = mdat_payload_start
    for item_id in NO_ALPHA_ITEM_IDS:
        offsets[item_id] = cursor
        cursor += len(payloads[item_id])
    meta = make_meta(offsets)
    if len(meta) != len(placeholder_meta):
        raise RuntimeError("AVIF metadata changed size while rebasing item locations")
    result = _box_bytes(template, ftyp) + meta + pack_box(b"mdat", mdat_payload)
    verify_no_alpha_structure(result, payloads, grid_size, descriptor)
    return result


def verify_no_alpha_structure(
    data: bytes,
    payloads: dict[int, bytes],
    grid_size: tuple[int, int],
    descriptor: bytes,
) -> None:
    top = parse_boxes(data, 0, len(data))
    if [box.kind for box in top] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("generated no-alpha grid top-level boxes are invalid")
    meta = unique_box(top, b"meta")
    if meta is None:
        raise RuntimeError("generated no-alpha grid is missing meta")
    meta_children = children(data, meta, prefix=4)
    pitm = unique_box(meta_children, b"pitm")
    iinf = unique_box(meta_children, b"iinf")
    iloc = unique_box(meta_children, b"iloc")
    iref = unique_box(meta_children, b"iref")
    iprp = unique_box(meta_children, b"iprp")
    if any(box is None for box in (pitm, iinf, iloc, iref, iprp)):
        raise RuntimeError("generated no-alpha grid metadata is incomplete")
    if parse_pitm(data, pitm) != 1:
        raise RuntimeError("generated no-alpha grid has an unexpected primary item")
    if parse_iinf(data, iinf) != {1: b"grid", 2: b"av01", 3: b"av01"}:
        raise RuntimeError("generated no-alpha grid still has non-color items")
    if parse_iref(data, iref) != [(b"dimg", 1, 2), (b"dimg", 1, 3)]:
        raise RuntimeError("generated no-alpha grid has auxiliary-alpha references")
    records = parse_items(data, meta)
    if [item["item_id"] for item in records["color"]] != [2, 3] or records["alpha"]:
        raise RuntimeError("generated no-alpha grid has an unexpected item inventory")

    locations = parse_iloc(data, iloc, idat=None)
    if set(locations) != set(NO_ALPHA_ITEM_IDS):
        raise RuntimeError("generated no-alpha grid item locations are incomplete")
    if [data[start:end] for start, end in locations[1][1]] != [descriptor]:
        raise RuntimeError("generated no-alpha grid descriptor is invalid")
    for item_id, expected_payload in payloads.items():
        location = locations[item_id]
        if location[0] != "file" or len(location[1]) != 1:
            raise RuntimeError(f"generated no-alpha grid item {item_id} has invalid extents")
        start, end = location[1][0]
        if data[start:end] != expected_payload:
            raise RuntimeError(f"generated no-alpha grid item {item_id} payload differs")

    ipco = unique_box(children(data, iprp), b"ipco")
    if ipco is None:
        raise RuntimeError("generated no-alpha grid is missing its property table")
    properties = children(data, ipco)
    if [property_box.kind for property_box in properties] != [
        b"ispe",
        b"pixi",
        b"colr",
        b"ispe",
        b"av1C",
        b"pixi",
    ]:
        raise RuntimeError("generated no-alpha grid retains auxiliary-alpha properties")
    width, height = struct.unpack_from(">II", data, properties[0].payload_start + 4)
    if (width, height) != grid_size:
        raise RuntimeError("generated no-alpha grid spatial extent is invalid")
    if data[properties[4].payload_start : properties[4].end] != MONO_AV1C:
        raise RuntimeError("generated no-alpha grid is not configured as monochrome")


def expected_no_alpha_pixels(cell0: bytes, cell1: bytes) -> bytes:
    visible_right = NO_ALPHA_GRID_SIZE[0] - CELL_SIZE[0]
    pixels = bytearray()
    for y in range(NO_ALPHA_GRID_SIZE[1]):
        row_start = y * CELL_SIZE[0]
        luma_row = cell0[row_start : row_start + CELL_SIZE[0]]
        luma_row += cell1[row_start : row_start + visible_right]
        for value in luma_row:
            pixels.extend((value, value, value))
    return bytes(pixels)


def verify_no_alpha_pixels(data: bytes, cell0: bytes, cell1: bytes) -> bytes:
    with Image.open(io.BytesIO(data)) as image:
        image.verify()
    with Image.open(io.BytesIO(data)) as image:
        image.load()
        if image.format != "AVIF" or image.mode != "RGB" or image.size != NO_ALPHA_GRID_SIZE:
            raise RuntimeError("Pillow decoded an unexpected no-alpha monochrome grid")
        actual = image.tobytes()
    expected = expected_no_alpha_pixels(cell0, cell1)
    if actual != expected or sha256(actual) != EXPECTED_NO_ALPHA_PIXELS_SHA256:
        raise RuntimeError("Pillow pixels differ from the pinned no-alpha RGB reference")
    return actual


def store_fixture(path: Path, fixture: bytes, *, check: bool) -> None:
    if check:
        if not path.is_file() or path.read_bytes() != fixture:
            raise RuntimeError(f"fixture is stale or missing: {path.relative_to(ROOT)}")
        return
    if path.exists():
        if path.read_bytes() != fixture:
            raise RuntimeError(f"refusing to replace different fixture at {path}")
        return
    path.write_bytes(fixture)


def generate(*, check: bool) -> None:
    manifest = yaml.safe_load(MANIFEST.read_text())
    grid.oracle.verify_primary_oracle(manifest)
    if PILLOW_VERSION != "12.2.0" or features.version("avif") != "1.4.1":
        raise RuntimeError("AVIF grid generation requires the pinned Pillow oracle")
    template = SOURCE.read_bytes()
    if sha256(template) != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("source grid.avif differs from the pinned libavif fixture")

    alpha_cells = (grid.encode_cell(0), grid.encode_cell(1))
    bottom_fixture = grid.assemble_fixture(
        template,
        alpha_cells,
        grid_size=BOTTOM_GRID_SIZE,
        grid_descriptor=BOTTOM_GRID_DESCRIPTOR,
    )
    bottom_pixels = grid.verify_fixture_pixels(
        bottom_fixture,
        alpha_cells[0][2],
        alpha_cells[1][2],
        grid_size=BOTTOM_GRID_SIZE,
        expected_hash=EXPECTED_BOTTOM_PIXELS_SHA256,
        orientation="vertical",
    )
    if sha256(bottom_fixture) != EXPECTED_BOTTOM_FIXTURE_SHA256:
        raise RuntimeError("bottom-clipped AVIF grid fixture differs from the pinned bytes")

    no_alpha_cells = (encode_luma_cell(0), encode_luma_cell(1))
    no_alpha_fixture = assemble_no_alpha_fixture(
        template,
        (no_alpha_cells[0][0], no_alpha_cells[1][0]),
        NO_ALPHA_GRID_SIZE,
        NO_ALPHA_GRID_DESCRIPTOR,
    )
    no_alpha_pixels = verify_no_alpha_pixels(
        no_alpha_fixture, no_alpha_cells[0][1], no_alpha_cells[1][1]
    )
    if sha256(no_alpha_fixture) != EXPECTED_NO_ALPHA_FIXTURE_SHA256:
        raise RuntimeError(
            "no-alpha AVIF grid fixture differs from its pinned bytes: "
            + sha256(no_alpha_fixture)
        )

    store_fixture(BOTTOM_CLIP_OUTPUT, bottom_fixture, check=check)
    store_fixture(NO_ALPHA_OUTPUT, no_alpha_fixture, check=check)
    action = "Verified" if check else "Wrote or verified"
    print(
        f"{action} {BOTTOM_CLIP_OUTPUT.relative_to(ROOT)} "
        f"sha256={sha256(bottom_fixture)} pixels={len(bottom_pixels)}"
    )
    print(
        f"{action} {NO_ALPHA_OUTPUT.relative_to(ROOT)} "
        f"sha256={sha256(no_alpha_fixture)} mode=RGB pixels={len(no_alpha_pixels)}"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the fixtures without writing them",
    )
    arguments = parser.parse_args()
    generate(check=arguments.check)


if __name__ == "__main__":
    main()
