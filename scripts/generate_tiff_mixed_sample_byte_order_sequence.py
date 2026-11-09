#!/usr/bin/env python3
"""Create a two-page TIFF input for mixed-sample-byte-order encode parity."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/fixtures/input/images/tiff/mixed_sample_byte_order_sequence.tiff"
EXPECTED_SHA256 = "71ff1a014289b1a9f325a8f87f044f0b82f73decef614c069375ecf41fdfd800"
SIZE = (4, 2)
PAGE_BYTES = (
    bytes.fromhex("00000100020003000400050006000700"),
    bytes.fromhex("10001100120013001400150016001700"),
)
EXPECTED_PILLOW_ERROR = "IIMM of new page doesn't match IIMM of first page"


def fixture_bytes() -> bytes:
    """Build two same-mode pages whose second page can be mode-converted."""
    from PIL import Image

    frames = [Image.frombytes("I;16", SIZE, pixels) for pixels in PAGE_BYTES]
    output = BytesIO()
    frames[0].save(
        output,
        format="TIFF",
        save_all=True,
        append_images=frames[1:],
        compression="raw",
    )
    return output.getvalue()


def verify_pillow_error(data: bytes) -> None:
    """Check Pillow's result for the same normal multi-frame save request."""
    import PIL
    from PIL import Image

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PIL.__version__}")

    with Image.open(BytesIO(data)) as source:
        if source.format != "TIFF" or source.n_frames != 2 or source.mode != "I;16":
            raise RuntimeError("source TIFF must have two unsigned 16-bit grayscale pages")
        frames = []
        for index, expected in enumerate(PAGE_BYTES):
            source.seek(index)
            source.load()
            if source.mode != "I;16" or source.tobytes() != expected:
                raise RuntimeError(f"source TIFF page {index} changed")
            frames.append(source.copy())

    frames[1] = frames[1].convert("I;16B")
    output = BytesIO()
    try:
        frames[0].save(
            output,
            format="TIFF",
            save_all=True,
            append_images=frames[1:],
            compression="raw",
        )
    except RuntimeError as error:
        if str(error) != EXPECTED_PILLOW_ERROR:
            raise RuntimeError(f"unexpected Pillow error: {error}") from error
    else:
        raise RuntimeError("Pillow unexpectedly accepted mixed-byte-order TIFF pages")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--generate", action="store_true")
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args()

    generated = fixture_bytes()
    verify_pillow_error(generated)
    digest = sha256(generated).hexdigest()

    if args.generate:
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_bytes(generated)
        print(f"Generated {OUTPUT.relative_to(ROOT)} (sha256={digest})")
        return 0

    if digest != EXPECTED_SHA256:
        raise RuntimeError(f"generated fixture hash changed: {digest}")
    actual = OUTPUT.read_bytes()
    if actual != generated:
        raise RuntimeError(f"Fixture differs from generator output: {OUTPUT}")
    print(f"Verified {OUTPUT.relative_to(ROOT)} (sha256={digest})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
