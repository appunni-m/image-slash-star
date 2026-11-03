#!/usr/bin/env python3
"""Generate and validate an AVIF still with active loop-filter sharpness."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from PIL import Image

from generate_avif_filmgrain_edge_fixtures import require_pinned_oracle
from inspect_av1_obus import inspect


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = "avif_loopfilter_sharpness_420_8bit_still_128x128"
INPUT = ROOT / "tests" / "fixtures" / "input" / "images" / "avif" / f"{FIXTURE}.avif"
OUTPUT = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"


def validate(path: Path) -> None:
    document = inspect(path)
    frames = [
        obu["frame_header"]
        for sample in document["samples"]
        for obu in sample["obus"]
        if isinstance(obu.get("frame_header"), dict)
    ]
    if len(frames) != 1:
        raise RuntimeError(f"{path.name}: expected one coded frame, found {len(frames)}")
    frame = frames[0]
    loop_filter = frame.get("loop_filter", {})
    if (
        frame.get("frame_type") != "key"
        or loop_filter.get("sharpness") != 7
        or not any(loop_filter.get("level_y", []))
        or loop_filter.get("level_u", 0) == 0
        or loop_filter.get("level_v", 0) == 0
    ):
        raise RuntimeError(f"{path.name}: active sharpness-7 Y/U/V deblocking is absent")

    with Image.open(path) as image:
        image.load()
        if image.size != (128, 128) or image.mode != "RGB":
            raise RuntimeError(f"{path.name}: Pillow did not decode a 128x128 RGB still")
    with Image.open(path) as image:
        image.verify()


def generate(encoder: Path, destination: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="avif-loopfilter-sharpness-") as temporary:
        outputs = []
        for name in ("first.avif", "second.avif"):
            path = Path(temporary) / name
            result = subprocess.run(
                [str(encoder), str(path)], capture_output=True, text=True, check=False
            )
            if result.returncode != 0:
                raise RuntimeError(f"native producer failed: {result.stderr}")
            validate(path)
            outputs.append(path.read_bytes())
        if outputs[0] != outputs[1]:
            raise RuntimeError("independent pinned AVIF encodes differ")
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(outputs[0])
        print(f"input sha256: {hashlib.sha256(outputs[0]).hexdigest()}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--encoder", type=Path)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT)
    args = parser.parse_args()
    require_pinned_oracle()

    destination = args.output_dir / f"{FIXTURE}.avif"
    if args.check:
        if not destination.is_file():
            raise RuntimeError(f"missing AVIF fixture: {destination}")
        validate(destination)
        print(f"AV1 loop-filter sharpness fixture OK: {destination}")
        return

    if args.encoder is None:
        parser.error("--encoder is required to generate native inputs")
    encoder = args.encoder.expanduser().resolve(strict=True)
    version = subprocess.run(
        [str(encoder), "--version"], capture_output=True, text=True, check=True
    )
    expected = "libavif 1.4.1 (aom [enc]:v3.13.2)"
    if version.stdout.strip() != expected:
        raise RuntimeError(f"pinned producer required: {version.stdout.strip()!r}")
    generate(encoder, destination)


if __name__ == "__main__":
    main()
