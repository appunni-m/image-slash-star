#!/usr/bin/env python3
"""Generate AVIF container, track, data-reference and parser-budget edge inputs.

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
    "grid": "bae56368b348b1d847e2bfb662522599f0c63dfe62fb68826c9e42a300ff405d",
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
    "trailing_boxes_over_budget.avif": "0a92d1c734bb48ab51ffe4aef1f0c5f5693ca06790058cb9a0c74c4c9eb73971",
    "error_iref_records_over_budget_trailing_byte.avif": "a160f1c9ddc2dd178b79ba152ac834d8887c6ec1921b4d5e904ec9df37c2c11c",
    "error_grid_without_dimg_references.avif": "a283fb5590e31db1f491fe1e010a22fec3f55da99080dd4dcc61b95e89d6316a",
    "error_grid_descriptor_short_extent.avif": "95670f2861407e944391adfdd75bbc97e6ebd010aedda7850dd65a7286cb49e5",
    "error_grid_descriptor_overlong_extent.avif": "e0ef3f8597d7ae703cd5ccf7ee8eb24293a857ea4c1e86c8828a899508e54e4f",
    "error_iloc_zero_item_id.avif": "6628085ef24f7bb3a62eb656950f609d585ebcb5e27b3fd1c2f9d04ad50c06fa",
    "error_infe_zero_item_id.avif": "9cd1623b8c3cf615b708968e90f6b9bb0fb081dbda2185348ebf087db82e7e7f",
    "error_iloc_duplicate_item_id.avif": "37b723c1e183488bbf0ddf7c0cad7ae271f54dfe912b040c4721e0af7e1d48fd",
    "error_iloc_reserved_construction_method_bits.avif": "14045959d327207b120b345879a1c83a6a0cb252526ce9c9a06101244cc60c2f",
    "pitm_version_1_wide_item_id.avif": "985e27b0dbaa32a2835d940ac0729db4b43addf80d207f4598b11d518719874c",
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


def append_trailing_boxes_over_budget(source: bytes) -> bytes:
    """Append valid empty boxes beyond the decoder's 4,096-box budget."""
    root_boxes = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in root_boxes] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("box-budget source must have the baseline still's three root boxes")

    # Keep this at one beyond container.rs::MAX_BOXES so inspect reaches the
    # budget guard only after it has parsed the complete still metadata.
    total_box_count = 4_097
    additional_count = total_box_count - len(root_boxes)
    candidate = source + b"".join(pack_box(b"free", b"") for _ in range(additional_count))
    if len(read_boxes(candidate, 0, len(candidate))) != total_box_count:
        raise RuntimeError("box-budget fixture must contain exactly 4,097 root boxes")
    return candidate


