#!/usr/bin/env python3
"""Generate complete JPEG parity inputs for signed JCOEF writes and refinement."""
from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as pillow_version

ROOT = Path(__file__).resolve().parents[1]
# category, negative amplitude, refine through Al=0, input SHA-256, pixel SHA-256
EXPECTED = {
    'progressive_jcoef_cat15_al12_positive_first': (
        15, False, False,
        '540e7ec7134b075da460b0b6993a214295c253ecc0343c446cfa272be296fc7e',
        '733ab3e3e2e3cd83729cc89eca97a2362c127227496a9d57d39160ee6de1d6e3',
    ),
    'progressive_jcoef_cat15_al12_negative_first': (
        15, True, False,
        '2476cb63f7a9cabf50aac5c85eb722755335f9e55f533ac1812e8ca0e332d6b2',
        'f7d5c667e7c2e0b44c4c5f00b712048e052f4386448b90d36da393584079b847',
    ),
    'progressive_jcoef_cat15_al12_positive_refined': (
        15, False, True,
        '62b4b80f02f38f5270bc4038c5a84e2112af9345ed518248caf2e71ef271da91',
        '15cb30a1731da030e4ad8b3be7059a875caaa0665f190f13469f0a932cf50179',
    ),
    'progressive_jcoef_cat15_al12_negative_refined': (
        15, True, True,
        'ba7d78ffe9482773d98b99ded5fc8d2c60fbb6181080dd297b83ea517e9bafc6',
        '21e351fdbe25b0bb993550eb8ef890c27354ebbf0b49445bd5f5015c38dd6157',
    ),
    'progressive_jcoef_cat1_al12_positive_refined': (
        1, False, True,
        '8c021f5cbfdb147f5b3f822311b35e681d45285c77e6cda83047c265a1abcac2',
        '21e351fdbe25b0bb993550eb8ef890c27354ebbf0b49445bd5f5015c38dd6157',
    ),
    'progressive_jcoef_cat1_al12_negative_refined': (
        1, True, True,
        'c448c7ddaad19fe3b255bcb9b0ade28bfdd6014bb1be28df92e6e53ee2f39695',
        '15cb30a1731da030e4ad8b3be7059a875caaa0665f190f13469f0a932cf50179',
    ),
}

def segment(marker: int, payload: bytes) -> bytes:
    return bytes([0xFF, marker]) + (len(payload) + 2).to_bytes(2, "big") + payload

def entropy(bits: str) -> bytes:
    bits += "1" * ((-len(bits)) % 8)
    packed = bytes(int(bits[i:i + 8], 2) for i in range(0, len(bits), 8))
    return packed.replace(b"\xff", b"\xff\x00")

def amplitude(category: int, negative: bool) -> str:
    return ("0" if negative else "1") * category

def fixture(
    width: int, height: int, category: int, negative: bool,
    first_al: int, refine: bool,
) -> bytes:
    quant = segment(0xdb, bytes([0]) + bytes([1]) * 64)
    frame = segment(
        0xc2,
        bytes([8]) + height.to_bytes(2, "big") + width.to_bytes(2, "big")
        + bytes([1, 1, 0x11, 0]),
    )
    dc = bytes([0, 1]) + bytes(15) + bytes([category])
    ac = bytes([0x10, 0, 2]) + bytes(14) + bytes([category, 0])
    tables = segment(0xc4, dc + ac)
    blocks = ((width + 7) // 8) * ((height + 7) // 8)

    def scan(ss: int, se: int, ah: int, al: int, bits: str) -> bytes:
        header = segment(0xda, bytes([1, 1, 0, ss, se, (ah << 4) | al]))
        return header + entropy(bits)
    first_bits = amplitude(category, negative)
    dc_first = scan(0, 0, 0, first_al, ("0" + first_bits) * blocks)
    ac_first = scan(1, 63, 0, first_al, ("00" + first_bits + "01") * blocks)
    scans = dc_first + ac_first
    if refine:
        # Existing AC coefficient is nonzero after 16-bit storage narrowing.
        value = (-(2 ** category - 1) if negative else 2 ** category - 1) << first_al
        assert value & 0xffff
        for previous in range(first_al, 0, -1):
            scans += scan(0, 0, previous, previous - 1, "1" * blocks)
            scans += scan(1, 63, previous, previous - 1, "011" * blocks)
    return b"\xff\xd8" + quant + frame + tables + scans + b"\xff\xd9"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    args = parser.parse_args()
    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    for name, (category, negative, refine, input_hash, pixel_hash) in EXPECTED.items():
        encoded = fixture(8, 8, category, negative, 12, refine)
        if hashlib.sha256(encoded).hexdigest() != input_hash:
            raise RuntimeError(f"JPEG input bytes changed: {name}")
        with Image.open(BytesIO(encoded)) as image:
            image.verify()
        with Image.open(BytesIO(encoded)) as image:
            image.load()
            if image.size != (8, 8) or image.mode != "L":
                raise RuntimeError(f"Pillow metadata changed: {name}")
            if hashlib.sha256(image.tobytes()).hexdigest() != pixel_hash:
                raise RuntimeError(f"Pillow pixels changed: {name}")
        path = ROOT / "tests/fixtures/input/images/jpeg" / (name + ".jpg")
        if path.exists() and path.read_bytes() != encoded:
            raise RuntimeError(f"refusing to replace different fixture bytes: {name}")
        if args.check:
            if not path.is_file():
                raise RuntimeError(f"fixture is missing: {name}")
        else:
            path.write_bytes(encoded)
        print(f"{'Checked' if args.check else 'Generated'} {name}")


if __name__ == "__main__":
    main()
