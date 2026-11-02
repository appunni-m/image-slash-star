#!/usr/bin/env python3
"""Generate a tiny lossless AVIF that enables clamped monochrome superres."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile

from PIL import Image, _avif, features

from inspect_av1_obus import inspect as inspect_av1
from inspect_avif_bitstreams import Reader, children, inspect as inspect_avif, parse_boxes


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "portable_lossless_monochrome_superres_clamped_3x3.avif"
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
BASE_SHA256 = "6a0337819044ca1ebdd87919cbf6153d1212f8260e6756d6556815dfcd3915c8"
ASSET_SHA256 = "b2e3a50cf5e71bbde9324c3b8f3b1fb8a050e088c25aa1029fd7bad93dd0c2d6"
PIXEL_SHA256 = "d3032e6d59449568a19027d012277ab23dc9843f5293220df001c16217a6e35f"
WIDTH = 3
HEIGHT = 3

# These offsets belong to the exact pinned reduced-header, 3x3, single-key-frame
# seed. The syntax verifier below checks the surrounding profile before mutation.
SEQUENCE_ENABLE_SUPERRES_BIT = 25
FRAME_USE_SUPERRES_BIT = 2
SUPERRES_DENOMINATOR = 9


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest for fixture bytes."""
    return hashlib.sha256(data).hexdigest()


def source_y4m() -> bytes:
    """Build the fixed 3x3 full-range monochrome encoder input."""
    pixels = bytes((x * 17 + y * 31 + 19) & 0xFF for y in range(HEIGHT) for x in range(WIDTH))
    return (
        b"YUV4MPEG2 W3 H3 F30:1 Ip A1:1 Cmono XCOLORRANGE=FULL\nFRAME\n"
        + pixels
    )


