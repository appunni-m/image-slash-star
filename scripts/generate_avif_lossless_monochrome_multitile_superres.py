#!/usr/bin/env python3
"""Generate deterministic lossless monochrome AVIF edge-case fixtures."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile

from PIL import Image, _avif, features

from inspect_av1_obus import inspect as inspect_av1
from inspect_avif_bitstreams import children, inspect as inspect_avif, parse_boxes


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET_ROOT = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
SUPERRES_DENOMINATOR = 9
SUPERBLOCK_WIDTH = 128
SEQUENCE_MAX_WIDTH_OFFSET = 18
FRAME_USE_SUPERRES_OFFSET = 2


@dataclass(frozen=True)
class FixtureProfile:
    """Pinned geometry, encoder options, and oracle evidence for one AVIF fixture."""

    name: str
    asset_name: str
    coded_width: int
    upscaled_width: int
    height: int
    tile_columns: int
    width_bits: int
    height_bits: int
    header_bits: int
    source_sha256: str
    seed_sha256: str
    asset_sha256: str
    pixel_sha256: str
    superres_enabled: bool = True
    edge_tile_width: int | None = None
    min_partition_size: int = 16
    max_partition_size: int = 16
    source_pattern: str = "gradient"

    @property
    def asset(self) -> Path:
        """Return the retained input fixture path."""
        return ASSET_ROOT / self.asset_name


PROFILES = {
    "full-width-tiles": FixtureProfile(
        name="full-width-tiles",
        asset_name="portable_lossless_monochrome_multitile_superres_256x64.avif",
        coded_width=228,
        upscaled_width=256,
        height=64,
        tile_columns=2,
        width_bits=8,
        height_bits=6,
        header_bits=20,
        source_sha256="e0bca18edff09d90caf06c2f493f8c259016eac2b0353b6624968e40ba75508f",
        seed_sha256="834910dc7abe16434204c2c3d9f6e3337cae98f4a8b5321cd1ed9fdda36cdf43",
        asset_sha256="47d5096e64a5d61a6247575f645490813a55ba2ddd4c77379987c1a37ea6dd91",
        pixel_sha256="bc51fcf2f5bfe8ddd2d500f0ac1ef8fface82095da5127792ca79461518553a3",
    ),
    "narrow-edge-tile": FixtureProfile(
        name="narrow-edge-tile",
        asset_name="portable_lossless_monochrome_multitile_superres_narrow_edge_145x64.avif",
        coded_width=129,
        upscaled_width=145,
        height=64,
        tile_columns=2,
        width_bits=8,
        height_bits=6,
        header_bits=20,
        source_sha256="39cd9fb3d088121227af80c8103a76b449cde0d6a78868af7335991233a06b53",
        seed_sha256="1259fc77880dc1dd9718660221b8951fd3464da26ffe1bf11a820782a9413f3c",
        asset_sha256="aa73cdf640627b55ea1cc7a35af26a42f92bce39c504911c0e0fee6124905320",
        pixel_sha256="1a3a18cc362bf40b0001563429b6446baca238f80a7a23adb6fbb54a3d12fd55",
        edge_tile_width=1,
    ),
    "above-right-context": FixtureProfile(
        name="above-right-context",
        asset_name="lossless_monochrome_above_right_superres_163x64.avif",
        coded_width=145,
        upscaled_width=163,
        height=64,
        tile_columns=2,
        width_bits=8,
        height_bits=6,
        header_bits=20,
        source_sha256="682eea88d3e2b3e90a8faf47cc0e31ed9aa69376853be937538a2a420b8ce282",
        seed_sha256="623af9c9ba910356c1d2a18e4856eab44a2b9e4f7efcdd0cf34be9cb7cf5d316",
        asset_sha256="b60d77de7232868f5c4796875641dd93c0ec00f0fc29ae911af06ca8a36c9f0a",
        pixel_sha256="099e4cf9fcfa8ddc4269585a7effc6b3d7ea96c956bf50a104d79e971302cea9",
        edge_tile_width=17,
    ),
    "above-right-narrow-edge-145x128": FixtureProfile(
        name="above-right-narrow-edge-145x128",
        asset_name="lossless_monochrome_above_right_narrow_edge_superres_145x128.avif",
        coded_width=129,
        upscaled_width=145,
        height=128,
        tile_columns=2,
        width_bits=8,
        height_bits=7,
        header_bits=20,
        source_sha256="452d0b75202b87d40d5fcfb6de56656e8cef3567154493de037f8c019eeb8e21",
        seed_sha256="24b488b2c942b98a4d3c940bd307eccfa61a2e542e4db10be271aed2ec7ccc34",
        asset_sha256="fedc4c2dd4e3418ae0d55885679617bcf566cc5e003c46da337f15238974301a",
        pixel_sha256="8840b809781870585889c541ab6ed1b204410460a5baca62fd759cf64b13b417",
        edge_tile_width=1,
        min_partition_size=4,
        max_partition_size=64,
        source_pattern="above-right-tjunction",
    ),
    "adaptive-above-right-128x128": FixtureProfile(
        name="adaptive-above-right-128x128",
        asset_name="lossless_monochrome_adaptive_above_right_128x128.avif",
        coded_width=128,
        upscaled_width=128,
        height=128,
        tile_columns=1,
        width_bits=7,
        height_bits=7,
        header_bits=16,
        source_sha256="24acd1b7f08c711bb1e394f72673d14db9b4c8dde89648cb704b2bfd25c657b2",
        seed_sha256="fb2a58549ade32caec13ddd8fa35adc8ddb57e0d87de5d73c620445b04ff1d67",
        asset_sha256="fb2a58549ade32caec13ddd8fa35adc8ddb57e0d87de5d73c620445b04ff1d67",
        pixel_sha256="eaaec8d21329a21ee563b5d9bd57052f1d6c3d5dbd9992cd2862f85b3b9f206c",
        superres_enabled=False,
        min_partition_size=4,
        max_partition_size=64,
        source_pattern="above-right-tjunction",
    ),
}

ADVANCED_OPTIONS = (
    ("superres-mode", "1"),
    ("superres-denominator", "9"),
    ("superres-kf-denominator", "9"),
    ("min-partition-size", "16"),
    ("max-partition-size", "16"),
    ("enable-tx-size-search", "0"),
    ("enable-restoration", "1"),
    ("enable-cdef", "0"),
    ("loopfilter-control", "0"),
    ("enable-global-motion", "0"),
    ("enable-warped-motion", "0"),
    ("enable-masked-comp", "0"),
    ("enable-dist-wtd-comp", "0"),
    ("enable-onesided-comp", "0"),
    ("enable-interintra-comp", "0"),
    ("enable-diff-wtd-comp", "0"),
    ("enable-interinter-wedge", "0"),
    ("enable-interintra-wedge", "0"),
    ("enable-obmc", "0"),
    ("enable-ref-frame-mvs", "0"),
    ("enable-intrabc", "0"),
    ("aq-mode", "0"),
    ("deltaq-mode", "0"),
    ("enable-chroma-deltaq", "0"),
)
LOSSLESS_OPTIONS = tuple(
    (key, "0" if key == "enable-restoration" else value)
    for key, value in ADVANCED_OPTIONS
)


def sha256(data: bytes) -> str:
    """Return a lowercase SHA-256 digest."""
    return hashlib.sha256(data).hexdigest()


def source_y4m(profile: FixtureProfile) -> bytes:
    """Build the fixed full-range monochrome encoder input for one profile."""
    if profile.source_pattern == "above-right-tjunction":
        def sample(x: int, y: int) -> int:
            if y < 64:
                return 72 if x < 64 else 184
            if x < 64:
                return (x * 73 + y * 29 + x * y * 17 + ((x ^ y) * 31)) & 0xFF
            return 192
    elif profile.source_pattern == "gradient":
        def sample(x: int, y: int) -> int:
            return (x * 13 + y * 7 + ((x * y) & 31)) & 0xFF
    else:
        raise ValueError(f"unsupported monochrome source pattern: {profile.source_pattern}")

    pixels = bytes(sample(x, y) for y in range(profile.height) for x in range(profile.coded_width))
    header = (
        f"YUV4MPEG2 W{profile.coded_width} H{profile.height} F30:1 Ip A1:1 Cmono "
        "XCOLORRANGE=FULL\nFRAME\n"
    )
    return header.encode("ascii") + pixels

def run(command: list[str]) -> str:
    """Run one pinned fixture-generation command."""
    result = subprocess.run(
        command,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return result.stdout


def encode_command(
    avifenc: Path, source: Path, output: Path, profile: FixtureProfile
) -> list[str]:
    """Build the deterministic lossless seed command for one profile."""
    if profile.tile_columns <= 0 or profile.tile_columns & (profile.tile_columns - 1):
        raise ValueError("tile column count must be a positive power of two")
    command = [
        str(avifenc),
        "-q",
        "100",
        "-s",
        "0",
        "-j",
        "1",
        "--tilecolslog2",
        str(profile.tile_columns.bit_length() - 1),
        "--tilerowslog2",
        "0",
    ]
    partition_sizes = {
        "min-partition-size": str(profile.min_partition_size),
        "max-partition-size": str(profile.max_partition_size),
    }
    for key, value in LOSSLESS_OPTIONS:
        value = partition_sizes.get(key, value)
        command.extend(("--advanced", f"{key}={value}"))
    command.extend((str(source), str(output)))
    return command


def set_field(bits: str, start: int, width: int, value: int) -> str:
    """Replace one bounded MSB-first AV1 syntax field."""
    if start < 0 or width < 1 or start + width > len(bits):
        raise ValueError("AV1 syntax field lies outside its payload")
    if not 0 <= value < (1 << width):
        raise ValueError("AV1 syntax value does not fit its field")
    return bits[:start] + f"{value:0{width}b}" + bits[start + width :]


def bytes_from_bits(bits: str) -> bytes:
    """Pack an aligned MSB-first bit string."""
    if len(bits) % 8:
        raise ValueError("AV1 payload is not byte aligned")
    return bytes(
        int(bits[offset : offset + 8], 2)
        for offset in range(0, len(bits), 8)
    )


def ispe_dimensions(data: bytes) -> tuple[int, int, int]:
    """Return the sole primary ispe dimensions and width-field offset."""
    top_level = parse_boxes(data, 0, len(data))
    meta = next((box for box in top_level if box.kind == b"meta"), None)
    if meta is None:
        raise ValueError("AVIF seed has no meta box")
    iprp = next(
        (box for box in children(data, meta, prefix=4) if box.kind == b"iprp"),
        None,
    )
    if iprp is None:
        raise ValueError("AVIF seed has no item properties")
    ipco = next((box for box in children(data, iprp) if box.kind == b"ipco"), None)
    if ipco is None:
        raise ValueError("AVIF seed has no item property container")
    ispe = [box for box in children(data, ipco) if box.kind == b"ispe"]
    if len(ispe) != 1:
        raise ValueError("AVIF seed must contain exactly one ispe property")
    width_offset = ispe[0].payload_start + 4
    width, height = struct.unpack_from(">II", data, width_offset)
    return width, height, width_offset


def patch_superres(
    seed: bytes,
    seed_path: Path,
    profile: FixtureProfile,
) -> bytes:
    """Enable denominator-9 syntax while preserving the lossless tile payload."""
    report = inspect_av1(seed_path)
    samples = report["samples"]
    if len(samples) != 1 or len(samples[0]["spans"]) != 1:
        raise ValueError("AVIF seed must contain one contiguous AV1 item")
    sample = samples[0]
    sequence_obus = [obu for obu in sample["obus"] if "sequence_header" in obu]
    frame_obus = [obu for obu in sample["obus"] if "frame_header" in obu]
    if len(sequence_obus) != 1 or len(frame_obus) != 1:
        raise ValueError("AVIF seed must contain one sequence and frame header")
    sequence_obu = sequence_obus[0]
    frame_obu = frame_obus[0]
    sequence = sequence_obu["sequence_header"]
    frame = frame_obu["frame_header"]
    if (
        sequence["profile"] != 0
        or not sequence["still_picture"]
        or not sequence["reduced_still_picture_header"]
        or sequence["max_width"] != profile.coded_width
        or sequence["max_height"] != profile.height
        or sequence["width_bits"] != profile.width_bits
        or sequence["height_bits"] != profile.height_bits
        or not sequence["monochrome"]
        or sequence["enable_superres"]
        or sequence["enable_cdef"]
        or sequence["enable_restoration"]
        or frame["frame_type"] != "key"
        or frame["frame_width"] != profile.coded_width
        or frame["upscaled_width"] != profile.coded_width
        or frame["frame_height"] != profile.height
        or frame["superres_enabled"]
        or not frame["all_lossless"]
        or frame["tiling"]["columns"] != profile.tile_columns
        or frame["tiling"]["rows"] != 1
        or frame["header_bits"] != profile.header_bits
    ):
        raise ValueError("AVIF seed differs from its pinned lossless tile profile")

    patched = bytearray(seed)
    sequence_spans = sequence_obu["payload_spans"]
    if len(sequence_spans) != 1:
        raise ValueError("AV1 sequence header must have one payload span")
    sequence_span = sequence_spans[0]
    sequence_payload = seed[
        sequence_span["offset"] : sequence_span["offset"]
        + sequence_obu["payload_length"]
    ]
    sequence_bits = "".join(f"{byte:08b}" for byte in sequence_payload)
    sequence_bits = set_field(
        sequence_bits,
        SEQUENCE_MAX_WIDTH_OFFSET,
        sequence["width_bits"],
        profile.upscaled_width - 1,
    )
    sequence_superres_offset = (
        3
        + 1
        + 1
        + 5
        + 4
        + 4
        + sequence["width_bits"]
        + sequence["height_bits"]
        + 3
    )
    sequence_bits = set_field(sequence_bits, sequence_superres_offset, 1, 1)
    patched_sequence = bytes_from_bits(sequence_bits)
    if len(patched_sequence) != len(sequence_payload):
        raise ValueError("sequence-header patch changed its payload size")
    patched[
        sequence_span["offset"] : sequence_span["offset"] + len(sequence_payload)
    ] = patched_sequence

    frame_spans = frame_obu["payload_spans"]
    if len(frame_spans) != 1:
        raise ValueError("AV1 frame header must have one payload span")
    frame_span = frame_spans[0]
    frame_payload = seed[
        frame_span["offset"] : frame_span["offset"] + frame_obu["payload_length"]
    ]
    frame_bits = "".join(f"{byte:08b}" for byte in frame_payload)
    header_bits = frame["header_bits"]
    aligned_header_bits = (header_bits + 7) & ~7
    if any(bit != "0" for bit in frame_bits[header_bits:aligned_header_bits]):
        raise ValueError("AVIF seed frame header has invalid byte-alignment bits")
    frame_header = (
        frame_bits[:FRAME_USE_SUPERRES_OFFSET]
        + "1"
        + f"{SUPERRES_DENOMINATOR - 9:03b}"
        + frame_bits[FRAME_USE_SUPERRES_OFFSET:header_bits]
    )
    if len(frame_header) != aligned_header_bits:
        raise ValueError("superres syntax does not consume the seed's alignment bits")
    patched_frame = bytes_from_bits(frame_header + frame_bits[aligned_header_bits:])
    if len(patched_frame) != len(frame_payload):
        raise ValueError("frame-header patch changed its payload size")
    patched[frame_span["offset"] : frame_span["offset"] + len(frame_payload)] = (
        patched_frame
    )

    width, height, width_offset = ispe_dimensions(seed)
    if (width, height) != (profile.coded_width, profile.height):
        raise ValueError("AVIF seed ispe dimensions differ from the coded raster")
    struct.pack_into(">I", patched, width_offset, profile.upscaled_width)
    if len(patched) != len(seed):
        raise ValueError("patch changed the AVIF container size")
    return bytes(patched)

def validate(path: Path, profile: FixtureProfile) -> dict[str, object]:
    """Verify the AVIF container, AV1 syntax, and pinned Pillow pixels."""
    data = path.read_bytes()
    if sha256(data) != profile.asset_sha256:
        raise ValueError("fixture SHA-256 differs from its pin")
    oracle = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if oracle != PINNED_ORACLE:
        raise ValueError(f"Pillow/native version differs from pin: {oracle!r}")

    width, height, _ = ispe_dimensions(data)
    if (width, height) != (profile.upscaled_width, profile.height):
        raise ValueError("AVIF ispe dimensions differ from the requested output")
    container = inspect_avif(path)
    color_items = container["items"]["color"]
    if (
        len(color_items) != 1
        or color_items[0]["type"] != "av01"
        or container["items"]["alpha"]
    ):
        raise ValueError("fixture is not one monochrome AV1 color item")

    report = inspect_av1(path)
    samples = report["samples"]
    if len(samples) != 1 or len(samples[0]["spans"]) != 1:
        raise ValueError("fixture must contain one contiguous AV1 sample")
    sequence = next(
        obu["sequence_header"]
        for obu in samples[0]["obus"]
        if "sequence_header" in obu
    )
    frame = next(
        obu["frame_header"] for obu in samples[0]["obus"] if "frame_header" in obu
    )
    expected_sequence = {
        "profile": 0,
        "still_picture": True,
        "reduced_still_picture_header": True,
        "max_width": profile.upscaled_width,
        "max_height": profile.height,
        "width_bits": profile.width_bits,
        "height_bits": profile.height_bits,
        "monochrome": True,
        "enable_superres": profile.superres_enabled,
        "enable_cdef": False,
        "enable_restoration": False,
    }
    expected_frame = {
        "frame_type": "key",
        "frame_width": profile.coded_width,
        "frame_height": profile.height,
        "upscaled_width": profile.upscaled_width,
        "superres_enabled": profile.superres_enabled,
        "all_lossless": True,
    }
    if profile.superres_enabled:
        expected_frame["superres_denominator"] = SUPERRES_DENOMINATOR
    if any(sequence.get(key) != value for key, value in expected_sequence.items()):
        raise ValueError("sequence header differs from the pinned AV1 profile")
    if any(frame.get(key) != value for key, value in expected_frame.items()):
        raise ValueError("frame header differs from the pinned lossless profile")
    tiling = frame.get("tiling", {})
    if tiling.get("columns") != profile.tile_columns or tiling.get("rows") != 1:
        raise ValueError("fixture does not use its pinned tile layout")
    if profile.edge_tile_width is not None:
        if (
            tiling.get("column_starts") != [0, 1, 2]
            or profile.coded_width - SUPERBLOCK_WIDTH != profile.edge_tile_width
        ):
            raise ValueError("fixture does not retain its pinned right-edge tile")

    with Image.open(path) as image:
        if (
            image.size != (profile.upscaled_width, profile.height)
            or image.mode != "RGB"
            or image.n_frames != 1
        ):
            raise ValueError(
                f"Pillow must decode one {profile.upscaled_width}x{profile.height} RGB frame"
            )
        image.load()
        pixels = image.tobytes()
    pixel_hash = sha256(pixels)
    if (
        len(pixels) != profile.upscaled_width * profile.height * 3
        or pixel_hash != profile.pixel_sha256
    ):
        raise ValueError("Pillow RGB pixels differ from the pinned exact reference")
    return {
        "asset": path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else "staging",
        "bytes": len(data),
        "sha256": sha256(data),
        "coded_size": [profile.coded_width, profile.height],
        "upscaled_size": [profile.upscaled_width, profile.height],
        "tile_columns": profile.tile_columns,
        "superres_denominator": SUPERRES_DENOMINATOR if profile.superres_enabled else None,
        "all_lossless": True,
        "edge_tile_width": profile.edge_tile_width,
        "pixels": {
            "mode": "RGB",
            "bytes": len(pixels),
            "sha256": pixel_hash,
        },
        "verified": True,
    }

def generate(avifenc: Path, output: Path, profile: FixtureProfile) -> None:
    """Double-encode a pinned seed, patch its headers, and retain the fixture."""
    avifenc = avifenc.expanduser().resolve()
    if not avifenc.is_file():
        raise ValueError(f"avifenc does not exist: {avifenc}")
    version = run([str(avifenc), "--version"])
    if "1.4.1" not in version or "3.13.2" not in version:
        raise ValueError("avifenc must use libavif 1.4.1 and libaom 3.13.2")

    source_data = source_y4m(profile)
    if sha256(source_data) != profile.source_sha256:
        raise ValueError("generated Y4M input differs from its pin")
    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix=f".avif-{profile.name}-",
        dir=STAGING_ROOT,
    ) as temporary:
        work = Path(temporary)
        source = work / "source.y4m"
        source.write_bytes(source_data)
        seeds = []
        for index in range(2):
            seed_path = work / f"seed-{index}.avif"
            run(encode_command(avifenc, source, seed_path, profile))
            seed = seed_path.read_bytes()
            if sha256(seed) != profile.seed_sha256:
                raise ValueError("AVIF seed differs from its pinned encoder bytes")
            seeds.append(seed)
        if seeds[0] != seeds[1]:
            raise ValueError("independent AVIF lossless seed encodes differ")

        seed_path = work / "seed-inspection.avif"
        seed_path.write_bytes(seeds[0])
        patched = (
            patch_superres(seeds[0], seed_path, profile)
            if profile.superres_enabled
            else seeds[0]
        )
        candidate = work / "fixture.avif"
        candidate.write_bytes(patched)
        validate(candidate, profile)
        if output.exists() and output.read_bytes() != patched:
            raise ValueError(f"refusing to replace a different fixture at {output}")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(patched)
    print(
        json.dumps(
            {**validate(output, profile), "avifenc": str(avifenc), "version": version.strip()},
            indent=2,
        )
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--profile",
        choices=tuple(PROFILES),
        default="full-width-tiles",
        help="Pinned tile geometry to generate or verify",
    )
    parser.add_argument("--avifenc", type=Path, help="Pinned libavif/libaom avifenc executable")
    parser.add_argument("--output", type=Path, help="Optional output below target/oracle-staging")
    parser.add_argument("--check", action="store_true", help="Verify the retained fixture")
    arguments = parser.parse_args()
    profile = PROFILES[arguments.profile]
    output = profile.asset if arguments.output is None else arguments.output.expanduser().resolve()
    if arguments.check and arguments.avifenc is not None:
        parser.error("--check cannot be combined with --avifenc")
    if output != profile.asset and STAGING_ROOT.resolve() not in output.parents:
        parser.error("--output must be the selected fixture or below target/oracle-staging")
    try:
        if arguments.check:
            print(json.dumps(validate(profile.asset, profile), indent=2))
        elif arguments.avifenc is None:
            parser.error("--avifenc is required unless --check is used")
        else:
            generate(arguments.avifenc, output, profile)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"error: {error}\n")

if __name__ == "__main__":
    main()
