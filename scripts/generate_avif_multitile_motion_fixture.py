#!/usr/bin/env python3
"""Generate pinned lossy and lossless AVIF motion sequences with two tile columns.

The split input preserves every encoded tile and frame-header field. Its item
extents and track sample tables are repacked together. Observable image results
are compared through live Pillow; hashes identify the generated input bytes.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import tempfile
from io import BytesIO
from pathlib import Path

import yaml
from PIL import Image, ImageDraw

from generate_decode_refs import verify_primary_oracle
from inspect_av1_obus import inspect as inspect_av1
from inspect_avif_bitstreams import (
    children,
    full_box_reader,
    inspect as inspect_container,
    parse_boxes,
    unique_box,
)

ROOT = Path(__file__).resolve().parents[1]
OUTPUT_DIR = ROOT / "tests/fixtures/input/images/avif"
SOURCE_NAME = "animated_motion_multitile.avif"
SPLIT_NAME = "animated_motion_multitile_split_groups.avif"
SOURCE_SHA256 = "c248f019a008e9dea4425e1d5f44d1418b44929604312d95a69a0a3ac5d4709f"
SPLIT_SHA256 = "eb19bcebc0698dfa3149c76fb6828705b22e37f45e213b70541a61b3abd20b89"
LOSSLESS_SOURCE_NAME = "animated_lossless_motion_multitile.avif"
LOSSLESS_SPLIT_NAME = "animated_lossless_motion_multitile_split_groups.avif"
LOSSLESS_SOURCE_SHA256 = "24612186624c6b470a50a549d876469c8265976633afee87fa030d910cd0532b"
LOSSLESS_SPLIT_SHA256 = "04b61cf8b3e9887ee651cec5dc055b5a102198de9f69e2ad64b4e183639cf596"
PROFILES = (
    (80, SOURCE_NAME, SPLIT_NAME, SOURCE_SHA256, SPLIT_SHA256),
    (
        100,
        LOSSLESS_SOURCE_NAME,
        LOSSLESS_SPLIT_NAME,
        LOSSLESS_SOURCE_SHA256,
        LOSSLESS_SPLIT_SHA256,
    ),
)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encode_uleb128(value: int) -> bytes:
    output = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        output.append(byte | (0x80 if value else 0))
        if not value:
            return bytes(output)


def gather_spans(data: bytes, spans: list[dict]) -> bytes:
    return b"".join(
        data[span["offset"] : span["offset"] + span["length"]] for span in spans
    )


def normalize_timestamps(data: bytes) -> bytes:
    """Clear creation/modification times in the parsed version-one movie boxes."""

    output = bytearray(data)
    moov = unique_box(parse_boxes(data, 0, len(data)), b"moov")
    movie_boxes = children(data, moov)
    mvhd = unique_box(movie_boxes, b"mvhd")
    tracks = [box for box in movie_boxes if box.kind == b"trak"]
    if len(tracks) != 1:
        raise RuntimeError("motion sequence must have one color track")
    track_boxes = children(data, tracks[0])
    tkhd = unique_box(track_boxes, b"tkhd")
    mdia = unique_box(track_boxes, b"mdia")
    mdhd = unique_box(children(data, mdia), b"mdhd")
    for box in (mvhd, tkhd, mdhd):
        version, _, reader = full_box_reader(data, box)
        if version != 1:
            raise RuntimeError("motion sequence requires version-one timestamps")
        timestamp_start = reader.offset
        reader.take(16)
        output[timestamp_start : timestamp_start + 16] = bytes(16)
    return bytes(output)


def encode_source(quality: int = 80) -> bytes:
    frames = []
    for index in range(4):
        frame = Image.new("RGB", (256, 128), (32, 64, 96))
        draw = ImageDraw.Draw(frame)
        patch_x = 4 + index
        draw.rectangle((patch_x, 8, patch_x + 7, 15), fill=(240, 180, 32))
        draw.rectangle((patch_x + 2, 10, patch_x + 5, 13), fill=(255, 255, 0))
        frames.append(frame)
    output = BytesIO()
    frames[0].save(
        output,
        format="AVIF",
        save_all=True,
        append_images=frames[1:],
        duration=[100] * len(frames),
        loop=0,
        quality=quality,
        speed=6,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        tile_cols=1,
        tile_rows=0,
        codec="aom",
    )
    return normalize_timestamps(output.getvalue())


def split_sample(data: bytes, sample: dict) -> tuple[tuple[int, int], bytes]:
    """Replace one complete frame OBU with a header and two ordered groups."""

    if len(sample["spans"]) != 1:
        raise RuntimeError("motion source sample must be contiguous")
    span = sample["spans"][0]
    start, length = span["offset"], span["length"]
    if length != sample["length"]:
        raise RuntimeError("motion source span length differs")
    frames = [obu for obu in sample["obus"] if obu["type"] == 6]
    if len(frames) != 1:
        raise RuntimeError("motion source sample must contain one frame OBU")
    frame = frames[0]
    header = frame["frame_header"]
    tiling = header["tiling"]
    group = frame["tile_group"]
    tiles = group["tiles"]
    if (
        tiling["columns"] != 2
        or tiling["rows"] != 1
        or [tile["index"] for tile in tiles] != [0, 1]
        or (group["start"], group["end"]) != (0, 1)
        or frame["has_extension"]
        or not frame["has_size_field"]
        or gather_spans(data, frame["header_spans"])[0] != 0x32
    ):
        raise RuntimeError("motion source frame differs from the two-column layout")

    payload = gather_spans(data, frame["payload_spans"])
    full_bytes, remaining_bits = divmod(header["header_bits"], 8)
    standalone_header = bytearray(payload[:full_bytes])
    if remaining_bits:
        last_byte = payload[full_bytes]
        padding_mask = (1 << (8 - remaining_bits)) - 1
        if last_byte & padding_mask:
            raise RuntimeError("motion source has nonzero frame alignment bits")
        standalone_header.append(last_byte | (1 << (7 - remaining_bits)))
    else:
        standalone_header.append(0x80)
    # Standalone frame headers use trailing_bits; OBU_FRAME uses byte_alignment.
    replacement = bytearray((0x1A,))
    replacement.extend(encode_uleb128(len(standalone_header)))
    replacement.extend(standalone_header)

    tile_index_bits = tiling["log2_columns"] + tiling["log2_rows"]
    group_header_bits = 1 + 2 * tile_index_bits
    if group_header_bits > 8:
        raise RuntimeError("motion source requires a one-byte tile-group header")
    for tile in tiles:
        index = tile["index"]
        group_bits = (1 << (2 * tile_index_bits)) | (index << tile_index_bits) | index
        tile_payload = gather_spans(data, tile["physical_spans"])
        if len(tile_payload) != tile["length"]:
            raise RuntimeError("motion source tile span length differs")
        group_payload = bytes((group_bits << (8 - group_header_bits),)) + tile_payload
        replacement.append(0x22)
        replacement.extend(encode_uleb128(len(group_payload)))
        replacement.extend(group_payload)

    frame_start = frame["header_spans"][0]["offset"]
    final_span = frame["payload_spans"][-1]
    frame_end = final_span["offset"] + final_span["length"]
    if not start <= frame_start < frame_end <= start + length:
        raise RuntimeError("motion frame bounds exceed its sample")
    encoded = data[start:frame_start] + replacement + data[frame_end : start + length]
    return (start, length), bytes(encoded)


def repack_sequence(data: bytes, report: dict, container: dict) -> bytes:
    replacements = {}
    for sample in report["samples"]:
        key, value = split_sample(data, sample)
        if key in replacements and replacements[key] != value:
            raise RuntimeError("shared item/track sample replacements differ")
        replacements[key] = value
    ranges = sorted(replacements)
    for previous, following in zip(ranges, ranges[1:]):
        if previous[0] + previous[1] > following[0]:
            raise RuntimeError("motion source sample ranges partially overlap")

    top = parse_boxes(data, 0, len(data))
    mdat = unique_box(top, b"mdat")
    if mdat.end != len(data) or mdat.header_size != 8:
        raise RuntimeError("motion source requires a terminal ordinary mdat")
    if not all(
        mdat.payload_start <= start < start + length <= mdat.end
        for start, length in ranges
    ):
        raise RuntimeError("motion source replacement lies outside mdat")
    prefix = bytearray(data[: mdat.payload_start])
    deltas = {key: len(replacements[key]) - key[1] for key in ranges}

    def shifted(offset: int) -> int:
        for start, length in ranges:
            if start < offset < start + length:
                raise RuntimeError("motion metadata offset enters a replaced sample")
        return offset + sum(
            deltas[key] for key in ranges if key[0] + key[1] <= offset
        )

    def patch(position: int, width: int, value: int) -> None:
        prefix[position : position + width] = value.to_bytes(width, "big")

    meta = unique_box(top, b"meta")
    iloc = unique_box(children(data, meta, 4), b"iloc")
    version, _, reader = full_box_reader(data, iloc)
    sizes = reader.u16()
    offset_width = sizes >> 12
    length_width = (sizes >> 8) & 0xF
    base_width = (sizes >> 4) & 0xF
    index_width = sizes & 0xF if version in (1, 2) else 0
    if version not in (0, 1, 2) or any(
        width not in (0, 4, 8)
        for width in (offset_width, length_width, base_width, index_width)
    ):
        raise RuntimeError("motion source iloc layout is unsupported")
    item_count = reader.u16() if version < 2 else reader.u32()
    for _ in range(item_count):
        reader.u16() if version < 2 else reader.u32()
        method = reader.u16() if version in (1, 2) else 0
        if method != 0 or reader.u16() != 0:
            raise RuntimeError("motion source requires internal file-backed extents")
        base_offset = reader.uint(base_width)
        for _ in range(reader.u16()):
            if index_width:
                reader.uint(index_width)
            offset_position = reader.offset
            extent_offset = reader.uint(offset_width)
            length_position = reader.offset
            extent_length = reader.uint(length_width)
            absolute = base_offset + extent_offset
            key = (absolute, extent_length)
            if key not in replacements:
                raise RuntimeError("motion item extent does not match a complete sample")
            patch(offset_position, offset_width, shifted(absolute) - base_offset)
            patch(length_position, length_width, len(replacements[key]))
    if reader.offset != reader.end:
        raise RuntimeError("motion source has trailing iloc bytes")

    moov = unique_box(top, b"moov")
    tracks = [box for box in children(data, moov) if box.kind == b"trak"]
    if len(tracks) != len(container["tracks"]):
        raise RuntimeError("motion source track count differs")
    for track, track_report in zip(tracks, container["tracks"]):
        mdia = unique_box(children(data, track), b"mdia")
        minf = unique_box(children(data, mdia), b"minf")
        stbl = unique_box(children(data, minf), b"stbl")
        table = children(data, stbl)
        stsz = unique_box(table, b"stsz")
        version, _, reader = full_box_reader(data, stsz)
        common_size = reader.u32()
        sample_count = reader.u32()
        if (
            version != 0
            or common_size != 0
            or sample_count != len(track_report["samples"])
        ):
            raise RuntimeError("motion source requires explicit per-sample stsz entries")
        for sample in track_report["samples"]:
            size_position = reader.offset
            original_size = reader.u32()
            key = (sample["offset"], sample["length"])
            if original_size != key[1] or key not in replacements:
                raise RuntimeError("motion track size differs from its replacement")
            patch(size_position, 4, len(replacements[key]))
        if reader.offset != reader.end:
            raise RuntimeError("motion source has trailing stsz bytes")
        offset_boxes = [box for box in table if box.kind in (b"stco", b"co64")]
        if len(offset_boxes) != 1:
            raise RuntimeError("motion source requires one track chunk-offset table")
        box = offset_boxes[0]
        width = 4 if box.kind == b"stco" else 8
        version, _, reader = full_box_reader(data, box)
        if version != 0:
            raise RuntimeError("motion chunk-offset version is unsupported")
        for _ in range(reader.u32()):
            offset_position = reader.offset
            chunk_offset = reader.uint(width)
            patch(offset_position, width, shifted(chunk_offset))
        if reader.offset != reader.end:
            raise RuntimeError("motion source has trailing chunk-offset bytes")

    payload = bytearray()
    position = mdat.payload_start
    for key in ranges:
        start, length = key
        payload.extend(data[position:start])
        payload.extend(replacements[key])
        position = start + length
    payload.extend(data[position : mdat.end])
    patch(mdat.start, 4, mdat.header_size + len(payload))
    result = bytes(prefix + payload)
    if len(result) != len(data) + sum(deltas.values()):
        raise RuntimeError("repacked motion sequence length differs")
    return result


def pillow_observations(data: bytes) -> list[tuple]:
    observations = []
    with Image.open(BytesIO(data)) as image:
        if image.n_frames != 4:
            raise RuntimeError("motion sequence must expose four Pillow frames")
        for index in range(image.n_frames):
            image.seek(index)
            image.load()
            if image.mode != "RGB" or image.size != (256, 128):
                raise RuntimeError("motion sequence Pillow mode or dimensions differ")
            observations.append(
                (image.format, image.mode, image.size, image.info.copy(), image.tobytes())
            )
    return observations


def verify_split(source: bytes, output: bytes, source_report: dict, output_path: Path) -> None:
    report = inspect_av1(output_path)
    if len(report["samples"]) != len(source_report["samples"]):
        raise RuntimeError("split motion sample count differs")
    for original, sample in zip(source_report["samples"], report["samples"]):
        if (original["role"], original["identity"]) != (
            sample["role"],
            sample["identity"],
        ):
            raise RuntimeError("split motion sample identity differs")
        types = [obu["type"] for obu in sample["obus"]]
        if 6 in types or types[-3:] != [3, 4, 4]:
            raise RuntimeError("split motion OBU sequence differs")
        groups = [obu["tile_group"] for obu in sample["obus"] if obu["type"] == 4]
        if [(group["start"], group["end"]) for group in groups] != [(0, 0), (1, 1)]:
            raise RuntimeError("split motion tile-group ranges differ")
        original_frame = next(obu for obu in original["obus"] if obu["type"] == 6)
        split_header = next(obu["frame_header"] for obu in sample["obus"] if obu["type"] == 3)
        if original_frame["frame_header"] != split_header:
            raise RuntimeError("split motion frame-header fields differ")
        original_tiles = original_frame["tile_group"]["tiles"]
        split_tiles = [tile for group in groups for tile in group["tiles"]]
        if len(original_tiles) != len(split_tiles):
            raise RuntimeError("split motion tile count differs")
        for before, after in zip(original_tiles, split_tiles):
            if before["index"] != after["index"] or gather_spans(
                source, before["physical_spans"]
            ) != gather_spans(output, after["physical_spans"]):
                raise RuntimeError("split motion encoded tile bytes differ")
    if pillow_observations(source) != pillow_observations(output):
        raise RuntimeError("split motion sequence changed Pillow pixels or metadata")


def generate_fixtures(output_dir: Path = OUTPUT_DIR) -> dict[str, str]:
    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    output_dir.mkdir(parents=True, exist_ok=True)
    hashes = {}
    with tempfile.TemporaryDirectory(prefix=".avif-multitile-motion-", dir=output_dir) as temporary:
        work = Path(temporary)
        for quality, source_name, split_name, source_hash, split_hash in PROFILES:
            source = encode_source(quality)
            if source != encode_source(quality):
                raise RuntimeError(
                    f"Q{quality} multi-tile motion AVIF encoding is not deterministic"
                )
            if digest(source) != source_hash:
                raise RuntimeError(
                    f"Q{quality} multi-tile motion AVIF source hash differs: {digest(source)}"
                )
            source_path = work / source_name
            source_path.write_bytes(source)
            source_report = inspect_av1(source_path)
            output = repack_sequence(source, source_report, inspect_container(source_path))
            if digest(output) != split_hash:
                raise RuntimeError(
                    f"Q{quality} split multi-tile motion AVIF hash differs: {digest(output)}"
                )
            split_path = work / split_name
            split_path.write_bytes(output)
            verify_split(source, output, source_report, split_path)
            hashes.update({source_name: source_hash, split_name: split_hash})
        # Verify both quality profiles before publishing any input.
        for name in hashes:
            os.replace(work / name, output_dir / name)
    return hashes


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT_DIR)
    args = parser.parse_args()
    for name, source_hash in generate_fixtures(args.output_dir).items():
        print(f"Generated {name}: {source_hash}")


if __name__ == "__main__":
    main()
