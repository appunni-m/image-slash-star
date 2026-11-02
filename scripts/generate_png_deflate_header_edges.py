#!/usr/bin/env python3
"""Generate complete PNGs with invalid zlib and DEFLATE headers.

The pinned Pillow oracle accepts inspection and CRC verification for these
inputs, then rejects pixel decoding with its observed stream errors.
"""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path
import struct
import zlib

from generate_png_idat_split_zlib_header_fixture import (
    EXPECTED_SOURCE_SHA256,
    PINNED_IHDR,
    PNG_SIGNATURE,
    SOURCE,
    png_chunk,
    read_chunks,
)
from generate_test_assets import malformed_dynamic_zlib


ROOT = Path(__file__).resolve().parent.parent
OUTPUT_DIRECTORY = ROOT / "tests/fixtures/input/images/png"
FIXTURE_SHA256 = {
    "zlib_window_size_above_limit.png":
        "85f3908b8187264634018450f6a3f7b75b82e62fee6f073b01141d78607f731b",
    "zlib_dynamic_repeat_without_previous.png":
        "499e28c39f5b6528ee9af60f0b4d953d7cbd14f4f7e3927074982114ba96c044",
    "zlib_header_checksum_mismatch.png":
        "e19795bb1425702730cfa038b17e2a2ed52556b63e3d01ceb054942aa2c0fb0c",
    "zlib_preset_dictionary.png":
        "8bd1ca9a90c24d35a1048cc4fbfc90c0e397cbe42c1105370268c31699d84969",
}
PILLOW_ERROR = "broken data stream when reading image file"


def build_fixtures(source: bytes) -> dict[str, bytes]:
    """Replace IDAT streams while retaining the pinned image header and IEND."""
    if sha256(source).hexdigest() != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("1x1 PNG source differs from its pinned hash")
    chunks = read_chunks(source)
    if (
        [kind for kind, _ in chunks] != [b"IHDR", b"IDAT", b"IEND"]
        or chunks[0][1] != PINNED_IHDR
    ):
        raise RuntimeError("source must contain the pinned one-pixel RGB layout")

    # CINFO=8 requests a 64 KiB window, exceeding zlib's 32 KiB limit. Keep
    # CM=8, FDICT=0 and a valid FCHECK so only the window field is invalid.
    cmf = 0x88
    flg = (-cmf * 256) % 31
    original = chunks[1][1]
    invalid_window = bytes((cmf, flg)) + original[2:]
    invalid_checksum = original[:1] + bytes((original[1] ^ 1,)) + original[2:]

    # A complete, valid zlib dictionary stream is forbidden inside PNG.
    # The original literal-only payload does not refer to the dictionary.
    dictionary_id = zlib.adler32(b"png dictionary")
    dictionary_flg = (original[1] & 0xC0) | 0x20
    dictionary_flg += (-(original[0] * 256 + dictionary_flg)) % 31
    preset_dictionary = (
        bytes((original[0], dictionary_flg))
        + struct.pack(">I", dictionary_id)
        + original[2:]
    )
    inflater = zlib.decompressobj(zdict=b"png dictionary")
    restored = inflater.decompress(preset_dictionary) + inflater.flush()
    if not inflater.eof or inflater.unused_data or restored != zlib.decompress(original):
        raise RuntimeError("dictionary stream must retain the complete source raster")

    # In CODE_LENGTH_ORDER, symbols 16 and 0 have length one. Their complete
    # canonical tree assigns symbol 16 the one-bit code 1. Emitting it first
    # requests three copies before any previous code length exists.
    invalid_repeat = malformed_dynamic_zlib([1, 0, 0, 1], [(1, 1), (0, 2)])
    streams = {
        "zlib_window_size_above_limit.png": invalid_window,
        "zlib_dynamic_repeat_without_previous.png": invalid_repeat,
        "zlib_header_checksum_mismatch.png": invalid_checksum,
        "zlib_preset_dictionary.png": preset_dictionary,
    }
    fixtures = {}
    for name, compressed in streams.items():
        encoded = PNG_SIGNATURE + b"".join(
            png_chunk(kind, compressed if kind == b"IDAT" else payload)
            for kind, payload in chunks
        )
        read_chunks(encoded)
        if sha256(encoded).hexdigest() != FIXTURE_SHA256[name]:
            raise RuntimeError(f"generated PNG differs from its pinned hash: {name}")
        fixtures[name] = encoded
    return fixtures


def verify_pillow(fixtures: dict[str, bytes]) -> None:
    """Verify successful metadata/CRC checks and failed pixel materialization."""
    import yaml
    from PIL import Image

    from generate_decode_refs import verify_primary_oracle

    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    for name, encoded in fixtures.items():
        expected_error = (
            "image file is truncated (0 bytes not processed)"
            if name == "zlib_preset_dictionary.png"
            else PILLOW_ERROR
        )
        with Image.open(BytesIO(encoded)) as image:
            if (image.format, image.mode, image.size, image.n_frames) != (
                "PNG", "RGB", (1, 1), 1
            ):
                raise RuntimeError(f"Pillow PNG metadata changed: {name}")
            image.verify()
        for operation in ("decode", "decode_sequence"):
            try:
                with Image.open(BytesIO(encoded)) as image:
                    if operation == "decode_sequence":
                        image.seek(0)
                    image.load()
                    image.tobytes()
            except OSError as error:
                if str(error) != expected_error:
                    raise RuntimeError(f"Pillow {operation} error changed: {name}") from error
            else:
                raise RuntimeError(f"Pillow unexpectedly decoded malformed IDAT: {name}")


def main() -> None:
    """Generate fixtures or check existing bytes without rewriting them."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--verify-pillow", action="store_true")
    args = parser.parse_args()
    fixtures = build_fixtures(SOURCE.read_bytes())
    if args.verify_pillow or not args.check:
        verify_pillow(fixtures)
    for name, encoded in fixtures.items():
        path = OUTPUT_DIRECTORY / name
        if args.check:
            if not path.is_file() or path.read_bytes() != encoded:
                raise RuntimeError(f"PNG fixture is missing or stale: {path}")
        elif path.exists() and path.read_bytes() != encoded:
            raise RuntimeError(f"refusing to replace a different PNG fixture: {path}")
        elif not path.exists():
            path.write_bytes(encoded)
        print(f"Verified {path.relative_to(ROOT)} ({len(encoded)} bytes)")


if __name__ == "__main__":
    main()
