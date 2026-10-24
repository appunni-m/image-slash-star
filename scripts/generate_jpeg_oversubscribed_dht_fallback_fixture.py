#!/usr/bin/env python3
"""Keep JPEG sibling Huffman tables while oversubscribing DC table zero."""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
INPUT_DIR = ROOT / "tests/fixtures/input/images/jpeg"
FIXTURE_SPECS = {
    "dht_oversubscribed_retained_tables.jpg": (
        "baseline_444.jpg",
        "60dc8ca9db232378a58f6c45321303775dc0b0a8762dd5bfad553cefe390621a",
        "d308eee3946fdfa39f65045ee77d49ba407868dbea4a8a421fde3179b45bc457",
    ),
    "sampling_3x1_oversubscribed_dc0.jpg": (
        "sampling_3x1.jpg",
        "722add8d9c6ae7dd509e4c68254a6e53d51b7fc63b5987bb1209edfd86ed6076",
        "7e895799133d2308b6e0eef0795c67da67675771693cde3aad1f49dc81bd6625",
    ),
}
BROKEN_DATA_MESSAGE = "broken data stream when reading image file"
OVERSUBSCRIBED_DC0 = bytes((0, 3, *([0] * 15), 0, 1, 2))


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def replace_dc0_table(payload: bytes) -> bytes:
    """Replace only DC0, preserving the baseline AC and chroma tables."""
    rewritten = bytearray()
    position = 0
    replaced = False
    while position < len(payload):
        if len(payload) - position < 17:
            raise ValueError("truncated JPEG Huffman table header")
        table_info = payload[position]
        symbol_count = sum(payload[position + 1 : position + 17])
        record_end = position + 17 + symbol_count
        if record_end > len(payload):
            raise ValueError("truncated JPEG Huffman table symbols")
        if table_info == 0 and not replaced:
            rewritten.extend(OVERSUBSCRIBED_DC0)
            replaced = True
        else:
            rewritten.extend(payload[position:record_end])
        position = record_end
    if not replaced:
        raise ValueError("baseline JPEG has no DC0 Huffman table")
    return bytes(rewritten)


def generate_fixture(source_name: str, source_sha: str, output_sha: str) -> bytes:
    """Replace the first pre-scan DHT record with an overfull DC0 table."""
    source = (INPUT_DIR / source_name).read_bytes()
    source_sha256 = sha256(source)
    if source_sha256 != source_sha:
        raise RuntimeError(
            f"{source_name} SHA-256 mismatch: expected {source_sha}, found {source_sha256}"
        )
    if not source.startswith(b"\xff\xd8"):
        raise RuntimeError("source JPEG does not start with SOI")

    position = 2
    while position < len(source):
        marker_start = position
        if source[position] != 0xFF:
            raise RuntimeError("unexpected non-marker byte before JPEG scan")
        while position < len(source) and source[position] == 0xFF:
            position += 1
        if position >= len(source):
            raise RuntimeError("truncated JPEG marker")
        marker_code = source[position]
        position += 1
        if marker_code == 0xDA:
            break
        if marker_code in (0xD8, 0xD9) or 0xD0 <= marker_code <= 0xD7:
            continue
        if position + 2 > len(source):
            raise RuntimeError("truncated JPEG marker length")
        segment_length = int.from_bytes(source[position : position + 2], "big")
        if segment_length < 2:
            raise RuntimeError("invalid JPEG marker length")
        segment_end = position + segment_length
        if segment_end > len(source):
            raise RuntimeError("truncated JPEG marker segment")
        if marker_code == 0xC4:
            payload = replace_dc0_table(source[position + 2 : segment_end])
            encoded = (
                source[:marker_start]
                + b"\xff\xc4"
                + (len(payload) + 2).to_bytes(2, "big")
                + payload
                + source[segment_end:]
            )
            output_sha256 = sha256(encoded)
            if output_sha256 != output_sha:
                raise RuntimeError(
                    f"generated JPEG SHA-256 mismatch: expected {output_sha}, "
                    f"found {output_sha256}"
                )
            return encoded
        position = segment_end
    raise RuntimeError("baseline JPEG has no pre-scan DHT segment")


def verify_pillow_reference(encoded: bytes, filename: str) -> None:
    """Pin successful structural verification and the decode error to Pillow."""
    import PIL
    from PIL import Image

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PIL.__version__}")
    with Image.open(BytesIO(encoded)) as image:
        image.verify()
    try:
        with Image.open(BytesIO(encoded)) as image:
            image.load()
    except OSError as error:
        if str(error) != BROKEN_DATA_MESSAGE:
            raise RuntimeError(
                f"unexpected Pillow decode error for {filename}: {error}"
            ) from error
    else:
        raise RuntimeError(f"Pillow unexpectedly decoded {filename}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed fixture without rewriting it",
    )
    args = parser.parse_args()

    fixtures = {
        filename: generate_fixture(source, source_sha, output_sha)
        for filename, (source, source_sha, output_sha) in FIXTURE_SPECS.items()
    }
    if args.check:
        for filename, encoded in fixtures.items():
            output = INPUT_DIR / filename
            if not output.is_file() or output.read_bytes() != encoded:
                raise RuntimeError(f"{output.relative_to(ROOT)} differs from generated bytes")
            print(f"Fixture matches: {output.relative_to(ROOT)}")
        return 0

    for filename, encoded in fixtures.items():
        verify_pillow_reference(encoded, filename)
        output = INPUT_DIR / filename
        output.write_bytes(encoded)
        print(f"Wrote {output.relative_to(ROOT)} ({len(encoded)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
