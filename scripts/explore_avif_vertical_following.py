#!/usr/bin/env python3
"""Search a fixed 4:2:0 AVIF corpus for a right-hand Vertical16x32 leaf.

This diagnostic is deliberately bounded and input-driven. It creates exactly
one hundred deterministic 32x32 RGB candidates, encodes each candidate twice
with the pinned Pillow/libavif/libaom stack, and classifies the independent
scalar dav1d trace. Generated AVIFs are temporary unless ``--retain-dir`` is
provided; the JSON report records hashes, trace-derived predicates, and every
rejection reason. No repository Rust code is invoked.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import re
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from generate_av1_reconstruction_refs import (
    DAV1D_COMMIT,
    build_dav1d,
    extract_color_item,
    resolve_tool,
    run,
    verify_source,
)


SIZE = (32, 32)
SUBSAMPLING = "4:2:0"
ADVANCED = {
    "min-partition-size": "16",
    "max-partition-size": "32",
    "use-intra-dct-only": "1",
    "enable-filter-intra": "1",
    "enable-intra-edge-filter": "0",
    "enable-smooth-intra": "0",
    "enable-paeth-intra": "0",
    "enable-directional-intra": "0",
    "enable-cfl-intra": "0",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "loopfilter-control": "0",
    "aq-mode": "0",
    "deltaq-mode": "0",
}
BLOCK_PATTERN = re.compile(
    r"^poc=(?P<poc>-?\d+),y=(?P<y>-?\d+),x=(?P<x>-?\d+),"
    r"bl=(?P<level>\d+),ctx=(?P<context>\d+),bp=(?P<partition>\d+): "
    r"r=(?P<range>\d+)$"
)
FILTER_PATTERN = re.compile(r"^Post-filterintramode\[(?P<mode>\d+)/(?P<angle>\d+)\]")
TX_PATTERN = re.compile(r"^Post-tx\[(?P<tx>\d+)\]")
LUMA_PATTERN = re.compile(
    r"^Post-y-cf-blk\[tx=(?P<tx>\d+),txtp=(?P<txtp>-?\d+),eob=(?P<eob>-?\d+)\]"
)
CHROMA_PATTERN = re.compile(
    r"^Post-uv-cf-blk\[pl=(?P<plane>\d+),tx=(?P<tx>\d+),"
    r"txtp=(?P<txtp>-?\d+),eob=(?P<eob>-?\d+)\]"
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def clamp(value: int) -> int:
    return max(0, min(255, value))


def rgb_noise(seed: int) -> bytes:
    state = random.Random(seed)
    return bytes(state.randrange(256) for _ in range(SIZE[0] * SIZE[1] * 3))


def gray_noise(seed: int) -> bytes:
    state = random.Random(seed)
    return bytes(
        component
        for _ in range(SIZE[0] * SIZE[1])
        for component in (state.randrange(256),) * 3
    )


def left_ramp_right_noise(seed: int) -> bytes:
    right = rgb_noise(seed)
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if x < 16:
                base = 32 + ((7 * y + 3 * x) % 96)
                pixels.extend((clamp(base + 18), base, clamp(base - 18)))
            else:
                offset = (y * 16 + (x - 16)) * 3
                pixels.extend(right[offset : offset + 3])
    return bytes(pixels)


def left_noise_right_grid(seed: int) -> bytes:
    left = rgb_noise(seed)
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if x < 16:
                offset = (y * 32 + x) * 3
                pixels.extend(left[offset : offset + 3])
                continue
            tile_x = (x - 16) // 8
            tile_y = y // 8
            base = 28 + 37 * ((tile_x + 2 * tile_y) % 6)
            ripple = ((11 * x + 17 * y + seed) % 9) - 4
            pixels.extend(
                (
                    clamp(base + ripple + 14),
                    clamp(base + ripple),
                    clamp(base + ripple - 14),
                )
            )
    return bytes(pixels)


def flat_left_right_noise(seed: int, level: int) -> bytes:
    right = rgb_noise(seed)
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if x < 16:
                pixels.extend((level + 20, level, max(0, level - 20)))
            else:
                offset = (y * 16 + (x - 16)) * 3
                pixels.extend(right[offset : offset + 3])
    return bytes(pixels)


def smooth_left_split_right(seed: int) -> bytes:
    state = random.Random(seed)
    regions = [
        bytes(state.randrange(256) for _ in range(16 * 16 * 3)),
        bytes(state.randrange(256) for _ in range(16 * 16 * 3)),
    ]
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if x < 16:
                base = 48 + ((5 * x + 9 * y) % 64)
                pixels.extend((clamp(base + 16), base, clamp(base - 16)))
            else:
                region = 0 if y < 16 else 1
                offset = ((y % 16) * 16 + (x - 16)) * 3
                pixels.extend(regions[region][offset : offset + 3])
    return bytes(pixels)


def smooth_left_grid(seed: int) -> bytes:
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if x < 16:
                base = 64 + ((3 * x + 5 * y + seed) % 48)
                pixels.extend((clamp(base + 12), base, clamp(base - 12)))
            else:
                tile_x = (x - 16) // 8
                tile_y = y // 8
                base = 36 + 29 * ((tile_x + 3 * tile_y + seed) % 7)
                ripple = ((7 * x + 13 * y) % 11) - 5
                pixels.extend(
                    (
                        clamp(base + ripple + 10),
                        clamp(base + ripple),
                        clamp(base + ripple - 10),
                    )
                )
    return bytes(pixels)


def transform_grid_mosaic(index: int) -> bytes:
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if x < 16:
                luma = 80 + ((y * 4 + 7 * (index + 1)) % 128)
            else:
                local_x = x - 16
                quadrant = (local_x // 8) + 2 * (y // 16)
                luma = (40, 100, 180, 232)[quadrant] + ((local_x + y + index) % 5) - 2
            pixels.extend((clamp(luma), 128, 128))
    return bytes(pixels)


def candidates() -> list[dict[str, object]]:
    result: list[dict[str, object]] = []
    for index in range(10):
        result.append(
            {
                "id": f"f01_rgb_noise_{index:02d}",
                "family": "F01_rgb_noise",
                "seed": 211 + index,
                "pixels": rgb_noise(211 + index),
                "quality": 76,
                "speed": 0,
            }
        )
        result.append(
            {
                "id": f"f02_gray_noise_{index:02d}",
                "family": "F02_gray_noise",
                "seed": 311 + index,
                "pixels": gray_noise(311 + index),
                "quality": 76,
                "speed": 0,
            }
        )
        result.append(
            {
                "id": f"f03_ramp_noise_{index:02d}",
                "family": "F03_left_ramp_right_noise",
                "seed": 300 + index,
                "pixels": left_ramp_right_noise(300 + index),
                "quality": 76,
                "speed": 0,
            }
        )
        result.append(
            {
                "id": f"f04_noise_grid_{index:02d}",
                "family": "F04_left_noise_right_grid",
                "seed": 400 + index,
                "pixels": left_noise_right_grid(400 + index),
                "quality": 76,
                "speed": 0,
            }
        )
        result.append(
            {
                "id": f"f05_flat_noise_{index:02d}",
                "family": "F05_flat_left_right_noise",
                "seed": 500 + index,
                "pixels": flat_left_right_noise(500 + index, 32 + 20 * index),
                "quality": 76,
                "speed": 0,
            }
        )
        result.append(
            {
                "id": f"f06_split_noise_{index:02d}",
                "family": "F06_split_right_regions",
                "seed": 600 + index,
                "pixels": smooth_left_split_right(600 + index),
                "quality": 76,
                "speed": 0,
            }
        )
        result.append(
            {
                "id": f"f07_grid_{index:02d}",
                "family": "F07_smooth_left_right_grid",
                "seed": 700 + index,
                "pixels": smooth_left_grid(700 + index),
                "quality": 76,
                "speed": 0,
            }
        )
        result.append(
            {
                "id": f"f08_mosaic_{index:02d}",
                "family": "F08_transform_grid_mosaic",
                "seed": index,
                "pixels": transform_grid_mosaic(index),
                "quality": 76,
                "speed": 0,
            }
        )
    quality_values = (40, 48, 56, 64, 72, 76, 80, 84, 90, 96)
    quality_pixels = rgb_noise(811)
    for index, quality in enumerate(quality_values):
        result.append(
            {
                "id": f"f09_quality_{index:02d}",
                "family": "F09_quality_sweep",
                "seed": 811,
                "pixels": quality_pixels,
                "quality": quality,
                "speed": 0,
            }
        )
    speed_pixels = rgb_noise(811)
    for speed in range(10):
        result.append(
            {
                "id": f"f10_speed_{speed:02d}",
                "family": "F10_speed_sweep",
                "seed": 811,
                "pixels": speed_pixels,
                "quality": 76,
                "speed": speed,
            }
        )
    if len(result) != 100:
        raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
    return result


def encode(pixels: bytes, quality: int, speed: int) -> bytes:
    image = Image.frombytes("RGB", SIZE, pixels)
    output = BytesIO()
    image.save(
        output,
        format="AVIF",
        quality=quality,
        speed=speed,
        max_threads=1,
        subsampling=SUBSAMPLING,
        autotiling=False,
        advanced=ADVANCED,
    )
    return output.getvalue()


def parse_groups(output: str) -> tuple[list[dict[str, int]], list[list[str]], int]:
    lines = [line.rstrip() for line in output.splitlines() if line.strip()]
    entropy = [
        json.loads(line.removeprefix("@MSAC "))
        for line in lines
        if line.startswith("@MSAC ")
    ]
    if not entropy or entropy[0].get("operation") != "init":
        raise RuntimeError("missing scalar MSAC trace")
    if [operation["step"] for operation in entropy] != list(range(len(entropy))):
        raise RuntimeError("non-contiguous scalar MSAC trace")
    debug = [line for line in lines if not line.startswith("@MSAC ")]
    blocks = []
    for line in debug:
        match = BLOCK_PATTERN.fullmatch(line)
        if match is not None:
            blocks.append({name: int(value) for name, value in match.groupdict().items()})
    groups: list[list[str]] = []
    for line in debug:
        if line.startswith("Post-skip["):
            groups.append([])
        if groups:
            groups[-1].append(line)
    if not blocks:
        raise RuntimeError("missing partition trace")
    return blocks, groups, len(entropy)


def right_leaf_predicates(
    blocks: list[dict[str, int]], groups: list[list[str]]
) -> dict[str, object]:
    root = next(
        (block for block in blocks if block["level"] == 2 and block["x"] == 0 and block["y"] == 0),
        None,
    )
    root_vertical = root is not None and root["partition"] == 2
    right = groups[1] if len(groups) >= 2 else []
    filter_modes = []
    transforms = []
    luma_payloads = []
    chroma_payloads = []
    for line in right:
        if match := FILTER_PATTERN.match(line):
            filter_modes.append({"mode": int(match["mode"]), "angle": int(match["angle"])})
        if match := TX_PATTERN.match(line):
            transforms.append(int(match["tx"]))
        if match := LUMA_PATTERN.match(line):
            luma_payloads.append({name: int(value) for name, value in match.groupdict().items()})
        if match := CHROMA_PATTERN.match(line):
            chroma_payloads.append({name: int(value) for name, value in match.groupdict().items()})
    nonempty_luma = any(payload["eob"] >= 0 for payload in luma_payloads)
    split_luma = len(luma_payloads) in (2, 8)
    exact_chroma = len(chroma_payloads) == 2 and {item["plane"] for item in chroma_payloads} == {0, 1}
    predicates = {
        "frame_root_vertical": root_vertical,
        "two_following_leaf_groups": len(groups) == 2,
        "right_filter_intra": any(item["mode"] == 13 for item in filter_modes),
        "right_luma_split": split_luma,
        "right_luma_nonempty": nonempty_luma,
        "right_chroma_8x16_pair": exact_chroma,
    }
    reasons = [name for name, passed in predicates.items() if not passed]
    return {
        "root_partition": root,
        "group_count": len(groups),
        "right_filter_modes": filter_modes,
        "right_transform_symbols": transforms,
        "right_luma_payloads": luma_payloads,
        "right_chroma_payloads": chroma_payloads,
        "predicates": predicates,
        "rejection_reasons": reasons,
        "qualifies": not reasons,
    }


def decode_candidate(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    candidate: dict[str, object],
    retain_dir: Path | None,
) -> dict[str, object]:
    pixels = candidate["pixels"]
    quality = int(candidate["quality"])
    speed = int(candidate["speed"])
    if not isinstance(pixels, bytes):
        raise TypeError("candidate pixels must be bytes")
    encoded = encode(pixels, quality, speed)
    if encoded != encode(pixels, quality, speed):
        raise RuntimeError(f"nondeterministic encoding for {candidate['id']}")
    path = work / f"{candidate['id']}.avif"
    path.write_bytes(encoded)
    if retain_dir is not None:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path.name).write_bytes(encoded)
    sample, _ = extract_color_item(path)
    sample_path = work / f"{candidate['id']}.obu"
    output_path = work / f"{candidate['id']}.yuv"
    sample_path.write_bytes(sample)
    result = run(
        [
            str(executable),
            "--input",
            str(sample_path),
            "--demuxer",
            "section5",
            "--output",
            str(output_path),
            "--muxer",
            "yuv",
            "--threads",
            "1",
            "--framedelay",
            "1",
            "--cpumask",
            "0",
            "--quiet",
        ],
        env=environment,
    )
    blocks, groups, entropy_count = parse_groups(result.stdout)
    classification = right_leaf_predicates(blocks, groups)
    return {
        "id": candidate["id"],
        "family": candidate["family"],
        "seed": candidate["seed"],
        "quality": quality,
        "speed": speed,
        "input_rgb_sha256": sha256(pixels),
        "encoded_file_sha256": sha256(encoded),
        "encoded_item_sha256": sha256(sample),
        "encoded_item_length": len(sample),
        "entropy_operation_count": entropy_count,
        **classification,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument("--dav1d-source", type=Path)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--python-path", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--retain-dir", type=Path)
    args = parser.parse_args()

    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")

    with tempfile.TemporaryDirectory(prefix="image-star-avif-vertical-following-") as name:
        work = Path(name)
        if args.dav1d_source is not None:
            source = args.dav1d_source.resolve()
            verify_source(source)
            executable, environment = build_dav1d(
                source,
                work,
                resolve_tool(args.meson, "Meson"),
                resolve_tool(args.ninja, "Ninja"),
                args.python_path.resolve() if args.python_path else None,
            )
        else:
            executable = args.dav1d.resolve()
            environment = dict()
        version_result = run([str(executable), "--version"], env=environment)
        version = (version_result.stdout + version_result.stderr).strip()
        if not version.startswith("1.5.3-0-gb546257"):
            raise RuntimeError(f"unexpected dav1d executable version: {version}")
        reports = [
            decode_candidate(executable, environment, work, candidate, args.retain_dir)
            for candidate in candidates()
        ]

    report = {
        "format_version": 1,
        "oracle": {
            "pillow": Image.__version__,
            "libavif": features.version("avif"),
            "codecs": codecs,
            "dav1d": version,
            "dav1d_commit": DAV1D_COMMIT,
        },
        "encoding": {
            "size": list(SIZE),
            "subsampling": SUBSAMPLING,
            "max_threads": 1,
            "autotiling": False,
            "advanced": ADVANCED,
        },
        "search": {
            "candidate_count": len(reports),
            "target": "root PARTITION_V with two 16x32 leaves; right filter-intra leaf with split luma and 8x16 U/V pair",
            "families": [
                "F01_rgb_noise",
                "F02_gray_noise",
                "F03_left_ramp_right_noise",
                "F04_left_noise_right_grid",
                "F05_flat_left_right_noise",
                "F06_split_right_regions",
                "F07_smooth_left_right_grid",
                "F08_transform_grid_mosaic",
                "F09_quality_sweep",
                "F10_speed_sweep",
            ],
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "by_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in (
                    "frame_root_vertical",
                    "two_following_leaf_groups",
                    "right_filter_intra",
                    "right_luma_split",
                    "right_luma_nonempty",
                    "right_chroma_8x16_pair",
                )
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic vertical-following traces: {args.output}")


if __name__ == "__main__":
    main()
