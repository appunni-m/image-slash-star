#!/usr/bin/env python3
"""Add a conflicting duplicate nclx property to a pinned AVIS sample entry."""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path
import struct

from PIL import Image, UnidentifiedImageError
import yaml

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes
from generate_decode_refs import (
    avif_detection_oracle,
    decode_error_kind,
    is_pillow_codec_exception,
    verify_primary_oracle,
)


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/avif"
SOURCE = ASSETS / "animated_sequence_only_no_meta.avif"
SOURCE_SHA256 = "c861761e45518018150dc52302da7ff5015d1f259ff2c1309eea94eaf8176fdb"
OUTPUT = ASSETS / "error_animated_duplicate_sample_description_colr.avif"
OUTPUT_SHA256 = "99f4051b60a85669670f9664d20a061a57a7e8ad00115219ab34bfb2ef65c396"
SAMPLE_DESCRIPTION_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stsd")
CHUNK_OFFSETS_PATH = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stco")
VISUAL_SAMPLE_ENTRY_SIZE = 78
DUPLICATE_NCLX = b"nclx" + struct.pack(">HHHB", 9, 16, 9, 0)
DUPLICATE_COLOR_BOX = pack_box(b"colr", DUPLICATE_NCLX)


def unique_payload(
    boxes: list[tuple[bytes, bytes, bytes]], kind: bytes, context: str
) -> bytes:
    """Return one box payload after checking its pinned cardinality."""
    matches = [payload for box_kind, _, payload in boxes if box_kind == kind]
    if len(matches) != 1:
        raise RuntimeError(f"expected one {kind!r} box in {context}, found {len(matches)}")
    return matches[0]


def sample_description(source: bytes) -> bytes:
    """Return the one AV1 sample-entry payload from the pinned source track."""
    top_level = read_boxes(source, 0, len(source))
    moov = unique_payload(top_level, b"moov", "source file")
    tracks = [payload for kind, _, payload in read_boxes(moov, 0, len(moov)) if kind == b"trak"]
    if len(tracks) != 1:
        raise RuntimeError(f"expected one source track, found {len(tracks)}")
    media = unique_payload(read_boxes(tracks[0], 0, len(tracks[0])), b"mdia", "source track")
    media_info = unique_payload(read_boxes(media, 0, len(media)), b"minf", "source media")
    sample_table = unique_payload(
        read_boxes(media_info, 0, len(media_info)), b"stbl", "source media information"
    )
    description = unique_payload(
        read_boxes(sample_table, 0, len(sample_table)), b"stsd", "source sample table"
    )
    if len(description) < 8 or description[4:8] != struct.pack(">I", 1):
        raise RuntimeError("pinned AVIS must have one sample description")
    entries = read_boxes(description, 8, len(description))
    if len(entries) != 1 or entries[0][0] != b"av01":
        raise RuntimeError("pinned AVIS must have one AV1 sample entry")
    return entries[0][2]


