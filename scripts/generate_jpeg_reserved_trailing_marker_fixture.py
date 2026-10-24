#!/usr/bin/env python3
"""Generate a complete JPEG with a reserved post-scan marker segment.

The pinned Pillow oracle must continue to accept inspection and structural
verification while rejecting pixel materialization with its broken-stream
error. The mutation preserves the existing scan and trailing DQT bytes.
"""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path

import yaml
from PIL import Image

from generate_decode_refs import stable_error_message, verify_primary_oracle
from generate_test_assets import jpeg_scan_segments


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DIRECTORY = ROOT / "tests/fixtures/input/images/jpeg"
SOURCE_NAME = "baseline_444_single_scan_trailing_dqt.jpg"
SOURCE_SHA256 = "1b4519e48e1ce711cd1cbb44c3173ef3c97f222b8aa1686f9437df5e5f906372"
OUTPUT_NAME = "baseline_444_single_scan_reserved_jpg0.jpg"
OUTPUT_SHA256 = "8cac78f1ec5ba03d629ac828e27d12eb579084476639c90155069457d2728c55"
ERROR_MESSAGE = "broken data stream when reading image file"
RESERVED_SEGMENT = bytes.fromhex("ff f0 00 02")


def observe(data: bytes) -> dict[str, tuple]:
    """Collect Pillow's public inspection, verify, decode, and sequence results."""
    outcomes = {}
    for operation in ("inspect", "verify", "decode", "decode_sequence"):
        try:
            image = Image.open(BytesIO(data))
        except Exception as error:
            outcomes[operation] = (
                "error", "open", type(error).__module__, type(error).__name__,
                stable_error_message(error),
            )
            continue
        with image:
            metadata = image.format, image.mode, image.size, getattr(image, "n_frames", 1)
            if operation == "inspect":
                outcomes[operation] = ("ok", metadata)
                continue
            try:
                if operation == "verify":
                    image.verify()
                else:
                    frame_count = metadata[3] if operation == "decode_sequence" else 1
                    for frame in range(frame_count):
                        image.seek(frame)
                        image.load()
                        image.tobytes()
            except Exception as error:
                outcomes[operation] = (
                    "error", "verify" if operation == "verify" else "materialize",
                    type(error).__module__, type(error).__name__, stable_error_message(error),
                )
                continue
            outcomes[operation] = ("ok", metadata)
    return outcomes


def build(source: bytes) -> bytes:
    """Insert one zero-payload JPG0 segment before the final EOI marker."""
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned trailing-DQT JPEG source changed")
    scans, eoi_start = jpeg_scan_segments(source)
    if len(scans) != 1 or source[eoi_start:] != b"\xff\xd9":
        raise RuntimeError("expected one complete scan followed by a final EOI")
    output = source[:eoi_start] + RESERVED_SEGMENT + source[eoi_start:]
    if sha256(output).hexdigest() != OUTPUT_SHA256:
        raise RuntimeError("reserved-marker JPEG fixture differs from its pinned bytes")
    return output


def generate(directory: Path, *, check: bool) -> Path:
    """Verify the pinned Pillow lifecycle and generate or check the input."""
    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    output = build((directory / SOURCE_NAME).read_bytes())
    outcomes = observe(output)
    expected_metadata = ("JPEG", "RGB", (8, 8), 1)
    expected_error = (
        "error", "materialize", "builtins", "OSError", ERROR_MESSAGE,
    )
    expected = {
        "inspect": ("ok", expected_metadata),
        "verify": ("ok", expected_metadata),
        "decode": expected_error,
        "decode_sequence": expected_error,
    }
    if outcomes != expected:
        raise RuntimeError(f"pinned Pillow lifecycle changed: {outcomes!r}")

    path = directory / OUTPUT_NAME
    if check:
        if not path.is_file() or path.read_bytes() != output:
            raise RuntimeError(f"reserved-marker JPEG input differs: {path}")
    else:
        if path.exists() and path.read_bytes() != output:
            raise RuntimeError(f"refusing to replace different existing input: {path}")
        if not path.exists():
            path.write_bytes(output)
    return path


def main() -> None:
    """Generate the fixture, or compare the existing file without rewriting it."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, default=DEFAULT_DIRECTORY)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    print(generate(args.directory, check=args.check))


if __name__ == "__main__":
    main()
