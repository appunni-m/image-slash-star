#!/usr/bin/env python3
"""Generate size-preserving AVIF frame-header state parity fixtures."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from inspect_av1_obus import Bits, FrameState, parse_obus, read_uleb128
from inspect_avif_bitstreams import inspect as inspect_avif


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_lossless_inter_420_derived_skip_refs_b32x32_64x64.avif"
)
SOURCE_SHA256 = "50d8bdd33c5a5e958d5991d25767748088ba3354d35d4b809d52e84a45713c15"
EXPECTED_OBU_TYPES = (2, 1, 6)
EXPECTED_FRAME_HEADER_BITS = 33
EXPECTED_FRAME_PAYLOAD_LENGTH = 1505
LAYER_SOURCE = ROOT / "tests/fixtures/input/images/avif/animated_opidc_0x101.avif"
LAYER_SOURCE_SHA256 = "25b79a856ea2767e02e5a509f72e7af3f9563202301779be65b0745724ab23be"
LAYER_FIXTURES = {
    "temporal-id-mismatch": (
        "error_animated_tile_group_temporal_id_mismatch.avif",
        "0c23f3e5db7b1d9d7150144f5204044c2bee0804edd6eb022148fc7a4ef4a1d9",
        0x103,
        0x20,
        (1, 0),
    ),
    "spatial-id-mismatch": (
        "error_animated_tile_group_spatial_id_mismatch.avif",
        "2c23b60cc7f4e5951b24ba1309aa7ec1afda18fd97b74d66b54e982899a6963d",
        0x301,
        0x08,
        (0, 1),
    ),
}
FRAME_FIXTURES = {
    "matching-pending": (
        "error_animated_redundant_frame_header_with_pending.avif",
        "46c2a6f033820f828209f9a81ef8da4318c5f3255cee99d866321ce4635a86d5",
        (2, 1, 3, 7, 15),
        None,
    ),
    "without-pending": (
        "error_animated_redundant_frame_header_without_pending.avif",
        "9c29fa97ff57041eb15a7b49a067b2fb2f6f141f003a835d7a786089d1c2dee8",
        (2, 1, 7, 15),
        "redundant frame header has no original",
    ),
    "second-header-before-tiles": (
        "error_animated_frame_header_before_tiles.avif",
        "380c025386ee770a6573d88fa1bdf12adbe345149026c46a3aee78c922f06c78",
        (2, 1, 3, 3, 15),
        "new frame header before previous frame completion",
    ),
    "mismatched-redundant-header": (
        "error_animated_redundant_frame_header_mismatch.avif",
        "4e004fa44966ae7fc63f2ce5214283e4138e8cc6658e12fcf7b38fbd0b9897b0",
        (2, 1, 3, 7, 15),
        "redundant frame header differs from original",
    ),
}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encode_uleb128(value: int) -> bytes:
    if value < 0:
        raise ValueError("OBU size cannot be negative")

    encoded = bytearray()
    while value >= 0x80:
        encoded.append((value & 0x7F) | 0x80)
        value >>= 7
    encoded.append(value)
    return bytes(encoded)


def encode_obu(obu_type: int, flags: int, payload: bytes) -> bytes:
    header = (flags & 0x87) | (obu_type << 3)
    return bytes((header,)) + encode_uleb128(len(payload)) + payload


def encode_extended_obu(
    obu_type: int, flags: int, extension: int, payload: bytes
) -> bytes:
    encoded = encode_obu(obu_type, flags, payload)
    return encoded[:1] + bytes((extension,)) + encoded[1:]


def write_unsigned_bits(data: bytearray, position: int, width: int, value: int) -> None:
    if value < 0 or value >= 1 << width:
        raise ValueError("replacement bit-field value does not fit")
    for index in range(width):
        bit_position = position + index
        mask = 1 << (7 - bit_position % 8)
        if value & (1 << (width - index - 1)):
            data[bit_position // 8] |= mask
        else:
            data[bit_position // 8] &= ~mask


def encode_padding(total_length: int) -> bytes:
    for size_length in range(1, 9):
        payload_length = total_length - 1 - size_length
        if payload_length >= 0 and len(encode_uleb128(payload_length)) == size_length:
            return bytes((0x7A,)) + encode_uleb128(payload_length) + bytes(payload_length)
    raise ValueError("cannot encode OBU_PADDING to the required length")


def sample_layout(source: bytes) -> tuple[int, int, int, bytes, bytes, int]:
    if digest(source) != SOURCE_SHA256:
        raise ValueError("source AVIF differs from its pinned fixture")

    container = inspect_avif(SOURCE)
    picture_tracks = [
        track for track in container.get("tracks", []) if track["handler"] == "pict"
    ]
    if len(picture_tracks) != 1:
        raise ValueError("pinned AVIF must contain exactly one picture track")

    samples = picture_tracks[0]["samples"]
    if len(samples) != 5:
        raise ValueError("pinned AVIF must contain five picture samples")
    sample = samples[0]
    sample_start = sample["offset"]
    sample_end = sample_start + sample["length"]
    if not (0 <= sample_start < sample_end <= len(source)):
        raise ValueError("first picture sample is outside the pinned AVIF")

    sample_bytes = source[sample_start:sample_end]
    obus = parse_obus(sample_bytes, [(sample_start, sample_end)], FrameState())
    if tuple(obu["type"] for obu in obus) != EXPECTED_OBU_TYPES:
        raise ValueError("first picture sample OBU sequence changed")

    frame_obu = obus[-1]
    if frame_obu["has_extension"] or not frame_obu["has_size_field"]:
        raise ValueError("pinned frame OBU header flags changed")
    if frame_obu["payload_length"] != EXPECTED_FRAME_PAYLOAD_LENGTH:
        raise ValueError("pinned frame OBU payload length changed")

    frame_header_bits = frame_obu["frame_header"]["header_bits"]
    if frame_header_bits != EXPECTED_FRAME_HEADER_BITS:
        raise ValueError("pinned frame-header syntax length changed")

    header_spans = frame_obu["header_spans"]
    payload_spans = frame_obu["payload_spans"]
    if len(header_spans) != 1 or len(payload_spans) != 1:
        raise ValueError("pinned frame OBU must occupy one contiguous sample span")
    obu_start = header_spans[0]["offset"]
    payload_start = payload_spans[0]["offset"]
    payload_end = payload_start + frame_obu["payload_length"]
    if payload_start < obu_start or payload_end > sample_end:
        raise ValueError("pinned frame OBU spans outside the first picture sample")

    return (
        sample_start,
        sample_end,
        obu_start,
        source[obu_start:payload_end],
        source[payload_start:payload_end],
        frame_header_bits,
    )


def standalone_header_payload(original_payload: bytes, header_bits: int) -> bytes:
    payload_length = (header_bits + 1 + 7) // 8
    if payload_length > len(original_payload):
        raise ValueError("frame-header syntax exceeds the pinned OBU payload")

    payload = bytearray(original_payload[:payload_length])
    for position in range(header_bits + 1, payload_length * 8):
        mask = 1 << (7 - position % 8)
        if payload[position // 8] & mask:
            raise ValueError("pinned frame-header alignment bits are not zero")
    payload[header_bits // 8] |= 1 << (7 - header_bits % 8)
    return bytes(payload)


def replacement_obus(
    case: str, original_header: int, header_payload: bytes, original_length: int
) -> bytes:
    frame_header = encode_obu(3, original_header, header_payload)
    if case == "matching-pending":
        redundant = encode_obu(7, original_header, header_payload)
        parts = (frame_header, redundant)
    elif case == "without-pending":
        parts = (encode_obu(7, original_header, header_payload),)
    elif case == "second-header-before-tiles":
        parts = (frame_header, frame_header)
    elif case == "mismatched-redundant-header":
        changed_payload = bytearray(header_payload)
        order_hint_bit = 8
        byte_index, bit_index = divmod(order_hint_bit, 8)
        mask = 1 << (7 - bit_index)
        if changed_payload[byte_index] & mask:
            raise ValueError("pinned frame header has a nonzero first order-hint bit")
        changed_payload[byte_index] |= mask
        redundant = encode_obu(7, original_header, bytes(changed_payload))
        parts = (frame_header, redundant)
    else:
        raise ValueError(f"unknown frame-header fixture case: {case}")

    padding = encode_padding(original_length - sum(map(len, parts)))
    replacement = b"".join(parts) + padding
    if len(replacement) != original_length:
        raise ValueError("replacement OBUs do not preserve the original frame OBU size")
    return replacement


def obu_types(sample: bytes) -> tuple[int, ...]:
    types = []
    offset = 0
    while offset < len(sample):
        header = sample[offset]
        offset += 1
        if header & 0x81:
            raise ValueError("mutated sample has invalid OBU header bits")
        if header & 0x04:
            if offset >= len(sample):
                raise ValueError("mutated OBU extension is truncated")
            offset += 1
        if not header & 0x02:
            raise ValueError("mutated AVIF OBU has no size field")
        payload_length, offset = read_uleb128(sample, offset)
        payload_end = offset + payload_length
        if payload_end > len(sample):
            raise ValueError("mutated OBU payload exceeds the first sample")
        types.append((header >> 3) & 0x0F)
        offset = payload_end
    return tuple(types)


def fixture_bytes(source: bytes, case: str) -> bytes:
    (
        sample_start,
        sample_end,
        obu_start,
        original_obu,
        original_payload,
        header_bits,
    ) = sample_layout(source)
    header_payload = standalone_header_payload(original_payload, header_bits)
    replacement = replacement_obus(
        case, original_obu[0], header_payload, len(original_obu)
    )

    candidate = bytearray(source)
    candidate[obu_start : obu_start + len(original_obu)] = replacement
    if len(candidate) != len(source):
        raise ValueError("fixture mutation changed the AVIF file length")
    if (
        candidate[:obu_start] != source[:obu_start]
        or candidate[obu_start + len(original_obu) :]
        != source[obu_start + len(original_obu) :]
    ):
        raise ValueError("fixture mutation changed bytes outside the original frame OBU")

    sample = bytes(candidate[sample_start:sample_end])
    filename, expected_sha256, expected_types, expected_error = FRAME_FIXTURES[case]
    del filename
    if obu_types(sample) != expected_types:
        raise ValueError("mutated sample has an unexpected OBU type sequence")

    state = FrameState()
    try:
        obus = parse_obus(sample, [(sample_start, sample_end)], state)
    except ValueError as error:
        if str(error) != expected_error:
            raise
    else:
        if expected_error is not None:
            raise ValueError(f"AV1 inspector accepted {case}")
        if obus[2]["frame_header"] != obus[3]["frame_header"]:
            raise ValueError("redundant frame header does not match the pending frame")
        if state.pending is None:
            raise ValueError("matching redundant header did not retain the pending frame")

    result = bytes(candidate)
    if digest(result) != expected_sha256:
        raise ValueError(f"generated AVIF differs from the pinned hash for {case}")
    return result


def operating_point_idc_offset(payload: bytes) -> int:
    bits = Bits(payload)
    bits.read(3)  # seq_profile
    bits.read(1)  # still_picture
    reduced_still_picture_header = bits.read(1)
    if reduced_still_picture_header:
        raise ValueError("layer source unexpectedly uses a reduced sequence header")
    timing_info_present = bits.read(1)
    if timing_info_present:
        raise ValueError("layer source unexpectedly declares timing information")
    bits.read(1)  # initial_display_delay_present_flag
    operating_point_count = bits.read(5) + 1
    if operating_point_count != 1:
        raise ValueError("layer source must have exactly one operating point")
    return bits.position


def layer_mismatch_fixture(source: bytes, case: str) -> bytes:
    filename, expected_sha256, operating_point_idc, tile_extension, tile_layer = (
        LAYER_FIXTURES[case]
    )
    if digest(source) != LAYER_SOURCE_SHA256:
        raise ValueError("layer source AVIF differs from its pinned fixture")

    container = inspect_avif(LAYER_SOURCE)
    picture_tracks = [
        track for track in container.get("tracks", []) if track["handler"] == "pict"
    ]
    if len(picture_tracks) != 1 or len(picture_tracks[0]["samples"]) != 5:
        raise ValueError("pinned layer AVIF must contain five picture samples")

    first_sample = picture_tracks[0]["samples"][0]
    sample_start = first_sample["offset"]
    sample_end = sample_start + first_sample["length"]
    sample_bytes = source[sample_start:sample_end]
    original_obus = parse_obus(
        sample_bytes, [(sample_start, sample_end)], FrameState()
    )
    if tuple(obu["type"] for obu in original_obus) != (2, 1, 6):
        raise ValueError("layer source first sample OBU sequence changed")

    sequence_obu, frame_obu = original_obus[1:]
    sequence_header = sequence_obu["sequence_header"]
    if sequence_header["operating_points"][0]["idc"] != 0x101:
        raise ValueError("layer source operating-point IDC changed")
    if frame_obu["type"] != 6 or not frame_obu["has_extension"]:
        raise ValueError("layer source first frame OBU extension changed")
    if (frame_obu["temporal_id"], frame_obu["spatial_id"]) != (0, 0):
        raise ValueError("layer source frame OBU layer identity changed")

    sequence_payload_span = sequence_obu["payload_spans"][0]
    sequence_payload_start = sequence_payload_span["offset"]
    sequence_payload = bytearray(
        source[
            sequence_payload_start : sequence_payload_start
            + sequence_obu["payload_length"]
        ]
    )
    idc_offset = operating_point_idc_offset(sequence_payload)
    if idc_offset != 12:
        raise ValueError("layer source operating-point IDC syntax offset changed")
    idc_bits = Bits(bytes(sequence_payload))
    idc_bits.position = idc_offset
    if idc_bits.read(12) != 0x101:
        raise ValueError("layer source operating-point IDC bytes changed")
    write_unsigned_bits(sequence_payload, idc_offset, 12, operating_point_idc)

    frame_header_bits = frame_obu["frame_header"]["header_bits"]
    frame_payload_span = frame_obu["payload_spans"][0]
    original_payload = source[
        frame_payload_span["offset"] : frame_payload_span["offset"]
        + frame_obu["payload_length"]
    ]
    header_payload = standalone_header_payload(original_payload, frame_header_bits)
    frame_header_span = frame_obu["header_spans"][0]
    frame_obu_start = frame_header_span["offset"]
    original_frame_obu = source[
        frame_obu_start : frame_payload_span["offset"] + frame_obu["payload_length"]
    ]
    if frame_header_span["length"] != 3:
        raise ValueError("layer source frame OBU header size changed")
    original_extension = source[frame_obu_start + 1]
    frame_flags = original_frame_obu[0]
    frame_header = encode_extended_obu(3, frame_flags, original_extension, header_payload)

    # The expanded operating point admits both the pending frame's layer 0 and
    # the tile group's different temporal or spatial layer.
    tile_group = encode_extended_obu(
        4, (frame_flags & 0x83) | 0x04, tile_extension, b""
    )
    replacement_length = len(original_frame_obu)
    padding = encode_padding(replacement_length - len(frame_header) - len(tile_group))
    replacement = frame_header + tile_group + padding
    if len(replacement) != replacement_length:
        raise ValueError("layer mismatch OBUs do not preserve the original sample size")

    candidate = bytearray(source)
    candidate[
        sequence_payload_start : sequence_payload_start + len(sequence_payload)
    ] = sequence_payload
    candidate[
        frame_obu_start : frame_obu_start + replacement_length
    ] = replacement
    if len(candidate) != len(source):
        raise ValueError("layer mismatch mutation changed the AVIF file length")

    candidate_sample = bytes(candidate[sample_start:sample_end])
    if tuple(obu_types(candidate_sample)) != (2, 1, 3, 4, 15):
        raise ValueError("layer mismatch sample has an unexpected OBU sequence")
    parsed = parse_obus(
        candidate_sample, [(sample_start, sample_end)], FrameState()
    )
    if (
        parsed[1]["sequence_header"]["operating_points"][0]["idc"]
        != operating_point_idc
    ):
        raise ValueError("generated sequence header does not admit both layer identities")
    if (parsed[2]["temporal_id"], parsed[2]["spatial_id"]) != (0, 0):
        raise ValueError("generated frame header no longer uses layer zero")
    if (parsed[3]["temporal_id"], parsed[3]["spatial_id"]) != tile_layer:
        raise ValueError("generated tile-group layer does not match the requested mutation")

    result = bytes(candidate)
    if digest(result) != expected_sha256:
        raise ValueError(
            f"generated AVIF differs from the pinned hash for {case}: {digest(result)}"
        )
    return result


def generate_all(source: bytes) -> dict[str, bytes]:
    return {
        case: fixture_bytes(source, case)
        for case in FRAME_FIXTURES
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify all retained fixtures")
    args = parser.parse_args()

    fixtures = generate_all(SOURCE.read_bytes())
    outputs = {
        case: ROOT / "tests/fixtures/input/images/avif" / FRAME_FIXTURES[case][0]
        for case in FRAME_FIXTURES
    }
    layer_fixtures = {
        case: layer_mismatch_fixture(LAYER_SOURCE.read_bytes(), case)
        for case in LAYER_FIXTURES
    }
    layer_outputs = {
        case: ROOT / "tests/fixtures/input/images/avif" / LAYER_FIXTURES[case][0]
        for case in LAYER_FIXTURES
    }

    if args.check:
        for case, expected in fixtures.items():
            output = outputs[case]
            if not output.is_file() or output.read_bytes() != expected:
                raise SystemExit(f"{output} is missing or differs from its generated fixture")
            print(
                f"Verified {output.name} ({len(expected)} bytes, "
                f"sha256={FRAME_FIXTURES[case][1]})"
            )
        for case, expected in layer_fixtures.items():
            output = layer_outputs[case]
            if not output.is_file() or output.read_bytes() != expected:
                raise SystemExit(
                    f"{output} is missing or differs from its generated fixture"
                )
            print(
                f"Verified {output.name} ({len(expected)} bytes, "
                f"sha256={LAYER_FIXTURES[case][1]})"
            )
        return

    for case, expected in fixtures.items():
        output = outputs[case]
        if output.exists() and output.read_bytes() != expected:
            raise SystemExit(f"refusing to overwrite different fixture: {output}")
    for case, expected in layer_fixtures.items():
        output = layer_outputs[case]
        if output.exists() and output.read_bytes() != expected:
            raise SystemExit(f"refusing to overwrite different fixture: {output}")
    for case, expected in fixtures.items():
        output = outputs[case]
        if not output.exists():
            output.write_bytes(expected)
        print(
            f"{output.name}: {len(expected)} bytes, "
            f"sha256={FRAME_FIXTURES[case][1]}"
        )
    for case, expected in layer_fixtures.items():
        output = layer_outputs[case]
        if not output.exists():
            output.write_bytes(expected)
        print(
            f"{output.name}: {len(expected)} bytes, "
            f"sha256={LAYER_FIXTURES[case][1]}"
        )


if __name__ == "__main__":
    main()
