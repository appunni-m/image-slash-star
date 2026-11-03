#!/usr/bin/env python3
"""Generate pinned 10-bit I420/I422 intra-filter parity fixtures."""

from __future__ import annotations

import argparse
import hashlib
import random
import shutil
import struct
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, _avif, features

from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
AVIFENC_VERSION = "Version: 1.4.1 (aom [enc]:v3.13.2)"
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
WIDTH = 16
HEIGHT = 16


@dataclass(frozen=True)
class FixtureVariant:
    """Pinned source, syntax profile, and output identity for one AV1 fixture."""

    fixture_stem: str
    chroma_tag: str
    subsampling_x: int
    subsampling_y: int
    seed: int
    enable_cdef: bool
    enable_restoration: bool
    restoration_types: tuple[int, int, int]
    restoration_unit_size_log2: tuple[int, int]
    cdef_bits: int | None
    cdef_damping: int | None
    cdef_y_strengths: tuple[int, ...] | None
    cdef_uv_strengths: tuple[int, ...] | None
    source_sha256: str
    asset_sha256: str
    rgb_sha256: str
    width: int = WIDTH
    height: int = HEIGHT


VARIANTS = {
    "i420": FixtureVariant(
        fixture_stem="coverage_i420_intra_restoration_10bit_16x16",
        chroma_tag="420p10",
        subsampling_x=1,
        subsampling_y=1,
        seed=0x12A420,
        enable_cdef=False,
        enable_restoration=True,
        restoration_types=(2, 0, 0),
        restoration_unit_size_log2=(8, 8),
        cdef_bits=None,
        cdef_damping=None,
        cdef_y_strengths=None,
        cdef_uv_strengths=None,
        source_sha256="ace43f33a1d3c68bfdc27e34e3c9d32530bde72d9718a5fb761b775db6585d0e",
        asset_sha256="c508ed4754d696d6b3240ca552dd61a1481c062c9537904183c4657b13bc5f7f",
        rgb_sha256="257ef1a97e25cd6c57851f2ac0ffdec2febc773dd82e63104047255bb5903931",
    ),
    "i422": FixtureVariant(
        fixture_stem="coverage_i422_intra_restoration_10bit_16x16",
        chroma_tag="422p10",
        subsampling_x=1,
        subsampling_y=0,
        seed=0x12A422,
        enable_cdef=False,
        enable_restoration=True,
        restoration_types=(2, 0, 0),
        restoration_unit_size_log2=(8, 8),
        cdef_bits=None,
        cdef_damping=None,
        cdef_y_strengths=None,
        cdef_uv_strengths=None,
        source_sha256="631176b53370dac45da056298c0adb92f44239e0b43a1e70adcd5ff07c2d5190",
        asset_sha256="2dfdaa9d17c34e5656e5fa458ef951abe28cec6452d17eef1ca7820022a6c19f",
        rgb_sha256="93f7718993194043da301e4f78d6a36848058c1a18599546db3628ca579519dc",
    ),
    "i420_cdef": FixtureVariant(
        fixture_stem="coverage_i420_intra_cdef_10bit_16x16",
        chroma_tag="420p10",
        subsampling_x=1,
        subsampling_y=1,
        seed=0x12A420,
        enable_cdef=True,
        enable_restoration=False,
        restoration_types=(0, 0, 0),
        restoration_unit_size_log2=(8, 8),
        cdef_bits=0,
        cdef_damping=6,
        cdef_y_strengths=(63,),
        cdef_uv_strengths=(61,),
        source_sha256="ace43f33a1d3c68bfdc27e34e3c9d32530bde72d9718a5fb761b775db6585d0e",
        asset_sha256="3407c458add1348d2e0f0a73f8aaaa862fd435a18eb7c563b8461d89541656d2",
        rgb_sha256="c53e98f018266ced6203f60ddd618a1f321962fd333cafb0acea0fdd6485e987",
    ),
    "i420_cdef_odd_edge": FixtureVariant(
        fixture_stem="coverage_i420_intra_cdef_10bit_9x9",
        chroma_tag="420p10",
        subsampling_x=1,
        subsampling_y=1,
        seed=0x12A429,
        enable_cdef=True,
        enable_restoration=False,
        restoration_types=(0, 0, 0),
        restoration_unit_size_log2=(8, 8),
        cdef_bits=0,
        cdef_damping=5,
        cdef_y_strengths=(51,),
        cdef_uv_strengths=(16,),
        source_sha256="23626ee401fe122ba7701cc803b527d71e8babc39c997b296332d19fe05dbee1",
        asset_sha256="11c149dcf954268012c565a586cb1a64af8c86b6eb5cd1463ec6ff6567ba4917",
        rgb_sha256="cb5a5b6e5760a49068aa7d3863634600c82c5fa6500295082d524633d959efd1",
        width=9,
        height=9,
    ),
    "i420_cdef_odd_width": FixtureVariant(
        fixture_stem="coverage_i420_intra_cdef_10bit_9x16",
        chroma_tag="420p10",
        subsampling_x=1,
        subsampling_y=1,
        seed=0x12A916,
        enable_cdef=True,
        enable_restoration=False,
        restoration_types=(0, 0, 0),
        restoration_unit_size_log2=(8, 8),
        cdef_bits=0,
        cdef_damping=5,
        cdef_y_strengths=(63,),
        cdef_uv_strengths=(60,),
        source_sha256="aefc700594fabb0bf37018393bcd801ad99a433e597e1211326736cd8d12a76e",
        asset_sha256="9349dd957c42e2f39e8863893e1e86eb996bd112611f5543b2c4fa3e28c312fc",
        rgb_sha256="c99bfa3e07e5637aaf1b41db938b495bc8c43067c674be61753957f12d12e119",
        width=9,
        height=16,
    ),
    "i420_cdef_chroma_aligned_right_edge": FixtureVariant(
        fixture_stem="coverage_i420_intra_cdef_10bit_15x16",
        chroma_tag="420p10",
        subsampling_x=1,
        subsampling_y=1,
        seed=0x12A915,
        enable_cdef=True,
        enable_restoration=False,
        restoration_types=(0, 0, 0),
        restoration_unit_size_log2=(8, 8),
        cdef_bits=0,
        cdef_damping=5,
        cdef_y_strengths=(27,),
        cdef_uv_strengths=(57,),
        source_sha256="3be737be15e559208f7d77cf2aee16fc1d81f041c8cb3c2eb5f201c430d6590e",
        asset_sha256="43fef444f1e0144c74f60752ea255ac2d3f36d594c8172dbf09b57a6c5497b96",
        rgb_sha256="53e32b2c9de70cf7f2bf975e136c0e4f9517dbe73b755010de3b839c1ba799b3",
        width=15,
        height=16,
    ),
    "i420_cdef_restoration": FixtureVariant(
        fixture_stem="coverage_i420_intra_cdef_restoration_10bit_16x16",
        chroma_tag="420p10",
        subsampling_x=1,
        subsampling_y=1,
        seed=0x12A420,
        enable_cdef=True,
        enable_restoration=True,
        restoration_types=(3, 0, 0),
        restoration_unit_size_log2=(8, 8),
        cdef_bits=0,
        cdef_damping=5,
        cdef_y_strengths=(63,),
        cdef_uv_strengths=(61,),
        source_sha256="ace43f33a1d3c68bfdc27e34e3c9d32530bde72d9718a5fb761b775db6585d0e",
        asset_sha256="faff193dc1d3c62835490e949ed83314578abea8eb14357dffe361044914bb92",
        rgb_sha256="4d3f43960d12c77079790677f805b6deec2470a7c4deee5b498948d2f434dd30",
    ),
    "i420_cdef_restoration_striped": FixtureVariant(
        fixture_stem="coverage_i420_intra_cdef_restoration_10bit_16x64",
        chroma_tag="420p10",
        subsampling_x=1,
        subsampling_y=1,
        seed=0x12A164,
        enable_cdef=True,
        enable_restoration=True,
        restoration_types=(2, 0, 0),
        restoration_unit_size_log2=(8, 8),
        cdef_bits=0,
        cdef_damping=5,
        cdef_y_strengths=(47,),
        cdef_uv_strengths=(0,),
        source_sha256="7d30b4c3eb9b0157a01e54afa6b011dfb54701ebfd919a5ddc52f047474e3bc6",
        asset_sha256="ccf7cb2dc6c81749955e09853587f7b1047e3c89b17f2832a4196303d8302727",
        rgb_sha256="a6535b59e9da0eb66ee1bf768e080739bc8d88b2afc9554388b7620849900f1e",
        width=16,
        height=64,
    ),
    "i422_cdef": FixtureVariant(
        fixture_stem="coverage_i422_intra_cdef_10bit_16x16",
        chroma_tag="422p10",
        subsampling_x=1,
        subsampling_y=0,
        seed=0x12A422,
        enable_cdef=True,
        enable_restoration=False,
        restoration_types=(0, 0, 0),
        restoration_unit_size_log2=(8, 8),
        cdef_bits=0,
        cdef_damping=6,
        cdef_y_strengths=(51,),
        cdef_uv_strengths=(6,),
        source_sha256="631176b53370dac45da056298c0adb92f44239e0b43a1e70adcd5ff07c2d5190",
        asset_sha256="40e0090f85088bdef2e7cf08910002dd2dc48e95637c976173e71a9326a22352",
        rgb_sha256="3a79f4b4daa12011f7fb0e654df8c0677e17d2b2d417319ae15a6a38de51aee2",
    ),
    "i422_cdef_restoration": FixtureVariant(
        fixture_stem="coverage_i422_intra_cdef_restoration_10bit_16x16",
        chroma_tag="422p10",
        subsampling_x=1,
        subsampling_y=0,
        seed=0x12A422,
        enable_cdef=True,
        enable_restoration=True,
        restoration_types=(2, 0, 0),
        restoration_unit_size_log2=(8, 8),
        cdef_bits=0,
        cdef_damping=5,
        cdef_y_strengths=(51,),
        cdef_uv_strengths=(6,),
        source_sha256="631176b53370dac45da056298c0adb92f44239e0b43a1e70adcd5ff07c2d5190",
        asset_sha256="aa368cf810ef5d48ce300f3582f9574744a1929beeb5525497675176b6b8beac",
        rgb_sha256="afb7a71a8c86bf53df63a3735752e299ced6712e8b718cf4725fb3a2b61eaec6",
    ),
}


