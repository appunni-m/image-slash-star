#!/usr/bin/env python3
"""Generate complete AVIF track, association and data-reference edge inputs.

Pinned Pillow accepts version-zero track headers, optional zero property
indices and ignored nonzero data-reference fields without changing its public
image result. Essential zero and out-of-range property indices fail at open.
All mutations preserve the complete AV1 payloads and use bounded BMFF boxes.
"""

from __future__ import annotations

import argparse
from collections.abc import Callable
import hashlib
from io import BytesIO
from pathlib import Path
import struct

import yaml
from PIL import Image, UnidentifiedImageError

from generate_avif_config_disagreement_fixtures import pillow_snapshot
from generate_avif_zero_width_clap_fixture import pack_box, read_boxes
from generate_decode_refs import stable_error_message, verify_primary_oracle


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT_DIR = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
FILE_SOURCE = DEFAULT_OUTPUT_DIR / "baseline.avif"
IDAT_SOURCE = DEFAULT_OUTPUT_DIR / "iloc_idat_indexed_extent.avif"
SEQUENCE_SOURCE = DEFAULT_OUTPUT_DIR / "animated.avif"
SOURCE_SHA256 = {
    "file": "d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2",
    "idat": "d90e970b5b9570c5091df64871d58a8f500c72df294c456037fad90cc1d92aac",
    "sequence": "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d",
}
FIXTURE_SHA256 = {
    "animated_tkhd_version_zero.avif": "eb7b131fb8288fbe8e67daf9e41e7f80bdf3f923d474177fac84a0b58550b674",
    "animated_tkhd_version_zero_unknown_duration.avif": "ca1834317cd4b5b0db00130370cf3db3c94426ff7479c6414c4da2cabfef2a09",
    "ipma_optional_zero_index.avif": "590514b40d0df05335861bf4345eb99068dc1264d38715f23f8e931c5376aaf3",
    "ipma_essential_zero_index.avif": "b4f6f62f44d85c84a27bd98c1ad9529453c7ce31fa88602fc88eae0641b06c80",
    "error_ipma_property_index_out_of_bounds.avif": "ca46ccb69a865f237092cf3ed7fb77862fa3faff112d311f860ecfb91eec77dd",
    "iloc_file_data_reference_1.avif": "d4b06f63b3bd3ce2bd409930b7c2892236000ff4583e70e771f4a21573dbb6af",
    "iloc_file_data_reference_65535.avif": "a9d263c7af3c3d1219cf2187bef5916ace90bf1018dfb6fa58650f99f51be0f7",
    "iloc_idat_data_reference_1.avif": "b0689513b0693085ef11397fa967bcc56741ae79b80d8a81d4ef424ce561bea2",
    "iloc_idat_data_reference_65535.avif": "756ea5b7a21c472cee85913fbb64260fccd5267f98900dd2b815760738f07eb4",
}
CONTAINER_KINDS = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"iprp")


def rewrite_boxes(
    data: bytes,
    transform: Callable[[tuple[bytes, ...], bytes], bytes],
    path: tuple[bytes, ...] = (),
) -> bytes:
    """Rebuild standard-size boxes while preserving every untouched payload."""
    boxes = read_boxes(data, 0, len(data))
    if b"".join(pack_box(kind, payload) for kind, _, payload in boxes) != data:
        raise RuntimeError("container-edge source must use standard-size box headers")
    rebuilt = []
    for kind, _, payload in boxes:
        child_path = path + (kind,)
        if kind == b"meta":
            if payload[:4] != bytes(4):
                raise RuntimeError("container-edge meta must use a version-zero FullBox")
            payload = payload[:4] + rewrite_boxes(payload[4:], transform, child_path)
        elif kind in CONTAINER_KINDS:
            payload = rewrite_boxes(payload, transform, child_path)
        rebuilt.append(pack_box(kind, transform(child_path, payload)))
    return b"".join(rebuilt)


