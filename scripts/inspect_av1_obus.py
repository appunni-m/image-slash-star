#!/usr/bin/env python3
"""Report AV1 OBU framing and sequence headers from committed AVIF fixtures.

This is an independent reverse-mapping oracle for the portable AV1 parser. It
uses only the Python standard library and the byte boundaries reported by
inspect_avif_bitstreams.py. It does not import Pillow, call a native codec, or
reuse Rust output.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from inspect_avif_bitstreams import DEFAULT_INPUT, inspect as inspect_avif


OBU_NAMES = {
    0: "reserved",
    1: "sequence_header",
    2: "temporal_delimiter",
    3: "frame_header",
    4: "tile_group",
    5: "metadata",
    6: "frame",
    7: "redundant_frame_header",
    8: "tile_list",
    15: "padding",
}


class Bits:
    def __init__(self, data: bytes):
        self.data = data
        self.position = 0

    def read(self, count: int) -> int:
        if count < 0 or count > 32:
            raise ValueError(f"invalid bit count {count}")
        if self.position + count > len(self.data) * 8:
            raise ValueError("truncated sequence header")
        value = 0
        for _ in range(count):
            byte = self.data[self.position // 8]
            shift = 7 - self.position % 8
            value = (value << 1) | ((byte >> shift) & 1)
            self.position += 1
        return value

    def bit(self) -> int:
        return self.read(1)

    def signed(self, count: int) -> int:
        value = self.read(count)
        sign = 1 << (count - 1)
        return value - (1 << count) if value & sign else value

    def ns(self, count: int) -> int:
        if count <= 1:
            return 0
        width = count.bit_length()
        threshold = (1 << width) - count
        value = self.read(width - 1)
        return value if value < threshold else (value << 1) - threshold + self.bit()

    def uvlc(self) -> int:
        leading_zeroes = 0
        while self.bit() == 0:
            leading_zeroes += 1
            if leading_zeroes == 32:
                raise ValueError("UVLC value exceeds u32")
        return (1 << leading_zeroes) - 1 + self.read(leading_zeroes)

    def subexp(self, reference: int, bits: int) -> int:
        count = 2 << bits
        recentered_reference = reference + (1 << bits)
        value = 0
        index = 0
        while True:
            width = 3 if index == 0 else 3 + index - 1
            if count < value + 3 * (1 << width):
                value += self.ns(count - value + 1)
                break
            if self.bit() == 0:
                value += self.read(width)
                break
            value += 1 << width
            index += 1
        if recentered_reference * 2 <= count:
            decoded = inverse_recenter(recentered_reference, value)
        else:
            decoded = count - inverse_recenter(count - recentered_reference, value)
        return decoded - (1 << bits)

    def align(self) -> None:
        while self.position % 8:
            if self.bit() != 0:
                raise ValueError("nonzero AV1 byte-alignment bit")

    def trailing_bits(self) -> None:
        if self.bit() != 1:
            raise ValueError("sequence header lacks trailing one bit")
        while self.position < len(self.data) * 8:
            if self.bit() != 0:
                raise ValueError("nonzero sequence-header trailing bit")


def inverse_recenter(reference: int, value: int) -> int:
    if value > reference * 2:
        return value
    if value % 2 == 0:
        return (value // 2) + reference
    return reference - ((value + 1) // 2)


def read_uleb128(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    for index in range(8):
        if offset >= len(data):
            raise ValueError("truncated OBU size")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << (index * 7)
        if byte & 0x80 == 0:
            if value > 0xFFFF_FFFF:
                raise ValueError("OBU size exceeds u32")
            return value, offset
    raise ValueError("OBU size exceeds eight bytes")


def logical_spans(
    physical_spans: list[tuple[int, int]], start: int, end: int
) -> list[dict[str, int]]:
    result = []
    logical = 0
    for physical_start, physical_end in physical_spans:
        length = physical_end - physical_start
        overlap_start = max(start, logical)
        overlap_end = min(end, logical + length)
        if overlap_start < overlap_end:
            result.append(
                {
                    "offset": physical_start + overlap_start - logical,
                    "length": overlap_end - overlap_start,
                }
            )
        logical += length
    if sum(span["length"] for span in result) != end - start:
        raise ValueError("logical OBU range exceeds sample spans")
    return result


def parse_sequence_header(payload: bytes) -> dict[str, object]:
    bits = Bits(payload)
    result: dict[str, object] = {}
    profile = bits.read(3)
    if profile > 2:
        raise ValueError(f"unsupported AV1 profile {profile}")
    still_picture = bool(bits.bit())
    reduced = bool(bits.bit())
    if reduced and not still_picture:
        raise ValueError("reduced sequence header without still_picture")
    result["profile"] = profile
    result["still_picture"] = still_picture
    result["reduced_still_picture_header"] = reduced

    timing: dict[str, object] | None = None
    decoder_model_present = False
    decoder_delay_length = 0
    display_model_present = False
    operating_points = []
    if reduced:
        operating_points.append(
            {
                "idc": 0,
                "level": bits.read(5),
                "tier": 0,
                "decoder_model_present": False,
                "display_model_present": False,
                "initial_display_delay": 10,
            }
        )
    else:
        timing_present = bool(bits.bit())
        if timing_present:
            num_units_in_tick = bits.read(32)
            time_scale = bits.read(32)
            if num_units_in_tick == 0 or time_scale == 0:
                raise ValueError("zero AV1 timing value")
            equal_picture_interval = bool(bits.bit())
            ticks_per_picture = bits.uvlc() + 1 if equal_picture_interval else None
            timing = {
                "num_units_in_tick": num_units_in_tick,
                "time_scale": time_scale,
                "equal_picture_interval": equal_picture_interval,
                "num_ticks_per_picture": ticks_per_picture,
            }
            decoder_model_present = bool(bits.bit())
            if decoder_model_present:
                decoder_delay_length = bits.read(5) + 1
                timing["num_units_in_decoding_tick"] = bits.read(32)
                timing["buffer_removal_delay_length"] = bits.read(5) + 1
                timing["frame_presentation_delay_length"] = bits.read(5) + 1
        display_model_present = bool(bits.bit())
        operating_point_count = bits.read(5) + 1
        for _ in range(operating_point_count):
            operating_point_idc = bits.read(12)
            if operating_point_idc and (
                operating_point_idc & 0xFF == 0
                or operating_point_idc & 0xF00 == 0
            ):
                raise ValueError("invalid operating_point_idc")
            level = bits.read(5)
            tier = bits.bit() if level > 7 else 0
            decoder_parameters = None
            if decoder_model_present and bits.bit():
                decoder_parameters = {
                    "decoder_buffer_delay": bits.read(decoder_delay_length),
                    "encoder_buffer_delay": bits.read(decoder_delay_length),
                    "low_delay_mode": bool(bits.bit()),
                }
            display_parameters = None
            if display_model_present and bits.bit():
                display_parameters = {"initial_display_delay": bits.read(4) + 1}
            operating_points.append(
                {
                    "idc": operating_point_idc,
                    "level": level,
                    "tier": tier,
                    "decoder_model_present": decoder_parameters is not None,
                    "decoder_model": decoder_parameters,
                    "display_model_present": display_parameters is not None,
                    "display_model": display_parameters,
                    "initial_display_delay": (
                        display_parameters["initial_display_delay"]
                        if display_parameters is not None
                        else 10
                    ),
                }
            )

    width_bits = bits.read(4) + 1
    height_bits = bits.read(4) + 1
    result["max_width"] = bits.read(width_bits) + 1
    result["max_height"] = bits.read(height_bits) + 1
    result["width_bits"] = width_bits
    result["height_bits"] = height_bits
    frame_id_numbers_present = False if reduced else bool(bits.bit())
    result["frame_id_numbers_present"] = frame_id_numbers_present
    if frame_id_numbers_present:
        delta_frame_id_bits = bits.read(4) + 2
        frame_id_bits = bits.read(3) + delta_frame_id_bits + 1
        if frame_id_bits > 16:
            raise ValueError("AV1 frame ID exceeds 16 bits")
        result["delta_frame_id_bits"] = delta_frame_id_bits
        result["frame_id_bits"] = frame_id_bits

    result["use_128x128_superblock"] = bool(bits.bit())
    result["enable_filter_intra"] = bool(bits.bit())
    result["enable_intra_edge_filter"] = bool(bits.bit())
    if reduced:
        result.update(
            {
                "enable_interintra_compound": False,
                "enable_masked_compound": False,
                "enable_warped_motion": False,
                "enable_dual_filter": False,
                "enable_order_hint": False,
                "enable_jnt_comp": False,
                "enable_ref_frame_mvs": False,
                "screen_content_tools": 2,
                "force_integer_mv": 2,
                "order_hint_bits": 0,
            }
        )
    else:
        result["enable_interintra_compound"] = bool(bits.bit())
        result["enable_masked_compound"] = bool(bits.bit())
        result["enable_warped_motion"] = bool(bits.bit())
        result["enable_dual_filter"] = bool(bits.bit())
        order_hint = bool(bits.bit())
        result["enable_order_hint"] = order_hint
        result["enable_jnt_comp"] = bool(bits.bit()) if order_hint else False
        result["enable_ref_frame_mvs"] = bool(bits.bit()) if order_hint else False
        result["screen_content_tools"] = 2 if bits.bit() else bits.bit()
        result["force_integer_mv"] = (
            2
            if result["screen_content_tools"] and bits.bit()
            else bits.bit() if result["screen_content_tools"] else 2
        )
        result["order_hint_bits"] = bits.read(3) + 1 if order_hint else 0

    result["enable_superres"] = bool(bits.bit())
    result["enable_cdef"] = bool(bits.bit())
    result["enable_restoration"] = bool(bits.bit())

    high_bitdepth = bits.bit()
    twelve_bit = bits.bit() if profile == 2 and high_bitdepth else 0
    result["bit_depth"] = 12 if twelve_bit else 10 if high_bitdepth else 8
    monochrome = False if profile == 1 else bool(bits.bit())
    result["monochrome"] = monochrome
    color_description_present = bool(bits.bit())
    if color_description_present:
        color_primaries = bits.read(8)
        transfer_characteristics = bits.read(8)
        matrix_coefficients = bits.read(8)
    else:
        color_primaries = transfer_characteristics = matrix_coefficients = 2
    result["color_primaries"] = color_primaries
    result["transfer_characteristics"] = transfer_characteristics
    result["matrix_coefficients"] = matrix_coefficients

    if monochrome:
        color_range = bits.bit()
        subsampling_x = subsampling_y = 1
        chroma_sample_position = 0
    elif (
        color_primaries == 1
        and transfer_characteristics == 13
        and matrix_coefficients == 0
    ):
        if profile != 1 and not (profile == 2 and result["bit_depth"] == 12):
            raise ValueError("identity color matrix requires 4:4:4 profile")
        color_range = 1
        subsampling_x = subsampling_y = 0
        chroma_sample_position = 0
    else:
        color_range = bits.bit()
        if profile == 0:
            subsampling_x = subsampling_y = 1
        elif profile == 1:
            subsampling_x = subsampling_y = 0
        elif result["bit_depth"] == 12:
            subsampling_x = bits.bit()
            subsampling_y = bits.bit() if subsampling_x else 0
        else:
            subsampling_x = 1
            subsampling_y = 0
        chroma_sample_position = (
            bits.read(2) if subsampling_x and subsampling_y else 0
        )
    if matrix_coefficients == 0 and (subsampling_x or subsampling_y):
        raise ValueError("identity color matrix with chroma subsampling")
    result["color_range"] = color_range
    result["subsampling_x"] = subsampling_x
    result["subsampling_y"] = subsampling_y
    result["chroma_sample_position"] = chroma_sample_position
    result["separate_uv_delta_q"] = False if monochrome else bool(bits.bit())
    result["film_grain_present"] = bool(bits.bit())
    bits.trailing_bits()
    result["timing"] = timing
    result["decoder_model_present"] = decoder_model_present
    result["display_model_present"] = display_model_present
    result["operating_points"] = operating_points
    result["payload_bits"] = len(payload) * 8
    return result


FRAME_TYPES = ("key", "inter", "intra_only", "switch")
DEFAULT_LOOP_FILTER_DELTAS = {
    "mode": [0, 0],
    "reference": [1, 0, 0, 0, -1, 0, -1, -1],
}
DEFAULT_GLOBAL_MOTION = {
    "type": "identity",
    "matrix": [0, 0, 1 << 16, 0, 0, 1 << 16],
}


def relative_distance(bits: int, first: int, second: int) -> int:
    if bits == 0:
        return 0
    sign = 1 << (bits - 1)
    difference = first - second
    return (difference & (sign - 1)) - (difference & sign)


def tile_log2(block_size: int, target: int) -> int:
    value = 0
    while block_size << value < target:
        value += 1
    return value


def frame_summary(header: dict[str, object] | None) -> object:
    if header is None:
        return None
    return {
        "frame_type": header["frame_type"],
        "frame_id": header["frame_id"],
        "order_hint": header["order_hint"],
        "showable_frame": header["showable_frame"],
        "refresh_frame_flags": header["refresh_frame_flags"],
        "upscaled_width": header["upscaled_width"],
        "frame_height": header["frame_height"],
        "render_width": header["render_width"],
        "render_height": header["render_height"],
    }


def read_frame_size(
    bits: Bits,
    sequence: dict[str, object],
    header: dict[str, object],
    references: list[dict[str, object] | None],
    use_reference: bool,
) -> None:
    if use_reference:
        for index in range(7):
            if bits.bit():
                reference_index = header["reference_indices"][index]
                reference = references[reference_index]
                if reference is None:
                    raise ValueError("frame size uses an empty reference slot")
                header["upscaled_width"] = reference["upscaled_width"]
                header["frame_height"] = reference["frame_height"]
                header["render_width"] = reference["render_width"]
                header["render_height"] = reference["render_height"]
                read_superres(bits, sequence, header)
                return

    if header["frame_size_override"]:
        header["upscaled_width"] = bits.read(sequence["width_bits"]) + 1
        header["frame_height"] = bits.read(sequence["height_bits"]) + 1
    else:
        header["upscaled_width"] = sequence["max_width"]
        header["frame_height"] = sequence["max_height"]
    read_superres(bits, sequence, header)
    have_render_size = bool(bits.bit())
    header["have_render_size"] = have_render_size
    if have_render_size:
        header["render_width"] = bits.read(16) + 1
        header["render_height"] = bits.read(16) + 1
    else:
        header["render_width"] = header["upscaled_width"]
        header["render_height"] = header["frame_height"]


def read_superres(
    bits: Bits,
    sequence: dict[str, object],
    header: dict[str, object],
) -> None:
    enabled = bool(sequence["enable_superres"] and bits.bit())
    denominator = bits.read(3) + 9 if enabled else 8
    upscaled_width = header["upscaled_width"]
    header["superres_enabled"] = enabled
    header["superres_denominator"] = denominator
    header["frame_width"] = (
        max((upscaled_width * 8 + denominator // 2) // denominator, min(16, upscaled_width))
        if enabled
        else upscaled_width
    )


def derive_short_references(
    sequence: dict[str, object],
    header: dict[str, object],
    references: list[dict[str, object] | None],
) -> list[int]:
    for reference in references:
        if reference is None:
            raise ValueError("short reference signaling uses an empty slot")
    result = [-1] * 7
    result[0] = header["last_frame_idx"]
    result[3] = header["gold_frame_idx"]
    offsets = [
        relative_distance(
            sequence["order_hint_bits"],
            reference["order_hint"],
            header["order_hint"],
        )
        for reference in references
    ]
    earliest_reference = min(range(8), key=offsets.__getitem__)
    used = {result[0], result[3]}

    nonnegative = [
        index for index, offset in enumerate(offsets) if index not in used and offset >= 0
    ]
    if not nonnegative:
        raise ValueError("short reference signaling lacks a future reference")
    result[6] = max(nonnegative, key=offsets.__getitem__)
    used.add(result[6])

    for output in (4, 5):
        positive = [
            index
            for index, offset in enumerate(offsets)
            if index not in used and offset >= 0
        ]
        if not positive:
            break
        result[output] = min(positive, key=offsets.__getitem__)
        used.add(result[output])

    for output in range(1, 7):
        if result[output] >= 0:
            continue
        past = [
            index
            for index, offset in enumerate(offsets)
            if index not in used and offset < 0
        ]
        if past:
            result[output] = max(past, key=offsets.__getitem__)
            used.add(result[output])
        else:
            result[output] = earliest_reference
    return result


def read_tiling(
    bits: Bits,
    sequence: dict[str, object],
    header: dict[str, object],
) -> dict[str, object]:
    uniform = bool(bits.bit())
    superblock_shift = 7 if sequence["use_128x128_superblock"] else 6
    superblock_size = 1 << superblock_shift
    superblock_width = (header["frame_width"] + superblock_size - 1) >> superblock_shift
    superblock_height = (
        header["frame_height"] + superblock_size - 1
    ) >> superblock_shift
    maximum_tile_width = 4096 >> superblock_shift
    maximum_tile_area = (4096 * 2304) >> (2 * superblock_shift)
    minimum_log2_columns = tile_log2(maximum_tile_width, superblock_width)
    maximum_log2_columns = tile_log2(1, min(superblock_width, 64))
    maximum_log2_rows = tile_log2(1, min(superblock_height, 64))
    minimum_log2_tiles = max(
        tile_log2(maximum_tile_area, superblock_width * superblock_height),
        minimum_log2_columns,
    )

    column_starts: list[int] = []
    row_starts: list[int] = []
    if uniform:
        log2_columns = minimum_log2_columns
        while log2_columns < maximum_log2_columns and bits.bit():
            log2_columns += 1
        tile_width = 1 + ((superblock_width - 1) >> log2_columns)
        column_starts = list(range(0, superblock_width, tile_width))
        minimum_log2_rows = max(minimum_log2_tiles - log2_columns, 0)
        log2_rows = minimum_log2_rows
        while log2_rows < maximum_log2_rows and bits.bit():
            log2_rows += 1
        tile_height = 1 + ((superblock_height - 1) >> log2_rows)
        row_starts = list(range(0, superblock_height, tile_height))
    else:
        start = 0
        widest_tile = 0
        while start < superblock_width and len(column_starts) < 64:
            column_starts.append(start)
            maximum = min(superblock_width - start, maximum_tile_width)
            width = bits.ns(maximum) + 1 if maximum > 1 else 1
            start += width
            widest_tile = max(widest_tile, width)
        if start != superblock_width:
            raise ValueError("tile columns do not cover the frame")
        log2_columns = tile_log2(1, len(column_starts))
        area = superblock_width * superblock_height
        if minimum_log2_tiles:
            area >>= minimum_log2_tiles + 1
        maximum_tile_height = max(area // widest_tile, 1)
        start = 0
        while start < superblock_height and len(row_starts) < 64:
            row_starts.append(start)
            maximum = min(superblock_height - start, maximum_tile_height)
            height = bits.ns(maximum) + 1 if maximum > 1 else 1
            start += height
        if start != superblock_height:
            raise ValueError("tile rows do not cover the frame")
        log2_rows = tile_log2(1, len(row_starts))
        minimum_log2_rows = max(minimum_log2_tiles - log2_columns, 0)

    column_starts.append(superblock_width)
    row_starts.append(superblock_height)
    columns = len(column_starts) - 1
    rows = len(row_starts) - 1
    context_update_tile = 0
    tile_size_bytes = 0
    if log2_columns or log2_rows:
        context_update_tile = bits.read(log2_columns + log2_rows)
        if context_update_tile >= columns * rows:
            raise ValueError("context update tile exceeds tile count")
        tile_size_bytes = bits.read(2) + 1
    return {
        "uniform": uniform,
        "minimum_log2_columns": minimum_log2_columns,
        "maximum_log2_columns": maximum_log2_columns,
        "log2_columns": log2_columns,
        "columns": columns,
        "column_starts": column_starts,
        "minimum_log2_rows": minimum_log2_rows,
        "maximum_log2_rows": maximum_log2_rows,
        "log2_rows": log2_rows,
        "rows": rows,
        "row_starts": row_starts,
        "context_update_tile": context_update_tile,
        "tile_size_bytes": tile_size_bytes,
    }


def read_quantization(
    bits: Bits, sequence: dict[str, object]
) -> dict[str, object]:
    base = bits.read(8)

    def delta() -> int:
        return bits.signed(7) if bits.bit() else 0

    y_dc = delta()
    u_dc = u_ac = v_dc = v_ac = 0
    different_uv = False
    if not sequence["monochrome"]:
        different_uv = bool(sequence["separate_uv_delta_q"] and bits.bit())
        u_dc = delta()
        u_ac = delta()
        if different_uv:
            v_dc = delta()
            v_ac = delta()
        else:
            v_dc = u_dc
            v_ac = u_ac
    using_matrix = bool(bits.bit())
    matrix_y = matrix_u = matrix_v = 0
    if using_matrix:
        matrix_y = bits.read(4)
        matrix_u = bits.read(4)
        matrix_v = bits.read(4) if sequence["separate_uv_delta_q"] else matrix_u
    return {
        "base": base,
        "y_dc_delta": y_dc,
        "u_dc_delta": u_dc,
        "u_ac_delta": u_ac,
        "v_dc_delta": v_dc,
        "v_ac_delta": v_ac,
        "different_uv_delta": different_uv,
        "using_matrix": using_matrix,
        "matrix_y": matrix_y,
        "matrix_u": matrix_u,
        "matrix_v": matrix_v,
    }


def empty_segment() -> dict[str, object]:
    return {
        "delta_q": 0,
        "delta_lf_y_vertical": 0,
        "delta_lf_y_horizontal": 0,
        "delta_lf_u": 0,
        "delta_lf_v": 0,
        "reference": -1,
        "skip": False,
        "global_motion": False,
    }


def read_segmentation(
    bits: Bits,
    header: dict[str, object],
    references: list[dict[str, object] | None],
) -> dict[str, object]:
    enabled = bool(bits.bit())
    segments = [empty_segment() for _ in range(8)]
    update_map = update_data = temporal = False
    if enabled:
        if header["primary_ref_frame"] == 7:
            update_map = update_data = True
        else:
            update_map = bool(bits.bit())
            temporal = bool(bits.bit()) if update_map else False
            update_data = bool(bits.bit())
        if update_data:
            widths = (9, 7, 7, 7, 7)
            names = (
                "delta_q",
                "delta_lf_y_vertical",
                "delta_lf_y_horizontal",
                "delta_lf_u",
                "delta_lf_v",
            )
            for segment in segments:
                for name, width in zip(names, widths):
                    if bits.bit():
                        segment[name] = bits.signed(width)
                if bits.bit():
                    segment["reference"] = bits.read(3)
                segment["skip"] = bool(bits.bit())
                segment["global_motion"] = bool(bits.bit())
        else:
            reference_index = header["reference_indices"][header["primary_ref_frame"]]
            reference = references[reference_index]
            if reference is None:
                raise ValueError("segmentation inherits from an empty reference")
            segments = [
                dict(segment) for segment in reference["segmentation"]["segments"]
            ]
    return {
        "enabled": enabled,
        "update_map": update_map,
        "temporal": temporal,
        "update_data": update_data,
        "segments": segments,
    }


def read_loop_filter(
    bits: Bits,
    sequence: dict[str, object],
    header: dict[str, object],
    references: list[dict[str, object] | None],
) -> dict[str, object]:
    if header["all_lossless"] or header["allow_intrabc"]:
        return {
            "level_y": [0, 0],
            "level_u": 0,
            "level_v": 0,
            "sharpness": 0,
            "delta_enabled": True,
            "delta_update": True,
            "deltas": {
                "mode": list(DEFAULT_LOOP_FILTER_DELTAS["mode"]),
                "reference": list(DEFAULT_LOOP_FILTER_DELTAS["reference"]),
            },
        }
    level_y = [bits.read(6), bits.read(6)]
    level_u = level_v = 0
    if not sequence["monochrome"] and any(level_y):
        level_u = bits.read(6)
        level_v = bits.read(6)
    sharpness = bits.read(3)
    if header["primary_ref_frame"] == 7:
        deltas = {
            "mode": list(DEFAULT_LOOP_FILTER_DELTAS["mode"]),
            "reference": list(DEFAULT_LOOP_FILTER_DELTAS["reference"]),
        }
    else:
        reference_index = header["reference_indices"][header["primary_ref_frame"]]
        reference = references[reference_index]
        if reference is None:
            raise ValueError("loop filter inherits from an empty reference")
        inherited = reference["loop_filter"]["deltas"]
        deltas = {
            "mode": list(inherited["mode"]),
            "reference": list(inherited["reference"]),
        }
    delta_enabled = bool(bits.bit())
    delta_update = bool(bits.bit()) if delta_enabled else False
    if delta_update:
        for index in range(8):
            if bits.bit():
                deltas["reference"][index] = bits.signed(7)
        for index in range(2):
            if bits.bit():
                deltas["mode"][index] = bits.signed(7)
    return {
        "level_y": level_y,
        "level_u": level_u,
        "level_v": level_v,
        "sharpness": sharpness,
        "delta_enabled": delta_enabled,
        "delta_update": delta_update,
        "deltas": deltas,
    }


def skip_mode_references(
    sequence: dict[str, object],
    header: dict[str, object],
    references: list[dict[str, object] | None],
) -> list[int] | None:
    if (
        header["reference_mode"] != "select"
        or header["frame_type"] not in ("inter", "switch")
        or not sequence["enable_order_hint"]
    ):
        return None
    order_hint = header["order_hint"]
    before: tuple[int, int] | None = None
    after: tuple[int, int] | None = None
    for index, reference_index in enumerate(header["reference_indices"]):
        reference = references[reference_index]
        if reference is None:
            raise ValueError("skip mode uses an empty reference")
        difference = relative_distance(
            sequence["order_hint_bits"], reference["order_hint"], order_hint
        )
        if difference > 0 and (
            after is None
            or relative_distance(
                sequence["order_hint_bits"], after[0], reference["order_hint"]
            )
            > 0
        ):
            after = (reference["order_hint"], index)
        elif difference < 0 and (
            before is None
            or relative_distance(
                sequence["order_hint_bits"], reference["order_hint"], before[0]
            )
            > 0
        ):
            before = (reference["order_hint"], index)
    if before is not None and after is not None:
        return sorted([before[1], after[1]])
    if before is None:
        return None
    second: tuple[int, int] | None = None
    for index, reference_index in enumerate(header["reference_indices"]):
        reference = references[reference_index]
        assert reference is not None
        if relative_distance(
            sequence["order_hint_bits"], reference["order_hint"], before[0]
        ) < 0 and (
            second is None
            or relative_distance(
                sequence["order_hint_bits"], reference["order_hint"], second[0]
            )
            > 0
        ):
            second = (reference["order_hint"], index)
    return sorted([before[1], second[1]]) if second is not None else None


def read_global_motion(
    bits: Bits,
    header: dict[str, object],
    references: list[dict[str, object] | None],
) -> list[dict[str, object]]:
    motions = [
        {"type": "identity", "matrix": list(DEFAULT_GLOBAL_MOTION["matrix"])}
        for _ in range(7)
    ]
    if header["frame_type"] not in ("inter", "switch"):
        return motions
    for index in range(7):
        if bits.bit() == 0:
            continue
        if bits.bit():
            kind = "rotzoom"
        elif bits.bit():
            kind = "translation"
        else:
            kind = "affine"
        if header["primary_ref_frame"] == 7:
            reference_matrix = DEFAULT_GLOBAL_MOTION["matrix"]
        else:
            slot = header["reference_indices"][header["primary_ref_frame"]]
            reference = references[slot]
            if reference is None:
                raise ValueError("global motion inherits from an empty reference")
            reference_matrix = reference["global_motion"][index]["matrix"]
        matrix = list(DEFAULT_GLOBAL_MOTION["matrix"])
        if kind in ("rotzoom", "affine"):
            matrix[2] = (1 << 16) + 2 * bits.subexp(
                (reference_matrix[2] - (1 << 16)) >> 1, 12
            )
            matrix[3] = 2 * bits.subexp(reference_matrix[3] >> 1, 12)
            parameter_bits = 12
            shift = 10
        else:
            parameter_bits = 9 if header["allow_high_precision_mv"] else 8
            shift = 13 if header["allow_high_precision_mv"] else 14
        if kind == "affine":
            matrix[4] = 2 * bits.subexp(reference_matrix[4] >> 1, 12)
            matrix[5] = (1 << 16) + 2 * bits.subexp(
                (reference_matrix[5] - (1 << 16)) >> 1, 12
            )
        else:
            matrix[4] = -matrix[3]
            matrix[5] = matrix[2]
        matrix[0] = (
            bits.subexp(reference_matrix[0] >> shift, parameter_bits) << shift
        )
        matrix[1] = (
            bits.subexp(reference_matrix[1] >> shift, parameter_bits) << shift
        )
        motions[index] = {"type": kind, "matrix": matrix}
    return motions


def read_film_grain(
    bits: Bits,
    sequence: dict[str, object],
    header: dict[str, object],
    references: list[dict[str, object] | None],
) -> dict[str, object] | None:
    if not (
        sequence["film_grain_present"]
        and (header["show_frame"] or header["showable_frame"])
        and bits.bit()
    ):
        return None
    seed = bits.read(16)
    update = header["frame_type"] != "inter" or bool(bits.bit())
    if not update:
        slot = bits.read(3)
        if slot not in header["reference_indices"]:
            raise ValueError("film grain references an unused slot")
        reference = references[slot]
        if reference is None or reference["film_grain"] is None:
            raise ValueError("film grain inherits from an empty reference")
        inherited = dict(reference["film_grain"])
        inherited["seed"] = seed
        inherited["update"] = False
        inherited["reference_slot"] = slot
        return inherited

    y_count = bits.read(4)
    if y_count > 14:
        raise ValueError("too many film grain Y points")
    y_points = []
    for _ in range(y_count):
        point = [bits.read(8), bits.read(8)]
        if y_points and y_points[-1][0] >= point[0]:
            raise ValueError("film grain Y points are not increasing")
        y_points.append(point)
    chroma_from_luma = bool(bits.bit()) if not sequence["monochrome"] else False
    uv_points: list[list[list[int]]] = [[], []]
    if not (
        sequence["monochrome"]
        or chroma_from_luma
        or (
            sequence["subsampling_x"]
            and sequence["subsampling_y"]
            and y_count == 0
        )
    ):
        for plane in range(2):
            count = bits.read(4)
            if count > 10:
                raise ValueError("too many film grain UV points")
            for _ in range(count):
                point = [bits.read(8), bits.read(8)]
                if uv_points[plane] and uv_points[plane][-1][0] >= point[0]:
                    raise ValueError("film grain UV points are not increasing")
                uv_points[plane].append(point)
    if (
        sequence["subsampling_x"]
        and sequence["subsampling_y"]
        and bool(uv_points[0]) != bool(uv_points[1])
    ):
        raise ValueError("4:2:0 film grain UV point presence differs")
    scaling_shift = bits.read(2) + 8
    ar_coefficient_lag = bits.read(2)
    y_positions = 2 * ar_coefficient_lag * (ar_coefficient_lag + 1)
    ar_y = [bits.read(8) - 128 for _ in range(y_positions)] if y_count else []
    ar_uv = [[], []]
    for plane in range(2):
        if uv_points[plane] or chroma_from_luma:
            count = y_positions + int(bool(y_count))
            ar_uv[plane] = [bits.read(8) - 128 for _ in range(count)]
    ar_coefficient_shift = bits.read(2) + 6
    grain_scale_shift = bits.read(2)
    uv_multiplier = [0, 0]
    uv_luma_multiplier = [0, 0]
    uv_offset = [0, 0]
    for plane in range(2):
        if uv_points[plane]:
            uv_multiplier[plane] = bits.read(8) - 128
            uv_luma_multiplier[plane] = bits.read(8) - 128
            uv_offset[plane] = bits.read(9) - 256
    return {
        "seed": seed,
        "update": True,
        "y_points": y_points,
        "chroma_scaling_from_luma": chroma_from_luma,
        "uv_points": uv_points,
        "scaling_shift": scaling_shift,
        "ar_coefficient_lag": ar_coefficient_lag,
        "ar_coefficients_y": ar_y,
        "ar_coefficients_uv": ar_uv,
        "ar_coefficient_shift": ar_coefficient_shift,
        "grain_scale_shift": grain_scale_shift,
        "uv_multiplier": uv_multiplier,
        "uv_luma_multiplier": uv_luma_multiplier,
        "uv_offset": uv_offset,
        "overlap": bool(bits.bit()),
        "clip_to_restricted_range": bool(bits.bit()),
    }


def parse_frame_header(
    payload: bytes,
    sequence: dict[str, object],
    references: list[dict[str, object] | None],
    *,
    temporal_id: int,
    spatial_id: int,
) -> tuple[dict[str, object], Bits]:
    bits = Bits(payload)
    show_existing = (
        False
        if sequence["reduced_still_picture_header"]
        else bool(bits.bit())
    )
    header: dict[str, object] = {
        "show_existing_frame": show_existing,
        "existing_frame_idx": None,
        "temporal_id": temporal_id,
        "spatial_id": spatial_id,
        "frame_id": 0,
        "order_hint": 0,
        "refresh_frame_flags": 0,
    }
    timing = sequence["timing"]
    if show_existing:
        slot = bits.read(3)
        header["existing_frame_idx"] = slot
        reference = references[slot]
        if reference is None:
            raise ValueError("show-existing uses an empty reference slot")
        if not reference["showable_frame"]:
            raise ValueError("show-existing uses a non-showable frame")
        if (
            sequence["decoder_model_present"]
            and timing is not None
            and not timing["equal_picture_interval"]
        ):
            header["frame_presentation_delay"] = bits.read(
                timing["frame_presentation_delay_length"]
            )
        if sequence["frame_id_numbers_present"]:
            header["frame_id_bit"] = bits.position
            header["frame_id"] = bits.read(sequence["frame_id_bits"])
            if header["frame_id"] != reference["frame_id"]:
                raise ValueError("show-existing frame ID mismatch")
        header.update(
            {
                "frame_type": reference["frame_type"],
                "show_frame": True,
                "showable_frame": reference["showable_frame"],
                "error_resilient_mode": False,
                "disable_cdf_update": False,
                "primary_ref_frame": 7,
                "upscaled_width": reference["upscaled_width"],
                "frame_width": reference["frame_width"],
                "frame_height": reference["frame_height"],
                "render_width": reference["render_width"],
                "render_height": reference["render_height"],
                "header_bits": bits.position,
            }
        )
        return header, bits

    if sequence["reduced_still_picture_header"]:
        frame_type_index = 0
        show_frame = True
    else:
        frame_type_index = bits.read(2)
        show_frame = bool(bits.bit())
    frame_type = FRAME_TYPES[frame_type_index]
    if show_frame:
        if (
            sequence["decoder_model_present"]
            and timing is not None
            and not timing["equal_picture_interval"]
        ):
            header["frame_presentation_delay"] = bits.read(
                timing["frame_presentation_delay_length"]
            )
        showable_frame = frame_type != "key"
    else:
        showable_frame = bool(bits.bit())
    error_resilient = (
        (frame_type == "key" and show_frame)
        or frame_type == "switch"
        or sequence["reduced_still_picture_header"]
        or bool(bits.bit())
    )
    disable_cdf_update = bool(bits.bit())
    allow_screen_content_tools = (
        bool(bits.bit())
        if sequence["screen_content_tools"] == 2
        else bool(sequence["screen_content_tools"])
    )
    force_integer_mv = (
        bool(bits.bit())
        if allow_screen_content_tools and sequence["force_integer_mv"] == 2
        else bool(sequence["force_integer_mv"]) if allow_screen_content_tools else False
    )
    if frame_type in ("key", "intra_only"):
        force_integer_mv = True
    if sequence["frame_id_numbers_present"]:
        header["frame_id_bit"] = bits.position
        header["frame_id"] = bits.read(sequence["frame_id_bits"])
    frame_size_override = (
        False
        if sequence["reduced_still_picture_header"]
        else frame_type == "switch" or bool(bits.bit())
    )
    if sequence["enable_order_hint"]:
        header["order_hint"] = bits.read(sequence["order_hint_bits"])
    primary_ref_frame = (
        bits.read(3)
        if not error_resilient and frame_type in ("inter", "switch")
        else 7
    )
    header.update(
        {
            "frame_type": frame_type,
            "show_frame": show_frame,
            "showable_frame": showable_frame,
            "error_resilient_mode": error_resilient,
            "disable_cdf_update": disable_cdf_update,
            "allow_screen_content_tools": allow_screen_content_tools,
            "force_integer_mv": force_integer_mv,
            "frame_size_override": frame_size_override,
            "primary_ref_frame": primary_ref_frame,
        }
    )

    buffer_removal_times = []
    if sequence["decoder_model_present"]:
        present = bool(bits.bit())
        if present:
            assert timing is not None
            for operating_point in sequence["operating_points"]:
                if not operating_point["decoder_model_present"]:
                    continue
                idc = operating_point["idc"]
                in_temporal = (idc >> temporal_id) & 1
                in_spatial = (idc >> (spatial_id + 8)) & 1
                if idc == 0 or (in_temporal and in_spatial):
                    buffer_removal_times.append(
                        bits.read(timing["buffer_removal_delay_length"])
                    )
    header["buffer_removal_times"] = buffer_removal_times

    intra = frame_type in ("key", "intra_only")
    if intra:
        refresh = 0xFF if frame_type == "key" and show_frame else bits.read(8)
        header["refresh_frame_flags"] = refresh
        if refresh != 0xFF and error_resilient and sequence["enable_order_hint"]:
            header["reference_order_hints"] = [
                bits.read(sequence["order_hint_bits"]) for _ in range(8)
            ]
        if frame_type == "intra_only" and refresh == 0xFF:
            raise ValueError("intra-only frame refreshes every reference")
        header["reference_indices"] = []
        read_frame_size(bits, sequence, header, references, False)
        header["allow_intrabc"] = bool(
            allow_screen_content_tools
            and not header["superres_enabled"]
            and bits.bit()
        )
        header["allow_high_precision_mv"] = False
        header["interpolation_filter"] = "eighttap"
        header["motion_mode_switchable"] = False
        header["use_ref_frame_mvs"] = False
    else:
        refresh = 0xFF if frame_type == "switch" else bits.read(8)
        header["refresh_frame_flags"] = refresh
        if error_resilient and sequence["enable_order_hint"]:
            header["reference_order_hints"] = [
                bits.read(sequence["order_hint_bits"]) for _ in range(8)
            ]
        short = bool(sequence["enable_order_hint"] and bits.bit())
        header["frame_refs_short_signaling"] = short
        if short:
            header["last_frame_idx"] = bits.read(3)
            header["gold_frame_idx"] = bits.read(3)
            reference_indices = derive_short_references(
                sequence, header, references
            )
        else:
            reference_indices = [bits.read(3) for _ in range(7)]
        header["reference_indices"] = reference_indices
        if sequence["frame_id_numbers_present"]:
            for reference_index in reference_indices:
                delta = bits.read(sequence["delta_frame_id_bits"]) + 1
                expected = (
                    header["frame_id"] + (1 << sequence["frame_id_bits"]) - delta
                ) & ((1 << sequence["frame_id_bits"]) - 1)
                reference = references[reference_index]
                if reference is None or reference["frame_id"] != expected:
                    actual = None if reference is None else reference["frame_id"]
                    header.setdefault("reference_frame_id_mismatches", []).append(
                        {
                            "current": header["frame_id"],
                            "slot": reference_index,
                            "delta_bits": sequence["delta_frame_id_bits"],
                            "delta": delta,
                            "expected": expected,
                            "actual": actual,
                        }
                    )
        use_reference_size = not error_resilient and frame_size_override
        read_frame_size(
            bits, sequence, header, references, use_reference_size
        )
        allow_high_precision_mv = False if force_integer_mv else bool(bits.bit())
        interpolation_filter = (
            "switchable"
            if bits.bit()
            else ("eighttap", "eighttap_smooth", "eighttap_sharp", "bilinear")[
                bits.read(2)
            ]
        )
        motion_mode_switchable = bool(bits.bit())
        use_ref_frame_mvs = bool(
            not error_resilient
            and sequence["enable_ref_frame_mvs"]
            and sequence["enable_order_hint"]
            and bits.bit()
        )
        header.update(
            {
                "allow_intrabc": False,
                "allow_high_precision_mv": allow_high_precision_mv,
                "interpolation_filter": interpolation_filter,
                "motion_mode_switchable": motion_mode_switchable,
                "use_ref_frame_mvs": use_ref_frame_mvs,
            }
        )

    header["refresh_frame_context"] = bool(
        not sequence["reduced_still_picture_header"]
        and not disable_cdf_update
        and not bits.bit()
    )
    header["tiling"] = read_tiling(bits, sequence, header)
    header["quantization"] = read_quantization(bits, sequence)
    header["segmentation"] = read_segmentation(bits, header, references)
    base_q = header["quantization"]["base"]
    delta_q_present = bool(base_q and bits.bit())
    delta_q_resolution = bits.read(2) if delta_q_present else 0
    delta_lf_present = bool(
        delta_q_present and not header["allow_intrabc"] and bits.bit()
    )
    delta_lf_resolution = bits.read(2) if delta_lf_present else 0
    delta_lf_multi = bool(bits.bit()) if delta_lf_present else False
    header["delta_q"] = {
        "present": delta_q_present,
        "resolution_log2": delta_q_resolution,
    }
    header["delta_loop_filter"] = {
        "present": delta_lf_present,
        "resolution_log2": delta_lf_resolution,
        "multi": delta_lf_multi,
    }

    quantization = header["quantization"]
    delta_lossless = not any(
        quantization[name]
        for name in (
            "y_dc_delta",
            "u_dc_delta",
            "u_ac_delta",
            "v_dc_delta",
            "v_ac_delta",
        )
    )
    segment_lossless = []
    segment_qindex = []
    for segment in header["segmentation"]["segments"]:
        qindex = (
            max(0, min(255, base_q + segment["delta_q"]))
            if header["segmentation"]["enabled"]
            else base_q
        )
        segment_qindex.append(qindex)
        segment_lossless.append(qindex == 0 and delta_lossless)
    header["segment_qindex"] = segment_qindex
    header["segment_lossless"] = segment_lossless
    header["all_lossless"] = all(segment_lossless)
    header["loop_filter"] = read_loop_filter(
        bits, sequence, header, references
    )

    cdef = {"damping": 0, "bits": 0, "y_strengths": [], "uv_strengths": []}
    if (
        not header["all_lossless"]
        and sequence["enable_cdef"]
        and not header["allow_intrabc"]
    ):
        cdef["damping"] = bits.read(2) + 3
        cdef["bits"] = bits.read(2)
        for _ in range(1 << cdef["bits"]):
            cdef["y_strengths"].append(bits.read(6))
            if not sequence["monochrome"]:
                cdef["uv_strengths"].append(bits.read(6))
    header["cdef"] = cdef

    restoration_types = [0, 0, 0]
    restoration_units = [8, 8]
    if (
        (not header["all_lossless"] or header["superres_enabled"])
        and sequence["enable_restoration"]
        and not header["allow_intrabc"]
    ):
        restoration_types[0] = bits.read(2)
        if not sequence["monochrome"]:
            restoration_types[1] = bits.read(2)
            restoration_types[2] = bits.read(2)
        if any(restoration_types):
            restoration_units[0] = (
                7 if sequence["use_128x128_superblock"] else 6
            )
            if bits.bit():
                restoration_units[0] += 1
                if not sequence["use_128x128_superblock"]:
                    restoration_units[0] += bits.bit()
            restoration_units[1] = restoration_units[0]
            if (
                (restoration_types[1] or restoration_types[2])
                and sequence["subsampling_x"]
                and sequence["subsampling_y"]
            ):
                restoration_units[1] -= bits.bit()
    header["restoration"] = {
        "types": restoration_types,
        "unit_size_log2": restoration_units,
    }

    header["transform_mode"] = (
        "only_4x4"
        if header["all_lossless"]
        else "select" if bits.bit() else "largest"
    )
    reference_mode_bit = bits.position if frame_type in ("inter", "switch") else None
    header["reference_mode_bit"] = reference_mode_bit
    header["reference_mode"] = (
        "select"
        if frame_type in ("inter", "switch") and bits.bit()
        else "single"
    )
    skip_references = skip_mode_references(sequence, header, references)
    header["skip_mode_references"] = skip_references
    header["skip_mode_bit"] = bits.position if skip_references is not None else None
    header["skip_mode_enabled"] = bool(skip_references is not None and bits.bit())
    header["allow_warped_motion"] = bool(
        not error_resilient
        and frame_type in ("inter", "switch")
        and sequence["enable_warped_motion"]
        and bits.bit()
    )
    header["reduced_transform_set"] = bool(bits.bit())
    header["global_motion"] = read_global_motion(bits, header, references)
    header["film_grain"] = read_film_grain(
        bits, sequence, header, references
    )
    header["header_bits"] = bits.position
    return header, bits


def parse_tile_group_header(
    bits: Bits, header: dict[str, object]
) -> dict[str, object]:
    tiling = header["tiling"]
    tile_count = tiling["columns"] * tiling["rows"]
    if tile_count > 1 and bits.bit():
        width = tiling["log2_columns"] + tiling["log2_rows"]
        start = bits.read(width)
        end = bits.read(width)
    else:
        start = 0
        end = tile_count - 1
    if start > end or end >= tile_count:
        raise ValueError("invalid tile group range")
    bits.align()
    return {"start": start, "end": end, "data_bit": bits.position}


def parse_tile_payloads(
    payload: bytes,
    group: dict[str, object],
    header: dict[str, object],
    physical_spans: list[tuple[int, int]],
    payload_start: int,
) -> None:
    """Split one tile-group payload exactly as dav1d's decode.c does."""
    cursor = group["data_bit"] // 8
    end_tile = group["end"]
    size_width = header["tiling"]["tile_size_bytes"]
    tiles = []
    for tile_index in range(group["start"], end_tile + 1):
        size_field = None
        if tile_index == end_tile:
            tile_size = len(payload) - cursor
        else:
            size_start = cursor
            size_end = size_start + size_width
            if size_end > len(payload):
                raise ValueError("truncated AV1 tile size")
            encoded_size = int.from_bytes(payload[size_start:size_end], "little")
            tile_size = encoded_size + 1
            cursor = size_end
            size_field = {
                "logical_offset": payload_start + size_start,
                "width": size_width,
                "encoded_size_minus_one": encoded_size,
                "physical_spans": logical_spans(
                    physical_spans,
                    payload_start + size_start,
                    payload_start + size_end,
                ),
            }
        tile_end = cursor + tile_size
        if tile_end > len(payload):
            raise ValueError("AV1 tile size exceeds tile-group payload")
        tile = payload[cursor:tile_end]
        tiles.append(
            {
                "index": tile_index,
                "logical_offset": payload_start + cursor,
                "length": tile_size,
                "sha256": hashlib.sha256(tile).hexdigest(),
                "physical_spans": logical_spans(
                    physical_spans,
                    payload_start + cursor,
                    payload_start + tile_end,
                ),
                "size_field": size_field,
            }
        )
        cursor = tile_end
    if cursor != len(payload):
        raise ValueError("unassigned AV1 tile-group bytes")
    group["tiles"] = tiles


