#!/usr/bin/env python3
"""Generate an AVIS fixture whose show-existing frame names an empty slot."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from inspect_avif_bitstreams import children, parse_boxes, parse_track, unique_box

ROOT = Path(__file__).resolve().parent.parent
SOURCE_PATH = ROOT / "tests/fixtures/input/images/avif/animated.avif"
OUTPUT_PATH = (
    ROOT
    / "tests/fixtures/input/images/avif/error_animated_show_existing_empty_reference.avif"
)
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
OUTPUT_SHA256 = "4edf1ad3aca5683da9b01569d76381185f6dc057e089337466085f044e786f7c"
SHOW_EXISTING_SAMPLE_INDEX = 2
SOURCE_REFERENCE_SLOT = 2
EMPTY_REFERENCE_SLOT = 7
SOURCE_SHOW_EXISTING_SAMPLE = bytes.fromhex("12001a01a8")
INITIAL_SEQUENCE_HEADER = bytes.fromhex("12000a0e00000003bcaca9b5f22021a0d080")
EMPTY_REFERENCE_FRAME = bytes.fromhex("12000a0e00000003bcaca9b5f22021a0d0801a01f8")


def fixture_bytes(source: bytes) -> bytes:
    """Replace the first key frame with a show-existing frame for empty slot 7."""
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise ValueError("animated AVIF source differs from its pinned input")

    top_level = parse_boxes(source, 0, len(source))
    movie = unique_box(top_level, b"moov")
    if movie is None:
        raise ValueError("animated AVIF source has no movie box")
    tracks = [box for box in children(source, movie) if box.kind == b"trak"]
    if len(tracks) != 1:
        raise ValueError("animated AVIF must contain exactly one movie track")

    track = parse_track(source, tracks[0])
    samples = track["samples"]
    if track["handler"] != "pict" or len(samples) <= SHOW_EXISTING_SAMPLE_INDEX:
        raise ValueError("animated AVIF color track no longer has the pinned samples")
    first_sample = samples[0]
    show_existing_sample = samples[SHOW_EXISTING_SAMPLE_INDEX]
    first_start = first_sample["offset"]
    first_end = first_start + first_sample["length"]
    show_start = show_existing_sample["offset"]
    show_end = show_start + show_existing_sample["length"]
    if not source[first_start:first_end].startswith(INITIAL_SEQUENCE_HEADER):
        raise ValueError("initial AV1 sample no longer has its pinned sequence header")
    if source[show_start:show_end] != SOURCE_SHOW_EXISTING_SAMPLE:
        raise ValueError("show-existing AV1 sample differs from its pinned payload")
    payload_byte = SOURCE_SHOW_EXISTING_SAMPLE[-1]
    if payload_byte >> 7 != 1 or (payload_byte >> 4) & 0x07 != SOURCE_REFERENCE_SLOT:
        raise ValueError("show-existing syntax differs from its pinned reference slot")
    target_payload_byte = EMPTY_REFERENCE_FRAME[-1]
    if target_payload_byte >> 7 != 1 or (target_payload_byte >> 4) & 0x07 != EMPTY_REFERENCE_SLOT:
        raise ValueError("generated show-existing syntax does not select the empty slot")

    if len(EMPTY_REFERENCE_FRAME) > first_sample["length"]:
        raise ValueError("empty-reference frame exceeds the first sample extent")
    replacement = EMPTY_REFERENCE_FRAME + bytes(
        first_sample["length"] - len(EMPTY_REFERENCE_FRAME)
    )
    output = bytearray(source)
    output[first_start:first_end] = replacement
    result = bytes(output)
    changed = [
        index
        for index, (before, after) in enumerate(zip(source, result))
        if before != after
    ]
    if any(not first_start <= index < first_end for index in changed):
        raise ValueError("empty-reference mutation changed bytes outside the first sample")
    if hashlib.sha256(result).hexdigest() != OUTPUT_SHA256:
        raise ValueError("empty-reference AVIF output differs from its pinned mutation")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    result = fixture_bytes(SOURCE_PATH.read_bytes())
    result_hash = hashlib.sha256(result).hexdigest()
    if result_hash != OUTPUT_SHA256:
        raise SystemExit("generated show-existing AVIF hash differs from its pin")
    if args.check:
        if not OUTPUT_PATH.exists() or OUTPUT_PATH.read_bytes() != result:
            raise SystemExit(f"{OUTPUT_PATH} differs from its deterministic mutation")
        action = "verified"
    else:
        OUTPUT_PATH.write_bytes(result)
        action = "wrote"
    print(
        f"{action} {OUTPUT_PATH.relative_to(ROOT)} "
        f"sha256={result_hash}"
    )


if __name__ == "__main__":
    main()
