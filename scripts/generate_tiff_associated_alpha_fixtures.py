#!/usr/bin/env python3
"""Generate complete TIFFs exercising Pillow's associated RGBA transfer rules.

Stored samples include transparent hidden colors, partially transparent values
above their alpha, and opaque pixels. Strips and clipped edge tiles cover both
contiguous and separate planes, plus compressed per-plane prediction.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from io import BytesIO
from pathlib import Path
import struct
import zlib

import yaml
from PIL import Image

from generate_decode_refs import stable_error_message, verify_primary_oracle
from generate_test_assets import pack_lzw_codes_with_growth


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DIRECTORY = ROOT / "tests" / "fixtures" / "input" / "images" / "tiff"


@dataclass(frozen=True)
class Recipe:
    """One complete associated-alpha TIFF storage configuration."""

    name: str
    planar: int
    tiled: bool
    compression: int
    predictor: int = 1
    alpha_plane: str = "complete"

    @property
    def decode_error(self) -> bool:
        """Pillow's raw per-plane importer cannot unpack its lowercase alpha band."""
        return self.planar == 2 and self.compression == 1 and self.alpha_plane != "omitted"


RECIPES = tuple(
    Recipe(
        f"associated_rgba_{'separate' if planar == 2 else 'contiguous'}_"
        f"{'tiles' if tiled else 'strips'}_{compression_name}",
        planar, tiled, compression,
    )
    for planar in (1, 2)
    for tiled in (False, True)
    for compression, compression_name in ((1, "raw"), (8, "deflate"), (5, "lzw"), (32773, "packbits"))
) + (
    Recipe("associated_rgba_separate_strips_deflate_predictor", 2, False, 8, 2),
    Recipe("associated_rgba_separate_tiles_deflate_predictor", 2, True, 8, 2),
) + tuple(
    Recipe(
        f"associated_rgba_separate_{'tiles' if tiled else 'strips'}_raw_"
        f"{'missing_alpha_plane' if alpha_plane == 'omitted' else 'first_alpha_block'}",
        2, tiled, 1, alpha_plane=alpha_plane,
    )
    for tiled in (False, True)
    for alpha_plane in ("omitted", "first")
)


