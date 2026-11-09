#!/usr/bin/env python3
"""Generate TIFF inputs for extra samples and palette-alpha storage layouts.

The family keeps every stored plane, including ignored trailing samples. Pixel
references and Pillow's unsupported-layout errors belong to the normal reference
producer; this generator writes only input bytes and storage declarations. The
palette-alpha pair covers contiguous and separate-planar sample storage.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from io import BytesIO
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
PALETTE_ALPHA_PIXELS = bytes([0, 0, 1, 17, 2, 128, 1, 255, 2, 64, 0, 200])


def palette_alpha_rgb() -> bytes:
    """Return a complete deterministic RGB palette for the PA witness."""
    return bytes(
        value
        for index in range(256)
        for value in ((index * 3) & 255, (index * 5 + 1) & 255, (index * 7 + 2) & 255)
    )


def build_palette_alpha_tiff() -> bytes:
    """Build a Pillow-authored TIFF carrying palette indices and alpha bytes."""
    from PIL import Image

    palette = palette_alpha_rgb()
    image = Image.frombytes("PA", (3, 2), PALETTE_ALPHA_PIXELS)
    image.putpalette(palette, rawmode="RGB")
    output = BytesIO()
    image.save(output, format="TIFF")
    data = output.getvalue()

    with Image.open(BytesIO(data)) as reopened:
        tags = reopened.tag_v2
        if reopened.mode != "PA" or reopened.size != (3, 2):
            raise RuntimeError("Pillow did not retain the PA TIFF mode and dimensions")
        if reopened.tobytes() != PALETTE_ALPHA_PIXELS:
            raise RuntimeError("Pillow did not retain the PA index/alpha sample pairs")
        if bytes(reopened.getpalette("RGB") or b"") != palette:
            raise RuntimeError("Pillow did not retain the PA RGB palette")
        if (
            tuple(tags.get(258, ())) != (8, 8)
            or tags.get(262) != 3
            or tags.get(277) != 2
            or tags.get(284) != 1
            or tuple(tags.get(338, ())) != (2,)
            or len(tags.get(320, ())) != 768
        ):
            raise RuntimeError("Pillow emitted unexpected PA TIFF sample or palette tags")
    return data


def build_palette_alpha_separate_planar_tiff() -> bytes:
    """Build separate PA sample planes with valid per-plane strip tables."""
    width, height = 3, 2
    palette = palette_alpha_rgb()
    pixels = PALETTE_ALPHA_PIXELS
    planes = (pixels[0::2], pixels[1::2])
    color_map = [
        palette[index * 3 + channel] << 8
        for channel in range(3)
        for index in range(256)
    ]
    entries = [
        (256, 4, [width]),
        (257, 4, [height]),
        (258, 3, [8, 8]),
        (259, 3, [1]),
        (262, 3, [3]),
        (273, 4, [0, 0]),
        (277, 3, [2]),
        (278, 4, [height]),
        (279, 4, [len(planes[0]), len(planes[1])]),
        (284, 3, [2]),
        (320, 3, color_map),
        (338, 3, [2]),
    ]
    entries.sort()

    cursor = 8 + 2 + 12 * len(entries) + 4
    buffers = {}
    for tag, field_type, values in entries:
        code = "H" if field_type == 3 else "I"
        packed = struct.pack("<" + code * len(values), *values)
        if len(packed) > 4:
            buffers[tag] = (cursor, packed)
            cursor += len(packed)
            cursor += cursor % 2

    strip_offsets = []
    for plane in planes:
        strip_offsets.append(cursor)
        cursor += len(plane)
    offsets_position, _ = buffers[273]
    buffers[273] = (offsets_position, struct.pack("<II", *strip_offsets))

    output = bytearray(b"II*\0\x08\0\0\0" + struct.pack("<H", len(entries)))
    for tag, field_type, values in entries:
        if tag == 273:
            values = strip_offsets
        code = "H" if field_type == 3 else "I"
        packed = struct.pack("<" + code * len(values), *values)
        value = struct.pack("<I", buffers[tag][0]) if len(packed) > 4 else packed.ljust(4, b"\0")
        output.extend(struct.pack("<HHI", tag, field_type, len(values)) + value)
    output.extend(bytes(4))
    for offset, buffer in sorted(buffers.values()):
        output.extend(bytes(offset - len(output)))
        output.extend(buffer)
    for plane in planes:
        output.extend(plane)

    data = bytes(output)
    from PIL import Image

    with Image.open(BytesIO(data)) as image:
        if image.mode != "PA" or image.size != (width, height) or image.tag_v2.get(284) != 2:
            raise RuntimeError("Pillow did not retain the separate-planar PA header")
        image.verify()
    with Image.open(BytesIO(data)) as image:
        try:
            image.load()
        except ValueError as error:
            if str(error) != "unknown raw mode for given image mode":
                raise RuntimeError(f"Pillow PA decode error changed: {error}") from error
        else:
            raise RuntimeError("Pillow unexpectedly decoded separate-planar PA samples")
    return data


def build_palette_unspecified_extra_sample_tiff() -> bytes:
    """Use Pillow's PA TIFF layout with an unspecified second sample."""
    from PIL import Image

    data = bytearray(build_palette_alpha_tiff())
    if data[:8] != b"II*\0\x08\0\0\0":
        raise RuntimeError("Pillow emitted an unexpected classic TIFF header")
    entry_count = struct.unpack_from("<H", data, 8)[0]
    extra_sample_entry = None
    for index in range(entry_count):
        offset = 10 + index * 12
        tag, field_type, count = struct.unpack_from("<HHI", data, offset)
        if tag == 338:
            if field_type != 3 or count != 1:
                raise RuntimeError("Pillow emitted an unexpected ExtraSamples field")
            extra_sample_entry = offset
            break
    if extra_sample_entry is None:
        raise RuntimeError("Pillow omitted the PA ExtraSamples field")
    # The single SHORT value is stored inline in the first two value bytes.
    struct.pack_into("<H", data, extra_sample_entry + 8, 0)
    result = bytes(data)
    with Image.open(BytesIO(result)) as image:
        if image.mode != "P" or image.size != (3, 2):
            raise RuntimeError("Pillow did not expose unspecified-extra PA TIFF as P")
        if tuple(image.tag_v2.get(338, ())) != (0,):
            raise RuntimeError("Pillow did not retain the unspecified ExtraSamples tag")
        image.verify()
    with Image.open(BytesIO(result)) as image:
        image.load()
        if image.tobytes() != PALETTE_ALPHA_PIXELS[0::2]:
            raise RuntimeError("Pillow did not ignore the unspecified extra sample")
        if bytes(image.getpalette("RGB") or b"") != palette_alpha_rgb():
            raise RuntimeError("Pillow did not retain the indexed RGB palette")
    return result


