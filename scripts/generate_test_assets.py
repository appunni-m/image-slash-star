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
import hashlib
import os
import random
import struct
import subprocess
import tempfile
import zlib
from io import BytesIO
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


def jpeg_segment(data, marker):
    """Return ``(start, payload_start, end)`` for a pre-scan JPEG segment."""
    position = 2
    while position + 3 < len(data):
        if data[position] != 0xFF:
            position += 1
            continue
        while position < len(data) and data[position] == 0xFF:
            position += 1
        if position >= len(data):
            break
        code = data[position]
        start = position - 1
        position += 1
        if code in (0xD8, 0xD9) or 0xD0 <= code <= 0xD7:
            continue
        if position + 2 > len(data):
            break
        length = struct.unpack(">H", data[position : position + 2])[0]
        end = position + length
        if code == marker:
            return start, position + 2, end
        if code == 0xDA:
            break
        position = end
    raise ValueError(f"JPEG marker FF{marker:02X} not found")


def mutate_jpeg_payload(data, marker, offset, value):
    mutated = bytearray(data)
    _, payload_start, _ = jpeg_segment(mutated, marker)
    mutated[payload_start + offset] = value
    return bytes(mutated)


def jpeg_segments(data, marker):
    """Return every ``(start, payload_start, end)`` for a pre-scan segment."""
    segments = []
    position = 2
    while position + 3 < len(data):
        if data[position] != 0xFF:
            position += 1
            continue
        while position < len(data) and data[position] == 0xFF:
            position += 1
        if position >= len(data):
            break
        code = data[position]
        start = position - 1
        position += 1
        if code in (0xD8, 0xD9) or 0xD0 <= code <= 0xD7:
            continue
        if position + 2 > len(data):
            break
        length = struct.unpack(">H", data[position : position + 2])[0]
        end = position + length
        if code == marker:
            segments.append((start, position + 2, end))
        if code == 0xDA:
            break
        position = end
    return segments


def remove_jpeg_segments(data, marker):
    """Remove every pre-scan segment with ``marker``."""
    output = bytearray()
    position = 0
    for start, _, end in jpeg_segments(data, marker):
        output.extend(data[position:start])
        position = end
    output.extend(data[position:])
    return bytes(output)


def mutate_jpeg_huffman_table_id(data, table_class, table_id):
    """Move the first DHT table of ``table_class`` to ``table_id``."""
    mutated = bytearray(data)
    for _, payload_start, _ in jpeg_segments(mutated, 0xC4):
        info = mutated[payload_start]
        if info >> 4 == table_class:
            mutated[payload_start] = (info & 0xF0) | table_id
            return bytes(mutated)
    raise ValueError(f"JPEG DHT class {table_class} not found")


def zero_sample_jpeg(base, width, height, y_sampling):
    """Build a one-MCU RGB JPEG with standard-table zero coefficient blocks."""
    data = bytearray(base)
    _, sof_payload, _ = jpeg_segment(data, 0xC0)
    data[sof_payload + 1 : sof_payload + 3] = struct.pack(">H", height)
    data[sof_payload + 3 : sof_payload + 5] = struct.pack(">H", width)
    data[sof_payload + 7] = y_sampling

    _, _, entropy_start = jpeg_segment(data, 0xDA)
    y_blocks = (y_sampling >> 4) * (y_sampling & 0x0F)
    # Standard tables: luminance DC(0)+EOB = 00 1010; chroma = 00 00.
    bits = "001010" * y_blocks + "0000" * 2
    bits += "1" * ((-len(bits)) % 8)
    entropy = bytes(int(bits[offset : offset + 8], 2) for offset in range(0, len(bits), 8))
    return bytes(data[:entropy_start]) + entropy + b"\xff\xd9"


def png_chunk(kind, payload):
    return (
        struct.pack(">I", len(payload))
        + kind
        + payload
        + struct.pack(">I", binascii.crc32(kind + payload) & 0xFFFF_FFFF)
    )


def png_chunks(data):
    """Return every complete PNG chunk as ``(kind, payload)``."""
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("invalid PNG signature")
    chunks = []
    position = 8
    while position < len(data):
        if position + 12 > len(data):
            raise ValueError("truncated PNG chunk")
        length = struct.unpack_from(">I", data, position)[0]
        end = position + 12 + length
        if end > len(data):
            raise ValueError("truncated PNG chunk")
        kind = data[position + 4 : position + 8]
        payload = data[position + 8 : position + 8 + length]
        chunks.append((kind, payload))
        position = end
        if kind == b"IEND":
            break
    return chunks


def rebuild_png(chunks):
    """Serialize PNG chunks with fresh CRCs."""
    return b"\x89PNG\r\n\x1a\n" + b"".join(
        png_chunk(kind, payload) for kind, payload in chunks
    )


def mutate_png_chunk(data, kind, occurrence, mutate):
    """Mutate one PNG chunk payload and recompute every chunk CRC."""
    chunks = png_chunks(data)
    seen = 0
    for index, (chunk_kind, payload) in enumerate(chunks):
        if chunk_kind != kind:
            continue
        if seen == occurrence:
            mutable = bytearray(payload)
            mutate(mutable)
            chunks[index] = (chunk_kind, bytes(mutable))
            return rebuild_png(chunks)
        seen += 1
    raise ValueError(f"PNG chunk {kind!r} occurrence {occurrence} not found")


def deflate_bits(fields):
    """Pack ``(value, width)`` DEFLATE fields in least-significant-bit order."""
    output = bytearray()
    accumulator = 0
    bit_count = 0
    for value, width in fields:
        accumulator |= value << bit_count
        bit_count += width
        while bit_count >= 8:
            output.append(accumulator & 0xFF)
            accumulator >>= 8
            bit_count -= 8
    if bit_count:
        output.append(accumulator & 0xFF)
    return bytes(output)


def reverse_bits(value, width):
    reversed_value = 0
    for _ in range(width):
        reversed_value = (reversed_value << 1) | (value & 1)
        value >>= 1
    return reversed_value


def fixed_deflate_symbol(symbol):
    """Return the bit-reversed canonical code and width for a fixed tree symbol."""
    if symbol <= 143:
        code, width = symbol + 0x30, 8
    elif symbol <= 255:
        code, width = symbol - 144 + 0x190, 9
    elif symbol <= 279:
        code, width = symbol - 256, 7
    elif symbol <= 287:
        code, width = symbol - 280 + 0xC0, 8
    else:
        raise ValueError("invalid fixed DEFLATE symbol")
    return reverse_bits(code, width), width


def malformed_fixed_zlib(symbols, distances=()):
    """Build a zlib stream with a final fixed-Huffman DEFLATE block."""
    fields = [(1, 1), (1, 2)]
    distance_iter = iter(distances)
    for symbol in symbols:
        fields.append(fixed_deflate_symbol(symbol))
        if 257 <= symbol <= 285:
            # These fixtures use symbol 257, whose length has no extra bits.
            distance = next(distance_iter)
            fields.append((reverse_bits(distance, 5), 5))
    payload = deflate_bits(fields)
    return b"\x78\x01" + payload + b"\x00\x00\x00\x01"


def malformed_dynamic_zlib(code_length_lengths, encoded_fields=()):
    """Build a zlib stream around a malformed final dynamic DEFLATE block."""
    if len(code_length_lengths) < 4:
        raise ValueError("dynamic blocks encode at least four code lengths")
    fields = [
        (1, 1),
        (2, 2),
        (0, 5),  # HLIT: 257 literal/length symbols
        (0, 5),  # HDIST: one distance symbol
        (len(code_length_lengths) - 4, 4),
    ]
    fields.extend((length, 3) for length in code_length_lengths)
    fields.extend(encoded_fields)
    return b"\x78\x01" + deflate_bits(fields) + b"\x00\x00\x00\x01"


def minimal_dynamic_zlib():
    """Encode one black RGB scanline with a deliberately small dynamic tree."""
    # Code-length symbols 0, 1, 17, and 18 each have width two. Their reversed
    # canonical codes are respectively 00, 10, 01, and 11.
    code_length_lengths = [0, 2, 2, 2] + [0] * 13 + [2]
    encoded_lengths = [
        (2, 2),  # literal symbol 0 has length one
        (3, 2),
        (127, 7),  # 138 zero lengths
        (3, 2),
        (106, 7),  # 117 zero lengths
        (2, 2),  # end-of-block symbol 256 has length one
        (2, 2),  # distance symbol 0 has length one
    ]
    fields = [
        (1, 1),
        (2, 2),
        (0, 5),
        (0, 5),
        (14, 4),
    ]
    fields.extend((length, 3) for length in code_length_lengths)
    fields.extend(encoded_lengths)
    fields.extend([(0, 1)] * 4)  # filter byte and black RGB pixel
    fields.append((1, 1))  # end-of-block
    return b"\x78\x01" + deflate_bits(fields) + b"\x00\x04\x00\x01"


