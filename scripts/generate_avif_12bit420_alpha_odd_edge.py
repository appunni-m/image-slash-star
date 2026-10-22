#!/usr/bin/env python3
"""Generate a pinned odd-edge 12-bit 4:2:0 AVIF alpha parity fixture."""

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
    / "high_bitdepth_still_12bit_420_alpha_odd_edge_17x9.avif"
)
WIDTH = 17
HEIGHT = 9
CHROMA_WIDTH = (WIDTH + 1) // 2
CHROMA_HEIGHT = (HEIGHT + 1) // 2
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
SOURCE_SHA256 = "328b5dcd7a453f3da9410f52bc46ae1de4077278ea84fcc0cc509404d5eadd7a"
ASSET_SHA256 = "ce408e4a66fe57b734544c2f30f17e90f725e2f2bae75d748f9eda75bdf23761"
PIXELS_SHA256 = "fdbe40dcc0814533e88fd4be8ae8a953973a7d9f4c635c21400b952396f0053c"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def source_samples(x: int, y: int) -> tuple[int, int, int, int]:
    """Return varied 12-bit RGBA codes with low bits at the odd edges."""
    alpha = (x * 719 + y * 383 + x * y * 11) & 0xFFF
    if (x, y) == (WIDTH - 1, HEIGHT - 1):
        alpha = 0xFFF
    return (
        (x * 257 + y * 641 + x * y * 23) & 0xFFF,
        (x * 719 + y * 383 + x * y * 11 + 137) & 0xFFF,
        (x * 433 + y * 827 + x * y * 29 + 941) & 0xFFF,
        alpha,
    )


def source_png() -> bytes:
    """Build a deterministic 16-bit RGBA PNG for the pinned 12-bit encoder."""
    rows = bytearray()
    channel_samples = [set() for _ in range(4)]
    for y in range(HEIGHT):
        rows.append(0)  # PNG filter: None.
        for x in range(WIDTH):
            for channel, sample in enumerate(source_samples(x, y)):
                channel_samples[channel].add(sample)
                # Map each 12-bit code back to the first matching 16-bit code.
                sample_16 = (sample * 0xFFFF + 0xFFE) // 0xFFF
                rows.extend(struct.pack(">H", sample_16))

    for channel, samples in zip("RGBA", channel_samples, strict=True):
        if len({sample & 0xF for sample in samples}) != 16:
            raise RuntimeError(f"the {channel} source must include every low-nibble value")
    for channel in (0, 3):
        if min(channel_samples[channel]) != 0 or max(channel_samples[channel]) != 0xFFF:
            raise RuntimeError("the red and alpha source planes must include both endpoints")

    def chunk(kind: bytes, data: bytes) -> bytes:
        body = kind + data
        return (
            struct.pack(">I", len(data))
            + body
            + struct.pack(">I", zlib.crc32(body) & 0xFFFF_FFFF)
        )

    header = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 16, 6, 0, 0, 0)
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
        raise RuntimeError(
            f"command failed ({result.returncode}): {' '.join(command)}\n{result.stdout}"
        )
    return result.stdout


def validate_tool_version(tool: Path, name: str) -> None:
    if not tool.is_file():
        raise RuntimeError(f"{name} does not exist: {tool}")
    version = run([str(tool), "--version"])
    expected = "Version: 1.4.1 (aom [enc/dec]:3.13.2)"
    if expected not in version:
        raise RuntimeError(f"{name} must use libavif 1.4.1 and libaom 3.13.2: {version}")


def validate_bitstream(path: Path) -> None:
    report = inspect_obus(path)
    samples = report["samples"]
    if [sample["role"] for sample in samples] != ["item_color", "item_alpha"]:
        raise RuntimeError("AVIF must contain color and auxiliary-alpha items in order")

    for sample in samples:
        role = sample["role"]
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
            raise RuntimeError(f"{role} must contain one sequence and frame header")

        header = headers[0]
        monochrome = role == "item_alpha"
        expected_header = {
            "profile": 2,
            "bit_depth": 12,
            "max_width": WIDTH,
            "max_height": HEIGHT,
            "monochrome": monochrome,
            "color_range": 1,
            "subsampling_x": 1,
            "subsampling_y": 1,
        }
        if role == "item_color":
            expected_header.update(
                color_primaries=1,
                transfer_characteristics=13,
                matrix_coefficients=6,
            )
        if any(header.get(key) != value for key, value in expected_header.items()):
            raise RuntimeError(f"AV1 {role} sequence differs from the pinned 12-bit 4:2:0 profile")

        frame = frames[0]
        expected_frame = {
            "frame_type": "key",
            "frame_width": WIDTH,
            "frame_height": HEIGHT,
            "all_lossless": True,
        }
        if any(frame.get(key) != value for key, value in expected_frame.items()):
            raise RuntimeError(f"AV1 {role} frame is not an all-lossless 17x9 key frame")


