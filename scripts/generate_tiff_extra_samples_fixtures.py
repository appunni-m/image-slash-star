#!/usr/bin/env python3
"""Generate complete TIFF inputs for unspecified RGB and grayscale extra samples.

The family keeps every stored plane, including ignored trailing samples. Pixel
references and Pillow's unsupported-layout errors belong to the normal reference
producer; this generator writes only input bytes and storage declarations.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import struct
import zlib

import yaml

from generate_decode_refs import verify_primary_oracle
from generate_test_assets import pack_lzw_codes_with_growth


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DIRECTORY = ROOT / "tests" / "fixtures" / "input" / "images" / "tiff"
WIDTH, HEIGHT = 17, 9
ROWS_PER_STRIP, TILE_SIZE = 4, 16


@dataclass(frozen=True)
class Recipe:
    """One complete classic TIFF storage configuration."""

    family: str
    photometric: int
    samples: int
    extra_sample: int
    planar: int
    tiled: bool
    compression: int
    compression_name: str
    width: int = WIDTH
    height: int = HEIGHT

    @property
    def name(self) -> str:
        organization = "separate" if self.planar == 2 else "contiguous"
        storage = "tiles" if self.tiled else "strips"
        return f"{self.family}_{organization}_{storage}_{self.compression_name}"


RECIPES = tuple(
    Recipe(family, photometric, samples, extra_sample, planar, tiled, compression, compression_name)
    for family, photometric, samples, extra_sample in (
        ("extra_samples_zero_rgbx", 2, 4, 0),
        ("extra_samples_zero_grayx", 1, 2, 0),
        ("associated_gray_alpha", 1, 2, 1),
        ("straight_gray_alpha", 1, 2, 2),
    )
    for planar in (1, 2)
    for tiled in (False, True)
    for compression, compression_name in ((1, "raw"), (8, "deflate"), (5, "lzw"), (32773, "packbits"))
) + tuple(
    Recipe(f"extra_samples_zero_rgbx_row_{width}", 2, 4, 0, 1, False, 1, "raw", width, 1)
    for width in (255, 256, 257)
)


def stored_pixels(recipe: Recipe) -> bytes:
    """Build stored samples without normalized or reference pixel expectations."""
    output = bytearray()
    alphas = (0, 1, 2, 17, 64, 127, 128, 192, 254, 255)
    for y in range(recipe.height):
        for x in range(recipe.width):
            extra = alphas[(x + y) % len(alphas)]
            for channel in range(recipe.samples - 1):
                value = (x * 31 + y * 17 + channel * 59) & 255
                output.append(value * extra // 255 if recipe.extra_sample == 1 else value)
            output.append(extra)
    return bytes(output)


def encode_block(raw: bytes, compression: int) -> bytes:
    """Encode a complete block with the declared lossless TIFF compression."""
    if compression == 1:
        return raw
    if compression == 8:
        return zlib.compress(raw)
    if compression == 5:
        return pack_lzw_codes_with_growth([256, *raw, 257])
    if compression == 32773:
        output = bytearray()
        for start in range(0, len(raw), 128):
            literal = raw[start:start + 128]
            output.append(len(literal) - 1)
            output.extend(literal)
        return bytes(output)
    raise ValueError(f"unknown TIFF compression {compression}")


def build_tiff(recipe: Recipe) -> bytes:
    """Write one IFD and every declared strip or padded edge-tile payload."""
    raw = stored_pixels(recipe)
    width, height = recipe.width, recipe.height
    blocks = []
    channels = range(recipe.samples) if recipe.planar == 2 else (None,)
    for channel in channels:
        plane = raw[channel::recipe.samples] if channel is not None else raw
        samples = 1 if channel is not None else recipe.samples
        if recipe.tiled:
            for tile_y in range(0, height, TILE_SIZE):
                for tile_x in range(0, width, TILE_SIZE):
                    block = bytearray(TILE_SIZE * TILE_SIZE * samples)
                    for row in range(min(TILE_SIZE, height - tile_y)):
                        copied = min(TILE_SIZE, width - tile_x) * samples
                        source = ((tile_y + row) * width + tile_x) * samples
                        destination = row * TILE_SIZE * samples
                        block[destination:destination + copied] = plane[source:source + copied]
                    blocks.append(encode_block(bytes(block), recipe.compression))
        else:
            for row in range(0, height, ROWS_PER_STRIP):
                block = plane[row * width * samples:min(row + ROWS_PER_STRIP, height) * width * samples]
                blocks.append(encode_block(block, recipe.compression))

    offsets_tag, counts_tag = (324, 325) if recipe.tiled else (273, 279)
    entries = [
        (256, 4, [width]), (257, 4, [height]), (258, 3, [8] * recipe.samples),
        (259, 3, [recipe.compression]), (262, 3, [recipe.photometric]),
        (277, 3, [recipe.samples]), (284, 3, [recipe.planar]),
        (338, 3, [recipe.extra_sample]), (offsets_tag, 4, [0] * len(blocks)),
        (counts_tag, 4, [len(block) for block in blocks]),
    ]
    entries += [(322, 4, [TILE_SIZE]), (323, 4, [TILE_SIZE])] if recipe.tiled else [(278, 4, [ROWS_PER_STRIP])]
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
    if len(offsets) > 1:
        buffers[offsets_tag] = (buffers[offsets_tag][0], struct.pack("<" + "I" * len(offsets), *offsets))
    output = bytearray(b"II*\0\x08\0\0\0" + struct.pack("<H", len(entries)))
    for tag, kind, values in entries:
        if tag == offsets_tag:
            values = offsets
        packed = struct.pack("<" + ("H" if kind == 3 else "I") * len(values), *values)
        value = struct.pack("<I", buffers[tag][0]) if len(packed) > 4 else packed.ljust(4, b"\0")
        output.extend(struct.pack("<HHI", tag, kind, len(values)) + value)
    output.extend(bytes(4))
    for offset, buffer in sorted(buffers.values()):
        output.extend(bytes(offset - len(output)))
        output.extend(buffer)
    for block in blocks:
        output.extend(block)
    return bytes(output)


def generate_fixtures(directory: Path = DEFAULT_DIRECTORY, *, check: bool = False) -> None:
    """Generate deterministic inputs; the reference producer owns observations."""
    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    directory.mkdir(parents=True, exist_ok=True)
    for recipe in RECIPES:
        destination = directory / (recipe.name + ".tiff")
        data = build_tiff(recipe)
        if check:
            if not destination.is_file() or destination.read_bytes() != data:
                raise RuntimeError(f"{destination}: deterministic extra-sample input differs")
        else:
            destination.write_bytes(data)


def main() -> None:
    """Expose focused generation and the canonical input byte check."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-directory", type=Path, default=DEFAULT_DIRECTORY)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    generate_fixtures(arguments.output_directory, check=arguments.check)
    print(f"{'Checked' if arguments.check else 'Generated'} {len(RECIPES)} complete extra-sample TIFFs")


if __name__ == "__main__":
    main()
