"""Reproducible same-length AVIF sample-table mutations."""

from __future__ import annotations

import hashlib
from pathlib import Path

from inspect_avif_bitstreams import children, parse_boxes, unique_box


ROOT = Path(__file__).resolve().parent.parent
SOURCE_PATH = ROOT / "tests/fixtures/input/images/avif/animated.avif"
OUTPUT_PATH = ROOT / "tests/fixtures/input/images/avif/animated_stts_zero_delta.avif"
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
OUTPUT_SHA256 = "5b2c438346c66b7ee1cb165742e639bc98e3180d5da42dd8e479f48b99598105"
STTS_BOX_OFFSET = 883
SAMPLE_DELTA_OFFSET = 903


def zero_stts_sample_delta(source: bytes) -> bytes:
    """Set the sole five-sample `stts` delta to zero without changing box sizes."""
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise ValueError("animated AVIF source differs from its pinned input")

    top_level = parse_boxes(source, 0, len(source))
    movie = unique_box(top_level, b"moov")
    assert movie is not None
    tracks = [box for box in children(source, movie) if box.kind == b"trak"]
    if len(tracks) != 1:
        raise ValueError("animated AVIF must contain exactly one movie track")

    media = unique_box(children(source, tracks[0]), b"mdia")
    assert media is not None
    media_info = unique_box(children(source, media), b"minf")
    assert media_info is not None
    sample_table = unique_box(children(source, media_info), b"stbl")
    assert sample_table is not None
    timing = unique_box(children(source, sample_table), b"stts")
    assert timing is not None
    if timing.start != STTS_BOX_OFFSET or timing.size != 24:
        raise ValueError("animated AVIF stts box bounds differ from the pinned layout")

    payload = timing.payload_start
    version_and_flags = int.from_bytes(source[payload : payload + 4], "big")
    entry_count = int.from_bytes(source[payload + 4 : payload + 8], "big")
    sample_count = int.from_bytes(source[payload + 8 : payload + 12], "big")
    sample_delta = int.from_bytes(source[payload + 12 : payload + 16], "big")
    if (
        version_and_flags != 0
        or entry_count != 1
        or sample_count != 5
        or sample_delta != 1
        or payload + 12 != SAMPLE_DELTA_OFFSET
    ):
        raise ValueError("animated AVIF stts entry differs from the pinned source")

    output = bytearray(source)
    output[SAMPLE_DELTA_OFFSET : SAMPLE_DELTA_OFFSET + 4] = bytes(4)
    result = bytes(output)
    if len(result) != len(source) or hashlib.sha256(result).hexdigest() != OUTPUT_SHA256:
        raise ValueError("zero-delta AVIF output differs from its pinned mutation")
    changed = [index for index, (before, after) in enumerate(zip(source, result)) if before != after]
    if changed != [SAMPLE_DELTA_OFFSET + 3]:
        raise ValueError("zero-delta mutation changed bytes outside the sample_delta field")
    return result


def write_fixture(source: bytes | None = None, output_path: Path = OUTPUT_PATH) -> str:
    """Write the pinned fixture, refusing to replace a different file."""
    data = SOURCE_PATH.read_bytes() if source is None else source
    output = zero_stts_sample_delta(data)
    if output_path.exists() and output_path.read_bytes() != output:
        raise FileExistsError(f"refusing to replace a different fixture at {output_path}")
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_bytes(output)
    return hashlib.sha256(output).hexdigest()


if __name__ == "__main__":
    print(f"wrote AVIF fixture with SHA-256 {write_fixture()}")
