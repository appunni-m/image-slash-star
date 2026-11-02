#!/usr/bin/env python3
"""Generate input-only AVIF sequences with adjacent 64x64 inter blocks.

Pinned Pillow produces 8-bit inputs; pinned libavif/libaom consumes independent
10/12-bit planar sources. Expected frames and native loop observations belong
to the ordinary reference generators, never this input producer.
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
from generate_avif_mode2_deep16_fixture import make_frames as checker_frames
from inspect_av1_obus import inspect

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/fixtures/input/images/avif"
ADVANCED = {
    "sb-size": "64", "min-partition-size": "64", "max-partition-size": "64",
    "aq-mode": "0", "deltaq-mode": "0", "enable-cdef": "0",
    "enable-restoration": "0", "loopfilter-control": "0", "enable-warped-motion": "0",
}
FIXTURE_SHA256 = {
    "animated_lossy_inter_422_8bit_b64x64_chroma_neighbors_q70_128x128": "4ebbf5720ed2c7f37624c276cef435a298b07676af339fc6e18413c82aadf08e",
    "animated_lossy_inter_422_8bit_b64x64_chroma_neighbors_q95_128x128": "9d8d7c7b7d54558a0f13d5796d085f6ffbd5a66a2f4854c8614378e15a6e9d6c",
    "animated_lossy_inter_422_8bit_b64x64_chroma_neighbors_q99_128x128": "fa9eccb266209951f7029d31a9411cc8bd7a64f3b5120b394433a28aba639d0e",
    "animated_lossy_inter_444_8bit_b64x64_chroma_neighbors_q70_128x128": "245d91f8a17fac4aee30b0d0317e9fb33931e6747fe481383b799e75d69f2046",
    "animated_lossy_inter_444_8bit_b64x64_chroma_neighbors_q95_128x128": "1d72a389510eb9de4107b14242405177a1ece6a8f7cf95336a56aeb1e8c46c6d",
    "animated_lossy_inter_444_8bit_b64x64_chroma_neighbors_q99_128x128": "d152f921af6d2a2a6c48dc5e851646d400dc8994b2acb57797f41ba2a0133d55",
    "animated_lossy_inter_422_10bit_b64x64_chroma_neighbors_q70_128x128": "edf322e0eadf3410d5235bb485ae2c527d50a7b48ee52f3905f7474208acf9c1",
    "animated_lossy_inter_422_10bit_b64x64_chroma_neighbors_q95_128x128": "76cc24ef48bd0f97153a515e3fcc2305273d3703826bf9283abe06fe2d03a481",
    "animated_lossy_inter_422_10bit_b64x64_chroma_neighbors_q99_128x128": "6b9ecccad6f3c17011a770383d67174ae377f2d2e5b04cafb49abba059e83a2b",
    "animated_lossy_inter_444_10bit_b64x64_chroma_neighbors_q70_128x128": "6702f30cc913b6593855f9794df3a8b7ba7d5f0b2a5250c2cccd7d54c86867f7",
    "animated_lossy_inter_444_10bit_b64x64_chroma_neighbors_q95_128x128": "0dcaa8301a001f1e12dcbb320643f790bac0cee8497b22597d52fa0164645baa",
    "animated_lossy_inter_444_10bit_b64x64_chroma_neighbors_q99_128x128": "b66ae69d670292ac9037c284223771a8ea67619b339a2732e677e317e72970d6",
    "animated_lossy_inter_422_12bit_b64x64_chroma_neighbors_q70_128x128": "8eb986ee0975669302bce752153dbb74110f2fc6658ad6041da9e94e43ea88b2",
    "animated_lossy_inter_422_12bit_b64x64_chroma_neighbors_q95_128x128": "6e5ac1c5a5e90c5a7a1dc0c029a535bcadd74bc2ce5e53176c9ff60e765c0f54",
    "animated_lossy_inter_422_12bit_b64x64_chroma_neighbors_q99_128x128": "5e20c66fdfcf724dd912fa42bc1bd01184c6ffebd9ced56579e3ab203cc97b0e",
    "animated_lossy_inter_444_12bit_b64x64_chroma_neighbors_q70_128x128": "9783e41462f61f123fface7381182f3806c8b0ac01e00c326c5fd270b488752c",
    "animated_lossy_inter_444_12bit_b64x64_chroma_neighbors_q95_128x128": "051fe0551d5131335d62ebf088c9a05be5d33315734baf2b3a4c31d222f59fe5",
    "animated_lossy_inter_444_12bit_b64x64_chroma_neighbors_q99_128x128": "000526c180b9b11154623bd297ee4550b583694cc2636715b2e7cc8b69bfc0f5"
}


def recipes():
    """Yield layout, sample precision and quality for each complete source."""
    for depth in (8, 10, 12):
        for layout in ("422", "444"):
            for quality in (70, 95, 99):
                name = f"animated_lossy_inter_{layout}_{depth}bit_b64x64_chroma_neighbors_q{quality}_128x128"
                yield name, layout, depth, quality


def source_frames() -> tuple[Image.Image, ...]:
    """Build a gray key frame followed by four colored checker periods."""
    frames = []
    for frame in checker_frames():
        gray = frame.crop((0, 0, 128, 128)).getchannel("R").tobytes()
        pixels = bytes(channel for value in gray for channel in
                       (value, 128 + (value - 128) // 2, 128 - (value - 128) // 3))
        frames.append(Image.frombytes("RGB", (128, 128), pixels))
    return tuple(frames)


def source_y4m(layout: str, depth: int) -> bytes:
    """Build planar checker samples with nonzero low bits at 10/12 bits."""
    data = bytearray(f"YUV4MPEG2 W128 H128 F10:1 Ip A1:1 C{layout}p{depth} XCOLORRANGE=FULL\n".encode())
    scale = 1 << (depth - 8)
    for frame in source_frames():
        data.extend(b"FRAME\n")
        for plane in range(3):
            samples = frame.getchannel("RGB"[plane]).tobytes()
            step_x = 2 if plane and layout == "422" else 1
            values = [samples[y * 128 + x] * scale + (x + 3 * y + plane) % scale
                      for y in range(128) for x in range(0, 128, step_x)]
            data.extend(struct.pack("<" + "H" * len(values), *values))
    return bytes(data)


def encode_pillow(layout: str, quality: int) -> bytes:
    """Encode the complete five-frame 8-bit source with ordinary AV1 controls."""
    frames = source_frames()
    output = BytesIO()
    frames[0].save(output, format="AVIF", save_all=True, append_images=frames[1:],
                   duration=[100] * len(frames), loop=0, quality=quality, speed=0,
                   max_threads=1, subsampling={"422": "4:2:2", "444": "4:4:4"}[layout],
                   autotiling=False, codec="aom", advanced=ADVANCED)
    return normalize_creation_timestamps(output.getvalue())


def encode_native(encoder: Path, source: Path, output: Path, quality: int) -> bytes:
    """Encode a complete independent planar source using the pinned CLI."""
    command = [str(encoder), "-q", str(quality), "--cicp", "1/13/6", "--speed", "0",
               "--jobs", "1", "--tilerowslog2", "0", "--tilecolslog2", "0",
               "--creation-time", "1", "--modification-time", "1"]
    for option, value in ADVANCED.items():
        command.extend(["-a", f"{option}={value}"])
    subprocess.run([*command, str(source), str(output)], check=True, capture_output=True)
    return normalize_creation_timestamps(output.read_bytes())


def validate(path: Path, name: str, layout: str, depth: int) -> None:
    """Validate source identity, AV1 headers and live Pillow sequence metadata."""
    if hashlib.sha256(path.read_bytes()).hexdigest() != FIXTURE_SHA256[name]:
        raise ValueError(f"complete-input digest differs: {path}")
    report = inspect(path)
    obus = [obu for sample in report["samples"] if sample["role"] == "track_pict" for obu in sample["obus"]]
    sequence = next(obu["sequence_header"] for obu in obus if "sequence_header" in obu)
    frames = [obu["frame_header"] for obu in obus if "frame_header" in obu]
    coded = [frame for frame in frames if not frame["show_existing_frame"]]
    if (sequence["bit_depth"] != depth or sequence["monochrome"]
            or sequence["subsampling_x"] != (layout == "422") or sequence["subsampling_y"]
            or sequence["use_128x128_superblock"]
            or not any(frame["transform_mode"] == "select" for frame in coded)
            or any((frame["frame_width"], frame["frame_height"]) != (128, 128) for frame in coded)):
        raise ValueError(f"complete-input syntax differs: {name}")
    with Image.open(path) as image:
        if image.format != "AVIF" or image.size != (128, 128) or image.mode != "RGB" or image.n_frames != 5:
            raise ValueError(f"live Pillow metadata differs: {name}")
        for index in range(image.n_frames):
            image.seek(index)
            image.load()
            if image.info.get("duration") != 100:
                raise ValueError(f"live Pillow frame duration differs: {name}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, help="pinned libavif 1.4.1 / libaom 3.13.2 avifenc")
    parser.add_argument("--output-dir", type=Path, default=OUTPUT)
    parser.add_argument("--variant", choices=sorted(FIXTURE_SHA256))
    parser.add_argument("--check", action="store_true", help="validate retained inputs without writing or encoding")
    args = parser.parse_args()
    require_pinned_oracle()
    if set(FIXTURE_SHA256) != {recipe[0] for recipe in recipes()}:
        raise ValueError("source recipes and complete-input digest pins differ")
    encoder = args.encoder.resolve() if args.encoder else None
    selected = [recipe for recipe in recipes() if args.variant is None or recipe[0] == args.variant]
    if not args.check:
        if any(depth > 8 for _, _, depth, _ in selected):
            if encoder is None or "Version: 1.4.1 (aom [enc]:3.13.2)" not in subprocess.check_output([str(encoder), "--version"], text=True):
                raise ValueError("10/12-bit regeneration requires the pinned libavif/libaom encoder")
        args.output_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="avif-square64-neighbors-") as directory:
        temporary = Path(directory)
        for name, layout, depth, quality in selected:
            destination = args.output_dir / (name + ".avif")
            if not args.check:
                if depth == 8:
                    encoded = [encode_pillow(layout, quality) for _ in range(2)]
                else:
                    source = temporary / "source.y4m"
                    source.write_bytes(source_y4m(layout, depth))
                    encoded = [encode_native(encoder, source, temporary / f"encode-{index}.avif", quality)
                               for index in range(2)]
                if encoded[0] != encoded[1]:
                    raise ValueError(f"independent normalized encodes differ: {name}")
                if hashlib.sha256(encoded[0]).hexdigest() != FIXTURE_SHA256[name]:
                    raise ValueError(f"regenerated complete input differs: {name}")
                destination.write_bytes(encoded[0])
            validate(destination, name, layout, depth)
            print(f"Validated {name}")


if __name__ == "__main__":
    main()
