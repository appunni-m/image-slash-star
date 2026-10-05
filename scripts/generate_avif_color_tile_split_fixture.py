#!/usr/bin/env python3
"""Generate pinned AV1 tile-group and skipped-CDEF public input fixtures."""

from __future__ import annotations

import argparse
import hashlib
import os
import struct
import subprocess
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as PILLOW_VERSION
from PIL import features

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tests/fixtures/input/images/avif/multitile.avif"
DESTINATION = ROOT / "tests/fixtures/input/images/avif/multitile_color_split_groups.avif"
SOURCE_SHA256 = "28bd09d7f17a15fcf3457eb21d2bebc36054718b20338191793e2d5faa61f253"
EXPECTED_OUTPUT_SHA256 = "654ef88dd8ebc71a554c03f721ad97581fcf2d2df0d825020d267939eb8f5e34"
CDEF_SKIPPED_DESTINATION = (
    ROOT / "tests/fixtures/input/images/avif/multitile_skipped_cdef_color_alpha.avif"
)
CDEF_SKIPPED_SOURCE_SHA256 = "85f8ba2ff1a674d02fc6208a36e740645517a058d811dc287aad10b35e9577a2"
CDEF_SKIPPED_SHA256 = "36c83e5e5104e74a8b3474b3ee313645a7bf86381d1578087e5cef3453d289eb"
CDEF_SKIPPED_PIXELS_SHA256 = "67d47633eeb4ab9211bfaddc84e6d5c09a958588867dcdc4b2169ad74b73fa0e"
AOM_COMMIT = "ad44980d7f3c7a2605c25d51ea96946949000841"


def encode_uleb128(value: int) -> bytes:
    encoded = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            byte |= 0x80
        encoded.append(byte)
        if not value:
            return bytes(encoded)


def split_frame(source: bytes, sample: dict[str, object]) -> bytes:
    from inspect_av1_obus import inspect as inspect_av1

    frame = next(obu for obu in sample["obus"] if obu["type"] == 6)
    tile_group = frame["tile_group"]
    tiles = tile_group["tiles"]
    frame_header_bits = frame["frame_header"]["header_bits"]
    tiling = frame["frame_header"]["tiling"]
    tile_index_bits = tiling["log2_columns"] + tiling["log2_rows"]
    if (
        (tile_group["start"], tile_group["end"]) != (0, 1)
        or [tile["index"] for tile in tiles] != [0, 1]
        or [tile["length"] for tile in tiles] != [3662, 3497]
        or frame_header_bits != 108
        or tile_index_bits != 1
        or tiles[0]["size_field"] is None
        or tiles[1]["size_field"] is not None
    ):
        raise RuntimeError("multi-tile color AV1 layout differs from its pinned fixture")

    payload_spans = frame["payload_spans"]
    if len(payload_spans) != 1:
        raise RuntimeError("multi-tile color frame payload is not contiguous")
    payload_span = payload_spans[0]
    frame_payload = source[
        payload_span["offset"] : payload_span["offset"] + payload_span["length"]
    ]
    header_length = (frame_header_bits + 7) // 8
    header = bytearray(frame_payload[:header_length])
    if frame_header_bits % 8 != 4 or header[-1] & 0x0F:
        raise RuntimeError("multi-tile color frame-header alignment differs")
    header[-1] |= 0x08  # AV1 trailing_bits for a standalone frame-header OBU.

    sample_spans = sample["spans"]
    if len(sample_spans) != 1:
        raise RuntimeError("multi-tile color item is not a single contiguous sample")
    sample_start = sample_spans[0]["offset"]
    sample_end = sample_start + sample["length"]
    frame_start = frame["header_spans"][0]["offset"]
    split_sample = bytearray(source[sample_start:frame_start])
    split_sample.extend((0x1A, len(header)))  # OBU_FRAME_HEADER plus its payload size.
    split_sample.extend(header)

    for tile in tiles:
        tile_payload = b"".join(
            source[span["offset"] : span["offset"] + span["length"]]
            for span in tile["physical_spans"]
        )
        if len(tile_payload) != tile["length"]:
            raise RuntimeError("multi-tile color payload span length differs")
        tile_index = tile["index"]
        header_bit_count = 1 + 2 * tile_index_bits
        header_bits = (
            (1 << (2 * tile_index_bits)) | (tile_index << tile_index_bits) | tile_index
        )
        group_payload = bytes((header_bits << (8 - header_bit_count),)) + tile_payload
        split_sample.extend((0x22,))  # OBU_TILE_GROUP.
        split_sample.extend(encode_uleb128(len(group_payload)))
        split_sample.extend(group_payload)

    if len(split_sample) != sample["length"] + 4:
        raise RuntimeError("split color tile-group AV1 sample has an unexpected length")
    return bytes(source[:sample_start] + split_sample + source[sample_end:])