def append_property_index(source: bytes, association: int) -> bytes:
    """Append one bounded primary-item association to the indexed-idat still."""
    if association not in (0, 0x80, 0x7F):
        raise ValueError("association must be optional zero, essential zero or index 127")
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path[-1] != b"ipma":
            return payload
        if payload != bytes.fromhex("000000000000000100010401028304"):
            raise RuntimeError("indexed-idat source property associations changed")
        changed += 1
        # The item remains in idat; growing meta does not move any referenced
        # file extent. Preserve all four existing associations in their order.
        return payload[:10] + bytes((5,)) + payload[11:] + bytes((association,))

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source) + 1:
        raise RuntimeError("property mutation must change exactly one association box")
    return candidate


def use_version_zero_track_header(source: bytes, unknown_duration: bool) -> bytes:
    """Narrow tkhd fields and rebase the unchanged mdat's file references."""
    changed = {b"tkhd": 0, b"stco": 0, b"iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        kind = path[-1]
        if kind == b"tkhd":
            if len(payload) != 96 or payload[:4] != bytes.fromhex("01000001"):
                raise RuntimeError("sequence source track header changed")
            creation, modification = struct.unpack_from(">QQ", payload, 4)
            duration = struct.unpack_from(">Q", payload, 28)[0]
            if max(creation, modification, duration) > 0xFFFF_FFFF:
                raise RuntimeError("sequence tkhd values cannot fit version zero")
            changed[kind] += 1
            return (
                bytes((0,)) + payload[1:4]
                + struct.pack(">II", creation, modification)
                + payload[20:28]
                + struct.pack(">I", 0xFFFF_FFFF if unknown_duration else duration)
                + payload[36:]
            )
        if kind == b"stco":
            if len(payload) != 12 or payload[:8] != bytes.fromhex("0000000000000001"):
                raise RuntimeError("sequence source must have one 32-bit chunk offset")
            changed[kind] += 1
            offset = struct.unpack_from(">I", payload, 8)[0]
            if offset < 12:
                raise RuntimeError("sequence chunk offset cannot be rebased")
            return payload[:8] + struct.pack(">I", offset - 12)
        if kind == b"iloc":
            if len(payload) != 22 or payload[:14] != bytes.fromhex("0000000044000001000100000001"):
                raise RuntimeError("sequence source primary file extent changed")
            changed[kind] += 1
            offset = struct.unpack_from(">I", payload, 14)[0]
            if offset < 12:
                raise RuntimeError("sequence primary extent cannot be rebased")
            return payload[:14] + struct.pack(">I", offset - 12) + payload[18:]
        return payload

    candidate = rewrite_boxes(source, transform)
    if changed != {b"tkhd": 1, b"stco": 1, b"iloc": 1} or len(candidate) != len(source) - 12:
        raise RuntimeError("version-zero track mutation changed unexpected boxes")
    return candidate


def set_data_reference(source: bytes, idat: bool, value: int) -> bytes:
    """Change only the ignored iloc data-reference field for a supported source."""
    if value not in (1, 0xFFFF):
        raise ValueError("data-reference witness must use one or UINT16_MAX")
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path[-1] != b"iloc":
            return payload
        expected = bytes.fromhex(
            "0100000044440001000100010000" if idat else "0000000044000001000100000001"
        )
        if payload[:14] != expected:
            raise RuntimeError("source iloc construction method or extent layout changed")
        start = 12 if idat else 10
        if payload[start:start + 2] != bytes(2):
            raise RuntimeError("source iloc already has a data-reference index")
        changed += 1
        return payload[:start] + struct.pack(">H", value) + payload[start + 2:]

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("data-reference mutation must preserve the full file size")
    return candidate


def verify_open_failure(source: bytes, filename: str) -> None:
    """Require the exact pinned public open rejection for invalid associations."""
    try:
        image = Image.open(BytesIO(source))
    except UnidentifiedImageError as error:
        if stable_error_message(str(error)) != "cannot identify image file <bytes>":
            raise RuntimeError(f"{filename}: Pillow open diagnostic changed") from error
        return
    image.close()
    raise RuntimeError(f"{filename}: Pillow unexpectedly opened an invalid association")


def encoded_media_payloads(source: bytes) -> tuple[tuple[bytes, bytes], ...]:
    """Retain complete mdat and idat bytes, including the active idat item."""
    boxes = read_boxes(source, 0, len(source))
    mdat = [payload for kind, _, payload in boxes if kind == b"mdat"]
    meta = [payload for kind, _, payload in boxes if kind == b"meta"]
    if len(mdat) != 1 or len(meta) != 1 or meta[0][:4] != bytes(4):
        raise RuntimeError("container-edge source must have one mdat and version-zero meta")
    idat = [payload for kind, _, payload in read_boxes(meta[0], 4, len(meta[0])) if kind == b"idat"]
    if len(idat) > 1:
        raise RuntimeError("container-edge source must not have duplicate idat boxes")
    return ((b"mdat", mdat[0]),) + tuple((b"idat", payload) for payload in idat)


def generate(
    output_dir: Path = DEFAULT_OUTPUT_DIR,
    file_source: Path = FILE_SOURCE,
    idat_source: Path = IDAT_SOURCE,
    sequence_source: Path = SEQUENCE_SOURCE,
) -> tuple[Path, ...]:
    """Generate or verify nine inputs against live pinned Pillow observations."""
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_bytes())
    verify_primary_oracle(manifest)
    sources = {
        "file": file_source.read_bytes(),
        "idat": idat_source.read_bytes(),
        "sequence": sequence_source.read_bytes(),
    }
    for name, data in sources.items():
        if hashlib.sha256(data).hexdigest() != SOURCE_SHA256[name]:
            raise RuntimeError(f"{name} container-edge source differs from its pinned hash")
    references = {name: pillow_snapshot(data) for name, data in sources.items()}
    variants = [
        ("animated_tkhd_version_zero.avif", "sequence", use_version_zero_track_header(sources["sequence"], False), False),
        ("animated_tkhd_version_zero_unknown_duration.avif", "sequence", use_version_zero_track_header(sources["sequence"], True), False),
        ("ipma_optional_zero_index.avif", "idat", append_property_index(sources["idat"], 0), False),
        ("ipma_essential_zero_index.avif", "idat", append_property_index(sources["idat"], 0x80), True),
        ("error_ipma_property_index_out_of_bounds.avif", "idat", append_property_index(sources["idat"], 0x7F), True),
    ]
    for name, idat in (("file", False), ("idat", True)):
        for value in (1, 0xFFFF):
            filename = f"iloc_{name}_data_reference_{value}.avif"
            variants.append((filename, name, set_data_reference(sources[name], idat, value), False))
    for filename, source_name, data, open_rejected in variants:
        if hashlib.sha256(data).hexdigest() != FIXTURE_SHA256[filename]:
            raise RuntimeError(f"{filename}: deterministic mutation differs from its pinned hash")
        if encoded_media_payloads(data) != encoded_media_payloads(sources[source_name]):
            raise RuntimeError(f"{filename}: mutation changed encoded AV1 media bytes")
        if open_rejected:
            verify_open_failure(data, filename)
        elif pillow_snapshot(data) != references[source_name]:
            raise RuntimeError(f"{filename}: Pillow image/frame/metadata bytes changed")
        path = output_dir / filename
        if path.exists() and path.read_bytes() != data:
            raise RuntimeError(f"refusing to replace a different fixture at {path}")
    output_dir.mkdir(parents=True, exist_ok=True)
    for filename, _, data, _ in variants:
        path = output_dir / filename
        if not path.exists():
            path.write_bytes(data)
        print(f"Verified AVIF container-edge fixture: {path}")
    return tuple(output_dir / filename for filename, _, _, _ in variants)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--file-source", type=Path, default=FILE_SOURCE)
    parser.add_argument("--idat-source", type=Path, default=IDAT_SOURCE)
    parser.add_argument("--sequence-source", type=Path, default=SEQUENCE_SOURCE)
    args = parser.parse_args()
    generate(args.output_dir, args.file_source, args.idat_source, args.sequence_source)


if __name__ == "__main__":
    main()
