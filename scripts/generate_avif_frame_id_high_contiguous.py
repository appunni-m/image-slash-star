#!/usr/bin/env python3
"""Generate a valid AVIF sequence above its delta-frame-ID range."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parent.parent
INPUT_DIR = ROOT / "tests/fixtures/input/images/avif"
SOURCE_PATH = INPUT_DIR / "animated_error_resilient.avif"
OUTPUT_PATH = INPUT_DIR / "animated_frame_id_high_contiguous.avif"
SOURCE_SHA256 = "06ea9771f8b46c3432c6c6cdf324f1c05e86a5fdccd774c8e3c9a8fce0b831f0"
OUTPUT_SHA256 = "25c5e52ca0759816ce94f4aa5d2be42ed99353a004da85e1cf80e8006925eecf"
SOURCE_FRAME_IDS = (4626, 4627)
OUTPUT_FRAME_IDS = (20000, 20001)
FRAME_ID_BITS = 15
DELTA_FRAME_ID_BITS = 14


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_bits(data: bytearray, start: int, width: int, value: int) -> None:
    """Replace one bounded big-endian AV1 syntax field."""
    if start < 0 or width <= 0 or start + width > len(data) * 8:
        raise ValueError("frame-ID field is outside the pinned AVIF")
    if value < 0 or value >= 1 << width:
        raise ValueError("replacement frame ID does not fit the encoded field")
    for index in range(width):
        position = start + index
        mask = 1 << (7 - position % 8)
        bit = (value >> (width - index - 1)) & 1
        data[position // 8] = (data[position // 8] & ~mask) | (bit * mask)


def frame_id_fields(path: Path) -> tuple[list[int], list[int], int, int]:
    """Locate and validate both frame-ID fields in the pinned two-frame track."""
    report = inspect(path)
    samples = [sample for sample in report["samples"] if sample["role"] == "track_pict"]
    if len(samples) != 2:
        raise ValueError("pinned error-resilient sequence must contain two picture samples")

    frame_ids: list[int] = []
    starts: list[int] = []
    frame_id_bits = None
    delta_frame_id_bits = None
    for sample in samples:
        frame_obus = [obu for obu in sample["obus"] if obu["type"] == 6]
        if len(frame_obus) != 1 or len(frame_obus[0]["payload_spans"]) != 1:
            raise ValueError("frame header must have one contiguous payload span")
        frame = frame_obus[0]
        header = frame["frame_header"]
        sequence = next(
            (
                obu["sequence_header"]
                for obu in sample["obus"]
                if obu["type"] == 1
            ),
            None,
        )
        if sequence is None:
            sequence = next(
                (
                    obu["sequence_header"]
                    for earlier in samples
                    for obu in earlier["obus"]
                    if obu["type"] == 1
                ),
                None,
            )
        if sequence is None:
            raise ValueError("pinned sequence has no AV1 sequence header")
        width = sequence.get("frame_id_bits")
        delta_width = sequence.get("delta_frame_id_bits")
        if frame_id_bits is None:
            frame_id_bits = width
            delta_frame_id_bits = delta_width
        if (width, delta_width) != (frame_id_bits, delta_frame_id_bits):
            raise ValueError("sequence header frame-ID widths changed between samples")
        frame_ids.append(header.get("frame_id"))
        starts.append(
            frame["payload_spans"][0]["offset"] * 8 + header["frame_id_bit"]
        )

    if (frame_id_bits, delta_frame_id_bits) != (FRAME_ID_BITS, DELTA_FRAME_ID_BITS):
        raise ValueError("pinned frame-ID widths differ from the expected AV1 sequence")
    return frame_ids, starts, frame_id_bits, delta_frame_id_bits


def fixture_bytes(source: bytes) -> bytes:
    """Raise both contiguous frame IDs without changing AV1 reference deltas."""
    if sha256(source) != SOURCE_SHA256:
        raise ValueError("error-resilient AVIF differs from its pinned source")
    source_ids, starts, width, delta_width = frame_id_fields(SOURCE_PATH)
    if tuple(source_ids) != SOURCE_FRAME_IDS:
        raise ValueError("error-resilient frame IDs differ from their pinned values")
    if OUTPUT_FRAME_IDS[1] - OUTPUT_FRAME_IDS[0] != SOURCE_FRAME_IDS[1] - SOURCE_FRAME_IDS[0]:
        raise ValueError("replacement frame IDs must preserve the reference distance")
    if OUTPUT_FRAME_IDS[0] < 1 << delta_width:
        raise ValueError("replacement frame IDs must enter the high-ID range")

    output = bytearray(source)
    for start, frame_id in zip(starts, OUTPUT_FRAME_IDS, strict=True):
        write_bits(output, start, width, frame_id)
    changed = {
        index
        for index, (before, after) in enumerate(zip(source, output, strict=True))
        if before != after
    }
    allowed = {
        position // 8
        for start in starts
        for position in range(start, start + width)
    }
    if not changed or not changed <= allowed:
        raise ValueError("fixture mutation changed bytes outside the frame-ID fields")

    result = bytes(output)
    if sha256(result) != OUTPUT_SHA256:
        raise ValueError("generated high-frame-ID fixture differs from its pinned bytes")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    output = fixture_bytes(SOURCE_PATH.read_bytes())
    frame_ids, _, _, _ = frame_id_fields(SOURCE_PATH)
    if tuple(frame_ids) != SOURCE_FRAME_IDS:
        raise SystemExit("source frame IDs differ from their pinned values")
    if args.check:
        if not OUTPUT_PATH.exists() or OUTPUT_PATH.read_bytes() != output:
            raise SystemExit(f"{OUTPUT_PATH} differs from the deterministic mutation")
        action = "verified"
    else:
        OUTPUT_PATH.write_bytes(output)
        action = "wrote"
    print(f"{action} {OUTPUT_PATH.relative_to(ROOT)} sha256={OUTPUT_SHA256}")


if __name__ == "__main__":
    main()
