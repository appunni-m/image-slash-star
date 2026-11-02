#!/usr/bin/env python3
"""Generate input-only RGBA PNGs for WebP alpha effort and filter boundaries."""

from __future__ import annotations

import argparse
import hashlib
import struct
from pathlib import Path

from generate_webp_dimension_limit_fixture import PNG_SIGNATURE, png_chunk, stored_zlib


ROOT = Path(__file__).resolve().parent.parent
SPECS = (
    (
        "thin_nonzero",
        33,
        1,
        "858a5836d8336698a2d4165337c8b2e6be7935c32da3d6b9a063e1f8d081247f",
        "c3cd03a3d95d1e01add194ef7335bdf9a2ba7a7a2f105d3cdddf451b26feb4bd",
    ),
    (
        "combined",
        17,
        19,
        "5c725a339f939b94b7016cad3d0bbf400cfda09c46fdb44dd48c27fd5c498ecd",
        "3bc20a089d3539a69c5d98fd917d7414d392d507780ba65469cfd6d82e31e409",
    ),
    (
        "horizontal",
        17,
        19,
        "4695c2a5962afb62afa3cd380a0ea9cd42f787820477293a6363f3c26f7b47f8",
        "7e8cb835a943512b512c027738e57535c98d8ebb5b9659dc59aa4c7ecc0e5852",
    ),
    (
        "palette16",
        17,
        19,
        "f23e15899522766c869938a0955fef93a18fc36e37d00a04d04519bf473f37cb",
        "5051667e06079ad8eb77a330d1425ec23039c89f1fffd5414185732b25154994",
    ),
    (
        "palette17",
        17,
        19,
        "e1b923f7e8f98d7407576e3122cd029060a4e3fbb3b509f9537cab3d5c66cf37",
        "7cdadeb31092da6723ccbe790cd7b5b55aa9b25d91f80fe36f3940fab8fc7ce9",
    ),
    (
        "random",
        17,
        19,
        "21ed9d17f7c07d7b237aed380c3aca6f911ffb43b2775d6e92d1b41daed1fdb1",
        "be166596257f7f6cc21cda7ab8f0435644df1e0d8c650f74fed7704a28b44998",
    ),
    (
        "vertical",
        17,
        19,
        "44ec172448d76eecadfff6807dea0894b5ad82347e6b5ab4dd7e30ec6172ea80",
        "cf7637501f4b73ef694a70dd68f1cc2f79ea4b65bce227364f92bc122db7c251",
    ),
    (
        "band_zero",
        128,
        128,
        "6d40a1fc8f9bf5d4ce57b3b93d2bc6f978068dfc11ce18f64ef7139a475547cf",
        "3bca0d82b56e977a0e0538d0d52fc9e25bf4f6a9d4ec0896f6a171603341a1ef",
    ),
    (
        "band_nonzero",
        128,
        128,
        "5ae6d65fc32beb90333e4d471693925a96769134d537d010c06f7941f4f2f137",
        "f7421695e2bd2e69027a8086080284b1ebc2e030e97a8053697d292d2ba49ef8",
    ),
    (
        "transparent_gradient",
        33,
        35,
        "d9efb2e1d5038876487d76fab339efe2538d0327121b27b3f7ff80e77c66a186",
        "584385b7c6cb9425cba7d9a526d7a8526fe54f57a94da5936d463f392953f84d",
    ),
)


def generate(name: str, width: int, height: int) -> tuple[bytes, bytes]:
    """Construct source samples and a complete stored-DEFLATE PNG."""
    pixels = bytearray()
    state = 0x12345678
    for y in range(height):
        for x in range(width):
            state = (1664525 * state + 1013904223) & 0xFFFFFFFF
            if name == "transparent_gradient":
                pixels.extend((
                    (11 * x + 3 * y) & 255,
                    (5 * x + 7 * y) & 255,
                    (3 * x + 13 * y) & 255,
                    0 if x == 0 else 128 + (x + y) % 128,
                ))
                continue
            if name in ("band_zero", "band_nonzero"):
                alpha = 128 + (state >> 25)
                if name == "band_zero" and (x + 5 * y) % 127 == 0:
                    alpha = 0
            elif name == "thin_nonzero":
                alpha = 128 + (3 * x) % 128
            elif name == "combined":
                alpha = (17 * x + 31 * y) & 255
            elif name == "vertical":
                alpha = (17 * y) & 255
            elif name == "horizontal":
                alpha = (17 * x) & 255
            elif name == "palette16":
                alpha = 17 * ((x + 3 * y) % 16)
            elif name == "palette17":
                alpha = 15 * ((x + 3 * y) % 17)
            elif name == "random":
                alpha = state >> 24
            else:
                raise ValueError(f"unknown alpha source: {name}")
            pixels.extend((130, 130, 130, alpha))
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
    return bytes(pixels), png


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--check", action="store_true", help="verify without writing")
    action.add_argument("--write", action="store_true", help="write source PNGs")
    args = parser.parse_args()
    for name, width, height, pixels_sha256, source_sha256 in SPECS:
        pixels, png = generate(name, width, height)
        if hashlib.sha256(pixels).hexdigest() != pixels_sha256:
            raise RuntimeError(f"alpha effort source pixels changed: {name}")
        if hashlib.sha256(png).hexdigest() != source_sha256:
            raise RuntimeError(f"alpha effort source PNG changed: {name}")
        path = ROOT / "tests/fixtures/input/images/png" / f"webp_alpha_effort_{name}_{width}x{height}.png"
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
