#!/usr/bin/env python3
"""Generate a Pillow-parity case for an AVIF property box shorter than its header."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path

from PIL import Image, UnidentifiedImageError
import yaml

from avif_bmff_boxes import read_boxes
from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE_FILENAME = "hdr.avif"
OUTPUT_FILENAME = "error_ipco_property_size_below_header.avif"
SOURCE_SHA256 = "9980e58ddf718a923f1738c34aad1c72f8e5795ec07e68f1a5f9bd216ca19740"
FIXTURE_SHA256 = "1376cc7d7360eae6b03c97b6d3808dc95bfa945aa8578e4443c540489ea2ed20"
ISPE_PAYLOAD = bytes.fromhex("00000000000000c8000000c8")
INVALID_SIZE = bytes.fromhex("00000004")


def ispe_size_offset(source: bytes) -> int:
    """Locate the first pinned `ipco/ispe` size field in the complete file."""
    top_level = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in top_level] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("pinned HDR top-level box layout changed")
    meta_index = next(index for index, box in enumerate(top_level) if box[0] == b"meta")
    meta_offset = sum(len(box[1]) for box in top_level[:meta_index])
    meta_payload = top_level[meta_index][2]
    if meta_payload[:4] != bytes(4):
        raise RuntimeError("pinned HDR meta FullBox header changed")

    meta_children = read_boxes(meta_payload, 4, len(meta_payload))
    iprp_indexes = [index for index, box in enumerate(meta_children) if box[0] == b"iprp"]
    if len(iprp_indexes) != 1:
        raise RuntimeError(f"expected one HDR iprp box, found {len(iprp_indexes)}")
    iprp_index = iprp_indexes[0]
    iprp_payload = meta_children[iprp_index][2]
    iprp_children = read_boxes(iprp_payload, 0, len(iprp_payload))
    ipco_indexes = [index for index, box in enumerate(iprp_children) if box[0] == b"ipco"]
    if len(ipco_indexes) != 1:
        raise RuntimeError(f"expected one HDR ipco box, found {len(ipco_indexes)}")
    ipco_index = ipco_indexes[0]
    properties = read_boxes(iprp_children[ipco_index][2], 0, len(iprp_children[ipco_index][2]))
    if not properties or properties[0][0] != b"ispe" or properties[0][2] != ISPE_PAYLOAD:
        raise RuntimeError("pinned HDR first ispe property changed")

    return (
        meta_offset
        + 8
        + 4
        + sum(len(box[1]) for box in meta_children[:iprp_index])
        + 8
        + sum(len(box[1]) for box in iprp_children[:ipco_index])
        + 8
    )


def candidate_bytes(source: bytes) -> bytes:
    """Change only the first `ipco/ispe` size from 20 to 4 bytes."""
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned HDR source hash changed")
    offset = ispe_size_offset(source)
    if source[offset : offset + 4] != bytes.fromhex("00000014"):
        raise RuntimeError("pinned HDR ispe box size changed")

    candidate = source[:offset] + INVALID_SIZE + source[offset + 4 :]
    if len(candidate) != len(source):
        raise RuntimeError("invalid-size mutation changed the complete file length")
    source_mdat = next(
        payload for kind, _, payload in read_boxes(source, 0, len(source)) if kind == b"mdat"
    )
    candidate_mdat = next(
        payload
        for kind, _, payload in read_boxes(candidate, 0, len(candidate))
        if kind == b"mdat"
    )
    if source_mdat != candidate_mdat:
        raise RuntimeError("invalid-size mutation changed the AV1 media payload")
    if sha256(candidate).hexdigest() != FIXTURE_SHA256:
        raise RuntimeError(f"{OUTPUT_FILENAME} hash changed: {sha256(candidate).hexdigest()}")
    return candidate


def verify_pillow_rejection(candidate: bytes) -> None:
    """Require the pinned Pillow oracle to reject the malformed nested box at open."""
    try:
        image = Image.open(BytesIO(candidate))
    except UnidentifiedImageError:
        return
    image.close()
    raise RuntimeError(f"Pillow unexpectedly opened {OUTPUT_FILENAME}")


def generate(*, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    source = (ASSETS / SOURCE_FILENAME).read_bytes()
    candidate = candidate_bytes(source)
    verify_pillow_rejection(candidate)

    output = ASSETS / OUTPUT_FILENAME
    if check:
        if not output.is_file() or output.read_bytes() != candidate:
            raise RuntimeError(f"generated fixture is missing or stale: {output}")
    elif output.exists():
        if output.read_bytes() != candidate:
            raise RuntimeError(f"refusing to replace a different fixture: {output}")
    else:
        output.write_bytes(candidate)
    print(f"{output}: {len(candidate)} bytes, sha256={FIXTURE_SHA256}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify the existing fixture")
    args = parser.parse_args()
    generate(check=args.check)


if __name__ == "__main__":
    main()
