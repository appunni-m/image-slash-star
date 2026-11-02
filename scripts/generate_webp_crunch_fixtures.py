#!/usr/bin/env python3
"""Generate input-only PNGs for WebP transform search and palette boundaries."""

from __future__ import annotations

import argparse
import hashlib
import struct
from pathlib import Path

from generate_webp_dimension_limit_fixture import PNG_SIGNATURE, png_chunk, stored_zlib

ROOT = Path(__file__).resolve().parent.parent
SPECS = (
    (
        "late_plane_transition",
        259,
        259,
        "97abf3110340ce827e012104ffde2f14fdcd47eb6ea8697a1f58f2f9f8bfaac7",
        "91699e7206b5525246a4bbcd2b87abced8c8f8e8c5048c7b110a969f7ccd01fd",
    ),
    (
        "zero_planes",
        33,
        35,
        "3d729c93219a7ba4bfd54b8fabcf61e98d3bb5f510acca8a8798bc54b2f7a16a",
        "4e8931d23066560d050d93c1747077ca842ea4ea74b6fad11a242498bf35c153",
    ),
    (
        "zero_plane_transition",
        33,
        35,
        "a33878fc40d3f01a7bbc944035b19d3cd9f48ad0347c29b18b802c45ffde6628",
        "0068578a4eba2822ec0bbebe78c377ee70843acf02edc6f651648d2129cc98e8",
    ),
    (
        "palette_chain16",
        33,
        35,
        "0a8135e28232ecfb6c4d1481582bd6629f18262e508c3a83950fc3021eac7b6a",
        "5044b36010048450f53cf05e0538cb7bcd393363d284abd8553acb329867b173",
    ),
    (
        "palette_chain17",
        33,
        35,
        "b05cdb9da44d6448b31b955be6aaad8918b60fcf3ae8c314c3b9ebab6931bea5",
        "dade38f38fe83d0ba5ae0ad2f3a4c02baed4dfb239124c5413c0ac8d6655bae0",
    ),
    (
        "opaque_single",
        1,
        1,
        "16b04257d756b40660895acb074066f9c55e78e914d45a8e5967c28274cfb374",
        "073992e3038438fa4383bb008a90e883b0cd18a97c2f5113fff580448d208c82",
    ),
    (
        "opaque_pair",
        1,
        2,
        "4c84614e540789c90e3d75e9cfa3a95fddec702d7c26926567e95697f3eff44c",
        "c7554919206ae25d8660e4ad057352c09c5de03ed85d412f02aa2bfd1bb08d34",
    ),
    (
        "alpha_column",
        1,
        33,
        "858a5836d8336698a2d4165337c8b2e6be7935c32da3d6b9a063e1f8d081247f",
        "e7d6134981548f50c55ba5a8f09182a0313fe56586cd9dabd0457a7dbb32e32a",
    ),
)


def generate(name: str, width: int, height: int) -> tuple[bytes, bytes]:
    """Build source samples without consulting any codec output."""
    if name == "late_plane_transition":
        pixels = bytes(
            value
            for y in range(height)
            for x in range(width)
            for value in (
                0,
                (x + y) & 255,
                (17 * x + 29 * y) & 255 if y >= 257 else 0,
                255,
            )
        )
    elif name in {"zero_planes", "zero_plane_transition"}:
        pixels = bytes(
            value
            for y in range(height)
            for x in range(width)
            for value in (
                193 if name == "zero_plane_transition" and x < 16 and y < 16 else 0,
                (17 * x + 31 * y) & 255,
                71 if name == "zero_plane_transition" and x >= 16 and y < 16 else 0,
                255,
            )
        )
    elif name == "alpha_column":
        pixels = bytes(
            value
            for y in range(height)
            for _ in range(width)
            for value in (130, 130, 130, 128 + (3 * y) % 128)
        )
    else:
        colors, seed = {
            "palette_chain16": (16, 7),
            "palette_chain17": (17, 1),
            "opaque_single": (1, 7),
            "opaque_pair": (2, 7),
        }[name]
        state = seed
        palette = []
        for _ in range(colors):
            state = (1664525 * state + 1013904223) & 0xFFFFFFFF
            palette.append((state >> 24, (state >> 16) & 255, (state >> 8) & 255, 255))
        pixels = bytes(
            value
            for y in range(height)
            for x in range(width)
            for value in palette[(x // 3 + y // 2) % colors if colors > 2 else y % colors]
        )
    row_bytes = width * 4
    scanlines = b"".join(
        b"\x00" + pixels[y * row_bytes : (y + 1) * row_bytes]
        for y in range(height)
    )
    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    png = (
        PNG_SIGNATURE
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", stored_zlib(scanlines))
        + png_chunk(b"IEND", b"")
    )
    return pixels, png


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--check", action="store_true", help="verify without writing")
    action.add_argument("--write", action="store_true", help="write source PNGs")
    args = parser.parse_args()
    for name, width, height, pixels_sha256, source_sha256 in SPECS:
        pixels, png = generate(name, width, height)
        if hashlib.sha256(pixels).hexdigest() != pixels_sha256:
            raise RuntimeError(f"crunch source pixels changed: {name}")
        if hashlib.sha256(png).hexdigest() != source_sha256:
            raise RuntimeError(f"crunch source PNG changed: {name}")
        path = ROOT / "tests/fixtures/input/images/png" / f"webp_crunch_{name}_{width}x{height}.png"
        if args.check:
            if not path.is_file() or path.read_bytes() != png:
                raise RuntimeError(f"stale or missing fixture: {path.relative_to(ROOT)}")
        else:
            if path.exists() and path.read_bytes() != png:
                raise RuntimeError(f"refusing to replace a different fixture: {path}")
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(png)
        print(f"{'Verified' if args.check else 'Wrote'} {path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
