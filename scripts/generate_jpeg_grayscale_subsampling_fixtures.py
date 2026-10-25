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


def add_restart_interval_before_sos(source, interval):
    """Set a DRI interval without changing the scan's entropy-coded bytes."""
    if not source.startswith(b"\xff\xd8") or not source.endswith(b"\xff\xd9"):
        raise RuntimeError("complete Pillow JPEG required for restart-interval input")
    if not 0 < interval <= 0xFFFF:
        raise ValueError("JPEG restart interval must fit a nonzero 16-bit value")

    position = 2
    while position < len(source):
        marker_start = position
        if source[position] != 0xFF:
            raise RuntimeError("unexpected JPEG marker alignment before SOS")
        while position < len(source) and source[position] == 0xFF:
            position += 1
        if position >= len(source):
            raise RuntimeError("truncated JPEG marker before SOS")
        marker = source[position]
        position += 1
        if marker == 0xDA:
            dri = b"\xff\xdd\x00\x04" + interval.to_bytes(2, "big")
            return source[:marker_start] + dri + source[marker_start:]
        if marker in (0xD8, 0xD9, *range(0xD0, 0xD8)):
            raise RuntimeError("unexpected standalone marker before SOS")
        if marker == 0xDD:
            raise RuntimeError("Pillow source JPEG already contains a DRI marker")
        if position + 2 > len(source):
            raise RuntimeError("truncated JPEG segment length before SOS")
        segment_length = int.from_bytes(source[position : position + 2], "big")
        if segment_length < 2 or position + segment_length > len(source):
            raise RuntimeError("invalid JPEG segment length before SOS")
        position += segment_length
    raise RuntimeError("Pillow JPEG has no SOS marker")


def add_restart_interval_before_eoi(source, interval):
    """Change the next-scan DRI state after a complete JPEG scan."""
    if not source.startswith(b"\xff\xd8") or not source.endswith(b"\xff\xd9"):
        raise RuntimeError("complete Pillow JPEG required for trailing DRI input")
    if not 0 <= interval <= 0xFFFF:
        raise ValueError("JPEG restart interval must fit a 16-bit value")
    dri = b"\xff\xdd\x00\x04" + interval.to_bytes(2, "big")
    return source[:-2] + dri + source[-2:]


def set_baseline_sos_field(source, field_name, expected, value, maximum):
    """Change one grayscale SOS byte without changing the entropy-coded scan."""
    if not source.startswith(b"\xff\xd8") or not source.endswith(b"\xff\xd9"):
        raise RuntimeError("complete Pillow JPEG required for a changed-SOS input")
    scan_start = source.find(b"\xff\xda")
    if scan_start < 0 or source.find(b"\xff\xda", scan_start + 2) >= 0:
        raise RuntimeError("changed-SOS input must have exactly one SOS")

    segment_length = int.from_bytes(source[scan_start + 2:scan_start + 4], "big")
    payload_start = scan_start + 4
    if segment_length < 8 or payload_start + segment_length - 2 > len(source):
        raise RuntimeError("baseline SOS header is incomplete")
    if source[payload_start] != 1:
        raise RuntimeError("changed-SOS input must have one grayscale scan component")
    field_offsets = {
        "spectral_start": payload_start + 3,
        "spectral_end": payload_start + 4,
        "successive_approximation": payload_start + 5,
    }
    offset = field_offsets.get(field_name)
    if offset is None:
        raise ValueError("unsupported baseline SOS field: " + field_name)
    if source[offset] != expected:
        raise RuntimeError("Pillow baseline source has an unexpected SOS " + field_name)
    if not 0 <= value <= maximum:
        raise ValueError("baseline SOS " + field_name + " is outside its byte-field range")

    candidate = bytearray(source)
    candidate[offset] = value
    return bytes(candidate)


def set_baseline_spectral_start(source, spectral_start):
    """Set the single baseline scan's Ss field without changing entropy bytes."""
    return set_baseline_sos_field(source, "spectral_start", 0, spectral_start, 63)


def set_baseline_spectral_end(source, spectral_end):
    """Set the single baseline scan's Se field without changing entropy bytes."""
    return set_baseline_sos_field(source, "spectral_end", 63, spectral_end, 63)


def set_baseline_successive_approximation(source, ah, al):
    """Set the single baseline scan's Ah/Al nibbles without changing entropy bytes."""
    if not 0 <= ah <= 15 or not 0 <= al <= 15:
        raise ValueError("baseline SOS Ah and Al values must fit four-bit fields")
    return set_baseline_sos_field(
        source, "successive_approximation", 0, (ah << 4) | al, 255
    )


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
    grayscale_422 = expected[Path("jpeg/grayscale_sampling_422_17x9.jpg")]
    scan_restart_7 = add_restart_interval_before_sos(grayscale_422, 7)
    expected[
        Path("jpeg/grayscale_sampling_422_17x9_restart_interval_snapshot_7_to_0.jpg")
    ] = add_restart_interval_before_eoi(scan_restart_7, 0)
    for name, complete_input in late_table_cases(expected[Path("jpeg/grayscale_sampling_420_17x9.jpg")]).items():
        expected[Path("jpeg") / (name + ".jpg")] = complete_input
    expected[Path("jpeg/grayscale_sampling_420_17x9_sos_ss_nonzero.jpg")] = (
        set_baseline_spectral_start(
            expected[Path("jpeg/grayscale_sampling_420_17x9.jpg")], 1
        )
    )
    grayscale_420 = expected[Path("jpeg/grayscale_sampling_420_17x9.jpg")]
    expected[Path("jpeg/grayscale_sampling_420_17x9_sos_se_nonmax.jpg")] = (
        set_baseline_spectral_end(grayscale_420, 62)
    )
    expected[Path("jpeg/grayscale_sampling_420_17x9_sos_ah_nonzero.jpg")] = (
        set_baseline_successive_approximation(grayscale_420, 1, 0)
    )
    expected[Path("jpeg/grayscale_sampling_420_17x9_sos_al_nonzero.jpg")] = (
        set_baseline_successive_approximation(grayscale_420, 0, 1)
    )
    for relative, data in expected.items():
        destination = output / relative
        if check:
            if not destination.is_file() or destination.read_bytes() != data:
                raise RuntimeError("complete generated input differs: " + str(destination))
        else:
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
    decode_declarations = sum(case["decode_input"] for case in cases()) + 1
    print(
        f"{'Checked' if check else 'Generated'} {len(expected)} complete inputs "
        f"for {len(cases())} encode and {decode_declarations} decode declarations"
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path,
                        default=ROOT / "tests/fixtures/input/images")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    generate(args.output.resolve(), args.check)


if __name__ == "__main__":
    main()
