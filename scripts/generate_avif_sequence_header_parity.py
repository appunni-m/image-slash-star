#!/usr/bin/env python3
"""Generate AVIF sequence-header parity fixtures from a pinned animation."""

from __future__ import annotations

import argparse
import hashlib
import struct
import sys
import tempfile
from pathlib import Path

from avif_bmff_boxes import rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "animated.avif"
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
SOURCE_SAMPLE_SIZES = (39, 113, 5, 30, 25)
STSZ_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stsz")
ILOC_PATH = (b"meta", b"iloc")
SOURCE_ILOC = bytes.fromhex("0000000044000001000100000001000003ff00000027")

CASES = {
    "timing-without-decoder-model": {
        "asset": "animated_timing_without_decoder_model.avif",
        "sha256": "7978ad21887be7ec5e74b13ab9555e2c2caef9911b900947b67c231b74f1f5cf",
        "initial_display_delay": None,
    },
    "equal-picture-interval-two-ticks": {
        "asset": "animated_equal_picture_interval_two_ticks.avif",
        "sha256": "e49851e6d94c721525f3b4130d7279c2ed2329a68032ea81f3714ae68119db7d",
        "initial_display_delay": None,
        "num_ticks_per_picture": 2,
    },
    "initial-display-delay-10": {
        "asset": "animated_initial_display_delay_10.avif",
        "sha256": "aa95d8f4b6f2576f91a010fd0cc1cd5ed10ea826b89f8aba661c3dd6a9fa1b7d",
        "initial_display_delay": 10,
    },
    "operating-point-high-tier": {
        "asset": "animated_operating_point_high_tier.avif",
        "sha256": "bb06afad6c56b58389eede0bdda980ca42ee36ca045eb92e39de7f5a85e19fb0",
        "initial_display_delay": None,
        "operating_point_level": 8,
    },
    "decoder-model-without-operating-point-parameters": {
        "asset": "animated_decoder_model_without_operating_point_parameters.avif",
        "sha256": "e220130a0614df8c5fce0acf525c2c69a9fccaa8ec140373eac1f91d424d757d",
        "initial_display_delay": None,
        "decoder_model_present": True,
    },
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def bits_from_bytes(data: bytes) -> list[int]:
    return [
        (data[index // 8] >> (7 - index % 8)) & 1
        for index in range(len(data) * 8)
    ]


def pack_bits(bits: list[int]) -> bytes:
    padded = bits + [0] * ((-len(bits)) % 8)
    output = bytearray(len(padded) // 8)
    for index, bit in enumerate(padded):
        output[index // 8] |= bit << (7 - index % 8)
    return bytes(output)


def unsigned_bits(value: int, width: int) -> list[int]:
    return [(value >> shift) & 1 for shift in reversed(range(width))]


def uvlc_bits(value: int) -> list[int]:
    require(value >= 0, "UVLC value must be non-negative")
    code_number = value + 1
    leading_zeroes = code_number.bit_length() - 1
    return [0] * leading_zeroes + unsigned_bits(code_number, leading_zeroes + 1)


def read_uleb(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    while True:
        require(offset < len(data) and shift <= 49, "truncated or oversized OBU length")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            return value, offset
        shift += 7


def write_uleb(value: int) -> bytes:
    output = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        output.append(byte | (0x80 if value else 0))
        if not value:
            return bytes(output)


def parse_obus(sample: bytes) -> list[tuple[int, bytes]]:
    obus = []
    offset = 0
    while offset < len(sample):
        header = sample[offset]
        offset += 1
        require(header & 0x84 == 0, "source OBU has unsupported header flags")
        require(header & 1 == 0, "source OBU has a reserved header bit")
        payload_size, payload_start = read_uleb(sample, offset)
        payload_end = payload_start + payload_size
        require(payload_end <= len(sample), "OBU payload exceeds its sample")
        obus.append((header, sample[payload_start:payload_end]))
        offset = payload_end
    return obus


def pack_obu(header: int, payload: bytes) -> bytes:
    return bytes([header]) + write_uleb(len(payload)) + payload


def sequence_layout(source: bytes) -> tuple[int, int, int, int]:
    """Return the display flags and level field positions for the pinned header."""
    sys.path.insert(0, str(ROOT / "scripts"))
    from inspect_av1_obus import Bits

    bits = Bits(source)
    require(bits.read(3) == 0, "source sequence profile changed")
    require(not bits.bit(), "source sequence unexpectedly sets still_picture")
    require(not bits.bit(), "source sequence unexpectedly sets reduced_still_picture_header")
    require(not bits.bit(), "source sequence unexpectedly contains timing syntax")
    display_model_position = bits.position
    require(not bits.bit(), "source sequence unexpectedly enables its display model")
    require(bits.read(5) == 0, "source sequence no longer has one operating point")
    require(bits.read(12) == 0, "source operating-point IDC changed")
    level_position = bits.position
    level = bits.read(5)
    if level > 7:
        bits.read(1)
    return display_model_position, bits.position, level_position, level


def sequence_payload(
    source: bytes,
    initial_display_delay: int | None,
    operating_point_level: int | None,
    decoder_model_present: bool,
    num_ticks_per_picture: int = 1,
) -> bytes:
    """Add timing syntax or set a valid high-level operating point."""
    raw_bits = bits_from_bytes(source)
    trailing_zeros = (source[-1] & -source[-1]).bit_length() - 1
    syntax_end = len(raw_bits) - trailing_zeros - 1
    syntax = raw_bits[:syntax_end]
    require(syntax[:6] == [0] * 6, "source sequence header prefix changed")

    display_model_position, point_display_position, level_position, source_level = (
        sequence_layout(source)
    )
    require(display_model_position == 6, "source display-model flag moved")
    if operating_point_level is not None:
        require(initial_display_delay is None, "high-tier case cannot add display timing")
        require(not decoder_model_present, "high-level case cannot add decoder-model timing")
        require(num_ticks_per_picture == 1, "high-tier case cannot add timing information")
        require(source_level == 0, "source operating-point level changed")
        require(8 <= operating_point_level <= 31, "high-tier level is out of range")
        updated = (
            syntax[:level_position]
            + unsigned_bits(operating_point_level, 5)
            + [0]  # seq_tier: main tier
            + syntax[level_position + 5 :]
        )
        return pack_bits(updated + [1])

    require(num_ticks_per_picture >= 1, "picture interval must contain at least one tick")
    timing = (
        unsigned_bits(1, 32)  # num_units_in_display_tick
        + unsigned_bits(30, 32)  # time_scale
        + [1]  # equal_picture_interval
        + uvlc_bits(num_ticks_per_picture - 1)
        + [int(decoder_model_present)]
    )
    if decoder_model_present:
        timing += (
            unsigned_bits(0, 5)  # buffer_delay_length_minus_1
            + unsigned_bits(1, 32)  # num_units_in_decoding_tick
            + unsigned_bits(0, 5)  # buffer_removal_time_length_minus_1
            + unsigned_bits(0, 5)  # frame_presentation_time_length_minus_1
        )
    if initial_display_delay is not None:
        require(1 <= initial_display_delay <= 10, "display delay must be between 1 and 10")

    operating_points = syntax[7:]
    if decoder_model_present:
        require(initial_display_delay is None, "decoder-model case cannot add display timing")
        operating_points = (
            syntax[7:point_display_position]
            + [0]  # decoder_model_present_for_this_op
            + syntax[point_display_position:]
        )
    updated = (
        syntax[:5]
        + [1]
        + timing
        + [int(initial_display_delay is not None)]
        + operating_points
    )
    if initial_display_delay is not None:
        insertion = point_display_position + len(timing)
        display_fields = [1] + unsigned_bits(initial_display_delay - 1, 4)
        updated = updated[:insertion] + display_fields + updated[insertion:]
    return pack_bits(updated + [1])


def update_stsz(payload: bytes, sample_sizes: tuple[int, ...]) -> bytes:
    require(payload[:4] == bytes(4), "source stsz is not a version-zero FullBox")
    common_size, count = struct.unpack_from(">II", payload, 4)
    require(
        common_size == 0 and count == len(SOURCE_SAMPLE_SIZES),
        "source stsz layout changed",
    )
    require(len(payload) == 12 + 4 * count, "source stsz table is truncated")
    require(
        struct.unpack_from(f">{count}I", payload, 12) == SOURCE_SAMPLE_SIZES,
        "source sample sizes changed",
    )
    return payload[:12] + struct.pack(f">{count}I", *sample_sizes)


def update_primary_item_extent(payload: bytes, sample_size: int) -> bytes:
    require(payload == SOURCE_ILOC, "source primary-item iloc layout changed")
    return payload[:18] + struct.pack(">I", sample_size) + payload[22:]


def candidate_bytes(case: str) -> bytes:
    source = SOURCE.read_bytes()
    source_digest = hashlib.sha256(source).hexdigest()
    require(source_digest == SOURCE_SHA256, f"source hash changed: {source_digest}")
    case_data = CASES[case]
    initial_display_delay = case_data["initial_display_delay"]
    operating_point_level = case_data.get("operating_point_level")
    decoder_model_present = case_data.get("decoder_model_present", False)
    num_ticks_per_picture = case_data.get("num_ticks_per_picture", 1)

    sys.path.insert(0, str(ROOT / "scripts"))
    from inspect_av1_obus import parse_sequence_header
    from inspect_avif_bitstreams import inspect as inspect_container

    container = inspect_container(SOURCE)
    track = next(item for item in container["tracks"] if item["handler"] == "pict")
    samples = track["samples"]
    require(
        tuple(sample["length"] for sample in samples) == SOURCE_SAMPLE_SIZES,
        "source sample table changed",
    )

    first_sample = source[
        samples[0]["offset"] : samples[0]["offset"] + samples[0]["length"]
    ]
    original_sequence = next(
        payload
        for header, payload in parse_obus(first_sample)
        if (header >> 3) & 0x0F == 1
    )
    updated_sequence = sequence_payload(
        original_sequence,
        initial_display_delay,
        operating_point_level,
        decoder_model_present,
        num_ticks_per_picture,
    )
    parsed_sequence = parse_sequence_header(updated_sequence)
    timing = parsed_sequence["timing"]
    if operating_point_level is None:
        expected_timing = {
            "num_units_in_tick": 1,
            "time_scale": 30,
            "equal_picture_interval": True,
            "num_ticks_per_picture": num_ticks_per_picture,
        }
        if decoder_model_present:
            expected_timing.update(
                {
                    "num_units_in_decoding_tick": 1,
                    "buffer_removal_delay_length": 1,
                    "frame_presentation_delay_length": 1,
                }
            )
        require(timing == expected_timing, "generated timing information differs from the intended syntax")
    else:
        require(timing is None, "high-tier case unexpectedly added timing syntax")
    require(
        parsed_sequence["decoder_model_present"] is decoder_model_present,
        "generated sequence decoder-model flag differs from the intended syntax",
    )
    expected_display_model = initial_display_delay is not None
    operating_point = parsed_sequence["operating_points"][0]
    if decoder_model_present:
        require(
            operating_point["decoder_model_present"] is False,
            "generated operating point unexpectedly declares decoder-model parameters",
        )
    if operating_point_level is not None:
        require(
            operating_point["level"] == operating_point_level
            and operating_point["tier"] == 0,
            "generated high-tier operating point differs from the intended syntax",
        )
    require(
        parsed_sequence["display_model_present"] is expected_display_model
        and operating_point["display_model_present"] is expected_display_model,
        "generated display-model flags differ from the intended syntax",
    )
    require(
        operating_point["initial_display_delay"]
        == (initial_display_delay if initial_display_delay is not None else 10),
        "generated initial display delay differs from the intended syntax",
    )

    updated_samples = []
    frame_syntax = None
    mutate_frame_payload = None
    if decoder_model_present:
        from generate_avif_decoder_model_parity_fixture import (
            FRAME_SYNTAX,
            mutate_frame_payload,
        )
        from inspect_av1_obus import inspect as inspect_obus

        frame_report = inspect_obus(SOURCE)
        track_frames = [sample for sample in frame_report["samples"] if sample["role"] == "track_pict"]
        require(len(track_frames) == len(FRAME_SYNTAX), "source frame-header inventory changed")
        for sample_index, (sample_report, expected_frames) in enumerate(zip(track_frames, FRAME_SYNTAX)):
            actual_frames = [obu for obu in sample_report["obus"] if "frame_header" in obu]
            require(len(actual_frames) == len(expected_frames), f"source sample {sample_index} frame count changed")
            for obu, expected in zip(actual_frames, expected_frames):
                header = obu["frame_header"]
                actual = (
                    header["header_bits"],
                    expected[1],
                    header["frame_type"],
                    header["show_frame"],
                    header["existing_frame_idx"],
                )
                require(actual == expected, f"source sample {sample_index} frame syntax changed: {actual!r}")
        frame_syntax = FRAME_SYNTAX

    for sample_index, sample in enumerate(samples):
        sample_bytes = source[sample["offset"] : sample["offset"] + sample["length"]]
        obus = [
            (header, payload) for header, payload in parse_obus(sample_bytes)
        ]
        rebuilt_obus = []
        frame_index = 0
        for header, payload in obus:
            obu_type = (header >> 3) & 0x0F
            if obu_type == 1:
                payload = updated_sequence
            if frame_syntax is not None and obu_type in (3, 6, 7):
                expected = frame_syntax[sample_index][frame_index]
                frame_index += 1
                if expected[1] is not None:
                    payload = mutate_frame_payload(payload, expected[0], expected[1])
            rebuilt_obus.append(pack_obu(header, payload))
        if frame_syntax is not None:
            require(frame_index == len(frame_syntax[sample_index]), f"sample {sample_index} frame count changed")
        updated_samples.append(b"".join(rebuilt_obus))
    sample_sizes = tuple(map(len, updated_samples))

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == STSZ_PATH:
            return update_stsz(payload, sample_sizes)
        if path == ILOC_PATH:
            return update_primary_item_extent(payload, sample_sizes[0])
        if path == (b"mdat",):
            return b"".join(updated_samples)
        return payload

    candidate = rewrite_boxes(source, transform)
    candidate_digest = hashlib.sha256(candidate).hexdigest()
    require(
        candidate_digest == case_data["sha256"],
        f"fixture hash changed: {candidate_digest}",
    )

    with tempfile.TemporaryDirectory(prefix="avif-sequence-header-") as temporary:
        candidate_path = Path(temporary) / "candidate.avif"
        candidate_path.write_bytes(candidate)
        rebuilt = inspect_container(candidate_path)
    rebuilt_track = next(item for item in rebuilt["tracks"] if item["handler"] == "pict")
    require(
        tuple(sample["length"] for sample in rebuilt_track["samples"]) == sample_sizes,
        "rewritten sample table differs from AV1 sample bytes",
    )
    require(
        rebuilt["items"]["color"][0]["length"] == sample_sizes[0],
        "primary-item extent differs from its AV1 sample bytes",
    )
    return candidate


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--case", choices=CASES, default="timing-without-decoder-model")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    output = ASSETS / str(CASES[args.case]["asset"])
    expected = candidate_bytes(args.case)
    if args.check:
        require(
            output.exists() and output.read_bytes() == expected,
            f"AVIF fixture is stale: {output.name}",
        )
    else:
        require(
            not output.exists() or output.read_bytes() == expected,
            f"refusing to replace a different fixture at {output}",
        )
        output.write_bytes(expected)
    print(f"AVIF fixture verified: {output.name} ({CASES[args.case]['sha256']})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
