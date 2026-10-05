#!/usr/bin/env python3
"""Generate a Pillow-verified I420 AVIF with zero Y film-grain points."""

from __future__ import annotations

import argparse
import hashlib
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features, __version__ as pillow_version

from generate_avif_filmgrain_chroma_from_luma_420 import (
    SOURCE_SHA256,
    encode_source,
)
from generate_avif_filmgrain_reference_reuse import (
    append_bits,
    bits_from_payload,
    leb128,
    pack_bits,
    padding_obu,
)
from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "portable_lossless_filmgrain_420_zero_y_points_64x64.avif"
)
OUTPUT_SHA256 = "c76ede4ac45599b182d023f8425744f09dc27e4011742d6295d3ec50e172ed9f"
RGB_SHA256 = "5d35ab50438b9f1536ef7de3cbca8ac3d01360efa9d77446dfe0d69786d2fb62"
SIZE = (64, 64)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of bytes."""

    return hashlib.sha256(data).hexdigest()


def inspect_avif(
    data: bytes,
) -> tuple[dict[str, object], dict[str, object], dict[str, object]]:
    """Return the color sample, sequence header, and its frame header."""

    with tempfile.NamedTemporaryFile(suffix=".avif") as temporary:
        temporary.write(data)
        temporary.flush()
        report = inspect(Path(temporary.name))

    sample = next(
        sample for sample in report["samples"] if sample["role"] == "item_color"
    )
    sequence = next(
        obu["sequence_header"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    )
    frame = next(
        obu["frame_header"]
        for obu in sample["obus"]
        if obu.get("frame_header") is not None
    )
    return sample, sequence, frame


def mutate(data: bytes) -> tuple[bytes, bytes]:
    """Rewrite only film-grain syntax, retaining AV1 tile data and item size."""

    sample, sequence, frame = inspect_avif(data)
    grain = frame["film_grain"]
    frame_obus = [obu for obu in sample["obus"] if obu.get("frame_header") is not None]
    if len(frame_obus) != 1:
        raise ValueError(f"expected one frame OBU, got {len(frame_obus)}")
    obu = frame_obus[0]
    tile_group = obu["tile_group"]
    if (
        sequence["bit_depth"] != 8
        or sequence["monochrome"]
        or not sequence["subsampling_x"]
        or not sequence["subsampling_y"]
        or not sequence["film_grain_present"]
        or frame["frame_type"] != "key"
        or not frame["all_lossless"]
        or (frame["frame_width"], frame["frame_height"]) != SIZE
    ):
        raise ValueError("input is not the expected 8-bit lossless I420 key frame")
    if (
        grain is None
        or not grain["update"]
        or grain["chroma_scaling_from_luma"]
        or not grain["y_points"]
        or not all(grain["uv_points"])
    ):
        raise ValueError("input lacks the expected explicit I420 film-grain syntax")

    payload_span = obu["payload_spans"][0]
    payload = data[
        payload_span["offset"] : payload_span["offset"] + payload_span["length"]
    ]
    grain_start = frame["film_grain_start_bit"]
    old_tile_start = tile_group["data_bit"]
    bits = bits_from_payload(payload, old_tile_start)
    cursor = grain_start

    def read(width: int) -> int:
        nonlocal cursor
        value = 0
        for _ in range(width):
            if cursor >= old_tile_start:
                raise ValueError("film-grain syntax runs into tile data")
            value = (value << 1) | bits[cursor]
            cursor += 1
        return value

    def skip(width: int) -> None:
        nonlocal cursor
        if width < 0 or cursor + width > old_tile_start:
            raise ValueError("film-grain syntax runs into tile data")
        cursor += width

    apply_grain = read(1)
    seed = read(16)
    y_count = read(4)
    if apply_grain != 1 or seed != grain["seed"] or y_count != len(grain["y_points"]):
        raise ValueError("film-grain prefix disagrees with the independent inspector")
    for expected_x, expected_y in grain["y_points"]:
        if (read(8), read(8)) != (expected_x, expected_y):
            raise ValueError("Y grain points disagree with the independent inspector")

    if read(1) != 0:
        raise ValueError("expected chroma_scaling_from_luma to be clear in the source")
    old_uv_points: list[list[tuple[int, int]]] = []
    for plane in range(2):
        count = read(4)
        points = [(read(8), read(8)) for _ in range(count)]
        old_uv_points.append(points)
    expected_uv_points = [
        [tuple(point) for point in points] for points in grain["uv_points"]
    ]
    if old_uv_points != expected_uv_points:
        raise ValueError("U/V grain points disagree with the independent inspector")

    scaling_and_lag_start = cursor
    scaling_shift = read(2) + 8
    lag = read(2)
    ar_positions = 2 * lag * (lag + 1)
    if scaling_shift != grain["scaling_shift"] or lag != grain["ar_coefficient_lag"]:
        raise ValueError(
            "grain scaling or lag fields disagree with the independent inspector"
        )

    y_coefficients = [read(8) - 128 for _ in range(ar_positions)]
    if y_coefficients != grain["ar_coefficients_y"]:
        raise ValueError("Y AR coefficients disagree with the independent inspector")
    uv_coefficients = []
    for plane_points in old_uv_points:
        coefficients = [read(8) - 128 for _ in range(ar_positions + int(y_count != 0))]
        if not plane_points:
            raise ValueError("source unexpectedly omits I420 U/V point tables")
        uv_coefficients.append(coefficients)
    if uv_coefficients != grain["ar_coefficients_uv"]:
        raise ValueError("U/V AR coefficients disagree with the independent inspector")

    ar_shifts_start = cursor
    ar_coefficient_shift = read(2) + 6
    grain_scale_shift = read(2)
    if (
        ar_coefficient_shift != grain["ar_coefficient_shift"]
        or grain_scale_shift != grain["grain_scale_shift"]
    ):
        raise ValueError(
            "grain AR/scaling shift fields disagree with the independent inspector"
        )

    uv_parameters = []
    for plane_points in old_uv_points:
        if plane_points:
            uv_parameters.append((read(8) - 128, read(8) - 128, read(9) - 256))
        else:
            uv_parameters.append((0, 0, 0))
    if (
        [values[0] for values in uv_parameters] != grain["uv_multiplier"]
        or [values[1] for values in uv_parameters] != grain["uv_luma_multiplier"]
        or [values[2] for values in uv_parameters] != grain["uv_offset"]
    ):
        raise ValueError("U/V grain parameters disagree with the independent inspector")

    trailing_flags_start = cursor
    overlap = read(1)
    clip = read(1)
    grain_end = cursor
    if (
        overlap != int(grain["overlap"])
        or clip != int(grain["clip_to_restricted_range"])
    ):
        raise ValueError("grain trailing flags disagree with the independent inspector")

    alignment = bits[grain_end:old_tile_start]
    if len(alignment) > 7 or any(alignment):
        raise ValueError(f"unexpected frame-header alignment bits: {alignment}")

    # In 4:2:0, zero Y points suppress both U/V point tables. With CFL clear,
    # the AR coefficient and multiplier syntax is also absent.
    mutated_bits = bits[:grain_start]
    append_bits(mutated_bits, 1, 1)
    append_bits(mutated_bits, seed, 16)
    append_bits(mutated_bits, 0, 4)
    append_bits(mutated_bits, 0, 1)
    mutated_bits.extend(bits[scaling_and_lag_start : scaling_and_lag_start + 4])
    mutated_bits.extend(bits[ar_shifts_start : ar_shifts_start + 4])
    mutated_bits.extend(bits[trailing_flags_start:grain_end])
    if len(mutated_bits) - grain_start != 32:
        raise ValueError(
            "rewritten zero-point film-grain syntax has an invalid bit length"
        )
    while len(mutated_bits) % 8:
        mutated_bits.append(0)
    new_tile_start = len(mutated_bits)
    new_payload = pack_bits(mutated_bits)
    new_payload.extend(payload[old_tile_start // 8 :])

    frame_header_span = obu["header_spans"][0]
    header_start = frame_header_span["offset"]
    header_length = frame_header_span["length"]
    base_header_length = 1 + int(obu["has_extension"])
    raw_header = data[header_start : header_start + header_length]
    new_frame_obu = (
        raw_header[:base_header_length] + leb128(len(new_payload)) + new_payload
    )

    sample_span = sample["spans"][0]
    sample_start = sample_span["offset"]
    sample_length = sample_span["length"]
    old_sample = data[sample_start : sample_start + sample_length]
    frame_offset = header_start - sample_start
    old_frame_length = header_length + payload_span["length"]
    if frame_offset + old_frame_length != len(old_sample):
        raise ValueError("frame OBU is not the last OBU in the AV1 item")
    new_sample = old_sample[:frame_offset] + new_frame_obu
    padding_length = len(old_sample) - len(new_sample)
    if padding_length <= 0:
        raise ValueError(f"mutated frame did not shorten the item ({padding_length})")
    new_sample += padding_obu(padding_length)
    if len(new_sample) != len(old_sample):
        raise ValueError("AV1 padding failed to preserve the item extent")
    mutated = bytearray(data)
    mutated[sample_start : sample_start + sample_length] = new_sample

    verified_sample, verified_sequence, verified_frame = inspect_avif(bytes(mutated))
    verified_grain = verified_frame["film_grain"]
    if (
        verified_sequence["monochrome"]
        or not verified_sequence["subsampling_x"]
        or not verified_sequence["subsampling_y"]
        or verified_frame["frame_type"] != "key"
        or verified_grain is None
        or not verified_grain["update"]
        or verified_grain["seed"] != seed
        or verified_grain["y_points"]
        or verified_grain["chroma_scaling_from_luma"]
        or any(verified_grain["uv_points"])
        or verified_grain["ar_coefficients_y"]
        or any(verified_grain["ar_coefficients_uv"])
    ):
        raise ValueError(
            "rewritten zero-Y-point film-grain syntax failed independent verification"
        )
    if verified_sample["spans"][0]["length"] != sample_length:
        raise ValueError("rewritten AV1 item changed its pinned extent")

    with Image.open(BytesIO(mutated)) as image:
        image.load()
        if image.size != SIZE:
            raise ValueError(f"Pillow decoded unexpected dimensions: {image.size}")
        pixels = image.convert("RGB").tobytes()
    return bytes(mutated), pixels


def main() -> None:
    """Generate and validate the pinned AVIF bytes and Pillow RGB pixels."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()

    if pillow_version != "12.2.0" or features.version("avif") != "1.4.1":
        raise RuntimeError("Pillow 12.2.0 with libavif 1.4.1 is required")
    codecs = _avif.codec_versions()
    if not all(
        version in codecs
        for version in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2")
    ):
        raise RuntimeError(f"pinned dav1d/libaom codecs are required, found {codecs}")

    source = encode_source()
    if sha256(source) != SOURCE_SHA256 or source != encode_source():
        raise RuntimeError("pinned lossless I420 source bytes differ")
    mutated, pixels = mutate(source)
    repeated, repeated_pixels = mutate(encode_source())
    if mutated != repeated or sha256(mutated) != OUTPUT_SHA256:
        raise RuntimeError(
            "zero-Y-point film-grain AVIF bytes differ from the pinned fixture"
        )
    if pixels != repeated_pixels or sha256(pixels) != RGB_SHA256:
        raise RuntimeError("pinned Pillow RGB reference differs")

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(mutated)
    print(f"Wrote {len(mutated)} bytes with SHA-256 {OUTPUT_SHA256}: {args.output}")
    print(f"Pillow RGB output: {len(pixels)} bytes with SHA-256 {RGB_SHA256}")


if __name__ == "__main__":
    main()
