#!/usr/bin/env python3
"""Add an empty MIME XMP item to a pinned, Pillow-decodable AVIF."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path
import struct

from PIL import Image
import yaml

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes
from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/avif"
SOURCE = ASSETS / "baseline.avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
SOURCE_IINF = bytes.fromhex(
    "0000000000010000001a696e6665020000000001000061763031436f6c6f7200"
)
SOURCE_ILOC = bytes.fromhex("00000000440000010001000000010000011a00000aeb")
XMP_INFE_PAYLOAD = bytes.fromhex("0200000000020000") + b"mimeXMP\x00application/rdf+xml\x00"
EMPTY_XMP_ILOC_RECORD = bytes.fromhex("000200000000")
OUTPUT = ASSETS / "empty_xmp_metadata_item_extent.avif"
OUTPUT_SHA256 = "98c1a14618e4577354d82b758a8840d079990364b10ce94b6ef49debc87c1642"
METADATA_PIXI_OUTPUT = ASSETS / "xmp_metadata_item_pixi_only.avif"
METADATA_PIXI_OUTPUT_SHA256 = "2626a70cdd4b505b30e55245e9068ba17994b0c4bafdf57c00221c54940d8e12"


def unique_payload(
    boxes: list[tuple[bytes, bytes, bytes]], kind: bytes, context: str
) -> bytes:
    """Return one box payload after checking its cardinality."""
    matches = [payload for box_kind, _, payload in boxes if box_kind == kind]
    if len(matches) != 1:
        raise RuntimeError(f"expected one {kind!r} box in {context}, found {len(matches)}")
    return matches[0]


def candidate_bytes() -> bytes:
    """Append a zero-extent XMP item and rebase the existing file extent."""
    source = SOURCE.read_bytes()
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned AVIF baseline differs from its maintained hash")
    top = read_boxes(source, 0, len(source))
    meta = unique_payload(top, b"meta", "source file")
    meta_children = read_boxes(meta, 4, len(meta))
    if unique_payload(meta_children, b"iinf", "source meta") != SOURCE_IINF:
        raise RuntimeError("pinned primary item-information records changed")
    if unique_payload(meta_children, b"iloc", "source meta") != SOURCE_ILOC:
        raise RuntimeError("pinned primary item-location record changed")

    xmp_item = pack_box(b"infe", XMP_INFE_PAYLOAD)
    delta = len(xmp_item) + len(EMPTY_XMP_ILOC_RECORD)
    changed = {b"iinf": 0, b"iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == (b"meta", b"iinf"):
            changed[b"iinf"] += 1
            if payload != SOURCE_IINF:
                raise RuntimeError("pinned primary item-information records changed")
            result = bytearray(payload)
            count = struct.unpack_from(">H", result, 4)[0]
            struct.pack_into(">H", result, 4, count + 1)
            return bytes(result) + xmp_item
        if path == (b"meta", b"iloc"):
            changed[b"iloc"] += 1
            if payload != SOURCE_ILOC:
                raise RuntimeError("pinned primary item-location record changed")
            result = bytearray(payload)
            item_count = struct.unpack_from(">H", result, 6)[0]
            struct.pack_into(">H", result, 6, item_count + 1)
            offset = struct.unpack_from(">I", result, 14)[0]
            struct.pack_into(">I", result, 14, offset + delta)
            return bytes(result) + EMPTY_XMP_ILOC_RECORD
        return payload

    candidate = rewrite_boxes(source, transform)
    if changed != dict.fromkeys(changed, 1):
        raise RuntimeError("expected one iinf and iloc mutation")
    if len(candidate) != len(source) + delta:
        raise RuntimeError("adding empty XMP metadata changed an unexpected byte count")

    result_top = read_boxes(candidate, 0, len(candidate))
    result_meta = unique_payload(result_top, b"meta", "mutated file")
    result_meta_children = read_boxes(result_meta, 4, len(result_meta))
    result_iinf = unique_payload(result_meta_children, b"iinf", "mutated meta")
    if struct.unpack_from(">H", result_iinf, 4)[0] != 2:
        raise RuntimeError("mutated item-information count differs from its recipe")
    infe_boxes = read_boxes(result_iinf, 6, len(result_iinf))
    if [kind for kind, _, _ in infe_boxes] != [b"infe", b"infe"]:
        raise RuntimeError("mutated item-information entries differ from their recipe")
    if infe_boxes[-1][2] != XMP_INFE_PAYLOAD:
        raise RuntimeError("mutated item is not the standard XMP MIME item")

    result_iloc = unique_payload(result_meta_children, b"iloc", "mutated meta")
    expected_iloc = bytearray(SOURCE_ILOC)
    struct.pack_into(">H", expected_iloc, 6, 2)
    original_offset = struct.unpack_from(">I", SOURCE_ILOC, 14)[0]
    struct.pack_into(">I", expected_iloc, 14, original_offset + delta)
    expected_iloc.extend(EMPTY_XMP_ILOC_RECORD)
    if result_iloc != expected_iloc:
        raise RuntimeError("mutated item-location records differ from the recipe")

    source_mdat = unique_payload(top, b"mdat", "source file")
    result_mdat = unique_payload(result_top, b"mdat", "mutated file")
    if result_mdat != source_mdat:
        raise RuntimeError("AVIF mutation changed encoded AV1 samples")
    extent_offset = struct.unpack_from(">I", result_iloc, 14)[0]
    extent_length = struct.unpack_from(">I", result_iloc, 18)[0]
    if candidate[extent_offset : extent_offset + extent_length] != result_mdat:
        raise RuntimeError("primary-item extent no longer selects the unchanged AV1 payload")

    digest = sha256(candidate).hexdigest()
    if digest != OUTPUT_SHA256:
        raise RuntimeError(f"mutated AVIF hash changed: {digest}")
    return candidate


def candidate_with_metadata_pixi(base: bytes) -> bytes:
    """Associate the existing ``pixi`` property with the empty XMP item."""
    if sha256(base).hexdigest() != OUTPUT_SHA256:
        raise RuntimeError("empty-XMP AVIF differs from its pinned output hash")
    top = read_boxes(base, 0, len(base))
    meta = unique_payload(top, b"meta", "empty-XMP file")
    meta_children = read_boxes(meta, 4, len(meta))
    iprp = unique_payload(meta_children, b"iprp", "empty-XMP meta")
    iprp_children = read_boxes(iprp, 0, len(iprp))
    ipco = unique_payload(iprp_children, b"ipco", "empty-XMP iprp")
    properties = read_boxes(ipco, 0, len(ipco))
    if len(properties) < 2 or properties[1][0] != b"pixi":
        raise RuntimeError("pinned property index 2 is no longer the pixi property")

    expected_ipma = bytes.fromhex("000000000000000100010401028304")
    ipma = unique_payload(iprp_children, b"ipma", "empty-XMP iprp")
    if ipma != expected_ipma:
        raise RuntimeError("pinned empty-XMP property associations changed")

    delta = len(pack_box(b"infe", XMP_INFE_PAYLOAD)) + len(EMPTY_XMP_ILOC_RECORD)
    expected_iloc = bytearray(SOURCE_ILOC)
    struct.pack_into(">H", expected_iloc, 6, 2)
    original_offset = struct.unpack_from(">I", SOURCE_ILOC, 14)[0]
    struct.pack_into(">I", expected_iloc, 14, original_offset + delta)
    expected_iloc.extend(EMPTY_XMP_ILOC_RECORD)
    if unique_payload(meta_children, b"iloc", "empty-XMP meta") != expected_iloc:
        raise RuntimeError("pinned empty-XMP item locations changed")

    changed = {b"ipma": 0, b"iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == (b"meta", b"iprp", b"ipma"):
            changed[b"ipma"] += 1
            return (
                payload[:4]
                + struct.pack(">I", 2)
                + payload[8:]
                + bytes.fromhex("00020102")
            )
        if path == (b"meta", b"iloc"):
            changed[b"iloc"] += 1
            result = bytearray(payload)
            offset = struct.unpack_from(">I", result, 14)[0]
            struct.pack_into(">I", result, 14, offset + 4)
            return bytes(result)
        return payload

    candidate = rewrite_boxes(base, transform)
    if changed != {b"ipma": 1, b"iloc": 1}:
        raise RuntimeError("expected one ipma and iloc mutation")
    if len(candidate) != len(base) + 4:
        raise RuntimeError("adding the XMP property association changed an unexpected byte count")

    result_top = read_boxes(candidate, 0, len(candidate))
    result_meta = unique_payload(result_top, b"meta", "mutated file")
    result_children = read_boxes(result_meta, 4, len(result_meta))
    result_iprp = unique_payload(result_children, b"iprp", "mutated meta")
    result_iprp_children = read_boxes(result_iprp, 0, len(result_iprp))
    if unique_payload(result_iprp_children, b"ipma", "mutated iprp") != (
        expected_ipma[:4] + struct.pack(">I", 2) + expected_ipma[8:] + bytes.fromhex("00020102")
    ):
        raise RuntimeError("mutated property associations differ from the recipe")

    result_iloc = unique_payload(result_children, b"iloc", "mutated meta")
    expected_iloc[14:18] = struct.pack(">I", original_offset + delta + 4)
    if result_iloc != expected_iloc:
        raise RuntimeError("mutated item locations differ from the recipe")
    if unique_payload(result_top, b"mdat", "mutated file") != unique_payload(
        top, b"mdat", "empty-XMP file"
    ):
        raise RuntimeError("associating pixi with XMP changed encoded AV1 samples")

    digest = sha256(candidate).hexdigest()
    if digest != METADATA_PIXI_OUTPUT_SHA256:
        raise RuntimeError(f"mutated AVIF hash changed: {digest}")
    return candidate


def pillow_observation(data: bytes) -> tuple[str, tuple[int, int], bytes, dict[str, object]]:
    """Return Pillow's decoded pixels and image information."""
    with Image.open(BytesIO(data)) as image:
        image.load()
        return image.mode, image.size, image.tobytes(), dict(image.info)