def update_container_lengths(source: bytes, output: bytearray, item_offset: int, item_length: int) -> None:
    extent_marker = struct.pack(">II", item_offset, item_length)
    if source.count(extent_marker) != 1:
        raise RuntimeError("multi-tile color iloc extent marker is not unique")
    extent_length_offset = source.index(extent_marker) + 4
    if struct.unpack_from(">I", output, extent_length_offset)[0] != item_length:
        raise RuntimeError("multi-tile color iloc extent length differs")
    new_item_length = item_length + 4
    struct.pack_into(">I", output, extent_length_offset, new_item_length)

    mdat_type_offset = source.rfind(b"mdat")
    if mdat_type_offset < 4 or source.count(b"mdat") != 1:
        raise RuntimeError("multi-tile color mdat box is missing or ambiguous")
    mdat_size_offset = mdat_type_offset - 4
    mdat_size = struct.unpack_from(">I", source, mdat_size_offset)[0]
    if mdat_size != len(source) - mdat_size_offset:
        raise RuntimeError("multi-tile color mdat is not the terminal file box")
    struct.pack_into(">I", output, mdat_size_offset, mdat_size + 4)


def verify_output(source: bytes, output: bytes, temporary_path: Path) -> None:
    from inspect_av1_obus import inspect as inspect_av1

    report = inspect_av1(temporary_path)
    obus = report["samples"][0]["obus"]
    if [obu["type"] for obu in obus] != [2, 1, 3, 4, 4]:
        raise RuntimeError("split color tile-group AV1 OBU sequence differs")
    groups = [obu["tile_group"] for obu in obus if obu["type"] == 4]
    if [(group["start"], group["end"]) for group in groups] != [(0, 0), (1, 1)]:
        raise RuntimeError("split color tile-group AV1 ranges differ")

    if PILLOW_VERSION != "12.2.0" or features.version("avif") != "1.4.1":
        raise RuntimeError("Pillow 12.2.0 with AVIF 1.4.1 is required")
    with Image.open(BytesIO(source)) as source_image:
        source_image.load()
        source_result = (
            source_image.format,
            source_image.mode,
            source_image.size,
            source_image.info,
            source_image.tobytes(),
        )
    with Image.open(BytesIO(output)) as output_image:
        output_image.load()
        output_result = (
            output_image.format,
            output_image.mode,
            output_image.size,
            output_image.info,
            output_image.tobytes(),
        )
    if output_result != source_result:
        raise RuntimeError("split color tile-group AVIF changed Pillow's public image result")


