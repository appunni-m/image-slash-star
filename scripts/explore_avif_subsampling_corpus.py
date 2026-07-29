#!/usr/bin/env python3
"""Trace deterministic constant-color AVIF subsampling candidates.

This diagnostic tool encodes a small source-color/dimension corpus through the
pinned Pillow/libavif/libaom oracle, decodes each AV1 item through instrumented
scalar dav1d, and records exact syntax, plane, and Pillow RGB evidence. Generated
AVIF, OBU, and YUV files remain in a temporary directory.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from generate_av1_reconstruction_refs import (
    build_dav1d,
    extract_color_item,
    parse_debug_log,
    portable_color_reference,
    resolve_tool,
    run,
    verify_source,
)


def sha256(data: bytes) -> str:
    """Return a lowercase SHA-256 digest."""

    return hashlib.sha256(data).hexdigest()


def parse_pair(value: str) -> tuple[int, int]:
    """Parse a positive WIDTH,HEIGHT pair."""

    try:
        pair = tuple(int(component) for component in value.split(","))
    except ValueError as error:
        raise argparse.ArgumentTypeError("size must be WIDTH,HEIGHT") from error
    if len(pair) != 2 or any(component <= 0 for component in pair):
        raise argparse.ArgumentTypeError("size must contain two positive integers")
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


def encode(color: tuple[int, int, int], size: tuple[int, int]) -> bytes:
    """Encode one deterministic 4:2:0 constant-color candidate."""

    image = Image.new("RGB", size, color)

    def encode_once() -> bytes:
        output = BytesIO()
        image.save(
            output,
            format="AVIF",
            quality=100,
            speed=8,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
        )
        return output.getvalue()

    first = encode_once()
    if first != encode_once():
        raise RuntimeError(f"nondeterministic AVIF encoding for {color} at {size}")
    return first


def plane_record(data: bytes, width: int) -> dict[str, object]:
    """Return exact bytes, rows, and digest for one decoded plane."""

    return {
        "bytes": len(data),
        "sha256": sha256(data),
        "rows": [data[offset : offset + width].hex() for offset in range(0, len(data), width)],
    }


def decode_case(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    color: tuple[int, int, int],
    size: tuple[int, int],
) -> dict[str, object]:
    """Encode and trace one corpus member."""

    encoded = encode(color, size)
    stem = f"rgb_{color[0]}_{color[1]}_{color[2]}_{size[0]}x{size[1]}_420"
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
    debug_log, event_stream, entropy_operations, blocks, states = parse_debug_log(
        result.stdout
    )
    yuv = output_path.read_bytes()
    chroma_size = ((size[0] + 1) // 2, (size[1] + 1) // 2)
    y_length = size[0] * size[1]
    chroma_length = chroma_size[0] * chroma_size[1]
    if len(yuv) != y_length + 2 * chroma_length:
        raise RuntimeError(f"unexpected decoded YUV length for {stem}: {len(yuv)}")
    y_plane = yuv[:y_length]
    u_plane = yuv[y_length : y_length + chroma_length]
    v_plane = yuv[y_length + chroma_length :]
    with Image.open(path) as image:
        image.load()
        pillow = image.tobytes()
        if image.mode != "RGB" or image.size != size:
            raise RuntimeError(f"unexpected Pillow output for {stem}")
    return {
        "color": list(color),
        "size": list(size),
        "file_length": len(encoded),
        "file_sha256": sha256(encoded),
        "sample_length": len(sample),
        "sample_sha256": sha256(sample),
        "portable_color": portable_color_reference(path),
        "partition_blocks": blocks,
        "entropy_operation_count": len(entropy_operations),
        "entropy_operations": entropy_operations,
        "debug_log": debug_log,
        "event_stream": event_stream,
        "states": states,
        "planes": {
            "y": plane_record(y_plane, size[0]),
            "u": plane_record(u_plane, chroma_size[0]),
            "v": plane_record(v_plane, chroma_size[0]),
        },
        "pillow": {
            "bytes": len(pillow),
            "sha256": sha256(pillow),
        },
    }


def main() -> None:
    """Run the requested deterministic subsampling corpus."""

    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument("--dav1d-source", type=Path)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--python-path", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--color", type=parse_color, action="append", required=True)
    parser.add_argument("--size", type=parse_pair, action="append", required=True)
    args = parser.parse_args()

    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")

    with tempfile.TemporaryDirectory(prefix="image-star-avif-subsampling-corpus-") as name:
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
            decode_case(executable, environment, work, color, size)
            for color in args.color
            for size in args.size
        ]

    report = {
        "oracle": {
            "pillow": Image.__version__,
            "libavif": features.version("avif"),
            "codecs": codecs,
            "dav1d_commit": "b546257f770768b2c88258c533da38b91a06f737",
        },
        "subsampling": "4:2:0",
        "cases": cases,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"Written deterministic subsampling corpus: {args.output}")


if __name__ == "__main__":
    main()
