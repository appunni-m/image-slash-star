#!/usr/bin/env python3
"""Generate a parity fixture for the AV1 nonuniform-tile column limit."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from PIL import Image
import yaml

import generate_decode_refs as oracle
import inspect_av1_obus as av1
from inspect_avif_bitstreams import inspect as inspect_avif

ROOT = Path(__file__).resolve().parent.parent
ASSET = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "images"
    / "avif"
    / "error_nonuniform_tile_column_cap_4097x1.avif"
)
EXPECTED_ASSET_SHA256 = "08da0817962bf18a5bdf2eba90c35c0ce82997649f393599ccf6ad2e4bb4c955"
EXPECTED_PILLOW_ERROR = "Failed to decode frame 0: Decoding of color planes failed"
WIDTH = 4097
HEIGHT = 1
TILE_COLUMN_LIMIT = 64
CORRUPTED_TILE_INDEX = 0
CORRUPTED_TILE_BYTE_OFFSET = 10
CORRUPTED_TILE_BYTE_XOR = 0x01
EXPECTED_PARSER_ERROR = "tile columns do not cover the frame"


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of bytes."""
    return hashlib.sha256(data).hexdigest()


def inspect_with_tile_trace(
    path: Path,
) -> tuple[dict | None, list[list[tuple[int, int, int]]], str | None]:
    """Capture each tiling header's `ns` reads and any AV1 parser error."""
    calls_by_tiling: list[list[tuple[int, int, int]]] = []
    state: dict[str, object] = {"active": False}
    original_read_tiling = av1.read_tiling
    original_ns = av1.Bits.ns

    def trace_tiling(bits, sequence, header):
        calls: list[tuple[int, int, int]] = []
        calls_by_tiling.append(calls)
        state["active"] = True
        state["current"] = calls
        try:
            return original_read_tiling(bits, sequence, header)
        finally:
            state["active"] = False

    def trace_ns(bits, count):
        position = bits.position
        value = original_ns(bits, count)
        if state["active"]:
            current = state["current"]
            current.append((count, position, value))
        return value

    av1.read_tiling = trace_tiling
    av1.Bits.ns = trace_ns
    try:
        try:
            return av1.inspect(path), calls_by_tiling, None
        except ValueError as error:
            return None, calls_by_tiling, str(error)
    finally:
        av1.read_tiling = original_read_tiling
        av1.Bits.ns = original_ns


def changed_bit_positions(before: bytes, after: bytes) -> list[int]:
    """Return changed MSB-first bit positions between equal-length inputs."""
    if len(before) != len(after):
        raise ValueError("fixture mutation changed the AVIF byte length")
    positions: list[int] = []
    for byte_index, (left, right) in enumerate(zip(before, after, strict=True)):
        difference = left ^ right
        for bit_index in range(8):
            if difference & (1 << (7 - bit_index)):
                positions.append(byte_index * 8 + bit_index)
    return positions


