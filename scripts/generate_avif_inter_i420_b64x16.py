#!/usr/bin/env python3
"""Generate pinned AV1 HORZ_4 I420 B64x16 inter-block fixtures."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from generate_avif_10bit_lossless_inter_fixture import normalize_sequence_timestamps
from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
OUTPUTS = {
    ("largest", 8): (
        ROOT
        / "tests/fixtures/input/images/avif/animated_lossy_inter_420_b64x16_mode_largest_64x64.avif"
    ),
    ("select", 8): (
        ROOT
        / "tests/fixtures/input/images/avif/animated_lossy_inter_420_b64x16_mode_select_64x64.avif"
    ),
    ("select", 10): (
        ROOT
        / "tests/fixtures/input/images/avif/animated_lossy_inter_420_b64x16_10bit_mode_select_64x64.avif"
    ),
    ("select", 12): (
        ROOT
        / "tests/fixtures/input/images/avif/animated_lossy_inter_420_b64x16_12bit_mode_select_64x64.avif"
    ),
}
EXPECTED_FILE_SHA256 = {
    ("largest", 8): "35f16973d0bda15b2a2038102b3f646f2b04d6e07157933bd03b8b32a6452daf",
    ("select", 8): "84eab6d96f97deb02fca49e66602f9cb507d2281cde5d4b52b2a50c54cb0433a",
    ("select", 10): "0d85dc1e3e8958e7375c3dcddd048f93cc5bd7ad12f50f82f1299f1c482c1880",
    ("select", 12): "8583340a327c7feffa803fa3b0a80267a49b6fa21c062f9b19799e2a82bf1dcc",
}
EXPECTED_FRAME_SHA256 = {
    ("largest", 8): (
        "c07d369a3aeeb878a8a9a3f3cfd2306e54769eba80634c36141ab3247d87eeac",
        "2f4054a20317ef9d8c6534b7b39e8a51bd5ad1c32750723c7acabc1d1db99f06",
    ),
    ("select", 8): (
        "547f53c99d9fc578e69c7cf37a55ef187ca316ce792d1641127de6f9c7ef58fa",
        "2b11147cafdba4c5e030333f4c73ce8ae9ed29a50e3e03a5221c6fd785c6c80d",
    ),
    ("select", 10): (
        "5429ffffd03adc23e63fdd3aecda2fdda95c06007af78480d631c30a05786b0a",
        "c0502eb79c3d370e2a19a0fa094fae0cff0e9c96d76c7ebcb0f62221aff6e7c8",
    ),
    ("select", 12): (
        "0b7d8dc664c698a7e35dc27d5fae5aeff206bb9797fe01174804ea01b03e18c0",
        "411f7788b73d06b68c545fccced0d5e9e6ee667037e1d198547213d48917d24b",
    ),
}
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of bytes."""
    return hashlib.sha256(data).hexdigest()


def check_pinned_oracle() -> None:
    """Require the repository's pinned Pillow and AVIF runtime."""
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow AVIF oracle required; found {observed!r}")


def validate_pixels(encoded: bytes, mode: str, bit_depth: int) -> None:
    """Require exact pinned Pillow metadata and RGB frame pixels."""
    with Image.open(BytesIO(encoded)) as image:
        if image.format != "AVIF" or image.mode != "RGB" or image.size != (64, 64):
            raise RuntimeError("Pillow AVIF metadata differs from the pinned fixture")
        if image.n_frames != len(EXPECTED_FRAME_SHA256[(mode, bit_depth)]):
            raise RuntimeError(f"expected two frames, found {image.n_frames}")
        for frame_index, expected_hash in enumerate(
            EXPECTED_FRAME_SHA256[(mode, bit_depth)]
        ):
            image.seek(frame_index)
            image.load()
            pixels = image.tobytes()
            if (
                len(pixels) != 64 * 64 * 3
                or sha256(pixels) != expected_hash
                or image.info.get("duration") != 100
            ):
                raise RuntimeError(f"Pillow frame {frame_index} differs from its pinned reference")


