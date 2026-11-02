#!/usr/bin/env python3
"""Generate full-file AVIF track-box parity fixtures from one pinned source."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path

from PIL import Image, UnidentifiedImageError
import yaml

from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/avif/animated.avif"
OUTPUT_DIR = ROOT / "tests/fixtures/input/images/avif"
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
FIXTURE_SHA256S = {
    "duplicate_tkhd": "06e8189946b910376e447dfb29db317f3c80b383a291a10799e45be78e6b2ab8",
    "duplicate_mdia": "83ba36a8ae5559dad80fcd4e45b7c54d55406cc1eb2368081f4fb4301004e54a",
    "duplicate_tref_auxl": "32f47d98b7e9406480d8a9f9351689ae43f1822cf7712931a0cdb433cef360a0",
    "duplicate_edts": "1a184c2afd0529985cbc9df3737b40e2df8578de04e7dd9a8b38144adfbe2cc7",
    "missing_tkhd": "67dc5859be58f020106cdaf9ede0648161583f723a43f82d0501f67c6f8526e2",
    "zero_track_id_error": "5b9718040db403bad0122fd59426e3e4d61652b88cea1a5654435075475180a3",
    "missing_mdia": "714dd98f5357e554fec337937e8f938a653490ceebb2be1f76af6be0644f68df",
    "repeating_duration_finite": "076c6e24ac7dc4c91f82b94ed214e57da2e2d0392269782e3097427e128bcca1",
    "repeating_duration_nondivisible": "fc517b7bd5850eff7f50332298ba8ac6772958c07965d2d6a5bb311c14e7f1b2",
    "repeating_duration_zero": "4dbabf9ca1462df172a4509912c0799f92874a51fdd935ea29a39d76e1ea19b6",
    "repeating_duration_large": "65f39e3119b871fa15333c98f52c00c69d1064bcb0b9da89ae46d3871ff8ece3",
    "edit_list_entry_count_error": "6c85c53210f2c251b432f82a14920b3592ff293a75af3cb8561cff05d6ab3c98",
    "edit_list_segment_duration_zero_error": "ff0d9a0a1588986bd66142fd1d02a36c5ff9732b2cba184a700e3580f2a17720",
    "edit_box_duplicate_elst_error": "4f39c64e57f8d24d058b7f08d5fb6e76d1c455e790a24622187e5f58e5525b81",
    "edit_box_missing_elst_error": "4b8d2110df3a049b39333d94d9533fbf4a1a987fd4a0cddc4e1be5e4a04915eb",
    "duplicate_tref_hint": "f20b077c2b38c9e8e944f4a39549fe215a9d864374c34fa8b44180b86c60254e",
    "duplicate_av1c_conflicting_depth": "5325225cbcd2171dbc009a935e6318a995be90ea1641bcf6f4eecc277aa445c8",
    "duplicate_auxi_conflicting_alpha": "d549d98a957011df232993ab5605012110e173bb200a1af1f7a0773c74464cc5",
    "stss_trailing_payload": "607f1a675785f65fe6579920e803969030a3b67e13152f7303b12b97c3c60768",
    "stss_truncated_record": "52cf735b88c44a975ebbf0d6f17ebe103a90eb59639f4de1f9bb089289d5bf20",
    "stss_nonzero_version": "2d6aad8fb2b8e247f3489d54bb30cf0f49e2cdb1f2e8632ae70144dc5a93fdac",
    "zero_tref_auxl": "00bd7d70c5c9285d612bfec8fcca43de555ee0d5f7deedd0cb0f52bc03971563",
    "duplicate_tref_auxl_children": "bb9dc21acddc7b76ae65f81bbf07f12dfeeb75a1fd2d3696fb57c55238680be3",
}
OUTPUT_NAMES = {
    "duplicate_tkhd": "error_duplicate_track_tkhd_before_mdia.avif",
    "duplicate_mdia": "error_duplicate_track_mdia_after_mdia.avif",
    "duplicate_tref_auxl": "error_duplicate_track_tref_auxl_before_edts.avif",
    "duplicate_edts": "error_duplicate_track_edts_after_edts.avif",
    "missing_tkhd": "error_missing_track_tkhd.avif",
    "zero_track_id_error": "error_track_tkhd_zero_id.avif",
    "missing_mdia": "error_missing_track_mdia.avif",
    "repeating_duration_finite": "animated_track_duration_repeating_5_of_5.avif",
    "repeating_duration_nondivisible": "animated_track_duration_repeating_6_of_5.avif",
    "repeating_duration_zero": "error_track_duration_repeating_zero.avif",
    "repeating_duration_large": "animated_track_duration_repeating_large.avif",
    "edit_list_entry_count_error": "error_track_edit_list_entry_count.avif",
    "edit_list_segment_duration_zero_error": "error_track_edit_list_segment_duration_zero.avif",
    "edit_box_duplicate_elst_error": "error_track_edit_list_duplicate_elst.avif",
    "edit_box_missing_elst_error": "error_track_edit_box_missing_elst.avif",
    "duplicate_tref_hint": "animated_duplicate_track_tref_hint_before_edts.avif",
    "duplicate_av1c_conflicting_depth": "animated_duplicate_track_av1c_conflicting_depth.avif",
    "duplicate_auxi_conflicting_alpha": "animated_duplicate_track_auxi_conflicting_alpha.avif",
    "stss_trailing_payload": "animated_stss_trailing_payload.avif",
    "stss_truncated_record": "error_animated_stss_truncated_record.avif",
    "stss_nonzero_version": "error_animated_stss_nonzero_version.avif",
    "zero_tref_auxl": "animated_zero_track_tref_auxl_before_edts.avif",
    "duplicate_tref_auxl_children": "error_duplicate_track_tref_auxl_children_before_edts.avif",
}
REPEATING_DURATION_CASES = {
    "repeating_duration_finite": (5, 5),
    "repeating_duration_nondivisible": (6, 5),
    "repeating_duration_zero": (0, 5),
    "repeating_duration_large": ((1 << 31) + 1, 1),
}
PRIMARY_ILOC = bytes.fromhex("0000000044000001000100000001000003ff00000027")
SOURCE_MDAT_PAYLOAD_OFFSET = 0x3FF


def box_ranges(data: bytes | bytearray, start: int, end: int) -> list[tuple[bytes, int, int]]:
    """Return bounded 32-bit ISO-BMFF child ranges."""
    boxes = []
    offset = start
    while offset < end:
        if end - offset < 8:
            raise RuntimeError("truncated child box header")
        size = int.from_bytes(data[offset : offset + 4], "big")
        kind = bytes(data[offset + 4 : offset + 8])
        if size < 8 or size > end - offset:
            raise RuntimeError(f"invalid {kind!r} box size")
        boxes.append((kind, offset, offset + size))
        offset += size
    if offset != end:
        raise RuntimeError("child boxes do not fill their parent")
    return boxes


def make_box(kind: bytes, payload: bytes) -> bytes:
    """Encode one bounded 32-bit box."""
    size = len(payload) + 8
    return size.to_bytes(4, "big") + kind + payload


def source_layout(source: bytes) -> tuple[tuple[int, int, int], ...]:
    """Validate and return the pinned source's metadata, movie, and track."""
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("animated AVIF differs from its pinned source hash")

    top_level = box_ranges(source, 0, len(source))
    if [kind for kind, _, _ in top_level] != [b"ftyp", b"meta", b"moov", b"mdat"]:
        raise RuntimeError("animated AVIF top-level box layout changed")
    meta = next((box for box in top_level if box[0] == b"meta"), None)
    moov = next((box for box in top_level if box[0] == b"moov"), None)
    if meta is None or moov is None:
        raise RuntimeError("animated AVIF is missing its metadata or movie box")
    if source[meta[1] + 8 : meta[1] + 12] != bytes(4):
        raise RuntimeError("animated AVIF metadata FullBox version changed")
    metadata_children = box_ranges(source, meta[1] + 12, meta[2])
    locations = [box for box in metadata_children if box[0] == b"iloc"]
    if len(locations) != 1 or source[locations[0][1] + 8 : locations[0][2]] != PRIMARY_ILOC:
        raise RuntimeError("animated AVIF primary-item iloc layout changed")

    movie_children = box_ranges(source, moov[1] + 8, moov[2])
    tracks = [box for box in movie_children if box[0] == b"trak"]
    if len(tracks) != 1:
        raise RuntimeError("animated AVIF must contain one track")
    return meta, moov, tracks[0]


