#!/usr/bin/env python3
"""Generate complete AVIF containers with truncated sequence-field payloads."""

from __future__ import annotations

import argparse
import hashlib
import struct
from dataclasses import dataclass
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, __version__ as PILLOW_VERSION, features

from avif_bmff_boxes import rewrite_boxes
from generate_avif_obu_header_fixtures import PILLOW_ORACLE
from generate_avif_sequence_header_parity import (
    ILOC_PATH, SOURCE_ILOC, STSZ_PATH, bits_from_bytes, pack_bits, pack_obu,
    parse_obus, unsigned_bits,
)
from inspect_av1_obus import Bits
from inspect_avif_bitstreams import inspect as inspect_container


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/avif"
SOURCE = ASSETS / "animated_decoder_model_without_operating_point_parameters.avif"
SOURCE_SHA256 = "e220130a0614df8c5fce0acf525c2c69a9fccaa8ec140373eac1f91d424d757d"
SOURCE_SAMPLE_SIZES = (53, 113, 5, 30, 25)


@dataclass(frozen=True)
class Source:
    filename: str
    digest: str
    sample_sizes: tuple[int, ...]
    sequence_bytes: int
    sequence_headers: int = 1


DEFAULT_SOURCE = Source(SOURCE.name, SOURCE_SHA256, SOURCE_SAMPLE_SIZES, 28)
SOURCE_ANIMATED = Source(
    "animated.avif",
    "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d",
    (39, 113, 5, 30, 25), 14,
)
SOURCE_DECODER_MODEL_PARAMETERS_CHANGE = Source(
    "animated_decoder_model_parameters_change.avif",
    "9191549ae1373fc31e093a36d9ff2d88525f4219378c7b7ba1ecb6b74cec8396",
    (53, 143, 5, 30, 25), 28, 2,
)
SOURCE_EQUAL_PICTURE_INTERVAL_TWO_TICKS = Source(
    "animated_equal_picture_interval_two_ticks.avif",
    "e49851e6d94c721525f3b4130d7279c2ed2329a68032ea81f3714ae68119db7d",
    (47, 113, 5, 30, 25), 22,
)
SOURCE_INITIAL_DISPLAY_DELAY_10 = Source(
    "animated_initial_display_delay_10.avif",
    "aa95d8f4b6f2576f91a010fd0cc1cd5ed10ea826b89f8aba661c3dd6a9fa1b7d",
    (48, 113, 5, 30, 25), 23,
)
SOURCE_OPERATING_POINT_HIGH_TIER = Source(
    "animated_operating_point_high_tier.avif",
    "bb06afad6c56b58389eede0bdda980ca42ee36ca045eb92e39de7f5a85e19fb0",
    (39, 113, 5, 30, 25), 14,
)
SOURCE_SCREEN_CONTENT_TOOLS_DISABLED = Source(
    "animated_screen_content_tools_disabled.avif",
    "d0c88a39ca31c30f56afa5540505d7e20892ed0d9d5339ff843cdf6d9040a357",
    (2414, 1981, 2120, 1952, 2057), 13,
)


SOURCE_INTEGER_MV_DISABLED = Source(
    "animated_integer_mv_disabled.avif",
    "47b3767081913f190e0473842a466a9a81ae4acce5e5099c323c9793187cdcce",
    (2415, 1981, 2120, 1952, 2057), 14,
)
SOURCE_ERROR_RESILIENT = Source(
    "animated_error_resilient.avif",
    "06ea9771f8b46c3432c6c6cdf324f1c05e86a5fdccd774c8e3c9a8fce0b831f0",
    (41, 46), 13,
)


SOURCE_TWELVE_BIT_420 = Source(
    'animated_lossy_inter_420_12bit_b4x16_q99_16x64.avif',
    'fab0e71887d8e8130e729d9449bd42fe951b045170a66f72db5dfa31eb54dca6',
    (468, 1146), 13, 1,
)

