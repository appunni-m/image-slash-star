#!/usr/bin/env python3
"""Generate complete AVIF frame-timing and implicit-layer parity inputs."""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from io import BytesIO
from pathlib import Path
import struct

from PIL import Image, _avif, __version__ as PILLOW_VERSION, features

from avif_bmff_boxes import rewrite_boxes
from generate_avif_decoder_model_parity_fixture import FRAME_SYNTAX
from generate_avif_obu_header_fixtures import PILLOW_ORACLE
from generate_avif_sequence_header_errors import sequence_header_layout
from generate_avif_sequence_header_parity import (
    ILOC_PATH, STSZ_PATH, bits_from_bytes, pack_bits, pack_obu, parse_obus,
    unsigned_bits,
)
from inspect_av1_obus import Bits, parse_sequence_header
from inspect_avif_bitstreams import inspect as inspect_container


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/avif"
TIMING_SOURCE = "animated_decoder_model_without_operating_point_parameters.avif"
SOURCE_HASHES = {
    TIMING_SOURCE: "e220130a0614df8c5fce0acf525c2c69a9fccaa8ec140373eac1f91d424d757d",
    "animated.avif": "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d",
    "animated_decoder_model_parameters_change.avif": "9191549ae1373fc31e093a36d9ff2d88525f4219378c7b7ba1ecb6b74cec8396",
}


@dataclass(frozen=True)
class Recipe:
    name: str
    digest: str
    points: tuple[tuple[int, bool], ...] = ((0, True),)
    width: int = 1
    timestamp: int = 0
    temporal_id: int = 0
    first_implicit: bool = False
    retained_frame_bytes: int | None = None
    source: str = TIMING_SOURCE
    malformed: bool = False


