#!/usr/bin/env python3
"""Generate an animated AVIF with a repeated AV1 decoder-model header."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct
import sys
import tempfile

from avif_bmff_boxes import read_boxes, rewrite_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "animated.avif"
OUTPUT = ASSETS / "animated_decoder_model_parameters_change.avif"
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
FIXTURE_SHA256 = "9191549ae1373fc31e093a36d9ff2d88525f4219378c7b7ba1ecb6b74cec8396"

SAMPLE_SIZES = (39, 113, 5, 30, 25)
STSZ_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stsz")
ILOC_PATH = (b"meta", b"iloc")
SOURCE_ILOC = bytes.fromhex("0000000044000001000100000001000003ff00000027")

# These offsets are guarded by the source digest and the independent OBU
# inspector below. Each value is the bit position where decoder-model-enabled
# frame syntax reads buffer_removal_time_present_flag.
FRAME_SYNTAX = (
    ((68, 14, "key", True, None),),
    (
        (118, 19, "inter", False, None),
        (118, 19, "inter", False, None),
        (117, 18, "inter", True, None),
    ),
    ((4, None, "inter", True, 2),),
    ((117, 18, "inter", True, None),),
    ((117, 18, "inter", True, None),),
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def bits_from_bytes(data: bytes, count: int | None = None) -> list[int]:
    length = len(data) * 8 if count is None else count
    return [(data[index // 8] >> (7 - index % 8)) & 1 for index in range(length)]


def pack_bits(bits: list[int]) -> bytes:
    padded = bits + [0] * ((-len(bits)) % 8)
    output = bytearray(len(padded) // 8)
    for index, bit in enumerate(padded):
        output[index // 8] |= bit << (7 - index % 8)
    return bytes(output)


def unsigned_bits(value: int, width: int) -> list[int]:
    return [(value >> shift) & 1 for shift in reversed(range(width))]


def sequence_payload(source: bytes, decoder_buffer_delay: int) -> bytes:
    """Add valid timing/model syntax and one operating-point parameter."""
    require(source == bytes.fromhex("00000003bcaca9b5f22021a0d080"), "source sequence payload changed")
    raw_bits = bits_from_bytes(source)
    last_byte = source[-1]
    trailing_zeros = (last_byte & -last_byte).bit_length() - 1
    syntax_end = len(raw_bits) - trailing_zeros - 1
    syntax = raw_bits[:syntax_end]
    require(syntax[:6] == [0, 0, 0, 0, 0, 0], "source sequence prefix changed")
    require(syntax[6:29] == [0] * 23, "source operating-point layout changed")
    require(decoder_buffer_delay in (0, 1), "decoder-buffer delay must fit the one-bit field")

    timing = (
        unsigned_bits(1, 32)  # num_units_in_display_tick
        + unsigned_bits(30, 32)  # time_scale
        + [1]  # equal_picture_interval
        + [1]  # uvlc(0), num_ticks_per_picture_minus_1
        + [1]  # decoder_model_info_present_flag
        + unsigned_bits(0, 5)  # buffer_delay_length_minus_1
        + unsigned_bits(1, 32)  # num_units_in_decoding_tick
        + unsigned_bits(0, 5)  # buffer_removal_time_length_minus_1
        + unsigned_bits(0, 5)  # frame_presentation_time_length_minus_1
    )
    require(len(timing) == 114, "decoder-model timing syntax has the wrong width")

    operating_point = [1, decoder_buffer_delay, 0, 0]
    updated = syntax[:5] + [1] + timing + syntax[6:29] + operating_point + syntax[29:]
    return pack_bits(updated + [1])


def read_uleb(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    while True:
        if offset >= len(data) or shift > 49:
            raise RuntimeError("OBU size field is truncated or too wide")
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
        require(header & 0x84 == 0, "source fixture unexpectedly has OBU extension bits")
        require(header & 1 == 0, "source fixture has a reserved OBU header bit")
        payload_size, offset = read_uleb(sample, offset)
        end = offset + payload_size
        require(end <= len(sample), "source OBU exceeds its sample")
        obus.append((header, sample[offset:end]))
        offset = end
    return obus


def pack_obu(header: int, payload: bytes) -> bytes:
    return bytes([header]) + write_uleb(len(payload)) + payload


def mutate_frame_payload(payload: bytes, header_bits: int, flag_bit: int) -> bytes:
    require(flag_bit < header_bits, "buffer-removal flag is outside the frame header")
    bits = bits_from_bytes(payload)
    frame_header = bits[:flag_bit] + [0] + bits[flag_bit:header_bits]
    aligned_header = pack_bits(frame_header)
    tile_payload = payload[(header_bits + 7) // 8 :]
    result = aligned_header + tile_payload
    require(len(result) == len(payload), "frame timing flag unexpectedly changes sample size")
    return result


def update_stsz(payload: bytes, sample_sizes: tuple[int, ...]) -> bytes:
    require(payload[:4] == bytes(4), "source stsz is not a version-zero FullBox")
    common_size, count = struct.unpack_from(">II", payload, 4)
    require(common_size == 0 and count == len(SAMPLE_SIZES), "source stsz layout changed")
    require(len(payload) == 12 + 4 * count, "source stsz table is truncated")
    require(struct.unpack_from(f">{count}I", payload, 12) == SAMPLE_SIZES, "source sample sizes changed")
    return payload[:12] + struct.pack(f">{count}I", *sample_sizes)


def update_primary_item_extent(payload: bytes, sample_size: int) -> bytes:
    require(payload == SOURCE_ILOC, "source primary-item iloc layout changed")
    return payload[:18] + struct.pack(">I", sample_size) + payload[22:]


def candidate_bytes() -> bytes:
    source = SOURCE.read_bytes()
    source_digest = hashlib.sha256(source).hexdigest()
    require(source_digest == SOURCE_SHA256, f"animated source hash changed: {source_digest}")

    sys.path.insert(0, str(ROOT / "scripts"))
    from inspect_av1_obus import inspect as inspect_obus
    from inspect_avif_bitstreams import inspect as inspect_container

    report = inspect_obus(SOURCE)
    track_samples = [
        sample for sample in report["samples"] if sample["role"] == "track_pict"
    ]
    require(len(track_samples) == len(FRAME_SYNTAX), "source track sample count changed")
    for index, (sample, expected_frames) in enumerate(zip(track_samples, FRAME_SYNTAX)):
        actual_frames = [
            obu for obu in sample["obus"] if "frame_header" in obu
        ]
        require(len(actual_frames) == len(expected_frames), f"sample {index} frame count changed")
        for obu, expected in zip(actual_frames, expected_frames):
            header = obu["frame_header"]
            actual = (
                header["header_bits"],
                expected[1],
                header["frame_type"],
                header["show_frame"],
                header["existing_frame_idx"],
            )
            require(actual == expected, f"sample {index} frame syntax changed: {actual!r}")

    container = inspect_container(SOURCE)
    track = next(item for item in container["tracks"] if item["handler"] == "pict")
    samples = track["samples"]
    require(tuple(item["length"] for item in samples) == SAMPLE_SIZES, "source AVIF sample table changed")
    top_level = read_boxes(source, 0, len(source))
    require([kind for kind, _, _ in top_level] == [b"ftyp", b"meta", b"moov", b"mdat"], "source top-level box layout changed")
    mdat_payload = top_level[-1][2]
    require(sum(SAMPLE_SIZES) == len(mdat_payload), "track samples no longer fill mdat")

    source_sequence = next(
        payload
        for header, payload in parse_obus(source[samples[0]["offset"] : samples[0]["offset"] + samples[0]["length"]])
        if (header >> 3) & 0x0F == 1
    )
    sequence_a = sequence_payload(source_sequence, 0)
    sequence_b = sequence_payload(source_sequence, 1)

    new_samples = []
    for sample in samples:
        index = sample["index"]
        data = source[sample["offset"] : sample["offset"] + sample["length"]]
        rebuilt = []
        frame_index = 0
        inserted_repeated_header = False
        for header, payload in parse_obus(data):
            obu_type = (header >> 3) & 0x0F
            if obu_type == 1:
                payload = sequence_a
            if obu_type in (3, 6, 7):
                syntax = FRAME_SYNTAX[index][frame_index]
                frame_index += 1
                if syntax[1] is not None:
                    payload = mutate_frame_payload(payload, syntax[0], syntax[1])
            rebuilt.append(pack_obu(header, payload))
            if index == 1 and obu_type == 2:
                rebuilt.append(pack_obu(0x0A, sequence_b))
                inserted_repeated_header = True
        require(frame_index == len(FRAME_SYNTAX[index]), f"sample {index} frame count changed during rewrite")
        if index == 1:
            require(inserted_repeated_header, "sample 1 has no temporal delimiter for the repeated header")
        new_samples.append(b"".join(rebuilt))

    sizes = tuple(len(sample) for sample in new_samples)

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == STSZ_PATH:
            return update_stsz(payload, sizes)
        if path == ILOC_PATH:
            return update_primary_item_extent(payload, sizes[0])
        if path == (b"mdat",):
            return b"".join(new_samples)
        return payload

    candidate = rewrite_boxes(source, transform)
    digest = hashlib.sha256(candidate).hexdigest()
    require(digest == FIXTURE_SHA256, f"generated fixture hash changed: {digest}")

    with tempfile.TemporaryDirectory(prefix="avif-decoder-model-") as temporary:
        candidate_path = Path(temporary) / "candidate.avif"
        candidate_path.write_bytes(candidate)
        rebuilt_container = inspect_container(candidate_path)
    rebuilt_track = next(item for item in rebuilt_container["tracks"] if item["handler"] == "pict")
    require(
        tuple(item["length"] for item in rebuilt_track["samples"]) == sizes,
        "rewritten sample table does not describe the AV1 sample bytes",
    )
    require(
        rebuilt_container["items"]["color"][0]["length"] == sizes[0],
        "primary-item extent does not describe the first AV1 sample",
    )
    return candidate


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    expected = candidate_bytes()
    if args.check:
        require(OUTPUT.exists() and OUTPUT.read_bytes() == expected, "AVIF decoder-model parity fixture is stale")
    else:
        require(not OUTPUT.exists() or OUTPUT.read_bytes() == expected, f"refusing to replace a different fixture at {OUTPUT}")
        OUTPUT.write_bytes(expected)
    print(f"AVIF decoder-model parity fixture verified: {FIXTURE_SHA256}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
