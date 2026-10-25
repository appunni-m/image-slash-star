#!/usr/bin/env python3
"""Generate a VP8 parity fixture with enabled loop-filter deltas and no update."""

from __future__ import annotations

import argparse
import hashlib
import shutil
import tempfile
from pathlib import Path

from PIL import Image, __version__ as PILLOW_VERSION

if __package__:
    from . import generate_webp_lf_mode_delta_fixture as mode_delta
else:
    import generate_webp_lf_mode_delta_fixture as mode_delta


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "webp"
    / "lossy_mode_filter_delta_no_update.webp"
)

ASSET_SHA256 = "39a3eeb4102f2b10bdb25f9cca0687345afa5ce3c27ff38a243b36b82ee78bd0"
PIXELS_SHA256 = "6b2c202c55d0bf70d9f32b74c1b8705232ae41e0ed24203ea2b1b138dfd914fe"

NEED_UPDATE_ASSIGNMENT = "const int need_update = (hdr->i4x4_lf_delta != 0);"
NO_UPDATE_ASSIGNMENT = "const int need_update = 0;"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def patch_no_update(source: Path) -> None:
    path = source / "src/enc/syntax_enc.c"
    text = path.read_text(encoding="utf-8")
    if text.count(NEED_UPDATE_ASSIGNMENT) != 1:
        raise RuntimeError("expected one pinned libwebp loop-filter update expression")
    path.write_text(
        text.replace(NEED_UPDATE_ASSIGNMENT, NO_UPDATE_ASSIGNMENT, 1),
        encoding="utf-8",
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
            raise RuntimeError("Pillow decoded the candidate with unexpected image facts")
        pixels_hash = sha256(image.tobytes())
    if pixels_hash != PIXELS_SHA256:
        raise RuntimeError(f"Pillow-decoded pixels differ from their pin: {pixels_hash}")
    return asset_hash, pixels_hash


def generate(source: Path, output: Path) -> tuple[str, str]:
    if PILLOW_VERSION != mode_delta.PINNED_PILLOW_VERSION:
        raise RuntimeError(
            f"Pillow {mode_delta.PINNED_PILLOW_VERSION} is required, found {PILLOW_VERSION}"
        )
    mode_delta.validate_source(source)

    output = output.resolve()
    with tempfile.TemporaryDirectory(prefix="image-star-webp-no-lf-update-") as temporary:
        temporary_root = Path(temporary)
        patched_source = temporary_root / "libwebp"
        shutil.copytree(
            source,
            patched_source,
            ignore=shutil.ignore_patterns(".git", "build", "cmake-build-*"),
        )
        mode_delta.patch_encoder(patched_source)
        patch_no_update(patched_source)
        cwebp = mode_delta.build_cwebp(patched_source, temporary_root / "build")

        source_ppm = temporary_root / "source.ppm"
        candidate = temporary_root / "candidate.webp"
        mode_delta.create_source_ppm(source_ppm)
        mode_delta.encode_candidate(cwebp, source_ppm, candidate)
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
        help=f"output fixture path (default: {DEFAULT_OUTPUT})",
    )
    args = parser.parse_args()

    asset_hash, pixels_hash = generate(args.libwebp_source.resolve(), args.output)
    print(f"Pillow: {PILLOW_VERSION}")
    print(f"libwebp: {mode_delta.PINNED_LIBWEBP_VERSION}")
    print(f"fixture: {args.output.resolve()}")
    print(f"asset_sha256: {asset_hash}")
    print(f"pixel_sha256: {pixels_hash}")
    print("Pillow open/verify/load: ok (WEBP RGB 128x128)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
