#!/usr/bin/env python3
"""Reverse-map deterministic constant AVIFs through pinned scalar dav1d.

This is a diagnostic fixture-selection tool. It encodes a fixed RGB corpus
with Pillow's pinned libavif/libaom stack, extracts the AV1 item without using
the Rust implementation, and records the first-block syntax emitted by an
instrumented scalar dav1d executable. Generated AVIFs remain in a temporary
directory; only the JSON report requested by the caller is retained.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import tempfile
from io import BytesIO
from itertools import product
from pathlib import Path

from PIL import Image, _avif, features

from generate_av1_reconstruction_refs import (
    build_dav1d,
    extract_color_item,
    parse_debug_log,
    resolve_tool,
    run,
    verify_source,
)


DEFAULT_LEVELS = (
    0,
    32,
    64,
    96,
    112,
    120,
    124,
    126,
    127,
    128,
    129,
    132,
    160,
    192,
    224,
    255,
)
POST_STATE = re.compile(
    r"^Post-(?P<label>skip|ymode|uvmode|y-cf-blk|uv-cf-blk)"
    r"\[(?P<value>[^\]]*)\]"
)
BLOCK = re.compile(
    r"^poc=(?P<poc>-?\d+),y=(?P<y>-?\d+),x=(?P<x>-?\d+),"
    r"bl=(?P<level>\d+),ctx=(?P<context>\d+),bp=(?P<partition>\d+): "
    r"r=(?P<range>\d+)$"
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encode(
    color: tuple[int, int, int],
    size: tuple[int, int],
    quality: int,
    speed: int,
) -> bytes:
    image = Image.new("RGB", size, color)
    output = BytesIO()
    image.save(
        output,
        format="AVIF",
        quality=quality,
        speed=speed,
        max_threads=1,
        subsampling="4:4:4",
        autotiling=False,
    )
    return output.getvalue()


def syntax_signature(log: str) -> dict[str, object]:
    block = None
    states: list[dict[str, str]] = []
    for line in log.splitlines():
        if block is None and (match := BLOCK.fullmatch(line)):
            block = {name: int(value) for name, value in match.groupdict().items()}
        if match := POST_STATE.match(line):
            states.append(match.groupdict())
    if block is None or not states:
        raise RuntimeError(f"incomplete first-block log: {log!r}")
    return {"block": block, "states": states}


def partition_trace(
    output: str,
) -> tuple[
    list[str],
    list[dict[str, object]],
    list[dict[str, object]],
    list[dict[str, int]],
]:
    all_lines = [line.rstrip() for line in output.splitlines() if line.strip()]
    event_stream = []
    entropy_operations = []
    blocks = []
    debug_log = []
    for line in all_lines:
        if line.startswith("@MSAC "):
            operation = json.loads(line.removeprefix("@MSAC "))
            entropy_operations.append(operation)
            event_stream.append({"kind": "entropy", "operation": operation})
        else:
            debug_log.append(line)
            event_stream.append({"kind": "debug", "line": line})
            if match := BLOCK.fullmatch(line):
                blocks.append(
                    {
                        name: int(value)
                        for name, value in match.groupdict().items()
                    }
                )
    if not entropy_operations or entropy_operations[0]["operation"] != "init":
        raise RuntimeError("missing scalar MSAC trace")
    expected_steps = list(range(len(entropy_operations)))
    if [operation["step"] for operation in entropy_operations] != expected_steps:
        raise RuntimeError("non-contiguous scalar MSAC trace")
    if not blocks:
        raise RuntimeError("missing partition-block trace")
    return debug_log, event_stream, entropy_operations, blocks


def decode_case(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    color: tuple[int, int, int],
    size: tuple[int, int],
    *,
    full_trace: bool,
    trace_partition_tree: bool,
    quality: int,
    speed: int,
) -> dict[str, object]:
    first = encode(color, size, quality, speed)
    if first != encode(color, size, quality, speed):
        raise RuntimeError(f"nondeterministic AVIF encoding for {color}")
    path = work / (
        f"rgb_{color[0]}_{color[1]}_{color[2]}_{size[0]}x{size[1]}.avif"
    )
    path.write_bytes(first)
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
    plane_length = size[0] * size[1]
    if len(yuv) != plane_length * 3:
        raise RuntimeError(f"unexpected decoded YUV length for {color}: {len(yuv)}")
    planes = [
        list(yuv[index * plane_length : (index + 1) * plane_length])
        for index in range(3)
    ]
    if any(len(set(plane)) != 1 for plane in planes):
        raise RuntimeError(
            f"constant RGB source reconstructed nonconstant planes: {color}"
        )
    with Image.open(path) as image:
        image.load()
        pillow = image.tobytes()
    case = {
        "color": list(color),
        "file_length": len(first),
        "file_sha256": sha256(first),
        "sample_length": len(sample),
        "sample_sha256": sha256(sample),
        "pillow_rgb": list(pillow[:3]),
        "pillow_sha256": sha256(pillow),
        "decoded_yuv": [plane[0] for plane in planes],
        "syntax": syntax_signature(result.stdout),
    }
    if full_trace:
        if trace_partition_tree:
            log, event_stream, entropy_operations, blocks = partition_trace(
                result.stdout
            )
            case["partition_blocks"] = blocks
        else:
            log, event_stream, entropy_operations, _, _ = parse_debug_log(
                result.stdout
            )
        case["decoder_events"] = event_stream
        case["entropy_operations"] = entropy_operations
        case["dav1d_debug_log"] = log
    return case


def colors(levels: tuple[int, ...], full_cube: bool) -> list[tuple[int, int, int]]:
    if full_cube:
        return list(product(levels, repeat=3))
    grayscale = [(value, value, value) for value in levels]
    probes = [
        (255, 0, 0),
        (0, 255, 0),
        (0, 0, 255),
        (255, 255, 0),
        (0, 255, 255),
        (255, 0, 255),
    ]
    return grayscale + probes


def parse_size(value: str) -> tuple[int, int]:
    try:
        width_text, height_text = value.lower().split("x", 1)
        width = int(width_text)
        height = int(height_text)
    except ValueError as error:
        raise argparse.ArgumentTypeError("size must be WIDTHxHEIGHT") from error
    if width <= 0 or height <= 0:
        raise argparse.ArgumentTypeError("size dimensions must be positive")
    return width, height


def parse_color(value: str) -> tuple[int, int, int]:
    try:
        color = tuple(int(component) for component in value.split(","))
    except ValueError as error:
        raise argparse.ArgumentTypeError("color must be R,G,B") from error
    if len(color) != 3 or any(component < 0 or component > 255 for component in color):
        raise argparse.ArgumentTypeError("color must contain three 8-bit components")
    return color


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument(
        "--dav1d-source",
        type=Path,
        help="Build and instrument this pinned dav1d checkout for the run",
    )
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument(
        "--python-path",
        type=Path,
        help="Optional site-packages path for an isolated Meson installation",
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--levels",
        default=",".join(str(value) for value in DEFAULT_LEVELS),
        help="Comma-separated 8-bit levels used for grayscale and optional RGB cube",
    )
    parser.add_argument(
        "--full-cube",
        action="store_true",
        help="Explore the Cartesian RGB cube instead of grayscale plus primary probes",
    )
    parser.add_argument(
        "--size",
        type=parse_size,
        default=(4, 4),
        metavar="WIDTHxHEIGHT",
        help="Constant image dimensions (default: 4x4)",
    )
    parser.add_argument(
        "--quality",
        type=int,
        default=100,
        help="Pillow AVIF quality in 0..100 (default: 100)",
    )
    parser.add_argument(
        "--speed",
        type=int,
        default=8,
        help="libaom speed in 0..10 (default: 8)",
    )
    parser.add_argument(
        "--color",
        type=parse_color,
        action="append",
        help="Explicit R,G,B probe; repeat to override the default corpus",
    )
    parser.add_argument(
        "--full-trace",
        action="store_true",
        help="Retain the complete deterministic dav1d entropy/debug trace",
    )
    parser.add_argument(
        "--partition-tree",
        action="store_true",
        help="Allow and retain multiple partition-block headers in a full trace",
    )
    args = parser.parse_args()

    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")
    levels = tuple(int(value) for value in args.levels.split(","))
    if not levels or any(value < 0 or value > 255 for value in levels):
        raise RuntimeError("levels must be nonempty 8-bit integers")
    if not 0 <= args.quality <= 100:
        raise RuntimeError("quality must be in 0..100")
    if not 0 <= args.speed <= 10:
        raise RuntimeError("speed must be in 0..10")
    if args.partition_tree and not args.full_trace:
        raise RuntimeError("--partition-tree requires --full-trace")

    with tempfile.TemporaryDirectory(prefix="image-star-avif-corpus-") as name:
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
        selected_colors = (
            args.color if args.color is not None else colors(levels, args.full_cube)
        )
        cases = [
            decode_case(
                executable,
                environment,
                work,
                color,
                args.size,
                full_trace=args.full_trace,
                trace_partition_tree=args.partition_tree,
                quality=args.quality,
                speed=args.speed,
            )
            for color in selected_colors
        ]
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
            "subsampling": "4:4:4",
            "autotiling": False,
        },
        "cases": cases,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(cases)} deterministic cases: {args.output}")


if __name__ == "__main__":
    main()
