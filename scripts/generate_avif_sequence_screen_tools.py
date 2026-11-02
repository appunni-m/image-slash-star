#!/usr/bin/env python3
"""Generate pinned AVIF parity cases for AV1 screen-content sequence flags."""

from __future__ import annotations

import argparse
import hashlib
import io
from pathlib import Path
import struct
import tempfile

import PIL
from PIL import Image, _avif, features

from avif_bmff_boxes import rewrite_boxes
from inspect_avif_bitstreams import inspect as inspect_container, parse_boxes
from inspect_av1_obus import Bits, inspect as inspect_obus, parse_sequence_header


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/avif"
SOURCE = ASSETS / "animated_transfer_bt709.avif"
SOURCE_SHA256 = "8b7c981b052241b55112e6ae3ce64b00681c101a25c7d9bd9c9091d727417af2"
SOURCE_SAMPLE_SIZES = (2414, 1981, 2120, 1952, 2057)
SOURCE_ILOC = bytes.fromhex("0000000044000001000100000001000003f10000096e")
STSZ_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stsz")
ILOC_PATH = (b"meta", b"iloc")
EXPECTED_CODECS = "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"

CASES = {
    "screen-content-disabled": {
        "asset": "animated_screen_content_tools_disabled.avif",
        "sha256": "d0c88a39ca31c30f56afa5540505d7e20892ed0d9d5339ff843cdf6d9040a357",
        "screen_content_tools": 0,
        "force_integer_mv": 2,
    },
    "integer-mv-disabled": {
        "asset": "animated_integer_mv_disabled.avif",
        "sha256": "47b3767081913f190e0473842a466a9a81ae4acce5e5099c323c9793187cdcce",
        "screen_content_tools": 2,
        "force_integer_mv": 0,
    },
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def bits_from_bytes(data: bytes) -> list[int]:
    return [
        (byte >> shift) & 1
        for byte in data
        for shift in range(7, -1, -1)
    ]


def pack_bits(bits: list[int]) -> bytes:
    padded = bits + [0] * ((-len(bits)) % 8)
    output = bytearray(len(padded) // 8)
    for index, bit in enumerate(padded):
        output[index // 8] |= bit << (7 - index % 8)
    return bytes(output)


def last_one_bit(bits: list[int]) -> int:
    for index in range(len(bits) - 1, -1, -1):
        if bits[index]:
            return index
    raise ValueError("AV1 payload has no trailing-one bit")


def read_uleb(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    for index in range(8):
        require(offset < len(data), "truncated OBU length")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << (index * 7)
        if byte & 0x80 == 0:
            require(value <= 0xFFFF_FFFF, "OBU length exceeds u32")
            return value, offset
    raise ValueError("OBU length exceeds eight bytes")


def write_uleb(value: int) -> bytes:
    require(0 <= value <= 0xFFFF_FFFF, "OBU payload exceeds u32")
    encoded = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        encoded.append(byte | (0x80 if value else 0))
        if not value:
            return bytes(encoded)


def parse_obus(sample: bytes) -> list[tuple[int, bytes]]:
    obus = []
    offset = 0
    while offset < len(sample):
        header = sample[offset]
        offset += 1
        require(header & 0x84 == 0, "pinned source OBU has unsupported header flags")
        require(header & 1 == 0, "pinned source OBU has a reserved bit")
        require(header & 4 == 0, "pinned source OBU unexpectedly has an extension")
        require(header & 2 != 0, "pinned source OBU has no size field")
        payload_size, payload_start = read_uleb(sample, offset)
        payload_end = payload_start + payload_size
        require(payload_end <= len(sample), "OBU payload exceeds its sample")
        obus.append((header, sample[payload_start:payload_end]))
        offset = payload_end
    return obus


def pack_obu(header: int, payload: bytes) -> bytes:
    return bytes([header]) + write_uleb(len(payload)) + payload


def sequence_selector_positions(payload: bytes) -> tuple[int, int, list[int]]:
    """Locate selector bits in this fixture's validated ordinary sequence header."""
    bits = Bits(payload)
    require(bits.read(3) == 0, "source AV1 profile changed")
    require(bits.bit() == 0, "source still_picture flag changed")
    require(bits.bit() == 0, "source reduced-header flag changed")
    require(bits.bit() == 0, "source timing syntax changed")
    require(bits.bit() == 0, "source display-model flag changed")
    require(bits.read(5) == 0, "source operating-point count changed")
    require(bits.read(12) == 0, "source operating-point IDC changed")
    level = bits.read(5)
    require(level <= 7, "source operating-point tier syntax changed")

    width_bits = bits.read(4) + 1
    height_bits = bits.read(4) + 1
    bits.read(width_bits)
    bits.read(height_bits)
    require(bits.bit() == 0, "source frame-ID syntax changed")
    bits.read(3)  # 128x128 superblocks, filter intra, and intra-edge filter.
    bits.read(4)  # Interintra, masked, warped, dual-filter, and order-hint flags.
    require(bits.bit() == 1, "source order hints were disabled")
    bits.read(2)  # Joint compound and reference-frame motion-vector flags.

    screen_choose_position = bits.position
    screen_choose = bits.bit()
    require(screen_choose == 1, "source screen-tools selector changed")
    screen_content_tools = 2 if screen_choose else bits.bit()
    require(screen_content_tools == 2, "source screen-content tools are not selectable")
    integer_choose_position = bits.position
    integer_choose = bits.bit()
    require(integer_choose == 1, "source integer-motion-vector selector changed")
    require(
        parse_sequence_header(payload)["force_integer_mv"] == 2,
        "source force-integer-MV value changed",
    )
    original_bits = bits_from_bytes(payload)
    logical_length = last_one_bit(original_bits) + 1
    require(bits.position < logical_length, "sequence selectors overlap trailing bits")
    return screen_choose_position, integer_choose_position, original_bits


def mutate_sequence_header(payload: bytes, case: str) -> bytes:
    screen_position, integer_position, syntax = sequence_selector_positions(payload)
    if case == "screen-content-disabled":
        # The source's integer selector becomes the explicit screen-tools value.
        updated = syntax[:screen_position] + [0, 0] + syntax[integer_position + 1 :]
    else:
        # Replace choose-integer-MV with choose=0 and its explicit value=0.
        updated = (
            syntax[:integer_position]
            + [0, 0]
            + syntax[integer_position + 1 :]
        )
    candidate = pack_bits(updated)
    parsed = parse_sequence_header(candidate)
    require(
        parsed["screen_content_tools"] == CASES[case]["screen_content_tools"],
        "mutated screen-content selector differs from requested case",
    )
    require(
        parsed["force_integer_mv"] == CASES[case]["force_integer_mv"],
        "mutated integer-MV selector differs from requested case",
    )
    return candidate


def allow_screen_tools_position(payload: bytes, sequence: dict[str, object]) -> int | None:
    """Return the optional frame-level screen-tools flag's bit position."""
    bits = Bits(payload)
    if sequence["reduced_still_picture_header"]:
        frame_type_index = 0
        show_frame = True
    else:
        if bits.bit():
            return None
        frame_type_index = bits.read(2)
        show_frame = bool(bits.bit())
    if not show_frame:
        bits.bit()  # showable_frame
    forced_error_resilient = (
        (frame_type_index == 0 and show_frame)
        or frame_type_index == 3
        or sequence["reduced_still_picture_header"]
    )
    if not forced_error_resilient:
        bits.bit()
    bits.bit()  # disable_cdf_update
    return bits.position


def remove_frame_screen_tools_flag(
    payload: bytes,
    frame_header: dict[str, object],
    tile_group: dict[str, object] | None,
    sequence: dict[str, object],
) -> bytes:
    position = allow_screen_tools_position(payload, sequence)
    require(position is not None, "show-existing frame unexpectedly has a screen-tools flag")
    require(
        frame_header.get("allow_screen_content_tools") is False,
        "source frame unexpectedly enables screen-content tools",
    )
    payload_bits = bits_from_bytes(payload)
    header_end = int(frame_header["header_bits"])
    require(position < header_end and payload_bits[position] == 0, "frame screen-tools bit changed")
    new_header = payload_bits[:position] + payload_bits[position + 1 : header_end]

    if tile_group is None:
        old_trailing = payload_bits[header_end:]
        require(old_trailing and old_trailing[0] == 1, "frame header has no trailing-one bit")
        require(all(bit == 0 for bit in old_trailing[1:]), "frame header trailing bits changed")
        return pack_bits(new_header + [1])

    old_aligned_end = (header_end + 7) & ~7
    data_bit = int(tile_group["data_bit"])
    require(old_aligned_end <= data_bit <= len(payload_bits), "tile-group bit range changed")
    require(data_bit % 8 == 0, "pinned tile payload is not byte aligned")
    require(
        all(bit == 0 for bit in payload_bits[header_end:old_aligned_end]),
        "frame-header alignment bits changed",
    )
    tile_group_header = payload_bits[old_aligned_end:data_bit]
    tile_payload = payload_bits[data_bit:]
    require(len(tile_payload) % 8 == 0, "tile payload is not a whole number of bytes")
    new_aligned = new_header + [0] * ((-len(new_header)) % 8)
    return pack_bits(new_aligned + tile_group_header + tile_payload)


def update_stsz(payload: bytes, sample_sizes: tuple[int, ...]) -> bytes:
    require(payload[:4] == bytes(4), "source stsz is not version zero")
    common_size, count = struct.unpack_from(">II", payload, 4)
    require(common_size == 0 and count == len(SOURCE_SAMPLE_SIZES), "source stsz layout changed")
    require(len(payload) == 12 + 4 * count, "source stsz sample-size table is truncated")
    require(
        struct.unpack_from(f">{count}I", payload, 12) == SOURCE_SAMPLE_SIZES,
        "source stsz sizes changed",
    )
    return payload[:12] + struct.pack(f">{count}I", *sample_sizes)


def update_primary_item_extent(payload: bytes, sample_size: int) -> bytes:
    require(payload == SOURCE_ILOC, "source primary-item iloc layout changed")
    return payload[:18] + struct.pack(">I", sample_size) + payload[22:]


def candidate_bytes(case: str) -> bytes:
    require(case in CASES, f"unknown AVIF sequence-header case: {case}")
    source = SOURCE.read_bytes()
    digest = hashlib.sha256(source).hexdigest()
    require(digest == SOURCE_SHA256, f"pinned source hash changed: {digest}")

    container = inspect_container(SOURCE)
    track = next(item for item in container["tracks"] if item["handler"] == "pict")
    samples = track["samples"]
    require(
        tuple(int(sample["length"]) for sample in samples) == SOURCE_SAMPLE_SIZES,
        "pinned source sample table changed",
    )
    item = container["items"]["color"][0]
    require(int(item["length"]) == SOURCE_SAMPLE_SIZES[0], "primary item extent changed")
    mdat = next(box for box in parse_boxes(source, 0, len(source)) if box.kind == b"mdat")
    require(
        mdat.end - mdat.payload_start == sum(SOURCE_SAMPLE_SIZES),
        "pinned mdat no longer contains exactly the track samples",
    )

    sequence_report = inspect_obus(SOURCE)
    original_sequence: dict[str, object] | None = None
    for sample_report in sequence_report["samples"]:
        for obu in sample_report["obus"]:
            if "sequence_header" in obu:
                original_sequence = obu["sequence_header"]
                break
        if original_sequence is not None:
            break
    require(original_sequence is not None, "source has no AV1 sequence header")
    require(
        original_sequence["screen_content_tools"] == 2
        and original_sequence["force_integer_mv"] == 2,
        "pinned source sequence selectors changed",
    )
    sequence_payload = bytes.fromhex(str(original_sequence["payload_hex"]))
    updated_sequence = mutate_sequence_header(sequence_payload, case)

    updated_samples = []
    frame_count = 0
    for sample_index, sample in enumerate(samples):
        sample_bytes = source[
            int(sample["offset"]) : int(sample["offset"]) + int(sample["length"])
        ]
        original_obus = parse_obus(sample_bytes)
        rebuilt = []
        obus_report = sequence_report["samples"]
        track_sample = next(
            entry
            for entry in obus_report
            if entry.get("role") == "track_pict"
            and isinstance(entry.get("identity"), dict)
            and entry["identity"].get("sample") == sample_index
        )
        reported_obus = iter(track_sample["obus"])
        for header, payload in original_obus:
            obu_type = (header >> 3) & 0x0F
            reported = next(reported_obus)
            if obu_type == 1:
                payload = updated_sequence
            elif obu_type in (3, 6, 7) and case == "screen-content-disabled":
                frame_header = reported.get("frame_header")
                require(isinstance(frame_header, dict), "frame report lacks parsed header")
                require(
                    not frame_header["show_existing_frame"],
                    "pinned source unexpectedly has a show-existing frame header",
                )
                tile_group = reported.get("tile_group")
                payload = remove_frame_screen_tools_flag(
                    payload,
                    frame_header,
                    tile_group if isinstance(tile_group, dict) else None,
                    original_sequence,
                )
                frame_count += 1
            rebuilt.append(pack_obu(header, payload))
        try:
            next(reported_obus)
        except StopIteration:
            pass
        else:
            raise ValueError("AV1 observer reported more OBUs than the sample contains")
        updated_samples.append(b"".join(rebuilt))

    if case == "screen-content-disabled":
        require(frame_count == 5, f"expected five mutable frame headers, found {frame_count}")
    sample_sizes = tuple(map(len, updated_samples))

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == STSZ_PATH:
            return update_stsz(payload, sample_sizes)
        if path == ILOC_PATH:
            return update_primary_item_extent(payload, sample_sizes[0])
        if path == (b"mdat",):
            return b"".join(updated_samples)
        return payload

    candidate = rewrite_boxes(source, transform)
    candidate_hash = hashlib.sha256(candidate).hexdigest()
    require(candidate_hash == CASES[case]["sha256"], f"fixture hash changed: {candidate_hash}")
    validate_output(case, candidate, source)
    return candidate


def pillow_frames(data: bytes) -> tuple[tuple[object, ...], ...]:
    frames = []
    with Image.open(io.BytesIO(data)) as image:
        for index in range(image.n_frames):
            image.seek(index)
            image.load()
            frames.append(
                (
                    image.format,
                    image.mode,
                    image.size,
                    image.info.get("duration"),
                    image.tobytes(),
                )
            )
    return tuple(frames)


def validate_output(case: str, candidate: bytes, source: bytes) -> None:
    require(PIL.__version__ == "12.2.0", "Pillow oracle version changed")
    require(features.version("avif") == "1.4.1", "libavif oracle version changed")
    require(_avif.codec_versions() == EXPECTED_CODECS, "AVIF codec version changed")

    with tempfile.TemporaryDirectory(prefix="avif-screen-content-") as temporary:
        path = Path(temporary) / "candidate.avif"
        path.write_bytes(candidate)
        container = inspect_container(path)
        obu_report = inspect_obus(path)

    samples = next(track["samples"] for track in container["tracks"] if track["handler"] == "pict")
    sample_sizes = tuple(int(sample["length"]) for sample in samples)
    require(
        sample_sizes[0] == container["items"]["color"][0]["length"],
        "item and track sample zero differ",
    )
    require(
        container["items"]["color"][0]["sha256"] == samples[0]["sha256"],
        "primary item and track sample zero bytes differ",
    )

    sequence_headers = [
        obu["sequence_header"]
        for sample in obu_report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    require(bool(sequence_headers), "candidate has no AV1 sequence header")
    for sequence in sequence_headers:
        require(
            sequence["screen_content_tools"] == CASES[case]["screen_content_tools"]
            and sequence["force_integer_mv"] == CASES[case]["force_integer_mv"],
            "candidate sequence selectors do not match the case",
        )
    require(
        pillow_frames(candidate) == pillow_frames(source),
        "Pillow decoded pixels, frame timing, or animation metadata changed",
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", choices=CASES, help="generate or check one parity case")
    parser.add_argument("--check", action="store_true", help="verify generated fixture bytes")
    args = parser.parse_args()
    selected = [args.case] if args.case else list(CASES)
    for case in selected:
        expected = candidate_bytes(case)
        output = ASSETS / str(CASES[case]["asset"])
        if args.check:
            require(
                output.is_file() and output.read_bytes() == expected,
                f"fixture is stale: {output.name}",
            )
            action = "verified"
        else:
            if output.exists() and output.read_bytes() != expected:
                raise SystemExit(f"refusing to replace different fixture at {output}")
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(expected)
            action = "wrote"
        print(f"{action} {output.relative_to(ROOT)} sha256={CASES[case]['sha256']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
