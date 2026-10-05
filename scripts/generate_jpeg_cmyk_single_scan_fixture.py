#!/usr/bin/env python3
"""Create and verify a baseline CMYK JPEG with a one-component scan.

The fixture starts from the pinned 13x9 CMYK JPEG, changes its four-component
SOS header to select only the first component, and preserves the original
entropy-coded bytes. Pillow 12.2.0 accepts the resulting complete JPEG and
defines its public decoded pixels.
"""

from __future__ import annotations

import hashlib
import struct
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as PILLOW_VERSION


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests" / "fixtures" / "input" / "images" / "jpeg" / "baseline_cmyk_13x9.jpg"
OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "jpeg"
    / "baseline_cmyk_single_component_scan_c.jpg"
)
EXPECTED_SOURCE_SHA256 = "c6860db09f1537886ae641d40e71b30793241ecb0d35f17f430e1c3cba277682"
EXPECTED_FIXTURE_SHA256 = "81109c230ad68dc6ddfccb25a441f9c0ceb2b5d336a3e948b2496628849c1c7f"
EXPECTED_PIXELS_SHA256 = "1ca1b4e0d3309f8b0e4d84ddc4fd20c8fc98827a62e0b4d3713ef3017f52f7b4"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def find_sos(data: bytes) -> tuple[int, int, int]:
    """Return the first SOS marker, payload start, and segment end."""
    if not data.startswith(b"\xff\xd8"):
        raise RuntimeError("source JPEG does not start with SOI")

    cursor = 2
    while cursor < len(data):
        if data[cursor] != 0xFF:
            raise RuntimeError("expected a JPEG marker before the scan")
        marker_start = cursor
        while cursor < len(data) and data[cursor] == 0xFF:
            cursor += 1
        if cursor >= len(data):
            break

        marker = data[cursor]
        cursor += 1
        if marker == 0xDA:
            if cursor + 2 > len(data):
                raise RuntimeError("SOS segment has no length field")
            segment_length = struct.unpack_from(">H", data, cursor)[0]
            if segment_length < 2:
                raise RuntimeError("SOS segment length is invalid")
            payload_start = cursor + 2
            segment_end = cursor + segment_length
            if segment_end > len(data):
                raise RuntimeError("SOS segment exceeds the source JPEG")
            return marker_start, payload_start, segment_end

        if marker in (0xD8, 0x01) or 0xD0 <= marker <= 0xD7:
            continue
        if cursor + 2 > len(data):
            raise RuntimeError("JPEG marker has no length field")
        segment_length = struct.unpack_from(">H", data, cursor)[0]
        if segment_length < 2:
            raise RuntimeError("JPEG marker length is invalid")
        cursor += segment_length

    raise RuntimeError("source JPEG contains no SOS marker")


def one_component_scan(data: bytes) -> bytes:
    """Retain the first component selector and the baseline scan parameters."""
    marker_start, payload_start, segment_end = find_sos(data)
    component_count = data[payload_start]
    if component_count != 4:
        raise RuntimeError("source SOS must select all four CMYK components")

    payload_length = segment_end - payload_start
    if payload_length != 12:
        raise RuntimeError("source CMYK SOS does not have the expected baseline shape")
    if data[payload_start + 9 : segment_end] != b"\x00\x3f\x00":
        raise RuntimeError("source CMYK SOS has unexpected spectral parameters")

    new_payload = (
        b"\x01"
        + data[payload_start + 1 : payload_start + 3]
        + data[payload_start + 9 : segment_end]
    )
    new_segment = b"\xff\xda" + struct.pack(">H", len(new_payload) + 2) + new_payload
    return data[:marker_start] + new_segment + data[segment_end:]


def verify_pillow(data: bytes) -> None:
    if PILLOW_VERSION != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PILLOW_VERSION}")

    with Image.open(BytesIO(data)) as image:
        image.verify()
    with Image.open(BytesIO(data)) as image:
        image.load()
        if image.format != "JPEG" or image.mode != "CMYK" or image.size != (13, 9):
            raise RuntimeError("Pillow decoded unexpected CMYK single-scan metadata")
        if sha256(image.tobytes()) != EXPECTED_PIXELS_SHA256:
            raise RuntimeError("Pillow decoded pixels differ from the pinned reference")


def main() -> None:
    source = SOURCE.read_bytes()
    if sha256(source) != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("baseline CMYK source differs from its pinned hash")

    fixture = one_component_scan(source)
    if sha256(fixture) != EXPECTED_FIXTURE_SHA256:
        raise RuntimeError("single-component CMYK fixture differs from its pinned hash")
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