def rebase_file_offsets(
    source: bytes, meta: tuple[bytes, int, int], moov: tuple[bytes, int, int], growth: int
) -> bytearray:
    """Keep primary-item and track sample extents aimed at the shifted mdat."""
    rebased = bytearray(source)
    metadata_children = box_ranges(source, meta[1] + 12, meta[2])
    iloc = next(box for box in metadata_children if box[0] == b"iloc")
    iloc_offset = iloc[1] + 8 + 14
    source_extent = int.from_bytes(rebased[iloc_offset : iloc_offset + 4], "big")
    if source_extent != SOURCE_MDAT_PAYLOAD_OFFSET:
        raise RuntimeError("animated AVIF primary-item extent offset changed")
    rebased[iloc_offset : iloc_offset + 4] = (source_extent + growth).to_bytes(4, "big")

    offsets = []
    for kind, track_start, track_end in box_ranges(source, moov[1] + 8, moov[2]):
        if kind != b"trak":
            continue
        for media_kind, media_start, media_end in box_ranges(source, track_start + 8, track_end):
            if media_kind != b"mdia":
                continue
            for info_kind, info_start, info_end in box_ranges(
                source, media_start + 8, media_end
            ):
                if info_kind != b"minf":
                    continue
                for table_kind, table_start, table_end in box_ranges(
                    source, info_start + 8, info_end
                ):
                    if table_kind != b"stbl":
                        continue
                    offsets.extend(
                        box
                        for box in box_ranges(source, table_start + 8, table_end)
                        if box[0] in (b"stco", b"co64")
                    )

    if len(offsets) != 1:
        raise RuntimeError("animated AVIF must contain one chunk-offset table")
    kind, start, end = offsets[0]
    width = 4 if kind == b"stco" else 8
    count = int.from_bytes(rebased[start + 12 : start + 16], "big")
    if count != 1:
        raise RuntimeError("animated AVIF must contain one chunk offset")
    chunk_offset = start + 16
    source_chunk_offset = int.from_bytes(rebased[chunk_offset : chunk_offset + width], "big")
    if source_chunk_offset != SOURCE_MDAT_PAYLOAD_OFFSET:
        raise RuntimeError("animated AVIF chunk offset changed")
    rebased[chunk_offset : chunk_offset + width] = (
        source_chunk_offset + growth
    ).to_bytes(width, "big")
    return rebased


