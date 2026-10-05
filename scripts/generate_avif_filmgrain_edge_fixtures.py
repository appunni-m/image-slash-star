#!/usr/bin/env python3
"""Generate full-file AVIF film-grain success and malformed-input fixtures.

All inputs come from the pinned Pillow/libavif/libaom encoder. Header mutations
use independently inspected field boundaries and preserve the encoded tile
payload. Shorter headers receive a padding OBU to retain the BMFF sample extent.
The live Pillow decoder supplies the success pixels and rejection outcomes.
"""

from __future__ import annotations

import argparse
import hashlib
import random
import tempfile
from copy import deepcopy
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features, __version__ as pillow_version

from generate_avif_filmgrain_chroma_from_luma_420 import (
    SOURCE_SHA256 as I420_SOURCE_SHA256,
    encode_source as encode_i420_source,
)
from generate_avif_filmgrain_reference_reuse import (
    ADVANCED,
    OUTPUT_SHA256 as REUSE_SOURCE_SHA256,
    SOURCE_SHA256 as ANIMATED_SOURCE_SHA256,
    append_bits,
    bits_from_payload,
    encode_source as encode_animated_source,
    leb128,
    mutate_inter_frame,
    normalize_sequence_timestamps,
    pack_bits,
    padding_obu,
    validate_timestamp_repeatability,
)
from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT_DIR = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SIZE = (64, 64)
I444_SOURCE_SHA256 = "5e40ca71068fdd232d11f356103f53c1fdd7b94356fa91b0004d7307de4cbd25"
I422_NAME = "portable_lossless_filmgrain_i422_64x64.avif"
INACTIVE_REUSE_NAME = "animated_filmgrain_inactive_reference_reuse_i444_64x64.avif"
FIXTURE_SHA256 = {
    I422_NAME: "7c0241226c98d663df8b704b60fabb22e625bf7b010ad194f41db4b80afa0344",
    "malformed_filmgrain_y_point_count_15.avif": (
        "81be05bdc9f5004a0c8afbbad23cf94bed8b888affbb45a728d05600940fd444"
    ),
    "malformed_filmgrain_uv_point_count_11.avif": (
        "d2e202c8fcd4942096a4d4369c0ca4856f51f7f8fefc16feb796148f87bcd0ed"
    ),
    "malformed_filmgrain_i420_unequal_uv_presence.avif": (
        "ed8928138b0c7758e69c436bcf9cb4354a8a9da7bafa041c8e25832cd10f27eb"
    ),
    "malformed_filmgrain_unused_reference_slot.avif": (
        "82dec6de0b8f46bf2157101847abe34e89a40cd23098b5beb69c1cd0d3eb5ae1"
    ),
    INACTIVE_REUSE_NAME: (
        "d1c4c2b6c9c24571452ccb95387a4c18580d9e7b871e704217b17b58ae916536"
    ),
}
PILLOW_RGB_SHA256 = {
    I422_NAME: (
        "0683e77f414bf0c550f763d136cbedd24ce9e6fb521777243e702a2f05af2016",
    ),
    INACTIVE_REUSE_NAME: (
        "2b546a196a9dbf7c347d733e69334f6a4c19753eeb274d23aad318068585db84",
        "2af8438108708dbe0fe860299b979f240526edd268ea5939d17f26f242651838",
    ),
}
MALFORMED_EXPECTATIONS = {
    "malformed_filmgrain_y_point_count_15.avif": (
        "too many film grain Y points",
        0,
    ),
    "malformed_filmgrain_uv_point_count_11.avif": (
        "too many film grain UV points",
        0,
    ),
    "malformed_filmgrain_i420_unequal_uv_presence.avif": (
        "4:2:0 film grain UV point presence differs",
        0,
    ),
    "malformed_filmgrain_unused_reference_slot.avif": (
        "film grain references an unused slot",
        1,
    ),
}


def sha256(data: bytes) -> str:
    """Return the lowercase digest used by the frozen input and pixel checks."""

    return hashlib.sha256(data).hexdigest()


def require_pinned_oracle() -> None:
    """Require the encoder and decoder versions that define these fixtures."""

    if (
        pillow_version != "12.2.0"
        or features.version("avif") != "1.4.1"
        or _avif.codec_versions()
        != "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
    ):
        raise RuntimeError("the pinned Pillow 12.2.0 AVIF oracle is required")


def encode_still(seed: int, subsampling: str) -> bytes:
    """Encode seeded 64x64 RGB with the existing fixed-partition grain recipe."""

    generator = random.Random(seed)
    pixels = bytes(generator.randrange(256) for _ in range(SIZE[0] * SIZE[1] * 3))
    output = BytesIO()
    Image.frombytes("RGB", SIZE, pixels).save(
        output,
        format="AVIF",
        quality=100,
        speed=0,
        max_threads=1,
        subsampling=subsampling,
        autotiling=False,
        codec="aom",
        advanced=ADVANCED,
    )
    return output.getvalue()


