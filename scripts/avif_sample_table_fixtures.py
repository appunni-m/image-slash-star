"""Reproducible AVIF sample-table timing mutations."""

from __future__ import annotations

import hashlib
import struct
from pathlib import Path

from inspect_avif_bitstreams import children, parse_boxes, parse_track, unique_box


ROOT = Path(__file__).resolve().parent.parent
SOURCE_PATH = ROOT / "tests/fixtures/input/images/avif/animated.avif"
OUTPUT_PATH = ROOT / "tests/fixtures/input/images/avif/animated_stts_zero_delta.avif"
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
OUTPUT_SHA256 = "5b2c438346c66b7ee1cb165742e639bc98e3180d5da42dd8e479f48b99598105"
ALPHA_TIMING_SOURCE_SHA256 = "3bf9f91da471749e7df639ba7945d4d94c1c3e3968c26f3619fbbcfc92790576"
ALPHA_TIMING_VARIANT_SHA256 = "b70e900cbe163b4e394aaa0bab6f283fec8f7f0093cc8dbf9ea790b85103902d"
STTS_BOX_OFFSET = 883
STTS_ENTRY_COUNT_OFFSET = 895
STTS_SAMPLE_COUNT_OFFSET = 899
SAMPLE_DELTA_OFFSET = 903
STTS_SECOND_RUN_OFFSET = 907
STCO_CHUNK_OFFSET_OFFSET = 991
ILOC_EXTENT_OFFSET_OFFSET = 132
STTS_VARIANT_SHA256 = {
    "stts_empty": "c23b9d465406ba51f46d5fd6c3cf8d7a71dde68110bb0ccc7512d48977208b30",
    "stts_zero_sample_count": "2e7fb956b121ced30abf59cc70a30fd7dc2dd8146484252883553651a86107d6",
    "stts_underfilled": "0c8b7e231c455711581f0eed1272f2be2b32a764fc9eb782c955678ba64e9738",
    "stts_overfilled": "fecedb638aaad110abfa0234edad9850ffaee9452b1633703b2426653d6e0bba",
    "stts_wrapped_sample_count": "877d9aa62c8d5ad750d6f4c60a293a2025fbe6ade96f4b8957301a73c37a4e1b",
}


def _validate_source(source: bytes):
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise ValueError("animated AVIF source differs from its pinned input")

    top_level = parse_boxes(source, 0, len(source))
    movie = unique_box(top_level, b"moov")
    assert movie is not None
    tracks = [box for box in children(source, movie) if box.kind == b"trak"]
    if len(tracks) != 1:
        raise ValueError("animated AVIF must contain exactly one movie track")

    media = unique_box(children(source, tracks[0]), b"mdia")
    assert media is not None
    media_info = unique_box(children(source, media), b"minf")
    assert media_info is not None
    sample_table = unique_box(children(source, media_info), b"stbl")
    assert sample_table is not None
    sample_table_children = children(source, sample_table)
    timing = unique_box(sample_table_children, b"stts")
    stco = unique_box(sample_table_children, b"stco")
    meta = unique_box(top_level, b"meta")
    assert timing is not None
    assert stco is not None
    assert meta is not None
    iloc = unique_box(children(source, meta, prefix=4), b"iloc")
    assert iloc is not None
    if timing.start != STTS_BOX_OFFSET or timing.size != 24:
        raise ValueError("animated AVIF stts box bounds differ from the pinned layout")
    if stco.start != 975 or iloc.start != 110:
        raise ValueError("animated AVIF item or chunk offset layout differs from the pinned source")

    payload = timing.payload_start
    version_and_flags = int.from_bytes(source[payload : payload + 4], "big")
    entry_count = int.from_bytes(source[payload + 4 : payload + 8], "big")
    sample_count = int.from_bytes(source[payload + 8 : payload + 12], "big")
    sample_delta = int.from_bytes(source[payload + 12 : payload + 16], "big")
    if (
        version_and_flags != 0
        or entry_count != 1
        or sample_count != 5
        or sample_delta != 1
        or payload + 12 != SAMPLE_DELTA_OFFSET
        or payload + 4 != STTS_ENTRY_COUNT_OFFSET
        or payload + 8 != STTS_SAMPLE_COUNT_OFFSET
    ):
        raise ValueError("animated AVIF stts entry differs from the pinned source")
    return timing, (sample_table, media_info, media, tracks[0], movie), stco, iloc