def fixture_paths(variant: str) -> tuple[Path, Path]:
    """Return the input AVIF and raw Pillow reference paths for a variant."""

    stem = VARIANTS[variant].fixture_stem
    return (
        ROOT / "tests" / "fixtures" / "input" / "images" / "avif" / f"{stem}.avif",
        ROOT
        / "tests"
        / "fixtures"
        / "outputs"
        / "raws"
        / f"Decode.avif_{stem}_avif.bin",
    )


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest for fixture evidence."""

    return hashlib.sha256(data).hexdigest()


def y4m_source(variant: FixtureVariant) -> bytes:
    """Build deterministic full-range 10-bit planar Y4M input."""

    generator = random.Random(variant.seed)
    width, height = variant.width, variant.height
    for _ in range(width * height):
        generator.randrange(1024)
    luma = [512 + generator.randrange(-160, 161) for _ in range(width * height)]
    chroma_width = (width + (1 << variant.subsampling_x) - 1) // (
        1 << variant.subsampling_x
    )
    chroma_height = (height + (1 << variant.subsampling_y) - 1) // (
        1 << variant.subsampling_y
    )
    chroma_u = [
        256 + ((x * 127 + y * 61) % 512)
        for y in range(chroma_height)
        for x in range(chroma_width)
    ]
    chroma_v = [
        768 - ((x * 71 + y * 89) % 512)
        for y in range(chroma_height)
        for x in range(chroma_width)
    ]
    planes = b"".join(
        struct.pack(f"<{len(samples)}H", *samples)
        for samples in (luma, chroma_u, chroma_v)
    )
    header = (
        f"YUV4MPEG2 W{width} H{height} F30:1 Ip A1:1 "
        f"C{variant.chroma_tag} XCOLORRANGE=FULL\nFRAME\n"
    ).encode("ascii")
    return header + planes


def encode_command(
    avifenc: Path,
    source: Path,
    output: Path,
    variant: FixtureVariant,
) -> list[str]:
    """Return pinned libaom settings for one 10-bit intra filter profile."""

    command = [str(avifenc), "-q", "40", "-s", "0", "-j", "1"]
    advanced_options = (
        ("min-partition-size", "16"),
        ("max-partition-size", "16"),
        ("enable-tx-size-search", "0"),
        ("enable-qm", "0"),
        ("enable-restoration", str(int(variant.enable_restoration))),
        ("enable-cdef", str(int(variant.enable_cdef))),
        ("loopfilter-control", "0"),
        ("aq-mode", "0"),
        ("deltaq-mode", "0"),
    )
    for name, value in advanced_options:
        command.extend(("--advanced", f"{name}={value}"))
    command.extend((str(source), str(output)))
    return command


def run(command: list[str]) -> str:
    """Run a fixture tool and include its diagnostics on failure."""

    result = subprocess.run(
        command,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return result.stdout


def validate_bitstream(path: Path, variant: FixtureVariant) -> dict[str, object]:
    """Require the exact public intra-filter syntax profile."""

    report = inspect_obus(path)
    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    expected_sequence = {
        "bit_depth": 10,
        "max_width": variant.width,
        "max_height": variant.height,
        "monochrome": False,
        "subsampling_x": variant.subsampling_x,
        "subsampling_y": variant.subsampling_y,
        "enable_superres": False,
        "enable_cdef": variant.enable_cdef,
        "enable_restoration": variant.enable_restoration,
    }
    if not sequence_headers or any(
        any(header.get(key) != value for key, value in expected_sequence.items())
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence header differs from the 10-bit filter profile")

    samples = report["samples"]
    frame_headers = [
        obu["frame_header"]
        for sample in samples
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    if len(samples) != 1 or len(frame_headers) != 1:
        raise RuntimeError("AVIF must contain one still AV1 picture")

    frame = frame_headers[0]
    quantization = frame.get("quantization", {})
    loop_filter = frame.get("loop_filter", {})
    restoration = frame.get("restoration", {})
    if (
        frame.get("frame_type") != "key"
        or frame.get("frame_width") != variant.width
        or frame.get("frame_height") != variant.height
        or frame.get("upscaled_width") != variant.width
        or frame.get("superres_enabled") is not False
        or frame.get("all_lossless") is not False
        or frame.get("transform_mode") != "largest"
        or quantization.get("base", 0) == 0
        or quantization.get("using_matrix") is not False
        or loop_filter.get("level_y") != [0, 0]
        or loop_filter.get("level_u") != 0
        or loop_filter.get("level_v") != 0
        or frame.get("delta_q", {}).get("present") is not False
        or frame.get("delta_loop_filter", {}).get("present") is not False
        or frame.get("segmentation", {}).get("enabled") is not False
        or restoration.get("types") != list(variant.restoration_types)
        or restoration.get("unit_size_log2") != list(variant.restoration_unit_size_log2)
    ):
        raise RuntimeError("AV1 frame differs from the bounded 10-bit filter profile")

    cdef = frame.get("cdef", {})
    if variant.cdef_bits is not None and (
        cdef.get("bits") != variant.cdef_bits
        or cdef.get("damping") != variant.cdef_damping
        or cdef.get("y_strengths") != list(variant.cdef_y_strengths or ())
        or cdef.get("uv_strengths") != list(variant.cdef_uv_strengths or ())
    ):
        raise RuntimeError("AV1 CDEF parameters differ from the pinned 10-bit profile")

    return {
        "sequence_header": expected_sequence,
        "frame_type": frame["frame_type"],
        "restoration_types": restoration["types"],
        "restoration_unit_size_log2": restoration["unit_size_log2"],
        "cdef": cdef,
        "base_qindex": quantization["base"],
    }


def validate_pillow(path: Path, variant: FixtureVariant) -> bytes:
    """Return exact RGB bytes from the pinned Pillow/libavif oracle."""

    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"Pillow oracle differs from pin: {observed!r}")

    with Image.open(path) as image:
        if (
            image.format != "AVIF"
            or image.size != (variant.width, variant.height)
            or image.mode != "RGB"
        ):
            raise RuntimeError("Pillow opened AVIF with unexpected image properties")
        image.verify()

    with Image.open(path) as image:
        image.load()
        pixels = image.tobytes()
    if (
        len(pixels) != variant.width * variant.height * 3
        or sha256(pixels) != variant.rgb_sha256
    ):
        raise RuntimeError("Pillow RGB bytes differ from the pinned fixture reference")
    return pixels


def validate_existing(asset: Path, reference: Path, variant: FixtureVariant) -> None:
    """Verify fixture bytes, AV1 syntax, and the pinned Pillow reference hash."""

    if sha256(asset.read_bytes()) != variant.asset_sha256:
        raise RuntimeError("committed AVIF asset differs from its pinned SHA-256")
    validate_bitstream(asset, variant)
    if sha256(reference.read_bytes()) != variant.rgb_sha256:
        raise RuntimeError("committed RGB reference differs from its pinned Pillow hash")


def generate(
    avifenc: Path,
    asset: Path,
    reference: Path,
    variant: FixtureVariant,
) -> None:
    """Encode twice, validate independent pixels, and write the parity fixture."""

    avifenc = avifenc.expanduser().resolve()
    if not avifenc.is_file():
        raise RuntimeError(f"avifenc does not exist: {avifenc}")
    version = run([str(avifenc), "--version"])
    if version.splitlines()[0] != AVIFENC_VERSION:
        raise RuntimeError("avifenc must use libavif 1.4.1 and libaom 3.13.2")

    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix=f"av1-{variant.fixture_stem}-", dir=STAGING_ROOT
    ) as temporary:
        work = Path(temporary)
        source = work / "source.y4m"
        first = work / "first.avif"
        second = work / "second.avif"
        source_bytes = y4m_source(variant)
        if sha256(source_bytes) != variant.source_sha256:
            raise RuntimeError("generated Y4M source differs from its pinned SHA-256")
        source.write_bytes(source_bytes)

        run(encode_command(avifenc, source, first, variant))
        run(encode_command(avifenc, source, second, variant))
        asset_bytes = first.read_bytes()
        if asset_bytes != second.read_bytes() or sha256(asset_bytes) != variant.asset_sha256:
            raise RuntimeError("libaom output differs from the pinned AVIF bytes")

        profile = validate_bitstream(first, variant)
        pixels = validate_pillow(first, variant)
        asset.parent.mkdir(parents=True, exist_ok=True)
        reference.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(first, asset)
        reference.write_bytes(pixels)

    print(
        {
            "asset": str(asset),
            "asset_sha256": variant.asset_sha256,
            "reference": str(reference),
            "reference_sha256": variant.rgb_sha256,
            "profile": profile,
        }
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--variant", choices=tuple(VARIANTS), default="i420")
    parser.add_argument(
        "--avifenc",
        type=Path,
        help="avifenc built from libavif 1.4.1 and libaom 3.13.2",
    )
    parser.add_argument("--asset", type=Path)
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--check", action="store_true", help="validate committed outputs only")
    args = parser.parse_args()

    default_asset, default_reference = fixture_paths(args.variant)
    asset = args.asset or default_asset
    reference = args.reference or default_reference
    variant = VARIANTS[args.variant]

    if args.check:
        if args.avifenc is not None:
            parser.error("--check cannot be combined with --avifenc")
        validate_existing(asset, reference, variant)
        print(f"Verified {asset} and {reference}")
    elif args.avifenc is None:
        parser.error("--avifenc is required unless --check is used")
    else:
        generate(args.avifenc, asset, reference, variant)


if __name__ == "__main__":
    main()