def generate_fixture(source_path: Path = SOURCE, destination_path: Path = DESTINATION) -> str:
    source = source_path.read_bytes()
    source_sha256 = hashlib.sha256(source).hexdigest()
    if source_sha256 != SOURCE_SHA256:
        raise RuntimeError(f"multi-tile color source hash differs: {source_sha256}")

    from inspect_av1_obus import inspect as inspect_av1

    report = inspect_av1(source_path)
    sample = next(sample for sample in report["samples"] if sample["role"] == "item_color")
    split_sample_and_container = split_frame(source, sample)
    output = bytearray(split_sample_and_container)
    update_container_lengths(source, output, sample["spans"][0]["offset"], sample["length"])

    destination_path.parent.mkdir(parents=True, exist_ok=True)
    file_descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{destination_path.stem}.", suffix=".avif", dir=destination_path.parent
    )
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(file_descriptor, "wb") as temporary_file:
            temporary_file.write(output)
        verify_output(source, bytes(output), temporary_path)
        output_sha256 = hashlib.sha256(output).hexdigest()
        if EXPECTED_OUTPUT_SHA256 and output_sha256 != EXPECTED_OUTPUT_SHA256:
            raise RuntimeError(f"split color tile-group fixture hash differs: {output_sha256}")
        os.replace(temporary_path, destination_path)
    finally:
        temporary_path.unlink(missing_ok=True)
    return output_sha256


def replace_skipped_tiles(
    source: bytes, samples: list[dict], tiles: dict[str, bytes]
) -> bytes:
    """Replace two one-superblock tiles per item and preserve container bounds."""

    if (
        {sample["role"] for sample in samples} != {"item_color", "item_alpha"}
        or len(samples) != 2
    ):
        raise RuntimeError("expected one color and one alpha AV1 sample")
    replacements = []
    for sample in samples:
        spans = sample["spans"]
        if len(spans) != 1 or spans[0]["length"] != sample["length"]:
            raise RuntimeError("expected one contiguous item extent")
        start, length = spans[0]["offset"], sample["length"]
        if not 0 <= start < start + length <= len(source):
            raise RuntimeError("item extent is outside the source")
        frame = next(obu for obu in sample["obus"] if obu["type"] == 6)
        sequence = next(
            obu["sequence_header"]
            for obu in sample["obus"]
            if "sequence_header" in obu
        )
        header = frame["frame_header"]
        group = frame["tile_group"]
        frame_tiles = group["tiles"]
        if not (
            header["frame_width"] == 128
            and header["frame_height"] == 64
            and header["frame_type"] == "key"
            and not header["allow_screen_content_tools"]
            and not header["allow_intrabc"]
            and header["transform_mode"] == "largest"
            and not header["all_lossless"]
            and header["quantization"]["base"] == 4
            and sequence["enable_cdef"]
            and sequence["bit_depth"] == 8
            and sequence["monochrome"] == (sample["role"] == "item_alpha")
            and header["cdef"]["bits"] == 0
            and header["cdef"]["y_strengths"] == [0]
            and header["tiling"]["tile_size_bytes"] == 1
            and header["tiling"]["columns"] == 2
            and header["tiling"]["rows"] == 1
            and (group["start"], group["end"]) == (0, 1)
            and [tile["index"] for tile in frame_tiles] == [0, 1]
        ):
            raise RuntimeError("source does not match the bounded skipped-Square64 frame")
        first_size = frame_tiles[0]["size_field"]
        if (
            first_size is None
            or first_size["width"] != 1
            or len(first_size["physical_spans"]) != 1
        ):
            raise RuntimeError("first tile size is not one contiguous byte")
        if (
            frame_tiles[1]["size_field"] is not None
            or len(frame["payload_spans"]) != 1
        ):
            raise RuntimeError("frame payload or terminal tile size differs")
        payload_start = frame["payload_spans"][0]["offset"]
        payload_end = payload_start + frame["payload_length"]
        size_offset = first_size["physical_spans"][0]["offset"]
        frame_start = frame["header_spans"][0]["offset"]
        if not (
            start
            <= frame_start
            < payload_start
            <= size_offset
            < payload_end
            == start + length
        ):
            raise RuntimeError("frame/tile payload bounds differ")
        tile = tiles[sample["role"]]
        if not 1 <= len(tile) <= 64:
            raise RuntimeError("skipped tile length is outside the bounded encoder buffer")
        payload = (
            source[payload_start:size_offset] + bytes((len(tile) - 1,)) + tile + tile
        )
        new_sample = (
            source[start:frame_start] + b"\x32" + encode_uleb128(len(payload)) + payload
        )
        replacements.append((start, length, new_sample))

    output = bytearray(source)
    shift = 0
    previous_end = 0
    for start, length, new_sample in sorted(replacements):
        if start < previous_end:
            raise RuntimeError("item extents overlap")
        previous_end = start + length
        marker = struct.pack(">II", start, length)
        if source.count(marker) != 1:
            raise RuntimeError("item extent marker is not unique")
        extent_offset = source.index(marker)
        if extent_offset + 8 > min(item[0] for item in replacements):
            raise RuntimeError("iloc extent occurs inside item data")
        struct.pack_into(
            ">II", output, extent_offset, start + shift, len(new_sample)
        )
        shift += len(new_sample) - length
    if source.count(b"mdat") != 1:
        raise RuntimeError("expected one terminal mdat box")
    mdat_offset = source.index(b"mdat") - 4
    if mdat_offset < 0:
        raise RuntimeError("mdat box size is missing")
    mdat_size = struct.unpack_from(">I", source, mdat_offset)[0]
    if mdat_offset + mdat_size != len(source) or mdat_size + shift < 8:
        raise RuntimeError("terminal mdat size is invalid")
    struct.pack_into(">I", output, mdat_offset, mdat_size + shift)
    for start, length, new_sample in sorted(replacements, reverse=True):
        output[start : start + length] = new_sample
    return bytes(output)