def inspect_bytes(data: bytes) -> dict[str, object]:
    """Inspect a complete AVIF without retaining a temporary input file."""

    with tempfile.NamedTemporaryFile(suffix=".avif") as temporary:
        temporary.write(data)
        temporary.flush()
        return inspect(Path(temporary.name))


def frame_location(
    data: bytes, role: str = "item_color", sample_index: int | None = None
) -> tuple[dict[str, object], dict[str, object]]:
    """Find one checked single-extent frame in an independently inspected source."""

    report = inspect_bytes(data)
    samples = [
        sample
        for sample in report["samples"]
        if sample["role"] == role
        and (sample_index is None or sample["identity"]["sample"] == sample_index)
    ]
    if len(samples) != 1:
        raise ValueError("source does not contain exactly one requested AV1 sample")
    sample = samples[0]
    frames = [obu for obu in sample["obus"] if obu.get("frame_header") is not None]
    if len(frames) != 1:
        raise ValueError("source sample does not contain exactly one frame OBU")
    obu = frames[0]
    if any(len(spans) != 1 for spans in (
        sample["spans"], obu["payload_spans"], obu["header_spans"]
    )):
        raise ValueError("source sample or frame has multiple extents")
    return sample, obu


def replace_field(
    data: bytes, obu: dict[str, object], bit: int, width: int,
    expected: int, replacement: int,
) -> bytes:
    """Replace one verified MSB-first field within a complete frame payload."""

    span = obu["payload_spans"][0]
    if bit < 0 or width <= 0 or bit + width > span["length"] * 8:
        raise ValueError("film-grain field is outside the frame payload")
    if not 0 <= replacement < 1 << width:
        raise ValueError("replacement does not fit the film-grain field")
    payload = data[span["offset"]:span["offset"] + span["length"]]
    shift = len(payload) * 8 - bit - width
    mask = (1 << width) - 1
    encoded = int.from_bytes(payload, "big")
    if (encoded >> shift) & mask != expected:
        raise ValueError("film-grain field differs from the inspected source")
    encoded = (encoded & ~(mask << shift)) | (replacement << shift)
    mutated = bytearray(data)
    mutated[span["offset"]:span["offset"] + span["length"]] = encoded.to_bytes(
        len(payload), "big"
    )
    return bytes(mutated)


