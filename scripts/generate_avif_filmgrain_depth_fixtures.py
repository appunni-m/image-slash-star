#!/usr/bin/env python3
"""Generate complete AVIF film-grain depth, clipping and inactive-plane inputs.

Sources are deterministic full-range planar Y4M samples. The pinned native
encoder supplies lossless AV1 tiles; checked grain-header rewrites preserve
those tiles. Expected pixels come independently from the pinned Pillow oracle.
Use --encoder PATH to regenerate; --check needs only the retained inputs.
"""
from __future__ import annotations

import argparse
from copy import deepcopy
from io import BytesIO
from pathlib import Path
import hashlib
import struct
import subprocess
import tempfile

from PIL import Image

from generate_avif_filmgrain_edge_fixtures import (
    encode_update_grain,
    enable_grain_overlap,
    enable_restricted_range_clipping,
    frame_location,
    replace_grain_sentence,
    require_pinned_oracle,
)
from generate_avif_filmgrain_reference_reuse import append_bits
from inspect_av1_obus import inspect

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/fixtures/input/images/avif"
# Complete-input digests; no expected pixels participate in construction.
FIXTURES = {
    "filmgrain_422_8bit_u_only_64x64": (8, "422", "u_only",
        "bac813e438e46bc8065331cec1d4ebf648742ee823140d30c8dc662f1a43ad35"),
    "filmgrain_422_8bit_v_only_64x64": (8, "422", "v_only",
        "c42471e8fcb995350148c3effa8d0f22bc55be705e2d80f98be5027623faa69a"),
    "filmgrain_444_8bit_u_only_64x64": (8, "444", "u_only",
        "0fd616f3a8e4f8a631dd3072362d6e8bd530d69807f6301b1924353ede24848e"),
    "filmgrain_444_8bit_v_only_64x64": (8, "444", "v_only",
        "3d464fdbc956b6eced2576177fe7af54ce4ee4901518077c16156a36812b1607"),
    "filmgrain_400_10bit_full_64x64": (10, "400", "full",
        "45d960ae8493629422d0a7a8c5cdf709c48ff484a9d7df27d44258e3275be48f"),
    "filmgrain_400_10bit_overlap_restricted_64x64": (10, "400", "overlap_restricted",
        "5468dcc1e952ae751c5278771a192bb2cee6117d02f447454bd82494e75abb18"),
    "filmgrain_420_10bit_full_64x64": (10, "420", "full",
        "3ebcbaff41e824cd59ea7010e7d47023f8037121e764ddb0a0c952bd6d5a4496"),
    "filmgrain_420_10bit_overlap_restricted_64x64": (10, "420", "overlap_restricted",
        "13c80cb8b7d4f03eb4423a30e931b2bfa4059225c6b4ff5c3c9da51e545987fc"),
    "filmgrain_422_10bit_full_64x64": (10, "422", "full",
        "4b8847a2e5b1498d572b8c4db1db81d4b8bad61f7b4bdf40c854464e7e1860d0"),
    "filmgrain_422_10bit_overlap_restricted_64x64": (10, "422", "overlap_restricted",
        "bd23cec41876f15f7f4b0341a5fc642b916e5ade7d739ce25fb988d01b81de3b"),
    "filmgrain_422_10bit_u_only_64x64": (10, "422", "u_only",
        "dd5460e0024323621013ec7579dee6f21df61c17ec5963e87ee58201b7f1f3c2"),
    "filmgrain_422_10bit_v_only_64x64": (10, "422", "v_only",
        "736115cce682ca9509265bf377b008e099b549892e00d642b64273315e366061"),
    "filmgrain_444_10bit_full_64x64": (10, "444", "full",
        "c8eaf11a69582084692eef0a3ec234d8389327b6fd3c835bc802624bb205332b"),
    "filmgrain_444_10bit_overlap_restricted_64x64": (10, "444", "overlap_restricted",
        "344762940d6d5dd9921d12e38ae3043eb8b59eb26466911065b3b22d0ac147c9"),
    "filmgrain_444_10bit_u_only_64x64": (10, "444", "u_only",
        "c1f8b92b485da422ba1a71508d8b6678c3cc267a67fc7aae42db2993aaf14790"),
    "filmgrain_444_10bit_v_only_64x64": (10, "444", "v_only",
        "fda20975eb5c5b90f7183a85c36c5c1cbee9a429db5b74f3453808064a4bfe29"),
    "filmgrain_400_12bit_full_64x64": (12, "400", "full",
        "e7ea84e4d50d152fc1367c9ac0e2848e3de5afbae74c271206dfbb30cd1f5cbf"),
    "filmgrain_400_12bit_overlap_restricted_64x64": (12, "400", "overlap_restricted",
        "0dfca2800ba09729d15223760ef294a2e7f71fb5927c1d76ae8c958fde9c7200"),
    "filmgrain_420_12bit_full_64x64": (12, "420", "full",
        "1947b4b898d9cfed530efb421f31194a53f43fa460482e4f96c7bc2df68e2aa8"),
    "filmgrain_420_12bit_overlap_restricted_64x64": (12, "420", "overlap_restricted",
        "04c761a52dafb7952157d90949f84c8d04dec45156595e0b7941eee00ff54a25"),
    "filmgrain_422_12bit_full_64x64": (12, "422", "full",
        "8a634a92f0bcf87c8fb97c7dc753543dfc25506a3ee9ba01cd2e221aefd61cd1"),
    "filmgrain_422_12bit_overlap_restricted_64x64": (12, "422", "overlap_restricted",
        "f33c3a4988e746455b9b1a4c300beda142a7e190bfb5c4ad92a38bfde59c30bb"),
    "filmgrain_422_12bit_u_only_64x64": (12, "422", "u_only",
        "62247bacff9f9edcb6ffd3f8adc4bc66e553a13b18858f69a6e8fdbeb738fd29"),
    "filmgrain_422_12bit_v_only_64x64": (12, "422", "v_only",
        "894bd114243996ad1227792ea9cef6fe640ef38f14d7cc75fbff5d62b1fa3f7e"),
    "filmgrain_444_12bit_full_64x64": (12, "444", "full",
        "3e09fdae90949fa442e2f579fc4a5ac9d23f7638ec9f86f6eca6a8f95ea151d3"),
    "filmgrain_444_12bit_overlap_restricted_64x64": (12, "444", "overlap_restricted",
        "3ed33254e91772ff4b11ff102cdccef86889892fcdbd725c46fc8775d134da9d"),
    "filmgrain_444_12bit_u_only_64x64": (12, "444", "u_only",
        "39c9e0f6101295590a6f7987117978319e7b3458cb4780c28ae7619aae08f9de"),
    "filmgrain_444_12bit_v_only_64x64": (12, "444", "v_only",
        "fe6bffd3ff34da375e9667bea361f160737eb3aff719c30217998bc504ed47d3"),
    "filmgrain_400_8bit_no_active_points_64x64": (8, "400", "no_active_points",
        "dc41776f17254db98d78fa6beb32f2c4b64564f288f0267e335c27d7c488cdff"),
    "filmgrain_444_8bit_no_active_points_64x64": (8, "444", "no_active_points",
        "cc3d568ec6b1045ea52e42cb9357e015325ad704cd3e2dab13ecc7eb4406193f"),
    "filmgrain_400_10bit_no_active_points_64x64": (10, "400", "no_active_points",
        "0b7520738a3356e0ee74a77d7e36f89c5d651d8b1cf4f7399188485e710d8e59"),
    "filmgrain_444_10bit_no_active_points_64x64": (10, "444", "no_active_points",
        "8529f48b44881ece714d12056a26d781e53700a932384b028d292f9be81c11c6"),
    "filmgrain_400_12bit_no_active_points_64x64": (12, "400", "no_active_points",
        "91d0429c9ed0643df4d2b9aba095cb09562865f53ad53acb9aa57b6bc008ecbc"),
    "filmgrain_444_12bit_no_active_points_64x64": (12, "444", "no_active_points",
        "11381676448383fae35dd53ad86ebd6eb9eb5c9e905d4c657b39850f564814e5"),
}
OPTIONS = (
    "min-partition-size=64", "max-partition-size=64", "use-intra-dct-only=1",
    "enable-cdef=0", "enable-restoration=0", "aq-mode=0", "deltaq-mode=0",
    "film-grain-test=1",
)


