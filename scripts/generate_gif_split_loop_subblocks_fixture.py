#!/usr/bin/env python3
"""Generate GIFs with split and empty NETSCAPE2.0 loop payloads."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tests/fixtures/input/images/gif/animated.gif"
OUTPUT = ROOT / "tests/fixtures/input/images/gif/netscape_loop_split_subblocks.gif"
EMPTY_OUTPUT = ROOT / "tests/fixtures/input/images/gif/netscape_loop_empty_payload.gif"
EXPECTED_SOURCE_SHA256 = "237f29cf1246fb549a5005cfb230d6a392fe4bb8e7e25c3d77d66759112fd6cf"
EXPECTED_OUTPUT_SHA256 = "f37bed09e23a28e757415e9a2e238743ca8e14010bc8f2a9f4203560e193326a"
EXPECTED_EMPTY_OUTPUT_SHA256 = "20dad1519e7cd65f7f2aff71ca773bfedea1eb4d475b9ac795d0b71eb26202db"
SPLIT_LOOP_BLOCKS = bytes((1, 1, 2, 1, 0, 0))
EMPTY_LOOP_BLOCKS = bytes((0, 0))


def sha256_bytes(data: bytes) -> str:
    return sha256(data).hexdigest()


def checked_end(data: bytes, offset: int, length: int, context: str) -> int:
    if offset < 0 or length < 0 or offset > len(data) - length:
        raise ValueError(f"truncated GIF while reading {context}")
    return offset + length


def read_sub_blocks(
    data: bytes, offset: int, context: str
) -> tuple[list[bytes], int]:
    blocks = []
    while True:
        end_of_length = checked_end(data, offset, 1, f"{context} sub-block length")
        block_length = data[offset]
        offset = end_of_length
        if block_length == 0:
            return blocks, offset
        block_end = checked_end(data, offset, block_length, f"{context} sub-block")
        blocks.append(data[offset:block_end])
        offset = block_end


def gif_netscape_extension(
    data: bytes,
) -> tuple[int, list[bytes], tuple[int, int]]:
    """Walk GIF structure and return frames, loop blocks, and their full span."""
    if len(data) < 13 or data[:6] not in (b"GIF87a", b"GIF89a"):
        raise ValueError("input is not a GIF87a/GIF89a file")

    packed = data[10]
    offset = 13
    if packed & 0x80:
        color_table_entries = 1 << ((packed & 0x07) + 1)
        offset = checked_end(data, offset, 3 * color_table_entries, "global color table")

    frame_count = 0
    extension_span = None
    loop_blocks = None
    while True:
        marker_end = checked_end(data, offset, 1, "block marker")
        marker = data[offset]
        offset = marker_end
        if marker == 0x3B:
            if offset != len(data):
                raise ValueError("GIF trailer is not the end of the file")
            if extension_span is None or loop_blocks is None:
                raise ValueError("GIF lacks a NETSCAPE2.0 application extension")
            return frame_count, loop_blocks, extension_span
        if marker == 0x00:
            continue
        if marker == 0x21:
            label_end = checked_end(data, offset, 1, "extension label")
            label = data[offset]
            offset = label_end
            identifier = b""
            if label == 0xFF:
                identifier_length_end = checked_end(
                    data, offset, 1, "application identifier length"
                )
                identifier_length = data[offset]
                offset = identifier_length_end
                identifier_end = checked_end(
                    data, offset, identifier_length, "application identifier"
                )
                identifier = data[offset:identifier_end]
                offset = identifier_end
            sub_blocks_start = offset
            blocks, offset = read_sub_blocks(data, offset, "extension")
            if identifier == b"NETSCAPE2.0":
                if extension_span is not None:
                    raise ValueError("GIF contains duplicate NETSCAPE2.0 extensions")
                extension_span = (sub_blocks_start, offset)
                loop_blocks = blocks
            continue
        if marker == 0x2C:
            descriptor_end = checked_end(data, offset, 9, "image descriptor")
            image_packed = data[offset + 8]
            offset = descriptor_end
            if image_packed & 0x80:
                color_table_entries = 1 << ((image_packed & 0x07) + 1)
                offset = checked_end(data, offset, 3 * color_table_entries, "local color table")
            offset = checked_end(data, offset, 1, "LZW minimum code size")
            _, offset = read_sub_blocks(data, offset, "image data")
            frame_count += 1
            continue
        raise ValueError(f"unsupported GIF block marker 0x{marker:02x}")


def generate_fixture() -> bytes:
    source = SOURCE.read_bytes()
    source_hash = sha256_bytes(source)
    if source_hash != EXPECTED_SOURCE_SHA256:
        raise ValueError(f"source GIF SHA-256 changed: {source_hash}")

    frame_count, blocks, span = gif_netscape_extension(source)
    if frame_count != 2 or blocks != [bytes((1, 0, 0))]:
        raise ValueError(
            "source GIF must have two frames and one three-byte infinite loop record"
        )

    start, end = span
    result = source[:start] + SPLIT_LOOP_BLOCKS + source[end:]
    output_frame_count, output_blocks, output_span = gif_netscape_extension(result)
    if output_frame_count != 2 or output_blocks != [bytes((1,)), bytes((1, 0))]:
        raise ValueError("generated GIF does not retain the requested split sub-blocks")
    if output_span != (start, start + len(SPLIT_LOOP_BLOCKS)):
        raise ValueError("generated NETSCAPE2.0 sub-block layout has an unexpected span")

    result_hash = sha256_bytes(result)
    if result_hash != EXPECTED_OUTPUT_SHA256:
        raise ValueError(f"generated GIF SHA-256 changed: {result_hash}")
    return result


def generate_empty_payload_fixture() -> bytes:
    source = SOURCE.read_bytes()
    source_hash = sha256_bytes(source)
    if source_hash != EXPECTED_SOURCE_SHA256:
        raise ValueError(f"source GIF SHA-256 changed: {source_hash}")

    frame_count, blocks, span = gif_netscape_extension(source)
    if frame_count != 2 or blocks != [bytes((1, 0, 0))]:
        raise ValueError(
            "source GIF must have two frames and one three-byte infinite loop record"
        )

    start, end = span
    # Keep a second zero as tolerated top-level padding. Pillow's GIF reader
    # consumes it as the empty result from its extension-data loop, while the
    # decoder skips it as a zero block marker.
    result = source[:start] + EMPTY_LOOP_BLOCKS + source[end:]
    output_frame_count, output_blocks, output_span = gif_netscape_extension(result)
    if output_frame_count != 2 or output_blocks:
        raise ValueError("generated GIF does not have an empty NETSCAPE2.0 payload")
    if output_span != (start, start + 1) or result[start + 1] != 0:
        raise ValueError("generated empty NETSCAPE2.0 payload lacks its padding marker")

    result_hash = sha256_bytes(result)
    if result_hash != EXPECTED_EMPTY_OUTPUT_SHA256:
        raise ValueError(f"generated empty-payload GIF SHA-256 changed: {result_hash}")
    return result


def pillow_frames(
    data: bytes,
) -> tuple[
    str,
    tuple[int, int],
    int,
    int | None,
    bool,
    list[tuple[str, tuple[int, int], bytes]],
]:
    import PIL
    from PIL import Image

    if PIL.__version__ != "12.2.0":
        raise ValueError(f"Pillow 12.2.0 is required, found {PIL.__version__}")

    with Image.open(BytesIO(data)) as image:
        image.verify()

    with Image.open(BytesIO(data)) as image:
        frame_count = image.n_frames
        loop_count = image.info.get("loop")
        is_animated = image.is_animated
        mode = image.mode
        size = image.size
        frames = []
        for index in range(frame_count):
            image.seek(index)
            image.load()
            frames.append((image.mode, image.size, image.tobytes()))
        return mode, size, frame_count, loop_count, is_animated, frames


def verify_pillow_behavior(source: bytes, output: bytes) -> None:
    source_result = pillow_frames(source)
    output_result = pillow_frames(output)
    if source_result[:5] != ("P", (128, 128), 2, 0, True):
        raise ValueError(
            f"pinned Pillow returned unexpected source GIF metadata: {source_result[:5]}"
        )
    if output_result[:5] != ("P", (128, 128), 2, None, True):
        raise ValueError(
            f"pinned Pillow returned unexpected split-loop GIF metadata: {output_result[:5]}"
        )
    if source_result[5] != output_result[5]:
        raise ValueError("splitting the loop record changed Pillow-decoded frame pixels")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed deterministic fixture without rewriting it",
    )
    args = parser.parse_args()

    source = SOURCE.read_bytes()
    encoded = generate_fixture()
    empty_payload = generate_empty_payload_fixture()
    verify_pillow_behavior(source, encoded)
    verify_pillow_behavior(source, empty_payload)
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != encoded:
            raise ValueError(f"{OUTPUT.relative_to(ROOT)} differs from generated bytes")
        if not EMPTY_OUTPUT.is_file() or EMPTY_OUTPUT.read_bytes() != empty_payload:
            raise ValueError(f"{EMPTY_OUTPUT.relative_to(ROOT)} differs from generated bytes")
        print(
            "GIF loop-payload fixtures match Pillow 12.2.0 and their pinned full-file "
            f"SHA-256 values {EXPECTED_OUTPUT_SHA256} and {EXPECTED_EMPTY_OUTPUT_SHA256}"
        )
        return 0

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(encoded)
    EMPTY_OUTPUT.write_bytes(empty_payload)
    print(
        f"Wrote {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes, SHA-256 {EXPECTED_OUTPUT_SHA256})"
    )
    print(
        f"Wrote {EMPTY_OUTPUT.relative_to(ROOT)} "
        f"({len(empty_payload)} bytes, SHA-256 {EXPECTED_EMPTY_OUTPUT_SHA256})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
