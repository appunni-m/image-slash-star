#!/usr/bin/env python3
"""Generate pinned full-range 12-bit monochrome AVIF parity fixtures."""

from __future__ import annotations

import argparse
import hashlib
import struct
import subprocess
import tempfile
import zlib
from pathlib import Path

from PIL import Image, _avif, features

from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET_PATH = (
    ROOT
    / "tests/fixtures/input/images/avif/"
    / "portable_lossless_monochrome_fullrange_12bit_17x9.avif"
)
ALPHA_ASSET_PATH = (
    ROOT
    / "tests/fixtures/input/images/avif/"
    / "portable_lossless_monochrome_fullrange_alpha_12bit_17x9.avif"
)
WIDTH = 17
HEIGHT = 9
SAMPLE_COUNT = WIDTH * HEIGHT
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
SOURCE_SHA256 = "a4f197e65aba75c8d3249f0ac38aab7ab2ba30085087d88636a6dc3b696e507e"
ASSET_SHA256 = "07793973838382f7de986377c7ed2ea139144b1a4413092e65bb6713db10ae57"
PIXELS_SHA256 = "02e8eedf632446b91a506c81aafd795b434a14723394faeb249b74a3cfb2e908"
ALPHA_SOURCE_SHA256 = "8b2b2f01fd6a50ebbe5ec97cee723ac207bb1d33af437a95ecf7a0eb9a74533b"
ALPHA_ASSET_SHA256 = "3acad68f56c9eced8f975303cce603d3404fd00d10bb2f92b24573395b34c25b"
ALPHA_PIXELS_SHA256 = "2a39c5bb870685946e9b0906f6a860916225086d138851b8e324af5bb93a678f"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def source_samples() -> list[int]:
    """Return a varied 12-bit plane including both endpoints and every low nibble."""
    samples = [
        (x * 257 + y * 641 + x * y * 23) & 0xFFF
        for y in range(HEIGHT)
        for x in range(WIDTH)
    ]
    if min(samples) != 0 or max(samples) != 0xFFF:
        raise RuntimeError("the monochrome source must include both 12-bit endpoints")
    if {sample & 0xF for sample in samples} != set(range(16)):
        raise RuntimeError("the monochrome source must include all low-nibble values")
    return samples


def source_y4m(samples: list[int]) -> bytes:
    """Build one full-range Cmono12 Y4M frame with little-endian samples."""
    if len(samples) != SAMPLE_COUNT:
        raise RuntimeError("the monochrome source has an unexpected sample count")
    header = (
        f"YUV4MPEG2 W{WIDTH} H{HEIGHT} F30:1 Ip A1:1 "
        "Cmono12 XYSCSS=400 XCOLORRANGE=FULL\nFRAME\n"
    ).encode("ascii")
    return header + struct.pack(f"<{SAMPLE_COUNT}H", *samples)


def alpha_source_png() -> bytes:
    """Build a 16-bit grayscale-plus-alpha PNG from varied 12-bit samples."""
    rows = bytearray()
    luma_samples: set[int] = set()
    alpha_samples: set[int] = set()
    for y in range(HEIGHT):
        rows.append(0)  # PNG filter: None.
        for x in range(WIDTH):
            luma = (x * 257 + y * 641 + x * y * 23) & 0xFFF
            alpha = (x * 719 + y * 383 + x * y * 11) & 0xFFF
            if (x, y) == (WIDTH - 1, HEIGHT - 1):
                alpha = 0xFFF
            luma_samples.add(luma)
            alpha_samples.add(alpha)
            # Choose the first 16-bit value that maps back to each 12-bit
            # sample when the pinned encoder reduces PNG input depth.
            luma_16 = (luma * 0xFFFF + 0xFFE) // 0xFFF
            alpha_16 = (alpha * 0xFFFF + 0xFFE) // 0xFFF
            rows.extend(struct.pack(">HH", luma_16, alpha_16))
    if min(luma_samples) != 0 or max(luma_samples) != 0xFFF:
        raise RuntimeError("the monochrome source must include both 12-bit endpoints")
    if min(alpha_samples) != 0 or max(alpha_samples) != 0xFFF:
        raise RuntimeError("the alpha source must include both 12-bit endpoints")
    if {sample & 0xF for sample in luma_samples} != set(range(16)):
        raise RuntimeError("the monochrome source must include all low-nibble values")
    if {sample & 0xF for sample in alpha_samples} != set(range(16)):
        raise RuntimeError("the alpha source must include all low-nibble values")

    def chunk(kind: bytes, data: bytes) -> bytes:
        body = kind + data
        return (
            struct.pack(">I", len(data))
            + body
            + struct.pack(">I", zlib.crc32(body) & 0xFFFF_FFFF)
        )

    header = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 16, 4, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(rows))
        + chunk(b"IEND", b"")
    )


