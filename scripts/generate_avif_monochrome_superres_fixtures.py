#!/usr/bin/env python3
"""Generate input-only ordinary monochrome AVIF super-resolution sequences."""
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
FIXTURES = {
    "animated_monochrome_neutral_superres_8bit_80x48":
        (8, 0, "b7daf431bafe9eb89a9c2b4f34eda3ac50d8477f9591ec85a8fd4cbf8c398f30"),
    "animated_monochrome_neutral_superres_10bit_80x48":
        (10, 0, "b73ca9f56bb2f60dbe29e85c5cdb687982e6d2cbbc0e93de362312f580881177"),
    "animated_monochrome_neutral_superres_12bit_80x48":
        (12, 0, "116e22846e49619fec712071799ff602b2ee7f4178bb3cd4920ebd7bbd56feb1"),
    "animated_monochrome_sgr_superres_8bit_80x48":
        (8, 1, "9753ecbfbe6c924e1199968ebd44064656562f0265252511df730b22030b3924"),
    "animated_monochrome_sgr_superres_10bit_80x48":
        (10, 1, "e3703a35efc5085564f5c2e570808bb72a5e6e4131442017ab5dca5a0b198afb"),
}


def validate(path: Path, name: str) -> None:
    """Validate complete input identity and independently parsed AV1 features."""
    depth, restoration, expected_sha = FIXTURES[name]
    if hashlib.sha256(path.read_bytes()).hexdigest() != expected_sha:
        raise RuntimeError(f"{name}: complete input differs from pinned recipe")
    report = inspect(path)
    headers = [obu["sequence_header"] for sample in report["samples"]
               for obu in sample["obus"] if "sequence_header" in obu]
    expected = {"bit_depth": depth, "max_width": 80, "max_height": 48,
                "monochrome": True, "enable_superres": True, "enable_restoration": bool(restoration)}
    if not headers or any(any(header.get(k) != v for k, v in expected.items()) for header in headers):
        raise RuntimeError(f"{name}: sequence features differ from the planar recipe")
    frames = [obu["frame_header"] for sample in report["samples"]
              if sample.get("role") == "track_pict" for obu in sample["obus"]
              if "frame_header" in obu]
    if len(frames) != 2:
        raise RuntimeError(f"{name}: expected one key and one inter frame")
    for index, frame in enumerate(frames):
        expected_restoration = 2 if restoration and index == 1 else 0
        if (frame.get("frame_type") != ("key" if index == 0 else "inter")
                or frame.get("frame_width") != (80 if index == 0 else 64)
                or frame.get("upscaled_width") != 80 or frame.get("frame_height") != 48
                or frame.get("superres_enabled") != bool(index)
                or frame.get("segmentation", {}).get("enabled") is not False
                or frame.get("all_lossless") is not False
                or frame.get("restoration", {}).get("types") != [expected_restoration, 0, 0]
                or frame.get("loop_filter", {}).get("level_y") != [0, 0]):
            raise RuntimeError(f"{name}: frame lacks the ordinary super-resolution features")


def generate(encoder: Path, destination: Path, name: str) -> None:
    depth, restoration, _ = FIXTURES[name]
    with tempfile.TemporaryDirectory(prefix="avif-monochrome-neutral-superres-") as temporary:
        outputs = []
        for attempt in range(2):
            path = Path(temporary) / f"{attempt}.avif"
            command = [str(encoder), str(path), "80", "48", str(depth),
                       "400", "45", "3", "10", str(restoration), "ordinary"]
            result = subprocess.run(command, capture_output=True, text=True, check=False)
            if result.returncode != 0:
                raise RuntimeError(f"native producer failed ({result.returncode}): {result.stderr}")
            data = normalize_sequence_timestamps(path.read_bytes())
            path.write_bytes(data)
            validate(path, name)
            outputs.append(data)
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
    print(f"ordinary monochrome super-resolution inputs: {len(names)} "
          f"{'checked' if args.check else 'reproduced twice'}")


if __name__ == "__main__":
    main()
