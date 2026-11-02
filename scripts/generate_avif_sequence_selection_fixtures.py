#!/usr/bin/env python3
"""Generate AVIF sequence track-selection parity inputs from one pinned source."""

from __future__ import annotations

import argparse
import hashlib
import struct
from io import BytesIO
from pathlib import Path

from PIL import Image, UnidentifiedImageError

from inspect_avif_bitstreams import children, parse_boxes, parse_iloc, parse_track, unique_box


ROOT = Path(__file__).resolve().parent.parent
FIXTURE_DIR = ROOT / "tests/fixtures/input/images/avif"
SOURCE_PATH = FIXTURE_DIR / "10bit.avif"
SOURCE_SHA256 = "3bf9f91da471749e7df639ba7945d4d94c1c3e3968c26f3619fbbcfc92790576"
FIXTURES = {
    "duplicate_alpha_track": (
        "animated_duplicate_alpha_track.avif",
        "a1635ad5da1194bed67af764f88f49801cc4afbe4711c71eb9b59404eca4c89a",
    ),
    "no_color_track_candidate": (
        "error_sequence_no_color_track_candidate.avif",
        "8cde6b3f382eb4bc9b19b9ebbb2256a42d2d98f2f2dc607ab1cf1884deacd5b8",
    ),
}


def _make_box(kind: bytes, payload: bytes) -> bytes:
    size = len(payload) + 8
    if size > 0xFFFF_FFFF:
        raise ValueError("mutated AVIF box exceeds the 32-bit box-size limit")
    return struct.pack(">I", size) + kind + payload


def _track_boxes(data: bytes, movie):
    tracks = [box for box in children(data, movie) if box.kind == b"trak"]
    if not tracks:
        raise ValueError("pinned AVIF has no sequence tracks")
    return tracks


def _shift_file_item_extents(data: bytearray, amount: int) -> None:
    top_level = parse_boxes(data, 0, len(data))
    meta = unique_box(top_level, b"meta")
    assert meta is not None
    iloc = unique_box(children(data, meta, prefix=4), b"iloc")
    assert iloc is not None

    before = parse_iloc(data, iloc, None)
    payload = iloc.payload_start
    version = data[payload]
    sizes = struct.unpack_from(">H", data, payload + 4)[0]
    offset_size = (sizes >> 12) & 0xF
    length_size = (sizes >> 8) & 0xF
    base_offset_size = (sizes >> 4) & 0xF
    index_size = (sizes & 0xF) if version in (1, 2) else 0
    cursor = payload + 6
    item_count_size = 4 if version == 2 else 2
    item_count = int.from_bytes(data[cursor : cursor + item_count_size], "big")
    cursor += item_count_size

    for _ in range(item_count):
        cursor += 4 if version == 2 else 2
        construction_method = 0
        if version in (1, 2):
            construction_method = struct.unpack_from(">H", data, cursor)[0] & 0xF
            cursor += 2
        cursor += 2 + base_offset_size
        extent_count = struct.unpack_from(">H", data, cursor)[0]
        cursor += 2
        for _ in range(extent_count):
            cursor += index_size
            if construction_method == 0 and offset_size:
                old_offset = int.from_bytes(data[cursor : cursor + offset_size], "big")
                new_offset = old_offset + amount
                if new_offset >= 1 << (8 * offset_size):
                    raise ValueError("shifted iloc extent exceeds its encoded width")
                data[cursor : cursor + offset_size] = new_offset.to_bytes(
                    offset_size, "big"
                )
            cursor += offset_size + length_size

    after = parse_iloc(data, iloc, None)
    expected = {
        item_id: (
            source_kind,
            [
                (start + amount, end + amount) if source_kind == "file" else (start, end)
                for start, end in extents
            ],
        )
        for item_id, (source_kind, extents) in before.items()
    }
    if after != expected:
        raise ValueError("AVIF iloc rebasing changed unexpected item extents")


