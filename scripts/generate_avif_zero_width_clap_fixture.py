#!/usr/bin/env python3
"""Create and verify an AVIF with a zero-width clean-aperture property.

The full-file mutation adds an essential ``clap`` item property with a zero
width numerator to the pinned AVIF still. Pillow 12.2.0 ignores the invalid
aperture and decodes the primary item pixels unchanged.
"""

from __future__ import annotations

import hashlib
import struct
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as PILLOW_VERSION, _avif, features


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests" / "fixtures" / "input" / "images" / "avif" / "baseline.avif"
OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "clap_zero_width_ignored.avif"
)
EXPECTED_SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
EXPECTED_FIXTURE_SHA256 = "ce95ee92b5498964ff6e3e0bd383608c559fafcafe7bd110e5f2101d10734572"
EXPECTED_PIXELS_SHA256 = "f1a2555b1c61036af2bd1d3d125a6a1343993873d9e5d58e3caee79989095dcf"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_boxes(data: bytes, start: int, end: int) -> list[tuple[bytes, bytes, bytes]]:
    """Return box kind, complete bytes, and payload for one bounded box range."""
    boxes = []
    position = start
    while position < end:
        if end - position < 8:
            raise RuntimeError("AVIF box header is truncated")
        size = struct.unpack_from(">I", data, position)[0]
        kind = data[position + 4 : position + 8]
        header_size = 8
        if size == 1:
            if end - position < 16:
                raise RuntimeError("AVIF large-box header is truncated")
            size = struct.unpack_from(">Q", data, position + 8)[0]
            header_size = 16
        elif size == 0:
            size = end - position
        if size < header_size or size > end - position:
            raise RuntimeError("AVIF box size is outside its parent")
        box_end = position + size
        boxes.append((kind, data[position:box_end], data[position + header_size : box_end]))
        position = box_end
    if position != end:
        raise RuntimeError("AVIF boxes do not fill their parent")
    return boxes


def pack_box(kind: bytes, payload: bytes) -> bytes:
    if len(kind) != 4:
        raise RuntimeError("AVIF box kind must contain exactly four bytes")
    size = len(payload) + 8
    if size > 0xFFFF_FFFF:
        raise RuntimeError("AVIF fixture box exceeds its 32-bit size field")
    return struct.pack(">I4s", size, kind) + payload


def append_zero_width_clap(source: bytes) -> bytes:
    """Add the primary item's essential zero-width ``clap`` association."""
    top_level = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in top_level] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("baseline AVIF top-level boxes differ from the pinned layout")
    ftyp, meta, mdat = (complete for _, complete, _ in top_level)

    # The baseline's single primary-item extent points into the mdat payload.
    # Adding a 40-byte clap box and one ipma association moves that payload by
    # 41 bytes, so preserve the complete coded AV1 item by advancing its offset.
    clap_payload = struct.pack(">8I", 0, 1, 128, 1, 0, 1, 0, 1)
    clap = pack_box(b"clap", clap_payload)
    delta = len(clap) + 1

    rebuilt_meta_children = []
    for kind, complete, payload in read_boxes(meta, 12, len(meta)):
        if kind == b"iloc":
            location = bytearray(payload)
            if (
                location[:4] != bytes(4)
                or location[4:6] != b"\x44\x00"
                or location[6:8] != b"\x00\x01"
                or location[8:10] != b"\x00\x01"
                or location[10:14] != b"\x00\x00\x00\x01"
                or int.from_bytes(location[14:18], "big") != 0x11A
                or int.from_bytes(location[18:22], "big") != len(mdat) - 8
                or len(location) != 22
            ):
                raise RuntimeError("baseline AVIF iloc differs from the pinned layout")
            location[14:18] = (0x11A + delta).to_bytes(4, "big")
            rebuilt_meta_children.append(pack_box(kind, bytes(location)))
        elif kind == b"iprp":
            rebuilt_properties = []
            properties = read_boxes(payload, 0, len(payload))
            if [property_kind for property_kind, _, _ in properties] != [b"ipco", b"ipma"]:
                raise RuntimeError("baseline AVIF iprp differs from the pinned layout")
            for property_kind, property_box, property_payload in properties:
                if property_kind == b"ipco":
                    existing = read_boxes(property_payload, 0, len(property_payload))
                    if [item[0] for item in existing] != [b"ispe", b"pixi", b"av1C", b"colr"]:
                        raise RuntimeError("baseline AVIF primary properties differ")
                    rebuilt_properties.append(pack_box(property_kind, property_payload + clap))
                else:
                    associations = bytearray(property_payload)
                    if associations[:15] != bytes.fromhex("000000000000000100010401028304"):
                        raise RuntimeError("baseline AVIF ipma differs from the pinned layout")
                    associations[10] = 5
                    associations.append(0x85)  # Property 5, marked essential.
                    rebuilt_properties.append(pack_box(property_kind, bytes(associations)))
            rebuilt_meta_children.append(pack_box(kind, b"".join(rebuilt_properties)))
        else:
            rebuilt_meta_children.append(complete)

    meta_payload = meta[8:12] + b"".join(rebuilt_meta_children)
    rebuilt_meta = pack_box(b"meta", meta_payload)
    return ftyp + rebuilt_meta + mdat


def verify_pillow(data: bytes) -> None:
    if PILLOW_VERSION != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PILLOW_VERSION}")
    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"libavif 1.4.1 is required, found {features.version('avif')}")
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in _avif.codec_versions():
            raise RuntimeError(f"AVIF oracle requires {expected}, found {_avif.codec_versions()}")

    with Image.open(BytesIO(data)) as image:
        image.verify()
    with Image.open(BytesIO(data)) as image:
        image.load()
        if image.format != "AVIF" or image.mode != "RGB" or image.size != (128, 128):
            raise RuntimeError("Pillow decoded unexpected zero-width clap metadata")
        if sha256(image.tobytes()) != EXPECTED_PIXELS_SHA256:
            raise RuntimeError("Pillow pixels differ from the pinned AVIF baseline")


def main() -> None:
    source = SOURCE.read_bytes()
    if sha256(source) != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("baseline AVIF source differs from its pinned hash")

    fixture = append_zero_width_clap(source)
    if sha256(fixture) != EXPECTED_FIXTURE_SHA256:
        raise RuntimeError("zero-width clap fixture differs from its pinned hash")
    verify_pillow(fixture)

    if OUTPUT.exists():
        if OUTPUT.read_bytes() != fixture:
            raise RuntimeError(f"refusing to replace a different fixture at {OUTPUT}")
        print(f"Verified existing fixture: {OUTPUT}")
        return

    OUTPUT.write_bytes(fixture)
    print(f"Wrote verified fixture: {OUTPUT}")


if __name__ == "__main__":
    main()
