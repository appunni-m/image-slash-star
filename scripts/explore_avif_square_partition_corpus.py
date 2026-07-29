#!/usr/bin/env python3
"""Reverse-map deterministic two-color square AVIFs through pinned dav1d.

This diagnostic tool searches replacement colors and rectangle origins around
the committed square-partition fixtures. It records coefficient syntax and
decoded plane hashes from Pillow's pinned AVIF stack without invoking Rust.
Generated AVIF, OBU, and YUV files remain in a temporary directory.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
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


COEFFICIENT_PREFIXES = (
    "Post-non-zero",
    "Post-eob",
    "Post-lo_tok",
    "Post-hi_tok",
    "Post-dc_lo_tok",
    "Post-dc_hi_tok",
    "Post-dc_sign",
    "Post-sign",
    "Post-y-cf-blk",
    "Post-uv-cf-blk",
)


def sha256(data: bytes) -> str:
    """Return a lowercase SHA-256 digest."""

    return hashlib.sha256(data).hexdigest()


def parse_pair(value: str) -> tuple[int, int]:
    """Parse an unsigned X,Y pair."""

    try:
        pair = tuple(int(component) for component in value.split(","))
    except ValueError as error:
        raise argparse.ArgumentTypeError("value must be X,Y") from error
    if len(pair) != 2 or any(component < 0 for component in pair):
        raise argparse.ArgumentTypeError("value must contain two unsigned integers")
    return pair


def parse_color(value: str) -> tuple[int, int, int]:
    """Parse an R,G,B color."""

    try:
        color = tuple(int(component) for component in value.split(","))
    except ValueError as error:
        raise argparse.ArgumentTypeError("color must be R,G,B") from error
    if len(color) != 3 or any(component < 0 or component > 255 for component in color):
        raise argparse.ArgumentTypeError("color must contain three 8-bit components")
    return color


def parse_origin_box(value: str) -> tuple[int, int, int, int]:
    """Parse inclusive MIN_X,MIN_Y,MAX_X,MAX_Y origin bounds."""

    try:
        bounds = tuple(int(component) for component in value.split(","))
    except ValueError as error:
        raise argparse.ArgumentTypeError(
            "origin box must be MIN_X,MIN_Y,MAX_X,MAX_Y"
        ) from error
    if (
        len(bounds) != 4
        or any(component < 0 for component in bounds)
        or bounds[0] > bounds[2]
        or bounds[1] > bounds[3]
    ):
        raise argparse.ArgumentTypeError("origin box bounds are invalid")
    return bounds


def encode(
    source: tuple[int, int, int],
    replacement: tuple[int, int, int],
    size: tuple[int, int],
    origin: tuple[int, int],
) -> bytes:
    """Encode one deterministic two-color rectangle."""

    pixels = bytes(
        component
        for y in range(size[1])
        for x in range(size[0])
        for component in (
            replacement if x >= origin[0] and y >= origin[1] else source
        )
    )
    image = Image.frombytes("RGB", size, pixels)

    def encode_once() -> bytes:
        output = BytesIO()
        image.save(
            output,
            format="AVIF",
            quality=100,
            speed=8,
            max_threads=1,
            subsampling="4:4:4",
            autotiling=False,
        )
        return output.getvalue()

    first = encode_once()
    if first != encode_once():
        raise RuntimeError(
            f"nondeterministic AVIF encoding for {replacement} at {origin}"
        )
    return first


def decode_case(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    source: tuple[int, int, int],
    replacement: tuple[int, int, int],
    size: tuple[int, int],
    origin: tuple[int, int],
) -> dict[str, object]:
    """Encode and trace one corpus member."""

    encoded = encode(source, replacement, size, origin)
    stem = (
        f"rgb_{replacement[0]}_{replacement[1]}_{replacement[2]}"
        f"_origin_{origin[0]}_{origin[1]}"
    )
    path = work / f"{stem}.avif"
    path.write_bytes(encoded)
    sample, _ = extract_color_item(path)
    sample_path = work / f"{stem}.obu"
    output_path = work / f"{stem}.yuv"
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
    debug_log, _, entropy_operations, blocks, _ = parse_debug_log(result.stdout)
    yuv = output_path.read_bytes()
    plane_length = size[0] * size[1]
    if len(yuv) != plane_length * 3:
        raise RuntimeError(f"unexpected decoded YUV length for {stem}: {len(yuv)}")
    plane_bytes = [
        yuv[index * plane_length : (index + 1) * plane_length] for index in range(3)
    ]
    with Image.open(path) as image:
        image.load()
        pillow = image.tobytes()
    return {
        "replacement": list(replacement),
        "origin": list(origin),
        "file_length": len(encoded),
        "file_sha256": sha256(encoded),
        "sample_length": len(sample),
        "sample_sha256": sha256(sample),
        "partition_blocks": blocks,
        "entropy_operation_count": len(entropy_operations),
        "coefficient_trace": [
            line for line in debug_log if line.startswith(COEFFICIENT_PREFIXES)
        ],
        "plane_sha256": [sha256(plane) for plane in plane_bytes],
        "pillow_sha256": sha256(pillow),
    }


def main() -> None:
    """Run the requested deterministic corpus."""

    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument("--dav1d-source", type=Path)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--python-path", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source", type=parse_color, default=(17, 91, 203))
    parser.add_argument("--replacement", type=parse_color, action="append", required=True)
    parser.add_argument("--size", type=parse_pair, default=(12, 12))
    origins = parser.add_mutually_exclusive_group(required=True)
    origins.add_argument("--origin", type=parse_pair, action="append")
    origins.add_argument("--origin-box", type=parse_origin_box)
    args = parser.parse_args()

    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")
    if any(dimension == 0 for dimension in args.size):
        raise RuntimeError("size dimensions must be positive")
    if args.origin_box is not None:
        min_x, min_y, max_x, max_y = args.origin_box
        selected_origins = list(
            product(range(min_x, max_x + 1), range(min_y, max_y + 1))
        )
    else:
        selected_origins = args.origin
    if selected_origins is None:
        raise RuntimeError("at least one origin is required")
    if any(
        origin[0] >= args.size[0] or origin[1] >= args.size[1]
        for origin in selected_origins
    ):
        raise RuntimeError("every origin must lie inside the encoded image")

    with tempfile.TemporaryDirectory(prefix="image-star-avif-square-corpus-") as name:
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
        cases = [
            decode_case(
                executable,
                environment,
                work,
                args.source,
                replacement,
                args.size,
                origin,
            )
            for replacement in args.replacement
            for origin in selected_origins
        ]

    report = {
        "oracle": {
            "pillow": Image.__version__,
            "libavif": features.version("avif"),
            "codecs": codecs,
            "dav1d_commit": "b546257f770768b2c88258c533da38b91a06f737",
        },
        "source": list(args.source),
        "size": list(args.size),
        "cases": cases,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"Written deterministic square corpus: {args.output}")


if __name__ == "__main__":
    main()