def invalid_dynamic_backreference_zlib():
    """Encode valid dynamic tables followed by a back-reference before output."""
    # Symbols 0, 1, 2, and 18 form a complete width-two code-length tree.
    code_length_lengths = [0, 0, 2, 2] + [0] * 11 + [2, 0, 2]
    fields = [
        (1, 1),
        (2, 2),
        (1, 5),  # HLIT: include literal/length symbol 257
        (0, 5),
        (14, 4),
    ]
    fields.extend((length, 3) for length in code_length_lengths)
    fields.extend(
        [
            (1, 2),
            (1, 2),  # symbols 0 and 1 have length two
            (3, 2),
            (127, 7),  # 138 zero lengths
            (3, 2),
            (105, 7),  # 116 zero lengths
            (1, 2),
            (1, 2),  # symbols 256 and 257 have length two
            (2, 2),  # distance symbol 0 has length one
            (3, 2),  # literal/length symbol 257
            (0, 1),  # distance one, invalid before any output
        ]
    )
    return b"\x78\x01" + deflate_bits(fields) + b"\x00\x00\x00\x01"


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
    pattern_img("RGBA", (17, 19)).save(out_dir / "rgba_odd.png")
    subtract_green_rng = random.Random(0)
    subtract_green = Image.new("RGB", (17, 17))
    subtract_green_pixels = []
    for _ in range(17 * 17):
        green = subtract_green_rng.randrange(256)
        red = (green + subtract_green_rng.choice([3, 5])) & 255
        blue = (green + subtract_green_rng.choice([7, 11])) & 255
        subtract_green_pixels.append((red, green, blue))
    subtract_green.putdata(subtract_green_pixels)
    subtract_green.save(out_dir / "webp_subtract_green.png")
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
    (d / "cmyk_no_adobe_app14.jpg").write_bytes(
        remove_jpeg_segments((d / "baseline_cmyk.jpg").read_bytes(), 0xEE)
    )
    Image.new("L", (2048, 1024), 128).save(d / "progressive_eob_source.jpg", quality=85)
    img.save(d / "progressive.jpg", quality=85, progressive=True)
    img.save(d / "progressive_spectral.jpg", quality=70, progressive=True)
    img.convert("L").save(d / "progressive_gray.jpg", quality=85, progressive=True)
    img.convert("CMYK").save(d / "progressive_cmyk.jpg", quality=85, progressive=True)
    img.save(
        d / "progressive_restart.jpg",
        quality=85,
        progressive=True,
        restart_marker_rows=2,
    )
    img.save(d / "restart.jpg", quality=85, restart_marker_rows=4)
    pattern_img("RGB", (1, 1)).save(d / "1x1.jpg", quality=95)
    pattern_img("RGB", (1, 8)).save(
        d / "1x8_422.jpg", quality=95, subsampling=1
    )
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
    baseline = (d / "baseline.jpg").read_bytes()
    baseline_gray = (d / "baseline_gray.jpg").read_bytes()

    def jpeg_marker_segment(marker, payload):
        return (
            b"\xff"
            + bytes([marker])
            + struct.pack(">H", len(payload) + 2)
            + payload
        )

    minimal_sof = jpeg_marker_segment(
        0xC0, b"\x08\x00\x01\x00\x01\x01\x01\x11\x00"
    )

    def jpeg_truncated_sof(prefix_len):
        payload = b"\x08\x00\x01\x00\x01\x01\x01\x11\x00"[:prefix_len]
        return b"\xff\xd8\xff\xc0" + struct.pack(">H", 11) + payload

    def jpeg_truncated_dqt(prefix_len, precision=0):
        if precision == 0:
            payload = bytes([0]) + bytes(range(1, 65))
        else:
            payload = bytes([0x10]) + b"".join(
                struct.pack(">H", value) for value in range(1, 65)
            )
        return (
            b"\xff\xd8\xff\xdb"
            + struct.pack(">H", len(payload) + 2)
            + payload[:prefix_len]
        )

    def jpeg_truncated_dht(prefix_len):
        payload = bytes([0]) + bytes([1] + [0] * 15) + b"\x00"
        return (
            b"\xff\xd8\xff\xc4"
            + struct.pack(">H", len(payload) + 2)
            + payload[:prefix_len]
        )

    def jpeg_truncated_sos(prefix_len):
        payload = b"\x01\x01\x00\x00\x3f\x00"
        return (
            b"\xff\xd8"
            + minimal_sof
            + b"\xff\xda"
            + struct.pack(">H", len(payload) + 2)
            + payload[:prefix_len]
        )

    d.joinpath("sampling_3x1.jpg").write_bytes(
        zero_sample_jpeg(baseline, 24, 8, 0x31)
    )
    d.joinpath("sampling_1x3.jpg").write_bytes(
        zero_sample_jpeg(baseline, 8, 24, 0x13)
    )
    d.joinpath("entropy_eoi_padding.jpg").write_bytes(
        baseline[:-2] + b"\xff\xff\xd9"
    )
    sos_start, _, sos_end = jpeg_segment(baseline, 0xDA)
    d.joinpath("entropy_empty_scan.jpg").write_bytes(
        baseline[:sos_end] + b"\xff\xd9"
    )
    d.joinpath("entropy_early_eoi_1.jpg").write_bytes(
        baseline[:sos_end + 1] + b"\xff\xd9"
    )
    d.joinpath("entropy_early_eoi_64.jpg").write_bytes(
        baseline[:sos_end + 64] + b"\xff\xd9"
    )
    d.joinpath("entropy_truncated_tail_64.jpg").write_bytes(
        baseline[:-66] + b"\xff\xd9"
    )
    d.joinpath("entropy_stuffed_ff_prefix.jpg").write_bytes(
        baseline[:sos_end] + b"\xff\x00" + baseline[sos_end:]
    )
    d.joinpath("entropy_unexpected_marker.jpg").write_bytes(
        baseline.replace(b"\xff\x00", b"\xff\x02\x00\x02", 1)
    )
    d.joinpath("dangling_marker.jpg").write_bytes(b"\xff\xd8\xff")
    d.joinpath("fill_marker_only.jpg").write_bytes(b"\xff\xd8\xff\xff\xd9")
    d.joinpath("markerless_tail.jpg").write_bytes(b"\xff\xd8NO-MARKER")
    d.joinpath("sof_no_length.jpg").write_bytes(b"\xff\xd8\xff\xc0")
    d.joinpath("sof_no_precision.jpg").write_bytes(jpeg_truncated_sof(0))
    d.joinpath("sof_no_height.jpg").write_bytes(jpeg_truncated_sof(1))
    d.joinpath("sof_no_width.jpg").write_bytes(jpeg_truncated_sof(3))
    d.joinpath("sof_no_components.jpg").write_bytes(jpeg_truncated_sof(5))
    d.joinpath("sof_no_comp_id.jpg").write_bytes(jpeg_truncated_sof(6))
    d.joinpath("sof_no_sampling.jpg").write_bytes(jpeg_truncated_sof(7))
    d.joinpath("sof_no_quant.jpg").write_bytes(jpeg_truncated_sof(8))
    d.joinpath("sof_precision_12.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xC0, 0, 12)
    )
    d.joinpath("sof_two_components.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xC0, 5, 2)
    )
    d.joinpath("sof_zero_sampling.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xC0, 7, 0)
    )
    d.joinpath("sof_high_h_sampling.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xC0, 7, 0x51)
    )
    d.joinpath("sof_zero_v_sampling.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xC0, 7, 0x10)
    )
    d.joinpath("sof_high_v_sampling.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xC0, 7, 0x15)
    )
    d.joinpath("sof_bad_quant_table.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xC0, 8, 4)
    )
    d.joinpath("sof_missing_quant_table.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xC0, 8, 2)
    )
    sparse_quant = bytearray(baseline_gray)
    _, dqt_payload, _ = jpeg_segment(sparse_quant, 0xDB)
    sparse_quant[dqt_payload] = (sparse_quant[dqt_payload] & 0xF0) | 3
    _, sof_payload, _ = jpeg_segment(sparse_quant, 0xC0)
    sparse_quant[sof_payload + 8] = 2
    d.joinpath("sof_sparse_quant_table.jpg").write_bytes(bytes(sparse_quant))
    d.joinpath("dqt_bad_table.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xDB, 0, 4)
    )
    d.joinpath("dqt_no_length.jpg").write_bytes(b"\xff\xd8\xff\xdb")
    d.joinpath("dqt_no_info.jpg").write_bytes(jpeg_truncated_dqt(0))
    d.joinpath("dqt_truncated_8bit_value.jpg").write_bytes(
        jpeg_truncated_dqt(10)
    )
    d.joinpath("dqt_truncated_16bit_value.jpg").write_bytes(
        jpeg_truncated_dqt(2, precision=1)
    )
    d.joinpath("dht_bad_table.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xC4, 0, 4)
    )
    d.joinpath("dht_no_length.jpg").write_bytes(b"\xff\xd8\xff\xc4")
    d.joinpath("dht_no_info.jpg").write_bytes(jpeg_truncated_dht(0))
    d.joinpath("dht_truncated_counts.jpg").write_bytes(jpeg_truncated_dht(5))
    d.joinpath("dht_truncated_values.jpg").write_bytes(jpeg_truncated_dht(17))
    dht_start, _, dht_end = jpeg_segment(baseline, 0xC4)
    oversubscribed_dht = b"\xff\xc4" + struct.pack(">H", 22) + bytes([0, 3] + [0] * 15 + [0, 1, 2])
    d.joinpath("dht_oversubscribed.jpg").write_bytes(
        baseline[:dht_start] + oversubscribed_dht + baseline[dht_end:]
    )
    d.joinpath("sos_no_length.jpg").write_bytes(
        b"\xff\xd8" + minimal_sof + b"\xff\xda"
    )
    d.joinpath("sos_no_component_count.jpg").write_bytes(jpeg_truncated_sos(0))
    d.joinpath("sos_no_comp_id.jpg").write_bytes(jpeg_truncated_sos(1))
    d.joinpath("sos_no_table.jpg").write_bytes(jpeg_truncated_sos(2))
    d.joinpath("sos_no_ss.jpg").write_bytes(jpeg_truncated_sos(3))
    d.joinpath("sos_no_se.jpg").write_bytes(jpeg_truncated_sos(4))
    d.joinpath("sos_no_ahal.jpg").write_bytes(jpeg_truncated_sos(5))
    d.joinpath("sos_unknown_component.jpg").write_bytes(
        b"\xff\xd8"
        + minimal_sof
        + jpeg_marker_segment(0xDA, b"\x01\x02\x00\x00\x3f\x00")
    )
    d.joinpath("sos_zero_components.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xDA, 0, 0)
    )
    d.joinpath("sos_bad_dc_table.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xDA, 2, 0x40)
    )
    d.joinpath("sos_bad_ac_table.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xDA, 2, 0x04)
    )
    d.joinpath("sos_missing_dc_table.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xDA, 2, 0x20)
    )
    d.joinpath("sos_missing_ac_table.jpg").write_bytes(
        mutate_jpeg_payload(baseline, 0xDA, 2, 0x02)
    )
    sparse_dc = bytearray(mutate_jpeg_huffman_table_id(baseline_gray, 0, 3))
    _, sos_payload, _ = jpeg_segment(sparse_dc, 0xDA)
    sparse_dc[sos_payload + 2] = (2 << 4) | (sparse_dc[sos_payload + 2] & 0x0F)
    d.joinpath("sos_sparse_dc_table.jpg").write_bytes(bytes(sparse_dc))
    sparse_ac = bytearray(mutate_jpeg_huffman_table_id(baseline_gray, 1, 3))
    _, sos_payload, _ = jpeg_segment(sparse_ac, 0xDA)
    sparse_ac[sos_payload + 2] = (sparse_ac[sos_payload + 2] & 0xF0) | 2
    d.joinpath("sos_sparse_ac_table.jpg").write_bytes(bytes(sparse_ac))
    sof_start, _, sof_end = jpeg_segment(baseline, 0xC0)
    d.joinpath("duplicate_sof.jpg").write_bytes(
        baseline[:sof_end] + baseline[sof_start:sof_end] + baseline[sof_end:]
    )
    d.joinpath("sos_before_sof.jpg").write_bytes(
        baseline[:2] + baseline[sos_start:sos_end] + b"\xff\xd9"
    )
    d.joinpath("eoi_without_sos.jpg").write_bytes(baseline[:sos_start] + b"\xff\xd9")
    d.joinpath("missing_eoi.jpg").write_bytes(baseline[:-2])
    d.joinpath("wrong_soi.jpg").write_bytes(b"\xff\xd7" + baseline[2:])
    near_miss_marker = bytearray(baseline)
    near_miss_marker[2] = 0
    d.joinpath("near_miss_third_marker.jpg").write_bytes(near_miss_marker)
    # Keep marker framing valid through SOS so inspection reaches the malformed
    # SOF fields instead of failing earlier on an absent next marker.
    inspection_sos = b"\xff\xda\x00\x02"
    d.joinpath("truncated_sof_payload.jpg").write_bytes(
        b"\xff\xd8\xff\xc0\x00\x02" + inspection_sos
    )
    d.joinpath("sof_short_height.jpg").write_bytes(
        b"\xff\xd8\xff\xc0\x00\x03\x08" + inspection_sos
    )
    d.joinpath("sof_partial_height.jpg").write_bytes(
        b"\xff\xd8\xff\xc0\x00\x04\x08\x00" + inspection_sos
    )
    d.joinpath("sof_short_width.jpg").write_bytes(
        b"\xff\xd8\xff\xc0\x00\x05\x08\x00\x01" + inspection_sos
    )
    d.joinpath("sof_short_components.jpg").write_bytes(
        b"\xff\xd8\xff\xc0\x00\x07\x08\x00\x01\x00\x01" + inspection_sos
    )
    d.joinpath("sof_short_component_table.jpg").write_bytes(
        b"\xff\xd8\xff\xc0\x00\x08\x08\x00\x01\x00\x01\x03" + inspection_sos
    )
    d.joinpath("fill_marker_truncated.jpg").write_bytes(b"\xff\xd8\xff\xff")
    d.joinpath("prefixed_stuffed_marker.jpg").write_bytes(
        baseline[:2] + b"\xff\x00" + baseline[2:]
    )
    d.joinpath("dri_no_length.jpg").write_bytes(b"\xff\xd8\xff\xdd")
    d.joinpath("dri_no_value.jpg").write_bytes(b"\xff\xd8\xff\xdd\x00\x04\x00")
    d.joinpath("app14_short_length.jpg").write_bytes(
        baseline[:2] + b"\xff\xee\x00\x01" + baseline[2:]
    )
    d.joinpath("app14_no_length.jpg").write_bytes(b"\xff\xd8\xff\xee")
    d.joinpath("app14_declared_too_long.jpg").write_bytes(
        b"\xff\xd8\xff\xee\x00\x10Adobe"
    )
    d.joinpath("app14_truncated_payload.jpg").write_bytes(
        baseline[:2] + b"\xff\xee\xff\xff"
    )
    d.joinpath("app14_non_adobe.jpg").write_bytes(
        baseline[:2] + b"\xff\xee\x00\x0eNotAdobeData" + baseline[2:]
    )
    d.joinpath("app14_adobe_short_transform.jpg").write_bytes(
        b"\xff\xd8\xff\xee\x00\x07Adobe\xff\xd9"
    )
    d.joinpath("tem_marker.jpg").write_bytes(baseline[:2] + b"\xff\x01" + baseline[2:])
    d.joinpath("unknown_no_length.jpg").write_bytes(b"\xff\xd8\xff\xe2")
    d.joinpath("unknown_short_length.jpg").write_bytes(
        baseline[:2] + b"\xff\xe2\x00\x01" + baseline[2:]
    )
    zero_width = mutate_jpeg_payload(baseline, 0xC0, 3, 0)
    zero_width = mutate_jpeg_payload(zero_width, 0xC0, 4, 0)
    d.joinpath("sof_zero_width.jpg").write_bytes(zero_width)
    zero_height = mutate_jpeg_payload(baseline, 0xC0, 1, 0)
    zero_height = mutate_jpeg_payload(zero_height, 0xC0, 2, 0)
    d.joinpath("sof_zero_height.jpg").write_bytes(zero_height)
    d.joinpath("restart_before_scan.jpg").write_bytes(b"\xff\xd8\xff\xd0\xff\xd9")
    dqt_start, dqt_payload, dqt_end = jpeg_segment(baseline, 0xDB)
    dqt_source = baseline[dqt_payload:dqt_end]
    wide_dqt_payload = bytes([0x10 | (dqt_source[0] & 0x0F)]) + b"".join(
        struct.pack(">H", value) for value in dqt_source[1:65]
    )
    wide_dqt = b"\xff\xdb" + struct.pack(">H", len(wide_dqt_payload) + 2) + wide_dqt_payload
    d.joinpath("dqt_16bit.jpg").write_bytes(
        baseline[:dqt_start] + wide_dqt + baseline[dqt_end:]
    )
    progressive = (d / "progressive.jpg").read_bytes()
    d.joinpath("progressive_missing_quant_table.jpg").write_bytes(
        mutate_jpeg_payload(progressive, 0xC2, 8, 2)
    )
    _, _, progressive_sos_end = jpeg_segment(progressive, 0xDA)
    d.joinpath("progressive_scan0_empty.jpg").write_bytes(
        progressive[:progressive_sos_end] + b"\xff\xd9"
    )
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
    noise_state = 0x51A7_E123
    noise_pixels = []
    for _ in range(64 * 64):
        noise_state = (1_664_525 * noise_state + 1_013_904_223) & 0xFFFF_FFFF
        noise_pixels.append(
            ((noise_state >> 24) & 255, (noise_state >> 16) & 255, (noise_state >> 8) & 255)
        )
    zlib_stored_source = Image.new("RGB", (64, 64))
    zlib_stored_source.putdata(noise_pixels)
    zlib_stored_source.save(d / "zlib_stored_source.png")
    boundary_pixels = []
    seed = 0xA53C_91E7
    phrase = [(17, 29, 43), (19, 31, 47), (23, 37, 53), (29, 41, 59)]
    for y in range(96):
        for x in range(96):
            if y % 6 in (0, 1, 2):
                r, g, b = phrase[(x + y) % len(phrase)]
                boundary_pixels.append(((r + x) & 255, (g + y * 3) & 255, (b + x + y) & 255))
            else:
                seed = (1_103_515_245 * seed + 12_345 + x + y * 97) & 0x7FFF_FFFF
                boundary_pixels.append(((seed >> 16) & 255, (seed >> 8) & 255, seed & 255))
    zlib_boundary_source = Image.new("RGB", (96, 96))
    zlib_boundary_source.putdata(boundary_pixels)
    zlib_boundary_source.save(d / "zlib_boundary_source.png")
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
    Image.new("L", (16, 1), 37).save(d / "tiff_lzw_byte_aligned.png")
    Image.new("RGBA", (1, 1), (128, 0, 0, 255)).save(d / "gif_rgba_opaque.png")
    Image.new("RGBA", (1, 1), (128, 0, 0, 0)).save(d / "gif_rgba.png")
    gif_rgba_mixed = Image.new("RGBA", (4, 2))
    gif_rgba_mixed.putdata(
        [
            (9, 8, 7, 0),
            (255, 0, 0, 255),
            (0, 255, 0, 255),
            (255, 0, 0, 255),
            (1, 2, 3, 127),
            (0, 0, 255, 128),
            (0, 255, 0, 255),
            (255, 255, 255, 255),
        ]
    )
    gif_rgba_mixed.save(d / "gif_rgba_mixed.png")
    gif_rgba_high_color = high_color.convert("RGBA")
    gif_rgba_high_color.putpixel((0, 0), (17, 19, 23, 0))
    gif_rgba_high_color.save(d / "gif_rgba_high_color.png")
    octree_colors = []
    for r_bucket in range(8):
        for g_bucket in range(16):
            for b_bucket in range(8):
                for a_bucket in range(8):
                    offset = (((r_bucket * 16 + g_bucket) * 8 + b_bucket) * 8) + a_bucket
                    color = (
                        r_bucket * 32 + 15,
                        g_bucket * 16 + 7,
                        b_bucket * 32 + 15,
                        a_bucket * 32 + 15,
                    )
                    octree_colors.extend([color] * (1 + ((offset * 17 + 5) % 3)))
    gif_rgba_octree = Image.new("RGBA", (len(octree_colors), 1))
    gif_rgba_octree.putdata(octree_colors)
    gif_rgba_octree.save(d / "gif_rgba_octree.png")
    sorted_octree_colors = []
    for coarse_offset in range(256):
        r_bucket = (coarse_offset >> 6) & 3
        g_bucket = (coarse_offset >> 4) & 3
        b_bucket = (coarse_offset >> 2) & 3
        a_bucket = coarse_offset & 3
        color = (
            r_bucket * 64 + 31,
            g_bucket * 64 + 31,
            b_bucket * 64 + 31,
            a_bucket * 64 + 31,
        )
        sorted_octree_colors.extend([color] * (256 - coarse_offset))
    gif_rgba_octree_sorted = Image.new("RGBA", (len(sorted_octree_colors), 1))
    gif_rgba_octree_sorted.putdata(sorted_octree_colors)
    gif_rgba_octree_sorted.save(d / "gif_rgba_octree_sorted.png")
    img.convert("L").save(d / "gray.png")
    img.convert("LA").save(d / "gray_alpha.png")
    # Minimal non-opaque LA input: large gradient fixtures exercise unrelated
    # VP8/VP8L optimizer choices, while this row exists to prove the LA→RGBA
    # transfer contract and alpha-bearing branch exactly.
    gray_alpha_partial = Image.new("LA", (1, 1), (17, 128))
    gray_alpha_partial.save(d / "gray_alpha_partial.png")
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
    l16_clamp = Image.new("I;16", (8, 1))
    l16_clamp.putdata([0, 1, 127, 255, 256, 257, 511, 65535])
    l16_clamp.save(d / "l16_clamp.png")
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

    # Pillow does not expose APNG interlace encoding and Pillow 12.2 cannot
    # load an Adam7 fdAT frame. Assemble a valid one-frame APNG so sequence
    # parity still proves the APNG-controlled IDAT path through Adam7; fdAT
    # extraction is covered independently by the non-interlaced families.
    with tempfile.TemporaryDirectory(prefix="image-star-apng-") as temporary:
        temporary = Path(temporary)
        first_path = temporary / "first.png"
        first = pattern_img("RGB", (9, 7))
        write_rgb_png(first_path, first, interlace=True)
        first_chunks = png_chunks(first_path.read_bytes())
    ihdr = next(payload for kind, payload in first_chunks if kind == b"IHDR")
    first_idat = next(payload for kind, payload in first_chunks if kind == b"IDAT")
    first_control = struct.pack(">IIIIIHHBB", 0, 9, 7, 0, 0, 1, 10, 0, 0)
    (d / "apng_adam7.png").write_bytes(
        rebuild_png(
            [
                (b"IHDR", ihdr),
                (b"acTL", struct.pack(">II", 1, 1)),
                (b"fcTL", first_control),
                (b"IDAT", first_idat),
                (b"IEND", b""),
            ]
        )
    )

    apng_base = Image.new("RGBA", (4, 4), (220, 20, 10, 255))
    apng_over = apng_base.copy()
    apng_over.putpixel((1, 1), (10, 240, 20, 128))
    apng_previous = apng_over.copy()
    for y in range(1, 3):
        for x in range(1, 3):
            apng_previous.putpixel((x, y), (20, 30, 240, 192))
    apng_base.save(
        d / "apng_rgba_controls.png",
        save_all=True,
        append_images=[apng_over, apng_previous],
        duration=[10, 20, 30],
        disposal=[0, 1, 2],
        blend=[0, 1, 0],
        loop=2,
    )

    apng_default = Image.new("RGBA", (4, 4), (12, 34, 56, 255))
    apng_first = Image.new("RGBA", (4, 4), (200, 10, 40, 160))
    apng_second = Image.new("RGBA", (4, 4), (20, 210, 70, 255))
    apng_default.save(
        d / "apng_default_image.png",
        save_all=True,
        append_images=[apng_first, apng_second],
        default_image=True,
        duration=[20, 30],
        disposal=[1, 2],
        blend=[1, 0],
        loop=3,
    )

    apng_l1_base = Image.new("1", (8, 2))
    apng_l1_base.putpixel((0, 0), 1)
    apng_l1_middle = apng_l1_base.copy()
    apng_l1_middle.putpixel((2, 1), 1)
    apng_l1_final = apng_l1_middle.copy()
    apng_l1_final.putpixel((7, 0), 1)
    apng_l1_base.save(
        d / "apng_l1_controls.png",
        save_all=True,
        append_images=[apng_l1_middle, apng_l1_final],
        duration=[10, 20, 30],
        disposal=[0, 0, 0],
        blend=[0, 0, 0],
        loop=1,
    )
    (d / "apng_l1_controls.png").write_bytes(
        mutate_png_chunk(
            (d / "apng_l1_controls.png").read_bytes(),
            b"fcTL",
            1,
            lambda payload: payload.__setitem__(24, 1),
        )
    )

    apng_la_base = Image.new("LA", (4, 4), (100, 255))
    apng_la_over = apng_la_base.copy()
    apng_la_over.putpixel((1, 1), (200, 128))
    apng_la_base.save(
        d / "apng_la_over.png",
        save_all=True,
        append_images=[apng_la_over],
        duration=[10, 20],
        disposal=[0, 1],
        blend=[0, 1],
        loop=1,
    )

    apng_l_base = Image.new("L", (4, 4), 50)
    apng_l_over = apng_l_base.copy()
    apng_l_over.putpixel((1, 1), 200)
    apng_l_base.save(
        d / "apng_l_over.png",
        save_all=True,
        append_images=[apng_l_over],
        duration=[10, 20],
        disposal=[0, 1],
        blend=[0, 1],
        loop=1,
    )

    apng_p_base = Image.new("P", (4, 4), 2)
    apng_palette = [0, 0, 0, 255, 0, 0, 0, 255, 0] + [0, 0, 0] * 253
    apng_p_base.putpalette(apng_palette)
    apng_p_base.info["transparency"] = bytes([0, 128, 255])
    apng_p_over = apng_p_base.copy()
    apng_p_over.putpixel((1, 1), 1)
    apng_p_base.save(
        d / "apng_palette_over.png",
        save_all=True,
        append_images=[apng_p_over],
        duration=[10, 20],
        disposal=[0, 1],
        blend=[0, 1],
        loop=1,
    )

    animated = (d / "apng_animated.png").read_bytes()
    controls = (d / "apng_rgba_controls.png").read_bytes()
    (d / "apng_zero_delay_den.png").write_bytes(
        mutate_png_chunk(
            controls,
            b"fcTL",
            1,
            lambda payload: payload.__setitem__(slice(22, 24), b"\0\0"),
        )
    )
    (d / "apng_bad_first_sequence.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"fcTL",
            0,
            lambda payload: payload.__setitem__(slice(0, 4), struct.pack(">I", 1)),
        )
    )
    (d / "apng_gap_sequence.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"fdAT",
            0,
            lambda payload: payload.__setitem__(
                slice(0, 4), struct.pack(">I", struct.unpack(">I", payload[:4])[0] + 1)
            ),
        )
    )
    (d / "apng_duplicate_sequence.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"fdAT",
            0,
            lambda payload: payload.__setitem__(
                slice(0, 4), struct.pack(">I", struct.unpack(">I", payload[:4])[0] - 1)
            ),
        )
    )
    (d / "apng_frame_outside_canvas.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"fcTL",
            1,
            lambda payload: payload.__setitem__(slice(12, 16), struct.pack(">I", 1)),
        )
    )
    (d / "apng_declared_frame_mismatch.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"acTL",
            0,
            lambda payload: payload.__setitem__(slice(0, 4), struct.pack(">I", 3)),
        )
    )
    (d / "apng_short_fctl.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"fcTL",
            1,
            lambda payload: payload.__delitem__(slice(25, 26)),
        )
    )
    (d / "apng_short_fdat.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"fdAT",
            0,
            lambda payload: payload.__delitem__(slice(3, None)),
        )
    )
    (d / "apng_corrupt_default_data.png").write_bytes(
        mutate_png_chunk(
            (d / "apng_default_image.png").read_bytes(),
            b"IDAT",
            0,
            lambda payload: payload.__setitem__(slice(None), b"\0"),
        )
    )
    (d / "apng_corrupt_frame_data.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"fdAT",
            0,
            lambda payload: payload.__setitem__(slice(4, None), b"\0"),
        )
    )
    (d / "apng_invalid_disposal.png").write_bytes(
        mutate_png_chunk(
            controls,
            b"fcTL",
            1,
            lambda payload: payload.__setitem__(24, 3),
        )
    )
    (d / "apng_invalid_blend.png").write_bytes(
        mutate_png_chunk(
            controls,
            b"fcTL",
            1,
            lambda payload: payload.__setitem__(25, 2),
        )
    )
    (d / "apng_first_previous.png").write_bytes(
        mutate_png_chunk(
            controls,
            b"fcTL",
            0,
            lambda payload: payload.__setitem__(24, 2),
        )
    )
    (d / "apng_large_frame_count.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"acTL",
            0,
            lambda payload: payload.__setitem__(
                slice(0, 4), struct.pack(">I", 0x8000_0001)
            ),
        )
    )
    (d / "apng_long_actl.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"acTL",
            0,
            lambda payload: payload.extend(b"\0"),
        )
    )
    duplicated_actl = png_chunks(animated)
    first_actl = next(
        index for index, (kind, _) in enumerate(duplicated_actl) if kind == b"acTL"
    )
    duplicated_actl.insert(first_actl + 1, duplicated_actl[first_actl])
    (d / "apng_duplicate_actl.png").write_bytes(rebuild_png(duplicated_actl))

    (d / "apng_short_first_fctl.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"fcTL",
            0,
            lambda payload: payload.__delitem__(slice(25, 26)),
        )
    )
    (d / "apng_first_frame_outside_x.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"fcTL",
            0,
            lambda payload: payload.__setitem__(slice(12, 16), struct.pack(">I", 1)),
        )
    )
    (d / "apng_first_frame_outside_y.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"fcTL",
            0,
            lambda payload: payload.__setitem__(slice(16, 20), struct.pack(">I", 1)),
        )
    )
    (d / "apng_zero_frame_width.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"fcTL",
            0,
            lambda payload: payload.__setitem__(slice(4, 8), b"\0\0\0\0"),
        )
    )
    (d / "apng_zero_frame_height.png").write_bytes(
        mutate_png_chunk(
            animated,
            b"fcTL",
            0,
            lambda payload: payload.__setitem__(slice(8, 12), b"\0\0\0\0"),
        )
    )

    missing_between = png_chunks(controls)
    first_fdat = next(
        index for index, (kind, _) in enumerate(missing_between) if kind == b"fdAT"
    )
    missing_between.pop(first_fdat)
    (d / "apng_missing_between_frame_data.png").write_bytes(
        rebuild_png(missing_between)
    )
    missing_final = png_chunks(animated)
    final_fdat = max(
        index for index, (kind, _) in enumerate(missing_final) if kind == b"fdAT"
    )
    missing_final.pop(final_fdat)
    (d / "apng_missing_final_frame_data.png").write_bytes(rebuild_png(missing_final))
    no_control = png_chunks(animated)
    later_control = max(
        index for index, (kind, _) in enumerate(no_control) if kind == b"fcTL"
    )
    no_control.pop(later_control)
    (d / "apng_fdat_without_fctl.png").write_bytes(rebuild_png(no_control))
    (d / "apng_short_fdat_without_fctl.png").write_bytes(
        mutate_png_chunk(
            rebuild_png(no_control),
            b"fdAT",
            0,
            lambda payload: payload.__delitem__(slice(3, None)),
        )
    )

    duplicate_default_control = png_chunks(animated)
    first_control = next(
        index
        for index, (kind, _) in enumerate(duplicate_default_control)
        if kind == b"fcTL"
    )
    second_control = bytearray(duplicate_default_control[first_control][1])
    second_control[:4] = struct.pack(">I", 1)
    duplicate_default_control.insert(
        first_control + 1, (b"fcTL", bytes(second_control))
    )
    duplicate_default_control[first_actl] = (
        b"acTL",
        struct.pack(">II", 2, 0),
    )
    (d / "apng_multiple_default_controls.png").write_bytes(
        rebuild_png(duplicate_default_control)
    )

    static_chunks = png_chunks((d / "rgb.png").read_bytes())
    static_chunks.insert(1, (b"acTL", struct.pack(">II", 1, 0)))
    (d / "apng_no_controlled_frames.png").write_bytes(rebuild_png(static_chunks))
    animated_chunks = png_chunks(animated)
    no_idat = [
        animated_chunks[0],
        next(chunk for chunk in animated_chunks if chunk[0] == b"acTL"),
        next(chunk for chunk in animated_chunks if chunk[0] == b"fcTL"),
        next(chunk for chunk in animated_chunks if chunk[0] == b"IEND"),
    ]
    (d / "apng_no_idat.png").write_bytes(rebuild_png(no_idat))

    moved_chunks = png_chunks(animated)
    actl_index = next(
        index for index, (kind, _) in enumerate(moved_chunks) if kind == b"acTL"
    )
    idat_index = next(
        index for index, (kind, _) in enumerate(moved_chunks) if kind == b"IDAT"
    )
    actl_chunk = moved_chunks.pop(actl_index)
    if actl_index < idat_index:
        idat_index -= 1
    moved_chunks.insert(idat_index + 1, actl_chunk)
    (d / "actl_after_idat.png").write_bytes(rebuild_png(moved_chunks))
    # Error
    d.joinpath("truncated.png").write_bytes(b"\x89PNG\r\n\x1a\n\x00\x00\x00")
    d.joinpath("short_signature.png").write_bytes(b"\x89PNG")
    d.joinpath("short_chunk_kind.png").write_bytes(b"\x89PNG\r\n\x1a\n\x00\x00\x00\x01tE")
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
    write_mutated_ihdr(
        "zero_height.png", lambda payload: payload.__setitem__(slice(4, 8), b"\0\0\0\0")
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
    d.joinpath("ihdr_only.png").write_bytes(
        b"\x89PNG\r\n\x1a\n" + png_chunk(b"IHDR", rgb_header)
    )
    d.joinpath("ihdr_trailing_byte.png").write_bytes(
        b"\x89PNG\r\n\x1a\n" + png_chunk(b"IHDR", rgb_header) + b"\0"
    )
    d.joinpath("truncated_ancillary_chunk.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", rgb_header)
        + struct.pack(">I", 4)
        + b"tEXt"
        + b"x"
    )
    d.joinpath("rgb_trns.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", rgb_header)
        + png_chunk(b"tRNS", b"\0\0\0\0\0\0")
        + png_chunk(b"IDAT", zlib.compress(b"\x00\x80\x00\x00"))
        + png_chunk(b"IEND", b"")
    )
    d.joinpath("actl_short.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", rgb_header)
        + png_chunk(b"acTL", b"\0" * 7)
        + png_chunk(b"IDAT", zlib.compress(b"\x00\x80\x00\x00"))
        + png_chunk(b"IEND", b"")
    )
    d.joinpath("actl_after_idat_short.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", rgb_header)
        + png_chunk(b"IDAT", zlib.compress(b"\x00\x80\x00\x00"))
        + png_chunk(b"acTL", b"\0" * 7)
        + png_chunk(b"IEND", b"")
    )
    d.joinpath("iend_without_idat.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", rgb_header)
        + png_chunk(b"IEND", b"")
    )
    bad_idat_crc = bytearray(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", rgb_header)
        + png_chunk(b"IDAT", zlib.compress(b"\x00\x80\x00\x00"))
        + png_chunk(b"IEND", b"")
    )
    idat_kind = bad_idat_crc.index(b"IDAT")
    idat_length = struct.unpack(">I", bad_idat_crc[idat_kind - 4 : idat_kind])[0]
    bad_idat_crc[idat_kind + 4 + idat_length] ^= 0xFF
    d.joinpath("bad_idat_crc.png").write_bytes(bad_idat_crc)
    d.joinpath("actl_zero_frames.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", rgb_header)
        + png_chunk(b"acTL", b"\0" * 8)
        + png_chunk(b"IDAT", zlib.compress(b"\x00\x80\x00\x00"))
        + png_chunk(b"IEND", b"")
    )
    (d / "idat_truncated_chunk_no_iend.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", rgb_header)
        + png_chunk(b"IDAT", zlib.compress(b"\x00\x80\x00\x00"))
        + struct.pack(">I", 4)
        + b"tEXt"
        + b"x"
    )
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

    def write_raw_zlib_png(name, payload):
        (d / name).write_bytes(
            b"\x89PNG\r\n\x1a\n"
            + png_chunk(b"IHDR", rgb_header)
            + png_chunk(b"IDAT", payload)
            + png_chunk(b"IEND", b"")
        )

    write_raw_zlib_png("zlib_short_header.png", b"\x78\x01\x00\x00\x00")
    write_raw_zlib_png("zlib_invalid_header.png", b"\x00\x00\x00\x00\x00\x00")
    write_raw_zlib_png("zlib_reserved_block.png", b"\x78\x01\x07\x00\x00\x00\x00")
    write_raw_zlib_png(
        "zlib_bad_stored_complement.png",
        b"\x78\x01\x01\x01\x00\x01\x00\x00\x00\x00\x00\x00",
    )
    bad_adler = bytearray(zlib.compress(b"\x00\x80\x00\x00", level=0))
    bad_adler[-1] ^= 0x01
    write_raw_zlib_png("zlib_bad_adler.png", bytes(bad_adler))
    write_raw_zlib_png(
        "zlib_oversized_scanline.png", zlib.compress(b"\x00\x80\x00\x00\x00", level=6)
    )
    stored_scanline = b"\x00\x80\x00\x00\x00"
    write_raw_zlib_png(
        "zlib_oversized_stored_scanline.png",
        b"\x78\x01\x01"
        + struct.pack("<HH", len(stored_scanline), 0xFFFF ^ len(stored_scanline))
        + stored_scanline
        + struct.pack(">I", zlib.adler32(stored_scanline)),
    )
    write_raw_zlib_png(
        "zlib_oversized_backreference_scanline.png",
        malformed_fixed_zlib([0, 0, 0, 0, 257, 256], distances=[0]),
    )
    write_raw_zlib_png("zlib_minimal_dynamic.png", minimal_dynamic_zlib())
    write_raw_zlib_png(
        "zlib_dynamic_backreference_before_output.png",
        invalid_dynamic_backreference_zlib(),
    )
    write_raw_zlib_png(
        "zlib_backreference_before_output.png",
        malformed_fixed_zlib([257, 256], distances=[0]),
    )
    write_raw_zlib_png(
        "zlib_reserved_distance_symbol.png",
        malformed_fixed_zlib([ord("A"), 257, 256], distances=[30]),
    )
    write_raw_zlib_png(
        "zlib_reserved_literal_symbol.png", malformed_fixed_zlib([286])
    )
    # A final fixed block without enough bits to decode its first symbol.
    write_raw_zlib_png("zlib_truncated_fixed_block.png", b"\x78\x01\x03\x00\x00\x00\x01")
    write_raw_zlib_png(
        "zlib_empty_code_length_tree.png", malformed_dynamic_zlib([0, 0, 0, 0])
    )
    write_raw_zlib_png(
        "zlib_oversubscribed_code_length_tree.png",
        malformed_dynamic_zlib([1, 1, 1, 0]),
    )
    write_raw_zlib_png(
        "zlib_dynamic_repeat_overflow.png",
        malformed_dynamic_zlib(
            [0, 0, 1, 0],
            [
                (0, 1),
                (127, 7),  # symbol 18, repeat 138 zeroes
                (0, 1),
                (127, 7),  # another 138 exceeds the 258-symbol limit
            ],
        ),
    )
    write_raw_zlib_png(
        "zlib_undecodable_code_length.png",
        malformed_dynamic_zlib([0, 0, 2, 0], [(3, 2)]),
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
    (d / "palette_missing_plte.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", palette_header)
        + png_chunk(b"IDAT", zlib.compress(b"\0\0"))
        + png_chunk(b"IEND", b"")
    )
    (d / "palette_empty_plte.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", palette_header)
        + png_chunk(b"PLTE", b"")
        + png_chunk(b"IDAT", zlib.compress(b"\0\0"))
        + png_chunk(b"IEND", b"")
    )
    (d / "palette_short_plte.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", palette_header)
        + png_chunk(b"PLTE", b"\0")
        + png_chunk(b"IDAT", zlib.compress(b"\0\0"))
        + png_chunk(b"IEND", b"")
    )
    (d / "palette_partial_plte.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", palette_header)
        + png_chunk(b"PLTE", b"\0\0\0\xff")
        + png_chunk(b"IDAT", zlib.compress(b"\0\0"))
        + png_chunk(b"IEND", b"")
    )
    overlong_palette = bytes(
        value for index in range(257) for value in (index & 0xFF, index & 0xFF, index & 0xFF)
    )
    (d / "palette_overlong_plte.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", palette_header)
        + png_chunk(b"PLTE", overlong_palette)
        + png_chunk(b"IDAT", zlib.compress(b"\0\0"))
        + png_chunk(b"IEND", b"")
    )
    (d / "palette_trns_without_plte.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", palette_header)
        + png_chunk(b"tRNS", b"\xff")
        + png_chunk(b"IDAT", zlib.compress(b"\0\0"))
        + png_chunk(b"IEND", b"")
    )
    (d / "duplicate_plte.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", palette_header)
        + png_chunk(b"PLTE", b"\0\0\0\xff\xff\xff")
        + png_chunk(b"PLTE", b"\0\0\0\xff\xff\xff")
        + png_chunk(b"IDAT", zlib.compress(b"\0\0"))
        + png_chunk(b"IEND", b"")
    )
    (d / "duplicate_trns.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", palette_header)
        + png_chunk(b"PLTE", b"\0\0\0\xff\xff\xff")
        + png_chunk(b"tRNS", b"\0\xff")
        + png_chunk(b"tRNS", b"\0\xff")
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


