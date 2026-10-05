#!/usr/bin/env python3
"""Generate complete AVIFs exercising bounded, indexed idat item locations.

The deterministic input mutation keeps the AV1 item bytes and properties from
the pinned still, then selects an idat extent through iloc construction method
one. A second input includes a zero, unused extent index accepted by pinned
Pillow for that construction method. Additional inputs split the item across
two extents, truncate an extent index, declare an unsupported index width, or
extend the item one byte beyond idat. Live Pillow observations distinguish
open failures from the last input's successful inspection and decode failure.
"""

from __future__ import annotations

import argparse
import hashlib
import struct
from io import BytesIO
from pathlib import Path

import yaml
from PIL import Image, UnidentifiedImageError

from generate_avif_config_disagreement_fixtures import pillow_snapshot
from generate_avif_zero_width_clap_fixture import pack_box, read_boxes
from generate_decode_refs import stable_error_message, verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT_DIR = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
DEFAULT_SOURCE = DEFAULT_OUTPUT_DIR / "baseline.avif"
FILENAME = "iloc_idat_primary_item.avif"
INDEXED_FILENAME = "iloc_idat_indexed_extent.avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
IDAT_SOURCE_SHA256 = "cc020ae1e91a2cad8c4f66b10165702bf11abc39a462a06b9665664f3c63c337"
INDEXED_SOURCE_SHA256 = "d90e970b5b9570c5091df64871d58a8f500c72df294c456037fad90cc1d92aac"
EDGE_SHA256 = {
    "error_iloc_truncated_extent_index.avif": "62f5d6dd3b3cfb368030407977a2af434659c178c1412844781e43c03e25d8ed",
    "error_iloc_index_size_three.avif": "53be83151f9031fbc56c0f3bc91fd50f5c2b7b3376ac760fb662b458eadc572b",
    "error_iloc_idat_extent_out_of_bounds.avif": "1e0f61d11eb4cf291f4af89d3636ff874f517358e2eecb20d8b92d1a4a05e46b",
    "iloc_idat_split_extents.avif": "b92d22f904a48d380a99f7b91ff78aa73ab4fc73e9d2f8b71697d843881aa1bd",
}
BASE_OFFSET = 8
EXTENT_OFFSET = 8
IDAT_PADDING = b"\xff" * (BASE_OFFSET + EXTENT_OFFSET)


def move_primary_item_to_idat(source: bytes) -> bytes:
    """Relocate the pinned primary item while retaining all existing boxes."""
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("baseline AVIF source differs from its pinned hash")
    top = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in top] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("baseline AVIF top-level layout changed")
    ftyp, meta, mdat = (complete for _, complete, _ in top)
    coded_item = mdat[8:]
    children = read_boxes(meta, 12, len(meta))
    locations = [payload for kind, _, payload in children if kind == b"iloc"]
    if (
        meta[8:12] != bytes(4)
        or len(locations) != 1
        or any(kind == b"idat" for kind, _, _ in children)
        or locations[0] != bytes.fromhex("0000000044000001000100000001")
        + struct.pack(">II", len(ftyp) + len(meta) + 8, len(coded_item))
    ):
        raise RuntimeError("baseline AVIF primary-item extent changed")

    # iloc v1: four-byte offset, length and base offset; no extent index.
    # Item 1 uses construction method 1 and data_reference_index zero.
    location = bytes.fromhex("0100000044400001000100010000") + struct.pack(
        ">IHII", BASE_OFFSET, 1, EXTENT_OFFSET, len(coded_item)
    )
    rebuilt = [
        pack_box(b"iloc", location) if kind == b"iloc" else complete
        for kind, complete, _ in children
    ]
    rebuilt.append(pack_box(b"idat", IDAT_PADDING + coded_item))
    # The original mdat remains unchanged and unreferenced. No absolute file
    # extent remains to rebase when the meta box grows.
    return ftyp + pack_box(b"meta", meta[8:12] + b"".join(rebuilt)) + mdat


def add_unused_extent_index(source: bytes) -> bytes:
    """Add a four-byte extent index without changing the idat item extent."""
    if hashlib.sha256(source).hexdigest() != IDAT_SOURCE_SHA256:
        raise RuntimeError("idat AVIF source differs from its pinned hash")
    top = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in top] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("idat AVIF top-level layout changed")
    ftyp, meta, mdat = (complete for _, complete, _ in top)
    children = read_boxes(meta, 12, len(meta))
    locations = [payload for kind, _, payload in children if kind == b"iloc"]
    if (
        meta[8:12] != bytes(4)
        or len(locations) != 1
        or locations[0]
        != bytes.fromhex("01000000444000010001000100000000000800010000000800000aeb")
    ):
        raise RuntimeError("idat AVIF primary-item extent changed")
    location = locations[0]
    # iloc v1 index_size becomes four. Pinned Pillow accepts this zero unused
    # extent index for construction method 1; the base, offset and AV1 bytes
    # stay unchanged. This is not a construction-method-2 item reference.
    indexed_location = (
        location[:5]
        + bytes((location[5] | 4,))
        + location[6:20]
        + bytes(4)
        + location[20:]
    )
    rebuilt = [
        pack_box(b"iloc", indexed_location) if kind == b"iloc" else complete
        for kind, complete, _ in children
    ]
    # Rebuilding adds four bytes to iloc and meta. The selected idat offset is
    # relative to its unchanged payload, so no absolute extent needs rebasing.
    return ftyp + pack_box(b"meta", meta[8:12] + b"".join(rebuilt)) + mdat


