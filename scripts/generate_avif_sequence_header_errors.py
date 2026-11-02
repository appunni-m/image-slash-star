#!/usr/bin/env python3
"""Generate deterministic AVIFs for AV1 sequence-header error parity cases."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from inspect_av1_obus import Bits, parse_sequence_header
from inspect_avif_bitstreams import inspect as inspect_avif


ROOT = Path(__file__).resolve().parent.parent
INPUT_PATH = (
    ROOT / "tests/fixtures/input/images/avif/animated_decoder_model_parameters_change.avif"
)
INPUT_SHA256 = "9191549ae1373fc31e093a36d9ff2d88525f4219378c7b7ba1ecb6b74cec8396"
EQUAL_INTERVAL_INPUT_PATH = (
    ROOT / "tests/fixtures/input/images/avif/animated_equal_picture_interval_two_ticks.avif"
)
EQUAL_INTERVAL_INPUT_SHA256 = "e49851e6d94c721525f3b4130d7279c2ed2329a68032ea81f3714ae68119db7d"
EXPECTED_SAMPLE_COUNT = 5
EMPTY_OBU_PADDING_HEADER = 0x7A  # OBU type 15 with a size field and no payload.
CASES = {
    "reduced-header-nonstill": {
        "asset": "error_animated_reduced_header_nonstill.avif",
        "sha256": "61d55ddcaafde6347e60cc7b9ffe4e2c18ac448f85e14c8d3b6a9a2f0056ff6f",
        "error": "reduced sequence header without still_picture",
    },
    "invalid-operating-point-idc": {
        "asset": "error_animated_invalid_operating_point_idc.avif",
        "sha256": "d5553ed290d398d26ef3e731c98e4bee6b2d8d06b8a00dd4c876d851c61196c0",
        "error": "invalid operating_point_idc",
    },
    "initial-display-delay-over-10": {
        "asset": "error_animated_initial_display_delay_over_10.avif",
        "sha256": "5b1c2097577d75e116016f0760f6c589c12e1304fb42e7ef9a2c982f020b555a",
        "error": "initial display delay exceeds 10",
    },
    "sequence-header-short-payload": {
        "asset": "error_animated_sequence_header_short_payload.avif",
        "sha256": "801608c817468a05bbf62f04ebee2cf5fbe324296c48e98560e7f74350357197",
        "error": "truncated sequence header",
    },
    "sequence-header-omitted": {
        "asset": "error_animated_frame_before_sequence_header.avif",
        "sha256": "4990da835025fe108dc99cea39b521e0ad9fb77e3461f71daa0fc70c2e4bb56d",
        "error": "frame appears before a sequence header",
    },
    "uvlc-leading-zero-limit": {
        "asset": "error_animated_equal_picture_interval_uvlc_32_leading_zeroes.avif",
        "sha256": "842854d4e586bf3b84607f45c85479cb37c3357a78296b7bf39da5ad71ad74e8",
        "error": "UVLC value exceeds u32",
    },
}


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_uleb(data: bytes, offset: int) -> tuple[int, int]:
    """Read one bounded AV1 OBU length field."""
    value = 0
    shift = 0
    while True:
        if offset >= len(data) or shift > 49:
            raise ValueError("AV1 OBU size field is truncated or too wide")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            return value, offset
        shift += 7


def sequence_obus(data: bytes, container: dict) -> list[tuple[int, int, bytes]]:
    """Return sequence-header payload offsets and bytes for each picture sample."""
    tracks = [track for track in container["tracks"] if track["handler"] == "pict"]
    if len(tracks) != 1:
        raise ValueError("pinned AVIF must contain one picture track")
    samples = tracks[0]["samples"]
    if len(samples) != EXPECTED_SAMPLE_COUNT:
        raise ValueError("pinned AVIF must contain five picture samples")

    headers = []
    for sample_index, sample in enumerate(samples):
        sample_start = sample["offset"]
        sample_end = sample_start + sample["length"]
        cursor = sample_start
        while cursor < sample_end:
            header = data[cursor]
            if header & 0x85 or not header & 0x02:
                raise ValueError("pinned AVIF has an unexpected AV1 OBU header")
            payload_size, payload_start = read_uleb(data, cursor + 1)
            payload_end = payload_start + payload_size
            if payload_end > sample_end:
                raise ValueError("AV1 OBU exceeds its pinned sample")
            if (header >> 3) & 0x0F == 1:
                headers.append((sample_index, payload_start, data[payload_start:payload_end]))
            cursor = payload_end
        if cursor != sample_end:
            raise ValueError("AV1 OBUs do not fill their pinned sample")
    return headers


def sequence_header_layout(payload: bytes) -> dict[str, int | bool]:
    """Locate relevant AV1 syntax fields in the pinned ordinary sequence header."""
    bits = Bits(payload)
    bits.read(3)  # seq_profile
    still_picture = bool(bits.bit())
    reduced = bool(bits.bit())
    if still_picture or reduced:
        raise ValueError("pinned source must use an ordinary animated sequence header")

    timing_present = bool(bits.bit())
    decoder_model_present = False
    decoder_delay_length = 0
    ticks_position = -1
    ticks_minus_one = -1
    if timing_present:
        bits.read(32)  # num_units_in_display_tick
        bits.read(32)  # time_scale
        if bits.bit():  # equal_picture_interval
            ticks_position = bits.position
            ticks_minus_one = bits.uvlc()
        decoder_model_present = bool(bits.bit())
        if decoder_model_present:
            decoder_delay_length = bits.read(5) + 1
            bits.read(32)  # num_units_in_decoding_tick
            bits.read(5)  # buffer_removal_delay_length_minus_1
            bits.read(5)  # frame_presentation_delay_length_minus_1

    display_model_position = bits.position
    display_model_present = bool(bits.bit())
    operating_point_count = bits.read(5) + 1
    if operating_point_count != 1:
        raise ValueError("pinned sequence must contain exactly one operating point")

    idc_position = bits.position
    idc = bits.read(12)
    level = bits.read(5)
    if level > 7:
        bits.bit()  # seq_tier

    if decoder_model_present and bits.bit():
        bits.read(decoder_delay_length)
        bits.read(decoder_delay_length)
        bits.bit()  # low_delay_mode_flag

    point_display_position = bits.position
    point_display_present = bool(bits.bit()) if display_model_present else False
    delay_position = bits.position if point_display_present else -1
    initial_display_delay = bits.read(4) + 1 if point_display_present else 10
    return {
        "display_model_position": display_model_position,
        "display_model_present": display_model_present,
        "ticks_position": ticks_position,
        "ticks_minus_one": ticks_minus_one,
        "idc_position": idc_position,
        "idc": idc,
        "level": level,
        "point_display_position": point_display_position,
        "point_display_present": point_display_present,
        "delay_position": delay_position,
        "initial_display_delay": initial_display_delay,
    }


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


def changed_bits(before: bytes, after: bytes) -> list[int]:
    positions = []
    for byte_index, (left, right) in enumerate(zip(before, after, strict=True)):
        different = left ^ right
        for bit_index in range(8):
            if different & (1 << (7 - bit_index)):
                positions.append(byte_index * 8 + bit_index)
    return positions


def fixture_bytes(source: bytes, case: str) -> bytes:
    """Apply one pinned sequence-header mutation to the first sample."""
    source_path = (
        EQUAL_INTERVAL_INPUT_PATH if case == "uvlc-leading-zero-limit" else INPUT_PATH
    )
    expected_source_sha256 = (
        EQUAL_INTERVAL_INPUT_SHA256 if case == "uvlc-leading-zero-limit" else INPUT_SHA256
    )
    if sha256(source) != expected_source_sha256:
        raise ValueError("AVIF differs from its pinned sequence-header source")

    headers = sequence_obus(source, inspect_avif(source_path))
    expected_header_samples = [0] if case == "uvlc-leading-zero-limit" else [0, 1]
    if [sample for sample, _, _ in headers] != expected_header_samples:
        raise ValueError("pinned AVIF sequence-header sample layout changed")
    parsed = [parse_sequence_header(payload) for _, _, payload in headers]
    if any(
        header["operating_points"][0]["idc"] != 0
        or header["display_model_present"]
        for header in parsed
    ):
        raise ValueError("pinned sequence headers differ from their syntax preconditions")
    layout = sequence_header_layout(headers[0][2])

    output = bytearray(source)
    payload_start = headers[0][1]
    changed_field_positions: set[int]
    if case == "uvlc-leading-zero-limit":
        if layout["ticks_position"] < 0 or layout["ticks_minus_one"] != 1:
            raise ValueError("pinned sequence header must encode a two-tick equal interval")
        position = payload_start * 8 + int(layout["ticks_position"])
        if position + 32 > (payload_start + len(headers[0][2])) * 8:
            raise ValueError("pinned sequence header has insufficient bits for the UVLC limit")
        write_bits(output, position, 32, 0)
        changed_field_positions = set(range(position, position + 32))
    elif case == "reduced-header-nonstill":
        position = payload_start * 8 + 4
        write_bits(output, position, 1, 1)
        changed_field_positions = {position}
    elif case == "invalid-operating-point-idc":
        if layout["idc"] != 0:
            raise ValueError("pinned operating-point IDC is not zero")
        position = payload_start * 8 + int(layout["idc_position"])
        write_bits(output, position, 12, 1)
        changed_field_positions = {position + 11}
    elif case == "initial-display-delay-over-10":
        if layout["display_model_present"] or layout["point_display_present"]:
            raise ValueError("pinned display-model flags are not clear")
        display_position = payload_start * 8 + int(layout["display_model_position"])
        point_position = payload_start * 8 + int(layout["point_display_position"])
        delay_position = point_position + 1
        write_bits(output, display_position, 1, 1)
        write_bits(output, point_position, 1, 1)
        write_bits(output, delay_position, 4, 10)
        changed_field_positions = {
            display_position,
            point_position,
            *(delay_position + offset for offset in range(4)),
        }
    elif case == "sequence-header-short-payload":
        payload = headers[0][2]
        if (
            len(payload) != 28
            or source[payload_start - 2 : payload_start] != b"\x0a\x1c"
        ):
            raise ValueError("pinned sequence OBU layout changed")
        # Shorten the sequence payload by two bytes. The reclaimed bytes form
        # an empty OBU_PADDING, so the sample length and all BMFF offsets stay
        # fixed while the sequence syntax ends before its final fields.
        output[payload_start - 1] = len(payload) - 2
        output[payload_start + len(payload) - 2] = EMPTY_OBU_PADDING_HEADER
        output[payload_start + len(payload) - 1] = 0
        changed_field_positions = (
            set(range((payload_start - 1) * 8, payload_start * 8))
            | set(
                range(
                    (payload_start + len(payload) - 2) * 8,
                    (payload_start + len(payload)) * 8,
                )
            )
        )
    elif case == "sequence-header-omitted":
        if source[payload_start - 2 : payload_start] != b"\x0a\x1c":
            raise ValueError("pinned first sequence OBU header layout changed")
        # Preserve the item extent while changing the first OBU into padding.
        # Its following frame now arrives before sequence state exists.
        header_position = payload_start - 2
        output[header_position] = EMPTY_OBU_PADDING_HEADER
        changed_field_positions = set(
            range(header_position * 8, (header_position + 1) * 8)
        )
    else:
        raise ValueError(f"unsupported sequence-header error case {case!r}")

    result = bytes(output)
    if not set(changed_bits(source, result)) <= changed_field_positions:
        raise ValueError("fixture mutation changed bits outside its pinned syntax fields")
    if sha256(result) != CASES[case]["sha256"]:
        raise ValueError("generated AVIF differs from its pinned bytes")
    return result


def verify_output(case: str, output_path: Path) -> None:
    data = output_path.read_bytes()
    headers = sequence_obus(data, inspect_avif(output_path))
    if case == "sequence-header-omitted":
        expected_header_samples = [1]
    else:
        expected_header_samples = [0] if case == "uvlc-leading-zero-limit" else [0, 1]
    if [sample for sample, _, _ in headers] != expected_header_samples:
        raise ValueError("generated AVIF sequence-header sample layout changed")
    if case == "sequence-header-omitted":
        for _, _, payload in headers:
            parse_sequence_header(payload)
    elif case == "initial-display-delay-over-10":
        layout = sequence_header_layout(headers[0][2])
        if not layout["display_model_present"] or not layout["point_display_present"]:
            raise ValueError("generated display-model flags are not enabled")
        if layout["initial_display_delay"] != 11:
            raise ValueError("generated initial display delay is not 11")
    else:
        try:
            parse_sequence_header(headers[0][2])
        except ValueError as error:
            if str(error) != CASES[case]["error"]:
                raise
        else:
            raise ValueError("generated first sequence header did not trigger its pinned error")
    if len(headers) > 1:
        parse_sequence_header(headers[1][2])


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", required=True, choices=CASES)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    output_path = ROOT / "tests/fixtures/input/images/avif" / str(CASES[args.case]["asset"])
    source_path = (
        EQUAL_INTERVAL_INPUT_PATH
        if args.case == "uvlc-leading-zero-limit"
        else INPUT_PATH
    )
    output = fixture_bytes(source_path.read_bytes(), args.case)
    if args.check:
        if not output_path.exists() or output_path.read_bytes() != output:
            raise SystemExit(f"{output_path} differs from the deterministic mutation")
        action = "verified"
    else:
        if output_path.exists() and output_path.read_bytes() != output:
            raise SystemExit(f"refusing to replace a different fixture at {output_path}")
        output_path.write_bytes(output)
        action = "wrote"
    verify_output(args.case, output_path)
    print(f"{action} {output_path.relative_to(ROOT)} sha256={CASES[args.case]['sha256']}")


if __name__ == "__main__":
    main()