def pack_gif_lzw_codes(codes, minimum_code_size):
    """Pack GIF LZW codes least-significant bit first with growing widths."""
    clear = 1 << minimum_code_size
    end = clear + 1
    first_free = end + 1
    code_width = minimum_code_size + 1
    next_code = first_free
    previous = None
    bits = []
    for code in codes:
        bits.extend((code >> shift) & 1 for shift in range(code_width))
        if code == clear:
            code_width = minimum_code_size + 1
            next_code = first_free
            previous = None
            continue
        if code == end:
            continue
        if previous is None:
            previous = code
            continue
        if next_code < 4096:
            next_code += 1
            if code_width < 12 and next_code == 1 << code_width:
                code_width += 1
        previous = code
    output = bytearray((len(bits) + 7) // 8)
    for index, bit in enumerate(bits):
        output[index // 8] |= bit << (index % 8)
    return bytes(output)


def write_gif_lzw_fixture(path, width, codes, minimum_code_size=2):
    """Write a one-row four-color GIF around an explicit LZW code stream."""
    palette = bytes((0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 255, 0))
    payload = pack_gif_lzw_codes(codes, minimum_code_size)
    blocks = b"".join(
        bytes((len(payload[offset : offset + 255]),))
        + payload[offset : offset + 255]
        for offset in range(0, len(payload), 255)
    ) + b"\0"
    output = bytearray(b"GIF89a")
    output.extend(struct.pack("<HH", width, 1))
    output.extend((0x81, 0, 0))
    output.extend(palette)
    output.extend(b"\x2c\0\0\0\0")
    output.extend(struct.pack("<HH", width, 1))
    output.append(0)
    output.append(minimum_code_size)
    output.extend(blocks)
    output.append(0x3B)
    path.write_bytes(output)


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
    (d / "truncated_signature.gif").write_bytes(b"GIF8")
    (d / "truncated_logical_screen.gif").write_bytes(b"GIF89a\x01")
    (d / "truncated_after_width.gif").write_bytes(b"GIF89a\x01\x00")
    (d / "truncated_after_height.gif").write_bytes(b"GIF89a\x01\x00\x01\x00")
    (d / "truncated_after_packed.gif").write_bytes(b"GIF89a\x01\x00\x01\x00\x00")
    (d / "truncated_after_background.gif").write_bytes(
        b"GIF89a\x01\x00\x01\x00\x00\x00"
    )
    (d / "declared_global_palette_short.gif").write_bytes(
        b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00"
    )
    (d / "no_frame_trailer.gif").write_bytes(b"GIF89a\x01\x00\x01\x00\x00\x00\x00\x3b")
    (d / "no_frame_no_trailer.gif").write_bytes(b"GIF89a\x01\x00\x01\x00\x00\x00\x00")
    (d / "truncated_global_palette.gif").write_bytes(bytes(static[: table_end - 1]))
    (d / "extension_no_label.gif").write_bytes(bytes(static[:image_offset]) + b"\x21")
    (d / "application_no_length.gif").write_bytes(bytes(static[:image_offset]) + b"\x21\xff")
    (d / "truncated_image_descriptor.gif").write_bytes(bytes(static[: image_offset + 4]))
    (d / "image_no_left.gif").write_bytes(bytes(static[: image_offset + 1]))
    (d / "image_truncated_after_left.gif").write_bytes(bytes(static[: image_offset + 3]))
    (d / "image_truncated_after_top.gif").write_bytes(bytes(static[: image_offset + 5]))
    (d / "image_truncated_after_width.gif").write_bytes(bytes(static[: image_offset + 7]))
    (d / "image_truncated_after_height.gif").write_bytes(bytes(static[: image_offset + 9]))
    (d / "image_truncated_after_packed.gif").write_bytes(bytes(static[: image_offset + 10]))
    truncated_local_palette = bytearray(static)
    truncated_local_palette[image_offset + 9] = 0x80 | (static[10] & 7)
    (d / "truncated_local_palette.gif").write_bytes(
        bytes(truncated_local_palette[: image_offset + 13])
    )
    (d / "truncated_image_data.gif").write_bytes(bytes(static[: image_offset + 11]))
    truncated_sub_block = bytearray(static)
    truncated_sub_block[image_offset + 11 : image_offset + 13] = b"\x04\x01"
    (d / "truncated_sub_block.gif").write_bytes(bytes(truncated_sub_block[: image_offset + 13]))
    invalid_signature = bytearray(static)
    invalid_signature[:6] = b"NOTGIF"
    (d / "invalid_signature.gif").write_bytes(invalid_signature)
    near_miss_version = bytearray(static)
    near_miss_version[:6] = b"GIF80a"
    (d / "near_miss_version.gif").write_bytes(near_miss_version)
    unknown_block = bytearray(static)
    unknown_block[image_offset] = 0
    (d / "unknown_block.gif").write_bytes(unknown_block)
    zero_frame_width = bytearray(static)
    zero_frame_width[image_offset + 5 : image_offset + 7] = b"\0\0"
    (d / "zero_frame_width.gif").write_bytes(zero_frame_width)
    zero_frame_height = bytearray(static)
    zero_frame_height[image_offset + 7 : image_offset + 9] = b"\0\0"
    (d / "zero_frame_height.gif").write_bytes(zero_frame_height)
    min_code_one = bytearray(static)
    min_code_one[image_offset + 10] = 1
    (d / "min_code_one.gif").write_bytes(min_code_one)
    min_code_nine = bytearray(static)
    min_code_nine[image_offset + 10] = 9
    (d / "min_code_nine.gif").write_bytes(min_code_nine)

    zero_logical_size = bytearray(static)
    zero_logical_size[6:10] = b"\0\0\0\0"
    (d / "zero_logical_size.gif").write_bytes(zero_logical_size)
    frame_outside_logical = bytearray(static)
    frame_outside_logical[6:10] = b"\x01\x00\x01\x00"
    (d / "frame_outside_logical.gif").write_bytes(frame_outside_logical)

    comment_extension = bytearray(static)
    comment_extension[image_offset:image_offset] = b"\x21\xfe\x03abc\x00"
    (d / "comment_extension.gif").write_bytes(comment_extension)

    unknown_application = bytearray(static)
    unknown_application[image_offset:image_offset] = (
        b"\x21\xff\x0bUNKNOWNAPP1\x03\x01\x02\x03\x00"
    )
    (d / "unknown_application.gif").write_bytes(unknown_application)

    no_palette = bytearray(static)
    no_palette[10] &= 0x7F
    del no_palette[13:table_end]
    (d / "no_palette.gif").write_bytes(no_palette)

    luminance_first = Image.new("L", (8, 8), 10)
    luminance_second = Image.new("L", (8, 8), 200)
    luminance_first.save(
        d / "animated_no_palette.gif",
        save_all=True,
        append_images=[luminance_second],
        duration=100,
        loop=0,
        optimize=False,
    )
    animated_no_palette = bytearray((d / "animated_no_palette.gif").read_bytes())
    animated_table_end = 13 + 3 * (1 << ((animated_no_palette[10] & 7) + 1))
    animated_no_palette[10] &= 0x7F
    del animated_no_palette[13:animated_table_end]
    (d / "animated_no_palette.gif").write_bytes(animated_no_palette)

    local_only = bytearray(static)
    palette = bytes(local_only[13:table_end])
    local_only[10] &= 0x7F
    del local_only[13:table_end]
    local_image_offset = local_only.index(0x2C, 13)
    local_only[local_image_offset + 9] = 0x80 | (static[10] & 7)
    local_only[local_image_offset + 10 : local_image_offset + 10] = palette
    (d / "local_palette_only.gif").write_bytes(local_only)

    animext = bytearray((d / "animated.gif").read_bytes())
    netscape = animext.index(b"NETSCAPE2.0")
    animext[netscape : netscape + 11] = b"ANIMEXTS1.0"
    (d / "animext_loop.gif").write_bytes(animext)
    animext_bad_payload = bytearray(animext)
    animext_payload = animext_bad_payload.index(b"\x03\x01", netscape)
    animext_bad_payload[animext_payload + 1] = 0
    (d / "animext_bad_payload.gif").write_bytes(animext_bad_payload)
    short_loop_payload = bytearray(static)
    short_loop_payload[image_offset:image_offset] = b"\x21\xff\x0bNETSCAPE2.0\x01\x01\x00"
    (d / "short_loop_payload.gif").write_bytes(short_loop_payload)
    bad_loop_payload = bytearray(static)
    bad_loop_payload[image_offset:image_offset] = (
        b"\x21\xff\x0bNETSCAPE2.0\x03\x02\x01\x00\x00"
    )
    (d / "bad_loop_payload.gif").write_bytes(bad_loop_payload)
    (d / "truncated_application_identifier.gif").write_bytes(
        bytes(static[:image_offset]) + b"\x21\xff\x0bNETS"
    )
    (d / "truncated_application_subblock.gif").write_bytes(
        bytes(static[:image_offset]) + b"\x21\xff\x0bUNKNOWNAPP1\x04\x01"
    )
    (d / "truncated_comment_subblock.gif").write_bytes(
        bytes(static[:image_offset]) + b"\x21\xfe\x04ab"
    )
    (d / "comment_no_subblock_length.gif").write_bytes(bytes(static[:image_offset]) + b"\x21\xfe")

    gce = bytearray((d / "gce.gif").read_bytes())
    gce_offset = gce.index(b"\x21\xf9")
    bad_gce_terminator = bytearray(gce)
    bad_gce_terminator[gce_offset + 7] = 1
    (d / "bad_gce_terminator.gif").write_bytes(bad_gce_terminator)
    (d / "gce_recovery_payload_truncated.gif").write_bytes(
        bytes(gce[: gce_offset + 7]) + b"\x04\x01\x02"
    )
    (d / "gce_recovery_subblock_truncated.gif").write_bytes(
        bytes(gce[: gce_offset + 7]) + b"\x01\xaa\x04\x01\x02"
    )
    (d / "truncated_gce.gif").write_bytes(bytes(gce[: gce_offset + 5]))
    (d / "gce_no_size.gif").write_bytes(bytes(static[:image_offset]) + b"\x21\xf9")
    (d / "gce_truncated_after_size.gif").write_bytes(
        bytes(static[:image_offset]) + b"\x21\xf9\x04"
    )
    (d / "gce_truncated_after_packed.gif").write_bytes(
        bytes(static[:image_offset]) + b"\x21\xf9\x04\x00"
    )
    (d / "gce_truncated_after_delay.gif").write_bytes(
        bytes(static[:image_offset]) + b"\x21\xf9\x04\x00\x00\x00"
    )
    (d / "gce_truncated_after_index.gif").write_bytes(
        bytes(static[:image_offset]) + b"\x21\xf9\x04\x00\x00\x00\x00"
    )
    nonstandard_gce_size = bytearray(gce)
    nonstandard_gce_size[gce_offset + 2] = 3
    (d / "nonstandard_gce_size.gif").write_bytes(nonstandard_gce_size)
    disposal_keep = bytearray(gce)
    disposal_keep[gce_offset + 3] = (disposal_keep[gce_offset + 3] & 0xE3) | (1 << 2)
    (d / "disposal_keep.gif").write_bytes(disposal_keep)
    disposal_reserved = bytearray(gce)
    disposal_reserved[gce_offset + 3] = (disposal_reserved[gce_offset + 3] & 0xE3) | (4 << 2)
    (d / "disposal_reserved.gif").write_bytes(disposal_reserved)

    out_of_range_transparency = bytearray((d / "local_ct.gif").read_bytes())
    local_table_end = 13 + 3 * (1 << ((out_of_range_transparency[10] & 7) + 1))
    local_image_offset = out_of_range_transparency.index(0x2C, local_table_end)
    out_of_range_transparency[local_image_offset:local_image_offset] = (
        b"\x21\xf9\x04\x01\x00\x00\xff\x00"
    )
    (d / "out_of_range_transparency.gif").write_bytes(out_of_range_transparency)

    clear, end = 4, 5
    write_gif_lzw_fixture(d / "lzw_kwkwk.gif", 3, [clear, 0, 6, end])
    write_gif_lzw_fixture(d / "lzw_kwkwk_clipped.gif", 2, [clear, 0, 6, end])
    write_gif_lzw_fixture(d / "lzw_no_eoi.gif", 1, [clear, 0])
    write_gif_lzw_fixture(d / "lzw_invalid_first.gif", 1, [6])
    write_gif_lzw_fixture(d / "lzw_end_only.gif", 1, [clear, end])
    write_gif_lzw_fixture(d / "lzw_invalid_future.gif", 2, [clear, 0, 7])
    write_gif_lzw_fixture(d / "lzw_truncated_output.gif", 2, [clear, 0])
    palette_payload = pack_gif_lzw_codes([clear, 2, end], 2)
    palette_blocks = bytes([len(palette_payload)]) + palette_payload + b"\0"
    palette_index_out_of_range = bytearray(b"GIF89a")
    palette_index_out_of_range.extend(struct.pack("<HH", 1, 1))
    palette_index_out_of_range.extend((0x80, 0, 0))
    palette_index_out_of_range.extend(b"\x00\x00\x00\xff\xff\xff")
    palette_index_out_of_range.extend(b"\x2c\0\0\0\0")
    palette_index_out_of_range.extend(struct.pack("<HH", 1, 1))
    palette_index_out_of_range.append(0)
    palette_index_out_of_range.append(2)
    palette_index_out_of_range.extend(palette_blocks)
    palette_index_out_of_range.append(0x3B)
    (d / "palette_index_out_of_range.gif").write_bytes(palette_index_out_of_range)
    literal_count = 4100
    write_gif_lzw_fixture(
        d / "lzw_dictionary_saturation.gif",
        literal_count,
        [256] + [0] * literal_count + [257],
        minimum_code_size=8,
    )
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


def bmp_file_header(file_size=14, pixel_offset=54):
    return b"BM" + struct.pack("<IHHI", file_size, 0, 0, pixel_offset)


def write_bmp_prefix(path, dib_prefix, pixel_offset=54):
    path.write_bytes(bmp_file_header(14 + len(dib_prefix), pixel_offset) + dib_prefix)


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


def write_bmp_4(path, width=16, height=16, grayscale_palette=False):
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
    palette = (
        bytes(value for index in range(16) for value in (index, index, index, 0))
        if grayscale_palette
        else bmp_palette(16)
    )
    write_bmp(path, dib, bytes(rows), palette)


def write_bmp_2(path, grayscale_palette=False, width=9, height=5):
    stride = ((width * 2 + 31) // 32) * 4
    row_bytes = (width + 3) // 4
    rows = bytearray()
    for y in range(height - 1, -1, -1):
        row = bytearray(row_bytes)
        for x in range(width):
            row[x // 4] |= ((x + y) & 0x03) << (6 - 2 * (x % 4))
        rows.extend(row)
        rows.extend(b"\0" * (stride - row_bytes))

    if grayscale_palette:
        palette = bytes(value for index in range(4) for value in (index, index, index, 0))
    else:
        palette = bmp_palette(4)
    dib = bmp_info_header(width, height, 2, 0, len(rows), 4)
    write_bmp(path, dib, bytes(rows), palette)


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


def write_bmp_bitfields_v2_32(path):
    dib = bytearray(52)
    pixels = bytes((0x30, 0x20, 0x10, 0xff))
    struct.pack_into(
        "<IiiHHIIiiII", dib, 0, 52, 1, 1, 1, 32, 3, len(pixels), 3_780, 3_780, 0, 0
    )
    struct.pack_into("<III", dib, 40, 0x00FF0000, 0x0000FF00, 0x000000FF)
    write_bmp(path, bytes(dib), pixels)


def write_bmp_v4_16(path):
    """Write a V4 RGB555 bitmap whose optional alpha mask is ignored by Pillow."""
    dib = bytearray(108)
    pixels = struct.pack("<H", 0xFFFF) + b"\0\0"
    struct.pack_into(
        "<IiiHHIIiiII", dib, 0, 108, 1, 1, 1, 16, 3, len(pixels), 3_780, 3_780, 0, 0
    )
    struct.pack_into("<IIII", dib, 40, 0x7C00, 0x03E0, 0x001F, 0x8000)
    struct.pack_into("<I", dib, 56, 0x73524742)
    write_bmp(path, bytes(dib), pixels)


def gen_bmp():
    d = OUT / "bmp"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    img.save(d / "24bit.bmp")
    img.convert("RGBA").save(d / "32bit.bmp")
    img.convert("1").save(d / "1bit.bmp")
    write_bmp_2(d / "2bit.bmp")
    write_bmp_2(d / "2bit_gray.bmp", grayscale_palette=True)
    write_bmp_4(d / "4bit.bmp")
    write_bmp_4(d / "4bit_gray.bmp", grayscale_palette=True)
    img.convert("P").save(d / "8bit.bmp")
    implicit_palette = bytearray((d / "8bit.bmp").read_bytes())
    struct.pack_into("<I", implicit_palette, 46, 0)
    (d / "8bit_implicit_palette.bmp").write_bytes(implicit_palette)
    write_bmp_16(d / "16bit.bmp", img)
    img.convert("L").save(d / "gray.bmp")
    img.save(d / "uncompressed.bmp")
    img.save(d / "bottom_up.bmp")
    write_bmp_24(d / "top_down.bmp", img, top_down=True)
    for depth in (1, 4, 8, 16, 32):
        write_bmp_top_down(d / f"top_down_{depth}.bmp", depth)
    top_down_1 = (d / "top_down_1.bmp").read_bytes()
    canonical_top_down_1 = bytearray(top_down_1)
    canonical_top_down_1[58:62] = b"\xff\xff\xff\0"
    (d / "top_down_1_canonical.bmp").write_bytes(canonical_top_down_1)
    bottom_up_1_palette = bytearray(top_down_1)
    struct.pack_into("<i", bottom_up_1_palette, 22, 5)
    (d / "bottom_up_1_palette.bmp").write_bytes(bottom_up_1_palette)
    write_bmp_bitfields(d / "bitfields.bmp", pattern_img("RGBA"))
    zero_mask = bytearray((d / "bitfields.bmp").read_bytes())
    struct.pack_into("<I", zero_mask, 58, 0)
    (d / "bitfields_zero_mask.bmp").write_bytes(zero_mask)
    zero_red_mask = bytearray((d / "bitfields.bmp").read_bytes())
    struct.pack_into("<I", zero_red_mask, 54, 0)
    (d / "bitfields_zero_red_mask.bmp").write_bytes(zero_red_mask)
    zero_blue_mask = bytearray((d / "bitfields.bmp").read_bytes())
    struct.pack_into("<I", zero_blue_mask, 62, 0)
    (d / "bitfields_zero_blue_mask.bmp").write_bytes(zero_blue_mask)
    write_bmp_bitfields_v2_32(d / "bitfields_v2_32_no_alpha.bmp")
    write_bmp_bitfields(d / "v4header.bmp", pattern_img("RGBA"), header_size=108)
    write_bmp_bitfields(d / "v5header.bmp", pattern_img("RGBA"), header_size=124)
    write_bmp_v4_16(d / "v4header16.bmp")
    write_bmp_24(d / "os2v1.bmp", img, core_header=True)
    write_bmp_rle(d / "rle8.bmp", 8)
    write_bmp_rle(d / "rle4.bmp", 4)
    write_bmp_rle_mixed(d / "rle8_mixed.bmp", 8)
    write_bmp_rle_mixed(d / "rle4_mixed.bmp", 4)
    top_down_rle = bytearray((d / "rle8.bmp").read_bytes())
    struct.pack_into("<i", top_down_rle, 22, -16)
    (d / "rle8_top_down.bmp").write_bytes(top_down_rle)
    invalid_rle8_depth = bytearray((d / "rle8.bmp").read_bytes())
    struct.pack_into("<H", invalid_rle8_depth, 28, 24)
    (d / "rle8_invalid_depth.bmp").write_bytes(invalid_rle8_depth)
    invalid_rle4_depth = bytearray((d / "rle4.bmp").read_bytes())
    struct.pack_into("<H", invalid_rle4_depth, 28, 24)
    (d / "rle4_invalid_depth.bmp").write_bytes(invalid_rle4_depth)
    early_eob = bytes((4, 7, 0, 1))
    early_eob_dib = bmp_info_header(4, 2, 8, 1, len(early_eob), 256)
    write_bmp(d / "rle8_early_eob.bmp", early_eob_dib, early_eob, bmp_palette(256))
    early_eob4 = bytes((4, 0x77, 0, 1))
    early_eob4_dib = bmp_info_header(4, 2, 4, 2, len(early_eob4), 16)
    write_bmp(d / "rle4_early_eob.bmp", early_eob4_dib, early_eob4, bmp_palette(16))
    rle8_delta = bytes((0, 2, 1, 1, 4, 7, 4, 8))
    rle8_delta_dib = bmp_info_header(4, 2, 8, 1, len(rle8_delta), 256)
    write_bmp(d / "rle8_delta.bmp", rle8_delta_dib, rle8_delta, bmp_palette(256))
    rle8_absolute_odd = bytes((0, 3, 1, 2, 3, 0, 0, 0, 4, 9))
    rle8_absolute_odd_dib = bmp_info_header(4, 2, 8, 1, len(rle8_absolute_odd), 256)
    write_bmp(
        d / "rle8_absolute_odd.bmp",
        rle8_absolute_odd_dib,
        rle8_absolute_odd,
        bmp_palette(256),
    )
    rle8_delta_truncated = bytes((0, 2, 1))
    rle8_delta_truncated_dib = bmp_info_header(4, 2, 8, 1, len(rle8_delta_truncated), 256)
    write_bmp(
        d / "rle8_delta_truncated.bmp",
        rle8_delta_truncated_dib,
        rle8_delta_truncated,
        bmp_palette(256),
    )
    rle8_absolute_truncated = bytes((0, 3, 1, 2))
    rle8_absolute_truncated_dib = bmp_info_header(
        4, 2, 8, 1, len(rle8_absolute_truncated), 256
    )
    write_bmp(
        d / "rle8_absolute_truncated.bmp",
        rle8_absolute_truncated_dib,
        rle8_absolute_truncated,
        bmp_palette(256),
    )
    rle4_delta_truncated = bytes((0, 2, 1))
    rle4_delta_truncated_dib = bmp_info_header(4, 2, 4, 2, len(rle4_delta_truncated), 16)
    write_bmp(
        d / "rle4_delta_truncated.bmp",
        rle4_delta_truncated_dib,
        rle4_delta_truncated,
        bmp_palette(16),
    )
    rle4_absolute_truncated = bytes((0, 5, 0x12))
    rle4_absolute_truncated_dib = bmp_info_header(
        4, 2, 4, 2, len(rle4_absolute_truncated), 16
    )
    write_bmp(
        d / "rle4_absolute_truncated.bmp",
        rle4_absolute_truncated_dib,
        rle4_absolute_truncated,
        bmp_palette(16),
    )
    Image.new("RGB", (1,1), (128,0,0)).save(d / "1x1.bmp")
    Image.new("RGB", (17,17), (128,0,0)).save(d / "odd_width.bmp")
    pattern_img("RGB", (2, 5)).save(d / "width2.bmp")
    pattern_img("RGB", (3, 5)).save(d / "width3.bmp")
    pattern_img("RGB", (31, 7)).save(d / "width31.bmp")
    d.joinpath("not_bmp.bmp").write_bytes(b"NOTABMP")
    signature_source = (d / "24bit.bmp").read_bytes()
    for signature in (b"BA", b"CI", b"CP", b"IC", b"PT"):
        related_bitmap = bytearray(signature_source)
        related_bitmap[:2] = signature
        (d / f"related_{signature.decode('ascii').lower()}.bmp").write_bytes(
            related_bitmap
        )
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
    struct.pack_into("<i", malformed, 18, 16_385)
    (d / "oversized_width.bmp").write_bytes(malformed)
    malformed = bytearray(baseline)
    struct.pack_into("<i", malformed, 22, 16_385)
    (d / "oversized_height.bmp").write_bytes(malformed)
    malformed = bytearray(baseline)
    struct.pack_into("<H", malformed, 28, 3)
    (d / "invalid_depth.bmp").write_bytes(malformed)
    for channel_name, channel in (("blue", 0), ("green", 1), ("red", 2)):
        palette = bytearray()
        for index in range(256):
            entry = [index, index, index, 0]
            if index == 1:
                entry[channel] = 2
            palette.extend(entry)
        row = bytes((1, 0, 0, 0))
        dib = bmp_info_header(2, 1, 8, 0, len(row), 256)
        write_bmp(d / f"palette_{channel_name}_mismatch.bmp", dib, row, bytes(palette))
    (d / "truncated_magic.bmp").write_bytes(b"B")
    (d / "truncated_file_size.bmp").write_bytes(b"BM\0")
    (d / "truncated_data_offset.bmp").write_bytes(
        b"BM" + struct.pack("<IHH", 0, 0, 0) + b"\0"
    )
    (d / "truncated_dib_header_size.bmp").write_bytes(bmp_file_header())
    write_bmp(
        d / "core_zero_width.bmp",
        struct.pack("<IHHHH", 12, 0, 1, 1, 24),
        b"\0" * 4,
    )
    offset_before_header = bytearray(baseline)
    struct.pack_into("<I", offset_before_header, 10, 53)
    (d / "data_offset_before_header.bmp").write_bytes(offset_before_header)

    core_prefix = struct.pack("<I", 12)
    write_bmp_prefix(d / "core_header_truncated_width.bmp", core_prefix, 26)
    write_bmp_prefix(
        d / "core_header_truncated_height.bmp",
        core_prefix + struct.pack("<H", 1),
        26,
    )
    write_bmp_prefix(
        d / "core_header_truncated_planes.bmp",
        core_prefix + struct.pack("<HH", 1, 1),
        26,
    )
    write_bmp_prefix(
        d / "core_header_truncated_depth.bmp",
        core_prefix + struct.pack("<HHH", 1, 1, 1),
        26,
    )

    info_prefix = struct.pack("<I", 40)
    info_fields = [
        ("height", struct.pack("<i", 1)),
        ("planes", struct.pack("<H", 1)),
        ("depth", struct.pack("<H", 24)),
        ("compression", struct.pack("<I", 0)),
        ("image_size", struct.pack("<I", 0)),
        ("x_pels", struct.pack("<i", 3_780)),
        ("y_pels", struct.pack("<i", 3_780)),
        ("colors_used", struct.pack("<I", 0)),
        ("colors_important", struct.pack("<I", 0)),
    ]
    payload = info_prefix + struct.pack("<i", 1)
    for field_name, encoded in info_fields:
        write_bmp_prefix(d / f"info_header_truncated_{field_name}.bmp", payload)
        payload += encoded

    write_bmp(d / "bitfields_truncated_masks.bmp", bmp_info_header(1, 1, 16, 3, 0), b"")
    write_bmp(
        d / "bitfields_truncated_green_mask.bmp",
        bmp_info_header(1, 1, 16, 3, 0),
        b"",
        masks=struct.pack("<I", 0x7C00),
    )
    write_bmp(
        d / "bitfields_truncated_blue_mask.bmp",
        bmp_info_header(1, 1, 16, 3, 0),
        b"",
        masks=struct.pack("<II", 0x7C00, 0x03E0),
    )
    v4_truncated = bytearray(bmp_info_header(1, 1, 32, 3, 0))
    struct.pack_into("<I", v4_truncated, 0, 108)
    write_bmp(d / "v4_bitfields_truncated_masks.bmp", bytes(v4_truncated), b"")
    write_bmp(
        d / "v4_bitfields_truncated_green_mask.bmp",
        bytes(v4_truncated),
        b"",
        masks=struct.pack("<I", 0x00FF_0000),
    )
    write_bmp(
        d / "v4_bitfields_truncated_blue_mask.bmp",
        bytes(v4_truncated),
        b"",
        masks=struct.pack("<II", 0x00FF_0000, 0x0000_FF00),
    )
    write_bmp(
        d / "v4_bitfields_truncated_alpha_mask.bmp",
        bytes(v4_truncated),
        b"",
        masks=struct.pack("<III", 0x00FF_0000, 0x0000_FF00, 0x0000_00FF),
    )
    write_bmp(
        d / "oversized_palette.bmp",
        bmp_info_header(1, 1, 8, 0, 4, 257),
        b"\0\0\0\0",
        bmp_palette(257),
    )
    write_bmp(d / "rle8_empty_stream.bmp", bmp_info_header(4, 2, 8, 1, 0, 256), b"", bmp_palette(256))
    write_bmp(
        d / "rle8_short_pair.bmp",
        bmp_info_header(4, 2, 8, 1, 1, 256),
        bytes((4,)),
        bmp_palette(256),
    )
    write_bmp(
        d / "rle8_delta_missing_payload.bmp",
        bmp_info_header(4, 2, 8, 1, 2, 256),
        bytes((0, 2)),
        bmp_palette(256),
    )
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
    Image.new("RGB", (17, 19), (83, 121, 177)).save(
        d / "lossy_solid_17x19_q90_m0.webp",
        lossless=False,
        quality=90,
        method=0,
    )
    checker = Image.new("RGB", (17, 19))
    checker_pixels = checker.load()
    for y in range(checker.height):
        for x in range(checker.width):
            checker_pixels[x, y] = (
                (255, 255, 255) if ((x // 4) + (y // 4)) % 2 else (0, 0, 0)
            )
    checker.save(
        d / "lossy_checker_17x19_q1_m0.webp",
        lossless=False,
        quality=1,
        method=0,
    )
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
    sequence_rgba_first = Image.new("RGBA", (9, 7), (17, 34, 51, 128))
    sequence_rgba_second = Image.new("RGBA", (9, 7), (201, 7, 99, 192))
    sequence_rgba_first.save(
        d / "animated_sequence_rgba_keyframes.webp",
        save_all=True,
        append_images=[sequence_rgba_second],
        duration=[17, 33],
        loop=2,
        lossless=True,
        method=4,
        kmax=1,
    )
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
    for name, rgba in (
        ("partial_background", (17, 34, 51, 128)),
        ("opaque_background", (17, 34, 51, 255)),
    ):
        background_variant = bytearray(animated_alpha_holes)
        animation_chunk = background_variant.find(b"ANIM")
        if animation_chunk < 0:
            raise RuntimeError("animated WebP did not contain an ANIM chunk")
        red, green, blue, alpha = rgba
        background_variant[animation_chunk + 8 : animation_chunk + 12] = bytes(
            (blue, green, red, alpha)
        )
        (d / f"animated_alpha_holes_{name}.webp").write_bytes(background_variant)

    animated_rgb_background = bytearray((d / "animated.webp").read_bytes())
    animation_chunk = animated_rgb_background.find(b"ANIM")
    if animation_chunk < 0:
        raise RuntimeError("animated RGB WebP did not contain an ANIM chunk")
    animated_rgb_background[animation_chunk + 8 : animation_chunk + 12] = bytes(
        (51, 34, 17, 255)
    )
    (d / "animated_rgb_opaque_background.webp").write_bytes(animated_rgb_background)

    animated_rgb_palette = Image.new("RGB", (64, 64), (255, 0, 0))
    animated_rgb_palette_next = animated_rgb_palette.copy()
    ImageDraw.Draw(animated_rgb_palette_next).rectangle(
        [24, 24, 39, 39], fill=(0, 128, 0)
    )
    animated_rgb_palette.save(
        d / "animated_rgb_palette_background.webp",
        save_all=True,
        append_images=[animated_rgb_palette_next],
        duration=100,
        loop=0,
        lossless=True,
        minimize_size=False,
    )
    animated_rgb_palette_background = bytearray(
        (d / "animated_rgb_palette_background.webp").read_bytes()
    )
    animation_chunk = animated_rgb_palette_background.find(b"ANIM")
    if animation_chunk < 0:
        raise RuntimeError("animated palette WebP did not contain an ANIM chunk")
    animated_rgb_palette_background[animation_chunk + 8 : animation_chunk + 12] = bytes(
        (0, 0, 255, 255)
    )
    (d / "animated_rgb_palette_background.webp").write_bytes(
        animated_rgb_palette_background
    )

    animated_rgb_full_delta = Image.new("RGB", (64, 64), (255, 0, 0))
    animated_rgb_full_delta_next = animated_rgb_full_delta.copy()
    full_delta_draw = ImageDraw.Draw(animated_rgb_full_delta_next)
    full_delta_draw.point((0, 0), fill=(0, 128, 0))
    full_delta_draw.point((63, 63), fill=(0, 128, 0))
    animated_rgb_full_delta.save(
        d / "animated_rgb_full_delta.webp",
        save_all=True,
        append_images=[animated_rgb_full_delta_next],
        duration=100,
        loop=0,
        lossless=True,
        minimize_size=False,
    )
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
    d.joinpath("short_riff.webp").write_bytes(b"RIFF")
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

    def write_truncated_vp8(name, keep):
        source = (d / "lossy.webp").read_bytes()
        chunk = source.find(b"VP8 ")
        length = struct.unpack_from("<I", source, chunk + 4)[0]
        payload = source[chunk + 8 : chunk + 8 + length]
        kept = len(payload) - 1 if keep == "all_but_one" else min(keep, len(payload))
        malformed = bytearray(source[: chunk + 4])
        malformed.extend(struct.pack("<I", kept))
        malformed.extend(payload[:kept])
        if kept & 1:
            malformed.append(0)
        struct.pack_into("<I", malformed, 4, len(malformed) - 8)
        (d / name).write_bytes(malformed)

    for name, keep in {
        "vp8_empty_payload.webp": 0,
        "vp8_three_byte_payload.webp": 3,
        "vp8_short_header_payload.webp": 10,
        "vp8_short_partition_payload.webp": 50,
        "vp8_half_payload.webp": len((d / "lossy.webp").read_bytes()) // 2,
        "vp8_tail_truncated.webp": 20,
        "vp8_missing_last_byte.webp": "all_but_one",
    }.items():
        write_truncated_vp8(name, keep)

    def write_truncated_vp8l(name, keep, source_name="lossless.webp"):
        source = (d / source_name).read_bytes()
        chunk = source.find(b"VP8L")
        length = struct.unpack_from("<I", source, chunk + 4)[0]
        payload = source[chunk + 8 : chunk + 8 + length]
        kept = len(payload) - 1 if keep == "all_but_one" else min(keep, len(payload))
        malformed = bytearray(source[: chunk + 4])
        malformed.extend(struct.pack("<I", kept))
        malformed.extend(payload[:kept])
        if kept & 1:
            malformed.append(0)
        struct.pack_into("<I", malformed, 4, len(malformed) - 8)
        (d / name).write_bytes(malformed)

    for name, keep, source_name in (
        ("vp8l_header_only.webp", 5, "lossless.webp"),
        ("vp8l_alpha_header_only.webp", 5, "with_alpha.webp"),
        ("vp8l_truncated_6.webp", 6, "lossless.webp"),
        ("vp8l_truncated_8.webp", 8, "lossless.webp"),
        ("vp8l_truncated_12.webp", 12, "lossless.webp"),
        ("vp8l_truncated_16.webp", 16, "lossless.webp"),
        ("vp8l_truncated_24.webp", 24, "lossless.webp"),
        ("vp8l_truncated_32.webp", 32, "lossless.webp"),
        ("vp8l_truncated_64.webp", 64, "lossless.webp"),
        ("vp8l_truncated_128.webp", 128, "lossless.webp"),
        ("vp8l_plane_distance_truncated_12.webp", 12, "vp8l_plane_distance_clamp.webp"),
        ("vp8l_meta_cache_truncated_10.webp", 10, "vp8l_meta_cache_fast_fill.webp"),
        ("vp8l_single_cache_truncated_18.webp", 18, "vp8l_single_cache_peek.webp"),
    ):
        write_truncated_vp8l(name, keep, source_name)

    def write_vp8l_bits(name, bits, width=1, height=1):
        encoded = bytearray((len(bits) + 7) // 8)
        for index, bit in enumerate(bits):
            encoded[index // 8] |= bit << (index % 8)
        dimensions = (width - 1) | ((height - 1) << 14)
        payload = b"\x2f" + struct.pack("<I", dimensions) + encoded
        chunk = b"VP8L" + struct.pack("<I", len(payload)) + payload
        if len(payload) & 1:
            chunk += b"\0"
        webp = b"RIFF" + struct.pack("<I", len(chunk) + 4) + b"WEBP" + chunk
        (d / name).write_bytes(webp)

    def append_lsb(bits, value, width):
        bits.extend((value >> offset) & 1 for offset in range(width))

    write_vp8l_bits("vp8l_duplicate_transform.webp", [1, 0, 1, 1, 0, 1])
    write_vp8l_bits("vp8l_invalid_color_cache.webp", [0, 1, 0, 0, 0, 0])

    def simple_tree(bits, symbols):
        bits.extend((1, len(symbols) - 1))
        append_lsb(bits, int(symbols[0] > 1), 1)
        append_lsb(bits, symbols[0], 8 if symbols[0] > 1 else 1)
        if len(symbols) == 2:
            append_lsb(bits, symbols[1], 8)

    for name, distance_symbols in {
        "vp8l_invalid_zero_symbol.webp": (40,),
        "vp8l_unused_invalid_one_symbol.webp": (0, 40),
    }.items():
        bits = [0, 0, 0]
        for _ in range(4):
            simple_tree(bits, (0,))
        simple_tree(bits, distance_symbols)
        write_vp8l_bits(name, bits)

    bits = [0, 0, 0, 0]
    append_lsb(bits, 0, 4)
    for _ in range(4):
        append_lsb(bits, 0, 3)
    write_vp8l_bits("vp8l_empty_code_length_tree.webp", bits)

    bits = [0, 0, 0, 0]
    append_lsb(bits, 1, 4)
    for length in (0, 0, 0, 0, 1):
        append_lsb(bits, length, 3)
    bits.append(1)
    append_lsb(bits, 0, 3)
    append_lsb(bits, 0, 2)
    for _ in range(4):
        simple_tree(bits, (0,))
    write_vp8l_bits("vp8l_incomplete_huffman_tree.webp", bits)

    def code_length_tree_prefix():
        bits = [0, 0, 0, 0]
        append_lsb(bits, 0, 4)
        for length in (1, 1, 0, 0):
            append_lsb(bits, length, 3)
        return bits

    bits = code_length_tree_prefix()
    bits.append(1)
    append_lsb(bits, 7, 3)
    append_lsb(bits, 300, 16)
    write_vp8l_bits("vp8l_invalid_max_symbol.webp", bits)

    write_vp8l_bits("vp8l_color_index_size_truncated.webp", [1, 1, 1])

    bits = [1, 0, 0]
    append_lsb(bits, 0, 3)
    write_vp8l_bits("vp8l_predictor_transform_stream_truncated.webp", bits)

    bits = [1, 1, 0]
    append_lsb(bits, 0, 3)
    write_vp8l_bits("vp8l_color_transform_stream_truncated.webp", bits)

    bits = [0, 0, 1]
    append_lsb(bits, 0, 3)
    write_vp8l_bits("vp8l_meta_huffman_stream_truncated.webp", bits)

    bits = [0, 0, 0, 1, 1, 0, 0]
    write_vp8l_bits("vp8l_two_symbol_truncated_one_symbol.webp", bits)

    bits = [0, 0, 0, 0]
    append_lsb(bits, 4, 4)
    for length in (1, 1, 0, 0, 0, 0, 0, 0):
        append_lsb(bits, length, 3)
    write_vp8l_bits("vp8l_code_lengths_max_symbol_flag_truncated.webp", bits)

    bits = [0, 0, 0, 0]
    append_lsb(bits, 1, 4)
    for length in (1, 1, 0, 0, 0):
        append_lsb(bits, length, 3)
    bits.append(1)
    write_vp8l_bits("vp8l_code_lengths_length_nbits_truncated.webp", bits)

    bits = code_length_tree_prefix()
    bits.append(1)
    append_lsb(bits, 0, 3)
    write_vp8l_bits("vp8l_code_lengths_max_value_truncated.webp", bits)

    bits = [1, 1, 1]
    append_lsb(bits, 0, 8)
    write_vp8l_bits("vp8l_color_index_stream_truncated.webp", bits)

    write_vp8l_bits("vp8l_zero_symbol_truncated.webp", [0, 0, 0, 1, 0, 1])

    bits = [0, 0, 0, 0]
    append_lsb(bits, 2, 4)
    for length in (1, 1, 0, 0, 0):
        append_lsb(bits, length, 3)
    write_vp8l_bits("vp8l_code_length_alphabet_truncated.webp", bits)

    bits = code_length_tree_prefix()
    bits.extend((0, 0))
    write_vp8l_bits("vp8l_repeat_code17_extra_truncated.webp", bits)

    bits = code_length_tree_prefix()
    bits.extend((0, 1))
    write_vp8l_bits("vp8l_repeat_code18_extra_truncated.webp", bits)

    bits = code_length_tree_prefix()
    bits.append(0)
    for repeat_extra in (127, 127, 0):
        bits.append(1)
        append_lsb(bits, repeat_extra, 7)
    write_vp8l_bits("vp8l_repeat_overflow.webp", bits)

    bits = [0, 0, 0, 0]
    append_lsb(bits, 0, 4)
    for length in (0, 1, 0, 1):
        append_lsb(bits, length, 3)
    bits.append(0)
    for repeat_extra in (127, 107):
        bits.append(1)
        append_lsb(bits, repeat_extra, 7)
    bits.append(0)
    bits.append(1)
    append_lsb(bits, 12, 7)
    for _ in range(4):
        simple_tree(bits, (0,))
    write_vp8l_bits("vp8l_single_backref.webp", bits)

    bits = [0, 0, 0, 0]
    append_lsb(bits, 0, 4)
    for length in (0, 2, 1, 2):
        append_lsb(bits, length, 3)
    bits.append(1)
    append_lsb(bits, 0, 3)
    append_lsb(bits, 2, 2)
    bits.extend((1, 0))
    for repeat_extra in (127, 106):
        bits.extend((1, 1))
        append_lsb(bits, repeat_extra, 7)
    bits.extend((1, 0))
    for _ in range(4):
        simple_tree(bits, (0,))
    bits.append(1)
    write_vp8l_bits("vp8l_backref_before_output.webp", bits)

    bits = [0, 0, 0, 0]
    append_lsb(bits, 0, 4)
    for length in (0, 2, 1, 2):
        append_lsb(bits, length, 3)
    bits.append(1)
    append_lsb(bits, 0, 3)
    append_lsb(bits, 2, 2)
    bits.extend((1, 0))
    for repeat_extra in (127, 106):
        bits.extend((1, 1))
        append_lsb(bits, repeat_extra, 7)
    bits.extend((1, 0))
    for _ in range(3):
        simple_tree(bits, (0,))
    simple_tree(bits, (6,))
    bits.extend((0, 1))
    append_lsb(bits, 1, 2)
    write_vp8l_bits("vp8l_plane_distance_clamp.webp", bits, width=2)

    bits = [0]
    bits.append(1)
    append_lsb(bits, 1, 4)
    bits.append(1)
    append_lsb(bits, 0, 3)
    bits.append(0)
    for _ in range(5):
        simple_tree(bits, (0,))
    simple_tree(bits, (7,))
    simple_tree(bits, (1,))
    simple_tree(bits, (2,))
    simple_tree(bits, (255,))
    simple_tree(bits, (0,))
    write_vp8l_bits("vp8l_meta_cache_fast_fill.webp", bits, width=5)

    bits = [0]
    bits.append(1)
    append_lsb(bits, 1, 4)
    bits.append(1)
    append_lsb(bits, 0, 3)
    bits.append(0)
    simple_tree(bits, (0, 1))
    for _ in range(4):
        simple_tree(bits, (0,))
    bits.extend((0, 1))
    simple_tree(bits, (7,))
    simple_tree(bits, (1,))
    simple_tree(bits, (2,))
    simple_tree(bits, (255,))
    simple_tree(bits, (0,))
    bits.append(0)
    append_lsb(bits, 0, 4)
    for length in (2, 2, 2, 2):
        append_lsb(bits, length, 3)
    bits.append(0)
    for repeat_extra in (127, 127):
        bits.extend((1, 1))
        append_lsb(bits, repeat_extra, 7)
    for _ in range(5):
        bits.extend((0, 0))
    bits.extend((0, 1))
    for _ in range(4):
        simple_tree(bits, (0,))
    write_vp8l_bits("vp8l_single_cache_peek.webp", bits, width=6)

    def write_vp8_partition_size(name, size, source="lossy.webp"):
        malformed = bytearray((d / source).read_bytes())
        payload = malformed.find(b"VP8 ") + 8
        tag = int.from_bytes(malformed[payload : payload + 3], "little")
        malformed[payload : payload + 3] = ((tag & 0x1F) | (size << 5)).to_bytes(3, "little")
        (d / name).write_bytes(malformed)

    for size in range(33):
        write_vp8_partition_size(f"vp8_partition_{size}.webp", size)

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
        "riff_wave.webp",
        "lossy.webp",
        lambda data: data.__setitem__(slice(8, 12), b"WAVE"),
    )
    write_mutated_webp(
        "riff_webp_unknown_chunk.webp",
        "lossy.webp",
        lambda data: data.__setitem__(slice(12, 16), b"JUNK"),
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
        "vp8_zero_height.webp",
        "lossy.webp",
        lambda data: data.__setitem__(slice(data.find(b"VP8 ") + 16, data.find(b"VP8 ") + 18), b"\0\0"),
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

    def remove_top_level_chunk(fourcc):
        def remove(data):
            cursor = 12
            while cursor + 8 <= len(data):
                chunk_size = struct.unpack_from("<I", data, cursor + 4)[0]
                chunk_end = cursor + 8 + chunk_size + (chunk_size & 1)
                if data[cursor : cursor + 4] == fourcc:
                    del data[cursor:chunk_end]
                    struct.pack_into("<I", data, 4, len(data) - 8)
                    return
                cursor = chunk_end
            raise RuntimeError(f"WebP did not contain a {fourcc!r} chunk")

        return remove

    def remove_all_top_level_chunks(fourcc):
        def remove(data):
            cursor = 12
            removed = 0
            while cursor + 8 <= len(data):
                chunk_size = struct.unpack_from("<I", data, cursor + 4)[0]
                chunk_end = cursor + 8 + chunk_size + (chunk_size & 1)
                if data[cursor : cursor + 4] == fourcc:
                    del data[cursor:chunk_end]
                    struct.pack_into("<I", data, 4, len(data) - 8)
                    removed += 1
                    continue
                cursor = chunk_end
            if removed == 0:
                raise RuntimeError(f"WebP did not contain a {fourcc!r} chunk")

        return remove

    write_mutated_webp("extended_missing_exif_chunk.webp", "exif.webp", remove_top_level_chunk(b"EXIF"))
    write_mutated_webp("extended_missing_xmp_chunk.webp", "xmp.webp", remove_top_level_chunk(b"XMP "))
    write_mutated_webp(
        "alpha_missing_chunk.webp",
        "alpha_lossy_horizontal.webp",
        remove_top_level_chunk(b"ALPH"),
    )

    def write_vp8x_container(name, flags=0, trailing=b""):
        vp8x_payload = bytearray([flags, 0, 0, 0])
        vp8x_payload.extend((15).to_bytes(3, "little"))
        vp8x_payload.extend((15).to_bytes(3, "little"))
        vp8x_chunk = bytearray(b"VP8X")
        vp8x_chunk.extend(struct.pack("<I", len(vp8x_payload)))
        vp8x_chunk.extend(vp8x_payload)
        payload = b"WEBP" + bytes(vp8x_chunk) + trailing
        webp = bytearray(b"RIFF")
        webp.extend(struct.pack("<I", len(payload)))
        webp.extend(payload)
        (d / name).write_bytes(webp)

    write_vp8x_container("extended_vp8x_no_chunks.webp")
    write_vp8x_container("extended_vp8x_truncated_chunk_header.webp", trailing=b"JUNK")
    partial_first_chunk_payload = b"WEBPVP8 "
    (d / "partial_first_chunk_header.webp").write_bytes(
        b"RIFF"
        + struct.pack("<I", len(partial_first_chunk_payload))
        + partial_first_chunk_payload
    )
    bomb_vp8_header = (
        b"\0\0\0"
        + b"\x9d\x01\x2a"
        + struct.pack("<HH", 16_383, 16_383)
    )
    bomb_vp8_payload = b"WEBPVP8 " + struct.pack("<I", len(bomb_vp8_header))
    bomb_vp8_payload += bomb_vp8_header
    (d / "vp8_decompression_bomb.webp").write_bytes(
        b"RIFF" + struct.pack("<I", len(bomb_vp8_payload)) + bomb_vp8_payload
    )
    short_vp8x = b"VP8X" + struct.pack("<I", 9) + b"\0" * 9 + b"\0"
    short_vp8x_payload = b"WEBP" + short_vp8x
    (d / "vp8x_short_header.webp").write_bytes(
        b"RIFF" + struct.pack("<I", len(short_vp8x_payload)) + short_vp8x_payload
    )
    short_anmf = b"ANMF" + struct.pack("<I", 16) + b"\0" * 16
    write_vp8x_container(
        "animated_short_anmf_header.webp",
        flags=0x02,
        trailing=short_anmf,
    )
    short_vp8l = b"VP8L" + struct.pack("<I", 1) + b"\x2f\0"
    short_vp8l_payload = b"WEBP" + short_vp8l
    (d / "vp8l_short_header.webp").write_bytes(
        b"RIFF" + struct.pack("<I", len(short_vp8l_payload)) + short_vp8l_payload
    )

    def write_extended_vp8l_alpha_header_only():
        source = (d / "with_alpha.webp").read_bytes()
        vp8l = source.find(b"VP8L")
        if vp8l < 0:
            raise RuntimeError("with_alpha WebP did not contain a VP8L chunk")
        payload = source[vp8l + 8 : vp8l + 13]
        header = int.from_bytes(payload[1:5], "little")
        header |= 1 << 28
        payload = payload[:1] + header.to_bytes(4, "little")
        width = (1 + header) & 0x3FFF
        height = (1 + (header >> 14)) & 0x3FFF

        vp8x_payload = bytearray([0x10, 0, 0, 0])
        vp8x_payload.extend((width - 1).to_bytes(3, "little"))
        vp8x_payload.extend((height - 1).to_bytes(3, "little"))
        vp8x_chunk = b"VP8X" + struct.pack("<I", len(vp8x_payload)) + bytes(vp8x_payload)

        vp8l_chunk = b"VP8L" + struct.pack("<I", len(payload)) + payload
        if len(payload) & 1:
            vp8l_chunk += b"\0"

        payload = b"WEBP" + vp8x_chunk + vp8l_chunk
        webp = b"RIFF" + struct.pack("<I", len(payload)) + payload
        (d / "extended_vp8l_alpha_header_only.webp").write_bytes(webp)

        payload = b"WEBP" + vp8l_chunk
        webp = b"RIFF" + struct.pack("<I", len(payload)) + payload
        (d / "vp8l_alpha_header_only.webp").write_bytes(webp)

    write_extended_vp8l_alpha_header_only()

    write_mutated_webp(
        "animated_missing_anim.webp", "animated.webp", remove_top_level_chunk(b"ANIM")
    )
    write_mutated_webp(
        "animated_missing_anmf.webp", "animated.webp", remove_all_top_level_chunks(b"ANMF")
    )

    def write_extended_vp8_dimension_mismatch():
        source = (d / "lossy.webp").read_bytes()
        vp8 = source.find(b"VP8 ")
        if vp8 < 0:
            raise RuntimeError("lossy WebP did not contain a VP8 chunk")
        vp8_size = struct.unpack_from("<I", source, vp8 + 4)[0]
        vp8_chunk_end = vp8 + 8 + vp8_size + (vp8_size & 1)
        vp8_chunk = source[vp8:vp8_chunk_end]
        vp8x_payload = bytearray((0, 0, 0, 0))
        vp8x_payload.extend((63).to_bytes(3, "little"))
        vp8x_payload.extend((63).to_bytes(3, "little"))
        vp8x_chunk = bytearray(b"VP8X")
        vp8x_chunk.extend(struct.pack("<I", len(vp8x_payload)))
        vp8x_chunk.extend(vp8x_payload)
        payload = b"WEBP" + bytes(vp8x_chunk) + vp8_chunk
        webp = bytearray(b"RIFF")
        webp.extend(struct.pack("<I", len(payload)))
        webp.extend(payload)
        (d / "extended_vp8_dimension_mismatch.webp").write_bytes(webp)

    write_extended_vp8_dimension_mismatch()

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

    def webp_chunk_bytes(data, fourcc):
        cursor = 12
        while cursor + 8 <= len(data):
            chunk_size = struct.unpack_from("<I", data, cursor + 4)[0]
            chunk_end = cursor + 8 + chunk_size + (chunk_size & 1)
            if data[cursor : cursor + 4] == fourcc:
                return bytes(data[cursor:chunk_end])
            cursor = chunk_end
        raise RuntimeError(f"WebP did not contain a {fourcc!r} chunk")

    def riff_from_webp_chunks(chunks):
        payload = b"WEBP" + b"".join(chunks)
        return b"RIFF" + struct.pack("<I", len(payload)) + payload

    def write_anim_payload_eof_after_anmf():
        source = (d / "animated.webp").read_bytes()
        vp8x = webp_chunk_bytes(source, b"VP8X")
        anmf = webp_chunk_bytes(source, b"ANMF")
        anim = webp_chunk_bytes(source, b"ANIM")
        truncated_anim = b"ANIM" + struct.pack("<I", 6) + anim[8:12]
        (d / "animated_anim_payload_eof_after_anmf.webp").write_bytes(
            riff_from_webp_chunks((vp8x, anmf, truncated_anim))
        )

    write_anim_payload_eof_after_anmf()

    def truncate_after_first_nested_alpha(data):
        anmf = data.find(b"ANMF")
        alpha = data.find(b"ALPH", anmf + 8)
        if alpha < 0:
            raise RuntimeError("animated alpha WebP did not contain a nested ALPH chunk")
        alpha_size = struct.unpack_from("<I", data, alpha + 4)[0]
        alpha_end = alpha + 8 + alpha_size + (alpha_size & 1)
        del data[alpha_end:]
        struct.pack_into("<I", data, 4, len(data) - 8)

    write_mutated_webp(
        "animated_alpha_missing_nested_vp8_header.webp",
        "animated_alpha_lossy.webp",
        truncate_after_first_nested_alpha,
    )

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
                elif compression == 5:
                    codes = []
                    for value in payload:
                        codes.extend((256, value))
                    codes.append(257)
                    tile_payloads.append(pack_lzw_codes(codes))
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


def write_compressed_grayscale_tiff(path, payload, compression, width=1):
    """Write a one-row grayscale TIFF around an explicit compressed stream."""
    entries = [
        (256, 4, 1, width),
        (257, 4, 1, 1),
        (258, 3, 1, 8),
        (259, 3, 1, compression),
        (262, 3, 1, 1),
        (273, 4, 1, "pixels"),
        (277, 3, 1, 1),
        (278, 4, 1, 1),
        (279, 4, 1, len(payload)),
    ]
    pixel_offset = 8 + 2 + len(entries) * 12 + 4
    output = bytearray(b"II*\0\x08\0\0\0")
    output.extend(struct.pack("<H", len(entries)))
    for tag, field_type, count, value in entries:
        output.extend(struct.pack("<HHI", tag, field_type, count))
        if value == "pixels":
            output.extend(struct.pack("<I", pixel_offset))
        elif field_type == 3:
            output.extend(struct.pack("<H", value) + b"\0\0")
        else:
            output.extend(struct.pack("<I", value))
    output.extend(struct.pack("<I", 0))
    output.extend(payload)
    path.write_bytes(output)


def write_packbits_tiff(path, payload):
    write_compressed_grayscale_tiff(path, payload, 32773)


def pack_lzw_codes(codes):
    """Pack the small nine-bit code streams used by LZW boundary fixtures."""
    bits = "".join(f"{code:09b}" for code in codes)
    bits += "0" * (-len(bits) % 8)
    return int(bits, 2).to_bytes(len(bits) // 8, "big")


def write_lzw_tiff(path, codes, width=1):
    write_compressed_grayscale_tiff(path, pack_lzw_codes(codes), 5, width)


def write_lzw_dictionary_saturation_tiff(path, pixel_count=4100):
    """Write literal LZW codes through the full 12-bit dictionary range."""
    code_width = 9
    next_code = 258
    fields = [(256, code_width), (0, code_width)]
    for _ in range(1, pixel_count):
        fields.append((0, code_width))
        if next_code < 4096:
            next_code += 1
            if code_width < 12 and next_code == (1 << code_width) - 1:
                code_width += 1
    fields.append((257, code_width))
    bits = "".join(f"{code:0{width}b}" for code, width in fields)
    bits += "0" * (-len(bits) % 8)
    payload = int(bits, 2).to_bytes(len(bits) // 8, "big")
    write_compressed_grayscale_tiff(path, payload, 5, pixel_count)


def write_grayscale_predictor_tiff(
    path, bits, byte_order, photometric=1, sample_format=3
):
    """Write Deflate-compressed grayscale samples with horizontal prediction."""
    width, height = 4, 2
    marker = b"II" if byte_order == "<" else b"MM"
    if bits == 16:
        rows = ([1000, 2000, 4000, 8000], [123, 456, 789, 1024])
        format_code = "H"
        mask = 0xFFFF
    else:
        rows = (
            [struct.unpack("<I", struct.pack("<f", value))[0] for value in (1.0, 2.0, 4.0, 8.0)],
            [struct.unpack("<I", struct.pack("<f", value))[0] for value in (0.5, 1.5, 3.5, 7.5)],
        )
        format_code = "I"
        mask = 0xFFFF_FFFF
    predicted = bytearray()
    for row in rows:
        previous = 0
        for value in row:
            predicted.extend(struct.pack(byte_order + format_code, (value - previous) & mask))
            previous = value
    payload = zlib.compress(predicted)
    entries = [
        (256, 4, 1, width),
        (257, 4, 1, height),
        (258, 3, 1, bits),
        (259, 3, 1, 8),
        (262, 3, 1, photometric),
        (273, 4, 1, "pixels"),
        (277, 3, 1, 1),
        (278, 4, 1, height),
        (279, 4, 1, len(payload)),
        (317, 3, 1, 2),
    ]
    if bits == 32:
        entries.append((339, 3, 1, sample_format))
    entries.sort()
    pixel_offset = 8 + 2 + len(entries) * 12 + 4
    output = bytearray(marker + struct.pack(byte_order + "H", 42) + struct.pack(byte_order + "I", 8))
    output.extend(struct.pack(byte_order + "H", len(entries)))
    for tag, field_type, count, value in entries:
        output.extend(struct.pack(byte_order + "HHI", tag, field_type, count))
        if value == "pixels":
            output.extend(struct.pack(byte_order + "I", pixel_offset))
        elif field_type == 3:
            output.extend(struct.pack(byte_order + "H", value) + b"\0\0")
        else:
            output.extend(struct.pack(byte_order + "I", value))
    output.extend(struct.pack(byte_order + "I", 0))
    output.extend(payload)
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


def mutate_tiff_tag_count(source, destination, tag, count):
    """Patch one classic-TIFF entry count without rewriting its payload."""
    data = bytearray(source.read_bytes())
    byte_order = "<" if data[:2] == b"II" else ">"
    ifd_offset = struct.unpack_from(byte_order + "I", data, 4)[0]
    entry_count = struct.unpack_from(byte_order + "H", data, ifd_offset)[0]
    for index in range(entry_count):
        start = ifd_offset + 2 + index * 12
        actual_tag = struct.unpack_from(byte_order + "H", data, start)[0]
        if actual_tag == tag:
            struct.pack_into(byte_order + "I", data, start + 4, count)
            destination.write_bytes(data)
            return
    raise ValueError(f"TIFF tag {tag} not found")


def mutate_tiff_tag_type(source, destination, tag, field_type):
    """Patch the type of one classic-TIFF directory entry."""
    data = bytearray(source.read_bytes())
    byte_order = "<" if data[:2] == b"II" else ">"
    ifd_offset = struct.unpack_from(byte_order + "I", data, 4)[0]
    entry_count = struct.unpack_from(byte_order + "H", data, ifd_offset)[0]
    for index in range(entry_count):
        start = ifd_offset + 2 + index * 12
        actual_tag = struct.unpack_from(byte_order + "H", data, start)[0]
        if actual_tag == tag:
            struct.pack_into(byte_order + "H", data, start + 2, field_type)
            destination.write_bytes(data)
            return
    raise ValueError(f"TIFF tag {tag} not found")


def mutate_tiff_tag_id(source, destination, tag, replacement):
    """Rename one classic-TIFF directory tag while retaining its payload."""
    data = bytearray(source.read_bytes())
    byte_order = "<" if data[:2] == b"II" else ">"
    ifd_offset = struct.unpack_from(byte_order + "I", data, 4)[0]
    entry_count = struct.unpack_from(byte_order + "H", data, ifd_offset)[0]
    for index in range(entry_count):
        start = ifd_offset + 2 + index * 12
        actual_tag = struct.unpack_from(byte_order + "H", data, start)[0]
        if actual_tag == tag:
            struct.pack_into(byte_order + "H", data, start, replacement)
            destination.write_bytes(data)
            return
    raise ValueError(f"TIFF tag {tag} not found")


def mutate_tiff_next_ifd(source, destination, next_offset):
    """Patch the first classic-TIFF directory's next-IFD pointer."""
    data = bytearray(source.read_bytes())
    byte_order = "<" if data[:2] == b"II" else ">"
    ifd_offset = struct.unpack_from(byte_order + "I", data, 4)[0]
    entry_count = struct.unpack_from(byte_order + "H", data, ifd_offset)[0]
    position = ifd_offset + 2 + entry_count * 12
    struct.pack_into(byte_order + "I", data, position, next_offset)
    destination.write_bytes(data)


def write_tiff_truncated_second_ifd(source, destination):
    """Append an incomplete second IFD and point the first directory at it."""
    data = bytearray(source.read_bytes())
    byte_order = "<" if data[:2] == b"II" else ">"
    ifd_offset = struct.unpack_from(byte_order + "I", data, 4)[0]
    entry_count = struct.unpack_from(byte_order + "H", data, ifd_offset)[0]
    position = ifd_offset + 2 + entry_count * 12
    struct.pack_into(byte_order + "I", data, position, len(data))
    data.extend(b"\x01")
    destination.write_bytes(data)


def write_descending_strip_offsets_tiff(path):
    """Write a compressed classic TIFF with inferred descending strip offsets."""
    entries = [
        (256, 4, 1, 1),
        (257, 4, 1, 2),
        (258, 3, 1, 8),
        (259, 3, 1, 32773),
        (262, 3, 1, 1),
        (273, 4, 2, "offsets"),
        (277, 3, 1, 1),
        (278, 4, 1, 1),
        (279, 4, 0, 0),
        (284, 3, 1, 1),
    ]
    entries.sort()
    external_start = 8 + 2 + len(entries) * 12 + 4
    payload = b"\x00\x07\x00\x08"
    pixel_offset = external_start + 8
    offsets = (pixel_offset + 2, pixel_offset)
    out = bytearray(b"II*\0\x08\0\0\0")
    out.extend(struct.pack("<H", len(entries)))
    for tag, field_type, count, value in entries:
        out.extend(struct.pack("<HHI", tag, field_type, count))
        if value == "offsets":
            out.extend(struct.pack("<I", external_start))
        elif field_type == 3:
            out.extend(struct.pack("<H", value) + b"\0\0")
        else:
            out.extend(struct.pack("<I", value))
    out.extend(struct.pack("<I", 0))
    out.extend(struct.pack("<II", *offsets))
    out.extend(payload)
    path.write_bytes(out)


def write_oversized_rgba_tile_tiff(path):
    """Write a valid RGBA layout whose TIFF LONG tile geometry overflows."""
    entries = [
        (256, 4, 1, 1),
        (257, 4, 1, 1),
        (258, 3, 4, "bits"),
        (259, 3, 1, 1),
        (262, 3, 1, 2),
        (277, 3, 1, 4),
        (284, 3, 1, 1),
        (322, 4, 1, 0xFFFF_FFFF),
        (323, 4, 1, 0xFFFF_FFFF),
        (324, 4, 1, "pixels"),
        (325, 4, 1, 0),
        (338, 3, 1, 2),
    ]
    entries.sort()
    external_start = 8 + 2 + len(entries) * 12 + 4
    bits = struct.pack("<HHHH", 8, 8, 8, 8)
    pixel_offset = external_start + len(bits)
    out = bytearray(b"II*\0\x08\0\0\0")
    out.extend(struct.pack("<H", len(entries)))
    for tag, field_type, count, value in entries:
        out.extend(struct.pack("<HHI", tag, field_type, count))
        if value == "bits":
            out.extend(struct.pack("<I", external_start))
        elif value == "pixels":
            out.extend(struct.pack("<I", pixel_offset))
        elif field_type == 3:
            out.extend(struct.pack("<H", value) + b"\0\0")
        else:
            out.extend(struct.pack("<I", value))
    out.extend(struct.pack("<I", 0))
    out.extend(bits)
    out.extend(b"\0")
    path.write_bytes(out)


def gen_tiff():
    d = OUT / "tiff"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    img.save(d / "rgb.tiff")
    img.save(d / "rgb_dpi.tiff", dpi=(96, 96))
    img.save(d / "single.tiff")
    img.convert("L").save(d / "gray.tiff")
    img.convert("1").save(d / "1bit.tiff")
    img.convert("L").save(d / "8bit.tiff")
    img.convert("I;16").save(d / "16bit.tiff")
    img.convert("F").save(d / "float32.tiff")
    img.convert("RGBA").save(d / "rgba.tiff")
    img.convert("LA").save(d / "gray_alpha.tiff")
    img.convert("P").save(d / "palette.tiff")
    img.convert("CMYK").save(d / "cmyk.tiff")
    write_ycbcr_tiff(d / "ycbcr.tiff", img.resize((17, 13)))
    img.convert("1").save(d / "bilevel.tiff")
    low_depth = img.convert("L").resize((17, 13))
    write_low_depth_tiff(d / "miniswhite_1bit.tiff", low_depth, 1, 0)
    write_low_depth_tiff(
        d / "miniswhite_1bit_aligned.tiff",
        low_depth.resize((16, 8)),
        1,
        0,
    )
    write_low_depth_tiff(d / "miniswhite_8bit.tiff", low_depth, 8, 0)
    write_low_depth_tiff(d / "gray2.tiff", low_depth, 2, 1)
    write_low_depth_tiff(d / "gray4.tiff", low_depth, 4, 1)
    write_low_depth_tiff(d / "miniswhite_2bit.tiff", low_depth, 2, 0)
    write_low_depth_tiff(d / "miniswhite_4bit.tiff", low_depth, 4, 0)
    write_low_depth_tiff(d / "palette2.tiff", low_depth, 2, 3)
    write_low_depth_tiff(d / "palette4.tiff", low_depth, 4, 3)
    img.save(d / "uncompressed.tiff", compression=None)
    img.save(d / "lzw.tiff", compression="tiff_lzw")
    img.save(d / "deflate.tiff", compression="tiff_adobe_deflate")
    img.save(d / "packbits.tiff", compression="packbits")
    write_packbits_tiff(d / "packbits_noop.tiff", b"\x80\x00\x7f")
    write_packbits_tiff(d / "packbits_literal_overrun.tiff", b"\x01\x00\x01")
    write_packbits_tiff(d / "packbits_run_overrun.tiff", b"\xff\x00")
    write_compressed_grayscale_tiff(
        d / "deflate_short_header.tiff",
        b"\x78\x01\x00\x00\x00",
        8,
    )
    write_compressed_grayscale_tiff(
        d / "deflate_invalid_header.tiff",
        b"\x00\x00\x00\x00\x00\x00",
        8,
    )
    write_compressed_grayscale_tiff(
        d / "deflate_reserved_block.tiff",
        b"\x78\x01\x07\x00\x00\x00\x00",
        8,
    )
    write_compressed_grayscale_tiff(
        d / "deflate_bad_stored_complement.tiff",
        b"\x78\x01\x01\x01\x00\x01\x00\x00\x00\x00\x00\x00",
        8,
    )
    bad_tiff_adler = bytearray(zlib.compress(b"\x80", level=0))
    bad_tiff_adler[-1] ^= 0x01
    write_compressed_grayscale_tiff(
        d / "deflate_bad_adler.tiff",
        bytes(bad_tiff_adler),
        8,
    )
    write_compressed_grayscale_tiff(
        d / "deflate_truncated_fixed_block.tiff",
        b"\x78\x01\x03\x00\x00\x00\x01",
        8,
    )
    write_compressed_grayscale_tiff(
        d / "deflate_backreference_before_output.tiff",
        malformed_fixed_zlib([257, 256], distances=[0]),
        8,
    )
    write_compressed_grayscale_tiff(
        d / "deflate_oversized_stored_output.tiff",
        zlib.compress(b"\x80\x81", level=0),
        8,
    )
    write_lzw_tiff(d / "lzw_no_eoi.tiff", [256, 7])
    write_lzw_tiff(d / "lzw_trailing_code.tiff", [256, 0, 300])
    write_lzw_tiff(d / "lzw_kwkwk_clipped.tiff", [256, 0, 258, 257], width=2)
    write_lzw_tiff(d / "lzw_invalid_first.tiff", [258])
    write_lzw_tiff(d / "lzw_invalid_future_code.tiff", [256, 0, 300], width=2)
    write_lzw_tiff(d / "lzw_clear_only.tiff", [256])
    write_lzw_tiff(d / "lzw_end_only.tiff", [256, 257])
    write_lzw_dictionary_saturation_tiff(d / "lzw_dictionary_saturation.tiff")
    img.convert("L").save(d / "gray_lzw.tiff", compression="tiff_lzw")
    img.convert("L").save(d / "gray_deflate.tiff", compression="tiff_adobe_deflate")
    img.convert("F").save(
        d / "float32_deflate_predictor.tiff",
        compression="tiff_adobe_deflate",
        tiffinfo={317: 2},
    )
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
    write_rgb_tiff(
        d / "tiled_lzw_predictor.tiff",
        img,
        tile_size=32,
        compression=5,
        predictor=2,
    )
    write_rgb_tiff(
        d / "tiled_adobe_deflate_predictor.tiff",
        img,
        tile_size=32,
        compression=32946,
        predictor=2,
    )
    write_grayscale_predictor_tiff(d / "be_float32_predictor.tiff", 32, ">")
    write_grayscale_predictor_tiff(
        d / "le_unsigned32_predictor.tiff", 32, "<", sample_format=1
    )
    write_grayscale_predictor_tiff(
        d / "be_signed32_predictor.tiff", 32, ">", sample_format=2
    )
    write_grayscale_predictor_tiff(
        d / "unsupported_sample_format.tiff", 32, "<", sample_format=4
    )
    signed32 = Image.new("I", (4, 2))
    signed32.putdata([-2, -1, 0, 1, 2, 1024, -1024, 2_147_483_647])
    signed32.save(d / "signed32.tiff")
    write_grayscale_predictor_tiff(
        d / "be_16bit_unsupported_photometric.tiff", 16, ">", photometric=4
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
    sequence_page = pattern_img("RGB", (9, 7))
    sequence_page.save(
        d / "multipage.tiff",
        save_all=True,
        append_images=[sequence_page.transpose(Image.Transpose.FLIP_LEFT_RIGHT)],
    )
    mixed_page = Image.new("L", (5, 3), 137)
    sequence_page.save(
        d / "multipage_mixed.tiff",
        save_all=True,
        append_images=[mixed_page],
    )
    d.joinpath("bad_ifd.tiff").write_bytes(b"II\x2a\x00\x08\x00\x00\x00\xff\xff\xff")
    d.joinpath("truncated_signature.tiff").write_bytes(b"I")
    d.joinpath("truncated_magic.tiff").write_bytes(b"II")
    d.joinpath("truncated_ifd_offset.tiff").write_bytes(b"II\x2a\x00")
    d.joinpath("empty_ifd_chain.tiff").write_bytes(b"II\x2a\x00\0\0\0\0")
    d.joinpath("truncated_ifd_count.tiff").write_bytes(b"II\x2a\x00\x08\x00\x00\x00")
    d.joinpath("truncated_ifd_entry.tiff").write_bytes(b"II\x2a\x00\x08\x00\x00\x00\x01\x00")
    d.joinpath("oob_tag_value_offset.tiff").write_bytes(
        b"II"
        + struct.pack("<HI", 42, 8)
        + struct.pack("<H", 1)
        + struct.pack("<HHII", 256, 4, 2, 0xFFFF_FFF0)
        + struct.pack("<I", 0)
    )
    invalid_magic = bytearray((d / "rgb.tiff").read_bytes())
    invalid_magic[2:4] = b"+\0"
    (d / "invalid_magic.tiff").write_bytes(invalid_magic)
    for source, name, signature in (
        ("le.tiff", "legacy_le_swapped_magic.tiff", b"II\0*"),
        ("be.tiff", "legacy_be_swapped_magic.tiff", b"MM*\0"),
        ("be.tiff", "bigtiff_be_signature.tiff", b"MM\0+"),
    ):
        variant = bytearray((d / source).read_bytes())
        variant[:4] = signature
        (d / name).write_bytes(variant)
    invalid_endian = bytearray((d / "rgb.tiff").read_bytes())
    invalid_endian[:2] = b"ZZ"
    (d / "invalid_endian.tiff").write_bytes(invalid_endian)
    mutate_tiff_tag(d / "rgb.tiff", d / "zero_width.tiff", 256, 0)
    mutate_tiff_tag(d / "rgb.tiff", d / "zero_height.tiff", 257, 0)
    mutate_tiff_tag_id(d / "rgb.tiff", d / "missing_height.tiff", 257, 65_000)
    mutate_tiff_tag(d / "rgb.tiff", d / "decompression_bomb.tiff", 256, 0xFFFF_FFFF)
    mutate_tiff_tag(
        d / "decompression_bomb.tiff",
        d / "decompression_bomb.tiff",
        257,
        0xFFFF_FFFF,
    )
    mutate_tiff_tag_count(d / "gray.tiff", d / "empty_width.tiff", 256, 0)
    mutate_tiff_tag_type(d / "gray.tiff", d / "bits_256.tiff", 258, 4)
    mutate_tiff_tag(d / "bits_256.tiff", d / "bits_256.tiff", 258, 256)
    mutate_tiff_tag(d / "16bit.tiff", d / "miniswhite_16bit.tiff", 262, 0)
    mutate_tiff_tag(d / "rgb.tiff", d / "mixed_bits.tiff", 258, 16, 1)
    mutate_tiff_tag_count(d / "rgb.tiff", d / "empty_bits.tiff", 258, 0)
    mutate_tiff_next_ifd(d / "rgb.tiff", d / "cyclic_ifd.tiff", 8)
    write_tiff_truncated_second_ifd(d / "rgb.tiff", d / "truncated_second_ifd.tiff")
    mutate_tiff_tag(d / "rgb.tiff", d / "rows_zero.tiff", 278, 0)
    mutate_tiff_tag(d / "rgb.tiff", d / "unknown_compression.tiff", 259, 999)
    mutate_tiff_tag(d / "rgb.tiff", d / "unsupported_photometric.tiff", 262, 4)
    mutate_tiff_tag_type(d / "rgb.tiff", d / "byte_strip_offset.tiff", 273, 1)
    mutate_tiff_tag_type(d / "rgb_dpi.tiff", d / "unknown_field_type.tiff", 282, 13)
    mutate_tiff_tag_type(d / "rgb.tiff", d / "ascii_width.tiff", 256, 2)
    mutate_tiff_tag_type(d / "rgb.tiff", d / "ascii_height.tiff", 257, 2)
    mutate_tiff_tag_type(d / "rgb.tiff", d / "ascii_bits.tiff", 258, 2)
    mutate_tiff_tag_id(d / "palette.tiff", d / "missing_color_map.tiff", 320, 65000)
    mutate_tiff_tag_count(d / "palette.tiff", d / "short_color_map.tiff", 320, 1)
    mutate_tiff_tag_type(d / "rgb.tiff", d / "ascii_strip_offsets.tiff", 273, 2)
    mutate_tiff_tag_type(
        d / "deflate.tiff", d / "ascii_compressed_strip_byte_counts.tiff", 279, 2
    )
    mutate_tiff_tag_count(d / "deflate.tiff", d / "compressed_empty_strip_counts.tiff", 279, 0)
    mutate_tiff_tag_count(d / "deflate.tiff", d / "compressed_bad_strip_counts.tiff", 279, 2)
    write_descending_strip_offsets_tiff(d / "compressed_descending_strip_offsets.tiff")
    mutate_tiff_tag_count(d / "lzw_no_eoi.tiff", d / "lzw_post_ifd_empty_count.tiff", 279, 0)
    mutate_tiff_tag(d / "rgb.tiff", d / "uncompressed_bad_byte_count.tiff", 279, 1)
    mutate_tiff_tag(d / "rgb.tiff", d / "uncompressed_missing_strips.tiff", 278, 1)
    mutate_tiff_tag(
        d / "uncompressed_missing_strips.tiff",
        d / "uncompressed_missing_strips.tiff",
        279,
        384,
    )
    mutate_tiff_tag(
        d / "stripped.tiff", d / "uncompressed_extra_strips.tiff", 278, 128
    )
    mutate_tiff_tag(
        d / "gray_alpha.tiff", d / "miniswhite_gray_alpha.tiff", 262, 0
    )
    mutate_tiff_tag(
        d / "rgb_deflate_predictor.tiff",
        d / "invalid_predictor.tiff",
        317,
        3,
    )
    mutate_tiff_tag(d / "rgb.tiff", d / "oob_strip.tiff", 273, 0xFFFF_FFF0)
    mutate_tiff_tag_count(d / "rgb.tiff", d / "empty_strip_offsets.tiff", 273, 0)
    mutate_tiff_tag_id(
        d / "rgb.tiff", d / "missing_strip_offsets.tiff", 273, 65_000
    )
    mutate_tiff_tag_id(
        d / "rgb.tiff", d / "missing_strip_byte_counts.tiff", 279, 65_000
    )
    mutate_tiff_tag(d / "tiled.tiff", d / "zero_tile_width.tiff", 322, 0)
    mutate_tiff_tag_id(
        d / "tiled.tiff", d / "missing_tile_width.tiff", 322, 65_000
    )
    mutate_tiff_tag_id(
        d / "tiled.tiff", d / "missing_tile_height.tiff", 323, 65_000
    )
    mutate_tiff_tag_type(d / "tiled.tiff", d / "ascii_tile_width.tiff", 322, 2)
    mutate_tiff_tag_type(d / "tiled.tiff", d / "ascii_tile_height.tiff", 323, 2)
    mutate_tiff_tag_count(d / "tiled.tiff", d / "empty_tile_offsets.tiff", 324, 0)
    mutate_tiff_tag_count(d / "tiled.tiff", d / "empty_tile_byte_counts.tiff", 325, 0)
    write_oversized_rgba_tile_tiff(d / "oversized_rgba_tile.tiff")
    mutate_tiff_tag_count(
        d / "tiled_deflate_predictor.tiff",
        d / "compressed_empty_tile_byte_counts.tiff",
        325,
        0,
    )
    print(f"  TIFF: {len(list(d.glob('*.tiff')))} files")


def gen_ico():
    d = OUT / "ico"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB").resize((16,16))
    img.save(d / "16x16.ico", format="ICO", sizes=[(16,16)])
    img.save(d / "single.ico", format="ICO", sizes=[(16,16)])
    pattern_img("RGB").save(d / "multi.ico", format="ICO", sizes=[(16,16),(32,32)])
    multi_descending = bytearray((d / "multi.ico").read_bytes())
    multi_count = struct.unpack_from("<H", multi_descending, 4)[0]
    multi_entries = [
        bytes(multi_descending[6 + index * 16 : 22 + index * 16])
        for index in range(multi_count)
    ]
    for index, entry in enumerate(reversed(multi_entries)):
        multi_descending[6 + index * 16 : 22 + index * 16] = entry
    (d / "multi_descending.ico").write_bytes(multi_descending)
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
    bmp_default_palette = bytearray(ico)
    struct.pack_into("<I", bmp_default_palette, 22 + 32, 0)
    (d / "bmp_default_palette.ico").write_bytes(bmp_default_palette)
    cursor = bytearray(ico)
    struct.pack_into("<H", cursor, 2, 2)
    struct.pack_into("<HH", cursor, 10, 3, 5)
    (d / "cursor.cur").write_bytes(cursor)

    cursor_default_palette = bytearray(cursor)
    struct.pack_into("<I", cursor_default_palette, 22 + 32, 0)
    (d / "cursor_default_palette.cur").write_bytes(cursor_default_palette)

    cursor_24bit = bytearray((d / "bmp_entry.ico").read_bytes())
    struct.pack_into("<H", cursor_24bit, 2, 2)
    struct.pack_into("<HH", cursor_24bit, 10, 3, 5)
    (d / "cursor_24bit.cur").write_bytes(cursor_24bit)

    transparent_24bit = bytearray((d / "bmp_entry.ico").read_bytes())
    transparent_24bit[-64] |= 0x80
    (d / "bmp_24bit_transparent.ico").write_bytes(transparent_24bit)

    odd_1bit = bytearray((d / "bmp_1bit.ico").read_bytes())
    odd_1bit[6] = 15
    struct.pack_into("<I", odd_1bit, 22 + 4, 15)
    (d / "bmp_1bit_odd.ico").write_bytes(odd_1bit)

    for source_name, destination_name in [
        ("bmp_1bit.ico", "bmp_1bit_short_palette.ico"),
        ("bmp_8bit.ico", "bmp_8bit_short_palette.ico"),
    ]:
        short_palette = bytearray((d / source_name).read_bytes())
        payload_offset = struct.unpack_from("<I", short_palette, 18)[0]
        struct.pack_into("<I", short_palette, payload_offset + 32, 1)
        (d / destination_name).write_bytes(short_palette)
    short_palette_1bit = bytearray((d / "bmp_1bit_short_palette.ico").read_bytes())
    short_palette_1bit_offset = struct.unpack_from("<I", short_palette_1bit, 18)[0]
    short_palette_1bit[short_palette_1bit_offset + 44] = 0x80
    (d / "bmp_1bit_short_palette.ico").write_bytes(short_palette_1bit)
    for name, first_index_byte in [
        ("bmp_4bit_short_palette_high.ico", 0x10),
        ("bmp_4bit_short_palette_low.ico", 0x01),
    ]:
        short_palette_4bit = bytearray((d / "bmp_4bit.ico").read_bytes())
        payload_offset = struct.unpack_from("<I", short_palette_4bit, 18)[0]
        struct.pack_into("<I", short_palette_4bit, payload_offset + 32, 1)
        short_palette_4bit[payload_offset + 44] = first_index_byte
        (d / name).write_bytes(short_palette_4bit)

    def write_truncated_payload(name, source_name, payload_len):
        truncated = bytearray((d / source_name).read_bytes())
        data_offset = struct.unpack_from("<I", truncated, 18)[0]
        struct.pack_into("<I", truncated, 14, payload_len)
        del truncated[data_offset + payload_len :]
        (d / name).write_bytes(truncated)

    write_truncated_payload("bmp_32bit_truncated_pixels.ico", "bmp_32bit.ico", 40)
    write_truncated_payload("bmp_24bit_truncated_pixels.ico", "bmp_entry.ico", 40)
    write_truncated_payload("bmp_8bit_truncated_pixels.ico", "bmp_8bit.ico", 40)
    write_truncated_payload("bmp_4bit_truncated_pixels.ico", "bmp_4bit.ico", 40)
    write_truncated_payload("bmp_1bit_truncated_pixels.ico", "bmp_1bit.ico", 40)
    write_truncated_payload("png_short_header.ico", "png_entry.ico", 8)
    write_truncated_payload("cursor_truncated_dib.cur", "cursor.cur", 20)

    short_dib = bytearray(ico[: 22 + 20])
    struct.pack_into("<I", short_dib, 14, 20)
    (d / "short_dib.ico").write_bytes(short_dib)
    zero_width = bytearray(ico)
    struct.pack_into("<I", zero_width, 22 + 4, 0)
    (d / "zero_width.ico").write_bytes(zero_width)
    zero_height = bytearray(ico)
    struct.pack_into("<I", zero_height, 22 + 8, 0)
    (d / "zero_height.ico").write_bytes(zero_height)
    oversized_width = bytearray(ico)
    struct.pack_into("<I", oversized_width, 22 + 4, 16_385)
    (d / "oversized_width.ico").write_bytes(oversized_width)
    oversized_height = bytearray(ico)
    struct.pack_into("<I", oversized_height, 22 + 8, 32_770)
    (d / "oversized_height.ico").write_bytes(oversized_height)
    unsupported_bpp = bytearray(ico)
    struct.pack_into("<H", unsupported_bpp, 22 + 14, 2)
    (d / "unsupported_bpp.ico").write_bytes(unsupported_bpp)
    cursor_short_header = bytearray(cursor)
    struct.pack_into("<I", cursor_short_header, 22, 20)
    (d / "cursor_short_header.cur").write_bytes(cursor_short_header)
    cursor_header_oob = bytearray(cursor)
    cursor_payload_len = struct.unpack_from("<I", cursor_header_oob, 14)[0]
    struct.pack_into("<I", cursor_header_oob, 22, cursor_payload_len + 1)
    (d / "cursor_header_oob.cur").write_bytes(cursor_header_oob)
    cursor_palette_overflow = bytearray(cursor)
    struct.pack_into("<I", cursor_palette_overflow, 22 + 32, 0xFFFF_FFFF)
    (d / "cursor_palette_overflow.cur").write_bytes(cursor_palette_overflow)

    (d / "empty.ico").write_bytes(b"")
    (d / "invalid_reserved.ico").write_bytes(struct.pack("<HHH", 1, 1, 0))
    (d / "invalid_type.ico").write_bytes(struct.pack("<HHH", 0, 3, 0))
    (d / "zero_entries.ico").write_bytes(struct.pack("<HHH", 0, 1, 0))
    (d / "truncated_directory.ico").write_bytes(struct.pack("<HHH", 0, 1, 1) + b"\0" * 4)
    zero_entry = bytearray(struct.pack("<HHH", 0, 1, 1))
    zero_entry.extend(struct.pack("<BBBBHHII", 16, 16, 0, 0, 1, 32, 0, 0))
    (d / "zero_entry.ico").write_bytes(zero_entry)
    zero_offset = bytearray(ico)
    struct.pack_into("<I", zero_offset, 18, 0)
    (d / "zero_offset.ico").write_bytes(zero_offset)
    (d / "too_many_entries.ico").write_bytes(struct.pack("<HHH", 0, 1, 256))
    (d / "truncated_entry.ico").write_bytes(ico[:-20])
    img.resize((256,256)).save(d / "256x256.ico", format="ICO", sizes=[(256,256)])
    img.resize((48, 48)).save(d / "48x48.ico", format="ICO", sizes=[(48, 48)])
    print(f"  ICO: {len(list(d.glob('*.ico')))} files")


def gen_avif():
    d = OUT / "avif"
    d.mkdir(parents=True, exist_ok=True)
    from PIL import _avif, features

    def replace_top_level_box_kind(data, old_kind, new_kind):
        if len(old_kind) != 4 or len(new_kind) != 4:
            raise RuntimeError("AVIF box kinds must contain exactly four bytes")
        output = bytearray(data)
        cursor = 0
        replacements = 0
        while cursor < len(data):
            if len(data) - cursor < 8:
                raise RuntimeError("AVIF top-level box header is truncated")
            size = struct.unpack_from(">I", data, cursor)[0]
            header_size = 8
            if size == 1:
                if len(data) - cursor < 16:
                    raise RuntimeError("AVIF top-level large box header is truncated")
                size = struct.unpack_from(">Q", data, cursor + 8)[0]
                header_size = 16
            elif size == 0:
                size = len(data) - cursor
            if size < header_size or size > len(data) - cursor:
                raise RuntimeError("AVIF top-level box size is invalid")
            if data[cursor + 4 : cursor + 8] == old_kind:
                output[cursor + 4 : cursor + 8] = new_kind
                replacements += 1
            cursor += size
        if replacements != 1:
            raise RuntimeError(
                f"expected one top-level {old_kind!r} box, found {replacements}"
            )
        return bytes(output)

    codec_versions = _avif.codec_versions()
    if features.version("avif") != "1.4.1":
        raise RuntimeError(
            "AVIF fixture oracle requires libavif 1.4.1, "
            f"found {features.version('avif')}"
        )
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codec_versions:
            raise RuntimeError(
                f"AVIF fixture oracle requires {expected}, found {codec_versions}"
            )

    forbidden_422_source = d / "10bit.avif"
    forbidden_422_partition = bytearray(forbidden_422_source.read_bytes())
    if hashlib.sha256(forbidden_422_partition).hexdigest() != (
        "3bf9f91da471749e7df639ba7945d4d94c1c3e3968c26f3619fbbcfc92790576"
    ):
        raise RuntimeError("forbidden 4:2:2 partition source differs")
    original_422_tile = bytes.fromhex("00e234fe35f6ba4026a9e0b77e80")
    if forbidden_422_partition[2047:2061] != original_422_tile:
        raise RuntimeError("forbidden 4:2:2 partition tile moved")
    # The pinned scalar dav1d/Rust entropy oracle proves that this same-length
    # prefix selects a partition forbidden for horizontally subsampled 4:2:2
    # chroma. Retain the complete licensed AVIF container and frame header.
    forbidden_422_partition[2047:2061] = bytes.fromhex(
        "f83f9ffd73c02fa55948fac5e574"
    )
    if hashlib.sha256(forbidden_422_partition).hexdigest() != (
        "de34b2dc5855166b32e61aadffbead4989db3787e6db26fab77ae7129ec93381"
    ):
        raise RuntimeError("forbidden 4:2:2 partition mutation differs")
    (d / "forbidden_422_partition.avif").write_bytes(forbidden_422_partition)

    animated_source = d / "animated.avif"
    animated_bytes = animated_source.read_bytes()
    if hashlib.sha256(animated_bytes).hexdigest() != (
        "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
    ):
        raise RuntimeError("animated AVIF source differs from the pinned libavif fixture")
    animated_track_only = replace_top_level_box_kind(
        animated_bytes,
        b"meta",
        b"free",
    )
    (d / "animated_track_only.avif").write_bytes(animated_track_only)
    if animated_bytes.count(b"stsz") != 1:
        raise RuntimeError("animated AVIF must contain exactly one stsz box")
    animated_missing_stsz = animated_bytes.replace(b"stsz", b"free", 1)
    (d / "animated_missing_stsz.avif").write_bytes(animated_missing_stsz)
    if animated_bytes.count(b"stbl") != 1:
        raise RuntimeError("animated AVIF must contain exactly one stbl box")
    animated_missing_stbl = animated_bytes.replace(b"stbl", b"free", 1)
    (d / "animated_missing_stbl.avif").write_bytes(animated_missing_stbl)

    def encode_error_resilient_animation():
        first_frame = Image.new("RGB", (16, 16), (10, 20, 30))
        second_frame = Image.new("RGB", (16, 16), (40, 50, 60))
        output = BytesIO()
        first_frame.save(
            output,
            format="AVIF",
            save_all=True,
            append_images=[second_frame],
            duration=[100, 100],
            quality=80,
            speed=8,
            max_threads=1,
            advanced={"error-resilient": "1"},
        )
        encoded = bytearray(output.getvalue())
        for box_kind in (b"mvhd", b"tkhd", b"mdhd"):
            kind_offset = encoded.find(box_kind)
            if kind_offset < 4 or encoded[kind_offset + 4] != 1:
                raise RuntimeError(
                    f"error-resilient AVIF lacks one version-one {box_kind!r} box"
                )
            if encoded.find(box_kind, kind_offset + 4) != -1:
                raise RuntimeError(
                    f"error-resilient AVIF contains multiple {box_kind!r} boxes"
                )
            encoded[kind_offset + 8 : kind_offset + 24] = bytes(16)
        return bytes(encoded)

    error_resilient_animation = encode_error_resilient_animation()
    if error_resilient_animation != encode_error_resilient_animation():
        raise RuntimeError("error-resilient AVIF fixture is not deterministic")
    if hashlib.sha256(error_resilient_animation).hexdigest() != (
        "06ea9771f8b46c3432c6c6cdf324f1c05e86a5fdccd774c8e3c9a8fce0b831f0"
    ):
        raise RuntimeError("error-resilient AVIF fixture differs from its pinned hash")
    (d / "animated_error_resilient.avif").write_bytes(error_resilient_animation)

    repeated_frame_id = bytearray(error_resilient_animation)
    frame_id_start_bit = 1042 * 8 + 7
    frame_id_width = 15
    original_frame_id = 0
    for index in range(frame_id_width):
        bit_position = frame_id_start_bit + index
        original_frame_id = (original_frame_id << 1) | (
            (repeated_frame_id[bit_position // 8] >> (7 - bit_position % 8)) & 1
        )
    if original_frame_id != 4627:
        raise RuntimeError("error-resilient AVIF second frame ID moved")
    for index in range(frame_id_width):
        bit_position = frame_id_start_bit + index
        replacement = (4626 >> (frame_id_width - index - 1)) & 1
        mask = 1 << (7 - bit_position % 8)
        repeated_frame_id[bit_position // 8] = (
            repeated_frame_id[bit_position // 8] & ~mask
        ) | (replacement * mask)
    if hashlib.sha256(repeated_frame_id).hexdigest() != (
        "34ba8322879102ee291f9ec06703f20973c16475a0ebafb1c763e89ee9c73427"
    ):
        raise RuntimeError("repeated-frame-ID AVIF mutation differs")
    (d / "animated_repeated_frame_id.avif").write_bytes(repeated_frame_id)

    def write_portable_image(
        name,
        image,
        quality=100,
        speed=8,
        subsampling="4:4:4",
        advanced=None,
    ):
        def encode():
            output = BytesIO()
            options = {
                "format": "AVIF",
                "quality": quality,
                "speed": speed,
                "max_threads": 1,
                "subsampling": subsampling,
                "autotiling": False,
            }
            if advanced is not None:
                options["advanced"] = advanced
            image.save(output, **options)
            return output.getvalue()

        first = encode()
        second = encode()
        if first != second:
            raise RuntimeError(f"AVIF fixture {name} is not deterministic")
        (d / name).write_bytes(first)

    def write_portable(
        name,
        color,
        size=(4, 4),
        quality=100,
        speed=8,
        subsampling="4:4:4",
    ):
        write_portable_image(
            name,
            Image.new("RGB", size, color),
            quality=quality,
            speed=speed,
            subsampling=subsampling,
        )

    def write_portable_luma_pattern(name, size, sample):
        pixels = bytes(
            channel
            for y in range(size[1])
            for x in range(size[0])
            for channel in (sample(x, y),) * 3
        )
        write_portable_image(
            name,
            Image.frombytes("RGB", size, pixels),
            quality=99,
            subsampling="4:2:0",
        )

    def write_portable_lossless(
        name,
        color,
        size=(4, 4),
        speed=8,
        subsampling="4:4:4",
    ):
        write_portable(
            name,
            color,
            size=size,
            quality=100,
            speed=speed,
            subsampling=subsampling,
        )

    def write_square_partition(
        name,
        replacement,
        size=(16, 16),
        replacement_origin=(8, 8),
        subsampling="4:4:4",
    ):
        source = (17, 91, 203)
        pixels = bytes(
            component
            for y in range(size[1])
            for x in range(size[0])
            for component in (
                replacement
                if x >= replacement_origin[0] and y >= replacement_origin[1]
                else source
            )
        )
        image = Image.frombytes("RGB", size, pixels)

        def encode():
            output = BytesIO()
            image.save(
                output,
                format="AVIF",
                quality=100,
                speed=8,
                max_threads=1,
                subsampling=subsampling,
                autotiling=False,
            )
            return output.getvalue()

        first = encode()
        second = encode()
        if first != second:
            raise RuntimeError(f"AVIF fixture {name} is not deterministic")
        (d / name).write_bytes(first)

    def clamp_channel(value):
        return max(0, min(255, int(value)))

    def image_from_pixels(size, pixel):
        width, height = size
        pixels = bytes(
            component
            for y in range(height)
            for x in range(width)
            for component in pixel(x, y)
        )
        return Image.frombytes("RGB", size, pixels)

    def write_campaign_image(
        name,
        image,
        subsampling,
        advanced=None,
        quality=99,
        speed=8,
    ):
        write_portable_image(
            f"{name}.avif",
            image,
            quality=quality,
            speed=speed,
            subsampling=subsampling,
            advanced=advanced,
        )

    def write_campaign_family(
        prefix,
        count,
        make_image,
        subsampling,
        advanced=None,
        quality=99,
        speed=8,
    ):
        for index in range(count):
            write_campaign_image(
                f"{prefix}_{index + 1:02d}",
                make_image(index),
                subsampling,
                advanced=advanced,
                quality=quality,
                speed=speed,
            )

    def cfl_signal(family, index, x, y):
        """Return the deterministic origin Square16 CFL search field."""

        phase = (7 * family + 11 * index) % 16
        horizontal = ((x * 17 + phase) % 32) - 16
        vertical = ((y * 19 + phase) % 32) - 16
        diagonal = (((x + y) * 13 + phase) % 32) - 16
        opposing = (((x - y) * 11 + phase) % 32) - 16
        if family == 0:
            return horizontal * 4
        if family == 1:
            return (horizontal if y % 2 == 0 else -horizontal) * 4
        if family == 2:
            return vertical * 4
        if family == 3:
            return (vertical if x % 2 == 0 else -vertical) * 4
        if family == 4:
            return diagonal * 4
        if family == 5:
            return opposing * 4
        if family == 6:
            return (38 if (x // 2 + y // 2 + index) % 2 == 0 else -38) + horizontal
        if family == 7:
            quadrant = (x // 4) + 4 * (y // 4)
            return ((quadrant * 23 + phase) % 128) - 64
        if family == 8:
            distance = abs(x - 7) + abs(y - 7)
            return 80 - 10 * distance + (18 if x == 7 or y == 7 else 0)
        return diagonal * 2 + horizontal + (
            28 if (x + 2 * y + index) % 5 == 0 else -14
        )

    def square16_cfl_image(family, index):
        """Create one public 16x16 I444 CFL witness from the search corpus."""

        def yuv_to_rgb(y, u, v):
            du = u - 128
            dv = v - 128
            return (
                clamp_channel(y + (358 * dv + 128) // 256),
                clamp_channel(y - (88 * du + 183 * dv + 128) // 256),
                clamp_channel(y + (453 * du + 128) // 256),
            )

        def pixel(x, y):
            value = cfl_signal(family, index, x, y)
            orthogonal = ((23 * x + 29 * y + 7 * index + family) % 13) - 6
            luma = clamp_channel(128 + value // 3)
            u = clamp_channel(128 + value // 5 + orthogonal)
            v = clamp_channel(128 - (value * (3 + index % 3)) // 20 - orthogonal)
            return yuv_to_rgb(luma, u, v)

        return image_from_pixels((16, 16), pixel)

    cfl_advanced = {
        "min-partition-size": "16",
        "max-partition-size": "16",
        "use-intra-dct-only": "1",
        "enable-filter-intra": "0",
        "enable-intra-edge-filter": "0",
        "enable-smooth-intra": "0",
        "enable-paeth-intra": "0",
        "enable-directional-intra": "0",
        "enable-cfl-intra": "1",
        "enable-cdef": "0",
        "enable-restoration": "0",
        "loopfilter-control": "0",
        "aq-mode": "0",
        "deltaq-mode": "0",
    }
    for name, family, index in (
        ("coverage_i444_square16_cfl_01", 4, 2),
        ("coverage_i444_square16_cfl_02", 8, 2),
        ("coverage_i444_square16_cfl_03", 9, 6),
    ):
        write_campaign_image(
            name,
            square16_cfl_image(family, index),
            "4:4:4",
            advanced=cfl_advanced,
            quality=76,
            speed=0,
        )

    def write_v4_vertical_checker():
        """Generate the pinned 16x16 4:2:0 PARTITION_V4 witness."""

        def pixel(x, y):
            band = min(3, x // 4)
            phase = ((x + band * 3) // 2 + y * (band + 1)) % 4
            base = (24, 88, 152, 216)[band]
            return tuple(
                max(0, min(255, base + phase * step - 18))
                for step in (1, 3, 5)
            )

        write_campaign_image(
            "coverage_v4_vertical_checker",
            image_from_pixels((16, 16), pixel),
            "4:2:0",
            advanced={
                "enable-filter-intra": "0",
                "enable-restoration": "0",
                "min-partition-size": "4",
                "max-partition-size": "16",
            },
            quality=76,
            speed=0,
        )

    write_v4_vertical_checker()

    def write_h4_horizontal_bands():
        """Generate the pinned 16x16 4:2:0 PARTITION_H4 witness."""

        bands = (
            (224, 106, 202),
            (235, 115, 18),
            (74, 138, 132),
            (111, 243, 208),
        )

        def pixel(x, y):
            base = bands[min(3, y // 4)]
            delta = ((x * 3 + y * 7) % 17) - 8
            return tuple(
                clamp_channel(channel + delta) for channel in base
            )

        write_campaign_image(
            "coverage_h4_horizontal_bands",
            image_from_pixels((16, 16), pixel),
            "4:2:0",
            advanced={
                "enable-filter-intra": "0",
                "enable-restoration": "0",
                "min-partition-size": "4",
                "max-partition-size": "16",
            },
            quality=50,
            speed=0,
        )

    write_h4_horizontal_bands()

    rect4_filter_intra_advanced = {
        "min-partition-size": "4",
        "max-partition-size": "16",
        "use-intra-dct-only": "1",
        "enable-filter-intra": "1",
        "enable-intra-edge-filter": "0",
        "enable-smooth-intra": "0",
        "enable-paeth-intra": "0",
        "enable-directional-intra": "0",
        "enable-cfl-intra": "0",
        "enable-cdef": "0",
        "enable-restoration": "0",
        "loopfilter-control": "0",
        "aq-mode": "0",
        "deltaq-mode": "0",
    }

    def rect4_filter_intra_ramp(vertical):
        """Generate the exact public ramp reaching rectangular CDF rows 14/19."""

        bands = (
            (17, 91, 203),
            (32, 32, 32),
            (0, 255, 0),
            (127, 127, 127),
        )

        def pixel(x, y):
            coordinate = x if vertical else y
            return bands[min(3, coordinate // 4)]

        return image_from_pixels((16, 16), pixel)

    write_campaign_image(
        "coverage_h16x4_filter_intra_cdf14_false_01",
        rect4_filter_intra_ramp(False),
        "4:2:0",
        advanced=rect4_filter_intra_advanced,
        quality=12,
        speed=0,
    )
    write_campaign_image(
        "coverage_v4x16_filter_intra_cdf19_false_01",
        rect4_filter_intra_ramp(True),
        "4:2:0",
        advanced=rect4_filter_intra_advanced,
        quality=12,
        speed=0,
    )

    # Coverage campaign candidates are intentionally declarative and generated
    # through the same pinned Pillow/libaom path as the rest of this file. The
    # manifest decides which candidates become parity rows after public Rust
    # decode and AV1 trace inspection; none of these inputs are private hooks.
    write_campaign_family(
        "coverage_r4x16_band",
        10,
        lambda index: image_from_pixels(
            (4, 32),
            lambda x, y: (
                clamp_channel(118 + index + 8 * (y >= 16) + 2 * (x == 3)),
            )
            * 3,
        ),
        "4:2:0",
    )
    write_campaign_family(
        "coverage_r4x16_grid",
        10,
        lambda index: image_from_pixels(
            (8, 32),
            lambda x, y: (
                clamp_channel(112 + 2 * index + 7 * (x >= 4) + 5 * (y >= 16)),
                clamp_channel(126 + index - 5 * (x >= 4) + 7 * (y >= 16)),
                clamp_channel(140 - index + 3 * (x >= 4) - 4 * (y >= 16)),
            ),
        ),
        "4:2:0",
    )

    def directional_band(index):
        patterns = (
            lambda x, _y: 104 + 3 * x,
            lambda _x, y: 96 + 2 * y,
            lambda x, y: 100 + 2 * (x + y),
            lambda x, y: 100 + 2 * (7 - x + y),
            lambda x, y: 112 if ((x // 4) + (y // 4)) % 2 else 136,
        )
        pattern = patterns[index // 2]
        offset = -2 if index % 2 == 0 else 2
        return image_from_pixels(
            (8, 32),
            lambda x, y: (clamp_channel(pattern(x, y) + offset),) * 3,
        )

    write_campaign_family(
        "coverage_r8x16_band", 10, directional_band, "4:2:0"
    )

    def positioned_mosaic(index):
        base = (120 + index, 128, 136 - index)
        deltas = ((0, 0, 0), (6, -4, 3), (-5, 7, -3), (9, 5, -7))
        rotation = index % 3

        def pixel(x, y):
            quadrant = (2 if y >= 16 else 0) + (1 if x >= 8 else 0)
            delta = deltas[quadrant]
            rotated = tuple(delta[(channel + rotation) % 3] for channel in range(3))
            return tuple(
                clamp_channel(base[channel] + rotated[channel]) for channel in range(3)
            )

        return image_from_pixels((16, 32), pixel)

    write_campaign_family(
        "coverage_r8x16_neighbor", 10, positioned_mosaic, "4:2:0"
    )
    write_campaign_family(
        "coverage_r16x8_band",
        10,
        lambda index: image_from_pixels(
            (16, 16),
            lambda x, y: (
                clamp_channel(
                    116
                    + index
                    + 8 * (y >= 8)
                    + ((x + index) % 4)
                    + (x // 3 if index % 2 else 0)
                ),
            )
            * 3,
        ),
        "4:2:0",
    )

    def upper_context_mosaic(index):
        luma = (0, 5 + index % 3, -4 - index % 2, 8 + index)
        chroma = ((0, 0), (4, -3), (-3, 5), (6, 4))

        def pixel(x, y):
            quadrant = (2 if y >= 8 else 0) + (1 if x >= 16 else 0)
            u, v = chroma[quadrant]
            y_value = 124 + luma[quadrant]
            return (
                clamp_channel(y_value + u + v),
                clamp_channel(128 + luma[quadrant] - u),
                clamp_channel(128 + luma[quadrant] - v),
            )

        return image_from_pixels((32, 16), pixel)

    write_campaign_family(
        "coverage_r16x8_neighbor", 10, upper_context_mosaic, "4:2:0"
    )

    def transform_grid_mosaic(index):
        def pixel(x, y):
            if x < 16:
                luma = 80 + ((y * 4 + 7 * (index + 1)) % 128)
            else:
                local_x = x - 16
                quadrant = (local_x // 8) + 2 * (y // 16)
                luma = (40, 100, 180, 232)[quadrant] + ((local_x + y + index) % 5) - 2
            return (
                clamp_channel(luma),
                128,
                128,
            )

        return image_from_pixels((32, 32), pixel)

    write_campaign_family(
        "coverage_r16x32_grid",
        10,
        transform_grid_mosaic,
        "4:2:0",
        advanced={"enable-filter-intra": "0", "enable-restoration": "0"},
        quality=76,
        speed=0,
    )

    def horizontal_transform_origin():
        def pixel(x, y):
            quadrant = 2 * (y >= 8) + (x >= 16)
            base = (56, 108, 164, 212)[quadrant]
            ripple = ((7 * x + 11 * y) % 9) - 4
            u_delta = -10 if x < 16 else 12
            v_delta = -8 if y < 8 else 11
            return (
                clamp_channel(base + ripple + u_delta + v_delta),
                clamp_channel(base + ripple - u_delta),
                clamp_channel(base + ripple - v_delta),
            )

        return image_from_pixels((32, 16), pixel)

    write_campaign_image(
        "coverage_r32x16_origin_01",
        horizontal_transform_origin(),
        "4:2:0",
        advanced={
            "min-partition-size": "32",
            "max-partition-size": "32",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def horizontal_transform_following():
        def pixel(x, y):
            base = 40 if y < 16 else 210
            ripple = 2 * (((7 * x + 11 * y) % 9) - 4)
            chroma_delta = 8 if x >= 16 else -8
            luma = base + ripple
            return (
                clamp_channel(luma + chroma_delta),
                clamp_channel(luma),
                clamp_channel(luma - chroma_delta),
            )

        return image_from_pixels((32, 32), pixel)

    write_campaign_image(
        "coverage_r32x32_following_01",
        horizontal_transform_following(),
        "4:2:0",
        advanced={
            "min-partition-size": "16",
            "max-partition-size": "32",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    write_campaign_image(
        "coverage_r32x32_filter_intra_probe_01",
        horizontal_transform_following(),
        "4:2:0",
        advanced={
            "min-partition-size": "16",
            "max-partition-size": "32",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "1",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def filter_intra_mode3_noise():
        """Generate a deterministic noisy mode-3 filter-intra witness."""

        random_state = random.Random(1015)
        pixels = bytes(
            component
            for _ in range(32 * 32)
            for component in (random_state.randrange(256) for _ in range(3))
        )
        return Image.frombytes("RGB", (32, 32), pixels)

    write_campaign_image(
        "coverage_r32x32_filter_intra_mode3_01",
        filter_intra_mode3_noise(),
        "4:2:0",
        advanced={
            "min-partition-size": "16",
            "max-partition-size": "32",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "1",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def filter_intra_following_split_mode0_noise():
        """Generate a following H32x16 filter-intra/TX16x16 witness."""

        random_state = random.Random(3)
        # The candidate search's RGB-noise family consumes one grayscale-sized
        # prefix before producing the lower leaf's RGB samples. Retain that
        # deterministic construction so the promoted bytes remain identical
        # to the independently traced candidate.
        for _ in range(32 * 16):
            random_state.randrange(256)
        lower = bytes(
            random_state.randrange(256) for _ in range(32 * 16 * 3)
        )
        upper = bytes((128, 128, 128)) * (32 * 16)
        return Image.frombytes("RGB", (32, 32), upper + lower)

    write_campaign_image(
        "coverage_r32x32_following_filter_intra_split_mode0_01",
        filter_intra_following_split_mode0_noise(),
        "4:2:0",
        advanced={
            "min-partition-size": "16",
            "max-partition-size": "32",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "1",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def filter_intra_tx8x8_noise():
        """Generate the origin TX8x8 split witness with filter-intra disabled."""

        random_state = random.Random(2)
        pixels = bytes(
            random_state.randrange(256) for _ in range(32 * 16 * 3)
        )
        return Image.frombytes("RGB", (32, 16), pixels)

    write_campaign_image(
        "coverage_r32x16_filter_intra_tx8x8_01",
        filter_intra_tx8x8_noise(),
        "4:2:0",
        advanced={
            "min-partition-size": "32",
            "max-partition-size": "32",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "1",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def filter_intra_i444_mode3_noise():
        """Generate the I444 R16x32 following-leaf filter-intra witness."""

        random_state = random.Random(211)
        pixels = bytes(
            component
            for _ in range(32 * 32)
            for component in (random_state.randrange(256) for _ in range(3))
        )
        return Image.frombytes("RGB", (32, 32), pixels)

    write_campaign_image(
        "coverage_i444_v16x32_following_filter_intra_mode3_01",
        filter_intra_i444_mode3_noise(),
        "4:4:4",
        advanced={
            "min-partition-size": "16",
            "max-partition-size": "32",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "1",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def filter_intra_420_v16x32_following_split_mode3_noise():
        """Generate the 4:2:0 right-hand V16x32 mode-3/TX16 witness."""

        random_state = random.Random(211)
        pixels = bytes(
            component
            for _ in range(32 * 32)
            for component in (random_state.randrange(256) for _ in range(3))
        )
        return Image.frombytes("RGB", (32, 32), pixels)

    write_campaign_image(
        "coverage_r16x32_following_filter_intra_split_mode3_01",
        filter_intra_420_v16x32_following_split_mode3_noise(),
        "4:2:0",
        advanced={
            "min-partition-size": "16",
            "max-partition-size": "32",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "1",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def filter_intra_420_v16x32_following_split_mode0_ramp_noise():
        """Generate the 4:2:0 right-hand V16x32 mode-0/ramp witness."""

        random_state = random.Random(307)
        right = bytes(random_state.randrange(256) for _ in range(16 * 32 * 3))
        pixels = bytearray()
        for y in range(32):
            for x in range(32):
                if x < 16:
                    base = 32 + ((7 * y + 3 * x) % 96)
                    pixels.extend(
                        (
                            clamp_channel(base + 18),
                            base,
                            clamp_channel(base - 18),
                        )
                    )
                else:
                    offset = (y * 16 + (x - 16)) * 3
                    pixels.extend(right[offset : offset + 3])
        return Image.frombytes("RGB", (32, 32), bytes(pixels))

    write_campaign_image(
        "coverage_r16x32_following_filter_intra_split_mode0_01",
        filter_intra_420_v16x32_following_split_mode0_ramp_noise(),
        "4:2:0",
        advanced={
            "min-partition-size": "16",
            "max-partition-size": "32",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "1",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def filter_intra_square16_mode0_noise():
        """Generate the origin Square16 filter-intra mode-0 witness."""

        random_state = random.Random(109)
        pixels = bytes(
            random_state.randrange(256) for _ in range(16 * 16 * 3)
        )
        return Image.frombytes("RGB", (16, 16), pixels)

    write_campaign_image(
        "coverage_square16_filter_intra_mode0_01",
        filter_intra_square16_mode0_noise(),
        "4:2:0",
        advanced={
            "min-partition-size": "16",
            "max-partition-size": "16",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "1",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def filter_intra_vertical8x16_mode0_noise():
        """Generate the origin Vertical8x16 filter-intra mode-0 witness."""

        random_state = random.Random(102)
        pixels = bytes(
            random_state.randrange(256) for _ in range(8 * 16 * 3)
        )
        return Image.frombytes("RGB", (8, 16), pixels)

    write_campaign_image(
        "coverage_vertical8x16_filter_intra_mode0_01",
        filter_intra_vertical8x16_mode0_noise(),
        "4:2:0",
        advanced={
            "min-partition-size": "8",
            "max-partition-size": "16",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "1",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def filter_intra_vertical8x16_mode1_noise():
        """Generate the origin Vertical8x16 filter-intra mode-1 witness."""

        random_state = random.Random(105)
        pixels = bytes(
            random_state.randrange(256) for _ in range(8 * 16 * 3)
        )
        return Image.frombytes("RGB", (8, 16), pixels)

    write_campaign_image(
        "coverage_vertical8x16_filter_intra_mode1_01",
        filter_intra_vertical8x16_mode1_noise(),
        "4:2:0",
        advanced={
            "min-partition-size": "8",
            "max-partition-size": "16",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "1",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def chroma_diagonal113_square8():
        """Generate the right-hand Square8 Diagonal113 witness."""

        seed = 310

        def pixel(x, y):
            phase = (x + y + seed % 7) % 16
            return (
                clamp_channel(24 + 15 * phase),
                clamp_channel(180 - 9 * phase),
                clamp_channel(230 - 11 * phase),
            )

        return image_from_pixels((16, 8), pixel)

    write_campaign_image(
        "coverage_square8_chroma_diagonal113_01",
        chroma_diagonal113_square8(),
        "4:2:0",
        advanced={
            "min-partition-size": "8",
            "max-partition-size": "8",
            "use-intra-dct-only": "0",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "1",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def luma_diagonal_down_right_square8():
        """Generate the promoted right-hand luma mode-4 witness."""

        family = 0
        candidate = 3
        seed = 4000 + 10 * family + candidate
        phase = (7 * family + 11 * candidate) % 16

        def left_signal(x, y):
            return ((17 * x + 31 * y + phase + seed) % 33) - 16

        def diagonal_signal(x, y):
            coordinate = x - y
            wrapped = (coordinate * (1 + family % 3) + phase) % 32
            return (wrapped - 16) * 2

        def yuv_to_rgb(y, u, v):
            du = u - 128
            dv = v - 128
            return (
                clamp_channel(y + (358 * dv + 128) // 256),
                clamp_channel(y - (88 * du + 183 * dv + 128) // 256),
                clamp_channel(y + (453 * du + 128) // 256),
            )

        def pixel(x, y):
            cx, cy = x // 2, y // 2
            edge = left_signal(7, y)
            if x < 8:
                luma = 128 + left_signal(x, y)
                chroma = ((3 * x + 5 * y + seed) % 7) - 3
                scale = 1
            else:
                luma = 128 + edge + diagonal_signal(x - 8, y)
                chroma = ((13 * cx + 17 * cy + phase + seed) % 17) - 8
                scale = 2 + (candidate % 3)
            u_delta = scale * chroma + ((cx + 2 * cy + family) % 3) - 1
            v_delta = scale * chroma + ((2 * cx + cy + candidate) % 3) - 1
            return yuv_to_rgb(luma, 128 + u_delta, 128 + v_delta)

        return image_from_pixels((16, 8), pixel)

    write_campaign_image(
        "coverage_square8_luma_diagonal_down_right_01",
        luma_diagonal_down_right_square8(),
        "4:2:0",
        advanced={
            "min-partition-size": "8",
            "max-partition-size": "8",
            "use-intra-dct-only": "0",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "1",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def luma_smooth_square8(family):
        """Generate one of the promoted right-hand luma smooth witnesses."""

        def pixel(x, y):
            if family == 0:
                luma = 32 + (191 * x // 15) + ((191 * y // 7) // 2)
            elif family == 3:
                luma = (191 * x // 15) + (191 * y // 7)
            elif family == 6:
                luma = 32 + ((11 * x + 2 * y) % 160)
            else:
                raise ValueError(f"unknown luma smooth family: {family}")
            luma = clamp_channel(luma)
            return (luma, luma, luma)

        return image_from_pixels((16, 8), pixel)

    smooth_luma_advanced = {
        "min-partition-size": "8",
        "max-partition-size": "8",
        "use-intra-dct-only": "1",
        "enable-filter-intra": "0",
        "enable-intra-edge-filter": "0",
        "enable-smooth-intra": "1",
        "enable-paeth-intra": "0",
        "enable-directional-intra": "0",
        "enable-cfl-intra": "0",
        "enable-cdef": "0",
        "enable-restoration": "0",
        "loopfilter-control": "0",
        "aq-mode": "0",
        "deltaq-mode": "0",
    }
    for name, family in (
        ("coverage_square8_luma_smooth_01", 0),
        ("coverage_square8_luma_smooth_vertical_01", 3),
        ("coverage_square8_luma_smooth_horizontal_01", 6),
    ):
        write_campaign_image(
            name,
            luma_smooth_square8(family),
            "4:2:0",
            advanced=smooth_luma_advanced,
            quality=76,
            speed=0,
        )

    def luma_diagonal45_square8():
        """Generate the promoted following-leaf luma mode-3 witness."""

        def pixel(x, y):
            luma = 120 if x < 8 else 120 + x - 8 + y
            return (luma, luma, luma)

        return image_from_pixels((16, 8), pixel)

    write_campaign_image(
        "coverage_square8_luma_diagonal45_01",
        luma_diagonal45_square8(),
        "4:2:0",
        advanced={
            "min-partition-size": "8",
            "max-partition-size": "8",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "1",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def chroma_diagonal45_angle51_square8():
        """Generate the promoted right-hand Square8 chroma mode-3 witness.

        This is the deterministic CD45-F05-N02 input from the 100-case
        input-only campaign. The coded predictor is nominal Diagonal45, with
        angle symbol 5 resolving to 51 degrees; the generator remains in YUV
        space so the opposing U/V signal stays independent of luma mode
        selection.
        """

        family = 4
        candidate = 2
        a, b, kind = (1, -1, 4)
        amplitude = 8 + 2 * (candidate % 8)
        phase = (candidate * 3 + a * 7 + b * 11) % 32

        def yuv_to_rgb(y, u, v):
            du = u - 128
            dv = v - 128
            return (
                clamp_channel(y + (358 * dv + 128) // 256),
                clamp_channel(y - (88 * du + 183 * dv + 128) // 256),
                clamp_channel(y + (453 * du + 128) // 256),
            )

        def pixel(x, y):
            cx, cy = x // 2, y // 2
            if cx < 4:
                chroma = (a * cx + b * cy + phase) % 16 - 8
            else:
                value = a * cx + b * cy + phase
                chroma = amplitude if value % 3 == 0 else -amplitude // 2
            return yuv_to_rgb(128, 128 + chroma, 128 - chroma)

        return image_from_pixels((16, 8), pixel)

    write_campaign_image(
        "coverage_square8_chroma_diagonal45_angle51_01",
        chroma_diagonal45_angle51_square8(),
        "4:2:0",
        advanced={
            "min-partition-size": "8",
            "max-partition-size": "8",
            "use-intra-dct-only": "0",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "1",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def chroma_smooth_horizontal_square16():
        """Generate the following Square16 SmoothHorizontal witness.

        This is the promoted SF16-F06-N01 input from the deterministic
        100-case search. Keep the generator algebra identical to the campaign
        so the committed fixture remains reproducible from its input-only
        provenance.
        """

        family = 5
        candidate = 1
        phase = (3 * family + candidate) % 8

        def row_signal(row):
            return (((row * 7 + phase) % 16) - 8) * (3 + candidate % 3)

        def yuv_to_rgb(y, u, v):
            du = u - 128
            dv = v - 128
            return (
                clamp_channel(y + (358 * dv + 128) // 256),
                clamp_channel(y - (88 * du + 183 * dv + 128) // 256),
                clamp_channel(y + (453 * du + 128) // 256),
            )

        def pixel(x, y):
            cx, cy = x // 2, y // 2
            left = row_signal(cy)
            if cx < 8:
                horizontal = (cx - 7) * (1 + family % 3)
                u_delta = left + horizontal
                v_delta = left + horizontal // 2
            else:
                step = cx - 8
                continuation = left + ((row_signal(0) - left) * step + 3) // 7
                ripple = ((cx + 2 * cy + candidate + family) % 3) - 1
                u_delta = continuation + ripple
                v_delta = continuation + ripple // 2
            luma = 128 + ((7 * x + 11 * y + candidate + family) % 7) - 3
            return yuv_to_rgb(luma, 128 + u_delta, 128 + v_delta)

        return image_from_pixels((32, 16), pixel)

    write_campaign_image(
        "coverage_square16_chroma_smooth_horizontal_01",
        chroma_smooth_horizontal_square16(),
        "4:2:0",
        advanced={
            "min-partition-size": "16",
            "max-partition-size": "16",
            "use-intra-dct-only": "0",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "1",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def chroma_diagonal157_vertical8x16():
        """Generate the following Vertical8x16 Diagonal157 witness."""

        family = 5
        candidate = 0
        seed = 1000 + 10 * family + candidate

        def yuv_to_rgb(y, u, v):
            du = u - 128
            dv = v - 128
            return (
                clamp_channel(y + (358 * dv + 128) // 256),
                clamp_channel(y - (88 * du + 183 * dv + 128) // 256),
                clamp_channel(y + (453 * du + 128) // 256),
            )

        def pixel(x, y):
            cx, cy = x // 2, y // 2
            phase = (11 * candidate + 7 * family + 3) % 32
            amplitude = 16 + (candidate % 5) * 3
            coordinate = 5 * cx - 2 * cy + phase
            wrapped = coordinate % 32
            wave = wrapped - 16
            chroma = (wave * amplitude) // 16
            if cx >= 4:
                chroma *= 2
            u_delta = chroma + ((37 * cx + 19 * cy + seed) % 121) - 60
            v_delta = chroma + ((23 * cx + 47 * cy + 3 * seed) % 121) - 60
            luma = 128
            if x >= 8 and ((cx + cy + seed) % 2):
                luma += 14
            return yuv_to_rgb(luma, 128 + u_delta, 128 + v_delta)

        return image_from_pixels((16, 16), pixel)

    write_campaign_image(
        "coverage_vertical8x16_chroma_diagonal157_01",
        chroma_diagonal157_vertical8x16(),
        "4:2:0",
        advanced={
            "min-partition-size": "8",
            "max-partition-size": "16",
            "use-intra-dct-only": "0",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "1",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def chroma_horizontal_vertical8x16():
        """Generate the qualified following Horizontal witness."""

        family = 3
        candidate = 6
        amplitude = 18 + (candidate % 5) * 3
        phase = (3 * candidate + 5 * family) % 8

        def deltas(cx, cy):
            row = cy + phase
            base = (row - 3) * (amplitude // 3)
            scale = 1 if cx < 4 else 2
            ripple = (cx - 3) if cx >= 4 else (cx - 3)
            u_delta = scale * base + ripple
            v_delta = scale * base + ((2 * cx + candidate) % 5) - 2
            return u_delta, v_delta

        def yuv_to_rgb(y, u, v):
            du = u - 128
            dv = v - 128
            return (
                clamp_channel(y + (358 * dv + 128) // 256),
                clamp_channel(y - (88 * du + 183 * dv + 128) // 256),
                clamp_channel(y + (453 * du + 128) // 256),
            )

        def pixel(x, y):
            u_delta, v_delta = deltas(x // 2, y // 2)
            return yuv_to_rgb(128, 128 + u_delta, 128 + v_delta)

        return image_from_pixels((16, 16), pixel)

    write_campaign_image(
        "coverage_vertical8x16_chroma_horizontal_01",
        chroma_horizontal_vertical8x16(),
        "4:2:0",
        advanced={
            "min-partition-size": "8",
            "max-partition-size": "16",
            "use-intra-dct-only": "0",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "1",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def chroma_paeth_vertical8x16(family, candidate):
        """Generate one exact following-leaf chroma-Paeth witness."""

        seed = 1000 + 10 * family + candidate
        amplitude = 24 + 4 * (candidate % 5)
        phase = (candidate + 2 * family) % 8
        epsilon = 2 + candidate % 4

        def deltas(cx, cy):
            row = cy + phase
            if family == 0:
                base = amplitude if row % 2 == 0 else -amplitude
            elif family == 1:
                base = amplitude if (row // 2) % 2 == 0 else -amplitude
            elif family == 2:
                base = (row - 3) * (amplitude // 3)
            elif family == 3:
                base = (row % 4 - 1) * (amplitude // 2)
            elif family == 4:
                base = (4 - abs((row % 8) - 4)) * (amplitude // 4)
            elif family == 5:
                base = amplitude if row >= 4 + candidate % 3 else -amplitude
            elif family == 6:
                base = (
                    amplitude
                    if row in {2 + candidate % 2, 6 + candidate % 2}
                    else -amplitude // 2
                )
            elif family == 7:
                base = amplitude if row == 3 + candidate % 3 else -amplitude // 3
            elif family == 8:
                base = amplitude if (row + phase) % 3 == 0 else -amplitude // 2
            else:
                base = ((row * 3 + phase) % 9 - 4) * (amplitude // 4)
            horizontal = (cx - 3) * epsilon
            if family in {2, 4, 8}:
                return (
                    base + horizontal,
                    -base + (cx + cy + candidate) % 3 - 1,
                )
            if family in {5, 6}:
                return base + horizontal, base - horizontal
            return base + horizontal, base + ((2 * cx + candidate) % 5) - 2

        def pixel(x, y):
            cx, cy = x // 2, y // 2
            u_delta, v_delta = deltas(cx, cy)
            luma = 128
            if family in (3, 6, 8, 9):
                luma += ((5 * x + 3 * y + seed) % 9) - 4
            if x >= 8:
                luma += 9 if (x // 2 + y // 2 + seed) % 2 else -9
            du = u_delta
            dv = v_delta
            return (
                clamp_channel(luma + (358 * dv + 128) // 256),
                clamp_channel(luma - (88 * du + 183 * dv + 128) // 256),
                clamp_channel(luma + (453 * du + 128) // 256),
            )

        return image_from_pixels((16, 16), pixel)

    paeth_advanced = {
        "min-partition-size": "8",
        "max-partition-size": "16",
        "use-intra-dct-only": "0",
        "enable-filter-intra": "0",
        "enable-intra-edge-filter": "0",
        "enable-smooth-intra": "0",
        "enable-paeth-intra": "1",
        "enable-directional-intra": "0",
        "enable-cfl-intra": "0",
        "enable-cdef": "0",
        "enable-restoration": "0",
        "loopfilter-control": "0",
        "aq-mode": "0",
        "deltaq-mode": "0",
    }
    for name, family, candidate in (
        ("coverage_vertical8x16_chroma_paeth_01", 2, 9),
        ("coverage_vertical8x16_chroma_paeth_02", 8, 0),
        ("coverage_vertical8x16_chroma_paeth_03", 9, 2),
    ):
        write_campaign_image(
            name,
            chroma_paeth_vertical8x16(family, candidate),
            "4:2:0",
            advanced=paeth_advanced,
            quality=76,
            speed=0,
        )

    def horizontal_r32x8_ripple():
        """Generate a deterministic PARTITION_H4 32x8-transform witness."""

        def pixel(x, y):
            band = min(3, y // 8)
            base = (48, 104, 160, 216)[band]
            ripple = ((13 * x + 17 * y + x * y) % 31) - 15
            return (
                clamp_channel(base + ripple + (8 if (x + y) % 3 else -8)),
                clamp_channel(base + ripple),
                clamp_channel(base - ripple),
            )

        return image_from_pixels((32, 32), pixel)

    write_campaign_image(
        "coverage_r32x8_h4_ripple_01",
        horizontal_r32x8_ripple(),
        "4:2:0",
        advanced={
            "min-partition-size": "8",
            "max-partition-size": "32",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def horizontal_r32x8_filter_intra_cdf9_false():
        """Generate the pinned H32x8 false-filter CDF-index-9 witness.

        This is the exact F10/N05 candidate from the bounded input-only
        campaign (seed 7095), including its deterministic in-band noise.
        """

        random_state = random.Random(7095)

        def pixel(x, y):
            band = min(3, y // 8)
            base = (44, 100, 156, 212)[band]
            sample = random_state.randrange(-12, 13)
            ripple = ((13 * x + 17 * y + x * y + 16) % 31) - 15
            return (
                clamp_channel(base + ripple + ripple + sample // 2),
                clamp_channel(base + sample // 3 + sample),
                clamp_channel(base - ripple - ripple),
            )

        return image_from_pixels((32, 32), pixel)

    write_campaign_image(
        "coverage_r32x8_filter_intra_cdf9_false_01",
        horizontal_r32x8_filter_intra_cdf9_false(),
        "4:2:0",
        advanced={
            "min-partition-size": "8",
            "max-partition-size": "32",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "1",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def horizontal_h64x16_ramp():
        """Generate a 64x64 PARTITION_H4 frame with four H64x16 leaves."""

        bands = (
            (17, 91, 203),
            (32, 32, 32),
            (0, 255, 0),
            (127, 127, 127),
        )

        return image_from_pixels(
            (64, 64), lambda _x, y: bands[min(3, y // 4)]
        )

    write_campaign_image(
        "coverage_h64x16_horizontal_ramp_01",
        horizontal_h64x16_ramp(),
        "4:2:0",
        advanced={
            "min-partition-size": "16",
            "max-partition-size": "64",
            "use-intra-dct-only": "1",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-smooth-intra": "0",
            "enable-paeth-intra": "0",
            "enable-directional-intra": "0",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=76,
        speed=0,
    )

    def vertical_transform_grid_mosaic(index):
        bands = (44, 100, 156, 212)

        def pixel(x, y):
            band = min(3, y // 16)
            luma = bands[band] + ((x * 17 + y * 13 + index) % 17) - 8
            chroma_delta = 4 if ((x + y + index) % 2) else -4
            return (
                clamp_channel(luma + chroma_delta),
                clamp_channel(luma),
                clamp_channel(luma - chroma_delta),
            )

        return image_from_pixels((16, 64), pixel)

    write_campaign_family(
        "coverage_r16x64_grid",
        10,
        vertical_transform_grid_mosaic,
        "4:2:0",
        advanced={"enable-filter-intra": "0", "enable-restoration": "0"},
        quality=76,
        speed=0,
    )

    def full_chroma_square(index):
        def pixel(x, y):
            if index == 0:
                return (clamp_channel(96 + 4 * x), 128, 128)
            if index == 1:
                return (128, clamp_channel(96 + 4 * y), 128)
            if index == 2:
                return (128, 128, clamp_channel(96 + 2 * (x + y)))
            if index == 3:
                return (
                    clamp_channel(96 + 2 * (15 - x + y)),
                    clamp_channel(96 + 2 * (x + 15 - y)),
                    128,
                )
            if index == 4:
                value = 104 if ((x // 4) + (y // 4)) % 2 else 152
                return (value, 128, 160 - value // 4)
            if index == 5:
                inside = 4 <= x < 12 and 4 <= y < 12
                return (152 if inside else 104, 128, 128)
            if index == 6:
                return (152, 104, 128) if y < 4 else (104, 152, 128)
            if index == 7:
                return (152, 128, 104) if x < 4 else (104, 128, 152)
            if index == 8:
                cross = x in range(6, 10) or y in range(6, 10)
                return (152, 104, 152) if cross else (104, 152, 104)
            quadrant = (2 if y >= 8 else 0) + (1 if x >= 8 else 0)
            return ((104, 128, 152), (152, 104, 128), (128, 152, 104), (152, 152, 104))[quadrant]

        return image_from_pixels((16, 16), pixel)

    write_campaign_family(
        "coverage_i444_square8", 10, full_chroma_square, "4:4:4"
    )

    def full_chroma_rect(index):
        geometries = (
            ((16, 16), "vertical"),
            ((16, 16), "horizontal"),
            ((16, 24), "diagonal"),
            ((16, 24), "checker"),
            ((16, 32), "vertical"),
            ((24, 16), "horizontal"),
            ((24, 16), "diagonal"),
            ((32, 16), "checker"),
            ((8, 32), "vertical"),
            ((32, 8), "horizontal"),
        )
        size, pattern = geometries[index]
        width, height = size

        def pixel(x, y):
            if pattern == "vertical":
                phase = x * 40 // max(1, width - 1) - 20
            elif pattern == "horizontal":
                phase = y * 40 // max(1, height - 1) - 20
            elif pattern == "diagonal":
                phase = (x + y) * 40 // max(1, width + height - 2) - 20
            else:
                phase = 20 if ((x // 4) + (y // 4)) % 2 else -20
            return (
                clamp_channel(128 + phase),
                clamp_channel(128 - phase),
                clamp_channel(128 + (phase // 2)),
            )

        return image_from_pixels(size, pixel)

    write_campaign_family("coverage_i444_rect", 10, full_chroma_rect, "4:4:4")

    def entropy_mosaic(index):
        rectangles = (
            (4, 4, 8, 16),
            (8, 4, 16, 8),
            (12, 4, 8, 16),
            (4, 8, 16, 8),
            (8, 8, 8, 16),
            (16, 8, 8, 16),
            (8, 12, 16, 8),
            (4, 16, 16, 8),
            (12, 16, 8, 12),
            (16, 16, 12, 8),
        )
        x0, y0, width, height = rectangles[index]
        second = (
            (x0, min(31, y0 + height), width, min(32 - (y0 + height), height))
            if index % 2 == 0
            else (min(31, x0 + width), y0, min(32 - (x0 + width), width), height)
        )
        x1, y1, width1, height1 = second

        def inside(x, y, left, top, rect_width, rect_height):
            return left <= x < left + rect_width and top <= y < top + rect_height

        def pixel(x, y):
            rgb = [120, 128, 136]
            if inside(x, y, x0, y0, width, height):
                for channel, delta in enumerate((8, -5, 6)):
                    rgb[channel] += delta
            if inside(x, y, x1, y1, width1, height1):
                for channel, delta in enumerate((-4, 7, -3)):
                    rgb[channel] += delta
            return tuple(clamp_channel(value) for value in rgb)

        return image_from_pixels((32, 32), pixel)

    write_campaign_family(
        "coverage_entropy_mosaic", 10, entropy_mosaic, "4:2:0"
    )

    def public_adst(index):
        width, height = (
            (4, 8),
            (8, 4),
            (4, 16),
            (16, 4),
            (8, 16),
            (16, 8),
            (8, 32),
            (32, 8),
            (16, 16),
            (32, 16),
        )[index]
        denominator = max(1, width + 2 * height - 3)
        transposed_denominator = max(1, height + 2 * width - 3)

        def pixel(x, y):
            red = 112 + (32 * (x + 2 * y)) // denominator
            green = 112 + (32 * (y + 2 * x)) // transposed_denominator
            blue = 255 - red
            return tuple(clamp_channel(value) for value in (red, green, blue))

        return image_from_pixels((width, height), pixel)

    write_campaign_family("coverage_adst_public", 10, public_adst, "4:4:4")

    # Independent topology witness: the pinned dav1d trace proves that these
    # options produce one 16x16 superblock split into four terminal 8x8
    # 4:4:4 leaves. Keep the input structurally distinct from the broad
    # candidate families so its public parity row proves the positioned full-
    # chroma path rather than relying on a color or filename special case.
    partitioned_full_chroma = image_from_pixels(
        (16, 16),
        lambda x, y: (
            ((17, 91, 203), (32, 32, 32), (0, 255, 0), (127, 127, 127))[
                int(y >= 8) * 2 + int(x >= 8)
            ]
        ),
    )
    write_campaign_image(
        "coverage_i444_square8_four_leaves",
        partitioned_full_chroma,
        "4:4:4",
        advanced={"min-partition-size": "8", "max-partition-size": "8"},
    )
    print("  AVIF coverage campaign: wrote 100 candidates and one topology witness")

    write_portable_lossless("portable_lossless_a.avif", (17, 91, 203))
    write_portable_lossless("portable_lossless_b.avif", (199, 37, 83))
    write_portable_lossless(
        "portable_lossless_420_a.avif",
        (17, 91, 203),
        subsampling="4:2:0",
    )
    write_portable_lossless(
        "portable_lossless_420_b.avif",
        (199, 37, 83),
        subsampling="4:2:0",
    )
    write_portable_lossless(
        "portable_lossless_420_8x8_a.avif",
        (17, 91, 203),
        size=(8, 8),
        subsampling="4:2:0",
    )
    write_portable_lossless(
        "portable_lossless_420_8x8_b.avif",
        (199, 37, 83),
        size=(8, 8),
        subsampling="4:2:0",
    )
    empty_tile_source = d / "portable_lossless_420_8x8_a.avif"
    empty_tile_payload = bytearray(empty_tile_source.read_bytes())
    if hashlib.sha256(empty_tile_payload).hexdigest() != (
        "21d453da436be1bbb47238e35d919499c7814a2a8073550b9ae958cafe78d15e"
    ):
        raise RuntimeError("empty-tile AVIF source differs from its pinned fixture")
    extent = (275).to_bytes(4, "big") + (32).to_bytes(4, "big")
    if empty_tile_payload.count(extent) != 1:
        raise RuntimeError("empty-tile AVIF item extent moved")
    extent_offset = empty_tile_payload.index(extent)
    empty_tile_payload[extent_offset + 4 : extent_offset + 8] = (17).to_bytes(
        4, "big"
    )
    if empty_tile_payload[287:289] != b"\x32\x12":
        raise RuntimeError("empty-tile AVIF frame OBU moved")
    # Keep the complete frame header and tile-group header while ending the
    # item extent exactly where the first tile entropy payload would begin.
    empty_tile_payload[288] = 3
    if hashlib.sha256(empty_tile_payload).hexdigest() != (
        "03203e35905a79b9556e19f2e3925abc1c1f7541c431eee556637d59cfee1f52"
    ):
        raise RuntimeError("empty-tile AVIF mutation differs from its pinned hash")
    (d / "empty_tile_payload.avif").write_bytes(empty_tile_payload)
    for gray in (0, 64, 126, 127, 129, 130, 192, 255):
        write_portable(
            f"portable_lossy_420_q99_gray_{gray}.avif",
            (gray, gray, gray),
            quality=99,
            subsampling="4:2:0",
        )
        write_portable(
            f"portable_lossy_420_q99_8x8_gray_{gray}.avif",
            (gray, gray, gray),
            size=(8, 8),
            quality=99,
            subsampling="4:2:0",
        )
    for gray in (122, 123, 124, 125, 128, 131, 132, 133, 134):
        write_portable(
            f"portable_lossy_420_q99_gray_{gray}_control.avif",
            (gray, gray, gray),
            quality=99,
            subsampling="4:2:0",
        )
    for gray in (122, 123, 124, 125, 131, 132, 133, 134):
        write_portable(
            f"portable_lossy_420_q99_8x8_gray_{gray}_control.avif",
            (gray, gray, gray),
            size=(8, 8),
            quality=99,
            subsampling="4:2:0",
        )

    # These are the first independent legal AC coefficient classes admitted
    # by the portable decoder. Keep the source patterns deliberately simple:
    # they exercise EOB-bin five, EOB-bin six, and the 8x8 moving level-context
    # plane without relying on a payload mutation or a native decoder.
    write_portable_luma_pattern(
        "portable_lossy_420_q99_rampx_eob5.avif",
        (4, 4),
        lambda x, _y: 96 + 8 * x,
    )
    write_portable_luma_pattern(
        "portable_lossy_420_q99_rampy_eob6.avif",
        (4, 4),
        lambda _x, y: 96 + 8 * y,
    )
    write_portable_luma_pattern(
        "portable_lossy_420_q99_8x8_diag_eob6.avif",
        (8, 8),
        lambda x, y: 129 if x == y else 127,
    )

    token_boundary_source = d / "portable_lossy_420_q99_gray_0.avif"
    token_boundary_bytes = bytearray(token_boundary_source.read_bytes())
    if hashlib.sha256(token_boundary_bytes).hexdigest() != (
        "7f1485129fd93e4318cf21bcf59934963c1a84b3bcb0d74f3e7555b3bad20b38"
    ):
        raise RuntimeError("Slice 39 token-boundary source differs")
    if token_boundary_bytes[303] != 0x42:
        raise RuntimeError("Slice 39 token-boundary source byte differs")
    token_boundary_bytes[303] = 0x43
    if hashlib.sha256(token_boundary_bytes).hexdigest() != (
        "1097067dca85e499768a40e15232dce3602afbb1cabcbf485e8a14bf83e9bb73"
    ):
        raise RuntimeError("Slice 39 token-boundary mutation differs")
    (d / "portable_lossy_420_q99_token_1048_control.avif").write_bytes(
        token_boundary_bytes
    )

    def write_slice40_token_fixture(name, replacements, expected_sha256, suffix=b""):
        mutated = bytearray(token_boundary_source.read_bytes())
        for offset, old, new in replacements:
            if mutated[offset] != old:
                raise RuntimeError(
                    f"Slice 40 mutation source byte differs at {offset}"
                )
            mutated[offset] = new
        mutated.extend(suffix)
        if hashlib.sha256(mutated).hexdigest() != expected_sha256:
            raise RuntimeError(f"Slice 40 mutation {name} differs from its pinned hash")
        (d / name).write_bytes(mutated)

    for name, replacements, expected_sha256, suffix in (
        (
            "portable_lossy_420_q99_token_2061.avif",
            (
                (301, 0x9E, 0x7E),
                (302, 0xBF, 0xEB),
                (303, 0x42, 0x40),
            ),
            "bc97b1f2ca96f6072239101e096e1b18fe87cb6ecf13b48188b37b52a50d761e",
            b"",
        ),
        (
            "portable_lossy_420_q99_token_2988.avif",
            (
                (301, 0x9E, 0x7E),
                (302, 0xBF, 0xE5),
                (303, 0x42, 0xFF),
                (304, 0x40, 0x10),
            ),
            "0153d56609f86e637159836af94d103523853c9002c92dc7411925d97a919250",
            b"",
        ),
        (
            "portable_lossy_420_q99_token_7940.avif",
            (
                (301, 0x9E, 0x7E),
                (302, 0xBF, 0xE4),
                (303, 0x42, 0xFF),
                (304, 0x40, 0x04),
            ),
            "503ca52689395ec769b5453f7a30b4340f4234132338b1dd16e6a945ab34c37a",
            b"",
        ),
        (
            "portable_lossy_420_q99_token_7764.avif",
            (
                (302, 0xBF, 0xBC),
                (303, 0x42, 0xFF),
                (304, 0x40, 0x04),
            ),
            "15822dfb32fea6432adf1c7ddb9ea648dd6d2e028b12c9f117c6031420760367",
            b"",
        ),
        (
            "portable_lossy_420_q99_token_2097724_masked_572.avif",
            (
                (120, 0x1E, 0x22),
                (270, 0x26, 0x2A),
                (288, 0x10, 0x14),
                (301, 0x9E, 0x7E),
                (302, 0xBF, 0xE3),
                (303, 0x42, 0x00),
                (304, 0x40, 0x84),
            ),
            "d492c364655cad1f950bd37fbf63b1b9eecc42dff0bae3f95d2d15d8f0f86f63",
            b"\x11\x00\x00\x00",
        ),
    ):
        write_slice40_token_fixture(name, replacements, expected_sha256, suffix)

    mutation_source = d / "portable_lossy_420_q99_gray_126.avif"
    mutation_source_bytes = mutation_source.read_bytes()
    mutation_source_sha256 = hashlib.sha256(mutation_source_bytes).hexdigest()
    if mutation_source_sha256 != (
        "f82b264295ffb7ea9e357a352e674200ed89138a182b0de7c4002fbc55fade4d"
    ):
        raise RuntimeError("Slice 35 mutation source differs from the pinned fixture")
    for name, offset, old, new, expected_sha256 in (
        (
            "portable_lossy_420_q99_eob_bin_control.avif",
            299,
            0x72,
            0x73,
            "0ff53f82624ab0c9e213a7398251aef6d14af7a91ca3a31ba757d1fe36f8cdea",
        ),
        (
            "portable_lossy_420_q99_eob_base_control.avif",
            300,
            0xE1,
            0x1E,
            "ebf00b9dc914982bd698af0413a0e26a6a849208871abbeccc6789541efb08f5",
        ),
    ):
        mutated = bytearray(mutation_source_bytes)
        if mutated[offset] != old:
            raise RuntimeError(f"Slice 35 mutation source byte differs at {offset}")
        mutated[offset] = new
        if hashlib.sha256(mutated).hexdigest() != expected_sha256:
            raise RuntimeError(f"Slice 35 mutation {name} differs from its pinned hash")
        (d / name).write_bytes(mutated)
    for geometry, size in (
        ("4x8", (4, 8)),
        ("8x4", (8, 4)),
    ):
        write_portable_lossless(
            f"portable_lossless_420_leaf_{geometry}_a.avif",
            (17, 91, 203),
            size=size,
            subsampling="4:2:0",
        )
    for geometry, size in (
        ("12x4", (12, 4)),
        ("16x4", (16, 4)),
        ("12x8", (12, 8)),
        ("16x8", (16, 8)),
        ("4x12", (4, 12)),
        ("4x16", (4, 16)),
        ("8x12", (8, 12)),
        ("8x16", (8, 16)),
    ):
        write_portable_lossless(
            f"portable_lossless_420_rect_{geometry}_gray_127.avif",
            (127, 127, 127),
            size=size,
            subsampling="4:2:0",
        )
        write_portable_lossless(
            f"portable_lossless_420_split_{geometry}_a.avif",
            (17, 91, 203),
            size=size,
            subsampling="4:2:0",
        )
    for geometry, size in (
        ("12x12", (12, 12)),
        ("12x16", (12, 16)),
        ("16x12", (16, 12)),
        ("16x16", (16, 16)),
    ):
        write_portable_lossless(
            f"portable_lossless_420_square_{geometry}_a.avif",
            (17, 91, 203),
            size=size,
            subsampling="4:2:0",
        )
    write_square_partition(
        "partitioned_square_420_16x16_rgb_delta.avif",
        (22, 96, 208),
        subsampling="4:2:0",
    )
    write_square_partition(
        "partitioned_square_420_16x16_g96.avif",
        (17, 96, 203),
        subsampling="4:2:0",
    )
    write_portable_lossless("portable_lossless_gray_32.avif", (32, 32, 32))
    write_portable_lossless("portable_lossless_gray_127.avif", (127, 127, 127))
    write_portable_lossless("portable_probe_gray_128.avif", (128, 128, 128))
    write_portable_lossless("portable_probe_gray_129.avif", (129, 129, 129))
    write_portable_lossless(
        "portable_lossless_8x8_a.avif", (17, 91, 203), size=(8, 8)
    )
    write_portable_lossless(
        "portable_lossless_8x8_gray_127.avif", (127, 127, 127), size=(8, 8)
    )
    write_portable_lossless(
        "portable_probe_8x8_gray_128.avif", (128, 128, 128), size=(8, 8)
    )
    write_portable_lossless(
        "portable_probe_8x8_gray_129.avif", (129, 129, 129), size=(8, 8)
    )
    for orientation, size in (("4x8", (4, 8)), ("8x4", (8, 4))):
        write_portable_lossless(
            f"portable_lossless_{orientation}_a.avif",
            (17, 91, 203),
            size=size,
        )
        write_portable_lossless(
            f"portable_lossless_{orientation}_gray_127.avif",
            (127, 127, 127),
            size=size,
        )
        write_portable_lossless(
            f"portable_probe_{orientation}_gray_128.avif",
            (128, 128, 128),
            size=size,
        )
        write_portable_lossless(
            f"portable_probe_{orientation}_gray_129.avif",
            (129, 129, 129),
            size=size,
        )
    for dimension in (12, 16):
        geometry = f"{dimension}x{dimension}"
        size = (dimension, dimension)
        write_portable_lossless(
            f"portable_lossless_{geometry}_a.avif",
            (17, 91, 203),
            size=size,
        )
        write_portable_lossless(
            f"portable_lossless_{geometry}_gray_127.avif",
            (127, 127, 127),
            size=size,
        )
        write_portable_lossless(
            f"portable_probe_{geometry}_gray_128.avif",
            (128, 128, 128),
            size=size,
        )
        write_portable_lossless(
            f"portable_probe_{geometry}_gray_129.avif",
            (129, 129, 129),
            size=size,
        )
    write_square_partition(
        "partitioned_square_12x12_g96_direct_tokens.avif",
        (17, 96, 203),
        size=(12, 12),
    )
    write_square_partition(
        "partitioned_square_12x12_midpoint_g96_ac.avif",
        (17, 96, 203),
        size=(12, 12),
        replacement_origin=(6, 6),
    )
    write_square_partition(
        "partitioned_square_12x12_top_left_luma_eob4.avif",
        (22, 96, 208),
        size=(12, 12),
        replacement_origin=(6, 6),
    )
    write_square_partition(
        "partitioned_square_12x12_top_left_luma_eob12_control.avif",
        (22, 96, 208),
        size=(12, 12),
        replacement_origin=(7, 6),
    )
    write_square_partition(
        "partitioned_square_12x12_luma_eob1.avif",
        (22, 96, 208),
        size=(12, 12),
        replacement_origin=(10, 8),
    )
    write_square_partition(
        "partitioned_square_12x12_luma_eob2_control.avif",
        (22, 96, 208),
        size=(12, 12),
        replacement_origin=(8, 10),
    )
    write_square_partition(
        "partitioned_square_12x12_luma_eob4_control.avif",
        (22, 96, 208),
        size=(12, 12),
        replacement_origin=(10, 10),
    )
    write_square_partition(
        "partitioned_square_12x12_luma_eob6_control.avif",
        (22, 96, 208),
        size=(12, 12),
        replacement_origin=(9, 8),
    )
    write_square_partition(
        "partitioned_square_12x12_luma_eob9_control.avif",
        (22, 96, 208),
        size=(12, 12),
        replacement_origin=(8, 9),
    )
    write_square_partition(
        "partitioned_square_12x12_luma_eob10_control.avif",
        (22, 96, 208),
        size=(12, 12),
        replacement_origin=(10, 9),
    )
    write_square_partition(
        "partitioned_square_12x12_luma_eob12_control.avif",
        (22, 96, 208),
        size=(12, 12),
        replacement_origin=(9, 10),
    )
    write_square_partition(
        "partitioned_square_12x12_luma_eob15_control.avif",
        (22, 96, 208),
        size=(12, 12),
        replacement_origin=(9, 9),
    )
    write_square_partition("partitioned_square_16x16_g64.avif", (17, 64, 203))
    write_square_partition(
        "partitioned_square_16x16_g96_direct_tokens.avif",
        (17, 96, 203),
    )
    write_square_partition("partitioned_square_16x16_r64.avif", (64, 91, 203))
    write_square_partition("partitioned_square_16x16_g127.avif", (17, 127, 203))
    for orientation, size in (("12x16", (12, 16)), ("16x12", (16, 12))):
        write_portable_lossless(
            f"portable_lossless_{orientation}_a.avif",
            (17, 91, 203),
            size=size,
        )
        write_portable_lossless(
            f"portable_lossless_{orientation}_gray_127.avif",
            (127, 127, 127),
            size=size,
        )
        write_portable_lossless(
            f"portable_probe_{orientation}_gray_128.avif",
            (128, 128, 128),
            size=size,
        )
        write_portable_lossless(
            f"portable_probe_{orientation}_gray_129.avif",
            (129, 129, 129),
            size=size,
        )
    for orientation, size in (
        ("4x12", (4, 12)),
        ("12x4", (12, 4)),
        ("4x16", (4, 16)),
        ("16x4", (16, 4)),
        ("8x12", (8, 12)),
        ("12x8", (12, 8)),
        ("8x16", (8, 16)),
        ("16x8", (16, 8)),
    ):
        write_portable_lossless(
            f"partitioned_{orientation}_a.avif",
            (17, 91, 203),
            size=size,
        )
        if orientation in {"4x12", "12x4"}:
            write_portable_lossless(
                f"partitioned_{orientation}_gray_127.avif",
                (127, 127, 127),
                size=size,
            )
        write_portable_lossless(
            f"partitioned_{orientation}_gray_32.avif",
            (32, 32, 32),
            size=size,
        )
        write_portable_lossless(
            f"partitioned_{orientation}_green.avif",
            (0, 255, 0),
            size=size,
        )
    for orientation, size in (
        ("12x4", (12, 4)),
        ("12x8", (12, 8)),
        ("16x4", (16, 4)),
        ("16x8", (16, 8)),
        ("4x12", (4, 12)),
        ("8x12", (8, 12)),
        ("4x16", (4, 16)),
        ("8x16", (8, 16)),
    ):
        for gray in (128, 129):
            write_portable_lossless(
                f"portable_rect_{orientation}_gray_{gray}.avif",
                (gray, gray, gray),
                size=size,
            )
        if orientation not in {"12x4", "4x12"}:
            write_portable_lossless(
                f"portable_rect_{orientation}_gray_127.avif",
                (127, 127, 127),
                size=size,
            )
    for orientation, size in (("12x4", (12, 4)), ("4x12", (4, 12))):
        write_portable_lossless(
            f"portable_rect_{orientation}_a_speed0.avif",
            (17, 91, 203),
            size=size,
            speed=0,
        )
        write_portable_lossless(
            f"portable_rect_{orientation}_gray_32_speed0.avif",
            (32, 32, 32),
            size=size,
            speed=0,
        )

    multitile_path = d / "multitile.avif"
    pattern_img("RGB", (256, 128)).save(
        multitile_path,
        format="AVIF",
        quality=75,
        speed=6,
        max_threads=1,
        tile_cols=1,
    )
    from inspect_av1_obus import inspect as inspect_av1

    report = inspect_av1(multitile_path)
    size_field = next(
        tile["size_field"]
        for sample in report["samples"]
        for obu in sample["obus"]
        for tile in obu.get("tile_group", {}).get("tiles", [])
        if tile["size_field"] is not None
    )
    size_spans = size_field["physical_spans"]
    if len(size_spans) != 1 or size_spans[0]["length"] < 2:
        raise RuntimeError("generated AVIF does not have one two-byte tile size field")
    malformed = bytearray(multitile_path.read_bytes())
    # Change only the most-significant byte of tile_size_minus_1. The resulting
    # first tile crosses the frame OBU payload while the container and frame
    # header remain intact.
    malformed[size_spans[0]["offset"] + size_spans[0]["length"] - 1] = 0xFF
    (d / "invalid_tile_size.avif").write_bytes(malformed)

    baseline_path = d / "baseline.avif"
    baseline = bytearray(baseline_path.read_bytes())
    for brand in (b"mif1", b"msf1"):
        accepted_major_brand = bytearray(baseline)
        if accepted_major_brand[4:8] != b"ftyp":
            raise RuntimeError("baseline AVIF must begin with an ftyp box")
        accepted_major_brand[8:12] = brand
        (d / f"major_brand_{brand.decode('ascii')}.avif").write_bytes(accepted_major_brand)
    late_compatible_brand = bytearray(baseline)
    late_compatible_brand[8:12] = b"mif1"
    late_compatible_brand[16:20] = b"mif1"
    late_compatible_brand[20:24] = b"avif"
    (d / "major_brand_mif1_late_avif.avif").write_bytes(late_compatible_brand)
    for brand in (b"mif1", b"msf1"):
        generic_brand = bytearray(baseline)
        generic_brand[8:12] = brand
        generic_brand[16:20] = brand
        (d / f"generic_{brand.decode('ascii')}.avif").write_bytes(generic_brand)
    no_compatible_brands = bytearray(baseline[:16] + baseline[32:])
    no_compatible_brands[:4] = (16).to_bytes(4, "big")
    no_compatible_brands[8:12] = b"mif1"
    (d / "generic_mif1_no_compatible_brands.avif").write_bytes(
        no_compatible_brands
    )
    malformed_size = bytearray(baseline)
    malformed_size[:4] = (31).to_bytes(4, "big")
    malformed_size[8:12] = b"mif1"
    (d / "malformed_mif1_ftyp_size.avif").write_bytes(malformed_size)
    oversized_box = bytearray(baseline)
    oversized_box[:4] = (len(oversized_box) + 4).to_bytes(4, "big")
    oversized_box[8:12] = b"mif1"
    (d / "oversized_mif1_ftyp.avif").write_bytes(oversized_box)
    unsupported_major_brand = bytearray(baseline)
    if unsupported_major_brand[4:8] != b"ftyp":
        raise RuntimeError("baseline AVIF must begin with an ftyp box")
    unsupported_major_brand[8:12] = b"heic"
    unsupported_major_brand[16:20] = b"heic"
    (d / "unsupported_major_brand.avif").write_bytes(unsupported_major_brand)
    non_image_bmff = bytearray(baseline)
    non_image_bmff[8:12] = b"isom"
    non_image_bmff[16:32] = b"isomiso2mp41av01"
    (d / "non_image_isom_bmff.avif").write_bytes(non_image_bmff)
    sequence_marker = bytes.fromhex("0a091819bfff6880868342")
    sequence_offset = baseline.index(sequence_marker) + 2
    baseline[sequence_offset] = (baseline[sequence_offset] & 0x1f) | 0xe0
    (d / "invalid_sequence_profile.avif").write_bytes(baseline)
    print(
        "  AVIF: wrote portable lossless/lossy, multi-tile success/error, "
        "and existing error fixtures"
    )


def main():
    generators = {
        "jpeg": gen_jpeg,
        "png": gen_png,
        "gif": gen_gif,
        "bmp": gen_bmp,
        "webp": gen_webp,
        "tiff": gen_tiff,
        "ico": gen_ico,
        "avif": gen_avif,
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
