#!/usr/bin/env python3
"""Verify the third-party legal material shipped with the crate."""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]

EXPECTED_SHA256 = {
    "PATENTS": "661fb8e504744e95587b556b94a58343448300606a41bea8c7a9b97125696e61",
    "third_party/apple-libc/LICENSE": (
        "37f9a70ea8cf7842f0c5b6a5effacb00c988b6edc8bee180138c5882f8266660"
    ),
    "third_party/bytemuck/LICENSE-APACHE": (
        "e3ba223bb1423f0aad8c3dfce0fe3148db48926d41e6fbc3afbbf5ff9e1c89cb"
    ),
    "third_party/bytemuck/LICENSE-MIT": (
        "9df9ba60a11af705f2e451b53762686e615d86f76b169cf075c3237730dbd7e2"
    ),
    "third_party/bytemuck/LICENSE-ZLIB": (
        "84b34dd7608f7fb9b17bd588a6bf392bf7de504e2716f024a77d89f1b145a151"
    ),
    "third_party/dav1d/COPYING": (
        "dd92c3c2247c5651606fc23a5e2d6a1ebc5ace9a3e49cbde0e12f05ad1cb1ee5"
    ),
    "third_party/image-webp/LICENSE-APACHE": (
        "0d542e0c8804e39aa7f37eb00da5a762149dc682d7829451287e11b938e94594"
    ),
    "third_party/image-webp/LICENSE-MIT": (
        "c77a4cf9da729987d0fe7ccd811e3bd27393914ddf3d23467c18cc22954513b3"
    ),
    "third_party/libaom/LICENSE": (
        "4764a286d8b2faeaf42f4418e7d7a28d58fc8fd4d00a3d0a7f44b0a4099de7f2"
    ),
    "third_party/libaom/PATENTS": (
        "661fb8e504744e95587b556b94a58343448300606a41bea8c7a9b97125696e61"
    ),
    "third_party/libavif/LICENSE": (
        "165abf92cc04b39e80d29cadea7a6a7e8fddf59407d4ad2616507a7ebe8216f9"
    ),
    "third_party/libavif/include/avif/avif.h": (
        "2fcde09bb0124f4c1d1fbc5dfbf06ade08a66d8c58854fd3fe3411a6483bd26e"
    ),
    "third_party/libjpeg-turbo/LICENSE.md": (
        "e10114e6e40f3d0311c401ca25245ac5ef459a43c20f976fd63f03e816f5741f"
    ),
    "third_party/libjpeg-turbo/README.ijg": (
        "75815e3bf6484201a3c3d17a1bbf10f2e8e3237f84df10a2357ea896db2a81d6"
    ),
    "third_party/libwebp/COPYING": (
        "e293d1dddc9785200b1f58a4f5293543cf8566d9e0b8a3c02fad955035b19f42"
    ),
    "third_party/libyuv/LICENSE": (
        "2b2cc1180c7e6988328ad2033b04b80117419db9c4c584918bbb3cfec7e9364f"
    ),
    "third_party/pillow/LICENSE": (
        "15181e7363dca9aed78b79bebebc7fde7f1814b8bd311ea3b87ae8ccadfc185b"
    ),
    "third_party/pillow/QUANT-OCTREE-LICENSE": (
        "38fecb6df26ecfc36c567c6e213463a6bc1304d7efe832497b737a6a0cd68b97"
    ),
    "third_party/zlib-ng/LICENSE.md": (
        "6c9f0d975b41afaa34d22f55bb8986ce69e5cb7ad327cb2b28820cd425edf5ee"
    ),
}

NOTICE_FRAGMENTS = (
    "third_party/apple-libc/LICENSE",
    "third_party/bytemuck/",
    "third_party/dav1d/COPYING",
    "third_party/image-webp/",
    "third_party/libaom/LICENSE",
    "third_party/libavif/",
    "third_party/libjpeg-turbo/",
    "third_party/libwebp/COPYING",
    "third_party/libyuv/LICENSE",
    "third_party/pillow/LICENSE",
    "third_party/pillow/QUANT-OCTREE-LICENSE",
    "third_party/zlib-ng/LICENSE.md",
    "This software is based in part on the work of the Independent JPEG Group.",
)


def sha256(path: Path) -> str:
    """Return the lowercase SHA-256 digest for one file."""
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(128 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def tracked_legal_files() -> set[str]:
    """Return every legal-text path under ``third_party``."""
    paths: set[str] = set()
    for path in (ROOT / "third_party").rglob("*"):
        if not path.is_file():
            continue
        name = path.name
        if (
            name.startswith(("LICENSE", "COPYING", "PATENTS"))
            or name in {"README.ijg", "QUANT-OCTREE-LICENSE"}
        ):
            paths.add(path.relative_to(ROOT).as_posix())
    return paths


def main() -> int:
    """Validate hashes, inventory coverage, and distribution references."""
    failures: list[str] = []
    for relative, expected in EXPECTED_SHA256.items():
        path = ROOT / relative
        if not path.is_file():
            failures.append(f"missing retained file: {relative}")
            continue
        actual = sha256(path)
        if actual != expected:
            failures.append(
                f"hash mismatch: {relative}: expected {expected}, got {actual}"
            )

    expected_legal = {
        path
        for path in EXPECTED_SHA256
        if path.startswith("third_party/")
        and path != "third_party/libavif/include/avif/avif.h"
    }
    untracked = tracked_legal_files() - expected_legal
    if untracked:
        failures.append(
            "legal files missing from checksum inventory: " + ", ".join(sorted(untracked))
        )

    if (ROOT / "PATENTS").read_bytes() != (
        ROOT / "third_party/libaom/PATENTS"
    ).read_bytes():
        failures.append("root PATENTS differs from third_party/libaom/PATENTS")

    notice = (ROOT / "NOTICE.md").read_text(encoding="utf-8")
    for fragment in NOTICE_FRAGMENTS:
        if fragment not in notice:
            failures.append(f"NOTICE.md is missing required reference: {fragment}")

    cargo_manifest = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    for packaged in ('"/PATENTS"', '"/third_party/**/*"'):
        if packaged not in cargo_manifest:
            failures.append(f"Cargo.toml package include is missing {packaged}")

    if failures:
        for failure in failures:
            print(f"third-party license verification failed: {failure}", file=sys.stderr)
        return 1

    print(f"verified {len(EXPECTED_SHA256)} retained legal/provenance files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
