#!/usr/bin/env python3
"""Append trailing UUID box size variants to the pinned AVIF baseline."""

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
FIXTURE = ROOT / "tests/fixtures/input/images/avif/trailing_uuid_zero_size_ignored.avif"
FIXTURE_SHA256 = "5fe9b4f99f994e4424ecc8467d12b42b1d6c7f41fa1ad48f1e78eafb4919df88"
UUID_USER_TYPE = bytes.fromhex("00112233445566778899aabbccddeeff")
EXTENDED_FIXTURE = ROOT / "tests/fixtures/input/images/avif/trailing_uuid_extended_size_ignored.avif"
EXTENDED_FIXTURE_SHA256 = "2e3a871c9481898f7f6ab78fc0d79c6abc24924b00d8c65871b6fdee90e73966"
EXTENDED_UUID_USER_TYPE = bytes.fromhex("102132435465768798a9bacbdcedfe0f")
OVERSIZED_FIXTURE = ROOT / "tests/fixtures/input/images/avif/trailing_uuid_oversized_ignored.avif"
OVERSIZED_FIXTURE_SHA256 = "667587f4cdaecbf61f8be838eee71042a3205c276d229aa080f7c6fb32b3ffc3"


def pillow_observation(data: bytes) -> tuple[str, tuple[int, int], bytes, dict[str, object]]:
    """Return Pillow's decoded pixels and image information."""
    with Image.open(BytesIO(data)) as image:
        image.load()
        return image.mode, image.size, image.tobytes(), dict(image.info)


def candidate_bytes() -> bytes:
    """Append a final open-ended UUID box with a pinned user type."""
    source = SOURCE.read_bytes()
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned AVIF baseline differs from its maintained hash")
    candidate = source + struct.pack(">I4s", 0, b"uuid") + UUID_USER_TYPE
    digest = sha256(candidate).hexdigest()
    if digest != FIXTURE_SHA256:
        raise RuntimeError(f"mutated AVIF hash changed: {digest}")
    return candidate


def extended_candidate_bytes() -> bytes:
    """Append a fixed-size UUID box using its extended 64-bit size field."""
    source = SOURCE.read_bytes()
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned AVIF baseline differs from its maintained hash")
    extended_header = struct.pack(">I4sQ", 1, b"uuid", 32)
    candidate = source + extended_header + EXTENDED_UUID_USER_TYPE
    digest = sha256(candidate).hexdigest()
    if digest != EXTENDED_FIXTURE_SHA256:
        raise RuntimeError(f"extended-size AVIF hash changed: {digest}")
    return candidate


def oversized_candidate_bytes() -> bytes:
    """Append a UUID box declaring a size larger than the 32-bit box limit."""
    source = SOURCE.read_bytes()
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned AVIF baseline differs from its maintained hash")
    oversized_header = struct.pack(">I4sQ", 1, b"uuid", 0x1_0000_0000)
    candidate = source + oversized_header + EXTENDED_UUID_USER_TYPE
    digest = sha256(candidate).hexdigest()
    if digest != OVERSIZED_FIXTURE_SHA256:
        raise RuntimeError(f"oversized AVIF hash changed: {digest}")
    return candidate


def verify_pillow(source: bytes, candidate: bytes) -> None:
    """Require Pillow to ignore the UUID and preserve its full observation."""
    if pillow_observation(candidate) != pillow_observation(source):
        raise RuntimeError("Pillow observation changed after the trailing UUID box")


def generate(*, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    source = SOURCE.read_bytes()
    candidates = (
        (FIXTURE, candidate_bytes(), FIXTURE_SHA256),
        (EXTENDED_FIXTURE, extended_candidate_bytes(), EXTENDED_FIXTURE_SHA256),
        (OVERSIZED_FIXTURE, oversized_candidate_bytes(), OVERSIZED_FIXTURE_SHA256),
    )
    for path, candidate, expected_sha256 in candidates:
        verify_pillow(source, candidate)
        if check:
            if not path.is_file() or path.read_bytes() != candidate:
                raise RuntimeError(f"generated fixture is missing or stale: {path}")
        elif path.exists():
            if path.read_bytes() != candidate:
                raise RuntimeError(f"refusing to replace a different fixture: {path}")
        else:
            path.write_bytes(candidate)
        print(f"{path}: {len(candidate)} bytes, sha256={expected_sha256}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify fixture bytes without writing")
    args = parser.parse_args()
    generate(check=args.check)


if __name__ == "__main__":
    raise SystemExit(main())
