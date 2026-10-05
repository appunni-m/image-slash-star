#!/usr/bin/env python3
"""Generate a two-frame AVIF whose inter frame reuses key-frame grain."""

from __future__ import annotations

import hashlib
import random
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features, __version__ as pillow_version

from generate_avif_10bit_lossless_inter_fixture import (
    normalize_sequence_timestamps,
    validate_timestamp_repeatability,
)
from inspect_av1_obus import inspect as inspect_av1


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "animated_filmgrain_reference_reuse_i444_64x64.avif"
)
SIZE = (64, 64)
SOURCE_SHA256 = "4dca31b495bd80ec40d0d84291782f285cc091f8b39fcec63ffb03bf16b334a6"
OUTPUT_SHA256 = "bd947085ed6437edfd50a97b43506cc56e96af8b5450a8ef5cb8289b8ec62b34"
FRAME_SHA256 = (
    "a4e4fa07369777b09a15c680d4812441a5509a7662740c039127a998bdf3d9ab",
    "d9902750b3685e4c451df39a76c6f18848dc7bb6cb01055174d34d8d0def0052",
)
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


def source_frames() -> tuple[Image.Image, Image.Image]:
    """Build deterministic RGB frames with a localized second-frame change."""

    width, height = SIZE
    generator = random.Random(0x4672)
    first = bytearray(generator.randrange(256) for _ in range(width * height * 3))
    second = bytearray(first)
    for y in range(20, 36):
        for x in range(20, 36):
            offset = (y * width + x) * 3
            second[offset : offset + 3] = bytes(
                255 - first[offset + channel] for channel in range(3)
            )
    return (
        Image.frombytes("RGB", SIZE, bytes(first)),
        Image.frombytes("RGB", SIZE, bytes(second)),
    )


def encode_source() -> bytes:
    """Encode the exact Pillow two-frame source before AV1 header mutation."""

    first, second = source_frames()
    output = BytesIO()
    first.save(
        output,
        format="AVIF",
        save_all=True,
        append_images=[second],
        duration=[100, 100],
        loop=0,
        quality=100,
        speed=0,
        max_threads=1,
        subsampling="4:4:4",
        autotiling=False,
        codec="aom",
        advanced=ADVANCED,
    )
    return output.getvalue()


def leb128(value: int) -> bytes:
    """Encode one unsigned AV1 OBU size value."""

    encoded = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            byte |= 0x80
        encoded.append(byte)
        if not value:
            return bytes(encoded)


def padding_obu(length: int) -> bytes:
    """Return an AV1 padding OBU of an exact byte length."""

    for payload_length in range(length):
        candidate = b"\x7a" + leb128(payload_length) + bytes(payload_length)
        if len(candidate) == length:
            return candidate
    raise RuntimeError(f"cannot encode an AV1 padding OBU of {length} bytes")


