#!/usr/bin/env python3
"""Generate malformed auxiliary-alpha association cases from pinned ``alpha.avif``."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path

from PIL import Image, UnidentifiedImageError
import yaml

from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/avif/alpha.avif"
FIXTURE = ROOT / "tests/fixtures/input/images/avif/error_alpha_auxiliary_missing_ispe.avif"
SOURCE_SHA256 = "b19f57d9421bbd3d0b0706c8fe79cef802aebf106afefa6bddde9de1a07509c9"
FIXTURE_SHA256 = "15755192d922569479398381a910a6a73bc9d964e37acf89c26633439ccc0b0c"
FIXTURE_WITHOUT_ISPE_AND_PIXI = (
    ROOT / "tests/fixtures/input/images/avif/error_alpha_auxiliary_missing_ispe_and_pixi.avif"
)
FIXTURE_WITHOUT_ISPE_AND_PIXI_SHA256 = "e28737256c72dd7b31b3377667fb3fabfc5efeb5cd8dcfed5d12847c0642fe48"


def box_ranges(data: bytes | bytearray, start: int, end: int) -> list[tuple[bytes, int, int]]:
    """Return bounded standard-size child ranges."""
    boxes = []
    offset = start
    while offset < end:
        if end - offset < 8:
            raise RuntimeError("truncated box header")
        size = int.from_bytes(data[offset : offset + 4], "big")
        kind = bytes(data[offset + 4 : offset + 8])
        if size < 8 or size > end - offset:
            raise RuntimeError(f"invalid {kind!r} box size")
        boxes.append((kind, offset, offset + size))
        offset += size
    if offset != end:
        raise RuntimeError("child boxes do not fill their parent")
    return boxes


def unique_box(
    boxes: list[tuple[bytes, int, int]], kind: bytes, context: str
) -> tuple[bytes, int, int]:
    """Return exactly one box of the requested kind."""
    matches = [box for box in boxes if box[0] == kind]
    if len(matches) != 1:
        raise RuntimeError(f"expected one {kind!r} box in {context}")
    return matches[0]


def fixture_bytes(source: bytes, *, remove_pixi: bool = False) -> bytes:
    """Remove the auxiliary item's ispe and optionally its pixi association."""
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("alpha AVIF differs from its pinned source hash")
    top_level = box_ranges(source, 0, len(source))
    meta = unique_box(top_level, b"meta", "alpha AVIF")
    if source[meta[1] + 8 : meta[1] + 12] != bytes(4):
        raise RuntimeError("alpha AVIF meta FullBox layout changed")

    meta_children = box_ranges(source, meta[1] + 12, meta[2])
    iprp = unique_box(meta_children, b"iprp", "alpha meta")
    iprp_children = box_ranges(source, iprp[1] + 8, iprp[2])
    ipma = unique_box(iprp_children, b"ipma", "alpha iprp")
    payload = ipma[1] + 8
    version = source[payload]
    flags = int.from_bytes(source[payload + 1 : payload + 4], "big")
    if version != 0 or flags != 0:
        raise RuntimeError("alpha AVIF ipma must use version zero and narrow associations")

    entry_count = int.from_bytes(source[payload + 4 : payload + 8], "big")
    position = payload + 8
    auxiliary_ispe_offset = None
    for _ in range(entry_count):
        if ipma[2] - position < 3:
            raise RuntimeError("alpha AVIF ipma entry is truncated")
        item_id = int.from_bytes(source[position : position + 2], "big")
        position += 2
        association_count = source[position]
        position += 1
        if ipma[2] - position < association_count:
            raise RuntimeError("alpha AVIF ipma associations are truncated")
        associations = source[position : position + association_count]
        if item_id == 2:
            property_indices = tuple(association & 0x7F for association in associations)
            if property_indices != (5, 6, 7, 8) or associations[0] != 5:
                raise RuntimeError("alpha item's property associations differ from their pin")
            auxiliary_ispe_offset = position
        position += association_count

    if position != ipma[2]:
        raise RuntimeError("alpha AVIF ipma has unexpected trailing bytes")
    if auxiliary_ispe_offset is None:
        raise RuntimeError("alpha AVIF ipma is missing auxiliary item 2")

    candidate = bytearray(source)
    candidate[auxiliary_ispe_offset] = 0
    if remove_pixi:
        candidate[auxiliary_ispe_offset + 1] = 0
    result = bytes(candidate)
    expected_sha256 = (
        FIXTURE_WITHOUT_ISPE_AND_PIXI_SHA256 if remove_pixi else FIXTURE_SHA256
    )
    if len(result) != len(source) or sha256(result).hexdigest() != expected_sha256:
        raise RuntimeError("mutated alpha AVIF differs from its pinned output")
    return result


def verify_oracle(candidate: bytes, *, remove_pixi: bool = False) -> None:
    """Check the complete malformed-file result against pinned Pillow."""
    try:
        Image.open(BytesIO(candidate))
    except UnidentifiedImageError:
        return
    except Exception as error:
        raise RuntimeError(
            f"expected Pillow UnidentifiedImageError, got {type(error).__name__}: {error}"
        ) from error
    omitted_properties = "ispe and pixi" if remove_pixi else "ispe"
    raise RuntimeError(
        f"Pillow unexpectedly opened the auxiliary item without {omitted_properties}"
    )


def generate(check: bool = False) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    source = SOURCE.read_bytes()
    cases = (
        (FIXTURE, fixture_bytes(source), False, FIXTURE_SHA256),
        (
            FIXTURE_WITHOUT_ISPE_AND_PIXI,
            fixture_bytes(source, remove_pixi=True),
            True,
            FIXTURE_WITHOUT_ISPE_AND_PIXI_SHA256,
        ),
    )
    for fixture, candidate, remove_pixi, expected_sha256 in cases:
        verify_oracle(candidate, remove_pixi=remove_pixi)
        if check:
            if not fixture.is_file() or fixture.read_bytes() != candidate:
                raise RuntimeError(
                    f"fixture is missing or differs from generated bytes: {fixture}"
                )
        elif fixture.exists():
            if fixture.read_bytes() != candidate:
                raise RuntimeError(f"refusing to replace a different fixture at {fixture}")
        else:
            fixture.write_bytes(candidate)
        print(f"Verified {fixture} ({len(candidate)} bytes, sha256={expected_sha256})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    generate(check=arguments.check)


if __name__ == "__main__":
    main()
