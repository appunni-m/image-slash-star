#!/usr/bin/env python3
"""Generate animated AVIF parity fixtures for media-box edge cases.

The successful mutation preserves the encoded payload while adding repeated
media headers and sample tables. Two error mutations add a repeated ``minf``
or ``stco`` box. Absolute ``iloc`` and ``stco`` offsets are rebased whenever
the rebuilt movie moves the ``mdat`` payload.
"""

from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
from pathlib import Path
import struct

import yaml
from PIL import Image, UnidentifiedImageError

from avif_bmff_boxes import pack_box, read_boxes, rewrite_boxes
from generate_decode_refs import verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
SOURCE = ASSETS / "animated.avif"
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"

SUCCESS_FILENAME = "animated_duplicate_media_boxes.avif"
SUCCESS_SHA256 = "340adc83f1288ad23d5efe2511a8b100381d862108210c7b3d4b5ef57a5def87"
SUCCESS_FRAME_SHA256 = (
    "426020a90d0efab71b689d180a1b3c8c5e296619771fbbf7ed586e963702342e",
    "b713db5eaea775f35de7df5aef10d4f2cc4c25a3bccc215ccd13d9be528b43e7",
    "c6ae1de6153af976c7f680e5149065725a6ba88d81984ed061638f2b2cc42679",
    "bb83d486de48dc0b03563d6c3d3284e1af3ee4d30f35529617a29deeb92c5746",
    "7698b34acbf12356e47dc8e8eea3bb75df4537006bfc5fb120c475932329eb85",
)

DUPLICATE_MINF_FILENAME = "error_animated_duplicate_minf.avif"
DUPLICATE_MINF_SHA256 = "cfc1d8068dc74494499a8a6edec5d84a95d0d621c0eb5206b5664de872fe0cff"
DUPLICATE_STCO_FILENAME = "error_animated_duplicate_stco.avif"
DUPLICATE_STCO_SHA256 = "85f8cc8cc87af8748399bece4a318996ae73c6152d7f4551556ab9be1f7d1c61"
MISSING_HANDLER_FILENAME = "animated_missing_hdlr.avif"
MISSING_HANDLER_SHA256 = "eb3369448430e7ad13614ee1e7c9caea16cb885667297114ca2a21747326f891"
MISSING_MDHD_FILENAME = "error_animated_missing_mdhd.avif"
MISSING_MDHD_SHA256 = "541dd9dceb7270214d1a0d12d2b0a3b9956f1f026798390f865864975941ca2f"
MISSING_MINF_FILENAME = "error_animated_missing_minf.avif"
MISSING_MINF_SHA256 = "cadba57a37368f95d4deea057f50b55ab7e41ca43babfb02bb1d1e4d39a5b147"

MDIA_PATH = (b"moov", b"trak", b"mdia")
MINF_PATH = MDIA_PATH + (b"minf",)
STBL_PATH = MINF_PATH + (b"stbl",)
STCO_PATH = STBL_PATH + (b"stco",)
ILOC_PATH = (b"meta", b"iloc")
EXPECTED_STCO = bytes.fromhex("0000000000000001000003ff")
EXPECTED_ILOC = bytes.fromhex("0000000044000001000100000001000003ff00000027")


def boxes_at_path(data: bytes, path: tuple[bytes, ...]) -> tuple[bytes, bytes]:
    """Return the unique complete box and payload at a nested box path."""
    if not path:
        raise ValueError("box path cannot be empty")
    matches = [box for box in read_boxes(data, 0, len(data)) if box[0] == path[0]]
    if len(matches) != 1:
        raise RuntimeError(f"expected one {path[0]!r} box, found {len(matches)}")
    kind, complete, payload = matches[0]
    if len(path) == 1:
        return complete, payload
    if kind == b"meta":
        if payload[:4] != bytes(4):
            raise RuntimeError("pinned meta must be a version-zero FullBox")
        payload = payload[4:]
    return boxes_at_path(payload, path[1:])