def stored_pixels(width: int, height: int) -> bytes:
    """Build stored RGBA samples independently of the normalized oracle pixels."""
    output = bytearray()
    alphas = (0, 1, 2, 17, 64, 127, 128, 192, 254, 255)
    boundary_pixels = ((11, 22, 33, 0), (0, 1, 2, 1), (1, 2, 3, 2), (17, 18, 255, 17))
    for y in range(height):
        for x in range(width):
            if y == 0 and x < len(boundary_pixels):
                output.extend(boundary_pixels[x])
                continue
            alpha = alphas[(x + y) % len(alphas)]
            output.extend(((x * 31 + y * 17 + channel * 59) & 255) * alpha // 255 for channel in range(3))
            output.append(alpha)
    return bytes(output)


def encode_block(raw: bytes, recipe: Recipe, row_width: int) -> bytes:
    """Encode one plane or interleaved block with the declared TIFF compression."""
    predicted = bytearray(raw)
    if recipe.predictor == 2:
        if recipe.planar != 2 or recipe.compression == 1:
            raise ValueError("associated-alpha prediction requires a compressed separate plane")
        for row in range(0, len(predicted), row_width):
            for column in range(row_width - 1, 0, -1):
                index = row + column
                predicted[index] = (predicted[index] - predicted[index - 1]) & 255
    if recipe.compression == 1:
        return bytes(predicted)
    if recipe.compression == 8:
        return zlib.compress(predicted)
    if recipe.compression == 5:
        return pack_lzw_codes_with_growth([256, *predicted, 257])
    if recipe.compression == 32773:
        output = bytearray()
        for start in range(0, len(predicted), 128):
            literal = predicted[start:start + 128]
            output.append(len(literal) - 1)
            output.extend(literal)
        return bytes(output)
    raise ValueError(f"unknown TIFF compression {recipe.compression}")


def build_tiff(recipe: Recipe) -> bytes:
    """Write classic little-endian IFDs and every declared strip or tile payload."""
    width, height = (65, 33) if recipe.tiled else (17, 9)
    raw = stored_pixels(width, height)
    rows_per_strip, tile_size = 4, 16
    blocks = []
    channels = range(4) if recipe.planar == 2 else (None,)
    for channel in channels:
        plane = raw[channel::4] if channel is not None else raw
        samples = 1 if channel is not None else 4
        if recipe.tiled:
            for tile_y in range(0, height, tile_size):
                for tile_x in range(0, width, tile_size):
                    block = bytearray(tile_size * tile_size * samples)
                    for row in range(min(tile_size, height - tile_y)):
                        copied = min(tile_size, width - tile_x) * samples
                        source = ((tile_y + row) * width + tile_x) * samples
                        destination = row * tile_size * samples
                        block[destination:destination + copied] = plane[source:source + copied]
                    blocks.append(encode_block(bytes(block), recipe, tile_size * samples))
        else:
            for row in range(0, height, rows_per_strip):
                block = plane[row * width * samples:min(row + rows_per_strip, height) * width * samples]
                blocks.append(encode_block(block, recipe, width * samples))

    offsets_tag, counts_tag = (324, 325) if recipe.tiled else (273, 279)
    entries = [
        (256, 4, [width]), (257, 4, [height]), (258, 3, [8, 8, 8, 8]),
        (259, 3, [recipe.compression]), (262, 3, [2]), (277, 3, [4]),
        (284, 3, [recipe.planar]), (338, 3, [1]),
        (offsets_tag, 4, [0] * len(blocks)),
        (counts_tag, 4, [len(block) for block in blocks]),
    ]
    if recipe.tiled:
        entries += [(322, 4, [tile_size]), (323, 4, [tile_size])]
    else:
        entries.append((278, 4, [rows_per_strip]))
    if recipe.predictor != 1:
        entries.append((317, 3, [recipe.predictor]))
    entries.sort()

    cursor = 8 + 2 + 12 * len(entries) + 4
    buffers = {}
    for tag, kind, values in entries:
        packed = struct.pack("<" + ("H" if kind == 3 else "I") * len(values), *values)
        if len(packed) > 4:
            buffers[tag] = (cursor, packed)
            cursor += len(packed)
            cursor += cursor % 2
    offsets = []
    for block in blocks:
        offsets.append(cursor)
        cursor += len(block)
    buffers[offsets_tag] = (buffers[offsets_tag][0], struct.pack("<" + "I" * len(offsets), *offsets))
    output = bytearray(b"II*\0\x08\0\0\0" + struct.pack("<H", len(entries)))
    for tag, kind, values in entries:
        packed = struct.pack("<" + ("H" if kind == 3 else "I") * len(values), *values)
        value = struct.pack("<I", buffers[tag][0]) if len(packed) > 4 else packed.ljust(4, b"\0")
        output.extend(struct.pack("<HHI", tag, kind, len(values)) + value)
    output.extend(bytes(4))
    for offset, buffer in sorted(buffers.values()):
        output.extend(bytes(offset - len(output)))
        output.extend(buffer)
    for block in blocks:
        output.extend(block)
    if recipe.alpha_plane != "complete":
        if recipe.planar != 2 or recipe.compression != 1:
            raise ValueError("the raw alpha-plane boundary requires uncompressed separate planes")
        declared_offsets = len(blocks) // 4 * 3 + (recipe.alpha_plane == "first")
        entry_index = next(index for index, entry in enumerate(entries) if entry[0] == offsets_tag)
        # Keep all component payloads and the byte-count entry intact. Only the
        # declared offset count controls whether Pillow visits the alpha plane.
        struct.pack_into("<I", output, 10 + entry_index * 12 + 4, declared_offsets)
    return bytes(output)


def verify_reference(data: bytes, recipe: Recipe) -> None:
    """Require fresh Pillow opens and declared public operation boundaries."""
    for operation in ("inspect", "verify", "decode", "decode_sequence"):
        with Image.open(BytesIO(data)) as image:
            expected_size = (65, 33) if recipe.tiled else (17, 9)
            if image.mode != "RGBA" or image.size != expected_size or image.n_frames != 1:
                raise RuntimeError(f"{recipe.name}: Pillow metadata changed")
            if operation == "verify":
                image.verify()
            elif operation.startswith("decode"):
                try:
                    image.load()
                    image.tobytes()
                except ValueError as error:
                    if not recipe.decode_error or stable_error_message(error) != "unknown raw mode for given image mode":
                        raise
                else:
                    if recipe.decode_error:
                        raise RuntimeError(f"{recipe.name}: Pillow unexpectedly accepted raw associated planes")


def generate_fixtures(directory: Path = DEFAULT_DIRECTORY, *, check: bool = False) -> None:
    """Generate deterministic complete inputs; keep all oracle pixels runner-owned."""
    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    directory.mkdir(parents=True, exist_ok=True)
    for recipe in RECIPES:
        data = build_tiff(recipe)
        verify_reference(data, recipe)
        destination = directory / (recipe.name + ".tiff")
        if check:
            if not destination.is_file() or destination.read_bytes() != data:
                raise RuntimeError(f"{destination}: deterministic associated-alpha input differs")
        else:
            destination.write_bytes(data)


def main() -> None:
    """Expose the same focused generation command as the normal TIFF asset hook."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-directory", type=Path, default=DEFAULT_DIRECTORY)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    generate_fixtures(arguments.output_directory, check=arguments.check)
    print(f"{'Checked' if arguments.check else 'Generated'} {len(RECIPES)} complete associated-alpha TIFFs")


if __name__ == "__main__":
    main()
