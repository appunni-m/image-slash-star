#!/usr/bin/env python3
"""Reverse-map deterministic AVIF luma-residual patterns through pinned dav1d.

This diagnostic fixture-selection tool complements the constant-color
subsampling corpus. It perturbs grayscale source pixels around a constant base
while preserving explicit 4:2:0 encoding, then records the complete scalar
dav1d syntax and entropy trace. Generated inputs remain temporary unless the
caller requests a diagnostic retention directory.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
from io import BytesIO
from pathlib import Path
from typing import Iterable

from PIL import Image, _avif, features

from explore_avif_subsampling_corpus import decode_encoded_case, parse_pair
from generate_av1_reconstruction_refs import (
    build_dav1d,
    resolve_tool,
    verify_source,
)


DEFAULT_BASES = (127, 129)
DEFAULT_DELTAS = (-32, -16, -8, -4, -2, -1, 1, 2, 4, 8, 16, 32)


def signed_label(value: int) -> str:
    """Return a filename-safe signed integer label."""

    return f"p{value}" if value >= 0 else f"n{-value}"


def encode(
    pixels: tuple[int, ...],
    size: tuple[int, int],
    quality: int,
    speed: int,
) -> bytes:
    """Encode one deterministic grayscale-pattern RGB image as 4:2:0 AVIF."""

    image = Image.new("RGB", size)
    image.putdata([(value, value, value) for value in pixels])

    def encode_once() -> bytes:
        output = BytesIO()
        image.save(
            output,
            format="AVIF",
            quality=quality,
            speed=speed,
            max_threads=1,
            subsampling="4:2:0",
            autotiling=False,
        )
        return output.getvalue()

    first = encode_once()
    if first != encode_once():
        raise RuntimeError("nondeterministic AVIF residual-pattern encoding")
    return first


def replace_selected(
    pixels: list[int],
    indexes: Iterable[int],
    value: int,
) -> tuple[int, ...]:
    """Return a copy of ``pixels`` with ``indexes`` replaced by ``value``."""

    result = pixels.copy()
    for index in indexes:
        result[index] = value
    return tuple(result)


def candidates(
    size: tuple[int, int],
    bases: tuple[int, ...],
    deltas: tuple[int, ...],
) -> list[tuple[str, tuple[int, ...], dict[str, object]]]:
    """Build deterministic impulse, stripe, split, and checker candidates."""

    width, height = size
    result: list[tuple[str, tuple[int, ...], dict[str, object]]] = []
    seen: set[tuple[int, ...]] = set()

    def add(
        name: str,
        pixels: tuple[int, ...],
        source: dict[str, object],
    ) -> None:
        if pixels not in seen:
            seen.add(pixels)
            result.append((name, pixels, source))

    for base in bases:
        constant = [base] * (width * height)
        for delta in deltas:
            changed = base + delta
            if not 0 <= changed <= 255:
                continue
            suffix = f"b{base}_{signed_label(delta)}"
            common: dict[str, object] = {
                "base": base,
                "delta": delta,
                "changed": changed,
            }
            for y in range(height):
                for x in range(width):
                    index = y * width + x
                    add(
                        f"impulse_x{x}_y{y}_{suffix}",
                        replace_selected(constant, (index,), changed),
                        {**common, "pattern": "impulse", "x": x, "y": y},
                    )
            for y in range(height):
                indexes = range(y * width, (y + 1) * width)
                add(
                    f"row_y{y}_{suffix}",
                    replace_selected(constant, indexes, changed),
                    {**common, "pattern": "row", "y": y},
                )
            for x in range(width):
                indexes = range(x, width * height, width)
                add(
                    f"column_x{x}_{suffix}",
                    replace_selected(constant, indexes, changed),
                    {**common, "pattern": "column", "x": x},
                )
            for split in range(1, width):
                indexes = (
                    y * width + x
                    for y in range(height)
                    for x in range(split)
                )
                add(
                    f"vertical_split_x{split}_{suffix}",
                    replace_selected(constant, indexes, changed),
                    {**common, "pattern": "vertical_split", "split": split},
                )
            for split in range(1, height):
                indexes = range(split * width)
                add(
                    f"horizontal_split_y{split}_{suffix}",
                    replace_selected(constant, indexes, changed),
                    {**common, "pattern": "horizontal_split", "split": split},
                )
            add(
                f"vertical_stripes_{suffix}",
                replace_selected(
                    constant,
                    (
                        y * width + x
                        for y in range(height)
                        for x in range(width)
                        if x % 2 == 0
                    ),
                    changed,
                ),
                {**common, "pattern": "vertical_stripes"},
            )
            add(
                f"horizontal_stripes_{suffix}",
                replace_selected(
                    constant,
                    (
                        y * width + x
                        for y in range(height)
                        for x in range(width)
                        if y % 2 == 0
                    ),
                    changed,
                ),
                {**common, "pattern": "horizontal_stripes"},
            )
            add(
                f"checkerboard_{suffix}",
                replace_selected(
                    constant,
                    (
                        y * width + x
                        for y in range(height)
                        for x in range(width)
                        if (x + y) % 2 == 0
                    ),
                    changed,
                ),
                {**common, "pattern": "checkerboard"},
            )
    return result


def main() -> None:
    """Generate and trace the requested residual-pattern corpus."""

    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument("--dav1d-source", type=Path)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--python-path", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--retain-dir", type=Path)
    parser.add_argument("--size", type=parse_pair, default=(4, 4))
    parser.add_argument("--base", type=int, action="append")
    parser.add_argument("--delta", type=int, action="append")
    parser.add_argument(
        "--pattern",
        action="append",
        help="Retain only an exact generated pattern name; repeat as needed",
    )
    parser.add_argument("--quality", type=int, default=99)
    parser.add_argument("--speed", type=int, default=8)
    args = parser.parse_args()

    bases = tuple(args.base or DEFAULT_BASES)
    deltas = tuple(args.delta or DEFAULT_DELTAS)
    if any(value < 0 or value > 255 for value in bases):
        parser.error("--base values must be in 0..255")
    if not deltas or any(value == 0 or value < -255 or value > 255 for value in deltas):
        parser.error("--delta values must be nonzero and in -255..255")
    if not 0 <= args.quality <= 100:
        parser.error("--quality must be in 0..100")
    if not 0 <= args.speed <= 10:
        parser.error("--speed must be in 0..10")

    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")

    selected = candidates(args.size, bases, deltas)
    if args.pattern:
        requested = set(args.pattern)
        selected = [candidate for candidate in selected if candidate[0] in requested]
        found = {candidate[0] for candidate in selected}
        if missing := requested - found:
            parser.error(f"unknown generated patterns: {sorted(missing)}")

    with tempfile.TemporaryDirectory(prefix="image-star-avif-residual-corpus-") as name:
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

        cases = []
        for pattern, pixels, source in selected:
            encoded = encode(pixels, args.size, args.quality, args.speed)
            if args.retain_dir is not None:
                args.retain_dir.mkdir(parents=True, exist_ok=True)
                (args.retain_dir / f"{pattern}.avif").write_bytes(encoded)
            cases.append(
                decode_encoded_case(
                    executable,
                    environment,
                    work,
                    encoded,
                    pattern,
                    {
                        **source,
                        "pattern_name": pattern,
                        "source_pixels_sha256": hashlib.sha256(bytes(pixels)).hexdigest(),
                    },
                )
            )

    report = {
        "oracle": {
            "pillow": Image.__version__,
            "libavif": features.version("avif"),
            "codecs": codecs,
            "dav1d_commit": "b546257f770768b2c88258c533da38b91a06f737",
        },
        "encoding": {
            "size": list(args.size),
            "quality": args.quality,
            "speed": args.speed,
            "max_threads": 1,
            "subsampling": "4:2:0",
            "autotiling": False,
        },
        "cases": cases,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"Written {len(cases)} deterministic residual cases: {args.output}")


if __name__ == "__main__":
    main()
