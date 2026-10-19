#!/usr/bin/env python3
"""Generate or verify libjpeg-turbo YCCK sampling-edge JPEG fixtures."""

from __future__ import annotations

import argparse
import hashlib
import os
import platform
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
ASSET_DIR = ROOT / "tests/fixtures/input/images/jpeg"
SOURCE = ROOT / "scripts/fixtures/jpeg_ycck_subsampled_17x9.c"
FIXTURES = {
    "baseline_ycck_subsampled_17x9.jpg": (
        (),
        "39d712b61d2abe430c9767a1630a4f76dd8757d2b1357cec8524af4f414c23b7",
    ),
    "progressive_ycck_subsampled_17x9.jpg": (
        ("progressive",),
        "fdcb83551e242178ccef25f4e47943c555a362d37530a7118cb54f939830f014",
    ),
    "baseline_ycck_h2v1_17x9.jpg": (
        ("h2v1",),
        "220828c276f262ee48c0bc8c1829e14bdad6eb11b36959b2cc1bfb2e8564aa88",
    ),
    "baseline_ycck_h4v1_17x9.jpg": (
        ("h4v1",),
        "a48aa5e81cbb354fb032be6f03f28195ae70f067d22503066cca5d5ac9894b52",
    ),
    "baseline_ycck_vertical_only_17x9.jpg": (
        ("v2",),
        "cf5996c469605f93b1d0fa2fcd13c614cc45ad6ddad83d63749dd6837dd04307",
    ),
    "baseline_ycck_vertical_only_even_height_17x10.jpg": (
        ("v2_even_height",),
        "eb81f72b3e99aaa819d0b22b696628a2771c22ad44fc588322799c4616423307",
    ),
    "baseline_ycck_luma_upsampled_17x9.jpg": (
        ("luma_upsample",),
        "e870d1a9a8eac573e742681e85a7660fc38de3f7ab786511892020ef0ae279cd",
    ),
    "baseline_ycck_black_upsampled_17x9.jpg": (
        ("black_upsample",),
        "58ded3a110ea6b9a583021f5bf577f12238c6ea2d44b6a9debc1ee91d0f6c155",
    ),
    "baseline_ycck_h2v2_single_sample_2x2.jpg": (
        ("tiny_2x2",),
        "0c2f73a0727f6457b636a2409ad01143aafd52c30f3c1edfbd5714828d3edf5b",
    ),
    "baseline_ycck_h2v1_single_sample_2x2.jpg": (
        ("tiny_2x2", "h2v1"),
        "b7be4ab92b1acae8ad419abbbb3b9270f123af93a08fd325e176862ae0c05d74",
    ),
}
FRACTIONAL_FIXTURE = (
    "ycck_fractional_3x2_8x8.jpg",
    "bc0b7c83c57fbe53482742cc9f5b7006c0923a09d370453571a23d33cad29e7b",
)
DEFAULT_PREFIX = ROOT / "target/benchmark-oracle/3.2.0/install"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify(data: bytes, expected_sha256: str, asset_name: str) -> None:
    actual = sha256(data)
    if actual != expected_sha256:
        raise SystemExit(
            f"{asset_name} differs from pinned libjpeg-turbo output: {actual}"
        )


def jpeg_marker(marker: int, payload: bytes) -> bytes:
    length = len(payload) + 2
    return b"\xff" + bytes((marker,)) + length.to_bytes(2, "big") + payload


def generate_fractional_fixture() -> bytes:
    """Build one valid 8x8 YCCK MCU whose 3:2 chroma ratio Pillow rejects."""
    adobe = jpeg_marker(0xEE, b"Adobe\x00\x64\x00\x00\x00\x00\x02")
    quantization = jpeg_marker(0xDB, b"\x00" + b"\x01" * 64)
    frame_payload = (
        bytes((8,))
        + b"\x00\x08\x00\x08\x04"
        + bytes((1, 0x31, 0, 2, 0x21, 0, 3, 0x21, 0, 4, 0x31, 0))
    )
    frame = jpeg_marker(0xC0, frame_payload)
    huffman_payload = (
        bytes((0, 1))
        + bytes(15)
        + bytes((0,))
        + bytes((0x10, 1))
        + bytes(15)
        + bytes((0,))
    )
    huffman = jpeg_marker(0xC4, huffman_payload)
    scan = jpeg_marker(0xDA, bytes((4, 1, 0, 2, 0, 3, 0, 4, 0, 0, 63, 0)))
    # Ten blocks each encode DC category zero and EOB, followed by pad bits.
    entropy = b"\x00\x00\x0f"
    return b"\xff\xd8" + adobe + quantization + frame + huffman + scan + entropy + b"\xff\xd9"