def validate_frame_headers(encoded: bytes, mode: str, bit_depth: int) -> None:
    """Require a lossy key/inter I420 sequence with the requested transform mode."""
    with tempfile.TemporaryDirectory(prefix="avif-i420-b64x16-64x64-") as directory:
        path = Path(directory) / "fixture.avif"
        path.write_bytes(encoded)
        report = inspect_obus(path)
    samples = sorted(
        (sample for sample in report["samples"] if sample.get("role") == "track_pict"),
        key=lambda sample: sample["identity"]["sample"],
    )
    if len(samples) != 2:
        raise RuntimeError("AVIF must contain exactly two picture-track samples")
    sequence_headers = []
    frame_headers = []
    for sample in samples:
        sequence_headers.extend(
            obu["sequence_header"] for obu in sample["obus"] if "sequence_header" in obu
        )
        headers = [obu["frame_header"] for obu in sample["obus"] if "frame_header" in obu]
        if len(headers) != 1:
            raise RuntimeError("each AV1 picture sample must contain exactly one frame header")
        frame_headers.extend(headers)
    if not sequence_headers:
        raise RuntimeError("AV1 picture samples must declare a sequence header")
    sequence = sequence_headers[0]
    if (
        any(header != sequence for header in sequence_headers[1:])
        or sequence.get("profile") != (2 if bit_depth == 12 else 0)
        or sequence.get("bit_depth") != bit_depth
        or sequence.get("monochrome") is not False
        or sequence.get("subsampling_x") != 1
        or sequence.get("subsampling_y") != 1
    ):
        raise RuntimeError(f"sequence header differs from the pinned {bit_depth}-bit I420 profile")
    key, inter = frame_headers
    if (
        key.get("frame_type") != "key"
        or inter.get("frame_type") != "inter"
        or inter.get("frame_width") != 64
        or inter.get("frame_height") != 64
        or inter.get("transform_mode") != mode
        or inter.get("all_lossless") is not False
        or inter.get("quantization", {}).get("base", 0) <= 0
    ):
        raise RuntimeError(f"inter frame differs from the lossy I420 {mode} profile")


def encode_twice(encoder: Path, mode: str, bit_depth: int) -> bytes:
    """Encode twice and require stable AVIF bytes after timestamp normalization."""
    with tempfile.TemporaryDirectory(prefix="avif-i420-b64x16-") as directory:
        paths = [Path(directory) / f"encoded-{index}.avif" for index in range(2)]
        for path in paths:
            subprocess.run(
                [str(encoder), str(path), mode, str(bit_depth)], check=True, cwd=ROOT
            )
        encoded = [normalize_sequence_timestamps(path.read_bytes()) for path in paths]
    if encoded[0] != encoded[1]:
        raise RuntimeError(f"I420 B64x16 {mode} AVIF fixture is not deterministic")
    return encoded[0]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=sorted({mode for mode, _ in OUTPUTS}), required=True)
    parser.add_argument("--bit-depth", choices=(8, 10, 12), default=8, type=int)
    parser.add_argument("--encoder", type=Path, help="fixture-only pinned AVIF encoder")
    parser.add_argument("--output", type=Path, help="override the mode's default output path")
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed fixture without rebuilding the fixture encoder",
    )
    args = parser.parse_args()
    check_pinned_oracle()

    case = (args.mode, args.bit_depth)
    if case not in OUTPUTS:
        parser.error(f"unsupported fixture profile: {args.bit_depth}-bit {args.mode}")
    output = (args.output or OUTPUTS[case]).resolve()
    if args.check:
        if not output.is_file():
            raise RuntimeError(f"fixture does not exist: {output}")
        encoded = output.read_bytes()
    else:
        if args.encoder is None:
            parser.error("--encoder is required when generating a fixture")
        encoder = args.encoder.resolve()
        if not encoder.is_file():
            raise RuntimeError(f"fixture encoder does not exist: {encoder}")
        encoded = encode_twice(encoder, args.mode, args.bit_depth)
        if output.exists() and output.read_bytes() != encoded:
            raise RuntimeError(f"refusing to replace differing fixture: {output}")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(encoded)

    file_hash = sha256(encoded)
    if file_hash != EXPECTED_FILE_SHA256[case]:
        raise RuntimeError(f"fixture hash changed: {file_hash}")
    validate_pixels(encoded, args.mode, args.bit_depth)
    validate_frame_headers(encoded, args.mode, args.bit_depth)
    print(f"Verified {output} ({len(encoded)} bytes, sha256={file_hash})")


if __name__ == "__main__":
    main()