def run(command: list[str]) -> str:
    """Run a pinned fixture-generation command and return combined output."""
    result = subprocess.run(
        command,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return result.stdout


def set_bit(data: bytearray, index: int) -> None:
    """Set one MSB-first bit in a bounded syntax payload."""
    if index < 0 or index >= len(data) * 8:
        raise ValueError("AV1 syntax bit lies outside its payload")
    data[index // 8] |= 1 << (7 - index % 8)


def iloc_extent_length_offset(data: bytes, expected_length: int) -> int:
    """Find the sole file-backed AV1 item's fixed-width iloc extent length."""
    top_level = parse_boxes(data, 0, len(data))
    meta = next((box for box in top_level if box.kind == b"meta"), None)
    if meta is None:
        raise ValueError("AVIF seed has no meta box")
    iloc = next((box for box in children(data, meta, prefix=4) if box.kind == b"iloc"), None)
    if iloc is None:
        raise ValueError("AVIF seed has no iloc box")

    reader = Reader(data, iloc.payload_start, iloc.end)
    version = reader.u32() >> 24
    if version > 2:
        raise ValueError("AVIF seed uses an unsupported iloc version")
    sizes = reader.u16()
    offset_size = sizes >> 12
    length_size = (sizes >> 8) & 0xF
    base_offset_size = (sizes >> 4) & 0xF
    index_size = sizes & 0xF if version in (1, 2) else 0
    if length_size != 4 or any(size not in (0, 4, 8) for size in (offset_size, base_offset_size, index_size)):
        raise ValueError("AVIF seed iloc widths differ from the pinned layout")
    item_count = reader.u16() if version < 2 else reader.u32()
    if item_count != 1:
        raise ValueError("AVIF seed must contain exactly one item")

    if version < 2:
        reader.u16()
    else:
        reader.u32()
    construction = reader.u16() if version in (1, 2) else 0
    if construction & 0xF != 0:
        raise ValueError("AVIF seed AV1 item is not file-backed")
    reader.u16()  # data_reference_index
    reader.uint(base_offset_size)
    if reader.u16() != 1:
        raise ValueError("AVIF seed must use one item extent")
    if index_size:
        reader.uint(index_size)
    reader.uint(offset_size)
    length_offset = reader.offset
    if reader.uint(length_size) != expected_length:
        raise ValueError("AVIF seed iloc extent differs from its AV1 item")
    if reader.offset != reader.end:
        raise ValueError("AVIF seed has trailing iloc fields")
    return length_offset


def patch_superres(seed: bytes) -> bytes:
    """Enable the size-clamped superres syntax in the pinned lossless seed."""
    seed_path = STAGING_ROOT / "lossless-monochrome-superres-seed.avif"
    seed_path.parent.mkdir(parents=True, exist_ok=True)
    seed_path.write_bytes(seed)
    report = inspect_av1(seed_path)
    sample_report = report["samples"]
    if len(sample_report) != 1 or len(sample_report[0]["spans"]) != 1:
        raise ValueError("AVIF seed must be one contiguous monochrome item")
    sample = sample_report[0]
    sequence_obu = next(obu for obu in sample["obus"] if "sequence_header" in obu)
    frame_obu = next(obu for obu in sample["obus"] if "frame_header" in obu)
    sequence = sequence_obu["sequence_header"]
    frame = frame_obu["frame_header"]
    if (
        sequence["profile"] != 0
        or not sequence["reduced_still_picture_header"]
        or sequence["max_width"] != WIDTH
        or sequence["max_height"] != HEIGHT
        or not sequence["monochrome"]
        or sequence["enable_superres"]
        or sequence["enable_restoration"]
        or frame["frame_type"] != "key"
        or not frame["all_lossless"]
        or frame["superres_enabled"]
        or frame["frame_width"] != WIDTH
        or frame["upscaled_width"] != WIDTH
        or frame["frame_height"] != HEIGHT
        or frame["header_bits"] != 16
    ):
        raise ValueError("AVIF seed differs from the pinned all-lossless 3x3 profile")

    item_span = sample["spans"][0]
    sample_offset = item_span["offset"]
    sample_length = item_span["length"]
    sample_bytes = bytearray(seed[sample_offset : sample_offset + sample_length])

    sequence_span = sequence_obu["payload_spans"][0]
    sequence_payload_offset = sequence_span["offset"] - sample_offset
    sequence_payload_length = sequence_obu["payload_length"]
    sequence_payload = bytearray(
        sample_bytes[
            sequence_payload_offset : sequence_payload_offset + sequence_payload_length
        ]
    )
    set_bit(sequence_payload, SEQUENCE_ENABLE_SUPERRES_BIT)
    sample_bytes[
        sequence_payload_offset : sequence_payload_offset + sequence_payload_length
    ] = sequence_payload

    frame_header_span = frame_obu["header_spans"][0]
    frame_start = frame_header_span["offset"] - sample_offset
    frame_payload_span = frame_obu["payload_spans"][0]
    frame_payload_offset = frame_payload_span["offset"] - sample_offset
    old_payload_length = frame_obu["payload_length"]
    if frame_header_span["length"] != 2 or frame_payload_offset != frame_start + 2:
        raise ValueError("AVIF seed frame OBU layout differs from the pinned syntax")
    if sample_bytes[frame_start + 1] != old_payload_length or old_payload_length >= 127:
        raise ValueError("AVIF seed frame OBU size is not a one-byte length")

    old_payload = bytes(
        sample_bytes[frame_payload_offset : frame_payload_offset + old_payload_length]
    )
    bits = "".join(f"{byte:08b}" for byte in old_payload)
    header_bits = frame["header_bits"]
    new_header = (
        bits[:FRAME_USE_SUPERRES_BIT]
        + "1"
        + f"{SUPERRES_DENOMINATOR - 9:03b}"
        + bits[FRAME_USE_SUPERRES_BIT:header_bits]
    )
    new_header += "0" * (-len(new_header) % 8)
    new_payload_bits = new_header + bits[header_bits:]
    new_payload = bytes(
        int(new_payload_bits[index : index + 8], 2)
        for index in range(0, len(new_payload_bits), 8)
    )
    if len(new_payload) != old_payload_length + 1:
        raise ValueError("superres frame-header insertion did not add one byte")

    frame_header = bytearray(sample_bytes[frame_start : frame_start + 2])
    frame_header[1] = len(new_payload)
    patched_sample = (
        sample_bytes[:frame_start]
        + frame_header
        + new_payload
        + sample_bytes[frame_payload_offset + old_payload_length :]
    )
    if len(patched_sample) != sample_length + 1:
        raise ValueError("patched AV1 item length differs from its extent update")

    top_level = parse_boxes(seed, 0, len(seed))
    media_data = [box for box in top_level if box.kind == b"mdat"]
    if len(media_data) != 1 or media_data[0].end != len(seed):
        raise ValueError("AVIF seed must end with one 32-bit mdat box")
    if media_data[0].size + 1 >= 1 << 32:
        raise ValueError("patched AVIF media data exceeds its 32-bit box size")
    length_offset = iloc_extent_length_offset(seed, sample_length)
    patched = bytearray(seed[:sample_offset] + patched_sample + seed[sample_offset + sample_length :])
    struct.pack_into(">I", patched, media_data[0].start, media_data[0].size + 1)
    struct.pack_into(">I", patched, length_offset, sample_length + 1)
    return bytes(patched)


def validate(path: Path) -> dict[str, object]:
    """Verify the AVIF structure, requested AV1 syntax, and Pillow pixels."""
    data = path.read_bytes()
    if sha256(data) != ASSET_SHA256:
        raise ValueError("fixture SHA-256 differs from its pin")
    oracle = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if oracle != PINNED_ORACLE:
        raise ValueError(f"Pillow/native version differs from pin: {oracle!r}")

    container = inspect_avif(path)
    color_items = container["items"]["color"]
    if (
        len(color_items) != 1
        or color_items[0]["type"] != "av01"
        or color_items[0]["length"] != 33
        or container["items"]["alpha"]
    ):
        raise ValueError("fixture is not the pinned single-item AV1 image")

    av1_report = inspect_av1(path)
    sequence = next(
        obu["sequence_header"]
        for sample in av1_report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    )
    frame = next(
        obu["frame_header"]
        for sample in av1_report["samples"]
        for obu in sample["obus"]
        if "frame_header" in obu
    )
    expected_sequence = {
        "profile": 0,
        "max_width": WIDTH,
        "max_height": HEIGHT,
        "monochrome": True,
        "enable_superres": True,
        "enable_restoration": False,
    }
    expected_frame = {
        "frame_type": "key",
        "frame_width": WIDTH,
        "frame_height": HEIGHT,
        "upscaled_width": WIDTH,
        "superres_enabled": True,
        "superres_denominator": SUPERRES_DENOMINATOR,
        "all_lossless": True,
    }
    if any(sequence.get(key) != value for key, value in expected_sequence.items()):
        raise ValueError("sequence header differs from the pinned clamped-superres profile")
    if any(frame.get(key) != value for key, value in expected_frame.items()):
        raise ValueError("frame header differs from the pinned lossless-superres profile")
    if frame.get("tiling", {}).get("columns") != 1 or frame.get("tiling", {}).get("rows") != 1:
        raise ValueError("fixture must use one tile")

    with Image.open(path) as image:
        if image.size != (WIDTH, HEIGHT) or image.mode != "RGB" or image.n_frames != 1:
            raise ValueError("Pillow must decode one 3x3 RGB frame")
        image.load()
        pixels = image.tobytes()
    pixel_hash = sha256(pixels)
    if len(pixels) != WIDTH * HEIGHT * 3 or pixel_hash != PIXEL_SHA256:
        raise ValueError("Pillow RGB pixels differ from the pinned exact reference")
    return {
        "asset": path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else "staging",
        "bytes": len(data),
        "sha256": sha256(data),
        "syntax": {"sequence_header": expected_sequence, "frame": expected_frame},
        "pixels": {"mode": "RGB", "bytes": len(pixels), "sha256": pixel_hash},
        "verified": True,
    }


def encode_command(avifenc: Path, source: Path, output: Path) -> list[str]:
    """Encode the unmodified lossless seed with pinned single-thread settings."""
    command = [
        str(avifenc),
        "-q",
        "100",
        "-s",
        "0",
        "-j",
        "1",
        "--tilecolslog2",
        "0",
        "--tilerowslog2",
        "0",
    ]
    for key, value in (
        ("superres-mode", "1"),
        ("superres-denominator", "9"),
        ("superres-kf-denominator", "9"),
        ("enable-restoration", "0"),
        ("min-partition-size", "4"),
        ("max-partition-size", "16"),
    ):
        command.extend(("--advanced", f"{key}={value}"))
    command.extend((str(source), str(output)))
    return command


def generate(avifenc: Path, output: Path) -> None:
    """Encode, validate, patch, and retain the deterministic public fixture."""
    avifenc = avifenc.expanduser().resolve()
    if not avifenc.is_file():
        raise ValueError(f"avifenc does not exist: {avifenc}")
    version = run([str(avifenc), "--version"])
    if "1.4.1" not in version or "3.13.2" not in version:
        raise ValueError("avifenc must use libavif 1.4.1 and libaom 3.13.2")

    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".lossless-mono-superres-", dir=STAGING_ROOT) as temporary:
        work = Path(temporary)
        source = work / "frame.y4m"
        source.write_bytes(source_y4m())
        seeds = []
        for index in range(2):
            seed_path = work / f"seed-{index}.avif"
            run(encode_command(avifenc, source, seed_path))
            seeds.append(seed_path.read_bytes())
        if seeds[0] != seeds[1]:
            raise ValueError("independent lossless AVIF seed encodes differ")
        if sha256(seeds[0]) != BASE_SHA256:
            raise ValueError("AVIF seed differs from its pinned encoder bytes")
        patched = patch_superres(seeds[0])
        if sha256(patched) != ASSET_SHA256:
            raise ValueError("patched AVIF differs from its pinned fixture bytes")
        temporary_asset = work / "fixture.avif"
        temporary_asset.write_bytes(patched)
        validate(temporary_asset)
        if output.exists() and output.read_bytes() != patched:
            raise ValueError(f"refusing to replace a different fixture at {output}")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(patched)
    print(json.dumps({**validate(output), "avifenc": str(avifenc), "version": version.strip()}, indent=2))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--avifenc", type=Path, help="Pinned libavif/libaom avifenc executable")
    parser.add_argument("--output", type=Path, help="Optional output below target/oracle-staging")
    parser.add_argument("--check", action="store_true", help="Verify the retained fixture")
    arguments = parser.parse_args()
    output = ASSET if arguments.output is None else arguments.output.expanduser().resolve()
    if arguments.check and arguments.avifenc is not None:
        parser.error("--check cannot be combined with --avifenc")
    if output != ASSET and STAGING_ROOT.resolve() not in output.parents:
        parser.error("--output must be the retained fixture or below target/oracle-staging")
    try:
        if arguments.check:
            print(json.dumps(validate(ASSET), indent=2))
        else:
            if arguments.avifenc is None:
                raise ValueError("--avifenc is required unless --check is used")
            generate(arguments.avifenc, output)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
