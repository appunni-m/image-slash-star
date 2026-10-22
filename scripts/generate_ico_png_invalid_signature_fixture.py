#!/usr/bin/env python3
"""Generate an ICO whose PNG entry has a corrupted signature suffix."""

from __future__ import annotations

import argparse
import hashlib
import io
import struct
from pathlib import Path

import yaml
from PIL import Image

from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tests/fixtures/input/images/ico/png_entry.ico"
OUTPUT_NAME = Path("ico/png_invalid_signature.ico")
EXPECTED_SOURCE_SHA256 = "930a2b2ea365e5f252f9050a9d6085d79024d97a3e2c779554a0cc9bd29a9ff6"
EXPECTED_FIXTURE_SHA256 = "69864fbd1392610d04953ca7ff3de0b0e9643a24f3edeabee748e06fe251549e"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def fixture_bytes() -> bytes:
    """Change only the fifth PNG signature byte inside the complete ICO."""
    source = SOURCE.read_bytes()
    if sha256(source) != EXPECTED_SOURCE_SHA256:
        raise RuntimeError("base PNG-backed ICO differs from its pinned source hash")
    if len(source) < 22 or struct.unpack_from("<HHH", source) != (0, 1, 1):
        raise RuntimeError("base fixture must contain one complete ICO entry")

    _, _, _, _, _, _, resource_size, resource_offset = struct.unpack_from(
        "<BBBBHHII", source, 6
    )
    resource_end = resource_offset + resource_size
    if resource_end != len(source):
        raise RuntimeError("base ICO entry must cover the complete file payload")
    if source[resource_offset : resource_offset + len(PNG_SIGNATURE)] != PNG_SIGNATURE:
        raise RuntimeError("base ICO entry does not begin with a complete PNG signature")

    fixture = bytearray(source)
    fixture[resource_offset + 4] ^= 1
    if fixture[resource_offset : resource_offset + 4] != PNG_SIGNATURE[:4]:
        raise RuntimeError("fixture must retain the ICO decoder's four-byte PNG prefix")
    if fixture[resource_offset : resource_offset + len(PNG_SIGNATURE)] == PNG_SIGNATURE:
        raise RuntimeError("fixture PNG signature was not corrupted")
    if sha256(fixture) != EXPECTED_FIXTURE_SHA256:
        raise RuntimeError("generated ICO differs from its pinned fixture hash")
    return bytes(fixture)


def verify_pillow(data: bytes) -> None:
    """Pin the full-file mutation to Pillow's observable malformed-input result."""
    try:
        with Image.open(io.BytesIO(data)) as image:
            image.load()
    except OSError as error:
        if type(error) is not OSError or str(error) != "Truncated File Read":
            raise RuntimeError(
                "Pillow returned an unexpected error for the corrupted ICO PNG signature"
            ) from error
    else:
        raise RuntimeError("Pillow unexpectedly accepted the corrupted ICO PNG signature")


def generate(destination: Path, *, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    data = fixture_bytes()
    verify_pillow(data)

    if check:
        if not destination.is_file() or destination.read_bytes() != data:
            raise RuntimeError(f"complete generated input differs: {destination}")
    else:
        destination.parent.mkdir(parents=True, exist_ok=True)
        if destination.exists() and destination.read_bytes() != data:
            raise RuntimeError(f"refusing to replace different fixture: {destination}")
        destination.write_bytes(data)
    print(f"{'Checked' if check else 'Generated'} {destination}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "tests" / "fixtures" / "input" / "images",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    generate(args.output.resolve() / OUTPUT_NAME, check=args.check)


if __name__ == "__main__":
    main()