def shift_track_chunk_offsets(data: bytearray, amount: int) -> None:
    top_level = parse_boxes(data, 0, len(data))
    movie = unique_box(top_level, b"moov")
    assert movie is not None
    for track in _track_boxes(data, movie):
        mdia = unique_box(children(data, track), b"mdia")
        assert mdia is not None
        minf = unique_box(children(data, mdia), b"minf")
        assert minf is not None
        stbl = unique_box(children(data, minf), b"stbl")
        assert stbl is not None
        offset_tables = [
            box for box in children(data, stbl) if box.kind in (b"stco", b"co64")
        ]
        if len(offset_tables) != 1:
            raise ValueError("each AVIF track must contain one chunk-offset table")
        table = offset_tables[0]
        width = 4 if table.kind == b"stco" else 8
        count = struct.unpack_from(">I", data, table.payload_start + 4)[0]
        for index in range(count):
            offset = table.payload_start + 8 + index * width
            value = int.from_bytes(data[offset : offset + width], "big") + amount
            if value >= 1 << (8 * width):
                raise ValueError("shifted chunk offset exceeds its encoded width")
            data[offset : offset + width] = value.to_bytes(width, "big")


def duplicate_alpha_track(source: bytes) -> bytes:
    """Append a distinct track ID with the same alpha relation and samples."""
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise ValueError("high-bit-depth AVIF source differs from its pinned input")
    top_level = parse_boxes(source, 0, len(source))
    if [box.kind for box in top_level] != [b"ftyp", b"meta", b"moov", b"mdat"]:
        raise ValueError("pinned high-bit-depth AVIF top-level layout changed")
    movie = unique_box(top_level, b"moov")
    assert movie is not None
    tracks = _track_boxes(source, movie)
    track_info = [(track, parse_track(source, track)) for track in tracks]
    colors = [
        info
        for _, info in track_info
        if info["handler"] in ("pict", "vide") and info["aux_for_track_id"] is None
    ]
    alphas = [
        (track, info)
        for track, info in track_info
        if info["handler"] == "auxv" and info["aux_for_track_id"] == 1
    ]
    ids = [info["track_id"] for _, info in track_info]
    if len(colors) != 1 or len(alphas) != 1 or sorted(ids) != [1, 2]:
        raise ValueError("pinned high-bit-depth track layout changed")

    alpha_box, alpha_info = alphas[0]
    track_bytes = bytearray(source[alpha_box.start : alpha_box.end])
    cloned_track = parse_boxes(track_bytes, 0, len(track_bytes))[0]
    tkhd = unique_box(children(track_bytes, cloned_track), b"tkhd")
    assert tkhd is not None
    version = track_bytes[tkhd.payload_start]
    track_id_offset = tkhd.payload_start + (20 if version == 1 else 12)
    if version not in (0, 1):
        raise ValueError(f"unsupported cloned tkhd version {version}")
    if int.from_bytes(track_bytes[track_id_offset : track_id_offset + 4], "big") != 2:
        raise ValueError("source alpha track ID changed")
    struct.pack_into(">I", track_bytes, track_id_offset, 3)

    growth = len(track_bytes)
    new_movie_payload = source[movie.payload_start : movie.end] + bytes(track_bytes)
    candidate = bytearray(
        source[: movie.start]
        + _make_box(b"moov", new_movie_payload)
        + source[movie.end :]
    )
    _shift_file_item_extents(candidate, growth)
    shift_track_chunk_offsets(candidate, growth)

    updated_top = parse_boxes(candidate, 0, len(candidate))
    updated_movie = unique_box(updated_top, b"moov")
    updated_mdat = unique_box(updated_top, b"mdat")
    source_mdat = unique_box(top_level, b"mdat")
    assert updated_movie is not None and updated_mdat is not None and source_mdat is not None
    updated_tracks = _track_boxes(candidate, updated_movie)
    updated_info = [parse_track(candidate, track) for track in updated_tracks]
    if [info["track_id"] for info in updated_info] != [1, 2, 3]:
        raise ValueError("duplicate-alpha mutation changed track ordering or IDs")
    cloned_info = updated_info[2]
    if (
        cloned_info["handler"] != "auxv"
        or cloned_info["aux_for_track_id"] != 1
        or cloned_info["samples"] != [
            {**sample, "offset": sample["offset"] + growth}
            for sample in alpha_info["samples"]
        ]
    ):
        raise ValueError("cloned alpha track differs beyond its ID and file offsets")
    if candidate[updated_mdat.payload_start : updated_mdat.end] != source[
        source_mdat.payload_start : source_mdat.end
    ]:
        raise ValueError("duplicate-alpha mutation changed media payload bytes")

    result = bytes(candidate)
    if hashlib.sha256(result).hexdigest() != FIXTURES["duplicate_alpha_track"][1]:
        raise ValueError("duplicate-alpha fixture differs from its pinned mutation")
    return result