SOURCE_MONOCHROME_SUPERRES = Source(
    'animated_monochrome_neutral_superres_8bit_80x48.avif',
    'b7daf431bafe9eb89a9c2b4f34eda3ac50d8477f9591ec85a8fd4cbf8c398f30',
    (34, 766), 13, 1,
)


@dataclass(frozen=True)
class Recipe:
    field: str
    retained_bytes: int
    digest: str
    source: Source = DEFAULT_SOURCE
    accepted: bool = False
    operating_point_count: int = 1
    ticks_per_picture: int | None = None

    @property
    def filename(self) -> str:
        if self.accepted:
            return f"animated_{self.field}.avif"
        return f"error_sequence_truncated_{self.field}.avif"


RECIPES = (
    Recipe("display_tick", 4, "03982266c35744a8c9aafae3f5a20b50fdcd250469d69ae75fa9607a7e525a02"),
    Recipe("time_scale", 8, "67fe605c4b9b56330cc060306d10130b92bbd0378c10c435878690acb57861cf"),
    Recipe("decoder_model_flag", 9, "69bd4fffece4e801aaf35413ab7e9c3276a87d83659732bc9b6996a2c3311485"),
    Recipe("decoder_tick", 12, "9e108f33e8f98ebc8ebb745354db5e6810575b731f839c8b8c9bdb43d5dbcb07"),
    Recipe("buffer_removal_delay_length", 14, "a963296e04b04b92c61d3d3f5e5a8fbf45e445f6178a8b5f400b996acb963b2a"),
    Recipe("operating_point", 17, "5c15ff0e24fa70c73044a2efc57d555641000d429221aca233e3961b6df5676e"),
    Recipe("frame_width_bits", 18, "f56e82af5610360236bbedbe68d4b4ba91200ff5eed8a7c427fb2925fcf04680"),
    Recipe("maximum_frame_width", 19, "d60ec439294c18c23d56d21f979efe4ffb7f9dcdbdaf89c2550c3e1892267d98"),
    Recipe("frame_bounds", 20, "ae70e71f99b18c53ae79b659652f05a689510dba2246c61c77c5261426324a2b"),
    Recipe("frame_id_numbers_flag", 21, "db063b32bc884e2d1d4890b6bf39869d27b17a6a10cb4d3a5957e661c90279c0"),
    Recipe("order_hint_flag", 22, "547492b8e238e4346e922f8c1d7469d0f217aa53f901425090003eabe934282b"),
    Recipe("color_configuration", 26, "b70962898634caa998a3a20fdfbc4aee11240f333827b2283183caecefed9fc6"),
    Recipe("operating_point_count", 1,
           "020788edfd884b264e68cd108b28a0ae86462ec7fc735caeee21ffa5006e6c7b", source=SOURCE_ANIMATED),
    Recipe("screen_content_tools_flag", 8,
           "1121fe69b9c1655c3e2cc782588ea9dd1a0790db37cd4239b5cf2eb30d008044", source=SOURCE_ANIMATED),
    Recipe("warped_motion_flag", 16,
           "cf6f68053920cfbcfe2f88d767e866e453578dbfa4b9a05de29c453fbfd87b84", source=SOURCE_EQUAL_PICTURE_INTERVAL_TWO_TICKS),
    Recipe("operating_point_display_model_flag", 12,
           "46c46b867e9326c612b4dca266376e3ae9b56dc4b6778717779b2ada112c6779", source=SOURCE_INITIAL_DISPLAY_DELAY_10),
    Recipe("filter_intra_flag", 7,
           "6ed510d8f601d8dc546b226736f63fec24c079c32ceae8634a76f76c6d458a3f", source=SOURCE_OPERATING_POINT_HIGH_TIER),
    Recipe("reference_frame_mvs_flag", 8,
           "2e963376ec192f9fbb5f6c21abc64f208afbf4563bc072aef438dba1f8d14c15", source=SOURCE_OPERATING_POINT_HIGH_TIER),
    Recipe("masked_compound_flag", 22,
           "e9a1f28fb897693f813840b5ef0157224f08d23fd8d7bbcb1fc7279dca229b43", source=SOURCE_DECODER_MODEL_PARAMETERS_CHANGE),
    Recipe("dual_filter_flag", 7,
           "4f1741025c86aa282616fec2ae8c53cc542147343fc3b55368715b80a45386de", source=SOURCE_SCREEN_CONTENT_TOOLS_DISABLED),
    Recipe("sequence_without_extra_zero_padding", 13,
           "94eff1c2a3171c570e8eb9000436ba4f1d2d62cf271b00a602054a689ed25819", source=SOURCE_INTEGER_MV_DISABLED, accepted=True),
    Recipe("joint_compound_flag", 12,
           "d009a1130bb1faa5e27364d08e7232f762b20e5b354c970feaba491bb24941d2", source=SOURCE_ANIMATED, operating_point_count=3),
    Recipe("cdef_flag", 13,
           "451075726dd8d6b431bdf7df8d01c890cf992a756c14e4f4a01402efcea6a71d", source=SOURCE_ANIMATED, operating_point_count=3),
    Recipe("interintra_compound_flag", 22,
           "ce63428a845bffade23f9f08fa6bcbf847daec8a93b77fe9f0eb3a8651d9c3db", source=SOURCE_ANIMATED, operating_point_count=8),
    Recipe("integer_mv_selector", 23,
           "9ae2ac7273e9570278c316e661ffe31848b42f03813027b108ce6e740624e24b", source=SOURCE_ANIMATED, operating_point_count=8),
    Recipe("monochrome_flag", 24,
           "9be8e0885b924f44a36c1dbe8a369e0ab3418bfa32153cbde9d699eb91bf5daf", source=SOURCE_ANIMATED, operating_point_count=8),
    Recipe("screen_content_tools_value", 14,
           "d05b54a00d3b3b7b860b4022c4ec10f6e4b30fb87bb7b79e073a81d05809c19b", source=SOURCE_SCREEN_CONTENT_TOOLS_DISABLED, operating_point_count=4),
    Recipe("delta_frame_id_width", 6,
           "eca158f245799141948211490e72c64c99c5219fbc22889b2021c91cb58ec9cf", source=SOURCE_ERROR_RESILIENT),
    Recipe("additional_frame_id_width", 15,
           "ec450a7afd0f79a66969a9e98930adb877ea4a05a1d8d8656c90bf1489c7764b", source=SOURCE_ERROR_RESILIENT, operating_point_count=5),
    Recipe('frame_presentation_delay_width', 15,
           '2b714076d200ec274c242e5244227b827f01b56019757b7078a7ec7a6abb1dcf', source=SOURCE_DECODER_MODEL_PARAMETERS_CHANGE, operating_point_count=1, ticks_per_picture=2),
    Recipe('decoder_buffer_delay_width', 10,
           '8271821951182c35a492be0f178da0c29621795b508a0156e34ef209fc06812c', source=SOURCE_DECODER_MODEL_PARAMETERS_CHANGE, operating_point_count=1, ticks_per_picture=4),
    Recipe('low_delay_mode_flag', 19,
           '357d89c101f57fc3e19c3c18fd7df4badcf498610fe7081a9f5a3767cfe4e00f', source=SOURCE_DECODER_MODEL_PARAMETERS_CHANGE, operating_point_count=1, ticks_per_picture=8),
    Recipe('initial_display_delay_value', 15,
           '163429f0c27d498bc3136efb1d8fb7fa872f5cd34eab4a8002db5dac28756407', source=SOURCE_INITIAL_DISPLAY_DELAY_10, operating_point_count=2, ticks_per_picture=1),
    Recipe('subsampling_y_flag', 12,
           '25d24d4ff14dd6d08d1932e49d4145afe232587bfa8091e8c9f2d8648b665fca', source=SOURCE_TWELVE_BIT_420, operating_point_count=1),
    Recipe('subsampling_x_flag', 14,
           '51cdaa6b30c50b0d367f7fe952b7aa468df09dea8112015097ea44b7730a06ac', source=SOURCE_TWELVE_BIT_420, operating_point_count=2),
    Recipe('twelve_bit_flag', 19,
           '4689be9776acf48b13c3a15bb08390000ecaf60dbbcec5cc3797b514b00d1f9f', source=SOURCE_TWELVE_BIT_420, operating_point_count=6),
    Recipe('monochrome_color_range_flag', 12,
           'dd699adbdd38c6d3e9b67922ee9dcc47705392c84a9afc24648f08b458b2cb79', source=SOURCE_MONOCHROME_SUPERRES, operating_point_count=1),
)