def generate_skipped_cdef_fixture(
    destination_path: Path = CDEF_SKIPPED_DESTINATION,
    avifenc: Path = ROOT / "target/oracle-staging/libavif-superres-build/avifenc",
    aom_source: Path = ROOT / "target/oracle-staging/libaom",
    aom_build: Path = ROOT / "target/oracle-staging/libaom-build",
) -> str:
    """Encode a source twice and construct valid skipped tiles independently."""

    from inspect_av1_obus import inspect as inspect_av1

    if PILLOW_VERSION != "12.2.0" or features.version("avif") != "1.4.1":
        raise RuntimeError("Pillow 12.2.0 with AVIF 1.4.1 is required")
    version = subprocess.run(
        [str(avifenc), "--version"],
        check=True,
        capture_output=True,
        text=True,
    )
    if "Version: 1.4.1 (aom [enc]:3.13.2)" not in version.stdout:
        raise RuntimeError("expected pinned libavif 1.4.1/libaom 3.13.2 avifenc")
    revision = subprocess.run(
        ["git", "-C", str(aom_source), "rev-parse", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    )
    status = subprocess.run(
        ["git", "-C", str(aom_source), "status", "--porcelain"],
        check=True,
        capture_output=True,
        text=True,
    )
    if revision.stdout.strip() != AOM_COMMIT or status.stdout:
        raise RuntimeError("native entropy writer requires clean pinned libaom sources")

    with tempfile.TemporaryDirectory(prefix="avif-skipped-cdef-") as temporary:
        directory = Path(temporary)
        source_path = directory / "source.png"
        Image.new("RGBA", (128, 64), (128, 128, 128, 128)).save(source_path)
        native_encoder = directory / "encode_skipped_square64"
        subprocess.run(
            [
                os.environ.get("CC", "cc"),
                "-O2",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-I",
                str(aom_source),
                "-I",
                str(aom_build),
                str(ROOT / "scripts/avif_fixture_oracle/encode_skipped_square64.c"),
                str(aom_build / "libaom.a"),
                "-o",
                str(native_encoder),
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        generated = []
        for attempt in range(2):
            base_path = directory / f"base-{attempt}.avif"
            subprocess.run(
                [
                    str(avifenc),
                    "--depth",
                    "8",
                    "--yuv",
                    "420",
                    "--qcolor",
                    "99",
                    "--qalpha",
                    "99",
                    "--speed",
                    "8",
                    "--jobs",
                    "1",
                    "--tilecolslog2",
                    "1",
                    "--tilerowslog2",
                    "0",
                    "--advanced",
                    "enable-cdef=1",
                    "--advanced",
                    "min-partition-size=64",
                    "--advanced",
                    "max-partition-size=64",
                    str(source_path),
                    str(base_path),
                ],
                check=True,
                capture_output=True,
                text=True,
            )
            source = base_path.read_bytes()
            if hashlib.sha256(source).hexdigest() != CDEF_SKIPPED_SOURCE_SHA256:
                raise RuntimeError("skipped-CDEF source encode differs from the pinned bytes")
            native_tiles = {}
            for role, mode, expected in (
                ("item_color", "color", b"\x98\x80"),
                ("item_alpha", "monochrome", b"\x99"),
            ):
                tile_path = directory / f"{role}-{attempt}.bin"
                subprocess.run(
                    [str(native_encoder), mode, str(tile_path)],
                    check=True,
                    capture_output=True,
                    text=True,
                )
                tile = tile_path.read_bytes()
                if tile != expected:
                    raise RuntimeError("native skipped-Square64 entropy bytes differ")
                native_tiles[role] = tile
            output = replace_skipped_tiles(
                source, inspect_av1(base_path)["samples"], native_tiles
            )
            output_path = directory / f"output-{attempt}.avif"
            output_path.write_bytes(output)
            with Image.open(output_path) as image:
                image.load()
                if (image.format, image.mode, image.size) != (
                    "AVIF", "RGBA", (128, 64)
                ):
                    raise RuntimeError("skipped-CDEF Pillow output shape differs")
                pixels = image.tobytes()
                if pixels != bytes((128,)) * (128 * 64 * 4):
                    raise RuntimeError("skipped-CDEF Pillow output pixels differ")
                if hashlib.sha256(pixels).hexdigest() != CDEF_SKIPPED_PIXELS_SHA256:
                    raise RuntimeError("skipped-CDEF Pillow output digest differs")
            generated.append(output)
        if generated[0] != generated[1]:
            raise RuntimeError("skipped-CDEF fixture generation is not repeatable")
        digest = hashlib.sha256(generated[0]).hexdigest()
        if digest != CDEF_SKIPPED_SHA256:
            raise RuntimeError(f"skipped-CDEF fixture hash differs: {digest}")
        destination_path.parent.mkdir(parents=True, exist_ok=True)
        descriptor, temporary_name = tempfile.mkstemp(
            prefix=f".{destination_path.stem}.",
            suffix=".avif",
            dir=destination_path.parent,
        )
        temporary_path = Path(temporary_name)
        try:
            with os.fdopen(descriptor, "wb") as temporary_file:
                temporary_file.write(generated[0])
            os.replace(temporary_path, destination_path)
        finally:
            temporary_path.unlink(missing_ok=True)
        return digest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--profile",
        choices=("color-split", "cdef-skipped-square64"),
        default="color-split",
    )
    parser.add_argument("--output", type=Path)
    parser.add_argument(
        "--avifenc",
        type=Path,
        default=ROOT / "target/oracle-staging/libavif-superres-build/avifenc",
    )
    parser.add_argument(
        "--aom-source",
        type=Path,
        default=ROOT / "target/oracle-staging/libaom",
    )
    parser.add_argument(
        "--aom-build",
        type=Path,
        default=ROOT / "target/oracle-staging/libaom-build",
    )
    args = parser.parse_args()
    if args.profile == "color-split":
        print(generate_fixture(destination_path=args.output or DESTINATION))
    else:
        print(
            generate_skipped_cdef_fixture(
                destination_path=args.output or CDEF_SKIPPED_DESTINATION,
                avifenc=args.avifenc,
                aom_source=args.aom_source,
                aom_build=args.aom_build,
            )
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