def candidate_bytes() -> bytes:
    """Add a conflicting color property and rebase the media chunk offsets."""
    source = SOURCE.read_bytes()
    source_hash = sha256(source).hexdigest()
    if source_hash != SOURCE_SHA256:
        raise RuntimeError(f"pinned AVIS source hash changed: {source_hash}")

    top_level = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in top_level] != [b"ftyp", b"free", b"moov", b"mdat"]:
        raise RuntimeError("pinned AVIS top-level box layout changed")
    sample = sample_description(source)
    if len(sample) < VISUAL_SAMPLE_ENTRY_SIZE:
        raise RuntimeError("pinned AVIS visual sample entry is too short")
    properties = read_boxes(sample, VISUAL_SAMPLE_ENTRY_SIZE, len(sample))
    if [kind for kind, _, _ in properties] != [b"av1C", b"colr", b"ccst"]:
        raise RuntimeError("pinned AVIS sample-entry properties changed")
    if unique_payload(properties, b"colr", "source sample entry") != bytes.fromhex(
        "6e636c780001000d000680"
    ):
        raise RuntimeError("pinned AVIS color property changed")

    offset_delta = len(DUPLICATE_COLOR_BOX)
    changed = {"stsd": 0, "stco": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == SAMPLE_DESCRIPTION_PATH:
            changed["stsd"] += 1
            if len(payload) < 8 or payload[4:8] != struct.pack(">I", 1):
                raise RuntimeError("pinned AVIS sample-description table changed")
            entries = read_boxes(payload, 8, len(payload))
            if len(entries) != 1 or entries[0][0] != b"av01":
                raise RuntimeError("pinned AVIS sample entry changed")
            kind, _, entry_payload = entries[0]
            if entry_payload != sample:
                raise RuntimeError("pinned AVIS sample-entry bytes changed")
            child_boxes = read_boxes(entry_payload, VISUAL_SAMPLE_ENTRY_SIZE, len(entry_payload))
            rebuilt_entry = entry_payload[:VISUAL_SAMPLE_ENTRY_SIZE] + b"".join(
                encoded for _, encoded, _ in child_boxes
            ) + DUPLICATE_COLOR_BOX
            return payload[:8] + pack_box(kind, rebuilt_entry)

        if path == CHUNK_OFFSETS_PATH:
            changed["stco"] += 1
            if len(payload) < 8 or (len(payload) - 8) % 4:
                raise RuntimeError("pinned AVIS chunk-offset table is malformed")
            entry_count = struct.unpack_from(">I", payload, 4)[0]
            if len(payload) != 8 + 4 * entry_count:
                raise RuntimeError("pinned AVIS chunk-offset table count changed")
            result = bytearray(payload)
            for offset in range(8, len(payload), 4):
                current = struct.unpack_from(">I", payload, offset)[0]
                adjusted = current + offset_delta
                if adjusted > 0xFFFF_FFFF:
                    raise RuntimeError("AVIS chunk offset cannot be rebased without overflow")
                struct.pack_into(">I", result, offset, adjusted)
            return bytes(result)

        return payload

    candidate = rewrite_boxes(source, transform)
    if changed != {"stsd": 1, "stco": 1}:
        raise RuntimeError(f"expected one sample description and chunk-offset table: {changed}")
    if len(candidate) != len(source) + offset_delta:
        raise RuntimeError("AVIS mutation changed an unexpected number of bytes")

    result_top_level = read_boxes(candidate, 0, len(candidate))
    if unique_payload(top_level, b"mdat", "source file") != unique_payload(
        result_top_level, b"mdat", "mutated file"
    ):
        raise RuntimeError("AVIS mutation changed the encoded AV1 samples")
    result_sample = sample_description(candidate)
    result_properties = read_boxes(result_sample, VISUAL_SAMPLE_ENTRY_SIZE, len(result_sample))
    if [kind for kind, _, _ in result_properties] != [b"av1C", b"colr", b"ccst", b"colr"]:
        raise RuntimeError("mutated AVIS sample-entry properties differ from the recipe")
    if result_properties[-1][2] != DUPLICATE_NCLX:
        raise RuntimeError("mutated AVIS does not contain the conflicting nclx property")

    digest = sha256(candidate).hexdigest()
    if digest != OUTPUT_SHA256:
        raise RuntimeError(f"mutated AVIS hash changed: {digest}")
    return candidate


def verify_pillow(source: bytes, candidate: bytes) -> None:
    """Confirm Pillow recognizes the AVIS signature and rejects duplicate colr."""
    with Image.open(BytesIO(source)) as image:
        if image.format != "AVIF" or image.n_frames != 5:
            raise RuntimeError("pinned AVIS source no longer opens as a five-frame AVIF")

    if not avif_detection_oracle(candidate):
        raise RuntimeError("mutated AVIS no longer matches Pillow's signature predicate")
    try:
        with Image.open(BytesIO(candidate)) as image:
            image.load()
    except UnidentifiedImageError as error:
        if not is_pillow_codec_exception(error):
            raise RuntimeError("Pillow rejected duplicate colr with an unrecognized failure") from error
        if decode_error_kind(True, error) != "malformed":
            raise RuntimeError("Pillow duplicate-colr failure no longer maps to malformed") from error
    else:
        raise RuntimeError("Pillow unexpectedly accepted duplicate AV1 sample-entry colr")


def generate(*, check: bool) -> None:
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_text(encoding="utf-8"))
    verify_primary_oracle(manifest)
    source = SOURCE.read_bytes()
    candidate = candidate_bytes()
    verify_pillow(source, candidate)
    if check:
        if not OUTPUT.is_file() or OUTPUT.read_bytes() != candidate:
            raise RuntimeError(f"generated fixture is missing or stale: {OUTPUT}")
    elif OUTPUT.exists():
        if OUTPUT.read_bytes() != candidate:
            raise RuntimeError(f"refusing to replace a different fixture: {OUTPUT}")
    else:
        OUTPUT.write_bytes(candidate)
    print(f"{OUTPUT}: {len(candidate)} bytes, sha256={OUTPUT_SHA256}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify fixture bytes without writing")
    args = parser.parse_args()
    generate(check=args.check)


if __name__ == "__main__":
    main()
