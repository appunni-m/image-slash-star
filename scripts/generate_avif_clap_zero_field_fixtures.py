#!/usr/bin/env python3
"""Create AVIF clean-aperture edge cases accepted and ignored by Pillow."""

from __future__ import annotations

import hashlib
import struct
from pathlib import Path

from generate_avif_zero_width_clap_fixture import (
    EXPECTED_SOURCE_SHA256,
    SOURCE,
    append_zero_width_clap,
    pack_box,
    read_boxes,
    verify_pillow,
)


ROOT = Path(__file__).resolve().parent.parent
OUTPUT_DIRECTORY = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
CASES = (
    (
        "clap_zero_width_denominator_ignored.avif",
        4,
        "8641c0eff26bfdf83f998e2a13737eeea8dbff30d04c818109a358d3a267b4e1",
    ),
    (
        "clap_zero_height_numerator_ignored.avif",
        8,
        "ec0031d6d620d303a05937051504d621498dfe73920c51fb072e03c7746fbe2c",
    ),
    (
        "clap_zero_height_denominator_ignored.avif",
        12,
        "1004a811270a222f0072637fafc2b1004f25084a6f8f1b34976696595fefefab",
    ),
    (
        "clap_zero_horizontal_offset_denominator_ignored.avif",
        20,
        "9d67878e798b93a6fe551fdd492701a501d4744d847f26c670e5c793c9a367c3",
    ),
    (
        "clap_zero_vertical_offset_denominator_ignored.avif",
        28,
        "662fca402bf3a78537aaaadbe101c5ceaee736a064ddcaa46ff5931ec17971c2",
    ),
)
TRAILING_PAYLOAD_CASES = (
    (
        "clap_trailing_payload_byte_ignored.avif",
        b"\x00",
        "ece2fc8f98cc42f31a8806690ac415460d5eadf96c93492ba2d710c0b2e22bde",
    ),
    (
        "clap_trailing_payload_16_bytes_ignored.avif",
        bytes(16),
        "93038a983d242ee2204d866b63a1811573acc7f8e38699595d328ccdf49cf2d7",
    ),
)


def zero_field_clap(source: bytes, field_offset: int) -> bytes:
    """Set valid dimensions, then zero one selected clean-aperture field."""
    fixture = bytearray(append_zero_width_clap(source))
    if fixture.count(b"clap") != 1:
        raise RuntimeError("expected exactly one clean-aperture property")
    clap_payload = fixture.index(b"clap") + 4
    struct.pack_into(">II", fixture, clap_payload, 128, 1)
    struct.pack_into(">I", fixture, clap_payload + field_offset, 0)
    return bytes(fixture)


def write_or_verify(path: Path, expected: bytes) -> None:
    if path.exists():
        if path.read_bytes() != expected:
            raise RuntimeError(f"refusing to replace a different fixture at {path}")
        print(f"Verified existing fixture: {path}")
        return

    path.write_bytes(expected)
    print(f"Wrote verified fixture: {path}")


def append_clap_trailing_payload(source: bytes, trailing: bytes) -> bytes:
    """Add trailing bytes and repair every enclosing AVIF box size."""
    fixture = bytearray(source)
    if fixture.count(b"clap") != 1:
        raise RuntimeError("expected exactly one clean-aperture property")
    struct.pack_into(">I", fixture, fixture.index(b"clap") + 4, 128)
    source = bytes(fixture)

    top_level = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in top_level] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("baseline AVIF top-level boxes differ from the pinned layout")
    ftyp, meta, mdat = (complete for _, complete, _ in top_level)

    rebuilt_meta_children = []
    for kind, complete, payload in read_boxes(meta, 12, len(meta)):
        if kind == b"iloc":
            location = bytearray(payload)
            if len(location) != 22 or int.from_bytes(location[14:18], "big") != 0x143:
                raise RuntimeError("baseline AVIF iloc differs from the pinned layout")
            location[14:18] = (0x143 + len(trailing)).to_bytes(4, "big")
            rebuilt_meta_children.append(pack_box(kind, bytes(location)))
        elif kind == b"iprp":
            rebuilt_properties = []
            properties = read_boxes(payload, 0, len(payload))
            if [property_kind for property_kind, _, _ in properties] != [b"ipco", b"ipma"]:
                raise RuntimeError("baseline AVIF iprp differs from the pinned layout")
            for property_kind, property_box, property_payload in properties:
                if property_kind != b"ipco":
                    rebuilt_properties.append(property_box)
                    continue

                rebuilt_items = []
                clap_count = 0
                for item_kind, item_box, item_payload in read_boxes(
                    property_payload, 0, len(property_payload)
                ):
                    if item_kind == b"clap":
                        clap_count += 1
                        rebuilt_items.append(pack_box(item_kind, item_payload + trailing))
                    else:
                        rebuilt_items.append(item_box)
                if clap_count != 1:
                    raise RuntimeError("expected exactly one clean-aperture property")
                rebuilt_properties.append(pack_box(property_kind, b"".join(rebuilt_items)))
            rebuilt_meta_children.append(pack_box(kind, b"".join(rebuilt_properties)))
        else:
            rebuilt_meta_children.append(complete)

    meta_payload = meta[8:12] + b"".join(rebuilt_meta_children)
    return ftyp + pack_box(b"meta", meta_payload) + mdat


def main() -> None:
    source = SOURCE.read_bytes()
    if hashlib.sha256(source).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("baseline AVIF source differs from its pinned hash")

    for filename, field_offset, expected_sha256 in CASES:
        fixture = zero_field_clap(source, field_offset)
        if hashlib.sha256(fixture).hexdigest() != expected_sha256:
            raise RuntimeError(f"{filename} differs from its pinned hash")
        verify_pillow(fixture)
        write_or_verify(OUTPUT_DIRECTORY / filename, fixture)

    for filename, trailing, expected_sha256 in TRAILING_PAYLOAD_CASES:
        fixture = append_clap_trailing_payload(append_zero_width_clap(source), trailing)
        if hashlib.sha256(fixture).hexdigest() != expected_sha256:
            raise RuntimeError(f"{filename} differs from its pinned hash")
        verify_pillow(fixture)
        write_or_verify(OUTPUT_DIRECTORY / filename, fixture)


if __name__ == "__main__":
    main()
