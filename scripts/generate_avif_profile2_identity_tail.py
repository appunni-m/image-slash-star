#!/usr/bin/env python3
"""Derive a 9x9 identity-CICP parity fixture from a pinned AVIF still."""

from __future__ import annotations

import argparse
import hashlib
import struct
from pathlib import Path
import tempfile

from PIL import Image, __version__ as PILLOW_VERSION

from inspect_av1_obus import Bits, inspect
from inspect_avif_bitstreams import children, parse_boxes, unique_box


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SOURCE = ROOT / "tests/fixtures/input/images/avif/profile2_identity_12bit_444.avif"
DEFAULT_OUTPUT = (
    ROOT / "tests/fixtures/input/images/avif/profile2_identity_12bit_444_9x9.avif"
)
SOURCE_SHA256 = "1ce2aae5539074e3a65092f185c4db34537462d637859d4fd85449bb15557931"
FIXTURE_SHA256 = "2329ba1c0b8cb384727b1a3c0b817998fee4da5a3aa0254801afdcefb2f70c0d"
PILLOW_RGB_SHA256 = "081589477332f1b32b6175028a983d88a15fc1b995b32abcfe4280cfcdb8d2d8"
SOURCE_EXTENT = (16, 16)
FIXTURE_EXTENT = (9, 9)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def bit_values(data: bytes) -> list[int]:
    return [
        (byte >> shift) & 1
        for byte in data
        for shift in range(7, -1, -1)
    ]


def pack_bits(values: list[int]) -> bytes:
    if len(values) % 8:
        raise RuntimeError("AV1 sequence header rewrite is not byte-aligned")
    packed = bytearray()
    for offset in range(0, len(values), 8):
        value = 0
        for bit in values[offset : offset + 8]:
            value = (value << 1) | bit
        packed.append(value)
    return bytes(packed)


def set_field(bits: list[int], start: int, width: int, value: int) -> None:
    if not 0 <= value < 1 << width:
        raise RuntimeError("AV1 frame dimension does not fit its sequence field")
    for offset in range(width):
        bits[start + offset] = (value >> (width - offset - 1)) & 1


def cropped_sequence_header(payload: bytes, sequence: dict[str, object]) -> bytes:
    expected = (
        sequence["profile"],
        sequence["still_picture"],
        sequence["reduced_still_picture_header"],
        sequence["max_width"],
        sequence["max_height"],
        sequence["bit_depth"],
        sequence["monochrome"],
        sequence["color_primaries"],
        sequence["transfer_characteristics"],
        sequence["matrix_coefficients"],
        sequence["color_range"],
        sequence["subsampling_x"],
        sequence["subsampling_y"],
    )
    if expected != (2, True, True, 16, 16, 12, False, 1, 13, 0, 1, 0, 0):
        raise RuntimeError("pinned AV1 sequence no longer matches the 16x16 source")

    bits = Bits(payload)
    profile = bits.read(3)
    still_picture = bool(bits.bit())
    reduced = bool(bits.bit())
    bits.read(5)  # Reduced-still-picture operating-point level.
    width_bits = bits.read(4) + 1
    height_bits = bits.read(4) + 1
    width_start = bits.position
    width_minus_one = bits.read(width_bits)
    height_start = bits.position
    height_minus_one = bits.read(height_bits)
    if (profile, still_picture, reduced, width_minus_one, height_minus_one) != (
        2,
        True,
        True,
        SOURCE_EXTENT[0] - 1,
        SOURCE_EXTENT[1] - 1,
    ):
        raise RuntimeError("pinned AV1 dimension fields changed")
    if (width_bits, height_bits) != (
        sequence["width_bits"],
        sequence["height_bits"],
    ):
        raise RuntimeError("AV1 dimension field widths disagree with the inspector")

    values = bit_values(payload)
    set_field(values, width_start, width_bits, FIXTURE_EXTENT[0] - 1)
    set_field(values, height_start, height_bits, FIXTURE_EXTENT[1] - 1)
    return pack_bits(values)


