#!/usr/bin/env python3
"""Generate a lossy AVIF sequence with displayed AV1 skip-mode frames."""

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
FILENAME = "animated_lossy_inter_420_skip_mode_enabled_64x64.avif"
SIZE = (64, 64)
FRAME_COUNT = 8
DURATION_MS = 100
ASSET_SHA256 = "98ead811784a1a8434582d6993fdc9891672543b59ed6135fbb0feabc205c8e3"
SOURCE_FRAME_SHA256 = (
    "8e9c6d444964a6e2b5194db55fe31244ea98a1d83365e9e73a90ec7931b253b8",
    "141f9d10c9570a4dcde8a58e10c62c349e46425fec8918435f599893ac42339b",
    "8c6a64a333483c6c460bd723d9da0cd39f3369b2e70d352f6f10db917e564e5f",
    "407b641795b2226679cb4efc87ab7845d2ccb59a1bf075119ee80757c5f8da77",
    "8e9c6d444964a6e2b5194db55fe31244ea98a1d83365e9e73a90ec7931b253b8",
    "141f9d10c9570a4dcde8a58e10c62c349e46425fec8918435f599893ac42339b",
    "8c6a64a333483c6c460bd723d9da0cd39f3369b2e70d352f6f10db917e564e5f",
    "407b641795b2226679cb4efc87ab7845d2ccb59a1bf075119ee80757c5f8da77",
)
REFERENCE_FRAME_SHA256 = (
    "300558e7184a0c5225e41136dde155e8e6e8a5c5e411a864f7d82de4919d3343",
    "96732e22fd07a9e36615852e03fc82230668578265a41e0eadf127622928552d",
    "f55548555c99e63c2cf79eccd829b0d12e81ca04a6eeaad070d6859dfb8c6efd",
    "0a87a57d7cc194ed36e1c28e075b37a462ef787905545d154df893db95a3cf2a",
    "76fab8245921ce12def9e1d41cd61bd7864ac10d8d8893e0720acce0de685ccc",
    "96732e22fd07a9e36615852e03fc82230668578265a41e0eadf127622928552d",
    "5b79346e71f680a919944c20c976639d0d28721c7fa1cff82f924c66626ae9ce",
    "0a87a57d7cc194ed36e1c28e075b37a462ef787905545d154df893db95a3cf2a",
)
ADVANCED = {
    "min-partition-size": "16",
    "max-partition-size": "64",
    "aq-mode": "0",
    "deltaq-mode": "0",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "loopfilter-control": "0",
    "enable-warped-motion": "0",
    "enable-global-motion": "0",
    "enable-interintra-comp": "0",
    "color-primaries": "1",
    "transfer-characteristics": "13",
    "matrix-coefficients": "6",
}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def source_frames() -> list[Image.Image]:
    frames = []
    for frame_index in range(FRAME_COUNT):
        patch_left = 8 + 8 * (frame_index % 4)
        pixels = bytearray()
        for y in range(SIZE[1]):
            for x in range(SIZE[0]):
                color = (
                    (17 * x + 5 * y) % 256,
                    (3 * x + 19 * y) % 256,
                    (7 * x + 11 * y) % 256,
                )
                if patch_left <= x < patch_left + 8 and 24 <= y < 32:
                    color = (240, 40, 190)
                pixels.extend(color)
        frames.append(Image.frombytes("RGB", SIZE, bytes(pixels)))

    actual = tuple(digest(frame.tobytes()) for frame in frames)
    if actual != SOURCE_FRAME_SHA256:
        raise RuntimeError("skip-mode source frame hashes differ")
    return frames


def encode_fixture() -> bytes:
    frames = source_frames()
    output = BytesIO()
    frames[0].save(
        output,
        format="AVIF",
        save_all=True,
        append_images=frames[1:],
        duration=[DURATION_MS] * FRAME_COUNT,
        loop=0,
        quality=45,
        speed=0,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        codec="aom",
        advanced=ADVANCED,
    )
    return normalize_timestamps(output.getvalue())


