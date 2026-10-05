#!/usr/bin/env python3
"""Generate a libwebp 1.6.0 VP8 mode-loop-filter-delta parity candidate.

The encoder patch is applied only to a temporary source copy. The generated
WebP uses the repository's deterministic 128x128 RGB pattern and receives
Pillow 12.2.0 open/verify/load checks before it is written to the output path.

Example:
    .oracle-venv/bin/python scripts/generate_webp_lf_mode_delta_fixture.py \
        /path/to/libwebp-1.6.0 \
        --output target/release-evidence/webp_vp8_mode_delta_candidate.webp
"""

from __future__ import annotations

import argparse
import hashlib
import shutil
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, __version__ as PILLOW_VERSION

if __package__:
    from .generate_test_assets import pattern_img
else:
    from generate_test_assets import pattern_img


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT = ROOT / "target/release-evidence/webp_vp8_mode_delta_candidate.webp"

PINNED_LIBWEBP_VERSION = "1.6.0"
PINNED_PILLOW_VERSION = "12.2.0"
SOURCE_RGB_SHA256 = "8a1d6fcc36f5b5e70fbf949d4e612630a1931279383e7c21db27cd3cbad98131"
SOURCE_PPM_SHA256 = "9efd98332fada058f0ec106ac5c714b7898d640379b3739a22a1185397604421"
ASSET_SHA256 = "86b97b8489b9ec2318858bef5d91be138e1222398dd2b8ffbde6dded0e598b55"
PIXELS_SHA256 = "ce195285d0ae374bc2ae5538da590658fbe26846b27cf2b0d2300979319f4c99"

ENCODER_HEADER = Path("src/enc/webp_enc.c")
ORIGINAL_FILTER_DELTA_ASSIGNMENT = "hdr->i4x4_lf_delta = 0;"
MODE_FILTER_DELTA_ASSIGNMENT = "hdr->i4x4_lf_delta = 5;"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def run(
    command: list[str], *, cwd: Path | None = None
) -> subprocess.CompletedProcess[str]:
    """Run one bounded build command and retain its diagnostics on failure."""
    result = subprocess.run(
        command,
        cwd=cwd,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode:
        details = "\n".join(part for part in (result.stdout, result.stderr) if part)
        raise RuntimeError(
            f"command exited {result.returncode}: "
            f"{' '.join(command)}\n{details[-6000:]}"
        )
    return result


def validate_source(source: Path) -> None:
    news = source / "NEWS"
    if not news.is_file() or "version 1.6.0" not in news.read_text(
        encoding="utf-8"
    )[:128]:
        raise RuntimeError("libwebp source must be the pinned 1.6.0 release")


def patch_encoder(source: Path) -> None:
    path = source / ENCODER_HEADER
    text = path.read_text(encoding="utf-8")
    if text.count(ORIGINAL_FILTER_DELTA_ASSIGNMENT) != 1:
        raise RuntimeError("expected one default VP8 i4x4 loop-filter delta assignment")
    path.write_text(
        text.replace(ORIGINAL_FILTER_DELTA_ASSIGNMENT, MODE_FILTER_DELTA_ASSIGNMENT, 1),
        encoding="utf-8",
    )


def build_cwebp(source: Path, build: Path) -> Path:
    run(
        [
            "cmake",
            "-S",
            str(source),
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
        ]
    )
    run(["cmake", "--build", str(build), "--target", "cwebp", "--parallel", "2"])

    candidates = (build / "cwebp", build / "examples/cwebp")
    executable = next(
        (candidate for candidate in candidates if candidate.is_file()),
        None,
    )
    if executable is None:
        raise RuntimeError("CMake built cwebp but its executable was not found")
    version_output = run([str(executable), "-version"]).stdout.strip()
    version = version_output.splitlines()[0]
    if version != PINNED_LIBWEBP_VERSION:
        raise RuntimeError(f"built cwebp reports {version!r}, expected '1.6.0'")
    return executable


def create_source_ppm(path: Path) -> None:
    image = pattern_img("RGB")
    if image.size != (128, 128) or image.mode != "RGB":
        raise RuntimeError("VP8 mode-delta source must be 128x128 RGB")
    source_pixels_hash = sha256(image.tobytes())
    if source_pixels_hash != SOURCE_RGB_SHA256:
        raise RuntimeError("Pillow pattern source pixels differ from their pin")

    image.save(path, format="PPM")
    if sha256(path.read_bytes()) != SOURCE_PPM_SHA256:
        raise RuntimeError("Pillow pattern PPM differs from its pin")


def encode_candidate(cwebp: Path, source_ppm: Path, candidate: Path) -> None:
    run(
        [
            str(cwebp),
            "-quiet",
            "-q",
            "75",
            "-m",
            "4",
            "-segments",
            "4",
            "-sns",
            "100",
            "-f",
            "60",
            str(source_ppm),
            "-o",
            str(candidate),
        ]
    )


def validate_candidate(candidate: Path) -> tuple[str, str]:
    asset_hash = sha256(candidate.read_bytes())
    if asset_hash != ASSET_SHA256:
        raise RuntimeError(f"generated WebP asset differs from its pin: {asset_hash}")

    with Image.open(candidate) as image:
        image.verify()
    with Image.open(candidate) as image:
        image.load()
        if image.format != "WEBP" or image.mode != "RGB" or image.size != (128, 128):
            raise RuntimeError(
                "Pillow decoded the candidate with unexpected image facts"
            )
        pixels_hash = sha256(image.tobytes())
    if pixels_hash != PIXELS_SHA256:
        raise RuntimeError(
            f"Pillow decoded pixels differ from their pin: {pixels_hash}"
        )
    return asset_hash, pixels_hash


def generate(source: Path, output: Path) -> tuple[str, str]:
    if PILLOW_VERSION != PINNED_PILLOW_VERSION:
        raise RuntimeError(
            f"Pillow {PINNED_PILLOW_VERSION} is required, found {PILLOW_VERSION}"
        )
    validate_source(source)

    output = output.resolve()
    with tempfile.TemporaryDirectory(prefix="image-star-webp-mode-delta-") as temporary:
        temporary_root = Path(temporary)
        patched_source = temporary_root / "libwebp"
        shutil.copytree(
            source,
            patched_source,
            ignore=shutil.ignore_patterns(".git", "build", "cmake-build-*"),
        )
        patch_encoder(patched_source)
        cwebp = build_cwebp(patched_source, temporary_root / "build")

        source_ppm = temporary_root / "source.ppm"
        candidate = temporary_root / "candidate.webp"
        create_source_ppm(source_ppm)
        encode_candidate(cwebp, source_ppm, candidate)
        asset_hash, pixels_hash = validate_candidate(candidate)

        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(candidate, output)
    return asset_hash, pixels_hash


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "libwebp_source",
        type=Path,
        help="unpacked upstream libwebp 1.6.0 source tree",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        help=f"output candidate path (default: {DEFAULT_OUTPUT})",
    )
    args = parser.parse_args()

    asset_hash, pixels_hash = generate(args.libwebp_source.resolve(), args.output)
    print(f"Pillow: {PILLOW_VERSION}")
    print(f"libwebp: {PINNED_LIBWEBP_VERSION}")
    print(f"fixture: {args.output.resolve()}")
    print(f"asset_sha256: {asset_hash}")
    print(f"pixel_sha256: {pixels_hash}")
    print("Pillow open/verify/load: ok (WEBP RGB 128x128)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
