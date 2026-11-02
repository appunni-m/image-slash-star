#!/usr/bin/env python3
"""Generate complete JPEG inputs for empty progressive scan and restart parity."""
from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as pillow_version
from generate_jpeg_jcoef_boundary_fixtures import segment
from generate_jpeg_partial_smoothing_fixtures import band_fixture, scan_ranges

ROOT = Path(__file__).resolve().parents[1]
# Fixed complete-input and independently observed pinned Pillow pixel SHA-256.
EXPECTED = {
    "progressive_empty_dc_before_dc_positive": (
        "70bd66d2df1a3970122a26576aa9a1f647ff8fe3bf0d158d43affdccebe197c7",
        "d621cd1bcea4c7150ab610e31f47f8768a603170a436a3ac42ffa1ebb34e0403",
    ),
    "progressive_empty_ac_before_ac_positive": (
        "80e2e448fac95a73dc6f542588336285f9e06b85f474e2f3dc1818aee409f9e6",
        "d621cd1bcea4c7150ab610e31f47f8768a603170a436a3ac42ffa1ebb34e0403",
    ),
    "progressive_empty_dc_before_com_positive": (
        "335d531ce76c3a07f9b3c5dd2c73069bfe8dfe087cb00fb936a73de51dddf6df",
        "012f8688643ec76f1582ed667fa07edfd80918546433e3c241b5a4d41491c640",
    ),
    "progressive_empty_ac_before_com_positive": (
        "11e62dd70a0ebedf6a6ad9535dfd9b5528b71aab8f897cb0372bac8bcb344ca4",
        "d621cd1bcea4c7150ab610e31f47f8768a603170a436a3ac42ffa1ebb34e0403",
    ),
    "progressive_empty_dc_before_eoi_positive": (
        "3ff79b4ac3a0601ad4cdd093d2ce029a2e684d802d75d2f0529a659a9afe224b",
        "012f8688643ec76f1582ed667fa07edfd80918546433e3c241b5a4d41491c640",
    ),
    "progressive_empty_ac_before_eoi_positive": (
        "e695e6aa166aea9612cdc62d1e96e1869995d505b8040449f7ce53d75e7dff6a",
        "d621cd1bcea4c7150ab610e31f47f8768a603170a436a3ac42ffa1ebb34e0403",
    ),
    "progressive_empty_dc_no_dri_before_com_positive": (
        "e10a2708670443302991c97736fe450d9749d6bb1f10a175174e9426cf6e2e3a",
        "012f8688643ec76f1582ed667fa07edfd80918546433e3c241b5a4d41491c640",
    ),
    "progressive_empty_ac_no_dri_before_com_positive": (
        "d51b0f8fbe1df72f0922fbe184c29dc2f6871e52b07d683b8f77dc6405790e9d",
        "d621cd1bcea4c7150ab610e31f47f8768a603170a436a3ac42ffa1ebb34e0403",
    ),
    "progressive_empty_dc_max_dri_before_com_positive": (
        "eb720004bfc5780afe82b2d76a727144e1eed6b69e2afecb2b5f767c51800f45",
        "012f8688643ec76f1582ed667fa07edfd80918546433e3c241b5a4d41491c640",
    ),
    "progressive_empty_dc_max_dri_before_eoi_positive": (
        "b0cb0dad65616915ed49dc94be2822a84865b57b32a63c1ee12835640cccf859",
        "012f8688643ec76f1582ed667fa07edfd80918546433e3c241b5a4d41491c640",
    ),
    "progressive_empty_dc_before_dc_negative": (
        "edda77837b59ec464b7bcfabbf452b37ff88f8053a4fd75f8e977e81aa0bac70",
        "f7c839f570fd58468a5beb14d20f94f13ebbcab390c0fbdb54b16fdfda9a42b8",
    ),
    "progressive_empty_ac_before_ac_negative": (
        "064e955be9b5c9251de6830d62be52d1eaa79390834a253520f6c062c6bb8af0",
        "f7c839f570fd58468a5beb14d20f94f13ebbcab390c0fbdb54b16fdfda9a42b8",
    ),
    "progressive_empty_dc_before_com_negative": (
        "c723f2f12f31aa74d99307365564d83a0b334bf111717d9025cbe7259f8cd28e",
        "39f49b7b760ab89bbc174b767c9613d28d8ed96e4b1c03dc611bd9d8e2957bd9",
    ),
    "progressive_empty_ac_before_com_negative": (
        "946a0830f92614a108e90e62ddf3b0d383a9fd545afad3b030d51ba838b02878",
        "f7c839f570fd58468a5beb14d20f94f13ebbcab390c0fbdb54b16fdfda9a42b8",
    ),
    "progressive_empty_dc_before_eoi_negative": (
        "82a3ac561ad04741b8dd5721ab4635459397459ae75598bd24ab40370e2e8692",
        "39f49b7b760ab89bbc174b767c9613d28d8ed96e4b1c03dc611bd9d8e2957bd9",
    ),
    "progressive_empty_ac_before_eoi_negative": (
        "21115bb53f07373f31ee9cada346546098c08d410fda012ca4bb3f0bfae7c755",
        "f7c839f570fd58468a5beb14d20f94f13ebbcab390c0fbdb54b16fdfda9a42b8",
    ),
    "progressive_empty_dc_no_dri_before_com_negative": (
        "428ac9a8b68189a5cec58d1c6da20678a135d6c8738a7933115f2f351fa576f1",
        "39f49b7b760ab89bbc174b767c9613d28d8ed96e4b1c03dc611bd9d8e2957bd9",
    ),
    "progressive_empty_ac_no_dri_before_com_negative": (
        "7056c7247b76e7f09821329e0f30987491524a31030f54ab4a43acc4a5dc8d4f",
        "f7c839f570fd58468a5beb14d20f94f13ebbcab390c0fbdb54b16fdfda9a42b8",
    ),
    "progressive_empty_dc_max_dri_before_com_negative": (
        "3d4a2ae52f10fcffbd72d42d9e164ea6ab0096a2f588f11d837e64d493dc7915",
        "39f49b7b760ab89bbc174b767c9613d28d8ed96e4b1c03dc611bd9d8e2957bd9",
    ),
    "progressive_empty_dc_max_dri_before_eoi_negative": (
        "da947af0f97c41621865c13714f1e9eda8c4155084294b3da2305143dd08ed20",
        "39f49b7b760ab89bbc174b767c9613d28d8ed96e4b1c03dc611bd9d8e2957bd9",
    ),
}


