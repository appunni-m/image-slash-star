#!/usr/bin/env python3
"""Generate complete wide-block AVIF transform-selection parity inputs.

Repeated pinned Pillow encodes supply 8-bit inputs; pinned libavif/libaom
encodes supply 10/12-bit planar inputs with nonzero fractional sample bits.
Input construction never reads expected outputs or executes Rust. The normal
reference workflow independently records live Pillow pixels and native loops.
"""
from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path
import struct
import subprocess
import tempfile

from PIL import Image

from avif_bmff_boxes import normalize_creation_timestamps
from generate_avif_filmgrain_edge_fixtures import require_pinned_oracle
from generate_avif_mode2_deep16_fixture import make_frames as deep_frames
from generate_avif_mode2_mixed_fixture import make_frames as mixed_frames
from generate_avif_mode2_split32_fixture import make_frames as split_frames
from inspect_av1_obus import inspect

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/fixtures/input/images/avif"
VARIANTS = {"split32": (split_frames, 80), "deep16": (deep_frames, 95), "mixed": (mixed_frames, 80)}
ADVANCED = {
    "sb-size": "128", "min-partition-size": "128", "max-partition-size": "128",
    "aq-mode": "0", "deltaq-mode": "0", "enable-cdef": "0", "enable-restoration": "0",
    "loopfilter-control": "0", "enable-warped-motion": "0",
}
# Complete-input and planar-source digests only; expected pixels are independent.
FIXTURE_SHA256 = {'animated_lossy_wide_i420_10bit_mode2_deep16_b128x128': 'ce093eb35970f1070ce151241c49a302d1143e6d6db75af1e4cd93745ab6800a',
 'animated_lossy_wide_i420_10bit_mode2_mixed_b128x128': '61b9891a58b657ba3090c7a2a2f516a0a16e7eb0c9277163f59eaa54bcae6b67',
 'animated_lossy_wide_i420_10bit_mode2_split32_b128x128': '089c1bfc9d00dff16f6d2b9f5355680f74d5b97c04f71e563db2c9f41ef57c81',
 'animated_lossy_wide_i420_12bit_mode2_deep16_b128x128': '282b01da5f5d88b613d9887a21884562024940f5ea63d1c7badec6266d1270f3',
 'animated_lossy_wide_i420_12bit_mode2_mixed_b128x128': '4095136d5ed94192930a33ce81c4e2cab40c8cf8386aa183e859666338683488',
 'animated_lossy_wide_i420_12bit_mode2_split32_b128x128': '7ad161fe3ad29199f2d910959ac58f03f0a052980fbcdfcdbd1860f9c6e629f0',
 'animated_lossy_wide_i420_mode2_deep16_b128x128': '6dc4388ea8eeaa7b0dbbed2ad2eff580d916f891f8cb4e3b6c444b4bd10ec9e4',
 'animated_lossy_wide_i420_mode2_deep16_color_b128x128': '09ecf856d078b089112b3ed1e28e837403b4dc7365124a7a2a7913fc68ba82cc',
 'animated_lossy_wide_i420_mode2_mixed_b128x128': '7a7b335faee120f33536f653f31992a604056f35f2d90c14fc587f63ae0ab488',
 'animated_lossy_wide_i420_mode2_mixed_color_b128x128': '81b6f280034503dde23a4f9a113ffedb3a8ac9fb07f5825ffe9ec58556a43761',
 'animated_lossy_wide_i420_mode2_split32_b128x128': 'c207f562d7652dc445e86e53bdf8ee4507d122d78edf04fbaa555d07606f47ed',
 'animated_lossy_wide_i420_mode2_split32_color_b128x128': 'cf6d3c6efc2a844e68f58dcef57e22e1215c340dc8f82b35cbcee056cbb53e72',
 'animated_lossy_wide_i422_10bit_mode2_deep16_b128x128': '240c9f9f7e053ff2d45ba2829547502167230f2ecd31739b37c3b62fb74d07ff',
 'animated_lossy_wide_i422_10bit_mode2_mixed_b128x128': '1de536b8a610005830a9f036cd3ee35529ba1ecb1224953f4417c1ba8ee3fe95',
 'animated_lossy_wide_i422_10bit_mode2_split32_b128x128': 'ea85fb65343171ab15c81a7bf8a98852679c1097d7f81d0f85fd947720de88a6',
 'animated_lossy_wide_i422_12bit_mode2_deep16_b128x128': 'fcb9a3fe363293799510fb34c9bd4999d8cc2ff03dcbeb179796b04cf39e6200',
 'animated_lossy_wide_i422_12bit_mode2_mixed_b128x128': '02b6878702cd871581b6ddc28f9b3cb960bf0c9671daf79e20b208df2bde044a',
 'animated_lossy_wide_i422_12bit_mode2_split32_b128x128': '2fa5567f2fdd9f467ebabba6d9b37e373da54ba67d082d0b7afa8ce2e7e37223',
 'animated_lossy_wide_i422_mode2_deep16_b128x128': 'a251e0e94f1005c3d566a7dcb89a8b5223d86d54ed035d17fd372d48e981fd33',
 'animated_lossy_wide_i422_mode2_deep16_color_b128x128': '65769fa6ce4920d390e415e3ae40aba7cbe24ec5299ade14517f6662ad7f3b2e',
 'animated_lossy_wide_i422_mode2_mixed_b128x128': '3384cb410730928ddc57d09f89728e93f901feee013ad17c8f6c8729542cc86b',
 'animated_lossy_wide_i422_mode2_mixed_color_b128x128': '8cedea55c079f444f48876de9d23b7918c59a262cc64d2d41aaa2a073d65c168',
 'animated_lossy_wide_i422_mode2_split32_b128x128': '66aaf55006ce3c37b92329ec1b3290d2dc676b6d3c75f1a8cf1879bbe7a7fe82',
 'animated_lossy_wide_i422_mode2_split32_color_b128x128': '3955d8b4618ce2de6993e7a1fe4dd4cd45a24c8adbbb75f8512f94c84b6bc9bc',
 'animated_lossy_wide_i444_10bit_mode2_deep16_b128x128': 'dfa469bff5617b7cc27384c281e5a78082979e40a2208e80ca90f43fe96ef789',
 'animated_lossy_wide_i444_10bit_mode2_mixed_b128x128': '143452baa7be593e89e7f31b6266ddfb54166bee4762be2b1a7100bd235448f2',
 'animated_lossy_wide_i444_10bit_mode2_split32_b128x128': 'ac36ee94eb7cbe79e8677791fef5b12e3656232c3cf9e3fa4eda23494c30dde5',
 'animated_lossy_wide_i444_12bit_mode2_deep16_b128x128': '5d0d6645530501e99fd57398de0c51fbc1d8e226fb660b153302e53cc481d4ac',
 'animated_lossy_wide_i444_12bit_mode2_mixed_b128x128': '11407ec5bb0765818b543bc70f69f04b8b802ad7279a5b74541aa732304b9717',
 'animated_lossy_wide_i444_12bit_mode2_split32_b128x128': '38b9aaa05374897ad076d38b53fd9765437a52058365d4571703836e128591c6'}
