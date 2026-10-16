#!/usr/bin/env python3
"""Generate a pinned animated I444 AV1 frame-header AVIF witness."""

from __future__ import annotations

import hashlib
import argparse
import tempfile
from io import BytesIO
from pathlib import Path

from inspect_av1_obus import inspect as inspect_av1


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "animated_lossy_inter_i444_tx_largest_64x64.avif"
)
EXPECTED_FILE_SHA256 = "270ab1fb4f9e066d6898c52bdd8d94911502684d13eb7cdc62d1c034a57148ea"
EXPECTED_FRAME_SHA256 = (
    "a96ff730a3f0be2c2dc4c20528db0eea3839c1f868a0afd274e66e7ff072077d",
    "17befb70fc9c8f3d64649426f610b397e62d6507ab56c87cd82f67fffd232ca9",
    "5bef112c430f60a8dfd564bb4dac8dd67e209f9b44d763fbb563de80cae6b23c",
)


def sha256(data: bytes) -> str:
    """Return a lowercase SHA-256 digest."""

    return hashlib.sha256(data).hexdigest()


def normalize_sequence_timestamps(encoded: bytes) -> bytes:
    """Zero version-one BMFF timestamps to make Pillow output reproducible."""
    normalized = bytearray(encoded)
    for box_type in (b"mvhd", b"tkhd", b"mdhd"):
        offset = normalized.find(box_type)
        if offset < 4 or normalized[offset + 4] != 1:
            raise RuntimeError(f"expected a version-one {box_type!r} box")
        if normalized.find(box_type, offset + 4) != -1:
            raise RuntimeError(f"found duplicate {box_type!r} boxes")
        normalized[offset + 8 : offset + 24] = bytes(16)
    return bytes(normalized)


def make_frames() -> tuple[Image.Image, Image.Image, Image.Image]:
    """Build a smooth 64px key image, a changed patch, and their half blend."""
    from PIL import Image

    width = height = 64
    base = Image.new("RGB", (width, height))
    base.putdata(
        [
            (
                30 + 2 * x + ((x // 8) % 2) * 5,
                40 + 2 * y + ((y // 8) % 2) * 7,
                60 + x + y + (((x // 16) + (y // 16)) % 2) * 8,
            )
            for y in range(height)
            for x in range(width)
        ]
    )
    changed = base.copy()
    for y in range(16, 48):
        for x in range(16, 48):
            changed.putpixel((x, y), (230, 35, 80))
    return base, changed, Image.blend(base, changed, 0.5)


def encode_fixture() -> bytes:
    """Encode a three-frame, full-frame I444 sequence with pinned libaom."""
    frames = make_frames()
    output = BytesIO()
    frames[0].save(
        output,
        format="AVIF",
        save_all=True,
        append_images=list(frames[1:]),
        duration=[100] * len(frames),
        loop=0,
        quality=50,
        speed=0,
        max_threads=1,
        subsampling="4:4:4",
        autotiling=False,
        codec="aom",
        advanced={
            "sb-size": "64",
            "min-partition-size": "64",
            "max-partition-size": "64",
            "aq-mode": "0",
            "deltaq-mode": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "enable-warped-motion": "0",
            "enable-global-motion": "0",
        },
    )
    return normalize_sequence_timestamps(output.getvalue())


def pillow_frame_hashes(encoded: bytes) -> tuple[str, ...]:
    """Hash every exact frame rendered by the pinned Pillow oracle."""
    from PIL import Image

    with Image.open(BytesIO(encoded)) as sequence:
        if sequence.n_frames != len(EXPECTED_FRAME_SHA256):
            raise RuntimeError(f"expected three frames, found {sequence.n_frames}")
        hashes = []
        for frame_index in range(sequence.n_frames):
            sequence.seek(frame_index)
            sequence.load()
            if sequence.mode != "RGB" or sequence.size != (64, 64):
                raise RuntimeError("Pillow frame mode or dimensions changed")
            hashes.append(sha256(sequence.tobytes()))
    return tuple(hashes)


def verify_frame_header(encoded: bytes) -> None:
    """Check the sequence format and final AV1 frame-header declarations."""
    with tempfile.NamedTemporaryFile(suffix=".avif") as temporary:
        temporary.write(encoded)
        temporary.flush()
        report = inspect_av1(Path(temporary.name))

    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if not sequence_headers or any(
        header["max_width"] != 64
        or header["max_height"] != 64
        or header["bit_depth"] != 8
        or header["monochrome"]
        or header["subsampling_x"]
        or header["subsampling_y"]
        for header in sequence_headers
    ):
        raise RuntimeError("encoded sequence is not 8-bit full-resolution I444")

    frame_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    final_frames = [header for header in frame_headers if header["order_hint"] == 2]
    if len(final_frames) != 1:
        raise RuntimeError("expected one final order-hint-2 frame header")
    final_frame, = final_frames
    if (
        final_frame["frame_type"] != "inter"
        or final_frame["transform_mode"] != "largest"
        or final_frame["reference_mode"] != "select"
        or final_frame["quantization"]["base"] <= 0
    ):
        raise RuntimeError("final frame header no longer signals the pinned modes")


def verify_existing_fixture() -> None:
    """Check the committed byte pin and AV1 topology without Pillow."""
    encoded = OUTPUT.read_bytes()
    if sha256(encoded) != EXPECTED_FILE_SHA256:
        raise RuntimeError("animated I444 AVIF fixture differs from its pin")
    verify_frame_header(encoded)
    print(f"Verified {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes)")


def main() -> None:
    """Generate the input, or check its committed pin without writing it."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the tracked fixture without requiring Pillow or writing it",
    )
    args = parser.parse_args()

    if args.check:
        verify_existing_fixture()
        return

    from PIL import _avif, features
    from PIL import __version__ as PILLOW_VERSION

    if (
        PILLOW_VERSION != "12.2.0"
        or features.version("avif") != "1.4.1"
        or _avif.codec_versions()
        != "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
    ):
        raise RuntimeError("the pinned Pillow 12.2.0 AVIF oracle is required")

    encoded = encode_fixture()
    if encoded != encode_fixture():
        raise RuntimeError("animated I444 AVIF fixture is not deterministic")
    if sha256(encoded) != EXPECTED_FILE_SHA256:
        raise RuntimeError("animated I444 AVIF fixture differs from its pin")
    if pillow_frame_hashes(encoded) != EXPECTED_FRAME_SHA256:
        raise RuntimeError("animated I444 Pillow frames differ from their pins")
    verify_frame_header(encoded)

    if OUTPUT.exists():
        if OUTPUT.read_bytes() != encoded:
            raise RuntimeError("existing fixture differs from generated bytes; refusing to overwrite")
        print(f"Verified {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes)")
    else:
        OUTPUT.write_bytes(encoded)
        print(f"Wrote {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes)")


if __name__ == "__main__":
    main()