def with_short_palette_color_map(data: bytes) -> bytes:
    """Return a TIFF whose one-entry ColorMap is accepted without a palette."""
    output = bytearray(data)
    if output[:8] != b"II*\0\x08\0\0\0":
        raise RuntimeError("Pillow emitted an unexpected classic TIFF header")
    entry_count = struct.unpack_from("<H", output, 8)[0]
    for index in range(entry_count):
        offset = 10 + index * 12
        tag, field_type, count = struct.unpack_from("<HHI", output, offset)
        if tag == 320:
            if field_type != 3 or count != 768:
                raise RuntimeError("Pillow emitted an unexpected TIFF ColorMap")
            # Keep the complete TIFF file and its pixel data, but declare a
            # ColorMap too short for Pillow to retain as an indexed palette.
            struct.pack_into("<I", output, offset + 4, 1)
            return bytes(output)
    raise RuntimeError("Pillow omitted the TIFF ColorMap")


def build_palette_alpha_short_color_map_tiff() -> bytes:
    """Build Pillow's PA samples with a short ColorMap and no retained table."""
    result = with_short_palette_color_map(build_palette_alpha_tiff())
    _verify_short_color_map_palette_tiff(
        result, expected_mode="PA", expected_pixels=PALETTE_ALPHA_PIXELS, extra_sample=2
    )
    return result