def mutate_tile_limit(seed_path: Path) -> bytes:
    """Expose the cap's false branch, then corrupt tile data for Pillow parity."""
    report, calls_by_tiling, error = inspect_with_tile_trace(seed_path)
    if error is not None or report is None:
        raise ValueError(f"pinned encoder seed failed AV1 inspection: {error}")

    samples = report["samples"]
    if len(samples) != 1 or samples[0]["role"] != "item_color":
        raise ValueError("pinned seed must contain exactly one AV1 still item")
    frame = next((obu for obu in samples[0]["obus"] if obu["type"] == 6), None)
    if frame is None or len(calls_by_tiling) != 1:
        raise ValueError("pinned seed must contain one AV1 frame header")

    header = frame["frame_header"]
    tiling = header["tiling"]
    expected_starts = [*range(TILE_COLUMN_LIMIT), TILE_COLUMN_LIMIT + 1]
    calls = calls_by_tiling[0]
    last_call = calls[-1] if calls else None
    tiles = frame["tile_group"]["tiles"]
    if (
        (header["frame_width"], header["frame_height"]) != (WIDTH, HEIGHT)
        or tiling["uniform"] is not False
        or tiling["columns"] != TILE_COLUMN_LIMIT
        or tiling["column_starts"] != expected_starts
        or len(calls) != TILE_COLUMN_LIMIT
        or last_call is None
        or last_call[0] != 2
        or last_call[2] != 1
        or len(tiles) != TILE_COLUMN_LIMIT
        or len(tiles[CORRUPTED_TILE_INDEX]["physical_spans"]) != 1
    ):
        raise ValueError("pinned seed differs from the 64-column tile profile")

    seed = seed_path.read_bytes()
    header_span = frame["payload_spans"][0]
    header_bit_position = header_span["offset"] * 8 + last_call[1]
    header_byte, header_bit = divmod(header_bit_position, 8)
    header_mask = 1 << (7 - header_bit)
    if not seed[header_byte] & header_mask:
        raise ValueError("final tile-width bit differs from its pinned seed")

    tile_span = tiles[CORRUPTED_TILE_INDEX]["physical_spans"][0]
    if CORRUPTED_TILE_BYTE_OFFSET >= tile_span["length"]:
        raise ValueError("pinned tile is shorter than the corruption offset")
    tile_byte = tile_span["offset"] + CORRUPTED_TILE_BYTE_OFFSET
    mutation_bit_position = tile_byte * 8 + (7 - (CORRUPTED_TILE_BYTE_XOR.bit_length() - 1))

    mutated = bytearray(seed)
    mutated[header_byte] &= ~header_mask
    mutated[tile_byte] ^= CORRUPTED_TILE_BYTE_XOR
    result = bytes(mutated)
    if changed_bit_positions(seed, result) != [header_bit_position, mutation_bit_position]:
        raise ValueError("fixture mutation changed bits outside the pinned tile profile")
    return result


def validate(path: Path) -> None:
    """Check the pinned AVIF, cap parse failure, and independent Pillow failure."""
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    oracle.verify_primary_oracle(manifest)

    data = path.read_bytes()
    if sha256(data) != EXPECTED_ASSET_SHA256:
        raise ValueError("AVIF bytes differ from the pinned tile-cap error fixture")

    container = inspect_avif(path)
    if container["ftyp"]["major"] != "avif" or container.get("tracks"):
        raise ValueError("fixture must be a still AVIF primary item without tracks")
    if len(container["items"]["color"]) != 1:
        raise ValueError("fixture must contain exactly one color item")

    _, calls_by_tiling, parser_error = inspect_with_tile_trace(path)
    calls = calls_by_tiling[0] if len(calls_by_tiling) == 1 else []
    if (
        parser_error != EXPECTED_PARSER_ERROR
        or len(calls) != TILE_COLUMN_LIMIT
        or calls[-1][0] != 2
        or calls[-1][2] != 0
    ):
        raise ValueError("independent AV1 parser did not reach the 64-column cap")

    try:
        with Image.open(path) as image:
            image.load()
    except RuntimeError as error:
        if EXPECTED_PILLOW_ERROR not in str(error):
            raise ValueError(f"Pillow error differs from the pinned failure: {error}") from error
    else:
        raise ValueError("Pillow accepts the declared AVIF error fixture")

def regenerate(encoder: Path) -> None:
    """Regenerate the pinned encoder seed and apply the two documented bit flips."""
    if not encoder.is_file():
        raise ValueError(f"encoder does not exist: {encoder}")
    staging = ROOT / "target" / "oracle-staging"
    staging.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="tile-column-cap-error-", dir=staging) as temporary:
        seed = Path(temporary) / "seed.avif"
        output = Path(temporary) / ASSET.name
        subprocess.run([str(encoder), "--tile-limit", str(seed)], check=True)
        output.write_bytes(mutate_tile_limit(seed))
        validate(output)
        ASSET.write_bytes(output.read_bytes())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, help="regenerate with the pinned fixture encoder")
    parser.add_argument("--check", action="store_true", help="verify the retained fixture")
    args = parser.parse_args()

    try:
        if args.encoder is not None and args.check:
            parser.error("--encoder and --check are mutually exclusive")
        if args.encoder is not None:
            regenerate(args.encoder.resolve())
        validate(ASSET)
    except (OSError, ValueError, subprocess.CalledProcessError, KeyError, TypeError) as error:
        parser.exit(1, f"error: {error}\n")

    print(f"Verified AV1 tile-column-cap error fixture: {ASSET.relative_to(ROOT)}")
    print(f"AVIF SHA-256: {sha256(ASSET.read_bytes())}")
    print(f"Pinned Pillow error contains: {EXPECTED_PILLOW_ERROR}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