def verify_pillow(source: bytes, candidate: bytes) -> None:
    """Require Pillow to ignore the empty metadata item and preserve pixels."""
    source_mode, source_size, source_pixels, source_info = pillow_observation(source)
    mode, size, pixels, info = pillow_observation(candidate)
    if (mode, size, pixels) != (source_mode, source_size, source_pixels):
        raise RuntimeError("Pillow pixels changed for the empty-XMP mutation")
    if info != source_info or "xmp" in info:
        raise RuntimeError("Pillow exposed the empty XMP item or changed image information")


def verify_metadata_pixi_pillow(base: bytes, candidate: bytes) -> None:
    """Require the XMP property association to preserve Pillow's full observation."""
    if pillow_observation(candidate) != pillow_observation(base):
        raise RuntimeError("Pillow observation changed for the XMP pixi association")


def generate(*, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    source = SOURCE.read_bytes()
    empty_xmp = candidate_bytes()
    metadata_pixi = candidate_with_metadata_pixi(empty_xmp)
    verify_pillow(source, empty_xmp)
    verify_metadata_pixi_pillow(empty_xmp, metadata_pixi)
    for path, candidate, expected_sha256 in (
        (OUTPUT, empty_xmp, OUTPUT_SHA256),
        (METADATA_PIXI_OUTPUT, metadata_pixi, METADATA_PIXI_OUTPUT_SHA256),
    ):
        if check:
            if not path.is_file() or path.read_bytes() != candidate:
                raise RuntimeError(f"generated fixture is missing or stale: {path}")
        elif path.exists():
            if path.read_bytes() != candidate:
                raise RuntimeError(f"refusing to replace a different fixture: {path}")
        else:
            path.write_bytes(candidate)
        print(f"{path}: {len(candidate)} bytes, sha256={expected_sha256}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify fixture bytes without writing")
    args = parser.parse_args()
    generate(check=args.check)


if __name__ == "__main__":
    main()