def run(command: list[str]) -> str:
    result = subprocess.run(
        command,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(f"command failed ({result.returncode}): {' '.join(command)}\n{result.stdout}")
    return result.stdout


def validate_tool_version(tool: Path, name: str) -> None:
    if not tool.is_file():
        raise RuntimeError(f"{name} does not exist: {tool}")
    version = run([str(tool), "--version"])
    expected = "Version: 1.4.1 (aom [enc/dec]:3.13.2)"
    if expected not in version:
        raise RuntimeError(f"{name} must use clean libavif 1.4.1 and libaom 3.13.2: {version}")


def validate_bitstream(path: Path) -> None:
    report = inspect_obus(path)
    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    frame_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    if len(report["samples"]) != 1 or len(sequence_headers) != 1 or len(frame_headers) != 1:
        raise RuntimeError("AVIF must contain one sample with one sequence and frame header")

    sequence = sequence_headers[0]
    expected_sequence = {
        "profile": 2,
        "bit_depth": 12,
        "max_width": WIDTH,
        "max_height": HEIGHT,
        "monochrome": True,
        "color_range": 1,
        "subsampling_x": 1,
        "subsampling_y": 1,
    }
    if any(sequence.get(key) != value for key, value in expected_sequence.items()):
        raise RuntimeError("AV1 sequence header differs from the 12-bit full-range monochrome profile")

    frame = frame_headers[0]
    expected_frame = {
        "frame_type": "key",
        "frame_width": WIDTH,
        "frame_height": HEIGHT,
        "upscaled_width": WIDTH,
        "superres_enabled": False,
        "all_lossless": True,
    }
    if any(frame.get(key) != value for key, value in expected_frame.items()):
        raise RuntimeError("AV1 frame header is not a 17x9 all-lossless key frame")


def validate_decoded_samples(avifdec: Path, path: Path, expected: list[int], work: Path) -> None:
    decoded_path = work / "decoded.y4m"
    run([str(avifdec), str(path), str(decoded_path)])
    decoded = decoded_path.read_bytes()
    header, separator, remainder = decoded.partition(b"\n")
    expected_tokens = {
        "YUV4MPEG2",
        f"W{WIDTH}",
        f"H{HEIGHT}",
        "Cmono12",
        "XYSCSS=400",
        "XCOLORRANGE=FULL",
    }
    if not separator or not expected_tokens.issubset(set(header.decode("ascii").split())):
        raise RuntimeError("avifdec did not preserve the 17x9 full-range Cmono12 declaration")
    frame_marker = b"FRAME\n"
    if not remainder.startswith(frame_marker):
        raise RuntimeError("avifdec output is missing its one Y4M frame")
    plane = remainder[len(frame_marker) :]
    if len(plane) != SAMPLE_COUNT * 2:
        raise RuntimeError("avifdec output has an unexpected monochrome plane length")
    decoded_samples = list(struct.unpack(f"<{SAMPLE_COUNT}H", plane))
    if decoded_samples != expected:
        raise RuntimeError("the AV1 lossless frame did not preserve all 12-bit source samples")


def validate_pillow(path: Path) -> None:
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"Pillow oracle differs from pin: {observed!r}")

    with Image.open(path) as image:
        image.verify()

    with Image.open(path) as image:
        image.load()
        pixels = image.tobytes()
        if (
            image.format != "AVIF"
            or image.mode != "RGB"
            or image.size != (WIDTH, HEIGHT)
            or image.n_frames != 1
            or len(pixels) != SAMPLE_COUNT * 3
            or sha256(pixels) != PIXELS_SHA256
        ):
            raise RuntimeError("Pillow RGB pixels differ from the pinned AVIF still reference")


def validate_alpha_bitstream(path: Path) -> None:
    report = inspect_obus(path)
    if [sample["role"] for sample in report["samples"]] != ["item_color", "item_alpha"]:
        raise RuntimeError("AVIF must contain monochrome color and auxiliary-alpha items")
    for sample in report["samples"]:
        headers = [
            obu["sequence_header"]
            for obu in sample["obus"]
            if "sequence_header" in obu
        ]
        frames = [
            obu["frame_header"]
            for obu in sample["obus"]
            if "frame_header" in obu
        ]
        if len(headers) != 1 or len(frames) != 1:
            raise RuntimeError("each AV1 item must contain one sequence and frame header")
        sequence = headers[0]
        expected_sequence = {
            "profile": 2,
            "bit_depth": 12,
            "max_width": WIDTH,
            "max_height": HEIGHT,
            "monochrome": True,
            "color_range": 1,
        }
        if any(sequence.get(key) != value for key, value in expected_sequence.items()):
            raise RuntimeError("AV1 item differs from the 12-bit full-range monochrome profile")
        frame = frames[0]
        if (
            frame.get("frame_type") != "key"
            or frame.get("frame_width") != WIDTH
            or frame.get("frame_height") != HEIGHT
            or not frame.get("all_lossless")
        ):
            raise RuntimeError("AV1 item is not a 17x9 all-lossless key frame")


