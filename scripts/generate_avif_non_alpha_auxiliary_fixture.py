#!/usr/bin/env python3
"""Generate Pillow parity for a non-alpha AVIF auxiliary item."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path

from PIL import Image
import yaml

from avif_bmff_boxes import read_boxes
from generate_avif_alpha_missing_auxiliary_ispe_fixture import SOURCE, SOURCE_SHA256
from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
INPUTS = ROOT / "tests/fixtures/input/images/avif"
FIXTURE = INPUTS / "non_alpha_auxiliary_depth.avif"
FIXTURE_SHA256 = "635da9c080199e4f1c43472e7bf0349d61b51d70504b4ded7e8df9bf25338380"
ALPHA_URN = b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha"
DEPTH_URN = b"urn:mpeg:mpegB:cicp:systems:auxiliary:depth"


def unique_payload(boxes: list[tuple[bytes, bytes, bytes]], kind: bytes, context: str) -> bytes:
    """Return the payload of exactly one child box."""
    matches = [payload for box_kind, _, payload in boxes if box_kind == kind]
    if len(matches) != 1:
        raise RuntimeError(f"expected one {kind!r} box in {context}")
    return matches[0]


def auxiliary_property_payload(source: bytes) -> bytes:
    """Read the pinned alpha item's single auxC property."""
    top_level = read_boxes(source, 0, len(source))
    meta = unique_payload(top_level, b"meta", "source")
    if meta[:4] != bytes(4):
        raise RuntimeError("source meta FullBox differs from its version-zero pin")
    meta_children = read_boxes(meta, 4, len(meta))
    iprp = unique_payload(meta_children, b"iprp", "meta")
    iprp_children = read_boxes(iprp, 0, len(iprp))
    ipco = unique_payload(iprp_children, b"ipco", "iprp")
    properties = read_boxes(ipco, 0, len(ipco))
    return unique_payload(properties, b"auxC", "ipco")


def fixture_bytes(source: bytes) -> bytes:
    """Replace the alpha type with a same-length depth type."""
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("alpha AVIF differs from its pinned source hash")
    if len(ALPHA_URN) != len(DEPTH_URN) or source.count(ALPHA_URN) != 1:
        raise RuntimeError("alpha auxiliary type does not match its one-field mutation pin")
    expected_payload = bytes(4) + ALPHA_URN + b"\0"
    if auxiliary_property_payload(source) != expected_payload:
        raise RuntimeError("source auxC property differs from its pinned alpha declaration")

    result = source.replace(ALPHA_URN, DEPTH_URN)
    if len(result) != len(source):
        raise RuntimeError("same-length auxC mutation changed the file size")
    if auxiliary_property_payload(result) != bytes(4) + DEPTH_URN + b"\0":
        raise RuntimeError("mutated auxC property differs from its depth declaration")
    if unique_payload(read_boxes(source, 0, len(source)), b"mdat", "source") != unique_payload(
        read_boxes(result, 0, len(result)), b"mdat", "mutated file"
    ):
        raise RuntimeError("auxiliary-type mutation changed encoded AV1 samples")
    if sha256(result).hexdigest() != FIXTURE_SHA256:
        raise RuntimeError("mutated AVIF differs from its pinned output")
    return result


def verify_oracle(candidate: bytes) -> None:
    """Check RGB mode and samples against the pinned source converted to RGB."""
    try:
        with Image.open(BytesIO(candidate)) as image:
            image.load()
            observed = (image.format, image.size, image.mode, image.tobytes())
        with Image.open(SOURCE) as source:
            source.load()
            rgb = source.convert("RGB")
            expected = (source.format, rgb.size, rgb.mode, rgb.tobytes())
            rgb.close()
    except Exception as error:
        raise RuntimeError(f"Pillow rejected non-alpha auxiliary input: {error}") from error
    if observed != expected or observed[:3] != ("AVIF", (64, 64), "RGB"):
        raise RuntimeError("non-alpha auxiliary output differs from the pinned RGB result")


def generate(check: bool = False) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    candidate = fixture_bytes(SOURCE.read_bytes())
    verify_oracle(candidate)
    if check:
        if not FIXTURE.is_file() or FIXTURE.read_bytes() != candidate:
            raise RuntimeError(f"fixture is missing or stale: {FIXTURE}")
    elif FIXTURE.exists():
        if FIXTURE.read_bytes() != candidate:
            raise RuntimeError(f"refusing to replace a different fixture: {FIXTURE}")
    else:
        FIXTURE.write_bytes(candidate)
    print(f"Verified {FIXTURE} (sha256={FIXTURE_SHA256})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail instead of writing a fixture")
    generate(check=parser.parse_args().check)


if __name__ == "__main__":
    main()
