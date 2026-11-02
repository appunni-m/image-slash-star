#!/usr/bin/env python3
"""Generate complete monochrome AVIF multi-superblock and partial-edge inputs.

Repeated pinned native encodes supply independent 8/10/12-bit planar sources.
Construction reads no expected pixels and never executes Rust. Exact public
frames and loop observations come from the normal live reference workflows.
"""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct
import subprocess
import tempfile

from PIL import Image

from generate_avif_filmgrain_edge_fixtures import require_pinned_oracle
from generate_avif_wide_mode2_fixtures import VARIANTS, encode_native
from inspect_av1_obus import inspect

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/fixtures/input/images/avif"
# Complete-input/source identity pins only; expected outcomes are independent.
FIXTURE_SHA256 = {'animated_lossy_wide_i400_10bit_mode2_deep16_b128x128': 'cb17bf443b3d4a623a7165bf797cf02fdebb5056ce54f3ecf4824cee98d08b75',
 'animated_lossy_wide_i400_10bit_mode2_mixed_128x132': '348d2ae12759f9a9fe2778ee2efd978a82e99173aa3a6c4763ed3de2a6b50c62',
 'animated_lossy_wide_i400_10bit_mode2_mixed_132x128': 'b4cafd28775d59196645a837bb978a3690d7f5a660b38282b904226b58b4238d',
 'animated_lossy_wide_i400_10bit_mode2_mixed_252x256': '9e9fc446aa79ea3767000e1e798b290f6e163c47e62c3e46fe1ab74b4a7619bb',
 'animated_lossy_wide_i400_10bit_mode2_mixed_256x252': '6dac20ec493d4efc699473aeb5bd438e17af5261dda8be52eab71ea053a6fae7',
 'animated_lossy_wide_i400_10bit_mode2_mixed_b128x128': 'bc13f8c23e3cd4c6bf4a1ec23f53f407b4ebb45aba9fd277f3557c3173cb3150',
 'animated_lossy_wide_i400_10bit_mode2_split32_b128x128': 'd15cd04681be4e6c7605a1488252fb276558086cfaa835a287bf2959731b0288',
 'animated_lossy_wide_i400_12bit_mode2_deep16_b128x128': '7c2868e05c761c4f5aa356eb61021bb309035d5a0adc81317c9c46f387837a5f',
 'animated_lossy_wide_i400_12bit_mode2_mixed_128x132': 'd28a062e3ce117594f296c051289f052b072883c6ca5160d96d561b6d432e159',
 'animated_lossy_wide_i400_12bit_mode2_mixed_132x128': '94f35e81abcf02ab24e1e5156e5c6844ab3f7af6df0bd0e9eac1e84745461402',
 'animated_lossy_wide_i400_12bit_mode2_mixed_252x256': '87f954a0ea31e07eb6f396c3f21cb1573a1c6395534cf636bf79a5b6f4144887',
 'animated_lossy_wide_i400_12bit_mode2_mixed_256x252': 'e784414f0f68cbcc113703de367828f7579a380bb59c6826b54fffd1f2f9138f',
 'animated_lossy_wide_i400_12bit_mode2_mixed_b128x128': '637f3f3d70346c5ea629af8a0aab5e48e4e3d5d8d66b251f9abb7d1efb80f690',
 'animated_lossy_wide_i400_12bit_mode2_split32_b128x128': 'e3d377a55d86099f6f081dd4f956729dd0e831a810c375cefff2b7a19c916cf8',
 'animated_lossy_wide_i400_8bit_mode2_deep16_b128x128': '902c108ea984964ec6a3ca701c3636fda239b255529d26074ff6cde88814ce67',
 'animated_lossy_wide_i400_8bit_mode2_mixed_128x132': 'a97d0523087de28964dc8c45502ecabeca833605936332db897c695feb9bf0f4',
 'animated_lossy_wide_i400_8bit_mode2_mixed_132x128': 'fccebfffe74243614b8cb99522a31d97d6110513ca9fc36d13ed666bb5eaa6a4',
 'animated_lossy_wide_i400_8bit_mode2_mixed_252x256': '8a130511632143469866dedbd41fea5abbd998784cb127ff5b429f8979cfa9a7',
 'animated_lossy_wide_i400_8bit_mode2_mixed_256x252': '11d43ae00445eea9b4779cf7439ec4ab7ba64756a77b6b50276f9f59208fb9fa',
 'animated_lossy_wide_i400_8bit_mode2_mixed_b128x128': 'f9d4cbc2f1f176f6e373ce840f0c0e3d3d3ceb61357345e8c105abec3c24ccc0',
 'animated_lossy_wide_i400_8bit_mode2_split32_b128x128': '81021b030555153e49c564baeee9f1f41e876af43fab2c05a8a3d329e5d5f153'}
