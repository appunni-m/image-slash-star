#!/usr/bin/env python3
"""Verify Pillow's observable TIFF source-byte-order contract.

This retained exploration is not a Rust test oracle. It proves that the
version-pinned Pillow TIFF directory exposes the same ``II``/``MM`` byte-order
marker as the complete fixture and that the marker remains stable before and
after pixel materialization, including every page in a main IFD chain.
"""

import hashlib
import json
from pathlib import Path

from PIL import Image, features
from PIL import __version__ as pillow_version


ROOT = Path(__file__).parent.parent
TIFF_FIXTURES = ROOT / "tests" / "fixtures" / "input" / "images" / "tiff"
CASES = (
    "le.tiff",
    "be.tiff",
    "be_float32_predictor.tiff",
    "be_signed32_predictor.tiff",
    "le_unsigned32_predictor.tiff",
    "multipage.tiff",
    "multipage_mixed.tiff",
)


def marker_name(marker):
    if marker == b"II":
        return "little"
    if marker == b"MM":
        return "big"
    raise ValueError(f"invalid TIFF byte-order marker: {marker!r}")


def inspect_case(name):
    path = TIFF_FIXTURES / name
    data = path.read_bytes()
    file_marker = data[:2]
    file_byte_order = marker_name(file_marker)

    pages = []
    with Image.open(path) as image:
        if image.format != "TIFF":
            raise ValueError(f"{name}: Pillow selected {image.format!r}, not TIFF")
        for index in range(image.n_frames):
            image.seek(index)
            directory_marker = image.tag_v2.prefix
            if directory_marker != file_marker:
                raise ValueError(
                    f"{name} page {index}: Pillow prefix {directory_marker!r} "
                    f"differs from file marker {file_marker!r}"
                )
            before_load = marker_name(directory_marker)
            image.load()
            after_load = marker_name(image.tag_v2.prefix)
            if before_load != after_load:
                raise ValueError(
                    f"{name} page {index}: byte order changed while loading pixels"
                )
            pixels = image.tobytes()
            pages.append(
                {
                    "index": index,
                    "mode": image.mode,
                    "size": list(image.size),
                    "source_byte_order_before_load": before_load,
                    "source_byte_order_after_load": after_load,
                    "pixels_sha256": hashlib.sha256(pixels).hexdigest(),
                }
            )

    return {
        "fixture": name,
        "file_marker_hex": file_marker.hex(),
        "source_byte_order": file_byte_order,
        "pages": pages,
    }


def main():
    if pillow_version != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {pillow_version}")
    libtiff = features.version_codec("libtiff")
    if libtiff != "4.7.1":
        raise RuntimeError(f"libtiff 4.7.1 is required, found {libtiff}")

    print(
        json.dumps(
            {
                "pillow": pillow_version,
                "libtiff": libtiff,
                "cases": [inspect_case(name) for name in CASES],
            },
            indent=2,
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