def fixture_inputs() -> dict[str, bytes]:
    cases = {}
    for negative in (False, True):
        encoded = band_fixture(negative, (1, 5), 2, True, False)
        (dc_start, dc_end), (ac_start, ac_end) = scan_ranges(encoded)
        prefix = encoded[:dc_start]
        dc, ac = encoded[dc_start:dc_end], encoded[ac_start:ac_end]
        dc_header = dc[:2 + int.from_bytes(dc[2:4], "big")]
        ac_header = ac[:2 + int.from_bytes(ac[2:4], "big")]
        dri = segment(0xdd, b"\x00\x01")
        reset = segment(0xdd, b"\x00\x00")
        maximum = segment(0xdd, b"\xff\xff")
        comment = segment(0xfe, b"empty progressive entropy parity")
        eoi = b"\xff\xd9"
        # Every result remains a complete SOI-to-EOI JPEG. Empty headers are
        # followed by actual markers, rather than truncated synthetic prefixes.
        variants = {
            "empty_dc_before_dc": prefix + dri + dc_header + reset + dc + ac + eoi,
            "empty_ac_before_ac": prefix + dc + dri + ac_header + reset + ac + eoi,
            "empty_dc_before_com": prefix + dc + ac + dri + dc_header + comment + eoi,
            "empty_ac_before_com": prefix + dc + ac + dri + ac_header + comment + eoi,
            "empty_dc_before_eoi": prefix + dc + ac + dri + dc_header + eoi,
            "empty_ac_before_eoi": prefix + dc + ac + dri + ac_header + eoi,
            "empty_dc_no_dri_before_com": prefix + dc + ac + dc_header + comment + eoi,
            "empty_ac_no_dri_before_com": prefix + dc + ac + ac_header + comment + eoi,
            "empty_dc_max_dri_before_com": prefix + dc + ac + maximum + dc_header + comment + eoi,
            "empty_dc_max_dri_before_eoi": prefix + dc + ac + maximum + dc_header + eoi,
        }
        sign = "negative" if negative else "positive"
        cases.update((f"progressive_{name}_{sign}", data) for name, data in variants.items())
    return cases


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    args = parser.parse_args()
    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    cases = fixture_inputs()
    if cases.keys() != EXPECTED.keys():
        raise RuntimeError("empty progressive scan inventory changed")
    for name, encoded in cases.items():
        input_hash, pixel_hash = EXPECTED[name]
        if hashlib.sha256(encoded).hexdigest() != input_hash:
            raise RuntimeError(f"JPEG input bytes changed: {name}")
        with Image.open(BytesIO(encoded)) as image:
            image.verify()
        with Image.open(BytesIO(encoded)) as image:
            image.load()
            if image.mode != "L" or image.size != (17, 9):
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
