#!/usr/bin/env python3
"""Generate a pinned AVIF sequence for distance-compound altref parity."""

from __future__ import annotations

import argparse
import hashlib
import random
import struct
import sys
from io import BytesIO
from pathlib import Path
from tempfile import TemporaryDirectory

from PIL import Image, _avif, features
from PIL import __version__ as PILLOW_VERSION

from generate_avif_interinter_distance_i420 import normalize_timestamps
from inspect_av1_obus import inspect as inspect_av1
from inspect_avif_bitstreams import children, inspect as inspect_avif, parse_boxes


ROOT = Path(__file__).resolve().parent.parent
FILENAME = "animated_lossy_interinter_distance_altref_i420_b16x16_64x64.avif"
OUTPUT = ROOT / "tests/fixtures/input/images/avif" / FILENAME
FILE_SHA256 = "89ffa2673b63ff0c8afc9c59ce2cc76d932b5bdbaeedeef63b7bb19dec8d927c"
FRAME_SHA256 = (
    "be95f561631303547d443f7bd4c6faeac992c19707e633d7e25e6ae94846d1eb",
    "4a8e38ecf4741d7739101d3e01f2b2374f342d667b2cd928a4a6f10f511f9725",
    "475660fd8e5acdc4e3d7cdbf83ac1f0e4c1d3ce7f4e2d22b287d67b01402e221",
    "8962e7786ddf48debbc27755831c821f964db098198848e7b2bac5920788aa83",
    "651f0a169fb1cd08a34dce4b18f105d51af00aa988ec0bdf7902d46ff3710a50",
    "06fae3e9b146cb3cfba594194d375cd91dfe56b5dc3f02ddea29df3c254f8d8d",
)
PINNED_ORACLE = (
    "12.2.0",
    "1.4.1",
    "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2",
)
WIDTH = 64
HEIGHT = 64
INPUT_FRAME_COUNT = 8
TRACK_SAMPLE_COUNT = 6
SEED = 5013


def check_pinned_oracle() -> None:
    """Require the pinned Pillow and AVIF codec versions."""
    observed = (PILLOW_VERSION, features.version("avif"), _avif.codec_versions())
    if observed != PINNED_ORACLE:
        raise RuntimeError(f"pinned AVIF oracle required; found {observed!r}")


def make_frames() -> list[Image.Image]:
    """Create eight deterministic grayscale RGB frames for the AOM lookahead."""
    frames = []
    for frame_index in range(INPUT_FRAME_COUNT):
        random_source = random.Random(SEED + frame_index)
        gray = bytes(
            random_source.randrange(256) for _ in range(WIDTH * HEIGHT)
        )
        rgb = bytes(channel for value in gray for channel in (value,) * 3)
        frames.append(Image.frombytes("RGB", (WIDTH, HEIGHT), rgb))
    return frames


def encode_full_sequence(frames: list[Image.Image]) -> bytes:
    """Encode hidden alternate references with distance compound enabled."""
    output = BytesIO()
    frames[0].save(
        output,
        format="AVIF",
        save_all=True,
        append_images=frames[1:],
        duration=[100] * len(frames),
        loop=0,
        quality=70,
        speed=0,
        max_threads=1,
        subsampling="4:2:0",
        autotiling=False,
        codec="aom",
        advanced={
            "auto-alt-ref": "1",
            "arnr-maxframes": "7",
            "arnr-strength": "5",
            "enable-dist-wtd-comp": "1",
            "enable-masked-comp": "0",
            "enable-interintra-comp": "0",
            "enable-global-motion": "0",
            "enable-warped-motion": "0",
            "enable-cdef": "0",
            "enable-restoration": "0",
            "loopfilter-control": "0",
            "sb-size": "64",
            "min-partition-size": "16",
            "max-partition-size": "16",
            "aq-mode": "0",
            "deltaq-mode": "0",
        },
    )
    return normalize_timestamps(output.getvalue())


