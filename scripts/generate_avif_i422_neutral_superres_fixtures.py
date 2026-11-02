#!/usr/bin/env python3
"""Generate input-only ordinary 8-bit I422 super-resolution AVIF sequences."""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from generate_avif_10bit_lossless_inter_fixture import normalize_sequence_timestamps
from generate_avif_filmgrain_edge_fixtures import require_pinned_oracle
from inspect_av1_obus import inspect

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/fixtures/input/images/avif"
# Display width, height, denominator, and complete timestamp-normalized input SHA.
FIXTURES = {
    "animated_lossy_inter_422_neutral_superres_8bit_80x48_d10":
        (80, 48, 10, "147ddc1287cbf44bdfc591190ef2b2212b6a668961d8ad1f9dfe7c2fe76e111f"),
    "animated_lossy_inter_422_neutral_superres_8bit_128x64_d16":
        (128, 64, 16, "3a8c24812f32856b16d670ef2c3b013f7971673b44c2ca077594ea217c2334d5"),
    "animated_lossy_inter_422_neutral_superres_8bit_40x16_d10":
        (40, 16, 10, "a2e188c2232952bccff3d1d9d19ee1614b381193a9a4638be3af7b3dff4d3e94"),
    "animated_lossy_inter_422_neutral_superres_8bit_80x48_d9":
        (80, 48, 9, "6c8a1ea5d23ef79d4261d585b520918deb4cf0d4fce939dd1563fafd5400838f"),
}


def validate(path: Path, name: str) -> None:
    """Verify input identity and independently parsed sequence/frame geometry."""
    width, height, denominator, expected_sha = FIXTURES[name]
    if hashlib.sha256(path.read_bytes()).hexdigest() != expected_sha:
        raise RuntimeError(f"{name}: complete input differs from pinned recipe")
    report = inspect(path)
    headers = [obu["sequence_header"] for sample in report["samples"]
               for obu in sample["obus"] if "sequence_header" in obu]
    expected = {"bit_depth": 8, "max_width": width, "max_height": height,
                "monochrome": False, "subsampling_x": True, "subsampling_y": False,
                "enable_superres": True, "enable_restoration": False}
    if not headers or any(any(header.get(key) != value for key, value in expected.items())
                          for header in headers):
        raise RuntimeError(f"{name}: sequence features differ from the planar recipe")
    frames = [obu["frame_header"] for sample in report["samples"]
              if sample.get("role") == "track_pict" for obu in sample["obus"]
              if "frame_header" in obu]
    if len(frames) != 2:
        raise RuntimeError(f"{name}: expected one key and one inter frame")
    coded_width = (width * 8 + denominator // 2) // denominator
    for index, frame in enumerate(frames):
        if (frame.get("frame_type") != ("key" if index == 0 else "inter")
                or frame.get("frame_width") != (width if index == 0 else coded_width)
                or frame.get("upscaled_width") != width or frame.get("frame_height") != height
                or frame.get("superres_enabled") != bool(index)
                or frame.get("segmentation", {}).get("enabled") is not False
                or frame.get("all_lossless") is not False
                or frame.get("restoration", {}).get("types") != [0, 0, 0]
                or frame.get("loop_filter", {}).get("level_y") != [0, 0]
                or frame.get("loop_filter", {}).get("level_u") != 0
                or frame.get("loop_filter", {}).get("level_v") != 0):
            raise RuntimeError(f"{name}: frame lacks the ordinary I422 super-resolution features")


def generate(encoder: Path, destination: Path, name: str) -> None:
    width, height, denominator, _ = FIXTURES[name]
    with tempfile.TemporaryDirectory(prefix="avif-i422-neutral-superres-") as temporary:
        outputs = []
        for attempt in range(2):
            path = Path(temporary) / f"{attempt}.avif"
            command = [str(encoder), str(path), str(width), str(height), "8",
                       "422", "45", "3", str(denominator), "0", "ordinary"]
            result = subprocess.run(command, capture_output=True, text=True, check=False)
            if result.returncode != 0:
                raise RuntimeError(f"native producer failed ({result.returncode}): {result.stderr}")
            path.write_bytes(normalize_sequence_timestamps(path.read_bytes()))
            validate(path, name)
            outputs.append(path.read_bytes())
        if outputs[0] != outputs[1]:
            raise RuntimeError(f"{name}: independently normalized encodes differ")
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(outputs[0])


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--encoder", type=Path)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT)
    parser.add_argument("--variant", choices=tuple(FIXTURES))
    args = parser.parse_args()
    require_pinned_oracle()
    encoder = None
    if not args.check:
        if args.encoder is None:
            parser.error("--encoder is required to generate native inputs")
        encoder = args.encoder.expanduser().resolve(strict=True)
        version = subprocess.run([str(encoder), "--version"], capture_output=True, text=True, check=True)
        if version.stdout.strip() != "libavif 1.4.1 (aom [enc]:v3.13.2)":
            raise RuntimeError(f"pinned producer required: {version.stdout!r}")
    names = [args.variant] if args.variant else FIXTURES
    for name in names:
        path = args.output_dir / (name + ".avif")
        if args.check:
            validate(path, name)
        else:
            assert encoder is not None
            generate(encoder, path, name)
    print(f"ordinary I422 neutral super-resolution inputs: {len(names)} "
          f"{'checked' if args.check else 'reproduced twice'}")


if __name__ == "__main__":
    main()
