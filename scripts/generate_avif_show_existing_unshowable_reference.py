#!/usr/bin/env python3
"""Generate an AVIS fixture with a show-existing reference to an unshowable slot."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from inspect_avif_bitstreams import children, parse_boxes, parse_track, unique_box


ROOT = Path(__file__).resolve().parent.parent
SOURCE_PATH = ROOT / "tests/fixtures/input/images/avif/animated.avif"
OUTPUT_PATH = (
    ROOT
    / "tests/fixtures/input/images/avif/error_animated_show_existing_unshowable_reference.avif"
)
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
OUTPUT_SHA256 = "bf8daa2a92e16a7c9b12cd5ef6d724b36c30e724d5f8a2952480387ff20ecf9d"
SHOW_EXISTING_SAMPLE_INDEX = 2
SOURCE_REFERENCE_SLOT = 2
UNSHOWABLE_KEY_SLOT = 0
SHOW_EXISTING_SAMPLE = bytes.fromhex("12001a01a8")


def fixture_bytes(source: bytes) -> bytes:
    """Change only the existing-frame slot in the pinned AVIS sample."""
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise ValueError("animated AVIF source differs from its pinned input")

    top_level = parse_boxes(source, 0, len(source))
    movie = unique_box(top_level, b"moov")
    assert movie is not None
    tracks = [box for box in children(source, movie) if box.kind == b"trak"]
    if len(tracks) != 1:
        raise ValueError("animated AVIF must contain exactly one movie track")

    track = parse_track(source, tracks[0])
    samples = track["samples"]
    if track["handler"] != "pict" or len(samples) <= SHOW_EXISTING_SAMPLE_INDEX:
        raise ValueError("animated AVIF color track no longer has the pinned samples")

    sample = samples[SHOW_EXISTING_SAMPLE_INDEX]
    sample_offset = sample["offset"]
    sample_length = sample["length"]
    if sample_length != len(SHOW_EXISTING_SAMPLE):
        raise ValueError("show-existing AV1 sample length differs from its pin")
    if source[sample_offset : sample_offset + sample_length] != SHOW_EXISTING_SAMPLE:
        raise ValueError("show-existing AV1 sample differs from its pinned payload")

    payload_offset = sample_offset + 4
    payload_byte = source[payload_offset]
    show_existing = payload_byte >> 7
    reference_slot = (payload_byte >> 4) & 0x07
    if show_existing != 1 or reference_slot != SOURCE_REFERENCE_SLOT:
        raise ValueError("show-existing syntax differs from its pinned reference slot")

    output = bytearray(source)
    output[payload_offset] = (payload_byte & 0x8F) | (UNSHOWABLE_KEY_SLOT << 4)
    result = bytes(output)
    changed = [
        index
        for index, (before, after) in enumerate(zip(source, result))
        if before != after
    ]
    if changed != [payload_offset]:
        raise ValueError("show-existing mutation changed bytes outside its slot field")
    if len(result) != len(source) or hashlib.sha256(result).hexdigest() != OUTPUT_SHA256:
        raise ValueError("show-existing AVIF output differs from its pinned mutation")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    result = fixture_bytes(SOURCE_PATH.read_bytes())
    if args.check:
        if not OUTPUT_PATH.exists() or OUTPUT_PATH.read_bytes() != result:
            raise SystemExit(f"{OUTPUT_PATH} differs from its deterministic mutation")
        action = "verified"
    else:
        OUTPUT_PATH.write_bytes(result)
        action = "wrote"
    print(f"{action} {OUTPUT_PATH.relative_to(ROOT)} sha256={OUTPUT_SHA256}")


if __name__ == "__main__":
    main()