def validate_alpha_pillow(path: Path) -> None:
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_PILLOW:
        raise RuntimeError(f"Pillow oracle differs from pin: {observed!r}")
    with Image.open(path) as image:
        image.verify()
    with Image.open(path) as image:
        image.load()
        pixels = image.tobytes()
        if (
            image.format != "AVIF"
            or image.mode != "RGBA"
            or image.size != (WIDTH, HEIGHT)
            or image.n_frames != 1
            or len(pixels) != SAMPLE_COUNT * 4
            or sha256(pixels) != ALPHA_PIXELS_SHA256
        ):
            raise RuntimeError("Pillow RGBA pixels differ from the pinned AVIF still reference")


def generate(avifenc: Path, avifdec: Path, check: bool) -> None:
    avifenc = avifenc.expanduser().resolve()
    avifdec = avifdec.expanduser().resolve()
    validate_tool_version(avifenc, "avifenc")
    validate_tool_version(avifdec, "avifdec")

    samples = source_samples()
    source = source_y4m(samples)
    if sha256(source) != SOURCE_SHA256:
        raise RuntimeError("generated Cmono12 Y4M source differs from its hash pin")

    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".avif-mono12-tail-", dir=STAGING_ROOT) as temporary:
        work = Path(temporary)
        source_path = work / "source.y4m"
        source_path.write_bytes(source)

        encoded_runs = []
        for index in range(2):
            candidate = work / f"candidate-{index}.avif"
            command = [
                str(avifenc),
                "-l",
                "-q",
                "100",
                "-s",
                "0",
                "-j",
                "1",
                str(source_path),
                str(candidate),
            ]
            run(command)
            data = candidate.read_bytes()
            if sha256(data) != ASSET_SHA256:
                raise RuntimeError(f"encoded AVIF differs from its hash pin: {sha256(data)}")
            validate_bitstream(candidate)
            validate_decoded_samples(avifdec, candidate, samples, work)
            validate_pillow(candidate)
            encoded_runs.append(data)

        if encoded_runs[0] != encoded_runs[1]:
            raise RuntimeError("independent full-range 12-bit AVIF encodes differ")

        if check:
            if not ASSET_PATH.is_file() or ASSET_PATH.read_bytes() != encoded_runs[0]:
                raise RuntimeError("committed AVIF fixture differs from the deterministic encoding")
        else:
            ASSET_PATH.parent.mkdir(parents=True, exist_ok=True)
            ASSET_PATH.write_bytes(encoded_runs[0])

    action = "Verified" if check else "Wrote"
    print(f"{action} {ASSET_PATH} ({len(encoded_runs[0])} bytes, sha256 {ASSET_SHA256})")

    alpha_source = alpha_source_png()
    if sha256(alpha_source) != ALPHA_SOURCE_SHA256:
        raise RuntimeError("generated 16-bit monochrome-alpha PNG differs from its hash pin")
    with tempfile.TemporaryDirectory(prefix=".avif-mono12-alpha-", dir=STAGING_ROOT) as temporary:
        work = Path(temporary)
        source_path = work / "source.png"
        source_path.write_bytes(alpha_source)
        encoded_runs = []
        for index in range(2):
            candidate = work / f"candidate-{index}.avif"
            command = [
                str(avifenc),
                "--lossless",
                "--qcolor",
                "100",
                "--qalpha",
                "100",
                "--speed",
                "0",
                "--jobs",
                "1",
                "--depth",
                "12",
                "--yuv",
                "400",
                "--range",
                "full",
                str(source_path),
                str(candidate),
            ]
            run(command)
            data = candidate.read_bytes()
            if sha256(data) != ALPHA_ASSET_SHA256:
                raise RuntimeError(f"encoded alpha AVIF differs from its hash pin: {sha256(data)}")
            validate_alpha_bitstream(candidate)
            validate_alpha_pillow(candidate)
            encoded_runs.append(data)

        if encoded_runs[0] != encoded_runs[1]:
            raise RuntimeError("independent full-range 12-bit alpha AVIF encodes differ")

        if check:
            if not ALPHA_ASSET_PATH.is_file() or ALPHA_ASSET_PATH.read_bytes() != encoded_runs[0]:
                raise RuntimeError("committed AVIF alpha fixture differs from the deterministic encoding")
        else:
            ALPHA_ASSET_PATH.parent.mkdir(parents=True, exist_ok=True)
            ALPHA_ASSET_PATH.write_bytes(encoded_runs[0])

    print(
        f"{action} {ALPHA_ASSET_PATH} "
        f"({len(encoded_runs[0])} bytes, sha256 {ALPHA_ASSET_SHA256})"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--avifenc",
        type=Path,
        required=True,
        help="avifenc built from libavif 1.4.1 and libaom 3.13.2",
    )
    parser.add_argument(
        "--avifdec",
        type=Path,
        required=True,
        help="avifdec built from libavif 1.4.1 and libaom 3.13.2 with AV1 decoding",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the existing fixture without writing it",
    )
    arguments = parser.parse_args()
    generate(arguments.avifenc, arguments.avifdec, arguments.check)


if __name__ == "__main__":
    main()