def mutate(source_path: Path) -> bytes:
    original = source_path.read_bytes()
    if sha256(original) != SOURCE_SHA256:
        raise RuntimeError("pinned 16x16 identity AVIF source hash differs")

    report = inspect(source_path)
    color_items = [item for item in report["samples"] if item["role"] == "item_color"]
    alpha_items = [item for item in report["samples"] if item["role"] == "item_alpha"]
    if len(color_items) != 1 or alpha_items:
        raise RuntimeError("source must contain one color item without auxiliary alpha")
    sequence_obus = [obu for obu in color_items[0]["obus"] if obu["type"] == 1]
    if len(sequence_obus) != 1 or len(sequence_obus[0]["payload_spans"]) != 1:
        raise RuntimeError("expected one contiguous AV1 sequence header")

    data = bytearray(original)
    sequence_obu = sequence_obus[0]
    payload_span = sequence_obu["payload_spans"][0]
    start = payload_span["offset"]
    end = start + sequence_obu["payload_length"]
    payload = bytes(data[start:end])
    replacement = cropped_sequence_header(
        payload, sequence_obu["sequence_header"]
    )
    if len(replacement) != len(payload):
        raise RuntimeError("frame dimension mutation changed the sequence OBU size")
    data[start:end] = replacement

    top_level = parse_boxes(data, 0, len(data))
    meta = unique_box(top_level, b"meta")
    assert meta is not None
    iprp = unique_box(children(data, meta, prefix=4), b"iprp")
    assert iprp is not None
    ipco = unique_box(children(data, iprp), b"ipco")
    assert ipco is not None
    spatial_properties = [
        prop for prop in children(data, ipco) if prop.kind == b"ispe"
    ]
    if len(spatial_properties) != 1:
        raise RuntimeError("expected one primary spatial-extent property")
    spatial = spatial_properties[0]
    dimensions_start = spatial.payload_start + 4  # FullBox version and flags.
    dimensions_end = dimensions_start + 8
    if struct.unpack(">II", data[dimensions_start:dimensions_end]) != SOURCE_EXTENT:
        raise RuntimeError("pinned AVIF spatial extent is not 16x16")
    data[dimensions_start:dimensions_end] = struct.pack(">II", *FIXTURE_EXTENT)
    return bytes(data)


def validate_structure(path: Path) -> None:
    report = inspect(path)
    color_items = [item for item in report["samples"] if item["role"] == "item_color"]
    alpha_items = [item for item in report["samples"] if item["role"] == "item_alpha"]
    if len(color_items) != 1 or alpha_items:
        raise RuntimeError("identity fixture must contain RGB color without alpha")
    sequence = next(
        obu["sequence_header"]
        for obu in color_items[0]["obus"]
        if obu["type"] == 1
    )
    actual = (
        sequence["profile"],
        sequence["bit_depth"],
        sequence["color_primaries"],
        sequence["transfer_characteristics"],
        sequence["matrix_coefficients"],
        sequence["color_range"],
        sequence["subsampling_x"],
        sequence["subsampling_y"],
        sequence["max_width"],
        sequence["max_height"],
    )
    if actual != (2, 12, 1, 13, 0, 1, 0, 0, *FIXTURE_EXTENT):
        raise RuntimeError(f"identity fixture sequence differs: {actual!r}")

    data = path.read_bytes()
    top_level = parse_boxes(data, 0, len(data))
    meta = unique_box(top_level, b"meta")
    assert meta is not None
    iprp = unique_box(children(data, meta, prefix=4), b"iprp")
    assert iprp is not None
    ipco = unique_box(children(data, iprp), b"ipco")
    assert ipco is not None
    spatial = unique_box(children(data, ipco), b"ispe")
    assert spatial is not None
    dimensions_start = spatial.payload_start + 4
    dimensions_end = dimensions_start + 8
    if struct.unpack(">II", data[dimensions_start:dimensions_end]) != FIXTURE_EXTENT:
        raise RuntimeError("identity fixture spatial extent is not 9x9")


def validate_pillow(path: Path) -> None:
    if PILLOW_VERSION != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {PILLOW_VERSION}")
    with Image.open(path) as image:
        if image.format != "AVIF" or image.size != FIXTURE_EXTENT or image.mode != "RGB":
            raise RuntimeError("Pillow opened identity fixture with unexpected image info")
        image.verify()

    with Image.open(path) as image:
        image.load()
        if image.mode != "RGB" or image.size != FIXTURE_EXTENT:
            raise RuntimeError("Pillow decoded identity fixture with unexpected shape")
        if sha256(image.tobytes()) != PILLOW_RGB_SHA256:
            raise RuntimeError("Pillow RGB bytes differ from the pinned identity output")


def generate(source: Path, output: Path) -> None:
    encoded = mutate(source.expanduser().resolve())
    digest = sha256(encoded)
    if digest != FIXTURE_SHA256:
        raise RuntimeError(f"generated AVIF hash differs: {digest}")

    output = output.expanduser().resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="avif-profile2-identity-tail-") as temporary:
        candidate = Path(temporary) / output.name
        candidate.write_bytes(encoded)
        validate_structure(candidate)
        validate_pillow(candidate)
        output.write_bytes(encoded)

    print(f"Wrote {output} ({len(encoded)} bytes, sha256 {FIXTURE_SHA256})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    generate(args.source, args.output)


if __name__ == "__main__":
    main()
