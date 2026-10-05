#!/usr/bin/env python3
"""Create full-file AVIF configuration and duplicate-pixi parity cases.

Pinned Pillow accepts redundant profile, level, tier, and chroma declarations
without changing the decoded image. Disagreeing pixi/av1C depths fail at open.
It validates the first associated pixi and ignores a differing later pixi.
Matching pixi/av1C depths may also differ from the decoded sequence depth.
The twelve-bit declaration is accepted even when high_bitdepth is clear.
Every mutation keeps all container sizes, item extents, and AV1 bytes intact.
"""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from io import BytesIO
from pathlib import Path

import yaml
from PIL import Image, UnidentifiedImageError

from generate_avif_zero_width_clap_fixture import read_boxes
from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT_DIR = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
DEFAULT_SOURCE = DEFAULT_OUTPUT_DIR / "baseline.avif"
SOURCE_SHA256 = "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2"


@dataclass(frozen=True)
class Mutation:
    filename: str
    byte_index: int
    xor_mask: int
    sha256: str
    open_rejected: bool = False
    pixi_depth: int | None = None


@dataclass(frozen=True)
class DepthMutation:
    filename: str
    depth: int
    sha256: str


MUTATIONS = (
    Mutation(
        "av1c_profile_disagreement.avif", 1, 0x20,
        "b121418213ffce59190855a0bfcd285bf0f114e28e847024b633ccf6c8a97a0f",
    ),
    Mutation(
        "av1c_level_disagreement.avif", 1, 0x01,
        "f9c1ed6f53420b64abb098b8616229e3b6a93447f082e0d5acf4f8902b9bda6e",
    ),
    Mutation(
        "av1c_tier_disagreement.avif", 2, 0x80,
        "a9b416067d9531d8f285d67b478b56e100fc68062cd78410abec3bfaadd01e17",
    ),
    Mutation(
        "av1c_monochrome_disagreement.avif", 2, 0x10,
        "7979583621de681a8c441edeb02d4ac4d75ab474682dd7a6848be2349c9a82ec",
    ),
    Mutation(
        "av1c_subsampling_x_disagreement.avif", 2, 0x08,
        "79de6a2617c3b6e3cf8581354a750caebefeb1446615aaf431c783581180b3c1",
    ),
    Mutation(
        "av1c_subsampling_y_disagreement.avif", 2, 0x04,
        "49b6b7cc3ade8745d865a1f281b2f8386d81f1baf7c2918bc14e052118bda40a",
    ),
    Mutation(
        "av1c_chroma_position_disagreement.avif", 2, 0x01,
        "8d1ab6941b6dac678d8be39c62f207c60d1cb41ffe2841d47bd86948b37a2b2d",
    ),
    Mutation(
        "av1c_bit_depth_disagreement.avif", 2, 0x40,
        "5c4f5eb53d0136d7799cd79b8c524b842ec028b24eec80d03368903fcce41301",
        open_rejected=True,
    ),
    Mutation(
        "av1c_twelve_bit_depth_disagreement.avif", 2, 0x60,
        "ce0944c4be7b97aa7c28d963e46e3dfe1c44612a308b1c1bd32b3e8a3e2fcd79",
        open_rejected=True,
    ),
    Mutation(
        "av1c_pixi_twelve_bit_without_high_bitdepth.avif", 2, 0x20,
        "9e9ab41acf3e0cae7e4dc8b0047b962293706057d8276cc6f5b214c3672e40c5",
        pixi_depth=12,
    ),
)


DUPLICATE_PIXI_MUTATIONS = (
    DepthMutation(
        "duplicate_pixi_trailing_ten_bit_depth.avif", 10,
        "d42428a1834db38569c76aa162ba7f2a7c06a807a3334ab741347da3d2e3e8c7",
    ),
    DepthMutation(
        "duplicate_pixi_trailing_twelve_bit_depth.avif", 12,
        "ba3019e72b079f9e1255a592c447b4d535422fbfd55579e4065ab6093d2e55d4",
    ),
)


