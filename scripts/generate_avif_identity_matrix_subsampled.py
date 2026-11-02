#!/usr/bin/env python3
"""Generate an AVIF whose identity matrix is paired with subsampled chroma."""

from __future__ import annotations

import argparse
import hashlib
import struct
from pathlib import Path

from inspect_av1_obus import Bits, parse_sequence_header, read_uleb128
from inspect_avif_bitstreams import inspect as inspect_avif


ROOT = Path(__file__).resolve().parent.parent
SOURCE_PATH = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_lossy_inter_420_b64x16_12bit_mode_select_64x64.avif"
)
SOURCE_SHA256 = "8583340a327c7feffa803fa3b0a80267a49b6fa21c062f9b19799e2a82bf1dcc"
OUTPUT_PATH = (
    ROOT
    / "tests/fixtures/input/images/avif/error_animated_identity_matrix_subsampled.avif"
)
EXPECTED_SHA256 = "c20cb97af0fe69ddd1cfc9e38f223b1df5084655af6dacfd2b3e0c502e8d6162"
EXPECTED_SAMPLE_COUNT = 2
SOURCE_CICP = (1, 13, 6)
INVALID_CICP = (9, 13, 0)
EXPECTED_NCLX_COUNT = 2
EXPECTED_ERROR = "identity color matrix with chroma subsampling"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sequence_headers(
    data: bytes,
    path: Path,
) -> list[tuple[int, int, bytes]]:
    """Return sequence-header payloads and absolute offsets from picture samples."""
    tracks = [track for track in inspect_avif(path)["tracks"] if track["handler"] == "pict"]
    if len(tracks) != 1:
        raise ValueError("pinned AVIF must contain one picture track")
    samples = tracks[0]["samples"]
    if len(samples) != EXPECTED_SAMPLE_COUNT:
        raise ValueError("pinned AVIF must contain two picture samples")

    headers = []
    for sample_index, sample in enumerate(samples):
        cursor = sample["offset"]
        sample_end = cursor + sample["length"]
        while cursor < sample_end:
            header = data[cursor]
            if header & 0x85 or not header & 0x02:
                raise ValueError("pinned AVIF has an unexpected AV1 OBU header")
            payload_size, payload_start = read_uleb128(data, cursor + 1)
            payload_end = payload_start + payload_size
            if payload_end > sample_end:
                raise ValueError("AV1 OBU exceeds its pinned sample")
            if (header >> 3) & 0x0F == 1:
                headers.append((sample_index, payload_start, data[payload_start:payload_end]))
            cursor = payload_end
        if cursor != sample_end:
            raise ValueError("AV1 OBUs do not fill their pinned sample")
    return headers


def color_field_positions(payload: bytes) -> tuple[int, int, dict[str, int | bool]]:
    """Locate color primaries and matrix fields in the pinned AV1 header."""
    bits = Bits(payload)
    profile = bits.read(3)
    still_picture = bool(bits.bit())
    reduced = bool(bits.bit())
    if profile != 2 or still_picture or reduced:
        raise ValueError("pinned source must use an ordinary profile-2 sequence header")

    decoder_model_present = False
    decoder_delay_length = 0
    if bits.bit():
        bits.read(32)
        bits.read(32)
        if bits.bit():
            bits.uvlc()
        decoder_model_present = bool(bits.bit())
        if decoder_model_present:
            decoder_delay_length = bits.read(5) + 1
            bits.read(32)
            bits.read(5)
            bits.read(5)

    display_model_present = bool(bits.bit())
    operating_point_count = bits.read(5) + 1
    for _ in range(operating_point_count):
        bits.read(12)
        level = bits.read(5)
        if level > 7:
            bits.bit()
        if decoder_model_present and bits.bit():
            bits.read(decoder_delay_length)
            bits.read(decoder_delay_length)
            bits.bit()
        if display_model_present and bits.bit():
            bits.read(4)

    width_bits = bits.read(4) + 1
    height_bits = bits.read(4) + 1
    bits.read(width_bits)
    bits.read(height_bits)
    if bits.bit():
        bits.read(4)
        bits.read(3)

    bits.read(3)  # use_128x128_superblock and intra-filter flags
    bits.read(4)  # compound, warped-motion, and dual-filter flags
    order_hint = bool(bits.bit())
    if order_hint:
        bits.read(2)
    screen_content_tools = 2 if bits.bit() else bits.bit()
    if screen_content_tools and not bits.bit():
        bits.bit()
    if order_hint:
        bits.read(3)
    bits.read(3)  # super-resolution, CDEF, and restoration flags

    high_bitdepth = bool(bits.bit())
    twelve_bit = bool(bits.bit()) if profile == 2 and high_bitdepth else False
    monochrome = False if profile == 1 else bool(bits.bit())
    if not high_bitdepth or not twelve_bit or monochrome:
        raise ValueError("pinned source must be a color 12-bit sequence")
    if not bits.bit():
        raise ValueError("pinned source must include AV1 color description fields")

    primaries_position = bits.position
    color_primaries = bits.read(8)
    transfer_characteristics = bits.read(8)
    matrix_position = bits.position
    matrix_coefficients = bits.read(8)
    subsampling_x = bool(bits.bit())
    subsampling_y = bool(bits.bit()) if subsampling_x else False
    return primaries_position, matrix_position, {
        "color_primaries": color_primaries,
        "transfer_characteristics": transfer_characteristics,
        "matrix_coefficients": matrix_coefficients,
        "subsampling_x": subsampling_x,
        "subsampling_y": subsampling_y,
    }


