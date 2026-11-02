#!/usr/bin/env python3
"""Generate complete AVIF OBU-header compatibility inputs and malformed neighbors."""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, __version__ as PILLOW_VERSION, features

from inspect_avif_bitstreams import children, parse_boxes, parse_iloc, unique_box


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
PIXELS_SHA256 = "f1a2555b1c61036af2bd1d3d125a6a1343993873d9e5d58e3caee79989095dcf"
PILLOW_ORACLE = (
    "12.2.0", "1.4.1", "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
HEADER_OFFSETS = {2: 282, 1: 284, 6: 295}


@dataclass(frozen=True)
class Recipe:
    filename: str
    obu_type: int
    digest: str
    flag_bits: int = 0
    extension: int | None = None
    truncate: bool = False


RECIPES = (
    Recipe("obu_temporal_delimiter_flags_80.avif", 2,
           "8147fbd017c14f9585f9b6952044ea9dc48af88bf51f57b603921ec3759b5096", flag_bits=0x80),
    Recipe("obu_temporal_delimiter_flags_01.avif", 2,
           "de4411ca5e5a65ecc26d1dab6e0597645e30df0aa0dc19f106a8f6137cc2ff0b", flag_bits=0x01),
    Recipe("obu_temporal_delimiter_flags_81.avif", 2,
           "136756c0be9ff3138652273b850cb6f6e72b3602975cb63c0f388c08b1d6d93a", flag_bits=0x81),
    Recipe("obu_sequence_header_flags_80.avif", 1,
           "61e955456ba802ab86d186e52c0693b7426207c5c48b080c14afb6079fd4cd58", flag_bits=0x80),
    Recipe("obu_sequence_header_flags_01.avif", 1,
           "968fcd6da470240cd5ae2cc93a50004f50b1b208ac75bcbdffbff2d8b8489a5a", flag_bits=0x01),
    Recipe("obu_sequence_header_flags_81.avif", 1,
           "783172d0b4f2e80270bc6fb3cbb374e65511f30b9a0f2b2191690cba55cdec1b", flag_bits=0x81),
    Recipe("obu_frame_flags_80.avif", 6,
           "a335592fe32630672ff434715e80bd83e14157b81721acf4b8e4e61ff3e1d612", flag_bits=0x80),
    Recipe("obu_frame_flags_01.avif", 6,
           "b8bb94153fd78c416b606854acf1bd2eaa4c0c7bcb1743b954ace98056f7a401", flag_bits=0x01),
    Recipe("obu_frame_flags_81.avif", 6,
           "bf11e2a66bb6106e856340d819a584b05dd010f2c13a2ac11ce05d32332a14bc", flag_bits=0x81),
    Recipe("obu_temporal_delimiter_extension_00.avif", 2,
           "f18a87bc7ff748c68d2d08e45481aa718560fee9967a1009f961ad8dde6fc5ac", extension=0x00),
    Recipe("obu_temporal_delimiter_extension_01.avif", 2,
           "4bfd823f568b4fa0511dc9f6f80eb36487cf4b0d9b66d8c0805e63a636e451ee", extension=0x01),
    Recipe("obu_temporal_delimiter_extension_02.avif", 2,
           "0abb3dfe5d494eca86417114182df819b391e18c8ee707d5505d9acf2c5efa4f", extension=0x02),
    Recipe("obu_temporal_delimiter_extension_03.avif", 2,
           "b606e710dcb92e6457ecfa8a382a54e580260655466f30d4d087913fee961c7d", extension=0x03),
    Recipe("obu_temporal_delimiter_extension_04.avif", 2,
           "9ebbe5c0c2632be4eb4fbf93372025bd25fe43692e9751311091831b4799eb0d", extension=0x04),
    Recipe("obu_temporal_delimiter_extension_05.avif", 2,
           "c5acd54d079b257548d21a05ec18645599ed351c23c7121ead399fc7ce496a28", extension=0x05),
    Recipe("obu_temporal_delimiter_extension_06.avif", 2,
           "ed5803378e7c8d9064acf88ee8277f08b56be03798980a729b760196ad652285", extension=0x06),
    Recipe("obu_temporal_delimiter_extension_07.avif", 2,
           "75fbba19b4e861bb983dda61b1e9e9750588e81fefd849dbcc8a085ace072eca", extension=0x07),
    Recipe("error_obu_temporal_delimiter_flags_81_truncated_frame.avif", 2,
           "a493783bc4d77e155d31974926680f8226d80f42990737a4a1a1e8efbb0ba94a", flag_bits=0x81, truncate=True),
    Recipe("obu_sequence_header_extension_00.avif", 1,
           "83cac1c53b85b40d16def0cd7e008ef9a5de5e40a8a908bcc5c9b1c87671bd7a", extension=0x00),
    Recipe("obu_sequence_header_extension_01.avif", 1,
           "a83ee945c8da5974d51a4c01b0c8749317c2b19f66c83dbbd40d3c20c129b6eb", extension=0x01),
    Recipe("obu_sequence_header_extension_02.avif", 1,
           "cff0f2c51169f22688e0901b40f018d55e7c7e188d55d9ca62f8be8ef4362da0", extension=0x02),
    Recipe("obu_sequence_header_extension_03.avif", 1,
           "fd19281644117cd0cfb8e9a650c11f94a483c619504cdc31ea3a8fb0f945e3dc", extension=0x03),
    Recipe("obu_sequence_header_extension_04.avif", 1,
           "f86980b1a1ce3e554a0e1cf6f6c1cf4078ba2116bfdb2c99bce3fa779e501b9d", extension=0x04),
    Recipe("obu_sequence_header_extension_05.avif", 1,
           "c3e064d4fd1deb51a6f6fbb95850a335f5cf884d5543572edfa93c0fc7e04edb", extension=0x05),
    Recipe("obu_sequence_header_extension_06.avif", 1,
           "4ac6e51f27806e9e11647543736175ef029547ea593659f7dff812f6576cbec9", extension=0x06),
    Recipe("obu_sequence_header_extension_07.avif", 1,
           "bf2e6d7ecef57ac2ca97d26060ef7614f12152946a68b1fce04cada3d7080179", extension=0x07),
    Recipe("error_obu_sequence_header_flags_81_truncated_frame.avif", 1,
           "a1bb49a2d395c3d9bf8fb414a0b2a317249595e92bd795e3adbdd11997fcb31e", flag_bits=0x81, truncate=True),
    Recipe("obu_frame_extension_00.avif", 6,
           "b10dce5df181dca13e8b00a95d019140dcfd922fb456884500f7366d1f33e56f", extension=0x00),
    Recipe("obu_frame_extension_01.avif", 6,
           "85348b9469faff59a6d137a1ce65a4588dc4531e4a8553c226254ab028d761f9", extension=0x01),
    Recipe("obu_frame_extension_02.avif", 6,
           "161dac932c480226328a52634d725c978cba0ece783a6fdf98886110edbd429e", extension=0x02),
    Recipe("obu_frame_extension_03.avif", 6,
           "f81e27e7cd3a2667ad5ec72c63c8be8c83da669465e2b75590e4313025deb093", extension=0x03),
    Recipe("obu_frame_extension_04.avif", 6,
           "837b6e500606f8af419d640a4ad1f3b29185dfb2df27079dfc3d28f2456982e1", extension=0x04),
    Recipe("obu_frame_extension_05.avif", 6,
           "3fd5220caca1106f32a8096fa4a7ea5165b2d78456622be30922677ef4d56fe2", extension=0x05),
    Recipe("obu_frame_extension_06.avif", 6,
           "ef026718015d04fdeeea61ec07e712710214734b3c0d58ff06e836d53cd979db", extension=0x06),
    Recipe("obu_frame_extension_07.avif", 6,
           "0f088ed053b3ed4851940b1973e1c8869c99c095a61d917c280322af1eb5995f", extension=0x07),
    Recipe("error_obu_frame_flags_81_truncated_frame.avif", 6,
           "64ebe3741e9be3d37df3b698546b89a5cf5bcabb3e3c2b1ca58f77afd372fdf1", flag_bits=0x81, truncate=True),
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def fixture_bytes(source: bytes, recipe: Recipe) -> bytes:
    """Change only the declared OBU header and any necessary container lengths."""
    if sha256(source) != SOURCE_SHA256:
        raise RuntimeError("pinned baseline AVIF changed")
    boxes = parse_boxes(source, 0, len(source))
    meta = unique_box(boxes, b"meta")
    mdat = unique_box(boxes, b"mdat")
    if meta is None or mdat is None:
        raise RuntimeError("pinned source lacks meta or mdat")
    iloc = unique_box(children(source, meta, prefix=4), b"iloc")
    if iloc is None or source[iloc.payload_start:iloc.end] != bytes.fromhex(
        "00000000440000010001000000010000011a00000aeb"
    ):
        raise RuntimeError("pinned single-extent iloc layout changed")
    if mdat.payload_start != 282 or mdat.end != len(source) or parse_iloc(source, iloc, None) != {
        1: ("file", [(mdat.payload_start, mdat.end)])
    }:
        raise RuntimeError("pinned item does not fill the final mdat box")
    offset = HEADER_OFFSETS[recipe.obu_type]
    if source[offset] != (recipe.obu_type << 3) | 2:
        raise RuntimeError("pinned source OBU header changed")
    if recipe.flag_bits & ~0x81 or recipe.extension is not None and not 0 <= recipe.extension <= 7:
        raise RuntimeError("recipe changes a field other than the declared header bits")
    candidate = bytearray(source)
    candidate[offset] |= recipe.flag_bits
    if recipe.extension is not None:
        candidate[offset] |= 4
        candidate[offset + 1:offset + 1] = bytes((recipe.extension,))
    if recipe.truncate:
        del candidate[-1]
    payload_size = len(candidate) - mdat.payload_start
    candidate[mdat.start:mdat.start + 4] = (payload_size + mdat.header_size).to_bytes(4, "big")
    candidate[iloc.end - 4:iloc.end] = payload_size.to_bytes(4, "big")
    result = bytes(candidate)
    updated = unique_box(parse_boxes(result, 0, len(result)), b"mdat")
    if updated is None or parse_iloc(result, iloc, None) != {
        1: ("file", [(updated.payload_start, updated.end)])
    }:
        raise RuntimeError("rewritten extent does not fill the bounded mdat box")
    return result


def verify_pillow(candidate: bytes, truncate: bool) -> None:
    """Observe complete live Pillow pixels or the malformed-frame failure."""
    with Image.open(BytesIO(candidate)) as image:
        image.verify()
    with Image.open(BytesIO(candidate)) as image:
        if (image.format, image.mode, image.size, image.n_frames) != ("AVIF", "RGB", (128, 128), 1):
            raise RuntimeError("header mutation changed the image header observations")
        if truncate:
            try:
                image.load()
            except RuntimeError as error:
                if str(error) != "Failed to decode frame 0: Decoding of color planes failed":
                    raise RuntimeError("truncated frame's Pillow failure changed") from error
            else:
                raise RuntimeError("Pillow accepted the truncated frame")
        else:
            image.load()
            pixels = image.tobytes()
            if len(pixels) != 128 * 128 * 3 or sha256(pixels) != PIXELS_SHA256:
                raise RuntimeError("header mutation changed the complete baseline pixels")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PILLOW_ORACLE:
        raise RuntimeError(f"Pillow oracle differs from its pin: {observed!r}")
    source = (ASSETS / "baseline.avif").read_bytes()
    for recipe in RECIPES:
        candidate = fixture_bytes(source, recipe)
        if sha256(candidate) != recipe.digest:
            raise RuntimeError(f"generated fixture differs from its pin: {recipe.filename}")
        verify_pillow(candidate, recipe.truncate)
        output = ASSETS / recipe.filename
        if args.check:
            if not output.is_file() or output.read_bytes() != candidate:
                raise RuntimeError(f"fixture is missing or differs: {output}")
        elif output.exists():
            if output.read_bytes() != candidate:
                raise RuntimeError(f"refusing to replace differing fixture: {output}")
        else:
            output.write_bytes(candidate)
        print(f"Verified {output.relative_to(ROOT)} ({len(candidate)} bytes)")


if __name__ == "__main__":
    main()