def mdat_payload(data: bytes) -> tuple[int, bytes]:
    """Return the payload offset and bytes of the final top-level ``mdat``."""
    boxes = read_boxes(data, 0, len(data))
    if [kind for kind, _, _ in boxes] != [b"ftyp", b"meta", b"moov", b"mdat"]:
        raise RuntimeError("pinned animated AVIF box layout changed")
    offset = sum(len(complete) for _, complete, _ in boxes[:3]) + 8
    return offset, boxes[3][2]


def adjust_stco(payload: bytes, growth: int) -> bytes:
    """Move every 32-bit chunk offset with the ``mdat`` payload."""
    if len(payload) < 8 or payload[:4] != bytes(4):
        raise RuntimeError("pinned stco must be a version-zero FullBox")
    count = struct.unpack_from(">I", payload, 4)[0]
    if len(payload) != 8 + count * 4:
        raise RuntimeError("pinned stco record count changed")
    result = bytearray(payload)
    for index in range(count):
        offset = 8 + index * 4
        value = struct.unpack_from(">I", result, offset)[0]
        adjusted = value + growth
        if adjusted > 0xFFFF_FFFF:
            raise RuntimeError("rebased stco offset exceeds its 32-bit field")
        struct.pack_into(">I", result, offset, adjusted)
    return bytes(result)


def adjust_iloc(payload: bytes, growth: int) -> bytes:
    """Move the pinned primary item extent with the ``mdat`` payload."""
    if payload != EXPECTED_ILOC:
        raise RuntimeError("pinned primary-item iloc changed")
    result = bytearray(payload)
    offset = struct.unpack_from(">I", result, 14)[0]
    struct.pack_into(">I", result, 14, offset + growth)
    return bytes(result)


def verify_rebased_offsets(
    source: bytes,
    candidate: bytes,
    growth: int,
    *,
    duplicate_minf: bool = False,
    duplicate_stco: bool = False,
) -> None:
    """Check absolute sample and primary-item offsets against the new mdat."""
    source_offset, source_mdat = mdat_payload(source)
    candidate_offset, candidate_mdat = mdat_payload(candidate)
    if candidate_offset != source_offset + growth or candidate_mdat != source_mdat:
        raise RuntimeError("box mutation changed or misplaced AV1 sample bytes")

    expected_stco = adjust_stco(boxes_at_path(source, STCO_PATH)[1], growth)
    if duplicate_minf:
        mdia = boxes_at_path(candidate, MDIA_PATH)[1]
        minf_boxes = [box for box in read_boxes(mdia, 0, len(mdia)) if box[0] == b"minf"]
        if len(minf_boxes) != 2:
            raise RuntimeError("duplicate-minf fixture must contain two minf boxes")
        stco_tables = [
            boxes_at_path(complete, (b"minf", b"stbl", b"stco"))[1]
            for _, complete, _ in minf_boxes
        ]
    else:
        stbl = boxes_at_path(candidate, STBL_PATH)[1]
        stco_tables = [
            payload
            for kind, _, payload in read_boxes(stbl, 0, len(stbl))
            if kind == b"stco"
        ]
        expected_count = 2 if duplicate_stco else 1
        if len(stco_tables) != expected_count:
            raise RuntimeError("generated stbl contains an unexpected stco count")
    if any(table != expected_stco for table in stco_tables):
        raise RuntimeError("generated stco offsets do not point to the rebased mdat")
    if struct.unpack_from(">I", expected_stco, 8)[0] != candidate_offset:
        raise RuntimeError("sample chunk offset differs from the mdat payload offset")

    expected_iloc = adjust_iloc(EXPECTED_ILOC, growth)
    if boxes_at_path(candidate, ILOC_PATH)[1] != expected_iloc:
        raise RuntimeError("primary-item iloc offset does not match the rebased mdat")
    if struct.unpack_from(">I", expected_iloc, 14)[0] != candidate_offset:
        raise RuntimeError("primary-item extent differs from the mdat payload offset")


