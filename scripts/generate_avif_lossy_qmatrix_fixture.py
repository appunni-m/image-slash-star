#!/usr/bin/env python3
"""Generate a textured lossy AV1 still with uniform level-15 matrices.

The pinned 16x64 8-bit I420 image uses qindex 76 and matrix level 15 for all
three planes, reaching the normative no-matrix path during coefficient
reconstruction.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, _avif, features

from inspect_av1_obus import inspect as inspect_obus
from inspect_avif_bitstreams import inspect as inspect_container


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "lossy_420_qmatrix_level15_qindex76_16x64.avif"
)
WIDTH = 16
HEIGHT = 64
Y4M_SHA256 = "7465e8bb1f3dbfe2856791a35b5465b282f96737e4a7c00bfd119be14d22af2a"
ASSET_SHA256 = "23cdcad6c54eb49db6a5f8a28b29bb5504ec64cb0ef8489f8ba7216f2893a43d"
FRAME_SHA256 = "905944e04f18c055aff6942d86a85a0845f3eefa106db10fcc9ce14958e31851"
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def y4m_still() -> bytes:
    """Return a deterministic full-range I420 texture as one-frame Y4M."""
    output = bytearray(
        b"YUV4MPEG2 W16 H64 F10:1 Ip A1:1 C420jpeg XCOLORRANGE=FULL\nFRAME\n"
    )
    state = 0xA71
    for _ in range(WIDTH * HEIGHT + 2 * (WIDTH // 2) * (HEIGHT // 2)):
        state ^= (state << 13) & 0xFFFF_FFFF
        state ^= state >> 17
        state ^= (state << 5) & 0xFFFF_FFFF
        output.append(state & 0xFF)
    return bytes(output)


def run(command: list[str]) -> str:
    completed = subprocess.run(
        command,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return completed.stdout


def encode_command(avifenc: Path, source: Path, output: Path) -> list[str]:
    return [
        str(avifenc),
        "-q",
        "75",
        "-s",
        "0",
        "-j",
        "1",
        "--cicp",
        "1/13/6",
        "--advanced",
        "color:enable-qm=1",
        "--advanced",
        "color:qm-min=15",
        "--advanced",
        "color:qm-max=15",
        str(source),
        str(output),
    ]


def validate_bitstream(path: Path) -> dict[str, object]:
    container = inspect_container(path)
    if container["ftyp"]["major"] != "avif":
        raise RuntimeError("fixture does not declare an AVIF still image")

    report = inspect_obus(path)
    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    expected_sequence = {
        "bit_depth": 8,
        "max_width": WIDTH,
        "max_height": HEIGHT,
        "monochrome": False,
        "subsampling_x": 1,
        "subsampling_y": 1,
        "color_primaries": 1,
        "transfer_characteristics": 13,
        "matrix_coefficients": 6,
        "color_range": 1,
    }
    if not sequence_headers or any(
        any(header.get(key) != value for key, value in expected_sequence.items())
        for header in sequence_headers
    ):
        raise RuntimeError("sequence header differs from the pinned 8-bit I420 profile")

    frame_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    if len(frame_headers) != 1:
        raise RuntimeError("AV1 OBU report does not contain one still-image frame")
    header = frame_headers[0]
    quantization = header["quantization"]
    if (
        header.get("frame_type") != "key"
        or header.get("all_lossless") is not False
        or header.get("frame_width") != WIDTH
        or header.get("frame_height") != HEIGHT
        or quantization.get("base") != 76
        or quantization.get("using_matrix") is not True
        or tuple(quantization.get(key) for key in ("matrix_y", "matrix_u", "matrix_v"))
        != (15, 15, 15)
    ):
        raise RuntimeError("frame differs from the lossy qindex-76 level-15 profile")
    return {
        "frame_type": "key",
        "all_lossless": False,
        "base_qindex": 76,
        "using_matrix": True,
        "matrix_levels": [15, 15, 15],
    }


def pillow_frame(path: Path) -> dict[str, object]:
    if (Image.__version__, features.version("avif"), _avif.codec_versions()) != PINNED_PILLOW:
        raise RuntimeError("Pillow/libavif/codec versions differ from fixture pins")
    with Image.open(path) as image:
        if image.size != (WIDTH, HEIGHT) or image.mode != "RGB" or image.n_frames != 1:
            raise RuntimeError("Pillow did not decode the fixture as one 16x64 RGB image")
        image.load()
        pixels = image.tobytes()
        digest = sha256(pixels)
        if len(pixels) != WIDTH * HEIGHT * 3 or digest != FRAME_SHA256:
            raise RuntimeError("Pillow pixels differ from the pinned reference")
        return {
            "mode": image.mode,
            "size": [WIDTH, HEIGHT],
            "bytes": len(pixels),
            "sha256": digest,
        }


def verify_fixture(path: Path) -> dict[str, object]:
    source_hash = sha256(y4m_still())
    if source_hash != Y4M_SHA256:
        raise RuntimeError(f"generated Y4M source differs from pin: {source_hash}")
    asset_hash = sha256(path.read_bytes())
    if asset_hash != ASSET_SHA256:
        raise RuntimeError(f"fixture differs from pin: {asset_hash}")
    return {
        "asset": str(path),
        "asset_bytes": path.stat().st_size,
        "asset_sha256": asset_hash,
        "av1_frame": validate_bitstream(path),
        "pillow_frame": pillow_frame(path),
        "y4m_sha256": source_hash,
    }


def generate(args: argparse.Namespace) -> None:
    if args.avifenc is None:
        raise RuntimeError("--avifenc is required unless --check is used")
    avifenc = args.avifenc.expanduser().resolve()
    if not avifenc.is_file():
        raise RuntimeError(f"avifenc does not exist: {avifenc}")
    version = run([str(avifenc), "--version"])
    if "Version: 1.4.1" not in version or "aom [enc]:3.13.2" not in version:
        raise RuntimeError("avifenc does not use the pinned libavif 1.4.1/libaom 3.13.2")

    source = y4m_still()
    source_hash = sha256(source)
    if source_hash != Y4M_SHA256:
        raise RuntimeError(f"generated Y4M source differs from pin: {source_hash}")

    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".avif-qmatrix-lossy-", dir=STAGING_ROOT) as temp:
        work = Path(temp)
        source_path = work / "qmatrix-level15-lossy.y4m"
        source_path.write_bytes(source)
        raw_paths = [work / f"run-{index}.avif" for index in range(2)]
        for path in raw_paths:
            run(encode_command(avifenc, source_path, path))
        raw_bytes = [path.read_bytes() for path in raw_paths]
        if raw_bytes[0] != raw_bytes[1]:
            raise RuntimeError("independent AVIF still-image encodes differ")

        staging_output = work / "fixture.avif"
        staging_output.write_bytes(raw_bytes[0])
        validate_bitstream(staging_output)
        pillow_frame(staging_output)
        asset_hash = sha256(raw_bytes[0])
        if asset_hash != ASSET_SHA256:
            raise RuntimeError(f"generated AVIF differs from pin: {asset_hash}")

        output = args.output.expanduser().resolve() if args.output else ASSET.resolve()
        if output != ASSET.resolve() and STAGING_ROOT.resolve() not in output.parents:
            raise RuntimeError("output must be the fixture path or a staging path")
        output.parent.mkdir(parents=True, exist_ok=True)
        temporary_output = output.with_name(f".{output.name}.tmp")
        try:
            temporary_output.write_bytes(raw_bytes[0])
            temporary_output.replace(output)
        finally:
            temporary_output.unlink(missing_ok=True)

    print(json.dumps(verify_fixture(output), indent=2, sort_keys=True))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--avifenc", type=Path, help="pinned libavif/libaom avifenc binary")
    parser.add_argument("--output", type=Path, help="fixture path or target/oracle-staging path")
    parser.add_argument("--check", action="store_true", help="verify the checked-in fixture")
    args = parser.parse_args()
    if args.check:
        output = args.output.expanduser().resolve() if args.output else ASSET.resolve()
        print(json.dumps(verify_fixture(output), indent=2, sort_keys=True))
    else:
        generate(args)


if __name__ == "__main__":
    main()
