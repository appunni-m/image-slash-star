#!/usr/bin/env python3
"""Generate a Pillow-verified I420 AVIF with chroma-scaled-from-luma grain."""

import argparse
import hashlib
import random
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, __version__ as pillow_version, features

from generate_avif_filmgrain_reference_reuse import (  # noqa: E402
    bits_from_payload,
    leb128,
    pack_bits,
    padding_obu,
)
from inspect_av1_obus import inspect  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "portable_lossless_filmgrain_420_chroma_from_luma_64x64.avif"
)
SOURCE_SHA256 = "9f68770ff865a1648abcbe018cea32a417104c47a8f4ff102b45c51c7560b294"
OUTPUT_SHA256 = "80c3a157f32f190a787588c48232113c4bbe915abd80812c5e0d1d62fd51c893"
RGB_SHA256 = "ee0b8557ef74499434382e1776a76492adc857b142f0481f4ef29c4e5a9d82cd"
SIZE = (64, 64)
ADVANCED = {
    "color-primaries": "1",
    "transfer-characteristics": "13",
    "matrix-coefficients": "6",
    "min-partition-size": "64",
    "max-partition-size": "64",
    "use-intra-dct-only": "1",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "aq-mode": "0",
    "deltaq-mode": "0",
    "film-grain-test": "1",
}


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of bytes."""

    return hashlib.sha256(data).hexdigest()


def encode_source() -> bytes:
    """Encode a deterministic lossless I420 input with pinned AOM grain syntax."""

    generator = random.Random(0x4672)
    pixels = bytes(generator.randrange(256) for _ in range(SIZE[0] * SIZE[1] * 3))
    output = BytesIO()
    Image.frombytes("RGB", SIZE, pixels).save(
        output,
        format="AVIF",
        quality=100,
        speed=0,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        codec="aom",
        advanced=ADVANCED,
    )
    return output.getvalue()


def mutate(data: bytes) -> tuple[bytes, bytes]:
    with tempfile.NamedTemporaryFile(suffix=".avif") as temporary:
        temporary.write(data)
        temporary.flush()
        report = inspect(Path(temporary.name))
    sample = next(sample for sample in report["samples"] if sample["role"] == "item_color")
    sequence = next(
        obu["sequence_header"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    )
    frame_obus = [obu for obu in sample["obus"] if obu.get("frame_header")]
    if len(frame_obus) != 1:
        raise ValueError(f"expected one frame OBU, got {len(frame_obus)}")
    obu = frame_obus[0]
    header = obu["frame_header"]
    grain = header["film_grain"]
    tile_group = obu["tile_group"]
    if (
        sequence["bit_depth"] != 8
        or sequence["monochrome"]
        or not sequence["subsampling_x"]
        or not sequence["subsampling_y"]
        or not sequence["film_grain_present"]
        or header["frame_type"] != "key"
        or not header["all_lossless"]
        or (header["frame_width"], header["frame_height"]) != SIZE
    ):
        raise ValueError("input is not the expected 8-bit lossless I420 key frame")
    if (
        grain is None
        or not grain["update"]
        or grain["chroma_scaling_from_luma"]
        or not all(grain["uv_points"])
        or not grain["y_points"]
    ):
        raise ValueError("input lacks the expected independent U/V grain points")

    payload_span = obu["payload_spans"][0]
    payload = data[payload_span["offset"] : payload_span["offset"] + payload_span["length"]]
    grain_start = header["film_grain_start_bit"]
    old_tile_start = tile_group["data_bit"]
    bits = bits_from_payload(payload, old_tile_start)
    cursor = grain_start

    def read(width: int) -> int:
        nonlocal cursor
        value = 0
        for _ in range(width):
            value = (value << 1) | bits[cursor]
            cursor += 1
        return value

    def skip(width: int) -> None:
        nonlocal cursor
        cursor += width

    apply_grain = read(1)
    seed = read(16)
    y_count = read(4)
    if apply_grain != 1 or seed != grain["seed"] or y_count != len(grain["y_points"]):
        raise ValueError("film-grain prefix disagrees with independent inspector")
    for expected_x, expected_y in grain["y_points"]:
        if (read(8), read(8)) != (expected_x, expected_y):
            raise ValueError("Y grain points disagree with independent inspector")

    cfl_bit = cursor
    if read(1) != 0:
        raise ValueError("expected chroma_scaling_from_luma to be clear before mutation")
    old_uv_points: list[list[tuple[int, int]]] = []
    for _plane in range(2):
        count = read(4)
        points = [(read(8), read(8)) for _ in range(count)]
        old_uv_points.append(points)
    expected_uv_points = [[tuple(point) for point in plane] for plane in grain["uv_points"]]
    if old_uv_points != expected_uv_points:
        raise ValueError("U/V grain points disagree with independent inspector")
    uv_points_end = cursor

    scaling_shift = read(2) + 8
    lag = read(2)
    ar_positions = lag * (lag + 1) * 2
    if scaling_shift != grain["scaling_shift"] or lag != grain["ar_coefficient_lag"]:
        raise ValueError("grain scaling/lag fields disagree with independent inspector")
    skip((ar_positions if y_count else 0) * 8)
    for plane_points in old_uv_points:
        if plane_points:
            skip((ar_positions + int(y_count != 0)) * 8)
    skip(4)  # AR coefficient shift and grain scaling shift.
    uv_parameters_start = cursor
    for plane_points in old_uv_points:
        if plane_points:
            skip(25)
    uv_parameters_end = cursor
    overlap = read(1)
    clip = read(1)
    grain_end = cursor
    if overlap != int(grain["overlap"]) or clip != int(grain["clip_to_restricted_range"]):
        raise ValueError("grain trailing flags disagree with independent inspector")

    alignment = bits[grain_end:old_tile_start]
    if len(alignment) > 7 or any(alignment):
        raise ValueError(f"unexpected frame-header alignment bits: {alignment}")

    # Keep the Y points and AR coefficients, switch to chroma-from-luma, and
    # remove the now-absent U/V point and multiplier syntax.
    mutated_bits = (
        bits[:cfl_bit]
        + [1]
        + bits[uv_points_end:uv_parameters_start]
        + bits[uv_parameters_end:grain_end]
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
    new_frame_obu = raw_header[:base_header_length] + leb128(len(new_payload)) + new_payload

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

    with tempfile.NamedTemporaryFile(suffix=".avif") as temporary:
        temporary.write(mutated)
        temporary.flush()
        verified = inspect(Path(temporary.name))
    verified_sample = next(
        sample for sample in verified["samples"] if sample["role"] == "item_color"
    )
    verified_header = next(
        obu["frame_header"] for obu in verified_sample["obus"] if obu.get("frame_header")
    )
    verified_grain = verified_header["film_grain"]
    if (
        not verified_grain["chroma_scaling_from_luma"]
        or any(verified_grain["uv_points"])
        or not verified_grain["y_points"]
        or not all(verified_grain["ar_coefficients_uv"])
        or verified_grain["ar_coefficients_y"] != grain["ar_coefficients_y"]
        or verified_grain["ar_coefficients_uv"] != grain["ar_coefficients_uv"]
    ):
        raise ValueError("mutated film-grain syntax failed independent verification")

    with Image.open(BytesIO(mutated)) as image:
        image.load()
        pixels = image.convert("RGB").tobytes()
    print(
        f"Verified tile data bit {old_tile_start} -> {new_tile_start}; "
        f"chroma-from-luma={verified_grain['chroma_scaling_from_luma']}"
    )
    return bytes(mutated), pixels


def main() -> None:
    """Generate and verify the pinned AVIF bytes and Pillow pixels."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()

    if pillow_version != "12.2.0" or features.version("avif") != "1.4.1":
        raise RuntimeError("Pillow 12.2.0 with libavif 1.4.1 is required")
    codecs = _avif.codec_versions()
    if not all(version in codecs for version in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2")):
        raise RuntimeError(f"pinned dav1d/libaom codecs are required, found {codecs}")

    source = encode_source()
    if sha256(source) != SOURCE_SHA256 or source != encode_source():
        raise RuntimeError("pinned lossless I420 source bytes differ")
    mutated, pixels = mutate(source)
    repeated, repeated_pixels = mutate(encode_source())
    if mutated != repeated or sha256(mutated) != OUTPUT_SHA256:
        raise RuntimeError("chroma-from-luma AVIF bytes differ from the pinned fixture")
    if pixels != repeated_pixels or sha256(pixels) != RGB_SHA256:
        raise RuntimeError("pinned Pillow RGB reference differs")

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(mutated)
    print(f"Wrote {len(mutated)} bytes with SHA-256 {OUTPUT_SHA256}: {args.output}")


if __name__ == "__main__":
    main()