def fixture_bytes(source: bytes, case: str) -> bytes:
    """Mutate one track box while preserving all shifted payload references."""
    meta, moov, track = source_layout(source)
    track_children = box_ranges(source, track[1] + 8, track[2])
    track_header = next((box for box in track_children if box[0] == b"tkhd"), None)
    if track_header is None:
        raise RuntimeError("animated AVIF is missing its track header")

    if case == "zero_track_id_error":
        version = source[track_header[1] + 8]
        if version not in (0, 1):
            raise RuntimeError("animated AVIF track-header version changed")
        track_id_offset = track_header[1] + (28 if version == 1 else 20)
        if source[track_id_offset : track_id_offset + 4] == bytes(4):
            raise RuntimeError("animated AVIF source track ID is already zero")
        candidate = bytearray(source)
        candidate[track_id_offset : track_id_offset + 4] = bytes(4)
        result = bytes(candidate)
        actual_sha256 = sha256(result).hexdigest()
        if actual_sha256 != FIXTURE_SHA256S[case]:
            raise RuntimeError(
                f"generated {case} fixture hash changed: "
                f"expected {FIXTURE_SHA256S[case]}, got {actual_sha256}"
            )
        return result

    if case == "duplicate_tkhd":
        target = track_header
        inserted = source[target[1] : target[2]]
        insertion_offset = target[2]
    elif case == "duplicate_mdia":
        media_boxes = [box for box in track_children if box[0] == b"mdia"]
        if len(media_boxes) != 1:
            raise RuntimeError("animated AVIF must contain one media box")
        target = media_boxes[0]
        inserted = source[target[1] : target[2]]
        insertion_offset = target[2]
    elif case == "duplicate_edts":
        edit_boxes = [box for box in track_children if box[0] == b"edts"]
        if len(edit_boxes) != 1:
            raise RuntimeError("animated AVIF must contain one edit box")
        target = edit_boxes[0]
        inserted = source[target[1] : target[2]]
        insertion_offset = target[2]
    elif case in {"missing_tkhd", "missing_mdia", "edit_box_missing_elst_error"}:
        if case == "edit_box_missing_elst_error":
            edit_box = next((box for box in track_children if box[0] == b"edts"), None)
            if edit_box is None:
                raise RuntimeError("animated AVIF is missing its edit box")
            target = next(
                (box for box in box_ranges(source, edit_box[1] + 8, edit_box[2])
                 if box[0] == b"elst"),
                None,
            )
            target_name = "elst"
        else:
            target_kind = b"tkhd" if case == "missing_tkhd" else b"mdia"
            target = next((box for box in track_children if box[0] == target_kind), None)
            target_name = target_kind.decode()
        if target is None:
            raise RuntimeError(f"animated AVIF is missing its {target_name} box")
        inserted = make_box(b"free", source[target[1] + 8 : target[2]])
        insertion_offset = target[1]
    elif case in REPEATING_DURATION_CASES or case in {
        "edit_list_entry_count_error",
        "edit_list_segment_duration_zero_error",
    }:
        inserted = b""
        insertion_offset = len(source)
    elif case == "edit_box_duplicate_elst_error":
        edit_boxes = [box for box in track_children if box[0] == b"edts"]
        if len(edit_boxes) != 1:
            raise RuntimeError("animated AVIF must contain one edit box")
        edit_box = edit_boxes[0]
        edit_lists = [
            box for box in box_ranges(source, edit_box[1] + 8, edit_box[2])
            if box[0] == b"elst"
        ]
        if len(edit_lists) != 1:
            raise RuntimeError("animated AVIF edit box must contain one edit list")
        edit_list = edit_lists[0]
        inserted = source[edit_list[1] : edit_list[2]]
        insertion_offset = edit_box[2]
    elif case == "duplicate_tref_hint":
        edit_box = next((box for box in track_children if box[0] == b"edts"), None)
        if edit_box is None:
            raise RuntimeError("animated AVIF is missing its edit box")
        version = source[track_header[1] + 8]
        track_id_offset = track_header[1] + (28 if version == 1 else 20)
        track_id = int.from_bytes(source[track_id_offset : track_id_offset + 4], "big")
        if track_id == 0:
            raise RuntimeError("animated AVIF track ID must be nonzero")
        reference = make_box(b"tref", make_box(b"hint", track_id.to_bytes(4, "big")))
        inserted = reference * 2
        insertion_offset = edit_box[1]
    elif case == "duplicate_tref_auxl":
        edit_box = next((box for box in track_children if box[0] == b"edts"), None)
        if edit_box is None:
            raise RuntimeError("animated AVIF is missing its edit box")
        version = source[track_header[1] + 8]
        track_id_offset = track_header[1] + (28 if version == 1 else 20)
        track_id = int.from_bytes(source[track_id_offset : track_id_offset + 4], "big")
        if track_id == 0:
            raise RuntimeError("animated AVIF track ID must be nonzero")
        reference = make_box(b"tref", make_box(b"auxl", track_id.to_bytes(4, "big")))
        inserted = reference * 2
        insertion_offset = edit_box[1]
    elif case in {"zero_tref_auxl", "duplicate_tref_auxl_children"}:
        edit_box = next((box for box in track_children if box[0] == b"edts"), None)
        if edit_box is None:
            raise RuntimeError("animated AVIF is missing its edit box")
        if case == "zero_tref_auxl":
            references = make_box(b"auxl", bytes(4))
        else:
            version = source[track_header[1] + 8]
            track_id_offset = track_header[1] + (28 if version == 1 else 20)
            track_id = int.from_bytes(source[track_id_offset : track_id_offset + 4], "big")
            if track_id == 0:
                raise RuntimeError("animated AVIF track ID must be nonzero")
            reference = make_box(b"auxl", track_id.to_bytes(4, "big"))
            references = reference * 2
        inserted = make_box(b"tref", references)
        insertion_offset = edit_box[1]
    elif case in {
        "duplicate_av1c_conflicting_depth",
        "duplicate_auxi_conflicting_alpha",
    }:
        media = next(box for box in track_children if box[0] == b"mdia")
        info = next(
            box for box in box_ranges(source, media[1] + 8, media[2]) if box[0] == b"minf"
        )
        table = next(
            box for box in box_ranges(source, info[1] + 8, info[2]) if box[0] == b"stbl"
        )
        descriptions = next(
            box for box in box_ranges(source, table[1] + 8, table[2]) if box[0] == b"stsd"
        )
        entries = box_ranges(source, descriptions[1] + 16, descriptions[2])
        samples = [box for box in entries if box[0] == b"av01"]
        if len(samples) != 1:
            raise RuntimeError("animated AVIF must contain one AV1 sample entry")
        sample = samples[0]
        sample_properties = box_ranges(source, sample[1] + 8 + 78, sample[2])
        if case == "duplicate_av1c_conflicting_depth":
            target = next((box for box in sample_properties if box[0] == b"av1C"), None)
            if target is None:
                raise RuntimeError("animated AVIF sample entry is missing av1C")
            inserted_bytes = bytearray(source[target[1] : target[2]])
            if len(inserted_bytes) < 11 or inserted_bytes[10] & 0x60:
                raise RuntimeError("animated AVIF sample entry is not 8-bit av1C")
            inserted_bytes[10] |= 0x40
            inserted = bytes(inserted_bytes)
            insertion_offset = target[2]
        else:
            ignored_auxiliary_type = make_box(b"auxi", bytes(4) + b"urn:example:ignored\0")
            alpha_auxiliary_type = make_box(
                b"auxi",
                bytes(4) + b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha\0",
            )
            inserted = ignored_auxiliary_type + alpha_auxiliary_type
            insertion_offset = sample[2]
        sample_entry_ancestors = (sample, descriptions, table, info, media)
    elif case in {
        "stss_trailing_payload",
        "stss_truncated_record",
        "stss_nonzero_version",
    }:
        media = next(box for box in track_children if box[0] == b"mdia")
        info = next(
            box for box in box_ranges(source, media[1] + 8, media[2]) if box[0] == b"minf"
        )
        table = next(
            box for box in box_ranges(source, info[1] + 8, info[2]) if box[0] == b"stbl"
        )
        sync_table = next(
            (box for box in box_ranges(source, table[1] + 8, table[2]) if box[0] == b"stss"),
            None,
        )
        if sync_table is None or sync_table[2] - sync_table[1] != 20:
            raise RuntimeError("animated AVIF stss table layout changed")
        sync_count = int.from_bytes(source[sync_table[1] + 12 : sync_table[1] + 16], "big")
        sync_sample = int.from_bytes(source[sync_table[1] + 16 : sync_table[1] + 20], "big")
        if sync_count != 1 or sync_sample != 1:
            raise RuntimeError("animated AVIF stss sample table changed")
        result = bytearray(source)
        if case == "stss_nonzero_version":
            if source[sync_table[1] + 8 : sync_table[1] + 12] != bytes(4):
                raise RuntimeError("animated AVIF stss FullBox changed")
            result[sync_table[1] + 8] = 1
        else:
            declared_count = 0 if case == "stss_trailing_payload" else 2
            result[sync_table[1] + 12 : sync_table[1] + 16] = declared_count.to_bytes(4, "big")
        result = bytes(result)
        actual_sha256 = sha256(result).hexdigest()
        if actual_sha256 != FIXTURE_SHA256S[case]:
            raise RuntimeError(
                f"generated {case} fixture hash changed: expected {FIXTURE_SHA256S[case]}, "
                f"got {actual_sha256}"
            )
        return result
    else:
        raise RuntimeError(f"unsupported fixture case: {case}")

    replacement_end = (
        target[2]
        if case in {"missing_tkhd", "missing_mdia", "edit_box_missing_elst_error"}
        else insertion_offset
    )
    growth = len(inserted) - (replacement_end - insertion_offset)
    rebased = rebase_file_offsets(source, meta, moov, growth)
    if case in {"duplicate_tkhd", "duplicate_mdia", "duplicate_edts"}:
        inserted = bytes(rebased[target[1] : target[2]])
    candidate = bytearray(rebased[:insertion_offset] + inserted + rebased[replacement_end:])
    if case in {
        "duplicate_av1c_conflicting_depth",
        "duplicate_auxi_conflicting_alpha",
    }:
        for ancestor in (*sample_entry_ancestors, track, moov):
            size = ancestor[2] - ancestor[1]
            candidate[ancestor[1] : ancestor[1] + 4] = (size + growth).to_bytes(4, "big")
    else:
        candidate[track[1] : track[1] + 4] = (track[2] - track[1] + growth).to_bytes(4, "big")
        candidate[moov[1] : moov[1] + 4] = (moov[2] - moov[1] + growth).to_bytes(4, "big")
        if case == "edit_box_duplicate_elst_error":
            candidate[edit_box[1] : edit_box[1] + 4] = (
                edit_box[2] - edit_box[1] + growth
            ).to_bytes(4, "big")
    if case in REPEATING_DURATION_CASES:
        edit_box = next(box for box in track_children if box[0] == b"edts")
        edit_list = next(
            box for box in box_ranges(source, edit_box[1] + 8, edit_box[2])
            if box[0] == b"elst"
        )
        version = source[track_header[1] + 8]
        duration_width = 8 if version == 1 else 4
        duration_offset = track_header[1] + (36 if version == 1 else 28)
        track_duration, segment_duration = REPEATING_DURATION_CASES[case]
        candidate[duration_offset : duration_offset + duration_width] = (
            track_duration.to_bytes(duration_width, "big")
        )
        candidate[edit_list[1] + 9 : edit_list[1] + 12] = (1).to_bytes(3, "big")
        segment_offset = edit_list[1] + 16
        candidate[segment_offset : segment_offset + duration_width] = (
            segment_duration.to_bytes(duration_width, "big")
        )
    elif case == "edit_list_entry_count_error":
        edit_box = next(box for box in track_children if box[0] == b"edts")
        edit_list = next(
            box for box in box_ranges(source, edit_box[1] + 8, edit_box[2])
            if box[0] == b"elst"
        )
        entry_count_offset = edit_list[1] + 12
        if int.from_bytes(source[entry_count_offset : entry_count_offset + 4], "big") != 1:
            raise RuntimeError("animated AVIF edit list must have one source entry")
        candidate[edit_list[1] + 9 : edit_list[1] + 12] = (1).to_bytes(3, "big")
        candidate[entry_count_offset : entry_count_offset + 4] = (2).to_bytes(4, "big")
    elif case == "edit_list_segment_duration_zero_error":
        edit_box = next(box for box in track_children if box[0] == b"edts")
        edit_list = next(
            box for box in box_ranges(source, edit_box[1] + 8, edit_box[2])
            if box[0] == b"elst"
        )
        candidate[edit_list[1] + 9 : edit_list[1] + 12] = (1).to_bytes(3, "big")
        duration_width = 8 if source[edit_list[1] + 8] == 1 else 4
        segment_offset = edit_list[1] + 16
        if int.from_bytes(source[segment_offset : segment_offset + duration_width], "big") == 0:
            raise RuntimeError("animated AVIF edit-list segment duration must be nonzero")
        candidate[segment_offset : segment_offset + duration_width] = bytes(duration_width)
    result = bytes(candidate)
    actual_sha256 = sha256(result).hexdigest()
    if actual_sha256 != FIXTURE_SHA256S[case]:
        raise RuntimeError(
            f"generated {case} fixture hash changed: expected {FIXTURE_SHA256S[case]}, "
            f"got {actual_sha256}"
        )
    return result