def nclx_offsets(data: bytes) -> list[int]:
    """Return the offsets of every nclx color property in the pinned AVIF."""
    offsets = []
    cursor = 0
    while True:
        offset = data.find(b"nclx", cursor)
        if offset < 0:
            break
        offsets.append(offset)
        cursor = offset + len(b"nclx")
    if len(offsets) != EXPECTED_NCLX_COUNT:
        raise ValueError("pinned AVIF must contain its item and track nclx properties")
    return offsets


def write_bits(data: bytearray, start: int, width: int, value: int) -> None:
    """Replace one bounded big-endian AV1 syntax field."""
    if start < 0 or width <= 0 or start + width > len(data) * 8:
        raise ValueError("AV1 sequence-header field is outside the pinned sample")
    if value < 0 or value >= 1 << width:
        raise ValueError("replacement AV1 syntax value does not fit its field")
    for index in range(width):
        position = start + index
        mask = 1 << (7 - position % 8)
        bit = (value >> (width - index - 1)) & 1
        data[position // 8] = (data[position // 8] & ~mask) | (bit * mask)


def fixture_bytes(source: bytes) -> bytes:
    """Create the two-field CICP mutation in the first AV1 sequence header."""
    if sha256(source) != SOURCE_SHA256:
        raise ValueError("pinned 12-bit 4:2:0 AVIF differs from its source hash")
    headers = sequence_headers(source, SOURCE_PATH)
    if [sample for sample, _, _ in headers] != [0]:
        raise ValueError("pinned AVIF must signal one sequence header in sample zero")
    _, payload_start, payload = headers[0]
    before = parse_sequence_header(payload)
    primary_position, matrix_position, fields = color_field_positions(payload)
    if (
        before["bit_depth"] != 12
        or before["subsampling_x"] != 1
        or before["subsampling_y"] != 1
        or (fields["color_primaries"], fields["transfer_characteristics"], fields["matrix_coefficients"])
        != SOURCE_CICP
    ):
        raise ValueError("pinned sequence header differs from its 12-bit 4:2:0 CICP precondition")

    output = bytearray(source)
    write_bits(output, payload_start * 8 + primary_position, 8, INVALID_CICP[0])
    write_bits(output, payload_start * 8 + matrix_position, 8, INVALID_CICP[2])

    for offset in nclx_offsets(source):
        property_start = offset + len(b"nclx")
        if struct.unpack_from(">HHH", source, property_start) != SOURCE_CICP:
            raise ValueError("pinned nclx property differs from its CICP precondition")
        struct.pack_into(">HHH", output, property_start, *INVALID_CICP)

    result = bytes(output)
    if sha256(result) != EXPECTED_SHA256:
        raise ValueError("generated AVIF differs from its pinned bytes")
    return result


def verify_output(output: bytes, output_path: Path) -> None:
    headers = sequence_headers(output, output_path)
    if [sample for sample, _, _ in headers] != [0]:
        raise ValueError("generated AVIF must retain one sequence header in sample zero")
    _, _, fields = color_field_positions(headers[0][2])
    if (
        (fields["color_primaries"], fields["transfer_characteristics"], fields["matrix_coefficients"])
        != INVALID_CICP
        or not fields["subsampling_x"]
        or not fields["subsampling_y"]
    ):
        raise ValueError("generated sequence header does not pair identity matrix with 4:2:0")
    try:
        parse_sequence_header(headers[0][2])
    except ValueError as error:
        if str(error) != EXPECTED_ERROR:
            raise
    else:
        raise ValueError("generated sequence header did not trigger the pinned error")
    for offset in nclx_offsets(output):
        if struct.unpack_from(">HHH", output, offset + len(b"nclx")) != INVALID_CICP:
            raise ValueError("generated nclx property does not match the AV1 sequence CICP")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    output = fixture_bytes(SOURCE_PATH.read_bytes())
    if args.check:
        if not OUTPUT_PATH.exists() or OUTPUT_PATH.read_bytes() != output:
            raise SystemExit(f"{OUTPUT_PATH} differs from the deterministic mutation")
        action = "verified"
    else:
        if OUTPUT_PATH.exists() and OUTPUT_PATH.read_bytes() != output:
            raise SystemExit(f"refusing to replace a different fixture at {OUTPUT_PATH}")
        OUTPUT_PATH.write_bytes(output)
        action = "wrote"
    verify_output(output, OUTPUT_PATH)
    print(f"{action} {OUTPUT_PATH.relative_to(ROOT)} sha256={sha256(output)}")


if __name__ == "__main__":
    main()
