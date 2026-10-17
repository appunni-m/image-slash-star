#!/usr/bin/env python3
"""Generate animated WebP files with invalid top-level image chunks.

The pinned Pillow decoder rejects these complete RIFF files, while earlier
versions of this crate accepted them and ignored the top-level images. The
generator copies a full-canvas VP8L payload from an ANMF frame in a known-good
animated fixture and inserts one or two copies before its ANIM chunk.
"""

from __future__ import annotations

import argparse
import hashlib
import struct
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_DIR = ROOT / "tests/fixtures/input/images/webp"
SOURCE = FIXTURE_DIR / "animated_rgb_palette_background.webp"
PINNED_PILLOW_VERSION = "12.2.0"
SOURCE_SHA256 = "f337a5980ca138148f59629627f18a064618fb6a662bad9f67ccd2d27cbb8c07"
OUTPUTS = {
    "animated_top_level_image_chunk.webp": (
        1,
        "2abd3b51baceb8e253c4f0034f99f34003fa21b999dc529a3a7966024e5fa822",
    ),
    "animated_duplicate_top_level_image_chunks.webp": (
        2,
        "b551de657d40fce5008c79624caacda9986f6ebc782a69069e357d732a328e09",
    ),
}


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of ``data``."""
    return hashlib.sha256(data).hexdigest()


def read_chunks(data: bytes, start: int, end: int) -> list[tuple[bytes, bytes]]:
    """Read complete RIFF chunks from ``start`` through the exact extent."""
    chunks = []
    position = start
    while position < end:
        header_end = position + 8
        if header_end > end:
            raise ValueError("RIFF extent ends inside a chunk header")
        kind = data[position : position + 4]
        payload_length = struct.unpack_from("<I", data, position + 4)[0]
        payload_end = header_end + payload_length
        chunk_end = payload_end + (payload_length & 1)
        if chunk_end > end:
            raise ValueError("RIFF chunk extends past its declared extent")
        if payload_length & 1 and data[payload_end] != 0:
            raise ValueError("RIFF chunk has a nonzero padding byte")
        chunks.append((kind, data[header_end:payload_end]))
        position = chunk_end
    return chunks


def pack_chunks(chunks: list[tuple[bytes, bytes]]) -> bytes:
    """Build one complete RIFF WebP file from its ordered chunks."""
    payload = bytearray(b"WEBP")
    for kind, data in chunks:
        payload.extend(kind)
        payload.extend(struct.pack("<I", len(data)))
        payload.extend(data)
        if len(data) & 1:
            payload.append(0)
    return b"RIFF" + struct.pack("<I", len(payload)) + payload


def frame_vp8l_payload(frame: bytes) -> bytes:
    """Find the first bounded VP8L chunk inside a complete ANMF payload."""
    frame_header_size = 16
    if len(frame) < frame_header_size:
        raise ValueError("source ANMF frame is shorter than its fixed header")
    nested_chunks = read_chunks(frame, frame_header_size, len(frame))
    for kind, payload in nested_chunks:
        if kind == b"VP8L":
            if len(payload) < 5 or payload[0] != 0x2F:
                raise ValueError("source ANMF VP8L payload has an invalid header")
            return payload
    raise ValueError("source ANMF frame has no VP8L image chunk")


def generated_fixtures() -> dict[str, bytes]:
    """Create both pinned full-file mutations from the animated source."""
    source = SOURCE.read_bytes()
    source_digest = sha256(source)
    if source_digest != SOURCE_SHA256:
        raise RuntimeError(
            f"source SHA-256 mismatch: expected {SOURCE_SHA256}, found {source_digest}"
        )
    if len(source) < 12 or source[:4] != b"RIFF" or source[8:12] != b"WEBP":
        raise ValueError("source is not a complete RIFF WebP file")
    riff_end = 8 + struct.unpack_from("<I", source, 4)[0]
    if riff_end != len(source):
        raise ValueError("source RIFF size does not match the complete file extent")

    chunks = read_chunks(source, 12, riff_end)
    if not chunks or chunks[0][0] != b"VP8X" or len(chunks[0][1]) != 10:
        raise ValueError("source must begin with one complete VP8X chunk")
    vp8x = chunks[0][1]
    if vp8x[0] & 0x02 == 0:
        raise ValueError("source VP8X header must declare animation")
    if any(kind in (b"VP8 ", b"VP8L") for kind, _ in chunks):
        raise ValueError("source already contains a top-level image chunk")

    animation_chunks = [payload for kind, payload in chunks if kind == b"ANIM"]
    frames = [payload for kind, payload in chunks if kind == b"ANMF"]
    if len(animation_chunks) != 1 or not frames:
        raise ValueError("source must contain one ANIM control and at least one ANMF")
    image_payload = frame_vp8l_payload(frames[0])

    image_header = int.from_bytes(image_payload[1:5], "little")
    image_size = (
        (image_header & 0x3FFF) + 1,
        ((image_header >> 14) & 0x3FFF) + 1,
    )
    canvas_size = (
        int.from_bytes(vp8x[4:7], "little") + 1,
        int.from_bytes(vp8x[7:10], "little") + 1,
    )
    if image_size != canvas_size:
        raise ValueError("copied VP8L payload does not match the animated canvas")

    output = {}
    for filename, (copy_count, expected_digest) in OUTPUTS.items():
        mutated = []
        inserted = False
        for kind, payload in chunks:
            if kind == b"ANIM":
                mutated.extend([(b"VP8L", image_payload)] * copy_count)
                inserted = True
            mutated.append((kind, payload))
        if not inserted:
            raise ValueError("source ANIM control chunk disappeared")
        encoded = pack_chunks(mutated)
        digest = sha256(encoded)
        if digest != expected_digest:
            raise RuntimeError(
                f"{filename} SHA-256 mismatch: expected {expected_digest}, found {digest}"
            )
        output[filename] = encoded
    return output


def verify_pillow_rejects(fixtures: dict[str, bytes]) -> None:
    """Require the pinned public Pillow decoder error for every mutation."""
    from PIL import Image, __version__ as pillow_version

    if pillow_version != PINNED_PILLOW_VERSION:
        raise RuntimeError(
            f"Pillow {PINNED_PILLOW_VERSION} is required, found {pillow_version}"
        )
    for filename, encoded in fixtures.items():
        try:
            with Image.open(BytesIO(encoded)) as image:
                image.load()
        except OSError as error:
            if str(error) != "could not create decoder object":
                raise RuntimeError(
                    f"{filename} produced an unexpected Pillow error: {error}"
                ) from error
        else:
            raise RuntimeError(f"Pillow unexpectedly accepted {filename}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify committed files against the deterministic mutations",
    )
    args = parser.parse_args()

    fixtures = generated_fixtures()
    if args.check:
        for filename, encoded in fixtures.items():
            path = FIXTURE_DIR / filename
            if not path.is_file() or path.read_bytes() != encoded:
                raise RuntimeError(f"{path.relative_to(ROOT)} differs from generated bytes")
            print(f"Fixture matches: {path.relative_to(ROOT)}")
        return 0

    verify_pillow_rejects(fixtures)
    for filename, encoded in fixtures.items():
        path = FIXTURE_DIR / filename
        path.write_bytes(encoded)
        print(f"Wrote {path.relative_to(ROOT)} ({len(encoded)} bytes, SHA-256 {sha256(encoded)})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
