#!/usr/bin/env python3
"""Generate Pillow-parity fixtures for AVIF non-primary item properties."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path
import struct

from PIL import Image
import yaml

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes
from generate_avif_alpha_missing_auxiliary_ispe_fixture import SOURCE, SOURCE_SHA256
from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
INPUTS = ROOT / "tests/fixtures/input/images/avif"
SOURCE_ILOC = bytes.fromhex(
    "0000000044000002000100000001000002d700001652000200000001000001c90000010e"
)
PROPERTY_KINDS = (
    b"ispe",
    b"pixi",
    b"av1C",
    b"colr",
    b"ispe",
    b"pixi",
    b"av1C",
    b"auxC",
)
ALPHA_ASSOCIATIONS = bytes((5, 6, 0x87, 8))
# ICC payload from the pinned primary-item `prof` property in icc_profile.avif.
ALPHA_ITEM_ICC_PROFILE = bytes.fromhex(
    "70726f6670696c6c6f772d72732d746573742d70726f66696c65"
)
BoxList = list[tuple[bytes, bytes, bytes]]
FIXTURES = {
    "duplicate_non_primary_ispe_first_wins": {
        "asset": INPUTS / "duplicate_non_primary_ispe_first_wins.avif",
        "sha256": "c1e7a5c8654652a117bb6e11d1e3ad1715ba18adb5191a1a53975be70ea86c9c",
        "previous_sha256": "c60e2d9f40d53e0ed8f264c3a8376176ffa8e1aa972f69230f9cbdf087e4ef4c",
        "kind": b"ispe",
        "payload": struct.pack(">III", 0, 32, 64),
        "mode": "RGBA",
    },
    "duplicate_non_primary_pixi_first_wins": {
        "asset": INPUTS / "duplicate_non_primary_pixi_first_wins.avif",
        "sha256": "ed581a79d3937c870b06bd1a830ddc31c4af20660da249059fc1b549c4194d52",
        "previous_sha256": "5e9547e3975604df2d34263ecdaeaa9d49199be11e0a70519e09497cc9dd7b73",
        "kind": b"pixi",
        "payload": bytes((0, 0, 0, 0, 1, 16)),
        "mode": "RGBA",
    },
    "duplicate_non_primary_av1c_first_wins": {
        "asset": INPUTS / "duplicate_non_primary_av1c_first_wins.avif",
        "sha256": "5c06b67ebc29379bc0dd58c61865dd28927ab3669d59d75a08afcdeb7bcd832f",
        "previous_sha256": "f4d7c49db1fbc76954e8a6d09fe4a7c5eb4796b0a946ea85ddbd47a123d74d30",
        "kind": b"av1C",
        "payload": bytes.fromhex("81005c00"),
        "mode": "RGBA",
    },
    "alpha_item_icc_profile_ignored": {
        "asset": INPUTS / "alpha_item_icc_profile_ignored.avif",
        "sha256": "4dfa9e87d65e9365b3e38ab13e67bdceadd9b8eb5ddef273da03eb3057b553d3",
        "kind": b"colr",
        "payload": ALPHA_ITEM_ICC_PROFILE,
        "mode": "RGBA",
    },
}


def box_payload(boxes: BoxList, kind: bytes, context: str) -> bytes:
    """Return the payload of exactly one child box."""
    matches = [payload for box_kind, _, payload in boxes if box_kind == kind]
    if len(matches) != 1:
        raise RuntimeError(f"expected one {kind!r} box in {context}")
    return matches[0]


def append_item_association(payload: bytes, item_id: int, index: int) -> bytes:
    """Append one narrow, non-essential association to a pinned item."""
    if len(payload) < 8 or payload[:4] != bytes(4):
        raise RuntimeError("source ipma must use version zero and zero flags")
    entry_count = int.from_bytes(payload[4:8], "big")
    output = bytearray(payload[:8])
    position = 8
    matches = 0
    for _ in range(entry_count):
        if len(payload) - position < 3:
            raise RuntimeError("source ipma entry is truncated")
        entry_id = int.from_bytes(payload[position : position + 2], "big")
        entry_bytes = payload[position : position + 2]
        position += 2
        association_count = payload[position]
        position += 1
        if len(payload) - position < association_count:
            raise RuntimeError("source ipma associations are truncated")
        associations = payload[position : position + association_count]
        position += association_count

        if entry_id == item_id:
            if matches or associations != ALPHA_ASSOCIATIONS:
                raise RuntimeError("alpha item associations differ from their pin")
            if association_count == 0xFF:
                raise RuntimeError("alpha item association count cannot be extended")
            output.extend(entry_bytes)
            output.append(association_count + 1)
            output.extend(associations)
            output.append(index)
            matches += 1
        else:
            output.extend(entry_bytes)
            output.append(association_count)
            output.extend(associations)

    if position != len(payload) or matches != 1:
        raise RuntimeError("source ipma has unexpected trailing data or item count")
    return bytes(output)


def relocate_file_extents(payload: bytes, delta: int) -> tuple[bytes, list[tuple[int, int]]]:
    """Rebase the two pinned file-backed alpha extents after metadata growth."""
    if payload != SOURCE_ILOC or payload[0] != 0 or payload[4:6] != b"\x44\x00":
        raise RuntimeError("alpha iloc differs from its pinned version-zero layout")

    result = bytearray(payload)
    item_count = int.from_bytes(payload[6:8], "big")
    position = 8
    extents = []
    for _ in range(item_count):
        item_id = int.from_bytes(payload[position : position + 2], "big")
        position += 2
        data_reference = int.from_bytes(payload[position : position + 2], "big")
        position += 2
        extent_count = int.from_bytes(payload[position : position + 2], "big")
        position += 2
        if data_reference != 0 or extent_count != 1 or item_id not in (1, 2):
            raise RuntimeError("alpha iloc item records differ from their pin")

        offset = int.from_bytes(payload[position : position + 4], "big")
        position += 4
        length = int.from_bytes(payload[position : position + 4], "big")
        position += 4
        relocated = offset + delta
        if relocated > 0xFFFF_FFFF:
            raise RuntimeError("relocated alpha item extent exceeds its 32-bit field")
        result[position - 8 : position - 4] = relocated.to_bytes(4, "big")
        extents.append((offset, length))

    if position != len(payload) or len(extents) != 2:
        raise RuntimeError("alpha iloc has unexpected trailing data or extent count")
    return bytes(result), extents


def source_boxes(
    source: bytes,
) -> tuple[BoxList, bytes, BoxList, bytes, BoxList, bytes, BoxList, bytes, bytes]:
    """Read the pinned meta, property, and extent structures."""
    top_level = read_boxes(source, 0, len(source))
    meta = box_payload(top_level, b"meta", "alpha source")
    if meta[:4] != bytes(4):
        raise RuntimeError("alpha meta FullBox differs from its pinned version-zero layout")
    meta_children = read_boxes(meta, 4, len(meta))
    iprp = box_payload(meta_children, b"iprp", "alpha meta")
    iprp_children = read_boxes(iprp, 0, len(iprp))
    ipco = box_payload(iprp_children, b"ipco", "alpha iprp")
    ipco_properties = read_boxes(ipco, 0, len(ipco))
    ipma = box_payload(iprp_children, b"ipma", "alpha iprp")
    iloc = box_payload(meta_children, b"iloc", "alpha meta")
    return top_level, meta, meta_children, iprp, iprp_children, ipco, ipco_properties, ipma, iloc


def fixture_bytes(source: bytes, case: dict) -> bytes:
    """Add an item property to auxiliary item 2 and rebase its file extents."""
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("alpha AVIF differs from its pinned source hash")
    top_level, _, _, _, _, _, properties, ipma, iloc = source_boxes(source)
    if tuple(kind for kind, _, _ in properties) != PROPERTY_KINDS:
        raise RuntimeError("alpha AVIF property table differs from its pinned order")

    property_index = len(properties) + 1
    if property_index > 0x7F:
        raise RuntimeError("duplicate property index does not fit narrow ipma associations")
    if case["kind"] == b"av1C":
        auxiliary_av1c = properties[6]
        if auxiliary_av1c[0] != b"av1C" or auxiliary_av1c[2] != bytes.fromhex("81001c00"):
            raise RuntimeError("source auxiliary av1C differs from its configuration pin")
    property_box = pack_box(case["kind"], case["payload"])
    association = bytes((property_index,))
    delta = len(property_box) + len(association)
    relocated_iloc, original_extents = relocate_file_extents(iloc, delta)
    expected_ipma = append_item_association(ipma, 2, property_index)
    changed = {"ipco": 0, "ipma": 0, "iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == (b"meta", b"iprp", b"ipco"):
            changed["ipco"] += 1
            return payload + property_box
        if path == (b"meta", b"iprp", b"ipma"):
            changed["ipma"] += 1
            result = append_item_association(payload, 2, property_index)
            if result != expected_ipma:
                raise RuntimeError("mutated ipma differs from its pinned association recipe")
            return result
        if path == (b"meta", b"iloc"):
            changed["iloc"] += 1
            return relocate_file_extents(payload, delta)[0]
        return payload

    result = rewrite_boxes(source, transform)
    if changed != {"ipco": 1, "ipma": 1, "iloc": 1}:
        raise RuntimeError("expected one ipco, ipma, and iloc mutation")
    if len(result) != len(source) + delta:
        raise RuntimeError("duplicate-property mutation changed an unexpected byte count")
    if sha256(result).hexdigest() != case["sha256"]:
        raise RuntimeError("duplicate-property AVIF differs from its pinned output")

    (
        result_top,
        _,
        _,
        _,
        _,
        result_ipco,
        _,
        result_ipma,
        result_iloc,
    ) = source_boxes(result)
    result_properties = read_boxes(result_ipco, 0, len(result_ipco))
    expected_property_kinds = PROPERTY_KINDS + (case["kind"],)
    if tuple(kind for kind, _, _ in result_properties) != expected_property_kinds:
        raise RuntimeError("mutated property table differs from its appended-property recipe")
    if result_ipma != expected_ipma or result_iloc != relocated_iloc:
        raise RuntimeError("mutated item associations or locations differ from their pins")

    source_mdat = box_payload(top_level, b"mdat", "alpha source")
    result_mdat = box_payload(result_top, b"mdat", "mutated file")
    if result_mdat != source_mdat:
        raise RuntimeError("duplicate-property mutation changed encoded AV1 samples")
    for offset, length in original_extents:
        if source[offset : offset + length] != result[offset + delta : offset + delta + length]:
            raise RuntimeError("relocated item extent no longer selects its original payload")

    return result


def verify_oracle(candidate: bytes, expected_mode: str) -> None:
    """Confirm Pillow accepts the file and preserves the source image pixels."""
    try:
        with Image.open(BytesIO(candidate)) as image:
            image.load()
            result_format = image.format
            result_size = image.size
            result_mode = image.mode
            result_pixels = image.tobytes()
        with Image.open(SOURCE) as reference:
            reference.load()
            reference_result = (
                reference.format,
                reference.size,
                reference.mode,
                reference.tobytes(),
            )
    except Exception as error:
        raise RuntimeError(f"Pillow rejected non-primary-property input: {error}") from error
    if result_format != "AVIF" or result_size != (64, 64):
        raise RuntimeError(f"unexpected Pillow AVIF result: {result_format} {result_size}")
    if result_mode != expected_mode:
        raise RuntimeError(f"expected Pillow mode {expected_mode}, got {result_mode}")
    if reference_result[:3] != ("AVIF", (64, 64), expected_mode):
        raise RuntimeError("pinned alpha source no longer has the expected Pillow result")
    if result_pixels != reference_result[3]:
        raise RuntimeError("non-primary property mutation changed Pillow-decoded pixels")


def generate(check: bool = False) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    source = SOURCE.read_bytes()
    for case_id, case in FIXTURES.items():
        candidate = fixture_bytes(source, case)
        verify_oracle(candidate, case["mode"])
        asset = case["asset"]
        if check:
            if not asset.is_file() or asset.read_bytes() != candidate:
                raise RuntimeError(f"fixture is missing or stale: {asset}")
        elif asset.exists():
            existing = asset.read_bytes()
            if existing != candidate:
                previous_sha256 = case.get("previous_sha256")
                if not previous_sha256 or sha256(existing).hexdigest() != previous_sha256:
                    raise RuntimeError(f"refusing to replace a different fixture: {asset}")
                asset.write_bytes(candidate)
        else:
            asset.write_bytes(candidate)
        print(f"Verified {asset} ({case_id}, sha256={case['sha256']})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail instead of writing stale fixtures",
    )
    generate(check=parser.parse_args().check)


if __name__ == "__main__":
    main()
