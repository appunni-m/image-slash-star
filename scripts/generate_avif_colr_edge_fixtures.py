#!/usr/bin/env python3
"""Generate pinned AVIF color-property parity edge cases.

The accepted case changes the primary item's ``nclx`` type to an unknown
four-character code while preserving pixels. Rejected cases set a reserved
``nclx`` flag bit or empty an ICC profile while preserving BMFF box bounds.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from hashlib import sha256
from io import BytesIO
from pathlib import Path
import struct

from PIL import Image, UnidentifiedImageError
import yaml

from generate_avif_config_disagreement_fixtures import item_property_spans
from generate_decode_refs import verify_primary_oracle
from generate_avif_zero_width_clap_fixture import read_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"
ICC_SOURCE_SHA256 = "a59749aa4985d4954cb8ef8f513e17b40f1051f969374eff7a654aeb0a629d95"
CLAP_SOURCE_SHA256 = "ece2fc8f98cc42f31a8806690ac415460d5eadf96c93492ba2d710c0b2e22bde"
SOURCE_COLR_BOX = bytes.fromhex("00000013636f6c726e636c780001000d000680")


@dataclass(frozen=True)
class Recipe:
    filename: str
    mutation: str
    sha256: str
    pillow_rejects: bool = False
    source_filename: str = "baseline.avif"
    source_sha256: str = SOURCE_SHA256


RECIPES = (
    Recipe(
        "colr_unknown_type_ignored.avif",
        "unknown_type",
        "fa2c4c6af711605a8f72893a3242abb49380ca276957b09c22516f49a0cbd8c8",
    ),
    Recipe(
        "error_colr_reserved_flag_bits.avif",
        "reserved_flag_bits",
        "638cf1dcfaedcd051f2b7250207e1fd38dcc3f622be048ddd1e2c3432933e51c",
        pillow_rejects=True,
    ),
    Recipe(
        "error_colr_empty_icc_profile.avif",
        "empty_icc_profile",
        "d9aa1a6a31622983ea3888c982a33cf5b82bb84a28d6e3b151ac41eac49ab111",
        pillow_rejects=True,
        source_filename="icc_profile.avif",
        source_sha256=ICC_SOURCE_SHA256,
    ),
    Recipe(
        "colr_nclx_trailing_payload_ignored.avif",
        "nclx_trailing_byte",
        "7922155e4e99e6fca5c2fb7a8786c0507cb2e9e8552b80e389a7ab196388774a",
        source_filename="clap_trailing_payload_byte_ignored.avif",
        source_sha256=CLAP_SOURCE_SHA256,
    ),
)


def pack_box(kind: bytes, payload: bytes) -> bytes:
    """Pack a standard-size BMFF box for the bounded fixture recipe."""
    size = len(payload) + 8
    if len(kind) != 4 or size > 0xFFFF_FFFF:
        raise ValueError("invalid standard-size BMFF box")
    return struct.pack(">I4s", size, kind) + payload


def empty_icc_profile(source: bytes) -> bytes:
    """Empty the prof payload and preserve ipco size with a trailing free box."""
    (_, _), (ipco_start, ipco_end) = item_property_spans(source)
    matches = []
    offset = ipco_start
    for kind, complete, payload in read_boxes(source, ipco_start, ipco_end):
        if kind == b"colr" and payload[:4] == b"prof":
            payload_start = offset + len(complete) - len(payload)
            matches.append((payload_start, offset + len(complete), payload))
        offset += len(complete)
    if len(matches) != 1:
        raise RuntimeError(f"expected one prof property, found {len(matches)}")

    colr_start, colr_end, payload = matches[0]
    if payload != b"profpillow-rs-test-profile":
        raise RuntimeError("pinned ICC profile property changed")
    removed = colr_end - colr_start - 4
    if removed < 8:
        raise RuntimeError("ICC profile payload is too short for a padding box")

    empty_colr = pack_box(b"colr", b"prof")
    free_box = pack_box(b"free", bytes(removed - 8))
    box_start = colr_start - 8
    candidate = source[:box_start] + empty_colr + source[colr_end:]
    free_start = ipco_end - removed
    candidate = candidate[:free_start] + free_box + candidate[free_start:]
    if len(candidate) != len(source):
        raise RuntimeError("empty-profile mutation changed the file size")
    return candidate


def nclx_trailing_byte(source: bytes) -> bytes:
    """Move a bounded clap tail to nclx without changing any parent sizes."""
    (_, _), (ipco_start, ipco_end) = item_property_spans(source)
    boxes = read_boxes(source, ipco_start, ipco_end)
    colr_payloads = [
        payload for kind, _, payload in boxes if kind == b"colr" and payload.startswith(b"nclx")
    ]
    clap_payloads = [payload for kind, _, payload in boxes if kind == b"clap"]
    if colr_payloads != [bytes.fromhex("6e636c780001000d000680")]:
        raise RuntimeError("pinned nclx property changed")
    if len(clap_payloads) != 1 or len(clap_payloads[0]) != 33:
        raise RuntimeError("pinned trailing-byte clap property changed")
    trailing = clap_payloads[0][-1:]
    if trailing != b"\x00":
        raise RuntimeError("pinned clap trailing byte changed")

    rebuilt = []
    for kind, _, payload in boxes:
        if kind == b"colr" and payload.startswith(b"nclx"):
            payload += trailing
        elif kind == b"clap":
            payload = payload[:-1]
        rebuilt.append(pack_box(kind, payload))
    replacement = b"".join(rebuilt)
    if len(replacement) != ipco_end - ipco_start:
        raise RuntimeError("transferring the trailing byte changed ipco size")
    return source[:ipco_start] + replacement + source[ipco_end:]


def candidate_bytes(source: bytes, recipe: Recipe) -> bytes:
    """Apply one exact in-place mutation to the pinned baseline AVIF."""
    if sha256(source).hexdigest() != recipe.source_sha256:
        raise RuntimeError(f"pinned source hash changed for {recipe.source_filename}")

    if recipe.mutation == "empty_icc_profile":
        candidate = empty_icc_profile(source)
    elif recipe.mutation == "nclx_trailing_byte":
        candidate = nclx_trailing_byte(source)
    else:
        if source.count(SOURCE_COLR_BOX) != 1:
            raise RuntimeError("expected one pinned primary-item nclx color property")
        box_start = source.index(SOURCE_COLR_BOX)
        candidate_buffer = bytearray(source)
        if recipe.mutation == "unknown_type":
            # Box header (8 bytes), then replace only the nclx property type.
            candidate_buffer[box_start + 8 : box_start + 12] = b"abcd"
        elif recipe.mutation == "reserved_flag_bits":
            # The final byte of the 11-byte nclx payload contains seven reserved bits.
            candidate_buffer[box_start + 18] |= 0x01
        else:
            raise ValueError(
                f"unsupported AVIF color-property mutation: {recipe.mutation}"
            )
        candidate = bytes(candidate_buffer)

    actual_hash = sha256(candidate).hexdigest()
    if actual_hash != recipe.sha256:
        raise RuntimeError(f"{recipe.filename} hash changed: {actual_hash}")
    return candidate


def pillow_pixels(data: bytes) -> tuple[str, tuple[int, int], bytes]:
    """Return Pillow's complete decoded image for accepted color metadata."""
    with Image.open(BytesIO(data)) as image:
        image.load()
        return image.mode, image.size, image.tobytes()


