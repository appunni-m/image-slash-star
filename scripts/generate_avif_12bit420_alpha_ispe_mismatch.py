#!/usr/bin/env python3
"""Give a pinned 12-bit AVIF alpha item a smaller spatial extent."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path

from PIL import Image
import yaml

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes
from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/avif/high_bitdepth_still_12bit_420_alpha_odd_edge_17x9.avif"
FIXTURE = ROOT / "tests/fixtures/input/images/avif/error_12bit420_alpha_ispe_extent_mismatch.avif"
SOURCE_SHA256 = "ce408e4a66fe57b734544c2f30f17e90f725e2f2bae75d748f9eda75bdf23761"
FIXTURE_SHA256 = "9150d49f7f499f86c9e778a70d6525bd06a5509e409366d082097e6d7c4a2be6"
EXPECTED_ERROR = "Failed to decode frame 0: Decoding of alpha plane failed"
COLOR_EXTENT = (17, 9)
ALPHA_EXTENT = (16, 9)
EXTENT_RELOCATION = 20


def unique_payload(boxes: list[tuple[bytes, bytes, bytes]], kind: bytes) -> bytes:
    """Return the sole payload for a child box."""
    matches = [payload for child_kind, _, payload in boxes if child_kind == kind]
    if len(matches) != 1:
        raise RuntimeError(f"expected one {kind!r} box, found {len(matches)}")
    return matches[0]


def item_extent_records(payload: bytes) -> list[tuple[int, int, int]]:
    """Parse the pinned version-zero iloc layout and return item extents."""
    if payload[:4] != bytes(4):
        raise RuntimeError("12-bit alpha source iloc must be version zero with no flags")
    if len(payload) < 8:
        raise RuntimeError("12-bit alpha source iloc header is truncated")

    offset_size, length_size = payload[4] >> 4, payload[4] & 0x0F
    base_offset_size = payload[5] >> 4
    index_size = payload[5] & 0x0F
    if (offset_size, length_size, base_offset_size, index_size) != (4, 4, 0, 0):
        raise RuntimeError("12-bit alpha source iloc field widths changed")

    item_count = int.from_bytes(payload[6:8], "big")
    position = 8
    records = []
    for _ in range(item_count):
        if len(payload) - position < 4:
            raise RuntimeError("12-bit alpha source iloc item is truncated")
        item_id = int.from_bytes(payload[position : position + 2], "big")
        data_reference = int.from_bytes(payload[position + 2 : position + 4], "big")
        position += 4
        if data_reference != 0:
            raise RuntimeError("12-bit alpha source iloc uses an unexpected data reference")

        if len(payload) - position < 2:
            raise RuntimeError("12-bit alpha source iloc extent count is truncated")
        extent_count = int.from_bytes(payload[position : position + 2], "big")
        position += 2
        if extent_count != 1:
            raise RuntimeError("12-bit alpha source items must each have one extent")

        field_end = position + offset_size + length_size
        if field_end > len(payload):
            raise RuntimeError("12-bit alpha source iloc extent is truncated")
        offset = int.from_bytes(payload[position : position + offset_size], "big")
        position += offset_size
        length = int.from_bytes(payload[position : position + length_size], "big")
        position += length_size
        records.append((item_id, offset, length))

    if position != len(payload) or [record[0] for record in records] != [1, 2]:
        raise RuntimeError("12-bit alpha source iloc item layout changed")
    return records


def shift_item_extents(payload: bytes) -> bytes:
    """Rebase item data after the new 20-byte ispe property is inserted."""
    records = item_extent_records(payload)
    if [(item_id, offset, length) for item_id, offset, length in records] != [
        (1, 799, 617),
        (2, 426, 373),
    ]:
        raise RuntimeError("12-bit alpha source extents differ from their pinned layout")

    result = bytearray(payload)
    position = 8
    for _, _, _ in records:
        position += 4  # item_ID and data_reference_index
        position += 2  # extent_count
        offset = int.from_bytes(result[position : position + 4], "big")
        result[position : position + 4] = (offset + EXTENT_RELOCATION).to_bytes(4, "big")
        position += 8  # offset and extent_length
    return bytes(result)


def replace_alpha_ispe_association(payload: bytes) -> bytes:
    """Point only auxiliary item 2 at the newly appended 16x9 ispe property."""
    if payload[:8] != bytes((0, 0, 0, 0, 0, 0, 0, 2)):
        raise RuntimeError("12-bit alpha source ipma header or entry count changed")

    result = bytearray(payload)
    position = 8
    found_alpha = False
    for _ in range(2):
        if len(result) - position < 3:
            raise RuntimeError("12-bit alpha source ipma entry is truncated")
        item_id = int.from_bytes(result[position : position + 2], "big")
        association_count = result[position + 2]
        position += 3
        end = position + association_count
        if end > len(result):
            raise RuntimeError("12-bit alpha source ipma associations are truncated")
        associations = tuple(result[position:end])
        if item_id == 2:
            if associations != (1, 5, 0x86, 7):
                raise RuntimeError("12-bit alpha source property associations changed")
            result[position] = 0x88  # essential association to appended property 8
            found_alpha = True
        position = end

    if position != len(result) or not found_alpha:
        raise RuntimeError("12-bit alpha source ipma entries changed")
    return bytes(result)


def fixture_bytes(source: bytes) -> bytes:
    """Append a mismatched auxiliary ispe and rebase both item extents."""
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("12-bit alpha source differs from its pinned hash")

    top_level = read_boxes(source, 0, len(source))
    meta = unique_payload(top_level, b"meta")
    if meta[:4] != bytes(4):
        raise RuntimeError("12-bit alpha source meta must be a version-zero FullBox")
    meta_children = read_boxes(meta, 4, len(meta))
    iprp = unique_payload(meta_children, b"iprp")
    iprp_children = read_boxes(iprp, 0, len(iprp))
    ipco = unique_payload(iprp_children, b"ipco")
    properties = read_boxes(ipco, 0, len(ipco))
    expected_kinds = [b"ispe", b"pixi", b"av1C", b"colr", b"pixi", b"av1C", b"auxC"]
    if [kind for kind, _, _ in properties] != expected_kinds:
        raise RuntimeError("12-bit alpha source property order changed")
    color_ispe = properties[0][2]
    if color_ispe != bytes(4) + COLOR_EXTENT[0].to_bytes(4, "big") + COLOR_EXTENT[1].to_bytes(4, "big"):
        raise RuntimeError("12-bit alpha color ispe differs from its pinned 17x9 extent")

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == (b"meta", b"iloc"):
            return shift_item_extents(payload)
        if path == (b"meta", b"iprp", b"ipco"):
            alpha_ispe = (
                bytes(4)
                + ALPHA_EXTENT[0].to_bytes(4, "big")
                + ALPHA_EXTENT[1].to_bytes(4, "big")
            )
            return payload + pack_box(b"ispe", alpha_ispe)
        if path == (b"meta", b"iprp", b"ipma"):
            return replace_alpha_ispe_association(payload)
        return payload

    candidate = rewrite_boxes(source, transform)
    if len(candidate) != len(source) + EXTENT_RELOCATION:
        raise RuntimeError("12-bit alpha fixture length differs from its property insertion")
    if sha256(candidate).hexdigest() != FIXTURE_SHA256:
        raise RuntimeError("12-bit alpha fixture differs from its pinned output hash")

    rewritten_boxes = read_boxes(candidate, 0, len(candidate))
    rewritten_meta = read_boxes(unique_payload(rewritten_boxes, b"meta"), 4, len(unique_payload(rewritten_boxes, b"meta")))
    rewritten_iprp = read_boxes(unique_payload(rewritten_meta, b"iprp"), 0, len(unique_payload(rewritten_meta, b"iprp")))
    rewritten_ipco = read_boxes(unique_payload(rewritten_iprp, b"ipco"), 0, len(unique_payload(rewritten_iprp, b"ipco")))
    if [kind for kind, _, _ in rewritten_ipco] != expected_kinds + [b"ispe"]:
        raise RuntimeError("12-bit alpha fixture property list differs from the source plus ispe")
    new_ispe = rewritten_ipco[-1][2]
    if new_ispe != bytes(4) + ALPHA_EXTENT[0].to_bytes(4, "big") + ALPHA_EXTENT[1].to_bytes(4, "big"):
        raise RuntimeError("12-bit alpha fixture auxiliary ispe is not 16x9")

    original_iloc = unique_payload(meta_children, b"iloc")
    rewritten_iloc = unique_payload(rewritten_meta, b"iloc")
    old_records = item_extent_records(original_iloc)
    new_records = item_extent_records(rewritten_iloc)
    if new_records != [
        (item_id, offset + EXTENT_RELOCATION, length)
        for item_id, offset, length in old_records
    ]:
        raise RuntimeError("12-bit alpha fixture item extents were not consistently rebased")
    for (_, old_offset, old_length), (_, new_offset, new_length) in zip(old_records, new_records):
        if source[old_offset : old_offset + old_length] != candidate[new_offset : new_offset + new_length]:
            raise RuntimeError("12-bit alpha fixture changed encoded AV1 item bytes")

    original_mdat = unique_payload(top_level, b"mdat")
    rewritten_mdat = unique_payload(rewritten_boxes, b"mdat")
    if original_mdat != rewritten_mdat:
        raise RuntimeError("12-bit alpha fixture changed the encoded AV1 media payload")
    return candidate


def verify_oracle(candidate: bytes) -> None:
    """Confirm the pinned Pillow decoder reports the expected public failure."""
    with Image.open(BytesIO(candidate)) as image:
        if (image.format, image.mode, image.size) != ("AVIF", "RGBA", COLOR_EXTENT):
            raise RuntimeError("Pillow no longer opens the mismatch as a 17x9 RGBA AVIF")
        try:
            image.load()
        except RuntimeError as error:
            if str(error) != EXPECTED_ERROR:
                raise RuntimeError(f"unexpected Pillow AVIF decode error: {error}") from error
        else:
            raise RuntimeError("Pillow unexpectedly decoded the mismatched alpha extent")


def generate(check: bool = False) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    candidate = fixture_bytes(SOURCE.read_bytes())
    verify_oracle(candidate)

    if check:
        if not FIXTURE.is_file() or FIXTURE.read_bytes() != candidate:
            raise RuntimeError(f"fixture is missing or differs from generated bytes: {FIXTURE}")
    elif FIXTURE.exists():
        if FIXTURE.read_bytes() != candidate:
            raise RuntimeError(f"refusing to replace a different fixture at {FIXTURE}")
    else:
        FIXTURE.write_bytes(candidate)
    print(f"Verified {FIXTURE} ({len(candidate)} bytes, sha256={FIXTURE_SHA256})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    generate(check=arguments.check)


if __name__ == "__main__":
    main()
