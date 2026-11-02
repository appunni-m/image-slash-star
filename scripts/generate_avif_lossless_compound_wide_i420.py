#!/usr/bin/env python3
"""Generate a pinned wide, compound-predicted lossless I420 AVIF sequence."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path
import random
from tempfile import TemporaryDirectory

from PIL import Image, _avif, features
from PIL import __version__ as PILLOW_VERSION

from generate_avif_multitile_motion_fixture import normalize_timestamps
from inspect_av1_obus import inspect as inspect_av1


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/avif/animated_lossless_compound_wide_i420_b128x128.avif"
)
SIZE = (128, 128)
EXPECTED_FILE_SHA256 = "7e736d84c4a9477930c013517e4f3a8e4ad759ebbcf524ecbb67240b8bbfaefc"
EXPECTED_FRAME_SHA256 = (
    "9f7a07bb44764873cf3e96985e7c29c0816738aa459970485fab489ddf20e0a4",
    "cf81eac1ea5daa17af44a10220b6926cf04ceab929d02de096c088e2490504f3",
    "8a5086b0f1e799eb16a7a3ca8e392a128383ee736f4f4cbdea0e83f0d36bdbbb",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
ADVANCED_OPTIONS = {
    "sb-size": "128",
    "min-partition-size": "128",
    "max-partition-size": "128",
    "aq-mode": "0",
    "deltaq-mode": "0",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "loopfilter-control": "0",
    "enable-warped-motion": "0",
    "enable-interintra-comp": "0",
    "enable-global-motion": "0",
}


def check_oracle() -> None:
    """Require the exact Pillow, libavif, and libaom fixture oracle."""
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned AVIF oracle required; found {observed!r}")


def source_frames() -> tuple[Image.Image, Image.Image, Image.Image]:
    """Build two deterministic references and their exact RGB average."""
    references = []
    for seed in (1096, 2096):
        rng = random.Random(seed)
        pixels = bytes(rng.randrange(256) for _ in range(SIZE[0] * SIZE[1] * 3))
        references.append(Image.frombytes("RGB", SIZE, pixels))
    return references[0], references[1], Image.blend(*references, 0.5)


def encode_fixture() -> bytes:
    """Encode all-lossless 128x128 inter leaves with compound prediction."""
    frames = source_frames()
    output = BytesIO()
    frames[0].save(
        output,
        format="AVIF",
        save_all=True,
        append_images=list(frames[1:]),
        duration=[100] * len(frames),
        loop=0,
        quality=100,
        speed=0,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        codec="aom",
        advanced=ADVANCED_OPTIONS,
    )
    return normalize_timestamps(output.getvalue())


def verify_observed_sequence(encoded: bytes) -> None:
    """Check the lossless compound-wide AV1 syntax and exact Pillow output."""
    if sha256(encoded).hexdigest() != EXPECTED_FILE_SHA256:
        raise RuntimeError("encoded file hash differs from its pinned bytes")

    with TemporaryDirectory(prefix="avif-lossless-compound-wide-") as directory:
        path = Path(directory) / "fixture.avif"
        path.write_bytes(encoded)
        report = inspect_av1(path)

    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if not sequence_headers or any(
        header.get("bit_depth") != 8
        or header.get("monochrome")
        or header.get("profile") != 0
        or header.get("subsampling_x") != 1
        or header.get("subsampling_y") != 1
        or header.get("use_128x128_superblock") is not True
        for header in sequence_headers
    ):
        raise RuntimeError("expected an 8-bit I420 sequence with 128x128 superblocks")

    frame_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    final_headers = [header for header in frame_headers if header.get("order_hint") == 2]
    if (
        len(final_headers) != 1
        or any(header.get("all_lossless") is not True for header in frame_headers)
        or final_headers[0].get("frame_type") != "inter"
        or final_headers[0].get("reference_mode") != "select"
        or final_headers[0].get("reference_indices") != [1, 0, 0, 0, 0, 0, 0]
    ):
        raise RuntimeError("final all-lossless frame does not expose both references")

    frame_hashes = []
    with Image.open(BytesIO(encoded)) as sequence:
        if (
            sequence.format != "AVIF"
            or sequence.mode != "RGB"
            or sequence.size != SIZE
            or sequence.n_frames != len(EXPECTED_FRAME_SHA256)
        ):
            raise RuntimeError("Pillow AVIF sequence metadata differs from the pin")
        for index in range(sequence.n_frames):
            sequence.seek(index)
            sequence.load()
            if sequence.info.get("duration") != 100:
                raise RuntimeError(f"Pillow frame {index} duration differs from the pin")
            frame_hashes.append(sha256(sequence.convert("RGB").tobytes()).hexdigest())
    if tuple(frame_hashes) != EXPECTED_FRAME_SHA256:
        raise RuntimeError(f"Pillow RGB hashes differ from the pin: {tuple(frame_hashes)!r}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, help="write or check a different fixture path")
    parser.add_argument("--check", action="store_true", help="verify the committed fixture")
    args = parser.parse_args()

    check_oracle()
    encoded = encode_fixture()
    if encoded != encode_fixture():
        raise RuntimeError("AVIF encoding is not byte-for-byte deterministic")
    verify_observed_sequence(encoded)

    output = (args.output or OUTPUT).resolve()
    if args.check:
        if not output.is_file() or output.read_bytes() != encoded:
            raise RuntimeError(f"fixture differs from deterministic output: {output}")
        print(f"Verified {output} ({len(encoded)} bytes, sha256={EXPECTED_FILE_SHA256})")
        return
    if output.exists() and output.read_bytes() != encoded:
        raise RuntimeError(f"refusing to replace differing fixture: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded)
    print(f"Wrote {output} ({len(encoded)} bytes, sha256={EXPECTED_FILE_SHA256})")


if __name__ == "__main__":
    main()