class FrameState:
    def __init__(self) -> None:
        self.sequence: dict[str, object] | None = None
        self.references: list[dict[str, object] | None] = [None] * 8
        self.pending: dict[str, object] | None = None
        self.next_tile = 0

    def accept_sequence(self, sequence: dict[str, object]) -> None:
        if self.sequence is not None and self.sequence != sequence:
            raise ValueError("sequence header changed inside coded sequence")
        self.sequence = sequence

    def reference_report(self) -> list[object]:
        return [frame_summary(reference) for reference in self.references]

    def begin_frame(
        self,
        payload: bytes,
        *,
        obu_type: int,
        temporal_id: int,
        spatial_id: int,
    ) -> tuple[dict[str, object], Bits]:
        if self.sequence is None:
            raise ValueError("frame header precedes sequence header")
        if obu_type == 7:
            if self.pending is None:
                raise ValueError("redundant frame header has no original")
            header, bits = parse_frame_header(
                payload,
                self.sequence,
                self.references,
                temporal_id=temporal_id,
                spatial_id=spatial_id,
            )
            if header != self.pending:
                raise ValueError("redundant frame header differs from original")
            return header, bits
        if self.pending is not None:
            raise ValueError("new frame header before previous frame completion")
        header, bits = parse_frame_header(
            payload,
            self.sequence,
            self.references,
            temporal_id=temporal_id,
            spatial_id=spatial_id,
        )
        if obu_type == 6 and header["show_existing_frame"]:
            raise ValueError("show-existing is forbidden in OBU_FRAME")
        self.pending = header
        self.next_tile = 0
        if header["show_existing_frame"]:
            slot = header["existing_frame_idx"]
            reference = self.references[slot]
            assert reference is not None
            if reference["frame_type"] == "key":
                reference["showable_frame"] = False
                self.references = [reference for _ in range(8)]
            self.pending = None
        return header, bits

    def accept_tile_group(self, group: dict[str, int]) -> None:
        if self.pending is None:
            raise ValueError("tile group has no pending frame")
        if group["start"] != self.next_tile:
            raise ValueError("tile groups are not contiguous")
        self.next_tile = group["end"] + 1
        tiling = self.pending["tiling"]
        tile_count = tiling["columns"] * tiling["rows"]
        if self.next_tile == tile_count:
            completed = self.pending
            for slot in range(8):
                if completed["refresh_frame_flags"] & (1 << slot):
                    self.references[slot] = completed
            self.pending = None
            self.next_tile = 0