def append_sample_tables(payload: bytes) -> bytes:
    """Append repeated stts, stsz, stss, and stsd tables."""
    boxes = read_boxes(payload, 0, len(payload))
    duplicates = []
    for kind in (b"stts", b"stsz", b"stss", b"stsd"):
        matches = [box for box in boxes if box[0] == kind]
        if len(matches) != 1:
            raise RuntimeError(f"expected one source {kind!r} box")
        duplicate_payload = bytearray(matches[0][2])
        if kind == b"stts":
            if len(duplicate_payload) < 16:
                raise RuntimeError("pinned stts table is too short")
            delta = struct.unpack_from(">I", duplicate_payload, 12)[0]
            if delta != 1:
                raise RuntimeError("pinned first stts sample delta changed")
            struct.pack_into(">I", duplicate_payload, 12, delta + 1)
        duplicates.append(pack_box(kind, bytes(duplicate_payload)))
    return payload + b"".join(duplicates)


def success_candidate(source: bytes) -> bytes:
    """Build a Pillow-accepted file with repeated media and sample boxes."""
    mdhd = boxes_at_path(source, MDIA_PATH + (b"mdhd",))[0]
    _, hdlr_payload = boxes_at_path(source, MDIA_PATH + (b"hdlr",))
    if len(hdlr_payload) < 12:
        raise RuntimeError("pinned hdlr is too short")
    duplicate_handler = bytearray(hdlr_payload)
    duplicate_handler[8:12] = b"auxv"
    duplicate_mdia_boxes = (
        mdhd + pack_box(b"hdlr", bytes(duplicate_handler)) + pack_box(b"free", b"")
    )
    source_stbl = boxes_at_path(source, STBL_PATH)[1]
    duplicate_table_bytes = append_sample_tables(source_stbl)[len(source_stbl) :]
    growth = len(duplicate_mdia_boxes) + len(duplicate_table_bytes)

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == MDIA_PATH:
            return payload + duplicate_mdia_boxes
        if path == STBL_PATH:
            return payload + duplicate_table_bytes
        if path == STCO_PATH:
            return adjust_stco(payload, growth)
        if path == ILOC_PATH:
            return adjust_iloc(payload, growth)
        return payload

    candidate = rewrite_boxes(source, transform)
    candidate_sha256 = sha256(candidate).hexdigest()
    if candidate_sha256 != SUCCESS_SHA256:
        raise RuntimeError(f"generated repeated-table AVIF hash changed: {candidate_sha256}")
    verify_rebased_offsets(source, candidate, growth)
    mdia = boxes_at_path(candidate, MDIA_PATH)[1]
    mdia_boxes = read_boxes(mdia, 0, len(mdia))
    if sum(kind == b"mdhd" for kind, _, _ in mdia_boxes) != 2:
        raise RuntimeError("generated mdia must contain two mdhd boxes")
    handlers = [payload for kind, _, payload in mdia_boxes if kind == b"hdlr"]
    if len(handlers) != 2 or handlers[1][8:12] != b"auxv":
        raise RuntimeError("generated mdia must retain the first hdlr and add auxv")
    if not any(kind == b"free" for kind, _, _ in mdia_boxes):
        raise RuntimeError("generated mdia must contain the unknown free child")
    candidate_stbl = boxes_at_path(candidate, STBL_PATH)[1]
    stbl_boxes = read_boxes(candidate_stbl, 0, len(candidate_stbl))
    for kind in (b"stts", b"stsz", b"stss", b"stsd"):
        if sum(box_kind == kind for box_kind, _, _ in stbl_boxes) != 2:
            raise RuntimeError(f"generated stbl must contain two {kind!r} boxes")
    return candidate


def duplicate_minf_candidate(source: bytes) -> bytes:
    """Build a Pillow-rejected file with a duplicate media-information box."""
    growth = len(boxes_at_path(source, MINF_PATH)[0])

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == STCO_PATH:
            return adjust_stco(payload, growth)
        if path == ILOC_PATH:
            return adjust_iloc(payload, growth)
        if path == MDIA_PATH:
            minf_boxes = [box for box in read_boxes(payload, 0, len(payload)) if box[0] == b"minf"]
            if len(minf_boxes) != 1:
                raise RuntimeError("expected one source minf box")
            return payload + minf_boxes[0][1]
        return payload

    candidate = rewrite_boxes(source, transform)
    if sha256(candidate).hexdigest() != DUPLICATE_MINF_SHA256:
        raise RuntimeError("generated duplicate-minf AVIF hash changed")
    verify_rebased_offsets(source, candidate, growth, duplicate_minf=True)
    return candidate