RECIPES = (
    Recipe('animated_buffer_removal_time_zero',
           '3079d08c334b737716c5571317e711130da028fc0d8e6e4a3e9ab85b1d9f7de6',
           points=((0, True),),
           width=1,
           timestamp=0),
    Recipe('animated_buffer_removal_time_one',
           '950f342512092d6cbc051e649acd23bbb9d31d1bbccb43fc5ae87ecb096a5347',
           points=((0, True),),
           width=1,
           timestamp=1),
    Recipe('animated_buffer_removal_time_without_point_parameters',
           '0906822ac392ecb8be54e5306d68690490c1f10db3266551ec45efa74504dbd4',
           points=((0, False),),
           width=1,
           timestamp=0),
    Recipe('animated_buffer_removal_time_mixed_point_parameters',
           'd5348e5c71841e293fbd7aaeea49a222f1d6b26eaa957457596461ec86f293f9',
           points=((0, True), (0, False)),
           width=1,
           timestamp=0),
    Recipe('animated_buffer_removal_time_ignored_temporal_point',
           'adaf92da2b6d3a7336b66020f1ff844f1868e774479170d950fb1bb17cfa8fcb',
           points=((0, True), (258, True)),
           width=1,
           timestamp=0),
    Recipe('animated_buffer_removal_time_ignored_spatial_point',
           '53aed64c97e99732dc538a951234dc42b73136974bdad09cabfebafad107d8a0',
           points=((0, True), (513, True)),
           width=1,
           timestamp=0),
    Recipe('animated_buffer_removal_time_ignored_temporal_spatial_point',
           'a46d624562637894c530d4320012aa233a7082da9d04ec1196e478bbdb4dc044',
           points=((0, True), (514, True)),
           width=1,
           timestamp=0),
    Recipe('animated_buffer_removal_time_two_admitted_points',
           '42a160e63388a938c4534a9463a26b28b7381ceff076675207e21282e342b6c4',
           points=((0, True), (257, True)),
           width=1,
           timestamp=1),
    Recipe('animated_buffer_removal_time_selected_explicit_layer',
           'ab87e6dfdd7d5cc36eccc6a9d579fdbe3cdfc6c32bc69c87617921ca00070b34',
           points=((257, True),),
           width=1,
           timestamp=0),
    Recipe('animated_buffer_removal_time_two_bit',
           'c55d5dd6433ae1fbdb4117f519db7ed0a9d17a8fe394d857c91fb0b061d236a7',
           points=((0, True),),
           width=2,
           timestamp=2),
    Recipe('animated_buffer_removal_time_selected_implicit_layer',
           'b5e3afc51c87eea23087acea01163c0e257d06b94cc3ad0bda64ca87bd1e9fc8',
           points=((257, True),),
           first_implicit=True),
    Recipe('animated_buffer_removal_time_selected_temporal_one',
           '0c5041bcbf6f64652e1d500acaca510ff5f1138f04aca648e13bb3e5426273cd',
           points=((258, True),),
           temporal_id=1),
    Recipe('animated_buffer_removal_time_width32_zero',
           '2433f9fc10478701343ec1b0963200049f1811ece986472a845076914d2a6c10',
           width=32,
           timestamp=0),
    Recipe('animated_buffer_removal_time_width32_maximum',
           'a51a3a5e1c35a87fcfdd6d063fdee046e5e4c77ba74f1679ea81320ba5bdee9d',
           width=32,
           timestamp=0xFFFF_FFFF),
    Recipe('animated_buffer_removal_time_points32_width1_zero',
           'fa4c61d6c8e9df517564fd18d09898d911718b1d2c6ca28f75a8e06375c97398',
           points=((0, True),) * 32),
    Recipe('animated_buffer_removal_time_points32_width1_one',
           'be30f0784f255a61430edf2cf1e3934054aefe1a030c94a0c065db2a5d254e7b',
           points=((0, True),) * 32,
           timestamp=1),
    Recipe('animated_buffer_removal_time_points32_width32_maximum',
           'aaf8d8bf72c07f2c67430f100f68f014dfe65967ad018fce57168319a01ce38b',
           points=((0, True),) * 32,
           width=32,
           timestamp=0xFFFF_FFFF),
    Recipe('animated_buffer_removal_time_points32_mixed_width32_maximum',
           '0394f364d1b98f6f36cd9c25a997634c2efbad7d1363eebdf81508e8880f9965',
           points=tuple((0, index % 2 == 1) for index in range(32)),
           width=32,
           timestamp=0xFFFF_FFFF),
    Recipe('animated_buffer_removal_time_points32_ignored_width32_maximum',
           '3f300b24fc723f708cffa38a992c896dc631628387e1100e6928f195c2a3ce03',
           points=((0, True),) + ((514, True),) * 31,
           width=32,
           timestamp=0xFFFF_FFFF),
    Recipe('animated_buffer_removal_time_all_layers_width32_maximum',
           '0cf2df00e877b1185927730f8a0228a0d1f033dded79fdc14f70aa2214f6a605',
           points=((4095, True),),
           width=32,
           timestamp=0xFFFF_FFFF),
    Recipe('error_buffer_removal_time_excluded_implicit_layer',
           'f8c6a20e2631404c7b6f8a7cf756988edb57fbce19c92066184d4593562a2363',
           points=((258, True),),
           temporal_id=1,
           first_implicit=True,
           malformed=True),
    Recipe('error_buffer_removal_time_truncated_timestamp',
           '328438684d4712bd061cca05ec3b9048100974bd95f88389e8d645837aab73d7',
           width=2,
           timestamp=2,
           retained_frame_bytes=2,
           malformed=True),
    Recipe('error_frame_header_truncated_prefix_0',
           '84187dd295a75f334056d459ebdce37349701133ec95929035de117f75193f32',
           retained_frame_bytes=0,
           source='animated.avif',
           malformed=True),
    Recipe('error_frame_header_truncated_prefix_1',
           'acce4eb0fcaceb7b62635d7ade336bb69a119eadc12a11694b748d368ced3093',
           retained_frame_bytes=1,
           source='animated.avif',
           malformed=True),
    Recipe('error_frame_header_truncated_prefix_2',
           'd392cd28cf17cdbedf626f34a8593a0028c199e5872300972a60c0a260f92b5b',
           retained_frame_bytes=2,
           source='animated.avif',
           malformed=True),
    Recipe('error_frame_header_truncated_prefix_4',
           'cdb26b1e3f9647c6cd980329b427b711096896ef943c40805dc24c7009219953',
           retained_frame_bytes=4,
           source='animated.avif',
           malformed=True),
    Recipe('error_frame_header_truncated_prefix_6',
           'd52e6c0aeabebe34e5ddaa95b0e54bcc78a01ea86b3309b3051f64097fc5625c',
           retained_frame_bytes=6,
           source='animated.avif',
           malformed=True),
    Recipe('error_frame_header_truncated_decoder_model_prefix_2',
           '3b440da70d1776b10883f077e48b81c10b3f030f85afe07779dfae61c6b36929',
           retained_frame_bytes=2,
           source='animated_decoder_model_parameters_change.avif',
           malformed=True),
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def sequence_payload(payload: bytes, recipe: Recipe) -> bytes:
    """Copy complete pinned syntax while replacing timing and operating points."""
    parsed = parse_sequence_header(payload)
    layout = sequence_header_layout(payload)
    require(parsed["decoder_model_present"] and not parsed["display_model_present"],
            "pinned sequence timing changed")
    require(len(parsed["operating_points"]) == 1 and layout["idc"] == 0,
            "pinned operating-point selection changed")
    reader = Bits(payload)
    reader.read(3)
    reader.bit()
    require(not reader.bit() and reader.bit(), "pinned timing flags changed")
    reader.read(32)
    reader.read(32)
    if reader.bit():
        reader.uvlc()
    require(bool(reader.bit()), "pinned decoder-model flag changed")
    reader.read(5)
    reader.read(32)
    width_position = reader.position
    require(reader.read(5) == 0, "pinned timestamp width changed")
    syntax = bits_from_bytes(payload)
    start, end = layout["idc_position"], layout["point_display_position"]
    parameters = syntax[start + 18:end] if syntax[start + 17] else [0, 0, 0]
    require(len(parameters) == 3, "pinned decoder-model parameters changed")
    syntax[width_position:width_position + 5] = unsigned_bits(recipe.width - 1, 5)
    points = []
    for identifier, present in recipe.points:
        points += unsigned_bits(identifier, 12) + syntax[start + 12:start + 17]
        points += [int(present)] + (parameters if present else [])
    syntax_end = max(index for index, bit in enumerate(syntax) if bit)
    result = pack_bits(
        syntax[:start - 5] + unsigned_bits(len(recipe.points) - 1, 5)
        + points + syntax[end:syntax_end] + [1]
    )
    check = parse_sequence_header(result)
    require(check["timing"]["buffer_removal_delay_length"] == recipe.width,
            "generated timestamp width differs")
    require(tuple((point["idc"], point["decoder_model_present"])
                  for point in check["operating_points"]) == recipe.points,
            "generated operating points differ")
    return result


def rewrite_samples(data: bytes, samples: list[dict], updated: list[bytes]) -> bytes:
    """Update bounded sample lengths and the matching primary-item extent."""
    old_sizes = tuple(sample["length"] for sample in samples)
    sizes = tuple(map(len, updated))
    originals = [data[sample["offset"]:sample["offset"] + sample["length"]]
                 for sample in samples]
    count = len(samples)

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == STSZ_PATH:
            require(payload == bytes(4) + struct.pack(">II", 0, count)
                    + struct.pack(f">{count}I", *old_sizes), "pinned sample table changed")
            return payload[:12] + struct.pack(f">{count}I", *sizes)
        if path == ILOC_PATH:
            require(len(payload) == 22 and payload[:14] == bytes.fromhex(
                "0000000044000001000100000001"), "pinned item layout changed")
            require(struct.unpack(">II", payload[14:]) == (samples[0]["offset"], old_sizes[0]),
                    "pinned primary-item extent changed")
            return payload[:18] + struct.pack(">I", sizes[0])
        if path == (b"mdat",):
            require(payload == b"".join(originals), "pinned media samples changed")
            return b"".join(updated)
        return payload

    return rewrite_boxes(data, transform)


def candidate_bytes(recipe: Recipe) -> tuple[bytes, tuple[int, ...]]:
    source = ASSETS / recipe.source
    data = source.read_bytes()
    require(hashlib.sha256(data).hexdigest() == SOURCE_HASHES[recipe.source],
            f"pinned source changed: {source}")
    samples = next(track for track in inspect_container(source)["tracks"]
                   if track["handler"] == "pict")["samples"]
    require(len(samples) == len(FRAME_SYNTAX), "pinned frame count changed")
    updated = []
    first_frame = True
    for sample, layouts in zip(samples, FRAME_SYNTAX, strict=True):
        original = data[sample["offset"]:sample["offset"] + sample["length"]]
        output = []
        frame_index = 0
        for header, payload in parse_obus(original):
            kind = (header >> 3) & 15
            timed = recipe.source == TIMING_SOURCE
            if timed and kind == 1:
                payload = sequence_payload(payload, recipe)
            if timed and kind in (3, 6, 7):
                original_bits, flag, _, _, _ = layouts[frame_index]
                frame_index += 1
                if flag is not None:
                    # The source already inserted the decoder-model presence flag.
                    header_bits = original_bits + 1
                    bits = bits_from_bytes(payload)
                    require(bits[flag] == 0, "pinned timestamp-present flag changed")
                    values = [recipe.timestamp for identifier, present in recipe.points
                              if present and (identifier == 0 or (
                                  identifier & (1 << recipe.temporal_id) and identifier & 0x100))]
                    timestamps = [bit for value in values for bit in unsigned_bits(value, recipe.width)]
                    payload = pack_bits(bits[:flag] + [1] + timestamps + bits[flag + 1:header_bits]) \
                        + payload[(header_bits + 7) // 8:]
            is_first_frame = kind == 6 and first_frame
            if is_first_frame:
                first_frame = False
                if recipe.retained_frame_bytes is not None:
                    require(recipe.retained_frame_bytes < len(payload), "frame truncation exceeds payload")
                    payload = payload[:recipe.retained_frame_bytes]
            encoded = pack_obu(header, payload)
            if timed and recipe.points[0][0] and kind in (3, 4, 6, 7):
                if not (is_first_frame and recipe.first_implicit):
                    encoded = bytes([header | 4, recipe.temporal_id << 5]) + encoded[1:]
            output.append(encoded)
        if timed:
            require(frame_index == len(layouts), "pinned frame-header layout changed")
        updated.append(b"".join(output))
    require(not first_frame, "source has no frame OBU")
    return rewrite_samples(data, samples, updated), tuple(map(len, updated))


def verify_pillow(candidate: bytes, recipe: Recipe) -> None:
    with Image.open(BytesIO(candidate)) as image:
        image.verify()
    with Image.open(BytesIO(candidate)) as image, Image.open(ASSETS / recipe.source) as control:
        if recipe.malformed:
            try:
                image.load()
            except RuntimeError as error:
                require(str(error) == "Failed to decode frame 0: Decoding of color planes failed",
                        f"malformed frame's Pillow failure changed: {error}")
            else:
                raise RuntimeError("Pillow accepted a malformed frame")
            return
        require(image.n_frames == control.n_frames, "timing mutation changed frame count")
        for index in range(image.n_frames):
            image.seek(index)
            control.seek(index)
            image.load()
            control.load()
            require((image.mode, image.size, image.info.get("duration"), image.tobytes()) == (
                control.mode, control.size, control.info.get("duration"), control.tobytes()),
                f"timing mutation changed complete frame {index} or its duration")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    require((PILLOW_VERSION, features.version("avif"), _avif.codec_versions()) == PILLOW_ORACLE,
            "Pillow oracle differs from its pin")
    for recipe in RECIPES:
        candidate, sizes = candidate_bytes(recipe)
        require(hashlib.sha256(candidate).hexdigest() == recipe.digest,
                f"generated bytes differ: {recipe.name}")
        verify_pillow(candidate, recipe)
        path = ASSETS / f"{recipe.name}.avif"
        if path.exists():
            require(path.read_bytes() == candidate, f"refusing to replace differing input: {path}")
        elif args.check:
            raise RuntimeError(f"fixture is missing: {path}")
        else:
            path.write_bytes(candidate)
        container = inspect_container(path)
        samples = next(track for track in container["tracks"] if track["handler"] == "pict")["samples"]
        require(tuple(sample["length"] for sample in samples) == sizes
                and container["items"]["color"][0]["length"] == sizes[0],
                "generated container extents differ from bounded samples")
        print(f"Verified {path.relative_to(ROOT)} ({len(candidate)} bytes)")


if __name__ == "__main__":
    main()
