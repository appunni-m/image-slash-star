#!/usr/bin/env python3
"""Generate an AVIF with a unique frame ID outside sequence continuity."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parent.parent
INPUT_DIR = ROOT / "tests/fixtures/input/images/avif"
SOURCE_PATH = INPUT_DIR / "animated_error_resilient.avif"
SOURCE_SHA256 = "06ea9771f8b46c3432c6c6cdf324f1c05e86a5fdccd774c8e3c9a8fce0b831f0"
PREVIOUS_FRAME_ID = 4626
SOURCE_FRAME_ID = 4627
FIXTURES = (
    (
        "animated_frame_id_outside_continuity.avif",
        21010,
        16384,
        "d51d8d0ca98aa4cd24e7b464d71235bc89e767c5948a70952efc9df45cd2255d",
    ),
    (
        "animated_frame_id_wrapped_outside_continuity.avif",
        4625,
        32767,
        "54329521e9374cd69c65026a99990ca9d8aae31a9a74f519f83fdb9228f5a07a",
    ),
)


def read_bits(data: bytes, start: int, width: int) -> int:
    if start < 0 or width <= 0 or start + width > len(data) * 8:
        raise ValueError("frame-ID field is outside the pinned AVIF")
    value = 0
    for position in range(start, start + width):
        value = (value << 1) | ((data[position // 8] >> (7 - position % 8)) & 1)
    return value


def write_bits(data: bytearray, start: int, width: int, value: int) -> None:
    if (
        start < 0
        or width <= 0
        or start + width > len(data) * 8
        or value < 0
        or value >= 1 << width
    ):
        raise ValueError("replacement frame-ID field is out of range")
    for index in range(width):
        position = start + index
        mask = 1 << (7 - position % 8)
        bit = (value >> (width - index - 1)) & 1
        data[position // 8] = (data[position // 8] & ~mask) | (bit * mask)


def frame_id_field(source: bytes) -> tuple[int, int]:
    """Locate the second frame ID in the pinned error-resilient sequence."""
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise ValueError("error-resilient AVIF differs from its pinned source")

    report = inspect(SOURCE_PATH)
    primary_sample = next(
        (sample for sample in report["samples"] if sample["role"] == "item_color"),
        None,
    )
    if primary_sample is None:
        raise ValueError("error-resilient AVIF has no primary color sample")
    sequence_obu = next(
        (obu for obu in primary_sample["obus"] if obu["type"] == 1),
        None,
    )
    if sequence_obu is None:
        raise ValueError("error-resilient AVIF has no AV1 sequence header")
    sequence_header = sequence_obu["sequence_header"]
    frame_id_width = sequence_header.get("frame_id_bits")
    if not sequence_header.get("frame_id_numbers_present") or frame_id_width != 15:
        raise ValueError("error-resilient AVIF frame-ID syntax differs from its pin")

    track_samples = [
        sample for sample in report["samples"] if sample["role"] == "track_pict"
    ]
    if len(track_samples) != 2:
        raise ValueError("error-resilient AVIF must retain its two color samples")

    frame_ids = []
    second_frame_id_start = None
    for sample in track_samples:
        frame_obus = [obu for obu in sample["obus"] if obu["type"] == 6]
        if len(frame_obus) != 1:
            raise ValueError("error-resilient AVIF sample has unexpected frame OBUs")
        frame_obu = frame_obus[0]
        spans = frame_obu["payload_spans"]
        if len(spans) != 1:
            raise ValueError("error-resilient frame header is not physically contiguous")
        frame_header = frame_obu["frame_header"]
        frame_id = frame_header.get("frame_id")
        frame_id_start = spans[0]["offset"] * 8 + frame_header["frame_id_bit"]
        if read_bits(source, frame_id_start, frame_id_width) != frame_id:
            raise ValueError("AV1 inspector frame ID differs from its encoded bits")
        frame_ids.append(frame_id)
        second_frame_id_start = frame_id_start

    if frame_ids != [PREVIOUS_FRAME_ID, SOURCE_FRAME_ID]:
        raise ValueError("error-resilient frame IDs differ from their pinned values")
    if second_frame_id_start is None:
        raise ValueError("error-resilient AVIF has no second frame-ID field")
    return second_frame_id_start, frame_id_width

def fixture_bytes(
    source: bytes,
    frame_id_start: int,
    frame_id_width: int,
    replacement_frame_id: int,
    expected_difference: int,
    expected_sha256: str,
) -> bytes:
    """Replace only the second frame ID and pin its modulo difference."""
    difference = (
        replacement_frame_id - PREVIOUS_FRAME_ID
        if replacement_frame_id > PREVIOUS_FRAME_ID
        else (1 << frame_id_width) + replacement_frame_id - PREVIOUS_FRAME_ID
    )
    if (
        replacement_frame_id == PREVIOUS_FRAME_ID
        or difference != expected_difference
        or difference < 1 << (frame_id_width - 1)
    ):
        raise ValueError("replacement frame ID differs from its pinned continuity edge")

    output = bytearray(source)
    write_bits(output, frame_id_start, frame_id_width, replacement_frame_id)
    if read_bits(output, frame_id_start, frame_id_width) != replacement_frame_id:
        raise ValueError("AV1 frame-ID replacement did not round-trip")

    changed = [
        index
        for index, (before, after) in enumerate(zip(source, output))
        if before != after
    ]
    first_changed_byte = frame_id_start // 8
    last_changed_byte = (frame_id_start + frame_id_width - 1) // 8
    if any(not first_changed_byte <= index <= last_changed_byte for index in changed):
        raise ValueError("frame-ID mutation changed bytes outside the encoded field")

    result = bytes(output)
    if hashlib.sha256(result).hexdigest() != expected_sha256:
        raise ValueError("frame-ID continuity fixture differs from its pinned mutation")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    source = SOURCE_PATH.read_bytes()
    frame_id_start, frame_id_width = frame_id_field(source)
    for name, replacement, difference, expected_sha256 in FIXTURES:
        output_path = INPUT_DIR / name
        result = fixture_bytes(
            source,
            frame_id_start,
            frame_id_width,
            replacement,
            difference,
            expected_sha256,
        )
        if args.check:
            if not output_path.exists() or output_path.read_bytes() != result:
                raise SystemExit(
                    f"{output_path} differs from its deterministic mutation"
                )
            action = "verified"
        else:
            output_path.write_bytes(result)
            action = "wrote"
        print(
            f"{action} {output_path.relative_to(ROOT)} "
            f"sha256={expected_sha256}"
        )


if __name__ == "__main__":
    main()