def repeated_operating_points(
    payload: bytes, count: int, ticks_per_picture: int | None = None,
) -> bytes:
    """Align later syntax using complete operating points and valid timing UVLC."""
    if count == 1 and ticks_per_picture is None:
        return payload
    if not 1 <= count <= 32:
        raise RuntimeError("operating-point count is outside the AV1 limit")
    bits = Bits(payload)
    bits.read(3)  # seq_profile
    bits.bit()  # still_picture
    if bits.bit():
        raise RuntimeError("pinned alignment source requires an ordinary sequence header")
    ticks_start = ticks_end = None
    decoder_model_present = False
    decoder_delay_length = 0
    if bits.bit():
        bits.read(32)  # num_units_in_display_tick
        bits.read(32)  # time_scale
        if bits.bit():
            ticks_start = bits.position
            bits.uvlc()
            ticks_end = bits.position
        decoder_model_present = bool(bits.bit())
        if decoder_model_present:
            decoder_delay_length = bits.read(5) + 1
            bits.read(32)  # num_units_in_decoding_tick
            bits.read(5)  # buffer_removal_time_length_minus_1
            bits.read(5)  # frame_presentation_time_length_minus_1
    display_model_present = bool(bits.bit())
    count_start = bits.position
    if bits.read(5) != 0:
        raise RuntimeError("pinned alignment source no longer has one operating point")
    point_start = bits.position
    bits.read(12)  # operating_point_idc
    if bits.read(5) > 7:
        bits.bit()  # seq_tier
    if decoder_model_present and bits.bit():
        bits.read(decoder_delay_length)
        bits.read(decoder_delay_length)
        bits.bit()  # low_delay_mode_flag
    if display_model_present and bits.bit():
        bits.read(4)  # initial_display_delay_minus_1
    point_end = bits.position
    syntax = bits_from_bytes(payload)
    trailing_one = max(index for index, value in enumerate(syntax) if value)
    syntax = (
        syntax[:count_start] + unsigned_bits(count - 1, 5)
        + syntax[point_start:point_end] * count + syntax[point_end:trailing_one]
    )
    if ticks_per_picture is not None:
        if ticks_start is None or ticks_end is None or ticks_per_picture <= 0:
            raise RuntimeError("timing alignment requires an existing positive picture interval")
        value = unsigned_bits(ticks_per_picture, ticks_per_picture.bit_length())
        syntax = syntax[:ticks_start] + [0] * (len(value) - 1) + value + syntax[ticks_end:]
    return pack_bits(syntax + [1])


