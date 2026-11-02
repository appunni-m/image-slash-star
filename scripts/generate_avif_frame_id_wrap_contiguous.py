#!/usr/bin/env python3
"""Generate a valid AVIF sequence whose 15-bit frame IDs wrap around."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parent.parent
INPUT_DIR = ROOT / "tests/fixtures/input/images/avif"
SOURCE_PATH = INPUT_DIR / "animated_error_resilient.avif"
OUTPUT_PATH = INPUT_DIR / "animated_frame_id_wrap_contiguous.avif"
SOURCE_SHA256 = "06ea9771f8b46c3432c6c6cdf324f1c05e86a5fdccd774c8e3c9a8fce0b831f0"
OUTPUT_SHA256 = "8a53bf96c97b66d07b849ec6e4a6dcf18f37ab9db5b3edb045f2b357bd59e35e"
SOURCE_FRAME_IDS = (4626, 4627)
OUTPUT_FRAME_IDS = (32760, 10)
FRAME_ID_BITS = 15
DELTA_FRAME_ID_BITS = 14
SOURCE_REFERENCE_DELTA = 1
OUTPUT_REFERENCE_DELTA = 18


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_bits(data: bytearray, start: int, width: int, value: int) -> None:
    """Replace one bounded big-endian AV1 syntax field."""
    if start < 0 or width <= 0 or start + width > len(data) * 8:
        raise ValueError("AV1 frame-ID field is outside the pinned AVIF")
    if value < 0 or value >= 1 << width:
        raise ValueError("replacement AV1 field does not fit its encoded width")
    for index in range(width):
        position = start + index
        mask = 1 << (7 - position % 8)
        bit = (value >> (width - index - 1)) & 1
        data[position // 8] = (data[position // 8] & ~mask) | (bit * mask)


def source_fields() -> tuple[list[tuple[int, int]], list[tuple[int, int]], dict]:
    """Validate the source headers and locate IDs plus all seven ref deltas."""
    report = inspect(SOURCE_PATH)
    samples = [sample for sample in report["samples"] if sample["role"] == "track_pict"]
    if len(samples) != 2:
        raise ValueError("pinned error-resilient sequence must have two picture samples")

    sequence = next(
        (obu["sequence_header"] for obu in samples[0]["obus"] if obu["type"] == 1),
        None,
    )
    if sequence is None or (
        sequence.get("frame_id_bits"), sequence.get("delta_frame_id_bits")
    ) != (FRAME_ID_BITS, DELTA_FRAME_ID_BITS):
        raise ValueError("pinned sequence frame-ID widths differ from the expected values")

    frame_fields: list[tuple[int, int]] = []
    frame_ids: list[int] = []
    second_frame = None
    for sample in samples:
        obus = [obu for obu in sample["obus"] if obu["type"] == 6]
        if len(obus) != 1 or len(obus[0]["payload_spans"]) != 1:
            raise ValueError("each frame header must have one contiguous payload span")
        frame = obus[0]
        header = frame["frame_header"]
        payload_start = frame["payload_spans"][0]["offset"] * 8
        frame_ids.append(header.get("frame_id"))
        frame_fields.append((payload_start + header["frame_id_bit"], FRAME_ID_BITS))
        second_frame = (frame, payload_start)

    if tuple(frame_ids) != SOURCE_FRAME_IDS or second_frame is None:
        raise ValueError("source frame IDs differ from their pinned values")
    frame, payload_start = second_frame
    reference_ids = frame["frame_header"].get("reference_frame_ids", [])
    if len(reference_ids) != 7 or any(
        (item.get("delta"), item.get("expected"), item.get("actual"))
        != (SOURCE_REFERENCE_DELTA, SOURCE_FRAME_IDS[0], SOURCE_FRAME_IDS[0])
        or item.get("delta_bits") != DELTA_FRAME_ID_BITS
        for item in reference_ids
    ):
        raise ValueError("source reference frame-ID deltas differ from their pinned values")
    delta_fields = [
        (payload_start + item["delta_bit"], item["delta_bits"])
        for item in reference_ids
    ]
    return frame_fields, delta_fields, sequence


def fixture_bytes(source: bytes) -> bytes:
    """Move both frame IDs across wrap while preserving decoded reference data."""
    if sha256(source) != SOURCE_SHA256:
        raise ValueError("error-resilient AVIF differs from its pinned source")
    frame_fields, delta_fields, sequence = source_fields()
    frame_id_range = 1 << sequence["frame_id_bits"]
    modulo_delta = (OUTPUT_FRAME_IDS[1] - OUTPUT_FRAME_IDS[0]) % frame_id_range
    if modulo_delta != OUTPUT_REFERENCE_DELTA:
        raise ValueError("replacement frame IDs do not produce the pinned modulo delta")

    output = bytearray(source)
    for (start, width), frame_id in zip(frame_fields, OUTPUT_FRAME_IDS, strict=True):
        write_bits(output, start, width, frame_id)
    encoded_delta_minus_one = OUTPUT_REFERENCE_DELTA - 1
    for start, width in delta_fields:
        write_bits(output, start, width, encoded_delta_minus_one)

    changed = {
        index
        for index, (before, after) in enumerate(zip(source, output, strict=True))
        if before != after
    }
    allowed = {
        position // 8
        for start, width in (*frame_fields, *delta_fields)
        for position in range(start, start + width)
    }
    if not changed or not changed <= allowed:
        raise ValueError("fixture mutation changed bytes outside AV1 frame-ID fields")

    result = bytes(output)
    if sha256(result) != OUTPUT_SHA256:
        raise ValueError("generated frame-ID wrap fixture differs from its pinned bytes")
    return result


def verify_output() -> None:
    """Confirm the generated public input has consistent wrapped references."""
    report = inspect(OUTPUT_PATH)
    samples = [sample for sample in report["samples"] if sample["role"] == "track_pict"]
    frames = [next(obu for obu in sample["obus"] if obu["type"] == 6) for sample in samples]
    if [frame["frame_header"].get("frame_id") for frame in frames] != list(
        OUTPUT_FRAME_IDS
    ):
        raise ValueError("generated frame IDs differ from their pinned wrapped values")
    references = frames[1]["frame_header"].get("reference_frame_ids", [])
    if len(references) != 7 or any(
        (item.get("delta"), item.get("expected"), item.get("actual"))
        != (OUTPUT_REFERENCE_DELTA, OUTPUT_FRAME_IDS[0], OUTPUT_FRAME_IDS[0])
        for item in references
    ):
        raise ValueError("generated frame references do not match the wrapped frame ID")


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
        OUTPUT_PATH.write_bytes(output)
        action = "wrote"
    verify_output()
    print(f"{action} {OUTPUT_PATH.relative_to(ROOT)} sha256={OUTPUT_SHA256}")


if __name__ == "__main__":
    main()
