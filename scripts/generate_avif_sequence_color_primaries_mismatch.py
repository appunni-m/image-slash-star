#!/usr/bin/env python3
"""Generate an AVIF with changed color primaries in a repeated AV1 header."""

from __future__ import annotations

import argparse
import copy
import hashlib
from pathlib import Path

from inspect_av1_obus import parse_sequence_header
from inspect_avif_bitstreams import inspect as inspect_avif


ROOT = Path(__file__).resolve().parent.parent
INPUT_PATH = (
    ROOT / "tests/fixtures/input/images/avif/animated_decoder_model_parameters_change.avif"
)
OUTPUT_PATH = (
    ROOT / "tests/fixtures/input/images/avif/error_animated_sequence_color_primaries_change.avif"
)
INPUT_SHA256 = "9191549ae1373fc31e093a36d9ff2d88525f4219378c7b7ba1ecb6b74cec8396"
OUTPUT_SHA256 = "3a4039604a7bf10739a7bcdf5a32c406c3ed5b772b9fdad58a88e73bc6571588"
COLOR_PRIMARIES_BIT = 193
SOURCE_COLOR_PRIMARIES = 1
OUTPUT_COLOR_PRIMARIES = 9
EXPECTED_SAMPLE_COUNT = 5


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


def write_bits(data: bytearray, start: int, width: int, value: int) -> None:
    """Replace one bounded big-endian AV1 syntax field."""
    if start < 0 or width <= 0 or start + width > len(data) * 8:
        raise ValueError("AV1 sequence-header field is outside the pinned sample")
    if value < 0 or value >= 1 << width:
        raise ValueError("replacement color-primaries value does not fit its field")
    for index in range(width):
        position = start + index
        mask = 1 << (7 - position % 8)
        bit = (value >> (width - index - 1)) & 1
        data[position // 8] = (data[position // 8] & ~mask) | (bit * mask)


def sequence_obus(data: bytes, container: dict) -> list[tuple[int, int, bytes]]:
    """Return each sequence-header payload with its sample index and offset."""
    tracks = [track for track in container["tracks"] if track["handler"] == "pict"]
    if len(tracks) != 1:
        raise ValueError("pinned AVIF must contain one picture track")
    samples = tracks[0]["samples"]
    if len(samples) != EXPECTED_SAMPLE_COUNT:
        raise ValueError("pinned AVIF must contain five picture samples")

    sequence_headers = []
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
                sequence_headers.append(
                    (sample_index, payload_start, data[payload_start:payload_end])
                )
            cursor = payload_end
        if cursor != sample_end:
            raise ValueError("AV1 OBUs do not fill the pinned sample")
    return sequence_headers


def without_color_primaries(header: dict) -> dict:
    normalized = copy.deepcopy(header)
    normalized["color_primaries"] = None
    return normalized


def fixture_bytes(source: bytes) -> bytes:
    """Change only color_primaries in the second sequence header."""
    if sha256(source) != INPUT_SHA256:
        raise ValueError("decoder-model AVIF differs from its pinned source")

    container = inspect_avif(INPUT_PATH)
    sequence_headers = sequence_obus(source, container)
    if [sample_index for sample_index, _, _ in sequence_headers] != [0, 1]:
        raise ValueError("pinned AVIF must repeat its sequence header in sample 1")
    parsed_headers = [parse_sequence_header(payload) for _, _, payload in sequence_headers]
    if any(
        header.get("color_primaries") != SOURCE_COLOR_PRIMARIES
        for header in parsed_headers
    ):
        raise ValueError("pinned AV1 color primaries differ from the expected value")
    if parsed_headers[1]["operating_points"][0]["decoder_model"][
        "decoder_buffer_delay"
    ] != 1:
        raise ValueError("pinned repeated header lost its allowed decoder-model change")

    output = bytearray(source)
    _, payload_start, _ = sequence_headers[1]
    write_bits(
        output,
        payload_start * 8 + COLOR_PRIMARIES_BIT,
        8,
        OUTPUT_COLOR_PRIMARIES,
    )
    result = bytes(output)

    changed_bits = []
    for byte_index, (before, after) in enumerate(zip(source, result, strict=True)):
        different = before ^ after
        for bit_index in range(8):
            if different & (1 << (7 - bit_index)):
                changed_bits.append(byte_index * 8 + bit_index)
    if changed_bits != [payload_start * 8 + COLOR_PRIMARIES_BIT + 4]:
        raise ValueError("fixture mutation changed bits outside color_primaries")

    output_headers = sequence_obus(result, container)
    parsed_output_headers = [
        parse_sequence_header(payload) for _, _, payload in output_headers
    ]
    if [header.get("color_primaries") for header in parsed_output_headers] != [
        SOURCE_COLOR_PRIMARIES,
        OUTPUT_COLOR_PRIMARIES,
    ]:
        raise ValueError("mutated sequence-header color primaries differ from their pin")
    if without_color_primaries(parsed_headers[1]) != without_color_primaries(
        parsed_output_headers[1]
    ):
        raise ValueError("fixture mutation changed another sequence-header field")
    if sha256(result) != OUTPUT_SHA256:
        raise ValueError("generated AVIF differs from its pinned bytes")
    return result


def verify_output() -> None:
    container = inspect_avif(OUTPUT_PATH)
    tracks = [track for track in container["tracks"] if track["handler"] == "pict"]
    if len(tracks) != 1 or len(tracks[0]["samples"]) != EXPECTED_SAMPLE_COUNT:
        raise ValueError("generated AVIF must contain one five-sample picture track")
    headers = sequence_obus(OUTPUT_PATH.read_bytes(), container)
    parsed_headers = [parse_sequence_header(payload) for _, _, payload in headers]
    if [header.get("color_primaries") for header in parsed_headers] != [
        SOURCE_COLOR_PRIMARIES,
        OUTPUT_COLOR_PRIMARIES,
    ]:
        raise ValueError("generated AV1 sequence headers differ from their pinned values")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    output = fixture_bytes(INPUT_PATH.read_bytes())
    if args.check:
        if not OUTPUT_PATH.exists() or OUTPUT_PATH.read_bytes() != output:
            raise SystemExit(f"{OUTPUT_PATH} differs from the deterministic mutation")
        action = "verified"
    else:
        if OUTPUT_PATH.exists() and OUTPUT_PATH.read_bytes() != output:
            raise SystemExit(f"refusing to replace a different fixture at {OUTPUT_PATH}")
        OUTPUT_PATH.write_bytes(output)
        action = "wrote"
    verify_output()
    print(f"{action} {OUTPUT_PATH.relative_to(ROOT)} sha256={OUTPUT_SHA256}")


if __name__ == "__main__":
    main()
