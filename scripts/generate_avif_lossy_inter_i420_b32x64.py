#!/usr/bin/env python3
"""Generate AV1 B32x64 inter-transform parity fixtures."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features


ROOT = Path(__file__).resolve().parent.parent
FIXTURES = {
    "deep": {
        "asset": (
            ROOT
            / "tests"
            / "fixtures"
            / "input"
            / "images"
            / "avif"
            / "animated_lossy_inter_420_deep_b32x64_64x64.avif"
        ),
        "file_sha256": "a64648f0a78b83b679a0a09448688be51e4c933b84a0215b6af8a766b52e2e8b",
        "frame_sha256": (
            "7c6e789ad7e137cb985bd76bbed90d70c0b3fc14806c7af2aa50ff3dcc6de1b1",
            "88554b895fddbd75230a4895bf84e54ca70ca44a1e8bfe0c111ef139f58f2f00",
        ),
    },
    "topology": {
        "asset": (
            ROOT
            / "tests"
            / "fixtures"
            / "input"
            / "images"
            / "avif"
            / "animated_lossy_inter_420_topology_b32x64_64x64.avif"
        ),
        "file_sha256": "71e64835c2d78a25599a5b195c9a819dba8b0f61ac63136a64af2932f7bdddd6",
        "frame_sha256": (
            "04f3811de5095024766a79ef50cb79cffe1dd52c6fd901640188f1a4da68ba40",
            "da0f604ab1db4dcbdf9f479586ec6414818da0fc4ec9beba7388c51606d70eee",
        ),
    },
}
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def check_pinned_oracle() -> None:
    """Require the Pillow AVIF decoder used for the recorded pixel references."""
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow AVIF oracle required; found {observed!r}")


def normalize_sequence_timestamps(encoded: bytes) -> bytes:
    """Zero version-one BMFF creation and modification timestamps."""
    normalized = bytearray(encoded)
    for box_type in (b"mvhd", b"tkhd", b"mdhd"):
        offset = normalized.find(box_type)
        if offset < 4 or normalized[offset + 4] != 1:
            raise RuntimeError(f"expected a version-one {box_type!r} box")
        if normalized.find(box_type, offset + 4) != -1:
            raise RuntimeError(f"found duplicate {box_type!r} boxes")
        end = offset + 24
        if end > len(normalized):
            raise RuntimeError(f"truncated {box_type!r} box")
        normalized[offset + 8 : end] = bytes(16)
    return bytes(normalized)


def observe_pillow_frames(
    encoded: bytes, expected_frame_sha256: tuple[str, ...]
) -> tuple[str, ...]:
    """Return exact RGB frame hashes after checking sequence metadata."""
    frame_hashes = []
    with Image.open(BytesIO(encoded)) as decoded:
        if (
            decoded.format != "AVIF"
            or decoded.mode != "RGB"
            or decoded.size != (64, 64)
            or decoded.n_frames != len(expected_frame_sha256)
        ):
            raise RuntimeError("Pillow AVIF sequence metadata differs from the fixture")
        for frame_index in range(decoded.n_frames):
            decoded.seek(frame_index)
            decoded.load()
            pixels = decoded.convert("RGB").tobytes()
            frame_hashes.append(sha256(pixels))
    observed = tuple(frame_hashes)
    if observed != expected_frame_sha256:
        raise RuntimeError(f"Pillow frame hashes changed: {observed!r}")
    return observed


def validate_fixture(encoded: bytes, fixture: dict[str, object]) -> None:
    observed = sha256(encoded)
    if observed != fixture["file_sha256"]:
        raise RuntimeError(f"fixture hash changed: {observed}")
    observe_pillow_frames(encoded, fixture["frame_sha256"])


def encode_fixture(encoder: Path, profile: str, fixture: dict[str, object]) -> bytes:
    """Run the pinned fixture encoder twice and require byte stability."""
    encoded_runs = []
    with tempfile.TemporaryDirectory(prefix=f"avif-i420-{profile}-b32x64-") as temporary:
        directory = Path(temporary)
        for run_index in range(2):
            output = directory / f"frame-sequence-{run_index}.avif"
            result = subprocess.run(
                [str(encoder), profile, str(output)],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
            )
            if result.returncode != 0:
                raise RuntimeError(
                    f"fixture encoder failed with status {result.returncode}:\n"
                    f"{result.stdout}"
                )
            encoded_runs.append(normalize_sequence_timestamps(output.read_bytes()))
    if encoded_runs[0] != encoded_runs[1]:
        raise RuntimeError("repeated AVIF encodes differ")
    validate_fixture(encoded_runs[0], fixture)
    return encoded_runs[0]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, help="fixture-specific AVIF encoder")
    parser.add_argument("--profile", choices=tuple(FIXTURES), default="deep")
    parser.add_argument("--output", type=Path)
    parser.add_argument(
        "--check", action="store_true", help="verify the retained fixture without encoding"
    )
    args = parser.parse_args()
    check_pinned_oracle()
    fixture = FIXTURES[args.profile]
    output = args.output or fixture["asset"]

    if args.check:
        validate_fixture(output.read_bytes(), fixture)
        print(f"Verified {output}")
        return
    if args.encoder is None:
        parser.error("--encoder is required unless --check is used")
    if not args.encoder.is_file():
        parser.error(f"fixture encoder does not exist: {args.encoder}")

    encoded = encode_fixture(args.encoder.resolve(), args.profile, fixture)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded)
    print(f"Wrote {output} ({len(encoded)} bytes, SHA-256 {sha256(encoded)})")


if __name__ == "__main__":
    main()
