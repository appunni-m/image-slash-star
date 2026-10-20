#!/usr/bin/env python3
"""Generate an animated WebP with origin-aligned partial frames."""

from __future__ import annotations

import argparse
import hashlib
import struct
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_DIR = ROOT / "tests/fixtures/input/images/webp"
SOURCE = FIXTURE_DIR / "animated_alpha.webp"
OUTPUT = FIXTURE_DIR / "animated_alpha_origin_partial.webp"
PINNED_PILLOW_VERSION = "12.2.0"
SOURCE_SHA256 = "f6b693d96ef83fd150ffe71ee826a4063b2a1f28a5d95eb67ad0bd1533e6d069"
OUTPUT_SHA256 = "a926d873715928a5226a50d71f4a95e178fc260793dd00ed5db834b7f8055ba0"
EXPECTED_FRAME_SHA256 = (
    "1ae35a26833de1fb20e07c4a4f3bad826f323b542572890e4f4d706067f77137",
    "98f57f8eba3238b5a332d005f9f8b46508f636f2126a9c4c2bb5b44da714e386",
)


def sha256(data: bytes) -> str:
    """Return a lowercase SHA-256 digest."""
    return hashlib.sha256(data).hexdigest()


def animated_frame_payloads(data: bytes) -> list[tuple[int, bytes]]:
    """Return the payload offsets and bytes for bounded top-level ANMF chunks."""
    if len(data) < 12 or data[:4] != b"RIFF" or data[8:12] != b"WEBP":
        raise ValueError("source is not a complete RIFF WebP file")
    riff_end = 8 + struct.unpack_from("<I", data, 4)[0]
    if riff_end != len(data):
        raise ValueError("source RIFF size does not match the complete file extent")

    frames = []
    position = 12
    while position < riff_end:
        header_end = position + 8
        if header_end > riff_end:
            raise ValueError("RIFF extent ends inside a chunk header")
        kind = data[position : position + 4]
        payload_length = struct.unpack_from("<I", data, position + 4)[0]
        payload_start = header_end
        payload_end = payload_start + payload_length
        chunk_end = payload_end + (payload_length & 1)
        if chunk_end > riff_end:
            raise ValueError("RIFF chunk extends past its declared extent")
        if payload_length & 1 and data[payload_end] != 0:
            raise ValueError("RIFF chunk has a nonzero padding byte")
        if kind == b"ANMF":
            if payload_length < 16:
                raise ValueError("ANMF frame is shorter than its fixed header")
            frames.append((payload_start, data[payload_start:payload_end]))
        position = chunk_end
    return frames


def generated_fixture() -> bytes:
    """Change only the two ANMF origin fields in the pinned source file."""
    source = SOURCE.read_bytes()
    source_digest = sha256(source)
    if source_digest != SOURCE_SHA256:
        raise RuntimeError(
            f"source SHA-256 mismatch: expected {SOURCE_SHA256}, found {source_digest}"
        )

    frames = animated_frame_payloads(source)
    if len(frames) != 2:
        raise ValueError(f"source must contain two ANMF frames, found {len(frames)}")
    expected_origins = ((4, 4), (16, 16))
    observed_origins = tuple(
        (
            int.from_bytes(payload[0:3], "little"),
            int.from_bytes(payload[3:6], "little"),
        )
        for _, payload in frames
    )
    if observed_origins != expected_origins:
        raise ValueError(
            f"source ANMF origins differ: expected {expected_origins}, "
            f"found {observed_origins}"
        )

    mutated = bytearray(source)
    expected_new_origins = ((0, 0), (0, 2))
    changed_offsets = set()
    for (payload_start, payload), (x, y) in zip(
        frames, expected_new_origins, strict=True
    ):
        for offset, value in enumerate(x.to_bytes(3, "little")):
            mutated[payload_start + offset] = value
            changed_offsets.add(payload_start + offset)
        for offset, value in enumerate(y.to_bytes(3, "little"), start=3):
            mutated[payload_start + offset] = value
            changed_offsets.add(payload_start + offset)

    actual_changed_offsets = {
        index for index, (before, after) in enumerate(zip(source, mutated, strict=True))
        if before != after
    }
    if not actual_changed_offsets.issubset(changed_offsets):
        raise RuntimeError("generated WebP changed bytes outside the ANMF origins")

    encoded = bytes(mutated)
    digest = sha256(encoded)
    if digest != OUTPUT_SHA256:
        raise RuntimeError(
            f"output SHA-256 mismatch: expected {OUTPUT_SHA256}, found {digest}"
        )
    return encoded


def verify_pillow_pixels(encoded: bytes) -> None:
    """Require pinned Pillow to accept and decode both composited frames."""
    from PIL import Image, __version__ as pillow_version

    if pillow_version != PINNED_PILLOW_VERSION:
        raise RuntimeError(
            f"Pillow {PINNED_PILLOW_VERSION} is required, found {pillow_version}"
        )
    with Image.open(BytesIO(encoded)) as image:
        image.verify()
    with Image.open(BytesIO(encoded)) as image:
        if image.format != "WEBP" or image.mode != "RGBA" or image.size != (64, 64):
            raise RuntimeError("Pillow returned unexpected WebP frame metadata")
        if image.n_frames != len(EXPECTED_FRAME_SHA256):
            raise RuntimeError(f"Pillow returned {image.n_frames} frames instead of two")
        for index, expected_digest in enumerate(EXPECTED_FRAME_SHA256):
            image.seek(index)
            image.load()
            digest = sha256(image.tobytes())
            if digest != expected_digest:
                raise RuntimeError(
                    f"Pillow frame {index} SHA-256 mismatch: "
                    f"expected {expected_digest}, found {digest}"
                )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed fixture against the deterministic mutation",
    )
    args = parser.parse_args()

    encoded = generated_fixture()
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != encoded:
            raise RuntimeError(f"{OUTPUT.relative_to(ROOT)} differs from generated bytes")
        print(f"Fixture matches: {OUTPUT.relative_to(ROOT)}")
        return 0

    verify_pillow_pixels(encoded)
    OUTPUT.write_bytes(encoded)
    print(
        f"Wrote {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes, "
        f"SHA-256 {sha256(encoded)})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
