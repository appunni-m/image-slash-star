#!/usr/bin/env python3
"""Generate a pinned AVIF parity case with active 8-bit I422 inter deblocking."""

from __future__ import annotations

import argparse
from hashlib import sha256 as sha256_digest
import subprocess
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from inspect_av1_obus import inspect as inspect_obus


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "animated_lossy_inter_i422_loopfilter_active_16x16.avif"
)
EXPECTED_FILE_SHA256 = (
    "757c9fc631ecee8522b14e66fafc6aebcbe90aa2bd5a3cc2d27df472e7440481"
)
EXPECTED_FRAME_SHA256 = (
    "a9e8b2a936ec307ba60ddb1eeac75f490f85044714b087d83f9a83cd753d844a",
    "3954cfa78f5feff23ff944cef45d2fbc1533ebe570a667b32f449a9de1cf5dbf",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
PINNED_ENCODER_VERSION = "Version: 1.4.1 (aom [enc]:v3.13.2)"


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of bytes."""
    return sha256_digest(data).hexdigest()


def check_pinned_oracle() -> None:
    """Require the exact Pillow AVIF decoder used for parity references."""
    observed = (Image.__version__, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned Pillow AVIF oracle required; found {observed!r}")


def y4m_sequence() -> bytes:
    """Build a deterministic two-frame, 16x16, 8-bit I422 Y4M sequence."""
    sequence = bytearray(
        b"YUV4MPEG2 W16 H16 F10:1 Ip A1:1 C422 XYSCSS=422 XCOLORRANGE=FULL\n"
    )
    for frame in range(2):
        sequence.extend(b"FRAME\n")
        for y in range(16):
            for x in range(16):
                delta = 13 if frame and (x + 2 * y) % 5 == 0 else 0
                sequence.append((63 + 9 * x + 7 * y + delta) & 0xFF)
        for base, x_step, y_step in ((82, 11, 3), (145, 7, 9)):
            for y in range(16):
                for x in range(8):
                    delta = 17 if frame and (x + y) % 4 == 0 else 0
                    sequence.append((base + x_step * x + y_step * y + delta) & 0xFF)
    return bytes(sequence)


def normalize_sequence_timestamps(encoded: bytes) -> bytes:
    """Zero the version-one BMFF creation and modification timestamps."""
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


def check_pinned_encoder(encoder: Path) -> None:
    """Require the pinned fixture-only libavif and libaom producer."""
    result = subprocess.run(
        [str(encoder), "--version"], capture_output=True, text=True, check=True
    )
    first_line = result.stdout.splitlines()[0].strip() if result.stdout else ""
    if first_line != PINNED_ENCODER_VERSION:
        raise RuntimeError(
            f"pinned AVIF fixture encoder required; found {first_line!r}"
        )


def encode_once(encoder: Path) -> bytes:
    """Encode one fixed 16x16 I422 sequence with active inter loop filtering."""
    staging = ROOT / "target" / "oracle-staging"
    staging.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="avif-i422-inter-loopfilter-", dir=staging
    ) as directory:
        temporary = Path(directory)
        source = temporary / "sequence.y4m"
        output = temporary / "sequence.avif"
        source.write_bytes(y4m_sequence())
        command = [
            str(encoder),
            "-s",
            "0",
            "-j",
            "1",
            "-q",
            "70",
            "-a",
            "min-partition-size=16",
            "-a",
            "max-partition-size=16",
            "-a",
            "enable-tx-size-search=0",
            "-a",
            "aq-mode=0",
            "-a",
            "deltaq-mode=0",
            "-a",
            "enable-cdef=0",
            "-a",
            "enable-restoration=0",
            "-a",
            "loopfilter-control=1",
            "-a",
            "enable-global-motion=0",
            "-a",
            "enable-warped-motion=0",
            "-a",
            "enable-ref-frame-mvs=0",
            "-a",
            "enable-masked-comp=0",
            "-a",
            "enable-dist-wtd-comp=0",
            str(source),
            str(output),
        ]
        subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=True)
        return normalize_sequence_timestamps(output.read_bytes())


def validate_pixels(encoded: bytes) -> None:
    """Require exact Pillow metadata, two durations, and both RGB frames."""
    with Image.open(BytesIO(encoded)) as image:
        if image.format != "AVIF" or image.mode != "RGB" or image.size != (16, 16):
            raise RuntimeError("Pillow AVIF metadata differs from the pinned fixture")
        if image.n_frames != len(EXPECTED_FRAME_SHA256):
            raise RuntimeError(f"expected two frames, found {image.n_frames}")
        for frame_index, expected_hash in enumerate(EXPECTED_FRAME_SHA256):
            image.seek(frame_index)
            image.load()
            pixels = image.tobytes()
            if (
                len(pixels) != 16 * 16 * 3
                or sha256(pixels) != expected_hash
                or image.info.get("duration") != 100
            ):
                raise RuntimeError(
                    f"Pillow frame {frame_index} differs from its reference"
                )


def validate_frame_headers(encoded: bytes) -> None:
    """Require a lossy I422 inter frame with nonzero Y/U/V loop-filter levels."""
    staging = ROOT / "target" / "oracle-staging"
    staging.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="avif-i422-inter-loopfilter-header-", dir=staging
    ) as directory:
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
        headers = [
            obu["frame_header"]
            for obu in sample["obus"]
            if "frame_header" in obu
        ]
        if len(headers) != 1:
            raise RuntimeError(
                "each AV1 picture sample must contain exactly one frame header"
            )
        frame_headers.extend(headers)

    if not sequence_headers:
        raise RuntimeError("AV1 picture samples must declare a sequence header")
    sequence = sequence_headers[0]
    if (
        any(header != sequence for header in sequence_headers[1:])
        or sequence.get("profile") != 2
        or sequence.get("bit_depth") != 8
        or sequence.get("monochrome") is not False
        or sequence.get("subsampling_x") != 1
        or sequence.get("subsampling_y") != 0
        or sequence.get("enable_cdef") is not False
        or sequence.get("enable_restoration") is not False
        or sequence.get("enable_ref_frame_mvs") is not False
        or sequence.get("enable_masked_compound") is not False
        or sequence.get("enable_jnt_comp") is not False
    ):
        raise RuntimeError("sequence header differs from the pinned 8-bit I422 profile")

    key, inter = frame_headers
    loop_filter = inter.get("loop_filter", {})
    if (
        key.get("frame_type") != "key"
        or inter.get("frame_type") != "inter"
        or inter.get("frame_width") != 16
        or inter.get("frame_height") != 16
        or inter.get("transform_mode") != "largest"
        or inter.get("all_lossless") is not False
        or inter.get("use_ref_frame_mvs") is not False
        or inter.get("quantization", {}).get("base", 0) <= 0
        or not any(loop_filter.get("level_y", []))
        or loop_filter.get("level_u", 0) == 0
        or loop_filter.get("level_v", 0) == 0
        or loop_filter.get("sharpness", 8) > 7
    ):
        raise RuntimeError(
            "inter frame differs from the active-loop-filter I422 profile"
        )


def validate(encoded: bytes) -> None:
    """Check immutable file identity, AV1 syntax, and Pillow pixel parity."""
    digest = sha256(encoded)
    if digest != EXPECTED_FILE_SHA256:
        raise RuntimeError(f"fixture hash changed: {digest}")
    validate_frame_headers(encoded)
    validate_pixels(encoded)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--avifenc", type=Path, help="fixture-only pinned AVIF encoder")
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed fixture with the pinned Pillow oracle",
    )
    args = parser.parse_args()
    check_pinned_oracle()

    output = args.output.resolve()
    if args.check:
        if not output.is_file():
            raise RuntimeError(f"fixture does not exist: {output}")
        validate(output.read_bytes())
        print(f"Verified {output} (sha256={EXPECTED_FILE_SHA256})")
        return

    if args.avifenc is None:
        parser.error("--avifenc is required when generating a fixture")
    encoder = args.avifenc.expanduser().resolve(strict=True)
    check_pinned_encoder(encoder)
    encoded = encode_once(encoder)
    if encode_once(encoder) != encoded:
        raise RuntimeError("repeated AVIF encodes differ")
    validate(encoded)
    if output.exists() and output.read_bytes() != encoded:
        raise RuntimeError(f"refusing to replace differing fixture: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded)
    print(f"Wrote {output} ({len(encoded)} bytes, sha256={EXPECTED_FILE_SHA256})")


if __name__ == "__main__":
    main()