SOURCE_SHA256 = {'animated_lossy_wide_i420_10bit_mode2_deep16_b128x128': 'e4a92962393d90787921a33c41eb1803c9df98ffec28fc18702f09ff9ff55bd5',
 'animated_lossy_wide_i420_10bit_mode2_mixed_b128x128': '00d5140fc479dd486bd21736f9d7a62fac3149ae918873ac254d2f69238047ea',
 'animated_lossy_wide_i420_10bit_mode2_split32_b128x128': '7753d710e3d8830079560663ac7b9795a6bb3ed1e6486ab998f5241259f72bf8',
 'animated_lossy_wide_i420_12bit_mode2_deep16_b128x128': 'd5b510b183e987ed4e31ba086eed94106975632f5559e0f22de620879fb575d6',
 'animated_lossy_wide_i420_12bit_mode2_mixed_b128x128': '0aac5d88cc80891c7822801b0c7add4445bf76a236d93d5765f2513ff442901a',
 'animated_lossy_wide_i420_12bit_mode2_split32_b128x128': '78af340d2b1fdc19be313eb2caac9095f42748bd08c2f6a0c4a19ee8be3f1808',
 'animated_lossy_wide_i422_10bit_mode2_deep16_b128x128': '83aaf625ad66734da907120375f0453c61f55eecae1a73863f0484dd2f1e316d',
 'animated_lossy_wide_i422_10bit_mode2_mixed_b128x128': 'ba8da0db93a1573accb466960cd400bf3a26586e9a9d328ad636adcebeec6de3',
 'animated_lossy_wide_i422_10bit_mode2_split32_b128x128': '30691b35cbdfc35b607ab140baa703238be0b04def8bc3509b6a76d2c9acb7c2',
 'animated_lossy_wide_i422_12bit_mode2_deep16_b128x128': 'e83be49f3abbd220de8be7f1e2469707aa1ed452b637808faf475bf20e7efebe',
 'animated_lossy_wide_i422_12bit_mode2_mixed_b128x128': 'bb0fd9c38ce3f8f2f64c5149a32f0f0e32cdff01742ae402ef60116806e1ef82',
 'animated_lossy_wide_i422_12bit_mode2_split32_b128x128': 'c514f3323dc268ccb6f3d3dc2e3a8244e5deb24676ba425b5ba2fa195462fc57',
 'animated_lossy_wide_i444_10bit_mode2_deep16_b128x128': '71f2b737213d3301b7bad527183391ab3bd4e3b5ea2ad130a9b62a107873007a',
 'animated_lossy_wide_i444_10bit_mode2_mixed_b128x128': 'd1779faae4c25d2df7294812ba33b20e0db3724b334754f07deaf4ea22ef1722',
 'animated_lossy_wide_i444_10bit_mode2_split32_b128x128': 'c5b1fed8d4eebdb9606ebd00bd9adabe1791772ecdda90d47cdab93eaaba533d',
 'animated_lossy_wide_i444_12bit_mode2_deep16_b128x128': 'b874034b61785f08d0f78c38782a051c21ac70fce8203815f3d0fb639010c919',
 'animated_lossy_wide_i444_12bit_mode2_mixed_b128x128': 'f82591f9941cae3affc997fa8af1b682a36e92308552ffe9795b0fe989dd43c2',
 'animated_lossy_wide_i444_12bit_mode2_split32_b128x128': 'b9986d4b9695336e8a70adb53837fbefdb8a4768bc9924b96d648e252205c9db'}