MATCHED_CONTAINER_DEPTH_MUTATIONS = (
    DepthMutation(
        "av1c_pixi_ten_bit_sequence_eight_bit.avif", 10,
        "c001be20f76a3b26b520012c534ffb0c6532e9a3f32543ab68693536e91aa113",
    ),
    DepthMutation(
        "av1c_pixi_twelve_bit_sequence_eight_bit.avif", 12,
        "abd8d5da2354d01861953e8a8604e4c3c8d13ee003cb072e59cbc61d0c7db9b4",
    ),
)


def unique_payload_span(
    source: bytes, start: int, end: int, wanted_kind: bytes
) -> tuple[int, int]:
    """Locate one child box while preserving its validated parent bounds."""
    if not 0 <= start <= end <= len(source):
        raise RuntimeError("AVIF parent box range is outside the source")
    matches = []
    offset = start
    for kind, complete, payload in read_boxes(source, start, end):
        if kind == wanted_kind:
            matches.append((offset + len(complete) - len(payload), offset + len(complete)))
        offset += len(complete)
    if len(matches) != 1:
        raise RuntimeError(f"expected one {wanted_kind!r} box, found {len(matches)}")
    return matches[0]


def item_property_spans(source: bytes) -> tuple[tuple[int, int], tuple[int, int]]:
    """Return the bounded property association and property container payloads."""
    meta_start, meta_end = unique_payload_span(source, 0, len(source), b"meta")
    if source[meta_start:meta_start + 4] != bytes(4):
        raise RuntimeError("baseline AVIF meta full-box header changed")
    iprp_start, iprp_end = unique_payload_span(source, meta_start + 4, meta_end, b"iprp")
    ipco_start, ipco_end = unique_payload_span(source, iprp_start, iprp_end, b"ipco")
    return (iprp_start, iprp_end), (ipco_start, ipco_end)


def config_payload_offset(source: bytes) -> int:
    """Return the pinned primary item's bounded four-byte av1C payload."""
    _, (ipco_start, ipco_end) = item_property_spans(source)
    config_start, config_end = unique_payload_span(source, ipco_start, ipco_end, b"av1C")
    if source[config_start:config_end] != bytes.fromhex("81000c00"):
        raise RuntimeError("baseline AVIF configuration record changed")
    return config_start


def duplicate_pixi_depth(source: bytes, depth: int) -> bytes:
    """Replace the later colr property with pixi while keeping its box extent.

    The primary item's unchanged ipma order selects the original eight-bit
    pixi before this duplicate. Pillow/libavif uses the first matching property.
    The replacement's three trailing zero bytes keep the nineteen-byte box
    extent; both parsers ignore those bytes after the version-zero fields.
    """
    if depth not in (10, 12):
        raise ValueError("duplicate pixi depth must be ten or twelve bits")
    (iprp_start, iprp_end), (ipco_start, ipco_end) = item_property_spans(source)
    pixi_start, pixi_end = unique_payload_span(source, ipco_start, ipco_end, b"pixi")
    colr_start, colr_end = unique_payload_span(source, ipco_start, ipco_end, b"colr")
    ipma_start, ipma_end = unique_payload_span(source, iprp_start, iprp_end, b"ipma")
    if (
        source[pixi_start:pixi_end] != bytes.fromhex("0000000003080808")
        or source[colr_start:colr_end] != bytes.fromhex("6e636c780001000d000680")
        or source[colr_start - 8:colr_start] != bytes.fromhex("00000013636f6c72")
        or source[ipma_start:ipma_end] != bytes.fromhex("000000000000000100010401028304")
        or pixi_start >= colr_start
    ):
        raise RuntimeError("baseline AVIF pixel-information association order changed")
    candidate = bytearray(source)
    candidate[colr_start - 4:colr_start] = b"pixi"
    candidate[colr_start:colr_end] = bytes(4) + bytes((3, depth, depth, depth)) + bytes(3)
    return bytes(candidate)


def pixel_information_span(source: bytes) -> tuple[int, int]:
    """Return the bounded, pinned eight-bit pixel-information payload."""
    _, (ipco_start, ipco_end) = item_property_spans(source)
    pixi_start, pixi_end = unique_payload_span(source, ipco_start, ipco_end, b"pixi")
    if source[pixi_start:pixi_end] != bytes.fromhex("0000000003080808"):
        raise RuntimeError("baseline AVIF pixel-information declaration changed")
    return pixi_start, pixi_end


