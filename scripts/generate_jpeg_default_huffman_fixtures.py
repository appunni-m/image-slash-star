#!/usr/bin/env python3
"""Generate complete JPEG inputs with missing or supplied Huffman table slots."""

from __future__ import annotations

import argparse
import io
from pathlib import Path

import yaml
from PIL import Image

from generate_decode_refs import encode_params, verify_primary_oracle
from generate_jpeg_grayscale_subsampling_fixtures import source_pixels


ROOT = Path(__file__).resolve().parents[1]
LAYOUTS = (("444", 0x11), ("420", 0x22))
RECIPES = (
    ("missing_dc_0", (0,), 0, 0, False),
    ("missing_ac_0", (1,), 0, 0, False),
    ("missing_both_0", (0, 1), 0, 0, False),
    ("missing_dc_1", (0,), 1, 0, False),
    ("missing_ac_1", (1,), 0, 1, False),
    ("missing_both_1", (0, 1), 1, 1, False),
    ("missing_dc_2", (0,), 2, 0, False),
    ("missing_ac_3", (1,), 0, 3, False),
    ("supplied_luma_slot_1", (), 1, 1, True),
    ("defaults_slot_0_preserve_supplied_slot_1", (), 0, 0, True),
)


def multiscan_late_huffman_default_fixture():
    """Build a complete baseline JPEG that redefines a defaulted table."""

    def segment(marker, payload):
        return (
            b"\xff"
            + bytes((marker,))
            + (len(payload) + 2).to_bytes(2, "big")
            + payload
        )

    # The first component scan has no DHT and therefore uses Pillow's standard
    # default tables. Its entropy is DC category 0, AC EOB, and one-fill.
    quantization = segment(0xDB, bytes((0,)) + bytes((0xFF,)) * 64)
    frame = segment(
        0xC0,
        bytes((8, 0, 8, 0, 8, 3, 1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0)),
    )
    first_scan = segment(0xDA, bytes((1, 1, 0, 0, 63, 0)))
    first_entropy = b"\x2b"

    # The later DC0 table maps code 0 to category 1. The AC0 table maps code
    # 0 to a one-coefficient value and code 10 to EOB; neither table assigns
    # an all-ones code, and the following Cb/Cr scan is encoded with them.
    dc0 = bytes((0, 1)) + bytes(15) + bytes((1,))
    ac0 = bytes((0x10, 1, 1)) + bytes(14) + bytes((1, 0))
    later_tables = segment(0xC4, dc0 + ac0)
    chroma_scan = segment(0xDA, bytes((2, 2, 0, 3, 0, 0, 63, 0)))
    chroma_entropy = b"\x66"

    return (
        b"\xff\xd8"
        + quantization
        + frame
        + first_scan
        + first_entropy
        + later_tables
        + chroma_scan
        + chroma_entropy
        + b"\xff\xd9"
    )


def split_header(source, sampling):
    """Boundedly locate the sole scan without interpreting entropy as markers."""
    if not source.startswith(b"\xff\xd8") or not source.endswith(b"\xff\xd9"):
        raise RuntimeError("complete Pillow JPEG required")
    segments = []
    position = 2
    frame_seen = False
    tables = set()
    while position + 4 <= len(source):
        if source[position] != 255:
            raise RuntimeError("unexpected Pillow JPEG marker boundary")
        marker = source[position + 1]
        length = int.from_bytes(source[position + 2:position + 4], "big")
        end = position + 2 + length
        if length < 2 or end > len(source) - 2:
            raise RuntimeError("incomplete Pillow JPEG header segment")
        segment = source[position:end]
        if marker == 0xC0:
            if frame_seen or segment[4:] != bytes((8, 0, 9, 0, 17, 1, 1, sampling, 0)):
                raise RuntimeError("Pillow grayscale baseline layout changed")
            frame_seen = True
        elif marker == 0xC4:
            if len(segment) < 21 or segment[4] not in (0, 0x10):
                raise RuntimeError("Pillow grayscale Huffman table layout changed")
            table_class = segment[4] >> 4
            if table_class in tables or len(segment) != 21 + sum(segment[5:21]):
                raise RuntimeError("expected one complete table per DHT segment")
            tables.add(table_class)
        elif marker == 0xDA:
            if not frame_seen or tables != {0, 1} or segment[4:] != b"\x01\x01\x00\x00\x3f\x00":
                raise RuntimeError("Pillow grayscale scan layout changed")
            # Pillow emits one baseline scan and no post-entropy definitions.
            # Byte stuffing and restart markers cannot contain an unescaped EOI.
            if b"\xff\xd9" in source[end:-2]:
                raise RuntimeError("unexpected embedded EOI in complete source")
            return segments, segment, source[end:]
        segments.append(segment)
        position = end
    raise RuntimeError("Pillow JPEG has no complete scan header")


def mutate(source, sampling, removed, dc_slot, ac_slot, move_tables):
    segments, scan, entropy_tail = split_header(source, sampling)
    result = bytearray(b"\xff\xd8")
    for segment in segments:
        if segment[:2] == b"\xff\xc4":
            table_class = segment[4] >> 4
            if table_class in removed:
                continue
            if move_tables:
                segment = segment[:4] + bytes((segment[4] | 1,)) + segment[5:]
        result.extend(segment)
    scan = scan[:6] + bytes(((dc_slot << 4) | ac_slot,)) + scan[7:]
    result.extend(scan)
    result.extend(entropy_tail)
    return bytes(result)


def generate(output, check):
    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    expected = {}
    for layout, sampling in LAYOUTS:
        with Image.frombytes("L", (17, 9), source_pixels(17, 9)) as image:
            buffer = io.BytesIO()
            image.save(buffer, format="JPEG", **encode_params("jpeg", {
                "quality": 85, "progressive": False, "optimize": False,
                "subsampling": layout,
            }))
        source = buffer.getvalue()
        for name, removed, dc_slot, ac_slot, move_tables in RECIPES:
            relative = Path("jpeg") / f"grayscale_huffman_{layout}_{name}_17x9.jpg"
            expected[relative] = mutate(source, sampling, removed, dc_slot, ac_slot, move_tables)
    expected[Path("jpeg") / "baseline_444_multiscan_late_dht_after_default_scan.jpg"] = (
        multiscan_late_huffman_default_fixture()
    )
    for relative, data in expected.items():
        destination = output / relative
        if check:
            if not destination.is_file() or destination.read_bytes() != data:
                raise RuntimeError("complete generated input differs: " + str(destination))
        else:
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
    print(f"{'Checked' if check else 'Generated'} {len(expected)} complete JPEG inputs")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "tests/fixtures/input/images")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    generate(args.output.resolve(), args.check)


if __name__ == "__main__":
    main()
