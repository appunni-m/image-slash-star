#!/usr/bin/env python3
"""Generate AVIF parity fixtures for edge cases in the ``irot`` property."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "primary_item_irot.avif"
SOURCE_SHA256 = "df1fadfd3b7780e3c41825352ee5768731a6753dac613412dfa94d6715f93f7f"
SOURCE_ILOC = bytes.fromhex("00000000440000010001000000010000011d00000028")
IROT_SOURCE = b"\x03"
FIXTURES = (
    (
        "primary_item_irot_trailing_payload.avif",
        b"\x03\x00",
        1,
        "49bb8805f73129fd022c97525f03faebfaf2517e74e90423180b3a06bdc6c5c9",
    ),
    (
        "error_primary_item_irot_invalid_value.avif",
        b"\x04",
        0,
        "feb3a381de36eac15916eaab142e4c4aff6e6243bb421565465fa98ab9ae376a",
    ),
)


def candidate_bytes(replacement: bytes, extent_delta: int, expected_hash: str) -> bytes:
    """Apply one bounded ``irot`` mutation to the pinned primary-item fixture."""
    source = SOURCE.read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned AVIF `irot` source hash changed")

    source_top = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in source_top] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("pinned AVIF top-level box layout changed")
    source_media = [complete for kind, complete, _ in source_top if kind == b"mdat"]
    changed = {"ipco": 0, "irot": 0, "iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == (b"meta", b"iprp", b"ipco"):
            changed["ipco"] += 1
            properties = []
            for kind, complete, property_payload in read_boxes(payload, 0, len(payload)):
                if kind == b"irot":
                    changed["irot"] += 1
                    if property_payload != IROT_SOURCE:
                        raise RuntimeError("pinned ``irot`` payload differs from 270 degrees")
                    properties.append(pack_box(kind, replacement))
                else:
                    properties.append(complete)
            return b"".join(properties)

        if extent_delta and path == (b"meta", b"iloc"):
            changed["iloc"] += 1
            if payload != SOURCE_ILOC:
                raise RuntimeError("pinned AVIF primary-item extent layout changed")
            result = bytearray(payload)
            extent_offset = struct.unpack_from(">I", result, 14)[0]
            struct.pack_into(">I", result, 14, extent_offset + extent_delta)
            return bytes(result)

        return payload

    candidate = rewrite_boxes(source, transform)
    if changed != {"ipco": 1, "irot": 1, "iloc": extent_delta}:
        raise RuntimeError(f"unexpected AVIF rewrite counts: {changed}")
    if len(candidate) != len(source) + extent_delta:
        raise RuntimeError("AVIF metadata growth differs from the selected `irot` mutation")
    candidate_top = read_boxes(candidate, 0, len(candidate))
    candidate_media = [complete for kind, complete, _ in candidate_top if kind == b"mdat"]
    if candidate_media != source_media:
        raise RuntimeError("appending ``irot`` data changed encoded AV1 samples")
    if hashlib.sha256(candidate).hexdigest() != expected_hash:
        raise RuntimeError("generated AVIF fixture differs from its pinned hash")
    return candidate


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    args = parser.parse_args()
    for filename, replacement, extent_delta, expected_hash in FIXTURES:
        output = ASSETS / filename
        candidate = candidate_bytes(replacement, extent_delta, expected_hash)

        if args.check:
            if not output.is_file() or output.read_bytes() != candidate:
                raise RuntimeError(
                    f"{output.relative_to(ROOT)} differs from its maintained recipe"
                )
        elif output.exists():
            if output.read_bytes() != candidate:
                raise RuntimeError(f"{output} exists with different bytes; refusing to overwrite")
        else:
            output.write_bytes(candidate)

        print(f"{output.relative_to(ROOT)}: {len(candidate)} bytes, sha256={expected_hash}")


if __name__ == "__main__":
    main()
