#!/usr/bin/env python3
"""Generate the pinned lossless I420 sequence with AV1 qmatrix level 15.

The fixture uses a two-frame 16x64 8-bit I420 sequence. Both frames are
qindex-zero and lossless; qmatrix syntax is enabled and fixed at level 15.
Supply an ``avifenc`` built from the repository-pinned libavif and libaom
sources. The generator checks repeatability, AV1 syntax, and Pillow frame
pixels before writing the fixture.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, _avif, features

from generate_avif_10bit_lossless_inter_fixture import (
    normalize_sequence_timestamps,
    sequence_timestamp_fields,
    validate_timestamp_repeatability,
)
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
    / "animated_lossless_inter_420_qmatrix_level15_qindex0_16x64.avif"
)
WIDTH = 16
HEIGHT = 64
Y4M_SHA256 = "afa1dadf1f4a40d773ce45dce0d1ecafdd05e7a4b1937bd26a4df05c528ee8b3"
ASSET_SHA256 = "b76924ec8cf9fa01ad27e48dec5990332b089d75460fa7bf37d9fa00108100de"
FRAME_SHA256 = (
    "44c3002ad58c2b063ac9c560c8baf279052e13d007eaeb84fef33c77a81b9d39",
    "e121e6212ae783e1d2a9c83c99f0fe931df1682b045e772cce7f60e693b3f7e5",
)
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def y4m_sequence() -> bytes:
    """Return two deterministic full-range 8-bit planar 4:2:0 frames."""
    output = bytearray(
        b"YUV4MPEG2 W16 H64 F10:1 Ip A1:1 C420jpeg XCOLORRANGE=FULL\n"
    )
    for frame_index in range(2):
        output.extend(b"FRAME\n")
        for plane_width, plane_height, base, step, period, row_divisor, frame_divisor in (
            (WIDTH, HEIGHT, 48, 37, 4, 8, 4),
            (WIDTH // 2, HEIGHT // 2, 80, 41, 3, 4, 2),
            (WIDTH // 2, HEIGHT // 2, 170, -29, 3, 4, 2),
        ):
            for y in range(plane_height):
                for x in range(plane_width):
                    band = (y // row_divisor + frame_index * (x // frame_divisor)) % period
                    output.append(base + band * step)
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
        "100",
        "-s",
        "0",
        "-j",
        "1",
        "--timescale",
        "1000",
        "--duration",
        "100",
        "--repetition-count",
        "infinite",
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


def validate_bitstream(path: Path) -> list[dict[str, object]]:
    container = inspect_container(path)
    if container["ftyp"]["major"] != "avis":
        raise RuntimeError("fixture does not declare an AVIF sequence")
    tracks = [track for track in container["tracks"] if track["handler"] == "pict"]
    if len(tracks) != 1:
        raise RuntimeError("fixture must have exactly one picture track")
    track = tracks[0]
    samples = track["samples"]
    if len(samples) != 2 or track["timescale"] != 1000:
        raise RuntimeError("fixture must contain two samples at a 1000 Hz timescale")
    if [sample["duration"] for sample in samples] != [100, 100]:
        raise RuntimeError("fixture frame durations differ from the 100 ms profile")
    if [sample["sync"] for sample in samples] != [True, False]:
        raise RuntimeError("fixture must contain a key frame followed by an inter frame")

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

    picture_samples = [
        sample
        for sample in report["samples"]
        if sample["role"] == "track_pict"
        and sample["identity"]["track_id"] == track["track_id"]
    ]
    picture_samples.sort(key=lambda sample: sample["identity"]["sample"])
    if len(picture_samples) != 2:
        raise RuntimeError("AV1 OBU report does not contain exactly two picture samples")

    observations = []
    for sample, expected_type in zip(picture_samples, ("key", "inter"), strict=True):
        headers = [obu["frame_header"] for obu in sample["obus"] if "frame_header" in obu]
        if len(headers) != 1:
            raise RuntimeError("each picture sample must contain one AV1 frame header")
        header = headers[0]
        quantization = header["quantization"]
        if (
            header.get("frame_type") != expected_type
            or header.get("all_lossless") is not True
            or header.get("frame_width") != WIDTH
            or header.get("frame_height") != HEIGHT
            or quantization.get("base") != 0
            or quantization.get("using_matrix") is not True
            or tuple(quantization.get(key) for key in ("matrix_y", "matrix_u", "matrix_v"))
            != (15, 15, 15)
            or any(
                quantization.get(key) != 0
                for key in (
                    "y_dc_delta",
                    "u_dc_delta",
                    "u_ac_delta",
                    "v_dc_delta",
                    "v_ac_delta",
                )
            )
        ):
            raise RuntimeError("picture frame differs from the all-lossless qmatrix level-15 profile")
        observations.append(
            {
                "sample": sample["identity"]["sample"],
                "frame_type": expected_type,
                "all_lossless": True,
                "base_qindex": 0,
                "using_matrix": True,
                "matrix_levels": [15, 15, 15],
            }
        )
    return observations


def pillow_frames(path: Path) -> list[dict[str, object]]:
    if (Image.__version__, features.version("avif"), _avif.codec_versions()) != PINNED_PILLOW:
        raise RuntimeError("Pillow/libavif/codec versions differ from fixture pins")
    observations = []
    with Image.open(path) as image:
        if image.size != (WIDTH, HEIGHT) or image.mode != "RGB" or image.n_frames != 2:
            raise RuntimeError("Pillow did not decode the fixture as two 16x64 RGB frames")
        for index, expected_hash in enumerate(FRAME_SHA256):
            image.seek(index)
            image.load()
            pixels = image.tobytes()
            digest = sha256(pixels)
            duration = image.info.get("duration")
            if len(pixels) != WIDTH * HEIGHT * 3 or digest != expected_hash or duration != 100:
                raise RuntimeError(f"Pillow frame {index} differs from its pinned reference")
            observations.append(
                {
                    "index": index,
                    "mode": image.mode,
                    "size": [WIDTH, HEIGHT],
                    "duration_ms": duration,
                    "bytes": len(pixels),
                    "sha256": digest,
                }
            )
    return observations


def check_fixture() -> None:
    source_hash = sha256(y4m_sequence())
    if source_hash != Y4M_SHA256:
        raise RuntimeError(f"generated Y4M source differs from pin: {source_hash}")

    try:
        asset = ASSET.read_bytes()
    except OSError as error:
        raise RuntimeError(f"cannot read pinned AVIF fixture: {ASSET}") from error
    asset_hash = sha256(asset)
    if asset_hash != ASSET_SHA256:
        raise RuntimeError(f"fixture differs from pin: {asset_hash}")

    observations = {
        "asset": str(ASSET.relative_to(ROOT)),
        "asset_bytes": len(asset),
        "asset_sha256": asset_hash,
        "y4m_sha256": source_hash,
        "av1_frames": validate_bitstream(ASSET),
        "pillow_frames": pillow_frames(ASSET),
    }
    print(json.dumps(observations, indent=2, sort_keys=True))


def generate(args: argparse.Namespace) -> None:
    if args.avifenc is None:
        raise RuntimeError("--avifenc is required unless --check is used")
    avifenc = args.avifenc.expanduser().resolve()
    if not avifenc.is_file():
        raise RuntimeError(f"avifenc does not exist: {avifenc}")
    version = run([str(avifenc), "--version"])
    if "Version: 1.4.1" not in version or "aom [enc]:3.13.2" not in version:
        raise RuntimeError("avifenc does not use the pinned libavif 1.4.1/libaom 3.13.2")

    source = y4m_sequence()
    source_hash = sha256(source)
    if source_hash != Y4M_SHA256:
        raise RuntimeError(f"generated Y4M source differs from pin: {source_hash}")

    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".avif-qmatrix-lossless-", dir=STAGING_ROOT) as temp:
        work = Path(temp)
        source_path = work / "qmatrix-level15.y4m"
        source_path.write_bytes(source)
        raw_paths = [work / f"run-{index}.avif" for index in range(2)]
        for path in raw_paths:
            run(encode_command(avifenc, source_path, path))
        raw_bytes = [path.read_bytes() for path in raw_paths]
        validate_timestamp_repeatability(raw_bytes[0], raw_bytes[1])
        generated = [normalize_sequence_timestamps(data) for data in raw_bytes]
        if generated[0] != generated[1]:
            raise RuntimeError("independent AVIF runs differ after timestamp normalization")

        staging_output = work / "fixture.avif"
        staging_output.write_bytes(generated[0])
        obu_observations = validate_bitstream(staging_output)
        frame_observations = pillow_frames(staging_output)
        asset_hash = sha256(generated[0])
        if asset_hash != ASSET_SHA256:
            raise RuntimeError(f"generated AVIF differs from pin: {asset_hash}")

        output = args.output.expanduser().resolve() if args.output else ASSET.resolve()
        if output != ASSET.resolve() and STAGING_ROOT.resolve() not in output.parents:
            raise RuntimeError("output must be the fixture path or a staging path")
        output.parent.mkdir(parents=True, exist_ok=True)
        temporary_output = output.with_name(f".{output.name}.tmp")
        try:
            temporary_output.write_bytes(generated[0])
            temporary_output.replace(output)
        finally:
            temporary_output.unlink(missing_ok=True)

    print(
        json.dumps(
            {
                "asset": str(output.relative_to(ROOT)) if output.is_relative_to(ROOT) else "staging",
                "asset_bytes": len(generated[0]),
                "asset_sha256": asset_hash,
                "y4m_sha256": source_hash,
                "timestamp_normalized_repeat_equal": True,
                "timestamp_fields": [
                    {"box": kind.decode("ascii"), "offsets": [start, end]}
                    for kind, start, end in sequence_timestamp_fields(raw_bytes[0])
                ],
                "av1_frames": obu_observations,
                "pillow_frames": frame_observations,
                "encoder": version.strip(),
            },
            indent=2,
            sort_keys=True,
        )
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--avifenc",
        type=Path,
        help="avifenc built with pinned libavif 1.4.1 and libaom 3.13.2",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="validate the committed fixture without encoding or writing files",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Optional staging output; defaults to the fixture asset",
    )
    args = parser.parse_args()
    try:
        if args.check:
            if args.avifenc is not None or args.output is not None:
                parser.error("--check cannot be combined with --avifenc or --output")
            check_fixture()
        else:
            generate(args)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