def validate_decoded_yuv(avifdec: Path, path: Path, work: Path) -> None:
    decoded_path = work / "decoded.y4m"
    run([str(avifdec), str(path), str(decoded_path)])
    decoded = decoded_path.read_bytes()
    header, separator, remainder = decoded.partition(b"\n")
    expected_tokens = {
        "YUV4MPEG2",
        f"W{WIDTH}",
        f"H{HEIGHT}",
        "C420p12",
        "XYSCSS=420P12",
        "XCOLORRANGE=FULL",
    }
    if not separator or not expected_tokens.issubset(set(header.decode("ascii").split())):
        raise RuntimeError("avifdec did not preserve the full-range 17x9 12-bit 4:2:0 planes")
    frame_marker = b"FRAME\n"
    if not remainder.startswith(frame_marker):
        raise RuntimeError("avifdec output is missing its one Y4M frame")

    plane_bytes = remainder[len(frame_marker) :]
    plane_counts = (
        WIDTH * HEIGHT,
        CHROMA_WIDTH * CHROMA_HEIGHT,
        CHROMA_WIDTH * CHROMA_HEIGHT,
    )
    if len(plane_bytes) != sum(plane_counts) * 2:
        raise RuntimeError("avifdec returned unexpected 12-bit YUV420 plane lengths")

    planes = []
    offset = 0
    for count in plane_counts:
        byte_count = count * 2
        plane = struct.unpack(f"<{count}H", plane_bytes[offset : offset + byte_count])
        if any(sample > 0xFFF for sample in plane):
            raise RuntimeError("decoded YUV420 sample exceeds the declared 12-bit depth")
        planes.append(plane)
        offset += byte_count

    y_plane, u_plane, v_plane = planes
    if len({sample & 0xF for sample in y_plane}) != 16:
        raise RuntimeError("decoded luma must include every discarded low-nibble value")
    for name, plane in (("U", u_plane), ("V", v_plane)):
        if len(set(plane)) < 40 or len({sample & 0xF for sample in plane}) < 12:
            raise RuntimeError(f"decoded {name} chroma does not retain varied 12-bit samples")
        right_edge = [plane[row * CHROMA_WIDTH + CHROMA_WIDTH - 1] & 0xF
                      for row in range(CHROMA_HEIGHT)]
        bottom_edge = [plane[(CHROMA_HEIGHT - 1) * CHROMA_WIDTH + column] & 0xF
                       for column in range(CHROMA_WIDTH)]
        if 0 in right_edge or len(set(right_edge)) < 3:
            raise RuntimeError(f"decoded {name} odd right chroma edge lacks varied low bits")
        if 0 in bottom_edge or len(set(bottom_edge)) < 4:
            raise RuntimeError(f"decoded {name} odd bottom chroma edge lacks varied low bits")


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
            or image.mode != "RGBA"
            or image.size != (WIDTH, HEIGHT)
            or image.n_frames != 1
            or len(pixels) != WIDTH * HEIGHT * 4
            or sha256(pixels) != PIXELS_SHA256
        ):
            raise RuntimeError("Pillow RGBA pixels differ from the pinned AVIF still reference")


def generate(avifenc: Path, avifdec: Path, check: bool) -> None:
    avifenc = avifenc.expanduser().resolve()
    avifdec = avifdec.expanduser().resolve()
    validate_tool_version(avifenc, "avifenc")
    validate_tool_version(avifdec, "avifdec")

    source = source_png()
    if sha256(source) != SOURCE_SHA256:
        raise RuntimeError("generated 16-bit RGBA PNG differs from its hash pin")

    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".avif-12bit420-alpha-", dir=STAGING_ROOT) as temporary:
        work = Path(temporary)
        source_path = work / "source.png"
        source_path.write_bytes(source)
        encoded_runs = []
        for index in range(2):
            candidate = work / f"candidate-{index}.avif"
            command = [
                str(avifenc),
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
                "420",
                "--cicp",
                "1/13/6",
                "--range",
                "full",
                str(source_path),
                str(candidate),
            ]
            run(command)
            data = candidate.read_bytes()
            if sha256(data) != ASSET_SHA256:
                raise RuntimeError(f"encoded AVIF differs from its hash pin: {sha256(data)}")
            validate_bitstream(candidate)
            validate_decoded_yuv(avifdec, candidate, work)
            validate_pillow(candidate)
            encoded_runs.append(data)

        if encoded_runs[0] != encoded_runs[1]:
            raise RuntimeError("independent odd-edge 12-bit AVIF encodes differ")

        if check:
            if not ASSET_PATH.is_file() or ASSET_PATH.read_bytes() != encoded_runs[0]:
                raise RuntimeError("committed odd-edge AVIF differs from its deterministic encoding")
        else:
            ASSET_PATH.parent.mkdir(parents=True, exist_ok=True)
            ASSET_PATH.write_bytes(encoded_runs[0])

    action = "Verified" if check else "Wrote"
    print(f"{action} {ASSET_PATH} ({len(encoded_runs[0])} bytes, sha256 {ASSET_SHA256})")


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
