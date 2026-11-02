#!/usr/bin/env python3
"""Add a conflicting duplicate primary-item mdcv property to a pinned AVIF."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path
import struct

from PIL import Image
import yaml

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes
from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/avif"
SOURCE = ASSETS / "icc_mdcv.avif"
SOURCE_SHA256 = "d06f268e6f7341d67e1bf62510681375bc40ea4815a27b6aa4e38172dde01882"
OUTPUT = ASSETS / "duplicate_mdcv_first_wins.avif"
OUTPUT_SHA256 = "a81d3f13b2412488464d461f9c81ab496ce5758bfba3f5f94de237b37b77faa8"
SOURCE_IPMA = bytes.fromhex("0000000000000001000106010283040506")
SOURCE_ILOC = bytes.fromhex("00000000440000010001000000010000015700000021")
SOURCE_MDCV = bytes.fromhex("33c286c41d4c0bb884d03e803d1340420098968000000032")


def unique_payload(
    boxes: list[tuple[bytes, bytes, bytes]], kind: bytes, context: str
) -> bytes:
    """Return one box payload after checking its pinned cardinality."""
    matches = [payload for box_kind, _, payload in boxes if box_kind == kind]
    if len(matches) != 1:
        raise RuntimeError(f"expected one {kind!r} box in {context}, found {len(matches)}")
    return matches[0]


def candidate_bytes() -> bytes:
    """Append a second associated mdcv property and relocate the item extent."""
    source = SOURCE.read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned AVIF source differs from its maintained hash")

    top = read_boxes(source, 0, len(source))
    meta = unique_payload(top, b"meta", "source file")
    meta_children = read_boxes(meta, 4, len(meta))
    iprp = unique_payload(meta_children, b"iprp", "source meta")
    iprp_children = read_boxes(iprp, 0, len(iprp))
    ipco = unique_payload(iprp_children, b"ipco", "source iprp")
    properties = read_boxes(ipco, 0, len(ipco))
    expected_kinds = [b"ispe", b"pixi", b"av1C", b"colr", b"colr", b"mdcv"]
    if [kind for kind, _, _ in properties] != expected_kinds:
        raise RuntimeError("pinned primary-item property order changed")
    if unique_payload(properties, b"mdcv", "source ipco") != SOURCE_MDCV:
        raise RuntimeError("pinned mastering-display property changed")
    if unique_payload(iprp_children, b"ipma", "source iprp") != SOURCE_IPMA:
        raise RuntimeError("pinned primary-item associations changed")
    if unique_payload(meta_children, b"iloc", "source meta") != SOURCE_ILOC:
        raise RuntimeError("pinned primary-item extent changed")

    duplicate = bytearray(SOURCE_MDCV)
    struct.pack_into(">H", duplicate, 0, 0x1234)
    duplicate_box = pack_box(b"mdcv", bytes(duplicate))
    delta = len(duplicate_box) + 1
    changed = {b"ipco": 0, b"ipma": 0, b"iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == (b"meta", b"iprp", b"ipco"):
            changed[b"ipco"] += 1
            return payload + duplicate_box
        if path == (b"meta", b"iprp", b"ipma"):
            changed[b"ipma"] += 1
            result = bytearray(payload)
            if payload != SOURCE_IPMA:
                raise RuntimeError("pinned primary-item associations changed")
            result[10] += 1
            result.append(0x07)
            return bytes(result)
        if path == (b"meta", b"iloc"):
            changed[b"iloc"] += 1
            if payload != SOURCE_ILOC:
                raise RuntimeError("pinned primary-item extent changed")
            result = bytearray(payload)
            offset = struct.unpack_from(">I", result, 14)[0]
            struct.pack_into(">I", result, 14, offset + delta)
            return bytes(result)
        return payload

    result = rewrite_boxes(source, transform)
    if changed != dict.fromkeys(changed, 1):
        raise RuntimeError("expected one ipco, ipma, and iloc mutation")
    if len(result) != len(source) + delta:
        raise RuntimeError("AVIF metadata mutation changed an unexpected byte count")

    result_top = read_boxes(result, 0, len(result))
    result_meta = unique_payload(result_top, b"meta", "mutated file")
    result_meta_children = read_boxes(result_meta, 4, len(result_meta))
    result_iprp = unique_payload(result_meta_children, b"iprp", "mutated meta")
    result_iprp_children = read_boxes(result_iprp, 0, len(result_iprp))
    result_ipco = unique_payload(result_iprp_children, b"ipco", "mutated iprp")
    result_properties = read_boxes(result_ipco, 0, len(result_ipco))
    if [kind for kind, _, _ in result_properties] != expected_kinds + [b"mdcv"]:
        raise RuntimeError("mutated primary-item property table differs from its recipe")
    result_ipma = unique_payload(result_iprp_children, b"ipma", "mutated iprp")
    expected_ipma = SOURCE_IPMA[:10] + b"\x07" + SOURCE_IPMA[11:] + b"\x07"
    if result_ipma != expected_ipma:
        raise RuntimeError("mutated primary item does not associate both mdcv properties")

    source_mdat = unique_payload(top, b"mdat", "source file")
    result_mdat = unique_payload(result_top, b"mdat", "mutated file")
    if result_mdat != source_mdat:
        raise RuntimeError("AVIF mutation changed encoded AV1 samples")
    result_iloc = unique_payload(result_meta_children, b"iloc", "mutated meta")
    expected_iloc = bytearray(SOURCE_ILOC)
    original_offset = struct.unpack_from(">I", SOURCE_ILOC, 14)[0]
    struct.pack_into(">I", expected_iloc, 14, original_offset + delta)
    if result_iloc != expected_iloc:
        raise RuntimeError("mutated primary-item extent differs from its rebased pin")
    extent_offset = struct.unpack_from(">I", result_iloc, 14)[0]
    extent_length = struct.unpack_from(">I", result_iloc, 18)[0]
    if result[extent_offset : extent_offset + extent_length] != result_mdat:
        raise RuntimeError("primary-item extent no longer selects the unchanged AV1 payload")

    digest = hashlib.sha256(result).hexdigest()
    if digest != OUTPUT_SHA256:
        raise RuntimeError(f"mutated AVIF hash changed: {digest}")
    return result


def verify_pillow(source: bytes, candidate: bytes) -> None:
    """Confirm Pillow accepts the mutation and keeps the source image result."""
    def observe(data: bytes) -> tuple[str, tuple[int, int], bytes, bytes | None]:
        with Image.open(BytesIO(data)) as image:
            image.load()
            return image.mode, image.size, image.tobytes(), image.info.get("icc_profile")

    if observe(source) != observe(candidate):
        raise RuntimeError("Pillow pixels or ICC metadata changed for duplicate mdcv")


def generate(*, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    source = SOURCE.read_bytes()
    candidate = candidate_bytes()
    verify_pillow(source, candidate)
    if check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != candidate:
            raise RuntimeError(f"generated fixture is missing or stale: {OUTPUT}")
    elif OUTPUT.exists():
        if OUTPUT.read_bytes() != candidate:
            raise RuntimeError(f"refusing to replace a different fixture: {OUTPUT}")
    else:
        OUTPUT.write_bytes(candidate)
    print(f"{OUTPUT}: {len(candidate)} bytes, sha256={OUTPUT_SHA256}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify fixture bytes without writing")
    args = parser.parse_args()
    generate(check=args.check)


if __name__ == "__main__":
    main()