def zero_stts_sample_delta(source: bytes) -> bytes:
    """Set the sole five-sample `stts` delta to zero without changing box sizes."""
    _validate_source(source)

    output = bytearray(source)
    output[SAMPLE_DELTA_OFFSET : SAMPLE_DELTA_OFFSET + 4] = bytes(4)
    result = bytes(output)
    if len(result) != len(source) or hashlib.sha256(result).hexdigest() != OUTPUT_SHA256:
        raise ValueError("zero-delta AVIF output differs from its pinned mutation")
    changed = [
        index
        for index, (before, after) in enumerate(zip(source, result))
        if before != after
    ]
    if changed != [SAMPLE_DELTA_OFFSET + 3]:
        raise ValueError("zero-delta mutation changed bytes outside the sample_delta field")
    return result


def alpha_track_duration_mismatch(source: bytes) -> bytes:
    """Change the pinned sequence's auxiliary alpha sample delta from 1 to 2."""
    if hashlib.sha256(source).hexdigest() != ALPHA_TIMING_SOURCE_SHA256:
        raise ValueError("AVIF high-bit-depth sequence source differs from its pin")

    movie = unique_box(parse_boxes(source, 0, len(source)), b"moov")
    tracks = [
        (track, parse_track(source, track))
        for track in children(source, movie)
        if track.kind == b"trak"
    ]
    color_track_ids = {
        track_info["track_id"]
        for _, track_info in tracks
        if track_info["handler"] in ("pict", "vide")
    }
    alpha_tracks = [
        track
        for track, track_info in tracks
        if track_info["handler"] == "auxv"
        and track_info["aux_for_track_id"] in color_track_ids
    ]
    if len(alpha_tracks) != 1:
        raise ValueError(
            f"expected one auxiliary alpha track, found {len(alpha_tracks)}"
        )

    alpha_track_boxes = children(source, alpha_tracks[0])
    alpha_mdia = unique_box(alpha_track_boxes, b"mdia")
    alpha_minf = unique_box(children(source, alpha_mdia), b"minf")
    alpha_stbl = unique_box(children(source, alpha_minf), b"stbl")
    alpha_stts = unique_box(children(source, alpha_stbl), b"stts")
    expected_timing = bytes.fromhex("00000000000000010000000500000001")
    if source[alpha_stts.payload_start : alpha_stts.end] != expected_timing:
        raise ValueError("AVIF alpha timing source differs from its pinned table")

    output = bytearray(source)
    struct.pack_into(">I", output, alpha_stts.payload_start + 12, 2)
    result = bytes(output)
    if len(result) != len(source) or hashlib.sha256(result).hexdigest() != (
        ALPHA_TIMING_VARIANT_SHA256
    ):
        raise ValueError("AVIF alpha timing variant differs from its pinned output")
    changed = [
        index
        for index, (before, after) in enumerate(zip(source, result))
        if before != after
    ]
    if changed != [alpha_stts.payload_start + 15]:
        raise ValueError("AVIF alpha timing mutation changed unexpected bytes")
    return result


