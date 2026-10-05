#!/usr/bin/env python3
"""Generate complete lossless AVIF sequences with unused derived skip references.

All five display samples remain in each container. The fourth sample derives
eligible skip references [0, 6] while its skip-mode flag is disabled. Fixed B32
encoder controls and pinned whole-file hashes preserve the encoded syntax;
these controls do not establish that a block used compound prediction.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import tempfile
from dataclasses import dataclass
from io import BytesIO
from pathlib import Path

import yaml
from PIL import Image

from generate_avif_multitile_motion_fixture import normalize_timestamps
from generate_decode_refs import verify_primary_oracle
from inspect_av1_obus import inspect as inspect_av1
from inspect_avif_bitstreams import (
    inspect as inspect_container,
    parse_boxes,
    unique_box,
)

ROOT = Path(__file__).resolve().parents[1]
OUTPUT_DIR = ROOT / "tests/fixtures/input/images/avif"
SIZE = (64, 64)
FRAME_COUNT = 5
DURATION_MS = 100
ADVANCED = {
    "min-partition-size": "32",
    "max-partition-size": "32",
    "aq-mode": "0",
    "deltaq-mode": "0",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "loopfilter-control": "0",
    "enable-warped-motion": "0",
    "enable-global-motion": "0",
    "enable-interintra-comp": "0",
}
SOURCE_FRAME_SHA256 = (
    "9cad05201f89a7c8e82ecc722fbe8e7d52497c2ddffd74db6ad48a507699ef1e",
    "4772c3dbd88deff0de7f19daddde2fd781ea3e9a338e61d895f01e6b8cfedf70",
    "d9edb14c67dcbf7ba0532350ad285e8d6848d3546ba83dedd73959268be09fa2",
    "2b9cec9720944c88390e2acadb85337e74e4cc5bff99dca20faf98ac25d09dc8",
    "4772c3dbd88deff0de7f19daddde2fd781ea3e9a338e61d895f01e6b8cfedf70",
)


@dataclass(frozen=True)
class Recipe:
    filename: str
    subsampling: str
    chroma: tuple[int, int]
    sha256: str


RECIPES = (
    Recipe(
        "animated_lossless_inter_420_derived_skip_refs_b32x32_64x64.avif",
        "4:2:0", (1, 1),
        "50d8bdd33c5a5e958d5991d25767748088ba3354d35d4b809d52e84a45713c15",
    ),
    Recipe(
        "animated_lossless_inter_i422_derived_skip_refs_b32x32_64x64.avif",
        "4:2:2", (1, 0),
        "0eae260be09a74b98b1f525261a237ef3783105870de44224de91dd65ee5d168",
    ),
    Recipe(
        "animated_lossless_inter_i444_derived_skip_refs_b32x32_64x64.avif",
        "4:4:4", (0, 0),
        "3769c145bb4d97f357a8b3c4660fb8a2f976c682643304e64833d06515901009",
    ),
)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def source_frames() -> list[Image.Image]:
    frames = []
    for state in range(3):
        pixels = bytearray()
        for y in range(SIZE[1]):
            for x in range(SIZE[0]):
                color = (32 + x // 8, 64 + y // 8, 96 + (x + y) // 16)
                if 8 <= x < 40 and 8 <= y < 40:
                    dx, dy, phase = x - 8, y - 8, state * 3
                    color = (
                        48 + ((dx + phase) * 7 + dy * 3) % 144,
                        32 + (dx * 5 + (dy + phase) * 11) % 176,
                        40 + (dx * 13 + dy * 7 + phase * 17) % 160,
                    )
                pixels.extend(color)
        frames.append(Image.frombytes("RGB", SIZE, bytes(pixels)))
    frames.append(Image.blend(frames[1], frames[2], 0.5))
    frames.append(frames[1].copy())
    if tuple(digest(frame.tobytes()) for frame in frames) != SOURCE_FRAME_SHA256:
        raise RuntimeError("lossless derived-skip source frame hashes differ")
    return frames


def encode_fixture(recipe: Recipe) -> bytes:
    frames = source_frames()
    output = BytesIO()
    frames[0].save(
        output,
        format="AVIF",
        save_all=True,
        append_images=frames[1:],
        duration=[DURATION_MS] * FRAME_COUNT,
        loop=0,
        quality=100,
        speed=4,
        max_threads=1,
        subsampling=recipe.subsampling,
        autotiling=False,
        codec="aom",
        advanced=ADVANCED,
    )
    return normalize_timestamps(output.getvalue())


def pillow_observations(encoded: bytes) -> tuple[tuple, ...]:
    # Opening/verifying and decoding use fresh public source objects. Unexpected
    # source failures propagate; file-system failures are never codec outcomes.
    with Image.open(BytesIO(encoded)) as image:
        if image.format != "AVIF" or image.mode != "RGB" or image.size != SIZE:
            raise RuntimeError("derived-skip Pillow image metadata differs")
        if image.n_frames != FRAME_COUNT or not image.is_animated:
            raise RuntimeError("derived-skip Pillow display frame count differs")
    with Image.open(BytesIO(encoded)) as image:
        image.verify()
    observations = []
    with Image.open(BytesIO(encoded)) as image:
        for index in range(image.n_frames):
            image.seek(index)
            image.load()
            if image.mode != "RGB" or image.size != SIZE:
                raise RuntimeError("derived-skip Pillow frame metadata differs")
            if image.info.get("duration") != DURATION_MS:
                raise RuntimeError("derived-skip Pillow frame duration differs")
            observations.append((image.mode, image.size, image.info.copy(), image.tobytes()))
    return tuple(observations)


def verify_encoded_profile(path: Path, recipe: Recipe) -> None:
    encoded = path.read_bytes()
    container = inspect_container(path)
    if len(container["tracks"]) != 1 or container["alpha_track_id"] is not None:
        raise RuntimeError("derived-skip AVIF requires one complete color track")
    track = container["tracks"][0]
    samples = track["samples"]
    if track["handler"] != "pict" or track["timescale"] != 1000:
        raise RuntimeError("derived-skip AVIF track metadata differs")
    if (
        len(samples) != FRAME_COUNT
        or [sample["duration"] for sample in samples] != [DURATION_MS] * FRAME_COUNT
    ):
        raise RuntimeError("derived-skip AVIF sample table differs")
    mdat = unique_box(parse_boxes(encoded, 0, len(encoded)), b"mdat")
    position = mdat.payload_start
    for sample in samples:
        if sample["offset"] != position or sample["length"] <= 0:
            raise RuntimeError("derived-skip AVIF samples are not complete and contiguous")
        position += sample["length"]
        if position > mdat.end:
            raise RuntimeError("derived-skip AVIF sample exceeds its payload")
    if position != mdat.end or mdat.end != len(encoded):
        raise RuntimeError("derived-skip AVIF payload or complete file tail differs")
    report = inspect_av1(path)
    tracked = [s for s in report["samples"] if s["role"] == "track_pict"]
    if [s["identity"]["sample"] for s in tracked] != list(range(FRAME_COUNT)):
        raise RuntimeError("derived-skip AV1 display samples differ")
    sequences = [
        obu["sequence_header"]
        for sample in tracked
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if len(sequences) != 1:
        raise RuntimeError("derived-skip AV1 sequence header count differs")
    sequence = sequences[0]
    profile = (
        sequence["bit_depth"], sequence["monochrome"],
        sequence["subsampling_x"], sequence["subsampling_y"],
    )
    if profile != (8, False, *recipe.chroma):
        raise RuntimeError("derived-skip AV1 depth or chroma layout differs")
    headers = [
        obu["frame_header"]
        for sample in tracked
        for obu in sample["obus"]
        if "frame_header" in obu
        and not obu["frame_header"].get("show_existing_frame", False)
    ]
    if not headers or not all(
        header.get("all_lossless") and not header.get("skip_mode_enabled")
        for header in headers
    ):
        raise RuntimeError("derived-skip AV1 lossless or disabled skip-mode profile differs")
    selected = [
        obu["frame_header"]
        for obu in tracked[3]["obus"]
        if "frame_header" in obu
    ]
    if (
        len(selected) != 1
        or selected[0].get("reference_mode") != "select"
        or selected[0].get("skip_mode_references") != [0, 6]
        or selected[0].get("skip_mode_enabled") is not False
    ):
        raise RuntimeError("derived-skip AV1 unused reference pair differs")


def generate_fixtures(
    output_dir: Path = OUTPUT_DIR, *, check: bool = False
) -> dict[str, str]:
    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    output_dir.mkdir(parents=True, exist_ok=True)
    hashes = {}
    with tempfile.TemporaryDirectory(prefix=".avif-derived-skip-", dir=output_dir) as temporary:
        work = Path(temporary)
        for recipe in RECIPES:
            encoded = encode_fixture(recipe)
            if encoded != encode_fixture(recipe):
                raise RuntimeError(f"{recipe.filename} encoding is not deterministic")
            if digest(encoded) != recipe.sha256:
                raise RuntimeError(f"{recipe.filename} differs from its pinned complete-input hash")
            observations = pillow_observations(encoded)
            if observations != pillow_observations(encoded):
                raise RuntimeError(f"{recipe.filename} Pillow observations are not repeatable")
            path = work / recipe.filename
            path.write_bytes(encoded)
            verify_encoded_profile(path, recipe)
            if check and (output_dir / recipe.filename).read_bytes() != encoded:
                raise RuntimeError(f"{recipe.filename} differs from deterministic regeneration")
            hashes[recipe.filename] = recipe.sha256
        # Complete all three independent source validations before publication.
        if not check:
            for name in hashes:
                os.replace(work / name, output_dir / name)
    return hashes


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT_DIR)
    parser.add_argument(
        "--check", action="store_true",
        help="Verify existing inputs without replacing them",
    )
    args = parser.parse_args()
    for filename, sha256 in generate_fixtures(args.output_dir, check=args.check).items():
        print(f"{filename}: {sha256}")


if __name__ == "__main__":
    main()