def verify_pillow_observations(encoded: bytes) -> None:
    with Image.open(BytesIO(encoded)) as image:
        if image.format != "AVIF" or image.mode != "RGB" or image.size != SIZE:
            raise RuntimeError("skip-mode Pillow image metadata differs")
        if image.n_frames != FRAME_COUNT or not image.is_animated:
            raise RuntimeError("skip-mode Pillow frame count differs")

    observations = []
    with Image.open(BytesIO(encoded)) as image:
        for index in range(image.n_frames):
            image.seek(index)
            image.load()
            if image.mode != "RGB" or image.size != SIZE:
                raise RuntimeError("skip-mode Pillow frame metadata differs")
            if image.info.get("duration") != DURATION_MS:
                raise RuntimeError("skip-mode Pillow frame duration differs")
            observations.append(digest(image.tobytes()))
    if tuple(observations) != REFERENCE_FRAME_SHA256:
        raise RuntimeError("skip-mode Pillow frame reference hashes differ")


def verify_encoded_profile(path: Path) -> None:
    container = inspect_container(path)
    if len(container["tracks"]) != 1 or container["alpha_track_id"] is not None:
        raise RuntimeError("skip-mode AVIF requires one complete color track")
    track = container["tracks"][0]
    samples = track["samples"]
    if (
        track["handler"] != "pict"
        or track["timescale"] != 1000
        or len(samples) != FRAME_COUNT
        or [sample["duration"] for sample in samples] != [DURATION_MS] * FRAME_COUNT
    ):
        raise RuntimeError("skip-mode AVIF sequence table differs")

    report = inspect_av1(path)
    tracked = [sample for sample in report["samples"] if sample["role"] == "track_pict"]
    if [sample["identity"]["sample"] for sample in tracked] != list(range(FRAME_COUNT)):
        raise RuntimeError("skip-mode AV1 display samples differ")

    sequence_headers = [
        obu["sequence_header"]
        for sample in tracked
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if not sequence_headers or any(
        (
            header["bit_depth"],
            header["monochrome"],
            header["subsampling_x"],
            header["subsampling_y"],
            header["color_primaries"],
            header["transfer_characteristics"],
            header["matrix_coefficients"],
            header["color_range"],
        )
        != (8, False, 1, 1, 1, 13, 6, 1)
        for header in sequence_headers
    ):
        raise RuntimeError("skip-mode AV1 color profile differs")

    displayed_headers = [
        obu["frame_header"]
        for sample in tracked
        for obu in sample["obus"]
        if "frame_header" in obu
        and obu["frame_header"].get("show_frame")
        and not obu["frame_header"].get("show_existing_frame")
    ]
    enabled = [header for header in displayed_headers if header.get("skip_mode_enabled")]
    if len(enabled) < 3 or any(
        len(header.get("skip_mode_references", [])) != 2 for header in enabled
    ):
        raise RuntimeError("skip-mode AV1 displayed reference profile differs")


def generate_fixture(output_dir: Path = OUTPUT_DIR, *, check: bool = False) -> str:
    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    output_dir.mkdir(parents=True, exist_ok=True)

    encoded = encode_fixture()
    if encoded != encode_fixture():
        raise RuntimeError("skip-mode AVIF encoding is not deterministic")
    if digest(encoded) != ASSET_SHA256:
        raise RuntimeError("skip-mode AVIF differs from its pinned complete-input hash")
    verify_pillow_observations(encoded)

    if check:
        path = output_dir / FILENAME
        if path.read_bytes() != encoded:
            raise RuntimeError(f"{FILENAME} differs from deterministic regeneration")
        verify_encoded_profile(path)
    else:
        with tempfile.TemporaryDirectory(
            prefix=".avif-skip-mode-", dir=output_dir
        ) as temporary:
            path = Path(temporary) / FILENAME
            path.write_bytes(encoded)
            verify_encoded_profile(path)
            os.replace(path, output_dir / FILENAME)
    return ASSET_SHA256


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT_DIR)
    parser.add_argument(
        "--check", action="store_true", help="Verify the existing input fixture"
    )
    args = parser.parse_args()
    print(f"{FILENAME}: {generate_fixture(args.output_dir, check=args.check)}")


if __name__ == "__main__":
    main()
