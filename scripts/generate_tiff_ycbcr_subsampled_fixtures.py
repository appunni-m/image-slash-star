#!/usr/bin/env python3
"""Generate Pillow-parity fixtures for TIFF YCbCr 2-by-2 LZW strips."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path
import struct

import yaml
from PIL import Image, __version__ as PILLOW_VERSION

from generate_decode_refs import verify_primary_oracle
from generate_test_assets import pack_lzw_codes_with_growth


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DIRECTORY = ROOT / "tests" / "fixtures" / "input" / "images" / "tiff"
PINNED_PILLOW = "12.2.0"
REFERENCE_PIXELS = bytes.fromhex(
    "004000004000002f000b61003da4000b72003d93000b6100001e003d8232000c276fa2bd3d82320b50003d708b0b3e59"
)
REFERENCE_PIXEL_SHA256 = sha256(REFERENCE_PIXELS).hexdigest()

REFERENCE_CANDIDATE = bytes.fromhex(
    "49 49 2a 00 1e 00 00 00 "
    "80 00 00 06 41 94 0c 00 32 82 42 20 70 43 21 90 00 96 82 44 20 20 "
    "0c 00 "
    "00 01 03 00 01 00 00 00 04 00 00 00 "
    "01 01 03 00 01 00 00 00 04 00 00 00 "
    "02 01 03 00 03 00 00 00 b4 00 00 00 "
    "03 01 03 00 01 00 00 00 05 00 00 00 "
    "06 01 03 00 01 00 00 00 06 00 00 00 "
    "11 01 04 00 01 00 00 00 08 00 00 00 "
    "15 01 03 00 01 00 00 00 03 00 00 00 "
    "16 01 03 00 01 00 00 00 04 00 00 00 "
    "17 01 04 00 01 00 00 00 16 00 00 00 "
    "1c 01 03 00 01 00 00 00 01 00 00 00 "
    "12 02 03 00 02 00 00 00 02 00 02 00 "
    "13 02 03 00 01 00 00 00 01 00 00 00 "
    "00 00 00 00 08 00 08 00 08 00"
)

ODD_EDGE_CLUMPS = (
    (80, 96, 112, 128, 90, 180),
    (144, 160, 176, 192, 130, 30),
    (208, 224, 240, 0, 64, 200),
    (16, 32, 48, 64, 192, 70),
)

TWO_STRIP_CLUMPS = (
    (20, 40, 60, 80, 70, 160),
    (100, 120, 140, 160, 180, 30),
    (180, 200, 220, 240, 100, 220),
    (4, 24, 44, 64, 210, 50),
)


def lzw_blocks(
    width: int, height: int, rows_per_strip: int, clumps: tuple[tuple[int, ...], ...]
) -> list[bytes]:
    """Encode one TIFF LZW stream for each strip of complete clump rows."""
    if width <= 0 or height <= 0:
        raise ValueError("YCbCr image dimensions must be positive")
    if rows_per_strip <= 0 or rows_per_strip % 2 != 0:
        raise ValueError("YCbCr RowsPerStrip must be a positive even value")
    clump_columns = (width + 1) // 2
    clump_rows = (height + 1) // 2
    clump_rows_per_strip = rows_per_strip // 2
    if len(clumps) != clump_columns * clump_rows:
        raise ValueError("YCbCr clump count does not match image geometry")
    if any(
        len(clump) != 6 or any(not 0 <= sample <= 255 for sample in clump)
        for clump in clumps
    ):
        raise ValueError("each YCbCr clump must contain six 8-bit samples")

    blocks = []
    for first_row in range(0, clump_rows, clump_rows_per_strip):
        start = first_row * clump_columns
        end = min(first_row + clump_rows_per_strip, clump_rows) * clump_columns
        samples = bytes(value for clump in clumps[start:end] for value in clump)
        blocks.append(pack_lzw_codes_with_growth([256, *samples, 257]))
    return blocks


def build_tiff(
    width: int,
    height: int,
    rows_per_strip: int,
    clumps: tuple[tuple[int, ...], ...],
    *,
    subsampling: tuple[int, int] | None,
    positioning: int | None,
) -> bytes:
    """Write a compact contiguous YCbCr TIFF with independent LZW strips."""
    blocks = lzw_blocks(width, height, rows_per_strip, clumps)
    entries = [
        (256, 4, [width]),
        (257, 4, [height]),
        (258, 3, [8, 8, 8]),
        (259, 3, [5]),
        (262, 3, [6]),
        (273, 4, [0] * len(blocks)),
        (277, 3, [3]),
        (278, 4, [rows_per_strip]),
        (279, 4, [len(block) for block in blocks]),
        (284, 3, [1]),
    ]
    if subsampling is not None:
        entries.append((530, 3, list(subsampling)))
    if positioning is not None:
        entries.append((531, 3, [positioning]))
    entries.sort()

    cursor = 8 + 2 + 12 * len(entries) + 4
    buffers: dict[int, tuple[int, bytes]] = {}
    for tag, field_type, values in entries:
        packed = struct.pack(
            "<" + ("H" if field_type == 3 else "I") * len(values), *values
        )
        if len(packed) > 4:
            cursor += cursor % 2
            buffers[tag] = (cursor, packed)
            cursor += len(packed)

    block_offsets = []
    for block in blocks:
        cursor += cursor % 2
        block_offsets.append(cursor)
        cursor += len(block)
    if len(block_offsets) > 1:
        offsets_position, _ = buffers[273]
        buffers[273] = (
            offsets_position,
            struct.pack("<" + "I" * len(block_offsets), *block_offsets),
        )

    output = bytearray(b"II*\0" + struct.pack("<I", 8) + struct.pack("<H", len(entries)))
    for tag, field_type, values in entries:
        if tag == 273:
            values = block_offsets
        packed = struct.pack(
            "<" + ("H" if field_type == 3 else "I") * len(values), *values
        )
        value = (
            struct.pack("<I", buffers[tag][0])
            if len(packed) > 4
            else packed.ljust(4, b"\0")
        )
        output.extend(struct.pack("<HHI", tag, field_type, len(values)) + value)
    output.extend(bytes(4))
    for offset, buffer in sorted(buffers.values()):
        output.extend(bytes(offset - len(output)))
        output.extend(buffer)
    for block in blocks:
        if len(output) % 2:
            output.append(0)
        output.extend(block)
    return bytes(output)


def build_full_resolution_lzw_tiff(width: int, height: int, pixels: bytes) -> bytes:
    """Create a deterministic Pillow-written 1-by-1 YCbCr LZW source."""
    source = Image.frombytes("YCbCr", (width, height), pixels)
    output = BytesIO()
    source.save(
        output,
        format="TIFF",
        compression="tiff_lzw",
        tiffinfo={262: 6, 277: 3, 530: (1, 1)},
    )
    return output.getvalue()


def fixtures() -> dict[
    str, tuple[bytes, tuple[int, int], str, dict[int, object], bytes | None]
]:
    """Return deterministic inputs and the pinned Pillow result metadata."""
    odd_edge = build_tiff(
        3,
        3,
        4,
        ODD_EDGE_CLUMPS,
        subsampling=None,
        positioning=None,
    )
    two_strips = build_tiff(
        4,
        4,
        2,
        TWO_STRIP_CLUMPS,
        subsampling=(2, 2),
        positioning=2,
    )
    full_resolution_pixels = bytes(
        value
        for y in range(4)
        for x in range(4)
        for value in (30 + 40 * x + 20 * y, 96 + 9 * x + 13 * y, 160 - 7 * x + 5 * y)
    )
    full_resolution = build_full_resolution_lzw_tiff(4, 4, full_resolution_pixels)
    checkpoint_clumps = tuple(
        (
            (index * 17 + 11) % 256,
            (index * 29 + 23) % 256,
            (index * 37 + 47) % 256,
            (index * 53 + 71) % 256,
            (index * 11 + 80) % 256,
            (index * 19 + 128) % 256,
        )
        for index in range(16 * 16)
    )
    checkpoint = build_tiff(
        32,
        32,
        32,
        checkpoint_clumps,
        subsampling=(2, 2),
        positioning=1,
    )
    return {
        "ycbcr_subsampled_lzw_2x2.tiff": (
            REFERENCE_CANDIDATE,
            (4, 4),
            REFERENCE_PIXEL_SHA256,
            {530: (2, 2), 531: 1},
            REFERENCE_PIXELS,
        ),
        "ycbcr_subsampled_lzw_2x2_odd_edge.tiff": (
            odd_edge,
            (3, 3),
            "027f5f35676c24314a2fefac2bb3fb6e8b06b144a96fd3f55b5c4b057473e673",
            {},
            None,
        ),
        "ycbcr_subsampled_lzw_2x2_two_strips.tiff": (
            two_strips,
            (4, 4),
            "7a2b02a725a8988a4e716873f80fe8290c26e274536c6716b4474c31b8379df4",
            {530: (2, 2), 531: 2},
            None,
        ),
        "ycbcr_full_resolution_lzw_1x1.tiff": (
            full_resolution,
            (4, 4),
            "a9ef8ac10e78a041a3b442685d6a85cc153d136972866ebd10fb03804bd13a7e",
            {530: (1, 1)},
            None,
        ),
        "ycbcr_subsampled_lzw_2x2_checkpoint_32x32.tiff": (
            checkpoint,
            (32, 32),
            "c5fc0bdafed814b597605fb7d00ce7510c60e1b45e774d0c71c4850ec39b4ded",
            {530: (2, 2), 531: 1},
            None,
        ),
    }


def verify_pillow_reference(
    name: str,
    data: bytes,
    expected_size: tuple[int, int],
    expected_pixel_sha256: str,
    expected_tags: dict[int, object],
    expected_pixels: bytes | None,
) -> None:
    """Check the fixture's declared YCbCr storage and exact Pillow pixels."""
    if PILLOW_VERSION != PINNED_PILLOW:
        raise RuntimeError(f"Pillow {PINNED_PILLOW} is required, found {PILLOW_VERSION}")
    with Image.open(BytesIO(data)) as verification_image:
        verification_image.verify()
    with Image.open(BytesIO(data)) as image:
        if image.size != expected_size:
            raise RuntimeError(f"{name}: unexpected Pillow dimensions {image.size}")
        tags = image.tag_v2
        actual_tags = {
            259: tags.get(259, 1),
            262: tags.get(262, 1),
            277: tags.get(277, 1),
            284: tags.get(284, 1),
            530: tags.get(530),
            531: tags.get(531),
        }
        expected = {
            259: 5,
            262: 6,
            277: 3,
            284: 1,
            530: expected_tags.get(530),
            531: expected_tags.get(531),
        }
        if actual_tags != expected:
            raise RuntimeError(f"{name}: TIFF tags differ: {actual_tags!r}")
        image.load()
        pixels = image.tobytes()
        actual_pixel_sha256 = sha256(pixels).hexdigest()
        if (
            image.mode != "RGB"
            or actual_pixel_sha256 != expected_pixel_sha256
            or (expected_pixels is not None and pixels != expected_pixels)
        ):
            raise RuntimeError(
                f"{name}: Pillow RGB reference changed "
                f"(mode={image.mode}, sha256={actual_pixel_sha256})"
            )