def append_reference_records_over_budget(source: bytes) -> bytes:
    """Append an oversized item-reference table with a malformed trailing byte.

    The still's item-information and property tables consume ten records in
    the sample parser. The 4,088 references exceed its 4,096-record budget
    before the parser retains them. Pillow rejects the malformed trailing
    child while preserving the media bytes.
    """
    root_boxes = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in root_boxes] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("record-budget source must have the baseline still's three root boxes")

    reference_count = 4_088
    reference_payload = (
        struct.pack(">HH", 1, reference_count)
        + struct.pack(">H", 1) * reference_count
    )
    reference_child = pack_box(b"zzzz", reference_payload)
    iref = pack_box(b"iref", bytes(4) + reference_child + b"\x00")
    offset_delta = len(iref)
    changed = {b"meta": 0, b"iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        kind = path[-1]
        if kind == b"iloc":
            if len(payload) != 22 or payload[4:6] != bytes.fromhex("4400"):
                raise RuntimeError("record-budget source iloc layout changed")
            changed[kind] += 1
            offset = struct.unpack_from(">I", payload, 14)[0]
            return payload[:14] + struct.pack(">I", offset + offset_delta) + payload[18:]
        if kind == b"meta":
            changed[kind] += 1
            return payload + iref
        return payload

    candidate = rewrite_boxes(source, transform)
    if changed != {b"meta": 1, b"iloc": 1} or len(candidate) != len(source) + offset_delta:
        raise RuntimeError("record-budget mutation changed unexpected container fields")
    return candidate


def replace_grid_dimg_reference_kind(source: bytes) -> bytes:
    """Replace the grid's dimg reference type while preserving box sizes."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path[-1] != b"iref":
            return payload
        if payload[:4] != bytes(4):
            raise RuntimeError("grid-reference source iref must be version zero")
        rewritten = []
        for kind, _, child_payload in read_boxes(payload, 4, len(payload)):
            if kind == b"dimg":
                changed += 1
                kind = b"zzzz"
            rewritten.append(pack_box(kind, child_payload))
        return payload[:4] + b"".join(rewritten)

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("grid-reference mutation must replace one same-size dimg box")
    return candidate


def shorten_grid_descriptor_extent(source: bytes) -> bytes:
    """Declare only three bytes for the primary grid's eight-byte descriptor."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path[-1] != b"iloc":
            return payload
        expected_prefix = bytes.fromhex("00000000440000050001000000010000027300000008")
        if payload[: len(expected_prefix)] != expected_prefix:
            raise RuntimeError("grid source iloc no longer has its pinned primary extent")
        changed += 1
        return payload[:18] + struct.pack(">I", 3) + payload[22:]

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("short grid descriptor mutation must preserve the file size")
    return candidate


def lengthen_grid_descriptor_extent(source: bytes) -> bytes:
    """Declare one extra byte for the primary grid's eight-byte descriptor."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path[-1] != b"iloc":
            return payload
        expected_prefix = bytes.fromhex("00000000440000050001000000010000027300000008")
        if payload[: len(expected_prefix)] != expected_prefix:
            raise RuntimeError("grid source iloc no longer has its pinned primary extent")
        changed += 1
        return payload[:18] + struct.pack(">I", 9) + payload[22:]

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("overlong grid descriptor mutation must preserve the file size")
    return candidate


def zero_iloc_first_item_id(source: bytes) -> bytes:
    """Set the pinned file-backed iloc's sole item ID to the invalid value zero."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path[-1] != b"iloc":
            return payload
        expected_payload = bytes.fromhex("00000000440000010001000000010000011a00000aeb")
        if payload != expected_payload:
            raise RuntimeError("file source iloc no longer has its pinned single-item layout")
        changed += 1
        return payload[:8] + bytes(2) + payload[10:]

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("zero iloc item-ID mutation must preserve the file size")
    return candidate


def zero_infe_first_item_id(source: bytes) -> bytes:
    """Set the baseline's version-two item-info identifier to zero."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path[-1] != b"iinf":
            return payload
        if payload[:6] != bytes.fromhex("000000000001"):
            raise RuntimeError("file source iinf no longer has its pinned single-entry layout")
        children = read_boxes(payload, 6, len(payload))
        if len(children) != 1 or children[0][0] != b"infe":
            raise RuntimeError("file source iinf no longer contains one infe entry")
        kind, _, item_info = children[0]
        if item_info[:6] != bytes.fromhex("020000000001"):
            raise RuntimeError("file source infe no longer has its pinned version-two item ID")
        changed += 1
        item_info = item_info[:4] + bytes(2) + item_info[6:]
        return payload[:6] + pack_box(kind, item_info)

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("zero infe item-ID mutation must preserve the file size")
    return candidate


def duplicate_iloc_first_item_id(source: bytes) -> bytes:
    """Declare the pinned file-backed iloc item record twice with the same ID."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path[-1] != b"iloc":
            return payload
        expected_payload = bytes.fromhex("00000000440000010001000000010000011a00000aeb")
        if payload != expected_payload:
            raise RuntimeError("file source iloc no longer has its pinned single-item layout")
        changed += 1
        item_record = payload[8:]
        return payload[:6] + struct.pack(">H", 2) + item_record + item_record

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source) + 14:
        raise RuntimeError("duplicate iloc item-ID mutation changed an unexpected file extent")
    return candidate


def set_iloc_reserved_construction_method_bit(source: bytes) -> bytes:
    """Set one reserved iloc construction-method bit without changing media."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path[-1] != b"iloc":
            return payload
        expected_payload = bytes.fromhex(
            "0100000044440001000100010000000000080001000000000000000800000aeb"
        )
        if payload != expected_payload:
            raise RuntimeError("indexed-idat iloc no longer has its pinned one-item layout")
        changed += 1
        construction = int.from_bytes(payload[10:12], "big")
        return payload[:10] + struct.pack(">H", construction | 0x0010) + payload[12:]

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("reserved iloc construction mutation changed an unexpected file extent")
    return candidate


def use_version_one_pitm_wide_item_id(source: bytes) -> bytes:
    """Encode the pinned primary item ID in pitm's version-one 32-bit field."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path[-1] != b"pitm":
            return payload
        if payload != bytes.fromhex("000000000001"):
            raise RuntimeError("indexed-idat pitm no longer has its pinned v0 item ID")
        changed += 1
        return bytes.fromhex("0100000000000001")

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source) + 2:
        raise RuntimeError("version-one pitm mutation changed an unexpected file extent")
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