def verify_oracle(source: bytes, recipe: Recipe, candidate: bytes) -> None:
    """Check the mutation against the pinned Pillow observable behavior."""
    if recipe.pillow_rejects:
        try:
            pillow_pixels(candidate)
        except UnidentifiedImageError:
            return
        raise RuntimeError(f"Pillow unexpectedly accepted {recipe.filename}")

    if pillow_pixels(candidate) != pillow_pixels(source):
        raise RuntimeError(f"Pillow pixels changed for {recipe.filename}")


def generate(*, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    ASSETS.mkdir(parents=True, exist_ok=True)
    for recipe in RECIPES:
        source = (ASSETS / recipe.source_filename).read_bytes()
        candidate = candidate_bytes(source, recipe)
        verify_oracle(source, recipe, candidate)
        output = ASSETS / recipe.filename
        if check:
            if not output.is_file() or output.read_bytes() != candidate:
                raise RuntimeError(f"generated fixture is missing or stale: {output}")
        elif output.exists():
            if output.read_bytes() != candidate:
                raise RuntimeError(f"refusing to replace a different fixture: {output}")
        else:
            output.write_bytes(candidate)
        print(f"{output}: {len(candidate)} bytes, sha256={recipe.sha256}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify existing fixtures")
    args = parser.parse_args()
    generate(check=args.check)


if __name__ == "__main__":
    main()