def duplicate_stco_candidate(source: bytes) -> bytes:
    """Build a Pillow-rejected file with duplicate chunk-offset tables."""
    growth = len(boxes_at_path(source, STCO_PATH)[0])

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == STCO_PATH:
            return adjust_stco(payload, growth)
        if path == ILOC_PATH:
            return adjust_iloc(payload, growth)
        if path == STBL_PATH:
            stco_boxes = [box for box in read_boxes(payload, 0, len(payload)) if box[0] == b"stco"]
            if len(stco_boxes) != 1:
                raise RuntimeError("expected one source stco box")
            return payload + stco_boxes[0][1]
        return payload

    candidate = rewrite_boxes(source, transform)
    if sha256(candidate).hexdigest() != DUPLICATE_STCO_SHA256:
        raise RuntimeError("generated duplicate-stco AVIF hash changed")
    verify_rebased_offsets(source, candidate, growth, duplicate_stco=True)
    return candidate


def missing_mdia_child_candidate(source: bytes, kind: bytes, expected_sha256: str) -> bytes:
    """Remove one ``mdia`` child and rebase every absolute media offset."""
    removed = boxes_at_path(source, MDIA_PATH + (kind,))[0]
    growth = -len(removed)

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == MDIA_PATH:
            children = read_boxes(payload, 0, len(payload))
            if sum(child_kind == kind for child_kind, _, _ in children) != 1:
                raise RuntimeError(f"expected one source {kind!r} child")
            return b"".join(complete for child_kind, complete, _ in children if child_kind != kind)
        if path == STCO_PATH:
            return adjust_stco(payload, growth)
        if path == ILOC_PATH:
            return adjust_iloc(payload, growth)
        return payload

    candidate = rewrite_boxes(source, transform)
    if sha256(candidate).hexdigest() != expected_sha256:
        raise RuntimeError(f"generated missing-{kind.decode('ascii')} AVIF hash changed")
    verify_rebased_offsets(source, candidate, growth)
    return candidate


def missing_minf_candidate(source: bytes) -> bytes:
    """Remove the sequence sample table while keeping the primary item valid."""
    removed = boxes_at_path(source, MINF_PATH)[0]
    growth = -len(removed)

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == MDIA_PATH:
            children = read_boxes(payload, 0, len(payload))
            if sum(kind == b"minf" for kind, _, _ in children) != 1:
                raise RuntimeError("expected one source minf child")
            return b"".join(complete for kind, complete, _ in children if kind != b"minf")
        if path == ILOC_PATH:
            return adjust_iloc(payload, growth)
        return payload

    candidate = rewrite_boxes(source, transform)
    if sha256(candidate).hexdigest() != MISSING_MINF_SHA256:
        raise RuntimeError("generated missing-minf AVIF hash changed")
    source_offset, source_mdat = mdat_payload(source)
    candidate_offset, candidate_mdat = mdat_payload(candidate)
    if candidate_offset != source_offset + growth or candidate_mdat != source_mdat:
        raise RuntimeError("missing-minf mutation changed or misplaced AV1 sample bytes")
    if boxes_at_path(candidate, ILOC_PATH)[1] != adjust_iloc(boxes_at_path(source, ILOC_PATH)[1], growth):
        raise RuntimeError("primary-item iloc offset does not match the rebased mdat")
    return candidate


