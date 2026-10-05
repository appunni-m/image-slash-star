#!/usr/bin/env python3
"""Generate and validate the pinned monochrome AV1 film-grain fixture."""

from __future__ import annotations

import argparse
import hashlib
import random
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features, __version__ as pillow_version

from inspect_av1_obus import inspect as inspect_avif


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "portable_lossless_filmgrain_monochrome_64x64.avif"
)
OUTPUT_SHA256 = "0bc3fe81f320d7f55853d53ec7b7fa20f099bf8af7e5e7ccbaa69556ccd980d4"
RGB_SHA256 = "853bfb557b4ab4960d708f4ecfeda145ed9feab8c987214d82ee6a20665e93f4"
SIZE = (64, 64)
ADVANCED = {
    "color-primaries": "1",
    "transfer-characteristics": "13",
    "matrix-coefficients": "6",
    "min-partition-size": "64",
    "max-partition-size": "64",
    "use-intra-dct-only": "1",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "aq-mode": "0",
    "deltaq-mode": "0",
    "film-grain-test": "1",
}


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of bytes."""

    return hashlib.sha256(data).hexdigest()


def source_pixels() -> bytes:
    """Return seeded grayscale RGB pixels for the fixed 64x64 fixture."""

    generator = random.Random(0x21109)
    return bytes(
        channel
        for _ in range(SIZE[0] * SIZE[1])
        for channel in (generator.randrange(256),) * 3
    )


def encode(pixels: bytes) -> bytes:
    """Encode the fixture with the pinned Pillow/libavif/libaom settings."""

    output = BytesIO()
    Image.frombytes("RGB", SIZE, pixels).save(
        output,
        format="AVIF",
        quality=100,
        speed=0,
        max_threads=1,
        subsampling="4:0:0",
        autotiling=False,
        codec="aom",
        advanced=ADVANCED,
    )
    return output.getvalue()


def validate_oracle(data: bytes) -> None:
    """Require the pinned encoder output to carry lossless monochrome grain."""

    with tempfile.TemporaryDirectory(prefix="avif-monochrome-filmgrain-") as name:
        path = Path(name) / "fixture.avif"
        path.write_bytes(data)
        report = inspect_avif(path)
    samples = report["samples"]
    color = next(sample for sample in samples if sample["role"] == "item_color")
    sequence = next(
        obu["sequence_header"]
        for obu in color["obus"]
        if "sequence_header" in obu
    )
    frame = next(
        obu["frame_header"]
        for obu in color["obus"]
        if obu.get("frame_header") is not None
    )
    grain = frame["film_grain"]
    if (
        sequence["monochrome"] is not True
        or sequence["film_grain_present"] is not True
        or (frame["frame_width"], frame["frame_height"]) != SIZE
        or frame["all_lossless"] is not True
        or grain is None
        or not grain["y_points"]
        or any(grain["uv_points"])
    ):
        raise RuntimeError("encoded AV1 item lacks the expected monochrome grain syntax")

    with Image.open(BytesIO(data)) as decoded:
        decoded.load()
        rgb = decoded.convert("RGB").tobytes()
    if sha256(rgb) != RGB_SHA256:
        raise RuntimeError("pinned Pillow RGB reference differs")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()

    if pillow_version != "12.2.0" or features.version("avif") != "1.4.1":
        raise RuntimeError("Pillow 12.2.0 with libavif 1.4.1 is required")
    codecs = _avif.codec_versions()
    if not all(version in codecs for version in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2")):
        raise RuntimeError(f"pinned dav1d/libaom codecs are required, found {codecs}")

    pixels = source_pixels()
    first = encode(pixels)
    second = encode(pixels)
    if first != second or sha256(first) != OUTPUT_SHA256:
        raise RuntimeError("monochrome film-grain AVIF bytes differ from the pinned fixture")
    validate_oracle(first)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(first)
    print(f"Wrote {len(first)} bytes with SHA-256 {OUTPUT_SHA256}: {args.output}")


if __name__ == "__main__":
    main()
