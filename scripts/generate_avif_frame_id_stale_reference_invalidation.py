#!/usr/bin/env python3
"""Generate a three-frame AVIF that invalidates stale frame-ID references."""

from __future__ import annotations

import argparse
import hashlib
import struct
from pathlib import Path

from inspect_av1_obus import inspect as inspect_obus
from inspect_avif_bitstreams import Box, children, inspect as inspect_avif, parse_boxes


ROOT = Path(__file__).resolve().parent.parent
INPUT_DIR = ROOT / "tests/fixtures/input/images/avif"
SOURCE_PATH = INPUT_DIR / "animated_error_resilient.avif"
OUTPUT_PATH = INPUT_DIR / "animated_frame_id_stale_reference_invalidation.avif"
SOURCE_SHA256 = "06ea9771f8b46c3432c6c6cdf324f1c05e86a5fdccd774c8e3c9a8fce0b831f0"
OUTPUT_SHA256 = "3b8ce1b8ddccf7b114f2ceac3505c254d73bc823d2138cae01106ce04029dd6d"
SOURCE_FRAME_IDS = (4626, 4627)
OUTPUT_FRAME_IDS = (0, 8000, 17000)
FRAME_ID_BITS = 15
DELTA_FRAME_ID_BITS = 14
REFERENCE_DELTA = 8000
MOOV_SIZE_GROWTH = 8

CONTAINER_PREFIXES = {
    b"meta": 4,
    b"moov": 0,
    b"trak": 0,
    b"edts": 0,
    b"mdia": 0,
    b"minf": 0,
    b"stbl": 0,
}


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


def source_fields(source: bytes) -> tuple[list[dict], list[tuple[dict, dict]]]:
    """Validate and locate both frame headers and the seven reference deltas."""
    if sha256(source) != SOURCE_SHA256:
        raise ValueError("error-resilient AVIF differs from its pinned source")

    container = inspect_avif(SOURCE_PATH)
    tracks = [track for track in container["tracks"] if track["handler"] == "pict"]
    if len(tracks) != 1 or len(tracks[0]["samples"]) != 2:
        raise ValueError("pinned AVIF must have one two-sample picture track")

    report = inspect_obus(SOURCE_PATH)
    samples = [sample for sample in report["samples"] if sample["role"] == "track_pict"]
    if len(samples) != 2:
        raise ValueError("pinned AV1 sequence must contain two picture samples")

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
        != (1, SOURCE_FRAME_IDS[0], SOURCE_FRAME_IDS[0])
        or item.get("delta_bits") != DELTA_FRAME_ID_BITS
        for item in references
    ):
        raise ValueError("source frame reference deltas differ from their pinned values")

    track_samples = tracks[0]["samples"]
    if [(sample["length"], sample["sync"], sample["duration"]) for sample in track_samples] != [
        (41, True, 100),
        (46, False, 100),
    ]:
        raise ValueError("source sample table differs from its pinned two-frame layout")
    return track_samples, frames


def patch_sample_frames(source: bytes) -> tuple[bytes, bytes]:
    """Set IDs to 0 and 8000, then clone the key frame as ID 17000."""
    track_samples, frames = source_fields(source)
    patched = bytearray(source)
    for (frame, header), frame_id in zip(frames, OUTPUT_FRAME_IDS[:2], strict=True):
        payload_start = frame["payload_spans"][0]["offset"] * 8
        write_bits(patched, payload_start + header["frame_id_bit"], FRAME_ID_BITS, frame_id)

    frame, _ = frames[1]
    references = frames[1][1]["reference_frame_ids"]
    for reference in references:
        write_bits(
            patched,
            frame["payload_spans"][0]["offset"] * 8 + reference["delta_bit"],
            DELTA_FRAME_ID_BITS,
            REFERENCE_DELTA - 1,
        )

    first_sample = track_samples[0]
    sample_start = first_sample["offset"]
    sample_end = sample_start + first_sample["length"]
    cloned_key_frame = bytearray(patched[sample_start:sample_end])
    frame, header = frames[0]
    local_frame_id_bit = (
        frame["payload_spans"][0]["offset"] - sample_start
    ) * 8 + header["frame_id_bit"]
    write_bits(
        cloned_key_frame,
        local_frame_id_bit,
        FRAME_ID_BITS,
        OUTPUT_FRAME_IDS[2],
    )
    return bytes(patched), bytes(cloned_key_frame)


def set_duration(payload: bytearray, kind: bytes) -> None:
    """Extend the pinned media and edit-list durations to three samples."""
    version = payload[0]
    if kind == b"mdhd":
        offset, width = (24, 8) if version == 1 else (16, 4)
    elif kind == b"elst":
        offset, width = (8, 8) if version == 1 else (8, 4)
    else:
        raise ValueError("unexpected duration box")
    old_duration = int.from_bytes(payload[offset : offset + width], "big")
    if version != 1 or old_duration != 200:
        raise ValueError(f"pinned {kind.decode()} duration differs from 200 ticks")
    payload[offset : offset + width] = (300).to_bytes(width, "big")