def parse_obus(
    sample: bytes,
    physical_spans: list[tuple[int, int]],
    state: FrameState | None = None,
) -> list[dict[str, object]]:
    state = state or FrameState()
    result = []
    offset = 0
    while offset < len(sample):
        start = offset
        header = sample[offset]
        offset += 1
        if header & 0x80:
            raise ValueError("nonzero OBU forbidden bit")
        obu_type = (header >> 3) & 0x0F
        has_extension = bool((header >> 2) & 1)
        has_size = bool((header >> 1) & 1)
        if header & 1:
            raise ValueError("nonzero OBU reserved bit")
        temporal_id = spatial_id = 0
        if has_extension:
            if offset >= len(sample):
                raise ValueError("truncated OBU extension")
            extension = sample[offset]
            offset += 1
            temporal_id = extension >> 5
            spatial_id = (extension >> 3) & 3
            if extension & 7:
                raise ValueError("nonzero OBU extension reserved bits")
        if not has_size:
            raise ValueError("AVIF OBU lacks a size field")
        payload_size, offset = read_uleb128(sample, offset)
        payload_start = offset
        payload_end = payload_start + payload_size
        if payload_end > len(sample):
            raise ValueError("OBU payload exceeds sample")
        payload = sample[payload_start:payload_end]
        entry: dict[str, object] = {
            "type": obu_type,
            "name": OBU_NAMES.get(obu_type, "reserved"),
            "has_extension": has_extension,
            "has_size_field": has_size,
            "temporal_id": temporal_id,
            "spatial_id": spatial_id,
            "header_length": payload_start - start,
            "header_spans": logical_spans(physical_spans, start, payload_start),
            "payload_spans": logical_spans(
                physical_spans, payload_start, payload_end
            ),
            "payload_length": payload_size,
            "payload_sha256": hashlib.sha256(payload).hexdigest(),
        }
        if obu_type == 1:
            entry["sequence_header"] = parse_sequence_header(payload)
            entry["sequence_header"]["payload_hex"] = payload.hex()
            state.accept_sequence(entry["sequence_header"])
        elif obu_type in (3, 6, 7):
            references_before = state.reference_report()
            frame_header, bits = state.begin_frame(
                payload,
                obu_type=obu_type,
                temporal_id=temporal_id,
                spatial_id=spatial_id,
            )
            entry["frame_header"] = frame_header
            if obu_type == 6:
                bits.align()
                tile_group = parse_tile_group_header(bits, frame_header)
                parse_tile_payloads(
                    payload,
                    tile_group,
                    frame_header,
                    physical_spans,
                    payload_start,
                )
                state.accept_tile_group(tile_group)
                entry["tile_group"] = tile_group
            else:
                bits.trailing_bits()
            entry["references_before"] = references_before
            entry["references_after"] = state.reference_report()
        elif obu_type == 4:
            if state.pending is None:
                raise ValueError("tile group has no pending frame")
            pending = state.pending
            bits = Bits(payload)
            tile_group = parse_tile_group_header(bits, pending)
            parse_tile_payloads(
                payload,
                tile_group,
                pending,
                physical_spans,
                payload_start,
            )
            state.accept_tile_group(tile_group)
            entry["tile_group"] = tile_group
            entry["references_after"] = state.reference_report()
        elif obu_type == 2 and state.pending is not None:
            raise ValueError("temporal delimiter follows an incomplete frame")
        result.append(entry)
        offset = payload_end
    return result


