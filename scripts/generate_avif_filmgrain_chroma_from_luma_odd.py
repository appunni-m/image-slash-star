#!/usr/bin/env python3
"""Generate and check a 65x65 I420 AVIF with chroma-scaled-from-luma grain."""

import argparse
import hashlib
from pathlib import Path

from PIL import Image, _avif, __version__ as pillow_version, features

from generate_avif_filmgrain_chroma_from_luma_420 import (
    ROOT,
    encode_source,
    mutate,
)

SIZE = (65, 65)
SOURCE_SHA256 = "c1d02c2240674aa4f6386f9df4a1e68535998deedaf6b1654ec7fda80d9578e8"
FIXTURE_SHA256 = "5f881e4c6eed5bb4c554004d72ca5e992f6b704bc39672b01d8b3a3236bb2330"
RGB_SHA256 = "d3c620ef7742e8702d1351c33b4ae9386a67fa1a761b6c4b9ea695a6fe08f0c9"
DEFAULT_OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "portable_lossless_filmgrain_420_chroma_from_luma_65x65.avif"
)
DEFAULT_REFERENCE = (
    ROOT
    / "tests"
    / "fixtures"
    / "outputs"
    / "raws"
    / "Decode.avif_portable_lossless_filmgrain_420_chroma_from_luma_65x65_avif.bin"
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of bytes."""

    return hashlib.sha256(data).hexdigest()


def fixture_bytes() -> tuple[bytes, bytes]:
    """Regenerate the odd-size fixture and verify its Pillow reference."""

    if pillow_version != "12.2.0" or features.version("avif") != "1.4.1":
        raise RuntimeError("Pillow 12.2.0 with libavif 1.4.1 is required")
    codecs = _avif.codec_versions()
    required_codecs = ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2")
    if not all(version in codecs for version in required_codecs):
        raise RuntimeError(f"pinned dav1d/libaom codecs are required, found {codecs}")

    source = encode_source(SIZE)
    if sha256(source) != SOURCE_SHA256 or source != encode_source(SIZE):
        raise RuntimeError("pinned odd-size lossless I420 source bytes differ")
    fixture, pixels = mutate(source, SIZE)
    repeated_fixture, repeated_pixels = mutate(encode_source(SIZE), SIZE)
    if fixture != repeated_fixture or sha256(fixture) != FIXTURE_SHA256:
        raise RuntimeError("pinned odd-size chroma-from-luma AVIF bytes differ")
    if pixels != repeated_pixels or sha256(pixels) != RGB_SHA256:
        raise RuntimeError("pinned odd-size Pillow RGB reference differs")
    return fixture, pixels


def main() -> None:
    """Write the pinned fixture or verify the tracked input bytes."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the pinned fixture without rewriting it",
    )
    args = parser.parse_args()

    fixture, pixels = fixture_bytes()
    if args.check:
        if not args.output.exists() or args.output.read_bytes() != fixture:
            raise RuntimeError(
                f"{args.output}: checked AVIF fixture differs or is missing"
            )
        if not DEFAULT_REFERENCE.exists() or DEFAULT_REFERENCE.read_bytes() != pixels:
            raise RuntimeError(
                f"{DEFAULT_REFERENCE}: Pillow reference differs or is missing"
            )
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(fixture)
    print(f"Verified {len(fixture)} bytes with SHA-256 {FIXTURE_SHA256}: {args.output}")


if __name__ == "__main__":
    main()
