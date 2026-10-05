#!/usr/bin/env python3
"""Build fixture-only libwebp 1.6.0 cwebp variants for VP8 segmentation cases.

Pass an unpacked upstream libwebp 1.6.0 source tree. The source is copied into
a temporary directory before the encoder-only patch is applied, so this tool
never modifies the supplied checkout.
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path


DELTA_SEGMENT_PATCHES = (
    (
        "VP8PutBitUniform(bw, 1);   // (segment_feature_mode = 1. Paragraph 9.3.)",
        "VP8PutBitUniform(bw, 0);   // (segment_feature_mode = 0. Paragraph 9.3.)",
    ),
    (
        "VP8PutSignedBits(bw, enc->dqm[s].quant, 7);",
        "VP8PutSignedBits(bw, enc->dqm[s].quant - enc->base_quant, 7);",
    ),
    (
        "VP8PutSignedBits(bw, enc->dqm[s].fstrength, 6);",
        "VP8PutSignedBits(bw, enc->dqm[s].fstrength - enc->filter_hdr.level, 6);",
    ),
)

SEGMENT_FEATURE_DATA_DISABLED_16X16_PATCHES = (
    (
        "if (VP8PutBitUniform(bw, (hdr->num_segments > 1))) {",
        "if (VP8PutBitUniform(bw, (hdr->num_segments > 1) ||\n"
        "      (enc->pic->width == 16 && enc->pic->height == 16))) {",
    ),
    ("const int update_data = 1;", "const int update_data = 0;"),
)


def patch_encoder(source: Path, variant: str) -> None:
    news = source / "NEWS"
    if not news.is_file() or "version 1.6.0" not in news.read_text(encoding="utf-8")[:128]:
        raise RuntimeError("libwebp source must be the pinned 1.6.0 release")

    path = source / "src" / "enc" / "syntax_enc.c"
    text = path.read_text(encoding="utf-8")
    patches = {
        "delta-segments": DELTA_SEGMENT_PATCHES,
        "feature-data-disabled-16x16": SEGMENT_FEATURE_DATA_DISABLED_16X16_PATCHES,
    }[variant]
    for original, replacement in patches:
        if text.count(original) != 1:
            raise RuntimeError(f"expected one libwebp encoder site: {original}")
        text = text.replace(original, replacement, 1)
    path.write_text(text, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="unpacked upstream libwebp 1.6.0 source")
    parser.add_argument("output", type=Path, help="destination for the fixture cwebp")
    parser.add_argument("--jobs", type=int, default=2, help="parallel CMake build jobs")
    parser.add_argument(
        "--variant",
        choices=("delta-segments", "feature-data-disabled-16x16"),
        default="delta-segments",
        help="VP8 segmentation header behavior to emit",
    )
    args = parser.parse_args()

    source = args.source.resolve()
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    if not (source / "CMakeLists.txt").is_file():
        parser.error(f"not a libwebp CMake source tree: {source}")

    with tempfile.TemporaryDirectory(prefix="image-star-webp-delta-") as temporary:
        root = Path(temporary)
        patched_source = root / "libwebp"
        shutil.copytree(
            source,
            patched_source,
            ignore=shutil.ignore_patterns(".git", "build", "cmake-build-*"),
        )
        patch_encoder(patched_source, args.variant)

        build = root / "build"
        subprocess.run(
            [
                "cmake",
                "-S",
                str(patched_source),
                "-B",
                str(build),
                "-DCMAKE_BUILD_TYPE=Release",
                "-DWEBP_BUILD_CWEBP=ON",
                "-DWEBP_BUILD_DWEBP=OFF",
                "-DWEBP_BUILD_GIF2WEBP=OFF",
                "-DWEBP_BUILD_IMG2WEBP=OFF",
                "-DWEBP_BUILD_VWEBP=OFF",
                "-DWEBP_BUILD_WEBPINFO=OFF",
                "-DWEBP_BUILD_WEBPMUX=OFF",
                "-DWEBP_BUILD_ANIM_UTILS=OFF",
                "-DWEBP_BUILD_EXTRAS=OFF",
            ],
            check=True,
        )
        subprocess.run(
            ["cmake", "--build", str(build), "--target", "cwebp", "--parallel", str(args.jobs)],
            check=True,
        )

        candidates = [build / "cwebp", build / "examples" / "cwebp"]
        executable = next((candidate for candidate in candidates if candidate.is_file()), None)
        if executable is None:
            raise RuntimeError("CMake built cwebp but its executable was not found")

        args.output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(executable, args.output)

    version = subprocess.run(
        [str(args.output), "-version"], check=True, capture_output=True, text=True
    ).stdout.splitlines()[0]
    if version != "1.6.0":
        raise RuntimeError(f"built cwebp reports {version!r}, expected '1.6.0'")
    print(f"Built fixture encoder: {args.output} (libwebp {version})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