SOURCE_SHA256 = {'animated_lossy_wide_i400_10bit_mode2_deep16_b128x128': '1520671781476e9b740c620184401df519af7bfce88479cda89e87335b1fba67',
 'animated_lossy_wide_i400_10bit_mode2_mixed_128x132': '451c7c4c4f671b0edc5fed1b39798759f01dc311416a3a20d3c92bffdbca93e5',
 'animated_lossy_wide_i400_10bit_mode2_mixed_132x128': 'cf13f3a5565bb91a8ce85035f83dde854f5843fac0c8ae06b003a8e0cea65ad7',
 'animated_lossy_wide_i400_10bit_mode2_mixed_252x256': '2a86f282a1ed85487e6109896ac4a23c20d0ba5ae3ccfaa689c50293eaa43d9a',
 'animated_lossy_wide_i400_10bit_mode2_mixed_256x252': 'af7b33adb59849ac9b51f4608132faa40f747288ca4729f09174d90b257ecd12',
 'animated_lossy_wide_i400_10bit_mode2_mixed_b128x128': 'ba0805929c769c848e880934c27a097df1d6ed98c84b6116b8bcb44c215ec4cb',
 'animated_lossy_wide_i400_10bit_mode2_split32_b128x128': '8113bc482d42f5829d854f3c3d77f3dafd1200feb3cf8cad4e2c3c86c74da261',
 'animated_lossy_wide_i400_12bit_mode2_deep16_b128x128': 'd388bd8145b15b78dd7334964cbb0765bf94595d9944c889c6b0d8118b07f2dd',
 'animated_lossy_wide_i400_12bit_mode2_mixed_128x132': 'f77a970caf9014f266df95057324703224bb2e1ad06a2188192f84081f2b6baf',
 'animated_lossy_wide_i400_12bit_mode2_mixed_132x128': '87d2dd642800fd271883d5cb4094b7b3d748a6d90ac4700b8e50c7b055eca832',
 'animated_lossy_wide_i400_12bit_mode2_mixed_252x256': '6443fbef69aeda431b20c5822a4f036f957ca5f644327e435b0d348eb47925d1',
 'animated_lossy_wide_i400_12bit_mode2_mixed_256x252': 'b3f624bd9cc57499fdea7a3b0c1418d750191b1a087d671c4c4c816a801d3ba6',
 'animated_lossy_wide_i400_12bit_mode2_mixed_b128x128': '0576d4f2f05006d3b0e700585790baf348a5f0fb2818ac2678e2db752568f165',
 'animated_lossy_wide_i400_12bit_mode2_split32_b128x128': '48682c7b8d5f621719d4970ab105c0eb312cfa278902df0c1c62f9a4503cd8b6',
 'animated_lossy_wide_i400_8bit_mode2_deep16_b128x128': 'be55375121b8d8cf30111a6c32a3a7f2203060d836ca29bc78e6a8d77b746968',
 'animated_lossy_wide_i400_8bit_mode2_mixed_128x132': '8d4d269779cfe395feb777ede7d2e9e2ee531616a3477ec2137490ab67058345',
 'animated_lossy_wide_i400_8bit_mode2_mixed_132x128': 'ed47a6d91d26e498f5b9410bfe1014b571be0766ab880aeaacfdeecdeba13e3b',
 'animated_lossy_wide_i400_8bit_mode2_mixed_252x256': '9124b9d1db36d1f04513d4c0c177a9190747719fafbecdbf064ec9b1f25a7345',
 'animated_lossy_wide_i400_8bit_mode2_mixed_256x252': '4d3ba3c80aa96a07774500698cdc762c99282a437bfd5164ef3be9a3b6fb8568',
 'animated_lossy_wide_i400_8bit_mode2_mixed_b128x128': '4b939fbdfffb531e8a888bf13ba8cc51ee45e679d31e791feb99f895b364ee4c',
 'animated_lossy_wide_i400_8bit_mode2_split32_b128x128': '04506d73427fc372e6e9426bc5d70a108afbe8a110f57acdbd926f034b8a3067'}


def recipes():
    """Include four-superblock frames and partial right/bottom coded edges."""
    shapes = [(256, 256, variant) for variant in VARIANTS]
    shapes.extend((width, height, "mixed") for width, height in ((132, 128), (128, 132), (252, 256), (256, 252)))
    for depth in (8, 10, 12):
        for width, height, variant in shapes:
            suffix = "b128x128" if (width, height) == (256, 256) else f"{width}x{height}"
            name = f"animated_lossy_wide_i400_{depth}bit_mode2_{variant}_{suffix}"
            yield name, depth, width, height, variant


