#!/usr/bin/env python3
"""Generate the pinned 10-bit lossless inter AVIF sequence fixture.

The Pillow bundled AOM encoder is 8-bit-only. This generator therefore builds
the repository-pinned libaom and libavif sources into a disposable staging
directory, encodes the planar sequence, normalizes BMFF timestamps, and checks
both repeatability and pinned Pillow frame pixels before writing the fixture.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, _avif, features

from generate_av1_sequence_refs import (
    LIBAVIF_COMMIT,
    digest_file,
    git_tree_digest,
    run,
    text_output,
)
from inspect_av1_obus import inspect as inspect_obus
from inspect_avif_bitstreams import (
    children,
    inspect as inspect_container,
    parse_boxes,
    unique_box,
)


ROOT = Path(__file__).resolve().parent.parent
STAGING_ROOT = ROOT / "target" / "oracle-staging"
ASSET = ROOT / "tests" / "fixtures" / "input" / "images" / "avif" / (
    "animated_lossless_inter_420_b32x32_10bit.avif"
)
ASSET_64 = ROOT / "tests" / "fixtures" / "input" / "images" / "avif" / (
    "animated_lossless_inter_420_b32x32_10bit_64x64.avif"
)
ASSET_CLIPPED_B32_28X64 = ROOT / "tests" / "fixtures" / "input" / "images" / "avif" / (
    "animated_lossless_inter_420_clipped_b32x32_10bit_partition32_28x64.avif"
)
ASSET_CLIPPED_B32_56X64 = ROOT / "tests" / "fixtures" / "input" / "images" / "avif" / (
    "animated_lossless_inter_420_clipped_b32x32_10bit_partition32_56x64.avif"
)
ENCODER_SOURCE = ROOT / "scripts" / "avif_fixture_oracle" / (
    "encode_lossless_inter_10bit.c"
)
AOM_COMMIT = "ad44980d7f3c7a2605c25d51ea96946949000841"
FIXTURE_SPECS = {
    "b32x32_10bit": {
        "asset": ASSET,
        "width": 32,
        "height": 32,
        "speed": 8,
        "partition_size": 0,
        "expected_file_sha256": (
            "c90bf9e75b0c091e19ab6b8aa70c17243ae1dbbf818493e815015f998f14e73a"
        ),
        "expected_frame_sha256": (
            "0a9a2996f570e2959cbd25e68991ceb2fbd153485af10d4ebfb2fed8fe3170a7",
            "a9346f58b101a9c058ad8271404b371773b0fea8fc3255c8c8befcf7104c4345",
        ),
    },
    "b32x32_10bit_64x64": {
        "asset": ASSET_64,
        "width": 64,
        "height": 64,
        "speed": 0,
        "partition_size": 32,
        "expected_file_sha256": (
            "62a154bb3e8a7c92816045d58f492aaa30c3aa215526f6c3d7433de942fb2448"
        ),
        "expected_frame_sha256": (
            "354596f7f9f9d319cce258fff9962cee39322555578143c69c076eba97711ed1",
            "0db725dc882996aaea0f9d84e8c9094d1ce25bc99e39ff8f38d39c452c1e24d7",
        ),
    },
    "clipped_b32x32_10bit_partition32_28x64": {
        "asset": ASSET_CLIPPED_B32_28X64,
        "width": 28,
        "height": 64,
        "speed": 0,
        "partition_size": 32,
        "patch_x": 2,
        "patch_y": 4,
        "patch_width": 24,
        "patch_height": 24,
        "patch_pattern": "texture",
        "expected_file_sha256": (
            "8f2b787faea151faf9c7f69c2b92e098dedd3bb70f28fd33546194c002b3456f"
        ),
        "expected_frame_sha256": (
            "3dc1cdc564be835c5afbd6cac356fcda1b5f8c7b58be38b18331a5b1deec0f9e",
            "15b9ec6a29475ae276ce6c118088943f352ac2b383f83218a0ce133cbd0ac420",
        ),
    },
    "clipped_b32x32_10bit_partition32_56x64": {
        "asset": ASSET_CLIPPED_B32_56X64,
        "width": 56,
        "height": 64,
        "speed": 0,
        "partition_size": 32,
        "patch_x": 34,
        "patch_y": 4,
        "patch_width": 16,
        "patch_height": 24,
        "patch_pattern": "texture",
        "expected_file_sha256": (
            "a819711996c804d7c5e4dcc26cee2e6a35bb82c48a88a6db8e34bd61d3b5f9fe"
        ),
        "expected_frame_sha256": (
            "8d14c698d8415e0e22c67a43b245c4a645cecd3e048ec1e8dcbf0bba1f112100",
            "3b24a0b82c7f7572d0fbf148d3eb205fdc251650f8d73258fbf16e4b371ef5b6",
        ),
    },
}
PINNED_PILLOW = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
AOM_OPTIONS = (
    "-DCMAKE_BUILD_TYPE=Release",
    "-DAOM_TARGET_CPU=generic",
    "-DENABLE_TESTS=OFF",
    "-DENABLE_DOCS=OFF",
    "-DENABLE_TOOLS=OFF",
    "-DENABLE_EXAMPLES=OFF",
    "-DCONFIG_AV1_DECODER=0",
    "-DCONFIG_MULTITHREAD=0",
)
AVIF_OPTIONS = (
    "-DCMAKE_BUILD_TYPE=Release",
    "-DBUILD_SHARED_LIBS=OFF",
    "-DAVIF_CODEC_AOM=SYSTEM",
    "-DAVIF_CODEC_AOM_DECODE=OFF",
    "-DAVIF_CODEC_DAV1D=OFF",
    "-DAVIF_LIBYUV=OFF",
    "-DAVIF_BUILD_APPS=OFF",
    "-DAVIF_BUILD_TESTS=OFF",
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def resolve_tool(value: str, label: str) -> str:
    resolved = shutil.which(value)
    if resolved is None:
        raise RuntimeError(f"{label} executable not found: {value}")
    return str(Path(resolved).resolve())


def validate_source(
    source: Path,
    *,
    commit: str,
    name: str,
    license_files: tuple[str, ...],
) -> dict[str, object]:
    if not source.is_dir():
        raise RuntimeError(f"{name} source directory does not exist: {source}")
    revision = text_output(run(["git", "-C", str(source), "rev-parse", "HEAD"]))
    if revision != commit:
        raise RuntimeError(f"{name} source must be at pinned commit {commit}")
    if run(
        ["git", "-C", str(source), "status", "--porcelain", "--untracked-files=all"]
    ).stdout:
        raise RuntimeError(f"{name} source checkout must be clean")

    files = []
    for filename in license_files:
        local_license = source / filename
        retained_license = ROOT / "third_party" / name / filename
        if digest_file(local_license) != digest_file(retained_license):
            raise RuntimeError(f"retained {name} {filename} differs from pinned source")
        files.append({"path": filename, "sha256": digest_file(local_license)})
    return {
        "commit": revision,
        "tree_sha256": git_tree_digest(source),
        "licenses": files,
    }


def static_library(directory: Path, name: str) -> Path:
    matches = sorted(directory.rglob(name))
    if len(matches) != 1:
        raise RuntimeError(f"expected one {name} below {directory}, found {len(matches)}")
    return matches[0]


def build_encoder(
    *,
    work: Path,
    aom_source: Path,
    libavif_source: Path,
    cmake: str,
    ninja: str,
    cc: str,
    jobs: int,
) -> Path:
    aom_build = work / "aom-build"
    prefix = work / "prefix"
    run(
        [
            cmake,
            "-S",
            str(aom_source),
            "-B",
            str(aom_build),
            "-G",
            "Ninja",
            f"-DCMAKE_MAKE_PROGRAM={ninja}",
            *AOM_OPTIONS,
            f"-DCMAKE_C_COMPILER={cc}",
            f"-DCMAKE_INSTALL_PREFIX={prefix}",
        ]
    )
    run([cmake, "--build", str(aom_build), "-j", str(jobs)])
    run([cmake, "--install", str(aom_build)])
    aom_library = static_library(prefix, "libaom.a")

    avif_build = work / "libavif-build"
    run(
        [
            cmake,
            "-S",
            str(libavif_source),
            "-B",
            str(avif_build),
            "-G",
            "Ninja",
            f"-DCMAKE_MAKE_PROGRAM={ninja}",
            *AVIF_OPTIONS,
            f"-DCMAKE_C_COMPILER={cc}",
            f"-DCMAKE_PREFIX_PATH={prefix}",
            f"-DAOM_LIBRARY={aom_library}",
            f"-DAOM_INCLUDE_DIR={aom_source}",
        ]
    )
    run([cmake, "--build", str(avif_build), "-j", str(jobs)])
    libavif_library = static_library(avif_build, "libavif.a")

    encoder = work / "encode-lossless-inter-10bit"
    run(
        [
            cc,
            "-std=c99",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            f"-I{libavif_source / 'include'}",
            str(ENCODER_SOURCE),
            str(libavif_library),
            str(aom_library),
            "-lm",
            "-o",
            str(encoder),
        ]
    )
    return encoder


def sequence_timestamp_fields(data: bytes) -> list[tuple[bytes, int, int]]:
    root_boxes = parse_boxes(data, 0, len(data))
    movie = unique_box(root_boxes, b"moov")
    if movie is None:
        raise RuntimeError("sequence fixture has no movie box")
    movie_children = children(data, movie)
    timestamp_boxes = [(b"mvhd", unique_box(movie_children, b"mvhd"))]
    for track in (box for box in movie_children if box.kind == b"trak"):
        track_children = children(data, track)
        media = unique_box(track_children, b"mdia")
        if media is None:
            raise RuntimeError("sequence track has no media box")
        timestamp_boxes.extend(
            (
                (b"tkhd", unique_box(track_children, b"tkhd")),
                (b"mdhd", unique_box(children(data, media), b"mdhd")),
            )
        )
    if len(timestamp_boxes) != 3 or any(box is None for _, box in timestamp_boxes):
        raise RuntimeError("fixture must have exactly one movie and one track timestamp set")

    fields = []
    for box_kind, box in timestamp_boxes:
        if box is None:
            raise RuntimeError(f"fixture lacks {box_kind!r}")
        if box.payload_start >= box.end or data[box.payload_start] != 1:
            raise RuntimeError(f"fixture lacks version-one {box_kind!r}")
        start = box.payload_start + 4
        end = start + 16
        if end > box.end:
            raise RuntimeError(f"truncated creation/modification fields in {box_kind!r}")
        fields.append((box_kind, start, end))
    return fields


def normalize_sequence_timestamps(data: bytes) -> bytes:
    """Zero only version-one creation and modification timestamps in movie boxes."""
    encoded = bytearray(data)
    for _, start, end in sequence_timestamp_fields(data):
        encoded[start:end] = bytes(end - start)
    return bytes(encoded)


def validate_timestamp_repeatability(first: bytes, second: bytes) -> None:
    if len(first) != len(second):
        raise RuntimeError("native AVIF generation runs have different file lengths")
    fields = sequence_timestamp_fields(first)
    differing_offsets = [
        index for index, (left, right) in enumerate(zip(first, second)) if left != right
    ]
    outside_timestamps = [
        offset
        for offset in differing_offsets
        if not any(start <= offset < end for _, start, end in fields)
    ]
    if outside_timestamps:
        raise RuntimeError(
            "native AVIF runs differ outside creation/modification timestamp fields: "
            + ", ".join(str(offset) for offset in outside_timestamps[:8])
        )


def validate_bitstream(path: Path, width: int, height: int) -> list[tuple[int, str]]:
    container = inspect_container(path)
    if container["ftyp"]["major"] != "avis":
        raise RuntimeError("fixture does not declare an AVIF sequence")
    tracks = [track for track in container["tracks"] if track["handler"] == "pict"]
    if len(tracks) != 1 or len(tracks[0]["samples"]) != 2:
        raise RuntimeError("fixture must contain one two-sample picture track")
    samples = tracks[0]["samples"]
    if samples[0]["sync"] is not True or samples[1]["sync"] is not False:
        raise RuntimeError("fixture must contain a key sample followed by an inter sample")

    report = inspect_obus(path)
    picture_samples = [
        sample
        for sample in report["samples"]
        if sample["role"] == "track_pict"
        and sample["identity"]["track_id"] == tracks[0]["track_id"]
    ]
    picture_samples.sort(key=lambda sample: sample["identity"]["sample"])
    if [sample["identity"]["sample"] for sample in picture_samples] != [0, 1]:
        raise RuntimeError("AV1 OBU report does not contain exactly two picture samples")

    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    expected_sequence = {
        "bit_depth": 10,
        "max_width": width,
        "max_height": height,
        "monochrome": False,
        "color_primaries": 1,
        "transfer_characteristics": 13,
        "matrix_coefficients": 6,
        "color_range": 1,
        "subsampling_x": 1,
        "subsampling_y": 1,
    }
    if not sequence_headers or any(
        any(header.get(key) != value for key, value in expected_sequence.items())
        for header in sequence_headers
    ):
        raise RuntimeError("sequence header differs from the pinned 10-bit full-range I420 shape")

    expected_types = ("key", "inter")
    obu_sample_hashes = []
    for sample, expected_type in zip(picture_samples, expected_types, strict=True):
        headers = [obu["frame_header"] for obu in sample["obus"] if "frame_header" in obu]
        if len(headers) != 1:
            raise RuntimeError("each picture sample must contain one AV1 frame header")
        header = headers[0]
        if (
            header.get("frame_type") != expected_type
            or header.get("all_lossless") is not True
            or header.get("quantization", {}).get("base") != 0
            or header.get("frame_width") != width
            or header.get("frame_height") != height
        ):
            raise RuntimeError("picture frame differs from the key/inter all-lossless fixture contract")
        obu_sample_hashes.append(
            (int(sample["identity"]["sample"]), str(sample["sha256"]))
        )
    return obu_sample_hashes


def pillow_frames(
    path: Path, width: int, height: int, expected_frame_sha256: tuple[str, str]
) -> list[dict[str, object]]:
    if (Image.__version__, features.version("avif"), _avif.codec_versions()) != PINNED_PILLOW:
        raise RuntimeError("Pillow/libavif/codec versions differ from fixture pins")
    frames = []
    with Image.open(path) as image:
        if (
            image.size != (width, height)
            or image.mode != "RGB"
            or image.n_frames != 2
        ):
            raise RuntimeError(
                f"Pillow did not decode the fixture as two {width}x{height} RGB frames"
            )
        for index, expected_hash in enumerate(expected_frame_sha256):
            image.seek(index)
            image.load()
            pixels = image.tobytes()
            duration = image.info.get("duration")
            digest = sha256(pixels)
            if (
                len(pixels) != width * height * 3
                or digest != expected_hash
                or duration != 100
            ):
                raise RuntimeError(f"Pillow frame {index} differs from its pinned reference")
            frames.append(
                {
                    "index": index,
                    "mode": image.mode,
                    "size": list(image.size),
                    "duration_ms": duration,
                    "bytes": len(pixels),
                    "sha256": digest,
                }
            )
    return frames


def generate(args: argparse.Namespace) -> None:
    aom_source = args.aom_source.resolve()
    libavif_source = args.libavif_source.resolve()
    aom_identity = validate_source(
        aom_source,
        commit=AOM_COMMIT,
        name="libaom",
        license_files=("LICENSE", "PATENTS"),
    )
    libavif_identity = validate_source(
        libavif_source,
        commit=LIBAVIF_COMMIT,
        name="libavif",
        license_files=("LICENSE",),
    )
    tools = {
        "cmake": resolve_tool(args.cmake, "CMake"),
        "ninja": resolve_tool(args.ninja, "Ninja"),
        "cc": resolve_tool(args.cc, "C compiler"),
    }

    selected_specs = {
        name: spec
        for name, spec in FIXTURE_SPECS.items()
        if args.fixture is None or name == args.fixture
    }
    if args.output is not None and len(selected_specs) != 1:
        raise RuntimeError("--output requires selecting exactly one fixture with --fixture")

    STAGING_ROOT.mkdir(parents=True, exist_ok=True)
    asset_reports = []
    with tempfile.TemporaryDirectory(
        prefix=".avif-hi10-lossless-", dir=STAGING_ROOT
    ) as temporary:
        work = Path(temporary)
        encoder = build_encoder(
            work=work,
            aom_source=aom_source,
            libavif_source=libavif_source,
            cmake=tools["cmake"],
            ninja=tools["ninja"],
            cc=tools["cc"],
            jobs=args.jobs,
        )
        for fixture_name, spec in selected_specs.items():
            generated = []
            observations = []
            obu_sample_hashes = []
            raw_runs = []
            for index in range(2):
                raw_path = work / f"{fixture_name}-run-{index}.avif"
                normalized_path = work / f"{fixture_name}-run-{index}-normalized.avif"
                command = [str(encoder), str(raw_path)]
                if (
                    spec["width"],
                    spec["height"],
                    spec["speed"],
                    spec["partition_size"],
                ) != (32, 32, 8, 0):
                    if spec["width"] == spec["height"]:
                        command.extend(
                            [
                                str(spec["width"]),
                                str(spec["speed"]),
                                str(spec["partition_size"]),
                            ]
                        )
                    elif "patch_width" in spec:
                        command.extend(
                            [
                                str(spec["width"]),
                                str(spec["height"]),
                                str(spec["speed"]),
                                str(spec["partition_size"]),
                                str(spec["patch_x"]),
                                str(spec["patch_y"]),
                                str(spec["patch_width"]),
                                str(spec["patch_height"]),
                                str(spec["patch_pattern"]),
                            ]
                        )
                    else:
                        command.extend(
                            [
                                str(spec["width"]),
                                str(spec["height"]),
                                str(spec["speed"]),
                                str(spec["partition_size"]),
                            ]
                        )
                run(command)
                raw = raw_path.read_bytes()
                normalized = normalize_sequence_timestamps(raw)
                normalized_path.write_bytes(normalized)
                obu_sample_hashes.append(
                    validate_bitstream(normalized_path, spec["width"], spec["height"])
                )
                observations.append(
                    pillow_frames(
                        normalized_path,
                        spec["width"],
                        spec["height"],
                        spec["expected_frame_sha256"],
                    )
                )
                generated.append(normalized)
                raw_runs.append(raw)

            validate_timestamp_repeatability(raw_runs[0], raw_runs[1])
            if generated[0] != generated[1]:
                raise RuntimeError(
                    f"independent {fixture_name} AVIF runs differ after timestamp normalization"
                )
            if obu_sample_hashes[0] != obu_sample_hashes[1]:
                raise RuntimeError(f"{fixture_name} AV1 OBU sample hashes differ between runs")
            if observations[0] != observations[1]:
                raise RuntimeError(f"{fixture_name} Pillow frame observations differ between runs")
            asset_hash = sha256(generated[0])
            if asset_hash != spec["expected_file_sha256"]:
                raise RuntimeError(
                    f"{fixture_name} normalized fixture hash differs from pin: {asset_hash}"
                )

            output = (
                args.output.resolve()
                if args.output is not None
                else spec["asset"].resolve()
            )
            if output != spec["asset"].resolve() and STAGING_ROOT.resolve() not in output.parents:
                raise RuntimeError("output must be the fixture path or a staging path")
            output.parent.mkdir(parents=True, exist_ok=True)
            temporary_output = output.with_name(f".{output.name}.tmp")
            try:
                temporary_output.write_bytes(generated[0])
                temporary_output.replace(output)
            finally:
                temporary_output.unlink(missing_ok=True)

            asset_reports.append(
                {
                    "fixture": fixture_name,
                    "asset": (
                        str(output.relative_to(ROOT))
                        if output.is_relative_to(ROOT)
                        else "staging"
                    ),
                    "asset_bytes": len(generated[0]),
                    "asset_sha256": asset_hash,
                    "timestamp_normalized_repeat_equal": True,
                    "timestamp_fields": [
                        {
                            "box": kind.decode("ascii"),
                            "offsets": [start, end],
                        }
                        for kind, start, end in sequence_timestamp_fields(raw_runs[0])
                    ],
                    "obu_sample_hashes": [
                        {"sample": sample, "sha256": digest}
                        for sample, digest in obu_sample_hashes[0]
                    ],
                    "frames": observations[0],
                }
            )

    print(
        json.dumps(
            {
                "assets": asset_reports,
                "sources": {"libaom": aom_identity, "libavif": libavif_identity},
                "build": {
                    "cmake": tools["cmake"],
                    "ninja": tools["ninja"],
                    "cc": tools["cc"],
                    "aom_options": AOM_OPTIONS,
                    "libavif_options": AVIF_OPTIONS,
                },
                "encoder_source_sha256": digest_file(ENCODER_SOURCE),
            },
            indent=2,
            sort_keys=True,
        )
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--aom-source", type=Path, required=True)
    parser.add_argument("--libavif-source", type=Path, required=True)
    parser.add_argument("--cmake", default="cmake")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--cc", default=os.environ.get("CC", "cc"))
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--fixture", choices=sorted(FIXTURE_SPECS))
    parser.add_argument(
        "--output",
        type=Path,
        help="Optional staging output for exactly one selected --fixture",
    )
    args = parser.parse_args()
    if not 1 <= args.jobs <= 64:
        parser.error("--jobs must be in the range 1..64")
    try:
        generate(args)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
