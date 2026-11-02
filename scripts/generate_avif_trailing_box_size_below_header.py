#!/usr/bin/env python3
"""Append a truncated trailing box with a size below its header length."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path
import struct

from PIL import Image
import yaml

from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/avif/baseline.avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
FIXTURE = ROOT / "tests/fixtures/input/images/avif/trailing_box_size_below_header_ignored.avif"
FIXTURE_SHA256 = "d99d5e1018f23c5251d163ea6352c2ac6f9a0559ea5f21748b3ddd0f7ba493bf"
TRAILING_BOX = struct.pack(">I4s", 4, b"free")


def pillow_observation(data: bytes) -> tuple[str, tuple[int, int], bytes, dict[str, object]]:
    """Return Pillow's decoded pixels and image information."""
    with Image.open(BytesIO(data)) as image:
        image.load()
        return image.mode, image.size, image.tobytes(), dict(image.info)


def candidate_bytes() -> bytes:
    """Append the pinned undersized box header after the complete baseline."""
    source = SOURCE.read_bytes()
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned AVIF baseline differs from its maintained hash")
    candidate = source + TRAILING_BOX
    digest = sha256(candidate).hexdigest()
    if digest != FIXTURE_SHA256:
        raise RuntimeError(f"mutated AVIF hash changed: {digest}")
    return candidate


def verify_pillow(source: bytes, candidate: bytes) -> None:
    """Require Pillow to ignore the trailing box and preserve its observation."""
    if pillow_observation(candidate) != pillow_observation(source):
        raise RuntimeError("Pillow observation changed after the undersized trailing box")


def generate(*, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    source = SOURCE.read_bytes()
    candidate = candidate_bytes()
    verify_pillow(source, candidate)
    if check:
        if not FIXTURE.is_file() or FIXTURE.read_bytes() != candidate:
            raise RuntimeError(f"generated fixture is missing or stale: {FIXTURE}")
    elif FIXTURE.exists():
        if FIXTURE.read_bytes() != candidate:
            raise RuntimeError(f"refusing to replace a different fixture: {FIXTURE}")
    else:
        FIXTURE.write_bytes(candidate)
    print(f"{FIXTURE}: {len(candidate)} bytes, sha256={FIXTURE_SHA256}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify fixture bytes without writing")
    args = parser.parse_args()
    generate(check=args.check)


if __name__ == "__main__":
    raise SystemExit(main())