def sample_report(
    data: bytes,
    *,
    role: str,
    identity: object,
    spans: list[tuple[int, int]],
    state: FrameState | None = None,
) -> dict[str, object]:
    sample = b"".join(data[start:end] for start, end in spans)
    return {
        "role": role,
        "identity": identity,
        "length": len(sample),
        "sha256": hashlib.sha256(sample).hexdigest(),
        "spans": [
            {"offset": start, "length": end - start} for start, end in spans
        ],
        "obus": parse_obus(sample, spans, state),
    }


def inspect(path: Path) -> dict[str, object]:
    container = inspect_avif(path)
    data = path.read_bytes()
    samples = []
    for role in ("color", "alpha"):
        for item in container.get("items", {}).get(role, []):
            spans = [
                (span["offset"], span["offset"] + span["length"])
                for span in item["spans"]
            ]
            samples.append(
                sample_report(
                    data,
                    role=f"item_{role}",
                    identity=item["item_id"],
                    spans=spans,
                    state=FrameState(),
                )
            )
    for track in container.get("tracks", []):
        state = FrameState()
        for sample in track["samples"]:
            start = sample["offset"]
            samples.append(
                sample_report(
                    data,
                    role=f"track_{track['handler']}",
                    identity={
                        "track_id": track["track_id"],
                        "sample": sample["index"],
                    },
                    spans=[(start, start + sample["length"])],
                    state=state,
                )
            )
    return {"file": path.name, "samples": samples}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "paths",
        nargs="*",
        type=Path,
        help="AVIF files; defaults to every committed AVIF fixture",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    paths = args.paths or sorted(DEFAULT_INPUT.glob("*.avif"))
    reports = []
    for path in paths:
        try:
            reports.append(inspect(path))
        except (AssertionError, KeyError, ValueError) as error:
            reports.append({"file": path.name, "error": str(error)})
    print(json.dumps(reports, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