def bits_from_payload(payload: bytes, end: int) -> list[int]:
    """Return the first `end` payload bits in AV1 most-significant order."""

    return [
        (payload[position // 8] >> (7 - position % 8)) & 1
        for position in range(end)
    ]


def append_bits(output: list[int], value: int, width: int) -> None:
    """Append one unsigned field in AV1 most-significant-bit order."""

    output.extend((value >> (width - offset - 1)) & 1 for offset in range(width))


def pack_bits(bits: list[int]) -> bytearray:
    """Pack AV1 bits and require byte alignment."""

    if len(bits) % 8:
        raise RuntimeError("AV1 frame header is not byte-aligned")
    packed = bytearray(len(bits) // 8)
    for position, value in enumerate(bits):
        if value:
            packed[position // 8] |= 1 << (7 - position % 8)
    return packed


def mutate_inter_frame(data: bytes) -> bytes:
    """Replace inter-frame grain parameters with a reference to key-frame grain."""

    with tempfile.NamedTemporaryFile(suffix=".avif") as temporary:
        temporary.write(data)
        temporary.flush()
        report = inspect_av1(Path(temporary.name))

    track_samples = sorted(
        (sample for sample in report["samples"] if sample["role"] == "track_pict"),
        key=lambda sample: sample["identity"]["sample"],
    )
    if len(track_samples) != 2:
        raise RuntimeError(f"expected two AVIF picture samples, found {len(track_samples)}")

    first_obu = next(
        obu for obu in track_samples[0]["obus"] if obu.get("frame_header") is not None
    )
    first_header = first_obu["frame_header"]
    first_grain = first_header["film_grain"]
    if (
        first_header["frame_type"] != "key"
        or first_header["refresh_frame_flags"] != 0xFF
        or first_grain is None
        or not first_grain["update"]
    ):
        raise RuntimeError("first AV1 frame does not refresh references with grain parameters")

    second_sample = track_samples[1]
    second_obu = next(
        obu for obu in second_sample["obus"] if obu.get("frame_header") is not None
    )
    second_header = second_obu["frame_header"]
    second_grain = second_header["film_grain"]
    reference_indices = second_header["reference_indices"]
    if (
        second_header["frame_type"] != "inter"
        or second_grain is None
        or not second_grain["update"]
        or not reference_indices
    ):
        raise RuntimeError("second AV1 frame lacks inter-frame grain parameters and references")

    reference_slot = reference_indices[0]
    frame_payload_span = second_obu["payload_spans"]
    frame_header_span = second_obu["header_spans"]
    sample_span = second_sample["spans"]
    tile_group = second_obu["tile_group"]
    if (
        len(frame_payload_span) != 1
        or len(frame_header_span) != 1
        or len(sample_span) != 1
        or tile_group["start"] != 0
        or tile_group["end"] != 0
    ):
        raise RuntimeError("fixture layout no longer matches the single-tile AV1 mutator")

    payload_offset = frame_payload_span[0]["offset"]
    payload_length = frame_payload_span[0]["length"]
    payload = data[payload_offset : payload_offset + payload_length]
    grain_start = second_header.get("film_grain_start_bit")
    old_tile_data_bit = tile_group["data_bit"]
    if (
        not isinstance(grain_start, int)
        or grain_start + 20 >= old_tile_data_bit
        or old_tile_data_bit % 8
    ):
        raise RuntimeError("inter-frame grain or tile-data bit offsets are invalid")

    frame_bits = bits_from_payload(payload, grain_start)
    append_bits(frame_bits, second_grain["seed"], 16)
    append_bits(frame_bits, 0, 1)
    append_bits(frame_bits, reference_slot, 3)
    while len(frame_bits) % 8:
        frame_bits.append(0)
    new_tile_data_bit = len(frame_bits)
    new_payload = pack_bits(frame_bits)
    new_payload.extend(payload[old_tile_data_bit // 8 :])

    frame_header_offset = frame_header_span[0]["offset"]
    frame_header_length = frame_header_span[0]["length"]
    base_header_length = 1 + int(second_obu["has_extension"])
    raw_header = data[frame_header_offset : frame_header_offset + frame_header_length]
    new_frame_obu = (
        raw_header[:base_header_length]
        + leb128(len(new_payload))
        + new_payload
    )

    sample_offset = sample_span[0]["offset"]
    sample_length = sample_span[0]["length"]
    sample_end = sample_offset + sample_length
    old_sample = data[sample_offset:sample_end]
    frame_obu_relative_offset = frame_header_offset - sample_offset
    old_frame_obu_length = frame_header_length + payload_length
    if frame_obu_relative_offset + old_frame_obu_length != len(old_sample):
        raise RuntimeError("frame OBU is not the last OBU in its AVIF sample")
    new_sample = (
        old_sample[:frame_obu_relative_offset]
        + new_frame_obu
    )
    size_difference = len(old_sample) - len(new_sample)
    if size_difference <= 1:
        raise RuntimeError("film-grain reuse did not shorten the frame OBU")
    new_sample += padding_obu(size_difference)
    if len(new_sample) != len(old_sample):
        raise RuntimeError("AV1 padding did not preserve the AVIF sample size")

    mutated = bytearray(data)
    mutated[sample_offset:sample_end] = new_sample

    with tempfile.NamedTemporaryFile(suffix=".avif") as temporary:
        temporary.write(mutated)
        temporary.flush()
        verified = inspect_av1(Path(temporary.name))
    verified_sample = next(
        sample
        for sample in verified["samples"]
        if sample["role"] == "track_pict" and sample["identity"]["sample"] == 1
    )
    verified_obus = verified_sample["obus"]
    verified_frame = next(
        obu["frame_header"]
        for obu in verified_obus
        if obu.get("frame_header") is not None
    )
    if (
        verified_frame["film_grain"] is None
        or verified_frame["film_grain"]["update"]
        or verified_frame["film_grain"]["reference_slot"] != reference_slot
        or not any(obu["name"] == "padding" for obu in verified_obus)
        or new_tile_data_bit % 8
    ):
        raise RuntimeError("rewritten frame does not signal valid grain reuse")
    return bytes(mutated)


def frame_hashes(data: bytes) -> tuple[str, ...]:
    """Decode both Pillow frames and return exact pixel-byte hashes."""

    hashes = []
    with Image.open(BytesIO(data)) as image:
        if image.n_frames != 2 or image.mode != "RGB" or image.size != SIZE:
            raise RuntimeError("film-grain reference-reuse output changed Pillow frame metadata")
        for frame_index in range(image.n_frames):
            image.seek(frame_index)
            image.load()
            if image.mode != "RGB" or image.size != SIZE:
                raise RuntimeError("film-grain reference-reuse frame metadata differs")
            hashes.append(hashlib.sha256(image.tobytes()).hexdigest())
    return tuple(hashes)


def main() -> None:
    """Generate the pinned full-file Pillow parity fixture."""

    if (
        pillow_version != "12.2.0"
        or features.version("avif") != "1.4.1"
        or _avif.codec_versions()
        != "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
    ):
        raise RuntimeError("the pinned Pillow 12.2.0 AVIF oracle is required")

    first_source = encode_source()
    second_source = encode_source()
    validate_timestamp_repeatability(first_source, second_source)
    source = normalize_sequence_timestamps(first_source)
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("film-grain source sequence differs from its pinned bytes")
    output = mutate_inter_frame(source)
    if hashlib.sha256(output).hexdigest() != OUTPUT_SHA256:
        raise RuntimeError("film-grain reference-reuse AVIF differs from its pinned bytes")
    if frame_hashes(output) != FRAME_SHA256:
        raise RuntimeError("Pillow frame pixels differ from their pinned hashes")

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(output)
    print(f"Wrote {OUTPUT.relative_to(ROOT)} ({len(output)} bytes)")


if __name__ == "__main__":
    main()