def stts_timing_variants(source: bytes) -> dict[str, bytes]:
    """Build pinned incomplete-table and wrapped-count `stts` variants."""
    timing, enclosing_boxes, stco, iloc = _validate_source(source)
    variants: dict[str, bytes] = {}
    for name, sample_count in (
        ("stts_zero_sample_count", 0),
        ("stts_underfilled", 4),
        ("stts_overfilled", 6),
    ):
        output = bytearray(source)
        output[STTS_SAMPLE_COUNT_OFFSET : STTS_SAMPLE_COUNT_OFFSET + 4] = (
            sample_count.to_bytes(4, "big")
        )
        result = bytes(output)
        if hashlib.sha256(result).hexdigest() != STTS_VARIANT_SHA256[name]:
            raise ValueError(f"{name} AVIF mutation differs from its pinned output")
        changed = [
            index
            for index, (before, after) in enumerate(zip(source, result))
            if before != after
        ]
        if changed != [STTS_SAMPLE_COUNT_OFFSET + 3]:
            raise ValueError(f"{name} mutation changed bytes outside sample_count")
        variants[name] = result

    empty = bytearray(source)
    empty[STTS_BOX_OFFSET : STTS_BOX_OFFSET + 4] = (16).to_bytes(4, "big")
    empty[STTS_ENTRY_COUNT_OFFSET : STTS_ENTRY_COUNT_OFFSET + 4] = bytes(4)
    empty[STTS_SAMPLE_COUNT_OFFSET : STTS_SAMPLE_COUNT_OFFSET + 8] = b"\x00\x00\x00\x08free"
    empty_result = bytes(empty)
    if hashlib.sha256(empty_result).hexdigest() != STTS_VARIANT_SHA256["stts_empty"]:
        raise ValueError("empty `stts` AVIF mutation differs from its pinned output")
    variants["stts_empty"] = empty_result

    wrapped = bytearray(source)
    stco_offset = int.from_bytes(
        source[STCO_CHUNK_OFFSET_OFFSET : STCO_CHUNK_OFFSET_OFFSET + 4], "big"
    )
    if stco_offset != 1023:
        raise ValueError("animated AVIF stco offset differs from the pinned source")
    iloc_offset = int.from_bytes(
        source[ILOC_EXTENT_OFFSET_OFFSET : ILOC_EXTENT_OFFSET_OFFSET + 4], "big"
    )
    if iloc_offset != 1023:
        raise ValueError("animated AVIF iloc extent differs from the pinned source")
    if (stco.start, iloc.start, timing.end) != (975, 110, STTS_SECOND_RUN_OFFSET):
        raise ValueError(
            "animated AVIF wrapped-count layout differs from the pinned source"
        )

    wrapped[STTS_BOX_OFFSET : STTS_BOX_OFFSET + 4] = (timing.size + 8).to_bytes(4, "big")
    wrapped[STTS_ENTRY_COUNT_OFFSET : STTS_ENTRY_COUNT_OFFSET + 4] = (2).to_bytes(4, "big")
    wrapped[STTS_SAMPLE_COUNT_OFFSET : STTS_SAMPLE_COUNT_OFFSET + 4] = (1).to_bytes(4, "big")
    for box in enclosing_boxes:
        wrapped[box.start : box.start + 4] = (box.size + 8).to_bytes(4, "big")
    for offset_field in (STCO_CHUNK_OFFSET_OFFSET, ILOC_EXTENT_OFFSET_OFFSET):
        current_offset = int.from_bytes(source[offset_field : offset_field + 4], "big")
        if current_offset < STTS_SECOND_RUN_OFFSET:
            raise ValueError("AVIF media reference precedes the inserted stts run")
        wrapped[offset_field : offset_field + 4] = (current_offset + 8).to_bytes(
            4, "big"
        )
    wrapped[STTS_SECOND_RUN_OFFSET:STTS_SECOND_RUN_OFFSET] = (
        (0xFFFF_FFFF).to_bytes(4, "big") + (2).to_bytes(4, "big")
    )
    wrapped_result = bytes(wrapped)
    if len(wrapped_result) != len(source) + 8:
        raise ValueError("wrapped-count mutation changed the file by an unexpected size")
    if (
        hashlib.sha256(wrapped_result).hexdigest()
        != STTS_VARIANT_SHA256["stts_wrapped_sample_count"]
    ):
        raise ValueError("wrapped-count AVIF mutation differs from its pinned output")
    source_mdat = unique_box(parse_boxes(source, 0, len(source)), b"mdat")
    mutated_mdat = unique_box(parse_boxes(wrapped_result, 0, len(wrapped_result)), b"mdat")
    assert source_mdat is not None and mutated_mdat is not None
    if wrapped_result[mutated_mdat.payload_start : mutated_mdat.end] != source[
        source_mdat.payload_start : source_mdat.end
    ]:
        raise ValueError("wrapped-count mutation changed the AVIF media payload")
    variants["stts_wrapped_sample_count"] = wrapped_result
    return variants


def write_fixture(source: bytes | None = None, output_path: Path = OUTPUT_PATH) -> str:
    """Write the pinned fixture, refusing to replace a different file."""
    data = SOURCE_PATH.read_bytes() if source is None else source
    output = zero_stts_sample_delta(data)
    if output_path.exists() and output_path.read_bytes() != output:
        raise FileExistsError(f"refusing to replace a different fixture at {output_path}")
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_bytes(output)
    return hashlib.sha256(output).hexdigest()


def write_timing_variants(
    source: bytes | None = None, output_dir: Path = SOURCE_PATH.parent
) -> dict[str, str]:
    """Write all pinned timing-table variants without replacing differing files."""
    data = SOURCE_PATH.read_bytes() if source is None else source
    variants = {"stts_zero_delta": zero_stts_sample_delta(data)}
    variants.update(stts_timing_variants(data))
    hashes = {}
    for name, output in variants.items():
        output_path = output_dir / f"animated_{name}.avif"
        if output_path.exists() and output_path.read_bytes() != output:
            raise FileExistsError(f"refusing to replace a different fixture at {output_path}")
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_bytes(output)
        hashes[name] = hashlib.sha256(output).hexdigest()
    return hashes


if __name__ == "__main__":
    print(f"wrote AVIF fixtures with SHA-256 {write_timing_variants()}")