def source_y4m(depth: int, layout: str) -> bytes:
    """Include saturated endpoints and fractional scaling indices in each plane."""
    maximum = (1 << depth) - 1
    planes = []
    for plane in range(1 if layout == "400" else 3):
        width = 32 if plane and layout in ("420", "422") else 64
        height = 32 if plane and layout == "420" else 64
        state = 0x12345678 + plane
        samples = []
        for y in range(height):
            for x in range(width):
                state ^= (state << 13) & 0xffffffff
                state ^= state >> 17
                state ^= (state << 5) & 0xffffffff
                value = state & maximum
                if x < 16:
                    value = (0, 1, maximum - 1, maximum)[(x // 4 + y // 8 + plane) % 4]
                samples.append(value)
        planes.append(bytes(samples) if depth == 8 else struct.pack("<" + "H" * len(samples), *samples))
    if layout == "400":
        tag = "mono" if depth == 8 else f"mono{depth}"
    else:
        tag = layout if depth == 8 else f"{layout}p{depth}"
    header = f"YUV4MPEG2 W64 H64 F1:1 Ip A1:1 C{tag} XCOLORRANGE=FULL\nFRAME\n"
    return header.encode() + b"".join(planes)


def grain_variant(data: bytes, layout: str, variant: str) -> bytes:
    """Change only declared display-grain syntax and retain the complete sample."""
    if variant == "full":
        return data
    if variant == "overlap_restricted":
        return enable_restricted_range_clipping(enable_grain_overlap(data))
    sample, obu = frame_location(data)
    grain = deepcopy(obu["frame_header"]["film_grain"])
    grain["y_points"] = []
    grain["ar_coefficients_y"] = []
    for plane in range(2):
        active = variant == ("u_only" if plane == 0 else "v_only")
        grain["uv_points"][plane] = (
            ([[0, 255], [255, 0]] if plane == 0 else [[0, 0], [255, 255]]) if active else []
        )
        grain["ar_coefficients_uv"][plane] = grain["ar_coefficients_uv"][plane][:-1] if active else []
        if not active:
            for field in ("uv_multiplier", "uv_luma_multiplier", "uv_offset"):
                grain[field][plane] = 0
    if layout != "400":
        bits = encode_update_grain(grain)
    else:
        if variant != "no_active_points":
            raise ValueError("monochrome grain cannot carry chroma-only scaling")
        bits = []
        for value, width in (
            (1, 1), (grain["seed"], 16), (0, 4), (grain["scaling_shift"] - 8, 2),
            (grain["ar_coefficient_lag"], 2), (grain["ar_coefficient_shift"] - 6, 2),
            (grain["grain_scale_shift"], 2), (int(grain["overlap"]), 1),
            (int(grain["clip_to_restricted_range"]), 1),
        ):
            append_bits(bits, value, width)
    changed = replace_grain_sentence(data, sample, obu, bits)
    _, updated = frame_location(changed)
    if updated["frame_header"]["film_grain"] != grain:
        raise RuntimeError("rewritten grain sentence differs from the declared input")
    return changed


def validate(data: bytes, depth: int, layout: str, variant: str, digest: str) -> None:
    """Check the complete input identity, AV1 grammar and live Pillow display."""
    if hashlib.sha256(data).hexdigest() != digest:
        raise RuntimeError("film-grain fixture input hash changed")
    with tempfile.NamedTemporaryFile(suffix=".avif", dir=ROOT / "target") as temporary:
        temporary.write(data)
        temporary.flush()
        report = inspect(Path(temporary.name))
    obus = [obu for sample in report["samples"] if sample["role"] == "item_color" for obu in sample["obus"]]
    sequences = [obu["sequence_header"] for obu in obus if "sequence_header" in obu]
    frames = [obu["frame_header"] for obu in obus if obu.get("frame_header")]
    if len(sequences) != 1 or len(frames) != 1:
        raise RuntimeError("expected a single AV1 sequence and frame")
    sequence, frame = sequences[0], frames[0]
    sampling = (0, 0) if layout == "444" else (1, 0) if layout == "422" else (1, 1)
    if (sequence["bit_depth"] != depth or sequence["monochrome"] != (layout == "400")
        or not sequence["film_grain_present"] or not frame["all_lossless"]
        or (frame["frame_width"], frame["frame_height"]) != (64, 64)
        or (layout != "400" and (sequence["subsampling_x"], sequence["subsampling_y"]) != sampling)):
        raise RuntimeError("film-grain source depth, sampling or geometry changed")
    grain = frame["film_grain"]
    if variant == "no_active_points":
        valid = not grain["y_points"] and not any(grain["uv_points"])
    elif variant in ("u_only", "v_only"):
        active = 0 if variant == "u_only" else 1
        valid = not grain["y_points"] and bool(grain["uv_points"][active]) and not grain["uv_points"][1-active]
    else:
        enabled = variant == "overlap_restricted"
        valid = bool(grain["y_points"]) and grain["overlap"] == enabled and grain["clip_to_restricted_range"] == enabled
    if not valid:
        raise RuntimeError("film-grain scaling or clipping controls changed")
    with Image.open(BytesIO(data)) as image:
        image.load()
        if image.mode != "RGB" or image.size != (64, 64) or image.n_frames != 1:
            raise RuntimeError("Pillow display metadata changed")


def generate(encoder: Path | None, output: Path, check: bool) -> None:
    require_pinned_oracle()
    if check:
        for name, (depth, layout, variant, digest) in FIXTURES.items():
            validate((output / (name + ".avif")).read_bytes(), depth, layout, variant, digest)
        print(f"Checked {len(FIXTURES)} complete film-grain inputs")
        return
    if encoder is None or not encoder.is_file():
        raise ValueError("--encoder must name the pinned avifenc executable")
    version = subprocess.check_output([str(encoder), "--version"], text=True)
    if "Version: 1.4.1 (aom [enc]:3.13.2)" not in version:
        raise RuntimeError("libavif 1.4.1 / libaom 3.13.2 encoder required")
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="filmgrain-depth-", dir=ROOT / "target") as temporary:
        directory = Path(temporary)
        sources = {}
        for name, (depth, layout, variant, digest) in FIXTURES.items():
            key = (depth, layout)
            if key not in sources:
                source = directory / f"source-{depth}-{layout}.y4m"
                source.write_bytes(source_y4m(depth, layout))
                runs = []
                for repeat in range(2):
                    dest = directory / f"encoded-{depth}-{layout}-{repeat}.avif"
                    command = [str(encoder), "-q", "100", "--cicp", "1/13/6", "--speed", "0",
                               "--jobs", "1", "--tilerowslog2", "0", "--tilecolslog2", "0"]
                    for option in OPTIONS:
                        command.extend(["-a", option])
                    subprocess.run([*command, str(source), str(dest)], check=True, capture_output=True)
                    runs.append(dest.read_bytes())
                if runs[0] != runs[1]:
                    raise RuntimeError("repeated AV1 source encodes differ")
                sources[key] = runs[0]
            data = grain_variant(sources[key], layout, variant)
            validate(data, depth, layout, variant, digest)
            destination = output / (name + ".avif")
            if destination.exists() and destination.read_bytes() != data:
                raise RuntimeError(f"refusing to replace different fixture: {destination}")
            destination.write_bytes(data)
    print(f"Generated {len(FIXTURES)} complete film-grain inputs")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    generate(args.encoder.resolve() if args.encoder else None, args.output_dir, args.check)


if __name__ == "__main__":
    main()
