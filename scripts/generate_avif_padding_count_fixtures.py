#!/usr/bin/env python3
"""Generate complete padding-heavy AVIF inputs and a truncated-frame neighbor."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, __version__ as PILLOW_VERSION, features

from inspect_avif_bitstreams import children, parse_boxes, parse_iloc, unique_box


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
PIXELS_SHA256 = "f1a2555b1c61036af2bd1d3d125a6a1343993873d9e5d58e3caee79989095dcf"
PILLOW_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
RECIPES = (
    ("padding_4096_obus.avif", 4096, False,
     "5ac66c8ff3612f60741940fea4f40887bad6e023a5ed39f0abb5127ca8045a5f"),
    ("padding_4097_obus.avif", 4097, False,
     "24d2a5c917a9ede63c1187ed99d0d1b36db81e24f0823d60b6caaeadbedaac06"),
    ("padding_8192_obus.avif", 8192, False,
     "0c9edab35b60db1383dde144144dacc32692ebf27223ba1e618528650eccecc2"),
    ("error_padding_8192_obus_truncated_frame.avif", 8192, True,
     "06ffe1ba1f22f1584ca8e3bf1aa2cb5f6ffb6706e7c67c458a9c60824923d9ae"),
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def source_obu_count(sample: bytes) -> int:
    """Validate the pinned temporal delimiter, sequence, and frame envelopes."""
    offset = 0
    types = []
    while offset < len(sample):
        header = sample[offset]
        offset += 1
        if header & 0x85 or not header & 2:
            raise RuntimeError("pinned source OBU header changed")
        size = 0
        for index in range(8):
            if offset >= len(sample):
                raise RuntimeError("pinned source OBU size is truncated")
            byte = sample[offset]
            offset += 1
            size |= (byte & 0x7F) << (7 * index)
            if not byte & 0x80:
                break
        else:
            raise RuntimeError("pinned source OBU size exceeds eight bytes")
        types.append((header >> 3) & 0x0F)
        offset += size
        if offset > len(sample):
            raise RuntimeError("pinned source OBU exceeds the sample")
    if types != [2, 1, 6]:
        raise RuntimeError("pinned source OBU order changed")
    return len(types)


def fixture_bytes(source: bytes, total: int, truncate: bool) -> bytes:
    """Prepend empty padding OBUs, preserving every original AV1 payload byte."""
    if sha256(source) != SOURCE_SHA256:
        raise RuntimeError("pinned baseline AVIF changed")
    boxes = parse_boxes(source, 0, len(source))
    meta = unique_box(boxes, b"meta")
    mdat = unique_box(boxes, b"mdat")
    if meta is None or mdat is None:
        raise RuntimeError("pinned source lacks meta or mdat")
    iloc = unique_box(children(source, meta, prefix=4), b"iloc")
    if iloc is None or source[iloc.payload_start : iloc.end] != bytes.fromhex(
        "00000000440000010001000000010000011a00000aeb"
    ):
        raise RuntimeError("pinned single-extent iloc layout changed")
    expected = {1: ("file", [(mdat.payload_start, mdat.end)])}
    if mdat.end != len(source) or parse_iloc(source, iloc, None) != expected:
        raise RuntimeError("pinned item does not fill the final mdat box")
    sample = source[mdat.payload_start : mdat.end]
    count = source_obu_count(sample)
    if total < count:
        raise RuntimeError("requested total is smaller than the original OBU count")
    # OBU_PADDING with a size field of zero consumes exactly two bytes. The
    # three original OBUs remain after all padding, so decoding must traverse
    # it before reaching the original sequence and image data.
    payload = b"\x7a\x00" * (total - count) + sample
    if truncate:
        payload = payload[:-1]
    candidate = bytearray(source[: mdat.payload_start] + payload)
    candidate[mdat.start : mdat.start + 4] = (len(payload) + mdat.header_size).to_bytes(4, "big")
    candidate[iloc.end - 4 : iloc.end] = len(payload).to_bytes(4, "big")
    result = bytes(candidate)
    updated = unique_box(parse_boxes(result, 0, len(result)), b"mdat")
    if updated is None or parse_iloc(result, iloc, None) != {
        1: ("file", [(updated.payload_start, updated.end)])
    }:
        raise RuntimeError("rewritten extent does not fill the bounded mdat box")
    return result


def verify_pillow(candidate: bytes, truncate: bool) -> None:
    """Check exact live Pillow output, including the malformed-frame outcome."""
    with Image.open(BytesIO(candidate)) as image:
        image.verify()
    with Image.open(BytesIO(candidate)) as image:
        if (image.format, image.mode, image.size, image.n_frames) != (
            "AVIF", "RGB", (128, 128), 1
        ):
            raise RuntimeError("padding changed the image header observations")
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
                raise RuntimeError("padding changed the complete baseline pixels")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PILLOW_ORACLE:
        raise RuntimeError(f"Pillow oracle differs from its pin: {observed!r}")
    source = (ASSETS / "baseline.avif").read_bytes()
    for filename, total, truncate, digest in RECIPES:
        candidate = fixture_bytes(source, total, truncate)
        if sha256(candidate) != digest:
            raise RuntimeError(f"generated fixture differs from its pin: {filename}")
        verify_pillow(candidate, truncate)
        output = ASSETS / filename
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
