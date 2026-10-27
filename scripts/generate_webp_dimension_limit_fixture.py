#!/usr/bin/env python3
"""Generate a one-row RGB PNG just beyond WebP's dimension limit."""

from __future__ import annotations

import argparse
import hashlib
import struct
import sys
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
FIXTURE_SPECS = (
    (
        16_383,
        ROOT / "tests/fixtures/input/images/png/webp_dimension_limit_16383x1.png",
        "dd59e1bb7d408faf6c5061560ba04b66966e63c3da901e9304d86bdf1c6e71d7",
        "30e9bba7e4b84b2f016b8cb76958ff136f74340ea2005a1838c85c1c8faef0df",
    ),
    (
        16_384,
        ROOT / "tests/fixtures/input/images/png/webp_dimension_limit_16384x1.png",
        "1b5f8e7a6f5a58768ffb12d573a1136bc9a2a8ca2fa680c273bb7fb998bb4bf7",
        "d5799faf97d0e3798b1716bb413e0dd76796b85c03dce0b5329690280e31471b",
    ),
    (
        16_385,
        ROOT / "tests/fixtures/input/images/png/webp_dimension_limit_16385x1.png",
        "026068a5ba0e71e99440afda3fdc4bb5df26b99715f69cdf4d2b5657823fc3af",
        "bc4c5e90ed145ef72aa6c5d16e697381a00ab02e8e6c3f5d9c78480db6c23ab9",
    ),
)
PIXEL = bytes((32, 64, 96))
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    """Serialize one PNG chunk with its CRC."""
    checksum = zlib.crc32(kind + payload) & 0xFFFF_FFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def stored_zlib(data: bytes) -> bytes:
    """Wrap deterministic uncompressed DEFLATE blocks in a zlib stream."""
    blocks = []
    for start in range(0, len(data), 0xFFFF):
        block = data[start : start + 0xFFFF]
        final = start + len(block) == len(data)
        header = b"\x01" if final else b"\x00"
        blocks.append(header + struct.pack("<HH", len(block), len(block) ^ 0xFFFF) + block)
    return (
        b"\x78\x01"
        + b"".join(blocks)
        + struct.pack(">I", zlib.adler32(data) & 0xFFFF_FFFF)
    )


def generate_fixture(width: int, expected_pixels_sha256: str, expected_source_sha256: str) -> bytes:
    """Build and verify a valid RGB PNG at one of the WebP width boundaries."""
    pixels = PIXEL * width
    if hashlib.sha256(pixels).hexdigest() != expected_pixels_sha256:
        raise RuntimeError("WebP dimension-limit pixels differ from their pinned digest")
    scanline = b"\x00" + pixels
    header = struct.pack(">IIBBBBB", width, 1, 8, 2, 0, 0, 0)
    fixture = (
        PNG_SIGNATURE
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", stored_zlib(scanline))
        + png_chunk(b"IEND", b"")
    )
    if hashlib.sha256(fixture).hexdigest() != expected_source_sha256:
        raise RuntimeError("WebP dimension-limit PNG differs from its pinned digest")
    return fixture


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group()
    action.add_argument("--check", action="store_true", help="verify without writing")
    action.add_argument("--write", action="store_true", help="write the source PNG")
    arguments = parser.parse_args()
    fixtures = [
        (path, generate_fixture(width, pixels_sha256, source_sha256))
        for width, path, pixels_sha256, source_sha256 in FIXTURE_SPECS
    ]

    if arguments.check:
        for path, fixture in fixtures:
            if not path.is_file() or path.read_bytes() != fixture:
                print(f"stale or missing WebP dimension fixture: {path}", file=sys.stderr)
                return 1
            print(f"Verified {path.relative_to(ROOT)}")
        return 0

    for (width, path, _, source_sha256), (_, fixture) in zip(FIXTURE_SPECS, fixtures):
        if path.exists() and path.read_bytes() != fixture:
            raise RuntimeError(f"refusing to replace a different fixture at {path.relative_to(ROOT)}")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(fixture)
        print(f"Wrote {path.relative_to(ROOT)} ({width}x1, sha256={source_sha256})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
