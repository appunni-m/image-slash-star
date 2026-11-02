#!/usr/bin/env python3
"""Generate an AVIF sequence whose valid nclx overrides its AV1 CICP header."""

from __future__ import annotations

import argparse
import hashlib
import struct
from pathlib import Path

from generate_avif_identity_matrix_subsampled import (
    EXPECTED_ERROR,
    INVALID_CICP,
    SOURCE_CICP,
    SOURCE_PATH,
    SOURCE_SHA256,
    color_field_positions,
    nclx_offsets,
    sequence_headers,
    write_bits,
)
from inspect_av1_obus import parse_sequence_header


ROOT = Path(__file__).resolve().parent.parent
OUTPUT_PATH = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_sequence_header_cicp_mismatch_nclx.avif"
)
EXPECTED_SHA256 = "74a0a8f8725cc7b44232c4e970859be1ace37ad1898f80da38dd5cc98b7ed628"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def fixture_bytes(source: bytes) -> bytes:
    if sha256(source) != SOURCE_SHA256:
        raise ValueError("pinned AVIF differs from its source hash")
    headers = sequence_headers(source, SOURCE_PATH)
    if [sample for sample, _, _ in headers] != [0]:
        raise ValueError("pinned AVIF must signal one sequence header in sample zero")
    _, payload_start, payload = headers[0]
    primary_position, matrix_position, fields = color_field_positions(payload)
    if (
        fields["color_primaries"],
        fields["transfer_characteristics"],
        fields["matrix_coefficients"],
    ) != SOURCE_CICP:
        raise ValueError("pinned sequence header differs from its CICP precondition")

    output = bytearray(source)
    write_bits(output, payload_start * 8 + primary_position, 8, INVALID_CICP[0])
    write_bits(output, payload_start * 8 + matrix_position, 8, INVALID_CICP[2])
    result = bytes(output)
    if sha256(result) != EXPECTED_SHA256:
        raise ValueError("generated AVIF differs from its pinned bytes")
    return result


def verify_output(output: bytes, output_path: Path) -> None:
    headers = sequence_headers(output, output_path)
    if [sample for sample, _, _ in headers] != [0]:
        raise ValueError("generated AVIF must retain one sequence header in sample zero")
    _, _, fields = color_field_positions(headers[0][2])
    if (
        fields["color_primaries"],
        fields["transfer_characteristics"],
        fields["matrix_coefficients"],
    ) != INVALID_CICP or not fields["subsampling_x"] or not fields["subsampling_y"]:
        raise ValueError("generated sequence header does not retain its CICP mismatch")
    try:
        parse_sequence_header(headers[0][2])
    except ValueError as error:
        if str(error) != EXPECTED_ERROR:
            raise
    else:
        raise ValueError("generated AV1 sequence header did not retain the mismatch")
    for offset in nclx_offsets(output):
        if struct.unpack_from(">HHH", output, offset + 4) != SOURCE_CICP:
            raise ValueError("generated AVIF changed its valid nclx declaration")
    if sha256(output) != EXPECTED_SHA256:
        raise ValueError("generated AVIF differs from its pinned hash")


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
        if OUTPUT_PATH.exists() and OUTPUT_PATH.read_bytes() != output:
            raise SystemExit(f"refusing to replace a different fixture at {OUTPUT_PATH}")
        OUTPUT_PATH.write_bytes(output)
        action = "wrote"
    verify_output(output, OUTPUT_PATH)
    print(f"{action} {OUTPUT_PATH.relative_to(ROOT)} sha256={sha256(output)}")


if __name__ == "__main__":
    main()
