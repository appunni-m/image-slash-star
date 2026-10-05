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
import math
import os
import random
import struct
import subprocess
import tempfile
import zlib
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageDraw, __version__ as PILLOW_VERSION

ROOT = Path(__file__).parent.parent
OUT = ROOT / "tests" / "fixtures" / "input" / "images"
SIZE = (128, 128)
PNG_ADAM7_PASSES = (
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
)


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


def zero_jpeg_quantization_table(data, table_id):
    """Set one complete DQT table to zero while preserving JPEG structure."""
    mutated = bytearray(data)
    _, payload_start, segment_end = jpeg_segment(mutated, 0xDB)
    cursor = payload_start
    while cursor < segment_end:
        info = mutated[cursor]
        precision = info >> 4
        current_table_id = info & 0x0F
        if precision not in (0, 1):
            raise ValueError("unsupported JPEG quantization table precision")
        entry_width = 1 if precision == 0 else 2
        table_end = cursor + 1 + 64 * entry_width
        if table_end > segment_end:
            raise ValueError("truncated JPEG quantization table")
        if current_table_id == table_id:
            mutated[cursor + 1 : table_end] = bytes(table_end - cursor - 1)
            return bytes(mutated)
        cursor = table_end
    raise ValueError(f"JPEG quantization table {table_id} not found")


def jpeg_scan_segments(data):
    """Return progressive scan ranges and the final EOI position."""
    scans = []
    position = 2
    while position < len(data):
        marker_start = position
        if data[position] != 0xFF:
            raise ValueError("expected a JPEG marker")
        while position < len(data) and data[position] == 0xFF:
            position += 1
        if position >= len(data):
            break
        marker = data[position]
        position += 1
        if marker == 0xD9:
            return scans, marker_start
        if marker in (0xD8, 0x01) or 0xD0 <= marker <= 0xD7:
            continue
        if position + 2 > len(data):
            raise ValueError("truncated JPEG marker length")
        length = struct.unpack(">H", data[position : position + 2])[0]
        payload_start = position + 2
        segment_end = position + length
        if length < 2 or segment_end > len(data):
            raise ValueError("invalid JPEG marker length")
        if marker != 0xDA:
            position = segment_end
            continue

        component_count = data[payload_start]
        spectral_start = payload_start + 1 + 2 * component_count
        if spectral_start + 3 > segment_end:
            raise ValueError("truncated JPEG scan header")
        ss, se, ah_al = data[spectral_start : spectral_start + 3]

        # Skip entropy bytes, stuffed 0xFF values, and restart markers until
        # the next structural marker so later scans can also be transformed.
        cursor = segment_end
        while cursor < len(data):
            if data[cursor] != 0xFF:
                cursor += 1
                continue
            next_marker_start = cursor
            cursor += 1
            while cursor < len(data) and data[cursor] == 0xFF:
                cursor += 1
            if cursor >= len(data):
                break
            next_marker = data[cursor]
            cursor += 1
            if next_marker == 0x00 or 0xD0 <= next_marker <= 0xD7:
                continue
            scans.append(
                (marker_start, next_marker_start, ss, se, ah_al, spectral_start + 1)
            )
            position = next_marker_start
            break
        else:
            raise ValueError("unterminated JPEG entropy scan")
    raise ValueError("JPEG EOI marker not found")


def progressive_dc_only_jpeg(data):
    """Keep a progressive JPEG's first DC scan as a complete one-scan file."""
    scans, _ = jpeg_scan_segments(data)
    if not scans or scans[0][2:4] != (0, 0):
        raise ValueError("expected a first progressive DC scan")
    return data[: scans[0][1]] + b"\xff\xd9"


def split_baseline_444_into_component_scans(
    data, scan_groups=((2,), (1,), (3,)), restart_interval=0
):
    """Rewrite a small 4:4:4 baseline scan into component scan groups."""
    scans, _ = jpeg_scan_segments(data)
    if len(scans) != 1:
        raise ValueError("expected one baseline entropy scan")
    scan_start, entropy_end, ss, se, ah_al, _ = scans[0]
    if (ss, se, ah_al) != (0, 63, 0):
        raise ValueError("expected a baseline sequential scan")

    _, sof_payload, sof_end = jpeg_segment(data, 0xC0)
    if data[sof_payload + 5] != 3:
        raise ValueError("expected a three-component frame")
    width = struct.unpack_from(">H", data, sof_payload + 3)[0]
    height = struct.unpack_from(">H", data, sof_payload + 1)[0]
    if width == 0 or height == 0 or width % 8 != 0 or height % 8 != 0:
        raise ValueError("component-scan splitting expects whole 8x8 MCUs")
    if restart_interval not in (0, 1):
        raise ValueError("component-scan splitting supports restart intervals 0 or 1")
    if restart_interval == 1 and (width, height) != (16, 8):
        raise ValueError("restart fixture expects exactly two horizontal MCUs")
    for offset in range(sof_payload + 6, sof_end, 3):
        if data[offset + 1] != 0x11:
            raise ValueError("component-scan splitting expects 4:4:4 sampling")

    _, sos_payload, sos_end = jpeg_segment(data, 0xDA)
    component_count = data[sos_payload]
    if component_count != 3:
        raise ValueError("expected a three-component interleaved scan")
    descriptors = {
        data[sos_payload + 1 + 2 * index]: data[sos_payload + 2 + 2 * index]
        for index in range(component_count)
    }
    if set(descriptors) != {1, 2, 3}:
        raise ValueError("expected component IDs 1, 2, and 3")
    flattened_groups = [component_id for group in scan_groups for component_id in group]
    if any(not group for group in scan_groups) or sorted(flattened_groups) != [1, 2, 3]:
        raise ValueError("scan groups must contain each frame component once")

    huffman_tables = {}
    marker_cursor = 2
    while marker_cursor < scan_start:
        if data[marker_cursor] != 0xFF:
            marker_cursor += 1
            continue
        while marker_cursor < scan_start and data[marker_cursor] == 0xFF:
            marker_cursor += 1
        if marker_cursor >= scan_start:
            break
        marker = data[marker_cursor]
        marker_cursor += 1
        if marker in (0xD8, 0xD9) or 0xD0 <= marker <= 0xD7:
            continue
        if marker_cursor + 2 > scan_start:
            raise ValueError("truncated JPEG segment before scan")
        segment_length = struct.unpack_from(">H", data, marker_cursor)[0]
        segment_end = marker_cursor + segment_length
        if segment_length < 2 or segment_end > scan_start:
            raise ValueError("invalid JPEG segment before scan")
        if marker == 0xC4:
            cursor = marker_cursor + 2
            while cursor < segment_end:
                table_info = data[cursor]
                cursor += 1
                code_counts = data[cursor : cursor + 16]
                if len(code_counts) != 16:
                    raise ValueError("truncated JPEG Huffman table counts")
                cursor += 16
                symbol_count = sum(code_counts)
                symbols = data[cursor : cursor + symbol_count]
                if len(symbols) != symbol_count:
                    raise ValueError("truncated JPEG Huffman table symbols")
                cursor += symbol_count

                code = 0
                symbol_index = 0
                lookup = {}
                for code_length, count in enumerate(code_counts, start=1):
                    for _ in range(count):
                        lookup[(code, code_length)] = symbols[symbol_index]
                        symbol_index += 1
                        code += 1
                    code <<= 1
                huffman_tables[(table_info >> 4, table_info & 0x0F)] = lookup
        marker_cursor = segment_end

    entropy = bytearray()
    cursor = sos_end
    while cursor < entropy_end:
        byte = data[cursor]
        cursor += 1
        entropy.append(byte)
        if byte == 0xFF:
            if cursor >= entropy_end or data[cursor] != 0:
                raise ValueError("unexpected marker in a source entropy scan")
            cursor += 1
    bits = [(byte >> shift) & 1 for byte in entropy for shift in range(7, -1, -1)]

    def read_symbol(table, position):
        code = 0
        for code_length in range(1, 17):
            if position >= len(bits):
                raise ValueError("truncated JPEG entropy code")
            code = (code << 1) | bits[position]
            position += 1
            symbol = table.get((code, code_length))
            if symbol is not None:
                return symbol, position
        raise ValueError("invalid JPEG Huffman code")

    def read_block(position, dc_table, ac_table):
        block_start = position
        category, position = read_symbol(dc_table, position)
        position += category
        coefficient = 1
        while coefficient < 64:
            run_size, position = read_symbol(ac_table, position)
            run = run_size >> 4
            size = run_size & 0x0F
            if size == 0:
                if run == 0:
                    break
                if run != 15:
                    raise ValueError("invalid JPEG AC run")
                coefficient += 16
            else:
                coefficient += run + 1
                position += size
            if position > len(bits) or coefficient > 64:
                raise ValueError("JPEG coefficient block exceeds its bounds")
        return position, bits[block_start:position], category

    mcu_count = (width // 8) * (height // 8)
    mcu_component_bits = []
    mcu_dc_categories = []
    position = 0
    for _ in range(mcu_count):
        component_bits = {}
        dc_categories = {}
        for component_id, table_info in descriptors.items():
            dc_table = huffman_tables[(0, table_info >> 4)]
            ac_table = huffman_tables[(1, table_info & 0x0F)]
            position, component_bits[component_id], dc_categories[component_id] = (
                read_block(position, dc_table, ac_table)
            )
        mcu_component_bits.append(component_bits)
        mcu_dc_categories.append(dc_categories)

    if restart_interval == 1 and any(
        category != 0
        for mcu_categories in mcu_dc_categories
        for category in mcu_categories.values()
    ):
        raise ValueError("restart fixture requires zero DC differences at every MCU")
    if any(bit != 1 for bit in bits[position:]):
        raise ValueError("JPEG scan padding is not all one bits")

    def pack_entropy(component_bit_slices):
        component = [bit for bit_slice in component_bit_slices for bit in bit_slice]
        padded = component + [1] * ((-len(component)) % 8)
        output = bytearray()
        for offset in range(0, len(padded), 8):
            byte = 0
            for bit in padded[offset : offset + 8]:
                byte = (byte << 1) | bit
            output.append(byte)
            if byte == 0xFF:
                output.append(0)
        return bytes(output)

    split_scans = bytearray()
    if restart_interval == 1:
        split_scans.extend(b"\xff\xdd\x00\x04\x00\x01")
    for group in scan_groups:
        sos_payload = bytearray((len(group),))
        for component_id in group:
            sos_payload.extend((component_id, descriptors[component_id]))
        sos_payload.extend((0, 63, 0))
        split_scans.extend(b"\xff\xda")
        split_scans.extend(struct.pack(">H", len(sos_payload) + 2))
        split_scans.extend(sos_payload)

        mcus_per_segment = restart_interval or mcu_count
        segment_count = (mcu_count + mcus_per_segment - 1) // mcus_per_segment
        for segment_index in range(segment_count):
            segment_start = segment_index * mcus_per_segment
            segment_end = min(segment_start + mcus_per_segment, mcu_count)
            segment_bits = []
            for mcu_index in range(segment_start, segment_end):
                for component_id in group:
                    segment_bits.append(mcu_component_bits[mcu_index][component_id])
            split_scans.extend(pack_entropy(segment_bits))
            if segment_end < mcu_count:
                split_scans.extend(bytes((0xFF, 0xD0 + (segment_index % 8))))

    return data[:scan_start] + bytes(split_scans) + b"\xff\xd9"


def set_baseline_scan_spectral_start(data, scan_index, spectral_start):
    """Set Ss in one single-component baseline scan header."""
    scans, _ = jpeg_scan_segments(data)
    if not 0 <= scan_index < len(scans):
        raise ValueError("baseline scan index is out of range")
    scan_start = scans[scan_index][0]
    component_count = data[scan_start + 4]
    if component_count != 1:
        raise ValueError("spectral-start fixture expects a single-component scan")
    mutated = bytearray(data)
    mutated[scan_start + 5 + 2 * component_count] = spectral_start
    return bytes(mutated)


def repeat_baseline_scan_component(data, scan_index, component_id):
    """Replace a single-component scan's ID to repeat an earlier component."""
    scans, _ = jpeg_scan_segments(data)
    if not 0 <= scan_index < len(scans):
        raise ValueError("baseline scan index is out of range")
    scan_start = scans[scan_index][0]
    if data[scan_start + 4] != 1:
        raise ValueError("repeated-component fixture expects a single-component scan")
    mutated = bytearray(data)
    mutated[scan_start + 5] = component_id
    return bytes(mutated)


def omit_final_baseline_scan(data):
    """End a component-scan JPEG before its final component scan."""
    scans, _ = jpeg_scan_segments(data)
    if len(scans) < 2:
        raise ValueError("expected multiple baseline component scans")
    return data[: scans[-1][0]] + b"\xff\xd9"


def retain_first_baseline_component_scan(data, component_id):
    """Keep a baseline 4:4:4 file's first single-component scan and end it."""
    scans, _ = jpeg_scan_segments(data)
    if len(scans) != 3:
        raise ValueError("expected three baseline component scans")

    scan_start, entropy_end, spectral_start, spectral_end, approximation, _ = scans[0]
    _, sos_payload, sos_end = jpeg_segment(data, 0xDA)
    if data[scan_start : scan_start + 2] != b"\xff\xda":
        raise ValueError("expected a structural marker before the first scan")
    if sos_end - sos_payload < 2:
        raise ValueError("first baseline scan header is truncated")
    if data[sos_payload] != 1 or data[sos_payload + 1] != component_id:
        raise ValueError("first baseline scan does not select the requested component")
    if (spectral_start, spectral_end, approximation) != (0, 63, 0):
        raise ValueError("expected a sequential baseline component scan")
    if entropy_end <= sos_end:
        raise ValueError("first baseline component scan has no entropy data")

    return data[:entropy_end] + b"\xff\xd9"


def empty_baseline_scan_entropy(data, scan_index):
    """Remove one component scan's entropy while retaining its SOS marker."""
    scans, _ = jpeg_scan_segments(data)
    if not 0 <= scan_index < len(scans):
        raise ValueError("baseline scan index is out of range")
    scan_start, entropy_end = scans[scan_index][:2]
    sos_length = struct.unpack_from(">H", data, scan_start + 2)[0]
    entropy_start = scan_start + 2 + sos_length
    if entropy_start > entropy_end:
        raise ValueError("baseline scan header exceeds its entropy bounds")
    return data[:entropy_start] + data[entropy_end:]


def append_restart_markers_before_eoi(data):
    """Add Pillow-tolerated restart markers, including an empty segment."""
    if not data.endswith(b"\xff\xd9"):
        raise ValueError("expected a JPEG EOI marker")
    return data[:-2] + b"\xff\xd0\xff\xd1\xff\xd9"


def append_progressive_restart_overrun(data):
    """Add the next two restart markers to the second progressive scan."""
    scans, _ = jpeg_scan_segments(data)
    if len(scans) < 3:
        raise ValueError("expected a following scan after progressive scan 1")

    scan_start, entropy_end, spectral_start, spectral_end, approximation, _ = scans[1]
    if (spectral_start, spectral_end, approximation) != (1, 5, 2):
        raise ValueError("progressive scan 1 differs from the expected AC-first scan")
    next_scan_start = scans[2][0]
    cursor = entropy_end
    while cursor < next_scan_start:
        if data[cursor] != 0xFF:
            raise ValueError("progressive scan 1 is followed by non-marker data")
        cursor += 1
        while cursor < next_scan_start and data[cursor] == 0xFF:
            cursor += 1
        if cursor >= next_scan_start:
            raise ValueError("progressive scan 1 is followed by an incomplete marker")
        marker = data[cursor]
        cursor += 1
        if marker not in (0xC4, 0xDD) or cursor + 2 > len(data):
            raise ValueError("progressive scan 1 is followed by an unexpected marker")
        marker_length = struct.unpack_from(">H", data, cursor)[0]
        if marker_length < 2:
            raise ValueError("progressive scan 1 is followed by an invalid marker length")
        cursor += marker_length
    if (
        cursor != next_scan_start
        or data[next_scan_start : next_scan_start + 2] != b"\xff\xda"
    ):
        raise ValueError("progressive scan 1 boundary does not lead to the following SOS")

    header_length = struct.unpack_from(">H", data, scan_start + 2)[0]
    entropy_start = scan_start + 2 + header_length
    if entropy_start > entropy_end:
        raise ValueError("progressive scan 1 header extends beyond its entropy data")
    component_count = data[scan_start + 4]
    if component_count != 1 or data[scan_start + 5] != 1:
        raise ValueError("progressive scan 1 no longer selects the expected Y component")

    restart_markers = []
    cursor = entropy_start
    while cursor < entropy_end:
        if data[cursor] != 0xFF:
            cursor += 1
            continue
        cursor += 1
        while cursor < entropy_end and data[cursor] == 0xFF:
            cursor += 1
        if cursor >= entropy_end:
            raise ValueError("progressive scan 1 ends with an incomplete marker")
        marker = data[cursor]
        cursor += 1
        if marker == 0x00:
            continue
        if 0xD0 <= marker <= 0xD7:
            restart_markers.append(marker)
            continue
        raise ValueError("progressive scan 1 contains a structural marker before its end")

    if restart_markers != list(range(0xD0, 0xD7)):
        raise ValueError(
            f"expected progressive scan 1 restart markers D0-D6, found {restart_markers}"
        )
    return data[:entropy_end] + b"\xff\xd7\xff\xd0" + data[entropy_end:]


def redefine_quant_table_after_first_scan(data, table_id):
    """Redefine one 8-bit quantization table between sequential scans."""
    scans, _ = jpeg_scan_segments(data)
    if len(scans) < 2:
        raise ValueError("expected multiple sequential scans")
    first_scan_end = scans[0][1]
    marker_cursor = 2

    while marker_cursor < scans[0][0]:
        if data[marker_cursor] != 0xFF:
            marker_cursor += 1
            continue
        while marker_cursor < scans[0][0] and data[marker_cursor] == 0xFF:
            marker_cursor += 1
        if marker_cursor >= scans[0][0]:
            break
        marker = data[marker_cursor]
        marker_cursor += 1
        if marker in (0xD8, 0xD9) or 0xD0 <= marker <= 0xD7:
            continue
        if marker_cursor + 2 > scans[0][0]:
            raise ValueError("truncated JPEG segment before first scan")
        segment_length = struct.unpack_from(">H", data, marker_cursor)[0]
        segment_end = marker_cursor + segment_length
        if segment_length < 2 or segment_end > scans[0][0]:
            raise ValueError("invalid JPEG segment before first scan")
        if marker == 0xDB:
            cursor = marker_cursor + 2
            while cursor < segment_end:
                table_info = data[cursor]
                cursor += 1
                precision = table_info >> 4
                current_table_id = table_info & 0x0F
                if precision != 0:
                    raise ValueError("quantization-table fixture expects 8-bit entries")
                table_end = cursor + 64
                if table_end > segment_end:
                    raise ValueError("truncated JPEG quantization table")
                values = data[cursor:table_end]
                cursor = table_end
                if current_table_id == table_id:
                    changed_values = bytes(min(255, value * 2) for value in values)
                    replacement = (
                        b"\xff\xdb\x00\x43"
                        + bytes((table_info,))
                        + changed_values
                    )
                    return (
                        data[:first_scan_end]
                        + replacement
                        + data[first_scan_end:]
                    )
        marker_cursor = segment_end

    raise ValueError(f"JPEG quantization table {table_id} not found")


def repeat_dqt_after_first_baseline_scan(data):
    """Repeat the first post-scan DQT marker before the next baseline scan."""
    scans, _ = jpeg_scan_segments(data)
    if len(scans) < 2:
        raise ValueError("expected multiple baseline component scans")
    marker_start = scans[0][1]
    if data[marker_start : marker_start + 2] != b"\xff\xdb":
        raise ValueError("expected DQT immediately after the first baseline scan")
    segment_length = struct.unpack_from(">H", data, marker_start + 2)[0]
    segment_end = marker_start + 2 + segment_length
    if segment_length < 2 or segment_end > len(data):
        raise ValueError("invalid DQT marker after the first baseline scan")
    if data[segment_end : segment_end + 2] != b"\xff\xda":
        raise ValueError("expected the next baseline scan immediately after DQT")
    dqt_segment = data[marker_start:segment_end]
    return data[:segment_end] + dqt_segment + data[segment_end:]


def repeat_dht_after_first_baseline_scan(data):
    """Repeat the first DHT marker twice between sequential baseline scans."""
    scans, _ = jpeg_scan_segments(data)
    if len(scans) < 2:
        raise ValueError("expected multiple baseline component scans")
    marker_start, _, marker_end = jpeg_segment(data, 0xC4)
    if marker_start >= scans[0][0] or data[marker_start : marker_start + 2] != b"\xff\xc4":
        raise ValueError("expected DHT before the first baseline scan")
    insertion_offset = scans[0][1]
    if data[insertion_offset : insertion_offset + 2] != b"\xff\xda":
        raise ValueError("expected the next baseline scan after first-scan entropy")
    dht_segment = data[marker_start:marker_end]
    if not dht_segment:
        raise ValueError("first baseline DHT marker is empty")
    return (
        data[:insertion_offset]
        + dht_segment
        + dht_segment
        + data[insertion_offset:]
    )


def repeat_dqt_after_first_progressive_scan(data):
    """Repeat the first DQT segment unchanged between progressive scans."""
    scans, _ = jpeg_scan_segments(data)
    if (
        not scans
        or scans[0][2:4] != (0, 0)
        or scans[0][4] >> 4 != 0
    ):
        raise ValueError("expected a progressive DC-first scan")
    marker_start, _, marker_end = jpeg_segment(data, 0xDB)
    if data[marker_start : marker_start + 2] != b"\xff\xdb":
        raise ValueError("expected a DQT marker before the first scan")
    first_scan_end = scans[0][1]
    dqt_segment = data[marker_start:marker_end]
    if not dqt_segment or first_scan_end >= len(data):
        raise ValueError("progressive JPEG has no insertion point after its first scan")
    return data[:first_scan_end] + dqt_segment + data[first_scan_end:]


def redefine_quant_table_after_single_scan(data, table_id):
    """Redefine a quantization table after a complete one-scan JPEG."""
    scans, eoi_start = jpeg_scan_segments(data)
    if len(scans) != 1:
        raise ValueError("expected one baseline entropy scan")
    if not 0 <= table_id <= 3:
        raise ValueError("8-bit JPEG quantization table ID must be between 0 and 3")
    payload = bytes([table_id]) + bytes([1]) * 64
    marker = b"\xff\xdb" + struct.pack(">H", len(payload) + 2) + payload
    return data[:eoi_start] + marker + data[eoi_start:]


def append_empty_dqt_after_single_scan(data):
    """Append Pillow-tolerated empty DQT marker after a one-scan JPEG."""
    scans, eoi_start = jpeg_scan_segments(data)
    if len(scans) != 1:
        raise ValueError("expected one baseline entropy scan")
    return data[:eoi_start] + b"\xff\xdb\x00\x02" + data[eoi_start:]


def add_fill_byte_before_trailing_dqt(data):
    """Insert an extra FF byte before a trailing DQT marker."""
    scans, _ = jpeg_scan_segments(data)
    if len(scans) != 1:
        raise ValueError("expected one baseline entropy scan")
    marker_start = scans[0][1]
    if data[marker_start : marker_start + 2] != b"\xff\xdb":
        raise ValueError("expected a trailing DQT marker")
    return data[:marker_start] + b"\xff" + data[marker_start:]


def mutate_progressive_ac_scan_end(data, new_se):
    """Set the spectral end to ``new_se`` in every progressive AC refinement scan."""
    scans, _ = jpeg_scan_segments(data)
    mutated = bytearray(data)
    changed = 0
    for _, _, ss, se, ah_al, se_offset in scans:
        if ss > 0 and se == 63 and ah_al >> 4 > 0:
            mutated[se_offset] = new_se
            changed += 1
    if changed == 0:
        raise ValueError("no progressive AC refinement scans found")
    return bytes(mutated)


def mutate_progressive_ac_scan_start(data, new_ss):
    """Set the spectral start to ``new_ss`` in every AC refinement scan."""
    scans, _ = jpeg_scan_segments(data)
    mutated = bytearray(data)
    changed = 0
    for _, _, ss, se, ah_al, se_offset in scans:
        if ss > 0 and se == 63 and ah_al >> 4 > 0:
            mutated[se_offset - 1] = new_ss
            changed += 1
    if changed == 0:
        raise ValueError("no progressive AC refinement scans found")
    return bytes(mutated)


def duplicate_last_progressive_ac_refinement_scan(data):
    """Repeat the final AC-refinement scan immediately before EOI."""
    scans, eoi_start = jpeg_scan_segments(data)
    scan = next(
        (
            candidate
            for candidate in reversed(scans)
            if candidate[2] > 0 and candidate[4] >> 4 > 0
        ),
        None,
    )
    if scan is None:
        raise ValueError("no progressive AC refinement scan found")
    start, end = scan[:2]
    return data[:eoi_start] + data[start:end] + data[eoi_start:]


def narrow_last_progressive_ac_refinement_scan(data, new_se):
    """Narrow the final progressive AC-refinement band to exercise its tail."""
    scans, _ = jpeg_scan_segments(data)
    scan = next(
        (
            candidate
            for candidate in reversed(scans)
            if candidate[2] > 0 and candidate[4] >> 4 > 0
        ),
        None,
    )
    if scan is None:
        raise ValueError("no progressive AC refinement scan found")
    _, _, ss, se, _, se_offset = scan
    if not ss <= new_se < se:
        raise ValueError("new spectral end must narrow the final AC refinement band")
    narrowed = bytearray(data)
    narrowed[se_offset] = new_se
    return bytes(narrowed)


def narrow_single_block_progressive_ac_first_tail(data, new_se):
    """Narrow one-block AC-first and refinement scans to a shared upper tail."""
    scans, _ = jpeg_scan_segments(data)
    narrowed = bytearray(data)
    initial_scans = 0
    refinement_scans = 0
    for _, _, ss, se, ah_al, se_offset in scans:
        successive_approximation_high = ah_al >> 4
        if ss == new_se and se == 63 and successive_approximation_high == 0:
            narrowed[se_offset] = new_se
            initial_scans += 1
        elif (
            0 < ss <= new_se < se == 63
            and successive_approximation_high > 0
        ):
            narrowed[se_offset] = new_se
            refinement_scans += 1
    if initial_scans != 1 or refinement_scans == 0:
        raise ValueError("expected one upper-band AC-first scan and its refinements")
    return bytes(narrowed)


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


def mutate_first_jpeg_dc_category(data, category):
    """Replace the first canonical DC symbol with an out-of-range category."""
    mutated = bytearray(data)
    for _, payload_start, end in jpeg_segments(mutated, 0xC4):
        position = payload_start
        while position < end:
            table_info = mutated[position]
            symbol_count = sum(mutated[position + 1 : position + 17])
            symbols_start = position + 17
            if table_info >> 4 == 0:
                if symbol_count == 0:
                    raise ValueError("JPEG DC Huffman table contains no symbols")
                mutated[symbols_start] = category
                return bytes(mutated)
            position = symbols_start + symbol_count
    raise ValueError("JPEG DC Huffman table not found")


def empty_jpeg_huffman_table(data, table_class, table_id):
    """Replace one JPEG DHT entry with a valid zero-symbol table."""
    for start, payload_start, end in jpeg_segments(data, 0xC4):
        payload = data[payload_start:end]
        rewritten = bytearray()
        position = 0
        replaced = False
        while position < len(payload):
            if position + 17 > len(payload):
                raise ValueError("truncated JPEG DHT table header")
            table_info = payload[position]
            symbol_count = sum(payload[position + 1 : position + 17])
            record_end = position + 17 + symbol_count
            if record_end > len(payload):
                raise ValueError("truncated JPEG DHT table symbols")
            if (
                not replaced
                and table_info >> 4 == table_class
                and table_info & 0x0F == table_id
            ):
                rewritten.append(table_info)
                rewritten.extend(bytes(16))
                replaced = True
            else:
                rewritten.extend(payload[position:record_end])
            position = record_end
        if replaced:
            marker = (
                b"\xff\xc4"
                + struct.pack(">H", len(rewritten) + 2)
                + rewritten
            )
            return bytes(data[:start]) + marker + bytes(data[end:])
    raise ValueError(
        f"JPEG DHT class {table_class}, id {table_id} not found"
    )


def build_progressive_dc_overflow_jcoef_compatibility_jpeg():
    """Build the pinned-oracle progressive DC/JCOEF narrowing edge case."""
    def marker_segment(marker, payload):
        return (
            b"\xff"
            + bytes([marker])
            + struct.pack(">H", len(payload) + 2)
            + payload
        )

    def entropy_bytes(bits):
        bits += "1" * (-len(bits) % 8)
        packed = bytes(
            int(bits[offset : offset + 8], 2)
            for offset in range(0, len(bits), 8)
        )
        return packed.replace(b"\xff", b"\xff\x00")

    # Pillow accepts this 8-bit, Pq=0 progressive stream. Its category-11 DC
    # first scan writes +2047 at Al=13, beyond signed 16-bit JCOEF range; keep
    # the exact bytes pinned as a libjpeg-turbo oracle-compatibility case.
    dqt = bytes([0, 255]) + bytes([1]) * 63
    sof = bytes([8]) + struct.pack(">HH", 8, 8) + bytes([1, 1, 0x11, 0])
    dht = bytes([0, 1]) + bytes(15) + bytes([11])
    dht += bytes([0x10, 1]) + bytes(15) + bytes([0])

    data = b"\xff\xd8"
    data += marker_segment(0xDB, dqt)
    data += marker_segment(0xC2, sof)
    data += marker_segment(0xC4, dht)
    data += marker_segment(0xDA, bytes([1, 1, 0, 0, 0, 0x0D]))
    data += entropy_bytes("0" + "1" * 11)
    data += marker_segment(0xDA, bytes([1, 1, 0, 1, 63, 0]))
    data += entropy_bytes("0")
    for ah in range(13, 0, -1):
        al = ah - 1
        data += marker_segment(
            0xDA, bytes([1, 1, 0, 0, 0, (ah << 4) | al])
        )
        data += entropy_bytes("0")
    data += b"\xff\xd9"

    expected_sha256 = (
        "9e147af5e9a139e428c67434dbac31a2f02757c42beb0854b891564166af29cb"
    )
    if hashlib.sha256(data).hexdigest() != expected_sha256:
        raise RuntimeError("progressive DC/JCOEF fixture bytes differ")
    return data


def zero_sample_jpeg(
    base, width, height, y_sampling, cb_sampling=0x11, cr_sampling=0x11
):
    """Build a one-MCU RGB JPEG with standard-table zero coefficient blocks."""
    data = bytearray(base)
    _, sof_payload, _ = jpeg_segment(data, 0xC0)
    data[sof_payload + 1 : sof_payload + 3] = struct.pack(">H", height)
    data[sof_payload + 3 : sof_payload + 5] = struct.pack(">H", width)
    sampling = (y_sampling, cb_sampling, cr_sampling)
    component_count = data[sof_payload + 5]
    if component_count != len(sampling):
        raise RuntimeError("zero-sample RGB JPEG template has an unexpected component count")
    for component_index, factor in enumerate(sampling):
        data[sof_payload + 7 + component_index * 3] = factor

    _, _, entropy_start = jpeg_segment(data, 0xDA)
    component_blocks = [
        (factor >> 4) * (factor & 0x0F) for factor in sampling
    ]
    # Standard tables: luminance DC(0)+EOB = 00 1010; chroma = 00 00.
    bits = "001010" * component_blocks[0]
    bits += "0000" * sum(component_blocks[1:])
    bits += "1" * ((-len(bits)) % 8)
    entropy = bytes(int(bits[offset : offset + 8], 2) for offset in range(0, len(bits), 8))
    return bytes(data[:entropy_start]) + entropy + b"\xff\xd9"


def zero_sample_cmyk_jpeg(base, width, height, sampling):
    """Build a one-MCU CMYK JPEG whose four components share one sampling factor."""
    data = bytearray(base)
    _, sof_payload, _ = jpeg_segment(data, 0xC0)
    data[sof_payload + 1 : sof_payload + 3] = struct.pack(">H", height)
    data[sof_payload + 3 : sof_payload + 5] = struct.pack(">H", width)
    component_count = data[sof_payload + 5]
    if component_count != 4:
        raise RuntimeError("zero-sample CMYK JPEG template has an unexpected component count")
    for component_index in range(component_count):
        data[sof_payload + 7 + component_index * 3] = sampling

    _, sos_payload, sos_end = jpeg_segment(data, 0xDA)
    scan_component_count = data[sos_payload]
    if scan_component_count != component_count:
        raise RuntimeError("zero-sample CMYK JPEG template has an unexpected scan shape")
    for component_index in range(scan_component_count):
        data[sos_payload + 2 + component_index * 2] = 0

    blocks_per_component = (sampling >> 4) * (sampling & 0x0F)
    bits = "001010" * (component_count * blocks_per_component)
    bits += "1" * ((-len(bits)) % 8)
    entropy = bytes(int(bits[offset : offset + 8], 2) for offset in range(0, len(bits), 8))
    return bytes(data[:sos_end]) + entropy + b"\xff\xd9"


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


def corrupt_png_chunk_crc(data, kind, occurrence=0):
    """Flip one CRC bit without changing the corresponding PNG chunk payload."""
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("invalid PNG signature")
    corrupted = bytearray(data)
    position = 8
    seen = 0
    while position < len(corrupted):
        if position + 12 > len(corrupted):
            raise ValueError("truncated PNG chunk")
        length = struct.unpack_from(">I", corrupted, position)[0]
        crc_start = position + 8 + length
        chunk_end = crc_start + 4
        if chunk_end > len(corrupted):
            raise ValueError("truncated PNG chunk")
        chunk_kind = bytes(corrupted[position + 4 : position + 8])
        if chunk_kind == kind:
            if seen == occurrence:
                corrupted[chunk_end - 1] ^= 0x01
                return bytes(corrupted)
            seen += 1
        position = chunk_end
        if chunk_kind == b"IEND":
            break
    raise ValueError(f"PNG chunk {kind!r} occurrence {occurrence} was not found")


def rebuild_png(chunks):
    """Serialize PNG chunks with fresh CRCs."""
    return b"\x89PNG\r\n\x1a\n" + b"".join(
        png_chunk(kind, payload) for kind, payload in chunks
    )


def insert_png_chunks(path, additions):
    """Insert ancillary chunks before IDAT and refresh every PNG CRC."""
    chunks = png_chunks(path.read_bytes())
    idat = next(
        (index for index, (kind, _) in enumerate(chunks) if kind == b"IDAT"),
        None,
    )
    if idat is None:
        raise ValueError(f"PNG has no IDAT chunk: {path}")
    chunks[idat:idat] = additions
    path.write_bytes(rebuild_png(chunks))


def insert_png_chunks_before_iend(path, additions):
    """Insert ancillary chunks after image data and before IEND."""
    chunks = png_chunks(path.read_bytes())
    idat = next(
        (index for index, (kind, _) in enumerate(chunks) if kind == b"IDAT"),
        None,
    )
    iend = next(
        (index for index, (kind, _) in enumerate(chunks) if kind == b"IEND"),
        None,
    )
    if idat is None or iend is None or idat >= iend:
        raise ValueError(f"PNG has no ordered IDAT/IEND pair: {path}")
    chunks[iend:iend] = additions
    path.write_bytes(rebuild_png(chunks))


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
        for x_start, y_start, x_step, y_step in PNG_ADAM7_PASSES:
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


def write_gray_adam7_png(path, width, height, samples):
    """Write a deterministic 4-bit grayscale PNG with Adam7 passes."""
    if len(samples) != width * height:
        raise ValueError("PNG sample count does not match image dimensions")
    if any(not 0 <= sample < 16 for sample in samples):
        raise ValueError("4-bit grayscale PNG samples must be in [0, 16)")

    scanlines = bytearray()
    for x_start, y_start, x_step, y_step in PNG_ADAM7_PASSES:
        for y in range(y_start, height, y_step):
            pass_samples = [
                samples[y * width + x]
                for x in range(x_start, width, x_step)
            ]
            packed = bytearray((len(pass_samples) + 1) // 2)
            for index, sample in enumerate(pass_samples):
                packed[index // 2] |= sample << (4 if index % 2 == 0 else 0)
            scanlines.append(0)
            scanlines.extend(packed)

    header = struct.pack(">IIBBBBB", width, height, 4, 0, 0, 0, 1)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", zlib.compress(scanlines, 6))
        + png_chunk(b"IEND", b"")
    )


def write_gray16_adam7_png(path, width, height, samples):
    """Write a deterministic 16-bit grayscale PNG with Adam7 passes."""
    if len(samples) != width * height:
        raise ValueError("PNG sample count does not match image dimensions")
    if any(not 0 <= sample <= 0xFFFF for sample in samples):
        raise ValueError("16-bit grayscale PNG samples must be in [0, 65536)")

    scanlines = bytearray()
    for x_start, y_start, x_step, y_step in PNG_ADAM7_PASSES:
        for y in range(y_start, height, y_step):
            scanlines.append(0)
            for x in range(x_start, width, x_step):
                scanlines.extend(struct.pack(">H", samples[y * width + x]))

    header = struct.pack(">IIBBBBB", width, height, 16, 0, 0, 0, 1)
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
    bottom_edge = pattern_img("RGBA", (16, 17))
    bottom_edge_pixels = bottom_edge.load()
    for x in range(bottom_edge.width):
        red, green, blue, _ = bottom_edge_pixels[x, bottom_edge.height - 1]
        bottom_edge_pixels[x, bottom_edge.height - 1] = (red, green, blue, 0)
    bottom_edge.save(out_dir / "rgba_bottom_edge_16x17.png")
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


def build_progressive_refinement_zrl_symbol_jpeg():
    """Build one progressive block whose refinement emits an AC ZRL symbol."""

    def segment(marker, payload):
        return b"\xff" + bytes([marker]) + struct.pack(">H", len(payload) + 2) + payload

    def entropy(bits):
        bits += "1" * ((-len(bits)) % 8)
        raw = bytearray(
            int(bits[offset : offset + 8], 2)
            for offset in range(0, len(bits), 8)
        )
        stuffed = bytearray()
        for byte in raw:
            stuffed.append(byte)
            if byte == 0xFF:
                stuffed.append(0)
        return bytes(stuffed)

    quantization = bytes([0]) + bytes([1] * 64)
    frame = bytes([8]) + struct.pack(">HHB", 8, 8, 1) + bytes([1, 0x11, 0])
    dc_huffman = bytes([0]) + bytes([0, 1] + [0] * 14) + bytes([0])
    ac_huffman = bytes([0x10]) + bytes([0, 3] + [0] * 14)
    ac_huffman += bytes([0xF0, 0x01, 0x00])

    jpeg = bytearray(b"\xff\xd8")
    jpeg += segment(0xDB, quantization)
    jpeg += segment(0xC2, frame)
    jpeg += segment(0xC4, dc_huffman + ac_huffman)
    jpeg += segment(0xDA, bytes([1, 1, 0, 0, 0, 0]))
    jpeg += entropy("0")
    jpeg += segment(0xDA, bytes([1, 1, 0, 1, 63, 0x01]))
    jpeg += entropy("10")
    jpeg += segment(0xDA, bytes([1, 1, 0, 1, 63, 0x10]))
    # ZRL skips coefficients 1 through 16; (0, 1) introduces +1 at 17.
    jpeg += entropy("00011")
    jpeg += b"\xff\xd9"
    return bytes(jpeg)


def build_baseline_idct_high_horizontal_frequencies_jpeg():
    """Build blocks that exercise every scalar IDCT row shortcut outcome."""

    def segment(marker, payload):
        return b"\xff" + bytes([marker]) + struct.pack(">H", len(payload) + 2) + payload

    def entropy(bits):
        bits += "1" * ((-len(bits)) % 8)
        raw = bytearray(
            int(bits[offset : offset + 8], 2)
            for offset in range(0, len(bits), 8)
        )
        stuffed = bytearray()
        for byte in raw:
            stuffed.append(byte)
            if byte == 0xFF:
                stuffed.append(0)
        return bytes(stuffed)

    zigzag_order = (
        0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5,
        12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28,
        35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51,
        58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
    )
    coefficients = [(0, horizontal) for horizontal in range(3, 8)]
    coefficients.append((1, 0))
    amplitude = 128
    amplitude_size = amplitude.bit_length()
    target_indices = [vertical * 8 + horizontal for vertical, horizontal in coefficients]
    residual_runs = {
        index: (zigzag_order.index(index) - 1) % 16 for index in target_indices
    }
    ac_symbols = [0x00, 0xF0]
    ac_symbols.extend(
        sorted((run << 4) | amplitude_size for run in residual_runs.values())
    )
    ac_codes = {}
    for code, symbol in enumerate(ac_symbols[:6]):
        ac_codes[symbol] = format(code, "03b")
    code = len(ac_symbols[:6]) << 1
    for symbol in ac_symbols[6:]:
        ac_codes[symbol] = format(code, "04b")
        code += 1

    quantization = bytes([0]) + bytes([1] * 64)
    frame = bytes([8]) + struct.pack(">HHB", 48, 8, 1) + bytes([1, 0x11, 0])
    dc_huffman = bytes([0]) + bytes([1] + [0] * 15) + bytes([0])
    ac_huffman = bytes([0x10]) + bytes([0, 0, 6, 2] + [0] * 12)
    ac_huffman += bytes(ac_symbols)

    encoded_blocks = []
    for index in target_indices:
        run = zigzag_order.index(index) - 1
        block_bits = "0" + ac_codes[0xF0] * (run // 16)
        residual_run = run % 16
        symbol = (residual_run << 4) | amplitude_size
        block_bits += ac_codes[symbol] + format(amplitude, f"0{amplitude_size}b")
        block_bits += ac_codes[0x00]
        encoded_blocks.append(block_bits)

    jpeg = bytearray(b"\xff\xd8")
    jpeg += segment(0xDB, quantization)
    jpeg += segment(0xC0, frame)
    jpeg += segment(0xDD, bytes([0, 1]))
    jpeg += segment(0xC4, dc_huffman + ac_huffman)
    jpeg += segment(0xDA, bytes([1, 1, 0, 0, 63, 0]))
    for block_index, block_bits in enumerate(encoded_blocks):
        jpeg += entropy(block_bits)
        if block_index < len(encoded_blocks) - 1:
            jpeg += b"\xff" + bytes([0xD0 + block_index % 8])
    jpeg += b"\xff\xd9"
    return bytes(jpeg)


def build_baseline_ac_pair_tail_jpeg(template):
    """Make one grayscale scan whose paired AC lookup crosses block end."""
    scans, eoi_start = jpeg_scan_segments(template)
    if len(scans) != 1 or eoi_start <= 0:
        raise ValueError("expected one baseline JPEG scan")

    dc_table = None
    ac_table = None
    position = 2
    while position < scans[0][0]:
        if template[position] != 0xFF:
            raise ValueError("expected a JPEG marker before scan")
        while template[position] == 0xFF:
            position += 1
        marker = template[position]
        position += 1
        length = struct.unpack(">H", template[position : position + 2])[0]
        payload_start = position + 2
        segment_end = position + length
        if marker == 0xC4:
            cursor = payload_start
            while cursor < segment_end:
                table_id = template[cursor]
                cursor += 1
                counts = template[cursor : cursor + 16]
                cursor += 16
                symbol_count = sum(counts)
                symbols = template[cursor : cursor + symbol_count]
                cursor += symbol_count
                code = 0
                symbol_index = 0
                mapping = {}
                for bit_length, count in enumerate(counts, start=1):
                    for _ in range(count):
                        mapping[symbols[symbol_index]] = format(code, f"0{bit_length}b")
                        symbol_index += 1
                        code += 1
                    code <<= 1
                if table_id == 0:
                    dc_table = mapping
                elif table_id == 0x10:
                    ac_table = mapping
        position = segment_end

    if dc_table is None or ac_table is None:
        raise ValueError("expected standard grayscale DC and AC Huffman tables")

    scan_start = scans[0][0]
    scan_header_end = scan_start + 2 + struct.unpack(">H", template[scan_start + 2 : scan_start + 4])[0]
    bits = dc_table[0]
    bits += ac_table[0xF0] * 3
    bits += ac_table[0xD1] + "1"
    bits += ac_table[0x11] + "1"
    # The first 0x11 follows coefficient 62 and runs past coefficient 63.
    # Its following 0x01 makes the fast two-symbol lookup reject the pair;
    # libjpeg-turbo consumes the amplitude and maps the overrun to coefficient 63.
    bits += ac_table[0x01] + "1"
    bits += "1" * ((-len(bits)) % 8)

    entropy = bytearray(
        int(bits[offset : offset + 8], 2)
        for offset in range(0, len(bits), 8)
    )
    stuffed = bytearray()
    for byte in entropy:
        stuffed.append(byte)
        if byte == 0xFF:
            stuffed.append(0)

    # Keep the 12-bit pair lookup available after the overrun symbol. The
    # decoder has finished its only block before it reaches this ignored tail.
    return template[:scan_header_end] + bytes(stuffed) + b"\x00\xff\xd9"


def build_baseline_reserved_ac_zero_size_eob_jpeg():
    """Build a grayscale JPEG with a reserved zero-size AC symbol."""
    def marker_segment(marker, payload):
        return (
            b"\xff"
            + bytes([marker])
            + struct.pack(">H", len(payload) + 2)
            + payload
        )

    counts = bytes([1]) + bytes(15)
    quantization = bytes([0]) + bytes([1]) * 64
    frame = bytes([8]) + struct.pack(">HH", 8, 8) + bytes([1, 1, 0x11, 0])
    huffman = bytes([0]) + counts + bytes([0])
    huffman += bytes([0x10]) + counts + bytes([0x10])

    data = b"\xff\xd8"
    data += marker_segment(0xDB, quantization)
    data += marker_segment(0xC0, frame)
    data += marker_segment(0xC4, huffman)
    data += marker_segment(0xDA, bytes([1, 1, 0, 0, 63, 0]))
    data += b"\x3f\xff\xd9"

    expected_sha256 = (
        "278b5b7d9a5d56799646149baec5c89be8d1f1bc1d39c74f0ab02b16a53c774d"
    )
    if hashlib.sha256(data).hexdigest() != expected_sha256:
        raise RuntimeError("reserved zero-size AC fixture bytes differ")
    with Image.open(BytesIO(data)) as decoded:
        decoded.load()
        if decoded.mode != "L" or decoded.size != (8, 8):
            raise RuntimeError("Pillow changed the reserved zero-size AC fixture layout")
        if decoded.tobytes() != bytes([128]) * 64:
            raise RuntimeError("Pillow changed the reserved zero-size AC fixture pixels")
    return data


def build_baseline_multiscan_reserved_ac_zero_size_eob_jpeg():
    """Build a 4:4:4 multiscan JPEG that uses the scalar block decoder."""
    def marker_segment(marker, payload):
        return (
            b"\xff"
            + bytes([marker])
            + struct.pack(">H", len(payload) + 2)
            + payload
        )

    counts = bytes([1]) + bytes(15)
    quantization = bytes([0]) + bytes([1]) * 64
    frame = bytes([8]) + struct.pack(">HH", 8, 8) + bytes(
        [3, 1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0]
    )
    huffman = bytes([0]) + counts + bytes([0])
    huffman += bytes([0x10]) + counts + bytes([0x10])

    data = b"\xff\xd8"
    data += marker_segment(0xDB, quantization)
    data += marker_segment(0xC0, frame)
    data += marker_segment(0xC4, huffman)
    for component in (1, 2, 3):
        data += marker_segment(0xDA, bytes([1, component, 0, 0, 63, 0]))
        data += b"\x3f"
    data += b"\xff\xd9"

    expected_sha256 = (
        "8aa96a1a0a6e8edd37fafbe7be424c3b3691deb16923a469c46718730a10e7bc"
    )
    if hashlib.sha256(data).hexdigest() != expected_sha256:
        raise RuntimeError("multiscan reserved zero-size AC fixture bytes differ")
    with Image.open(BytesIO(data)) as decoded:
        decoded.load()
        if decoded.mode != "RGB" or decoded.size != (8, 8):
            raise RuntimeError("Pillow changed the multiscan reserved AC fixture layout")
        if decoded.tobytes() != bytes([128]) * (8 * 8 * 3):
            raise RuntimeError("Pillow changed the multiscan reserved AC fixture pixels")
    return data


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
    baseline_444 = (d / "baseline_444.jpg").read_bytes()
    first_sos = baseline_444.find(b"\xff\xda")
    if first_sos < 0 or not baseline_444.endswith(b"\xff\xd9"):
        raise RuntimeError("baseline 4:4:4 JPEG is missing its scan or final EOI marker")
    short_marker_missing_eoi = (
        baseline_444[:first_sos] + b"\xff\xf0\x00\x01" + baseline_444[first_sos:-2]
    )
    if hashlib.sha256(short_marker_missing_eoi).hexdigest() != (
        "869e30d73346ca502d81c1a04881b3ca8cbb64d980cb73b73cf856d54a8bdcbc"
    ):
        raise RuntimeError("short-marker JPEG fixture differs from its pinned hash")
    (d / "short_marker_missing_eoi.jpg").write_bytes(short_marker_missing_eoi)
    single_scan_source = BytesIO()
    pattern_img("RGB", (8, 8)).save(
        single_scan_source,
        format="JPEG",
        quality=90,
        subsampling=0,
        progressive=False,
        optimize=False,
    )
    single_scan_jpeg = single_scan_source.getvalue()
    trailing_dqt = redefine_quant_table_after_single_scan(single_scan_jpeg, table_id=0)
    (d / "baseline_444_single_scan_trailing_dqt.jpg").write_bytes(trailing_dqt)
    (d / "baseline_444_single_scan_empty_trailing_dqt.jpg").write_bytes(
        append_empty_dqt_after_single_scan(single_scan_jpeg)
    )
    (d / "baseline_444_single_scan_dqt_marker_fill.jpg").write_bytes(
        add_fill_byte_before_trailing_dqt(trailing_dqt)
    )
    component_scan_source = BytesIO()
    pattern_img("RGB", (8, 8)).save(
        component_scan_source,
        format="JPEG",
        quality=85,
        subsampling=0,
        progressive=False,
    )
    three_scan_jpeg = split_baseline_444_into_component_scans(
        component_scan_source.getvalue()
    )
    (d / "baseline_444_three_sequential_scans.jpg").write_bytes(three_scan_jpeg)
    single_cb_scan = retain_first_baseline_component_scan(three_scan_jpeg, 2)
    if hashlib.sha256(single_cb_scan).hexdigest() != (
        "6ab4a1f95019f910ed48b81d5889ff9a464904c9ba158232ab290e41dfe16231"
    ):
        raise RuntimeError("single-component JPEG fixture differs from its pinned hash")
    with Image.open(BytesIO(single_cb_scan)) as reference:
        reference.load()
        pixels = reference.tobytes()
        if (
            reference.format != "JPEG"
            or reference.mode != "RGB"
            or reference.size != (8, 8)
            or hashlib.sha256(pixels).hexdigest()
            != "3100a6a318864cd8fff4192c9acc15b51f2219a0195043761c7a38caf5b3963a"
        ):
            raise RuntimeError("single-component JPEG fixture differs from Pillow parity")
    (d / "baseline_444_single_component_scan_cb.jpg").write_bytes(single_cb_scan)
    duplicate_dht_after_snapshot = repeat_dht_after_first_baseline_scan(three_scan_jpeg)
    if hashlib.sha256(duplicate_dht_after_snapshot).hexdigest() != (
        "60ce19025b6c5e78ff18659ffea26dc97a0c628bb29dbc9c78c4d653c8c708f6"
    ):
        raise RuntimeError("duplicate post-snapshot DHT fixture differs from its pinned hash")
    duplicate_dht_path = d / "baseline_444_multiscan_duplicate_dht_after_snapshot.jpg"
    duplicate_dht_path.write_bytes(duplicate_dht_after_snapshot)
    with Image.open(d / "baseline_444_three_sequential_scans.jpg") as reference, Image.open(
        duplicate_dht_path
    ) as variant:
        reference.load()
        variant.load()
        if (
            reference.mode != variant.mode
            or reference.size != variant.size
            or reference.tobytes() != variant.tobytes()
        ):
            raise RuntimeError("duplicate post-snapshot DHT changed Pillow pixels")
    restart_scan_source = BytesIO()
    Image.new("RGB", (16, 8), (128, 128, 128)).save(
        restart_scan_source,
        format="JPEG",
        quality=85,
        subsampling=0,
        progressive=False,
    )
    interleaved_restart_jpeg = split_baseline_444_into_component_scans(
        restart_scan_source.getvalue(),
        scan_groups=((1, 2), (3,)),
        restart_interval=1,
    )
    (d / "baseline_444_multiscan_interleaved_restart.jpg").write_bytes(
        interleaved_restart_jpeg
    )
    (d / "baseline_444_multiscan_restart_overrun.jpg").write_bytes(
        append_restart_markers_before_eoi(interleaved_restart_jpeg)
    )
    dc_only_scan_source = BytesIO()
    Image.new("RGB", (8, 8), (128, 128, 128)).save(
        dc_only_scan_source,
        format="JPEG",
        quality=85,
        subsampling=0,
        progressive=False,
        optimize=False,
    )
    dc_only_three_scans = split_baseline_444_into_component_scans(
        dc_only_scan_source.getvalue()
    )
    (d / "baseline_444_multiscan_duplicate_component_dc_only.jpg").write_bytes(
        repeat_baseline_scan_component(dc_only_three_scans, 2, 2)
    )
    dqt_redefined = redefine_quant_table_after_first_scan(three_scan_jpeg, table_id=1)
    dqt_redefined_path = d / "baseline_444_three_scans_dqt_redefined.jpg"
    dqt_redefined_path.write_bytes(dqt_redefined)
    duplicate_dqt_after_snapshot = repeat_dqt_after_first_baseline_scan(dqt_redefined)
    if hashlib.sha256(duplicate_dqt_after_snapshot).hexdigest() != (
        "a73504634df562811870132182af22730318fa2f4416347770499aa412c71e56"
    ):
        raise RuntimeError("duplicate post-snapshot DQT fixture differs from its pinned hash")
    duplicate_dqt_path = d / "baseline_444_multiscan_duplicate_dqt_after_snapshot.jpg"
    duplicate_dqt_path.write_bytes(duplicate_dqt_after_snapshot)
    with Image.open(dqt_redefined_path) as reference, Image.open(duplicate_dqt_path) as variant:
        reference.load()
        variant.load()
        if (
            reference.mode != variant.mode
            or reference.size != variant.size
            or reference.tobytes() != variant.tobytes()
        ):
            raise RuntimeError("duplicate post-snapshot DQT changed Pillow pixels")
    (d / "baseline_444_multiscan_ss_nonzero.jpg").write_bytes(
        set_baseline_scan_spectral_start(three_scan_jpeg, 0, 1)
    )
    (d / "baseline_444_multiscan_duplicate_component.jpg").write_bytes(
        repeat_baseline_scan_component(three_scan_jpeg, 2, 2)
    )
    (d / "baseline_444_multiscan_missing_component.jpg").write_bytes(
        omit_final_baseline_scan(three_scan_jpeg)
    )
    (d / "baseline_444_multiscan_empty_entropy.jpg").write_bytes(
        empty_baseline_scan_entropy(three_scan_jpeg, 0)
    )
    (d / "baseline_444_multiscan_final_empty_entropy.jpg").write_bytes(
        empty_baseline_scan_entropy(three_scan_jpeg, 2)
    )
    img.save(d / "baseline_422.jpg", quality=85, subsampling=1)
    for source_name, output_name in (
        ("baseline_444.jpg", "baseline_444_entropy_truncated_tail_64.jpg"),
        ("baseline_422.jpg", "baseline_422_entropy_truncated_tail_64.jpg"),
    ):
        source = (d / source_name).read_bytes()
        if not source.endswith(b"\xff\xd9"):
            raise RuntimeError(f"{source_name} is missing its JPEG end marker")
        if len(source) <= 66:
            raise RuntimeError(f"{source_name} is too short for the entropy-tail fixture")
        (d / output_name).write_bytes(source[:-66] + b"\xff\xd9")
    pattern_img("RGB", (13, 9)).save(
        d / "baseline_444_13x9.jpg", quality=85, subsampling=0
    )
    pattern_img("RGB", (17, 9)).save(
        d / "baseline_422_17x9.jpg", quality=85, subsampling=1
    )
    pattern_img("RGB", (64, 1)).save(
        d / "baseline_422_aligned_64x1.jpg", quality=85, subsampling=1
    )
    pattern_img("RGB", (64, 8)).save(
        d / "baseline_422_aligned_64x8.jpg", quality=85, subsampling=1
    )
    img.save(d / "baseline_420.jpg", quality=85, subsampling=2)
    img.save(d / "baseline_411.jpg", quality=85, subsampling=2)
    img.save(
        d / "baseline_420_restart_blocks_3.jpg",
        quality=85,
        subsampling=2,
        restart_marker_blocks=3,
    )
    restart_420_source = BytesIO()
    img.save(
        restart_420_source,
        format="JPEG",
        quality=85,
        subsampling=2,
        restart_marker_blocks=8,
    )
    restart_420_base = restart_420_source.getvalue()
    if hashlib.sha256(restart_420_base).hexdigest() != (
        "a0ea540bcedbb4fdb195055eee29aa0e3f5b9a856e6bea195002be0d8f7d5f4c"
    ):
        raise RuntimeError("baseline 4:2:0 JPEG DRI 8 fixture differs from its pinned source")
    restart_420_overrun = append_restart_markers_before_eoi(restart_420_base)
    if hashlib.sha256(restart_420_overrun).hexdigest() != (
        "d52d934b119504b84ee1697bec2ce78077788e3a98ff8073a371a2a42b26715c"
    ):
        raise RuntimeError("baseline 4:2:0 JPEG restart-overrun fixture differs from its pinned hash")
    (d / "baseline_420_restart_blocks_8.jpg").write_bytes(restart_420_base)
    for restart_data in (restart_420_base, restart_420_overrun):
        with Image.open(BytesIO(restart_data)) as image:
            image.verify()
    with Image.open(BytesIO(restart_420_base)) as base_image:
        base_image.load()
        restart_420_reference = (
            base_image.format,
            base_image.mode,
            base_image.size,
            base_image.tobytes(),
        )
    with Image.open(BytesIO(restart_420_overrun)) as overrun_image:
        overrun_image.load()
        restart_420_candidate = (
            overrun_image.format,
            overrun_image.mode,
            overrun_image.size,
            overrun_image.tobytes(),
        )
    if restart_420_candidate != restart_420_reference:
        raise RuntimeError("extra baseline 4:2:0 JPEG restart markers changed Pillow pixels")
    if hashlib.sha256(restart_420_reference[3]).hexdigest() != (
        "e92abdc1f9d14fd2491920de98bdcee028788715bebc6ba5d21b20d89b59fd76"
    ):
        raise RuntimeError("baseline 4:2:0 JPEG DRI 8 Pillow pixels differ from their pinned hash")
    (d / "baseline_420_restart_overrun.jpg").write_bytes(restart_420_overrun)
    img.save(
        d / "baseline_422_restart_blocks_1.jpg",
        quality=85,
        subsampling=1,
        restart_marker_blocks=1,
    )
    img.save(
        d / "baseline_444_restart_blocks_1.jpg",
        quality=85,
        subsampling=0,
        restart_marker_blocks=1,
    )
    restart_fill_pixels = bytes(
        value
        for y in range(27)
        for x in range(35)
        for value in (
            (13 * x + 7 * y + 3 * x * y) & 0xFF,
            (5 * x ^ 11 * y ^ x * y) & 0xFF,
            (x * x + 17 * y + 29 * x * y) & 0xFF,
        )
    )
    restart_fill_image = Image.frombytes("RGB", (35, 27), restart_fill_pixels)
    restart_fill_source = BytesIO()
    restart_fill_image.save(
        restart_fill_source,
        format="JPEG",
        quality=88,
        subsampling=0,
        restart_marker_rows=1,
        optimize=False,
        progressive=False,
    )
    restart_fill_base = restart_fill_source.getvalue()
    if hashlib.sha256(restart_fill_base).hexdigest() != (
        "6255ac27399206fb9d899afcd08819366f9859d84ea268f3df862f0a799ae33b"
    ):
        raise RuntimeError("baseline 4:4:4 JPEG restart fixture differs from its pinned source")
    restart_markers = [
        restart_fill_base[index + 1]
        for index in range(len(restart_fill_base) - 1)
        if restart_fill_base[index] == 0xFF
        and 0xD0 <= restart_fill_base[index + 1] <= 0xD7
    ]
    if restart_markers != [0xD0, 0xD1, 0xD2]:
        raise RuntimeError(f"expected three ordered JPEG restart markers, found {restart_markers}")
    restart_fill_bytes = restart_fill_base
    for marker in restart_markers:
        marker_bytes = b"\xff" + bytes([marker])
        if restart_fill_bytes.count(marker_bytes) != 1:
            raise RuntimeError(f"expected one JPEG restart marker {marker:#04x}")
        restart_fill_bytes = restart_fill_bytes.replace(
            marker_bytes,
            b"\xff\xff" + bytes([marker]),
        )
    if hashlib.sha256(restart_fill_bytes).hexdigest() != (
        "c5369d29b879ca0d40ac01c927c4a5d845d3fc9f59ae07ccb7cb5947b0e5fee2"
    ):
        raise RuntimeError("JPEG restart-marker fill fixture differs from its pinned hash")
    with Image.open(BytesIO(restart_fill_bytes)) as decoded_restart_fill:
        decoded_restart_fill.load()
        restart_fill_reference = decoded_restart_fill.convert("RGB").tobytes()
    if hashlib.sha256(restart_fill_reference).hexdigest() != (
        "0276fe6a4734a34cee3009e2c6aca9c36323785712622759f3a8b9e835453895"
    ):
        raise RuntimeError("JPEG restart-marker fill changed pinned Pillow RGB pixels")
    (d / "baseline_444_restart_marker_fill.jpg").write_bytes(restart_fill_bytes)
    img.convert("L").save(d / "baseline_gray.jpg", quality=85)
    ac_pair_template = BytesIO()
    Image.new("L", (8, 8), 128).save(
        ac_pair_template,
        format="JPEG",
        quality=85,
        progressive=False,
        optimize=False,
    )
    ac_pair_tail_path = d / "baseline_ac_pair_tail.jpg"
    ac_pair_tail_path.write_bytes(
        build_baseline_ac_pair_tail_jpeg(ac_pair_template.getvalue())
    )
    if hashlib.sha256(ac_pair_tail_path.read_bytes()).hexdigest() != (
        "8009a091b2ba885e505d626f019e220a92deabcf79529ba9a96dacaa5518319d"
    ):
        raise RuntimeError("baseline AC-pair block-tail fixture differs from its pinned hash")
    reserved_ac_eob_path = d / "baseline_reserved_ac_zero_size_eob.jpg"
    reserved_ac_eob_path.write_bytes(build_baseline_reserved_ac_zero_size_eob_jpeg())
    multiscan_reserved_ac_eob_path = d / "baseline_multiscan_reserved_ac_zero_size_eob.jpg"
    multiscan_reserved_ac_eob_path.write_bytes(
        build_baseline_multiscan_reserved_ac_zero_size_eob_jpeg()
    )
    pattern_img("L", (13, 9)).save(d / "baseline_gray_13x9.jpg", quality=85)
    gray_sampling_2x1 = mutate_jpeg_payload(
        (d / "baseline_gray_13x9.jpg").read_bytes(), 0xC0, 7, 0x21
    )
    gray_sampling_2x1_path = d / "baseline_gray_sampling_2x1.jpg"
    gray_sampling_2x1_path.write_bytes(gray_sampling_2x1)
    if hashlib.sha256(gray_sampling_2x1).hexdigest() != (
        "7e53a6d3ccd0f5e39b8c5cb45e5b216814517619e12be65a03dc57710035c862"
    ):
        raise RuntimeError("grayscale 2x1 JPEG sampling fixture differs from its pinned hash")
    gray_sampling_1x2_template = BytesIO()
    with Image.open(d / "baseline_gray_13x9.jpg") as gray_source:
        gray_source.crop((0, 0, 8, 9)).convert("L").save(
            gray_sampling_1x2_template,
            format="JPEG",
            quality=90,
            progressive=False,
            optimize=False,
        )
    gray_sampling_1x2 = mutate_jpeg_payload(
        gray_sampling_1x2_template.getvalue(), 0xC0, 7, 0x12
    )
    gray_sampling_1x2_path = d / "baseline_gray_sampling_1x2.jpg"
    gray_sampling_1x2_path.write_bytes(gray_sampling_1x2)
    if hashlib.sha256(gray_sampling_1x2).hexdigest() != (
        "c53dedef12106902d0e1a2bf97b7dd3d349186731b9c429557eace04b6ec6e63"
    ):
        raise RuntimeError("grayscale 1x2 JPEG sampling fixture differs from its pinned hash")
    pattern_img("L", (32, 1)).save(d / "baseline_gray_aligned_32x1.jpg", quality=85)
    pattern_img("L", (32, 8)).save(d / "baseline_gray_aligned_32x8.jpg", quality=85)
    pattern_img("RGB", (17, 13)).save(
        d / "progressive_422_17x13.jpg",
        quality=85,
        progressive=True,
        subsampling=1,
    )
    pattern_img("RGB", (1, 8)).save(
        d / "progressive_422_1x8.jpg",
        quality=85,
        progressive=True,
        subsampling=1,
    )
    img.convert("CMYK").save(d / "baseline_cmyk.jpg", quality=85)
    baseline_cmyk = (d / "baseline_cmyk.jpg").read_bytes()
    cmyk_sampling_cases = {
        "baseline_cmyk_sampling_h2v1.jpg": (
            16,
            8,
            0x21,
            "585d3142a52ac621b982545f8766657fec92043af24b4b69c3cd103bf530c075",
        ),
        "baseline_cmyk_sampling_h1v2.jpg": (
            8,
            16,
            0x12,
            "a597e1486f0afb4308471fdab2828eeae2b52f9709a4740e0b0c38e39b4eaa79",
        ),
    }
    for filename, (width, height, sampling, expected_sha256) in cmyk_sampling_cases.items():
        candidate = zero_sample_cmyk_jpeg(
            baseline_cmyk, width, height, sampling
        )
        (d / filename).write_bytes(candidate)
        if hashlib.sha256(candidate).hexdigest() != expected_sha256:
            raise RuntimeError(f"{filename} differs from its pinned sampling fixture")
    cmyk_partial_blocks = pattern_img("CMYK", (13, 9))
    cmyk_partial_blocks.save(d / "baseline_cmyk_13x9.jpg", quality=85)
    pattern_img("CMYK", (32, 8)).save(
        d / "baseline_cmyk_aligned_32x8.jpg", quality=85
    )
    pattern_img("CMYK", (32, 9)).save(
        d / "baseline_cmyk_aligned_32x9.jpg", quality=85
    )
    pattern_img("CMYK", (8, 9)).save(
        d / "baseline_cmyk_height_partial_8x9.jpg", quality=85
    )
    cmyk_partial_bytes = (d / "baseline_cmyk_13x9.jpg").read_bytes()
    if len(cmyk_partial_bytes) <= 66 or cmyk_partial_bytes[-2:] != b"\xff\xd9":
        raise RuntimeError("baseline CMYK partial-block fixture has unexpected JPEG framing")
    (d / "baseline_cmyk_entropy_truncated_tail_64.jpg").write_bytes(
        cmyk_partial_bytes[:-66] + b"\xff\xd9"
    )
    cmyk_aligned_bytes = (d / "baseline_cmyk_aligned_32x8.jpg").read_bytes()
    if len(cmyk_aligned_bytes) <= 66 or cmyk_aligned_bytes[-2:] != b"\xff\xd9":
        raise RuntimeError("baseline aligned CMYK fixture has unexpected JPEG framing")
    cmyk_aligned_truncated = cmyk_aligned_bytes[:-66] + b"\xff\xd9"
    if hashlib.sha256(cmyk_aligned_truncated).hexdigest() != (
        "51f4f76d8a6144572eb0880c5722f290840db23c6fa5a83148ac1a1947d7445c"
    ):
        raise RuntimeError("baseline aligned CMYK truncated-tail fixture differs")
    (d / "baseline_cmyk_aligned_entropy_truncated_tail_64.jpg").write_bytes(
        cmyk_aligned_truncated
    )
    pattern_img("CMYK", (13, 9)).save(
        d / "baseline_cmyk_restart_rows.jpg",
        quality=85,
        restart_marker_rows=1,
    )
    (d / "cmyk_no_adobe_app14.jpg").write_bytes(
        remove_jpeg_segments((d / "baseline_cmyk.jpg").read_bytes(), 0xEE)
    )
    Image.new("L", (2048, 1024), 128).save(d / "progressive_eob_source.jpg", quality=85)
    img.save(d / "progressive.jpg", quality=85, progressive=True)
    img.save(d / "progressive_spectral.jpg", quality=70, progressive=True)
    progressive_jpeg = (d / "progressive.jpg").read_bytes()
    if not progressive_jpeg.endswith(b"\xff\xd9"):
        raise RuntimeError("progressive JPEG fixture is missing its final EOI marker")
    progressive_com_tail_junk = progressive_jpeg[:-2] + b"\xff\xfe\x00\x02TAIL"
    if hashlib.sha256(progressive_com_tail_junk).hexdigest() != (
        "70dd4ed893d23e30d3e5c27ef1ac3f55e6a1a98e3aad166610db48754ee43ed2"
    ):
        raise RuntimeError("progressive COM-tail fixture differs from its pinned hash")
    (d / "progressive_com_tail_junk.jpg").write_bytes(progressive_com_tail_junk)
    progressive_dqt_redefined = repeat_dqt_after_first_progressive_scan(
        progressive_jpeg
    )
    if hashlib.sha256(progressive_dqt_redefined).hexdigest() != (
        "353d4f1b20a5b73e473deef0d401dd91a9d2973c6065b0dfddd005edf21785da"
    ):
        raise RuntimeError("progressive DQT redefinition fixture differs from its pinned hash")
    (d / "progressive_dqt_redefined_between_scans.jpg").write_bytes(
        progressive_dqt_redefined
    )
    (d / "progressive_dc_only_smoothing.jpg").write_bytes(
        progressive_dc_only_jpeg(progressive_jpeg)
    )
    constant_progressive_stream = BytesIO()
    Image.new("L", (64, 64), 128).save(
        constant_progressive_stream, format="JPEG", quality=85, progressive=True
    )
    constant_dc_only = progressive_dc_only_jpeg(
        constant_progressive_stream.getvalue()
    )
    (d / "progressive_dc_only_zero_quant.jpg").write_bytes(
        zero_jpeg_quantization_table(constant_dc_only, 0)
    )
    (d / "dc_overflow_progressive_al13.jpg").write_bytes(
        build_progressive_dc_overflow_jcoef_compatibility_jpeg()
    )
    (d / "idct_high_horizontal_frequencies.jpg").write_bytes(
        build_baseline_idct_high_horizontal_frequencies_jpeg()
    )
    ac_first_rng = random.Random(6)
    ac_first_source = Image.new("L", (8, 8))
    ac_first_source.putdata([ac_first_rng.randrange(256) for _ in range(64)])
    ac_first_stream = BytesIO()
    ac_first_source.save(ac_first_stream, format="JPEG", quality=100, progressive=True)
    ac_first_source_data = ac_first_stream.getvalue()
    (d / "progressive_ac_first_narrow_tail.jpg").write_bytes(
        narrow_single_block_progressive_ac_first_tail(ac_first_source_data, 6)
    )
    (d / "progressive_dc_category_16.jpg").write_bytes(
        mutate_first_jpeg_dc_category(ac_first_source_data, 16)
    )
    refinement_rng = random.Random(0xAC_CE55)
    refinement_noise = Image.new("RGB", (64, 64))
    refinement_noise.putdata(
        [
            (
                refinement_rng.randrange(256),
                refinement_rng.randrange(256),
                refinement_rng.randrange(256),
            )
            for _ in range(64 * 64)
        ]
    )
    refinement_noise.save(
        d / "progressive_refinement_noise.jpg",
        quality=95,
        subsampling=0,
        progressive=True,
    )
    refinement_noise_jpeg = (d / "progressive_refinement_noise.jpg").read_bytes()
    d.joinpath("progressive_refinement_se64.jpg").write_bytes(
        mutate_progressive_ac_scan_end(refinement_noise_jpeg, 64)
    )
    d.joinpath("progressive_refinement_ss64.jpg").write_bytes(
        mutate_progressive_ac_scan_start(refinement_noise_jpeg, 64)
    )
    refinement_long_rng = random.Random(0xAC_CE56)
    refinement_long = Image.new("RGB", (128, 128))
    refinement_long.putdata(
        [
            (
                refinement_long_rng.randrange(256),
                refinement_long_rng.randrange(256),
                refinement_long_rng.randrange(256),
            )
            for _ in range(128 * 128)
        ]
    )
    refinement_long.save(
        d / "progressive_refinement_long_scan.jpg",
        quality=95,
        subsampling=0,
        progressive=True,
    )
    Image.new("L", (64, 64), 128).save(
        d / "progressive_refinement_eobrun.jpg",
        quality=85,
        progressive=True,
    )
    refinement_eobrun_correction = Image.new("L", (16, 8))
    refinement_eobrun_correction_pixels = []
    for y in range(8):
        for x in range(16):
            u, v = x % 8, y % 8
            sample = 128 + 36 * math.cos((2 * u + 1) * math.pi / 16)
            sample += 18 * math.cos((2 * v + 1) * math.pi / 8)
            sample += 7 * math.cos((2 * u + 1) * 5 * math.pi / 16) * math.cos(
                (2 * v + 1) * 3 * math.pi / 16
            )
            refinement_eobrun_correction_pixels.append(
                max(0, min(255, round(sample)))
            )
    refinement_eobrun_correction.putdata(refinement_eobrun_correction_pixels)
    refinement_eobrun_correction_path = (
        d / "progressive_refinement_eobrun_correction_bit.jpg"
    )
    refinement_eobrun_correction.save(
        refinement_eobrun_correction_path,
        quality=75,
        progressive=True,
        optimize=False,
    )
    if hashlib.sha256(refinement_eobrun_correction_path.read_bytes()).hexdigest() != (
        "d1823abbbd3f9b78b440eb25abbb38b73db7dc30be1457d013131d481038c3eb"
    ):
        raise RuntimeError("progressive EOBRUN correction fixture differs from its pinned hash")
    refinement_zrl_source = Image.new("L", (8, 8))
    refinement_zrl_pixels = []
    for y in range(8):
        for x in range(8):
            sample = 128 + 2 * math.cos((2 * x + 1) * math.pi / 16)
            sample += 5 * math.cos((2 * x + 1) * 2 * math.pi / 16) * math.cos(
                (2 * y + 1) * 4 * math.pi / 16
            )
            refinement_zrl_pixels.append(max(0, min(255, round(sample))))
    refinement_zrl_source.putdata(refinement_zrl_pixels)
    refinement_zrl_source.save(
        d / "progressive_refinement_refine_zrl.jpg",
        quality=100,
        progressive=True,
    )
    (d / "progressive_refinement_zrl_symbol.jpg").write_bytes(
        build_progressive_refinement_zrl_symbol_jpeg()
    )
    # One vertical DCT frequency leaves sparse late-band coefficients; keep
    # the scan's ZRL and correction-bit behavior pinned by the encoded hash.
    refinement_zrl = Image.new("L", (64, 64))
    refinement_zrl_pixels = []
    for y in range(64):
        sample = round(
            128 + math.cos((2 * (y % 8) + 1) * 5 * math.pi / 16)
        )
        refinement_zrl_pixels.extend([sample] * 64)
    refinement_zrl.putdata(refinement_zrl_pixels)
    refinement_zrl.save(
        d / "progressive_refinement_zrl_correction.jpg",
        quality=100,
        progressive=True,
    )
    refinement_zrl_data = (d / "progressive_refinement_zrl_correction.jpg").read_bytes()
    refinement_zrl_scans, _ = jpeg_scan_segments(refinement_zrl_data)
    if (
        len(refinement_zrl_scans) != 6
        or refinement_zrl_scans[-1][2:5] != (1, 63, 0x10)
        or hashlib.sha256(refinement_zrl_data).hexdigest()
        != "bd323d26ab6a28556be36d26907a2e660a2a3032c5726c618d926cbb1225bb64"
    ):
        raise RuntimeError("progressive AC-refinement ZRL witness differs")
    sparse_refinement = Image.new("RGB", (64, 64))

    def sparse_refinement_pixel(x, y):
        x = x % 8
        y = y % 8
        low_frequency = 32 * math.cos((2 * x + 1) * math.pi / 16) * math.cos(
            (2 * y + 1) * 2 * math.pi / 16
        )
        high_frequency = 32 * math.cos((2 * x + 1) * 7 * math.pi / 16) * math.cos(
            (2 * y + 1) * 7 * math.pi / 16
        )
        sample = max(0, min(255, round(128 + low_frequency + high_frequency)))
        return sample, sample, sample

    sparse_refinement.putdata(
        [
            sparse_refinement_pixel(x, y)
            for y in range(64)
            for x in range(64)
        ]
    )
    sparse_refinement.save(
        d / "progressive_refinement_zrl.jpg",
        quality=100,
        subsampling=0,
        progressive=True,
    )
    sparse_refinement_jpeg = (d / "progressive_refinement_zrl.jpg").read_bytes()
    d.joinpath("progressive_refinement_zrl_duplicate_tail.jpg").write_bytes(
        duplicate_last_progressive_ac_refinement_scan(sparse_refinement_jpeg)
    )
    d.joinpath("progressive_refinement_narrow_tail.jpg").write_bytes(
        narrow_last_progressive_ac_refinement_scan(refinement_noise_jpeg, 1)
    )
    img.convert("L").save(d / "progressive_gray.jpg", quality=85, progressive=True)
    progressive_cmyk_path = d / "progressive_cmyk.jpg"
    img.convert("CMYK").save(progressive_cmyk_path, quality=85, progressive=True)
    progressive_cmyk = progressive_cmyk_path.read_bytes()
    if len(jpeg_segments(progressive_cmyk, 0xEE)) != 1:
        raise RuntimeError("progressive CMYK source must contain exactly one APP14 segment")
    d.joinpath("progressive_cmyk_no_app14.jpg").write_bytes(
        remove_jpeg_segments(progressive_cmyk, 0xEE)
    )
    progressive_restart_path = d / "progressive_restart.jpg"
    img.save(
        progressive_restart_path,
        quality=85,
        progressive=True,
        restart_marker_rows=2,
    )
    progressive_restart = progressive_restart_path.read_bytes()
    progressive_restart_overrun = append_progressive_restart_overrun(
        progressive_restart
    )
    with Image.open(BytesIO(progressive_restart)) as reference, Image.open(
        BytesIO(progressive_restart_overrun)
    ) as variant:
        reference.load()
        variant.load()
        reference_result = (
            reference.format,
            reference.mode,
            reference.size,
            reference.info,
            reference.getexif(),
            reference.tobytes(),
        )
        variant_result = (
            variant.format,
            variant.mode,
            variant.size,
            variant.info,
            variant.getexif(),
            variant.tobytes(),
        )
    if variant_result != reference_result:
        raise RuntimeError(
            "progressive restart overrun markers changed Pillow's observable image result"
        )
    (d / "progressive_restart_overrun.jpg").write_bytes(
        progressive_restart_overrun
    )
    img.save(d / "restart.jpg", quality=85, restart_marker_rows=4)
    pattern_img("RGB", (1, 1)).save(d / "1x1.jpg", quality=95)
    pattern_img("RGB", (1, 8)).save(
        d / "1x8_422.jpg", quality=95, subsampling=1
    )
    pattern_img("RGB", (8, 8)).save(d / "8x8.jpg", quality=95)
    pattern_img("RGB", (16, 2)).save(
        d / "rgb_16x2.jpg", quality=95, subsampling=0
    )
    pattern_img("RGB", (16, 1)).save(
        d / "rgb_16x1.jpg", quality=95, subsampling=0
    )
    pattern_img("RGB", (32, 33)).save(
        d / "rgb_32x33.jpg", quality=95, subsampling=0
    )
    pattern_img("RGB", (32, 32)).save(
        d / "rgb_32x32.jpg", quality=95, subsampling=0
    )
    pattern_img("RGB", (17, 2)).save(
        d / "rgb_17x2.jpg", quality=95, subsampling=0
    )
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
    rgb_sampling_cases = {
        "baseline_420_cb_h2v1.jpg": (
            16,
            16,
            0x22,
            0x21,
            0x11,
            "f3a81acda25d264ea4332f3bc3eb955fe753761e196d4370a1661f47c2762924",
        ),
        "baseline_420_singleton_cr_h2v2.jpg": (
            1,
            1,
            0x22,
            0x21,
            0x11,
            "186c14e5cf2221a8ce7e594a597a170994a92beb7617854ac28fa7ef4e418f1b",
        ),
        "baseline_420_cb_h1v2.jpg": (
            16,
            16,
            0x22,
            0x12,
            0x11,
            "52fd157d2c927d22b4d45462a4dcb3bbdb93e5dd8b31b344a08cc2d511f276df",
        ),
        "baseline_420_cr_h2v1.jpg": (
            16,
            16,
            0x22,
            0x11,
            0x21,
            "263c58c88a6d4eced5fc0762d4afc9a13014166e4076a82caff1d93d6a9c70e0",
        ),
        "baseline_420_cr_h1v2.jpg": (
            16,
            16,
            0x22,
            0x11,
            0x12,
            "31eae23e0a33bf4f35af76fe78cb1599e70f8f1addff66140ab15cea31c46834",
        ),
        "baseline_422_cb_h2v1.jpg": (
            16,
            8,
            0x21,
            0x21,
            0x11,
            "1fab1077eeb1ce0137e17be94fbf945530557ad136bba175433f5e7cc88b09b8",
        ),
        "baseline_422_cb_h1v2.jpg": (
            16,
            8,
            0x21,
            0x12,
            0x11,
            "6d0c0ed5e190c13203ac8b7bec2d82198360b8c9b8171e6f9692b1c4ca01e0b2",
        ),
        "baseline_422_cr_h2v1.jpg": (
            16,
            8,
            0x21,
            0x11,
            0x21,
            "8ca39b3c24ce089b017d041e27c1f5e97559dbf026192bf530d15d6045f9cb93",
        ),
        "baseline_422_cr_h1v2.jpg": (
            16,
            8,
            0x21,
            0x11,
            0x12,
            "24de32c272eb278aad80c139c339f8c40fe7d853649441908bbd73fbe4c1ff28",
        ),
    }
    for filename, (width, height, y, cb, cr, expected_sha256) in rgb_sampling_cases.items():
        candidate = zero_sample_jpeg(baseline, width, height, y, cb, cr)
        (d / filename).write_bytes(candidate)
        if hashlib.sha256(candidate).hexdigest() != expected_sha256:
            raise RuntimeError(f"{filename} differs from its pinned sampling fixture")

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

    sampling_3x1 = zero_sample_jpeg(baseline, 24, 8, 0x31)
    d.joinpath("sampling_3x1.jpg").write_bytes(sampling_3x1)
    d.joinpath("empty_dc_huffman_table.jpg").write_bytes(
        empty_jpeg_huffman_table(sampling_3x1, 0, 0)
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
    gray_scans, gray_eoi_start = jpeg_scan_segments(baseline_gray)
    if len(gray_scans) != 1 or gray_scans[0][2:5] != (0, 63, 0):
        raise ValueError("expected a single baseline grayscale entropy scan")
    _, _, gray_sos_end = jpeg_segment(baseline_gray, 0xDA)
    # Two stuffed FF data bytes provide a complete all-ones 16-bit code and
    # exercise libjpeg's overlong-code recovery without corrupting markers.
    d.joinpath("entropy_all_ones_huffman.jpg").write_bytes(
        baseline_gray[:gray_sos_end]
        + b"\xff\x00\xff\x00"
        + baseline_gray[gray_eoi_start:]
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
    d.joinpath("missing_eoi_dangling_ff.jpg").write_bytes(baseline[:-2] + b"\xff")
    d.joinpath("missing_eoi_stuffed_ff_tail.jpg").write_bytes(
        baseline[:-2] + b"\xff\x00"
    )
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


def gen_webp_lossless_sampling_equal_rows(directory):
    """Create equal tile-histogram rows for token-aware VP8L sampling."""
    directory.mkdir(parents=True, exist_ok=True)
    width, height = 8_192, 16
    pixels = bytearray(width * height * 3)
    tile_values = {
        tile_x: random.Random(tile_x * 1_009 + 0x5EED).sample(range(256), 64)
        for tile_x in range(width // 8)
    }
    permutations = {
        (tile_x, tile_y): random.Random(
            tile_x * 1_009 + tile_y * 0x1_0003 + 0x5EED
        ).sample(tile_values[tile_x], 64)
        for tile_y in range(height // 8)
        for tile_x in range(width // 8)
    }
    for y in range(height):
        tile_y, local_y = divmod(y, 8)
        for x in range(width):
            tile_x, local_x = divmod(x, 8)
            group = (tile_x // 256) & 1
            pixel_in_tile = local_y * 8 + local_x
            value = permutations[tile_x, tile_y][pixel_in_tile]
            rgb = (value, 0, 0) if group == 0 else (0, value, 0)
            offset = (y * width + x) * 3
            pixels[offset : offset + 3] = bytes(rgb)
    image = Image.frombytes("RGB", (width, height), bytes(pixels))
    image.save(directory / "webp_lossless_sampling_equal_rows_checkpoint.png")


def gen_png():
    d = OUT / "png"; d.mkdir(parents=True, exist_ok=True)
    img = pattern_img("RGB")
    img.save(d / "16x16.png")
    img.save(d / "rgb.png")
    pattern_img("RGB", (49, 32)).save(d / "rgb_49x32.png")
    webp_alpha_size = (64, 64)
    webp_alpha_rgb = (37, 91, 143)
    webp_alpha_checker = Image.new("RGBA", webp_alpha_size)
    webp_alpha_checker.putdata(
        [
            (*webp_alpha_rgb, 32 if (x + y) % 2 else 224)
            for y in range(webp_alpha_size[1])
            for x in range(webp_alpha_size[0])
        ]
    )
    webp_alpha_checker.save(d / "webp_rgba_alpha_compressible_checker.png")
    webp_alpha_noise_rng = random.Random(0xA17A)
    webp_alpha_noise = Image.new("RGBA", webp_alpha_size)
    webp_alpha_noise.putdata(
        [
            (*webp_alpha_rgb, webp_alpha_noise_rng.randrange(256))
            for _ in range(webp_alpha_size[0] * webp_alpha_size[1])
        ]
    )
    webp_alpha_noise.save(d / "webp_rgba_alpha_noisy_seeded.png")
    webp_alpha_tie = Image.new("RGBA", (9, 1), (*webp_alpha_rgb, 128))
    webp_alpha_tie.save(d / "webp_rgba_alpha_size_tie_9x1.png")
    sampling_width, sampling_height = 8_192, 16
    sampling_pixels = []
    sampling_tile_columns = sampling_width // 8
    for tile_y in range(sampling_height // 8):
        for local_y in range(8):
            for tile_x in range(sampling_tile_columns):
                group = tile_x // 512
                local_tile_x = tile_x % 512
                red_offset, green_offset, blue_offset = (
                    (0, 0, 0) if group == 0 else (128, 64, 192)
                )
                marker_id = tile_y * sampling_tile_columns + tile_x
                marker = (
                    64 + (marker_id >> 8),
                    128 + ((marker_id >> 4) & 0x0F),
                    128 + (marker_id & 0x0F),
                )
                for local_x in range(8):
                    pixel_in_tile = local_y * 8 + local_x
                    if pixel_in_tile < 63:
                        sampling_pixels.append(
                            (
                                red_offset + pixel_in_tile,
                                green_offset
                                + (pixel_in_tile + local_tile_x % 63) % 63,
                                blue_offset
                                + (2 * pixel_in_tile + local_tile_x // 63 + tile_y * 9)
                                % 63,
                            )
                        )
                    else:
                        sampling_pixels.append(marker)
    sampling_image = Image.new("RGB", (sampling_width, sampling_height))
    sampling_image.putdata(sampling_pixels)
    sampling_image.save(d / "webp_lossless_sampling_checkpoint.png")
    sampling_groups_width, sampling_groups_height = 8_192, 16
    sampling_groups_pixels = bytearray(
        sampling_groups_width * sampling_groups_height * 3
    )
    sampling_groups_permutations = {
        (tile_y, tile_x): random.Random(
            tile_y * 100_003 + tile_x * 1_009 + 0x5EED
        ).sample(range(64), 64)
        for tile_y in range(sampling_groups_height // 8)
        for tile_x in range(sampling_groups_width // 8)
    }
    for y in range(sampling_groups_height):
        tile_y, local_y = divmod(y, 8)
        for x in range(sampling_groups_width):
            tile_x, local_x = divmod(x, 8)
            group = (tile_x // 32) & 1
            pixel_in_tile = local_y * 8 + local_x
            value = sampling_groups_permutations[tile_y, tile_x][pixel_in_tile]
            rgb = (value, 0, 0) if group == 0 else (0, value, 0)
            offset = (y * sampling_groups_width + x) * 3
            sampling_groups_pixels[offset : offset + 3] = bytes(rgb)
    sampling_groups_image = Image.frombytes(
        "RGB",
        (sampling_groups_width, sampling_groups_height),
        bytes(sampling_groups_pixels),
    )
    sampling_groups_image.save(
        d / "webp_lossless_sampling_groups_checkpoint.png"
    )
    gen_webp_lossless_sampling_equal_rows(d)
    sampling_rows_width, sampling_rows_height = 512, 1_024
    sampling_rows_pixels = bytearray(sampling_rows_width * sampling_rows_height * 3)
    for tile_y in range(sampling_rows_height // 8):
        tile_permutations = [
            random.Random(tile_y * 100_003 + tile_x * 1_009 + 0x5EED).sample(
                range(64), 64
            )
            for tile_x in range(sampling_rows_width // 8)
        ]
        for local_y in range(8):
            for tile_x in range(sampling_rows_width // 8):
                # Two repeated histogram bands compress to a four-row map;
                # keeping the source tall crosses nonzero compaction rows.
                group = tile_x // 32
                for local_x in range(8):
                    pixel_in_tile = local_y * 8 + local_x
                    value = tile_permutations[tile_x][pixel_in_tile]
                    offset = (
                        (tile_y * 8 + local_y) * sampling_rows_width + tile_x * 8 + local_x
                    ) * 3
                    sampling_rows_pixels[offset : offset + 3] = (
                        bytes((value, 0, 0)) if group == 0 else bytes((0, value, 0))
                    )
    sampling_rows_image = Image.frombytes(
        "RGB",
        (sampling_rows_width, sampling_rows_height),
        bytes(sampling_rows_pixels),
    )
    sampling_rows_image.save(d / "webp_lossless_sampling_rows_checkpoint.png")
    sampling_tile_columns = sampling_width // 8
    sampling_tile_rows = sampling_height // 8
    alpha_tiles = []
    for tile_y in range(sampling_tile_rows):
        alpha_row = []
        for tile_x in range(sampling_tile_columns):
            group = tile_x // 512
            local_tile_x = tile_x % 512
            alpha = list(range(128, 192))
            random.Random(
                tile_y * sampling_tile_columns + local_tile_x * 2 + group
            ).shuffle(alpha)
            alpha_row.append(alpha)
        alpha_tiles.append(alpha_row)
    gray_alpha_pixels = []
    # Cross one token checkpoint before the scan finds varying alpha.
    opaque_prefix_pixels = 1_024
    for tile_y in range(sampling_tile_rows):
        for local_y in range(8):
            for tile_x in range(sampling_tile_columns):
                group = tile_x // 512
                alpha = alpha_tiles[tile_y][tile_x]
                for local_x in range(8):
                    pixel_in_tile = local_y * 8 + local_x
                    level = (pixel_in_tile % 63) + (128 if group else 0)
                    pixel_index = len(gray_alpha_pixels)
                    alpha_value = (
                        255
                        if pixel_index < opaque_prefix_pixels
                        else alpha[pixel_in_tile]
                    )
                    gray_alpha_pixels.append(
                        (level, level, level, alpha_value)
                    )
    gray_alpha_image = Image.new("RGBA", (sampling_width, sampling_height))
    gray_alpha_image.putdata(gray_alpha_pixels)
    gray_alpha_image.save(d / "webp_lossless_gray_alpha_predictor.png")
    rgba_channel_mismatch = pattern_img("RGBA", (32, 32))
    rgba_channel_mismatch.save(d / "webp_lossless_rgba_red_green_mismatch.png")
    rgba_green_blue_mismatch = rgba_channel_mismatch.copy()
    rgba_green_blue_mismatch.putpixel((2, 1), (8, 8, 9, 255))
    rgba_green_blue_mismatch.save(d / "webp_lossless_rgba_green_blue_mismatch.png")
    img.save(d / "fdat_without_actl_before_idat.png")
    insert_png_chunks(
        d / "fdat_without_actl_before_idat.png",
        [(b"fdAT", struct.pack(">I", 0))],
    )
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
    gif_balanced_checkerboard = Image.new("RGB", (16, 16))
    gif_balanced_checkerboard.putdata(
        [
            (0, 0, 0) if (x + y) % 2 == 0 else (255, 255, 255)
            for y in range(16)
            for x in range(16)
        ]
    )
    gif_balanced_checkerboard.save(d / "gif_rgb_balanced_checkerboard.png")
    high_color = Image.new("RGB", (17, 17))
    high_color.putdata(
        [
            ((x * 13 + y * 7) & 255, (x * 5 + y * 17) & 255, (x * 19 + y * 3) & 255)
            for y in range(17)
            for x in range(17)
        ]
    )
    high_color.save(d / "gif_rgb_high_color.png")
    high_color_checkpoint = Image.new("RGB", (33, 33))
    high_color_checkpoint.putdata(
        [
            ((x * 13 + y * 7) & 255, (x * 5 + y * 17) & 255, (x * 19 + y * 3) & 255)
            for y in range(33)
            for x in range(33)
        ]
    )
    high_color_checkpoint.save(d / "gif_rgb_high_color_token_checkpoint.png")
    skewed_colors = [(red, 0, 0) for red in range(256)]
    skewed_colors.append((254, 0, 1))
    dominant_color = (255, 0, 0)
    skewed_median_cut = Image.new("RGB", (32, 41))
    skewed_median_cut.putdata(skewed_colors + [dominant_color] * 1_055)
    skewed_median_cut.save(d / "gif_rgb_skewed_high_color_split.png")
    # Skew a distinct 257-color palette toward the low-green endpoint.
    endpoint_colors = [(0, green, 0) for green in range(1, 256)]
    endpoint_colors.extend([(0, 0, 0), (1, 0, 0)])
    endpoint_dominant = Image.new("RGB", (40, 16))
    endpoint_dominant.putdata(endpoint_colors + [(1, 0, 0)] * 383)
    endpoint_dominant.save(d / "gif_rgb_257_color_low_green_dominant.png")
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
    gif_rgba_mixed_colors = (
        (9, 8, 7, 0),
        (255, 0, 0, 255),
        (0, 255, 0, 255),
        (255, 0, 0, 255),
        (1, 2, 3, 127),
        (0, 0, 255, 128),
        (0, 255, 0, 255),
        (255, 255, 255, 255),
    )
    gif_rgba_mixed = Image.new("RGBA", (4, 2))
    gif_rgba_mixed.putdata(gif_rgba_mixed_colors)
    gif_rgba_mixed.save(d / "gif_rgba_mixed.png")
    gif_rgba_mixed_checkpoint = Image.new("RGBA", (33, 33))
    gif_rgba_mixed_checkpoint.putdata(
        [
            gif_rgba_mixed_colors[index % len(gif_rgba_mixed_colors)]
            for index in range(33 * 33)
        ]
    )
    gif_rgba_mixed_checkpoint.save(d / "gif_rgba_mixed_token_compact_checkpoint.png")
    gif_rgba_high_color = high_color.convert("RGBA")
    gif_rgba_high_color.putpixel((0, 0), (17, 19, 23, 0))
    gif_rgba_high_color.save(d / "gif_rgba_high_color.png")
    gif_rgba_checkpoint = Image.new("RGBA", (33, 33))
    gif_rgba_checkpoint.putdata(
        [
            (17, 19, 23, 0)
            if x == 0 and y == 0
            else (
                (13 * x + 7 * y) & 255,
                (5 * x + 17 * y) & 255,
                (19 * x + 3 * y) & 255,
                255,
            )
            for y in range(33)
            for x in range(33)
        ]
    )
    gif_rgba_checkpoint.save(d / "gif_rgba_high_color_token_checkpoint.png")
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
    adam7_gray_width = adam7_gray_height = 9
    adam7_gray_samples = [
        (x * 3 + y * 5) & 15
        for y in range(adam7_gray_height)
        for x in range(adam7_gray_width)
    ]
    write_gray_adam7_png(
        d / "adam7_gray_4bit.png",
        adam7_gray_width,
        adam7_gray_height,
        adam7_gray_samples,
    )
    adam7_gray16_samples = [
        (x * 257 + y * 4099) & 0xFFFF
        for y in range(adam7_gray_height)
        for x in range(adam7_gray_width)
    ]
    write_gray16_adam7_png(
        d / "adam7_gray_16bit.png",
        adam7_gray_width,
        adam7_gray_height,
        adam7_gray16_samples,
    )
    write_rgb_png(d / "no_interlace.png", img)
    # Chunks
    from PIL.PngImagePlugin import PngInfo
    meta = PngInfo()
    meta.add_text("Comment", "test")
    meta.add_text("CompressedComment", "test", zip=True)
    meta.add_itxt("International", "test", lang="en", tkey="Translation")
    img.save(d / "text_chunks.png", pnginfo=meta)
    text_metadata_edges = [
        (b"tEXt", b"Comment\0good"),
        (b"zTXt", b"Comment\0\0" + zlib.compress(b"good")),
        (b"zTXt", b"Bad\0\0invalid-zlib"),
        (b"zTXt", b"\0\0invalid-zlib"),
        (
            b"iTXt",
            b"Good\0\x01\x00en\0Translated\0" + zlib.compress(b"good"),
        ),
        (b"iTXt", b"Bad\0\x01\x00\0\0Translation\0invalid-zlib"),
        (b"iTXt", b"Method\0\x01\x01\0\0Translation\0ignored"),
        (b"iTXt", b"Flag\0\x02\x00\0\0Translation\0uncompressed"),
        (b"iTXt", b"\0\x01\x00\0\0\0" + zlib.compress(b"empty keyword")),
    ]
    img.save(d / "text_metadata_edges.png")
    insert_png_chunks(d / "text_metadata_edges.png", text_metadata_edges)
    img.save(d / "unknown_ancillary_reserved_bit.png")
    insert_png_chunks(
        d / "unknown_ancillary_reserved_bit.png",
        [(b"vpag", b"preserve-pixels")],
    )
    img.save(d / "unknown_ancillary_after_idat.png")
    insert_png_chunks_before_iend(
        d / "unknown_ancillary_after_idat.png",
        [(b"abCd", b"post-idat")],
    )
    bad_post_idat_crc = corrupt_png_chunk_crc(
        (d / "unknown_ancillary_after_idat.png").read_bytes(), b"abCd"
    )
    if hashlib.sha256(bad_post_idat_crc).hexdigest() != (
        "bfaa04b081a9f422ba6790b729cdbe2e85949b100c0da6572e0207aebb430e4d"
    ):
        raise RuntimeError("post-IDAT ancillary bad-CRC fixture differs from its pinned hash")
    (d / "unknown_ancillary_after_idat_bad_crc.png").write_bytes(bad_post_idat_crc)
    srgb = PngInfo()
    srgb.add(b"sRGB", b"\0")
    img.save(d / "srgb.png", pnginfo=srgb)
    img.save(d / "srgb_duplicate.png")
    insert_png_chunks(
        d / "srgb_duplicate.png",
        [(b"sRGB", b"\0"), (b"sRGB", b"\0")],
    )
    img.save(d / "srgb_invalid_intent.png")
    insert_png_chunks(d / "srgb_invalid_intent.png", [(b"sRGB", b"\x04")])
    img.save(d / "srgb_short_payload.png")
    insert_png_chunks(d / "srgb_short_payload.png", [(b"sRGB", b"\0\x01")])
    img.save(d / "iccp.png", icc_profile=b"pillow-rs-test-profile")
    iccp_payload = next(
        payload
        for kind, payload in png_chunks((d / "iccp.png").read_bytes())
        if kind == b"iCCP"
    )
    img.save(d / "iccp_duplicate.png")
    insert_png_chunks(
        d / "iccp_duplicate.png",
        [(b"iCCP", iccp_payload), (b"iCCP", iccp_payload)],
    )
    img.save(d / "iccp_empty_keyword.png")
    insert_png_chunks(
        d / "iccp_empty_keyword.png",
        [(b"iCCP", b"\0\0" + zlib.compress(b"empty keyword profile"))],
    )
    img.save(d / "iccp_empty_keyword_unsupported_method.png")
    insert_png_chunks(
        d / "iccp_empty_keyword_unsupported_method.png",
        [(b"iCCP", b"\0\x01junk")],
    )
    img.save(d / "iccp_invalid_compression.png")
    insert_png_chunks(
        d / "iccp_invalid_compression.png",
        [(b"iCCP", b"Test\0\0invalid-zlib")],
    )
    img.save(d / "iccp_missing_nul_separator.png")
    insert_png_chunks(
        d / "iccp_missing_nul_separator.png",
        [(b"iCCP", b"Profile")],
    )
    img.save(d / "iccp_missing_compression_method.png")
    insert_png_chunks(
        d / "iccp_missing_compression_method.png",
        [(b"iCCP", b"Profile\0")],
    )
    img.save(d / "gama_short_payload.png")
    insert_png_chunks(d / "gama_short_payload.png", [(b"gAMA", b"\0\0\0")])
    img.save(d / "ztxt_invalid_compression.png")
    insert_png_chunks(
        d / "ztxt_invalid_compression.png",
        [(b"zTXt", b"Comment\0\0invalid-zlib")],
    )
    img.save(d / "ztxt_unsupported_method.png")
    insert_png_chunks(
        d / "ztxt_unsupported_method.png",
        [(b"zTXt", b"Comment\0\x01unsupported-method")],
    )
    img.save(d / "itxt_invalid_compression.png")
    insert_png_chunks(
        d / "itxt_invalid_compression.png",
        [(b"iTXt", b"Comment\0\x01\x00\0\0Translation\0invalid-zlib")],
    )
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
    img.save(d / "gama_duplicate.png")
    gamma_payload = struct.pack(">I", 45_455)
    insert_png_chunks(
        d / "gama_duplicate.png",
        [(b"gAMA", gamma_payload), (b"gAMA", gamma_payload)],
    )
    chromaticities = struct.pack(
        ">8I", 31_270, 32_900, 64_000, 33_000, 30_000, 60_000, 15_000, 6_000
    )
    chrm = PngInfo()
    chrm.add(b"cHRM", chromaticities)
    img.save(d / "chrm.png", pnginfo=chrm)
    img.save(d / "chrm_duplicate.png")
    insert_png_chunks(
        d / "chrm_duplicate.png",
        [(b"cHRM", chromaticities), (b"cHRM", chromaticities)],
    )
    img.save(d / "chrm_short.png")
    insert_png_chunks(d / "chrm_short.png", [(b"cHRM", b"\0" * 8)])
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
    (d / "plte_trns_after_idat.png").write_bytes((d / "1x1.png").read_bytes())
    insert_png_chunks_before_iend(
        d / "plte_trns_after_idat.png",
        [(b"PLTE", b"\xff\x00\x00"), (b"tRNS", b"\x00" * 6)],
    )
    Image.new("RGB", (17,17), (128,0,0)).save(d / "odd_size.png")
    pattern_img("RGB", (2, 3)).save(d / "2x3.png")
    pattern_img("RGB", (1, 255)).save(d / "1x255.png")
    pattern_img("RGB", (255, 1)).save(d / "255x1.png")
    Image.new("RGB", (513,257), (128,0,0)).save(d / "large.png")
    # APNG-compatible files. Pillow writes a normal PNG when save_all is false.
    img.save(d / "apng_static.png")
    img2 = pattern_img("RGB").transpose(Image.Transpose.FLIP_LEFT_RIGHT)
    img.save(d / "apng_animated.png", save_all=True, append_images=[img2], duration=100, loop=0)
    img.save(
        d / "apng_unknown_ancillary.png",
        save_all=True,
        append_images=[img2],
        duration=100,
        loop=0,
    )
    insert_png_chunks(
        d / "apng_unknown_ancillary.png",
        [(b"vpag", b"preserve-pixels")],
    )

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
    before_idat = png_chunks(animated)
    idat_index = next(
        index for index, (kind, _) in enumerate(before_idat) if kind == b"IDAT"
    )
    for index in range(idat_index, len(before_idat)):
        kind, payload = before_idat[index]
        if kind in (b"fcTL", b"fdAT"):
            sequence = struct.unpack(">I", payload[:4])[0] + 1
            before_idat[index] = (kind, struct.pack(">I", sequence) + payload[4:])
    before_idat.insert(idat_index, (b"fdAT", struct.pack(">I", 1)))
    (d / "apng_empty_fdat_before_idat.png").write_bytes(rebuild_png(before_idat))
    truncated_fdat_before_idat = list(before_idat)
    truncated_fdat_before_idat[idat_index] = (b"fdAT", b"\x00\x00\x01")
    (d / "apng_truncated_fdat_before_idat.png").write_bytes(
        rebuild_png(truncated_fdat_before_idat)
    )
    nonempty_fdat_before_idat = list(before_idat)
    nonempty_fdat_before_idat[idat_index] = (
        b"fdAT",
        struct.pack(">I", 1) + b"\x00",
    )
    fdat_payload_path = d / "apng_fdat_payload_before_idat.png"
    fdat_payload_path.write_bytes(rebuild_png(nonempty_fdat_before_idat))
    with Image.open(fdat_payload_path) as oracle:
        try:
            oracle.load()
        except OSError as error:
            if str(error) != "broken data stream when reading image file":
                raise RuntimeError("APNG fdAT payload fixture changed its Pillow error") from error
        else:
            raise RuntimeError("APNG fdAT payload fixture unexpectedly decoded in Pillow")
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
    (d / "apng_fdat_without_fctl_bad_sequence.png").write_bytes(
        rebuild_png(no_control)
    )
    # Preserve sequence validity so the parser reaches the missing-fcTL path.
    no_control_data = mutate_png_chunk(
        rebuild_png(no_control),
        b"fdAT",
        0,
        lambda payload: payload.__setitem__(slice(0, 4), struct.pack(">I", 1)),
    )
    (d / "apng_fdat_without_fctl.png").write_bytes(no_control_data)
    (d / "apng_short_fdat_without_fctl.png").write_bytes(
        mutate_png_chunk(
            no_control_data,
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
    incomplete_image_data_header = struct.pack(">IIBBBBB", 1, 2, 8, 2, 0, 0, 0)
    (d / "incomplete_image_data_no_iend.png").write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", incomplete_image_data_header)
        + png_chunk(b"IDAT", zlib.compress(b"\x00\x80\x00\x00")[:-1])
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


def write_gif_lzw_fixture(path, width, codes, minimum_code_size=2, height=1):
    """Write a four-color GIF around an explicit LZW code stream."""
    palette = bytes((0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 255, 0))
    payload = pack_gif_lzw_codes(codes, minimum_code_size)
    blocks = b"".join(
        bytes((len(payload[offset : offset + 255]),))
        + payload[offset : offset + 255]
        for offset in range(0, len(payload), 255)
    ) + b"\0"
    output = bytearray(b"GIF89a")
    output.extend(struct.pack("<HH", width, height))
    output.extend((0x81, 0, 0))
    output.extend(palette)
    output.extend(b"\x2c\0\0\0\0")
    output.extend(struct.pack("<HH", width, height))
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
    one_pixel_without_trailer = (d / "1x1.gif").read_bytes()
    if not one_pixel_without_trailer.endswith(b"\x3b"):
        raise RuntimeError("1x1 GIF must end with its trailer before mutation")
    one_pixel_without_trailer = one_pixel_without_trailer[:-1]
    if hashlib.sha256(one_pixel_without_trailer).hexdigest() != (
        "9cbbe3f6a99791f1177c334b39f522b06bcc8e9faa481d95d13bfbaed2364929"
    ):
        raise RuntimeError("1x1 GIF trailer removal differs from its pinned hash")
    (d / "valid_frame_missing_trailer.gif").write_bytes(one_pixel_without_trailer)
    one_pixel = bytearray((d / "1x1.gif").read_bytes())
    if len(one_pixel) < 23 or one_pixel[:6] not in (b"GIF87a", b"GIF89a"):
        raise RuntimeError("1x1 GIF must have a complete supported header")
    screen_packed = one_pixel[10]
    if screen_packed & 0x80 == 0:
        raise RuntimeError("1x1 GIF must start with a global color table")
    palette_length = 3 << ((screen_packed & 7) + 1)
    palette_start = 13
    image_offset = palette_start + palette_length
    descriptor_end = image_offset + 10
    if (
        len(one_pixel) < descriptor_end
        or one_pixel[image_offset] != 0x2C
        or one_pixel[image_offset + 9] & 0x80 != 0
    ):
        raise RuntimeError("1x1 GIF image descriptor must have no local color table")
    global_palette = bytes(one_pixel[palette_start:image_offset])
    local_palette_gif = bytearray(
        one_pixel[:palette_start]
        + one_pixel[image_offset:descriptor_end]
        + global_palette
        + one_pixel[descriptor_end:]
    )
    local_palette_gif[10] &= 0x7F
    local_palette_flag = image_offset - palette_length + 9
    local_palette_gif[local_palette_flag] |= 0x80
    local_palette_path = d / "1x1_local_palette.gif"
    if hashlib.sha256(local_palette_gif).hexdigest() != (
        "2d029f561c59b4bf8c23b46e1acf61a144e16d7ec46901e0c428914801363146"
    ):
        raise RuntimeError("1x1 GIF local-palette mutation differs from its pinned hash")
    local_palette_path.write_bytes(local_palette_gif)
    with Image.open(d / "1x1.gif") as reference:
        reference.load()
        reference_pixels = reference.tobytes()
        reference_palette = reference.getpalette()
    with Image.open(local_palette_path) as oracle:
        oracle.verify()
    with Image.open(local_palette_path) as oracle:
        oracle.load()
        if (
            oracle.format != "GIF"
            or oracle.mode != "P"
            or oracle.size != (1, 1)
            or oracle.info.get("background") is not None
            or oracle.tobytes() != reference_pixels
            or oracle.getpalette() != reference_palette
        ):
            raise RuntimeError("1x1 GIF local-palette mutation changed Pillow pixels or palette")
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
    second_frame_no_palette = bytearray(animated_no_palette)
    if len(second_frame_no_palette) != 873 or second_frame_no_palette[85] != 0x87:
        raise RuntimeError("animated no-palette GIF local-table layout differs")
    second_frame_no_palette[85] &= 0x7F
    del second_frame_no_palette[86:854]
    second_frame_no_palette_path = d / "animated_no_palette_second_frame_no_local_table.gif"
    second_frame_no_palette_path.write_bytes(second_frame_no_palette)
    with Image.open(second_frame_no_palette_path) as oracle:
        if oracle.mode != "L" or oracle.size != (8, 8) or oracle.n_frames != 2:
            raise RuntimeError("palette-less animated GIF fixture changed its Pillow shape")
        frame_hashes = []
        for frame_index in range(oracle.n_frames):
            oracle.seek(frame_index)
            oracle.load()
            frame_hashes.append(hashlib.sha256(oracle.tobytes()).hexdigest())
        if frame_hashes != [
            "c2a74daea21f6caad6b7794dcfd121cbaa0bddc3407ce5f5fdc1914bd5e7ff90",
            "34cc5dbdf658a0cc3160111514671de40e72784439bf2f6a83ab2102004e1fcb",
        ]:
            raise RuntimeError("palette-less animated GIF fixture changed its Pillow frames")

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
    long_phrase_path = d / "lzw_long_expansion_token_checkpoint.gif"
    long_phrase_codes = [4, 0, *range(6, 1_031), *([0] * 509), 5]
    write_gif_lzw_fixture(
        long_phrase_path,
        1_024,
        long_phrase_codes,
        height=515,
    )
    with Image.open(long_phrase_path) as oracle:
        oracle.load()
        if oracle.format != "GIF" or oracle.size != (1_024, 515):
            raise RuntimeError("long-phrase GIF checkpoint fixture changed its Pillow shape")
        if len(oracle.tobytes()) != 1_024 * 515:
            raise RuntimeError("long-phrase GIF checkpoint fixture has incomplete Pillow pixels")
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
    checkpoint_row = Image.new("RGB", (1025, 1))
    checkpoint_row.putdata(
        [(x & 0xFF, (x * 73) & 0xFF, (x * 151) & 0xFF) for x in range(1025)]
    )
    checkpoint_row.save(d / "row_checkpoint_1025x1.bmp")
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
    write_bmp(
        d / "core_header_invalid_depth.bmp",
        struct.pack("<IHHHH", 12, 1, 1, 1, 2),
        b"\0" * 4,
        palette=b"\0" * 12,
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


WEBP_DELTA_SEGMENT_SHA256 = "9f6a1b274d9faf180fd024b962b068d6c4db5fe29e619868e7e4f9e91f12adbb"
WEBP_DELTA_SEGMENT_PIXELS_SHA256 = (
    "6b2c202c55d0bf70d9f32b74c1b8705232ae41e0ed24203ea2b1b138dfd914fe"
)


def gen_webp_delta_segment(directory, image, cwebp):
    """Generate or verify the pinned VP8 multi-segment delta-mode fixture."""
    path = directory / "lossy_delta_segment.webp"
    delta_cwebp = os.environ.get("WEBP_DELTA_CWEBP")

    with tempfile.TemporaryDirectory(prefix="image-star-webp-delta-") as temporary:
        source = Path(temporary) / "source.ppm"
        encoded_delta = Path(temporary) / "delta.webp"
        encoded_absolute = Path(temporary) / "absolute.webp"
        image.save(source)

        if delta_cwebp:
            version_output = subprocess.run(
                [delta_cwebp, "-version"], check=True, capture_output=True, text=True
            ).stdout.strip()
            version = version_output.splitlines()[0]
            if version != "1.6.0":
                raise RuntimeError(
                    f"WEBP_DELTA_CWEBP must be libwebp 1.6.0, found {version}"
                )
            subprocess.run(
                [
                    delta_cwebp,
                    "-quiet",
                    "-q",
                    "75",
                    "-m",
                    "4",
                    "-segments",
                    "4",
                    "-sns",
                    "100",
                    str(source),
                    "-o",
                    str(encoded_delta),
                ],
                check=True,
            )
            encoded = encoded_delta.read_bytes()
        else:
            if not path.is_file():
                raise RuntimeError(
                    "set WEBP_DELTA_CWEBP to regenerate lossy_delta_segment.webp; "
                    "see scripts/build_webp_delta_cwebp.py"
                )
            encoded = path.read_bytes()

        digest = hashlib.sha256(encoded).hexdigest()
        if digest != WEBP_DELTA_SEGMENT_SHA256:
            raise RuntimeError(
                "VP8 delta-segment fixture differs from its pinned hash: " + digest
            )

        with Image.open(BytesIO(encoded)) as decoded:
            decoded.load()
            if decoded.mode != "RGB" or decoded.size != (128, 128):
                raise RuntimeError("VP8 delta-segment fixture must decode as 128x128 RGB")
            delta_pixels = decoded.tobytes()
        if hashlib.sha256(delta_pixels).hexdigest() != WEBP_DELTA_SEGMENT_PIXELS_SHA256:
            raise RuntimeError("VP8 delta-segment fixture decoded pixels differ from the pin")

        if cwebp:
            subprocess.run(
                [
                    cwebp,
                    "-quiet",
                    "-q",
                    "75",
                    "-m",
                    "4",
                    "-segments",
                    "4",
                    "-sns",
                    "100",
                    str(source),
                    "-o",
                    str(encoded_absolute),
                ],
                check=True,
            )
            with Image.open(encoded_absolute) as decoded:
                decoded.load()
                if decoded.convert("RGB").tobytes() != delta_pixels:
                    raise RuntimeError(
                        "VP8 delta-mode fixture pixels differ from stock absolute mode"
                    )

        if delta_cwebp:
            path.write_bytes(encoded)


WEBP_SEGMENT_FEATURE_DATA_DISABLED_SOURCE_RGB_SHA256 = (
    "fe8b0b762b87b6031600f7c8da0c62b05a45581f7af6283e133abfe1e457aa2d"
)
WEBP_SEGMENT_FEATURE_DATA_DISABLED_SOURCE_PNG_SHA256 = (
    "9b3af9d1bd0994069ef61451c9ba360767ed78967cc0696165f88000f52f4854"
)
WEBP_SEGMENT_FEATURE_DATA_DISABLED_ASSET_SHA256 = (
    "b1fce88cb549bb4429cada0271752681530e8a86bc1c76256c57e581cf7c93ad"
)
WEBP_SEGMENT_FEATURE_DATA_DISABLED_PIXELS_SHA256 = (
    "3bc0db7e5ff0128a9d5c2f6193da39fc14ca5203340275b863119109c12aa7a2"
)


def webp_vp8_segmentation_flags(data):
    """Read keyframe segmentation flags with an independent VP8 bool reader."""
    if len(data) < 12 or data[:4] != b"RIFF" or data[8:12] != b"WEBP":
        raise RuntimeError("segmentation fixture has an invalid WebP RIFF header")
    if struct.unpack_from("<I", data, 4)[0] + 8 != len(data):
        raise RuntimeError("segmentation fixture RIFF length is inconsistent")

    offset = 12
    frame = None
    while offset + 8 <= len(data):
        chunk_kind = data[offset : offset + 4]
        chunk_size = struct.unpack_from("<I", data, offset + 4)[0]
        chunk_end = offset + 8 + chunk_size
        if chunk_end > len(data):
            raise RuntimeError("segmentation fixture contains a truncated RIFF chunk")
        if chunk_kind == b"VP8 ":
            frame = data[offset + 8 : chunk_end]
            break
        offset = chunk_end + (chunk_size & 1)
    if frame is None or len(frame) < 10:
        raise RuntimeError("segmentation fixture has no complete VP8 keyframe")

    frame_tag = int.from_bytes(frame[:3], "little")
    partition_size = frame_tag >> 5
    if frame_tag & 1 or frame[3:6] != b"\x9d\x01\x2a":
        raise RuntimeError("segmentation fixture is not a VP8 keyframe")
    if 10 + partition_size > len(frame):
        raise RuntimeError("segmentation fixture has a truncated first partition")

    class BoolReader:
        """Minimal independent VP8 Boolean reader for the first header flags."""

        def __init__(self, partition):
            self.partition = partition
            self.offset = 0
            self.value = 0
            self.range = 255
            self.bit_count = -8

        def read_flag(self, probability=128):
            if self.bit_count < 0:
                word = self.partition[self.offset : self.offset + 4].ljust(4, b"\0")
                self.offset += 4
                self.value = ((self.value << 32) | int.from_bytes(word, "big")) & (
                    (1 << 64) - 1
                )
                self.bit_count += 32

            split = 1 + (((self.range - 1) * probability) >> 8)
            boundary = split << self.bit_count
            if self.value >= boundary:
                self.range -= split
                self.value -= boundary
                bit = True
            else:
                self.range = split
                bit = False
            shift = max(0, 8 - self.range.bit_length())
            self.range <<= shift
            self.bit_count -= shift
            return bit

    partition = frame[10 : 10 + partition_size]
    reader = BoolReader(partition)
    color_space = reader.read_flag()
    pixel_type = reader.read_flag()
    segmentation_enabled = reader.read_flag()
    if not segmentation_enabled:
        return {
            "width": int.from_bytes(frame[6:8], "little") & 0x3FFF,
            "height": int.from_bytes(frame[8:10], "little") & 0x3FFF,
            "color_space": color_space,
            "pixel_type": pixel_type,
            "segmentation_enabled": False,
            "update_map": None,
            "update_feature_data": None,
        }
    return {
        "width": int.from_bytes(frame[6:8], "little") & 0x3FFF,
        "height": int.from_bytes(frame[8:10], "little") & 0x3FFF,
        "color_space": color_space,
        "pixel_type": pixel_type,
        "segmentation_enabled": True,
        "update_map": reader.read_flag(),
        "update_feature_data": reader.read_flag(),
    }


def gen_webp_segment_feature_data_disabled(directory):
    """Generate or verify the pinned 16x16 VP8 no-feature-update fixture."""
    path = directory / "segmentation_feature_data_disabled_16x16.webp"
    image = pattern_img("RGB", (16, 16))
    source_rgb = image.tobytes()
    if hashlib.sha256(source_rgb).hexdigest() != (
        WEBP_SEGMENT_FEATURE_DATA_DISABLED_SOURCE_RGB_SHA256
    ):
        raise RuntimeError("VP8 segmentation source pixels differ from the pin")

    writer = os.environ.get("WEBP_VP8_SEGMENT_FEATURE_DATA_DISABLED_CWEBP")
    with tempfile.TemporaryDirectory(prefix="image-star-webp-segment-disabled-") as temporary:
        source = Path(temporary) / "source.png"
        encoded_path = Path(temporary) / path.name
        image.save(source, format="PNG")
        if hashlib.sha256(source.read_bytes()).hexdigest() != (
            WEBP_SEGMENT_FEATURE_DATA_DISABLED_SOURCE_PNG_SHA256
        ):
            raise RuntimeError("VP8 segmentation source PNG differs from the Pillow pin")

        if writer:
            version_output = subprocess.run(
                [writer, "-version"], check=True, capture_output=True, text=True
            ).stdout.strip()
            version = version_output.splitlines()[0]
            if version != "1.6.0":
                raise RuntimeError(
                    "WEBP_VP8_SEGMENT_FEATURE_DATA_DISABLED_CWEBP must be libwebp 1.6.0, "
                    f"found {version}"
                )
            subprocess.run(
                [
                    writer,
                    "-quiet",
                    "-q",
                    "75",
                    "-m",
                    "4",
                    "-segments",
                    "4",
                    "-sns",
                    "100",
                    str(source),
                    "-o",
                    str(encoded_path),
                ],
                check=True,
            )
            encoded = encoded_path.read_bytes()
            path.write_bytes(encoded)
        else:
            if not path.is_file():
                raise RuntimeError(
                    "set WEBP_VP8_SEGMENT_FEATURE_DATA_DISABLED_CWEBP using "
                    "scripts/build_webp_delta_cwebp.py --variant "
                    "feature-data-disabled-16x16 to generate "
                    "segmentation_feature_data_disabled_16x16.webp"
                )
            encoded = path.read_bytes()

    if hashlib.sha256(encoded).hexdigest() != (
        WEBP_SEGMENT_FEATURE_DATA_DISABLED_ASSET_SHA256
    ):
        raise RuntimeError("VP8 segmentation WebP asset differs from the pinned hash")
    flags = webp_vp8_segmentation_flags(encoded)
    expected_flags = {
        "width": 16,
        "height": 16,
        "color_space": False,
        "pixel_type": False,
        "segmentation_enabled": True,
        "update_map": False,
        "update_feature_data": False,
    }
    if flags != expected_flags:
        raise RuntimeError(f"VP8 segmentation flags differ from the pin: {flags}")

    with Image.open(BytesIO(encoded)) as decoded:
        decoded.load()
        if decoded.mode != "RGB" or decoded.size != (16, 16):
            raise RuntimeError("VP8 segmentation fixture must decode as 16x16 RGB")
        pixels = decoded.tobytes()
    if hashlib.sha256(pixels).hexdigest() != (
        WEBP_SEGMENT_FEATURE_DATA_DISABLED_PIXELS_SHA256
    ):
        raise RuntimeError("VP8 segmentation fixture Pillow pixels differ from the pin")


def gen_webp_vp8l_canvas_mismatch(directory):
    """Create extended lossless files with conflicting canvas dimensions."""
    source = bytearray((directory / "exif.webp").read_bytes())
    if source[:4] != b"RIFF" or source[8:12] != b"WEBP":
        raise RuntimeError("EXIF WebP source has an invalid RIFF signature")
    if source[12:16] != b"VP8X" or struct.unpack_from("<I", source, 16)[0] != 10:
        raise RuntimeError("EXIF WebP source does not begin with a 10-byte VP8X chunk")

    image_chunk_offset = 12 + 8 + 10
    if source[image_chunk_offset : image_chunk_offset + 4] != b"VP8L":
        raise RuntimeError("EXIF WebP source does not contain a VP8L image chunk")

    for axis, field_offset in (("width", 12 + 8 + 4), ("height", 12 + 8 + 7)):
        mismatch = bytearray(source)
        dimension_minus_one = int.from_bytes(
            mismatch[field_offset : field_offset + 3], "little"
        )
        if dimension_minus_one == 0xFF_FFFF:
            raise RuntimeError(f"EXIF WebP source canvas {axis} cannot be increased")
        mismatch[field_offset : field_offset + 3] = (dimension_minus_one + 1).to_bytes(
            3, "little"
        )
        (directory / f"extended_vp8l_{axis}_mismatch.webp").write_bytes(mismatch)


def gen_webp():
    d = OUT / "webp"; d.mkdir(parents=True, exist_ok=True)
    truncated_riff_extent = b"RIFF" + struct.pack("<I", 24) + b"WEBPVP8L"
    (d / "riff_extent_truncated.webp").write_bytes(truncated_riff_extent)
    img = pattern_img("RGB")
    img.save(d / "lossy.webp", lossless=False)
    lossy_vp8 = (d / "lossy.webp").read_bytes()
    if hashlib.sha256(lossy_vp8).hexdigest() != (
        "352c7d601335ace77ca55471983b7b25279d0134bc58b20011dff52b452f9c4f"
    ):
        raise RuntimeError("lossy WebP source differs from the pinned VP8 fixture")
    if len(lossy_vp8) <= 30 or lossy_vp8[30] != 0x3E:
        raise RuntimeError("lossy WebP has an unexpected VP8 color-space bit byte")

    # Byte 30 is the first arithmetic-coded VP8 frame-header byte. Its high
    # bit selects the invalid non-zero color-space value for this keyframe.
    invalid_color_space = bytearray(lossy_vp8)
    invalid_color_space[30] |= 0x80
    invalid_color_space_sha256 = hashlib.sha256(invalid_color_space).hexdigest()
    if invalid_color_space_sha256 != (
        "ce6b3a9b8e69194d02cdc0739b9db9cdc668554abc0fe29e28fa902e49eb6e45"
    ):
        raise RuntimeError("VP8 invalid-color-space fixture differs from its pinned hash")
    (d / "vp8_color_space_invalid.webp").write_bytes(invalid_color_space)
    gen_webp_q0_uvdc_clamp(d)
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
    y2_empty_ac = Image.new("RGB", (16, 16))
    for y in range(y2_empty_ac.height):
        for x in range(y2_empty_ac.width):
            y2_empty_ac.putpixel(
                (x, y),
                (255, 93, 0)
                if ((x // 4) + (y // 4)) % 2 == 0
                else (0, 222, 0),
            )
    y2_empty_ac_output = BytesIO()
    y2_empty_ac.save(
        y2_empty_ac_output,
        format="WEBP",
        lossless=False,
        quality=90,
        method=4,
    )
    y2_empty_ac_bytes = y2_empty_ac_output.getvalue()
    if hashlib.sha256(y2_empty_ac_bytes).hexdigest() != (
        "e5e6cc56cf73aded4521a16de4ef0d08ff4066db75cdcd5527b15e6210d4ea98"
    ):
        raise RuntimeError("VP8 Y2 empty-AC fixture differs from its pinned hash")
    (d / "lossy_y2_empty_ac_16x16_q90_m4.webp").write_bytes(
        y2_empty_ac_bytes
    )
    macroblock_edges = Image.new("RGB", (32, 16))
    macroblock_pixels = macroblock_edges.load()
    for y in range(macroblock_edges.height):
        for x in range(macroblock_edges.width):
            if x < 16:
                pixel = (48, 96, 144)
            else:
                value = (x * 37 + y * 73 + x * y * 11) & 0xFF
                pixel = (value, (value * 5 + 29) & 0xFF, (value * 3 + 171) & 0xFF)
            macroblock_pixels[x, y] = pixel
    macroblock_edges.save(
        d / "lossy_macroblock_residual_context_edges_32x16_q75_m4.webp",
        lossless=False,
        quality=75,
        method=4,
    )
    intra_chroma_context = Image.new("RGB", (32, 32))
    quadrant_colors = (
        ((255, 0, 0), (0, 255, 0)),
        ((0, 0, 255), (0, 128, 128)),
    )
    context_pixels = intra_chroma_context.load()
    for y in range(intra_chroma_context.height):
        for x in range(intra_chroma_context.width):
            context_pixels[x, y] = quadrant_colors[y // 16][x // 16]
    intra_chroma_context.save(
        d / "lossy_intra_chroma_dc_32x32_q100_m4.webp",
        lossless=False,
        quality=100,
        method=4,
    )
    intra_chroma_horizontal = Image.new("RGB", (32, 32))
    horizontal_pixels = intra_chroma_horizontal.load()
    for y in range(intra_chroma_horizontal.height):
        for x in range(intra_chroma_horizontal.width):
            horizontal_pixels[x, y] = (128 + 3 * x, 128 - x, 128)
    intra_chroma_horizontal.save(
        d / "lossy_intra_chroma_horizontal_32x32_q100_m4.webp",
        lossless=False,
        quality=100,
        method=4,
    )
    intra_chroma_vertical = Image.new("RGB", (32, 32))
    vertical_pixels = intra_chroma_vertical.load()
    for y in range(intra_chroma_vertical.height):
        for x in range(intra_chroma_vertical.width):
            vertical_pixels[x, y] = (128, 128 + y, 128 - 3 * y)
    intra_chroma_vertical.save(
        d / "lossy_intra_chroma_vertical_32x32_q100_m4.webp",
        lossless=False,
        quality=100,
        method=4,
    )
    intra_chroma_modes = Image.new("RGB", (32, 32))
    mode_pixels = intra_chroma_modes.load()
    for y in range(intra_chroma_modes.height):
        for x in range(intra_chroma_modes.width):
            if x < 16 and y < 16:
                pixel = (128, 128, 128)
            elif x >= 16 and y < 16:
                horizontal = x - 16
                pixel = (128 + 2 * horizontal, 128 - horizontal, 128)
            elif x < 16:
                vertical = y - 16
                pixel = (128, 128 + vertical, 128 - 3 * vertical)
            else:
                horizontal = x - 16
                vertical = y - 16
                pixel = (
                    128 + 2 * horizontal,
                    128 - horizontal + vertical,
                    128 - 3 * vertical,
                )
            mode_pixels[x, y] = pixel
    intra_chroma_modes.save(
        d / "lossy_intra_chroma_modes_32x32_q100_m4.webp",
        lossless=False,
        quality=100,
        method=4,
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
    gen_webp_delta_segment(d, img, cwebp)
    gen_webp_segment_feature_data_disabled(d)
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
    gen_webp_lossless_single_symbol_copy_fallback(d)
    gen_webp_lossless_copy_out_of_bounds(d)
    gen_webp_lossless_branch_coverage(d)
    rgba_state = 0xC0FFEE01
    rgba_noise_bytes = bytearray()
    for _ in range(32 * 32 * 4):
        rgba_state = (rgba_state * 1664525 + 1013904223) & 0xFFFFFFFF
        rgba_noise_bytes.append(rgba_state >> 24)
    rgba_noise = Image.frombytes("RGBA", (32, 32), bytes(rgba_noise_bytes))
    rgba_noise_path = d / "lossless_rgba_huffman_fallback_32x32.webp"
    rgba_noise.save(rgba_noise_path, lossless=True, method=6, exact=True)
    encoded_rgba_noise = rgba_noise_path.read_bytes()
    vp8l_alpha = None
    offset = 12
    while offset + 8 <= len(encoded_rgba_noise):
        chunk_size = struct.unpack_from("<I", encoded_rgba_noise, offset + 4)[0]
        chunk_start = offset + 8
        chunk_end = chunk_start + chunk_size
        if chunk_end > len(encoded_rgba_noise):
            raise RuntimeError("generated lossless RGBA WebP has a truncated chunk")
        if encoded_rgba_noise[offset : offset + 4] == b"VP8L":
            payload = encoded_rgba_noise[chunk_start:chunk_end]
            if len(payload) < 5 or payload[0] != 0x2F:
                raise RuntimeError(
                    "generated lossless RGBA WebP has an invalid VP8L header"
                )
            vp8l_alpha = bool((int.from_bytes(payload[1:5], "little") >> 28) & 1)
            break
        offset = chunk_end + (chunk_size & 1)
    if not vp8l_alpha:
        raise RuntimeError(
            "generated lossless RGBA WebP has no alpha-bearing VP8L chunk"
        )
    with Image.open(rgba_noise_path) as decoded_rgba_noise:
        decoded_rgba_noise.load()
        if (
            decoded_rgba_noise.mode != "RGBA"
            or decoded_rgba_noise.tobytes() != bytes(rgba_noise_bytes)
        ):
            raise RuntimeError("generated lossless RGBA WebP did not round-trip exactly")
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

    box_chain_palette = [(0, 0, 0), (255, 0, 0), (0, 255, 0), (0, 0, 255)]
    box_chain_pattern = Image.new("RGB", (4, 4100))
    box_chain_pattern.putdata(
        [box_chain_palette[(x + y) % 4] for y in range(4100) for x in range(4)]
    )
    box_chain_pattern.save(d / "lossless_palette4_box_chain.webp", lossless=True, method=6)

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

    short_copy_pixels = near_black_pixels.copy()
    short_copy_phrase = short_copy_pixels[:4]
    for start in (20, 48, 112):
        short_copy_pixels[start : start + len(short_copy_phrase)] = short_copy_phrase
    short_copy = Image.new("RGB", (96, 96))
    short_copy.putdata(short_copy_pixels)
    short_copy.save(
        d / "lossless_short_backreference.webp",
        format="WEBP",
        lossless=True,
        quality=100,
        method=6,
        exact=True,
    )

    cache_rng = random.Random(0x51A7)
    cache_palette = []
    cache_palette_set = set()
    while len(cache_palette) < 512:
        color = tuple(cache_rng.randrange(256) for _ in range(3))
        if color not in cache_palette_set:
            cache_palette.append(color)
            cache_palette_set.add(color)
    cache_pixels = [
        cache_palette[cache_rng.randrange(len(cache_palette))]
        for _ in range(128 * 64)
    ]
    # End with a cached color, a new literal, and another cached color. This
    # exercises VP8L color-cache lookahead at the end of the RGB output block.
    cache_pixels[-3] = cache_palette[0]
    cache_literal = (255, 0, 254)
    if cache_literal in cache_palette_set:
        cache_literal = (255, 1, 254)
    cache_pixels[-2] = cache_literal
    cache_pixels[-1] = cache_palette[1]
    cache_image = Image.new("RGB", (128, 64))
    cache_image.putdata(cache_pixels)
    cache_output = BytesIO()
    cache_image.save(
        cache_output,
        format="WEBP",
        lossless=True,
        quality=100,
        method=6,
    )
    cache_bytes = cache_output.getvalue()
    if hashlib.sha256(cache_bytes).hexdigest() != (
        "920481d0986bf9034730ebdd4678c65540c985dc00c25c0cc293d45167b3e541"
    ):
        raise RuntimeError("VP8L color-cache lookahead fixture differs from its pinned hash")
    (d / "lossless_palette_cache_512_final.webp").write_bytes(cache_bytes)

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
    metadata_header_source = (d / "exif.webp").read_bytes()
    if (
        metadata_header_source[:4] != b"RIFF"
        or metadata_header_source[8:12] != b"WEBP"
    ):
        raise RuntimeError("EXIF WebP source has an invalid RIFF signature")
    declared_size = struct.unpack_from("<I", metadata_header_source, 4)[0]
    if declared_size != len(metadata_header_source) - 8:
        raise RuntimeError("EXIF WebP source has an inconsistent RIFF extent")
    retained_chunks = []
    removed_image_chunks = 0
    chunk_offset = 12
    while chunk_offset < len(metadata_header_source):
        if len(metadata_header_source) - chunk_offset < 8:
            raise RuntimeError("EXIF WebP source has an incomplete chunk header")
        chunk_kind = metadata_header_source[chunk_offset : chunk_offset + 4]
        chunk_size = struct.unpack_from(
            "<I", metadata_header_source, chunk_offset + 4
        )[0]
        chunk_end = chunk_offset + 8 + chunk_size + (chunk_size & 1)
        if chunk_end > len(metadata_header_source):
            raise RuntimeError("EXIF WebP source has a truncated chunk")
        if chunk_kind == b"VP8L":
            removed_image_chunks += 1
        else:
            retained_chunks.append(metadata_header_source[chunk_offset:chunk_end])
        chunk_offset = chunk_end
    if removed_image_chunks != 1:
        raise RuntimeError("EXIF WebP source must contain one VP8L image chunk")
    metadata_header_tail = bytearray(
        metadata_header_source[:12] + b"".join(retained_chunks)
    )
    metadata_header_tail.append(0)
    struct.pack_into("<I", metadata_header_tail, 4, len(metadata_header_tail) - 8)
    metadata_header_tail_sha256 = hashlib.sha256(metadata_header_tail).hexdigest()
    if metadata_header_tail_sha256 != (
        "a8c4e47813409583e1de7b09b55bf75da17953d80f02a3de02fefc676bdbb31c"
    ):
        raise RuntimeError("WebP incomplete-chunk-header fixture differs from its pin")
    try:
        Image.open(BytesIO(metadata_header_tail)).load()
    except OSError:
        pass
    else:
        raise RuntimeError("Pillow accepted the WebP with an incomplete chunk header")
    (d / "metadata_truncated_chunk_header.webp").write_bytes(metadata_header_tail)
    metadata_chunk_overflow = bytearray(metadata_header_tail)
    exif_chunk = metadata_chunk_overflow.find(b"EXIF")
    if exif_chunk < 0:
        raise RuntimeError("extended WebP source has no EXIF chunk")
    exif_size = struct.unpack_from("<I", metadata_chunk_overflow, exif_chunk + 4)[0]
    struct.pack_into("<I", metadata_chunk_overflow, exif_chunk + 4, exif_size + 3)
    metadata_chunk_overflow_sha256 = hashlib.sha256(metadata_chunk_overflow).hexdigest()
    if metadata_chunk_overflow_sha256 != (
        "e8c6da05d6b590b0865b17f0b0bfcefd6206951c8dec47d0b1b500fd7cb6bec7"
    ):
        raise RuntimeError("WebP RIFF-chunk-overflow fixture differs from its pin")
    try:
        Image.open(BytesIO(metadata_chunk_overflow)).load()
    except OSError:
        pass
    else:
        raise RuntimeError("Pillow accepted the WebP chunk beyond its RIFF extent")
    (d / "metadata_chunk_exceeds_riff.webp").write_bytes(metadata_chunk_overflow)
    gen_webp_vp8l_canvas_mismatch(d)
    img.save(d / "animated.webp", save_all=True, append_images=[pattern_img("RGB").transpose(Image.Transpose.FLIP_LEFT_RIGHT)], duration=100, loop=0)
    full_palette_first = Image.new("RGB", (16, 16))
    full_palette_first.putdata([(value, value, value) for value in range(256)])
    full_palette_second = full_palette_first.copy()
    full_palette_second.putpixel((0, 0), (255, 255, 255))
    full_palette_second.putpixel((15, 15), (0, 0, 0))

    def lossless_frame_chunk(frame):
        encoded = BytesIO()
        frame.save(encoded, format="WEBP", lossless=True, method=6)
        data = encoded.getvalue()
        offset = 12
        while offset + 8 <= len(data):
            size = struct.unpack_from("<I", data, offset + 4)[0]
            end = offset + 8 + size + (size & 1)
            if end > len(data):
                raise RuntimeError("Pillow wrote a truncated static WebP frame")
            if data[offset : offset + 4] == b"VP8L":
                return data[offset:end]
            offset = end
        raise RuntimeError("Pillow did not write a lossless RGB WebP frame")

    def webp_chunk(fourcc, payload):
        chunk = fourcc + struct.pack("<I", len(payload)) + payload
        return chunk + (b"\0" if len(payload) & 1 else b"")

    def animated_frame_chunk(frame):
        frame_header = (
            bytes(6)
            + (15).to_bytes(3, "little")
            + (15).to_bytes(3, "little")
            + (100).to_bytes(3, "little")
            + bytes((1,))
        )
        return webp_chunk(b"ANMF", frame_header + lossless_frame_chunk(frame))

    canvas_dimensions = (15).to_bytes(3, "little") * 2
    vp8x_payload = bytes((0x02, 0, 0, 0)) + canvas_dimensions
    anim_payload = bytes((0, 0, 0, 255)) + struct.pack("<H", 0)
    webp_payload = (
        b"WEBP"
        + webp_chunk(b"VP8X", vp8x_payload)
        + webp_chunk(b"ANIM", anim_payload)
        + animated_frame_chunk(full_palette_first)
        + animated_frame_chunk(full_palette_second)
    )
    full_palette_animation = b"RIFF" + struct.pack("<I", len(webp_payload)) + webp_payload
    (d / "animated_rgb_full_palette.webp").write_bytes(full_palette_animation)
    decoded_full_palette = Image.open(BytesIO(full_palette_animation))
    expected_frames = (full_palette_first, full_palette_second)
    if decoded_full_palette.mode != "RGB" or decoded_full_palette.n_frames != 2:
        raise RuntimeError("full-palette RGB animation lost its two-frame RGB shape")
    for index, expected_frame in enumerate(expected_frames):
        decoded_full_palette.seek(index)
        pixels_match = decoded_full_palette.convert("RGB").tobytes() == expected_frame.tobytes()
        duration = decoded_full_palette.info.get("duration")
        if duration != 100 or not pixels_match:
            raise RuntimeError(
                f"full-palette RGB animation frame {index} changed "
                f"(duration={duration}, pixels_match={pixels_match})"
            )
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
        if keep == "all_but_one":
            kept = len(payload) - 1
        elif keep == "all_but_two":
            kept = len(payload) - 2
        else:
            kept = min(keep, len(payload))
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
        (
            "lossless_short_backreference_truncated_tail.webp",
            "all_but_two",
            "lossless_short_backreference.webp",
        ),
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

    unrecognized_animation = BytesIO()
    unrecognized_frames = [
        Image.new("RGB", (16, 16), color)
        for color in ((20, 70, 120), (210, 30, 90))
    ]
    unrecognized_frames[0].save(
        unrecognized_animation,
        format="WEBP",
        save_all=True,
        append_images=unrecognized_frames[1:],
        duration=[100, 200],
        loop=0,
        lossless=True,
        method=6,
    )
    unrecognized_source = unrecognized_animation.getvalue()
    if hashlib.sha256(unrecognized_source).hexdigest() != (
        "56e46a255183e013ba43c12a115d58220a6835575ee1ef7a3b8e63c765715c10"
    ):
        raise RuntimeError("animated WebP source differs from its pinned fixture")

    unrecognized_bytes = bytearray(unrecognized_source)
    frame_count = 0
    cursor = 12
    while cursor + 8 <= len(unrecognized_bytes):
        fourcc = unrecognized_bytes[cursor : cursor + 4]
        chunk_size = struct.unpack_from("<I", unrecognized_bytes, cursor + 4)[0]
        chunk_end = cursor + 8 + chunk_size + (chunk_size & 1)
        if chunk_end > len(unrecognized_bytes):
            raise RuntimeError("generated animation has a truncated RIFF chunk")
        if fourcc == b"ANMF":
            nested = cursor + 24
            if (
                nested + 4 > cursor + 8 + chunk_size
                or unrecognized_bytes[nested : nested + 4] != b"VP8L"
            ):
                raise RuntimeError("generated lossless frame lacks its nested VP8L chunk")
            unrecognized_bytes[nested : nested + 4] = b"JUNK"
            frame_count += 1
        cursor = chunk_end
    if frame_count != 2:
        raise RuntimeError(f"expected two animated WebP frames, found {frame_count}")
    if hashlib.sha256(unrecognized_bytes).hexdigest() != (
        "cde9d77559e01ef3324fb73f6541c4cd6257b8563595a4ca397fc0ab52f69886"
    ):
        raise RuntimeError("animated WebP with unknown frame chunks differs from its pinned fixture")
    (d / "animated_all_frame_chunks_unrecognized.webp").write_bytes(unrecognized_bytes)

    def mutate_first_nested_vp8l(data, mutate_payload):
        anmf = data.find(b"ANMF")
        if anmf < 0:
            raise RuntimeError("animated WebP did not contain an ANMF chunk")
        vp8l = data.find(b"VP8L", anmf + 24)
        if vp8l < anmf + 24 or vp8l + 13 > len(data):
            raise RuntimeError("first animated WebP frame did not contain a complete VP8L header")
        mutate_payload(data, vp8l + 8)

    def invalidate_nested_vp8l_signature(data, payload):
        data[payload] = 0

    def invalidate_nested_vp8l_version(data, payload):
        header = int.from_bytes(data[payload + 1 : payload + 5], "little")
        data[payload + 1 : payload + 5] = (header | (1 << 29)).to_bytes(4, "little")

    write_mutated_webp(
        "animated_nested_vp8l_bad_signature.webp",
        "animated_sequence_rgba_keyframes.webp",
        lambda data: mutate_first_nested_vp8l(data, invalidate_nested_vp8l_signature),
    )
    write_mutated_webp(
        "animated_nested_vp8l_bad_version.webp",
        "animated_sequence_rgba_keyframes.webp",
        lambda data: mutate_first_nested_vp8l(data, invalidate_nested_vp8l_version),
    )

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

    def write_extended_vp8_mismatch(filename, canvas_width, canvas_height):
        source = (d / "lossy.webp").read_bytes()
        vp8 = source.find(b"VP8 ")
        if vp8 < 0:
            raise RuntimeError("lossy WebP did not contain a VP8 chunk")
        vp8_size = struct.unpack_from("<I", source, vp8 + 4)[0]
        vp8_chunk_end = vp8 + 8 + vp8_size + (vp8_size & 1)
        vp8_chunk = source[vp8:vp8_chunk_end]
        vp8_payload = vp8 + 8
        if source[vp8_payload + 3 : vp8_payload + 6] != b"\x9d\x01\x2a":
            raise RuntimeError("lossy WebP VP8 keyframe has an invalid start code")
        image_width = struct.unpack_from("<H", source, vp8_payload + 6)[0] & 0x3FFF
        image_height = struct.unpack_from("<H", source, vp8_payload + 8)[0] & 0x3FFF
        if (canvas_width, canvas_height) == (image_width, image_height):
            raise RuntimeError("extended VP8 fixture canvas must disagree with the image")

        vp8x_payload = bytearray((0, 0, 0, 0))
        vp8x_payload.extend((canvas_width - 1).to_bytes(3, "little"))
        vp8x_payload.extend((canvas_height - 1).to_bytes(3, "little"))
        vp8x_chunk = bytearray(b"VP8X")
        vp8x_chunk.extend(struct.pack("<I", len(vp8x_payload)))
        vp8x_chunk.extend(vp8x_payload)
        payload = b"WEBP" + bytes(vp8x_chunk) + vp8_chunk
        webp = bytearray(b"RIFF")
        webp.extend(struct.pack("<I", len(payload)))
        webp.extend(payload)
        (d / filename).write_bytes(webp)

    write_extended_vp8_mismatch("extended_vp8_dimension_mismatch.webp", 64, 64)
    lossy = (d / "lossy.webp").read_bytes()
    vp8 = lossy.find(b"VP8 ")
    vp8_payload = vp8 + 8
    image_width = struct.unpack_from("<H", lossy, vp8_payload + 6)[0] & 0x3FFF
    image_height = struct.unpack_from("<H", lossy, vp8_payload + 8)[0] & 0x3FFF
    write_extended_vp8_mismatch(
        "extended_vp8_height_mismatch.webp", image_width, image_height + 1
    )

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
        "animated_frame_bottom_outside.webp",
        "animated.webp",
        lambda data: mutate_anmf_field(data, 3, b"\x01\x00\x00"),
    )
    write_mutated_webp(
        "animated_frame_dimension_mismatch.webp",
        "animated.webp",
        lambda data: mutate_anmf_field(data, 6, b"\x3e\x00\x00"),
    )

    def enlarge_alpha_anmf_width(data):
        anmf = data.find(b"ANMF")
        alpha = data.find(b"ALPH", anmf + 8)
        if anmf < 0 or alpha < anmf + 24:
            raise RuntimeError("animated alpha WebP did not contain an ALPH frame")
        # ANMF dimensions are stored as dimension minus one. Keep the VP8X
        # canvas and nested ALPH/VP8 bitstreams unchanged so Pillow exercises
        # its tolerated declaration-versus-bitstream mismatch behavior.
        data[anmf + 8 + 6 : anmf + 8 + 9] = (16_384).to_bytes(3, "little")

    write_mutated_webp(
        "animated_alpha_anmf_width_mismatch.webp",
        "animated_alpha_lossy.webp",
        enlarge_alpha_anmf_width,
    )

    def enlarge_vp8l_anmf_width(data):
        anmf = data.find(b"ANMF")
        if anmf < 0 or data[anmf + 8 + 16 : anmf + 8 + 20] != b"VP8L":
            raise RuntimeError("animated lossless WebP did not contain a VP8L frame")
        # ANMF stores width minus one. Keep the nested VP8L header and the
        # 64x64 canvas unchanged to exercise Pillow's bitstream-dimension
        # behavior when the frame declaration exceeds the canvas.
        data[anmf + 8 + 6 : anmf + 8 + 9] = (16_384).to_bytes(3, "little")

    write_mutated_webp(
        "animated_vp8l_anmf_width_mismatch.webp",
        "animated_alpha.webp",
        enlarge_vp8l_anmf_width,
    )

    def truncate_vp8l_anmf_payload(data, oversized_width=False):
        anmf = data.find(b"ANMF")
        if anmf < 0:
            raise RuntimeError("animated WebP did not contain an ANMF frame")
        chunk = data.find(b"VP8L", anmf + 24)
        if chunk < anmf + 24:
            raise RuntimeError("animated lossless WebP did not contain a VP8L frame")
        struct.pack_into("<I", data, chunk + 4, 4)
        if oversized_width:
            data[anmf + 8 + 6 : anmf + 8 + 9] = (16_384).to_bytes(3, "little")

    write_mutated_webp(
        "animated_vp8l_short_header.webp",
        "animated_alpha.webp",
        truncate_vp8l_anmf_payload,
    )
    write_mutated_webp(
        "animated_vp8l_short_header_oversized_width.webp",
        "animated_alpha.webp",
        lambda data: truncate_vp8l_anmf_payload(data, oversized_width=True),
    )

    def move_full_vp8l_frame_outside_canvas(data, axis):
        anmf = data.find(b"ANMF")
        if anmf < 0:
            raise RuntimeError("animated WebP did not contain an ANMF frame")
        chunk = data.find(b"VP8L", anmf + 24)
        if chunk < anmf + 24:
            raise RuntimeError("animated lossless WebP did not contain a VP8L frame")
        payload_dimensions = int.from_bytes(data[chunk + 9 : chunk + 13], "little")
        dimensions = (
            (payload_dimensions & 0x3FFF) + 1,
            ((payload_dimensions >> 14) & 0x3FFF) + 1,
        )
        if dimensions != (64, 64):
            raise RuntimeError(f"expected a 64x64 VP8L frame, found {dimensions}")
        field_offset = {"right": 0, "bottom": 3}[axis]
        data[anmf + 8 + field_offset : anmf + 11 + field_offset] = (1).to_bytes(
            3, "little"
        )

    for axis in ("right", "bottom"):
        write_mutated_webp(
            f"animated_vp8l_frame_outside_{axis}.webp",
            "animated_alpha_full.webp",
            lambda data, axis=axis: move_full_vp8l_frame_outside_canvas(data, axis),
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

    def truncate_uncompressed_alpha_payload(data):
        alpha = data.find(b"ALPH")
        if alpha < 0:
            raise RuntimeError("WebP did not contain an ALPH chunk")

        alpha_size = struct.unpack_from("<I", data, alpha + 4)[0]
        payload_start = alpha + 8
        payload_end = payload_start + alpha_size
        if alpha_size < 2 or (data[payload_start] & 0x03) != 0:
            raise RuntimeError("WebP alpha chunk is not uncompressed raw alpha")

        alpha_end = payload_end + (alpha_size & 1)
        truncated_payload = data[payload_start : payload_start + 2]
        replacement = (
            b"ALPH" + struct.pack("<I", len(truncated_payload)) + truncated_payload
        )
        data[alpha:alpha_end] = replacement
        struct.pack_into("<I", data, 4, len(data) - 8)

    write_mutated_webp(
        "alpha_uncompressed_truncated_payload.webp",
        "alpha_uncompressed.webp",
        truncate_uncompressed_alpha_payload,
    )

    def empty_compressed_alpha_payload(data):
        alpha = data.find(b"ALPH")
        if alpha < 0:
            raise RuntimeError("WebP did not contain an ALPH chunk")

        alpha_size = struct.unpack_from("<I", data, alpha + 4)[0]
        payload_start = alpha + 8
        payload_end = payload_start + alpha_size
        if alpha_size < 2 or (data[payload_start] & 0x03) != 1:
            raise RuntimeError("WebP alpha chunk is not compressed alpha")

        alpha_end = payload_end + (alpha_size & 1)
        truncated_payload = data[payload_start : payload_start + 1]
        replacement = (
            b"ALPH" + struct.pack("<I", len(truncated_payload)) + truncated_payload
        )
        if len(truncated_payload) & 1:
            replacement += b"\0"
        data[alpha:alpha_end] = replacement
        struct.pack_into("<I", data, 4, len(data) - 8)

    write_mutated_webp(
        "alpha_lossy_gradient_empty_compressed_payload.webp",
        "alpha_lossy_gradient.webp",
        empty_compressed_alpha_payload,
    )

    write_mutated_webp(
        "alpha_preprocessing.webp",
        "alpha_lossy_horizontal.webp",
        lambda data: set_alpha_info(data, 0x30, 0x10),
    )
    print(f"  WebP: {len(list(d.glob('*.webp')))} files")


def gen_webp_q0_uvdc_clamp(directory):
    """Generate the pinned VP8 quality-zero quantizer-clamp decode input."""
    width = height = 32
    state = 0x31415926
    pixels = bytearray()
    for _ in range(width * height * 3):
        state = (state * 1664525 + 1013904223) & 0xFFFFFFFF
        pixels.append(state >> 24)

    image = Image.frombytes("RGB", (width, height), bytes(pixels))
    path = directory / "lossy_q0_uvdc_clamp_32x32.webp"
    image.save(path, format="WEBP", lossless=False, quality=0, method=6)
    encoded = path.read_bytes()

    # Pillow 12.2.0/libwebp 1.6.0's bitstream probe reports Base Q=127 and
    # UV DC delta=-2. DC_QUANT[125] is 151, so the decoder clamps it to 132.
    # Pin the encoded bytes to keep that bitstream evidence tied to this input.
    expected_sha256 = (
        "7d0c80e08d12b5ac215b48276cd330fcbc01d93f02b650f37339ae5eebc3da2a"
    )
    if hashlib.sha256(encoded).hexdigest() != expected_sha256:
        raise RuntimeError("VP8 q0 UV-DC clamp fixture differs from its pinned hash")
    if (
        encoded[:4] != b"RIFF"
        or encoded[8:12] != b"WEBP"
        or encoded[12:16] != b"VP8 "
    ):
        raise RuntimeError("VP8 q0 UV-DC clamp fixture is not a simple VP8 WebP")

    with Image.open(path) as decoded:
        decoded.load()
        if decoded.mode != "RGB" or decoded.size != (width, height):
            raise RuntimeError("VP8 q0 UV-DC clamp fixture did not decode as 32x32 RGB")


def _build_webp_lossless_single_symbol_copy_stream(green_symbol, extra_data_bits=0):
    """Build an 8x4 VP8L stream with a singleton green-channel symbol."""
    width, height = 8, 4
    bits = [0, 0, 1, 0, 0, 0]  # no transforms/cache; 4x4 meta-Huffman blocks

    def append_lsb(value, bit_count):
        bits.extend((value >> offset) & 1 for offset in range(bit_count))

    def write_simple_tree(symbols):
        bits.extend((1, len(symbols) - 1))
        append_lsb(int(symbols[0] >= 2), 1)
        append_lsb(symbols[0], 8 if symbols[0] >= 2 else 1)
        if len(symbols) == 2:
            append_lsb(symbols[1], 8)

    # The two metadata pixels map the left and right 4x4 blocks to groups 0/1.
    bits.append(0)  # metadata image has no color cache
    write_simple_tree((0, 1))  # green
    for _ in range(4):
        write_simple_tree((0,))  # red, blue, alpha, and distance
    bits.extend((0, 1))  # metadata green bytes select groups 0 and 1

    # Group 0 is an all-singleton literal block and fills the left half.
    for symbol in (20, 10, 30, 128, 1):  # green, red, blue, alpha, distance
        write_simple_tree((symbol,))

    # A simple singleton carries only eight symbol bits. Use a complete
    # two-symbol code-length alphabet to construct a non-simple singleton
    # without adding an unused second leaf.
    bits.append(0)  # dynamic Huffman tree
    append_lsb(0, 4)  # four code-length-code lengths: 17, 18, 0, and 1
    for code_length in (0, 0, 1, 1):
        append_lsb(code_length, 3)
    bits.append(0)  # code lengths continue through the green alphabet
    bits.extend(int(symbol == green_symbol) for symbol in range(256 + 24))

    # Group 1's copy uses this distance tree across the right half.
    for symbol in (0, 0, 0, 1):  # red, blue, alpha, distance
        write_simple_tree((symbol,))

    bits.extend([0] * extra_data_bits)
    packed_bits = bytearray((len(bits) + 7) // 8)
    for index, bit in enumerate(bits):
        packed_bits[index // 8] |= bit << (index % 8)
    dimensions = (width - 1) | ((height - 1) << 14) | (1 << 28)
    payload = b"\x2f" + struct.pack("<I", dimensions) + bytes(packed_bits)
    chunk = b"VP8L" + struct.pack("<I", len(payload)) + payload
    if len(payload) & 1:
        chunk += b"\0"
    encoded = b"RIFF" + struct.pack("<I", len(chunk) + 4) + b"WEBP" + chunk
    return width, height, encoded


def _append_vp8l_lsb(bits, value, width):
    bits.extend((value >> offset) & 1 for offset in range(width))


def _append_vp8l_single_symbol_tree(bits, symbol):
    # VP8L simple Huffman tree with exactly one symbol.
    bits.extend((1, 0))
    if symbol > 1:
        bits.append(1)
        _append_vp8l_lsb(bits, symbol, 8)
    else:
        bits.append(0)
        _append_vp8l_lsb(bits, symbol, 1)


def _build_vp8l_1x1_webp(bits, *, alpha_used):
    packed_bits = bytearray((len(bits) + 7) // 8)
    for index, bit in enumerate(bits):
        packed_bits[index // 8] |= bit << (index % 8)

    # One-pixel dimensions, the requested alpha-used flag, and version zero.
    dimensions = int(alpha_used) << 28
    payload = b"\x2f" + struct.pack("<I", dimensions) + bytes(packed_bits)
    chunk = b"VP8L" + struct.pack("<I", len(payload)) + payload
    if len(payload) & 1:
        chunk += b"\0"
    return b"RIFF" + struct.pack("<I", len(chunk) + 4) + b"WEBP" + chunk


def gen_webp_lossless_branch_coverage(directory):
    """Write minimal VP8L streams for RGB fallback and color-cache branches."""
    # An opaque image with SubtractGreen takes decode_frame_rgb's transformed
    # RGBA workspace path. Raw (G, R, B, A)=(20, 246, 10, 255) becomes RGB 10,20,30.
    bits = [1]  # transform present
    _append_vp8l_lsb(bits, 2, 2)  # SubtractGreen
    bits.append(0)  # no further transforms
    bits.extend((0, 0))  # no color cache; no meta-Huffman image
    for symbol in (20, 246, 10, 255, 0):  # green, red, blue, alpha, distance
        _append_vp8l_single_symbol_tree(bits, symbol)
    rgb = _build_vp8l_1x1_webp(bits, alpha_used=False)

    # An alpha-bearing image with a one-bit cache takes the generic four-byte
    # constant-pixel path and inserts the pixel into the enabled cache.
    bits = [0, 1]  # no transforms; color cache enabled
    _append_vp8l_lsb(bits, 1, 4)  # cache size is 1 << 1
    bits.append(0)  # no meta-Huffman image
    for symbol in (20, 10, 30, 128, 0):  # green, red, blue, alpha, distance
        _append_vp8l_single_symbol_tree(bits, symbol)
    rgba = _build_vp8l_1x1_webp(bits, alpha_used=True)

    fixtures = (
        (
            "lossless_subtract_green_rgb_fallback_1x1.webp",
            rgb,
            "a18c8a0f1512f6f6e1d0b588188136f236ff1fabac20ff49904e6aa902682deb",
            "RGB",
            bytes((10, 20, 30)),
        ),
        (
            "lossless_rgba_singleton_color_cache_1x1.webp",
            rgba,
            "f05bf2424594f98f7cefb5dd1af2cd59a72f1f43aad8be58ab2679cea01c0776",
            "RGBA",
            bytes((10, 20, 30, 128)),
        ),
    )
    for filename, encoded, expected_sha256, expected_mode, expected_pixels in fixtures:
        if hashlib.sha256(encoded).hexdigest() != expected_sha256:
            raise RuntimeError(f"{filename} differs from its pinned hash")
        path = directory / filename
        path.write_bytes(encoded)
        with Image.open(path) as decoded:
            decoded.load()
            if (
                decoded.format != "WEBP"
                or decoded.mode != expected_mode
                or decoded.size != (1, 1)
                or decoded.tobytes() != expected_pixels
            ):
                raise RuntimeError(f"{filename} differs from its pinned Pillow pixels")


def gen_webp_lossless_single_symbol_copy_fallback(directory):
    """Write a VP8L meta-group whose singleton copy code takes the fallback."""
    width, height, encoded = _build_webp_lossless_single_symbol_copy_stream(259)

    path = directory / "lossless_single_symbol_copy_fallback_8x4.webp"
    expected_sha256 = "b492af14d22326f9f9f17630fdfb64b5fb9a231a0a82cd13e4b2d4d0bb7f6acc"
    if hashlib.sha256(encoded).hexdigest() != expected_sha256:
        raise RuntimeError("VP8L singleton-copy fixture differs from its pinned hash")
    if (
        encoded[:4] != b"RIFF"
        or encoded[8:12] != b"WEBP"
        or encoded[12:16] != b"VP8L"
        or encoded[20] != 0x2F
        or (int.from_bytes(encoded[21:25], "little") & 0x3FFF) + 1 != width
        or ((int.from_bytes(encoded[21:25], "little") >> 14) & 0x3FFF) + 1
        != height
        or not (int.from_bytes(encoded[21:25], "little") & (1 << 28))
    ):
        raise RuntimeError("VP8L singleton-copy fixture has an invalid RGBA 8x4 header")
    path.write_bytes(encoded)

    expected_pixels = bytes((10, 20, 30, 128)) * (width * height)
    with Image.open(path) as decoded:
        decoded.load()
        if (
            decoded.format != "WEBP"
            or decoded.mode != "RGBA"
            or decoded.size != (width, height)
            or decoded.tobytes() != expected_pixels
        ):
            raise RuntimeError("VP8L singleton-copy fixture differs from pinned Pillow RGBA")


def gen_webp_lossless_copy_out_of_bounds(directory):
    """Write a public VP8L copy whose length overruns the 8x4 image."""
    width, height, encoded = _build_webp_lossless_single_symbol_copy_stream(
        265, extra_data_bits=8
    )
    path = directory / "lossless_copy_out_of_bounds_8x4.webp"
    expected_sha256 = "abb0a3d2b630b23fc5bdd476bb59e9b7790a461ae60522ddc282fd53b27ef075"
    if hashlib.sha256(encoded).hexdigest() != expected_sha256:
        raise RuntimeError("VP8L out-of-bounds-copy fixture differs from its pinned hash")
    if (
        encoded[:4] != b"RIFF"
        or encoded[8:12] != b"WEBP"
        or encoded[12:16] != b"VP8L"
        or encoded[20] != 0x2F
        or (int.from_bytes(encoded[21:25], "little") & 0x3FFF) + 1 != width
        or ((int.from_bytes(encoded[21:25], "little") >> 14) & 0x3FFF) + 1
        != height
        or not (int.from_bytes(encoded[21:25], "little") & (1 << 28))
    ):
        raise RuntimeError("VP8L out-of-bounds-copy fixture has an invalid RGBA 8x4 header")
    path.write_bytes(encoded)

    with Image.open(path) as decoded:
        if (
            decoded.format != "WEBP"
            or decoded.mode != "RGBA"
            or decoded.size != (width, height)
        ):
            raise RuntimeError("VP8L out-of-bounds-copy fixture has an invalid Pillow header")
        try:
            decoded.load()
        except OSError as error:
            if str(error) != "failed to read next frame":
                raise RuntimeError("VP8L out-of-bounds-copy fixture changed its Pillow error") from error
        else:
            raise RuntimeError("VP8L out-of-bounds-copy fixture unexpectedly decoded in Pillow")


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


def write_rgb_planar_tiff(
    path, image, rows_per_strip, compression=1, predictor=1, photometric=2
):
    """Write RGB or 1:1 YCbCr TIFF strips in separate component planes."""
    width, height = image.size
    if rows_per_strip <= 0:
        raise ValueError("rows_per_strip must be positive")
    if compression not in (1, 8) or predictor not in (1, 2):
        raise ValueError("separate RGB TIFFs support raw or Deflate strips and Predictor 1/2")
    if predictor == 2 and compression == 1:
        raise ValueError("horizontal prediction requires compressed TIFF strips")

    if photometric not in (2, 6):
        raise ValueError("separate sample planes support RGB or 1:1 YCbCr photometric data")
    sample_mode = "YCbCr" if photometric == 6 else "RGB"
    interleaved = image.convert(sample_mode).tobytes()
    planes = [interleaved[channel::3] for channel in range(3)]
    raw_strips = [
        plane[first_row * width : min(first_row + rows_per_strip, height) * width]
        for plane in planes
        for first_row in range(0, height, rows_per_strip)
    ]
    strips = []
    for strip in raw_strips:
        encoded = bytearray(strip)
        if predictor == 2:
            for row_start in range(0, len(encoded), width):
                for column in range(width - 1, 0, -1):
                    index = row_start + column
                    encoded[index] = (encoded[index] - encoded[index - 1]) & 255
        strips.append(zlib.compress(encoded) if compression == 8 else bytes(encoded))
    entries = [
        (256, 4, 1, width),
        (257, 4, 1, height),
        (258, 3, 3, "bits"),
        (259, 3, 1, compression),
        (262, 3, 1, photometric),
        (273, 4, len(strips), "strip_offsets"),
        (277, 3, 1, 3),
        (278, 4, 1, rows_per_strip),
        (279, 4, len(strips), "strip_byte_counts"),
        (284, 3, 1, 2),
    ]
    if predictor != 1:
        entries.append((317, 3, 1, predictor))
    if photometric == 6:
        entries.append((530, 3, 2, "subsampling"))
    entries.sort()

    cursor = 8 + 2 + len(entries) * 12 + 4
    bits_offset = cursor
    cursor += 6
    if cursor & 1:
        cursor += 1
    offsets_offset = cursor
    cursor += len(strips) * 4
    byte_counts_offset = cursor
    cursor += len(strips) * 4
    strip_offsets = []
    for strip in strips:
        strip_offsets.append(cursor)
        cursor += len(strip)

    output = bytearray(b"II*\0\x08\0\0\0")
    output.extend(struct.pack("<H", len(entries)))
    for tag, field_type, count, value in entries:
        output.extend(struct.pack("<HHI", tag, field_type, count))
        if value == "bits":
            output.extend(struct.pack("<I", bits_offset))
        elif value == "strip_offsets":
            output.extend(struct.pack("<I", offsets_offset))
        elif value == "strip_byte_counts":
            output.extend(struct.pack("<I", byte_counts_offset))
        elif value == "subsampling":
            output.extend(struct.pack("<HH", 1, 1))
        elif field_type == 3:
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


def write_rgb_planar_tiled_tiff(
    path, image, tile_size, compression=1, predictor=1, photometric=2
):
    """Write RGB or 1:1 YCbCr TIFF tiles in separate component planes."""
    width, height = image.size
    if tile_size <= 0:
        raise ValueError("tile_size must be positive")
    if compression not in (1, 8) or predictor not in (1, 2):
        raise ValueError("separate tiled TIFFs support raw or Deflate tiles and Predictor 1/2")
    if predictor == 2 and compression == 1:
        raise ValueError("horizontal prediction requires compressed TIFF tiles")
    if photometric not in (2, 6):
        raise ValueError("separate sample planes support RGB or 1:1 YCbCr photometric data")

    sample_mode = "YCbCr" if photometric == 6 else "RGB"
    interleaved = image.convert(sample_mode).tobytes()
    tiles_across = (width + tile_size - 1) // tile_size
    tiles_down = (height + tile_size - 1) // tile_size
    tiles = []
    for channel in range(3):
        for tile_y in range(tiles_down):
            for tile_x in range(tiles_across):
                encoded = bytearray(tile_size * tile_size)
                for row in range(tile_size):
                    source_y = tile_y * tile_size + row
                    if source_y >= height:
                        break
                    copied_width = min(tile_size, width - tile_x * tile_size)
                    source_pixel = source_y * width + tile_x * tile_size
                    destination = row * tile_size
                    for column in range(copied_width):
                        encoded[destination + column] = interleaved[
                            (source_pixel + column) * 3 + channel
                        ]
                    if predictor == 2:
                        for column in range(tile_size - 1, 0, -1):
                            position = destination + column
                            encoded[position] = (encoded[position] - encoded[position - 1]) & 255
                tiles.append(zlib.compress(encoded) if compression == 8 else bytes(encoded))

    entries = [
        (256, 4, 1, width),
        (257, 4, 1, height),
        (258, 3, 3, "bits"),
        (259, 3, 1, compression),
        (262, 3, 1, photometric),
        (277, 3, 1, 3),
        (284, 3, 1, 2),
        (322, 4, 1, tile_size),
        (323, 4, 1, tile_size),
        (324, 4, len(tiles), "tile_offsets"),
        (325, 4, len(tiles), "tile_byte_counts"),
    ]
    if predictor != 1:
        entries.append((317, 3, 1, predictor))
    if photometric == 6:
        entries.append((530, 3, 2, "subsampling"))
    entries.sort()

    cursor = 8 + 2 + len(entries) * 12 + 4
    bits_offset = cursor
    cursor += 6
    if cursor & 1:
        cursor += 1
    offsets_offset = cursor
    cursor += len(tiles) * 4
    byte_counts_offset = cursor
    cursor += len(tiles) * 4
    tile_offsets = []
    for tile in tiles:
        tile_offsets.append(cursor)
        cursor += len(tile)

    output = bytearray(b"II*\0\x08\0\0\0")
    output.extend(struct.pack("<H", len(entries)))
    for tag, field_type, count, value in entries:
        output.extend(struct.pack("<HHI", tag, field_type, count))
        if value == "bits":
            output.extend(struct.pack("<I", bits_offset))
        elif value == "tile_offsets":
            output.extend(struct.pack("<I", offsets_offset))
        elif value == "tile_byte_counts":
            output.extend(struct.pack("<I", byte_counts_offset))
        elif value == "subsampling":
            output.extend(struct.pack("<HH", 1, 1))
        elif field_type == 3:
            output.extend(struct.pack("<H", value) + b"\0\0")
        else:
            output.extend(struct.pack("<I", value))
    output.extend(struct.pack("<I", 0))
    output.extend(struct.pack("<HHH", 8, 8, 8))
    if len(output) & 1:
        output.append(0)
    output.extend(struct.pack(f"<{len(tile_offsets)}I", *tile_offsets))
    output.extend(struct.pack(f"<{len(tiles)}I", *(len(tile) for tile in tiles)))
    for tile in tiles:
        output.extend(tile)
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


def write_low_depth_tiled_tiff(
    path,
    width,
    height,
    bits,
    photometric,
    tile_width=16,
    tile_height=16,
    compression=1,
    samples_per_pixel=1,
):
    """Write packed grayscale, palette, or RGB TIFF tiles for Pillow parity fixtures."""
    if bits not in (1, 2, 4) or photometric not in (0, 1, 2, 3):
        raise ValueError("packed TIFF tiles support 1-, 2-, or 4-bit samples")
    if samples_per_pixel not in (1, 3):
        raise ValueError("packed TIFF tiles support one or three samples per pixel")
    if photometric == 2:
        if samples_per_pixel != 3:
            raise ValueError("packed RGB TIFF tiles require three samples per pixel")
    elif samples_per_pixel != 1:
        raise ValueError("packed non-RGB TIFF tiles require one sample per pixel")
    if width <= 0 or height <= 0 or tile_width <= 0 or tile_height <= 0:
        raise ValueError("TIFF image and tile dimensions must be positive")
    if compression not in (1, 8):
        raise ValueError("packed TIFF tiles support raw or Deflate compression")

    # Pillow's libtiff accepts packed tiles narrower than the TIFF multiple-of-16
    # recommendation; a 9-pixel width exercises unaligned packed sample placement.
    maximum = (1 << bits) - 1
    tile_row_bytes = (tile_width * bits * samples_per_pixel + 7) // 8
    encoded_tiles = []
    for tile_y in range((height + tile_height - 1) // tile_height):
        for tile_x in range((width + tile_width - 1) // tile_width):
            tile = bytearray(tile_row_bytes * tile_height)
            for row in range(tile_height):
                source_y = tile_y * tile_height + row
                if source_y >= height:
                    break
                for column in range(tile_width):
                    source_x = tile_x * tile_width + column
                    if source_x >= width:
                        break
                    for channel in range(samples_per_pixel):
                        if photometric == 3:
                            sample = (source_x * 3 + source_y * 5) & maximum
                        elif photometric == 2:
                            sample = (source_x * 3 + source_y * 5 + channel * 2) & maximum
                        else:
                            luminance = (source_x * 37 + source_y * 19) & 255
                            sample = (luminance * maximum + 127) // 255
                            if photometric == 0:
                                sample = maximum - sample
                        sample_index = column * samples_per_pixel + channel
                        bit = sample_index * bits
                        tile[row * tile_row_bytes + bit // 8] |= sample << (
                            8 - bits - bit % 8
                        )
            encoded_tiles.append(
                zlib.compress(tile) if compression == 8 else bytes(tile)
            )

    color_map = []
    entries = [
        (256, 4, 1, width),
        (257, 4, 1, height),
        (258, 3, samples_per_pixel, "bits_per_sample" if samples_per_pixel > 1 else bits),
        (259, 3, 1, compression),
        (262, 3, 1, photometric),
        (277, 3, 1, samples_per_pixel),
        (284, 3, 1, 1),
        (322, 4, 1, tile_width),
        (323, 4, 1, tile_height),
        (324, 4, len(encoded_tiles), "tile_offsets"),
        (325, 4, len(encoded_tiles), "tile_byte_counts"),
    ]
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
    bits_per_sample_offset = cursor
    if samples_per_pixel > 1:
        cursor += samples_per_pixel * 2
    if cursor & 1:
        cursor += 1
    tile_offsets_offset = cursor
    cursor += len(encoded_tiles) * 4
    tile_counts_offset = cursor
    cursor += len(encoded_tiles) * 4
    tile_offsets = []
    for tile in encoded_tiles:
        tile_offsets.append(cursor)
        cursor += len(tile)

    output = bytearray(b"II*\0\x08\0\0\0")
    output.extend(struct.pack("<H", len(entries)))
    for tag, field_type, count, value in entries:
        output.extend(struct.pack("<HHI", tag, field_type, count))
        if value == "color_map":
            output.extend(struct.pack("<I", color_map_offset))
        elif value == "bits_per_sample":
            output.extend(struct.pack("<I", bits_per_sample_offset))
        elif value == "tile_offsets":
            output.extend(struct.pack("<I", tile_offsets_offset))
        elif value == "tile_byte_counts":
            output.extend(struct.pack("<I", tile_counts_offset))
        elif field_type == 3:
            output.extend(struct.pack("<H", value) + b"\0\0")
        else:
            output.extend(struct.pack("<I", value))
    output.extend(struct.pack("<I", 0))
    if color_map:
        output.extend(struct.pack(f"<{len(color_map)}H", *color_map))
    if samples_per_pixel > 1:
        output.extend(struct.pack(f"<{samples_per_pixel}H", *([bits] * samples_per_pixel)))
    if len(output) & 1:
        output.append(0)
    output.extend(struct.pack(f"<{len(tile_offsets)}I", *tile_offsets))
    output.extend(
        struct.pack(f"<{len(encoded_tiles)}I", *(len(tile) for tile in encoded_tiles))
    )
    for tile in encoded_tiles:
        output.extend(tile)
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


def pack_lzw_codes_with_growth(codes):
    """Pack a TIFF LZW stream with the format's early-change code widths."""
    fields = []
    code_width = 9
    next_code = 258
    has_previous = False
    for code in codes:
        fields.append((code, code_width))
        if code == 256:
            code_width = 9
            next_code = 258
            has_previous = False
        elif code == 257:
            break
        elif not has_previous:
            has_previous = True
        elif next_code < 4096:
            next_code += 1
            if code_width < 12 and next_code == (1 << code_width) - 1:
                code_width += 1

    bits = "".join(f"{code:0{width}b}" for code, width in fields)
    bits += "0" * (-len(bits) % 8)
    return int(bits, 2).to_bytes(len(bits) // 8, "big")


def write_lzw_tiff(path, codes, width=1):
    write_compressed_grayscale_tiff(path, pack_lzw_codes(codes), 5, width)


def write_lzw_long_phrase_checkpoint_tiff(path):
    """Write a valid LZW stream whose final phrase crosses a token checkpoint."""
    final_phrase_length = 1_025
    pixel_count = final_phrase_length * (final_phrase_length + 1) // 2
    growing_phrases = range(258, 258 + final_phrase_length - 1)
    codes = [256, 0xAD, *growing_phrases, 257]
    payload = pack_lzw_codes_with_growth(codes)
    write_compressed_grayscale_tiff(path, payload, 5, pixel_count)


def write_lzw_extra_zero_output_strip(path):
    """Write a valid one-pixel LZW image with an extra zero-row strip."""
    first_strip = pack_lzw_codes([256, 65, 257])
    extra_strip = pack_lzw_codes([0])
    entries = [
        (256, 4, 1, 1),
        (257, 4, 1, 1),
        (258, 3, 1, 8),
        (259, 3, 1, 5),
        (262, 3, 1, 1),
        (273, 4, 2, "strip_offsets"),
        (277, 3, 1, 1),
        (278, 4, 1, 1),
        (279, 4, 2, "strip_byte_counts"),
    ]
    external_start = 8 + 2 + len(entries) * 12 + 4
    byte_counts_offset = external_start + 8
    pixels_offset = byte_counts_offset + 8
    strip_offsets = (pixels_offset, pixels_offset + len(first_strip))

    output = bytearray(b"II*\0" + struct.pack("<I", 8))
    output.extend(struct.pack("<H", len(entries)))
    for tag, field_type, count, value in entries:
        output.extend(struct.pack("<HHI", tag, field_type, count))
        if value == "strip_offsets":
            output.extend(struct.pack("<I", external_start))
        elif value == "strip_byte_counts":
            output.extend(struct.pack("<I", byte_counts_offset))
        elif field_type == 3:
            output.extend(struct.pack("<H", value) + b"\0\0")
        else:
            output.extend(struct.pack("<I", value))
    output.extend(struct.pack("<I", 0))
    output.extend(struct.pack("<II", *strip_offsets))
    output.extend(struct.pack("<II", len(first_strip), len(extra_strip)))
    output.extend(first_strip)
    output.extend(extra_strip)
    path.write_bytes(output)


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


def write_short_deflate_tile_tiff(source, destination, output_prefix_bytes):
    """Make one valid Deflate tile decode to fewer bytes than a packed row."""
    data = bytearray(source.read_bytes())
    if data[:2] not in (b"II", b"MM"):
        raise ValueError("TIFF byte-order marker is invalid")
    byte_order = "<" if data[:2] == b"II" else ">"
    ifd_offset = struct.unpack_from(byte_order + "I", data, 4)[0]
    entry_count = struct.unpack_from(byte_order + "H", data, ifd_offset)[0]
    entries = {}
    for index in range(entry_count):
        start = ifd_offset + 2 + index * 12
        tag, field_type, count = struct.unpack_from(byte_order + "HHI", data, start)
        if tag in (259, 324, 325):
            entries[tag] = (start, field_type, count)

    def integer_values(tag):
        try:
            start, field_type, count = entries[tag]
        except KeyError as error:
            raise ValueError(f"TIFF tag {tag} is missing") from error
        if field_type not in (3, 4):
            raise ValueError(f"TIFF tag {tag} is not an integer array")
        item_size, format_code = (2, "H") if field_type == 3 else (4, "I")
        value_position = (
            start + 8
            if count * item_size <= 4
            else struct.unpack_from(byte_order + "I", data, start + 8)[0]
        )
        values = struct.unpack_from(byte_order + f"{count}{format_code}", data, value_position)
        return values, value_position

    compression, _ = integer_values(259)
    if compression[0] not in (8, 32946):
        raise ValueError("TIFF source does not use Deflate compression")
    offsets, _ = integer_values(324)
    byte_counts, byte_counts_position = integer_values(325)
    if len(offsets) != len(byte_counts) or not offsets:
        raise ValueError("TIFF tile offsets and byte counts do not match")

    tile_offset = offsets[0]
    tile_end = tile_offset + byte_counts[0]
    if tile_end > len(data):
        raise ValueError("TIFF first tile payload is truncated")
    decoded_tile = zlib.decompress(data[tile_offset:tile_end])
    if not 0 < output_prefix_bytes < len(decoded_tile):
        raise ValueError("short Deflate output length is outside the tile")
    compressed_prefix = zlib.compress(decoded_tile[:output_prefix_bytes])
    if len(compressed_prefix) > byte_counts[0]:
        raise ValueError("short Deflate stream exceeds the original tile payload")

    data[tile_offset : tile_offset + len(compressed_prefix)] = compressed_prefix
    struct.pack_into(byte_order + "I", data, byte_counts_position, len(compressed_prefix))
    destination.write_bytes(data)


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
    rgba_frame = Image.new("RGBA", (2, 1))
    rgba_frame.putdata([(255, 0, 0, 0), (0, 0, 255, 255)])
    rgba_frame_next = Image.new("RGBA", (2, 1))
    rgba_frame_next.putdata([(0, 255, 0, 255), (0, 0, 0, 0)])
    rgba_frame.save(
        d / "rgba_animation_transparency.tiff",
        save_all=True,
        append_images=[rgba_frame_next],
    )
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
    write_ycbcr_tiff(d / "ycbcr_checkpoint_32x32.tiff", img.resize((32, 32)))
    img.convert("1").save(d / "bilevel.tiff")
    low_depth = img.convert("L").resize((17, 13))
    write_low_depth_tiff(d / "miniswhite_1bit.tiff", low_depth, 1, 0)
    write_low_depth_tiff(
        d / "miniswhite_1bit_aligned.tiff",
        low_depth.resize((16, 8)),
        1,
        0,
    )
    miniswhite_checkpoint = Image.new("L", (128, 64))
    miniswhite_checkpoint.putdata(
        [(index * 37 + 11) & 255 for index in range(128 * 64)]
    )
    write_low_depth_tiff(
        d / "miniswhite_checkpoint_128x64.tiff",
        miniswhite_checkpoint,
        1,
        0,
    )
    miniswhite_sample_checkpoint = Image.new("L", (32, 32))
    miniswhite_sample_checkpoint.putdata(
        [(index * 37 + 11) & 255 for index in range(32 * 32)]
    )
    write_low_depth_tiff(
        d / "miniswhite_gray8_checkpoint_32x32.tiff",
        miniswhite_sample_checkpoint,
        8,
        0,
    )
    write_low_depth_tiff(
        d / "miniswhite_gray2_checkpoint_32x32.tiff",
        miniswhite_sample_checkpoint,
        2,
        0,
    )
    write_low_depth_tiff(d / "miniswhite_8bit.tiff", low_depth, 8, 0)
    write_low_depth_tiff(d / "gray2.tiff", low_depth, 2, 1)
    write_low_depth_tiff(d / "gray4.tiff", low_depth, 4, 1)
    write_low_depth_tiff(d / "miniswhite_2bit.tiff", low_depth, 2, 0)
    write_low_depth_tiff(d / "miniswhite_4bit.tiff", low_depth, 4, 0)
    write_low_depth_tiff(d / "palette2.tiff", low_depth, 2, 3)
    write_low_depth_tiff(d / "palette4.tiff", low_depth, 4, 3)
    write_low_depth_tiled_tiff(
        d / "tiled_minisblack_1bit_edge.tiff", 17, 13, 1, 1
    )
    write_low_depth_tiled_tiff(
        d / "tiled_minisblack_1bit_unaligned_edge.tiff",
        17,
        1,
        1,
        1,
        tile_width=9,
    )
    write_low_depth_tiled_tiff(
        d / "tiled_gray2_deflate_edge.tiff", 17, 13, 2, 1, compression=8
    )
    short_deflate_tile = d / "tiled_gray2_deflate_short_tile.tiff"
    write_short_deflate_tile_tiff(
        d / "tiled_gray2_deflate_edge.tiff", short_deflate_tile, 3
    )
    if hashlib.sha256(short_deflate_tile.read_bytes()).hexdigest() != (
        "1fb7f77d7d6a281cb667c365fe097bc868d841ec246783bd0b5014f4417127d8"
    ):
        raise RuntimeError("short Deflate TIFF tile fixture differs from its pinned hash")
    write_low_depth_tiled_tiff(
        d / "tiled_gray2_unaligned_edge.tiff", 17, 1, 2, 1, tile_width=9
    )
    write_low_depth_tiled_tiff(
        d / "tiled_palette4_edge.tiff", 17, 13, 4, 3
    )
    mutate_tiff_tag(
        d / "tiled_palette4_edge.tiff",
        d / "tiled_palette4_oob_tile.tiff",
        324,
        0xFFFF_FFF0,
    )
    write_low_depth_tiled_tiff(
        d / "tiled_palette4_unaligned_edge.tiff",
        17,
        1,
        4,
        3,
        tile_width=9,
    )
    write_low_depth_tiled_tiff(
        d / "tiled_rgb1_packed_multisample.tiff",
        9,
        9,
        1,
        2,
        samples_per_pixel=3,
    )
    img.save(d / "uncompressed.tiff", compression=None)
    img.save(d / "lzw.tiff", compression="tiff_lzw")
    img.save(d / "deflate.tiff", compression="tiff_adobe_deflate")
    predictor_checkpoint = pattern_img("RGB", (343, 2))
    predictor_checkpoint_path = d / "deflate_predictor_checkpoint_343x2.tiff"
    predictor_checkpoint.save(
        predictor_checkpoint_path,
        compression="tiff_adobe_deflate",
        tiffinfo={317: 2},
    )
    with Image.open(predictor_checkpoint_path) as oracle:
        oracle.load()
        if (
            oracle.size != (343, 2)
            or oracle.tag_v2.get(317) != 2
            or oracle.tobytes() != predictor_checkpoint.tobytes()
        ):
            raise RuntimeError("TIFF predictor checkpoint fixture changed its Pillow output")

    gray16_predictor = Image.new("I;16", (513, 1))
    gray16_predictor.putdata([(x * 997 + 123) & 0xFFFF for x in range(513)])
    gray16_predictor_path = d / "deflate_predictor_checkpoint_513x1_gray16.tiff"
    gray16_predictor.save(
        gray16_predictor_path,
        compression="tiff_adobe_deflate",
        tiffinfo={317: 2},
    )
    with Image.open(gray16_predictor_path) as oracle:
        oracle.load()
        if (
            oracle.size != (513, 1)
            or oracle.mode != "I;16"
            or oracle.tag_v2.get(317) != 2
            or oracle.tobytes() != gray16_predictor.tobytes()
        ):
            raise RuntimeError("16-bit TIFF predictor checkpoint fixture changed its Pillow output")

    gray32_predictor = Image.new("I", (257, 1))
    gray32_predictor.putdata(
        [([0x7FFF_FFFF, -0x8000_0000, -1, 0][x % 4]) for x in range(257)]
    )
    gray32_predictor_path = d / "deflate_predictor_checkpoint_257x1_gray32_signed.tiff"
    gray32_predictor.save(
        gray32_predictor_path,
        compression="tiff_adobe_deflate",
        tiffinfo={317: 2},
    )
    with Image.open(gray32_predictor_path) as oracle:
        oracle.load()
        if (
            oracle.size != (257, 1)
            or oracle.mode != "I"
            or oracle.tag_v2.get(317) != 2
            or oracle.tag_v2.get(339) != (2,)
            or oracle.tobytes() != gray32_predictor.tobytes()
        ):
            raise RuntimeError("signed 32-bit TIFF predictor checkpoint fixture changed its Pillow output")
    img.save(d / "packbits.tiff", compression="packbits")
    write_packbits_tiff(d / "packbits_noop.tiff", b"\x80\x00\x7f")
    write_packbits_tiff(d / "packbits_trailing_noop.tiff", b"\x00\x2a\x80")
    write_packbits_tiff(d / "packbits_literal_overrun.tiff", b"\x01\x00\x01")
    write_packbits_tiff(d / "packbits_run_overrun.tiff", b"\xff\x00")
    write_packbits_tiff(d / "packbits_short_output.tiff", b"\x80")
    software_metadata = Image.new("L", (2, 1))
    software_metadata.putdata([17, 203])
    software_metadata.save(
        d / "software_metadata.tiff", tiffinfo={305: "coverage-witness"}
    )
    mutate_tiff_tag(
        d / "packbits.tiff",
        d / "packbits_zero_strip_byte_count.tiff",
        279,
        0,
    )
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
    write_lzw_extra_zero_output_strip(d / "lzw_extra_zero_output_strip.tiff")
    write_lzw_dictionary_saturation_tiff(d / "lzw_dictionary_saturation.tiff")
    write_lzw_long_phrase_checkpoint_tiff(
        d / "lzw_long_phrase_checkpoint_1024.tiff"
    )
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
    planar_image = pattern_img("RGB", size=(17, 17))
    write_rgb_planar_tiff(
        d / "planar_separate_rgb.tiff", planar_image, rows_per_strip=8
    )
    write_rgb_planar_tiff(
        d / "planar_separate_rgb_deflate_predictor.tiff",
        planar_image,
        rows_per_strip=8,
        compression=8,
        predictor=2,
    )
    write_rgb_planar_tiff(
        d / "planar_separate_rgb_deflate.tiff",
        planar_image,
        rows_per_strip=8,
        compression=8,
    )
    write_rgb_planar_tiff(
        d / "planar_separate_ycbcr.tiff",
        planar_image.resize((17, 13)),
        rows_per_strip=8,
        photometric=6,
    )
    planar_tiled_image = pattern_img("RGB", size=(65, 33))
    write_rgb_planar_tiled_tiff(
        d / "planar_separate_rgb_tiled_edge.tiff",
        planar_tiled_image,
        tile_size=32,
    )
    write_rgb_planar_tiled_tiff(
        d / "planar_separate_rgb_tiled_deflate_predictor.tiff",
        planar_tiled_image,
        tile_size=32,
        compression=8,
        predictor=2,
    )
    write_rgb_planar_tiled_tiff(
        d / "planar_separate_rgb_tiled_deflate.tiff",
        planar_tiled_image,
        tile_size=32,
        compression=8,
    )
    write_rgb_planar_tiled_tiff(
        d / "planar_separate_ycbcr_tiled.tiff",
        planar_image.resize((17, 13)),
        tile_size=32,
        photometric=6,
    )
    with tempfile.TemporaryDirectory() as scratch:
        partial_tile_offsets = Path(scratch) / "planar_tile_offsets.tiff"
        mutate_tiff_tag_count(
            d / "planar_separate_rgb_tiled_edge.tiff",
            partial_tile_offsets,
            324,
            17,
        )
        mutate_tiff_tag_count(
            partial_tile_offsets,
            d / "planar_separate_rgb_tiled_missing_tail_tile.tiff",
            325,
            17,
        )
    mutate_tiff_tag(
        d / "planar_separate_rgb_tiled_edge.tiff",
        d / "planar_separate_rgb_tiled_zero_width.tiff",
        322,
        0,
    )
    mutate_tiff_tag(
        d / "planar_separate_rgb_tiled_edge.tiff",
        d / "planar_separate_rgb_tiled_zero_height.tiff",
        323,
        0,
    )
    mutate_tiff_tag_count(
        d / "planar_separate_rgb_tiled_deflate_predictor.tiff",
        d / "planar_separate_rgb_tiled_deflate_mismatched_counts.tiff",
        325,
        17,
    )
    mutate_tiff_tag_count(
        d / "planar_separate_rgb_tiled_edge.tiff",
        d / "planar_separate_rgb_tiled_extra_offset.tiff",
        324,
        19,
    )
    mutate_tiff_tag(
        d / "planar_separate_rgb_tiled_edge.tiff",
        d / "planar_separate_rgb_tiled_oob_tile.tiff",
        324,
        0xFFFF_FFF0,
    )
    planar_gray = pattern_img("L", size=(17, 17))
    planar_gray.save(
        d / "planar_separate_gray_single_sample.tiff",
        compression=None,
        tiffinfo={284: 2},
    )
    mutate_tiff_tag(
        d / "planar_separate_rgb.tiff",
        d / "planar_separate_rgb_rows_zero.tiff",
        278,
        0,
    )
    mutate_tiff_tag(
        d / "planar_separate_rgb.tiff",
        d / "planar_separate_rgb_oob_strip.tiff",
        273,
        0xFFFF_FFF0,
    )
    mutate_tiff_tag(
        d / "planar_separate_rgb.tiff",
        d / "planar_separate_rgb_invalid_planar.tiff",
        284,
        3,
    )
    with tempfile.TemporaryDirectory() as scratch:
        partial_offsets = Path(scratch) / "planar_offsets.tiff"
        mutate_tiff_tag_count(
            d / "planar_separate_rgb.tiff", partial_offsets, 273, 8
        )
        mutate_tiff_tag_count(
            partial_offsets,
            d / "planar_separate_rgb_missing_tail_strip.tiff",
            279,
            8,
        )
    mutate_tiff_tag_count(
        d / "planar_separate_rgb.tiff",
        d / "planar_separate_rgb_extra_offset.tiff",
        273,
        10,
    )
    mutate_tiff_tag_count(
        d / "planar_separate_rgb_deflate_predictor.tiff",
        d / "planar_separate_rgb_deflate_mismatched_counts.tiff",
        279,
        8,
    )
    mutate_tiff_tag_count(
        d / "planar_separate_rgb_deflate_predictor.tiff",
        d / "planar_separate_rgb_deflate_missing_counts.tiff",
        279,
        0,
    )
    mutate_tiff_tag_id(
        d / "planar_separate_rgb_deflate_predictor.tiff",
        d / "planar_separate_rgb_deflate_no_counts.tiff",
        279,
        65000,
    )
    mutate_tiff_tag(
        d / "planar_separate_rgb_deflate_predictor.tiff",
        d / "planar_separate_rgb_deflate_zero_count.tiff",
        279,
        0,
    )
    write_rgb_tiff(d / "tiled.tiff", img, tile_size=32)
    write_rgb_tiff(
        d / "tiled_deflate_plain.tiff",
        img,
        tile_size=32,
        compression=8,
    )
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
    aligned_sequence_page = pattern_img("RGB", (4, 3))
    aligned_sequence_page.save(
        d / "multipage_aligned.tiff",
        save_all=True,
        append_images=[aligned_sequence_page.transpose(Image.Transpose.FLIP_LEFT_RIGHT)],
    )
    third_sequence_page = pattern_img("RGB", (5, 11))
    sequence_page.save(
        d / "multipage_three.tiff",
        save_all=True,
        append_images=[
            sequence_page.transpose(Image.Transpose.FLIP_LEFT_RIGHT),
            third_sequence_page,
        ],
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
    mutate_tiff_tag(d / "rgb.tiff", d / "samples_per_pixel_zero.tiff", 277, 0)
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
    mutate_tiff_tag(d / "tiled.tiff", d / "zero_tile_height.tiff", 323, 0)
    mutate_tiff_tag_id(
        d / "tiled.tiff", d / "missing_tile_width.tiff", 322, 65_000
    )
    mutate_tiff_tag_id(
        d / "tiled.tiff", d / "missing_tile_height.tiff", 323, 65_000
    )
    mutate_tiff_tag_id(
        d / "tiled.tiff",
        d / "tile_byte_counts_without_offsets.tiff",
        324,
        65_000,
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
    bmp_1bit_default_palette = bytearray((d / "bmp_1bit.ico").read_bytes())
    struct.pack_into("<I", bmp_1bit_default_palette, 22 + 32, 0)
    (d / "bmp_1bit_default_palette.ico").write_bytes(bmp_1bit_default_palette)
    img.convert("P", palette=Image.Palette.ADAPTIVE, colors=64).resize((16,16)).save(
        d / "bmp_8bit.ico", format="ICO", sizes=[(16,16)], bitmap_format="bmp"
    )
    img.convert("P", palette=Image.Palette.ADAPTIVE, colors=256).resize((16,16)).save(
        d / "bmp_8bit_default_palette.ico",
        format="ICO",
        sizes=[(16,16)],
        bitmap_format="bmp",
    )
    bmp_8bit_default_palette = bytearray((d / "bmp_8bit_default_palette.ico").read_bytes())
    struct.pack_into("<I", bmp_8bit_default_palette, 22 + 32, 0)
    (d / "bmp_8bit_default_palette.ico").write_bytes(bmp_8bit_default_palette)
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

    # Pillow promotes an identity grayscale palette in 4-bpp ICO DIBs to raw
    # luminance bytes; a non-identity gray palette remains indexed. A two-entry
    # black/white palette uses its separate 1-bpp raw interpretation.
    def write_4bit_odd_width(name, palette_values, xor=bytes((0x12, 0, 0, 0))):
        and_mask = bytes(4)
        dib = bytearray(
            struct.pack(
                "<IiiHHIIiiII",
                40,
                3,
                2,
                1,
                4,
                0,
                0,
                0,
                0,
                len(palette_values),
                0,
            )
        )
        for color in palette_values:
            if isinstance(color, int):
                color = (color, color, color)
            red, green, blue = color
            dib.extend(bytes((blue, green, red, 0)))
        dib.extend(xor)
        dib.extend(and_mask)
        ico = bytearray(struct.pack("<HHH", 0, 1, 1))
        ico.extend(struct.pack("<BBBBHHII", 3, 1, 0, 0, 1, 4, len(dib), 22))
        ico.extend(dib)
        (d / name).write_bytes(ico)

    write_4bit_odd_width("bmp_4bit_odd_width.ico", (0, 1, 2))
    write_4bit_odd_width("bmp_4bit_odd_width_gray_control.ico", (0, 10, 20))
    write_4bit_odd_width("bmp_4bit_odd_width_binary_gray.ico", (0, 255))
    write_4bit_odd_width(
        "bmp_4bit_odd_width_binary_gray_white_first.ico",
        (0, 255),
        bytes((0x80, 0, 0, 0)),
    )
    write_4bit_odd_width(
        "bmp_4bit_two_entry_red_green.ico",
        ((255, 0, 0), (0, 255, 0)),
        bytes((0x01, 0, 0, 0)),
    )
    write_4bit_odd_width(
        "bmp_4bit_two_entry_near_gray.ico",
        ((0, 0, 0), (254, 254, 254)),
        bytes((0x01, 0, 0, 0)),
    )
    write_4bit_odd_width(
        "bmp_4bit_missing_palette_index.ico",
        ((1, 2, 3), (4, 5, 6), (7, 8, 9)),
        bytes((0x30, 0, 0, 0)),
    )

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
    sub_eight_byte_payload = bytearray(struct.pack("<HHH", 0, 1, 1))
    sub_eight_byte_payload.extend(
        struct.pack("<BBBBHHII", 16, 16, 0, 0, 1, 32, 7, 22)
    )
    sub_eight_byte_payload.extend(struct.pack("<I", 40) + bytes((16, 0, 0)))
    sub_eight_byte_payload_path = d / "short_dib_payload_below_eight.ico"
    sub_eight_byte_payload_path.write_bytes(sub_eight_byte_payload)
    try:
        with Image.open(sub_eight_byte_payload_path) as oracle:
            oracle.load()
    except OSError as error:
        if str(error) != "Truncated File Read":
            raise RuntimeError("short ICO DIB fixture changed its Pillow error") from error
    else:
        raise RuntimeError("short ICO DIB fixture unexpectedly decoded in Pillow")
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
    short_header_path = d / "short_header.ico"
    short_header_path.write_bytes(struct.pack("<HH", 0, 1))
    try:
        with Image.open(short_header_path) as short_header:
            short_header.load()
    except OSError:
        pass
    else:
        raise RuntimeError("Pillow unexpectedly decoded an ICO header without its entry count")
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


def rewrite_avif_operating_point_sequence(source_path):
    """Set T0/S0 as the explicit operating point in a pinned AVIF sequence."""
    from inspect_av1_obus import parse_sequence_header, read_uleb128
    from inspect_avif_bitstreams import inspect as inspect_avif, parse_boxes, unique_box

    data = source_path.read_bytes()
    expected_source_hash = (
        "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
    )
    if hashlib.sha256(data).hexdigest() != expected_source_hash:
        raise RuntimeError("AV1 operating-point source differs from the pinned animation")

    report = inspect_avif(source_path)
    tracks = report["tracks"]
    if len(tracks) != 1 or len(tracks[0]["samples"]) != 5:
        raise RuntimeError("AV1 operating-point source must contain five track samples")
    samples = tracks[0]["samples"]
    original_lengths = [sample["length"] for sample in samples]
    if original_lengths != [39, 113, 5, 30, 25]:
        raise RuntimeError("AV1 operating-point source sample lengths changed")
    primary_item = report["items"]["color"][0]
    if primary_item["spans"] != [
        {"offset": samples[0]["offset"], "length": samples[0]["length"]}
    ]:
        raise RuntimeError("primary AV1 item no longer references the first sample")

    rewritten_samples = []
    insertions = []
    sequence_header_count = 0
    extension_count = 0
    for sample in samples:
        sample_start = sample["offset"]
        original = data[sample_start : sample_start + sample["length"]]
        rebuilt = bytearray()
        offset = 0
        while offset < len(original):
            obu_start = offset
            header = original[offset]
            offset += 1
            obu_type = (header >> 3) & 0x0F
            has_extension = bool(header & 0x04)
            has_size = bool(header & 0x02)
            if header & 0x81 or has_extension or not has_size:
                raise RuntimeError("pinned AV1 sequence has an unexpected OBU header")

            payload_size, payload_start = read_uleb128(original, offset)
            payload_end = payload_start + payload_size
            if payload_end > len(original):
                raise RuntimeError("pinned AV1 OBU payload exceeds its track sample")
            payload = bytearray(original[payload_start:payload_end])
            if obu_type == 1:
                sequence_header_count += 1
                parsed_header = parse_sequence_header(bytes(payload))
                operating_points = parsed_header["operating_points"]
                if (
                    parsed_header["still_picture"]
                    or parsed_header["reduced_still_picture_header"]
                    or len(operating_points) != 1
                    or operating_points[0]["idc"] != 0
                ):
                    raise RuntimeError("pinned AV1 sequence operating point changed")
                # The operating_point_idc field starts at bit offset 12 in this
                # full sequence header and occupies the next 12 bits.
                operating_point_idc = 0x101
                for bit_index in range(12):
                    absolute_bit = 12 + bit_index
                    mask = 1 << (7 - absolute_bit % 8)
                    bit = (operating_point_idc >> (11 - bit_index)) & 1
                    if bit:
                        payload[absolute_bit // 8] |= mask
                    else:
                        payload[absolute_bit // 8] &= ~mask
                parsed_header = parse_sequence_header(bytes(payload))
                if parsed_header["operating_points"][0]["idc"] != operating_point_idc:
                    raise RuntimeError("AV1 operating-point mutation was not applied")

            has_layer_extension = obu_type in (3, 4, 6, 7)
            if has_layer_extension:
                rebuilt.append(header | 0x04)
                # T0/S0 membership is selected by IDC 0x101; reserved bits stay 0.
                rebuilt.append(0)
                insertions.append(sample_start + obu_start + 1)
                extension_count += 1
            else:
                rebuilt.append(header)
            rebuilt.extend(_write_av1_leb128(payload_size))
            rebuilt.extend(payload)
            offset = payload_end
        rewritten_samples.append(bytes(rebuilt))

    if sequence_header_count != 1 or extension_count != 7:
        raise RuntimeError("AV1 operating-point source OBU inventory changed")
    expected_lengths = [40, 116, 6, 31, 26]
    if [len(sample) for sample in rewritten_samples] != expected_lengths:
        raise RuntimeError("AV1 operating-point sample mutation changed shape")

    boxes = parse_boxes(data, 0, len(data))
    mdat = unique_box(boxes, b"mdat")
    if mdat.header_size != 8 or mdat.end != len(data):
        raise RuntimeError("pinned AV1 sequence mdat layout changed")
    if samples[0]["offset"] != mdat.payload_start:
        raise RuntimeError("pinned AV1 samples no longer start at mdat payload")
    if samples[-1]["offset"] + samples[-1]["length"] != mdat.end:
        raise RuntimeError("pinned AV1 samples no longer end at mdat boundary")
    if any(
        left["offset"] + left["length"] != right["offset"]
        for left, right in zip(samples, samples[1:])
    ):
        raise RuntimeError("pinned AV1 track samples are no longer contiguous")

    new_payload = b"".join(rewritten_samples)
    added_bytes = len(new_payload) - (mdat.end - mdat.payload_start)
    if added_bytes != len(insertions):
        raise RuntimeError("AV1 OBU extension size accounting differs")
    mdat_header = bytearray(data[mdat.start : mdat.payload_start])
    struct.pack_into(">I", mdat_header, 0, mdat.size + added_bytes)
    output = bytearray(data[: mdat.start] + mdat_header + new_payload)

    if data.count(b"stsz") != 1:
        raise RuntimeError("pinned AV1 sequence must contain one stsz box")
    stsz = data.find(b"stsz") - 4
    if stsz < 0:
        raise RuntimeError("pinned AV1 sequence stsz box is truncated")
    fixed_size, sample_count = struct.unpack_from(">II", data, stsz + 12)
    if fixed_size != 0 or sample_count != len(rewritten_samples):
        raise RuntimeError("pinned AV1 stsz layout changed")
    for index, sample in enumerate(rewritten_samples):
        struct.pack_into(">I", output, stsz + 20 + 4 * index, len(sample))

    if data.count(b"stco") != 1:
        raise RuntimeError("pinned AV1 sequence must contain one stco box")
    stco = data.find(b"stco") - 4
    if stco < 0:
        raise RuntimeError("pinned AV1 sequence stco box is truncated")
    chunk_count = struct.unpack_from(">I", data, stco + 12)[0]
    for index in range(chunk_count):
        position = stco + 16 + 4 * index
        old_offset = struct.unpack_from(">I", data, position)[0]
        bytes_before = sum(
            len(rewritten_samples[sample_index]) - sample["length"]
            for sample_index, sample in enumerate(samples)
            if sample["offset"] + sample["length"] <= old_offset
        )
        struct.pack_into(">I", output, position, old_offset + bytes_before)

    if data.count(b"iloc") != 1:
        raise RuntimeError("pinned AV1 sequence must contain one iloc box")
    iloc = data.find(b"iloc") - 4
    if iloc < 0:
        raise RuntimeError("pinned AV1 sequence iloc box is truncated")
    position = iloc + 8
    version = data[position]
    position += 4
    widths = struct.unpack_from(">H", data, position)[0]
    position += 2
    offset_width = widths >> 12
    length_width = (widths >> 8) & 0x0F
    base_width = (widths >> 4) & 0x0F
    index_width = (widths & 0x0F) if version in (1, 2) else 0
    if (
        version not in (0, 1, 2)
        or offset_width not in (4, 8)
        or length_width not in (4, 8)
        or base_width not in (0, 4, 8)
        or index_width not in (0, 4, 8)
    ):
        raise RuntimeError("pinned AV1 iloc integer widths changed")
    item_count_width = 4 if version == 2 else 2
    item_count = int.from_bytes(data[position : position + item_count_width], "big")
    position += item_count_width
    primary_item_adjusted = False
    for _ in range(item_count):
        item_id_width = 4 if version == 2 else 2
        item_id = int.from_bytes(data[position : position + item_id_width], "big")
        position += item_id_width
        construction_method = 0
        if version in (1, 2):
            construction_method = struct.unpack_from(">H", data, position)[0] & 0x0F
            position += 2
        position += 2  # data_reference_index
        base_offset = (
            int.from_bytes(data[position : position + base_width], "big")
            if base_width
            else 0
        )
        position += base_width
        extent_count = struct.unpack_from(">H", data, position)[0]
        position += 2
        for _ in range(extent_count):
            if index_width:
                position += index_width
            extent_offset_position = position
            extent_offset = (
                int.from_bytes(data[position : position + offset_width], "big")
                if offset_width
                else 0
            )
            position += offset_width
            extent_length_position = position
            extent_length = (
                int.from_bytes(data[position : position + length_width], "big")
                if length_width
                else 0
            )
            position += length_width
            if construction_method != 0:
                continue
            extent_start = base_offset + extent_offset
            extent_end = extent_start + extent_length
            bytes_inside = sum(
                extent_start < insertion < extent_end for insertion in insertions
            )
            bytes_before = sum(insertion < extent_start for insertion in insertions)
            if item_id == report["items"]["primary_item_id"]:
                if extent_start != samples[0]["offset"] or not bytes_inside:
                    raise RuntimeError("primary AV1 item extent no longer matches sample 0")
                if length_width:
                    output[
                        extent_length_position : extent_length_position + length_width
                    ] = (extent_length + bytes_inside).to_bytes(length_width, "big")
                primary_item_adjusted = True
            elif bytes_inside and length_width:
                output[
                    extent_length_position : extent_length_position + length_width
                ] = (extent_length + bytes_inside).to_bytes(length_width, "big")
            if bytes_before and offset_width:
                output[
                    extent_offset_position : extent_offset_position + offset_width
                ] = (extent_offset + bytes_before).to_bytes(offset_width, "big")
    iloc_size = int.from_bytes(data[iloc : iloc + 4], "big")
    if position != iloc + iloc_size or not primary_item_adjusted:
        raise RuntimeError("pinned AV1 iloc extents were not updated exactly")

    expected_hash = (
        "25b79a856ea2767e02e5a509f72e7af3f9563202301779be65b0745724ab23be"
    )
    if hashlib.sha256(output).hexdigest() != expected_hash:
        raise RuntimeError("AV1 operating-point fixture differs from its pinned hash")
    return bytes(output)


def _write_av1_leb128(value):
    output = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            output.append(byte | 0x80)
        else:
            output.append(byte)
            return bytes(output)


def write_multitile_monochrome_alpha_zero_loop_filter(path):
    """Generate a two-tile alpha item that carries no loop-filter metadata."""
    from PIL import Image

    from inspect_av1_obus import inspect as inspect_av1

    image = Image.new("RGBA", (128, 64))
    image.putdata(
        [
            (
                37,
                83,
                131,
                (47 if x < 64 else 193)
                if (x + y) % 3
                else (128 if x < 64 else 222),
            )
            for y in range(64)
            for x in range(128)
        ]
    )
    image.save(
        path,
        format="AVIF",
        quality=100,
        speed=6,
        tile_cols=1,
        tile_rows=0,
        autotiling=False,
        advanced=[("enable-cdef", "0")],
    )

    if hashlib.sha256(path.read_bytes()).hexdigest() != (
        "2f331625a30f1b25ec19ef863bad9ae08e32eba4f40a6413892eb238fa1135e0"
    ):
        raise RuntimeError(
            "zero-loop-filter monochrome alpha AVIF differs from its pinned fixture"
        )

    report = inspect_av1(path)
    alpha_sample = next(
        sample for sample in report["samples"] if sample["role"] == "item_alpha"
    )
    sequence = next(
        obu["sequence_header"]
        for obu in alpha_sample["obus"]
        if "sequence_header" in obu
    )
    alpha_frame = next(obu for obu in alpha_sample["obus"] if obu["type"] == 6)
    header = alpha_frame["frame_header"]
    loop_filter = header["loop_filter"]
    tile_indices = [tile["index"] for tile in alpha_frame["tile_group"]["tiles"]]
    if (
        sequence["monochrome"] is not True
        or sequence["enable_cdef"] is not False
        or (header["frame_width"], header["frame_height"]) != (128, 64)
        or tile_indices != [0, 1]
        or header["all_lossless"] is not True
        or loop_filter["level_y"] != [0, 0]
        or loop_filter["level_u"] != 0
        or loop_filter["level_v"] != 0
    ):
        raise RuntimeError("zero-loop-filter monochrome alpha AV1 layout differs")

    with Image.open(path) as oracle:
        oracle.load()
        if oracle.mode != "RGBA" or oracle.size != image.size:
            raise RuntimeError(
                "zero-loop-filter monochrome alpha AVIF Pillow output differs"
            )


def avif_parse_boxes(data, start=0, end=None):
    """Parse a bounded sequence of ISO-BMFF boxes into kind/payload pairs."""
    end = len(data) if end is None else end
    if not 0 <= start <= end <= len(data):
        raise RuntimeError("AVIF box range is outside its parent")

    boxes = []
    position = start
    while position < end:
        remaining = end - position
        if remaining < 8:
            raise RuntimeError("AVIF box header is truncated")
        size, kind = struct.unpack_from(">I4s", data, position)
        header_size = 8
        if size == 1:
            if remaining < 16:
                raise RuntimeError("AVIF large-box header is truncated")
            size = struct.unpack_from(">Q", data, position + 8)[0]
            header_size = 16
        elif size == 0:
            size = remaining
        if size < header_size or size > remaining:
            raise RuntimeError(f"AVIF {kind!r} box size is invalid")
        payload_start = position + header_size
        boxes.append((kind, bytes(data[payload_start : position + size])))
        position += size
    return boxes


def avif_unique_top_level_payload_range(data, wanted_kind):
    """Return the payload range for one uniquely named top-level box."""
    ranges = []
    position = 0
    while position < len(data):
        remaining = len(data) - position
        if remaining < 8:
            raise RuntimeError("AVIF top-level box header is truncated")
        size, kind = struct.unpack_from(">I4s", data, position)
        header_size = 8
        if size == 1:
            if remaining < 16:
                raise RuntimeError("AVIF top-level large-box header is truncated")
            size = struct.unpack_from(">Q", data, position + 8)[0]
            header_size = 16
        elif size == 0:
            size = remaining
        if size < header_size or size > remaining:
            raise RuntimeError(f"AVIF top-level {kind!r} box size is invalid")
        if kind == wanted_kind:
            ranges.append((position + header_size, position + size))
        position += size
    if len(ranges) != 1:
        raise RuntimeError(
            f"expected one top-level {wanted_kind!r} box, found {len(ranges)}"
        )
    return ranges[0]


def avif_pack_boxes(boxes):
    """Serialize a bounded sequence of standard-size ISO-BMFF boxes."""
    packed = bytearray()
    for kind, payload in boxes:
        size = len(payload) + 8
        if len(kind) != 4 or size > 0xFFFF_FFFF:
            raise RuntimeError("AVIF fixture box cannot use a 32-bit header")
        packed.extend(struct.pack(">I4s", size, kind))
        packed.extend(payload)
    return bytes(packed)


def avif_duplicate_stsz_before_invalid_stco(source):
    """Duplicate stsz in an animated table before a truncated stco table."""
    root = avif_parse_boxes(source)
    if avif_pack_boxes(root) != source:
        raise RuntimeError("animated AVIF uses unsupported box headers")

    def unique_child(boxes, kind):
        indexes = [index for index, (child_kind, _) in enumerate(boxes) if child_kind == kind]
        if len(indexes) != 1:
            raise RuntimeError(
                f"animated AVIF needs one {kind!r} child, found {len(indexes)}"
            )
        return indexes[0]

    moov_index = unique_child(root, b"moov")
    moov = avif_parse_boxes(root[moov_index][1])
    trak_index = unique_child(moov, b"trak")
    trak = avif_parse_boxes(moov[trak_index][1])
    mdia_index = unique_child(trak, b"mdia")
    mdia = avif_parse_boxes(trak[mdia_index][1])
    minf_index = unique_child(mdia, b"minf")
    minf = avif_parse_boxes(mdia[minf_index][1])
    stbl_index = unique_child(minf, b"stbl")
    stbl = avif_parse_boxes(minf[stbl_index][1])
    stsz_index = unique_child(stbl, b"stsz")
    stss_index = unique_child(stbl, b"stss")
    stco_index = unique_child(stbl, b"stco")

    stco_payload = bytearray(stbl[stco_index][1])
    if len(stco_payload) != 12 or stco_payload[4:8] != bytes.fromhex("00000001"):
        raise RuntimeError("animated AVIF needs one bounded stco sample offset")
    stco_payload[4:8] = bytes.fromhex("00000002")

    duplicate_stsz = (b"stsz", stbl[stss_index][1])
    stbl = [
        child
        for index, child in enumerate(stbl)
        if index not in {stss_index, stco_index}
    ]
    stbl.insert(stsz_index + 1, duplicate_stsz)
    stbl.append((b"stco", bytes(stco_payload)))

    minf[stbl_index] = (b"stbl", avif_pack_boxes(stbl))
    mdia[minf_index] = (b"minf", avif_pack_boxes(minf))
    trak[mdia_index] = (b"mdia", avif_pack_boxes(mdia))
    moov[trak_index] = (b"trak", avif_pack_boxes(trak))
    root[moov_index] = (b"moov", avif_pack_boxes(moov))
    return avif_pack_boxes(root)


def widen_avif_ipma_associations(source):
    """Set ipma's wide-association flag and preserve its essential bits."""
    root = avif_parse_boxes(source)
    meta_indexes = [index for index, (kind, _) in enumerate(root) if kind == b"meta"]
    mdat_indexes = [index for index, (kind, _) in enumerate(root) if kind == b"mdat"]
    if len(meta_indexes) != 1 or len(mdat_indexes) != 1:
        raise RuntimeError("wide-ipma fixture needs one meta and one mdat box")
    meta_index = meta_indexes[0]
    if meta_index >= mdat_indexes[0]:
        raise RuntimeError("wide-ipma fixture metadata must precede its item data")

    meta_payload = root[meta_index][1]
    if len(meta_payload) < 4 or meta_payload[:4] != bytes(4):
        raise RuntimeError("wide-ipma fixture requires a version-zero meta box")
    meta_children = avif_parse_boxes(meta_payload, 4)
    iprp_indexes = [
        index for index, (kind, _) in enumerate(meta_children) if kind == b"iprp"
    ]
    iloc_indexes = [
        index for index, (kind, _) in enumerate(meta_children) if kind == b"iloc"
    ]
    if len(iprp_indexes) != 1 or len(iloc_indexes) != 1:
        raise RuntimeError("wide-ipma fixture needs one iprp and one iloc child")

    iprp_index = iprp_indexes[0]
    iprp_children = avif_parse_boxes(meta_children[iprp_index][1])
    ipma_indexes = [
        index for index, (kind, _) in enumerate(iprp_children) if kind == b"ipma"
    ]
    if len(ipma_indexes) != 1:
        raise RuntimeError("wide-ipma fixture needs one ipma child")
    ipma_index = ipma_indexes[0]
    ipma = iprp_children[ipma_index][1]
    if (
        len(ipma) != 15
        or ipma[:8] != bytes.fromhex("0000000000000001")
        or struct.unpack_from(">H", ipma, 8)[0] != 1
        or ipma[10] != 4
        or ipma[11:] != bytes((0x01, 0x02, 0x83, 0x04))
    ):
        raise RuntimeError("wide-ipma fixture source has an unexpected association layout")

    widened = bytearray(ipma[:11])
    widened[3] |= 1
    for association in ipma[11:]:
        value = ((association & 0x80) << 8) | (association & 0x7F)
        widened.extend(struct.pack(">H", value))
    growth = len(widened) - len(ipma)
    iprp_children[ipma_index] = (b"ipma", bytes(widened))
    meta_children[iprp_index] = (b"iprp", avif_pack_boxes(iprp_children))

    iloc = bytearray(meta_children[iloc_indexes[0]][1])
    if (
        len(iloc) != 22
        or iloc[:14] != bytes.fromhex("0000000044000001000100000001")
    ):
        raise RuntimeError("wide-ipma fixture source needs one file-backed iloc extent")
    struct.pack_into(">I", iloc, 14, struct.unpack_from(">I", iloc, 14)[0] + growth)
    meta_children[iloc_indexes[0]] = (b"iloc", bytes(iloc))
    root[meta_index] = (
        b"meta",
        meta_payload[:4] + avif_pack_boxes(meta_children),
    )
    encoded = avif_pack_boxes(root)
    if len(encoded) != len(source) + growth:
        raise RuntimeError("wide-ipma fixture changed an unexpected number of bytes")
    return encoded


def upgrade_avif_iloc_to_version_2(source):
    """Widen baseline item-location fields to the supported version 2 layout."""
    root = avif_parse_boxes(source)
    meta_indexes = [index for index, (kind, _) in enumerate(root) if kind == b"meta"]
    mdat_indexes = [index for index, (kind, _) in enumerate(root) if kind == b"mdat"]
    if len(meta_indexes) != 1 or len(mdat_indexes) != 1:
        raise RuntimeError("iloc-v2 fixture needs one meta and one mdat box")
    meta_index = meta_indexes[0]
    if meta_index >= mdat_indexes[0]:
        raise RuntimeError("iloc-v2 fixture metadata must precede its item data")

    meta_payload = root[meta_index][1]
    if len(meta_payload) < 4 or meta_payload[:4] != bytes(4):
        raise RuntimeError("iloc-v2 fixture requires a version-zero meta box")
    meta_children = avif_parse_boxes(meta_payload, 4)
    iloc_indexes = [
        index for index, (kind, _) in enumerate(meta_children) if kind == b"iloc"
    ]
    if len(iloc_indexes) != 1:
        raise RuntimeError("iloc-v2 fixture needs one iloc child")

    iloc_index = iloc_indexes[0]
    iloc = meta_children[iloc_index][1]
    if (
        len(iloc) != 22
        or iloc[:14] != bytes.fromhex("0000000044000001000100000001")
    ):
        raise RuntimeError("iloc-v2 fixture source has an unexpected item layout")
    item_count, item_id = struct.unpack_from(">HH", iloc, 6)
    data_reference_index = struct.unpack_from(">H", iloc, 10)[0]
    if (item_count, item_id, data_reference_index) != (1, 1, 0):
        raise RuntimeError("iloc-v2 fixture source needs one primary file-backed item")

    extent_offset, extent_length = struct.unpack_from(">II", iloc, 14)
    mdat_payload_start, mdat_payload_end = avif_unique_top_level_payload_range(
        source, b"mdat"
    )
    if not (
        mdat_payload_start <= extent_offset
        and extent_offset + extent_length <= mdat_payload_end
    ):
        raise RuntimeError("iloc-v2 source item extent is outside the mdat payload")

    # Version 2 widens item_count and item_ID by two bytes each and adds the
    # 16-bit construction_method field before data_reference_index.
    growth = 6
    widened = (
        bytes((2,))
        + iloc[1:6]
        + struct.pack(">IIHH", item_count, item_id, 0, data_reference_index)
        + iloc[12:14]
        + struct.pack(">II", extent_offset + growth, extent_length)
    )
    if len(widened) != len(iloc) + growth:
        raise RuntimeError("iloc-v2 fixture changed an unexpected number of bytes")
    meta_children[iloc_index] = (b"iloc", widened)
    root[meta_index] = (
        b"meta",
        meta_payload[:4] + avif_pack_boxes(meta_children),
    )
    encoded = avif_pack_boxes(root)
    if len(encoded) != len(source) + growth:
        raise RuntimeError("iloc-v2 fixture changed an unexpected file extent")

    encoded_root = avif_parse_boxes(encoded)
    encoded_meta = next(payload for kind, payload in encoded_root if kind == b"meta")
    encoded_iloc = next(
        payload
        for kind, payload in avif_parse_boxes(encoded_meta, 4)
        if kind == b"iloc"
    )
    encoded_mdat_start, encoded_mdat_end = avif_unique_top_level_payload_range(
        encoded, b"mdat"
    )
    encoded_extent_offset, encoded_extent_length = struct.unpack_from(
        ">II", encoded_iloc, 20
    )
    if (
        encoded_iloc[0] != 2
        or struct.unpack_from(">II", encoded_iloc, 6) != (1, 1)
        or struct.unpack_from(">HH", encoded_iloc, 14) != (0, 0)
        or encoded_extent_offset != encoded_mdat_start
        or encoded_extent_offset + encoded_extent_length > encoded_mdat_end
    ):
        raise RuntimeError("iloc-v2 fixture fields or relocated extent are invalid")
    return encoded


def add_avif_mdcv_property(source):
    """Associate one BT.2020/D65 `mdcv` property with the primary image."""
    root = avif_parse_boxes(source)
    meta_indexes = [index for index, (kind, _) in enumerate(root) if kind == b"meta"]
    mdat_indexes = [index for index, (kind, _) in enumerate(root) if kind == b"mdat"]
    if len(meta_indexes) != 1 or len(mdat_indexes) != 1:
        raise RuntimeError("AVIF ICC still must contain one meta and one mdat box")
    meta_index = meta_indexes[0]
    if meta_index >= mdat_indexes[0]:
        raise RuntimeError("AVIF ICC still must place metadata before its item data")

    meta_payload = root[meta_index][1]
    if len(meta_payload) < 4 or meta_payload[:4] != bytes(4):
        raise RuntimeError("AVIF ICC still must use a version-zero meta box")
    meta_children = avif_parse_boxes(meta_payload, 4)
    iloc_indexes = [
        index for index, (kind, _) in enumerate(meta_children) if kind == b"iloc"
    ]
    iprp_indexes = [
        index for index, (kind, _) in enumerate(meta_children) if kind == b"iprp"
    ]
    if len(iloc_indexes) != 1 or len(iprp_indexes) != 1:
        raise RuntimeError("AVIF ICC still must contain one iloc and one iprp box")
    pitm_indexes = [
        index for index, (kind, _) in enumerate(meta_children) if kind == b"pitm"
    ]
    if (
        len(pitm_indexes) != 1
        or meta_children[pitm_indexes[0]][1] != bytes.fromhex("000000000001")
    ):
        raise RuntimeError("AVIF ICC still primary item must be version-zero item 1")

    iloc_index = iloc_indexes[0]
    iloc = bytearray(meta_children[iloc_index][1])
    if (
        len(iloc) != 22
        or iloc[:4] != bytes(4)
        or iloc[4:6] != bytes((0x44, 0))
        or iloc[6:14] != bytes.fromhex("0001000100000001")
    ):
        raise RuntimeError("AVIF ICC still must have one version-zero file extent")
    old_extent_offset = struct.unpack_from(">I", iloc, 14)[0]
    extent_length = struct.unpack_from(">I", iloc, 18)[0]
    mdat_payload_start, mdat_payload_end = avif_unique_top_level_payload_range(
        source, b"mdat"
    )
    if not (
        mdat_payload_start <= old_extent_offset
        and old_extent_offset + extent_length <= mdat_payload_end
    ):
        raise RuntimeError("AVIF ICC still item extent is outside the mdat payload")
    original_item = source[old_extent_offset : old_extent_offset + extent_length]
    if original_item != root[mdat_indexes[0]][1]:
        raise RuntimeError("AVIF ICC still extent differs from the complete mdat payload")

    iprp_index = iprp_indexes[0]
    iprp = avif_parse_boxes(meta_children[iprp_index][1])
    ipco_indexes = [index for index, (kind, _) in enumerate(iprp) if kind == b"ipco"]
    ipma_indexes = [index for index, (kind, _) in enumerate(iprp) if kind == b"ipma"]
    if len(ipco_indexes) != 1 or len(ipma_indexes) != 1:
        raise RuntimeError("AVIF ICC still must contain one ipco and one ipma box")

    ipco_index = ipco_indexes[0]
    properties = avif_parse_boxes(iprp[ipco_index][1])
    if len(properties) != 5 or any(kind == b"mdcv" for kind, _ in properties):
        raise RuntimeError("AVIF ICC still property table differs from its fixture")

    ipma_index = ipma_indexes[0]
    ipma = bytearray(iprp[ipma_index][1])
    if (
        len(ipma) != 16
        or ipma[:8] != bytes.fromhex("0000000000000001")
        or ipma[8:11] != bytes((0, 1, 5))
        or ipma[11:16] != bytes((1, 2, 0x83, 4, 5))
    ):
        raise RuntimeError("AVIF ICC still primary property associations differ")

    # ISO/IEC 23008-12 stores green, blue, red, white, then max/min luminance.
    mdcv_payload = struct.pack(
        ">8H2I",
        13_250,
        34_500,
        7_500,
        3_000,
        34_000,
        16_000,
        15_635,
        16_450,
        10_000_000,
        50,
    )
    properties.append((b"mdcv", mdcv_payload))
    iprp[ipco_index] = (b"ipco", avif_pack_boxes(properties))

    # Property indexes are one-based; mdcv remains non-essential metadata.
    ipma[10] = 6
    ipma.append(6)
    iprp[ipma_index] = (b"ipma", bytes(ipma))
    meta_children[iprp_index] = (b"iprp", avif_pack_boxes(iprp))

    # The new box grows ipco by 32 bytes and its association grows ipma by one.
    metadata_growth = len(mdcv_payload) + 8 + 1
    struct.pack_into(">I", iloc, 14, old_extent_offset + metadata_growth)
    meta_children[iloc_index] = (b"iloc", bytes(iloc))
    root[meta_index] = (b"meta", meta_payload[:4] + avif_pack_boxes(meta_children))
    encoded = avif_pack_boxes(root)
    new_extent_offset = old_extent_offset + metadata_growth
    new_mdat_payload_start, new_mdat_payload_end = avif_unique_top_level_payload_range(
        encoded, b"mdat"
    )
    if (
        new_extent_offset != new_mdat_payload_start
        or new_extent_offset + extent_length != new_mdat_payload_end
        or encoded[new_extent_offset : new_extent_offset + extent_length] != original_item
    ):
        raise RuntimeError("AVIF mdcv insertion moved the primary item bytes")
    return encoded


def add_avif_pasp_property(source):
    """Associate a valid 4:3 `pasp` property with the primary image."""
    expected_source_sha256 = (
        "df1fadfd3b7780e3c41825352ee5768731a6753dac613412dfa94d6715f93f7f"
    )
    if hashlib.sha256(source).hexdigest() != expected_source_sha256:
        raise RuntimeError("AVIF primary-item rotation source differs from its pinned hash")

    root = avif_parse_boxes(source)

    def unique_index(boxes, kind, label):
        indexes = [index for index, (box_kind, _) in enumerate(boxes) if box_kind == kind]
        if len(indexes) != 1:
            raise RuntimeError(f"AVIF pasp fixture needs one {label} box")
        return indexes[0]

    meta_index = unique_index(root, b"meta", "meta")
    mdat_index = unique_index(root, b"mdat", "mdat")
    if meta_index >= mdat_index:
        raise RuntimeError("AVIF pasp fixture metadata must precede item data")

    meta_payload = root[meta_index][1]
    if len(meta_payload) < 4 or meta_payload[:4] != bytes(4):
        raise RuntimeError("AVIF pasp fixture requires a version-zero meta box")
    meta_children = avif_parse_boxes(meta_payload, 4)
    iloc_index = unique_index(meta_children, b"iloc", "iloc")
    iprp_index = unique_index(meta_children, b"iprp", "iprp")
    pitm_index = unique_index(meta_children, b"pitm", "pitm")
    if meta_children[pitm_index][1] != bytes.fromhex("000000000001"):
        raise RuntimeError("AVIF pasp fixture primary item must be version-zero item 1")

    iloc = bytearray(meta_children[iloc_index][1])
    if (
        len(iloc) != 22
        or iloc[:14] != bytes.fromhex("0000000044000001000100000001")
    ):
        raise RuntimeError("AVIF pasp fixture needs one version-zero 32-bit item extent")
    old_extent_offset, extent_length = struct.unpack_from(">II", iloc, 14)
    mdat_payload_start, mdat_payload_end = avif_unique_top_level_payload_range(
        source, b"mdat"
    )
    if not (
        old_extent_offset == mdat_payload_start
        and old_extent_offset + extent_length == mdat_payload_end
    ):
        raise RuntimeError("AVIF pasp source extent must cover the complete mdat payload")
    original_item = source[old_extent_offset : old_extent_offset + extent_length]

    iprp = avif_parse_boxes(meta_children[iprp_index][1])
    ipco_index = unique_index(iprp, b"ipco", "ipco")
    ipma_index = unique_index(iprp, b"ipma", "ipma")
    properties = avif_parse_boxes(iprp[ipco_index][1])
    if len(properties) != 5 or any(kind == b"pasp" for kind, _ in properties):
        raise RuntimeError("AVIF pasp source property table differs from its fixture")

    ipma = bytearray(iprp[ipma_index][1])
    if (
        len(ipma) != 16
        or ipma[:8] != bytes.fromhex("0000000000000001")
        or ipma[8:11] != bytes((0, 1, 5))
        or len(ipma[11:]) != ipma[10]
    ):
        raise RuntimeError("AVIF pasp source must associate five properties with item 1")

    pasp_payload = struct.pack(">II", 4, 3)
    properties.append((b"pasp", pasp_payload))
    iprp[ipco_index] = (b"ipco", avif_pack_boxes(properties))
    ipma[10] += 1
    ipma.append(len(properties))
    iprp[ipma_index] = (b"ipma", bytes(ipma))
    meta_children[iprp_index] = (b"iprp", avif_pack_boxes(iprp))

    metadata_growth = 8 + len(pasp_payload) + 1
    struct.pack_into(">I", iloc, 14, old_extent_offset + metadata_growth)
    meta_children[iloc_index] = (b"iloc", bytes(iloc))
    root[meta_index] = (b"meta", meta_payload[:4] + avif_pack_boxes(meta_children))
    encoded = avif_pack_boxes(root)
    new_extent_offset = old_extent_offset + metadata_growth
    new_mdat_payload_start, new_mdat_payload_end = avif_unique_top_level_payload_range(
        encoded, b"mdat"
    )
    if (
        len(encoded) != len(source) + metadata_growth
        or new_extent_offset != new_mdat_payload_start
        or new_extent_offset + extent_length != new_mdat_payload_end
        or encoded[new_extent_offset : new_extent_offset + extent_length] != original_item
    ):
        raise RuntimeError("AVIF pasp insertion moved the primary item bytes unexpectedly")
    expected_output_sha256 = (
        "2acc2e59572718a5fd1247aef7d18ab7666943533f068c221117df05378c4564"
    )
    if hashlib.sha256(encoded).hexdigest() != expected_output_sha256:
        raise RuntimeError("AVIF pasp fixture differs from its pinned hash")
    return encoded


def add_avif_mdcv_trailing_byte(source):
    """Add one bounded trailing metadata byte to a primary `mdcv` property."""
    root = avif_parse_boxes(source)
    meta_indexes = [index for index, (kind, _) in enumerate(root) if kind == b"meta"]
    if len(meta_indexes) != 1:
        raise RuntimeError("AVIF mdcv control must contain one meta box")
    meta_index = meta_indexes[0]
    meta_payload = root[meta_index][1]
    if len(meta_payload) < 4 or meta_payload[:4] != bytes(4):
        raise RuntimeError("AVIF mdcv control must use a version-zero meta box")
    meta_children = avif_parse_boxes(meta_payload, 4)

    iloc_indexes = [
        index for index, (kind, _) in enumerate(meta_children) if kind == b"iloc"
    ]
    iprp_indexes = [
        index for index, (kind, _) in enumerate(meta_children) if kind == b"iprp"
    ]
    if len(iloc_indexes) != 1 or len(iprp_indexes) != 1:
        raise RuntimeError("AVIF mdcv control must contain one iloc and one iprp box")
    pitm_indexes = [
        index for index, (kind, _) in enumerate(meta_children) if kind == b"pitm"
    ]
    if (
        len(pitm_indexes) != 1
        or meta_children[pitm_indexes[0]][1] != bytes.fromhex("000000000001")
    ):
        raise RuntimeError("AVIF mdcv control primary item must be version-zero item 1")
    iloc_index = iloc_indexes[0]
    iloc = bytearray(meta_children[iloc_index][1])
    if (
        len(iloc) != 22
        or iloc[:4] != bytes(4)
        or iloc[4:6] != bytes((0x44, 0))
        or iloc[6:14] != bytes.fromhex("0001000100000001")
    ):
        raise RuntimeError("AVIF mdcv control must have one version-zero file extent")

    iprp_index = iprp_indexes[0]
    iprp = avif_parse_boxes(meta_children[iprp_index][1])
    ipco_indexes = [index for index, (kind, _) in enumerate(iprp) if kind == b"ipco"]
    ipma_indexes = [index for index, (kind, _) in enumerate(iprp) if kind == b"ipma"]
    if len(ipco_indexes) != 1 or len(ipma_indexes) != 1:
        raise RuntimeError("AVIF mdcv control must contain one ipco and one ipma box")

    ipco_index = ipco_indexes[0]
    properties = avif_parse_boxes(iprp[ipco_index][1])
    ipma_index = ipma_indexes[0]
    ipma = iprp[ipma_index][1]
    if (
        len(properties) != 6
        or properties[-1][0] != b"mdcv"
        or len(properties[-1][1]) != 24
        or len(ipma) != 17
        or ipma[:8] != bytes.fromhex("0000000000000001")
        or ipma[8:11] != bytes((0, 1, 6))
        or ipma[11:17] != bytes((1, 2, 0x83, 4, 5, 6))
    ):
        raise RuntimeError("AVIF mdcv control property association differs")

    properties[-1] = (b"mdcv", properties[-1][1] + b"\x00")
    iprp[ipco_index] = (b"ipco", avif_pack_boxes(properties))
    meta_children[iprp_index] = (b"iprp", avif_pack_boxes(iprp))

    old_extent_offset = struct.unpack_from(">I", iloc, 14)[0]
    extent_length = struct.unpack_from(">I", iloc, 18)[0]
    mdat_payload_start, mdat_payload_end = avif_unique_top_level_payload_range(
        source, b"mdat"
    )
    if not (
        mdat_payload_start <= old_extent_offset
        and old_extent_offset + extent_length <= mdat_payload_end
    ):
        raise RuntimeError("AVIF mdcv control item extent is outside its mdat payload")
    original_item = source[old_extent_offset : old_extent_offset + extent_length]
    mdat_payload = next(payload for kind, payload in root if kind == b"mdat")
    if original_item != mdat_payload:
        raise RuntimeError("AVIF mdcv control extent differs from its mdat payload")

    new_extent_offset = old_extent_offset + 1
    struct.pack_into(">I", iloc, 14, new_extent_offset)
    meta_children[iloc_index] = (b"iloc", bytes(iloc))
    root[meta_index] = (b"meta", meta_payload[:4] + avif_pack_boxes(meta_children))
    encoded = avif_pack_boxes(root)
    new_mdat_payload_start, new_mdat_payload_end = avif_unique_top_level_payload_range(
        encoded, b"mdat"
    )
    if (
        new_extent_offset != new_mdat_payload_start
        or new_extent_offset + extent_length != new_mdat_payload_end
        or encoded[new_extent_offset : new_extent_offset + extent_length] != original_item
    ):
        raise RuntimeError("AVIF mdcv trailing-byte insertion moved the primary item")
    return encoded


def read_error_resilient_avif_sequence_header(directory):
    """Return the pinned animation's checked primary AV1 sequence header."""
    source_path = directory / "animated_error_resilient.avif"
    source = source_path.read_bytes()
    if hashlib.sha256(source).hexdigest() != (
        "06ea9771f8b46c3432c6c6cdf324f1c05e86a5fdccd774c8e3c9a8fce0b831f0"
    ):
        raise RuntimeError("error-resilient AVIF source differs from its pinned hash")

    from inspect_av1_obus import inspect as inspect_av1_obus

    report = inspect_av1_obus(source_path)
    sample = next(
        (item for item in report["samples"] if item["role"] == "item_color"),
        None,
    )
    if sample is None:
        raise RuntimeError("error-resilient AVIF has no primary color sample")
    sequence_obu = next(
        (item for item in sample["obus"] if item["type"] == 1),
        None,
    )
    if sequence_obu is None:
        raise RuntimeError("error-resilient AVIF has no sequence-header OBU")

    sequence_header = sequence_obu["sequence_header"]
    payload_spans = sequence_obu["payload_spans"]
    payload = bytearray.fromhex(sequence_header["payload_hex"])
    if (
        sequence_header["reduced_still_picture_header"]
        or not sequence_header["frame_id_numbers_present"]
        or sequence_header.get("delta_frame_id_bits") != 14
        or sequence_header.get("frame_id_bits") != 15
        or sequence_header.get("payload_bits") != 104
        or len(payload_spans) != 1
        or payload_spans[0]["length"] != len(payload)
    ):
        raise RuntimeError("error-resilient AVIF frame-ID header layout changed")

    payload_span = payload_spans[0]
    start = payload_span["offset"]
    end = start + payload_span["length"]
    if source[start:end] != payload:
        raise RuntimeError("error-resilient AVIF sequence-header payload moved")

    return source, sequence_header, payload_span, payload


def mutate_av1_sequence_header_field(payload, field_start, field_width, expected, replacement):
    """Replace one checked MSB-first sequence-header field in its payload."""
    if (
        field_start < 0
        or field_width <= 0
        or field_start + field_width > len(payload) * 8
    ):
        raise RuntimeError("AV1 sequence-header field is outside its payload")
    shift = len(payload) * 8 - field_start - field_width
    field_mask = (1 << field_width) - 1
    encoded_payload = int.from_bytes(payload, "big")
    current = (encoded_payload >> shift) & field_mask
    if current != expected:
        raise RuntimeError("AV1 sequence-header field differs from its pinned value")
    encoded_payload &= ~(field_mask << shift)
    encoded_payload |= replacement << shift
    return encoded_payload.to_bytes(len(payload), "big")


def write_avif_frame_id_width_error_fixture(directory):
    """Mutate the pinned AV1 sequence header to declare a 17-bit frame ID."""
    source, sequence_header, payload_span, payload = (
        read_error_resilient_avif_sequence_header(directory)
    )

    # The pinned 104-bit sequence header stores the three-bit additional
    # frame-ID length at bit 50. Replacing 0 with 2 makes the total width 17.
    payload = mutate_av1_sequence_header_field(
        payload, field_start=50, field_width=3, expected=0, replacement=2
    )

    mutated = bytearray(source)
    start = payload_span["offset"]
    end = start + payload_span["length"]
    mutated[start:end] = payload
    if hashlib.sha256(mutated).hexdigest() != (
        "6970dc8e41a8861097dff0fbd8577b84dde72ec2018d3f2f75169bf7d78a123e"
    ):
        raise RuntimeError("over-width AV1 frame-ID mutation differs")
    (directory / "error_sequence_frame_id_width_exceeds_16.avif").write_bytes(
        mutated
    )


def write_avif_identity_color_matrix_error_fixture(directory):
    """Declare identity-matrix CICP for an invalid profile-2 8-bit I420 stream."""
    source, sequence_header, payload_span, payload = (
        read_error_resilient_avif_sequence_header(directory)
    )
    if (
        sequence_header.get("profile") != 0
        or sequence_header.get("bit_depth") != 8
        or not sequence_header.get("subsampling_x")
        or not sequence_header.get("subsampling_y")
        or sequence_header.get("color_primaries") != 1
        or sequence_header.get("transfer_characteristics") != 13
        or sequence_header.get("matrix_coefficients") != 6
    ):
        raise RuntimeError("error-resilient AVIF 8-bit I420 color description changed")

    # Profile 2 with high_bitdepth=0 remains 8-bit and preserves 4:2:0
    # subsampling; its CICP matrix field is still at bit 90. Identity requires
    # profile-2 12-bit 4:4:4, so this remains an observable malformed stream.
    payload = mutate_av1_sequence_header_field(
        payload, field_start=0, field_width=3, expected=0, replacement=2
    )
    payload = mutate_av1_sequence_header_field(
        payload, field_start=90, field_width=8, expected=6, replacement=0
    )
    mutated = bytearray(source)
    start = payload_span["offset"]
    end = start + payload_span["length"]
    mutated[start:end] = payload
    if hashlib.sha256(mutated).hexdigest() != (
        "478d360b9c47d12f32c1a2f6ee9da0a483f7b6ac87b1ea93deda0e786dea07f6"
    ):
        raise RuntimeError("identity color-matrix AVIF mutation differs")
    (directory / "error_sequence_identity_color_matrix_profile2_8bit.avif").write_bytes(
        mutated
    )


def gen_avif():
    d = OUT / "avif"
    d.mkdir(parents=True, exist_ok=True)
    write_avif_frame_id_width_error_fixture(d)
    write_avif_identity_color_matrix_error_fixture(d)
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

    def normalize_sequence_timestamps(data, fixture_name):
        """Zero version-one movie timestamps so full AVIF bytes repeat across runs."""
        encoded = bytearray(data)
        for box_kind in (b"mvhd", b"tkhd", b"mdhd"):
            kind_offset = encoded.find(box_kind)
            if kind_offset < 4 or encoded[kind_offset + 4] != 1:
                raise RuntimeError(
                    f"{fixture_name} lacks version-one {box_kind!r}"
                )
            if encoded.find(box_kind, kind_offset + 4) != -1:
                raise RuntimeError(
                    f"{fixture_name} has multiple {box_kind!r} boxes"
                )
            encoded[kind_offset + 8 : kind_offset + 24] = bytes(16)
        return bytes(encoded)

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

    icc_source_path = OUT / "png" / "iccp.png"
    with Image.open(icc_source_path) as profile_source:
        icc_profile = profile_source.info.get("icc_profile")
    if not icc_profile:
        raise RuntimeError("AVIF ICC fixture source has no Pillow ICC profile")
    icc_image = Image.new("RGB", (4, 4), (17, 91, 203))

    def encode_icc_profile_still():
        output = BytesIO()
        icc_image.save(
            output,
            format="AVIF",
            icc_profile=icc_profile,
            quality=99,
            speed=8,
            max_threads=1,
            subsampling="4:4:4",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
            },
        )
        return output.getvalue()

    icc_profile_still = encode_icc_profile_still()
    if icc_profile_still != encode_icc_profile_still():
        raise RuntimeError("AVIF ICC fixture is not deterministic")

    if hashlib.sha256(icc_profile_still).hexdigest() != (
        "a59749aa4985d4954cb8ef8f513e17b40f1051f969374eff7a654aeb0a629d95"
    ):
        raise RuntimeError("AVIF ICC fixture differs from its pinned hash")
    icc_profile_path = d / "icc_profile.avif"
    icc_profile_path.write_bytes(icc_profile_still)
    with Image.open(icc_profile_path) as image:
        image.load()
        if (
            image.size != icc_image.size
            or image.mode != "RGB"
            or image.info.get("icc_profile") != icc_profile
        ):
            raise RuntimeError("Pillow did not retain the AVIF ICC profile")
        icc_profile_pixels = image.tobytes()

    icc_mdcv_still = add_avif_mdcv_property(icc_profile_still)
    if hashlib.sha256(icc_mdcv_still).hexdigest() != (
        "d06f268e6f7341d67e1bf62510681375bc40ea4815a27b6aa4e38172dde01882"
    ):
        raise RuntimeError("AVIF ICC mdcv fixture differs from its pinned hash")
    icc_mdcv_path = d / "icc_mdcv.avif"
    icc_mdcv_path.write_bytes(icc_mdcv_still)
    with Image.open(icc_mdcv_path) as image:
        image.load()
        if (
            image.size != icc_image.size
            or image.mode != "RGB"
            or image.tobytes() != icc_profile_pixels
            or image.info.get("icc_profile") != icc_profile
        ):
            raise RuntimeError("Pillow AVIF mdcv fixture differs from the ICC control")

    icc_mdcv_trailing_still = add_avif_mdcv_trailing_byte(icc_mdcv_still)
    if hashlib.sha256(icc_mdcv_trailing_still).hexdigest() != (
        "956465627904603da6df5c6ce0419cb401156dff57cf2a5ab28a332009d0e946"
    ):
        raise RuntimeError("AVIF mdcv trailing-byte fixture differs from its pinned hash")
    icc_mdcv_trailing_path = d / "icc_mdcv_trailing.avif"
    icc_mdcv_trailing_path.write_bytes(icc_mdcv_trailing_still)
    with Image.open(icc_mdcv_trailing_path) as image:
        image.load()
        if (
            image.size != icc_image.size
            or image.mode != "RGB"
            or image.tobytes() != icc_profile_pixels
            or image.info.get("icc_profile") != icc_profile
        ):
            raise RuntimeError(
                "Pillow trailing-byte AVIF mdcv differs from the ICC control"
            )

    icc_property_prefix = b"colrprof"
    if icc_profile_still.count(icc_property_prefix) != 1:
        raise RuntimeError("AVIF ICC fixture does not have one prof property")
    icc_ricc_still = icc_profile_still.replace(
        icc_property_prefix,
        b"colrrICC",
        1,
    )
    if hashlib.sha256(icc_ricc_still).hexdigest() != (
        "36e07c9ec8afd80be79396196b3fec7397caec1cec3bc5f89e0064d98a87ea9b"
    ):
        raise RuntimeError("AVIF rICC fixture differs from its pinned hash")
    icc_ricc_path = d / "icc_ricc_type.avif"
    icc_ricc_path.write_bytes(icc_ricc_still)
    with Image.open(icc_ricc_path) as image:
        image.load()
        if (
            image.size != icc_image.size
            or image.mode != "RGB"
            or image.tobytes() != icc_profile_pixels
            or image.info.get("icc_profile") != icc_profile
        ):
            raise RuntimeError("Pillow rICC fixture differs from the prof control")

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
    operating_point_sequence = rewrite_avif_operating_point_sequence(animated_source)
    operating_point_path = d / "animated_opidc_0x101.avif"
    operating_point_path.write_bytes(operating_point_sequence)

    def pillow_rgb_frames(path):
        with Image.open(path) as image:
            frames = []
            for frame_index in range(image.n_frames):
                image.seek(frame_index)
                frames.append(image.convert("RGB").tobytes())
            return frames

    if pillow_rgb_frames(animated_source) != pillow_rgb_frames(operating_point_path):
        raise RuntimeError("AV1 operating-point variant changed Pillow frame pixels")

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
    animated_duplicate_stsz = avif_duplicate_stsz_before_invalid_stco(
        animated_bytes
    )
    if animated_duplicate_stsz.count(b"stsz") != 2:
        raise RuntimeError("duplicate-stsz AVIF must contain two sample-size boxes")
    if hashlib.sha256(animated_duplicate_stsz).hexdigest() != (
        "99400ac1612db40de4af63b07e7511483381590689be3cc0ed1607df63e44959"
    ):
        raise RuntimeError("animated duplicate-stsz fixture differs from its pinned hash")
    (d / "animated_duplicate_stsz.avif").write_bytes(animated_duplicate_stsz)
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
        return normalize_sequence_timestamps(
            output.getvalue(), "error-resilient AVIF"
        )

    error_resilient_animation = encode_error_resilient_animation()
    if error_resilient_animation != encode_error_resilient_animation():
        raise RuntimeError("error-resilient AVIF fixture is not deterministic")
    if hashlib.sha256(error_resilient_animation).hexdigest() != (
        "06ea9771f8b46c3432c6c6cdf324f1c05e86a5fdccd774c8e3c9a8fce0b831f0"
    ):
        raise RuntimeError("error-resilient AVIF fixture differs from its pinned hash")
    error_resilient_path = d / "animated_error_resilient.avif"
    error_resilient_path.write_bytes(error_resilient_animation)

    from inspect_av1_obus import inspect as inspect_av1_obus

    error_resilient_report = inspect_av1_obus(error_resilient_path)
    primary_sample = next(
        (
            sample
            for sample in error_resilient_report["samples"]
            if sample["role"] == "item_color"
        ),
        None,
    )
    if primary_sample is None:
        raise RuntimeError("error-resilient AVIF has no primary color sample")
    sequence_obu = next(
        (
            obu
            for obu in primary_sample["obus"]
            if obu["type"] == 1
        ),
        None,
    )
    if sequence_obu is None:
        raise RuntimeError("error-resilient AVIF has no sequence-header OBU")
    sequence_payload_spans = sequence_obu["payload_spans"]
    sequence_header = sequence_obu["sequence_header"]
    if (
        len(sequence_payload_spans) != 1
        or sequence_header["reduced_still_picture_header"]
        or sequence_header["timing"] is not None
        or not sequence_header["frame_id_numbers_present"]
    ):
        raise RuntimeError("error-resilient AVIF sequence-header layout changed")
    sequence_payload_offset = sequence_payload_spans[0]["offset"]
    sequence_payload_length = sequence_payload_spans[0]["length"]
    sequence_payload = bytes.fromhex(sequence_header["payload_hex"])
    if (
        len(sequence_payload) != sequence_payload_length
        or error_resilient_animation[
            sequence_payload_offset : sequence_payload_offset + sequence_payload_length
        ]
        != sequence_payload
    ):
        raise RuntimeError("error-resilient AVIF sequence-header payload moved")

    def mutate_sequence_header_bits(field_updates):
        mutated = bytearray(error_resilient_animation)
        payload_bits = sequence_payload_length * 8
        for start, width, value in field_updates:
            if (
                width <= 0
                or start < 0
                or start + width > payload_bits
                or value < 0
                or value >= 1 << width
            ):
                raise RuntimeError("AV1 sequence-header bit mutation is out of range")
            for index in range(width):
                bit_position = start + index
                mask = 1 << (7 - bit_position % 8)
                replacement = (value >> (width - index - 1)) & 1
                byte_position = sequence_payload_offset + bit_position // 8
                mutated[byte_position] = (
                    mutated[byte_position] & ~mask
                ) | (replacement * mask)
        return bytes(mutated)

    zero_sequence_timing = mutate_sequence_header_bits(
        [(5, 1, 1), (6, 32, 0)]
    )
    if hashlib.sha256(zero_sequence_timing).hexdigest() != (
        "055dea40a69ca9eb7a60a499524a7e563990bccb708f6a9992c1691f304f15e2"
    ):
        raise RuntimeError("zero-rate AV1 sequence-timing mutation differs")
    (d / "error_sequence_zero_timing_rate.avif").write_bytes(
        zero_sequence_timing
    )

    zero_sequence_time_scale = mutate_sequence_header_bits(
        [(5, 1, 1), (6, 32, 1), (38, 32, 0)]
    )
    if hashlib.sha256(zero_sequence_time_scale).hexdigest() != (
        "92d7882e439adf1430d60460d488d0df2b7feb9f6ba3bb7931898c4f70f25ac0"
    ):
        raise RuntimeError("zero AV1 sequence time-scale mutation differs")
    (d / "error_sequence_zero_time_scale.avif").write_bytes(
        zero_sequence_time_scale
    )

    inconsistent_operating_point = mutate_sequence_header_bits([(12, 12, 1)])
    if hashlib.sha256(inconsistent_operating_point).hexdigest() != (
        "3c81896058305429697219be55ddcaf4fcf5293aca48570c1e7e22f6ebcd0a07"
    ):
        raise RuntimeError("inconsistent AV1 operating-point IDC mutation differs")
    (d / "error_inconsistent_operating_point_idc.avif").write_bytes(
        inconsistent_operating_point
    )

    inconsistent_operating_point_low_byte = mutate_sequence_header_bits(
        [(12, 12, 0x100)]
    )
    if hashlib.sha256(inconsistent_operating_point_low_byte).hexdigest() != (
        "d49e5403f421c1f71140c7442ba4493e51317bd1c9776d0aaf19069e9002fd71"
    ):
        raise RuntimeError("low-byte AV1 operating-point IDC mutation differs")
    (d / "error_inconsistent_operating_point_idc_low_byte.avif").write_bytes(
        inconsistent_operating_point_low_byte
    )

    def encode_lossless_inter_animation():
        """Encode a small inter leaf that selects the specialized I420 path."""
        from PIL import ImageDraw

        frames = []
        for frame_index in range(2):
            image = Image.new("RGB", (16, 16), (20, 80, 140))
            ImageDraw.Draw(image).rectangle(
                (4 + frame_index, 4, 7 + frame_index, 7), fill=(220, 35, 90)
            )
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=8,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "lossless inter AVIF"
        )

    lossless_inter_animation = encode_lossless_inter_animation()
    if lossless_inter_animation != encode_lossless_inter_animation():
        raise RuntimeError("lossless-inter AVIF fixture is not deterministic")
    (d / "animated_lossless_inter_420_b16x16.avif").write_bytes(
        lossless_inter_animation
    )

    def encode_lossless_inter_b8x16_animation():
        """Encode a rectangular 8x16 lossless I420 inter block."""
        from PIL import ImageDraw

        frames = []
        for frame_index in range(2):
            image = Image.new("RGB", (8, 16), (20, 80, 140))
            ImageDraw.Draw(image).rectangle(
                (2 + frame_index, 6, 5 + frame_index, 9), fill=(220, 35, 90)
            )
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=8,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "8x16 lossless inter AVIF"
        )

    lossless_inter_b8x16 = encode_lossless_inter_b8x16_animation()
    if lossless_inter_b8x16 != encode_lossless_inter_b8x16_animation():
        raise RuntimeError("8x16 lossless-inter AVIF fixture is not deterministic")
    if hashlib.sha256(lossless_inter_b8x16).hexdigest() != (
        "9e3fbac5e42c61413ec8c1f7f4f72bc1338fb7cfd21587e30826855732e369c0"
    ):
        raise RuntimeError("8x16 lossless-inter AVIF fixture differs from its pinned hash")
    (d / "animated_lossless_inter_420_b8x16.avif").write_bytes(
        lossless_inter_b8x16
    )

    def encode_odd_dimensions_inter_boundary_animation():
        """Encode odd 4:2:0 edges with an inter-intra neighbor boundary."""
        from PIL import ImageDraw

        frames = []
        for x0, y0 in ((11, 11), (10, 10)):
            image = Image.new("RGB", (17, 17), (20, 80, 140))
            ImageDraw.Draw(image).rectangle(
                (x0, y0, min(x0 + 5, 16), min(y0 + 5, 16)),
                fill=(220, 35, 90),
            )
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=8,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-warped-motion": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "odd-dimensions inter-boundary AVIF"
        )

    odd_dimensions_inter_boundary = encode_odd_dimensions_inter_boundary_animation()
    if odd_dimensions_inter_boundary != encode_odd_dimensions_inter_boundary_animation():
        raise RuntimeError("odd-dimensions inter-boundary AVIF is not deterministic")
    if hashlib.sha256(odd_dimensions_inter_boundary).hexdigest() != (
        "736914091e00473af577a12de9910ce6abe2b038c84d2f580c5f528b0618c422"
    ):
        raise RuntimeError("odd-dimensions inter-boundary AVIF differs from its pinned hash")
    (d / "animated_odd_dimensions_inter_intra_boundary_420.avif").write_bytes(
        odd_dimensions_inter_boundary
    )

    def encode_lossless_inter_420_b32x32_animation():
        """Encode a large color lossless inter leaf for the supported TX grid."""
        from PIL import ImageDraw

        frames = []
        for frame_index in range(2):
            image = Image.new("RGB", (32, 32), (20, 80, 140))
            ImageDraw.Draw(image).rectangle(
                (8 + frame_index, 8, 11 + frame_index, 11), fill=(220, 35, 90)
            )
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=8,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-warped-motion": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "large lossless color inter AVIF"
        )

    lossless_inter_420_b32x32_animation = (
        encode_lossless_inter_420_b32x32_animation()
    )
    if lossless_inter_420_b32x32_animation != (
        encode_lossless_inter_420_b32x32_animation()
    ):
        raise RuntimeError("large lossless-inter AVIF fixture is not deterministic")
    if hashlib.sha256(lossless_inter_420_b32x32_animation).hexdigest() != (
        "65e8617044f7f12db081f65276235296f8dc208a006451210ebb9603f27f20a7"
    ):
        raise RuntimeError("large lossless-inter AVIF fixture differs from its pinned hash")
    (d / "animated_lossless_inter_420_b32x32.avif").write_bytes(
        lossless_inter_420_b32x32_animation
    )

    def encode_lossless_inter_420_clipped_b32_animation():
        """Encode a 17x17 lossless sequence with an MI-clipped B32 edge grid."""
        from PIL import ImageDraw

        frames = []
        for frame_index in range(2):
            image = Image.new("RGB", (17, 17), (20, 80, 140))
            draw = ImageDraw.Draw(image)
            draw.rectangle(
                (
                    8 + frame_index,
                    8 + frame_index,
                    11 + frame_index,
                    11 + frame_index,
                ),
                fill=(220, 35, 90),
            )
            if frame_index:
                # Add a distinct sample at the clipped bottom-right edge.
                draw.point((16, 16), fill=(220, 35, 90))
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=8,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-warped-motion": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "clipped B32 lossless inter AVIF"
        )

    lossless_inter_420_clipped_b32_animation = (
        encode_lossless_inter_420_clipped_b32_animation()
    )
    if lossless_inter_420_clipped_b32_animation != (
        encode_lossless_inter_420_clipped_b32_animation()
    ):
        raise RuntimeError("clipped B32 lossless-inter AVIF is not deterministic")
    generated_sha256 = hashlib.sha256(lossless_inter_420_clipped_b32_animation).hexdigest()
    if generated_sha256 != (
        "b29bedb6d7486dae5bed2abb74986a65e47bc737bfd26f85b1f69fa8bd666819"
    ):
        raise RuntimeError(
            "clipped B32 lossless-inter AVIF differs from its pinned hash; "
            f"generated SHA-256: {generated_sha256}"
        )
    (d / "animated_lossless_inter_420_clipped_b32x32_17x17.avif").write_bytes(
        lossless_inter_420_clipped_b32_animation
    )

    def encode_lossless_inter_i444_clipped_b32x32_49x64_animation():
        """Encode a clipped right-edge B32 lossless inter grid in 4:4:4."""
        frames = []
        for shift_left in (False, True):
            pixels = bytearray()
            for y in range(64):
                for x in range(49):
                    source_x = max(0, x - 1) if shift_left else x
                    pixels.extend(
                        (
                            (source_x * 73 + y * 17 + source_x * y * 3) & 255,
                            (source_x * 19 + y * 61 + source_x * y * 7) & 255,
                            (source_x * 31 + y * 43 + source_x * y * 13) & 255,
                        )
                    )
            frames.append(Image.frombytes("RGB", (49, 64), bytes(pixels)))

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=0,
            max_threads=1,
            subsampling="4:4:4",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-warped-motion": "0",
                "min-partition-size": "32",
                "max-partition-size": "32",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "49x64 clipped I444 B32 lossless inter AVIF"
        )

    lossless_inter_i444_clipped_b32x32_49x64 = (
        encode_lossless_inter_i444_clipped_b32x32_49x64_animation()
    )
    if lossless_inter_i444_clipped_b32x32_49x64 != (
        encode_lossless_inter_i444_clipped_b32x32_49x64_animation()
    ):
        raise RuntimeError("49x64 clipped I444 B32 lossless-inter AVIF is not deterministic")
    if hashlib.sha256(lossless_inter_i444_clipped_b32x32_49x64).hexdigest() != (
        "d45defceb497272879b1093f76e6a6c645f0bde8a6d10a1903e2322af422caae"
    ):
        raise RuntimeError("49x64 clipped I444 B32 lossless-inter AVIF differs from its pinned hash")
    (d / "animated_lossless_inter_i444_clipped_b32x32_49x64.avif").write_bytes(
        lossless_inter_i444_clipped_b32x32_49x64
    )

    def encode_lossless_inter_i422_clipped_b32x32_49x64_animation():
        """Encode an odd-width clipped B32 lossless inter grid in 4:2:2."""
        from PIL import ImageDraw

        frames = []
        for patch in (False, True):
            image = Image.new("RGB", (49, 64), (20, 80, 140))
            if patch:
                ImageDraw.Draw(image).rectangle(
                    (32, 40, 47, 55), fill=(220, 35, 90)
                )
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=0,
            max_threads=1,
            subsampling="4:2:2",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-warped-motion": "0",
                "min-partition-size": "32",
                "max-partition-size": "32",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "49x64 odd-width clipped I422 B32 lossless inter AVIF"
        )

    lossless_inter_i422_clipped_b32x32_49x64 = (
        encode_lossless_inter_i422_clipped_b32x32_49x64_animation()
    )
    if lossless_inter_i422_clipped_b32x32_49x64 != (
        encode_lossless_inter_i422_clipped_b32x32_49x64_animation()
    ):
        raise RuntimeError("49x64 clipped I422 B32 lossless-inter AVIF is not deterministic")
    if hashlib.sha256(lossless_inter_i422_clipped_b32x32_49x64).hexdigest() != (
        "fd18cdcef915327fd4aab6992c901e27f2efc1411d1d2d01eee93dd3f277d7b1"
    ):
        raise RuntimeError("49x64 clipped I422 B32 lossless-inter AVIF differs from its pinned hash")
    (d / "animated_lossless_inter_i422_clipped_b32x32_49x64.avif").write_bytes(
        lossless_inter_i422_clipped_b32x32_49x64
    )

    def encode_lossless_inter_i422_clipped_b32x32_52x64_animation():
        """Encode a clipped right-edge B32 lossless inter grid in 4:2:2."""
        from PIL import ImageDraw

        frames = []
        for patch in (False, True):
            image = Image.new("RGB", (52, 64), (20, 80, 140))
            if patch:
                ImageDraw.Draw(image).rectangle(
                    (36, 40, 51, 55), fill=(220, 35, 90)
                )
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=0,
            max_threads=1,
            subsampling="4:2:2",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-warped-motion": "0",
                "min-partition-size": "32",
                "max-partition-size": "32",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "52x64 clipped I422 B32 lossless inter AVIF"
        )

    lossless_inter_i422_clipped_b32x32_52x64 = (
        encode_lossless_inter_i422_clipped_b32x32_52x64_animation()
    )
    if lossless_inter_i422_clipped_b32x32_52x64 != (
        encode_lossless_inter_i422_clipped_b32x32_52x64_animation()
    ):
        raise RuntimeError("52x64 clipped I422 B32 lossless-inter AVIF is not deterministic")
    if hashlib.sha256(lossless_inter_i422_clipped_b32x32_52x64).hexdigest() != (
        "43bbcd3d92593e026d88ce4dea727074f74134175ee42a962886c6de697a4e05"
    ):
        raise RuntimeError("52x64 clipped I422 B32 lossless-inter AVIF differs from its pinned hash")
    (d / "animated_lossless_inter_i422_clipped_b32x32_52x64.avif").write_bytes(
        lossless_inter_i422_clipped_b32x32_52x64
    )

    def encode_lossless_inter_i444_clipped_b32x32_64x56_animation():
        """Encode a bottom-clipped B32 lossless inter grid in 4:4:4."""
        frames = []
        for shift_up in (False, True):
            pixels = bytearray()
            for y in range(56):
                if y < 32:
                    pixels.extend(bytes((20, 80, 140)) * 64)
                    continue
                source_y = max(0, y - 33) if shift_up else y - 32
                for x in range(64):
                    pixels.extend(
                        (
                            (x * 73 + source_y * 17 + x * source_y * 3) & 255,
                            (x * 19 + source_y * 61 + x * source_y * 7) & 255,
                            (x * 31 + source_y * 43 + x * source_y * 13) & 255,
                        )
                    )
            frames.append(Image.frombytes("RGB", (64, 56), bytes(pixels)))

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=0,
            max_threads=1,
            subsampling="4:4:4",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-warped-motion": "0",
                "min-partition-size": "32",
                "max-partition-size": "32",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "64x56 bottom-clipped I444 B32 lossless inter AVIF"
        )

    lossless_inter_i444_clipped_b32x32_64x56 = (
        encode_lossless_inter_i444_clipped_b32x32_64x56_animation()
    )
    if lossless_inter_i444_clipped_b32x32_64x56 != (
        encode_lossless_inter_i444_clipped_b32x32_64x56_animation()
    ):
        raise RuntimeError("64x56 clipped I444 B32 lossless-inter AVIF is not deterministic")
    if hashlib.sha256(lossless_inter_i444_clipped_b32x32_64x56).hexdigest() != (
        "9004ec840b2a413c0e15294f52c723ebe5c3689cbe3decbd4afd5575cad78e23"
    ):
        raise RuntimeError("64x56 clipped I444 B32 lossless-inter AVIF differs from its pinned hash")
    (d / "animated_lossless_inter_i444_clipped_b32x32_64x56.avif").write_bytes(
        lossless_inter_i444_clipped_b32x32_64x56
    )

    dual_edge_frames = []
    for shift_up in (False, True):
        pixels = bytearray((20, 80, 140) * (52 * 60))
        for y in range(32, 60):
            source_y = max(0, y - 33) if shift_up else y - 32
            for x in range(32, 52):
                offset = (y * 52 + x) * 3
                pixels[offset : offset + 3] = bytes(
                    (
                        (73 * x + 17 * source_y + 3 * x * source_y) & 255,
                        (19 * x + 61 * source_y + 7 * x * source_y) & 255,
                        (31 * x + 43 * source_y + 13 * x * source_y) & 255,
                    )
                )
        dual_edge_frames.append(Image.frombytes("RGB", (52, 60), bytes(pixels)))

    def encode_lossless_inter_dual_edge_b32x32_52x60(subsampling):
        """Encode a simultaneously right- and bottom-clipped B32 grid."""
        output = BytesIO()
        dual_edge_frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=dual_edge_frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=0,
            max_threads=1,
            subsampling=subsampling,
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-warped-motion": "0",
                "min-partition-size": "32",
                "max-partition-size": "32",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), f"52x60 dual-edge {subsampling} B32 lossless inter AVIF"
        )

    for chroma_name, subsampling, expected_hash in (
        (
            "i422",
            "4:2:2",
            "53e06d79c586151d920b94cc6173eebcc1f9869ebb0f1fa5bec673df11c45e50",
        ),
        (
            "i444",
            "4:4:4",
            "9fdd099eb28fed9eaff2760b154e42d91933758fd6b01e532e441646f564993a",
        ),
    ):
        fixture_name = f"animated_lossless_inter_{chroma_name}_clipped_b32x32_52x60.avif"
        encoded = encode_lossless_inter_dual_edge_b32x32_52x60(subsampling)
        if encoded != encode_lossless_inter_dual_edge_b32x32_52x60(subsampling):
            raise RuntimeError(f"{fixture_name} is not deterministic")
        if hashlib.sha256(encoded).hexdigest() != expected_hash:
            raise RuntimeError(f"{fixture_name} differs from its pinned hash")
        (d / fixture_name).write_bytes(encoded)

    def encode_lossless_inter_420_clipped_b32x32_60x64_animation():
        """Encode a partial 28x32 lossless I420 inter grid at the right edge."""
        from PIL import ImageDraw

        frames = []
        for x0 in (32, 31):
            image = Image.new("RGB", (60, 64), (20, 80, 140))
            ImageDraw.Draw(image).rectangle(
                (x0, 16, x0 + 27, 47), fill=(220, 35, 90)
            )
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=8,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-warped-motion": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "60x64 clipped B32 lossless inter AVIF"
        )

    lossless_inter_420_clipped_b32x32_60x64 = (
        encode_lossless_inter_420_clipped_b32x32_60x64_animation()
    )
    if lossless_inter_420_clipped_b32x32_60x64 != (
        encode_lossless_inter_420_clipped_b32x32_60x64_animation()
    ):
        raise RuntimeError("60x64 clipped B32 lossless-inter AVIF is not deterministic")
    if hashlib.sha256(lossless_inter_420_clipped_b32x32_60x64).hexdigest() != (
        "a60dde64c6b7bf9f6fbb3afe7e75a74374d446f8106613b00837300df4963bbc"
    ):
        raise RuntimeError("60x64 clipped B32 lossless-inter AVIF differs from its pinned hash")
    (d / "animated_lossless_inter_420_clipped_b32x32_60x64.avif").write_bytes(
        lossless_inter_420_clipped_b32x32_60x64
    )

    def encode_lossless_inter_420_clipped_b32x32_64x60_animation():
        """Encode a lossless I420 sequence with a translated bottom-edge patch."""
        from PIL import ImageDraw

        frames = []
        for y0 in (32, 31):
            image = Image.new("RGB", (64, 60), (20, 80, 140))
            ImageDraw.Draw(image).rectangle(
                (16, y0, 47, y0 + 27), fill=(220, 35, 90)
            )
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=8,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-warped-motion": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "64x60 clipped B32 lossless inter AVIF"
        )

    lossless_inter_420_clipped_b32x32_64x60 = (
        encode_lossless_inter_420_clipped_b32x32_64x60_animation()
    )
    if lossless_inter_420_clipped_b32x32_64x60 != (
        encode_lossless_inter_420_clipped_b32x32_64x60_animation()
    ):
        raise RuntimeError("64x60 clipped B32 lossless-inter AVIF is not deterministic")
    generated_sha256 = hashlib.sha256(lossless_inter_420_clipped_b32x32_64x60).hexdigest()
    if generated_sha256 != (
        "7602fd062d0aefdaa0221b35a3a6c9403fecc27bf8a66cb00c0b4f9b8763c0d6"
    ):
        raise RuntimeError(
            "64x60 bottom-edge lossless-inter AVIF differs from its pinned hash; "
            f"generated SHA-256: {generated_sha256}"
        )
    (d / "animated_lossless_inter_420_clipped_b32x32_64x60.avif").write_bytes(
        lossless_inter_420_clipped_b32x32_64x60
    )

    def encode_lossless_inter_420_clipped_b32x32_184x64_animation():
        """Encode a clipped B32 lossless edge leaf with a textured source."""
        frames = []
        for frame_index in range(2):
            pixels = bytearray()
            for y in range(64):
                for x in range(184):
                    base = (x * 73 + y * 17 + x * y * 3) & 255
                    red = base
                    green = (x * 19 + y * 61 + x * y * 7) & 255
                    blue = (x * 31 + y * 43 + x * y * 13) & 255
                    if frame_index:
                        delta = (x * 7 + y * 11 + x * y) % 9 - 4
                        red = (red + delta) & 255
                        green = (green - delta) & 255
                        blue = (blue + 2 * delta) & 255
                    pixels.extend((red, green, blue))
            frames.append(Image.frombytes("RGB", (184, 64), bytes(pixels)))

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=0,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-warped-motion": "0",
                "min-partition-size": "32",
                "max-partition-size": "32",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "184x64 clipped B32 lossless inter AVIF"
        )

    lossless_inter_420_clipped_b32x32_184x64 = (
        encode_lossless_inter_420_clipped_b32x32_184x64_animation()
    )
    if lossless_inter_420_clipped_b32x32_184x64 != (
        encode_lossless_inter_420_clipped_b32x32_184x64_animation()
    ):
        raise RuntimeError("184x64 clipped B32 lossless-inter AVIF is not deterministic")
    if hashlib.sha256(lossless_inter_420_clipped_b32x32_184x64).hexdigest() != (
        "0a8c935fe67694fe576e7f21064eec4c428cffc2a05a3ff2d0be33e401a20c1d"
    ):
        raise RuntimeError("184x64 clipped B32 lossless-inter AVIF differs from its pinned hash")
    (d / "animated_lossless_inter_420_clipped_b32x32_184x64.avif").write_bytes(
        lossless_inter_420_clipped_b32x32_184x64
    )

    def encode_lossless_inter_420_clipped_b32x32_185x64_animation():
        """Encode a lossless right-edge B32 grid with a 25-pixel extent."""
        frames = []
        for frame_index in range(2):
            pixels = bytearray()
            for y in range(64):
                for x in range(185):
                    base = (x * 73 + y * 17 + x * y * 3) & 255
                    delta = (x * 7 + y * 11 + x * y) % 9 - 4 if frame_index else 0
                    pixels.extend(
                        (
                            (base + delta) & 255,
                            (x * 19 + y * 61 + x * y * 7 - delta) & 255,
                            (x * 31 + y * 43 + x * y * 13 + 2 * delta) & 255,
                        )
                    )
            frames.append(Image.frombytes("RGB", (185, 64), bytes(pixels)))

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=0,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-warped-motion": "0",
                "min-partition-size": "32",
                "max-partition-size": "32",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "185x64 clipped B32 lossless inter AVIF"
        )

    lossless_inter_420_clipped_b32x32_185x64 = (
        encode_lossless_inter_420_clipped_b32x32_185x64_animation()
    )
    if lossless_inter_420_clipped_b32x32_185x64 != (
        encode_lossless_inter_420_clipped_b32x32_185x64_animation()
    ):
        raise RuntimeError("185x64 clipped B32 lossless-inter AVIF is not deterministic")
    if hashlib.sha256(lossless_inter_420_clipped_b32x32_185x64).hexdigest() != (
        "701d247ebbe2666b1f28f447fa1a086573e7dc6d88b0b29405f9f7a6f9460b59"
    ):
        raise RuntimeError("185x64 clipped B32 lossless-inter AVIF differs from its pinned hash")
    (d / "animated_lossless_inter_420_clipped_b32x32_185x64.avif").write_bytes(
        lossless_inter_420_clipped_b32x32_185x64
    )

    def encode_lossless_inter_420_clipped_b16x16_60x64_animation():
        """Encode a shifted texture with a partial B16 lossless inter block."""
        pixels = bytearray()
        for y in range(64):
            for x in range(60):
                pixels.extend(
                    (
                        (x * 73 + y * 17 + x * y * 3) & 255,
                        (x * 19 + y * 61 + x * y * 7) & 255,
                        (x * 31 + y * 43 + x * y * 13) & 255,
                    )
                )
        first = Image.frombytes("RGB", (60, 64), bytes(pixels))
        shifted = bytearray(len(pixels))
        for y in range(64):
            for x in range(60):
                source_x = max(0, x - 1)
                destination = (y * 60 + x) * 3
                source = (y * 60 + source_x) * 3
                shifted[destination : destination + 3] = pixels[
                    source : source + 3
                ]
        second = Image.frombytes("RGB", (60, 64), bytes(shifted))

        output = BytesIO()
        first.save(
            output,
            format="AVIF",
            save_all=True,
            append_images=[second],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=0,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-warped-motion": "0",
                "min-partition-size": "16",
                "max-partition-size": "16",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "60x64 clipped B16 lossless inter AVIF"
        )

    lossless_inter_420_clipped_b16x16_60x64 = (
        encode_lossless_inter_420_clipped_b16x16_60x64_animation()
    )
    if lossless_inter_420_clipped_b16x16_60x64 != (
        encode_lossless_inter_420_clipped_b16x16_60x64_animation()
    ):
        raise RuntimeError("60x64 clipped B16 lossless-inter AVIF is not deterministic")
    if hashlib.sha256(lossless_inter_420_clipped_b16x16_60x64).hexdigest() != (
        "7c8bd0f88cd6654fb1150d2220ecb33c6ec5874b26a2250909b51cab4ed2e17f"
    ):
        raise RuntimeError("60x64 clipped B16 lossless-inter AVIF differs from its pinned hash")
    (d / "animated_lossless_inter_420_clipped_b16x16_60x64.avif").write_bytes(
        lossless_inter_420_clipped_b16x16_60x64
    )

    def encode_lossless_inter_monochrome_animation():
        """Encode a monochrome lossless inter leaf with a translating patch."""
        from PIL import ImageDraw

        frames = []
        for frame_index in range(2):
            image = Image.new("L", (16, 16), 96)
            ImageDraw.Draw(image).rectangle(
                (4 + frame_index, 4, 7 + frame_index, 7), fill=224
            )
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=100,
            speed=8,
            max_threads=1,
            subsampling="4:0:0",
            autotiling=False,
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-warped-motion": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "lossless monochrome inter AVIF"
        )

    lossless_inter_monochrome_animation = (
        encode_lossless_inter_monochrome_animation()
    )
    if lossless_inter_monochrome_animation != (
        encode_lossless_inter_monochrome_animation()
    ):
        raise RuntimeError("lossless monochrome AVIF fixture is not deterministic")
    (d / "animated_lossless_inter_monochrome_b16x16.avif").write_bytes(
        lossless_inter_monochrome_animation
    )

    def encode_spatial_neighbor_monochrome(enable_intrabc):
        """Encode a monochrome lossless repeated-patch frame with selectable IntraBC."""
        rng = random.Random(0x1BC20261001)
        patch = bytes(rng.randrange(256) for _ in range(16 * 8))
        pixels = bytearray([128]) * (256 * 256)
        for row in range(8):
            patch_row = patch[row * 16 : (row + 1) * 16]
            source_start = (64 + row) * 256
            destination_start = (128 + row) * 256 + 128
            pixels[source_start : source_start + 16] = patch_row
            pixels[destination_start : destination_start + 16] = patch_row

        image = Image.frombytes("L", (256, 256), bytes(pixels))
        output = BytesIO()
        image.save(
            output,
            format="AVIF",
            quality=100,
            speed=4,
            max_threads=1,
            autotiling=False,
            subsampling="4:0:0",
            advanced={
                "enable-intrabc": "1" if enable_intrabc else "0",
                "enable-palette": "0",
                "min-partition-size": "8",
                "max-partition-size": "8",
                "enable-cdef": "0",
                "enable-restoration": "0",
                "enable-tx64": "0",
                "force-video-mode": "0",
                "tune-content": "screen",
            },
        )
        return output.getvalue()

    intrabc_spatial_neighbor = encode_spatial_neighbor_monochrome(
        enable_intrabc=True
    )
    if intrabc_spatial_neighbor != encode_spatial_neighbor_monochrome(
        enable_intrabc=True
    ):
        raise RuntimeError("monochrome IntraBC spatial-neighbor AVIF is not deterministic")
    if hashlib.sha256(intrabc_spatial_neighbor).hexdigest() != (
        "c77c448e3668e90a4de1228c8b44a54fd47caae8e7538e314a1597dcc961795f"
    ):
        raise RuntimeError("monochrome IntraBC spatial-neighbor AVIF differs from its pinned hash")
    (d / "intrabc_spatial_neighbor_monochrome_256.avif").write_bytes(
        intrabc_spatial_neighbor
    )

    no_intrabc_repeated_patch = encode_spatial_neighbor_monochrome(
        enable_intrabc=False
    )
    if no_intrabc_repeated_patch != encode_spatial_neighbor_monochrome(
        enable_intrabc=False
    ):
        raise RuntimeError("monochrome no-IntraBC AVIF is not deterministic")
    if hashlib.sha256(no_intrabc_repeated_patch).hexdigest() != (
        "34321df1afb253b459f20169ddc4d3b588e517b4832c2c2460d6900ca647c7a3"
    ):
        raise RuntimeError("monochrome no-IntraBC AVIF differs from its pinned hash")
    (d / "no_intrabc_repeated_patch_monochrome_256.avif").write_bytes(
        no_intrabc_repeated_patch
    )

    def encode_lossy_inter_420_b16x16_split_animation():
        """Encode a lossy 4:2:0 inter leaf with a single B16 transform split."""
        base_rng = random.Random(491)
        base = [
            [base_rng.randrange(256) for _ in range(16)]
            for _ in range(16)
        ]
        frames = []
        for phase in range(3):
            pixels = bytearray()
            for y in range(16):
                for x in range(16):
                    value = base[y][(x - phase) % 16]
                    delta = (
                        0
                        if phase == 0
                        else 19
                        if (x + y + phase) & 1
                        else -15
                    )
                    pixels.extend(
                        (
                            max(0, min(255, value + delta)),
                            max(0, min(255, (value * 3 + 29 + delta) // 4)),
                            max(0, min(255, (value * 5 + 13 - delta) // 6)),
                        )
                    )
            frames.append(Image.frombytes("RGB", (16, 16), bytes(pixels)))

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=75,
            speed=0,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            advanced={
                "min-partition-size": "16",
                "max-partition-size": "16",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "lossy inter-transform AVIF"
        )

    lossy_inter_420_b16x16_split_animation = (
        encode_lossy_inter_420_b16x16_split_animation()
    )
    if lossy_inter_420_b16x16_split_animation != (
        encode_lossy_inter_420_b16x16_split_animation()
    ):
        raise RuntimeError("lossy split-inter AVIF fixture is not deterministic")
    if hashlib.sha256(lossy_inter_420_b16x16_split_animation).hexdigest() != (
        "9eb50f5a45dc2eb549c62ac9bcb67abd931a977076685bcf3c0f69e6827ce323"
    ):
        raise RuntimeError("lossy split-inter AVIF fixture differs from its pinned hash")
    (d / "animated_lossy_split_inter_420_b16x16.avif").write_bytes(
        lossy_inter_420_b16x16_split_animation
    )

    def encode_lossy_wide_monochrome_b128x128_animation():
        """Encode 128x128 monochrome inter blocks with switchable transforms."""
        width = height = 128
        base_rng = random.Random(7411)
        base = bytes(96 + base_rng.randrange(-16, 17) for _ in range(width * height))
        frames = [Image.frombytes("L", (width, height), base)]

        for patch_x, patch_y in ((0, 0), (96, 0), (0, 96)):
            pixels = bytearray(base)
            for y in range(patch_y, patch_y + 32):
                for x in range(patch_x, patch_x + 32):
                    dx = x - patch_x
                    dy = y - patch_y
                    pixels[y * width + x] = 205 + ((dx * 7 + dy * 11) % 36)
            frames.append(Image.frombytes("L", (width, height), bytes(pixels)))

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=0,
            speed=0,
            max_threads=1,
            subsampling="4:0:0",
            autotiling=False,
            advanced={
                "sb-size": "128",
                "min-partition-size": "128",
                "max-partition-size": "128",
                "aq-mode": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "lossy wide monochrome AVIF"
        )

    lossy_wide_monochrome_b128x128_animation = (
        encode_lossy_wide_monochrome_b128x128_animation()
    )
    if lossy_wide_monochrome_b128x128_animation != (
        encode_lossy_wide_monochrome_b128x128_animation()
    ):
        raise RuntimeError("lossy wide monochrome AVIF fixture is not deterministic")
    if hashlib.sha256(lossy_wide_monochrome_b128x128_animation).hexdigest() != (
        "eb8dda5000882ffd03c182a817e08fc944ef110afa1e78aaf1678bef5efc65bf"
    ):
        raise RuntimeError("lossy wide monochrome AVIF fixture differs from its pinned hash")
    (d / "animated_lossy_wide_monochrome_b128x128.avif").write_bytes(
        lossy_wide_monochrome_b128x128_animation
    )

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

    def encode_motion_chroma_animation():
        from PIL import ImageDraw

        frames = []
        for frame_index in range(4):
            image = Image.new("RGB", (32, 32), (32, 64, 96))
            draw = ImageDraw.Draw(image)
            patch_x = 4 + frame_index
            draw.rectangle((patch_x, 8, patch_x + 7, 15), fill=(220, 40, 72))
            draw.rectangle((patch_x + 2, 10, patch_x + 5, 13), fill=(245, 210, 35))
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=80,
            speed=6,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "motion/chroma AVIF"
        )

    motion_chroma_animation = encode_motion_chroma_animation()
    if motion_chroma_animation != encode_motion_chroma_animation():
        raise RuntimeError("motion/chroma AVIF fixture is not deterministic")
    (d / "animated_motion_chroma.avif").write_bytes(motion_chroma_animation)

    def encode_motion_chroma_422_animation():
        from PIL import ImageDraw

        frames = []
        for frame_index in range(4):
            image = Image.new("RGB", (16, 16), (32, 64, 96))
            draw = ImageDraw.Draw(image)
            patch_x = 3 + frame_index * 2
            draw.rectangle((patch_x, 4, patch_x + 3, 11), fill=(220, 40, 72))
            draw.rectangle((patch_x + 1, 6, patch_x + 2, 9), fill=(245, 210, 35))
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=76,
            speed=0,
            max_threads=1,
            subsampling="4:2:2",
            autotiling=False,
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "motion/chroma 4:2:2 AVIF"
        )

    motion_chroma_422_animation = encode_motion_chroma_422_animation()
    if motion_chroma_422_animation != encode_motion_chroma_422_animation():
        raise RuntimeError("motion/chroma 4:2:2 AVIF fixture is not deterministic")
    (d / "animated_motion_chroma_422.avif").write_bytes(motion_chroma_422_animation)

    def encode_lossy_inter_422_checker_random_b32x32_animation():
        """Build a jittered checker sequence for lossy 4:2:2 inter decoding."""
        frames = []
        for frame_index in range(2):
            rng = random.Random(490 + frame_index)
            pixels = bytearray()
            for y in range(32):
                for x in range(32):
                    value = 220 if ((x // 8 + y // 8) & 1) else 30
                    value += rng.randint(-8, 8)
                    if frame_index and (x - 2) % 32 < 4 and 8 <= y < 24:
                        value += 80 if (x + y) & 1 else -55
                    value = max(0, min(255, value))
                    pixels.extend((value, value, value))
            frames.append(Image.frombytes("RGB", (32, 32), bytes(pixels)))

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100, 100],
            loop=0,
            quality=75,
            speed=0,
            max_threads=1,
            subsampling="4:2:2",
            autotiling=False,
            codec="aom",
            advanced={
                "min-partition-size": "16",
                "max-partition-size": "32",
                "aq-mode": "0",
                "deltaq-mode": "0",
                "enable-cdef": "0",
                "enable-restoration": "0",
                "loopfilter-control": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "lossy 4:2:2 jittered-checker AVIF"
        )

    lossy_inter_422_checker_random_b32x32 = (
        encode_lossy_inter_422_checker_random_b32x32_animation()
    )
    if lossy_inter_422_checker_random_b32x32 != (
        encode_lossy_inter_422_checker_random_b32x32_animation()
    ):
        raise RuntimeError("lossy 4:2:2 jittered-checker AVIF fixture is not deterministic")
    if hashlib.sha256(lossy_inter_422_checker_random_b32x32).hexdigest() != (
        "e46aef4aed9076da48627a449e53c6f318a0494ba5d94a32a4f13edc548ab134"
    ):
        raise RuntimeError("lossy 4:2:2 jittered-checker AVIF fixture differs from its pinned hash")
    (d / "animated_lossy_inter_422_checker_random_b32x32.avif").write_bytes(
        lossy_inter_422_checker_random_b32x32
    )

    def encode_wide_motion_chroma_animation():
        size = (64, 64)
        frames = []
        for frame_index in range(4):
            pixels = bytearray()
            for y in range(size[1]):
                for x in range(size[0]):
                    pattern = ((x + frame_index * 4) // 4 + y // 4) % 4
                    pixels.extend(
                        (
                            (24, 220, 72, 180)[pattern],
                            (60, 40, 210, 30)[pattern],
                            (96, 72, 40, 220)[pattern],
                        )
                    )
            frames.append(Image.frombytes("RGB", size, bytes(pixels)))

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=80,
            speed=6,
            max_threads=1,
            subsampling="4:4:4",
            autotiling=False,
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "wide motion/chroma AVIF"
        )

    wide_motion_chroma_animation = encode_wide_motion_chroma_animation()
    if wide_motion_chroma_animation != encode_wide_motion_chroma_animation():
        raise RuntimeError("wide motion/chroma AVIF fixture is not deterministic")
    (d / "animated_motion_chroma_wide.avif").write_bytes(
        wide_motion_chroma_animation
    )

    def encode_tx64_root_split_animation():
        """Select a TX64 inter root split with two TX32 child decisions."""
        base = Image.new("RGB", (64, 64))
        pixels = base.load()
        for y in range(64):
            for x in range(64):
                value = 96 + ((7 * x + 11 * y) % 32)
                pixels[x, y] = (value, value, value)
        frames = [base.copy() for _ in range(3)]
        for frame_index, (x0, y0, x1, y1), even, odd in (
            (1, (16, 16, 48, 48), (240, 237, 183), (16, 77, 87)),
            (2, (8, 8, 56, 56), (235, 164, 224), (20, 113, 91)),
        ):
            target = frames[frame_index].load()
            for y in range(y0, y1):
                for x in range(x0, x1):
                    target[x, y] = even if (x + y) % 2 == 0 else odd

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=80,
            speed=0,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            codec="aom",
            advanced={
                "min-partition-size": "64",
                "max-partition-size": "64",
                "aq-mode": "0",
                "deltaq-mode": "0",
                "enable-cdef": "0",
                "enable-restoration": "0",
                "loopfilter-control": "0",
                "enable-global-motion": "0",
                "enable-warped-motion": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "TX64 root-split AVIF"
        )

    tx64_root_split_animation = encode_tx64_root_split_animation()
    if tx64_root_split_animation != encode_tx64_root_split_animation():
        raise RuntimeError("TX64 root-split AVIF fixture is not deterministic")
    if hashlib.sha256(tx64_root_split_animation).hexdigest() != (
        "9f4450d4d9c7c2738d4f9f34eafb02100c5fb15b85ac121a057c3eef367f6d7c"
    ):
        raise RuntimeError("TX64 root-split AVIF fixture differs from its pinned hash")
    (d / "animated_tx64_root_split_inter_420_64x64.avif").write_bytes(
        tx64_root_split_animation
    )

    def encode_projected_motion_window_animation(positions, fixture_name):
        """Move a textured patch across opposite projected x-window edges."""
        frames = []
        for patch_x in positions:
            image = Image.new("RGB", (512, 128), (16, 20, 24))
            pixels = image.load()
            for y in range(64):
                for x in range(96):
                    stripe = ((x // 8) ^ (y // 8)) & 1
                    mild = (x * 11 + y * 7 + (x * y) % 13) % 24
                    pixels[patch_x + x, y + 32] = (
                        72 + stripe * 112 + mild,
                        28 + stripe * 76 + mild // 2,
                        40 + stripe * 96 + mild // 3,
                    )
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=95,
            speed=0,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            codec="aom",
            advanced={
                "aq-mode": "0",
                "deltaq-mode": "0",
                "enable-cdef": "0",
                "enable-restoration": "0",
                "loopfilter-control": "0",
                "enable-warped-motion": "0",
                "enable-global-motion": "0",
            },
        )
        return normalize_sequence_timestamps(output.getvalue(), fixture_name)

    for fixture_name, positions, filename, expected_hash in (
        (
            "projected motion x-window left AVIF",
            (192, 32, 192, 32),
            "animated_motion_temporal_window_left_512x128.avif",
            "29e856c6c8a117c764bb0a176ccc1d82bac982fdf7287eb461f827ebdc31b399",
        ),
        (
            "projected motion x-window right AVIF",
            (32, 192, 32, 192),
            "animated_motion_temporal_window_right_512x128.avif",
            "10d8e5514c1d8d9cad355f9a912b3e3075bf8bd661ee6af5ce44a9ce62d7cad2",
        ),
    ):
        animation = encode_projected_motion_window_animation(positions, fixture_name)
        if animation != encode_projected_motion_window_animation(positions, fixture_name):
            raise RuntimeError(f"{fixture_name} fixture is not deterministic")
        if hashlib.sha256(animation).hexdigest() != expected_hash:
            raise RuntimeError(f"{fixture_name} fixture differs from its pinned hash")
        (d / filename).write_bytes(animation)

    def encode_lossy_i444_mode2_inter_animation():
        """Exercise lossy I444 inter parsing with transform mode two."""
        frames = []
        for patch_x in (0, 13, 27):
            image = Image.new("RGB", (32, 32), (32, 64, 96))
            ImageDraw.Draw(image).rectangle(
                (patch_x, 8, patch_x + 3, 11), fill=(220, 40, 180)
            )
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=80,
            speed=0,
            max_threads=1,
            subsampling="4:4:4",
            autotiling=False,
            codec="aom",
            advanced={
                "aq-mode": "0",
                "deltaq-mode": "0",
                "enable-cdef": "0",
                "enable-restoration": "0",
                "loopfilter-control": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "lossy I444 mode-2 inter AVIF"
        )

    lossy_i444_mode2_inter_animation = encode_lossy_i444_mode2_inter_animation()
    if lossy_i444_mode2_inter_animation != encode_lossy_i444_mode2_inter_animation():
        raise RuntimeError("lossy I444 mode-2 inter AVIF fixture is not deterministic")
    if hashlib.sha256(lossy_i444_mode2_inter_animation).hexdigest() != (
        "43aac6364eebb113b128861f0e1c2cfb976295d7b5e9e226fa8f0eaa93704af4"
    ):
        raise RuntimeError("lossy I444 mode-2 inter AVIF fixture differs from its pinned hash")
    (d / "animated_lossy_inter_i444_mode2_b32x32.avif").write_bytes(
        lossy_i444_mode2_inter_animation
    )

    def encode_lossy_interintra_420_nowedge_animation():
        """Select a non-wedge B16x16 inter-intra blend with exact parity."""
        frames = [Image.new("RGB", (64, 64), (200, 200, 200)) for _ in range(2)]
        final = Image.new("RGB", (64, 64), (0, 0, 0))
        ImageDraw.Draw(final).rectangle((16, 0, 31, 15), fill=(100, 100, 100))
        frames.append(final)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=50,
            speed=0,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            codec="aom",
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-interintra-comp": "1",
                "enable-interintra-wedge": "0",
                "enable-smooth-interintra": "1",
                "enable-cdef": "0",
                "enable-restoration": "0",
                "enable-warped-motion": "0",
                "min-partition-size": "16",
                "max-partition-size": "16",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "lossy I420 no-wedge inter-intra AVIF"
        )

    lossy_interintra_420_nowedge = encode_lossy_interintra_420_nowedge_animation()
    if (
        lossy_interintra_420_nowedge
        != encode_lossy_interintra_420_nowedge_animation()
    ):
        raise RuntimeError("lossy no-wedge inter-intra AVIF is not deterministic")
    if hashlib.sha256(lossy_interintra_420_nowedge).hexdigest() != (
        "adc58d72626865406be3d0a048ecf0b9a1670d16a68f48babe4b9167a2dc2dea"
    ):
        raise RuntimeError("lossy no-wedge inter-intra AVIF differs from its pinned hash")
    (d / "animated_lossy_interintra_420_nowedge_b16x16_64x64.avif").write_bytes(
        lossy_interintra_420_nowedge
    )

    def encode_lossy_interintra_420_nowedge_stripes_animation(direction):
        """Select a non-wedge B16x16 blend with striped intra neighbors."""
        frames = [Image.new("RGB", (64, 64), (200, 200, 200)) for _ in range(2)]
        final = Image.new("RGB", (64, 64), (0, 0, 0))
        draw = ImageDraw.Draw(final)
        for stripe in range(4):
            value = 224 if stripe % 2 else 32
            if direction == "horizontal":
                bounds = (16, 16 + stripe * 4, 31, 19 + stripe * 4)
            elif direction == "vertical":
                bounds = (16 + stripe * 4, 16, 19 + stripe * 4, 31)
            else:
                raise ValueError(f"unsupported inter-intra stripe direction: {direction}")
            draw.rectangle(bounds, fill=(value, value, value))
        frames.append(final)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=50,
            speed=0,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            codec="aom",
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-interintra-comp": "1",
                "enable-interintra-wedge": "0",
                "enable-smooth-interintra": "1",
                "enable-cdef": "0",
                "enable-restoration": "0",
                "enable-warped-motion": "0",
                "min-partition-size": "16",
                "max-partition-size": "16",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), f"lossy I420 no-wedge inter-intra {direction} stripes AVIF"
        )

    lossy_interintra_420_nowedge_mode1 = (
        encode_lossy_interintra_420_nowedge_stripes_animation("horizontal")
    )
    if lossy_interintra_420_nowedge_mode1 != encode_lossy_interintra_420_nowedge_stripes_animation(
        "horizontal"
    ):
        raise RuntimeError("lossy mode-1 no-wedge inter-intra AVIF is not deterministic")
    if hashlib.sha256(lossy_interintra_420_nowedge_mode1).hexdigest() != (
        "dd4e06e559d83cc0863d65878799d2f7496c8f5d378eaeb639704fe3ac539f2f"
    ):
        raise RuntimeError("lossy mode-1 no-wedge inter-intra AVIF differs from its pinned hash")
    (d / "animated_lossy_interintra_420_nowedge_mode1_b16x16_64x64.avif").write_bytes(
        lossy_interintra_420_nowedge_mode1
    )

    lossy_interintra_420_nowedge_mode2 = (
        encode_lossy_interintra_420_nowedge_stripes_animation("vertical")
    )
    if lossy_interintra_420_nowedge_mode2 != encode_lossy_interintra_420_nowedge_stripes_animation(
        "vertical"
    ):
        raise RuntimeError("lossy mode-2 no-wedge inter-intra AVIF is not deterministic")
    if hashlib.sha256(lossy_interintra_420_nowedge_mode2).hexdigest() != (
        "08f616c2e14f9abadb6157c425653b5a7fd703f2797f3db04e23b58bc519eda8"
    ):
        raise RuntimeError("lossy mode-2 no-wedge inter-intra AVIF differs from its pinned hash")
    (d / "animated_lossy_interintra_420_nowedge_mode2_b16x16_64x64.avif").write_bytes(
        lossy_interintra_420_nowedge_mode2
    )

    def encode_lossy_interintra_420_nowedge_mode3_animation():
        """Select the non-wedge B16x16 inter-intra mask's mode-3 path."""
        frames = [Image.new("RGB", (64, 64), (200, 200, 200)) for _ in range(2)]
        final = Image.new("RGB", (64, 64), (0, 0, 0))
        weights = [60, 45, 34, 26, 19, 15, 11, 8, 6, 5, 4, 3, 2, 2, 1, 1]
        for y in range(16):
            for x in range(16):
                weight = weights[min(x, y)]
                value = (200 * (64 - weight) + 32) // 64
                final.putpixel((16 + x, 16 + y), (value, value, value))
        frames.append(final)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=50,
            speed=0,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            codec="aom",
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-interintra-comp": "1",
                "enable-interintra-wedge": "0",
                "enable-smooth-interintra": "1",
                "enable-cdef": "0",
                "enable-restoration": "0",
                "enable-warped-motion": "0",
                "min-partition-size": "16",
                "max-partition-size": "16",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "lossy I420 no-wedge mode-3 inter-intra AVIF"
        )

    lossy_interintra_420_nowedge_mode3 = (
        encode_lossy_interintra_420_nowedge_mode3_animation()
    )
    if lossy_interintra_420_nowedge_mode3 != (
        encode_lossy_interintra_420_nowedge_mode3_animation()
    ):
        raise RuntimeError("lossy mode-3 no-wedge inter-intra AVIF is not deterministic")
    if hashlib.sha256(lossy_interintra_420_nowedge_mode3).hexdigest() != (
        "7e035cef7293dfff72e50ff8c729bc0fad360ee14d7bf80d663b82b96ca7c4d7"
    ):
        raise RuntimeError("lossy mode-3 no-wedge inter-intra AVIF differs from its pinned hash")
    (d / "animated_lossy_interintra_420_nowedge_mode3_b16x16_64x64.avif").write_bytes(
        lossy_interintra_420_nowedge_mode3
    )

    def encode_lossy_interintra_420_wedge_animation():
        """Select a B16x16 wedge and expose its 4:2:0 chroma mask reduction."""
        reference_color = (20, 80, 220)
        frames = [Image.new("RGB", (64, 64), reference_color) for _ in range(2)]
        final = Image.new("RGB", (64, 64), (0, 0, 0))
        wedge_vertical = [0, 2, 7, 21, 43, 57, 62, 64] + [64] * 8
        for y in range(16):
            mask = wedge_vertical[y]
            value = tuple((channel * mask + 32) // 64 for channel in reference_color)
            for x in range(16):
                final.putpixel((16 + x, 16 + y), value)
        frames.append(final)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=50,
            speed=0,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            codec="aom",
            advanced={
                "color-primaries": "1",
                "transfer-characteristics": "13",
                "matrix-coefficients": "6",
                "enable-interintra-comp": "1",
                "enable-interintra-wedge": "1",
                "enable-smooth-interintra": "1",
                "enable-cdef": "0",
                "enable-restoration": "0",
                "enable-warped-motion": "0",
                "min-partition-size": "16",
                "max-partition-size": "16",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "lossy I420 wedge inter-intra AVIF"
        )

    lossy_interintra_420_wedge = encode_lossy_interintra_420_wedge_animation()
    if lossy_interintra_420_wedge != encode_lossy_interintra_420_wedge_animation():
        raise RuntimeError("lossy wedge inter-intra AVIF is not deterministic")
    if hashlib.sha256(lossy_interintra_420_wedge).hexdigest() != (
        "1510b764d7e642f2caf02fff53e6dc966cfb92df8eca2e8f2d91c0b3bead1791"
    ):
        raise RuntimeError("lossy wedge inter-intra AVIF differs from its pinned hash")
    (d / "animated_lossy_interintra_420_wedge_b16x16_64x64.avif").write_bytes(
        lossy_interintra_420_wedge
    )

    def encode_lossy_i444_split_inter_animation():
        """Select a B16x32 mode-2 inter split with exact Pillow parity."""
        frames = []
        for frame_index in range(2):
            random_source = random.Random(490 + frame_index)
            pixels = bytearray()
            for y in range(32):
                for x in range(32):
                    value = 220 if ((x // 8 + y // 8) & 1) else 30
                    value += random_source.randint(-8, 8)
                    if frame_index and (x - 2) % 32 < 4 and 8 <= y < 24:
                        value += 80 if (x + y) & 1 else -55
                    value = max(0, min(255, value))
                    pixels.extend((value, value, value))
            frames.append(Image.frombytes("RGB", (32, 32), bytes(pixels)))

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=75,
            speed=0,
            max_threads=1,
            subsampling="4:4:4",
            autotiling=False,
            codec="aom",
            advanced={
                "min-partition-size": "16",
                "max-partition-size": "32",
                "aq-mode": "0",
                "deltaq-mode": "0",
                "enable-cdef": "0",
                "enable-restoration": "0",
                "loopfilter-control": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "lossy I444 split-plan AVIF"
        )

    lossy_i444_split_inter_animation = encode_lossy_i444_split_inter_animation()
    if (
        lossy_i444_split_inter_animation
        != encode_lossy_i444_split_inter_animation()
    ):
        raise RuntimeError("lossy I444 split-plan AVIF fixture is not deterministic")
    if hashlib.sha256(lossy_i444_split_inter_animation).hexdigest() != (
        "ad7ce564a11440b91237e0dfffedbde1053bc663ae0e9bf0894f87e05e669bd0"
    ):
        raise RuntimeError("lossy I444 split-plan AVIF fixture differs from its pinned hash")
    (d / "animated_lossy_inter_i444_split_b16x32_mode2.avif").write_bytes(
        lossy_i444_split_inter_animation
    )

    def encode_large_motion_420_animation():
        """Encode large inter leaves for the motion size-gate parity row."""
        width = height = 256
        random_source = random.Random(7411)
        base = bytearray()
        for _ in range(width * height):
            value = 96 + random_source.randrange(-16, 17)
            base.extend((value + 18, value, value - 18))

        frames = [bytes(base)]
        for patch_index in range(3):
            frame = bytearray(base)
            patch_x = (patch_index % 2) * 128
            patch_y = (patch_index // 2) * 128
            for y in range(patch_y, patch_y + 32):
                for x in range(patch_x, patch_x + 32):
                    dx = x - patch_x
                    dy = y - patch_y
                    value = 205 + ((dx * 7 + dy * 11) % 36)
                    offset = (y * width + x) * 3
                    frame[offset : offset + 3] = bytes(
                        (value, min(255, value + 12), max(0, value - 16))
                    )
            frames.append(bytes(frame))

        images = [Image.frombytes("RGB", (width, height), frame) for frame in frames]
        output = BytesIO()
        images[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=images[1:],
            duration=[100] * len(images),
            loop=0,
            quality=0,
            speed=0,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            advanced={
                "sb-size": "128",
                "min-partition-size": "128",
                "max-partition-size": "128",
                "aq-mode": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "large-inter 4:2:0 AVIF"
        )

    large_motion_420_animation = encode_large_motion_420_animation()
    if large_motion_420_animation != encode_large_motion_420_animation():
        raise RuntimeError("large-inter AVIF fixture is not deterministic")
    if hashlib.sha256(large_motion_420_animation).hexdigest() != (
        "5b9ea2b9d552e8ff44f2818a7ae2b73a9dbe1eda84ead9c24c9f8a957c6e546f"
    ):
        raise RuntimeError("large-inter AVIF fixture differs from its pinned hash")
    (d / "animated_motion_large_420.avif").write_bytes(
        large_motion_420_animation
    )

    def encode_large_motion_i444_animation():
        """Encode unsplit 128x128 I444 inter leaves with local motion."""
        width = height = 256
        palette = (
            (35, 65, 205),
            (225, 54, 32),
            (28, 192, 83),
            (178, 136, 24),
        )
        tile = bytearray()
        for y in range(64):
            for x in range(64):
                color = palette[(x // 8 + 3 * (y // 8)) & 3]
                grain = ((x * 13 + y * 7) % 17) - 8
                tile.extend(max(0, min(255, channel + grain)) for channel in color)

        base = bytearray(width * height * 3)
        for y in range(height):
            for x in range(width):
                source = ((y % 64) * 64 + (x % 64)) * 3
                destination = (y * width + x) * 3
                base[destination : destination + 3] = tile[source : source + 3]

        images = []
        for marker_x in (12, 28):
            frame = bytearray(base)
            for block_y in (0, 128):
                for block_x in (0, 128):
                    for y in range(block_y + 12, block_y + 44):
                        for x in range(block_x + marker_x, block_x + marker_x + 32):
                            offset = (y * width + x) * 3
                            frame[offset : offset + 3] = bytes((244, 230, 24))
            images.append(Image.frombytes("RGB", (width, height), bytes(frame)))

        output = BytesIO()
        images[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=images[1:],
            duration=[100, 100],
            loop=0,
            quality=80,
            speed=0,
            max_threads=1,
            subsampling="4:4:4",
            autotiling=False,
            codec="aom",
            advanced={
                "sb-size": "128",
                "min-partition-size": "128",
                "max-partition-size": "128",
                "aq-mode": "0",
                "deltaq-mode": "0",
                "enable-cdef": "0",
                "enable-restoration": "0",
                "loopfilter-control": "0",
                "enable-warped-motion": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "wide lossy I444 AVIF"
        )

    large_motion_i444_animation = encode_large_motion_i444_animation()
    if large_motion_i444_animation != encode_large_motion_i444_animation():
        raise RuntimeError("wide I444 AVIF fixture is not deterministic")
    if hashlib.sha256(large_motion_i444_animation).hexdigest() != (
        "b4693fd2fb43d1c9406fe4ac8d782d74709bf8eed1b2d36771c0bd80763d84e8"
    ):
        raise RuntimeError("wide I444 AVIF fixture differs from its pinned hash")
    (d / "animated_lossy_wide_i444_b128x128.avif").write_bytes(
        large_motion_i444_animation
    )

    def encode_lossy_i444_mode2_unsplit_wide_animation():
        """Encode a 128px I444 inter leaf with an unsplit mode-2 transform."""
        width = height = 256
        base = bytearray(width * height * 3)
        for y in range(height):
            for x in range(width):
                color = (42 + x // 8, 58 + y // 8, 112 + (x + y) // 16)
                offset = (y * width + x) * 3
                base[offset : offset + 3] = bytes(color)

        images = []
        for marker_x in (24, 48):
            frame = bytearray(base)
            for block_y in (0, 128):
                for block_x in (0, 128):
                    for y in range(block_y + 32, block_y + 80):
                        for x in range(block_x + marker_x, block_x + marker_x + 32):
                            offset = (y * width + x) * 3
                            frame[offset : offset + 3] = bytes((210, 48, 34))
            images.append(Image.frombytes("RGB", (width, height), bytes(frame)))

        output = BytesIO()
        images[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=images[1:],
            duration=[100, 100],
            loop=0,
            quality=80,
            speed=0,
            max_threads=1,
            subsampling="4:4:4",
            autotiling=False,
            codec="aom",
            advanced={
                "sb-size": "128",
                "min-partition-size": "128",
                "max-partition-size": "128",
                "aq-mode": "0",
                "deltaq-mode": "0",
                "enable-cdef": "0",
                "enable-restoration": "0",
                "loopfilter-control": "0",
                "enable-warped-motion": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "mode-2 unsplit wide I444 AVIF"
        )

    mode2_unsplit_wide_i444_animation = (
        encode_lossy_i444_mode2_unsplit_wide_animation()
    )
    if mode2_unsplit_wide_i444_animation != (
        encode_lossy_i444_mode2_unsplit_wide_animation()
    ):
        raise RuntimeError("wide mode-2 I444 AVIF fixture is not deterministic")
    if hashlib.sha256(mode2_unsplit_wide_i444_animation).hexdigest() != (
        "ea87cfc19c427135396b44ea6b287b24e5301437abb5bd6769666d9a24144565"
    ):
        raise RuntimeError("wide mode-2 I444 AVIF fixture differs from its pinned hash")
    (d / "animated_lossy_wide_i444_mode2_unsplit_b128x128.avif").write_bytes(
        mode2_unsplit_wide_i444_animation
    )

    def encode_global_halfblend_spatial_animation():
        """Encode an identity/RotZoom compound-neighbor motion witness."""
        width = height = 256
        fill = (19, 33, 47)

        base = Image.new("RGB", (width, height), fill)
        draw = ImageDraw.Draw(base)
        source_rng = random.Random(29)
        for _ in range(200):
            x = source_rng.randrange(width)
            y = source_rng.randrange(height)
            rectangle_width = source_rng.randrange(8, 64)
            rectangle_height = source_rng.randrange(8, 64)
            color = tuple(source_rng.randrange(256) for _ in range(3))
            draw.rectangle(
                (
                    x,
                    y,
                    x + rectangle_width - 1,
                    y + rectangle_height - 1,
                ),
                fill=color,
            )

        def rotate(image, degrees):
            return image.rotate(
                -degrees,
                resample=Image.Resampling.BICUBIC,
                expand=False,
                fillcolor=fill,
            )

        first_delta = rotate(base, 1)
        second_delta = rotate(base, 3)
        patch_rng = random.Random(1029)
        patch_draw = ImageDraw.Draw(second_delta)
        for _ in range(8):
            x = patch_rng.randrange(width)
            y = patch_rng.randrange(height)
            rectangle_width = patch_rng.randrange(8, 48)
            rectangle_height = patch_rng.randrange(8, 48)
            color = tuple(patch_rng.randrange(256) for _ in range(3))
            patch_draw.rectangle(
                (
                    x,
                    y,
                    x + rectangle_width - 1,
                    y + rectangle_height - 1,
                ),
                fill=color,
            )

        aligned = second_delta.rotate(
            2,
            resample=Image.Resampling.BICUBIC,
            expand=False,
            fillcolor=fill,
        )
        frames = [
            base,
            first_delta,
            second_delta,
            Image.blend(first_delta, aligned, 0.5),
        ]

        def encode():
            output = BytesIO()
            frames[0].save(
                output,
                format="AVIF",
                save_all=True,
                append_images=frames[1:],
                duration=[100] * len(frames),
                loop=0,
                quality=80,
                speed=0,
                max_threads=1,
                subsampling="4:4:4",
                autotiling=False,
                codec="aom",
                advanced={
                    "sb-size": "128",
                    "min-partition-size": "128",
                    "max-partition-size": "128",
                    "aq-mode": "0",
                    "deltaq-mode": "0",
                    "enable-cdef": "0",
                    "enable-restoration": "0",
                    "loopfilter-control": "0",
                    "enable-warped-motion": "0",
                    "enable-global-motion": "1",
                },
            )
            return normalize_sequence_timestamps(
                output.getvalue(), "global identity/RotZoom AVIF"
            )

        return encode()

    global_halfblend_animation = encode_global_halfblend_spatial_animation()
    if global_halfblend_animation != encode_global_halfblend_spatial_animation():
        raise RuntimeError("global identity/RotZoom AVIF fixture is not deterministic")
    if hashlib.sha256(global_halfblend_animation).hexdigest() != (
        "ec2f145f64fa87c5b7c255b13b6009fb58760be06070724ed367cd4ccd9a6d7b"
    ):
        raise RuntimeError("global identity/RotZoom AVIF fixture differs from its pinned hash")
    (d / "animated_lossy_global_halfblend_spatial_i444_b256x256.avif").write_bytes(
        global_halfblend_animation
    )
    from inspect_av1_obus import inspect as inspect_av1

    global_headers = [
        obu["frame_header"]
        for sample in inspect_av1(
            d / "animated_lossy_global_halfblend_spatial_i444_b256x256.avif"
        )["samples"]
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    final_global_header = [
        header for header in global_headers if header["order_hint"] == 3
    ]
    if (
        len(final_global_header) != 1
        or final_global_header[0]["reference_mode"] != "select"
        or final_global_header[0]["reference_indices"] != [1, 0, 0, 2, 0, 0, 0]
        or [
            final_global_header[0]["global_motion"][index]["type"]
            for index in (0, 3)
        ]
        != ["identity", "rotzoom"]
    ):
        raise RuntimeError("global identity/RotZoom AVIF header topology differs")

    def encode_compound_reference_context2_error():
        """Corrupt a late AVIF sample after a compound-reference prefix."""
        width = height = 128

        def texture_pixel(x, y):
            return (
                (x * 19 + y * 7 + x * y * 3) % 256,
                (x * 5 + y * 23 + x * y * 11) % 256,
                (x * 13 + y * 17 + x * y * 5) % 256,
            )

        background = Image.new("RGB", (width, height))
        background.putdata(
            [texture_pixel(x, y) for y in range(height) for x in range(width)]
        )
        frames = []
        for frame_index in range(8):
            frame = background.copy()
            draw = ImageDraw.Draw(frame)
            draw.rectangle(
                (6 + frame_index * 14, 18, 31 + frame_index * 14, 104),
                fill=(242, 42, 35),
            )
            draw.ellipse(
                (96 - frame_index * 11, 36, 117 - frame_index * 11, 57),
                fill=(10, 34, 244),
            )
            frames.append(frame)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=45,
            speed=4,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
        )
        encoded = normalize_sequence_timestamps(
            output.getvalue(), "compound-reference context AVIF"
        )
        if hashlib.sha256(encoded).hexdigest() != (
            "8824303d7f3e0f23362fa5aa56b345936562c34055051422437b2be72fba1943"
        ):
            raise RuntimeError("compound-reference AVIF differs from its pinned hash")
        if len(encoded) != 16109 or encoded[16098] != 0x91:
            raise RuntimeError("compound-reference AVIF mutation point moved")

        corrupted = bytearray(encoded)
        corrupted[16098] ^= 0x80
        corrupted_bytes = bytes(corrupted)
        if hashlib.sha256(corrupted_bytes).hexdigest() != (
            "c6e290b4a79c02c03205b279aa14946875f516d67e4d530e7c69dbace635852a"
        ):
            raise RuntimeError("compound-reference AVIF tail mutation differs")
        return corrupted_bytes

    compound_reference_context2_error = encode_compound_reference_context2_error()
    if (
        compound_reference_context2_error
        != encode_compound_reference_context2_error()
    ):
        raise RuntimeError("compound-reference AVIF error fixture is not deterministic")
    (d / "animated_compound_reference_context2_tail_error.avif").write_bytes(
        compound_reference_context2_error
    )

    def encode_has_chroma_4x4_animation():
        frames = []
        for frame_index in range(3):
            image = Image.new("RGB", (64, 64), (32, 64, 96))
            if frame_index > 0:
                ImageDraw.Draw(image).rectangle(
                    (8, 8, 11, 11),
                    fill=(250, 24, 210),
                )
            frames.append(image)

        output = BytesIO()
        frames[0].save(
            output,
            format="AVIF",
            save_all=True,
            append_images=frames[1:],
            duration=[100] * len(frames),
            loop=0,
            quality=80,
            speed=0,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
            advanced={
                "min-partition-size": "4",
                "max-partition-size": "4",
                "aq-mode": "0",
                "deltaq-mode": "0",
                "enable-cdef": "0",
                "enable-restoration": "0",
                "loopfilter-control": "0",
            },
        )
        return normalize_sequence_timestamps(
            output.getvalue(), "4x4 chroma-ownership AVIF"
        )

    has_chroma_4x4_animation = encode_has_chroma_4x4_animation()
    if has_chroma_4x4_animation != encode_has_chroma_4x4_animation():
        raise RuntimeError("4x4 chroma-ownership AVIF fixture is not deterministic")
    (d / "animated_has_chroma_4x4.avif").write_bytes(
        has_chroma_4x4_animation
    )

    def write_portable_image(
        name,
        image,
        quality=100,
        speed=8,
        subsampling="4:4:4",
        advanced=None,
        codec=None,
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
            if codec is not None:
                options["codec"] = codec
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

    def subsampled422_vertical_halves():
        """Generate the minimal lossy 4:2:2 origin reconstruction witness."""

        left = (17, 91, 203)
        right = (0, 255, 0)
        return image_from_pixels(
            (16, 16),
            lambda x, _y: left if x < 8 else right,
        )

    write_campaign_image(
        "coverage_422_square16_vertical_halves_01",
        subsampled422_vertical_halves(),
        "4:2:2",
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

    def square32_origin_tx16x16_split_noise():
        """Generate the origin Square32/TX16x16 split reconstruction witness."""

        random_state = random.Random(32001)
        pixels = bytes(
            random_state.randrange(256) for _ in range(32 * 32 * 3)
        )
        return Image.frombytes("RGB", (32, 32), pixels)

    write_campaign_image(
        "coverage_square32_origin_tx16x16_split_01",
        square32_origin_tx16x16_split_noise(),
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

    def square64_origin_tx32x32_split_noise():
        """Generate the origin Square64/TX32x32 split reconstruction witness."""

        random_state = random.Random(64000)
        pixels = bytes(
            random_state.randrange(256) for _ in range(64 * 64 * 3)
        )
        return Image.frombytes("RGB", (64, 64), pixels)

    write_campaign_image(
        "coverage_square64_origin_tx32x32_split_01",
        square64_origin_tx32x32_split_noise(),
        "4:2:0",
        advanced={
            "min-partition-size": "64",
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

    def horizontal16x8_origin_dct_dct():
        """Generate the promoted origin Horizontal16x8 DCT-DCT witness.

        This is h16x8-f01-n01 from the pinned input-only rectangular-transform
        campaign. Keep the pixel algebra and encoder controls identical to the
        campaign so the committed fixture remains independently reproducible.
        """

        amplitude = 32

        def pixel(x, y):
            value = 128 + (amplitude if (x + y + 1) % 2 else -amplitude)
            return (value, value, value)

        return image_from_pixels((16, 8), pixel)

    write_campaign_image(
        "coverage_h16x8_origin_dct_dct_01",
        horizontal16x8_origin_dct_dct(),
        "4:2:0",
        advanced={
            "min-partition-size": "8",
            "max-partition-size": "16",
            "use-intra-dct-only": "0",
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
        quality=44,
        speed=0,
    )

    def horizontal16x8_following_dct_dct():
        """Generate the following Horizontal16x8 DCT-DCT witness.

        This is h16x8-following-f01-n00 from the pinned input-only campaign:
        a 32x8 grayscale-as-RGB checker signal whose right 16x8 leaf
        exercises the completed left leaf's reconstructed edge.
        """

        amplitude = 24

        def pixel(x, y):
            value = 128 + amplitude * (1 if x % 2 else -1)
            return (value, value, value)

        return image_from_pixels((32, 8), pixel)

    write_campaign_image(
        "coverage_h16x8_following_dct_dct_01",
        horizontal16x8_following_dct_dct(),
        "4:2:0",
        advanced={
            "min-partition-size": "8",
            "max-partition-size": "16",
            "use-intra-dct-only": "0",
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
        quality=36,
        speed=0,
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

    def horizontal16x4_predictor_adst_dct():
        """Generate the selected predictor-enabled H16x4 witness.

        This is h16x4-f10-n00 from the bounded input-only campaign: the
        first Horizontal16x4 luma leaf selects CDF symbol 5 / dav1d txtp 1.
        Keep the construction in this maintained generator so the promoted
        fixture is reproducible without copying bytes from the campaign.
        """

        amplitude = 4
        phase = 9
        random_state = random.Random(164900)
        column_prbs = [random_state.choice((-1, 1)) for _ in range(16)]
        row_prbs = [random_state.choice((-1, 1)) for _ in range(4)]
        positive_row = random_state.randrange(4)
        negative_row = (positive_row + 1 + random_state.randrange(3)) % 4
        bases = (48, 104, 160, 216)

        def pixel(x, y):
            band = y // 4
            local_y = y % 4
            if band == 0:
                signal = amplitude * column_prbs[x]
            elif band == 1:
                signal = ((x - local_y + phase) % 16 - 8) * amplitude // 3
            elif band == 2:
                signal = (2 * local_y - 3) * amplitude
            elif local_y == positive_row:
                signal = amplitude
            elif local_y == negative_row:
                signal = -amplitude
            else:
                signal = 0
            value = clamp_channel(bases[band] + signal)
            return (value, value, value)

        return image_from_pixels((16, 16), pixel)

    write_campaign_image(
        "coverage_h16x4_predictor_adst_dct_01",
        horizontal16x4_predictor_adst_dct(),
        "4:2:0",
        advanced={
            "min-partition-size": "4",
            "max-partition-size": "16",
            "use-intra-dct-only": "0",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-directional-intra": "1",
            "enable-smooth-intra": "1",
            "enable-paeth-intra": "1",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=26,
        speed=0,
    )

    def horizontal16x4_predictor_adst_dct_f02_n08():
        """Generate the second independent H16x4 ADST-DCT witness.

        This is ``h16x4-f02-n08`` from the pinned predictor campaign.  Its
        third Horizontal16x4 luma leaf selects ADST-DCT while the other three
        leaves remain DCT-DCT.  The formula is kept here so the promoted
        fixture is regenerated from pixels rather than copied AVIF bytes.
        """

        amplitude = 20
        phase = 9
        bases = (48, 104, 160, 216)

        def pixel(x, y):
            band = y // 4
            local_y = y % 4
            signal = ((2 * x - 15) * amplitude) // 4
            signal += (local_y - 1) * amplitude // 3
            value = clamp_channel(bases[band] + signal)
            return (value, value, value)

        pixels = image_from_pixels((16, 16), pixel)
        expected_sha256 = (
            "cc593e8dcadcc51d2c00347443650283b75d96a137e03399479827b056e9cdfc"
        )
        if hashlib.sha256(pixels.tobytes()).hexdigest() != expected_sha256:
            raise RuntimeError("H16x4 predictor F02/n08 source changed")
        return pixels

    write_campaign_image(
        "coverage_h16x4_predictor_adst_dct_f02_n08",
        horizontal16x4_predictor_adst_dct_f02_n08(),
        "4:2:0",
        advanced={
            "min-partition-size": "4",
            "max-partition-size": "16",
            "use-intra-dct-only": "0",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-directional-intra": "1",
            "enable-smooth-intra": "1",
            "enable-paeth-intra": "1",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=74,
        speed=0,
    )

    def horizontal16x4_h_dct_cfl():
        """Generate the exact bounded H_DCT/CFL witness.

        The source bytes are the pinned RGB8 normalization of the retained
        input-only H16x4 oracle candidate. Keeping the normalized input here
        makes the promoted witness reproducible without copying an encoded
        AVIF bitstream into the asset generator.
        """

        pixels = bytes.fromhex(
            "13aa6991f9ce3a4445c8a4b0654037b17c80d381b186285abd626f3c1400a6dd9c71c47457873f667554bfb5da8c75bc34b08463b79d868a93a57f96b38b96deb3c4a9789d86557292655fe8dbb96a9d6ca9eab48baf8144513f453e5db2a1d8"
            "0038367faab1443a5569406b62366b624973b0c2d0719a8a5e885ab9e3b5668d7a203e3e3d4c535e64706b6c7e3b394e647b9d101e3b817e934d3e5b58457b292556789aa61c5345b9f5d16da08250696e262c448c87a55c596a6a7169b5c2ae"
            "5e4e8f716f96b7d1c4416855244c5e8da5c92a29496c66826d788e9dacbfced7e92e2b40301d316f6160b4c092a0b9774d34767e789e94b3a3447558184a4d748ea56b57788161886e668f3134551c1e2dc8c2c477625de5d5beadb5839cb170"
            "ababcf9498b3111b244a585b3d4c53626470d6c2cd705b6a47455a575f6c969e93babb9d463b0feaddb3a29d87a0a09447565b555e659999a3b1a9b8796c7ec7b2c392777ec7b2b536353da6b2b06d7c652b3b1412200091936b3b2927725567"
            "63755bbfcbb5c3c1b489787e3614376a406650293b5a3f448d84856670688fab95add8ba5e947494af9c86656eecacc49bbb8ac3d5ad7d77616b4e50eabadabc8aafa47f916958629ca4af667d8592aba84c6e654174678eaba7785d6ef4bed8"
            "83b87a9ab789857868ffdde198667185586559444b4d56676da1c72b5b89787ca175697f999da040484b7a7b904f4c6a326022849e71c1afa38e656bf4c4d05f3f4c9799a6355268204f795a7dadc5bae57a587bcdadc2a195a1455d67325d64"
            "778456babe9b7c6b61573742cea6c84331596182a5456b88828193c2adbe3921398b6888662f5660445a618f7aa2f5cb967967624d3ab3b29ecccccc7a6a912d285e5c7da82e4a6749363a6f49466e5457a68c97ccacc3a59ea591c3a0a9f6c0"
            "8f3950704244b9d4b180b599263e5c6772a2afb5d96265787a7675f4e8da957e6a9d977d9fc2a437674783a686bdd8b982103bcd919da2cda179c8a076a2bb233364e1daf9685d6bbfc1be84837169543574774c7bbe8858a571829c7f777a67"
        )
        expected_sha256 = "f3fb754117962b22ac3705b4f18996f1cf6deb1a8728106dfabe65296581dda8"
        if hashlib.sha256(pixels).hexdigest() != expected_sha256:
            raise RuntimeError("H16x4 H_DCT/CFL source normalization changed")
        return Image.frombytes("RGB", (16, 16), pixels)

    write_campaign_image(
        "coverage_h16x4_h_dct_cfl_01",
        horizontal16x4_h_dct_cfl(),
        "4:2:0",
        advanced={
            "min-partition-size": "4",
            "max-partition-size": "16",
            "use-intra-dct-only": "0",
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
        },
        quality=99,
        speed=0,
    )

    def horizontal16x4_following_h_dct():
        """Generate the selected following/right H16x4 H_DCT witness.

        The right 16x16 half is the exact promoted H_DCT control.  Only the
        preceding half is changed, using the F07/n02 edge-bias candidate from
        the maintained input-only campaign.  This makes the four real left
        samples consumed by the following Horizontal predictor observable
        without copying an encoded AVIF bitstream into the asset generator.
        """

        base = horizontal16x4_h_dct_cfl().tobytes()
        left = bytearray(base)
        for y in range(16):
            for x in range(16):
                delta = 5 if x in (0, 7, 15) else -1
                pixel = (y * 16 + x) * 3
                for channel in range(3):
                    left[pixel + channel] = clamp_channel(left[pixel + channel] + delta)
        pixels = bytearray()
        for y in range(16):
            row_start = y * 16 * 3
            pixels.extend(left[row_start : row_start + 16 * 3])
            pixels.extend(base[row_start : row_start + 16 * 3])
        pixels = bytes(pixels)
        expected_sha256 = (
            "9603e5bb354b29354a11123de6e46d80c22810e11e66aa23bda7fbf79eaeb73d"
        )
        if hashlib.sha256(pixels).hexdigest() != expected_sha256:
            raise RuntimeError("following H16x4 H_DCT source normalization changed")
        return Image.frombytes("RGB", (32, 16), pixels)

    write_campaign_image(
        "coverage_h16x4_following_h_dct_01",
        horizontal16x4_following_h_dct(),
        "4:2:0",
        advanced={
            "min-partition-size": "4",
            "max-partition-size": "16",
            "use-intra-dct-only": "0",
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
        },
        quality=99,
        speed=0,
    )

    def vertical4x16_predictor_adst_adst():
        """Generate the selected predictor-enabled V4x16 witness.

        This is v4x16-f07-n07 from the pinned vertical-flip campaign: the second
        Vertical4x16 luma leaf selects CDF symbol 4 / dav1d txtp 3
        (ADST-ADST), while the four terminal leaves use only DC, Horizontal,
        and Paeth prediction. Keep the reflection in this generator so the
        promoted fixture is reproducible without copying campaign bytes.
        """

        amplitude = 18
        phase = 11
        bases = (48, 104, 160, 216)

        def pixel(x, y):
            # The campaign reflects the horizontal predictor corpus vertically:
            # output[x,y] = source[x,15-y].
            source_x = x
            source_y = 15 - y
            band = source_y // 4
            local_y = source_y % 4
            horizontal = ((source_x + phase) % 16) - 8
            vertical = ((local_y + phase) % 4) - 2
            signal = (horizontal * horizontal + vertical * vertical - 24) * amplitude // 8
            value = clamp_channel(bases[band] + signal)
            return (value, value, value)

        return image_from_pixels((16, 16), pixel)

    write_campaign_image(
        "coverage_v4x16_predictor_adst_adst_01",
        vertical4x16_predictor_adst_adst(),
        "4:2:0",
        advanced={
            "min-partition-size": "4",
            "max-partition-size": "16",
            "use-intra-dct-only": "0",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-directional-intra": "1",
            "enable-smooth-intra": "1",
            "enable-paeth-intra": "1",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=68,
        speed=0,
    )

    def horizontal16x4_depth_one_tx8x4():
        """Generate the bounded depth-one TX8x4 H16x4 witness.

        This is h16x4-f10-n06 at quality 24 from the pinned input-only
        dav1d campaign. Its second H16x4 leaf selects two non-skipped TX8x4
        luma children with DCT_DCT residuals. Keep the source construction
        here so the promoted fixture remains reproducible.
        """

        amplitude = 16
        phase = 11
        random_state = random.Random(164906)
        column_prbs = [random_state.choice((-1, 1)) for _ in range(16)]
        row_prbs = [random_state.choice((-1, 1)) for _ in range(4)]
        positive_row = random_state.randrange(4)
        negative_row = (positive_row + 1 + random_state.randrange(3)) % 4
        bases = (48, 104, 160, 216)

        def pixel(x, y):
            band = y // 4
            local_y = y % 4
            if band == 0:
                signal = amplitude * column_prbs[x]
            elif band == 1:
                signal = ((x - local_y + phase) % 16 - 8) * amplitude // 3
            elif band == 2:
                signal = (2 * local_y - 3) * amplitude
            elif local_y == positive_row:
                signal = amplitude
            elif local_y == negative_row:
                signal = -amplitude
            else:
                signal = 0
            value = clamp_channel(bases[band] + signal)
            return (value, value, value)

        return image_from_pixels((16, 16), pixel)

    write_campaign_image(
        "coverage_h16x4_tx8x4_split_01",
        horizontal16x4_depth_one_tx8x4(),
        "4:2:0",
        advanced={
            "min-partition-size": "4",
            "max-partition-size": "16",
            "use-intra-dct-only": "0",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-directional-intra": "1",
            "enable-smooth-intra": "1",
            "enable-paeth-intra": "1",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=24,
        speed=0,
    )

    def horizontal16x4_third_leaf_no_chroma_tx8x4():
        """Generate an H4 with split luma residuals in the third leaf.

        In 4:2:0 PARTITION_H4, chroma belongs to alternating leaves. This
        third H16x4 leaf deliberately selects two non-skipped TX8x4 luma
        transforms, guarding the complete decoder's mixed-chroma leaf state.
        """

        amplitude = 16
        phase = 11
        random_state = random.Random(164906)
        column_prbs = [random_state.choice((-1, 1)) for _ in range(16)]
        positive_row = random_state.randrange(4)
        negative_row = (positive_row + 1 + random_state.randrange(3)) % 4
        bases = (48, 104, 160, 216)

        def pixel(x, y):
            band, local_y = divmod(y, 4)
            if band == 0:
                signal = amplitude * column_prbs[x]
            elif band == 1:
                signal = (2 * local_y - 3) * amplitude
            elif band == 2:
                signal = ((x - local_y + phase) % 16 - 8) * amplitude // 3
            elif local_y == positive_row:
                signal = amplitude
            elif local_y == negative_row:
                signal = -amplitude
            else:
                signal = 0
            value = clamp_channel(bases[band] + signal)
            return (value, value, value)

        return image_from_pixels((16, 16), pixel)

    write_campaign_image(
        "coverage_h16x4_third_leaf_no_chroma_tx8x4_01",
        horizontal16x4_third_leaf_no_chroma_tx8x4(),
        "4:2:0",
        advanced={
            "min-partition-size": "4",
            "max-partition-size": "16",
            "use-intra-dct-only": "0",
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
        quality=24,
        speed=0,
    )
    third_leaf_path = d / "coverage_h16x4_third_leaf_no_chroma_tx8x4_01.avif"
    if hashlib.sha256(third_leaf_path.read_bytes()).hexdigest() != (
        "923d41b784c7da682940a188aca7d82b41f9e52212079bbb8bba0bfee0109a68"
    ):
        raise RuntimeError(
            "H4 third-leaf no-chroma TX8x4 AVIF differs from the traced bitstream"
        )

    def horizontal16x4_filter_intra_depth_one_tx8x4():
        """Generate the following H16x4 FILTER_PRED/TX8x4 witness.

        This is h16x4-f10-n03 at quality 32 from the pinned input-only
        campaign. The fourth horizontal band is deliberately patterned so
        the following Horizontal16x4 leaf selects filter-intra mode 4 and
        transform depth one, with two non-empty TX8x4 luma children.
        """

        amplitude = 12
        candidate_index = 3
        random_state = random.Random(164903)
        column_prbs = [random_state.choice((-1, 1)) for _ in range(16)]
        levels = (-amplitude, -amplitude // 3, amplitude // 3, amplitude)
        bases = (40, 96, 160, 216)

        def pixel(x, y):
            band = y // 4
            local_y = y % 4
            if band < 2:
                signal = amplitude * column_prbs[x]
            else:
                signal = levels[(local_y + candidate_index) % len(levels)]
            value = clamp_channel(bases[band] + signal)
            return (value, value, value)

        return image_from_pixels((16, 16), pixel)

    write_campaign_image(
        "coverage_h16x4_filter_intra_tx8x4_split_01",
        horizontal16x4_filter_intra_depth_one_tx8x4(),
        "4:2:0",
        advanced={
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
        },
        quality=32,
        speed=0,
    )

    def horizontal16x4_split_adst_dct():
        """Generate a depth-two TX4x4 child-grid H16x4 witness.

        This is h16x4-f10-n06 from the pinned predictor campaign. Its second
        H16x4 leaf selects four TX4x4 luma children while the other three
        leaves remain TX16x4. Keep the input construction here so the
        production fixture is reproducible from the oracle corpus.
        """

        amplitude = 16
        phase = 11
        random_state = random.Random(164906)
        column_prbs = [random_state.choice((-1, 1)) for _ in range(16)]
        row_prbs = [random_state.choice((-1, 1)) for _ in range(4)]
        positive_row = random_state.randrange(4)
        negative_row = (positive_row + 1 + random_state.randrange(3)) % 4
        bases = (48, 104, 160, 216)

        def pixel(x, y):
            band = y // 4
            local_y = y % 4
            if band == 0:
                signal = amplitude * column_prbs[x]
            elif band == 1:
                signal = ((x - local_y + phase) % 16 - 8) * amplitude // 3
            elif band == 2:
                signal = (2 * local_y - 3) * amplitude
            elif local_y == positive_row:
                signal = amplitude
            elif local_y == negative_row:
                signal = -amplitude
            else:
                signal = 0
            value = clamp_channel(bases[band] + signal)
            return (value, value, value)

        return image_from_pixels((16, 16), pixel)

    write_campaign_image(
        "coverage_h16x4_tx4x4_split_01",
        horizontal16x4_split_adst_dct(),
        "4:2:0",
        advanced={
            "min-partition-size": "4",
            "max-partition-size": "16",
            "use-intra-dct-only": "0",
            "enable-filter-intra": "0",
            "enable-intra-edge-filter": "0",
            "enable-directional-intra": "1",
            "enable-smooth-intra": "1",
            "enable-paeth-intra": "1",
            "enable-cfl-intra": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
        quality=62,
        speed=0,
    )

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

    def filter_intra_420_v8x16_following_mode2_quadrants_noise():
        """Generate the following 8x16 mode-2 witness from the 100-case search.

        The 16x32 frame produces a split root whose upper and lower 16x16
        blocks each use a vertical pair of 8x16 leaves. The lower-left leaf
        is the first following Vertical8x16 block and selects FILTER_PRED
        mode 2 with one TX8x16 luma transform.
        """

        random_state = random.Random(1406)
        quadrants = (
            ((32, 80, 160), (224, 64, 32)),
            ((48, 192, 80), (208, 192, 48)),
        )
        pixels = bytearray()
        for y in range(32):
            for x in range(16):
                base = quadrants[y >= 16][x >= 8]
                delta = random_state.randrange(31) - 15
                pixels.extend(clamp_channel(component + delta) for component in base)
        return Image.frombytes("RGB", (16, 32), bytes(pixels))

    write_campaign_image(
        "coverage_vertical8x16_following_filter_intra_mode2_01",
        filter_intra_420_v8x16_following_mode2_quadrants_noise(),
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

    def filter_intra_vertical8x16_mode2_noise():
        """Generate the origin Vertical8x16 filter-intra mode-2 witness."""

        random_state = random.Random(107)
        pixels = bytes(
            random_state.randrange(256) for _ in range(8 * 16 * 3)
        )
        return Image.frombytes("RGB", (8, 16), pixels)

    write_campaign_image(
        "coverage_vertical8x16_filter_intra_mode2_01",
        filter_intra_vertical8x16_mode2_noise(),
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

    def filter_intra_vertical8x16_mode3_mosaic():
        """Generate the origin Vertical8x16 filter-intra mode-3 witness.

        This is the promoted ``f10_mosaic_04`` candidate from the pinned
        100-case input-only campaign. Keep the exact candidate construction
        here so the committed fixture can be regenerated without copying
        encoded bytes from the oracle work directory.
        """

        pixels = bytearray()
        seed = 1005
        for y in range(16):
            for x in range(8):
                tile = (x // 2) + 4 * (y // 4)
                luma = (36, 92, 148, 204)[(tile + seed) % 4]
                chroma = ((x * 9 + y * 7 + seed) % 31) - 15
                pixels.extend(
                    (
                        luma,
                        clamp_channel(128 + chroma),
                        clamp_channel(128 - chroma),
                    )
                )
        return Image.frombytes("RGB", (8, 16), bytes(pixels))

    write_campaign_image(
        "coverage_vertical8x16_filter_intra_mode3_01",
        filter_intra_vertical8x16_mode3_mosaic(),
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

    def filter_intra_vertical8x16_mode4_tx4x4_grid():
        """Generate the origin Vertical8x16 mode-4 2x4 TX4x4 witness.

        This is the promoted ``f03_color_ramp_08`` candidate from the pinned
        100-case input-only campaign. Keep its deterministic pixel formula
        here so the fixture can be regenerated from source rather than copied
        from the temporary oracle directory.
        """

        pixels = bytearray()
        seed = 309
        for y in range(16):
            for x in range(8):
                base = 24 + ((5 * x + 9 * y + seed) % 192)
                pixels.extend(
                    (
                        clamp_channel(base + 24),
                        base,
                        clamp_channel(base - 24),
                    )
                )
        return Image.frombytes("RGB", (8, 16), bytes(pixels))

    write_campaign_image(
        "coverage_vertical8x16_filter_intra_mode4_tx4x4_grid_01",
        filter_intra_vertical8x16_mode4_tx4x4_grid(),
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

    def luma_diagonal67_vertical_square8():
        """Generate the promoted vertical-following luma mode-8 witness.

        This is D67V-F06-N01 from the pinned 100-case input-only campaign.
        The bottom Square8 leaf has a real top edge and uses the nominal
        Diagonal67 angle with zero delta; neutral chroma keeps the fixture
        focused on the luma Zone-1 predictor and its non-empty TX8x8 residual.
        """

        family = 5
        candidate = 1

        def pixel(x, y):
            if y < 8:
                luma = 80 + 6 * x
            else:
                local_y = y - 8
                source_x = min(7, x + (local_y + 1) // 2)
                luma = 80 + 6 * source_x
                luma += (3 * x + 5 * local_y + 7 * family + candidate) % 7 - 3
            luma = clamp_channel(luma)
            return (luma, luma, luma)

        return image_from_pixels((8, 16), pixel)

    write_campaign_image(
        "coverage_square8_luma_diagonal67_vertical_01",
        luma_diagonal67_vertical_square8(),
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

    def luma_diagonal67_vertical_split_tx4x4_square8(family=1, candidate=0):
        """Generate a split-TX4x4 vertical-following luma witness.

        The default is D67V-F02-N00 from the pinned 100-case input-only
        campaign. The lower leaf is intentionally encoded with four DCT-DCT
        TX4x4 payloads and a nonzero AC coefficient while chroma remains
        neutral and skipped. The second promoted case uses family 4/candidate
        1 (D67V-F05-N01), which resolves to the 70-degree angle symbol.
        """

        if family not in (1, 4) or candidate not in (0, 1):
            raise ValueError("only the promoted D67 split witnesses are supported")

        def pixel(x, y):
            if y < 8:
                slopes = (3, -3, 2, 3, 3, 6, -2, 4, -4, 3)
                luma = 80 + slopes[family] * x + (y - 7) * ((family + candidate) % 2)
            else:
                local_y = y - 8
                source_x = min(7, x + (local_y + 1) // 2)
                luma = 80 + (3 if family == 4 else -3) * source_x
                if family == 1 and x < 4 and local_y < 4:
                    luma += 5 if (x + local_y) % 2 == 0 else -5
                elif family == 4 and x < 4 and local_y < 4:
                    luma += 4 if (x // 2 + local_y // 2) % 2 == 0 else -4
            luma = clamp_channel(luma)
            return (luma, luma, luma)

        return image_from_pixels((8, 16), pixel)

    write_campaign_image(
        "coverage_square8_luma_diagonal67_vertical_split_tx4x4_01",
        luma_diagonal67_vertical_split_tx4x4_square8(),
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

    write_campaign_image(
        "coverage_square8_luma_diagonal67_vertical_split_tx4x4_angle70_01",
        luma_diagonal67_vertical_split_tx4x4_square8(family=4, candidate=1),
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

    def chroma_diagonal67_vertical_square8():
        """Generate the promoted vertical-following UV mode-8 witness.

        This is the exact D67V-F01-N00 candidate from the bounded 100-case
        input-only campaign. The top and bottom 8x8 leaves are joined by a
        clipped 16x16 root split. The chroma field continues the top leaf's
        four-sample edge at the Zone-1 angle-67 slope while the luma field
        retains a small split-TX4x4 residual.
        """

        def edge_signal(x, plane):
            amplitude = 10
            value = (x - 1) * amplitude
            if plane == 1:
                value += x - 1
            return value

        def interpolate_edge(edge, position_q6):
            if position_q6 <= 0:
                return edge[0]
            last = len(edge) - 1
            index = position_q6 // 64
            if index >= last:
                return edge[last]
            fraction = position_q6 % 64
            return edge[index] + ((edge[index + 1] - edge[index]) * fraction + 32) // 64

        def chroma_sample(cx, cy):
            top_edges = [
                [edge_signal(x, plane) for x in range(4)]
                for plane in (0, 1)
            ]
            if cy < 4:
                vertical = cy - 3
                return (
                    top_edges[0][cx] + vertical,
                    top_edges[1][cx] - vertical,
                )
            local_y = cy - 4
            position_q6 = cx * 64 + (local_y + 1) * 27
            u = interpolate_edge(top_edges[0], position_q6)
            v = interpolate_edge(top_edges[1], position_q6)
            perturbation = (3 * cx + 5 * local_y) % 5 - 2
            return u + 3 * perturbation, v + 2 * perturbation

        def yuv_to_rgb(y, u, v):
            du = u - 128
            dv = v - 128
            return (
                clamp_channel(y + (358 * dv + 128) // 256),
                clamp_channel(y - (88 * du + 183 * dv + 128) // 256),
                clamp_channel(y + (453 * du + 128) // 256),
            )

        def pixel(x, y):
            if y < 8:
                luma = 80 + 3 * x
            else:
                local_y = y - 8
                source_x = min(7, x + local_y + 1)
                luma = 80 + 3 * source_x
                if (x, local_y) == (1, 0):
                    luma += 16
            u_delta, v_delta = chroma_sample(x // 2, y // 2)
            return yuv_to_rgb(luma, 128 + u_delta, 128 + v_delta)

        return image_from_pixels((8, 16), pixel)

    write_campaign_image(
        "coverage_square8_chroma_diagonal67_vertical_01",
        chroma_diagonal67_vertical_square8(),
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

    def chroma_smooth_vertical_square16():
        """Generate the following Square16 SmoothVertical witness.

        This is the promoted SV16-F06-N03 input from the deterministic
        100-case input-only campaign. Keep the generator algebra identical to
        the campaign so the committed fixture remains reproducible from its
        provenance.
        """

        family = 5
        candidate = 3
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
            edge = row_signal(cy)
            if cx < 8:
                horizontal = (cx - 7) * (1 + family % 3)
                u_delta = edge + horizontal
                v_delta = edge + horizontal // 2
            else:
                top = row_signal(0)
                bottom = row_signal(7)
                vertical = top + ((bottom - top) * cy + 3) // 7
                ripple = ((cx + 2 * cy + candidate + family) % 3) - 1
                u_delta = vertical + ripple
                v_delta = vertical + ripple // 2
            luma = 128 + ((7 * x + 11 * y + candidate + family) % 7) - 3
            return yuv_to_rgb(luma, 128 + u_delta, 128 + v_delta)

        return image_from_pixels((32, 16), pixel)

    write_campaign_image(
        "coverage_square16_chroma_smooth_vertical_01",
        chroma_smooth_vertical_square16(),
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

    def chroma_smooth_square16():
        """Generate the following Square16 chroma Smooth witness.

        This is the promoted SS16-F06-N01 input from the deterministic
        100-case input-only campaign. Keep the bilinear edge algebra identical
        to the campaign so the committed fixture remains reproducible from
        its provenance.
        """

        family = 5
        candidate = 1
        phase = (3 * family + candidate) % 8
        smooth_weights = (255, 197, 146, 105, 73, 50, 37, 32)

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
            edge = row_signal(cy)
            if cx < 8:
                horizontal = (cx - 7) * (1 + family % 3)
                u_delta = edge + horizontal
                v_delta = edge + horizontal // 2
            else:
                top = row_signal(0)
                bottom = row_signal(7)
                left = edge
                vertical_weight = smooth_weights[cy]
                horizontal_weight = smooth_weights[cx - 8]
                base = (
                    vertical_weight * top
                    + (256 - vertical_weight) * bottom
                    + horizontal_weight * left
                    + (256 - horizontal_weight) * top
                    + 256
                ) >> 9
                perturb = ((3 * cx + 5 * cy + candidate + family) % 5) - 2
                u_delta = base + 2 * perturb
                v_delta = base + perturb
            luma = 128 + ((7 * x + 11 * y + candidate + family) % 7) - 3
            return yuv_to_rgb(luma, 128 + u_delta, 128 + v_delta)

        return image_from_pixels((32, 16), pixel)

    write_campaign_image(
        "coverage_square16_chroma_smooth_01",
        chroma_smooth_square16(),
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

    def full_chroma_top_left_paeth():
        """Generate the promoted 32x32 I444 full-chroma Paeth witness.

        This is candidate ``i444-tl-f01-n05`` from the maintained
        input-only campaign.  The four 8x8 quadrants in the top-left 16x16
        region are deliberately distinct; the lower-right quadrant is
        Paeth-predictable from the top, left, and upper-left neighbors.
        """

        seed = 7005
        random_state = random.Random(seed)
        top_u = [125 + ((i * 3) % 5) * 7 for i in range(8)]
        left_u = [95 + ((i * 5) % 5) * 6 for i in range(8)]
        top_v = [185 - ((i * 2) % 5) * 7 for i in range(8)]
        left_v = [155 - ((i * 3) % 5) * 5 for i in range(8)]
        top_left_u = 145
        top_left_v = 110

        def paeth(left, top, top_left):
            prediction = left + top - top_left
            left_distance = abs(prediction - left)
            top_distance = abs(prediction - top)
            top_left_distance = abs(prediction - top_left)
            if left_distance <= top_distance and left_distance <= top_left_distance:
                return left
            if top_distance <= top_left_distance:
                return top
            return top_left

        def rgb_from_yuv(y, u, v):
            du = u - 128
            dv = v - 128
            return (
                clamp_channel(y + (358 * dv + 128) // 256),
                clamp_channel(y - (88 * du + 183 * dv + 128) // 256),
                clamp_channel(y + (453 * du + 128) // 256),
            )

        def pixel(x, y):
            if x < 8 and y < 8:
                u = 100 + 2 * x + 3 * y
                v = 150 - 2 * x + y
            elif y < 8 and 8 <= x < 16:
                u = top_u[x - 8]
                v = top_v[x - 8]
            elif x < 8 and 8 <= y < 16:
                u = left_u[y - 8]
                v = left_v[y - 8]
            elif 8 <= x < 16 and 8 <= y < 16:
                u = paeth(left_u[y - 8], top_u[x - 8], top_left_u)
                v = paeth(left_v[y - 8], top_v[x - 8], top_left_v)
                u += ((13 * x + 7 * y + seed) % 7) - 3
                v -= ((11 * x + 5 * y + seed) % 7) - 3
            else:
                u = 128 + random_state.randrange(-45, 46)
                v = 128 + random_state.randrange(-45, 46)
            luma = 128 + random_state.randrange(-20, 21)
            if x == 7 and y == 7:
                u = top_left_u
                v = top_left_v
            return rgb_from_yuv(luma, u, v)

        return image_from_pixels((32, 32), pixel)

    write_campaign_image(
        "coverage_i444_full_chroma_top_left_paeth_01",
        full_chroma_top_left_paeth(),
        "4:4:4",
        advanced={
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
        },
        quality=76,
        speed=0,
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

    portable_lossless_monochrome_1x1 = "portable_lossless_monochrome_1x1.avif"
    write_portable_image(
        portable_lossless_monochrome_1x1,
        Image.new("L", (1, 1), 127),
        speed=0,
        subsampling="4:0:0",
        advanced={
            "color-primaries": "1",
            "transfer-characteristics": "13",
            "matrix-coefficients": "6",
            "enable-warped-motion": "0",
        },
    )
    if hashlib.sha256((d / portable_lossless_monochrome_1x1).read_bytes()).hexdigest() != (
        "6c4212de07ead445c0b468c39b77f099cc8555e99edd6460806407d7536be305"
    ):
        raise RuntimeError("1x1 lossless monochrome AVIF differs from its pinned hash")

    portable_lossless_monochrome_alpha = (
        "portable_lossless_monochrome_alpha_17x17.avif"
    )
    monochrome_alpha_pixels = bytes(
        channel
        for y in range(17)
        for x in range(17)
        for channel in (
            (13 * x + 7 * y + 31) & 0xFF,
            (19 * x + 23 * y + 91) & 0xFF,
        )
    )
    monochrome_alpha_image = Image.frombytes("LA", (17, 17), monochrome_alpha_pixels)
    write_portable_image(
        portable_lossless_monochrome_alpha,
        monochrome_alpha_image,
        speed=0,
        subsampling="4:0:0",
        codec="aom",
        advanced={
            "color-primaries": "1",
            "transfer-characteristics": "13",
            "matrix-coefficients": "6",
            "enable-warped-motion": "0",
        },
    )
    if hashlib.sha256((d / portable_lossless_monochrome_alpha).read_bytes()).hexdigest() != (
        "573db25044c9dcbb03f00d623d20a30639d3334ba35feae6d57f115e208741bd"
    ):
        raise RuntimeError("17x17 lossless monochrome alpha AVIF differs from its pinned hash")

    portable_i444_quality100_64x64 = "portable_i444_quality100_64x64.avif"
    square64_source = image_from_pixels(
        (64, 64),
        lambda x, y: ((x * 4) % 256, (y * 4) % 256, ((x + y) * 2) % 256),
    )
    write_portable_image(
        portable_i444_quality100_64x64,
        square64_source,
        speed=0,
        subsampling="4:4:4",
        codec="aom",
        advanced={
            "color-primaries": "1",
            "transfer-characteristics": "13",
            "matrix-coefficients": "6",
            "min-partition-size": "64",
            "max-partition-size": "64",
            "use-intra-dct-only": "1",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
    )
    if hashlib.sha256((d / portable_i444_quality100_64x64).read_bytes()).hexdigest() != (
        "5ceb66b47bda43bec5c421222eeb3ef858c62702bc41833986fdf678f6867737"
    ):
        raise RuntimeError("64x64 quality-100 4:4:4 AVIF differs from its pinned hash")

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
    write_portable_luma_pattern(
        "portable_lossy_420_q99_luma_eob_bin2_eob3.avif",
        (4, 4),
        lambda x, y: 126 if (x, y) == (3, 0) else 127,
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

    uniform_tile_rows_path = d / "uniform_tile_rows_monochrome_256.avif"
    Image.new("L", (256, 256), 127).save(
        uniform_tile_rows_path,
        format="AVIF",
        quality=100,
        speed=8,
        tile_rows=2,
        tile_cols=0,
        autotiling=False,
        max_threads=1,
    )
    uniform_tile_rows_bytes = uniform_tile_rows_path.read_bytes()
    if hashlib.sha256(uniform_tile_rows_bytes).hexdigest() != (
        "782568b37a63b44e632aee68896bcee3c59c5492081c7ac2ac6a31fc0ca1df37"
    ):
        raise RuntimeError("four-row monochrome AVIF differs from its pinned fixture")
    uniform_tile_rows_report = inspect_av1(uniform_tile_rows_path)
    uniform_tile_rows_frame = next(
        obu
        for sample in uniform_tile_rows_report["samples"]
        for obu in sample["obus"]
        if obu["type"] == 6
    )
    if [tile["index"] for tile in uniform_tile_rows_frame["tile_group"]["tiles"]] != [
        0,
        1,
        2,
        3,
    ]:
        raise RuntimeError("monochrome AVIF does not contain four uniform tile rows")

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

    # Split the pinned two-tile monochrome AV1 frame across two tile-group
    # OBUs. The encoded tile payloads stay byte-for-byte identical; only the
    # frame/tile-group framing changes so the decoder assembles pending and
    # trailing tiles together.
    two_tile_source = (
        ROOT / "tests" / "fixtures" / "outputs" / "av1_encoder" / "two_tiles" / "encoded.avif"
    )
    two_tile_bytes = two_tile_source.read_bytes()
    if hashlib.sha256(two_tile_bytes).hexdigest() != (
        "fe27de3e9be2d73a1021799e7e06b1e4c7b3bac9ea609b013c783e893d641b90"
    ):
        raise RuntimeError("two-tile monochrome AVIF source differs from its pinned fixture")
    (d / "multitile_monochrome.avif").write_bytes(two_tile_bytes)

    two_tile_report = inspect_av1(two_tile_source)
    color_sample = next(
        sample
        for sample in two_tile_report["samples"]
        if sample["role"] == "item_color"
    )
    frame_obu = next(obu for obu in color_sample["obus"] if obu["type"] == 6)
    tile_group = frame_obu["tile_group"]
    tiles = tile_group["tiles"]
    if (
        color_sample["length"] != 43
        or frame_obu["frame_header"]["header_bits"] != 60
        or tile_group["start"] != 0
        or tile_group["end"] != 1
        or [tile["index"] for tile in tiles] != [0, 1]
        or any(tile["length"] != 9 for tile in tiles)
    ):
        raise RuntimeError("two-tile monochrome AV1 frame layout differs")

    frame_payload_spans = frame_obu["payload_spans"]
    if len(frame_payload_spans) != 1 or frame_payload_spans[0]["length"] != 28:
        raise RuntimeError("two-tile monochrome frame payload is not contiguous")
    frame_payload = two_tile_bytes[
        frame_payload_spans[0]["offset"] : frame_payload_spans[0]["offset"]
        + frame_payload_spans[0]["length"]
    ]
    frame_header = bytearray(frame_payload[:8])
    if len(frame_header) != 8 or frame_header[-1] & 0x0F:
        raise RuntimeError("two-tile monochrome frame-header alignment differs")
    frame_header[-1] |= 0x08  # AV1 trailing_bits before the standalone frame header.

    frame_start = frame_obu["header_spans"][0]["offset"]
    sample_start = color_sample["spans"][0]["offset"]
    sample_end = sample_start + color_sample["length"]
    sample_prefix = two_tile_bytes[sample_start:frame_start]
    split_sample = sample_prefix + bytes((0x1A, len(frame_header))) + frame_header
    for tile in tiles:
        tile_payload = b"".join(
            two_tile_bytes[span["offset"] : span["offset"] + span["length"]]
            for span in tile["physical_spans"]
        )
        tile_id = tile["index"]
        tile_group_header = 0x80 | (tile_id << 6) | (tile_id << 5)
        group_payload = bytes((tile_group_header,)) + tile_payload
        split_sample += bytes((0x22, len(group_payload))) + group_payload

    if len(split_sample) != color_sample["length"] + 4:
        raise RuntimeError("split tile-group AV1 sample has an unexpected length")
    split_container = bytearray(
        two_tile_bytes[:sample_start] + split_sample + two_tile_bytes[sample_end:]
    )
    # The source is hash-pinned and uses a v0 iloc with a 4-byte extent length
    # at offset 0x74, followed by one terminal mdat box.
    iloc_extent_marker = b"\x00\x00\x01\x0d\x00\x00\x00+"
    if two_tile_bytes.count(iloc_extent_marker) != 1:
        raise RuntimeError("two-tile monochrome iloc extent moved")
    iloc_length_offset = two_tile_bytes.index(iloc_extent_marker) + 4
    if struct.unpack_from(">I", split_container, iloc_length_offset)[0] != 43:
        raise RuntimeError("two-tile monochrome iloc extent length differs")
    struct.pack_into(">I", split_container, iloc_length_offset, len(split_sample))

    mdat_type_offset = two_tile_bytes.rfind(b"mdat")
    if mdat_type_offset < 4:
        raise RuntimeError("two-tile monochrome mdat box is missing")
    mdat_size_offset = mdat_type_offset - 4
    if struct.unpack_from(">I", split_container, mdat_size_offset)[0] != 51:
        raise RuntimeError("two-tile monochrome mdat size differs")
    struct.pack_into(">I", split_container, mdat_size_offset, 55)

    split_path = d / "multitile_monochrome_split_groups.avif"
    split_path.write_bytes(split_container)
    if hashlib.sha256(split_container).hexdigest() != (
        "57490ef5f80626a3218b96cae9f3e9601ff9db685fc608cd02ad39213638b24c"
    ):
        raise RuntimeError("split tile-group AVIF differs from its pinned transformation")
    split_report = inspect_av1(split_path)
    split_obus = split_report["samples"][0]["obus"]
    if [obu["type"] for obu in split_obus] != [2, 1, 3, 4, 4]:
        raise RuntimeError("split tile-group AV1 OBU sequence differs")
    split_groups = [obu["tile_group"] for obu in split_obus if obu["type"] == 4]
    if [(group["start"], group["end"]) for group in split_groups] != [(0, 0), (1, 1)]:
        raise RuntimeError("split tile-group AV1 ranges differ")
    source_pixels = Image.open(BytesIO(two_tile_bytes)).tobytes()
    split_pixels = Image.open(BytesIO(split_container)).tobytes()
    if split_pixels != source_pixels:
        raise RuntimeError("split tile-group AVIF changed Pillow-decoded pixels")

    from generate_avif_color_tile_split_fixture import generate_fixture

    color_split_path = d / "multitile_color_split_groups.avif"
    generate_fixture(multitile_path, color_split_path)

    duplicate_tile_start = bytearray(split_container)
    tile_group_obus = [obu for obu in split_obus if obu["type"] == 4]
    if len(tile_group_obus) != 2:
        raise RuntimeError("split tile-group AVIF no longer has two tile-group OBUs")
    second_group_payload_spans = tile_group_obus[1]["payload_spans"]
    if len(second_group_payload_spans) != 1:
        raise RuntimeError("second tile-group payload is no longer contiguous")
    second_group_header = second_group_payload_spans[0]["offset"]
    if duplicate_tile_start[second_group_header] != 0xE0:
        raise RuntimeError("second tile-group start index differs from tile 1")
    duplicate_tile_start[second_group_header] = 0x80
    duplicate_tile_start_path = d / "error_tile_group_start_order.avif"
    duplicate_tile_start_path.write_bytes(duplicate_tile_start)
    if hashlib.sha256(duplicate_tile_start).hexdigest() != (
        "68781acc246e2946c962c33990c53b50ed308a06fcdec6e421a38dbf353de4cf"
    ):
        raise RuntimeError("duplicate AV1 tile-group start fixture differs")
    try:
        with Image.open(BytesIO(duplicate_tile_start)) as malformed:
            malformed.load()
    except RuntimeError as error:
        if str(error) != "Failed to decode frame 0: Decoding of color planes failed":
            raise RuntimeError("duplicate AV1 tile-group start error changed") from error
    else:
        raise RuntimeError("Pillow unexpectedly accepted a duplicate AV1 tile-group start")

    # Exercise tiled monochrome reconstruction when CDEF is disabled on an
    # auxiliary alpha item. The main color item remains non-monochrome, so
    # Pillow's full-image decode also validates the alpha composition.
    cdef_disabled_path = d / "multitile_monochrome_alpha_cdef_disabled.avif"
    cdef_disabled_image = Image.new("RGBA", (128, 64))
    cdef_disabled_image.putdata(
        [
            (
                37,
                83,
                131,
                (47 if x < 64 else 193)
                if (x + y) % 3
                else (128 if x < 64 else 222),
            )
            for y in range(64)
            for x in range(128)
        ]
    )
    cdef_disabled_image.save(
        cdef_disabled_path,
        format="AVIF",
        quality=75,
        speed=6,
        tile_cols=1,
        tile_rows=0,
        autotiling=False,
        advanced=[("enable-cdef", "0")],
    )
    cdef_disabled_bytes = cdef_disabled_path.read_bytes()
    if hashlib.sha256(cdef_disabled_bytes).hexdigest() != (
        "c8518f229ee0688d8cdd026a4e6e52842374e14028f4584533478ea6eb34f5ae"
    ):
        raise RuntimeError(
            "CDEF-disabled monochrome alpha AVIF differs from its pinned fixture"
        )
    cdef_disabled_report = inspect_av1(cdef_disabled_path)
    alpha_sample = next(
        sample
        for sample in cdef_disabled_report["samples"]
        if sample["role"] == "item_alpha"
    )
    alpha_sequence = next(
        obu["sequence_header"]
        for obu in alpha_sample["obus"]
        if "sequence_header" in obu
    )
    alpha_frame = next(obu for obu in alpha_sample["obus"] if obu["type"] == 6)
    alpha_frame_header = alpha_frame["frame_header"]
    alpha_frame_dimensions = (
        alpha_frame_header["frame_width"],
        alpha_frame_header["frame_height"],
    )
    alpha_tiles = alpha_frame["tile_group"]["tiles"]
    if (
        alpha_sequence["monochrome"] is not True
        or alpha_sequence["enable_cdef"] is not False
        or alpha_frame_dimensions != (128, 64)
        or [tile["index"] for tile in alpha_tiles] != [0, 1]
    ):
        raise RuntimeError("CDEF-disabled monochrome alpha AV1 layout differs")
    with Image.open(cdef_disabled_path) as cdef_disabled_oracle:
        cdef_disabled_oracle.load()
        if (
            cdef_disabled_oracle.mode != "RGBA"
            or cdef_disabled_oracle.size != (128, 64)
        ):
            raise RuntimeError("CDEF-disabled monochrome alpha AVIF Pillow output differs")

    write_multitile_monochrome_alpha_zero_loop_filter(
        d / "multitile_monochrome_alpha_zero_loop_filter.avif"
    )

    # Preserve the final partial CDEF block of an odd-width auxiliary alpha
    # item while two independently decoded monochrome tiles are assembled.
    cdef_odd_path = d / "multitile_monochrome_alpha_cdef_odd_width.avif"
    cdef_odd_image = Image.new("RGBA", (127, 64))
    cdef_odd_image.putdata(
        [
            (
                37,
                83,
                131,
                (47 if x < 64 else 193)
                if (x + y) % 3
                else (128 if x < 64 else 222),
            )
            for y in range(64)
            for x in range(127)
        ]
    )
    cdef_odd_image.save(
        cdef_odd_path,
        format="AVIF",
        quality=75,
        speed=6,
        tile_cols=1,
        tile_rows=0,
        autotiling=False,
        advanced=[("enable-cdef", "1")],
    )
    cdef_odd_bytes = cdef_odd_path.read_bytes()
    if hashlib.sha256(cdef_odd_bytes).hexdigest() != (
        "e607a2377e82e3a8ea6e550968419947efd49caee7b46ae83ad5f84b4b91774a"
    ):
        raise RuntimeError("odd-width monochrome alpha AVIF differs from its pinned fixture")
    cdef_odd_report = inspect_av1(cdef_odd_path)
    alpha_sample = next(
        sample for sample in cdef_odd_report["samples"] if sample["role"] == "item_alpha"
    )
    alpha_sequence = next(
        obu["sequence_header"]
        for obu in alpha_sample["obus"]
        if "sequence_header" in obu
    )
    alpha_frame = next(obu for obu in alpha_sample["obus"] if obu["type"] == 6)
    alpha_frame_header = alpha_frame["frame_header"]
    alpha_frame_dimensions = (
        alpha_frame_header["frame_width"],
        alpha_frame_header["frame_height"],
    )
    alpha_tiles = alpha_frame["tile_group"]["tiles"]
    if (
        alpha_sequence["monochrome"] is not True
        or alpha_sequence["enable_cdef"] is not True
        or alpha_frame_dimensions != (127, 64)
        or [tile["index"] for tile in alpha_tiles] != [0, 1]
    ):
        raise RuntimeError("odd-width monochrome alpha AV1 layout differs")
    with Image.open(cdef_odd_path) as cdef_odd_oracle:
        cdef_odd_oracle.load()
        if cdef_odd_oracle.mode != "RGBA" or cdef_odd_oracle.size != (127, 64):
            raise RuntimeError("odd-width monochrome alpha AVIF Pillow output differs")

    baseline_path = d / "baseline.avif"
    baseline = bytearray(baseline_path.read_bytes())
    size_zero_top_level_free = (
        bytes(baseline)
        + struct.pack(">I4s", 0, b"free")
        + b"trailing data through EOF"
    )
    if hashlib.sha256(size_zero_top_level_free).hexdigest() != (
        "0390a339020a54aa4ddb7600b988407c10efdc21ec0bdff762a793b7a1217e6c"
    ):
        raise RuntimeError("AVIF size-zero top-level free fixture differs from its pinned hash")
    size_zero_top_level_free_path = d / "size_zero_top_level_free.avif"
    size_zero_top_level_free_path.write_bytes(size_zero_top_level_free)
    with Image.open(baseline_path) as reference, Image.open(
        size_zero_top_level_free_path
    ) as variant:
        reference.load()
        variant.load()
        if (
            variant.format != reference.format
            or variant.size != reference.size
            or variant.mode != reference.mode
            or variant.tobytes() != reference.tobytes()
        ):
            raise RuntimeError("AVIF size-zero top-level free box changed Pillow pixels")

    baseline_bytes = bytes(baseline)
    if (
        len(baseline) < 32
        or baseline[4:8] != b"ftyp"
        or baseline[8:12] != b"avif"
        or struct.unpack_from(">I", baseline, 0)[0] != 32
    ):
        raise RuntimeError("baseline AVIF must begin with the expected 32-byte avif ftyp box")
    malformed_avif_ftyp_size = bytearray(baseline)
    malformed_avif_ftyp_size[:4] = (31).to_bytes(4, "big")
    (d / "malformed_avif_ftyp_size.avif").write_bytes(malformed_avif_ftyp_size)
    av1c_type_offset = baseline_bytes.find(b"av1C")
    if baseline_bytes.count(b"av1C") != 1 or av1c_type_offset < 4:
        raise RuntimeError("baseline AVIF needs one bounded AV1 configuration property")
    av1c_header_offset = av1c_type_offset - 4
    av1c_box_size = struct.unpack_from(">I", baseline_bytes, av1c_header_offset)[0]
    av1c_payload_offset = av1c_type_offset + 4
    if (
        av1c_box_size < 12
        or av1c_header_offset + av1c_box_size > len(baseline_bytes)
        or av1c_payload_offset + 3 >= av1c_header_offset + av1c_box_size
        or baseline_bytes[av1c_payload_offset] != 0x81
    ):
        raise RuntimeError("baseline AV1 configuration record differs from its fixture")

    invalid_av1c_marker = bytearray(baseline_bytes)
    invalid_av1c_marker[av1c_payload_offset] = 0
    if hashlib.sha256(invalid_av1c_marker).hexdigest() != (
        "8e183203e849b4a7fd01bf5662b585a42b745b01b18a62a5c11cb64d550dbae9"
    ):
        raise RuntimeError("AVIF invalid av1C marker fixture differs from its pinned hash")
    invalid_av1c_marker_path = d / "error_av1c_invalid_marker.avif"
    invalid_av1c_marker_path.write_bytes(invalid_av1c_marker)

    invalid_av1c_depth_flags = bytearray(baseline_bytes)
    flags_offset = av1c_payload_offset + 2
    invalid_av1c_depth_flags[flags_offset] = (
        invalid_av1c_depth_flags[flags_offset] & 0x1F
    ) | 0x20
    if hashlib.sha256(invalid_av1c_depth_flags).hexdigest() != (
        "68e3963bfc3437156e871b5b48e279e1450fa8d67f22b03644925190b8b03695"
    ):
        raise RuntimeError("AVIF invalid av1C depth flags differ from their pinned hash")
    invalid_av1c_depth_flags_path = d / "error_av1c_twelve_bit_without_high_bit_depth.avif"
    invalid_av1c_depth_flags_path.write_bytes(invalid_av1c_depth_flags)

    for malformed_path in (
        invalid_av1c_marker_path,
        invalid_av1c_depth_flags_path,
    ):
        try:
            with Image.open(malformed_path) as malformed:
                malformed.load()
        except OSError:
            pass
        else:
            raise RuntimeError(f"Pillow unexpectedly decoded {malformed_path.name}")

    from generate_avif_config_disagreement_fixtures import generate as generate_config_disagreements
    from generate_avif_filmgrain_edge_fixtures import write_avif_filmgrain_edge_fixtures
    from generate_avif_idat_fixture import generate as generate_idat_fixture

    generate_config_disagreements(output_dir=d, source_path=baseline_path)
    write_avif_filmgrain_edge_fixtures(d)
    generate_idat_fixture(output_dir=d, source_path=baseline_path)

    meta_payload_start, meta_payload_end = avif_unique_top_level_payload_range(
        baseline_bytes, b"meta"
    )
    meta_children = avif_parse_boxes(baseline_bytes, meta_payload_start + 4, meta_payload_end)
    iprp_entries = [payload for kind, payload in meta_children if kind == b"iprp"]
    if len(iprp_entries) != 1:
        raise RuntimeError("nested-size-zero fixture needs one iprp property table")
    iprp_children = avif_parse_boxes(iprp_entries[0])
    if not iprp_children or iprp_children[-1][0] != b"ipma":
        raise RuntimeError("nested-size-zero fixture needs ipma as the final iprp child")
    if baseline_bytes.count(b"ipma") != 1:
        raise RuntimeError("nested-size-zero fixture needs one ipma box signature")
    ipma_type_offset = baseline_bytes.index(b"ipma")
    ipma_header_offset = ipma_type_offset - 4
    ipma_size = struct.unpack_from(">I", baseline_bytes, ipma_header_offset)[0]
    if ipma_size != len(iprp_children[-1][1]) + 8:
        raise RuntimeError("nested-size-zero fixture found an unexpected ipma extent")
    ipma_as_size_zero = bytearray(baseline_bytes)
    struct.pack_into(">I", ipma_as_size_zero, ipma_header_offset, 0)
    if hashlib.sha256(ipma_as_size_zero).hexdigest() != (
        "01ef1212f06626031f796c5268e49065a340ab0308a0cc7db4a0fdbafcfdbc65"
    ):
        raise RuntimeError("AVIF nested-size-zero ipma fixture differs from its pinned hash")
    nested_size_zero_ipma_path = d / "nested_size_zero_ipma.avif"
    nested_size_zero_ipma_path.write_bytes(ipma_as_size_zero)
    try:
        with Image.open(nested_size_zero_ipma_path) as malformed:
            malformed.load()
    except OSError:
        pass
    else:
        raise RuntimeError("Pillow unexpectedly accepted nested size-zero ipma")

    iloc_v2 = upgrade_avif_iloc_to_version_2(baseline)
    iloc_v2_path = d / "iloc_version_2_wide_item_ids.avif"
    if hashlib.sha256(iloc_v2).hexdigest() != (
        "39a494da43edb3e25dcc60c1c51c987452c66a2590e5ac9563d416d922199d87"
    ):
        raise RuntimeError("AVIF iloc-v2 fixture differs from its pinned hash")
    iloc_v2_path.write_bytes(iloc_v2)
    with Image.open(baseline_path) as reference, Image.open(iloc_v2_path) as variant:
        reference.load()
        variant.load()
        if (
            variant.format != reference.format
            or variant.size != reference.size
            or variant.mode != reference.mode
            or variant.info != reference.info
            or variant.tobytes() != reference.tobytes()
        ):
            raise RuntimeError("AVIF iloc-v2 mutation changed Pillow output")

    ipma_wide_associations = widen_avif_ipma_associations(baseline)
    ipma_wide_path = d / "ipma_wide_associations.avif"
    if hashlib.sha256(ipma_wide_associations).hexdigest() != (
        "7bef3a5c1db6196b199c03406719f5dcd4b0bb6a7320de44b39a7daa52e6116b"
    ):
        raise RuntimeError("AVIF wide-ipma fixture differs from its pinned hash")
    ipma_wide_path.write_bytes(ipma_wide_associations)
    with Image.open(baseline_path) as reference, Image.open(ipma_wide_path) as variant:
        reference.load()
        variant.load()
        if (
            variant.format != reference.format
            or variant.size != reference.size
            or variant.mode != reference.mode
            or variant.info != reference.info
            or variant.tobytes() != reference.tobytes()
        ):
            raise RuntimeError("AVIF wide-ipma mutation changed Pillow output")

    baseline_root = avif_parse_boxes(baseline)
    baseline_meta_indices = [
        index for index, (kind, _payload) in enumerate(baseline_root) if kind == b"meta"
    ]
    if len(baseline_meta_indices) != 1:
        raise RuntimeError("baseline AVIF needs one top-level meta box")
    baseline_meta_index = baseline_meta_indices[0]
    baseline_meta_payload = baseline_root[baseline_meta_index][1]
    if len(baseline_meta_payload) < 4 or baseline_meta_payload[:4] != bytes(4):
        raise RuntimeError("baseline AVIF meta must use a version-zero full box")
    baseline_meta_children = avif_parse_boxes(baseline_meta_payload, 4)
    iprp_indices = [
        index for index, (kind, _payload) in enumerate(baseline_meta_children)
        if kind == b"iprp"
    ]
    iloc_indices = [
        index for index, (kind, _payload) in enumerate(baseline_meta_children)
        if kind == b"iloc"
    ]
    if len(iprp_indices) != 1 or len(iloc_indices) != 1:
        raise RuntimeError("baseline AVIF needs one iprp and one iloc child")
    iprp_index = iprp_indices[0]
    iprp_children = avif_parse_boxes(baseline_meta_children[iprp_index][1])
    ipma_indices = [
        index for index, (kind, _payload) in enumerate(iprp_children) if kind == b"ipma"
    ]
    if len(ipma_indices) != 1:
        raise RuntimeError("baseline AVIF needs one ipma child")
    ipma_index = ipma_indices[0]
    ipma = iprp_children[ipma_index][1]
    if (
        len(ipma) != 15
        or ipma[:8] != bytes.fromhex("0000000000000001")
        or struct.unpack_from(">H", ipma, 8)[0] != 1
        or ipma[10] != 4
    ):
        raise RuntimeError("baseline AVIF ipma must have one version-zero item and four associations")
    item_id = struct.unpack_from(">H", ipma, 8)[0]
    ipma_v1 = bytes((1,)) + ipma[1:8] + struct.pack(">I", item_id) + ipma[10:]
    if len(ipma_v1) != len(ipma) + 2:
        raise RuntimeError("AVIF ipma version-one item ID width differs")
    iprp_children[ipma_index] = (b"ipma", ipma_v1)
    baseline_meta_children[iprp_index] = (
        b"iprp",
        avif_pack_boxes(iprp_children),
    )

    iloc_index = iloc_indices[0]
    iloc = bytearray(baseline_meta_children[iloc_index][1])
    if (
        len(iloc) != 22
        or iloc[:14] != bytes.fromhex("0000000044000001000100000001")
    ):
        raise RuntimeError("baseline AVIF iloc must have one version-zero 32-bit file extent")
    mdat_payload_start, mdat_payload_end = avif_unique_top_level_payload_range(
        baseline, b"mdat"
    )
    extent_offset = struct.unpack_from(">I", iloc, 14)[0]
    extent_length = struct.unpack_from(">I", iloc, 18)[0]
    if not (
        mdat_payload_start <= extent_offset
        and extent_offset + extent_length <= mdat_payload_end
    ):
        raise RuntimeError("baseline AVIF item extent is outside the mdat payload")
    relocated_extent = extent_offset + (len(ipma_v1) - len(ipma))
    struct.pack_into(">I", iloc, 14, relocated_extent)
    baseline_meta_children[iloc_index] = (b"iloc", bytes(iloc))
    baseline_root[baseline_meta_index] = (
        b"meta",
        baseline_meta_payload[:4] + avif_pack_boxes(baseline_meta_children),
    )
    ipma_v1_avif = avif_pack_boxes(baseline_root)
    if len(ipma_v1_avif) != len(baseline) + 2:
        raise RuntimeError("AVIF ipma version-one mutation changed the wrong extent")
    ipma_v1_path = d / "ipma_version_1_wide_item_ids.avif"
    if hashlib.sha256(ipma_v1_avif).hexdigest() != (
        "90ac8f0230ddede24a72b73fb120bd46680962d4c0a7facab31e77b088cd82c5"
    ):
        raise RuntimeError("AVIF ipma version-one fixture differs from its pinned hash")
    ipma_v1_path.write_bytes(ipma_v1_avif)
    with Image.open(baseline_path) as reference, Image.open(ipma_v1_path) as variant:
        reference.load()
        variant.load()
        if (
            variant.format != reference.format
            or variant.size != reference.size
            or variant.mode != reference.mode
            or variant.info != reference.info
            or variant.tobytes() != reference.tobytes()
        ):
            raise RuntimeError("AVIF ipma version-one mutation changed Pillow output")

    meta_payload_start, meta_payload_end = avif_unique_top_level_payload_range(
        baseline, b"meta"
    )
    if baseline[meta_payload_start] != 0:
        raise RuntimeError("baseline AVIF meta version must be zero")
    nonzero_meta_version = bytearray(baseline)
    nonzero_meta_version[meta_payload_start] = 1
    if hashlib.sha256(nonzero_meta_version).hexdigest() != (
        "81cbc07cf4cad9653c58c47c7b88fcd3c323d7f749e2c47cbb39995a3a411b9b"
    ):
        raise RuntimeError("AVIF nonzero meta-version fixture differs from its pinned hash")
    nonzero_meta_version_path = d / "meta_nonzero_version.avif"
    nonzero_meta_version_path.write_bytes(nonzero_meta_version)
    first_meta_child_offset = meta_payload_start + 4
    meta_children = avif_parse_boxes(baseline, first_meta_child_offset, meta_payload_end)
    if not meta_children or meta_children[0][0] != b"hdlr":
        raise RuntimeError("baseline AVIF meta must start with an hdlr child")
    handler_kind_offset = first_meta_child_offset + 4
    handler_size = struct.unpack_from(">I", baseline, first_meta_child_offset)[0]
    if (
        handler_size < 20
        or baseline[handler_kind_offset + 12 : handler_kind_offset + 16] != b"pict"
    ):
        raise RuntimeError("baseline AVIF meta handler must be pict")
    missing_meta_handler = bytearray(baseline)
    missing_meta_handler[handler_kind_offset : handler_kind_offset + 4] = b"free"
    (d / "meta_first_child_not_hdlr.avif").write_bytes(missing_meta_handler)
    wrong_meta_handler = bytearray(baseline)
    handler_type_offset = first_meta_child_offset + 16
    wrong_meta_handler[handler_type_offset : handler_type_offset + 4] = b"mdir"
    (d / "meta_hdlr_non_pict.avif").write_bytes(wrong_meta_handler)

    if len(meta_children) < 2 or meta_children[1][0] != b"pitm":
        raise RuntimeError("baseline AVIF meta must place pitm after its hdlr")
    primary_item_offset = first_meta_child_offset + handler_size

    meta_child_offsets = {}
    child_offset = first_meta_child_offset
    for child_kind, _ in meta_children:
        child_size = struct.unpack_from(">I", baseline, child_offset)[0]
        if child_size < 8 or child_size > meta_payload_end - child_offset:
            raise RuntimeError("baseline AVIF meta child has an invalid size")
        if child_kind in meta_child_offsets:
            raise RuntimeError(f"baseline AVIF meta repeats {child_kind!r}")
        meta_child_offsets[child_kind] = child_offset
        child_offset += child_size
    if child_offset != meta_payload_end:
        raise RuntimeError("baseline AVIF meta children do not fill their parent")

    for child_kind, fixture_name in (
        (b"pitm", "meta_missing_primary_item.avif"),
        (b"iinf", "meta_missing_item_info.avif"),
        (b"iprp", "meta_missing_item_properties.avif"),
    ):
        child_offset = meta_child_offsets.get(child_kind)
        if child_offset is None:
            raise RuntimeError(f"baseline AVIF meta has no {child_kind!r} child")
        missing_child = bytearray(baseline)
        missing_child[child_offset + 4 : child_offset + 8] = b"free"
        (d / fixture_name).write_bytes(missing_child)

    duplicate_meta_handler = bytearray(baseline)
    duplicate_meta_handler[primary_item_offset + 4 : primary_item_offset + 8] = b"hdlr"
    duplicate_meta_handler_path = d / "meta_duplicate_handler.avif"
    expected_duplicate_handler_sha256 = (
        "815b07f504c38fc5513722ab1a218ec55525b4e43df8a5f811e9d3d1c89786e9"
    )
    if hashlib.sha256(duplicate_meta_handler).hexdigest() != (
        expected_duplicate_handler_sha256
    ):
        raise RuntimeError("AVIF duplicate meta handler fixture differs from its pinned hash")
    duplicate_meta_handler_path.write_bytes(duplicate_meta_handler)

    iloc_offset = meta_child_offsets.get(b"iloc")
    if iloc_offset is None:
        raise RuntimeError("baseline AVIF meta has no item-location child")
    duplicate_primary_item = bytearray(baseline)
    duplicate_primary_item[iloc_offset + 4 : iloc_offset + 8] = b"pitm"
    if hashlib.sha256(duplicate_primary_item).hexdigest() != (
        "3badc9ce02273d7ac0210aa6bcc3634e75cfed2531b30427bf721620452ab2c3"
    ):
        raise RuntimeError("AVIF duplicate primary-item fixture differs from its pinned hash")
    duplicate_primary_item_path = d / "meta_duplicate_primary_item.avif"
    duplicate_primary_item_path.write_bytes(duplicate_primary_item)

    iprp_offset = meta_child_offsets.get(b"iprp")
    if iprp_offset is None:
        raise RuntimeError("baseline AVIF meta has no item-property child")
    duplicate_item_info = bytearray(baseline)
    duplicate_item_info[iprp_offset + 4 : iprp_offset + 8] = b"iinf"
    if hashlib.sha256(duplicate_item_info).hexdigest() != (
        "51045cb251dc8cbbdfa245be7893f8fa54735e16414a71cb77abd0708e4f7aec"
    ):
        raise RuntimeError("AVIF duplicate item-info fixture differs from its pinned hash")
    duplicate_item_info_path = d / "meta_duplicate_item_info.avif"
    duplicate_item_info_path.write_bytes(duplicate_item_info)

    iinf_offset = meta_child_offsets.get(b"iinf")
    if iinf_offset is None:
        raise RuntimeError("baseline AVIF meta has no item-information child")
    duplicate_item_location = bytearray(baseline)
    duplicate_item_location[iinf_offset + 4 : iinf_offset + 8] = b"iloc"
    if hashlib.sha256(duplicate_item_location).hexdigest() != (
        "8139b4019d1bdaad53622eaa137d0b8987b29b06a1d360df2c1dc0218d546f76"
    ):
        raise RuntimeError("AVIF duplicate item-location fixture differs from its pinned hash")
    duplicate_item_location_path = d / "meta_duplicate_item_location.avif"
    duplicate_item_location_path.write_bytes(duplicate_item_location)

    def append_meta_children(appended_children):
        root = avif_parse_boxes(baseline_bytes)
        meta_indexes = [index for index, (kind, _) in enumerate(root) if kind == b"meta"]
        if len(meta_indexes) != 1:
            raise RuntimeError("AVIF metadata mutation needs one top-level meta box")
        meta_index = meta_indexes[0]
        meta_payload = root[meta_index][1]
        if len(meta_payload) < 4 or meta_payload[:4] != bytes(4):
            raise RuntimeError("AVIF metadata mutation needs a version-zero meta box")
        meta_children = avif_parse_boxes(meta_payload, 4)
        iloc_indexes = [
            index for index, (kind, _) in enumerate(meta_children) if kind == b"iloc"
        ]
        if len(iloc_indexes) != 1:
            raise RuntimeError("AVIF metadata mutation needs one iloc child")

        iloc_index = iloc_indexes[0]
        iloc = bytearray(meta_children[iloc_index][1])
        if (
            len(iloc) != 22
            or iloc[:14] != bytes.fromhex("0000000044000001000100000001")
        ):
            raise RuntimeError("AVIF metadata mutation needs one v0 32-bit file extent")
        mdat_payload_start, mdat_payload_end = avif_unique_top_level_payload_range(
            baseline_bytes, b"mdat"
        )
        extent_offset = struct.unpack_from(">I", iloc, 14)[0]
        extent_length = struct.unpack_from(">I", iloc, 18)[0]
        if not (
            mdat_payload_start <= extent_offset
            and extent_offset + extent_length <= mdat_payload_end
        ):
            raise RuntimeError("AVIF metadata mutation found an invalid mdat extent")

        appended_bytes = avif_pack_boxes(appended_children)
        relocated_extent = extent_offset + len(appended_bytes)
        if relocated_extent > 0xFFFF_FFFF:
            raise RuntimeError("AVIF metadata mutation exceeds a 32-bit iloc offset")
        struct.pack_into(">I", iloc, 14, relocated_extent)
        meta_children[iloc_index] = (b"iloc", bytes(iloc))
        meta_children.extend(appended_children)
        root[meta_index] = (
            b"meta",
            meta_payload[:4] + avif_pack_boxes(meta_children),
        )
        return avif_pack_boxes(root)

    duplicate_item_properties = append_meta_children([(b"iprp", b"")])
    if hashlib.sha256(duplicate_item_properties).hexdigest() != (
        "9a245809b8f3a78aec952c15ee9703973a5d407bbaffe17580c4b6b3aca26ee8"
    ):
        raise RuntimeError("AVIF duplicate item-properties fixture differs from its pinned hash")
    duplicate_item_properties_path = d / "meta_duplicate_item_properties.avif"
    duplicate_item_properties_path.write_bytes(duplicate_item_properties)

    empty_item_references = [(b"iref", bytes(4)), (b"iref", bytes(4))]
    duplicate_item_references = append_meta_children(empty_item_references)
    if hashlib.sha256(duplicate_item_references).hexdigest() != (
        "ec6f361b37447d4abeb047508863c59e60d3aa73721adb4b5a76e7e159295f3e"
    ):
        raise RuntimeError("AVIF duplicate item-references fixture differs from its pinned hash")
    duplicate_item_references_path = d / "meta_duplicate_item_references.avif"
    duplicate_item_references_path.write_bytes(duplicate_item_references)

    future_iref_version = append_meta_children(
        [(b"iref", bytes.fromhex("02000000"))]
    )
    future_iref_version_path = d / "iref_future_version.avif"
    future_iref_version_path.write_bytes(future_iref_version)
    with Image.open(baseline_path) as reference, Image.open(
        future_iref_version_path
    ) as variant:
        reference.load()
        variant.load()
        if (
            variant.format != reference.format
            or variant.size != reference.size
            or variant.mode != reference.mode
            or variant.info != reference.info
            or variant.tobytes() != reference.tobytes()
        ):
            raise RuntimeError("AVIF future iref version changed Pillow output")

    empty_item_data = append_meta_children([(b"idat", b"")])
    if hashlib.sha256(empty_item_data).hexdigest() != (
        "281c5cf87f6d34f77635924a13384d107111870db7600e10636e4a2735d2c1c7"
    ):
        raise RuntimeError("AVIF empty item-data fixture differs from its pinned hash")
    empty_item_data_path = d / "meta_empty_item_data.avif"
    empty_item_data_path.write_bytes(empty_item_data)

    ispe_type_offset = baseline_bytes.find(b"ispe")
    if baseline_bytes.count(b"ispe") != 1:
        raise RuntimeError("AVIF ispe error fixtures need one image-spatial-extent box")
    ispe_nonzero_version = bytearray(baseline_bytes)
    ispe_nonzero_version[ispe_type_offset + 4] = 1
    if hashlib.sha256(ispe_nonzero_version).hexdigest() != (
        "9caaefec8072bd826607867f9d80c8ce2a10d67f1638bf8c1ac382055d75a791"
    ):
        raise RuntimeError(
            "AVIF nonzero ispe-version fixture differs from its pinned hash"
        )
    ispe_nonzero_version_path = d / "ispe_nonzero_version.avif"
    ispe_nonzero_version_path.write_bytes(ispe_nonzero_version)

    ispe_zero_width = bytearray(baseline_bytes)
    ispe_zero_width[ispe_type_offset + 8 : ispe_type_offset + 12] = bytes(4)
    if hashlib.sha256(ispe_zero_width).hexdigest() != (
        "3524b652351b5a668a7a91a7dcd10a376205d5771858d11e073f61bdf96de3da"
    ):
        raise RuntimeError("AVIF zero-width ispe fixture differs from its pinned hash")
    ispe_zero_width_path = d / "ispe_zero_width.avif"
    ispe_zero_width_path.write_bytes(ispe_zero_width)

    ispe_zero_height = bytearray(baseline_bytes)
    ispe_zero_height[ispe_type_offset + 12 : ispe_type_offset + 16] = bytes(4)
    if hashlib.sha256(ispe_zero_height).hexdigest() != (
        "e93421df7d5d3564f7d297c4deb09a765be702e16aa86de41c3705974d059024"
    ):
        raise RuntimeError("AVIF zero-height ispe fixture differs from its pinned hash")
    ispe_zero_height_path = d / "ispe_zero_height.avif"
    ispe_zero_height_path.write_bytes(ispe_zero_height)

    iloc_type_offset = baseline_bytes.find(b"iloc")
    if baseline_bytes.count(b"iloc") != 1:
        raise RuntimeError("AVIF iloc error fixture needs one item-location box")
    iloc_version_three = bytearray(baseline_bytes)
    iloc_version_three[iloc_type_offset + 4] = 3
    if hashlib.sha256(iloc_version_three).hexdigest() != (
        "9ce231f76eac5fb15f9da9d12cc34b6e6a59d7eeee8bcad4101a451ee9648dd2"
    ):
        raise RuntimeError(
            "AVIF iloc version-three fixture differs from its pinned hash"
        )
    iloc_version_three_path = d / "iloc_version_3.avif"
    iloc_version_three_path.write_bytes(iloc_version_three)

    for malformed_path in (
        nonzero_meta_version_path,
        d / "meta_first_child_not_hdlr.avif",
        d / "meta_hdlr_non_pict.avif",
        duplicate_meta_handler_path,
        duplicate_primary_item_path,
        duplicate_item_info_path,
        duplicate_item_location_path,
        duplicate_item_properties_path,
        duplicate_item_references_path,
        empty_item_data_path,
        ispe_nonzero_version_path,
        ispe_zero_width_path,
        ispe_zero_height_path,
        iloc_version_three_path,
        d / "meta_missing_primary_item.avif",
        d / "meta_missing_item_info.avif",
        d / "meta_missing_item_properties.avif",
    ):
        try:
            with Image.open(malformed_path) as oracle:
                oracle.load()
        except (OSError, RuntimeError):
            pass
        else:
            raise RuntimeError("malformed AVIF meta handler unexpectedly decoded in Pillow")

    uuid_source_path = d / "portable_i444_quality100_64x64.avif"
    uuid_source = uuid_source_path.read_bytes()
    uuid_payload = b"unknown-uuid-large-size"
    uuid_box = struct.pack(
        ">I4sQ16s",
        1,
        b"uuid",
        32 + len(uuid_payload),
        bytes.fromhex("0123456789abcdef0123456789abcdef"),
    ) + uuid_payload
    extended_uuid = uuid_source + uuid_box
    expected_extended_uuid_sha256 = (
        "270ba9556bd21bd85c56b5f7609a5d48e9a255ffc3ec4d1b1fa4c1885a70847d"
    )
    if hashlib.sha256(extended_uuid).hexdigest() != expected_extended_uuid_sha256:
        raise RuntimeError("AVIF extended-size UUID fixture differs from its pinned hash")
    extended_uuid_path = d / "unknown_extended_size_uuid_box.avif"
    extended_uuid_path.write_bytes(extended_uuid)
    with Image.open(uuid_source_path) as source_oracle:
        source_oracle.load()
        source_mode = source_oracle.mode
        source_size = source_oracle.size
        source_pixels = source_oracle.tobytes()
    with Image.open(extended_uuid_path) as uuid_oracle:
        uuid_oracle.load()
        if (
            uuid_oracle.mode != source_mode
            or uuid_oracle.size != source_size
            or uuid_oracle.tobytes() != source_pixels
        ):
            raise RuntimeError("AVIF extended-size UUID changed Pillow-decoded pixels")

    reordered_meta_source = d / "portable_lossy_420_q99_eob_bin_control.avif"
    reordered_meta = bytearray(reordered_meta_source.read_bytes())
    meta_payload_start, meta_payload_end = avif_unique_top_level_payload_range(
        reordered_meta, b"meta"
    )
    child_start = meta_payload_start + 4
    raw_children = []
    position = child_start
    while position < meta_payload_end:
        remaining = meta_payload_end - position
        if remaining < 8:
            raise RuntimeError("AVIF meta child header is truncated")
        size, kind = struct.unpack_from(">I4s", reordered_meta, position)
        header_size = 8
        if size == 1:
            if remaining < 16:
                raise RuntimeError("AVIF large meta child header is truncated")
            size = struct.unpack_from(">Q", reordered_meta, position + 8)[0]
            header_size = 16
        elif size == 0:
            size = remaining
        if size < header_size or size > remaining:
            raise RuntimeError(f"AVIF meta child {kind!r} has an invalid size")
        raw_children.append((kind, bytes(reordered_meta[position : position + size])))
        position += size
    handler_indices = [
        index for index, (kind, _) in enumerate(raw_children) if kind == b"hdlr"
    ]
    primary_indices = [
        index for index, (kind, _) in enumerate(raw_children) if kind == b"pitm"
    ]
    if len(handler_indices) != 1 or len(primary_indices) != 1:
        raise RuntimeError("AVIF meta must contain one hdlr and one pitm child")
    handler_index = handler_indices[0]
    primary_index = primary_indices[0]
    if handler_index >= primary_index:
        raise RuntimeError("AVIF source meta must place hdlr before pitm")
    handler_box = raw_children.pop(handler_index)
    primary_index -= 1
    raw_children.insert(primary_index + 1, handler_box)
    reordered_children = b"".join(raw for _, raw in raw_children)
    if len(reordered_children) != meta_payload_end - child_start:
        raise RuntimeError("AVIF meta child reordering changed the payload size")
    reordered_meta[child_start:meta_payload_end] = reordered_children
    reordered_meta_path = d / "meta_pitm_before_handler_invalid_tile.avif"
    expected_reordered_sha256 = (
        "c17ade7838897349c480b687c13261eb03b3dcb6b2c91f00fbfa754357bfc096"
    )
    if hashlib.sha256(reordered_meta).hexdigest() != expected_reordered_sha256:
        raise RuntimeError("AVIF reordered meta handler fixture differs from its pinned hash")
    reordered_meta_path.write_bytes(reordered_meta)
    try:
        with Image.open(reordered_meta_path) as oracle:
            oracle.load()
    except OSError:
        pass
    else:
        raise RuntimeError("AVIF meta with a non-first handler unexpectedly decoded in Pillow")

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

    if PILLOW_VERSION != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PILLOW_VERSION}")
    pasp_source_path = d / "primary_item_irot.avif"
    pasp_path = d / "primary_item_irot_pasp_4x3.avif"
    pasp_fixture = add_avif_pasp_property(pasp_source_path.read_bytes())
    pasp_path.write_bytes(pasp_fixture)
    with Image.open(pasp_source_path) as reference:
        reference.load()
        expected = (
            reference.format,
            reference.mode,
            reference.size,
            reference.info,
            reference.tobytes(),
        )
    with Image.open(pasp_path) as candidate:
        candidate.load()
        actual = (
            candidate.format,
            candidate.mode,
            candidate.size,
            candidate.info,
            candidate.tobytes(),
        )
    if actual != expected:
        raise RuntimeError("AVIF pasp property changed Pillow's public image result")
    print(
        "  AVIF: wrote portable lossless/lossy, clipped B32 lossless, "
        "multi-tile success/error, pasp metadata, and existing error fixtures"
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