def candidate_bytes(recipe: Recipe) -> tuple[bytes, tuple[int, ...]]:
    spec = recipe.source
    source_path = ASSETS / spec.filename
    source = source_path.read_bytes()
    if hashlib.sha256(source).hexdigest() != spec.digest:
        raise RuntimeError("pinned source animation changed")
    samples = next(t for t in inspect_container(source_path)["tracks"] if t["handler"] == "pict")["samples"]
    if tuple(sample["length"] for sample in samples) != spec.sample_sizes:
        raise RuntimeError("pinned sample table changed")
    updated = []
    sequence_count = 0
    for sample in samples:
        output = []
        for header, payload in parse_obus(source[sample["offset"]:sample["offset"] + sample["length"]]):
            if (header >> 3) & 15 == 1:
                if sequence_count == 0:
                    if len(payload) != spec.sequence_bytes:
                        raise RuntimeError("pinned first sequence-header size changed")
                payload = repeated_operating_points(
                    payload, recipe.operating_point_count, recipe.ticks_per_picture,
                )
                if sequence_count == 0:
                    if not 0 < recipe.retained_bytes < len(payload):
                        raise RuntimeError("retained prefix does not truncate the sequence header")
                    payload = payload[:recipe.retained_bytes]
                sequence_count += 1
            output.append(pack_obu(header, payload))
        updated.append(b"".join(output))
    if sequence_count != spec.sequence_headers:
        raise RuntimeError("pinned sequence-header inventory changed")
    sizes = tuple(map(len, updated))

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == STSZ_PATH:
            count = len(spec.sample_sizes)
            expected = bytes(4) + struct.pack(">II", 0, count) + struct.pack(f">{count}I", *spec.sample_sizes)
            if payload != expected:
                raise RuntimeError("pinned stsz layout changed")
            return payload[:12] + struct.pack(f">{count}I", *sizes)
        if path == ILOC_PATH:
            expected = SOURCE_ILOC[:14] + struct.pack(">II", samples[0]["offset"], spec.sample_sizes[0])
            if payload != expected:
                raise RuntimeError("pinned primary-item extent changed")
            return payload[:18] + struct.pack(">I", sizes[0]) + payload[22:]
        if path == (b"mdat",):
            return b"".join(updated)
        return payload

    return rewrite_boxes(source, transform), sizes