def replace_grain_sentence(
    data: bytes, sample: dict[str, object], obu: dict[str, object],
    grain_bits: list[int],
) -> bytes:
    """Replace grain syntax, preserve every tile byte, and retain the item extent."""

    frame = obu["frame_header"]
    payload_span = obu["payload_spans"][0]
    payload = data[
        payload_span["offset"]:payload_span["offset"] + payload_span["length"]
    ]
    tile_bit = obu["tile_group"]["data_bit"]
    grain_start = frame["film_grain_start_bit"]
    if not 0 <= grain_start < tile_bit <= len(payload) * 8 or tile_bit % 8:
        raise ValueError("film-grain or tile boundaries are invalid")
    bits = bits_from_payload(payload, grain_start) + grain_bits
    while len(bits) % 8:
        bits.append(0)
    new_payload = pack_bits(bits) + payload[tile_bit // 8:]

    header_span = obu["header_spans"][0]
    base_header_length = 1 + int(obu["has_extension"])
    header = data[header_span["offset"]:header_span["offset"] + base_header_length]
    new_obu = header + leb128(len(new_payload)) + new_payload
    sample_span = sample["spans"][0]
    old_sample = data[
        sample_span["offset"]:sample_span["offset"] + sample_span["length"]
    ]
    frame_offset = header_span["offset"] - sample_span["offset"]
    if frame_offset + header_span["length"] + payload_span["length"] != len(old_sample):
        raise ValueError("frame OBU is not the last OBU in its sample")
    new_sample = old_sample[:frame_offset] + new_obu
    difference = len(old_sample) - len(new_sample)
    if difference <= 1:
        raise ValueError("rewritten grain header does not leave room for padding")
    new_sample += padding_obu(difference)
    if len(new_sample) != len(old_sample):
        raise ValueError("AV1 padding did not retain the sample extent")
    mutated = bytearray(data)
    mutated[sample_span["offset"]:sample_span["offset"] + sample_span["length"]] = (
        new_sample
    )
    return bytes(mutated)


def encode_update_grain(grain: dict[str, object]) -> list[int]:
    """Write the explicit-point update sentence used by the checked I420 source."""

    if not grain["update"] or grain["chroma_scaling_from_luma"] or not grain["y_points"]:
        raise ValueError("source is not an explicit-point grain update")
    bits: list[int] = []
    append_bits(bits, 1, 1)
    append_bits(bits, grain["seed"], 16)
    append_bits(bits, len(grain["y_points"]), 4)
    for point in grain["y_points"]:
        for value in point:
            append_bits(bits, value, 8)
    append_bits(bits, 0, 1)
    for points in grain["uv_points"]:
        append_bits(bits, len(points), 4)
        for point in points:
            for value in point:
                append_bits(bits, value, 8)
    append_bits(bits, grain["scaling_shift"] - 8, 2)
    append_bits(bits, grain["ar_coefficient_lag"], 2)
    for coefficient in grain["ar_coefficients_y"]:
        append_bits(bits, coefficient + 128, 8)
    for points, coefficients in zip(grain["uv_points"], grain["ar_coefficients_uv"]):
        if points:
            for coefficient in coefficients:
                append_bits(bits, coefficient + 128, 8)
    append_bits(bits, grain["ar_coefficient_shift"] - 6, 2)
    append_bits(bits, grain["grain_scale_shift"], 2)
    for plane, points in enumerate(grain["uv_points"]):
        if points:
            append_bits(bits, grain["uv_multiplier"][plane] + 128, 8)
            append_bits(bits, grain["uv_luma_multiplier"][plane] + 128, 8)
            append_bits(bits, grain["uv_offset"][plane] + 256, 9)
    append_bits(bits, int(grain["overlap"]), 1)
    append_bits(bits, int(grain["clip_to_restricted_range"]), 1)
    return bits


def unequal_uv_presence(source: bytes) -> bytes:
    """Keep nonempty Y/V points while removing U points and their conditional fields."""

    sample, obu = frame_location(source)
    frame = obu["frame_header"]
    grain = deepcopy(frame["film_grain"])
    if frame["frame_type"] != "key" or not all(grain["uv_points"]):
        raise ValueError("source does not contain explicit key-frame U/V point tables")
    grain["uv_points"][0] = []
    grain["ar_coefficients_uv"][0] = []
    return replace_grain_sentence(source, sample, obu, encode_update_grain(grain))


def inactive_reference_reuse(source: bytes) -> bytes:
    """Disable key-frame grain while the inter frame reuses its listed reference.

    libaom 3.13.2 decodeframe.c clears parameters when apply_grain is false,
    while reference availability follows the sequence's grain-present flag.
    Inheriting those inactive parameters is valid and leaves both displays
    unchanged. The checked rewrite omits the key frame's conditional grain
    fields and leaves the second frame's reference sentence untouched.
    """

    sample, obu = frame_location(source, "track_pict", 0)
    frame = obu["frame_header"]
    if frame["frame_type"] != "key" or not frame["film_grain"]["update"]:
        raise ValueError("source does not refresh references with key-frame grain")
    return replace_grain_sentence(source, sample, obu, [0])


def pillow_frame_hashes(data: bytes) -> tuple[str, ...]:
    """Decode every live Pillow frame and retain exact RGB output hashes."""

    hashes = []
    with Image.open(BytesIO(data)) as image:
        if image.format != "AVIF" or image.size != SIZE or image.mode != "RGB":
            raise RuntimeError("Pillow AVIF image metadata differs")
        for index in range(image.n_frames):
            image.seek(index)
            image.load()
            if image.size != SIZE or image.mode != "RGB":
                raise RuntimeError("Pillow AVIF frame metadata differs")
            hashes.append(sha256(image.tobytes()))
    return tuple(hashes)


def validate_malformed(name: str, data: bytes) -> None:
    """Require the intended syntax violation and exact live Pillow rejection."""

    inspector_message, frame_index = MALFORMED_EXPECTATIONS[name]
    try:
        inspect_bytes(data)
    except ValueError as error:
        if str(error) != inspector_message:
            raise RuntimeError(f"{name}: unexpected inspector rejection: {error}") from error
    else:
        raise RuntimeError(f"{name}: independent inspector accepted malformed syntax")
    try:
        pillow_frame_hashes(data)
    except RuntimeError as error:
        expected = f"Failed to decode frame {frame_index}: Decoding of color planes failed"
        if str(error) != expected:
            raise RuntimeError(f"{name}: unexpected Pillow rejection: {error}") from error
    else:
        raise RuntimeError(f"{name}: Pillow accepted malformed syntax")


def fixture_bytes() -> dict[str, bytes]:
    """Regenerate all source encodes and construct the six input-only cases."""

    i444 = encode_still(0x21108, "4:4:4")
    if i444 != encode_still(0x21108, "4:4:4") or sha256(i444) != I444_SOURCE_SHA256:
        raise RuntimeError("pinned I444 grain source differs")
    i420 = encode_i420_source()
    if i420 != encode_i420_source() or sha256(i420) != I420_SOURCE_SHA256:
        raise RuntimeError("pinned I420 grain source differs")
    i422 = encode_still(0x4672, "4:2:2")
    if i422 != encode_still(0x4672, "4:2:2"):
        raise RuntimeError("I422 grain encoder output is not repeatable")

    first_animated = encode_animated_source()
    second_animated = encode_animated_source()
    validate_timestamp_repeatability(first_animated, second_animated)
    animated = normalize_sequence_timestamps(first_animated)
    if sha256(animated) != ANIMATED_SOURCE_SHA256:
        raise RuntimeError("pinned animated grain source differs")
    reuse = mutate_inter_frame(animated)
    if sha256(reuse) != REUSE_SOURCE_SHA256:
        raise RuntimeError("pinned grain-reference source differs")

    _, obu = frame_location(i444)
    grain = obu["frame_header"]["film_grain"]
    # Key frames have apply_grain, a 16-bit seed, and then num_y_points.
    y_count_bit = obu["frame_header"]["film_grain_start_bit"] + 1 + 16
    # The U count follows Y points and chroma_scaling_from_luma, which is clear.
    if grain["chroma_scaling_from_luma"]:
        raise ValueError("I444 source unexpectedly uses chroma scaling from luma")
    u_count_bit = y_count_bit + 4 + 16 * len(grain["y_points"]) + 1
    fixtures = {
        I422_NAME: i422,
        "malformed_filmgrain_y_point_count_15.avif": replace_field(
            i444, obu, y_count_bit, 4, len(grain["y_points"]), 15
        ),
        "malformed_filmgrain_uv_point_count_11.avif": replace_field(
            i444, obu, u_count_bit, 4, len(grain["uv_points"][0]), 11
        ),
        "malformed_filmgrain_i420_unequal_uv_presence.avif": unequal_uv_presence(i420),
    }
    _, inter_obu = frame_location(reuse, "track_pict", 1)
    inter = inter_obu["frame_header"]
    if inter["frame_type"] != "inter" or inter["film_grain"]["update"]:
        raise ValueError("source inter frame does not reuse grain parameters")
    unused_slot = next(
        (slot for slot in range(8) if slot not in inter["reference_indices"]), None
    )
    if unused_slot is None:
        raise ValueError("source inter frame lists every grain reference slot")
    # apply_grain and seed precede update_grain=0 and its three-bit slot index.
    reference_bit = inter["film_grain_start_bit"] + 1 + 16 + 1
    fixtures["malformed_filmgrain_unused_reference_slot.avif"] = replace_field(
        reuse, inter_obu, reference_bit, 3,
        inter["film_grain"]["reference_slot"], unused_slot,
    )
    fixtures[INACTIVE_REUSE_NAME] = inactive_reference_reuse(reuse)
    return fixtures


def write_avif_filmgrain_edge_fixtures(output_dir: Path) -> tuple[Path, ...]:
    """Validate frozen inputs and live outcomes, then write all six AVIF files."""

    require_pinned_oracle()
    fixtures = fixture_bytes()
    repeated = fixture_bytes()
    if fixtures != repeated:
        raise RuntimeError("film-grain header mutations are not repeatable")
    for name, data in fixtures.items():
        if sha256(data) != FIXTURE_SHA256[name]:
            raise RuntimeError(f"{name}: full-file bytes differ from their pinned hash")
        if name in MALFORMED_EXPECTATIONS:
            validate_malformed(name, data)
        elif pillow_frame_hashes(data) != PILLOW_RGB_SHA256[name]:
            raise RuntimeError(f"{name}: live Pillow pixels differ")

    report = inspect_bytes(fixtures[I422_NAME])
    sample = next(sample for sample in report["samples"] if sample["role"] == "item_color")
    sequence = next(obu["sequence_header"] for obu in sample["obus"] if "sequence_header" in obu)
    frame = next(
        obu["frame_header"]
        for obu in sample["obus"]
        if obu.get("frame_header") is not None
    )
    if not (
        sequence["bit_depth"] == 8
        and not sequence["monochrome"]
        and sequence["subsampling_x"]
        and not sequence["subsampling_y"]
        and frame["all_lossless"]
        and frame["film_grain"]["y_points"]
        and all(frame["film_grain"]["uv_points"])
    ):
        raise RuntimeError("I422 input lacks the expected explicit grain syntax")

    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    paths = []
    for name, data in fixtures.items():
        path = output_dir / name
        path.write_bytes(data)
        paths.append(path)
    return tuple(paths)


def main() -> None:
    """Expose the same validated generator used by the repository asset command."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    args = parser.parse_args()
    for path in write_avif_filmgrain_edge_fixtures(args.output_dir):
        print(f"Wrote {path} ({path.stat().st_size} bytes, SHA-256 {FIXTURE_SHA256[path.name]})")


if __name__ == "__main__":
    main()
