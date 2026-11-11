#!/usr/bin/env python3
"""Generate an AVIF whose missing sequence-header marker Pillow accepts."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from generate_avif_sequence_header_errors import (
    INPUT_PATH,
    INPUT_SHA256,
    sequence_obus,
)
from inspect_av1_obus import parse_sequence_header
from inspect_avif_bitstreams import inspect as inspect_avif


ROOT = Path(__file__).resolve().parent.parent
OUTPUT_PATH = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_sequence_header_missing_trailing_marker.avif"
)
OUTPUT_SHA256 = "f916d9d4a310dc2f808e3465e8ae075e70e45a72be1ee84d2c2715fe6f76e2fd"
EXPECTED_SEQUENCE_SAMPLES = [0, 1]
MISSING_MARKER_ERROR = "sequence header lacks trailing one bit"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def marker_bit_position(payload: bytes) -> int:
    """Find the final one bit before AV1 sequence-header zero padding."""
    if not payload:
        raise ValueError("pinned sequence header is empty")
    final_byte = payload[-1]
    if final_byte == 0:
        raise ValueError("pinned sequence header has no trailing marker bit")
    least_significant_one = (final_byte & -final_byte).bit_length() - 1
    return (len(payload) - 1) * 8 + 7 - least_significant_one


def fixture_bytes(source: bytes) -> bytes:
    if sha256(source) != INPUT_SHA256:
        raise ValueError("AVIF differs from its pinned sequence-header source")

    headers = sequence_obus(source, inspect_avif(INPUT_PATH))
    if [sample for sample, _, _ in headers] != EXPECTED_SEQUENCE_SAMPLES:
        raise ValueError("pinned AVIF sequence-header sample layout changed")
    for _, _, payload in headers:
        parse_sequence_header(payload)

    payload_start = headers[0][1]
    marker_position = marker_bit_position(headers[0][2])
    absolute_position = payload_start * 8 + marker_position
    output = bytearray(source)
    output[absolute_position // 8] &= ~(1 << (7 - absolute_position % 8))
    result = bytes(output)
    if sha256(result) != OUTPUT_SHA256:
        raise ValueError("generated AVIF differs from its pinned bytes")
    return result


def verify_output(data: bytes, output_path: Path) -> None:
    headers = sequence_obus(data, inspect_avif(output_path))
    if [sample for sample, _, _ in headers] != EXPECTED_SEQUENCE_SAMPLES:
        raise ValueError("generated AVIF sequence-header sample layout changed")
    try:
        parse_sequence_header(headers[0][2])
    except ValueError as error:
        if str(error) != MISSING_MARKER_ERROR:
            raise
    else:
        raise ValueError("generated sequence header still has its marker bit")
    parse_sequence_header(headers[1][2])


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
    verify_output(output, OUTPUT_PATH)
    print(f"{action} {OUTPUT_PATH.relative_to(ROOT)} sha256={OUTPUT_SHA256}")


if __name__ == "__main__":
    main()