def verify_outcome(candidate: bytes, recipe: Recipe) -> None:
    if recipe.accepted:
        with Image.open(BytesIO(candidate)) as image, Image.open(ASSETS / recipe.source.filename) as control:
            if image.n_frames != control.n_frames:
                raise RuntimeError("sequence padding changed the frame count")
            for index in range(image.n_frames):
                image.seek(index)
                control.seek(index)
                image.load()
                control.load()
                if (image.mode, image.size, image.info.get("duration"), image.tobytes()) != (
                    control.mode, control.size, control.info.get("duration"), control.tobytes()
                ):
                    raise RuntimeError("sequence padding changed a complete frame or duration")
        return
    with Image.open(BytesIO(candidate)) as image:
        try:
            image.load()
        except RuntimeError as error:
            if str(error) != "Failed to decode frame 0: Decoding of color planes failed":
                raise RuntimeError("truncated sequence's Pillow failure changed") from error
        else:
            raise RuntimeError("Pillow accepted the truncated sequence header")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if (PILLOW_VERSION, features.version("avif"), _avif.codec_versions()) != PILLOW_ORACLE:
        raise RuntimeError("Pillow oracle differs from its pin")
    for recipe in RECIPES:
        candidate, sizes = candidate_bytes(recipe)
        if hashlib.sha256(candidate).hexdigest() != recipe.digest:
            raise RuntimeError(f"generated bytes differ: {recipe.filename}")
        with Image.open(BytesIO(candidate)) as image:
            image.verify()
        verify_outcome(candidate, recipe)
        path = ASSETS / recipe.filename
        if path.exists():
            if path.read_bytes() != candidate:
                raise RuntimeError(f"refusing to replace differing input: {path}")
        elif args.check:
            raise RuntimeError(f"fixture is missing: {path}")
        else:
            path.write_bytes(candidate)
        rebuilt = inspect_container(path)
        track = next(t for t in rebuilt["tracks"] if t["handler"] == "pict")
        if tuple(s["length"] for s in track["samples"]) != sizes or rebuilt["items"]["color"][0]["length"] != sizes[0]:
            raise RuntimeError("generated container extents differ from bounded samples")
        print(f"Verified {path.relative_to(ROOT)} ({len(candidate)} bytes)")


if __name__ == "__main__":
    main()
