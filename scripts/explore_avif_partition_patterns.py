#!/usr/bin/env python3
"""Reverse-map patterned AVIF partition trees through pinned scalar dav1d.

This diagnostic tool generates a fixed set of small RGB boundary patterns,
encodes each one twice with Pillow's pinned libavif/libaom stack, extracts the
AV1 color item independently of the Rust implementation, and records the full
scalar dav1d partition and entropy trace. Generated images remain temporary;
only the requested JSON report is retained.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
from io import BytesIO
from pathlib import Path
from typing import Callable

from PIL import Image, _avif, features

from explore_avif_constant_corpus import parse_color, parse_size, partition_trace
from generate_av1_reconstruction_refs import (
    build_dav1d,
    extract_color_item,
    resolve_tool,
    run,
    verify_source,
)


Color = tuple[int, int, int]
Pattern = Callable[[int, int, int, int], Color]
AdvancedOptions = dict[str, str]

A = (17, 91, 203)
GRAY_32 = (32, 32, 32)
GREEN = (0, 255, 0)
GRAY_127 = (127, 127, 127)


def solid_a(_x: int, _y: int, _width: int, _height: int) -> Color:
    return A


def vertical_halves(x: int, _y: int, width: int, _height: int) -> Color:
    return A if x < width // 2 else GREEN


def horizontal_halves(_x: int, y: int, _width: int, height: int) -> Color:
    return A if y < height // 2 else GREEN


def four_quadrants(x: int, y: int, width: int, height: int) -> Color:
    index = int(y >= height // 2) * 2 + int(x >= width // 2)
    return (A, GRAY_32, GREEN, GRAY_127)[index]


def changed_bottom_right(x: int, y: int, width: int, height: int) -> Color:
    return GREEN if x >= width // 2 and y >= height // 2 else A


def changed_bottom_right_color(
    color: Color,
    origin: tuple[int, int] | None = None,
) -> Pattern:
    def pattern(x: int, y: int, width: int, height: int) -> Color:
        boundary_x, boundary_y = origin or (width // 2, height // 2)
        return color if x >= boundary_x and y >= boundary_y else A

    return pattern


def changed_top_left(x: int, y: int, width: int, height: int) -> Color:
    return GREEN if x < width // 2 and y < height // 2 else A


def changed_top_right(x: int, y: int, width: int, height: int) -> Color:
    return GREEN if x >= width // 2 and y < height // 2 else A


def changed_bottom_left(x: int, y: int, width: int, height: int) -> Color:
    return GREEN if x < width // 2 and y >= height // 2 else A


def diagonal_quadrants(x: int, y: int, width: int, height: int) -> Color:
    right = x >= width // 2
    bottom = y >= height // 2
    return A if right == bottom else GREEN


def checker(block: int) -> Pattern:
    def pattern(x: int, y: int, _width: int, _height: int) -> Color:
        return A if (x // block + y // block) % 2 == 0 else GREEN

    return pattern


def vertical_bands(block: int) -> Pattern:
    """Return a high-contrast pattern with independent vertical bands."""

    def pattern(x: int, _y: int, _width: int, _height: int) -> Color:
        return A if (x // block) % 2 == 0 else GREEN

    return pattern


def horizontal_bands(block: int) -> Pattern:
    """Return a high-contrast pattern with independent horizontal bands."""

    def pattern(_x: int, y: int, _width: int, _height: int) -> Color:
        return A if (y // block) % 2 == 0 else GREEN

    return pattern


def vertical_ramp(x: int, _y: int, _width: int, _height: int) -> Color:
    """Return four distinct vertical color bands."""

    return (A, GRAY_32, GREEN, GRAY_127)[min(3, x // 4)]


def horizontal_ramp(_x: int, y: int, _width: int, _height: int) -> Color:
    """Return four distinct horizontal color bands."""

    return (A, GRAY_32, GREEN, GRAY_127)[min(3, y // 4)]


def vertical_bands_ripple(x: int, y: int, _width: int, _height: int) -> Color:
    """Return vertical bands whose luma also varies along each band."""

    base = (32, 88, 160, 224)[min(3, x // 4)]
    ripple = ((11 * x + 17 * y) % 29) - 14
    return (
        max(0, min(255, base + ripple + 18)),
        max(0, min(255, base + ripple)),
        max(0, min(255, base + ripple - 18)),
    )


def vertical_checker(x: int, y: int, _width: int, _height: int) -> Color:
    """Return four vertical bands with different checker frequencies."""

    band = min(3, x // 4)
    phase = ((x + band * 3) // 2 + y * (band + 1)) % 4
    base = (24, 88, 152, 216)[band]
    return tuple(
        max(0, min(255, base + phase * step - 18))
        for step in (1, 3, 5)
    )


def changed_sample(x: int, y: int, width: int, height: int) -> Color:
    return GREEN if x == width - 1 and y == height - 1 else A


PATTERNS: tuple[tuple[str, Pattern, AdvancedOptions], ...] = (
    (
        "partition_bounds_8",
        solid_a,
        {"min-partition-size": "8", "max-partition-size": "8"},
    ),
    ("vertical_halves", vertical_halves, {}),
    ("horizontal_halves", horizontal_halves, {}),
    ("four_quadrants", four_quadrants, {}),
    ("changed_top_left", changed_top_left, {}),
    ("changed_top_right", changed_top_right, {}),
    ("changed_bottom_left", changed_bottom_left, {}),
    ("changed_bottom_right", changed_bottom_right, {}),
    ("diagonal_quadrants", diagonal_quadrants, {}),
    ("checker_8", checker(8), {}),
    ("checker_4", checker(4), {}),
    ("vertical_bands_2", vertical_bands(2), {}),
    ("vertical_bands_4", vertical_bands(4), {}),
    ("horizontal_bands_2", horizontal_bands(2), {}),
    ("horizontal_bands_4", horizontal_bands(4), {}),
    ("vertical_ramp", vertical_ramp, {}),
    ("horizontal_ramp", horizontal_ramp, {}),
    ("vertical_bands_ripple", vertical_bands_ripple, {}),
    ("vertical_checker", vertical_checker, {}),
    ("changed_sample", changed_sample, {}),
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encode_pattern(
    size: tuple[int, int],
    pattern: Pattern,
    quality: int,
    speed: int,
    advanced: AdvancedOptions,
    subsampling: str,
) -> bytes:
    width, height = size
    pixels = [
        component
        for y in range(height)
        for x in range(width)
        for component in pattern(x, y, width, height)
    ]
    image = Image.frombytes("RGB", size, bytes(pixels))
    output = BytesIO()
    image.save(
        output,
        format="AVIF",
        quality=quality,
        speed=speed,
        max_threads=1,
        subsampling=subsampling,
        autotiling=False,
        advanced=advanced,
    )
    return output.getvalue()


def row_bytes(data: bytes, width: int, channels: int) -> list[str]:
    stride = width * channels
    return [
        data[offset : offset + stride].hex()
        for offset in range(0, len(data), stride)
    ]


def decode_case(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    name: str,
    pattern: Pattern,
    size: tuple[int, int],
    quality: int,
    speed: int,
    advanced: AdvancedOptions,
    subsampling: str,
    retain_dir: Path | None,
) -> dict[str, object]:
    encoded = encode_pattern(size, pattern, quality, speed, advanced, subsampling)
    if encoded != encode_pattern(
        size, pattern, quality, speed, advanced, subsampling
    ):
        raise RuntimeError(f"nondeterministic AVIF encoding for {name}")
    path = work / f"{name}_{size[0]}x{size[1]}.avif"
    path.write_bytes(encoded)
    if retain_dir is not None:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path.name).write_bytes(encoded)
    sample, _ = extract_color_item(path)
    sample_path = path.with_suffix(".obu")
    output_path = path.with_suffix(".yuv")
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
    yuv = output_path.read_bytes()
    if subsampling == "4:2:0":
        chroma_size = ((size[0] + 1) // 2, (size[1] + 1) // 2)
    elif subsampling == "4:2:2":
        chroma_size = ((size[0] + 1) // 2, size[1])
    else:
        chroma_size = size
    plane_sizes = (size, chroma_size, chroma_size)
    plane_lengths = [width * height for width, height in plane_sizes]
    if len(yuv) != sum(plane_lengths):
        raise RuntimeError(f"unexpected decoded YUV length for {name}: {len(yuv)}")
    debug_log, events, entropy_operations, blocks = partition_trace(result.stdout)
    with Image.open(path) as image:
        image.load()
        if image.mode != "RGB" or image.size != size:
            raise RuntimeError(f"unexpected Pillow result for {name}")
        pillow = image.tobytes()
    planes = []
    offset = 0
    for plane_length in plane_lengths:
        planes.append(yuv[offset : offset + plane_length])
        offset += plane_length
    return {
        "name": name,
        "advanced": advanced,
        "file_length": len(encoded),
        "file_sha256": sha256(encoded),
        "sample_length": len(sample),
        "sample_sha256": sha256(sample),
        "partition_blocks": blocks,
        "entropy_operations": entropy_operations,
        "decoder_events": events,
        "dav1d_debug_log": debug_log,
        "decoded_planes": [
            {
                "sha256": sha256(plane),
                "row_bytes": row_bytes(plane, plane_sizes[index][0], 1),
            }
            for index, plane in enumerate(planes)
        ],
        "pillow": {
            "sha256": sha256(pillow),
            "row_bytes": row_bytes(pillow, size[0], 3),
        },
    }


def summarize_case(case: dict[str, object]) -> dict[str, object]:
    operations = case.pop("entropy_operations")
    if not isinstance(operations, list):
        raise RuntimeError("unexpected entropy-operation report")
    case["entropy_operation_count"] = len(operations)
    events = case.pop("decoder_events")
    if not isinstance(events, list):
        raise RuntimeError("unexpected decoder-event report")
    state_prefixes = (
        "poc=",
        "Post-skip",
        "Post-ymode",
        "Post-uvmode",
        "Post-y-cf-blk",
        "Post-uv-cf-blk",
        "Post-eob",
        "Post-lo_tok",
        "Post-hi_tok",
        "Post-dc_lo_tok",
        "Post-dc_hi_tok",
        "Post-dc_sign",
        "Post-sign",
        "Post-residual",
    )
    case["syntax_states"] = [
        line
        for event in events
        if isinstance(event, dict)
        and event.get("kind") == "debug"
        and isinstance((line := event.get("line")), str)
        and line.startswith(state_prefixes)
    ]
    case.pop("dav1d_debug_log")
    decoded_planes = case.get("decoded_planes")
    if not isinstance(decoded_planes, list):
        raise RuntimeError("unexpected decoded-plane report")
    for plane in decoded_planes:
        if isinstance(plane, dict):
            plane.pop("row_bytes", None)
    pillow = case.get("pillow")
    if isinstance(pillow, dict):
        pillow.pop("row_bytes", None)
    return case


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument("--dav1d-source", type=Path)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--python-path", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--size", type=parse_size, default=(16, 16))
    parser.add_argument(
        "--retain-dir",
        type=Path,
        help="Retain generated AVIF candidates in this diagnostic directory",
    )
    parser.add_argument("--quality", type=int, default=100)
    parser.add_argument("--speed", type=int, default=8)
    parser.add_argument(
        "--subsampling",
        choices=("4:4:4", "4:2:2", "4:2:0"),
        default="4:4:4",
        help="Chroma subsampling passed to the pinned Pillow AVIF encoder",
    )
    parser.add_argument(
        "--advanced",
        action="append",
        default=[],
        metavar="KEY=VALUE",
        help="Additional libaom advanced option applied to every candidate",
    )
    parser.add_argument(
        "--bottom-right-color",
        type=parse_color,
        action="append",
        help="Add a bottom-right-quadrant candidate color; repeat for a corpus",
    )
    parser.add_argument(
        "--bottom-right-origin",
        type=parse_size,
        action="append",
        help="Add an XxY origin instead of the midpoint; repeat for a sweep",
    )
    parser.add_argument(
        "--pattern",
        action="append",
        help="Retain only this named pattern; repeat to compare selected cases",
    )
    parser.add_argument(
        "--only-bottom-right-candidates",
        action="store_true",
        help="Skip built-ins and retain only added color/origin candidates",
    )
    parser.add_argument(
        "--summary-only",
        action="store_true",
        help="Retain hashes, topology, and operation counts without full traces",
    )
    args = parser.parse_args()

    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")
    if not 0 <= args.quality <= 100:
        raise RuntimeError("quality must be in 0..100")
    if not 0 <= args.speed <= 10:
        raise RuntimeError("speed must be in 0..10")
    advanced_options = {}
    for option in args.advanced:
        key, separator, value = option.partition("=")
        if not separator or not key or not value:
            raise RuntimeError(f"advanced option must be KEY=VALUE: {option!r}")
        advanced_options[key] = value
    for boundary_x, boundary_y in args.bottom_right_origin or ():
        if boundary_x >= args.size[0] or boundary_y >= args.size[1]:
            raise RuntimeError("bottom-right origin must be inside the declared size")

    with tempfile.TemporaryDirectory(prefix="image-star-avif-patterns-") as name:
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
            environment = dict(os.environ)
        version_result = run([str(executable), "--version"], env=environment)
        version = (version_result.stdout + version_result.stderr).strip()
        if not version.startswith("1.5.3-0-gb546257"):
            raise RuntimeError(f"unexpected dav1d executable version: {version}")
        patterns = [] if args.only_bottom_right_candidates else list(PATTERNS)
        if advanced_options:
            patterns = [
                (name, pattern, {**advanced_options, **advanced})
                for name, pattern, advanced in patterns
            ]
        if args.only_bottom_right_candidates and not args.bottom_right_color:
            raise RuntimeError(
                "only-bottom-right-candidates requires a bottom-right color"
            )
        origins = args.bottom_right_origin or [None]
        for color in args.bottom_right_color or ():
            for origin in origins:
                pattern_name = f"changed_bottom_right_{color[0]}_{color[1]}_{color[2]}"
                if origin is not None:
                    pattern_name += f"_at_{origin[0]}_{origin[1]}"
                patterns.append(
                    (
                        pattern_name,
                        changed_bottom_right_color(color, origin),
                        {},
                    )
                )
        if args.pattern:
            requested = set(args.pattern)
            available = {name for name, _pattern, _advanced in patterns}
            unknown = requested - available
            if unknown:
                choices = ", ".join(sorted(available))
                names = ", ".join(sorted(unknown))
                raise RuntimeError(
                    f"unknown pattern name(s) {names}; available patterns: {choices}"
                )
            patterns = [
                entry for entry in patterns if entry[0] in requested
            ]
        cases = [
            decode_case(
                executable,
                environment,
                work,
                pattern_name,
                pattern,
                args.size,
                args.quality,
                args.speed,
                advanced,
                args.subsampling,
                args.retain_dir,
            )
            for pattern_name, pattern, advanced in patterns
        ]
        if args.summary_only:
            cases = [summarize_case(case) for case in cases]

    report = {
        "oracle": {
            "pillow": Image.__version__,
            "libavif": features.version("avif"),
            "codecs": codecs,
            "dav1d": version,
        },
        "encoding": {
            "size": list(args.size),
            "quality": args.quality,
            "speed": args.speed,
            "max_threads": 1,
            "subsampling": args.subsampling,
            "autotiling": False,
        },
        "cases": cases,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(cases)} deterministic pattern traces: {args.output}")


if __name__ == "__main__":
    main()
