#!/usr/bin/env python3
"""Generate a sequence that invalidates future-ID reference slots after wrap."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from inspect_av1_obus import inspect as inspect_obus
from inspect_avif_bitstreams import inspect as inspect_avif


ROOT = Path(__file__).resolve().parent.parent
INPUT_DIR = ROOT / "tests/fixtures/input/images/avif"
SOURCE_PATH = INPUT_DIR / "animated_frame_id_stale_reference_invalidation.avif"
OUTPUT_PATH = INPUT_DIR / "animated_frame_id_future_reference_invalidation.avif"
SOURCE_SHA256 = "3b8ce1b8ddccf7b114f2ceac3505c254d73bc823d2138cae01106ce04029dd6d"
OUTPUT_SHA256 = "ebe40d4c20e39723979ac8fc5968c846b54f30045739af26d8d91974d7492cbb"
SOURCE_FRAME_IDS = (0, 8000, 17000)
OUTPUT_FRAME_IDS = (30000, 10000, 20000)
FRAME_ID_BITS = 15
DELTA_FRAME_ID_BITS = 14
SOURCE_REFERENCE_DELTA = 8000
OUTPUT_REFERENCE_DELTA = 12768


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_bits(data: bytearray, start: int, width: int, value: int) -> None:
    """Replace one bounded big-endian AV1 syntax field."""
    if start < 0 or width <= 0 or start + width > len(data) * 8:
        raise ValueError("AV1 syntax field is outside the pinned sample")
    if value < 0 or value >= 1 << width:
        raise ValueError("replacement AV1 syntax value does not fit its width")
    for index in range(width):
        position = start + index
        mask = 1 << (7 - position % 8)
        bit = (value >> (width - index - 1)) & 1
        data[position // 8] = (data[position // 8] & ~mask) | (bit * mask)


def frame_fields(source: bytes) -> list[tuple[dict, dict]]:
    """Validate the three-frame source and locate every ID and reference delta."""
    if sha256(source) != SOURCE_SHA256:
        raise ValueError("stale-reference AVIF differs from its pinned source")

    report = inspect_obus(SOURCE_PATH)
    samples = [sample for sample in report["samples"] if sample["role"] == "track_pict"]
    if len(samples) != 3:
        raise ValueError("pinned AV1 sequence must contain three picture samples")

    frames: list[tuple[dict, dict]] = []
    frame_ids = []
    sequence = None
    for sample in samples:
        sequence = sequence or next(
            (
                obu["sequence_header"]
                for obu in sample["obus"]
                if obu["type"] == 1
            ),
            None,
        )
        obus = [obu for obu in sample["obus"] if obu["type"] == 6]
        if len(obus) != 1 or len(obus[0]["payload_spans"]) != 1:
            raise ValueError("each AV1 frame header must have one contiguous span")
        frame = obus[0]
        header = frame["frame_header"]
        frame_ids.append(header.get("frame_id"))
        frames.append((frame, header))

    if tuple(frame_ids) != SOURCE_FRAME_IDS:
        raise ValueError("source AV1 frame IDs differ from their pinned values")
    if sequence is None or (
        sequence.get("frame_id_bits"),
        sequence.get("delta_frame_id_bits"),
    ) != (FRAME_ID_BITS, DELTA_FRAME_ID_BITS):
        raise ValueError("source AV1 frame-ID widths differ from their pinned values")

    references = frames[1][1].get("reference_frame_ids", [])
    if len(references) != 7 or any(
        (item.get("delta"), item.get("expected"), item.get("actual"))
        != (SOURCE_REFERENCE_DELTA, SOURCE_FRAME_IDS[0], SOURCE_FRAME_IDS[0])
        or item.get("delta_bits") != DELTA_FRAME_ID_BITS
        for item in references
    ):
        raise ValueError("source inter frame references differ from their pinned IDs")
    return frames


def fixture_bytes(source: bytes) -> bytes:
    """Wrap the first-to-middle IDs while preserving the decoded sample data."""
    frames = frame_fields(source)
    output = bytearray(source)
    changed_fields = []
    for (frame, header), frame_id in zip(frames, OUTPUT_FRAME_IDS, strict=True):
        start = frame["payload_spans"][0]["offset"] * 8 + header["frame_id_bit"]
        write_bits(output, start, FRAME_ID_BITS, frame_id)
        changed_fields.append((start, FRAME_ID_BITS))

    frame, header = frames[1]
    references = header["reference_frame_ids"]
    for reference in references:
        start = frame["payload_spans"][0]["offset"] * 8 + reference["delta_bit"]
        write_bits(
            output,
            start,
            DELTA_FRAME_ID_BITS,
            OUTPUT_REFERENCE_DELTA - 1,
        )
        changed_fields.append((start, DELTA_FRAME_ID_BITS))

    changed_bytes = {
        index
        for index, (before, after) in enumerate(zip(source, output, strict=True))
        if before != after
    }
    allowed_bytes = {
        position // 8
        for start, width in changed_fields
        for position in range(start, start + width)
    }
    if not changed_bytes or not changed_bytes <= allowed_bytes:
        raise ValueError("fixture mutation changed bytes outside frame-ID syntax fields")

    result = bytes(output)
    if sha256(result) != OUTPUT_SHA256:
        raise ValueError("generated future-reference AVIF differs from its pinned bytes")
    return result


def verify_output() -> None:
    """Confirm the wrapped sequence retains a stale high-ID slot before frame 3."""
    container = inspect_avif(OUTPUT_PATH)
    tracks = [track for track in container["tracks"] if track["handler"] == "pict"]
    if len(tracks) != 1 or len(tracks[0]["samples"]) != 3:
        raise ValueError("generated AVIF must have one three-sample picture track")
    if [(sample["sync"], sample["duration"]) for sample in tracks[0]["samples"]] != [
        (True, 100),
        (False, 100),
        (True, 100),
    ]:
        raise ValueError("generated AVIF sample flags or durations differ from their pin")

    report = inspect_obus(OUTPUT_PATH)
    samples = [sample for sample in report["samples"] if sample["role"] == "track_pict"]
    frames = [next(obu for obu in sample["obus"] if obu["type"] == 6) for sample in samples]
    headers = [frame["frame_header"] for frame in frames]
    if [header.get("frame_id") for header in headers] != list(OUTPUT_FRAME_IDS):
        raise ValueError("generated AV1 frame IDs differ from their pinned values")
    if [header.get("refresh_frame_flags") for header in headers] != [255, 2, 255]:
        raise ValueError("generated AV1 reference refresh flags differ from their pin")
    references = headers[1].get("reference_frame_ids", [])
    if len(references) != 7 or any(
        (item.get("delta"), item.get("expected"), item.get("actual"))
        != (OUTPUT_REFERENCE_DELTA, OUTPUT_FRAME_IDS[0], OUTPUT_FRAME_IDS[0])
        for item in references
    ):
        raise ValueError("generated inter frame does not reference frame ID 30000")
    if headers[2].get("frame_type") != "key":
        raise ValueError("the post-invalidation frame must decode independently")


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
