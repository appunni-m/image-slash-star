#!/usr/bin/env python3
"""Generate YCCK JPEG parity inputs with even-sized upsampled output edges."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from hashlib import sha256
from io import BytesIO
from pathlib import Path

import PIL
from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
ASSET_DIR = ROOT / "tests/fixtures/input/images/jpeg"
SOF0 = b"\xff\xc0"


@dataclass(frozen=True)
class Fixture:
    """A complete SOF-dimension mutation of a pinned libjpeg-turbo input."""

    source_name: str
    source_sha256: str
    output_name: str
    width: int
    height: int
    output_sha256: str
    pillow_sha256: str


FIXTURES = (
    Fixture(
        source_name="baseline_ycck_h2v1_17x9.jpg",
        source_sha256="220828c276f262ee48c0bc8c1829e14bdad6eb11b36959b2cc1bfb2e8564aa88",
        output_name="baseline_ycck_h2v1_even_width_18x9.jpg",
        width=18,
        height=9,
        output_sha256="3aa91b3831f8b82d66d978cc217ac4a00953c5b40e9c51d46eb93fdad9feb922",
        pillow_sha256="cfd5480dc9878c522f1fad89048331ff773c45d871a893223d743d3b909c9f45",
    ),
    Fixture(
        source_name="baseline_ycck_subsampled_17x9.jpg",
        source_sha256="39d712b61d2abe430c9767a1630a4f76dd8757d2b1357cec8524af4f414c23b7",
        output_name="baseline_ycck_subsampled_even_edges_18x10.jpg",
        width=18,
        height=10,
        output_sha256="ad0e70ac2a81695d5a41379d8b9931d1333330514b0a1a930c0e835b9aa5a8c8",
        pillow_sha256="e580c178a9dee525ef95849529c3f4ca0a7e1238cc77027d4133732fe78ea2a6",
    ),
)


def generate_fixture(fixture: Fixture) -> bytes:
    """Expose padded MCU samples by changing only dimensions in a full JPEG."""
    source = (ASSET_DIR / fixture.source_name).read_bytes()
    source_sha256 = sha256(source).hexdigest()
    if source_sha256 != fixture.source_sha256:
        raise ValueError(
            f"{fixture.source_name} SHA-256 mismatch: expected "
            f"{fixture.source_sha256}, found {source_sha256}"
        )
    if not source.startswith(b"\xff\xd8") or not source.endswith(b"\xff\xd9"):
        raise ValueError(f"{fixture.source_name} must be a complete JPEG")

    sof_offset = source.find(SOF0)
    if sof_offset < 0 or source.find(SOF0, sof_offset + len(SOF0)) >= 0:
        raise ValueError(f"{fixture.source_name} must contain one baseline SOF0")
    if source[sof_offset + 5 : sof_offset + 7] != b"\x00\x09":
        raise ValueError(f"{fixture.source_name} must be 9 pixels high")
    if source[sof_offset + 7 : sof_offset + 9] != b"\x00\x11":
        raise ValueError(f"{fixture.source_name} must be 17 pixels wide")
    if source[sof_offset + 9] != 4:
        raise ValueError(f"{fixture.source_name} must have four components")

    candidate = bytearray(source)
    candidate[sof_offset + 5 : sof_offset + 7] = fixture.height.to_bytes(2, "big")
    candidate[sof_offset + 7 : sof_offset + 9] = fixture.width.to_bytes(2, "big")
    encoded = bytes(candidate)
    actual_sha256 = sha256(encoded).hexdigest()
    if actual_sha256 != fixture.output_sha256:
        raise ValueError(
            f"{fixture.output_name} SHA-256 mismatch: expected "
            f"{fixture.output_sha256}, found {actual_sha256}"
        )
    return encoded


def verify_pillow_reference(encoded: bytes, fixture: Fixture) -> None:
    """Confirm Pillow accepts the full-file dimension mutation and exact pixels."""
    if PIL.__version__ != "12.2.0":
        raise ValueError(f"Pillow 12.2.0 is required, found {PIL.__version__}")

    with Image.open(BytesIO(encoded)) as image:
        image.verify()
    with Image.open(BytesIO(encoded)) as image:
        image.load()
        if (
            image.format != "JPEG"
            or image.mode != "CMYK"
            or image.size != (fixture.width, fixture.height)
            or image.info.get("adobe_transform") != 2
        ):
            raise ValueError(
                f"{fixture.output_name} has unexpected Pillow metadata: "
                f"format={image.format!r}, mode={image.mode!r}, size={image.size!r}, "
                f"Adobe transform={image.info.get('adobe_transform')!r}"
            )
        actual_pixels_sha256 = sha256(image.tobytes()).hexdigest()
    if actual_pixels_sha256 != fixture.pillow_sha256:
        raise ValueError(
            f"{fixture.output_name} Pillow pixel SHA-256 mismatch: expected "
            f"{fixture.pillow_sha256}, found {actual_pixels_sha256}"
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="verify committed fixtures")
    mode.add_argument("--generate", action="store_true", help="write deterministic fixtures")
    args = parser.parse_args()

    for fixture in FIXTURES:
        encoded = generate_fixture(fixture)
        verify_pillow_reference(encoded, fixture)
        output = ASSET_DIR / fixture.output_name
        if args.check:
            if not output.is_file() or output.read_bytes() != encoded:
                raise ValueError(f"{output.relative_to(ROOT)} differs from generated bytes")
        else:
            output.write_bytes(encoded)
        print(f"verified {output.relative_to(ROOT)} ({fixture.output_sha256})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
