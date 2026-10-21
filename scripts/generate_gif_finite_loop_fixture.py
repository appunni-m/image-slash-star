#!/usr/bin/env python3
"""Generate a GIF parity fixture with the maximum finite NETSCAPE repeat count."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tests/fixtures/input/images/gif/animated.gif"
OUTPUT = ROOT / "tests/fixtures/input/images/gif/finite_loop_u16_max.gif"
EXPECTED_SOURCE_SHA256 = "237f29cf1246fb549a5005cfb230d6a392fe4bb8e7e25c3d77d66759112fd6cf"
EXPECTED_OUTPUT_SHA256 = "fe32a8a09ef03cb17ee55e231435cfb7cef35705dc91331c08828a5a57120a9b"
MAXIMUM_FINITE_REPEATS = 0xFFFF


def sha256_bytes(data: bytes) -> str:
    return sha256(data).hexdigest()


def checked_end(data: bytes, offset: int, length: int, context: str) -> int:
    if offset < 0 or length < 0 or offset > len(data) - length:
        raise ValueError(f"truncated GIF while reading {context}")
    return offset + length


def read_sub_blocks(
    data: bytes, offset: int, context: str
) -> tuple[bytes, list[int], list[int], int]:
    payload = bytearray()
    payload_offsets: list[int] = []
    block_lengths: list[int] = []

    while True:
        end_of_length = checked_end(data, offset, 1, f"{context} sub-block length")
        block_length = data[offset]
        offset = end_of_length
        if block_length == 0:
            return bytes(payload), payload_offsets, block_lengths, offset

        block_end = checked_end(data, offset, block_length, f"{context} sub-block")
        payload.extend(data[offset:block_end])
        payload_offsets.extend(range(offset, block_end))
        block_lengths.append(block_length)
        offset = block_end


def gif_loop_field(data: bytes) -> tuple[int, int | None, tuple[int, int] | None]:
    """Walk complete GIF blocks and locate a NETSCAPE2.0 repeat field."""
    if len(data) < 13 or data[:6] not in (b"GIF87a", b"GIF89a"):
        raise ValueError("input is not a complete GIF87a/GIF89a header")

    packed = data[10]
    offset = 13
    if packed & 0x80:
        color_table_entries = 1 << ((packed & 0x07) + 1)
        offset = checked_end(
            data, offset, 3 * color_table_entries, "global color table"
        )

    frame_count = 0
    loop_count: int | None = None
    loop_offsets: tuple[int, int] | None = None

    while True:
        end_of_marker = checked_end(data, offset, 1, "block marker")
        marker = data[offset]
        offset = end_of_marker

        if marker == 0x3B:
            if offset != len(data):
                raise ValueError("GIF trailer is not the end of the source file")
            return frame_count, loop_count, loop_offsets

        if marker == 0x00:
            continue

        if marker == 0x21:
            end_of_label = checked_end(data, offset, 1, "extension label")
            label = data[offset]
            offset = end_of_label

            identifier = b""
            if label == 0xFF:
                end_of_length = checked_end(
                    data, offset, 1, "application identifier length"
                )
                identifier_length = data[offset]
                offset = end_of_length
                identifier_end = checked_end(
                    data, offset, identifier_length, "application identifier"
                )
                identifier = data[offset:identifier_end]
                offset = identifier_end

            payload, payload_offsets, block_lengths, offset = read_sub_blocks(
                data, offset, "extension"
            )
            if identifier == b"NETSCAPE2.0":
                if len(identifier) != 11:
                    raise ValueError("NETSCAPE2.0 identifier must contain 11 bytes")
                if loop_count is not None:
                    raise ValueError("source contains duplicate NETSCAPE2.0 extensions")
                if block_lengths != [3] or len(payload) != 3 or payload[0] != 1:
                    raise ValueError(
                        "NETSCAPE2.0 repeat field must be one three-byte loop sub-block"
                    )
                if len(payload_offsets) != 3:
                    raise ValueError("NETSCAPE2.0 payload offsets are incomplete")
                loop_count = int.from_bytes(payload[1:3], "little")
                loop_offsets = (payload_offsets[1], payload_offsets[2])
            continue

        if marker == 0x2C:
            descriptor_end = checked_end(data, offset, 9, "image descriptor")
            image_packed = data[offset + 8]
            offset = descriptor_end
            if image_packed & 0x80:
                color_table_entries = 1 << ((image_packed & 0x07) + 1)
                offset = checked_end(
                    data, offset, 3 * color_table_entries, "local color table"
                )
            offset = checked_end(data, offset, 1, "LZW minimum code size")
            _, _, _, offset = read_sub_blocks(data, offset, "image data")
            frame_count += 1
            continue

        raise ValueError(f"unsupported GIF block marker 0x{marker:02x}")


def generate_fixture() -> bytes:
    source = SOURCE.read_bytes()
    source_hash = sha256_bytes(source)
    if source_hash != EXPECTED_SOURCE_SHA256:
        raise ValueError(f"source GIF SHA-256 changed: {source_hash}")

    frame_count, loop_count, loop_offsets = gif_loop_field(source)
    if frame_count != 2 or loop_count != 0 or loop_offsets is None:
        raise ValueError(
            "source GIF must have two frames and an infinite NETSCAPE2.0 extension"
        )

    encoded = bytearray(source)
    first, second = loop_offsets
    encoded[first] = MAXIMUM_FINITE_REPEATS & 0xFF
    encoded[second] = MAXIMUM_FINITE_REPEATS >> 8
    result = bytes(encoded)

    changed_offsets = [
        index
        for index, (before, after) in enumerate(zip(source, result))
        if before != after
    ]
    if changed_offsets != [first, second]:
        raise ValueError(
            "fixture generation must change only the two NETSCAPE2.0 repeat bytes"
        )

    frame_count, loop_count, _ = gif_loop_field(result)
    if frame_count != 2 or loop_count != MAXIMUM_FINITE_REPEATS:
        raise ValueError("generated GIF does not retain the two-frame maximum finite loop")
    result_hash = sha256_bytes(result)
    if result_hash != EXPECTED_OUTPUT_SHA256:
        raise ValueError(f"generated GIF SHA-256 changed: {result_hash}")
    return result


def pillow_frames(
    data: bytes,
) -> tuple[str, tuple[int, int], int, int | None, bool, list[bytes]]:
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
        frames: list[bytes] = []
        for index in range(frame_count):
            image.seek(index)
            image.load()
            frames.append(image.tobytes())
        return mode, size, frame_count, loop_count, is_animated, frames


def verify_pillow_behavior(source: bytes, output: bytes) -> None:
    source_result = pillow_frames(source)
    output_result = pillow_frames(output)
    if source_result[:5] != ("P", (128, 128), 2, 0, True):
        raise ValueError(
            f"pinned Pillow returned unexpected source GIF metadata: {source_result[:5]}"
        )
    if output_result[:5] != ("P", (128, 128), 2, MAXIMUM_FINITE_REPEATS, True):
        raise ValueError(
            f"pinned Pillow returned unexpected output GIF metadata: {output_result[:5]}"
        )
    if source_result[5] != output_result[5]:
        raise ValueError("changing the repeat count changed Pillow-decoded frame pixels")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed deterministic fixture without rewriting it",
    )
    args = parser.parse_args()

    encoded = generate_fixture()
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != encoded:
            raise ValueError(f"{OUTPUT.relative_to(ROOT)} differs from generated bytes")
        print(
            "GIF maximum finite-repeat fixture matches its structurally generated "
            f"full-file SHA-256 {EXPECTED_OUTPUT_SHA256}"
        )
        return 0

    verify_pillow_behavior(SOURCE.read_bytes(), encoded)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(encoded)
    print(
        f"Wrote {OUTPUT.relative_to(ROOT)} "
        f"({len(encoded)} bytes, SHA-256 {EXPECTED_OUTPUT_SHA256})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