def iloc_edge_variants(source: bytes) -> tuple[tuple[str, bytes, str], ...]:
    """Mutate only the pinned indexed iloc, rebuilding enclosing box sizes."""
    if hashlib.sha256(source).hexdigest() != INDEXED_SOURCE_SHA256:
        raise RuntimeError("indexed idat AVIF differs from its pinned input hash")
    top = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in top] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("indexed idat AVIF top-level layout changed")
    ftyp, meta, mdat = (complete for _, complete, _ in top)
    children = read_boxes(meta, 12, len(meta))
    locations = [payload for kind, _, payload in children if kind == b"iloc"]
    if (
        meta[8:12] != bytes(4)
        or len(locations) != 1
        or locations[0]
        != bytes.fromhex("0100000044440001000100010000000000080001000000000000000800000aeb")
    ):
        raise RuntimeError("indexed idat AVIF primary-item extent changed")
    location = locations[0]
    # The boxes and complete AV1 payload remain present in every input. Only
    # iloc's bounded payload changes, so a truncated index cannot borrow bytes
    # from the next child and the overrun cannot borrow the adjacent mdat.
    mutations = (
        ("error_iloc_truncated_extent_index.avif", location[:22], "open_error"),
        (
            "error_iloc_index_size_three.avif",
            location[:5] + b"\x43" + location[6:],
            "open_error",
        ),
        (
            "error_iloc_idat_extent_out_of_bounds.avif",
            location[:28] + struct.pack(">I", 2796),
            "decode_error",
        ),
        (
            "iloc_idat_split_extents.avif",
            location[:18]
            + struct.pack(">H", 2)
            + struct.pack(">III", 0, 8, 1397)
            + struct.pack(">III", 0, 1405, 1398),
            "success",
        ),
    )
    variants = []
    for filename, payload, outcome in mutations:
        rebuilt = [
            pack_box(b"iloc", payload) if kind == b"iloc" else complete
            for kind, complete, _ in children
        ]
        candidate = ftyp + pack_box(b"meta", meta[8:12] + b"".join(rebuilt)) + mdat
        if hashlib.sha256(candidate).hexdigest() != EDGE_SHA256[filename]:
            raise RuntimeError(f"{filename} differs from its pinned input hash")
        variants.append((filename, candidate, outcome))
    return tuple(variants)


def verify_open_failure(source: bytes, filename: str) -> None:
    """Require Pillow's public malformed-container failure at Image.open."""
    try:
        with Image.open(BytesIO(source)):
            pass
    except UnidentifiedImageError as error:
        if stable_error_message(error) == "cannot identify image file <bytes>":
            return
        raise RuntimeError(f"{filename}: Pillow open diagnostic changed") from error
    raise RuntimeError(f"{filename}: Pillow unexpectedly opened the malformed iloc")


def verify_decode_failure(source: bytes, filename: str, reference_header: tuple) -> None:
    """Require intact public inspection/verify and a malformed frame at load."""
    with Image.open(BytesIO(source)) as image:
        header = (
            image.format, image.mode, image.size, image.n_frames,
            image.is_animated, dict(image.info),
        )
        if header != reference_header:
            raise RuntimeError(f"{filename}: Pillow inspection changed")
        image.verify()
    for sequence in (False, True):
        with Image.open(BytesIO(source)) as image:
            try:
                if sequence:
                    image.seek(0)
                image.load()
            except SyntaxError as error:
                if str(error) != "Failed to decode frame 0: BMFF parsing failed":
                    raise RuntimeError(f"{filename}: Pillow decode diagnostic changed") from error
            else:
                raise RuntimeError(f"{filename}: Pillow unexpectedly decoded the idat overrun")


def generate(
    output_dir: Path = DEFAULT_OUTPUT_DIR, source_path: Path = DEFAULT_SOURCE
) -> Path:
    """Generate or verify all six idat fixtures, returning the original path."""
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_bytes())
    verify_primary_oracle(manifest)
    source = source_path.read_bytes()
    candidate = move_primary_item_to_idat(source)
    indexed_candidate = add_unused_extent_index(candidate)
    reference = pillow_snapshot(source)
    variants = (
        (FILENAME, candidate, "success"),
        (INDEXED_FILENAME, indexed_candidate, "success"),
        *iloc_edge_variants(indexed_candidate),
    )
    for filename, data, outcome in variants:
        if outcome == "open_error":
            verify_open_failure(data, filename)
        elif outcome == "decode_error":
            verify_decode_failure(data, filename, reference[0])
        elif pillow_snapshot(data) != reference:
            raise RuntimeError(f"{filename}: idat mutation changed Pillow's public image result")
        path = output_dir / filename
        if path.exists() and path.read_bytes() != data:
            raise RuntimeError(f"refusing to replace a different fixture at {path}")
    output_dir.mkdir(parents=True, exist_ok=True)
    for filename, data, _ in variants:
        path = output_dir / filename
        if not path.exists():
            path.write_bytes(data)
        print(f"Verified AVIF idat fixture: {path}")
    return output_dir / FILENAME


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    args = parser.parse_args()
    generate(args.output_dir, args.source)


if __name__ == "__main__":
    main()
