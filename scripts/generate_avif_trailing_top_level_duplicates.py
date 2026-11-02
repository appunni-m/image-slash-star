#!/usr/bin/env python3
"""Generate AVIF duplicate-box parity fixtures before and after media data."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageSequence
import yaml

from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests" / "fixtures" / "input" / "images" / "avif" / "animated.avif"
OUTPUT_DIR = SOURCE.parent
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
FIXTURES = {
    "animated_duplicate_top_level_meta_after_mdat.avif": (
        b"meta",
        True,
        "86ae1b02cbcec44de7ca7bf5c08815a1ac9ca4325f5d74fb89835fb37516156d",
    ),
    "animated_duplicate_top_level_moov_after_mdat.avif": (
        b"moov",
        True,
        "f88aa18f6c19d1f29431dfec69eff4ac0f4cc5b91e56c4c84206d0ab629e59b5",
    ),
}
ERROR_FIXTURES = {
    "error_duplicate_top_level_meta_before_mdat.avif": (
        b"meta",
        False,
        "a409fdd42863ea2fcea4462589d034120b6529854320f3833fef47f0d836e9a2",
        "open",
        "UnidentifiedImageError",
    ),
    "error_duplicate_top_level_moov_before_mdat.avif": (
        b"moov",
        False,
        "9ca96808a42c259be912295f8286109e4cc6d7c2844abec252b1c36203ff0528",
        "decode",
        "RuntimeError",
    ),
}


def top_level_ranges(data: bytes) -> list[tuple[bytes, int, int]]:
    """Return bounded top-level box ranges from an ISO-BMFF file."""
    boxes = []
    offset = 0
    while offset < len(data):
        if len(data) - offset < 8:
            raise RuntimeError("truncated top-level box header")
        size = int.from_bytes(data[offset : offset + 4], "big")
        kind = data[offset + 4 : offset + 8]
        if size < 8 or size > len(data) - offset:
            raise RuntimeError("invalid top-level box size")
        boxes.append((kind, offset, offset + size))
        offset += size
    return boxes


def fixture_bytes(
    source: bytes,
    duplicate_kind: bytes,
    after_mdat: bool,
    expected_sha256: str,
) -> bytes:
    """Duplicate one pinned top-level box at the requested media-data boundary."""
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("animated AVIF differs from its pinned source hash")

    boxes = top_level_ranges(source)
    if [kind for kind, _, _ in boxes] != [b"ftyp", b"meta", b"moov", b"mdat"]:
        raise RuntimeError("animated AVIF top-level box layout changed")
    matches = [(start, end) for kind, start, end in boxes if kind == duplicate_kind]
    if len(matches) != 1:
        raise RuntimeError(f"animated AVIF must contain one {duplicate_kind!r} box")

    duplicate_start, duplicate_end = matches[0]
    duplicate = source[duplicate_start:duplicate_end]
    if after_mdat:
        candidate = source + duplicate
    else:
        media_data_start = next(start for kind, start, _ in boxes if kind == b"mdat")
        if duplicate_end > media_data_start:
            raise RuntimeError("pre-mdat duplicate must precede the media-data box")
        candidate = source[:duplicate_end] + duplicate + source[duplicate_end:]
    if sha256(candidate).hexdigest() != expected_sha256:
        raise RuntimeError(f"generated {duplicate_kind!r} fixture hash changed")
    return candidate


def pillow_observation(data: bytes) -> tuple[object, ...]:
    """Return the public image metadata and every decoded frame's pixels."""
    with Image.open(BytesIO(data)) as image:
        header = (
            image.format,
            image.mode,
            image.size,
            image.n_frames,
            image.is_animated,
            dict(image.info),
        )
    with Image.open(BytesIO(data)) as image:
        image.verify()
    with Image.open(BytesIO(data)) as image:
        frames = []
        for frame in ImageSequence.Iterator(image):
            frame.load()
            frames.append((frame.mode, frame.size, frame.tobytes()))
    return header, tuple(frames)


def verify_pillow_error(data: bytes, phase: str, expected_type: str) -> None:
    """Require Pillow to fail at the pinned public operation and exception type."""
    try:
        with Image.open(BytesIO(data)) as image:
            if phase == "open":
                raise RuntimeError("Pillow unexpectedly opened the malformed fixture")
            image.verify()

        if phase != "decode":
            raise RuntimeError(f"unsupported expected Pillow failure phase: {phase}")

        with Image.open(BytesIO(data)) as image:
            next(iter(ImageSequence.Iterator(image))).load()
    except RuntimeError as error:
        if str(error) == "Pillow unexpectedly opened the malformed fixture":
            raise
        if type(error).__name__ == expected_type:
            return
        raise RuntimeError(
            f"expected Pillow {expected_type} during {phase}, got {type(error).__name__}: {error}"
        ) from error
    except Exception as error:
        if phase == "open" and type(error).__name__ == expected_type:
            return
        raise RuntimeError(
            f"expected Pillow {expected_type} during {phase}, got {type(error).__name__}: {error}"
        ) from error

    raise RuntimeError(f"Pillow unexpectedly completed the {phase} operation")


def generate(output_dir: Path = OUTPUT_DIR, check: bool = False) -> None:
    """Verify Pillow parity and write or check deterministic duplicate fixtures."""
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)

    source = SOURCE.read_bytes()
    source_observation = pillow_observation(source)
    output_dir.mkdir(parents=True, exist_ok=True)
    for filename, (kind, after_mdat, expected_sha256) in FIXTURES.items():
        candidate = fixture_bytes(source, kind, after_mdat, expected_sha256)
        if pillow_observation(candidate) != source_observation:
            raise RuntimeError(f"Pillow observation changed for {filename}")

        path = output_dir / filename
        if check:
            if not path.is_file() or path.read_bytes() != candidate:
                raise RuntimeError(f"fixture is missing or differs from generated bytes: {path}")
        elif path.exists():
            if path.read_bytes() != candidate:
                raise RuntimeError(f"refusing to replace a different fixture at {path}")
        else:
            path.write_bytes(candidate)
        print(f"Verified AVIF trailing duplicate fixture: {path}")

    for filename, (kind, after_mdat, expected_sha256, phase, error_type) in (
        ERROR_FIXTURES.items()
    ):
        candidate = fixture_bytes(source, kind, after_mdat, expected_sha256)
        verify_pillow_error(candidate, phase, error_type)

        path = output_dir / filename
        if check:
            if not path.is_file() or path.read_bytes() != candidate:
                raise RuntimeError(f"fixture is missing or differs from generated bytes: {path}")
        elif path.exists():
            if path.read_bytes() != candidate:
                raise RuntimeError(f"refusing to replace a different fixture at {path}")
        else:
            path.write_bytes(candidate)
        print(f"Verified AVIF duplicate error fixture: {path}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT_DIR)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    generate(arguments.output_dir, arguments.check)


if __name__ == "__main__":
    main()
