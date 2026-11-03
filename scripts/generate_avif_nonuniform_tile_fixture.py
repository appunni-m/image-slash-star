#!/usr/bin/env python3
"""Generate and verify a valid lossless AVIF with nonuniform AV1 tile columns."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from PIL import Image
import yaml

import generate_decode_refs as oracle
from inspect_av1_obus import inspect as inspect_av1
from inspect_avif_bitstreams import inspect as inspect_avif

ROOT = Path(__file__).resolve().parent.parent
ASSET = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "portable_lossless_monochrome_nonuniform_tiles_256x256.avif"
)
EXPECTED_ASSET_SHA256 = "fa46345a828f46277562a5324c18066d6129b73b223e36a4214d492d5d6c344f"
EXPECTED_RGB_SHA256 = "b5cc944c73c13c1b613895869a0219be3449dfbe99be281984884be08cfc8293"
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
WIDTH = 256
HEIGHT = 256


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of bytes."""
    return hashlib.sha256(data).hexdigest()


def expected_rgb() -> bytes:
    """Build RGB bytes for the deterministic monochrome encoder input."""
    pixels = bytearray()
    for y in range(HEIGHT):
        for x in range(WIDTH):
            band = ((x // 16) ^ (y // 32)) * 43
            luma = (x * 31 + y * 17 + band + (x * y) // 13) & 255
            pixels.extend((luma, luma, luma))
    return bytes(pixels)


def validate(path: Path) -> None:
    """Check pinned oracle versions, AVIF/AV1 structure, and exact Pillow pixels."""
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    oracle.verify_primary_oracle(manifest)

    from PIL import _avif, __version__ as pillow_version, features

    observed_oracle = (
        pillow_version,
        features.version("avif"),
        _avif.codec_versions(),
    )
    if observed_oracle != PINNED_ORACLE:
        raise ValueError(f"Pillow/AVIF versions differ from pin: {observed_oracle!r}")

    data = path.read_bytes()
    if sha256(data) != EXPECTED_ASSET_SHA256:
        raise ValueError("AVIF bytes differ from the pinned nonuniform-tile fixture")

    container = inspect_avif(path)
    color_items = container["items"]["color"]
    if (
        len(color_items) != 1
        or color_items[0]["type"] != "av01"
        or container["items"]["alpha"]
    ):
        raise ValueError("fixture must contain one monochrome AV1 color item and no alpha")

    report = inspect_av1(path)
    samples = report["samples"]
    if len(samples) != 1 or len(samples[0]["spans"]) != 1:
        raise ValueError("fixture must contain one contiguous AV1 sample")
    sequence_headers = [
        obu["sequence_header"]
        for obu in samples[0]["obus"]
        if "sequence_header" in obu
    ]
    frame_headers = [
        obu["frame_header"]
        for obu in samples[0]["obus"]
        if "frame_header" in obu
    ]
    if len(sequence_headers) != 1 or len(frame_headers) != 1:
        raise ValueError("fixture must contain one sequence header and one frame header")

    sequence = sequence_headers[0]
    frame = frame_headers[0]
    tiling = frame["tiling"]
    if (
        sequence.get("profile") != 0
        or not sequence.get("still_picture")
        or not sequence.get("reduced_still_picture_header")
        or sequence.get("max_width") != WIDTH
        or sequence.get("max_height") != HEIGHT
        or not sequence.get("monochrome")
        or frame.get("frame_type") != "key"
        or frame.get("frame_width") != WIDTH
        or frame.get("frame_height") != HEIGHT
        or not frame.get("all_lossless")
        or tiling.get("uniform") is not False
        or tiling.get("columns") != 2
        or tiling.get("column_starts") != [0, 1, 4]
        or tiling.get("rows") != 1
        or tiling.get("row_starts") != [0, 4]
        or tiling.get("tile_size_bytes") != 2
    ):
        raise ValueError("AV1 headers differ from the pinned nonuniform tile profile")

    expected = expected_rgb()
    if sha256(expected) != EXPECTED_RGB_SHA256:
        raise ValueError("deterministic RGB source differs from its pinned digest")
    with Image.open(path) as image:
        image.load()
        if image.format != "AVIF" or image.mode != "RGB":
            raise ValueError("Pillow decoded the fixture with an unexpected format or mode")
        if image.size != (WIDTH, HEIGHT) or image.tobytes() != expected:
            raise ValueError("Pillow RGB pixels differ from the deterministic lossless source")


def regenerate(encoder: Path) -> None:
    """Accept encoder output only when it exactly matches the pinned asset."""
    if not encoder.is_file():
        raise ValueError(f"encoder does not exist: {encoder}")
    staging = ROOT / "target" / "oracle-staging"
    staging.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="nonuniform-tile-", dir=staging) as temporary:
        output = Path(temporary) / ASSET.name
        subprocess.run([str(encoder), str(output)], check=True)
        validate(output)
        ASSET.write_bytes(output.read_bytes())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, help="regenerate with a pinned fixture encoder")
    parser.add_argument("--check", action="store_true", help="verify the retained fixture")
    args = parser.parse_args()

    try:
        if args.encoder is not None and args.check:
            parser.error("--encoder and --check are mutually exclusive")
        if args.encoder is not None:
            regenerate(args.encoder.resolve())
        validate(ASSET)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"error: {error}\n")

    print(f"Verified nonuniform AV1 tile fixture: {ASSET.relative_to(ROOT)}")
    print(f"AVIF SHA-256: {EXPECTED_ASSET_SHA256}")
    print(f"Pillow RGB SHA-256: {EXPECTED_RGB_SHA256}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