def generate_fixtures(directory: Path = DEFAULT_DIRECTORY, *, check: bool = False) -> None:
    """Generate input bytes and require a matching pinned Pillow oracle."""
    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    inputs = fixtures()
    for name, (data, size, pixel_hash, tags, expected_pixels) in inputs.items():
        expected_input_hash = {
            "ycbcr_subsampled_lzw_2x2.tiff": "d9c3e4b7111658f3b86fdb1f94bf04e03aebe7cb4fbd01a98708ebfd27e65a6c",
            "ycbcr_subsampled_lzw_2x2_odd_edge.tiff": "67df0dce7b4cab6af8d43f258f7eb1b77ba119e096ad242083baec83ebd585e9",
            "ycbcr_subsampled_lzw_2x2_two_strips.tiff": "1dd542a0a6e22d922f66e0f19e92421dd15fd1108f726af38810a11a6bf13487",
            "ycbcr_full_resolution_lzw_1x1.tiff": "c3ebf5fdd35da227f9ef92cf45b81d726c908a0be1fe3e52c1bd3abfe699f9b6",
            "ycbcr_subsampled_lzw_2x2_checkpoint_32x32.tiff": "655b9b0172e44433b222cc93acdd2e14ca296e22f373e5f073e2e24d33b5111f",
        }[name]
        actual_input_hash = sha256(data).hexdigest()
        if actual_input_hash != expected_input_hash:
            raise RuntimeError(
                f"{name}: input SHA-256 changed "
                f"(expected {expected_input_hash}, found {actual_input_hash})"
            )
        verify_pillow_reference(name, data, size, pixel_hash, tags, expected_pixels)
    if check:
        for name, (data, _, _, _, _) in inputs.items():
            destination = directory / name
            if not destination.is_file() or destination.read_bytes() != data:
                raise RuntimeError(f"{destination}: deterministic YCbCr fixture differs")
        return

    directory.mkdir(parents=True, exist_ok=True)
    for name, (data, _, _, _, _) in inputs.items():
        destination = directory / name
        destination.write_bytes(data)


def main() -> None:
    """Expose fixture generation and deterministic input verification."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-directory", type=Path, default=DEFAULT_DIRECTORY)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    generate_fixtures(arguments.output_directory, check=arguments.check)
    action = "Checked" if arguments.check else "Generated"
    print(f"{action} {len(fixtures())} TIFF YCbCr fixtures")


if __name__ == "__main__":
    main()
