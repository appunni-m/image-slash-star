#!/usr/bin/env python3
"""Generate bounded AVIF operating-point layer-selection parity inputs."""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, __version__ as PILLOW_VERSION, features

from avif_bmff_boxes import rewrite_boxes
from generate_avif_obu_header_fixtures import PILLOW_ORACLE
from generate_avif_sequence_header_parity import (
    ILOC_PATH, SOURCE, SOURCE_SAMPLE_SIZES, SOURCE_SHA256, STSZ_PATH,
    bits_from_bytes, pack_bits, pack_obu, parse_obus, unsigned_bits,
    update_primary_item_extent, update_stsz,
)
from inspect_av1_obus import parse_sequence_header
from inspect_avif_bitstreams import inspect as inspect_container


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/avif"


@dataclass(frozen=True)
class Recipe:
    filename: str
    inserted_obu: bytes
    digest: str
    malformed: bool = False


RECIPES = (
    Recipe("animated_ignored_temporal_layer.avif", bytes((0x36, 0x20, 0)),
           "1ba3cb1481a80a6e776b5c42199defca0c51f82a6c92ccbebe044a7ff2ad3276"),
    Recipe("animated_ignored_spatial_layer.avif", bytes((0x36, 0x08, 0)),
           "93410e0c7dd6cc3048c3a741ad95ffcf87617bdd9fe9725df181bd1df34cb39e"),
    Recipe("error_animated_ignored_layer_payload_exceeds_sample.avif", bytes((0x36, 0x20, 0x80, 0x80, 0x40)),
           "ca2492155205c925014a304193ebb0cd58868b85c5e1ca809e6c39680135572f", malformed=True),
)


def candidate_bytes(recipe: Recipe) -> tuple[bytes, tuple[int, ...]]:
    source = SOURCE.read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned animation changed")
    container = inspect_container(SOURCE)
    samples = next(t for t in container["tracks"] if t["handler"] == "pict")["samples"]
    if tuple(sample["length"] for sample in samples) != SOURCE_SAMPLE_SIZES:
        raise RuntimeError("pinned sample table changed")
    updated = []
    for index, sample in enumerate(samples):
        output = []
        for header, payload in parse_obus(source[sample["offset"]:sample["offset"] + sample["length"]]):
            kind = (header >> 3) & 15
            if kind == 1:
                raw = bits_from_bytes(payload)
                if raw[12:24] != [0] * 12:
                    raise RuntimeError("pinned operating-point field changed")
                raw[12:24] = unsigned_bits(0x101, 12)
                payload = pack_bits(raw)
                if parse_sequence_header(payload)["operating_points"][0]["idc"] != 0x101:
                    raise RuntimeError("generated operating point differs")
            if kind in (3, 4, 6, 7):
                if index == 0:
                    output.append(recipe.inserted_obu)
                packed = pack_obu(header | 4, payload)
                output.append(packed[:1] + b"\0" + packed[1:])
            else:
                output.append(pack_obu(header, payload))
        updated.append(b"".join(output))
    sizes = tuple(map(len, updated))

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == STSZ_PATH:
            return update_stsz(payload, sizes)
        if path == ILOC_PATH:
            return update_primary_item_extent(payload, sizes[0])
        if path == (b"mdat",):
            return b"".join(updated)
        return payload

    return rewrite_boxes(source, transform), sizes


def verify_pillow(candidate: bytes, malformed: bool) -> None:
    with Image.open(BytesIO(candidate)) as image:
        image.verify()
    with Image.open(BytesIO(candidate)) as image, Image.open(SOURCE) as baseline:
        if (image.mode, image.size, image.n_frames) != (baseline.mode, baseline.size, baseline.n_frames):
            raise RuntimeError("layer mutation changed public sequence metadata")
        if malformed:
            try:
                image.load()
            except RuntimeError as error:
                if str(error) != "Failed to decode frame 0: Decoding of color planes failed":
                    raise RuntimeError("malformed layer's Pillow failure changed") from error
            else:
                raise RuntimeError("Pillow accepted the oversized ignored-layer OBU")
            return
        for index in range(image.n_frames):
            image.seek(index)
            baseline.seek(index)
            image.load()
            baseline.load()
            if image.tobytes() != baseline.tobytes() or image.info.get("duration") != baseline.info.get("duration"):
                raise RuntimeError(f"layer mutation changed complete frame {index} or its duration")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if (PILLOW_VERSION, features.version("avif"), _avif.codec_versions()) != PILLOW_ORACLE:
        raise RuntimeError("Pillow oracle differs from its pin")
    for recipe in RECIPES:
        candidate, sizes = candidate_bytes(recipe)
        if hashlib.sha256(candidate).hexdigest() != recipe.digest:
            raise RuntimeError(f"generated bytes differ: {recipe.filename}")
        verify_pillow(candidate, recipe.malformed)
        path = ASSETS / recipe.filename
        if path.exists():
            if path.read_bytes() != candidate:
                raise RuntimeError(f"refusing to replace differing input: {path}")
        elif args.check:
            raise RuntimeError(f"fixture is missing: {path}")
        else:
            path.write_bytes(candidate)
        rebuilt = inspect_container(path)
        track = next(t for t in rebuilt["tracks"] if t["handler"] == "pict")
        if tuple(sample["length"] for sample in track["samples"]) != sizes or rebuilt["items"]["color"][0]["length"] != sizes[0]:
            raise RuntimeError("generated container extents differ from bounded samples")
        print(f"Verified {path.relative_to(ROOT)} ({len(candidate)} bytes)")


if __name__ == "__main__":
    main()
