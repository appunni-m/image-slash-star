#!/usr/bin/env python3
"""Generate input-only monochrome AVIF sequences with mixed-segment key frames."""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile
from typing import NamedTuple

from generate_avif_10bit_lossless_inter_fixture import normalize_sequence_timestamps
from generate_avif_filmgrain_edge_fixtures import require_pinned_oracle
from inspect_av1_obus import inspect

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/fixtures/input/images/avif"


class Fixture(NamedTuple):
    """Input-only native recipe and complete normalized file identity."""

    width: int
    height: int
    depth: int
    speed: int
    restoration: int
    varied_key: bool
    denominator: int
    input_sha256: str


FIXTURES = {
    'animated_mixed_lossless_key_superres_400_8bit_flat_80x48':
        Fixture(80, 48, 8, 7, 1, False, 10,
                "75ceb95aac1814941074dd5972f1ae95b69a4c4fb52b6a3fbecae002d662b206"),
    'animated_mixed_lossless_key_superres_400_8bit_varied_s3_80x48':
        Fixture(80, 48, 8, 3, 0, True, 10,
                "74934829b5040745d8f266b56b07f62ea25f702a7d0b5f16ec29752490a6ccc4"),
    'animated_mixed_lossless_key_superres_400_10bit_varied_s3_80x48':
        Fixture(80, 48, 10, 3, 0, True, 10,
                "7f064d7e8bb6ea2d0fb852db8fca02da7021f5d617a322c00a71c060b2dd16ec"),
    'animated_mixed_lossless_key_superres_400_12bit_varied_s3_80x48':
        Fixture(80, 48, 12, 3, 0, True, 10,
                "da7835a6a45ca4eeb400e250e5f47f24908d5a59caeb69213404344a324b18ae"),
    'animated_mixed_lossless_key_superres_400_8bit_varied_s7_80x48':
        Fixture(80, 48, 8, 7, 0, True, 10,
                "c21b123af4891f15367691d3c167de90da03b19045b5a94c07da7fac147f8d05"),
    'animated_mixed_lossless_key_superres_400_8bit_varied_s3_40x16':
        Fixture(40, 16, 8, 3, 0, True, 10,
                "37426534a13298b0ccd46fcd79f6ed9def527b20ef0789c2cbeee16bedb1207b"),
    'animated_mixed_lossless_key_superres_400_10bit_varied_s3_40x16':
        Fixture(40, 16, 10, 3, 0, True, 10,
                "65da92bb19404a088ded5f6912318c54e5d7063862a683fd9a8df0a8e236b306"),
    'animated_mixed_lossless_key_superres_400_12bit_varied_s3_40x16':
        Fixture(40, 16, 12, 3, 0, True, 10,
                "c86a971a0cd76a5081ef586af951c2e5461d4549430925ab74b065b3a3f03bd9"),
    'animated_mixed_lossless_key_superres_400_8bit_varied_s3_60x20':
        Fixture(60, 20, 8, 3, 0, True, 10,
                "bd99fe703376390ce1097eedb4492cfd9018bba5a8e8187f20ec35fc8dfb81f3"),
    'animated_mixed_lossless_key_superres_400_10bit_varied_s3_60x20':
        Fixture(60, 20, 10, 3, 0, True, 10,
                "41f918afc0b1a4c33e40e2b302a8dc52fdaa50c0d017a55f72cb2a4a4efff40b"),
    'animated_mixed_lossless_key_superres_400_12bit_varied_s3_60x20':
        Fixture(60, 20, 12, 3, 0, True, 10,
                "7609b307a9adc128386fa87380cea5dca007166a17ca3e5bd93859fe6e19e2e0"),
    'animated_mixed_lossless_key_superres_400_8bit_varied_s3_128x128_d16':
        Fixture(128, 128, 8, 3, 0, True, 16,
                "8538f6a8513c13f0dfe4f4efdce8dca0dc8f0e214daf2c0b711bda472421f953"),
    'animated_mixed_lossless_key_superres_400_10bit_varied_s3_128x128_d16':
        Fixture(128, 128, 10, 3, 0, True, 16,
                "4ed6ed92bf50fba6647fa2a9f58dce84f140ddba09347c66d4fd1d3020a76730"),
    'animated_mixed_lossless_key_superres_400_12bit_varied_s3_128x128_d16':
        Fixture(128, 128, 12, 3, 0, True, 16,
                "21bd2732b82cf7e9a213d2b1e5143432f16026fbe78d136182e69051d1099700"),
    'animated_mixed_lossless_key_superres_400_8bit_varied_s3_128x128_d10':
        Fixture(128, 128, 8, 3, 0, True, 10,
                "ec9f489ed9eec383c1a333988a5e3b61e6efbc9373bb6808612e6ce8024cda55"),
    'animated_mixed_lossless_key_superres_400_10bit_varied_s3_128x128_d10':
        Fixture(128, 128, 10, 3, 0, True, 10,
                "1551d1351886eb0cafd4c3ab1aeb72da3049488f3f014ee4407b5da2402c4127"),
    'animated_mixed_lossless_key_superres_400_12bit_varied_s3_128x128_d10':
        Fixture(128, 128, 12, 3, 0, True, 10,
                "e509be8ff85da0bb8b9553c9db58819349e8751cd5bab789a2e683685e4b79ff"),
    'animated_mixed_lossless_key_superres_400_8bit_varied_s3_128x20_d10':
        Fixture(128, 20, 8, 3, 0, True, 10,
                "835ed4d5832c6798a5c2eb022121e2a501bcbfede889295a6195eab01366e99d"),
    'animated_mixed_lossless_key_superres_400_10bit_varied_s3_128x20_d10':
        Fixture(128, 20, 10, 3, 0, True, 10,
                "69dc712bb6f7fd326ee2f453a218413e10a1c0a58465ba147bf4271289d9eb66"),
    'animated_mixed_lossless_key_superres_400_12bit_varied_s3_128x20_d10':
        Fixture(128, 20, 12, 3, 0, True, 10,
                "548b05e77918217ca3dd983c10706039e86f1e4b007d76e66fc9b0ef2e12b567"),
    'animated_mixed_lossless_key_superres_400_8bit_varied_s3_80x20_d9':
        Fixture(80, 20, 8, 3, 0, True, 9,
                "49cefe1da0c4e2c56c45484336c4099cd78923675071d0343c2fab5f9a5cfb62"),
    'animated_mixed_lossless_key_superres_400_10bit_varied_s3_80x20_d9':
        Fixture(80, 20, 10, 3, 0, True, 9,
                "5470195c2ac498c0d317757835cd58e211ac130878a6a4c9e548cc6049fc89b9"),
    'animated_mixed_lossless_key_superres_400_12bit_varied_s3_80x20_d9':
        Fixture(80, 20, 12, 3, 0, True, 9,
                "734822466824cf283cc5b6cf6c348fafeb519d4be861bc7a61b103ceb9bd701b"),
    'animated_mixed_lossless_key_superres_400_8bit_varied_s3_112x20_d9':
        Fixture(112, 20, 8, 3, 0, True, 9,
                "bdfe1a8ab1bf9dad7c91f4dbd857ab1cc2378fc272ded17960bf5e2ccd1a2503"),
    'animated_mixed_lossless_key_superres_400_10bit_varied_s3_112x20_d9':
        Fixture(112, 20, 10, 3, 0, True, 9,
                "662344af2caed8f4110951d59819677496cf7c96a26a3a8a61866acb8ae5b306"),
    'animated_mixed_lossless_key_superres_400_12bit_varied_s3_112x20_d9':
        Fixture(112, 20, 12, 3, 0, True, 9,
                "8074f00bc56bcd5b67a9c1cbb2fcaa7493db8e8a63ffe21cbd5eb265a5e2e421"),
}


