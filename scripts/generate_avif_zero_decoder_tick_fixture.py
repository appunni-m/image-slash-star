#!/usr/bin/env python3
"""Generate a pinned AVIF whose AV1 decoder-model timing tick is zero."""

import argparse
import hashlib
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/avif/animated.avif"
OUTPUT = ROOT / "tests/fixtures/input/images/avif/error_sequence_zero_decoder_tick.avif"
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
OUTPUT_SHA256 = "5287aa51ceac1e2128487a90caf3b5b12fcb30348a866ae700da42ccd4d08b49"


def generate() -> bytes:
    sys.path.insert(0, str(ROOT / "scripts"))
    from inspect_av1_obus import inspect

    source = SOURCE.read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise ValueError("animated AVIF source differs from its pinned fixture")

    report = inspect(SOURCE)
    sequence_obu = next(
        (
            obu
            for sample in report["samples"]
            for obu in sample["obus"]
            if obu.get("type") == 1
        ),
        None,
    )
    if sequence_obu is None:
        raise ValueError("animated AVIF has no AV1 sequence-header OBU")

    header = sequence_obu["sequence_header"]
    spans = sequence_obu["payload_spans"]
    if (
        header["reduced_still_picture_header"]
        or header["timing"] is not None
        or header["decoder_model_present"]
        or len(spans) != 1
        or spans[0]["length"] != 14
    ):
        raise ValueError("animated AVIF sequence-header layout changed")

    payload = bytearray(bytes.fromhex(header["payload_hex"]))
    span = spans[0]
    if source[span["offset"] : span["offset"] + span["length"]] != payload:
        raise ValueError("AV1 sequence-header payload offset changed")

    # Turn on timing and decoder-model information in the existing 14-byte
    # payload. The zero decoder tick is rejected before the remaining fields
    # are interpreted, while the complete AVIF container stays in place.
    fields = (
        (5, 1, 1),
        (6, 32, 1),
        (38, 32, 1),
        (70, 1, 0),
        (71, 1, 1),
        (72, 5, 0),
        (77, 32, 0),
    )
    for start, width, value in fields:
        if start + width > len(payload) * 8:
            raise ValueError("AV1 sequence-header mutation is out of range")
        for index in range(width):
            bit_position = start + index
            mask = 1 << (7 - bit_position % 8)
            replacement = (value >> (width - index - 1)) & 1
            byte_position = bit_position // 8
            payload[byte_position] = (payload[byte_position] & ~mask) | (replacement * mask)

    result = bytearray(source)
    result[span["offset"] : span["offset"] + span["length"]] = payload
    output = bytes(result)
    if hashlib.sha256(output).hexdigest() != OUTPUT_SHA256:
        raise ValueError("zero AV1 decoder-tick mutation differs from its pinned output")
    return output


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    expected = generate()
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_bytes() != expected:
            raise SystemExit("AVIF zero decoder-tick fixture is stale")
    else:
        if OUTPUT.exists() and OUTPUT.read_bytes() != expected:
            raise SystemExit(f"refusing to replace a different fixture at {OUTPUT}")
        OUTPUT.write_bytes(expected)
    print(f"AVIF zero decoder-tick fixture verified: {OUTPUT_SHA256}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
