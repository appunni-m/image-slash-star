#!/usr/bin/env python3
"""Generate complete grayscale source inputs; reference outputs stay producer-owned."""

from __future__ import annotations

import argparse
import io
from pathlib import Path

import yaml
from PIL import Image

from generate_decode_refs import encode_params, verify_primary_oracle


ROOT = Path(__file__).resolve().parents[1]
SIZES = ((17, 9), (128, 128), (512, 512))
SUBSAMPLING = (("default", None), ("444", "444"), ("422", "422"), ("420", "420"))


def cases():
    """Declare the full input and public save-option family."""
    result = []
    for width, height in SIZES:
        for label, subsampling in SUBSAMPLING:
            params = {"quality": 85, "progressive": False, "optimize": False}
            if subsampling is not None:
                params["subsampling"] = subsampling
            result.append({"name": f"grayscale_sampling_{label}_{width}x{height}",
                           "width": width, "height": height, "params": params,
                           "decode_input": subsampling in ("422", "420")})
    for label, additional in (("optimized", {"optimize": True}),
                              ("progressive", {"progressive": True}),
                              ("restart_rows_4", {"restart_interval": 4})):
        result.append({"name": f"grayscale_sampling_420_128x128_{label}",
                       "width": 128, "height": 128,
                       "params": {"quality": 85, "progressive": False,
                                  "optimize": False, "subsampling": "420", **additional},
                       "decode_input": True})
    return result


def source_pixels(width, height):
    """Use the maintained benchmark's input-only xorshift32 sequence."""
    pixels = bytearray(width * height)
    state = 0x12345678
    for index in range(len(pixels)):
        state ^= (state << 13) & 0xFFFFFFFF
        state ^= state >> 17
        state ^= (state << 5) & 0xFFFFFFFF
        pixels[index] = state & 255
    return bytes(pixels)


def late_table_cases(source):
    """Append changed tables after complete entropy to protect first-scan provenance."""
    if not source.startswith(b"\xff\xd8") or not source.endswith(b"\xff\xd9"):
        raise RuntimeError("complete Pillow JPEG required for late-table inputs")
    quant = None
    dc = None
    position = 2
    while source[position:position + 2] != b"\xff\xda":
        if source[position] != 255:
            raise RuntimeError("unexpected Pillow JPEG header boundary")
        length = int.from_bytes(source[position + 2:position + 4], "big")
        end = position + length + 2
        if length < 2 or end > len(source):
            raise RuntimeError("incomplete Pillow JPEG header segment")
        segment = source[position:end]
        if segment[:2] == b"\xff\xdb" and segment[4] == 0:
            quant = bytearray(segment)
        if segment[:2] == b"\xff\xc4" and segment[4] == 0:
            dc = bytearray(segment)
        position = end
    if quant is None or dc is None or len(quant) != 69 or len(dc) < 23:
        raise RuntimeError("Pillow grayscale table layout changed")
    # Eight-bit Q0: change its first quantizer without touching the prior table.
    quant[5] = quant[5] + 1 if quant[5] < 255 else 254
    # The DC table's values follow its selector and sixteen length counts.
    dc[21], dc[22] = dc[22], dc[21]
    return {
        "grayscale_sampling_420_17x9_late_changed_dqt": source[:-2] + bytes(quant) + source[-2:],
        "grayscale_sampling_420_17x9_late_changed_dht": source[:-2] + bytes(dc) + source[-2:],
    }


def generate(output, check):
    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    expected = {}
    for width, height in SIZES:
        with Image.frombytes("L", (width, height), source_pixels(width, height)) as image:
            buffer = io.BytesIO()
            image.save(buffer, format="PNG", compress_level=9)
            expected[Path("png") / f"jpeg_grayscale_sampling_source_{width}x{height}.png"] = buffer.getvalue()
    for case in cases():
        if not case["decode_input"]:
            continue
        with Image.frombytes("L", (case["width"], case["height"]),
                             source_pixels(case["width"], case["height"])) as image:
            buffer = io.BytesIO()
            image.save(buffer, format="JPEG", **encode_params("jpeg", dict(case["params"])))
            expected[Path("jpeg") / (case["name"] + ".jpg")] = buffer.getvalue()
    for name, complete_input in late_table_cases(expected[Path("jpeg/grayscale_sampling_420_17x9.jpg")]).items():
        expected[Path("jpeg") / (name + ".jpg")] = complete_input
    for relative, data in expected.items():
        destination = output / relative
        if check:
            if not destination.is_file() or destination.read_bytes() != data:
                raise RuntimeError("complete generated input differs: " + str(destination))
        else:
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
    print(f"{'Checked' if check else 'Generated'} {len(expected)} complete inputs for {len(cases())} encode and 11 decode declarations")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path,
                        default=ROOT / "tests/fixtures/input/images")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    generate(args.output.resolve(), args.check)


if __name__ == "__main__":
    main()
