#!/usr/bin/env python3
"""Generate a complete AVIF parity fixture for nonuniform AV1 tile parsing."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

import inspect_av1_obus


ROOT = Path(__file__).resolve().parent.parent
SOURCE_PATH = (
    ROOT
    / "tests/fixtures/input/images/avif/uniform_tile_rows_monochrome_256.avif"
)
SOURCE_SHA256 = "782568b37a63b44e632aee68896bcee3c59c5492081c7ac2ac6a31fc0ca1df37"
UNIFORM_TILE_SPACING_FLAG_BIT = 3
EXPECTED_MUTATED_TILE_ERROR = "AV1 tile size exceeds tile-group payload"
CASES = {
    "tile-group-mismatch": {
        "asset": "error_nonuniform_tile_grid_tile_group_mismatch.avif",
        "sha256": "31f9bc09f7a52b56a9059a51d96c464bad715135557bfbf649c38501f55d6411",
        "rows": 4,
        "ns_threshold_false": False,
    },
    "ns-threshold-false": {
        "asset": "error_nonuniform_tile_grid_ns_threshold_false.avif",
        "sha256": "b7a9dc5b14f8115b21f131700351a26cb679f6c23ded975a47489291e074f9fa",
        "rows": 3,
        "ns_threshold_false": True,
    },
}


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def file_to_sample_offset(sample_spans: list[dict], file_offset: int) -> int:
    sample_offset = 0
    for span in sample_spans:
        start = span["offset"]
        end = start + span["length"]
        if start <= file_offset < end:
            return sample_offset + file_offset - start
        sample_offset += end - start
    raise ValueError("frame OBU payload is outside the pinned AV1 sample")


def source_frame_layout() -> tuple[dict, dict]:
    report = inspect_av1_obus.inspect(SOURCE_PATH)
    color_samples = [
        sample for sample in report["samples"] if sample["role"] == "item_color"
    ]
    if len(color_samples) != 1 or color_samples[0]["length"] != 69:
        raise ValueError("pinned monochrome AVIF sample layout changed")
    sample = color_samples[0]
    frame_obus = [obu for obu in sample["obus"] if obu["type"] == 6]
    if len(frame_obus) != 1:
        raise ValueError("pinned monochrome AVIF must contain one frame OBU")
    frame_obu = frame_obus[0]
    header = frame_obu.get("frame_header")
    if not isinstance(header, dict):
        raise ValueError("pinned monochrome AVIF frame header is unavailable")
    tiling = header.get("tiling")
    if (
        header.get("frame_width") != 256
        or header.get("frame_height") != 256
        or not isinstance(tiling, dict)
        or tiling.get("uniform") is not True
        or tiling.get("columns") != 1
        or tiling.get("rows") != 4
    ):
        raise ValueError("pinned frame is not a uniform 1x4 tile grid")
    payload_spans = frame_obu.get("payload_spans")
    if not isinstance(payload_spans, list) or len(payload_spans) != 1:
        raise ValueError("pinned frame OBU payload must be one contiguous span")
    payload_span = payload_spans[0]
    if payload_span["length"] < 1:
        raise ValueError("pinned frame OBU payload is empty")
    return sample, payload_span


def sample_bytes(data: bytes, spans: list[dict]) -> bytes:
    return b"".join(
        data[span["offset"] : span["offset"] + span["length"]]
        for span in spans
    )


def parse_nonuniform_sample(sample_payload: bytes) -> tuple[dict, list[tuple[int, int, int]]]:
    parsed_tilings: list[dict] = []
    ns_calls: list[tuple[int, int, int]] = []
    original_read_tiling = inspect_av1_obus.read_tiling
    original_ns = inspect_av1_obus.Bits.ns

    def record_tiling(bits, sequence, header):
        result = original_read_tiling(bits, sequence, header)
        parsed_tilings.append(result)
        return result

    def record_ns(bits, count):
        start = bits.position
        result = original_ns(bits, count)
        ns_calls.append((count, start, result))
        return result

    inspect_av1_obus.read_tiling = record_tiling
    inspect_av1_obus.Bits.ns = record_ns
    try:
        try:
            inspect_av1_obus.parse_obus(
                sample_payload,
                [(0, len(sample_payload))],
                inspect_av1_obus.FrameState(),
            )
        except ValueError as error:
            if str(error) != EXPECTED_MUTATED_TILE_ERROR:
                raise
        else:
            raise ValueError("mutated tile group unexpectedly parsed successfully")
    finally:
        inspect_av1_obus.read_tiling = original_read_tiling
        inspect_av1_obus.Bits.ns = original_ns

    if len(parsed_tilings) != 1:
        raise ValueError("mutated frame did not reach exactly one AV1 tile header")
    return parsed_tilings[0], ns_calls


def mutate_sample(data: bytes, sample: dict, payload_span: dict, case: dict) -> tuple[bytes, int | None]:
    sample_payload = bytearray(sample_bytes(data, sample["spans"]))
    frame_payload_offset = file_to_sample_offset(
        sample["spans"], payload_span["offset"]
    )
    flag_position = frame_payload_offset * 8 + UNIFORM_TILE_SPACING_FLAG_BIT
    sample_payload[flag_position // 8] ^= 1 << (7 - flag_position % 8)

    threshold_bit_position = None
    if case["ns_threshold_false"]:
        layout, calls = parse_nonuniform_sample(bytes(sample_payload))
        if layout.get("rows") != 4:
            raise ValueError("initial tile mutation no longer reaches the 2x4 grid")
        count_three = [call for call in calls if call[0] == 3]
        if len(count_three) != 1 or count_three[0][2] != 0:
            raise ValueError("pinned ns(3) prefix no longer takes the below-threshold arm")
        threshold_bit_position = count_three[0][1]
        absolute = frame_payload_offset * 8 + threshold_bit_position
        if (sample_payload[absolute // 8] >> (7 - absolute % 8)) & 1:
            raise ValueError("pinned ns(3) threshold bit is not clear")
        sample_payload[absolute // 8] |= 1 << (7 - absolute % 8)

    layout, calls = parse_nonuniform_sample(bytes(sample_payload))
    expected_rows = case["rows"]
    expected_row_starts = [0, 1, 2, 3, 4] if expected_rows == 4 else [0, 1, 3, 4]
    if (
        layout.get("uniform") is not False
        or layout.get("columns") != 2
        or layout.get("rows") != expected_rows
        or layout.get("column_starts") != [0, 2, 4]
        or layout.get("row_starts") != expected_row_starts
    ):
        raise ValueError("mutation did not select its pinned nonuniform tile grid")

    count_three = [call for call in calls if call[0] == 3]
    if len(count_three) != 1:
        raise ValueError("pinned tile grid did not execute exactly one ns(3) read")
    count, start, value = count_three[0]
    threshold = (1 << count.bit_length()) - count
    prefix = (sample_payload[frame_payload_offset + start // 8] >> (7 - start % 8)) & 1
    threshold_false = prefix >= threshold
    if threshold_false is not case["ns_threshold_false"]:
        raise ValueError("fixture does not select its declared ns threshold outcome")
    if threshold_false and value < threshold:
        raise ValueError("ns(3) extended value did not reach the fallback arm")
    if not threshold_false and value >= threshold:
        raise ValueError("ns(3) prefix did not select the early return arm")
    return bytes(sample_payload), threshold_bit_position


def changed_bit_positions(before: bytes, after: bytes) -> list[int]:
    positions = []
    for byte_index, (left, right) in enumerate(zip(before, after, strict=True)):
        different = left ^ right
        for bit_index in range(8):
            if different & (1 << (7 - bit_index)):
                positions.append(byte_index * 8 + bit_index)
    return positions


def fixture_bytes(case_name: str) -> bytes:
    case = CASES[case_name]
    source = SOURCE_PATH.read_bytes()
    if sha256(source) != SOURCE_SHA256:
        raise ValueError("source AVIF differs from its pinned bytes")
    sample, payload_span = source_frame_layout()
    mutated_sample, threshold_bit_position = mutate_sample(
        source, sample, payload_span, case
    )

    output = bytearray(source)
    cursor = 0
    for span in sample["spans"]:
        start = span["offset"]
        end = start + span["length"]
        output[start:end] = mutated_sample[cursor : cursor + end - start]
        cursor += end - start
    expected_bits = [
        payload_span["offset"] * 8 + UNIFORM_TILE_SPACING_FLAG_BIT
    ]
    if threshold_bit_position is not None:
        expected_bits.append(payload_span["offset"] * 8 + threshold_bit_position)
    result = bytes(output)
    if changed_bit_positions(source, result) != sorted(expected_bits):
        raise ValueError("fixture mutation changed bits outside its AV1 tile syntax")
    if sha256(result) != case["sha256"]:
        raise ValueError("generated AVIF differs from its pinned bytes")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", required=True, choices=CASES)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    output = fixture_bytes(args.case)
    output_path = ROOT / "tests/fixtures/input/images/avif" / CASES[args.case]["asset"]
    if args.check:
        if not output_path.exists() or output_path.read_bytes() != output:
            raise SystemExit(f"{output_path} differs from the deterministic mutation")
        action = "verified"
    else:
        if output_path.exists() and output_path.read_bytes() != output:
            raise SystemExit(f"refusing to replace a different fixture at {output_path}")
        output_path.write_bytes(output)
        action = "wrote"
    print(f"{action} {output_path.relative_to(ROOT)} sha256={CASES[args.case]['sha256']}")


if __name__ == "__main__":
    main()
