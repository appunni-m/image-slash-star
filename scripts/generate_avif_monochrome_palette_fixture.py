#!/usr/bin/env python3
"""Generate and verify a small monochrome AV1 palette AVIF witness."""

from __future__ import annotations

import argparse
import hashlib
import struct
import subprocess
import tempfile
import zlib
from pathlib import Path

import yaml
from PIL import Image

from generate_decode_refs import verify_primary_oracle
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
ASSET = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "coverage_monochrome_palette_16x16.avif"
)
EXPECTED_AVIF_SHA256 = "64d2f38fe719b898ad11c4c84e36ef26653daa2596844b2ff8cff782f1a75ff0"
EXPECTED_RGB_SHA256 = "7da6d6dc0cb94021c2425b4fc418377c69c69b03b494a8a9a1080461a8e77e72"
PINNED_AVIFENC_VERSION = "Version: 1.4.1 (aom [enc]:v3.13.2)"
WIDTH = 16
HEIGHT = 16
LEVELS = (0, 85, 170, 255)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    body = kind + payload
    return (
        struct.pack(">I", len(payload))
        + body
        + struct.pack(">I", zlib.crc32(body) & 0xFFFF_FFFF)
    )


def source_png() -> bytes:
    rows = bytearray()
    for y in range(HEIGHT):
        rows.append(0)
        for x in range(WIDTH):
            rows.append(LEVELS[((x // 4) + (y // 4)) % len(LEVELS)])

    header = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 0, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", zlib.compress(bytes(rows), level=9))
        + png_chunk(b"IEND", b"")
    )


def verify_pillow_pixels(path: Path) -> None:
    with Image.open(path) as image:
        image.load()
        pixels = image.convert("RGB").tobytes()
    actual = sha256(pixels)
    if actual != EXPECTED_RGB_SHA256:
        raise RuntimeError(f"Pillow RGB hash mismatch: {actual}")


def verify_monochrome_bitstream(path: Path) -> None:
    report = inspect_obus(path)
    samples = report.get("samples")
    if not isinstance(samples, list) or len(samples) != 1:
        raise RuntimeError("AVIF must contain one color sample")
    headers = [
        obu["sequence_header"]
        for obu in samples[0].get("obus", [])
        if "sequence_header" in obu
    ]
    if len(headers) != 1:
        raise RuntimeError("AVIF must contain one AV1 sequence header")
    header = headers[0]
    expected = {
        "bit_depth": 8,
        "max_width": WIDTH,
        "max_height": HEIGHT,
        "monochrome": True,
        "still_picture": True,
    }
    if any(header.get(key) != value for key, value in expected.items()):
        raise RuntimeError("AV1 sequence header does not match the monochrome witness")


def verify_fixture(path: Path) -> bytes:
    data = path.read_bytes()
    actual = sha256(data)
    if actual != EXPECTED_AVIF_SHA256:
        raise RuntimeError(f"AVIF hash mismatch: {actual}")
    verify_monochrome_bitstream(path)
    verify_pillow_pixels(path)
    return data


def encode(avifenc: Path, source: Path, output: Path) -> None:
    result = subprocess.run(
        [
            str(avifenc),
            "--yuv",
            "400",
            "--qcolor",
            "99",
            "--speed",
            "8",
            "--jobs",
            "1",
            "--tilerowslog2",
            "0",
            "--tilecolslog2",
            "0",
            "--advanced",
            "enable-palette=1",
            str(source),
            str(output),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"pinned avifenc failed with status {result.returncode}:\n{result.stdout}"
        )
    if "Format         : YUV400" not in result.stdout:
        raise RuntimeError("pinned avifenc did not produce a YUV400 image")


def generate(avifenc: Path) -> bytes:
    version = subprocess.run(
        [str(avifenc), "--version"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    ).stdout
    if PINNED_AVIFENC_VERSION not in version:
        raise RuntimeError(f"expected {PINNED_AVIFENC_VERSION!r}, found {version!r}")

    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)

    with tempfile.TemporaryDirectory(prefix="avif-monochrome-palette-") as directory:
        temp = Path(directory)
        source = temp / "source.png"
        first = temp / "first.avif"
        second = temp / "second.avif"
        source.write_bytes(source_png())
        encode(avifenc, source, first)
        encode(avifenc, source, second)
        first_bytes = first.read_bytes()
        if first_bytes != second.read_bytes():
            raise RuntimeError("repeated AVIF encodes are not byte-stable")
        if sha256(first_bytes) != EXPECTED_AVIF_SHA256:
            raise RuntimeError(f"AVIF hash mismatch: {sha256(first_bytes)}")
        verify_monochrome_bitstream(first)
        verify_pillow_pixels(first)
        return first_bytes


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--avifenc",
        type=Path,
        help="pinned avifenc binary; required to generate or re-encode the fixture",
    )
    parser.add_argument("--output", type=Path, default=ASSET)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.check:
        manifest = yaml.safe_load(
            (ROOT / "manifest.yaml").read_text(encoding="utf-8")
        )
        verify_primary_oracle(manifest)
        existing = verify_fixture(args.output)
        if args.avifenc is not None and existing != generate(args.avifenc.resolve()):
            raise RuntimeError(f"fixture differs from deterministic output: {args.output}")
        print(f"verified {args.output.relative_to(ROOT)} ({len(existing)} bytes)")
        return 0

    if args.avifenc is None:
        parser.error("--avifenc is required when generating the fixture")
    data = generate(args.avifenc.resolve())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(data)
    print(f"wrote {args.output.relative_to(ROOT)} ({len(data)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