def verify_success_with_pillow(source: bytes, candidate: bytes) -> None:
    """Check exact Pillow metadata, frame timing, and RGB bytes."""
    observed_frames = []
    observed_durations = []
    with Image.open(BytesIO(candidate)) as image:
        header = (image.format, image.mode, image.size, image.n_frames, image.is_animated)
        if header != ("AVIF", "RGB", (150, 150), 5, True):
            raise RuntimeError(f"Pillow metadata changed: {header!r}")
        for index in range(image.n_frames):
            image.seek(index)
            observed_frames.append(sha256(image.convert("RGB").tobytes()).hexdigest())
            observed_durations.append(image.info.get("duration"))
    if tuple(observed_frames) != SUCCESS_FRAME_SHA256:
        raise RuntimeError(f"Pillow RGB frame hashes changed: {tuple(observed_frames)!r}")
    if tuple(observed_durations) != (33, 33, 33, 33, 33):
        raise RuntimeError(f"Pillow frame durations changed: {tuple(observed_durations)!r}")

    with Image.open(BytesIO(source)) as original:
        original_frames = []
        for index in range(original.n_frames):
            original.seek(index)
            original_frames.append(sha256(original.convert("RGB").tobytes()).hexdigest())
    if tuple(original_frames) != SUCCESS_FRAME_SHA256:
        raise RuntimeError("pinned animated source no longer matches its RGB frame hashes")

    with Image.open(BytesIO(candidate)) as image:
        image.verify()


def verify_pillow_rejects(candidate: bytes, name: str) -> None:
    """Require Pillow identification to reject one malformed complete file."""
    try:
        with Image.open(BytesIO(candidate)):
            pass
    except UnidentifiedImageError:
        return
    raise RuntimeError(f"Pillow unexpectedly identified {name}")


def verify_pillow_decode_fails(candidate: bytes, name: str) -> None:
    """Require Pillow to fail while decoding one otherwise identifiable file."""
    try:
        with Image.open(BytesIO(candidate)) as image:
            for index in range(image.n_frames):
                image.seek(index)
                image.load()
    except (UnidentifiedImageError, ZeroDivisionError):
        return
    raise RuntimeError(f"Pillow unexpectedly decoded {name}")


def generate(check: bool) -> None:
    """Validate pinned oracle behavior and create or check all six fixtures."""
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_bytes())
    verify_primary_oracle(manifest)
    source = SOURCE.read_bytes()
    if sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("pinned animated AVIF source hash changed")
    if boxes_at_path(source, STCO_PATH)[1] != EXPECTED_STCO:
        raise RuntimeError("pinned animated AVIF stco changed")
    if boxes_at_path(source, ILOC_PATH)[1] != EXPECTED_ILOC:
        raise RuntimeError("pinned animated AVIF iloc changed")

    fixtures = (
        (SUCCESS_FILENAME, success_candidate(source)),
        (DUPLICATE_MINF_FILENAME, duplicate_minf_candidate(source)),
        (DUPLICATE_STCO_FILENAME, duplicate_stco_candidate(source)),
        (
            MISSING_HANDLER_FILENAME,
            missing_mdia_child_candidate(source, b"hdlr", MISSING_HANDLER_SHA256),
        ),
        (
            MISSING_MDHD_FILENAME,
            missing_mdia_child_candidate(source, b"mdhd", MISSING_MDHD_SHA256),
        ),
        (MISSING_MINF_FILENAME, missing_minf_candidate(source)),
    )
    verify_success_with_pillow(source, fixtures[0][1])
    verify_pillow_rejects(fixtures[1][1], DUPLICATE_MINF_FILENAME)
    verify_pillow_rejects(fixtures[2][1], DUPLICATE_STCO_FILENAME)
    verify_success_with_pillow(source, fixtures[3][1])
    verify_pillow_decode_fails(fixtures[4][1], MISSING_MDHD_FILENAME)
    verify_pillow_rejects(fixtures[5][1], MISSING_MINF_FILENAME)

    for filename, data in fixtures:
        path = ASSETS / filename
        if path.exists():
            if path.read_bytes() != data:
                raise RuntimeError(f"refusing to replace a different fixture at {path}")
        elif check:
            raise RuntimeError(f"fixture is missing: {path}")
        else:
            path.write_bytes(data)
        print(f"{path}: {len(data)} bytes, sha256={sha256(data).hexdigest()}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing fixtures")
    arguments = parser.parse_args()
    generate(arguments.check)


if __name__ == "__main__":
    main()
