#!/usr/bin/env python3
"""Generate an AVIF sequence with an oblique, no-sign-flip inter-intra wedge."""

from __future__ import annotations

import argparse
import hashlib
import os
import tempfile
from io import BytesIO
from pathlib import Path

import yaml
from PIL import Image

from generate_avif_multitile_motion_fixture import normalize_timestamps
from generate_decode_refs import verify_primary_oracle
from inspect_av1_obus import inspect as inspect_av1
from inspect_avif_bitstreams import inspect as inspect_container

ROOT = Path(__file__).resolve().parents[1]
OUTPUT_DIR = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
FILENAME = "animated_lossy_interintra_420_wedge_idx2_no_flip_b16x16_64x64.avif"
SIZE = (64, 64)
DURATION_MS = 100
ASSET_SHA256 = "62f1f2704bdd6efcd3b936909dd3d9351116fb8d072add8a0194d9ca5459e275"
FRAME_SHA256 = (
    "d4fcab1a5d65743e860487cb556d6f4d8c5c9db63f83fdb62569c53666bd95d7",
    "d4fcab1a5d65743e860487cb556d6f4d8c5c9db63f83fdb62569c53666bd95d7",
    "56c4a1871bd142794c454e73849b6c52f6c7b9815f7059d20dbe80b8873f9b6b",
)

# Pinned AV1 64x64 master wedge borders. The index-2, 16x16 block is centered
# in the master template and uses the oblique117 direction with no sign flip.
WEDGE_MASTER_EVEN = (
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 4, 11, 27,
    46, 58, 62, 63, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64,
    64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64,
)
WEDGE_MASTER_ODD = (
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 6, 18,
    37, 53, 60, 63, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64,
    64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64,
)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def source_frames() -> list[Image.Image]:
    """Create two flat references and the inverted index-2 wedge target."""
    reference_color = (20, 80, 220)
    frames = [Image.new("RGB", SIZE, reference_color) for _ in range(2)]
    final = Image.new("RGB", SIZE, (0, 0, 0))

    for y in range(16):
        master_y = 24 + y
        master = WEDGE_MASTER_EVEN if master_y % 2 == 0 else WEDGE_MASTER_ODD
        for x in range(16):
            # Index 2 uses oblique117: reflect the master column, then advance
            # its source border by ceil(row / 2). The inter-intra target uses
            # the complementary weight so its colored ramp rises diagonally.
            source_index = 35 + (y + 1) // 2 - x
            inter_weight = 64 - master[source_index]
            color = tuple(
                (channel * inter_weight + 32) // 64 for channel in reference_color
            )
            final.putpixel((16 + x, 16 + y), color)

    frames.append(final)
    return frames


def encode_fixture() -> bytes:
    frames = source_frames()
    output = BytesIO()
    frames[0].save(
        output,
        format="AVIF",
        save_all=True,
        append_images=frames[1:],
        duration=[DURATION_MS] * len(frames),
        loop=0,
        quality=50,
        speed=0,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        codec="aom",
        advanced={
            "color-primaries": "1",
            "transfer-characteristics": "13",
            "matrix-coefficients": "6",
            "enable-interintra-comp": "1",
            "enable-interintra-wedge": "1",
            "enable-smooth-interintra": "1",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "enable-warped-motion": "0",
            "min-partition-size": "16",
            "max-partition-size": "16",
        },
    )
    return normalize_timestamps(output.getvalue())


def verify_pillow_observations(encoded: bytes) -> None:
    with Image.open(BytesIO(encoded)) as sequence:
        if (
            sequence.format != "AVIF"
            or sequence.mode != "RGB"
            or sequence.size != SIZE
            or sequence.n_frames != len(FRAME_SHA256)
            or not sequence.is_animated
        ):
            raise RuntimeError("AVIF format, dimensions, mode, or frame count differs")

        observations = []
        for frame_index in range(sequence.n_frames):
            sequence.seek(frame_index)
            sequence.load()
            if (
                sequence.mode != "RGB"
                or sequence.size != SIZE
                or sequence.info.get("duration") != DURATION_MS
            ):
                raise RuntimeError("Pillow frame metadata or duration differs")
            observations.append(digest(sequence.convert("RGB").tobytes()))

    if tuple(observations) != FRAME_SHA256:
        raise RuntimeError("Pillow RGB frame hashes differ from the pinned references")


def verify_encoded_profile(path: Path) -> None:
    container = inspect_container(path)
    tracks = container["tracks"]
    if len(tracks) != 1 or tracks[0]["handler"] != "pict":
        raise RuntimeError("expected one complete AVIF picture track")
    samples = tracks[0]["samples"]
    if (
        tracks[0]["timescale"] != 1000
        or len(samples) != len(FRAME_SHA256)
        or [sample["duration"] for sample in samples] != [DURATION_MS] * len(samples)
    ):
        raise RuntimeError("AVIF sequence sample timing differs")

    report = inspect_av1(path)
    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    expected_profile = (0, 8, False, 1, 1, 1, 13, 6, 1)
    if not sequence_headers or any(
        (
            header["profile"],
            header["bit_depth"],
            header["monochrome"],
            header["subsampling_x"],
            header["subsampling_y"],
            header["color_primaries"],
            header["transfer_characteristics"],
            header["matrix_coefficients"],
            header["color_range"],
        )
        != expected_profile
        or not header["enable_interintra_compound"]
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence profile differs from 8-bit full-range I420")

    track_samples = [
        sample
        for sample in report["samples"]
        if isinstance(sample.get("identity"), dict)
        and sample["identity"].get("sample") is not None
    ]
    displayed_frames = [
        obu["frame_header"]
        for sample in track_samples
        for obu in sample["obus"]
        if "frame_header" in obu
        and obu["frame_header"].get("show_frame")
        and not obu["frame_header"].get("show_existing_frame")
    ]
    if (
        len(track_samples) != len(FRAME_SHA256)
        or [frame.get("order_hint") for frame in displayed_frames] != [0, 1, 2]
    ):
        raise RuntimeError("AV1 displayed frame order differs")
    if displayed_frames[-1].get("frame_type") != "inter":
        raise RuntimeError("the final AV1 frame must be inter coded")


def generate_fixture(output_dir: Path = OUTPUT_DIR, *, check: bool = False) -> str:
    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    encoded = encode_fixture()
    if encoded != encode_fixture():
        raise RuntimeError("inter-intra wedge AVIF encoding is not deterministic")
    if digest(encoded) != ASSET_SHA256:
        raise RuntimeError("AVIF differs from its pinned complete-input hash")
    verify_pillow_observations(encoded)

    output_dir.mkdir(parents=True, exist_ok=True)
    path = output_dir / FILENAME
    if check:
        if not path.is_file() or path.read_bytes() != encoded:
            raise RuntimeError(f"{FILENAME} differs from deterministic regeneration")
        verify_encoded_profile(path)
    else:
        with tempfile.TemporaryDirectory(
            prefix=".avif-interintra-wedge-", dir=output_dir
        ) as temporary:
            staged = Path(temporary) / FILENAME
            staged.write_bytes(encoded)
            verify_encoded_profile(staged)
            os.replace(staged, path)
    return ASSET_SHA256


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT_DIR)
    parser.add_argument(
        "--check", action="store_true", help="verify the existing fixture without writing"
    )
    args = parser.parse_args()
    print(f"{FILENAME}: {generate_fixture(args.output_dir, check=args.check)}")


if __name__ == "__main__":
    main()
