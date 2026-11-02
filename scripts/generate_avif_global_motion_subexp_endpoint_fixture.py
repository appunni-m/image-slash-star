#!/usr/bin/env python3
"""Generate an AVIF parity fixture that reaches the long subexp endpoint."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

import inspect_av1_obus


ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "tests/fixtures/input/images/avif"
SOURCE_PATH = FIXTURES / "animated_lossy_global_rotzoom_compound_i444_b128x128.avif"
OUTPUT_PATH = FIXTURES / "animated_lossy_global_subexp_endpoint_i444_b128x128.avif"
SOURCE_SHA256 = "14ffb1529c54f07a397a1fe94d02135f9def3a7730416cb80d5acbf3c54c144e"
OUTPUT_SHA256 = "ff156ac8e1bfeb017b38846ef0dc4abca0312083523d317a7a892bde16ef3e97"
TARGET_ORDER_HINT = 2
LONG_ENDPOINT_CODE = "1" * 10 + "1" * 12 + "0"
SHORT_NEAR_REFERENCE_CODE = "1100000"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def target_frame(path: Path) -> tuple[dict, dict, dict]:
    report = inspect_av1_obus.inspect(path)
    matches = [
        (sample, obu, obu["frame_header"])
        for sample in report["samples"]
        for obu in sample["obus"]
        if obu.get("frame_header", {}).get("order_hint") == TARGET_ORDER_HINT
    ]
    if len(matches) != 1:
        raise ValueError("pinned sequence must contain one frame with order hint 2")
    sample, obu, header = matches[0]
    if (
        header.get("frame_type") != "inter"
        or not any(motion["type"] == "rotzoom" for motion in header["global_motion"])
    ):
        raise ValueError("target frame no longer carries a RotZoom global-motion model")
    if len(obu.get("payload_spans", [])) != 1:
        raise ValueError("target frame OBU payload must remain contiguous")
    return sample, obu, header


def subexp_calls(path: Path, payload_sha256: str) -> list[tuple[int, int, int, int, int]]:
    calls: list[tuple[int, int, int, int, int]] = []
    original = inspect_av1_obus.Bits.subexp

    def record(bits, reference: int, width: int) -> int:
        start = bits.position
        value = original(bits, reference, width)
        if sha256(bits.data) == payload_sha256:
            calls.append((start, bits.position, reference, width, value))
        return value

    inspect_av1_obus.Bits.subexp = record
    try:
        inspect_av1_obus.inspect(path)
    finally:
        inspect_av1_obus.Bits.subexp = original
    return calls


def write_bits(data: bytearray, start: int, value: str) -> None:
    for index, bit in enumerate(value):
        position = start + index
        mask = 1 << (7 - position % 8)
        if bit == "1":
            data[position // 8] |= mask
        else:
            data[position // 8] &= ~mask


def generate() -> bytes:
    source = SOURCE_PATH.read_bytes()
    if sha256(source) != SOURCE_SHA256:
        raise ValueError("pinned global-motion source fixture changed")
    _, obu, header = target_frame(SOURCE_PATH)
    source_calls = subexp_calls(SOURCE_PATH, obu["payload_sha256"])
    if len(source_calls) < 2 or source_calls[:2] != [
        (102, 111, 0, 12, -18),
        (111, 132, 0, 12, 1147),
    ]:
        raise ValueError("pinned global-motion subexp parameter layout changed")

    replacement = LONG_ENDPOINT_CODE + SHORT_NEAR_REFERENCE_CODE
    if len(replacement) != source_calls[1][1] - source_calls[0][0]:
        raise ValueError("replacement subexp codes must preserve header length")
    output = bytearray(source)
    payload_start = obu["payload_spans"][0]["offset"] * 8
    write_bits(output, payload_start + source_calls[0][0], replacement)
    return bytes(output)


def validate_output() -> None:
    _, obu, header = target_frame(OUTPUT_PATH)
    calls = subexp_calls(OUTPUT_PATH, obu["payload_sha256"])
    if not calls or calls[:2] != [
        (102, 125, 0, 12, -4096),
        (125, 132, 0, 12, 8),
    ]:
        raise ValueError("fixture does not reach the terminal subexp/ns branch")
    if header["header_bits"] != 166 or obu["tile_group"]["data_bit"] != 168:
        raise ValueError("fixture mutation changed the frame-header or tile boundary")
    motion = next(value for value in header["global_motion"] if value["type"] == "rotzoom")
    if motion["matrix"] != [-354304, 328704, 57344, 16, -16, 57344]:
        raise ValueError("fixture RotZoom matrix differs from the pinned mutation")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify the pinned fixture")
    args = parser.parse_args()
    generated = generate()
    if args.check:
        if not OUTPUT_PATH.is_file() or OUTPUT_PATH.read_bytes() != generated:
            raise ValueError("global-motion subexp endpoint fixture differs from its generator")
    else:
        OUTPUT_PATH.write_bytes(generated)
    digest = sha256(OUTPUT_PATH.read_bytes())
    if digest != OUTPUT_SHA256:
        raise ValueError(f"global-motion subexp fixture hash differs: {digest}")
    validate_output()
    print(f"AVIF global-motion subexp endpoint fixture OK: {digest}")


if __name__ == "__main__":
    main()