def build_palette_unspecified_extra_sample_short_color_map_tiff() -> bytes:
    """Build P samples with an ignored byte and no retained palette table."""
    result = with_short_palette_color_map(build_palette_unspecified_extra_sample_tiff())
    _verify_short_color_map_palette_tiff(
        result,
        expected_mode="P",
        expected_pixels=PALETTE_ALPHA_PIXELS[0::2],
        extra_sample=0,
    )
    return result


def _verify_short_color_map_palette_tiff(
    data: bytes, *, expected_mode: str, expected_pixels: bytes, extra_sample: int
) -> None:
    """Check Pillow's mode, ignored samples, and absent palette for this input."""
    from PIL import Image

    with Image.open(BytesIO(data)) as image:
        if image.mode != expected_mode or image.size != (3, 2):
            raise RuntimeError("Pillow changed mode or dimensions for a short ColorMap TIFF")
        if tuple(image.tag_v2.get(338, ())) != (extra_sample,):
            raise RuntimeError("Pillow changed the short ColorMap TIFF ExtraSamples tag")
        if bytes(image.getpalette("RGB") or b""):
            raise RuntimeError("Pillow unexpectedly retained a palette from a short ColorMap")
        image.verify()
    with Image.open(BytesIO(data)) as image:
        image.load()
        if image.tobytes() != expected_pixels:
            raise RuntimeError("Pillow changed samples in a short ColorMap TIFF")
        if bytes(image.getpalette("RGB") or b""):
            raise RuntimeError("Pillow unexpectedly retained a palette after pixel decoding")


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
    palette_alpha_path = directory / "palette_alpha.tiff"
    palette_alpha_data = build_palette_alpha_tiff()
    if check:
        if not palette_alpha_path.is_file() or palette_alpha_path.read_bytes() != palette_alpha_data:
            raise RuntimeError(f"{palette_alpha_path}: deterministic PA input differs")
    else:
        palette_alpha_path.write_bytes(palette_alpha_data)

    separate_palette_alpha_path = directory / "palette_alpha_separate_planar.tiff"
    separate_palette_alpha_data = build_palette_alpha_separate_planar_tiff()
    if check:
        if (
            not separate_palette_alpha_path.is_file()
            or separate_palette_alpha_path.read_bytes() != separate_palette_alpha_data
        ):
            raise RuntimeError(f"{separate_palette_alpha_path}: separate-planar PA input differs")
    else:
        separate_palette_alpha_path.write_bytes(separate_palette_alpha_data)

    unspecified_palette_extra_path = directory / "palette_unspecified_extra_sample.tiff"
    unspecified_palette_extra_data = build_palette_unspecified_extra_sample_tiff()
    if check:
        if (
            not unspecified_palette_extra_path.is_file()
            or unspecified_palette_extra_path.read_bytes() != unspecified_palette_extra_data
        ):
            raise RuntimeError(
                f"{unspecified_palette_extra_path}: unspecified palette extra-sample input differs"
            )
    else:
        unspecified_palette_extra_path.write_bytes(unspecified_palette_extra_data)

    short_color_map_fixtures = (
        (
            "palette_alpha_short_color_map.tiff",
            build_palette_alpha_short_color_map_tiff(),
        ),
        (
            "palette_unspecified_extra_sample_short_color_map.tiff",
            build_palette_unspecified_extra_sample_short_color_map_tiff(),
        ),
    )
    for filename, data in short_color_map_fixtures:
        destination = directory / filename
        if check:
            if not destination.is_file() or destination.read_bytes() != data:
                raise RuntimeError(f"{destination}: short ColorMap TIFF input differs")
        else:
            destination.write_bytes(data)


def main() -> None:
    """Expose focused generation and the canonical input byte check."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-directory", type=Path, default=DEFAULT_DIRECTORY)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    generate_fixtures(arguments.output_directory, check=arguments.check)
    input_count = len(RECIPES) + 5
    print(
        f"{'Checked' if arguments.check else 'Generated'} "
        f"{input_count} extra-sample TIFF inputs, including "
        "contiguous and separate-planar PA, ignored palette extras, and "
        "short-ColorMap palette-missing cases"
    )


if __name__ == "__main__":
    main()
