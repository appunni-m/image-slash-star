#!/usr/bin/env python3
"""Inspect structural VP8L fields from a WebP fixture.

This is intentionally a small, dependency-free specification reader.  It is
not an alternate pixel decoder and it is not used by the Rust parity harness:
the output is structural provenance for the property map.  The parser walks
the VP8L image streams far enough to consume nested Huffman-coded images and
records the transform, cache, entropy-image, tree-form, and distance fields
that Pillow does not expose.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path


class ParseError(Exception):
    """The fixture is not a structurally readable VP8L stream."""


class BitReader:
    def __init__(self, data: bytes):
        self.data = data
        self.position = 0

    def read(self, count: int) -> int:
        if count < 0 or count > 32:
            raise ParseError(f"invalid bit count {count}")
        if self.position + count > len(self.data) * 8:
            raise ParseError("truncated bitstream")
        value = 0
        for offset in range(count):
            bit_position = self.position + offset
            value |= ((self.data[bit_position // 8] >> (bit_position % 8)) & 1) << offset
        self.position += count
        return value


def reverse_bits(value: int, count: int) -> int:
    result = 0
    for offset in range(count):
        result = (result << 1) | ((value >> offset) & 1)
    return result


class HuffmanTree:
    def __init__(self, symbols: dict[tuple[int, int], int], single: int | None = None):
        self.symbols = symbols
        self.single = single
        self.max_length = max((length for length, _ in symbols), default=0)

    @classmethod
    def single_node(cls, symbol: int) -> "HuffmanTree":
        return cls({}, symbol)

    @classmethod
    def two_node(cls, zero: int, one: int) -> "HuffmanTree":
        return cls({(1, 0): zero, (1, 1): one})

    @classmethod
    def from_lengths(cls, lengths: list[int]) -> "HuffmanTree":
        counts = Counter(length for length in lengths if length)
        if not counts:
            raise ParseError("empty Huffman tree")
        if sum(counts.values()) == 1:
            return cls.single_node(next(index for index, length in enumerate(lengths) if length))
        max_length = max(counts)
        if max_length > 15:
            raise ParseError("Huffman code length exceeds 15")

        next_codes = [0] * (max_length + 1)
        current = 0
        for length in range(1, max_length + 1):
            next_codes[length] = current
            current = (current + counts.get(length, 0)) << 1
        if current != 2 << max_length:
            raise ParseError("incomplete Huffman tree")

        symbols: dict[tuple[int, int], int] = {}
        for symbol, length in enumerate(lengths):
            if length == 0:
                continue
            code = next_codes[length]
            next_codes[length] += 1
            key = (length, reverse_bits(code, length))
            if key in symbols:
                raise ParseError("duplicate Huffman code")
            symbols[key] = symbol
        if len(symbols) == 1:
            return cls.single_node(next(iter(symbols.values())))
        return cls(symbols)

    def read(self, reader: BitReader) -> int:
        if self.single is not None:
            return self.single
        code = 0
        for length in range(1, self.max_length + 1):
            code |= reader.read(1) << (length - 1)
            symbol = self.symbols.get((length, code))
            if symbol is not None:
                return symbol
        raise ParseError("Huffman symbol is not in the tree")

    def form(self) -> str:
        return "simple_single" if self.single is not None else "full"


CODE_LENGTH_CODE_ORDER = [17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
DISTANCE_MAP = [
    (0, 1), (1, 0), (1, 1), (-1, 1), (0, 2), (2, 0), (1, 2), (-1, 2),
    (2, 1), (-2, 1), (2, 2), (-2, 2), (0, 3), (3, 0), (1, 3), (-1, 3),
    (3, 1), (-3, 1), (2, 3), (-2, 3), (3, 2), (-3, 2), (0, 4), (4, 0),
    (1, 4), (-1, 4), (4, 1), (-4, 1), (3, 3), (-3, 3), (2, 4), (-2, 4),
    (4, 2), (-4, 2), (0, 5), (3, 4), (-3, 4), (4, 3), (-4, 3), (5, 0),
    (1, 5), (-1, 5), (5, 1), (-5, 1), (2, 5), (-2, 5), (5, 2), (-5, 2),
    (4, 4), (-4, 4), (3, 5), (-3, 5), (5, 3), (-5, 3), (0, 6), (6, 0),
    (1, 6), (-1, 6), (6, 1), (-6, 1), (2, 6), (-2, 6), (6, 2), (-6, 2),
    (4, 5), (-4, 5), (5, 4), (-5, 4), (3, 6), (-3, 6), (6, 3), (-6, 3),
    (0, 7), (7, 0), (1, 7), (-1, 7), (5, 5), (-5, 5), (7, 1), (-7, 1),
    (4, 6), (-4, 6), (6, 4), (-6, 4), (2, 7), (-2, 7), (7, 2), (-7, 2),
    (3, 7), (-3, 7), (7, 3), (-7, 3), (5, 6), (-5, 6), (6, 5), (-6, 5),
    (8, 0), (4, 7), (-4, 7), (7, 4), (-7, 4), (8, 1), (8, 2), (6, 6),
    (-6, 6), (8, 3), (5, 7), (-5, 7), (7, 5), (-7, 5), (8, 4), (6, 7),
    (-6, 7), (7, 6), (-7, 6), (8, 5), (7, 7), (-7, 7), (8, 6), (8, 7),
]


def ceil_div(value: int, divisor: int) -> int:
    return (value + divisor - 1) // divisor


def read_huffman_code_lengths(reader: BitReader, code_length_tree: HuffmanTree, count: int) -> list[int]:
    if reader.read(1):
        length_nbits = 2 + 2 * reader.read(3)
        max_symbol = 2 + reader.read(length_nbits)
        if max_symbol > count:
            raise ParseError("Huffman max symbol exceeds alphabet")
    else:
        max_symbol = count

    lengths = [0] * count
    previous = 8
    symbol = 0
    while symbol < count and max_symbol:
        max_symbol -= 1
        code_length = code_length_tree.read(reader)
        if code_length < 16:
            lengths[symbol] = code_length
            symbol += 1
            if code_length:
                previous = code_length
            continue
        slot = code_length - 16
        if slot not in (0, 1, 2):
            raise ParseError("invalid Huffman repeat code")
        extra_bits = (2, 3, 7)[slot]
        repeat_offset = (3, 3, 11)[slot]
        repeat = reader.read(extra_bits) + repeat_offset
        if symbol + repeat > count:
            raise ParseError("Huffman repeat exceeds alphabet")
        value = previous if slot == 0 else 0
        for _ in range(repeat):
            lengths[symbol] = value
            symbol += 1
    return lengths


def read_huffman_tree(reader: BitReader, alphabet_size: int) -> HuffmanTree:
    if reader.read(1):
        num_symbols = reader.read(1) + 1
        first_8bits = reader.read(1)
        zero = reader.read(1 + 7 * first_8bits)
        if num_symbols == 1:
            return HuffmanTree.single_node(zero)
        return HuffmanTree.two_node(zero, reader.read(8))

    code_length_count = 4 + reader.read(4)
    code_length_lengths = [0] * 19
    for index in range(code_length_count):
        code_length_lengths[CODE_LENGTH_CODE_ORDER[index]] = reader.read(3)
    code_length_tree = HuffmanTree.from_lengths(code_length_lengths)
    lengths = read_huffman_code_lengths(reader, code_length_tree, alphabet_size)
    return HuffmanTree.from_lengths(lengths)


class StreamStats:
    def __init__(self, width: int, height: int, is_argb: bool):
        self.width = width
        self.height = height
        self.is_argb = is_argb
        self.color_cache_bits: int | None = None
        self.meta_huffman_bits: int | None = None
        self.entropy_image_size: tuple[int, int] | None = None
        self.huffman_groups = 1
        self.tree_forms: Counter[str] = Counter()
        self.backrefs = 0
        self.cache_lookups = 0
        self.distance_prefixes: set[int] = set()
        self.plane_codes: set[int] = set()
        self.mapped_distances: set[int] = set()
        self.green_values: set[int] = set()

    def summary(self) -> dict:
        return {
            "width": self.width,
            "height": self.height,
            "is_argb": self.is_argb,
            "color_cache_bits": self.color_cache_bits,
            "meta_huffman_bits": self.meta_huffman_bits,
            "entropy_image_size": list(self.entropy_image_size) if self.entropy_image_size else None,
            "huffman_groups": self.huffman_groups,
            "tree_forms": dict(sorted(self.tree_forms.items())),
            "backrefs": self.backrefs,
            "cache_lookups": self.cache_lookups,
            "distance_prefixes": sorted(self.distance_prefixes),
            "plane_codes": sorted(self.plane_codes),
            "mapped_distances": sorted(self.mapped_distances),
            "green_values": sorted(self.green_values),
        }


class ColorCache:
    def __init__(self, bits: int):
        self.bits = bits
        # VP8L initializes cache entries to zero; a lookup before a matching
        # insertion is a valid structural path and yields the zero entry.
        self.values: list[tuple[int, int, int, int]] = [(0, 0, 0, 0)] * (1 << bits)

    def insert(self, value: tuple[int, int, int, int]) -> None:
        packed = (value[0] << 16) | (value[1] << 8) | value[2] | (value[3] << 24)
        index = ((0x1E35A7BD * packed) & 0xFFFFFFFF) >> (32 - self.bits)
        self.values[index] = value

    def lookup(self, index: int) -> tuple[int, int, int, int]:
        return self.values[index]


class VP8LParser:
    def __init__(self, payload: bytes):
        self.reader = BitReader(payload)
        self.streams: list[StreamStats] = []
        self.transforms: list[dict] = []

    def read_distance(self, prefix: int) -> int:
        if prefix < 4:
            return prefix + 1
        extra_bits = (prefix - 2) >> 1
        offset = (2 + (prefix & 1)) << extra_bits
        return offset + self.reader.read(extra_bits) + 1

    @staticmethod
    def plane_distance(width: int, plane_code: int) -> int:
        if plane_code > 120:
            return plane_code - 120
        x_offset, y_offset = DISTANCE_MAP[plane_code - 1]
        return max(1, x_offset + y_offset * width)

    def read_image_stream(self, width: int, height: int, is_argb: bool) -> list[tuple[int, int, int, int]]:
        stats = StreamStats(width, height, is_argb)
        self.streams.append(stats)
        cache_bits = self.reader.read(1)
        color_cache = ColorCache(self.reader.read(4)) if cache_bits else None
        if color_cache is not None:
            if not 1 <= color_cache.bits <= 11:
                raise ParseError("invalid color-cache width")
            stats.color_cache_bits = color_cache.bits

        entropy_image: list[tuple[int, int, int, int]] = []
        huffman_bits = 0
        huffman_width = 1
        huffman_height = 1
        if is_argb and self.reader.read(1):
            huffman_bits = self.reader.read(3) + 2
            huffman_width = ceil_div(width, 1 << huffman_bits)
            huffman_height = ceil_div(height, 1 << huffman_bits)
            entropy_image = self.read_image_stream(huffman_width, huffman_height, False)
            stats.meta_huffman_bits = huffman_bits
            stats.entropy_image_size = (huffman_width, huffman_height)

        groups = 1
        if entropy_image:
            groups = max(1, max((pixel[0] << 8) | pixel[1] for pixel in entropy_image) + 1)
        stats.huffman_groups = groups
        trees: list[list[HuffmanTree]] = []
        alphabet_sizes = [280 + (1 << color_cache.bits) if color_cache else 280, 256, 256, 256, 40]
        for _ in range(groups):
            group = [read_huffman_tree(self.reader, size) for size in alphabet_sizes]
            trees.append(group)
            stats.tree_forms.update(tree.form() for tree in group)

        output: list[tuple[int, int, int, int]] = []
        values = width * height
        index = 0
        next_block_start = 0
        mask = (1 << huffman_bits) - 1 if huffman_bits else 0xFFFF
        active_trees = trees[0]
        while index < values:
            if index >= next_block_start:
                x = index % width
                y = index // width
                next_block_start = min(x | mask, width - 1) + y * width + 1
                group_index = 0 if huffman_bits == 0 else entropy_image[(y >> huffman_bits) * huffman_width + (x >> huffman_bits)][0] * 256 + entropy_image[(y >> huffman_bits) * huffman_width + (x >> huffman_bits)][1]
                if group_index >= len(trees):
                    raise ParseError("entropy image selects an absent Huffman group")
                active_trees = trees[group_index]
                if all(tree.single is not None for tree in active_trees) and active_trees[0].single < 256:
                    count = values if huffman_bits == 0 else next_block_start - index
                    value = (active_trees[1].single & 255, active_trees[0].single & 255, active_trees[2].single & 255, active_trees[3].single & 255)
                    output.extend([value] * count)
                    if color_cache is not None:
                        color_cache.insert(value)
                    index += count
                    continue

            green = active_trees[0].read(self.reader)
            if green < 256:
                red = active_trees[1].read(self.reader) & 255
                blue = active_trees[2].read(self.reader) & 255
                alpha = active_trees[3].read(self.reader) & 255
                value = (red, green, blue, alpha)
                output.append(value)
                if color_cache is not None:
                    color_cache.insert(value)
                index += 1
            elif green < 280:
                length = self.read_distance(green - 256)
                distance_prefix = active_trees[4].read(self.reader)
                distance = self.read_distance(distance_prefix)
                stats.backrefs += 1
                stats.distance_prefixes.add(distance_prefix)
                plane_code = distance
                stats.plane_codes.add(plane_code)
                distance = self.plane_distance(width, plane_code)
                stats.mapped_distances.add(distance)
                if distance > len(output) or index + length > values:
                    raise ParseError("back-reference is outside the image")
                if distance == 1:
                    for _ in range(length):
                        output.append(output[-distance])
                else:
                    for offset in range(length):
                        output.append(output[index - distance + offset])
                index += length
            else:
                if color_cache is None:
                    raise ParseError("color-cache symbol without a cache")
                cache_index = green - 280
                if cache_index >= len(color_cache.values):
                    raise ParseError("color-cache symbol is outside the cache")
                output.append(color_cache.lookup(cache_index))
                stats.cache_lookups += 1
                index += 1

        stats.green_values.update(pixel[1] for pixel in output)
        return output

    def parse(self) -> dict:
        if self.reader.read(8) != 0x2F:
            raise ParseError("VP8L signature is invalid")
        width = self.reader.read(14) + 1
        height = self.reader.read(14) + 1
        alpha_used = self.reader.read(1)
        version = self.reader.read(3)
        if version != 0:
            raise ParseError("VP8L version is not zero")

        xsize = width
        seen: set[int] = set()
        while self.reader.read(1):
            transform_type = self.reader.read(2)
            if transform_type in seen:
                raise ParseError("duplicate transform")
            seen.add(transform_type)
            if transform_type in (0, 1):
                size_bits = self.reader.read(3) + 2
                block_width = ceil_div(xsize, 1 << size_bits)
                block_height = ceil_div(height, 1 << size_bits)
                self.read_image_stream(block_width, block_height, False)
                self.transforms.append({"type": "predictor" if transform_type == 0 else "color", "size_bits": size_bits})
            elif transform_type == 2:
                self.transforms.append({"type": "subtract_green"})
            elif transform_type == 3:
                table_size = self.reader.read(8) + 1
                self.read_image_stream(table_size, 1, False)
                if table_size <= 2:
                    index_bits = 3
                elif table_size <= 4:
                    index_bits = 2
                elif table_size <= 16:
                    index_bits = 1
                else:
                    index_bits = 0
                xsize = ceil_div(xsize, 1 << index_bits)
                self.transforms.append({"type": "color_indexing", "table_size": table_size, "index_bits": index_bits})
            else:
                raise ParseError("invalid transform type")

        self.read_image_stream(xsize, height, True)
        return {
            "width": width,
            "height": height,
            "alpha_used": bool(alpha_used),
            "version": version,
            "transforms": self.transforms,
            "image_streams": [stream.summary() for stream in self.streams],
            "bit_offset_after_image": self.reader.position,
        }


def vp8l_payload(data: bytes) -> bytes:
    if data[:4] != b"RIFF" or data[8:12] != b"WEBP":
        raise ParseError("not a WebP RIFF file")
    offset = 12
    while offset + 8 <= len(data):
        tag = data[offset : offset + 4]
        size = int.from_bytes(data[offset + 4 : offset + 8], "little")
        start = offset + 8
        end = start + size
        if end > len(data):
            raise ParseError("truncated RIFF chunk")
        if tag == b"VP8L":
            return data[start:end]
        offset = end + (size & 1)
    raise ParseError("WebP has no VP8L chunk")


def inspect_path(path: Path) -> dict:
    payload = vp8l_payload(path.read_bytes())
    if not payload or payload[0] != 0x2F:
        raise ParseError("VP8L signature is missing")
    return VP8LParser(payload).parse()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="+", type=Path)
    args = parser.parse_args()
    result = {}
    for path in args.paths:
        try:
            result[str(path)] = {"status": "ok", "structure": inspect_path(path)}
        except (OSError, ParseError) as error:
            result[str(path)] = {"status": "error", "error": str(error)}
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if all(value["status"] == "ok" for value in result.values()) else 1


if __name__ == "__main__":
    raise SystemExit(main())
