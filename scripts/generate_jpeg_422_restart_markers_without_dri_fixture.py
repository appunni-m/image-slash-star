#!/usr/bin/env python3
"""Remove the DRI from Pillow's restart-coded baseline 4:2:2 JPEG."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tests/fixtures/input/images/jpeg/baseline_422_restart_blocks_1.jpg"
OUTPUT = (
    ROOT
    / "tests/fixtures/input/images/jpeg/baseline_422_restart_markers_without_dri.jpg"
)
EXPECTED_SOURCE_SHA256 = "5b028c8f6b91661cbed9019bed9012de5972a48c7a33c6ebf09488b6fd1c2126"
EXPECTED_OUTPUT_SHA256 = "ba16f0911f19776e8c7f57ea90c2dcc1dc4cf9814f2359d404b6ca796f3b5a3e"
EXPECTED_PILLOW_RGB_SHA256 = "f1d5b7784fbab6ae1c5d635f23d8e4a952e0f6f029b1cf391dd7df5097d592b3"
DRI_INTERVAL_ONE = bytes.fromhex("ffdd00040001")
EOI_MARKER = b"\xff\xd9"
SOS_MARKER = b"\xff\xda"


def restart_codes(data: bytes) -> tuple[int, ...]:
    """Read restart markers in the single baseline scan, skipping stuffed bytes."""
    if not data.startswith(b"\xff\xd8") or not data.endswith(EOI_MARKER):
        raise RuntimeError("complete JPEG framing is required")
    scan_start = data.find(SOS_MARKER)
    if scan_start < 0 or data.find(SOS_MARKER, scan_start + len(SOS_MARKER)) >= 0:
        raise RuntimeError("expected one baseline SOS")

    segment_length = int.from_bytes(data[scan_start + 2 : scan_start + 4], "big")
    entropy_start = scan_start + 2 + segment_length
    if segment_length < 2 or entropy_start > len(data) - len(EOI_MARKER):
        raise RuntimeError("baseline SOS extent is invalid")

    codes = []
    position = entropy_start
    entropy_end = len(data) - len(EOI_MARKER)
    while position < entropy_end:
        marker_start = data.find(b"\xff", position, entropy_end)
        if marker_start < 0:
            break
        code_position = marker_start + 1
        while code_position < entropy_end and data[code_position] == 0xFF:
            code_position += 1
        if code_position >= entropy_end:
            raise RuntimeError("entropy data ends in a truncated marker")

        code = data[code_position]
        if code == 0x00:
            position = code_position + 1
        elif 0xD0 <= code <= 0xD7:
            codes.append(code)
            position = code_position + 1
        else:
            raise RuntimeError(f"unexpected marker FF{code:02X} before EOI")

    if not codes or any(code != 0xD0 + index % 8 for index, code in enumerate(codes)):
        raise RuntimeError("entropy restart markers are missing or out of sequence")
    return tuple(codes)


def generate_fixture() -> bytes:
    """Remove only the pre-scan DRI segment from the pinned Pillow source."""
    source = SOURCE.read_bytes()
    source_sha256 = sha256(source).hexdigest()
    if source_sha256 != EXPECTED_SOURCE_SHA256:
        raise RuntimeError(
            f"source JPEG SHA-256 mismatch: expected {EXPECTED_SOURCE_SHA256}, "
            f"found {source_sha256}"
        )

    scan_start = source.find(SOS_MARKER)
    dri_start = source.find(DRI_INTERVAL_ONE)
    if (
        scan_start < 0
        or dri_start < 2
        or dri_start >= scan_start
        or source.find(DRI_INTERVAL_ONE, dri_start + len(DRI_INTERVAL_ONE)) >= 0
    ):
        raise RuntimeError("source must contain one DRI=1 segment before its scan")

    source_codes = restart_codes(source)
    candidate = source[:dri_start] + source[dri_start + len(DRI_INTERVAL_ONE) :]
    if restart_codes(candidate) != source_codes:
        raise RuntimeError("removing DRI changed the entropy restart markers")

    output_sha256 = sha256(candidate).hexdigest()
    if output_sha256 != EXPECTED_OUTPUT_SHA256:
        raise RuntimeError(
            f"generated JPEG SHA-256 mismatch: expected {EXPECTED_OUTPUT_SHA256}, "
            f"found {output_sha256}"
        )
    return candidate


def pillow_rgb(data: bytes, label: str) -> bytes:
    """Verify one JPEG with Pillow 12.2.0 and return its decoded RGB pixels."""
    import PIL
    from PIL import Image

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {PIL.__version__}")

    with Image.open(BytesIO(data)) as image:
        image.verify()
    with Image.open(BytesIO(data)) as image:
        image.load()
        if image.format != "JPEG" or image.mode != "RGB" or image.size != (128, 128):
            raise RuntimeError(
                f"Pillow returned unexpected {label} metadata: "
                f"format={image.format!r}, mode={image.mode!r}, size={image.size!r}"
            )
        return image.tobytes()


def verify_pillow_reference(candidate: bytes) -> None:
    """Pin Pillow's DRI-free decode and confirm it differs from the source decode."""
    source = SOURCE.read_bytes()
    source_pixels = pillow_rgb(source, "source")
    candidate_pixels = pillow_rgb(candidate, "candidate")
    if candidate_pixels == source_pixels:
        raise RuntimeError("Pillow result unexpectedly ignores the missing DRI segment")

    pixel_sha256 = sha256(candidate_pixels).hexdigest()
    if pixel_sha256 != EXPECTED_PILLOW_RGB_SHA256:
        raise RuntimeError(
            f"Pillow RGB SHA-256 mismatch: expected {EXPECTED_PILLOW_RGB_SHA256}, "
            f"found {pixel_sha256}"
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the committed fixture without rewriting it",
    )
    args = parser.parse_args()

    encoded = generate_fixture()
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != encoded:
            raise RuntimeError(f"{OUTPUT.relative_to(ROOT)} differs from generated bytes")
        verify_pillow_reference(encoded)
        print(f"Fixture matches: {OUTPUT.relative_to(ROOT)}")
        return 0

    verify_pillow_reference(encoded)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(encoded)
    print(f"Wrote {OUTPUT.relative_to(ROOT)} ({len(encoded)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