def source_y4m(depth: int, width: int, height: int, variant: str) -> bytes:
    """Crop input stimuli at the origin; retain fractional high-depth samples."""
    tag = "mono" if depth == 8 else f"mono{depth}"
    scale = 1 << (depth - 8)
    data = bytearray(f"YUV4MPEG2 W{width} H{height} F10:1 Ip A1:1 C{tag} XCOLORRANGE=FULL\n".encode())
    for frame in VARIANTS[variant][0]():
        data.extend(b"FRAME\n")
        gray = frame.getchannel("R").tobytes()
        samples = [min((1 << depth) - 1, gray[y * 256 + x] * scale + (x + 3 * y) % scale)
                   for y in range(height) for x in range(width)]
        data.extend(bytes(samples) if depth == 8 else struct.pack("<" + "H" * len(samples), *samples))
    return bytes(data)


def validate(path: Path, name: str, depth: int, width: int, height: int, variant: str) -> None:
    """Verify a complete input, its coded geometry and live Pillow metadata."""
    if hashlib.sha256(path.read_bytes()).hexdigest() != FIXTURE_SHA256[name]:
        raise ValueError(f"complete-input digest differs: {name}")
    report = inspect(path)
    obus = [obu for sample in report["samples"] if sample["role"] == "track_pict" for obu in sample["obus"]]
    sequence = next(obu["sequence_header"] for obu in obus if "sequence_header" in obu)
    frames = [obu["frame_header"] for obu in obus if "frame_header" in obu]
    coded = [frame for frame in frames if not frame["show_existing_frame"]]
    count = 5 if variant == "deep16" else 2
    if (not sequence["monochrome"] or sequence["bit_depth"] != depth
            or not sequence["use_128x128_superblock"] or not sequence["subsampling_x"]
            or not sequence["subsampling_y"] or sum(frame["show_frame"] for frame in frames) != count
            or any((frame["frame_width"], frame["frame_height"]) != (width, height) for frame in coded)):
        raise ValueError(f"complete-input syntax differs: {name}")
    with Image.open(path) as image:
        if image.format != "AVIF" or image.size != (width, height) or image.n_frames != count:
            raise ValueError(f"live Pillow metadata differs: {name}")
        for index in range(count):
            image.seek(index)
            image.load()
            if image.info.get("duration") != 100:
                raise ValueError(f"live Pillow duration differs: {name}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, help="libavif 1.4.1 / libaom 3.13.2 avifenc for regeneration")
    parser.add_argument("--output-dir", type=Path, default=OUTPUT)
    parser.add_argument("--check", action="store_true", help="read retained inputs without encoding or rewriting")
    args = parser.parse_args()
    require_pinned_oracle()
    if set(FIXTURE_SHA256) != set(SOURCE_SHA256) or set(FIXTURE_SHA256) != {recipe[0] for recipe in recipes()}:
        raise ValueError("input/source pins and recipes differ")
    encoder = args.encoder.resolve() if args.encoder else None
    if not args.check:
        if encoder is None or "Version: 1.4.1 (aom [enc]:3.13.2)" not in subprocess.check_output([str(encoder), "--version"], text=True):
            raise ValueError("regeneration requires the pinned libavif/libaom encoder")
        args.output_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="avif-mono-wide-") as directory:
        temporary = Path(directory)
        for name, depth, width, height, variant in recipes():
            path = args.output_dir / f"{name}.avif"
            if not args.check:
                source = temporary / "source.y4m"
                source.write_bytes(source_y4m(depth, width, height, variant))
                if hashlib.sha256(source.read_bytes()).hexdigest() != SOURCE_SHA256[name]:
                    raise ValueError(f"planar source digest differs: {name}")
                data = encode_native(encoder, source, temporary / "run0.avif", variant)
                repeated = encode_native(encoder, source, temporary / "run1.avif", variant)
                if data != repeated or hashlib.sha256(data).hexdigest() != FIXTURE_SHA256[name]:
                    raise ValueError(f"repeated native encode differs: {name}")
                if path.exists() and path.read_bytes() != data:
                    raise ValueError(f"refusing to replace different fixture: {path}")
                path.write_bytes(data)
            validate(path, name, depth, width, height, variant)
    print(f"{'Checked' if args.check else 'Generated'} {len(FIXTURE_SHA256)} complete monochrome AVIF inputs")


if __name__ == "__main__":
    main()