def matched_container_depth(source: bytes, depth: int) -> bytes:
    """Change matching pixi/av1C declarations and retain eight-bit AV1 syntax."""
    if depth not in (10, 12):
        raise ValueError("matched container depth must be ten or twelve bits")
    pixi_start, pixi_end = pixel_information_span(source)
    config_start = config_payload_offset(source)
    candidate = bytearray(source)
    candidate[pixi_start + 5:pixi_end] = bytes((depth, depth, depth))
    candidate[config_start + 2] ^= 0x40 if depth == 10 else 0x60
    return bytes(candidate)


def pillow_snapshot(source: bytes) -> tuple:
    """Observe complete Pillow image, frame, metadata, and raw-byte results."""
    with Image.open(BytesIO(source)) as image:
        image.verify()
    with Image.open(BytesIO(source)) as image:
        header = (
            image.format, image.mode, image.size, image.n_frames,
            image.is_animated, dict(image.info),
        )
        frames = []
        for index in range(image.n_frames):
            image.seek(index)
            image.load()
            frames.append((image.mode, image.size, dict(image.info), image.tobytes()))
        return header, frames


def verify_pillow_rejection(source: bytes, filename: str) -> None:
    """Require the pinned depth disagreement to fail at Image.open."""
    try:
        image = Image.open(BytesIO(source))
    except UnidentifiedImageError:
        return
    image.close()
    raise RuntimeError(f"Pillow unexpectedly opened {filename}")


def generate(
    output_dir: Path = DEFAULT_OUTPUT_DIR, source_path: Path = DEFAULT_SOURCE
) -> list[Path]:
    """Generate or verify all fourteen fixtures for the existing AVIF asset runner."""
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    source = source_path.read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("baseline AVIF source differs from its pinned hash")
    config_start = config_payload_offset(source)
    reference = pillow_snapshot(source)
    candidates = []
    for mutation in MUTATIONS:
        candidate = bytearray(source)
        candidate[config_start + mutation.byte_index] ^= mutation.xor_mask
        if mutation.pixi_depth is not None:
            pixi_start, pixi_end = pixel_information_span(source)
            candidate[pixi_start + 5:pixi_end] = bytes((mutation.pixi_depth,)) * 3
        candidate = bytes(candidate)
        if hashlib.sha256(candidate).hexdigest() != mutation.sha256:
            raise RuntimeError(f"{mutation.filename} differs from its pinned input hash")
        if mutation.open_rejected:
            verify_pillow_rejection(candidate, mutation.filename)
        elif pillow_snapshot(candidate) != reference:
            raise RuntimeError(f"{mutation.filename} changed Pillow's image or frame result")
        path = output_dir / mutation.filename
        if path.exists() and path.read_bytes() != candidate:
            raise RuntimeError(f"refusing to replace a different fixture at {path}")
        candidates.append((path, candidate))

    for mutations, build in (
        (DUPLICATE_PIXI_MUTATIONS, duplicate_pixi_depth),
        (MATCHED_CONTAINER_DEPTH_MUTATIONS, matched_container_depth),
    ):
        for mutation in mutations:
            candidate = build(source, mutation.depth)
            if hashlib.sha256(candidate).hexdigest() != mutation.sha256:
                raise RuntimeError(f"{mutation.filename} differs from its pinned input hash")
            if pillow_snapshot(candidate) != reference:
                raise RuntimeError(f"{mutation.filename} changed Pillow's image or frame result")
            path = output_dir / mutation.filename
            if path.exists() and path.read_bytes() != candidate:
                raise RuntimeError(f"refusing to replace a different fixture at {path}")
            candidates.append((path, candidate))

    output_dir.mkdir(parents=True, exist_ok=True)
    for path, candidate in candidates:
        if not path.exists():
            path.write_bytes(candidate)
        print(f"Verified AVIF configuration fixture: {path}")
    return [path for path, _ in candidates]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    args = parser.parse_args()
    generate(args.output_dir, args.source)


if __name__ == "__main__":
    main()