def generate(prefix: Path) -> dict[str, bytes]:
    include_dir = prefix / "include"
    library_dir = prefix / "lib"
    if not (include_dir / "jpeglib.h").is_file():
        raise SystemExit(f"libjpeg-turbo headers not found under {include_dir}")
    if not library_dir.is_dir():
        raise SystemExit(f"libjpeg-turbo library directory not found: {library_dir}")

    compiler = os.environ.get("CC", "cc")
    with tempfile.TemporaryDirectory(prefix="jpeg-ycck-") as temporary_name:
        temporary = Path(temporary_name)
        executable = temporary / "generate-ycck"
        subprocess.run(
            [
                compiler,
                "-std=c99",
                "-I",
                str(include_dir),
                str(SOURCE),
                "-L",
                str(library_dir),
                f"-Wl,-rpath,{library_dir}",
                "-ljpeg",
                "-o",
                str(executable),
            ],
            check=True,
        )
        environment = os.environ.copy()
        loader_path = "DYLD_LIBRARY_PATH" if platform.system() == "Darwin" else "LD_LIBRARY_PATH"
        environment[loader_path] = os.pathsep.join(
            [str(library_dir), environment.get(loader_path, "")]
        ).rstrip(os.pathsep)
        generated = {}
        for asset_name, (arguments, expected_sha256) in FIXTURES.items():
            image = temporary / asset_name
            command = [str(executable), str(image), *arguments]
            subprocess.run(command, check=True, env=environment)
            data = image.read_bytes()
            verify(data, expected_sha256, asset_name)
            generated[asset_name] = data
    fractional_name, fractional_sha256 = FRACTIONAL_FIXTURE
    fractional_data = generate_fractional_fixture()
    verify(fractional_data, fractional_sha256, fractional_name)
    generated[fractional_name] = fractional_data
    return generated


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="verify the checked-in fixtures")
    mode.add_argument("--generate", action="store_true", help="regenerate them with pinned libjpeg-turbo")
    parser.add_argument(
        "--prefix",
        type=Path,
        default=DEFAULT_PREFIX,
        help="installed libjpeg-turbo 3.2.0 prefix (default: %(default)s)",
    )
    args = parser.parse_args()

    if args.check:
        for asset_name, (_, expected_sha256) in FIXTURES.items():
            asset = ASSET_DIR / asset_name
            try:
                verify(asset.read_bytes(), expected_sha256, asset_name)
            except OSError as error:
                raise SystemExit(f"cannot read YCCK fixture {asset}: {error}") from error
            print(f"verified {asset.relative_to(ROOT)} ({expected_sha256})")
        fractional_name, fractional_sha256 = FRACTIONAL_FIXTURE
        asset = ASSET_DIR / fractional_name
        try:
            verify(asset.read_bytes(), fractional_sha256, fractional_name)
        except OSError as error:
            raise SystemExit(f"cannot read YCCK fixture {asset}: {error}") from error
        if asset.read_bytes() != generate_fractional_fixture():
            raise SystemExit(f"{fractional_name} differs from its deterministic JPEG source")
        print(f"verified {asset.relative_to(ROOT)} ({fractional_sha256})")
        return 0

    generated = generate(args.prefix)
    ASSET_DIR.mkdir(parents=True, exist_ok=True)
    for asset_name, data in generated.items():
        asset = ASSET_DIR / asset_name
        with tempfile.NamedTemporaryFile(dir=ASSET_DIR, delete=False) as temporary:
            temporary.write(data)
            generated_path = Path(temporary.name)
        generated_path.replace(asset)
        print(f"generated {asset.relative_to(ROOT)} ({sha256(data)})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