def verify_invalid_grid_rejection(source: bytes, filename: str) -> None:
    """Require Pillow's public rejection for a grid with no dimg reference."""
    stream = BytesIO(source)
    stream.name = f"tests/fixtures/input/images/avif/{filename}"
    try:
        with Image.open(stream) as image:
            image.load()
    except RuntimeError as error:
        if stable_error_message(str(error)) != "Failed to decode image: Invalid image grid":
            raise RuntimeError(f"{filename}: Pillow grid diagnostic changed") from error
        return
    raise RuntimeError(f"{filename}: Pillow unexpectedly decoded a grid without dimg references")


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
    check: bool = False,
) -> tuple[Path, ...]:
    """Generate or verify inputs against live pinned Pillow observations."""
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_bytes())
    verify_primary_oracle(manifest)
    sources = {
        "file": file_source.read_bytes(),
        "idat": idat_source.read_bytes(),
        "sequence": sequence_source.read_bytes(),
        "grid": (DEFAULT_OUTPUT_DIR / "grid.avif").read_bytes(),
    }
    for name, data in sources.items():
        if hashlib.sha256(data).hexdigest() != SOURCE_SHA256[name]:
            raise RuntimeError(f"{name} container-edge source differs from its pinned hash")
    references = {name: pillow_snapshot(data) for name, data in sources.items()}
    variants = [
        (
            "trailing_boxes_over_budget.avif",
            "file",
            append_trailing_boxes_over_budget(sources["file"]),
            None,
        ),
        (
            "error_iref_records_over_budget_trailing_byte.avif",
            "file",
            append_reference_records_over_budget(sources["file"]),
            "open_error",
        ),
        (
            "error_iloc_zero_item_id.avif",
            "file",
            zero_iloc_first_item_id(sources["file"]),
            "open_error",
        ),
        (
            "error_infe_zero_item_id.avif",
            "file",
            zero_infe_first_item_id(sources["file"]),
            "open_error",
        ),
        (
            "error_iloc_duplicate_item_id.avif",
            "file",
            duplicate_iloc_first_item_id(sources["file"]),
            "open_error",
        ),
        (
            "error_iloc_reserved_construction_method_bits.avif",
            "idat",
            set_iloc_reserved_construction_method_bit(sources["idat"]),
            "open_error",
        ),
        (
            "pitm_version_1_wide_item_id.avif",
            "idat",
            use_version_one_pitm_wide_item_id(sources["idat"]),
            None,
        ),
        (
            "error_grid_without_dimg_references.avif",
            "grid",
            replace_grid_dimg_reference_kind(sources["grid"]),
            "decode_error",
        ),
        (
            "error_grid_descriptor_short_extent.avif",
            "grid",
            shorten_grid_descriptor_extent(sources["grid"]),
            "open_error",
        ),
        (
            "error_grid_descriptor_overlong_extent.avif",
            "grid",
            lengthen_grid_descriptor_extent(sources["grid"]),
            "open_error",
        ),
        (
            "animated_tkhd_version_zero.avif",
            "sequence",
            use_version_zero_track_header(sources["sequence"], False),
            None,
        ),
        (
            "animated_tkhd_version_zero_unknown_duration.avif",
            "sequence",
            use_version_zero_track_header(sources["sequence"], True),
            None,
        ),
        (
            "ipma_optional_zero_index.avif",
            "idat",
            append_property_index(sources["idat"], 0),
            None,
        ),
        (
            "ipma_essential_zero_index.avif",
            "idat",
            append_property_index(sources["idat"], 0x80),
            "open_error",
        ),
        (
            "error_ipma_property_index_out_of_bounds.avif",
            "idat",
            append_property_index(sources["idat"], 0x7F),
            "open_error",
        ),
    ]
    for name, idat in (("file", False), ("idat", True)):
        for value in (1, 0xFFFF):
            filename = f"iloc_{name}_data_reference_{value}.avif"
            variants.append((filename, name, set_data_reference(sources[name], idat, value), None))
    for filename, source_name, data, oracle_error in variants:
        if hashlib.sha256(data).hexdigest() != FIXTURE_SHA256[filename]:
            raise RuntimeError(f"{filename}: deterministic mutation differs from its pinned hash")
        if encoded_media_payloads(data) != encoded_media_payloads(sources[source_name]):
            raise RuntimeError(f"{filename}: mutation changed encoded AV1 media bytes")
        if oracle_error == "open_error":
            verify_open_failure(data, filename)
        elif oracle_error == "decode_error":
            verify_invalid_grid_rejection(data, filename)
        elif oracle_error is None and pillow_snapshot(data) != references[source_name]:
            raise RuntimeError(f"{filename}: Pillow image/frame/metadata bytes changed")
        elif oracle_error is not None:
            raise RuntimeError(f"{filename}: unsupported oracle outcome {oracle_error!r}")
        path = output_dir / filename
        if check and (not path.exists() or path.read_bytes() != data):
            raise RuntimeError(f"fixture is missing or differs from its deterministic input: {path}")
        if not check and path.exists() and path.read_bytes() != data:
            raise RuntimeError(f"refusing to replace a different fixture at {path}")
    if not check:
        output_dir.mkdir(parents=True, exist_ok=True)
    for filename, _, data, _ in variants:
        path = output_dir / filename
        if not check and not path.exists():
            path.write_bytes(data)
        action = "Verified" if check else "Generated or verified"
        print(f"{action} AVIF container-edge fixture: {path}")
    return tuple(output_dir / filename for filename, _, _, _ in variants)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--file-source", type=Path, default=FILE_SOURCE)
    parser.add_argument("--idat-source", type=Path, default=IDAT_SOURCE)
    parser.add_argument("--sequence-source", type=Path, default=SEQUENCE_SOURCE)
    parser.add_argument("--check", action="store_true", help="verify fixtures without writing them")
    args = parser.parse_args()
    generate(args.output_dir, args.file_source, args.idat_source, args.sequence_source, args.check)


if __name__ == "__main__":
    main()