def patch_sample_table(box: Box, payload: bytearray, cloned_size: int) -> None:
    """Add one sync sample and extend the pinned one-chunk sample tables."""
    if box.kind == b"stsz":
        if (
            len(payload) != 20
            or int.from_bytes(payload[4:8], "big") != 0
            or int.from_bytes(payload[8:12], "big") != 2
        ):
            raise ValueError("pinned stsz table differs from two explicit sample sizes")
        payload[8:12] = (3).to_bytes(4, "big")
        payload.extend(cloned_size.to_bytes(4, "big"))
    elif box.kind == b"stsc":
        if (
            len(payload) != 20
            or int.from_bytes(payload[4:8], "big") != 1
            or int.from_bytes(payload[8:12], "big") != 1
            or int.from_bytes(payload[12:16], "big") != 2
            or int.from_bytes(payload[16:20], "big") != 1
        ):
            raise ValueError("pinned stsc table differs from its one two-sample chunk")
        payload[12:16] = (3).to_bytes(4, "big")
    elif box.kind == b"stts":
        if (
            len(payload) != 16
            or int.from_bytes(payload[4:8], "big") != 1
            or int.from_bytes(payload[8:12], "big") != 2
            or int.from_bytes(payload[12:16], "big") != 100
        ):
            raise ValueError("pinned stts table differs from two 100-tick samples")
        payload[8:12] = (3).to_bytes(4, "big")
    elif box.kind == b"stss":
        if (
            len(payload) != 12
            or int.from_bytes(payload[4:8], "big") != 1
            or int.from_bytes(payload[8:12], "big") != 1
        ):
            raise ValueError("pinned stss table differs from its first sync sample")
        payload[4:8] = (2).to_bytes(4, "big")
        payload.extend((3).to_bytes(4, "big"))
    elif box.kind == b"stco":
        if (
            len(payload) != 12
            or int.from_bytes(payload[4:8], "big") != 1
            or int.from_bytes(payload[8:12], "big") != 997
        ):
            raise ValueError("pinned stco table differs from its one media chunk")
        payload[8:12] = (997 + MOOV_SIZE_GROWTH).to_bytes(4, "big")
    elif box.kind in (b"mdhd", b"elst"):
        set_duration(payload, box.kind)
    elif box.kind == b"iloc":
        expected = "0000000044000001000100000001000003e500000029"
        if payload.hex() != expected:
            raise ValueError("pinned iloc layout differs from its single media extent")
        if int.from_bytes(payload[14:18], "big") != 997:
            raise ValueError("pinned iloc extent does not start at byte 997")
        payload[14:18] = (997 + MOOV_SIZE_GROWTH).to_bytes(4, "big")


def rewrite_box(data: bytes, box: Box, cloned_sample: bytes) -> bytes:
    """Rebuild known AVIF containers and update the fields that move with mdat."""
    if box.header_size != 8:
        raise ValueError("pinned AVIF uses an unexpected extended-size box")
    prefix_size = CONTAINER_PREFIXES.get(box.kind)
    if prefix_size is not None:
        prefix = data[box.payload_start : box.payload_start + prefix_size]
        nested = children(data, box, prefix_size)
        payload = prefix + b"".join(
            rewrite_box(data, child, cloned_sample) for child in nested
        )
    elif box.kind == b"mdat":
        payload = data[box.payload_start : box.end] + cloned_sample
    else:
        patched = bytearray(data[box.payload_start : box.end])
        patch_sample_table(box, patched, len(cloned_sample))
        payload = bytes(patched)
    return struct.pack(">I4s", box.header_size + len(payload), box.kind) + payload


def fixture_bytes(source: bytes) -> bytes:
    """Append a key frame after the stale-reference invalidation threshold."""
    patched_source, cloned_sample = patch_sample_frames(source)
    output = b"".join(
        rewrite_box(patched_source, box, cloned_sample)
        for box in parse_boxes(patched_source, 0, len(patched_source))
    )
    if sha256(output) != OUTPUT_SHA256:
        raise ValueError("generated stale-reference AVIF differs from its pinned bytes")
    return output


def verify_output() -> None:
    """Confirm the container and AV1 references encode the intended edge case."""
    container = inspect_avif(OUTPUT_PATH)
    tracks = [track for track in container["tracks"] if track["handler"] == "pict"]
    if len(tracks) != 1:
        raise ValueError("generated AVIF must have one picture track")
    samples = tracks[0]["samples"]
    if [(sample["sync"], sample["duration"]) for sample in samples] != [
        (True, 100),
        (False, 100),
        (True, 100),
    ]:
        raise ValueError("generated AVIF sample flags or durations differ from the pin")

    report = inspect_obus(OUTPUT_PATH)
    pictures = [sample for sample in report["samples"] if sample["role"] == "track_pict"]
    frames = [next(obu for obu in sample["obus"] if obu["type"] == 6) for sample in pictures]
    headers = [frame["frame_header"] for frame in frames]
    if [header.get("frame_id") for header in headers] != list(OUTPUT_FRAME_IDS):
        raise ValueError("generated AV1 frame IDs differ from their pinned values")
    if [header.get("refresh_frame_flags") for header in headers] != [255, 2, 255]:
        raise ValueError("generated AV1 reference refresh flags differ from their pin")
    references = headers[1].get("reference_frame_ids", [])
    if len(references) != 7 or any(
        (item.get("delta"), item.get("expected"), item.get("actual"))
        != (REFERENCE_DELTA, OUTPUT_FRAME_IDS[0], OUTPUT_FRAME_IDS[0])
        for item in references
    ):
        raise ValueError("generated inter frame does not reference frame ID zero")
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