def truncate_track_samples(encoded: bytes, sample_count: int) -> bytes:
    """Keep a valid track prefix and preserve media offsets with a free box."""
    staging = ROOT / "target" / "oracle-staging"
    staging.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory(prefix="avif-distance-altref-", dir=staging) as directory:
        full_path = Path(directory) / FILENAME
        full_path.write_bytes(encoded)
        report = inspect_avif(full_path)

    tracks = report["tracks"]
    if len(tracks) != 1 or len(tracks[0]["samples"]) != INPUT_FRAME_COUNT:
        raise RuntimeError("AOM output has an unexpected color-track sample table")
    samples = tracks[0]["samples"]
    if not 0 < sample_count < len(samples):
        raise RuntimeError("requested track prefix is outside the AOM sample table")
    final_sample = samples[sample_count - 1]
    media_end = final_sample["offset"] + final_sample["length"]

    def rebuild(box) -> bytes:
        payload = encoded[box.payload_start : box.end]
        if box.kind in (b"moov", b"trak", b"mdia", b"minf", b"stbl"):
            content = b"".join(rebuild(child) for child in children(encoded, box))
            if box.kind == b"stbl":
                padding = box.end - box.payload_start - len(content)
                if padding:
                    if padding < 8:
                        raise RuntimeError("sample-table padding cannot form a free box")
                    content += struct.pack(">I4s", padding, b"free")
                    content += bytes(padding - 8)
        elif box.kind == b"stsz":
            _, common_size, old_count = struct.unpack_from(">III", payload)
            if common_size or old_count != INPUT_FRAME_COUNT:
                raise RuntimeError("AOM sample-size table differs from the pinned layout")
            content = payload[: 12 + sample_count * 4]
            content = content[:8] + struct.pack(">I", sample_count) + content[12:]
        elif box.kind == b"stts":
            version_flags, entry_count, _, delta = struct.unpack_from(">IIII", payload)
            if entry_count != 1:
                raise RuntimeError("AOM time-to-sample table differs from the pinned layout")
            content = struct.pack(">IIII", version_flags, 1, sample_count, delta)
        elif box.kind == b"stsc":
            version_flags, entry_count, first_chunk, _, description = struct.unpack_from(
                ">IIIII", payload
            )
            if entry_count != 1 or first_chunk != 1:
                raise RuntimeError("AOM sample-to-chunk table differs from the pinned layout")
            content = struct.pack(
                ">IIIII", version_flags, 1, first_chunk, sample_count, description
            )
        else:
            content = payload
        return struct.pack(">I4s", 8 + len(content), box.kind) + content

    output = bytearray()
    boxes = parse_boxes(encoded, 0, len(encoded))
    media_boxes = [box for box in boxes if box.kind == b"mdat"]
    if len(media_boxes) != 1:
        raise RuntimeError("AOM output must contain one media-data box")
    for box in boxes:
        if box.kind == b"moov":
            output.extend(rebuild(box))
        elif box.kind == b"mdat":
            payload = encoded[box.payload_start:media_end]
            output.extend(struct.pack(">I4s", 8 + len(payload), b"mdat"))
            output.extend(payload)
        else:
            output.extend(encoded[box.start : box.end])

    result = bytes(output)
    with TemporaryDirectory(prefix="avif-distance-altref-check-", dir=staging) as directory:
        prefix_path = Path(directory) / FILENAME
        prefix_path.write_bytes(result)
        prefix = inspect_avif(prefix_path)
    if len(prefix["tracks"][0]["samples"]) != sample_count:
        raise RuntimeError("truncated AVIF sample table did not round-trip")
    return result


def encode_fixture() -> bytes:
    """Encode deterministically, then keep the six-sample decodable prefix."""
    frames = make_frames()
    full = encode_full_sequence(frames)
    if full != encode_full_sequence(frames):
        raise RuntimeError("distance-compound altref AVIF is not deterministic")
    return truncate_track_samples(full, TRACK_SAMPLE_COUNT)


def pillow_frame_hashes(encoded: bytes) -> tuple[str, ...]:
    """Check the exact six-frame Pillow RGB result and timing."""
    from PIL import Image

    with Image.open(BytesIO(encoded)) as sequence:
        if (
            sequence.format != "AVIF"
            or sequence.mode != "RGB"
            or sequence.size != (WIDTH, HEIGHT)
            or sequence.n_frames != TRACK_SAMPLE_COUNT
        ):
            raise RuntimeError("Pillow AVIF format, mode, dimensions, or frame count differ")
        hashes = []
        for index in range(sequence.n_frames):
            sequence.seek(index)
            sequence.load()
            if sequence.info.get("duration") != 100:
                raise RuntimeError("Pillow AVIF frame duration differs")
            hashes.append(hashlib.sha256(sequence.convert("RGB").tobytes()).hexdigest())
    return tuple(hashes)


def verify_av1_headers(encoded: bytes) -> None:
    """Require hidden altrefs and selectable distance-compound frames."""
    staging = ROOT / "target" / "oracle-staging"
    with TemporaryDirectory(prefix="avif-distance-altref-obu-", dir=staging) as directory:
        fixture = Path(directory) / FILENAME
        fixture.write_bytes(encoded)
        report = inspect_av1(fixture)

    sequence_headers = [
        obu["sequence_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "sequence_header" in obu
    ]
    if not sequence_headers or any(
        not header.get("enable_order_hint")
        or not header.get("enable_jnt_comp")
        or header.get("enable_masked_compound")
        or header.get("enable_warped_motion")
        for header in sequence_headers
    ):
        raise RuntimeError("AV1 sequence does not enable distance-only compound syntax")

    frame_headers = [
        obu["frame_header"]
        for sample in report["samples"]
        for obu in sample["obus"]
        if "frame_header" in obu
    ]
    hidden_inter = [
        header
        for header in frame_headers
        if header.get("frame_type") == "inter" and not header.get("show_frame")
    ]
    if len(hidden_inter) != 2 or not any(
        header.get("order_hint") == 3 and header.get("reference_mode") == "select"
        for header in hidden_inter
    ):
        raise RuntimeError("AV1 sequence does not retain the two hidden selectable altrefs")
    if not any(
        header.get("order_hint") == 2 and header.get("reference_mode") == "select"
        for header in frame_headers
    ):
        raise RuntimeError("AV1 sequence lacks the expected distance-compound display frame")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the checked-in input without writing it",
    )
    args = parser.parse_args()
    check_pinned_oracle()

    encoded = encode_fixture()
    file_hash = hashlib.sha256(encoded).hexdigest()
    if file_hash != FILE_SHA256:
        raise RuntimeError(f"fixture hash changed: {file_hash}")
    frame_hashes = pillow_frame_hashes(encoded)
    if frame_hashes != FRAME_SHA256:
        raise RuntimeError(f"Pillow RGB frame hashes changed: {frame_hashes!r}")
    verify_av1_headers(encoded)

    output = args.output.resolve()
    if args.check:
        if not output.is_file() or output.read_bytes() != encoded:
            raise RuntimeError(f"fixture differs from deterministic output: {output}")
    else:
        if output.exists() and output.read_bytes() != encoded:
            raise RuntimeError(f"refusing to replace differing fixture: {output}")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(encoded)
    print(f"Verified {output} ({len(encoded)} bytes, sha256={file_hash})")


if __name__ == "__main__":
    main()