def verify_oracle(source: bytes, candidate: bytes, case: str) -> None:
    """Check the case-specific full-file Pillow parity contract."""
    if case in {
        "duplicate_tkhd",
        "duplicate_mdia",
        "duplicate_edts",
        "duplicate_tref_auxl",
        "missing_tkhd",
        "missing_mdia",
        "zero_track_id_error",
        "repeating_duration_zero",
        "edit_list_entry_count_error",
        "edit_list_segment_duration_zero_error",
        "edit_box_duplicate_elst_error",
        "edit_box_missing_elst_error",
        "stss_truncated_record",
        "stss_nonzero_version",
        "duplicate_tref_auxl_children",
    }:
        try:
            Image.open(BytesIO(candidate))
        except UnidentifiedImageError:
            return
        except Exception as error:
            raise RuntimeError(
                f"expected Pillow UnidentifiedImageError, got {type(error).__name__}: {error}"
            ) from error
        raise RuntimeError(f"Pillow unexpectedly opened the {case} fixture")

    original = Image.open(BytesIO(source))
    changed = Image.open(BytesIO(candidate))
    if (changed.size, changed.mode, changed.n_frames) != (
        original.size,
        original.mode,
        original.n_frames,
    ):
        raise RuntimeError("Pillow changed AVIF dimensions, mode, or frame count")
    for frame_index in range(original.n_frames):
        original.seek(frame_index)
        changed.seek(frame_index)
        if original.info.get("duration") != changed.info.get("duration"):
            raise RuntimeError(f"Pillow timing changed in frame {frame_index}")
        if original.tobytes() != changed.tobytes():
            raise RuntimeError(f"Pillow pixels changed in frame {frame_index}")


def generate(
    case: str,
    output: Path | None = None,
    check: bool = False,
    replace: bool = False,
) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    source = SOURCE.read_bytes()
    candidate = fixture_bytes(source, case)
    verify_oracle(source, candidate, case)

    destination = output or (OUTPUT_DIR / OUTPUT_NAMES[case])
    destination.parent.mkdir(parents=True, exist_ok=True)
    if check:
        if not destination.is_file() or destination.read_bytes() != candidate:
            raise RuntimeError(f"fixture is missing or differs from generated bytes: {destination}")
    elif destination.exists():
        if destination.read_bytes() != candidate and not replace:
            raise RuntimeError(f"refusing to replace a different fixture at {destination}")
        if destination.read_bytes() != candidate:
            destination.write_bytes(candidate)
    else:
        destination.write_bytes(candidate)
    print(
        f"Verified {destination} ({len(candidate)} bytes, "
        f"sha256={FIXTURE_SHA256S[case]})"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", choices=tuple(OUTPUT_NAMES), default="duplicate_tkhd")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--replace", action="store_true")
    arguments = parser.parse_args()
    generate(arguments.case, arguments.output, arguments.check, arguments.replace)


if __name__ == "__main__":
    main()