def validate(path: Path, name: str) -> None:
    """Check file identity and independently parsed mixed-segment frame syntax."""
    spec = FIXTURES[name]
    if hashlib.sha256(path.read_bytes()).hexdigest() != spec.input_sha256:
        raise RuntimeError(f"{name}: complete input differs from pinned recipe")
    report = inspect(path)
    headers = [obu["sequence_header"] for sample in report["samples"]
               for obu in sample["obus"] if "sequence_header" in obu]
    expected = {"bit_depth": spec.depth, "max_width": spec.width, "max_height": spec.height,
                "monochrome": True, "enable_superres": True,
                "enable_restoration": False}
    if not headers or any(any(header.get(key) != value for key, value in expected.items())
                          for header in headers):
        raise RuntimeError(f"{name}: sequence features differ from the planar recipe")
    frames = [obu["frame_header"] for sample in report["samples"]
              if sample.get("role") == "track_pict" for obu in sample["obus"]
              if "frame_header" in obu]
    if len(frames) != 2:
        raise RuntimeError(f"{name}: expected one key and one inter frame")
    coded_width = max(min(spec.width, 16),
                      (spec.width * 8 + spec.denominator // 2) // spec.denominator)
    for index, frame in enumerate(frames):
        segmentation = frame.get("segmentation", {})
        base = frame.get("quantization", {}).get("base")
        qindices = {max(0, min(255, base + segment["delta_q"]))
                    for segment in segmentation.get("segments", [])} if base is not None else set()
        if (frame.get("frame_type") != ("key" if index == 0 else "inter")
                or frame.get("frame_width") != (spec.width if index == 0 else coded_width)
                or frame.get("upscaled_width") != spec.width
                or frame.get("frame_height") != spec.height
                or frame.get("superres_enabled") != bool(index)
                or frame.get("all_lossless") is not False
                or segmentation.get("enabled") is not True
                or segmentation.get("update_map") is not True
                or segmentation.get("temporal") is not False
                or 0 not in qindices or not any(qindex > 0 for qindex in qindices)
                or frame.get("restoration", {}).get("types") != [0, 0, 0]
                or frame.get("loop_filter", {}).get("level_y") != [0, 0]):
            raise RuntimeError(f"{name}: frame lacks the neutral mixed-segment features")


def generate(encoder: Path, destination: Path, name: str) -> None:
    spec = FIXTURES[name]
    with tempfile.TemporaryDirectory(prefix="avif-monochrome-mixed-key-") as temporary:
        outputs = []
        for attempt in range(2):
            path = Path(temporary) / f"{attempt}.avif"
            command = [str(encoder), str(path), str(spec.width), str(spec.height), str(spec.depth), "400",
                       "45", str(spec.speed), str(spec.denominator), str(spec.restoration)]
            if spec.varied_key:
                command.append("mixed-key")
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
        if version.stdout.strip() != "libavif 1.4.1 (aom [enc/dec]:v3.13.2)":
            raise RuntimeError(f"pinned producer required: {version.stdout!r}")
    names = [args.variant] if args.variant else FIXTURES
    for name in names:
        path = args.output_dir / (name + ".avif")
        if args.check:
            validate(path, name)
        else:
            assert encoder is not None
            generate(encoder, path, name)
    print(f"monochrome mixed-key inputs: {len(names)} "
          f"{'checked' if args.check else 'reproduced twice'}")


if __name__ == "__main__":
    main()