def no_color_track_candidate(source: bytes) -> bytes:
    """Hide the color handler and codec config so sequence fallback rejects it."""
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise ValueError("high-bit-depth AVIF source differs from its pinned input")
    output = bytearray(source)
    top_level = parse_boxes(output, 0, len(output))
    movie = unique_box(top_level, b"moov")
    assert movie is not None
    color_tracks = [
        track
        for track in _track_boxes(output, movie)
        if parse_track(output, track)["handler"] in ("pict", "vide")
    ]
    if len(color_tracks) != 1:
        raise ValueError("pinned high-bit-depth AVIF must have one color track")
    mdia = unique_box(children(output, color_tracks[0]), b"mdia")
    assert mdia is not None
    hdlr = unique_box(children(output, mdia), b"hdlr")
    minf = unique_box(children(output, mdia), b"minf")
    assert hdlr is not None and minf is not None
    stbl = unique_box(children(output, minf), b"stbl")
    assert stbl is not None
    stsd = unique_box(children(output, stbl), b"stsd")
    assert stsd is not None
    entries = parse_boxes(output, stsd.payload_start + 8, stsd.end)
    if len(entries) != 1 or entries[0].kind != b"av01":
        raise ValueError("pinned color sample description changed")
    sample_type_offset = entries[0].start + 4
    handler_type_offset = hdlr.payload_start + 8
    if output[handler_type_offset : handler_type_offset + 4] != b"pict":
        raise ValueError("pinned color handler changed")
    output[handler_type_offset : handler_type_offset + 4] = b"soun"
    output[sample_type_offset : sample_type_offset + 4] = b"zzzz"

    result = bytes(output)
    if hashlib.sha256(result).hexdigest() != FIXTURES["no_color_track_candidate"][1]:
        raise ValueError("no-color-track fixture differs from its pinned mutation")
    parsed_color = parse_track(result, color_tracks[0])
    if parsed_color["handler"] != "soun" or parsed_color["av1c"] is not None:
        raise ValueError("no-color-track mutation retained a color candidate")
    return result


def _pillow_frames(data: bytes):
    with Image.open(BytesIO(data)) as image:
        frames = []
        for frame_index in range(image.n_frames):
            image.seek(frame_index)
            frames.append((image.info.get("duration"), image.convert("RGBA").tobytes()))
        return image.format, image.size, image.mode, frames


def verify_oracle(source: bytes, candidate: bytes, case: str) -> None:
    if Image.__version__ != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {Image.__version__}")
    if case == "duplicate_alpha_track":
        if _pillow_frames(candidate) != _pillow_frames(source):
            raise RuntimeError("duplicate alpha track changed Pillow frames or timing")
        return
    if case == "no_color_track_candidate":
        try:
            Image.open(BytesIO(candidate))
        except UnidentifiedImageError:
            return
        raise RuntimeError("Pillow unexpectedly opened the no-color-track fixture")
    raise ValueError(f"unknown fixture case: {case}")


def generate(case: str, *, check: bool = False) -> Path:
    source = SOURCE_PATH.read_bytes()
    generators = {
        "duplicate_alpha_track": duplicate_alpha_track,
        "no_color_track_candidate": no_color_track_candidate,
    }
    candidate = generators[case](source)
    verify_oracle(source, candidate, case)
    filename, expected_sha256 = FIXTURES[case]
    destination = FIXTURE_DIR / filename
    if check:
        if not destination.is_file() or destination.read_bytes() != candidate:
            raise RuntimeError(f"{destination} differs from the pinned {case} fixture")
    else:
        destination.write_bytes(candidate)
    if hashlib.sha256(candidate).hexdigest() != expected_sha256:
        raise RuntimeError(f"{case} fixture SHA-256 changed")
    return destination


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", choices=(*FIXTURES, "all"), default="all")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    cases = FIXTURES if args.case == "all" else (args.case,)
    for case in cases:
        path = generate(case, check=args.check)
        print(f"{'Verified' if args.check else 'Generated'} {path}")


if __name__ == "__main__":
    main()