def recipes():
    """Yield each admitted layout/depth and its deterministic source recipe."""
    for depth in (8, 10, 12):
        for layout in (("420", "422") if depth == 8 else ("420", "422", "444")):
            for variant in VARIANTS:
                for colored in ((False, True) if depth == 8 else (True,)):
                    precision = "" if depth == 8 else f"_{depth}bit"
                    color = "_color" if depth == 8 and colored else ""
                    name = f"animated_lossy_wide_i{layout}{precision}_mode2_{variant}{color}_b128x128"
                    yield name, layout, depth, variant, colored


def source_y4m(layout: str, depth: int, variant: str) -> bytes:
    """Add low sample bits to independent luma and chroma checker/sinusoid planes."""
    data = bytearray(f"YUV4MPEG2 W256 H256 F10:1 Ip A1:1 C{layout}p{depth} XCOLORRANGE=FULL\n".encode())
    scale = 1 << (depth - 8)
    for frame in VARIANTS[variant][0]():
        data.extend(b"FRAME\n")
        gray = frame.getchannel("R").tobytes()
        for plane in range(3):
            sx = 2 if plane and layout in ("420", "422") else 1
            sy = 2 if plane and layout == "420" else 1
            values = []
            for y in range(0, 256, sy):
                for x in range(0, 256, sx):
                    value = gray[y * 256 + x]
                    if plane:
                        value = 128 + (value - 128) // (2 if plane == 1 else -3)
                    sample = value * scale + (x + 3 * y + plane) % scale
                    values.append(min((1 << depth) - 1, sample))
            data.extend(struct.pack("<" + "H" * len(values), *values))
    return bytes(data)


