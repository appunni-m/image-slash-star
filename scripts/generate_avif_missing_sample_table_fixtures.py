#!/usr/bin/env python3
"""Generate parity fixtures with one required AVIF sample table missing."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path

import yaml
from PIL import Image, UnidentifiedImageError

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes
from generate_decode_refs import stable_error_message, verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "animated.avif"
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
SAMPLE_TABLE_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl")
EXPECTED_ERROR = "cannot identify image file <bytes>"

FIXTURES = (
    (
        "animated_missing_stco.avif",
        b"stco",
        "71148f861ce9b342c4f9642216b2344a568c088adc06f6f7c1df6bac0fab60fe",
    ),
    (
        "animated_missing_stsc.avif",
        b"stsc",
        "d09ef902eacd76da8ece326e0aeb5464e68caac10c2b3408461df271bf35be41",
    ),
    (
        "animated_missing_stsd.avif",
        b"stsd",
        "50e398a37b90e7217117bd573b4529a530b479442bc756738e3c5fbce0fdcdf6",
    ),
)


def sample_table_payload(data: bytes) -> bytes:
    """Return the unique sample-table payload in the pinned animated file."""
    payload = data
    for kind in SAMPLE_TABLE_PATH:
        matches = [box for box in read_boxes(payload, 0, len(payload)) if box[0] == kind]
        if len(matches) != 1:
            raise RuntimeError(f"expected one {kind!r} box, found {len(matches)}")
        payload = matches[0][2]
    return payload


def replace_table_with_free(source: bytes, table_kind: bytes) -> bytes:
    """Hide one table while preserving every box size and payload byte."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path != SAMPLE_TABLE_PATH:
            return payload
        children = read_boxes(payload, 0, len(payload))
        rebuilt = []
        for kind, _, child_payload in children:
            if kind == table_kind:
                changed += 1
                rebuilt.append(pack_box(b"free", child_payload))
            else:
                rebuilt.append(pack_box(kind, child_payload))
        return b"".join(rebuilt)

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError(f"expected one same-size {table_kind!r} replacement")

    original_payload = sample_table_payload(source)
    candidate_payload = sample_table_payload(candidate)
    original_tables = read_boxes(original_payload, 0, len(original_payload))
    candidate_tables = read_boxes(candidate_payload, 0, len(candidate_payload))
    original_shape = [(kind, payload) for kind, _, payload in original_tables]
    candidate_shape = [(kind, payload) for kind, _, payload in candidate_tables]
    expected_shape = [
        (b"free" if kind == table_kind else kind, payload)
        for kind, payload in original_shape
    ]
    if candidate_shape != expected_shape:
        raise RuntimeError(f"replacing {table_kind!r} changed unrelated sample-table data")
    return candidate


def verify_open_failure(candidate: bytes, filename: str) -> None:
    """Require Pillow's exact public rejection for the malformed full file."""
    try:
        image = Image.open(BytesIO(candidate))
    except UnidentifiedImageError as error:
        if stable_error_message(error) != EXPECTED_ERROR:
            raise RuntimeError(f"{filename}: Pillow identification diagnostic changed") from error
        return
    image.close()
    raise RuntimeError(f"{filename}: Pillow unexpectedly identified the image")


def generate(output_dir: Path = ASSETS, check: bool = False) -> tuple[Path, ...]:
    """Generate or verify deterministic fixtures against the pinned Pillow oracle."""
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_bytes())
    verify_primary_oracle(manifest)

    source = SOURCE.read_bytes()
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned animated AVIF source hash changed")
    source_tables = sample_table_payload(source)
    if not any(kind == b"stco" for kind, _, _ in read_boxes(source_tables, 0, len(source_tables))):
        raise RuntimeError("pinned animated AVIF no longer has a stco table")

    generated = []
    for filename, table_kind, expected_hash in FIXTURES:
        candidate = replace_table_with_free(source, table_kind)
        actual_hash = sha256(candidate).hexdigest()
        if actual_hash != expected_hash:
            raise RuntimeError(f"{filename}: fixture hash changed to {actual_hash}")
        verify_open_failure(candidate, filename)

        path = output_dir / filename
        if path.exists():
            if path.read_bytes() != candidate:
                raise RuntimeError(f"refusing to replace a different fixture at {path}")
        elif check:
            raise RuntimeError(f"fixture is missing: {path}")
        else:
            output_dir.mkdir(parents=True, exist_ok=True)
            path.write_bytes(candidate)
        print(f"{path}: {len(candidate)} bytes, sha256={actual_hash}")
        generated.append(path)
    return tuple(generated)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing fixtures")
    args = parser.parse_args()
    generate(check=args.check)


if __name__ == "__main__":
    main()
