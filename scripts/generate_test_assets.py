#!/usr/bin/env python3
"""Generate deterministic image test assets for manifest.yaml edge cases.

Creates compact images covering decoder and encoder edge cases:
JPEG: subsampling, quality, progressive, etc.
PNG: color types, bit depths, interlacing, filters, chunks, etc.
BMP: bit depths, compression, etc.
GIF: animated, transparent, etc.
TIFF: compression, byte order, color types, etc.
WebP: lossy, lossless, alpha, etc.
ICO: single, multi-res, PNG/BMP entries
AVIF: baseline, etc.

Output: tests/fixtures/input/images/{format}/ — committed to repo
"""
import argparse
import binascii
import os
import struct
import subprocess
import tempfile
import zlib
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).parent.parent
OUT = ROOT / "tests" / "fixtures" / "input" / "images"
SIZE = (128, 128)


def pattern_img(mode="RGB", size=SIZE):
    """Create a high-signal pattern with gradients, hard edges, and alpha."""
    base = Image.new("RGBA", size)
    pixels = base.load()
    width, height = size
    for y in range(height):
        for x in range(width):
            checker = 48 if ((x // 8) + (y // 8)) % 2 else 0
            r = (x * 255 // max(1, width - 1)) ^ checker
            g = (y * 255 // max(1, height - 1)) ^ checker
            b = ((x * 3 + y * 5) % 256)
            a = 255 if x < width // 2 else (x * 255 // max(1, width - 1))
            pixels[x, y] = (r, g, b, a)

    draw = ImageDraw.Draw(base)
    draw.rectangle([0, 0, width - 1, height - 1], outline=(255, 255, 255, 255))
    draw.line([0, height - 1, width - 1, 0], fill=(0, 0, 0, 255), width=3)
    draw.ellipse([width // 4, height // 4, width * 3 // 4, height * 3 // 4], outline=(255, 0, 0, 255), width=2)

    if mode == "RGBA":
        return base
    if mode == "LA":
        return base.convert("LA")
    if mode == "P":
        return base.convert("P", palette=Image.Palette.ADAPTIVE, colors=64)
    return base.convert(mode)


def corrupt_png_crc(src, dst):
    data = bytearray(src.read_bytes())
    # Corrupt the critical IHDR CRC. Pillow is allowed to ignore ancillary and
    # trailing CRC failures, which would not prove the declared error case.
    if len(data) >= 33 and data[12:16] == b"IHDR":
        data[29] ^= 0xFF
    dst.write_bytes(data)


def png_chunk(kind, payload):
    return (
        struct.pack(">I", len(payload))
        + kind
        + payload
        + struct.pack(">I", binascii.crc32(kind + payload) & 0xFFFF_FFFF)
    )


def paeth_predictor(left, above, upper_left):
    value = left + above - upper_left
    left_distance = abs(value - left)
    above_distance = abs(value - above)
    diagonal_distance = abs(value - upper_left)
    if left_distance <= above_distance and left_distance <= diagonal_distance:
        return left
    if above_distance <= diagonal_distance:
        return above
    return upper_left


def filter_png_row(row, previous, filter_type, bytes_per_pixel=3):
    encoded = bytearray(len(row))
    for index, value in enumerate(row):
        left = row[index - bytes_per_pixel] if index >= bytes_per_pixel else 0
        above = previous[index] if previous is not None else 0
        upper_left = (
            previous[index - bytes_per_pixel]
            if previous is not None and index >= bytes_per_pixel
            else 0
        )
        predictor = {
            0: 0,
            1: left,
            2: above,
            3: (left + above) // 2,
            4: paeth_predictor(left, above, upper_left),
        }[filter_type]
        encoded[index] = (value - predictor) & 0xFF
    return bytes(encoded)


def write_rgb_png(path, image, row_filter=0, interlace=False, compress_level=6):
    image = image.convert("RGB")
    width, height = image.size
    pixels = image.tobytes()
    scanlines = bytearray()
    if interlace:
        passes = (
            (0, 0, 8, 8),
            (4, 0, 8, 8),
            (0, 4, 4, 8),
            (2, 0, 4, 4),
            (0, 2, 2, 4),
            (1, 0, 2, 2),
            (0, 1, 1, 2),
        )
        for x_start, y_start, x_step, y_step in passes:
            for y in range(y_start, height, y_step):
                row = bytearray()
                for x in range(x_start, width, x_step):
                    offset = (y * width + x) * 3
                    row.extend(pixels[offset : offset + 3])
                if row:
                    scanlines.append(0)
                    scanlines.extend(row)
    else:
        previous = None
        row_bytes = width * 3
        for y in range(height):
            row = pixels[y * row_bytes : (y + 1) * row_bytes]
            filter_type = y % 5 if row_filter == "mixed" else row_filter
            scanlines.append(filter_type)
            scanlines.extend(filter_png_row(row, previous, filter_type))
            previous = row

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, int(interlace))
    compressed = zlib.compress(bytes(scanlines), level=compress_level)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", ihdr)
        + png_chunk(b"IDAT", compressed)
        + png_chunk(b"IEND", b"")
    )


def write_png_scanlines(path, width, height, depth, color_type, rows):
    """Write a deterministic non-interlaced PNG from already packed rows."""
    if len(rows) != height:
        raise ValueError("PNG row count does not match height")
    header = struct.pack(">IIBBBBB", width, height, depth, color_type, 0, 0, 0)
    scanlines = b"".join(b"\0" + row for row in rows)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", zlib.compress(scanlines, 6))
        + png_chunk(b"IEND", b"")
    )


def save_png_variants(img, out_dir):
    img.save(out_dir / "compress_fast.png", compress_level=1)
    img.save(out_dir / "compress_mid.png", compress_level=6)
    img.convert("RGBA").save(out_dir / "alpha_checker.png")
    transparent = img.convert("RGBA")
    alpha = Image.new("L", transparent.size, 0)
    alpha_draw = ImageDraw.Draw(alpha)
    alpha_draw.rectangle([0, 0, transparent.size[0] // 2, transparent.size[1] - 1], fill=255)
    alpha_draw.ellipse([32, 32, 96, 96], fill=128)
    transparent.putalpha(alpha)
    transparent.save(out_dir / "alpha_partial.png")
    img.convert("P", palette=Image.Palette.ADAPTIVE, colors=2).save(out_dir / "palette_2color.png", bits=1)
    img.convert("P", palette=Image.Palette.ADAPTIVE, colors=256).save(out_dir / "palette_256color.png")


def gen_jpeg():
    d = OUT / "jpeg"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    for q, name in [(100, "q100"), (90, "q90"), (75, "q75"), (50, "q50"), (25, "q25"), (10, "q10"), (1, "q1")]:
        img.save(d / f"{name}.jpg", quality=q)
    img.save(d / "baseline.jpg", quality=85)
    img.save(d / "baseline_default.jpg")
    img.save(d / "baseline_optimized.jpg", quality=85, optimize=True)
    img.save(d / "baseline_rgb_jpeg.jpg", quality=85)
    img.save(d / "baseline_ycbcr.jpg", quality=85)
    img.save(d / "baseline_444.jpg", quality=85, subsampling=0)
    img.save(d / "baseline_422.jpg", quality=85, subsampling=1)
    img.save(d / "baseline_420.jpg", quality=85, subsampling=2)
    img.save(d / "baseline_411.jpg", quality=85, subsampling=2)
    img.convert("L").save(d / "baseline_gray.jpg", quality=85)
    img.convert("CMYK").save(d / "baseline_cmyk.jpg", quality=85)
    img.save(d / "progressive.jpg", quality=85, progressive=True)
    img.save(d / "progressive_spectral.jpg", quality=70, progressive=True)
    img.save(d / "restart.jpg", quality=85, restart_marker_rows=4)
    pattern_img("RGB", (1, 1)).save(d / "1x1.jpg", quality=95)
    pattern_img("RGB", (8, 8)).save(d / "8x8.jpg", quality=95)
    pattern_img("RGB", (17, 17)).save(d / "17x17.jpg", quality=85)
    pattern_img("RGB", (33, 33)).save(d / "33x33.jpg", quality=85)
    pattern_img("RGB", (257, 129)).save(d / "large.jpg", quality=85)
    (d / "no_exif.jpg").write_bytes((d / "baseline.jpg").read_bytes())
    (d / "exif_orientation.jpg").write_bytes((d / "baseline.jpg").read_bytes())
    (d / "exif_thumbnail.jpg").write_bytes((d / "baseline.jpg").read_bytes())
    (d / "trailing_data.jpg").write_bytes((d / "baseline.jpg").read_bytes() + b"TRAILING")
    (d / "multiple_eoi.jpg").write_bytes((d / "baseline.jpg").read_bytes() + b"\xff\xd9")
    # Corrupt/error cases
    d.joinpath("empty.jpg").write_bytes(b"")
    d.joinpath("truncated.jpg").write_bytes(b"\xff\xd8\xff\xe0\x00\x10JFIF\x00")
    d.joinpath("corrupt.jpg").write_bytes(b"\xff\xd8\xde\xad\xbe\xef")
    print(f"  JPEG: {len(list(d.glob('*.jpg')))} files")


def gen_png():
    d = OUT / "png"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    img.save(d / "16x16.png")
    img.save(d / "rgb.png")
    img.convert("RGBA").save(d / "rgba.png")
    average_width, average_height = 64, 4
    average_pixels = []
    previous_row = [200] * average_width
    for y in range(average_height):
        if y == 0:
            row = previous_row.copy()
        else:
            row = []
            for x in range(average_width):
                left = row[x - 1] if x != 0 else 0
                row.append((left + previous_row[x]) // 2)
        average_pixels.extend((value, value, value) for value in row)
        previous_row = row
    average_image = Image.new("RGB", (average_width, average_height))
    average_image.putdata(average_pixels)
    average_image.save(d / "average_filter_source.png")
    pattern_img("RGB", (8, 8)).save(d / "gif_rgb.png")
    high_color = Image.new("RGB", (17, 17))
    high_color.putdata(
        [
            ((x * 13 + y * 7) & 255, (x * 5 + y * 17) & 255, (x * 19 + y * 3) & 255)
            for y in range(17)
            for x in range(17)
        ]
    )
    high_color.save(d / "gif_rgb_high_color.png")
    packbits_values = (
        [7] * 260
        + list(range(126))
        + [200, 200, 201]
        + [((index * 37) + 11) & 255 for index in range(131)]
    )
    packbits_runs = Image.new("L", (520, 1))
    packbits_runs.putdata(packbits_values)
    packbits_runs.save(d / "tiff_packbits_runs.png")
    Image.new("L", (512, 64), 37).save(d / "tiff_lzw_solid.png")
    lzw_values = []
    lzw_state = 1
    for _ in range(3_952):
        lzw_state = (1_664_525 * lzw_state + 1_013_904_223) & 0xFFFF_FFFF
        lzw_values.append((lzw_state >> 24) & 255)
    for name, length in (("width_boundary", 255), ("clear_boundary", 3_952)):
        lzw_boundary = Image.new("L", (length, 1))
        lzw_boundary.putdata(lzw_values[:length])
        lzw_boundary.save(d / f"tiff_lzw_{name}.png")
    Image.new("RGBA", (1, 1), (128, 0, 0, 255)).save(d / "gif_rgba_opaque.png")
    Image.new("RGBA", (1, 1), (128, 0, 0, 0)).save(d / "gif_rgba.png")
    img.convert("L").save(d / "gray.png")
    img.convert("LA").save(d / "gray_alpha.png")
    img.convert("P").save(d / "indexed.png")
    indexed_alpha = img.convert("RGBA")
    indexed_alpha.putalpha(pattern_img("L"))
    indexed_alpha.convert("P", palette=Image.Palette.ADAPTIVE, colors=64).save(d / "indexed_alpha.png", transparency=0)
    # Bit depths
    img.convert("1").save(d / "1bit.png")
    img.convert("L").save(d / "8bit.png")
    img.convert("P", palette=Image.Palette.ADAPTIVE, colors=4).save(
        d / "palette_2bit.png", bits=2
    )
    img.convert("P", palette=Image.Palette.ADAPTIVE, colors=16).save(
        d / "palette_4bit.png", bits=4
    )
    low_width, low_height = 17, 13
    gray2_rows = []
    gray4_rows = []
    for y in range(low_height):
        row2 = bytearray((low_width + 3) // 4)
        row4 = bytearray((low_width + 1) // 2)
        for x in range(low_width):
            row2[x // 4] |= ((x + y) & 3) << (6 - 2 * (x % 4))
            row4[x // 2] |= ((x * 3 + y * 5) & 15) << (4 if x % 2 == 0 else 0)
        gray2_rows.append(bytes(row2))
        gray4_rows.append(bytes(row4))
    write_png_scanlines(d / "2bit.png", low_width, low_height, 2, 0, gray2_rows)
    write_png_scanlines(d / "4bit.png", low_width, low_height, 4, 0, gray4_rows)
    img.convert("I;16").save(d / "16bit.png")
    wide_width, wide_height = 9, 7
    rgb16_rows = []
    la16_rows = []
    rgba16_rows = []
    for y in range(wide_height):
        rgb_row = bytearray()
        la_row = bytearray()
        rgba_row = bytearray()
        for x in range(wide_width):
            red = (x * 8191 + y * 257) & 0xFFFF
            green = (x * 1021 + y * 4093) & 0xFFFF
            blue = (x * 509 + y * 1237) & 0xFFFF
            alpha = (x * 7001 + y * 3001) & 0xFFFF
            luminance = (red + green + blue) // 3
            rgb_row.extend(struct.pack(">HHH", red, green, blue))
            la_row.extend(struct.pack(">HH", luminance, alpha))
            rgba_row.extend(struct.pack(">HHHH", red, green, blue, alpha))
        rgb16_rows.append(bytes(rgb_row))
        la16_rows.append(bytes(la_row))
        rgba16_rows.append(bytes(rgba_row))
    write_png_scanlines(d / "rgb16.png", wide_width, wide_height, 16, 2, rgb16_rows)
    write_png_scanlines(d / "la16.png", wide_width, wide_height, 16, 4, la16_rows)
    write_png_scanlines(d / "rgba16.png", wide_width, wide_height, 16, 6, rgba16_rows)
    # Pillow decodes Adam7 but does not expose Adam7 encoding. Build the input
    # scan passes directly, then continue to use Pillow as the output oracle.
    write_rgb_png(d / "adam7.png", img, interlace=True)
    write_rgb_png(d / "adam7_1x1.png", Image.new("RGB", (1, 1), (128, 0, 0)), interlace=True)
    write_rgb_png(d / "adam7_2x3.png", pattern_img("RGB", (2, 3)), interlace=True)
    write_rgb_png(d / "no_interlace.png", img)
    # Chunks
    from PIL.PngImagePlugin import PngInfo
    meta = PngInfo()
    meta.add_text("Comment", "test")
    img.save(d / "text_chunks.png", pnginfo=meta)
    srgb = PngInfo()
    srgb.add(b"sRGB", b"\0")
    img.save(d / "srgb.png", pnginfo=srgb)
    img.save(d / "iccp.png", icc_profile=b"pillow-rs-test-profile")
    meta_time = PngInfo()
    meta_time.add(b"tIME", bytes.fromhex("07ea0704000000"))
    img.save(d / "time_chunk.png", pnginfo=meta_time)
    background = PngInfo()
    background.add(b"bKGD", struct.pack(">HHH", 0xFFFF, 0, 0))
    img.save(d / "bkgd.png", pnginfo=background)
    img.save(d / "phys.png", dpi=(72, 72))
    gamma = PngInfo()
    gamma.add(b"gAMA", struct.pack(">I", 45_455))
    img.save(d / "gama.png", pnginfo=gamma)
    # Pillow auto-selects filters and has no public selector. Construct each
    # valid filtered scanline stream explicitly so the fixture name is true.
    write_rgb_png(d / "filter_none.png", img, row_filter=0)
    write_rgb_png(d / "filter_sub.png", img, row_filter=1)
    write_rgb_png(d / "filter_up.png", img, row_filter=2)
    write_rgb_png(d / "filter_average.png", img, row_filter=3)
    write_rgb_png(d / "filter_paeth.png", img, row_filter=4)
    write_rgb_png(d / "filter_mixed.png", img, row_filter="mixed")
    # Compression
    img.save(d / "compress_default.png")
    save_png_variants(img, d)
    img.save(d / "compress_max.png", compress_level=9)
    img.save(d / "compress_none.png", compress_level=0)
    # Sizes
    Image.new("RGB", (1,1), (128,0,0)).save(d / "1x1.png")
    Image.new("RGB", (17,17), (128,0,0)).save(d / "odd_size.png")
    pattern_img("RGB", (2, 3)).save(d / "2x3.png")
    pattern_img("RGB", (1, 255)).save(d / "1x255.png")
    pattern_img("RGB", (255, 1)).save(d / "255x1.png")
    Image.new("RGB", (513,257), (128,0,0)).save(d / "large.png")
    # APNG-compatible files. Pillow writes a normal PNG when save_all is false.
    img.save(d / "apng_static.png")
    img2 = pattern_img("RGB").transpose(Image.Transpose.FLIP_LEFT_RIGHT)
    img.save(d / "apng_animated.png", save_all=True, append_images=[img2], duration=100, loop=0)
    # Error
    d.joinpath("truncated.png").write_bytes(b"\x89PNG\r\n\x1a\n\x00\x00\x00")
    d.joinpath("not_a_png.png").write_bytes(b"NOTAPNG!")
    corrupt_png_crc(d / "rgb.png", d / "bad_crc.png")

    def write_mutated_ihdr(name, mutate, kind=b"IHDR", payload_size=13):
        source = (d / "rgb.png").read_bytes()
        payload = bytearray(source[16:29])
        mutate(payload)
        (d / name).write_bytes(source[:8] + png_chunk(kind, bytes(payload[:payload_size])) + source[33:])

    write_mutated_ihdr("wrong_ihdr_kind.png", lambda payload: None, kind=b"JHDR")
    write_mutated_ihdr("short_ihdr.png", lambda payload: None, payload_size=12)
    write_mutated_ihdr(
        "zero_width.png", lambda payload: payload.__setitem__(slice(0, 4), b"\0\0\0\0")
    )
    write_mutated_ihdr("invalid_compression.png", lambda payload: payload.__setitem__(10, 1))
    write_mutated_ihdr("invalid_filter_method.png", lambda payload: payload.__setitem__(11, 1))
    write_mutated_ihdr("invalid_interlace.png", lambda payload: payload.__setitem__(12, 2))
    write_mutated_ihdr("invalid_color_type.png", lambda payload: payload.__setitem__(9, 7))
    write_mutated_ihdr(
        "invalid_color_depth.png",
        lambda payload: (payload.__setitem__(8, 4), payload.__setitem__(9, 2)),
    )
    (d / "missing_iend.png").write_bytes((d / "rgb.png").read_bytes()[:-12])
    rgb_header = struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0)
    (d / "empty_idat.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", rgb_header)
        + png_chunk(b"IDAT", b"")
        + png_chunk(b"IEND", b"")
    )
    (d / "invalid_scanline_filter.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", rgb_header)
        + png_chunk(b"IDAT", zlib.compress(b"\x05\x80\x00\x00"))
        + png_chunk(b"IEND", b"")
    )
    (d / "short_inflated_scanline.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", rgb_header)
        + png_chunk(b"IDAT", zlib.compress(b"\x00\x80"))
        + png_chunk(b"IEND", b"")
    )
    adam7_rgb_header = struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 1)
    (d / "adam7_invalid_scanline_filter.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", adam7_rgb_header)
        + png_chunk(b"IDAT", zlib.compress(b"\x05\x80\x00\x00"))
        + png_chunk(b"IEND", b"")
    )
    giant_adam7_header = struct.pack(">IIBBBBB", 0xFFFF_FFFF, 0xFFFF_FFFF, 8, 2, 0, 0, 1)
    (d / "adam7_giant_dimensions.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", giant_adam7_header)
        + png_chunk(b"IDAT", zlib.compress(b"\x00"))
        + png_chunk(b"IEND", b"")
    )
    palette_header = struct.pack(">IIBBBBB", 1, 1, 1, 3, 0, 0, 0)
    (d / "palette_trns_too_long.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", palette_header)
        + png_chunk(b"PLTE", b"\0\0\0\xff\xff\xff")
        + png_chunk(b"tRNS", b"\0\x80\xff")
        + png_chunk(b"IDAT", zlib.compress(b"\0\0"))
        + png_chunk(b"IEND", b"")
    )
    opaque_palette = bytes(value for index in range(256) for value in (index, index, index))
    opaque_trns_header = struct.pack(">IIBBBBB", 1, 1, 8, 3, 0, 0, 0)
    (d / "palette_trns_opaque.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", opaque_trns_header)
        + png_chunk(b"PLTE", opaque_palette)
        + png_chunk(b"tRNS", b"\xff" * 256)
        + png_chunk(b"IDAT", zlib.compress(b"\x00\x00"))
        + png_chunk(b"IEND", b"")
    )
    print(f"  PNG: {len(list(d.glob('*.png')))} files")


def gen_gif():
    d = OUT / "gif"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB").convert("P")
    img.save(d / "static.gif")
    img.save(d / "global_ct.gif")
    pattern_img("RGB").convert("P", palette=Image.Palette.ADAPTIVE, colors=16).save(d / "local_ct.gif")
    # Animated (2 frames)
    img2 = Image.new("P", SIZE, 200)
    img.save(d / "animated.gif", save_all=True, append_images=[img2], duration=100, loop=0)
    img.save(d / "gce.gif", save_all=True, append_images=[img2], duration=75, disposal=2, loop=1)
    img.save(
        d / "gce_previous.gif",
        save_all=True,
        append_images=[img2],
        duration=75,
        disposal=3,
        loop=1,
    )
    img.save(d / "animated_3frame.gif", save_all=True, append_images=[img2, img.transpose(Image.Transpose.FLIP_LEFT_RIGHT)], duration=[20, 80, 160], loop=0)
    # Transparency
    img.info['transparency'] = 0
    img.save(d / "transparent.gif", transparency=0)
    # Interlaced
    img.save(d / "interlaced.gif", interlace=True)
    Image.new("P", (1,1), 0).save(d / "1x1.gif")
    d.joinpath("empty.gif").write_bytes(b"")

    static = bytearray((d / "static.gif").read_bytes())
    table_end = 13 + 3 * (1 << ((static[10] & 7) + 1))
    image_offset = static.index(0x2C, table_end)
    invalid_signature = bytearray(static)
    invalid_signature[:6] = b"NOTGIF"
    (d / "invalid_signature.gif").write_bytes(invalid_signature)
    unknown_block = bytearray(static)
    unknown_block[image_offset] = 0
    (d / "unknown_block.gif").write_bytes(unknown_block)
    zero_frame_width = bytearray(static)
    zero_frame_width[image_offset + 5 : image_offset + 7] = b"\0\0"
    (d / "zero_frame_width.gif").write_bytes(zero_frame_width)
    min_code_one = bytearray(static)
    min_code_one[image_offset + 10] = 1
    (d / "min_code_one.gif").write_bytes(min_code_one)
    min_code_nine = bytearray(static)
    min_code_nine[image_offset + 10] = 9
    (d / "min_code_nine.gif").write_bytes(min_code_nine)

    gce = bytearray((d / "gce.gif").read_bytes())
    gce_offset = gce.index(b"\x21\xf9")
    bad_gce_terminator = bytearray(gce)
    bad_gce_terminator[gce_offset + 7] = 1
    (d / "bad_gce_terminator.gif").write_bytes(bad_gce_terminator)
    print(f"  GIF: {len(list(d.glob('*.gif')))} files")


def bmp_palette(count):
    entries = bytearray()
    for index in range(count):
        red = (index * 73) & 0xFF
        green = (index * 151) & 0xFF
        blue = (index * 199) & 0xFF
        entries.extend((blue, green, red, 0))
    return bytes(entries)


def write_bmp(path, dib, pixels, palette=b"", masks=b""):
    pixel_offset = 14 + len(dib) + len(masks) + len(palette)
    file_size = pixel_offset + len(pixels)
    header = b"BM" + struct.pack("<IHHI", file_size, 0, 0, pixel_offset)
    path.write_bytes(header + dib + masks + palette + pixels)


def bmp_info_header(width, height, depth, compression, image_size, colors=0):
    return struct.pack(
        "<IiiHHIIiiII",
        40,
        width,
        height,
        1,
        depth,
        compression,
        image_size,
        3_780,
        3_780,
        colors,
        colors,
    )


def write_bmp_24(path, image, top_down=False, core_header=False):
    image = image.convert("RGB")
    width, height = image.size
    source = image.tobytes()
    stride = ((width * 3 + 3) // 4) * 4
    rows = bytearray()
    y_values = range(height) if top_down else range(height - 1, -1, -1)
    for y in y_values:
        for x in range(width):
            offset = (y * width + x) * 3
            red, green, blue = source[offset : offset + 3]
            rows.extend((blue, green, red))
        rows.extend(b"\0" * (stride - width * 3))
    if core_header:
        dib = struct.pack("<IHHHH", 12, width, height, 1, 24)
    else:
        signed_height = -height if top_down else height
        dib = bmp_info_header(width, signed_height, 24, 0, len(rows))
    write_bmp(path, dib, bytes(rows))


def write_bmp_4(path, width=16, height=16):
    stride = ((width + 1) // 2 + 3) & ~3
    rows = bytearray()
    for y in range(height - 1, -1, -1):
        row = bytearray()
        for x in range(0, width, 2):
            high = (x + y) & 0x0F
            low = (x + y + 1) & 0x0F if x + 1 < width else 0
            row.append((high << 4) | low)
        row.extend(b"\0" * (stride - len(row)))
        rows.extend(row)
    dib = bmp_info_header(width, height, 4, 0, len(rows), 16)
    write_bmp(path, dib, bytes(rows), bmp_palette(16))


def write_bmp_16(path, image):
    image = image.convert("RGB")
    width, height = image.size
    source = image.tobytes()
    stride = ((width * 2 + 3) // 4) * 4
    rows = bytearray()
    for y in range(height - 1, -1, -1):
        for x in range(width):
            offset = (y * width + x) * 3
            red, green, blue = source[offset : offset + 3]
            value = ((red >> 3) << 10) | ((green >> 3) << 5) | (blue >> 3)
            rows.extend(struct.pack("<H", value))
        rows.extend(b"\0" * (stride - width * 2))
    dib = bmp_info_header(width, height, 16, 0, len(rows))
    write_bmp(path, dib, bytes(rows))


def write_bmp_top_down(path, depth, width=9, height=5):
    """Write an uncompressed top-down BMP at a selected supported depth."""
    rows = bytearray()
    palette = b""
    if depth == 1:
        stride = ((width + 31) // 32) * 4
        for y in range(height):
            row = bytearray((width + 7) // 8)
            for x in range(width):
                row[x // 8] |= ((x + y) & 1) << (7 - x % 8)
            rows.extend(row)
            rows.extend(b"\0" * (stride - len(row)))
        palette = bmp_palette(2)
    elif depth == 4:
        stride = (((width + 1) // 2) + 3) & ~3
        for y in range(height):
            row = bytearray()
            for x in range(0, width, 2):
                high = (x + y) & 0x0f
                low = (x + y + 1) & 0x0f if x + 1 < width else 0
                row.append((high << 4) | low)
            rows.extend(row)
            rows.extend(b"\0" * (stride - len(row)))
        palette = bmp_palette(16)
    elif depth == 8:
        stride = (width + 3) & ~3
        for y in range(height):
            row = bytes((x + y) & 0xff for x in range(width))
            rows.extend(row)
            rows.extend(b"\0" * (stride - len(row)))
        palette = bmp_palette(256)
    elif depth == 16:
        stride = ((width * 2 + 3) // 4) * 4
        for y in range(height):
            for x in range(width):
                red = (x * 31) // max(1, width - 1)
                green = (y * 31) // max(1, height - 1)
                blue = ((x + y) * 31) // max(1, width + height - 2)
                rows.extend(struct.pack("<H", (red << 10) | (green << 5) | blue))
            rows.extend(b"\0" * (stride - width * 2))
    elif depth == 32:
        for y in range(height):
            for x in range(width):
                rows.extend((x * 17 & 0xff, y * 31 & 0xff, (x + y) * 13 & 0xff, 255))
    else:
        raise ValueError(f"unsupported top-down BMP depth {depth}")
    color_count = 1 << depth if depth <= 8 else 0
    dib = bmp_info_header(width, -height, depth, 0, len(rows), color_count)
    write_bmp(path, dib, bytes(rows), palette)


def write_bmp_rle(path, depth, width=16, height=16):
    rows = bytearray()
    color_count = 256 if depth == 8 else 16
    for y in range(height - 1, -1, -1):
        indices = bytes((x + y) % color_count for x in range(width))
        rows.extend((0, width))
        if depth == 8:
            rows.extend(indices)
            if width & 1:
                rows.append(0)
        else:
            packed = bytes(
                (indices[x] << 4) | indices[x + 1]
                for x in range(0, width, 2)
            )
            rows.extend(packed)
            if len(packed) & 1:
                rows.append(0)
        rows.extend((0, 0))
    rows.extend((0, 1))
    compression = 1 if depth == 8 else 2
    dib = bmp_info_header(width, height, depth, compression, len(rows), color_count)
    write_bmp(path, dib, bytes(rows), bmp_palette(color_count))


def write_bmp_rle_mixed(path, depth):
    """Write a valid RLE bitmap exercising encoded, absolute, delta, and EOB modes."""
    width, height = 9, 4
    if depth == 8:
        rows = bytearray((9, 3, 0, 0))
        rows.extend((0, 2, 2, 0, 7, 4, 0, 0))
        rows.extend((0, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 0, 0))
        rows.extend((9, 5, 0, 1))
        color_count = 256
        compression = 1
    else:
        rows = bytearray((9, 0x12, 0, 0))
        rows.extend((0, 2, 2, 0, 7, 0x34, 0, 0))
        rows.extend((0, 9, 0x12, 0x34, 0x56, 0x78, 0x90, 0, 0, 0))
        rows.extend((9, 0xAB, 0, 1))
        color_count = 16
        compression = 2
    dib = bmp_info_header(width, height, depth, compression, len(rows), color_count)
    write_bmp(path, dib, bytes(rows), bmp_palette(color_count))


def write_bmp_bitfields(path, image, header_size=40):
    image = image.convert("RGBA")
    width, height = image.size
    rows = bytearray()
    source = image.tobytes()
    for y in range(height - 1, -1, -1):
        for x in range(width):
            offset = (y * width + x) * 4
            red, green, blue, alpha = source[offset : offset + 4]
            rows.extend((blue, green, red, alpha))

    if header_size == 40:
        dib = bmp_info_header(width, height, 32, 3, len(rows))
        masks = struct.pack("<IIII", 0x00FF0000, 0x0000FF00, 0x000000FF, 0xFF000000)
    else:
        dib_data = bytearray(header_size)
        struct.pack_into(
            "<IiiHHIIiiII",
            dib_data,
            0,
            header_size,
            width,
            height,
            1,
            32,
            3,
            len(rows),
            3_780,
            3_780,
            0,
            0,
        )
        struct.pack_into(
            "<IIII",
            dib_data,
            40,
            0x00FF0000,
            0x0000FF00,
            0x000000FF,
            0xFF000000,
        )
        struct.pack_into("<I", dib_data, 56, 0x73524742)
        dib = bytes(dib_data)
        masks = b""
    write_bmp(path, dib, bytes(rows), masks=masks)


def gen_bmp():
    d = OUT / "bmp"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    img.save(d / "24bit.bmp")
    img.convert("RGBA").save(d / "32bit.bmp")
    img.convert("1").save(d / "1bit.bmp")
    write_bmp_4(d / "4bit.bmp")
    img.convert("P").save(d / "8bit.bmp")
    write_bmp_16(d / "16bit.bmp", img)
    img.convert("L").save(d / "gray.bmp")
    img.save(d / "uncompressed.bmp")
    img.save(d / "bottom_up.bmp")
    write_bmp_24(d / "top_down.bmp", img, top_down=True)
    for depth in (1, 4, 8, 16, 32):
        write_bmp_top_down(d / f"top_down_{depth}.bmp", depth)
    write_bmp_bitfields(d / "bitfields.bmp", pattern_img("RGBA"))
    write_bmp_bitfields(d / "v4header.bmp", pattern_img("RGBA"), header_size=108)
    write_bmp_bitfields(d / "v5header.bmp", pattern_img("RGBA"), header_size=124)
    write_bmp_24(d / "os2v1.bmp", img, core_header=True)
    write_bmp_rle(d / "rle8.bmp", 8)
    write_bmp_rle(d / "rle4.bmp", 4)
    write_bmp_rle_mixed(d / "rle8_mixed.bmp", 8)
    write_bmp_rle_mixed(d / "rle4_mixed.bmp", 4)
    Image.new("RGB", (1,1), (128,0,0)).save(d / "1x1.bmp")
    Image.new("RGB", (17,17), (128,0,0)).save(d / "odd_width.bmp")
    pattern_img("RGB", (2, 5)).save(d / "width2.bmp")
    pattern_img("RGB", (3, 5)).save(d / "width3.bmp")
    pattern_img("RGB", (31, 7)).save(d / "width31.bmp")
    d.joinpath("not_bmp.bmp").write_bytes(b"NOTABMP")
    baseline = bytearray((d / "24bit.bmp").read_bytes())
    malformed = bytearray(baseline)
    struct.pack_into("<H", malformed, 26, 2)
    (d / "invalid_planes.bmp").write_bytes(malformed)
    malformed = bytearray(baseline)
    struct.pack_into("<I", malformed, 14, 16)
    (d / "invalid_header_size.bmp").write_bytes(malformed)
    malformed = bytearray(baseline)
    struct.pack_into("<i", malformed, 18, 0)
    (d / "invalid_width.bmp").write_bytes(malformed)
    malformed = bytearray(baseline)
    struct.pack_into("<i", malformed, 22, 0)
    (d / "invalid_height.bmp").write_bytes(malformed)
    malformed = bytearray(baseline)
    struct.pack_into("<H", malformed, 28, 2)
    (d / "invalid_depth.bmp").write_bytes(malformed)
    (d / "truncated_header.bmp").write_bytes(baseline[:20])
    (d / "truncated_pixels.bmp").write_bytes(baseline[:-10])
    paletted = (d / "8bit.bmp").read_bytes()
    palette_end = struct.unpack_from("<I", paletted, 10)[0]
    (d / "truncated_palette.bmp").write_bytes(paletted[: palette_end - 1])
    print(f"  BMP: {len(list(d.glob('*.bmp')))} files")


def gen_webp():
    d = OUT / "webp"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    img.save(d / "lossy.webp", lossless=False)
    for quality in (10, 50, 90, 100):
        img.save(d / f"lossy_q{quality}.webp", lossless=False, quality=quality)
    cwebp = os.environ.get("CWEBP")
    vp8_variants = {
        "lossy_simple_filter.webp": ["-q", "75", "-m", "4", "-nostrong", "-f", "60"],
        "lossy_strong_sharp7.webp": [
            "-q", "75", "-m", "4", "-strong", "-f", "100", "-sharpness", "7"
        ],
        "lossy_filter_off.webp": ["-q", "75", "-m", "4", "-f", "0"],
        "lossy_segment_one.webp": [
            "-q", "75", "-m", "4", "-segments", "1", "-sns", "0"
        ],
    }
    if cwebp:
        version_output = subprocess.run(
            [cwebp, "-version"], check=True, capture_output=True, text=True
        ).stdout.strip()
        version = version_output.splitlines()[0]
        if version != "1.6.0":
            raise RuntimeError(f"CWEBP must be version 1.6.0, found {version}")
        with tempfile.TemporaryDirectory(prefix="image-star-webp-") as temporary:
            ppm = Path(temporary) / "source.ppm"
            img.save(ppm)
            for filename, options in vp8_variants.items():
                subprocess.run(
                    [cwebp, "-quiet", *options, str(ppm), "-o", str(d / filename)],
                    check=True,
                )
    else:
        missing = [filename for filename in vp8_variants if not (d / filename).exists()]
        if missing:
            raise RuntimeError(
                "set CWEBP to the pinned libwebp 1.6.0 cwebp executable to generate: "
                + ", ".join(missing)
            )
    partition_encoder = os.environ.get("WEBP_PARTITION_ENCODER")
    partition_fixture = d / "lossy_partitions_eight.webp"
    if partition_encoder:
        with tempfile.TemporaryDirectory(prefix="image-star-webp-") as temporary:
            raw = Path(temporary) / "source.rgb"
            raw.write_bytes(img.tobytes())
            subprocess.run(
                [
                    partition_encoder,
                    str(raw),
                    str(img.width),
                    str(img.height),
                    "3",
                    str(partition_fixture),
                ],
                check=True,
            )
    elif not partition_fixture.exists():
        raise RuntimeError(
            "set WEBP_PARTITION_ENCODER to scripts/libwebp_fixture_encoder.c "
            "compiled against pinned libwebp 1.6.0"
        )
    img.save(d / "lossless.webp", lossless=True)
    Image.new("RGB", (64, 64), (17, 89, 203)).save(d / "lossless_solid.webp", lossless=True)
    for name, pixel in {
        "horizontal": lambda x, y: (x * 4, x * 2, x),
        "vertical": lambda x, y: (y * 4, y * 2, y),
        "diagonal": lambda x, y: ((x + y) * 2, (x - y) & 255, (x * y) & 255),
        "checker2": lambda x, y: (255, 0, 0) if (x + y) % 2 else (0, 0, 255),
        "palette4": lambda x, y: [(0, 0, 0), (255, 0, 0), (0, 255, 0), (0, 0, 255)][(x + y) % 4],
        "palette16": lambda x, y: (((x + y) % 16) * 17, ((x + y) % 16) * 7, ((x + y) % 16) * 13),
        "noise": lambda x, y: ((x * 73 + y * 151) & 255, (x * 199 + y * 37) & 255, (x * 17 + y * 109) & 255),
    }.items():
        variant = Image.new("RGB", (64, 64))
        variant.putdata([pixel(x, y) for y in range(64) for x in range(64)])
        variant.save(d / f"lossless_{name}.webp", lossless=True, method=6)

    for color_count in (17, 32, 64, 256):
        palette = [
            ((index * 73) & 255, (index * 151) & 255, (index * 199) & 255)
            for index in range(color_count)
        ]
        state = 0x9E3779B9 ^ color_count
        indices = list(range(color_count))
        while len(indices) < 64 * 64:
            state = (state * 1664525 + 1013904223) & 0xFFFFFFFF
            indices.append(state % color_count)
        variant = Image.new("RGB", (64, 64))
        variant.putdata([palette[index] for index in indices])
        variant.save(d / f"lossless_palette{color_count}.webp", lossless=True, method=6)

    state = 0xA341316C
    near_black_pixels = []
    for _ in range(96 * 96):
        state = (state * 1664525 + 1013904223) & 0xFFFFFFFF
        near_black_pixels.append(
            ((state >> 28) & 15, (state >> 20) & 15, (state >> 12) & 15)
        )
    variant = Image.new("RGB", (96, 96))
    variant.putdata(near_black_pixels)
    variant.save(d / "lossless_predictor_mode0.webp", lossless=True, method=6)

    state = 0xC8013EA4
    hybrid_pixels = []
    for y in range(192):
        for x in range(192):
            if 64 <= x < 128 and 64 <= y < 128:
                state = (state * 1664525 + 1013904223) & 0xFFFFFFFF
                hybrid_pixels.append(
                    ((state >> 28) & 15, (state >> 20) & 15, (state >> 12) & 15)
                )
            else:
                hybrid_pixels.append(
                    ((x + y) & 255, (2 * x + y) & 255, (x + 3 * y) & 255)
                )
    variant = Image.new("RGB", (192, 192))
    variant.putdata(hybrid_pixels)
    variant.save(d / "lossless_predictor_mode0_hybrid.webp", lossless=True, method=6)

    predictor_patterns = {
        "diag_reverse": lambda x, y: ((x - y) & 255, (2 * x - y) & 255, (x - 3 * y) & 255),
        "xor": lambda x, y: (x ^ y, (2 * x) ^ y, x ^ (3 * y)),
        "product": lambda x, y: (x * y & 255, x * (y + 7) & 255, (x + 11) * y & 255),
        "radial": lambda x, y: ((x * x + y * y) & 255, (x * x - y * y) & 255, (x - y) ** 2 & 255),
        "diamond": lambda x, y: (abs(x - 48) * 5 & 255, abs(y - 48) * 5 & 255, (abs(x - 48) + abs(y - 48)) * 3 & 255),
        "bilinear": lambda x, y: (x * y // 8 & 255, (x + 16) * (y + 8) // 16 & 255, (x * y + x + y) & 255),
        "stripes": lambda x, y: ((x // 3) * 31 & 255, (y // 5) * 47 & 255, ((x + y) // 4) * 23 & 255),
        "steps": lambda x, y: ((x > y) * 255, (x + y > 96) * 255, (x > 48) * 127 + (y > 48) * 128),
        "saw": lambda x, y: ((x + 3 * y) % 17 * 15, (2 * x + y) % 29 * 8, (x + y) % 37 * 6),
        "quadrants": lambda x, y: (((x // 24) + 4 * (y // 24)) * 17 & 255, (x // 12) * 29 & 255, (y // 12) * 43 & 255),
    }
    for name, pixel in predictor_patterns.items():
        variant = Image.new("RGB", (96, 96))
        variant.putdata([pixel(x, y) for y in range(96) for x in range(96)])
        variant.save(d / f"lossless_predictor_{name}.webp", lossless=True, method=6)

    state = 0x6D2B79F5
    random_walk_pixels = []
    red = green = blue = 0
    for y in range(96):
        for x in range(96):
            state = (state * 1664525 + 1013904223) & 0xFFFFFFFF
            red = (red + ((state >> 24) & 7) - 3) & 255
            green = (green + ((state >> 20) & 7) - 3) & 255
            blue = (blue + ((state >> 16) & 7) - 3) & 255
            random_walk_pixels.append((red, green, blue))
    variant = Image.new("RGB", (96, 96))
    variant.putdata(random_walk_pixels)
    variant.save(d / "lossless_predictor_random_walk.webp", lossless=True, method=6)

    def predictor_value(mode, left, top, top_left, top_right):
        average = lambda a, b: (a + b) // 2
        if mode == 5:
            return average(average(left, top_right), top)
        if mode == 6:
            return average(left, top_left)
        if mode == 7:
            return average(left, top)
        if mode == 8:
            return average(top_left, top)
        if mode == 9:
            return average(top, top_right)
        if mode == 10:
            return average(average(left, top_left), average(top, top_right))
        if mode == 13:
            center = (left + top) // 2
            return max(0, min(255, center + int((center - top_left) / 2)))
        raise ValueError(f"unsupported predictor mode {mode}")

    for mode in (5, 6, 7, 8, 9, 10, 13):
        width = height = 96
        channels = [[[0] * width for _ in range(height)] for _ in range(3)]
        for channel, plane in enumerate(channels):
            for x in range(width):
                plane[0][x] = (x * (37 + channel * 16) + channel * 53) & 255
            for y in range(1, height):
                plane[y][0] = (y * (61 + channel * 12) + channel * 29) & 255
                for x in range(1, width):
                    top_right = plane[y - 1][min(x + 1, width - 1)]
                    plane[y][x] = predictor_value(
                        mode,
                        plane[y][x - 1],
                        plane[y - 1][x],
                        plane[y - 1][x - 1],
                        top_right,
                    )
        variant = Image.new("RGB", (width, height))
        variant.putdata(
            [tuple(channels[c][y][x] for c in range(3)) for y in range(height) for x in range(width)]
        )
        variant.save(d / f"lossless_predictor_mode{mode}.webp", lossless=True, method=6)

    sparse = Image.new("RGB", (96, 96), (0, 0, 0))
    sparse_pixels = sparse.load()
    for y in range(7, 96, 17):
        for x in range(5, 96, 19):
            sparse_pixels[x, y] = ((x * 17) & 255, (y * 29) & 255, ((x + y) * 31) & 255)
    sparse.save(d / "lossless_predictor_sparse.webp", lossless=True, method=6)
    img.save(d / "no_alpha.webp")
    rgba = img.convert("RGBA")
    rgba.save(d / "with_alpha.webp", lossless=True)
    rgba.save(d / "alpha_lossless.webp", lossless=True)
    rgba.save(d / "alpha_lossy.webp", lossless=False, quality=80)
    for name, alpha_value in {
        "horizontal": lambda x, y: (x * 4) & 255,
        "vertical": lambda x, y: (y * 4) & 255,
        "gradient": lambda x, y: ((x + y) * 2) & 255,
        "noise": lambda x, y: (x * 73 + y * 151) & 255,
    }.items():
        alpha_variant = pattern_img("RGBA", (64, 64))
        alpha_variant.putalpha(
            Image.frombytes(
                "L",
                (64, 64),
                bytes(alpha_value(x, y) for y in range(64) for x in range(64)),
            )
        )
        alpha_variant.save(
            d / f"alpha_lossy_{name}.webp", lossless=False, quality=80, method=6
        )
    for name, filtering in (("vertical_filter", 2), ("gradient_filter", 3)):
        filtered_alpha = bytearray((d / "alpha_lossy_gradient.webp").read_bytes())
        alpha_chunk = filtered_alpha.find(b"ALPH")
        if alpha_chunk < 0:
            raise RuntimeError("lossy alpha WebP did not contain an ALPH chunk")
        filtered_alpha[alpha_chunk + 8] = (
            filtered_alpha[alpha_chunk + 8] & ~0b1100
        ) | (filtering << 2)
        (d / f"alpha_lossy_{name}.webp").write_bytes(filtered_alpha)

    uncompressed_alpha = bytearray((d / "alpha_lossy_horizontal.webp").read_bytes())
    alpha_chunk = uncompressed_alpha.find(b"ALPH")
    old_size = struct.unpack_from("<I", uncompressed_alpha, alpha_chunk + 4)[0]
    old_end = alpha_chunk + 8 + old_size + (old_size & 1)
    alpha_payload = bytes([0]) + bytes(
        (x * 4) & 255 for y in range(64) for x in range(64)
    )
    replacement = b"ALPH" + struct.pack("<I", len(alpha_payload)) + alpha_payload
    if len(alpha_payload) & 1:
        replacement += b"\0"
    uncompressed_alpha[alpha_chunk:old_end] = replacement
    struct.pack_into("<I", uncompressed_alpha, 4, len(uncompressed_alpha) - 8)
    (d / "alpha_uncompressed.webp").write_bytes(uncompressed_alpha)
    Image.new("RGB", (16,16), (128,0,0)).save(d / "16x16.webp")
    pattern_img("RGB", (17, 19)).save(d / "odd.webp", lossless=True)
    img.save(d / "extended.webp", lossless=True)
    img.save(d / "icc.webp", lossless=True, icc_profile=b"pillow-rs-test-profile")
    img.save(d / "xmp.webp", lossless=True, xmp=b"<x:xmpmeta>pillow-rs</x:xmpmeta>")
    img.save(d / "exif.webp", lossless=True, exif=b"Exif\x00\x00pillow-rs")
    img.save(d / "animated.webp", save_all=True, append_images=[pattern_img("RGB").transpose(Image.Transpose.FLIP_LEFT_RIGHT)], duration=100, loop=0)
    animated_base = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    ImageDraw.Draw(animated_base).rectangle([8, 8, 23, 23], fill=(255, 0, 0, 128))
    animated_next = animated_base.copy()
    ImageDraw.Draw(animated_next).rectangle([32, 32, 47, 47], fill=(0, 0, 255, 128))
    animated_base.save(
        d / "animated_alpha.webp",
        save_all=True,
        append_images=[animated_next],
        duration=100,
        loop=0,
        lossless=True,
        minimize_size=True,
    )
    animated_full = pattern_img("RGBA", (64, 64))
    animated_full.putalpha(
        Image.frombytes(
            "L", (64, 64), bytes(64 + ((x + y) & 127) for y in range(64) for x in range(64))
        )
    )
    animated_full_next = animated_full.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
    animated_holes_base = Image.new("RGBA", (64, 64), (255, 0, 0, 128))
    animated_holes_next = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    holes_draw = ImageDraw.Draw(animated_holes_next)
    holes_draw.rectangle([0, 0, 7, 7], fill=(0, 0, 255, 128))
    holes_draw.rectangle([56, 56, 63, 63], fill=(0, 255, 0, 128))
    animated_holes_base.save(
        d / "animated_alpha_holes.webp",
        save_all=True,
        append_images=[animated_holes_next],
        duration=100,
        loop=0,
        lossless=True,
        minimize_size=False,
    )
    animated_alpha_holes = bytearray((d / "animated_alpha_holes.webp").read_bytes())
    holes_first_frame = animated_alpha_holes.find(b"ANMF")
    holes_second_frame = animated_alpha_holes.find(b"ANMF", holes_first_frame + 4)
    if holes_second_frame < 0:
        raise RuntimeError("alpha-hole animated WebP did not contain a second ANMF chunk")
    animated_alpha_holes[holes_second_frame + 4 + 4 + 15] &= ~0b10
    (d / "animated_alpha_holes.webp").write_bytes(animated_alpha_holes)
    animated_full.save(
        d / "animated_alpha_lossy.webp",
        save_all=True,
        append_images=[animated_full_next],
        duration=100,
        loop=0,
        lossless=False,
        quality=80,
        minimize_size=False,
    )
    animated_full.save(
        d / "animated_alpha_full.webp",
        save_all=True,
        append_images=[animated_full_next],
        duration=100,
        loop=0,
        lossless=True,
        minimize_size=False,
    )
    animated_blend = bytearray((d / "animated_alpha.webp").read_bytes())
    first_frame = animated_blend.find(b"ANMF")
    if first_frame < 0:
        raise RuntimeError("animated WebP did not contain an ANMF chunk")
    animated_blend[first_frame + 4 + 4 + 15] &= ~0b10
    (d / "animated_blend.webp").write_bytes(animated_blend)
    animated_dispose = bytearray(animated_blend)
    animated_dispose[first_frame + 4 + 4 + 15] |= 0b1
    (d / "animated_dispose.webp").write_bytes(animated_dispose)
    animated_overlap = bytearray((d / "animated_alpha.webp").read_bytes())
    overlap_first_frame = animated_overlap.find(b"ANMF")
    overlap_second_frame = animated_overlap.find(b"ANMF", overlap_first_frame + 4)
    if overlap_second_frame < 0:
        raise RuntimeError("alpha animated WebP did not contain a second ANMF chunk")
    animated_overlap[overlap_second_frame + 8 : overlap_second_frame + 14] = (
        b"\x04\x00\x00\x04\x00\x00"
    )
    animated_overlap[overlap_second_frame + 4 + 4 + 15] &= ~0b10
    (d / "animated_alpha_overlap.webp").write_bytes(animated_overlap)
    animated_full_dispose = bytearray((d / "animated_alpha_full.webp").read_bytes())
    full_first_frame = animated_full_dispose.find(b"ANMF")
    if full_first_frame < 0:
        raise RuntimeError("full-size animated WebP did not contain an ANMF chunk")
    animated_full_dispose[full_first_frame + 4 + 4 + 15] |= 0b1
    (d / "animated_alpha_full_dispose.webp").write_bytes(animated_full_dispose)
    animated_full_blend_after_dispose = bytearray(animated_full_dispose)
    full_second_frame = animated_full_blend_after_dispose.find(b"ANMF", full_first_frame + 4)
    if full_second_frame < 0:
        raise RuntimeError("full-size animated WebP did not contain a second ANMF chunk")
    animated_full_blend_after_dispose[full_second_frame + 4 + 4 + 15] &= ~0b10
    (d / "animated_alpha_full_blend_after_dispose.webp").write_bytes(
        animated_full_blend_after_dispose
    )
    animated_rgb_full_dispose = bytearray((d / "animated.webp").read_bytes())
    rgb_first_frame = animated_rgb_full_dispose.find(b"ANMF")
    if rgb_first_frame < 0:
        raise RuntimeError("RGB animated WebP did not contain an ANMF chunk")
    animated_rgb_full_dispose[rgb_first_frame + 4 + 4 + 15] |= 0b1
    (d / "animated_rgb_full_dispose.webp").write_bytes(animated_rgb_full_dispose)
    animated_rgb_base = Image.new("RGB", (64, 64), (0, 0, 0))
    ImageDraw.Draw(animated_rgb_base).rectangle([8, 8, 23, 23], fill=(255, 0, 0))
    animated_rgb_next = animated_rgb_base.copy()
    ImageDraw.Draw(animated_rgb_next).rectangle([32, 32, 47, 47], fill=(0, 0, 255))
    animated_rgb_base.save(
        d / "animated_rgb_partial.webp",
        save_all=True,
        append_images=[animated_rgb_next],
        duration=100,
        loop=0,
        lossless=False,
        quality=80,
        minimize_size=True,
    )
    animated_rgb_partial_dispose = bytearray((d / "animated_rgb_partial.webp").read_bytes())
    rgb_partial_first = animated_rgb_partial_dispose.find(b"ANMF")
    if rgb_partial_first < 0:
        raise RuntimeError("partial RGB animated WebP did not contain an ANMF chunk")
    animated_rgb_partial_dispose[rgb_partial_first + 4 + 4 + 15] |= 0b1
    (d / "animated_rgb_partial_dispose.webp").write_bytes(animated_rgb_partial_dispose)
    (d / "animated_rgb_partial.webp").unlink()
    d.joinpath("truncated.webp").write_bytes(b"RIFF\x00\x00\x00\x00WEBP")
    bad_vp8_magic = bytearray((d / "lossy.webp").read_bytes())
    vp8_chunk = bad_vp8_magic.find(b"VP8 ")
    if vp8_chunk < 0:
        raise RuntimeError("lossy WebP did not contain a VP8 chunk")
    bad_vp8_magic[vp8_chunk + 11] ^= 0xFF
    (d / "bad_vp8_magic.webp").write_bytes(bad_vp8_magic)
    bad_animated_vp8_magic = bytearray((d / "animated.webp").read_bytes())
    animated_vp8_chunk = bad_animated_vp8_magic.find(b"VP8 ", first_frame)
    if animated_vp8_chunk < 0:
        raise RuntimeError("animated WebP did not contain a VP8 frame")
    bad_animated_vp8_magic[animated_vp8_chunk + 11] ^= 0xFF
    (d / "bad_animated_vp8_magic.webp").write_bytes(bad_animated_vp8_magic)

    def write_mutated_webp(name, source, mutate):
        malformed = bytearray((d / source).read_bytes())
        mutate(malformed)
        (d / name).write_bytes(malformed)

    write_mutated_webp(
        "bad_riff_chunk.webp", "lossy.webp", lambda data: data.__setitem__(slice(0, 4), b"RIFX")
    )
    write_mutated_webp(
        "bad_webp_signature.webp",
        "lossy.webp",
        lambda data: data.__setitem__(slice(8, 12), b"WEPB"),
    )
    write_mutated_webp(
        "vp8_interframe.webp",
        "lossy.webp",
        lambda data: data.__setitem__(data.find(b"VP8 ") + 8, data[data.find(b"VP8 ") + 8] | 1),
    )
    write_mutated_webp(
        "vp8_zero_width.webp",
        "lossy.webp",
        lambda data: data.__setitem__(slice(data.find(b"VP8 ") + 14, data.find(b"VP8 ") + 16), b"\0\0"),
    )
    write_mutated_webp(
        "bad_vp8l_signature.webp",
        "lossless.webp",
        lambda data: data.__setitem__(data.find(b"VP8L") + 8, 0),
    )
    write_mutated_webp(
        "bad_vp8l_version.webp",
        "lossless.webp",
        lambda data: data.__setitem__(data.find(b"VP8L") + 12, data[data.find(b"VP8L") + 12] | 0x20),
    )
    write_mutated_webp(
        "bad_initial_chunk.webp",
        "lossy.webp",
        lambda data: data.__setitem__(slice(12, 16), b"JUNK"),
    )

    def remove_extended_image_chunk(data):
        image_chunk = data.find(b"VP8L")
        data[image_chunk : image_chunk + 4] = b"JUNK"

    write_mutated_webp("extended_missing_image_chunk.webp", "icc.webp", remove_extended_image_chunk)

    def overflow_extended_canvas(data):
        vp8x = data.find(b"VP8X")
        data[vp8x + 12 : vp8x + 18] = b"\xff" * 6

    write_mutated_webp("extended_canvas_too_large.webp", "animated.webp", overflow_extended_canvas)

    def set_first_anmf_size(data, size):
        anmf = data.find(b"ANMF")
        struct.pack_into("<I", data, anmf + 4, size)

    write_mutated_webp(
        "bad_anmf_scan_size.webp", "animated.webp", lambda data: set_first_anmf_size(data, 20)
    )
    write_mutated_webp(
        "bad_anmf_decode_size.webp", "animated.webp", lambda data: set_first_anmf_size(data, 24)
    )

    def enlarge_anim_chunk(data):
        anim = data.find(b"ANIM")
        end = anim + 8 + 6
        data[end:end] = b"\0\0"
        struct.pack_into("<I", data, anim + 4, 8)
        struct.pack_into("<I", data, 4, len(data) - 8)

    write_mutated_webp("bad_anim_size.webp", "animated.webp", enlarge_anim_chunk)

    def shrink_anim_chunk(data):
        anim = data.find(b"ANIM")
        del data[anim + 8 + 4 : anim + 8 + 6]
        struct.pack_into("<I", data, anim + 4, 4)
        struct.pack_into("<I", data, 4, len(data) - 8)

    write_mutated_webp("anim_chunk_too_small.webp", "animated.webp", shrink_anim_chunk)

    def set_animation_loop(data, count):
        anim = data.find(b"ANIM")
        struct.pack_into("<H", data, anim + 12, count)

    write_mutated_webp("animated_loop_twice.webp", "animated.webp", lambda data: set_animation_loop(data, 2))

    def mutate_anmf_field(data, offset, value):
        anmf = data.find(b"ANMF")
        data[anmf + 8 + offset : anmf + 8 + offset + len(value)] = value

    write_mutated_webp(
        "animated_frame_too_large.webp",
        "animated.webp",
        lambda data: mutate_anmf_field(data, 6, b"\xff\xff\x00"),
    )
    write_mutated_webp(
        "animated_frame_outside.webp",
        "animated.webp",
        lambda data: mutate_anmf_field(data, 0, b"\x01\x00\x00"),
    )
    write_mutated_webp(
        "animated_frame_dimension_mismatch.webp",
        "animated.webp",
        lambda data: mutate_anmf_field(data, 6, b"\x3e\x00\x00"),
    )

    def set_nested_chunk_size(data, chunk_name, size):
        anmf = data.find(b"ANMF")
        chunk = data.find(chunk_name, anmf + 8)
        struct.pack_into("<I", data, chunk + 4, size)

    write_mutated_webp(
        "animated_nested_chunk_too_large.webp",
        "animated.webp",
        lambda data: set_nested_chunk_size(data, b"VP8 ", 0x100000),
    )

    def replace_nested_chunk(data):
        anmf = data.find(b"ANMF")
        chunk = data.find(b"VP8 ", anmf + 8)
        data[chunk : chunk + 4] = b"JUNK"

    write_mutated_webp("animated_bad_nested_chunk.webp", "animated.webp", replace_nested_chunk)
    write_mutated_webp(
        "animated_alpha_chunk_too_large.webp",
        "animated_alpha_lossy.webp",
        lambda data: set_nested_chunk_size(
            data, b"ALPH", struct.unpack_from("<I", data, data.find(b"ANMF") + 4)[0] - 28
        ),
    )

    def enlarge_nested_vp8(data):
        anmf = data.find(b"ANMF")
        vp8 = data.find(b"VP8 ", anmf + 8)
        struct.pack_into("<I", data, vp8 + 4, 0x100000)

    write_mutated_webp(
        "animated_alpha_vp8_too_large.webp", "animated_alpha_lossy.webp", enlarge_nested_vp8
    )

    def set_alpha_info(data, mask, value):
        alpha = data.find(b"ALPH")
        if alpha < 0:
            raise RuntimeError("WebP did not contain an ALPH chunk")
        data[alpha + 8] = (data[alpha + 8] & ~mask) | value

    write_mutated_webp(
        "alpha_invalid_preprocessing.webp",
        "alpha_lossy_horizontal.webp",
        lambda data: set_alpha_info(data, 0x30, 0x20),
    )
    write_mutated_webp(
        "alpha_invalid_compression.webp",
        "alpha_lossy_horizontal.webp",
        lambda data: set_alpha_info(data, 0x03, 0x02),
    )
    write_mutated_webp(
        "alpha_preprocessing.webp",
        "alpha_lossy_horizontal.webp",
        lambda data: set_alpha_info(data, 0x30, 0x10),
    )
    print(f"  WebP: {len(list(d.glob('*.webp')))} files")


def write_rgb_tiff(
    path, image, byte_order="<", tile_size=None, compression=1, predictor=1
):
    """Write a minimal classic RGB TIFF with explicit byte order/organization."""
    width, height = image.size
    pixels = image.convert("RGB").tobytes()
    marker = b"II" if byte_order == "<" else b"MM"
    entries = []

    def entry(tag, field_type, count, value):
        entries.append((tag, field_type, count, value))

    entry(256, 4, 1, width)
    entry(257, 4, 1, height)
    entry(258, 3, 3, "bits")
    entry(259, 3, 1, compression)
    entry(262, 3, 1, 2)
    entry(277, 3, 1, 3)
    entry(284, 3, 1, 1)
    if predictor != 1:
        entry(317, 3, 1, predictor)
    if tile_size is None:
        entry(273, 4, 1, "pixels")
        entry(278, 4, 1, height)
        entry(279, 4, 1, len(pixels))
    else:
        tiles_across = (width + tile_size - 1) // tile_size
        tiles_down = (height + tile_size - 1) // tile_size
        tile_payloads = []
        for tile_y in range(tiles_down):
            for tile_x in range(tiles_across):
                payload = bytearray(tile_size * tile_size * 3)
                for y in range(tile_size):
                    source_y = tile_y * tile_size + y
                    if source_y >= height:
                        break
                    copy_width = min(tile_size, width - tile_x * tile_size)
                    source = (source_y * width + tile_x * tile_size) * 3
                    destination = y * tile_size * 3
                    payload[destination : destination + copy_width * 3] = pixels[
                        source : source + copy_width * 3
                    ]
                if predictor == 2:
                    row_bytes = tile_size * 3
                    for row_start in range(0, len(payload), row_bytes):
                        for index in range(row_bytes - 1, 2, -1):
                            position = row_start + index
                            payload[position] = (
                                payload[position] - payload[position - 3]
                            ) & 255
                if compression in (8, 32946):
                    tile_payloads.append(zlib.compress(payload))
                elif compression == 1:
                    tile_payloads.append(bytes(payload))
                else:
                    raise ValueError(f"unsupported tiled TIFF compression {compression}")
        entry(322, 4, 1, tile_size)
        entry(323, 4, 1, tile_size)
        entry(324, 4, len(tile_payloads), "tile_offsets")
        entry(325, 4, len(tile_payloads), "tile_counts")

    entries.sort()
    ifd_size = 2 + len(entries) * 12 + 4
    cursor = 8 + ifd_size
    bits_offset = cursor
    cursor += 6
    if cursor & 1:
        cursor += 1
    if tile_size is None:
        pixel_offset = cursor
    else:
        offsets_offset = cursor
        cursor += len(tile_payloads) * 4
        counts_offset = cursor
        cursor += len(tile_payloads) * 4
        tile_offsets = []
        for payload in tile_payloads:
            tile_offsets.append(cursor)
            cursor += len(payload)

    output = bytearray(marker + struct.pack(byte_order + "H", 42) + struct.pack(byte_order + "I", 8))
    output.extend(struct.pack(byte_order + "H", len(entries)))
    for tag, field_type, count, value in entries:
        output.extend(struct.pack(byte_order + "HHI", tag, field_type, count))
        if value == "bits":
            output.extend(struct.pack(byte_order + "I", bits_offset))
        elif value == "pixels":
            output.extend(struct.pack(byte_order + "I", pixel_offset))
        elif value == "tile_offsets":
            output.extend(struct.pack(byte_order + "I", offsets_offset))
        elif value == "tile_counts":
            output.extend(struct.pack(byte_order + "I", counts_offset))
        elif field_type == 3:
            output.extend(struct.pack(byte_order + "H", value) + b"\0\0")
        else:
            output.extend(struct.pack(byte_order + "I", value))
    output.extend(struct.pack(byte_order + "I", 0))
    output.extend(struct.pack(byte_order + "HHH", 8, 8, 8))
    if len(output) & 1:
        output.append(0)
    if tile_size is None:
        output.extend(pixels)
    else:
        output.extend(struct.pack(byte_order + f"{len(tile_offsets)}I", *tile_offsets))
        output.extend(
            struct.pack(
                byte_order + f"{len(tile_payloads)}I",
                *(len(payload) for payload in tile_payloads),
            )
        )
        for payload in tile_payloads:
            output.extend(payload)
    path.write_bytes(output)


def write_rgb_multistrip_tiff(path, image, rows_per_strip):
    """Write a minimal little-endian RGB TIFF with multiple strips."""
    width, height = image.size
    pixels = image.convert("RGB").tobytes()
    row_bytes = width * 3
    strips = [
        pixels[start * row_bytes : min(start + rows_per_strip, height) * row_bytes]
        for start in range(0, height, rows_per_strip)
    ]
    entry_count = 10
    cursor = 8 + 2 + entry_count * 12 + 4
    bits_offset = cursor
    cursor += 6
    offsets_offset = cursor
    cursor += len(strips) * 4
    counts_offset = cursor
    cursor += len(strips) * 4
    strip_offsets = []
    for strip in strips:
        strip_offsets.append(cursor)
        cursor += len(strip)

    entries = [
        (256, 4, 1, width),
        (257, 4, 1, height),
        (258, 3, 3, bits_offset),
        (259, 3, 1, 1),
        (262, 3, 1, 2),
        (273, 4, len(strips), offsets_offset),
        (277, 3, 1, 3),
        (278, 4, 1, rows_per_strip),
        (279, 4, len(strips), counts_offset),
        (284, 3, 1, 1),
    ]
    output = bytearray(b"II*\0\x08\0\0\0")
    output.extend(struct.pack("<H", len(entries)))
    for tag, field_type, count, value in entries:
        output.extend(struct.pack("<HHI", tag, field_type, count))
        if field_type == 3 and count == 1:
            output.extend(struct.pack("<H", value) + b"\0\0")
        else:
            output.extend(struct.pack("<I", value))
    output.extend(struct.pack("<I", 0))
    output.extend(struct.pack("<HHH", 8, 8, 8))
    output.extend(struct.pack(f"<{len(strips)}I", *strip_offsets))
    output.extend(struct.pack(f"<{len(strips)}I", *(len(strip) for strip in strips)))
    for strip in strips:
        output.extend(strip)
    path.write_bytes(output)


def write_low_depth_tiff(path, image, bits, photometric):
    """Write a packed grayscale or palette classic TIFF."""
    width, height = image.size
    maximum = (1 << bits) - 1
    rows = []
    for y in range(height):
        packed = bytearray((width * bits + 7) // 8)
        for x in range(width):
            if photometric == 3:
                sample = (x * 3 + y * 5) & maximum
            else:
                luminance = image.getpixel((x, y))
                sample = (luminance * maximum + 127) // 255
                if photometric == 0:
                    sample = maximum - sample
            bit = x * bits
            packed[bit // 8] |= sample << (8 - bits - bit % 8)
        rows.append(bytes(packed))
    pixels = b"".join(rows)

    entries = [
        (256, 4, 1, width),
        (257, 4, 1, height),
        (258, 3, 1, bits),
        (259, 3, 1, 1),
        (262, 3, 1, photometric),
        (273, 4, 1, "pixels"),
        (277, 3, 1, 1),
        (278, 4, 1, height),
        (279, 4, 1, len(pixels)),
    ]
    color_map = []
    if photometric == 3:
        for channel in range(3):
            for index in range(maximum + 1):
                if channel == 0:
                    value = index * 255 // maximum
                elif channel == 1:
                    value = (maximum - index) * 255 // maximum
                else:
                    value = (index * 97) & 255
                color_map.append(value * 257)
        entries.append((320, 3, len(color_map), "color_map"))
    entries.sort()

    cursor = 8 + 2 + len(entries) * 12 + 4
    color_map_offset = cursor
    cursor += len(color_map) * 2
    pixel_offset = cursor
    output = bytearray(b"II*\0\x08\0\0\0")
    output.extend(struct.pack("<H", len(entries)))
    for tag, field_type, count, value in entries:
        output.extend(struct.pack("<HHI", tag, field_type, count))
        if value == "pixels":
            output.extend(struct.pack("<I", pixel_offset))
        elif value == "color_map":
            output.extend(struct.pack("<I", color_map_offset))
        elif field_type == 3:
            output.extend(struct.pack("<H", value) + b"\0\0")
        else:
            output.extend(struct.pack("<I", value))
    output.extend(struct.pack("<I", 0))
    if color_map:
        output.extend(struct.pack(f"<{len(color_map)}H", *color_map))
    output.extend(pixels)
    path.write_bytes(output)


def write_ycbcr_tiff(path, image):
    """Write Pillow's baseline four-byte RGBX storage for YCbCr TIFF."""
    width, height = image.size
    ycbcr = image.convert("YCbCr").tobytes()
    pixels = b"".join(
        ycbcr[offset : offset + 3] + b"\0" for offset in range(0, len(ycbcr), 3)
    )
    entries = [
        (256, 4, 1, width),
        (257, 4, 1, height),
        (258, 3, 3, "bits"),
        (259, 3, 1, 1),
        (262, 3, 1, 6),
        (273, 4, 1, "pixels"),
        (277, 3, 1, 3),
        (278, 4, 1, height),
        (279, 4, 1, len(pixels)),
        (284, 3, 1, 1),
        (530, 3, 2, "subsampling"),
    ]
    entries.sort()
    cursor = 8 + 2 + len(entries) * 12 + 4
    bits_offset = cursor
    cursor += 6
    pixel_offset = cursor
    output = bytearray(b"II*\0\x08\0\0\0")
    output.extend(struct.pack("<H", len(entries)))
    for tag, field_type, count, value in entries:
        output.extend(struct.pack("<HHI", tag, field_type, count))
        if value == "bits":
            output.extend(struct.pack("<I", bits_offset))
        elif value == "pixels":
            output.extend(struct.pack("<I", pixel_offset))
        elif value == "subsampling":
            output.extend(struct.pack("<HH", 1, 1))
        elif field_type == 3:
            output.extend(struct.pack("<H", value) + b"\0\0")
        else:
            output.extend(struct.pack("<I", value))
    output.extend(struct.pack("<I", 0))
    output.extend(struct.pack("<HHH", 8, 8, 8))
    output.extend(pixels)
    path.write_bytes(output)


def mutate_tiff_tag(source, destination, tag, value, value_index=0):
    """Patch one classic-TIFF integer tag value for malformed fixtures."""
    data = bytearray(source.read_bytes())
    byte_order = "<" if data[:2] == b"II" else ">"
    ifd_offset = struct.unpack_from(byte_order + "I", data, 4)[0]
    entry_count = struct.unpack_from(byte_order + "H", data, ifd_offset)[0]
    for index in range(entry_count):
        start = ifd_offset + 2 + index * 12
        actual_tag, field_type, count = struct.unpack_from(
            byte_order + "HHI", data, start
        )
        if actual_tag != tag:
            continue
        if value_index >= count or field_type not in (3, 4):
            raise ValueError(f"cannot patch TIFF tag {tag} value {value_index}")
        item_size = 2 if field_type == 3 else 4
        value_position = (
            start + 8
            if count * item_size <= 4
            else struct.unpack_from(byte_order + "I", data, start + 8)[0]
        )
        format_code = "H" if field_type == 3 else "I"
        struct.pack_into(
            byte_order + format_code,
            data,
            value_position + value_index * item_size,
            value,
        )
        destination.write_bytes(data)
        return
    raise ValueError(f"TIFF tag {tag} not found")


def gen_tiff():
    d = OUT / "tiff"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    img.save(d / "rgb.tiff")
    img.save(d / "single.tiff")
    img.convert("L").save(d / "gray.tiff")
    img.convert("1").save(d / "1bit.tiff")
    img.convert("L").save(d / "8bit.tiff")
    img.convert("I;16").save(d / "16bit.tiff")
    img.convert("F").save(d / "float32.tiff")
    img.convert("RGBA").save(d / "rgba.tiff")
    img.convert("P").save(d / "palette.tiff")
    img.convert("CMYK").save(d / "cmyk.tiff")
    write_ycbcr_tiff(d / "ycbcr.tiff", img.resize((17, 13)))
    img.convert("1").save(d / "bilevel.tiff")
    low_depth = img.convert("L").resize((17, 13))
    write_low_depth_tiff(d / "miniswhite_1bit.tiff", low_depth, 1, 0)
    write_low_depth_tiff(d / "miniswhite_8bit.tiff", low_depth, 8, 0)
    write_low_depth_tiff(d / "gray2.tiff", low_depth, 2, 1)
    write_low_depth_tiff(d / "gray4.tiff", low_depth, 4, 1)
    write_low_depth_tiff(d / "palette2.tiff", low_depth, 2, 3)
    write_low_depth_tiff(d / "palette4.tiff", low_depth, 4, 3)
    img.save(d / "uncompressed.tiff", compression=None)
    img.save(d / "lzw.tiff", compression="tiff_lzw")
    img.save(d / "deflate.tiff", compression="tiff_adobe_deflate")
    img.save(d / "packbits.tiff", compression="packbits")
    img.convert("L").save(d / "gray_lzw.tiff", compression="tiff_lzw")
    img.convert("L").save(d / "gray_deflate.tiff", compression="tiff_adobe_deflate")
    img.convert("RGBA").save(d / "rgba_lzw.tiff", compression="tiff_lzw")
    img.save(d / "le.tiff")  # little-endian default
    write_rgb_tiff(d / "be.tiff", img, byte_order=">")
    write_rgb_multistrip_tiff(d / "stripped.tiff", img, rows_per_strip=16)
    write_rgb_tiff(d / "tiled.tiff", img, tile_size=32)
    write_rgb_tiff(
        d / "tiled_deflate_predictor.tiff",
        img,
        tile_size=32,
        compression=8,
        predictor=2,
    )
    img.save(
        d / "rgb_lzw_predictor.tiff",
        compression="tiff_lzw",
        tiffinfo={317: 2},
    )
    img.save(
        d / "rgb_deflate_predictor.tiff",
        compression="tiff_adobe_deflate",
        tiffinfo={317: 2},
    )
    img.convert("I;16").save(
        d / "gray16_lzw_predictor.tiff",
        compression="tiff_lzw",
        tiffinfo={317: 2},
    )
    img.convert("I;16").save(
        d / "gray16_deflate_predictor.tiff",
        compression="tiff_adobe_deflate",
        tiffinfo={317: 2},
    )
    img.save(d / "multipage.tiff", save_all=True, append_images=[img.transpose(Image.Transpose.FLIP_LEFT_RIGHT)])
    d.joinpath("bad_ifd.tiff").write_bytes(b"II\x2a\x00\x08\x00\x00\x00\xff\xff\xff")
    invalid_magic = bytearray((d / "rgb.tiff").read_bytes())
    invalid_magic[2:4] = b"+\0"
    (d / "invalid_magic.tiff").write_bytes(invalid_magic)
    mutate_tiff_tag(d / "rgb.tiff", d / "zero_width.tiff", 256, 0)
    mutate_tiff_tag(d / "rgb.tiff", d / "zero_height.tiff", 257, 0)
    mutate_tiff_tag(d / "rgb.tiff", d / "mixed_bits.tiff", 258, 16, 1)
    mutate_tiff_tag(d / "rgb.tiff", d / "rows_zero.tiff", 278, 0)
    mutate_tiff_tag(d / "rgb.tiff", d / "unknown_compression.tiff", 259, 999)
    mutate_tiff_tag(
        d / "rgb_deflate_predictor.tiff",
        d / "invalid_predictor.tiff",
        317,
        3,
    )
    mutate_tiff_tag(d / "rgb.tiff", d / "oob_strip.tiff", 273, 0xFFFF_FFF0)
    print(f"  TIFF: {len(list(d.glob('*.tiff')))} files")


def gen_ico():
    d = OUT / "ico"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB").resize((16,16))
    img.save(d / "16x16.ico", format="ICO", sizes=[(16,16)])
    img.save(d / "single.ico", format="ICO", sizes=[(16,16)])
    pattern_img("RGB").save(d / "multi.ico", format="ICO", sizes=[(16,16),(32,32)])
    img.convert("RGBA").resize((32,32)).save(d / "png_entry.ico", format="ICO", sizes=[(32,32)])
    img.resize((16,16)).save(
        d / "bmp_entry.ico",
        format="ICO",
        sizes=[(16,16)],
        bitmap_format="bmp",
    )
    img.convert("1").resize((16,16)).save(
        d / "bmp_1bit.ico", format="ICO", sizes=[(16,16)], bitmap_format="bmp"
    )
    img.convert("P", palette=Image.Palette.ADAPTIVE, colors=64).resize((16,16)).save(
        d / "bmp_8bit.ico", format="ICO", sizes=[(16,16)], bitmap_format="bmp"
    )
    img.convert("RGBA").resize((16,16)).save(
        d / "bmp_32bit.ico", format="ICO", sizes=[(16,16)], bitmap_format="bmp"
    )
    palette = [
        ((index * 17) & 255, (index * 53) & 255, (index * 97) & 255)
        for index in range(16)
    ]
    xor_rows = bytearray()
    for y in reversed(range(16)):
        for x in range(0, 16, 2):
            xor_rows.append(((x + y) % 16) << 4 | ((x + y + 1) % 16))
    and_mask = bytes(4 * 16)
    dib = bytearray()
    dib.extend(struct.pack("<IiiHHIIiiII", 40, 16, 32, 1, 4, 0, len(xor_rows) + len(and_mask), 0, 0, 16, 16))
    for red, green, blue in palette:
        dib.extend(bytes((blue, green, red, 0)))
    dib.extend(xor_rows)
    dib.extend(and_mask)
    ico = bytearray(struct.pack("<HHH", 0, 1, 1))
    ico.extend(struct.pack("<BBBBHHII", 16, 16, 16, 0, 1, 4, len(dib), 22))
    ico.extend(dib)
    (d / "bmp_4bit.ico").write_bytes(ico)
    cursor = bytearray(ico)
    struct.pack_into("<H", cursor, 2, 2)
    struct.pack_into("<HH", cursor, 10, 3, 5)
    (d / "cursor.cur").write_bytes(cursor)

    (d / "empty.ico").write_bytes(b"")
    (d / "invalid_reserved.ico").write_bytes(struct.pack("<HHH", 1, 1, 0))
    (d / "invalid_type.ico").write_bytes(struct.pack("<HHH", 0, 3, 0))
    (d / "zero_entries.ico").write_bytes(struct.pack("<HHH", 0, 1, 0))
    (d / "truncated_directory.ico").write_bytes(struct.pack("<HHH", 0, 1, 1) + b"\0" * 4)
    zero_entry = bytearray(struct.pack("<HHH", 0, 1, 1))
    zero_entry.extend(struct.pack("<BBBBHHII", 16, 16, 0, 0, 1, 32, 0, 0))
    (d / "zero_entry.ico").write_bytes(zero_entry)
    (d / "truncated_entry.ico").write_bytes(ico[:-20])
    img.resize((256,256)).save(d / "256x256.ico", format="ICO", sizes=[(256,256)])
    print(f"  ICO: {len(list(d.glob('*.ico')))} files")


def main():
    generators = {
        "jpeg": gen_jpeg,
        "png": gen_png,
        "gif": gen_gif,
        "bmp": gen_bmp,
        "webp": gen_webp,
        "tiff": gen_tiff,
        "ico": gen_ico,
    }
    parser = argparse.ArgumentParser()
    parser.add_argument("--format", choices=generators)
    args = parser.parse_args()
    selected = [args.format] if args.format else generators
    for format_name in selected:
        generators[format_name]()
    print("\nDone. Run: .oracle-venv/bin/python scripts/generate_decode_refs.py")


if __name__ == "__main__":
    main()