def encode_pillow(layout: str, variant: str, colored: bool) -> bytes:
    """Encode full RGB frames with a fixed single-thread, single-tile preset."""
    frames = list(VARIANTS[variant][0]())
    if colored:
        converted = []
        for frame in frames:
            pixels = bytearray()
            for value in frame.getchannel("R").tobytes():
                pixels.extend((value, 128 + (value - 128) // 2, 128 - (value - 128) // 3))
            converted.append(Image.frombytes("RGB", frame.size, bytes(pixels)))
        frames = converted
    output = BytesIO()
    frames[0].save(output, format="AVIF", save_all=True, append_images=frames[1:],
                   duration=[100] * len(frames), loop=0, quality=VARIANTS[variant][1],
                   speed=0, max_threads=1, subsampling={"420": "4:2:0", "422": "4:2:2"}[layout],
                   autotiling=False, codec="aom", advanced=ADVANCED)
    return normalize_creation_timestamps(output.getvalue())


def encode_native(encoder: Path, source: Path, destination: Path, variant: str) -> bytes:
    """Encode an input-only planar source using the pinned native encoder."""
    command = [str(encoder), "-q", str(VARIANTS[variant][1]), "--cicp", "1/13/6",
               "--speed", "0", "--jobs", "1", "--tilerowslog2", "0", "--tilecolslog2", "0",
               "--creation-time", "1", "--modification-time", "1"]
    for option, value in ADVANCED.items():
        command.extend(["-a", f"{option}={value}"])
    subprocess.run([*command, str(source), str(destination)], check=True, capture_output=True)
    return normalize_creation_timestamps(destination.read_bytes())


def validate(path: Path, name: str, layout: str, depth: int, variant: str) -> None:
    """Verify retained input identity, AV1 syntax and live Pillow metadata."""
    if hashlib.sha256(path.read_bytes()).hexdigest() != FIXTURE_SHA256[name]:
        raise ValueError(f"complete-input digest differs: {path}")
    report = inspect(path)
    obus = [obu for sample in report["samples"] if sample["role"] == "track_pict" for obu in sample["obus"]]
    sequence = next(obu["sequence_header"] for obu in obus if "sequence_header" in obu)
    frames = [obu["frame_header"] for obu in obus if "frame_header" in obu]
    coded = [frame for frame in frames if not frame["show_existing_frame"]]
    expected_frames = 5 if variant == "deep16" else 2
    if (sequence["bit_depth"] != depth or sequence["monochrome"]
            or sequence["subsampling_x"] != (layout != "444")
            or sequence["subsampling_y"] != (layout == "420")
            or not sequence["use_128x128_superblock"]
            or sum(frame["show_frame"] for frame in frames) != expected_frames
            or not any(frame["transform_mode"] == "select" for frame in coded)
            or any((frame["frame_width"], frame["frame_height"]) != (256, 256)
                   or frame["transform_mode"] not in ("select", "largest") for frame in coded)):
        raise ValueError(f"complete-input syntax differs: {name}")
    with Image.open(path) as image:
        if image.format != "AVIF" or image.size != (256, 256) or image.mode != "RGB" or image.n_frames != expected_frames:
            raise ValueError(f"live Pillow metadata differs: {name}")
        for frame in range(expected_frames):
            image.seek(frame)
            image.load()
            if image.info.get("duration") != 100:
                raise ValueError(f"live Pillow frame duration differs: {name}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, help="libavif 1.4.1 / libaom 3.13.2 avifenc for regeneration")
    parser.add_argument("--output-dir", type=Path, default=OUTPUT)
    parser.add_argument("--check", action="store_true", help="read retained inputs without encoding or rewriting")
    args = parser.parse_args()
    require_pinned_oracle()
    if set(FIXTURE_SHA256) != {recipe[0] for recipe in recipes()}:
        raise ValueError("input digest pins and source recipes differ")
    encoder = args.encoder.resolve() if args.encoder else None
    if not args.check:
        if encoder is None or "Version: 1.4.1 (aom [enc]:3.13.2)" not in subprocess.check_output([str(encoder), "--version"], text=True):
            raise ValueError("regeneration requires the pinned libavif/libaom encoder")
        args.output_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="avif-wide-mode2-") as directory:
        temporary = Path(directory)
        for name, layout, depth, variant, colored in recipes():
            destination = args.output_dir / f"{name}.avif"
            if not args.check:
                if depth == 8:
                    data, repeated = encode_pillow(layout, variant, colored), encode_pillow(layout, variant, colored)
                else:
                    source = temporary / "source.y4m"
                    source.write_bytes(source_y4m(layout, depth, variant))
                    if hashlib.sha256(source.read_bytes()).hexdigest() != SOURCE_SHA256[name]:
                        raise ValueError(f"planar source digest differs: {name}")
                    data = encode_native(encoder, source, temporary / "run0.avif", variant)
                    repeated = encode_native(encoder, source, temporary / "run1.avif", variant)
                if data != repeated or hashlib.sha256(data).hexdigest() != FIXTURE_SHA256[name]:
                    raise ValueError(f"repeated pinned encode differs: {name}")
                if destination.exists() and destination.read_bytes() != data:
                    raise ValueError(f"refusing to replace different fixture: {destination}")
                destination.write_bytes(data)
            validate(destination, name, layout, depth, variant)
    print(f"{'Checked' if args.check else 'Generated'} {len(FIXTURE_SHA256)} complete wide-block AVIF inputs")


if __name__ == "__main__":
    main()
