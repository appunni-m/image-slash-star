#!/usr/bin/env python3
"""Generate AVIF container, handler, track, data-reference and parser-budget
edge inputs.

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
ALPHA_SOURCE = DEFAULT_OUTPUT_DIR / "alpha.avif"
SOURCE_SHA256 = {
    "alpha": "b19f57d9421bbd3d0b0706c8fe79cef802aebf106afefa6bddde9de1a07509c9",
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
    "ipma_trailing_payload.avif": "23a534bd7e04ad59d041c782d5983295736ec8ebcfdb43ee9bb0e8d0c5a38602",
    "error_ipma_zero_item_id.avif": "cf1f32e814397234389b0468b40767c549f7346a34533517df92b78c24b4bd58",
    "error_ipma_non_increasing_item_id.avif": "f40d900163475affb71ea9612378035702f5fbebff5fd16ff9a936a094057ede",
    "iloc_file_data_reference_1.avif": "d4b06f63b3bd3ce2bd409930b7c2892236000ff4583e70e771f4a21573dbb6af",
    "iloc_file_data_reference_65535.avif": "a9d263c7af3c3d1219cf2187bef5916ace90bf1018dfb6fa58650f99f51be0f7",
    "iloc_idat_data_reference_1.avif": "b0689513b0693085ef11397fa967bcc56741ae79b80d8a81d4ef424ce561bea2",
    "iloc_idat_data_reference_65535.avif": "756ea5b7a21c472cee85913fbb64260fccd5267f98900dd2b815760738f07eb4",
    "trailing_boxes_over_budget.avif": "0a92d1c734bb48ab51ffe4aef1f0c5f5693ca06790058cb9a0c74c4c9eb73971",
    "trailing_truncated_top_level_header.avif": "70def23bc6c03c8b3b49d79ca2f9fc3d01eeb60aa44fc00088e07ecb8ca7d616",
    "trailing_truncated_top_level_payload.avif": "78c60fa1cc0f453fbe9f6db714eb1f53001b0e7e57e7c7a401a4b4c3e7581816",
    "error_incomplete_still_truncated_top_level_header.avif": "286ab06301e7cd9b2355a651876b6f9935ca37d60fc2de42ed1808b095c15e72",
    "error_avif_missing_meta.avif": "5ed417664c2b5b74bd36591baf392c94a99a16724ee12454fa921d2ef1ff2a8b",
    "error_avis_missing_moov.avif": "5831e8f2c248e9cec39f80cd7856f95135be889e13a138fe436587f061321cf4",
    "error_avis_moov_without_trak.avif": "b7bad81724df0c975c0ba70935f07ade291f5aa55f9b539b230bbb7bfe00562f",
    "error_pitm_zero_item_id.avif": "c85de8c2432106246e3135cb873eaf4d48ec94ff7b1d34b69e77cb2e119c126d",
    "error_iinf_duplicate_item_id.avif": "b80ecc8c82022162c98d17e84047f9c6a5988345eb702e41445faf33469994e1",
    "error_iref_records_over_budget_trailing_byte.avif": "a160f1c9ddc2dd178b79ba152ac834d8887c6ec1921b4d5e904ec9df37c2c11c",
    "error_iref_zero_from_id.avif": "1ca57926bb34b675c86199ec41782bf0e1790d79c787443f77ee9c7e24c34380",
    "error_iref_zero_to_id.avif": "d30d90de12cf354712ae84ace87a95ea1d7879d943fd594eb87ce65c246a3297",
    "error_iref_trailing_payload.avif": "73e5d173083b327a269280cc67842f3dc34222d111dc6e9363ece4e169dd4600",
    "duplicate_alpha_auxl_reference_first_wins.avif": "680204d0a9bb9dd0f058b2539e0ccb527c265f6b2ee5e863f2f3674fbe9d03a6",
    "error_grid_without_dimg_references.avif": "a283fb5590e31db1f491fe1e010a22fec3f55da99080dd4dcc61b95e89d6316a",
    "error_grid_descriptor_short_extent.avif": "95670f2861407e944391adfdd75bbc97e6ebd010aedda7850dd65a7286cb49e5",
    "error_grid_descriptor_overlong_extent.avif": "e0ef3f8597d7ae703cd5ccf7ee8eb24293a857ea4c1e86c8828a899508e54e4f",
    "error_grid_version_1.avif": "6d75c008f18bf3c509c79ba810656a0e71d0f1da05c890456af598e51172ddeb",
    "error_grid_zero_width.avif": "37b53b695d8d8a97ddba92087ad15de6c75635c970c2dd94d037b6ad6f754bf2",
    "error_grid_zero_height.avif": "3f2fd4d6f618c5527c25a1750b508c5ed7122f69b4a7c44a7c3bd9c4fb5292b6",
    "error_primary_item_missing_ispe.avif": "475d45c0c712b011983d6d946a2e3ce4512d7968664853b0a6953fa7da0c06f7",
    "error_ftyp_misaligned_compatible_brand_payload.avif": "ada44e071b8609bc68b65ce123df4dac04b21e4b64d0865ec699bc7425400fc3",
    "ftyp_1025_compatible_brands.avif": "4ded572af8b55afd4d876e8a129c651074a6b55e1cb4a01646a144a92fa9bcec",
    "error_iloc_zero_item_id.avif": "6628085ef24f7bb3a62eb656950f609d585ebcb5e27b3fd1c2f9d04ad50c06fa",
    "error_infe_zero_item_id.avif": "9cd1623b8c3cf615b708968e90f6b9bb0fb081dbda2185348ebf087db82e7e7f",
    "error_infe_missing_item_name_terminator.avif": "b8609cd7fd3acd8c4f7eacad072ed85dcce52c65df9ebd9760ab1e1a8328620d",
    "error_meta_duplicate_idat.avif": "466e3ecd6e89e4cbd2dba8084642fc2ddbda83e196e005819a31aa54cc3c960b",
    "error_meta_hdlr_nonzero_version.avif": "e5d7f45c33cc3aec2868d57bddaad27b62df23210326822382f81e2cb344dc62",
    "error_meta_hdlr_nonzero_predefined.avif": "b9119779779df0addfdfcd92927ae06127109846796ca6806c1a0bc20ce7391d",
    "error_meta_reordered_handler.avif": "75d67ac340bfec40e5bce5e0f5d19d769053ec5fe774989fd56cb90b32392516",
    "error_auxc_nonzero_version.avif": "2357cdc2bc511390a97c6f84cdf1bbf03b0debe3b7bda4090b397d7bd20c3822",
    "error_pixi_zero_depth.avif": "7c815a023acc909df7a21d80d11aa1ca92aaae6ce0748bfc989288cae91d3c5f",
    "error_pixi_nonzero_version.avif": "a50bd4126788de5efdeefa05ec6583267311fe021af9168928850589d9fd5689",
    "error_pixi_zero_planes.avif": "b29a15defd39037aebaa7dffdd878c828201d1d9adc852302e8c76b123036431",
    "error_pixi_depth_above_16.avif": "bdf5ecf32956f1c05ed93c9b3e5a9fb2381f5429c1996ccb6b5701201fdbd34d",
    "error_pixi_channel_depth_mismatch.avif": "420568bd6387e4262a2b3076a466b0d39efe35f9c01c99a66364352389b92242",
    "error_iloc_duplicate_item_id.avif": "37b723c1e183488bbf0ddf7c0cad7ae271f54dfe912b040c4721e0af7e1d48fd",
    "error_iloc_reserved_construction_method_bits.avif": "14045959d327207b120b345879a1c83a6a0cb252526ce9c9a06101244cc60c2f",
    "pitm_version_1_wide_item_id.avif": "985e27b0dbaa32a2835d940ac0729db4b43addf80d207f4598b11d518719874c",
}
BASELINE_PIXI_PAYLOAD = bytes.fromhex("0000000003080808")
BASELINE_ALPHA_AUXC_PAYLOAD = bytes(4) + b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha\0"
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


def remove_primary_ispe_association(source: bytes) -> bytes:
    """Clear the baseline primary item's spatial-extent association."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path != (b"meta", b"iprp", b"ipma"):
            return payload
        if payload != bytes.fromhex("000000000000000100010401028304"):
            raise RuntimeError("file source primary-item property associations changed")
        changed += 1
        return payload[:11] + b"\0" + payload[12:]

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("missing-ispe mutation must change one association without resizing")
    if sum(left != right for left, right in zip(candidate, source, strict=True)) != 1:
        raise RuntimeError("missing-ispe mutation must change exactly one source byte")
    return candidate


def append_ipma_trailing_payload(source: bytes) -> bytes:
    """Append one ignored byte after the indexed-idat ipma records."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path[-1] != b"ipma":
            return payload
        if payload != bytes.fromhex("000000000000000100010401028304"):
            raise RuntimeError("indexed-idat source property associations changed")
        changed += 1
        return payload + b"\0"

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source) + 1:
        raise RuntimeError("ipma trailing-payload mutation changed an unexpected extent")
    return candidate


def set_ipma_item_id_zero(source: bytes) -> bytes:
    """Set the indexed-idat primary item's ipma identifier to zero."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path[-1] != b"ipma":
            return payload
        if payload != bytes.fromhex("000000000000000100010401028304"):
            raise RuntimeError("indexed-idat source property associations changed")
        changed += 1
        return payload[:8] + bytes(2) + payload[10:]

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("zero ipma item ID mutation must preserve the file size")
    return candidate


def append_duplicate_ipma_item_id(source: bytes) -> bytes:
    """Append a second ipma record with the existing item ID and no associations."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path[-1] != b"ipma":
            return payload
        if payload != bytes.fromhex("000000000000000100010401028304"):
            raise RuntimeError("indexed-idat source property associations changed")
        changed += 1
        return payload[:4] + (2).to_bytes(4, "big") + payload[8:] + bytes.fromhex("000100")

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source) + 3:
        raise RuntimeError("duplicate ipma item ID mutation changed an unexpected extent")
    return candidate


def zero_iref_item_id(source: bytes, *, source_id: bool) -> bytes:
    """Zero one endpoint in alpha.avif's pinned version-zero auxl reference."""
    if not isinstance(source_id, bool):
        raise TypeError("source_id must be a boolean")
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path != (b"meta", b"iref"):
            return payload
        if payload != bytes.fromhex("000000000000000e6175786c000200010001"):
            raise RuntimeError("alpha source item reference differs from its pinned layout")
        changed += 1
        start = 12 if source_id else 16
        return payload[:start] + bytes(2) + payload[start + 2 :]

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("zero iref item-ID mutation must preserve the file size")
    return candidate


def alpha_file_item_payloads(data: bytes) -> tuple[tuple[int, bytes], ...]:
    """Read bounded file-backed extents from the pinned alpha fixture layout."""
    top_level = read_boxes(data, 0, len(data))
    metas = [payload for kind, _, payload in top_level if kind == b"meta"]
    if len(metas) != 1 or metas[0][:4] != bytes(4):
        raise RuntimeError("expected one version-zero alpha meta box")
    children = read_boxes(metas[0], 4, len(metas[0]))
    locations = [payload for kind, _, payload in children if kind == b"iloc"]
    if len(locations) != 1 or locations[0][4:6] != bytes.fromhex("4400"):
        raise RuntimeError("alpha file extents differ from the pinned iloc layout")
    payload = locations[0]
    item_count = int.from_bytes(payload[6:8], "big")
    position = 8
    result = []
    for _ in range(item_count):
        item_id = int.from_bytes(payload[position : position + 2], "big")
        position += 2
        data_reference_index = int.from_bytes(payload[position : position + 2], "big")
        position += 2
        extent_count = int.from_bytes(payload[position : position + 2], "big")
        position += 2
        if data_reference_index != 0 or extent_count != 1:
            raise RuntimeError("alpha file extents are not local single extents")
        extent_offset = int.from_bytes(payload[position : position + 4], "big")
        position += 4
        extent_length = int.from_bytes(payload[position : position + 4], "big")
        position += 4
        extent = data[extent_offset : extent_offset + extent_length]
        if len(extent) != extent_length:
            raise RuntimeError("alpha file extent exceeds its complete input")
        result.append((item_id, extent))
    if position != len(payload):
        raise RuntimeError("alpha file extents contain unexpected trailing data")
    return tuple(result)


def append_iref_trailing_payload(source: bytes) -> bytes:
    """Append an ignored byte after alpha.avif's declared auxl reference."""
    changed = {"iref": 0, "iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == (b"meta", b"iref"):
            boxes = read_boxes(payload, 4, len(payload))
            if (
                payload[:4] != bytes(4)
                or len(boxes) != 1
                or boxes[0][0] != b"auxl"
                or boxes[0][2] != bytes.fromhex("000200010001")
            ):
                raise RuntimeError("alpha source item reference differs from its pinned layout")
            changed["iref"] += 1
            return payload[:4] + pack_box(b"auxl", boxes[0][2] + b"\0")
        if path == (b"meta", b"iloc"):
            if payload != bytes.fromhex(
                "0000000044000002000100000001000002d700001652"
                "000200000001000001c90000010e"
            ):
                raise RuntimeError("alpha source item locations differ from their pinned layout")
            changed["iloc"] += 1
            result = bytearray(payload)
            position = 8
            item_count = int.from_bytes(payload[6:8], "big")
            for _ in range(item_count):
                position += 2
                data_reference_index = int.from_bytes(payload[position : position + 2], "big")
                position += 2
                extent_count = int.from_bytes(payload[position : position + 2], "big")
                position += 2
                if data_reference_index != 0 or extent_count != 1:
                    raise RuntimeError("alpha source item locations are not the pinned file extents")
                source_offset = int.from_bytes(payload[position : position + 4], "big")
                position += 4
                position += 4  # extent length
                result[position - 8 : position - 4] = (source_offset + 1).to_bytes(4, "big")
            if position != len(payload):
                raise RuntimeError("alpha source item locations contain unexpected trailing data")
            return bytes(result)
        return payload

    candidate = rewrite_boxes(source, transform)
    if changed != {"iref": 1, "iloc": 1} or len(candidate) != len(source) + 1:
        raise RuntimeError("iref trailing-payload mutation changed an unexpected extent")
    if alpha_file_item_payloads(candidate) != alpha_file_item_payloads(source):
        raise RuntimeError("iref growth changed a referenced alpha AV1 payload")
    return candidate


def duplicate_alpha_auxl_reference_first_wins(source: bytes) -> bytes:
    """Duplicate alpha.avif's auxl destination while preserving both AV1 items."""
    changed = {"iref": 0, "iloc": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        if path == (b"meta", b"iref"):
            boxes = read_boxes(payload, 4, len(payload))
            if (
                payload[:4] != bytes(4)
                or len(boxes) != 1
                or boxes[0][0] != b"auxl"
                or boxes[0][2] != bytes.fromhex("000200010001")
            ):
                raise RuntimeError("alpha source item reference differs from its pinned layout")
            changed["iref"] += 1
            reference = boxes[0][2]
            reference_count = int.from_bytes(reference[2:4], "big")
            duplicate = (
                reference[:2]
                + (reference_count + 1).to_bytes(2, "big")
                + reference[4:]
                + reference[4:6]
            )
            return payload[:4] + pack_box(b"auxl", duplicate)
        if path == (b"meta", b"iloc"):
            if payload != bytes.fromhex(
                "0000000044000002000100000001000002d700001652"
                "000200000001000001c90000010e"
            ):
                raise RuntimeError("alpha source item locations differ from their pinned layout")
            changed["iloc"] += 1
            result = bytearray(payload)
            position = 8
            item_count = int.from_bytes(payload[6:8], "big")
            for _ in range(item_count):
                position += 2
                data_reference_index = int.from_bytes(payload[position : position + 2], "big")
                position += 2
                extent_count = int.from_bytes(payload[position : position + 2], "big")
                position += 2
                if data_reference_index != 0 or extent_count != 1:
                    raise RuntimeError("alpha source item locations are not the pinned file extents")
                source_offset = int.from_bytes(payload[position : position + 4], "big")
                position += 4
                position += 4  # extent length
                result[position - 8 : position - 4] = (source_offset + 2).to_bytes(4, "big")
            if position != len(payload):
                raise RuntimeError("alpha source item locations contain unexpected trailing data")
            return bytes(result)
        return payload

    candidate = rewrite_boxes(source, transform)
    if changed != {"iref": 1, "iloc": 1} or len(candidate) != len(source) + 2:
        raise RuntimeError("duplicate auxl mutation changed an unexpected extent")
    if alpha_file_item_payloads(candidate) != alpha_file_item_payloads(source):
        raise RuntimeError("duplicate auxl mutation changed a referenced alpha AV1 payload")
    return candidate


def set_ipco_property_byte(
    source: bytes,
    property_kind: bytes,
    baseline_payload: bytes,
    offset: int,
    value: int,
    field: str,
) -> bytes:
    """Set one byte in a pinned property under the baseline ipco box."""
    property_name = property_kind.decode("ascii")
    if not 0 <= value <= 255:
        raise ValueError(f"{property_name} {field} must fit in one byte")
    if not 0 <= offset < len(baseline_payload):
        raise ValueError(f"{property_name} mutation offset must address the baseline property")
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path != (b"meta", b"iprp", b"ipco"):
            return payload
        children = read_boxes(payload, 0, len(payload))
        rebuilt = []
        for kind, _, child in children:
            if kind == property_kind:
                if child != baseline_payload:
                    raise RuntimeError(
                        f"file source {property_name} no longer has its pinned property payload"
                    )
                changed += 1
                child = child[:offset] + bytes((value,)) + child[offset + 1 :]
            rebuilt.append(pack_box(kind, child))
        return b"".join(rebuilt)

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError(f"{property_name} {field} mutation must preserve the file size")
    return candidate


def set_pixi_byte(source: bytes, offset: int, value: int, field: str) -> bytes:
    """Set one byte in the pinned baseline pixi property."""
    return set_ipco_property_byte(
        source, b"pixi", BASELINE_PIXI_PAYLOAD, offset, value, field
    )


def set_pixi_first_channel_depth(source: bytes, depth: int) -> bytes:
    """Set the baseline pixi property's first channel depth."""
    return set_pixi_byte(source, 5, depth, "channel depth")


def zero_pixi_first_channel_depth(source: bytes) -> bytes:
    """Set the baseline pixi property's first channel depth to zero."""
    return set_pixi_first_channel_depth(source, 0)


def set_pixi_plane_count(source: bytes, planes: int) -> bytes:
    """Set the baseline pixi property's plane count."""
    return set_pixi_byte(source, 4, planes, "plane count")


def set_pixi_full_box_version(source: bytes, version: int) -> bytes:
    """Set the baseline pixi property's FullBox version byte."""
    return set_pixi_byte(source, 0, version, "FullBox version")


def set_auxc_full_box_version(source: bytes, version: int) -> bytes:
    """Set the pinned alpha source auxC property's FullBox version byte."""
    return set_ipco_property_byte(
        source,
        b"auxC",
        BASELINE_ALPHA_AUXC_PAYLOAD,
        0,
        version,
        "FullBox version",
    )


def mismatch_pixi_second_channel_depth(source: bytes) -> bytes:
    """Set only the baseline pixi property's second channel depth to seven."""
    return set_pixi_byte(source, 6, 7, "second channel depth")


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


def append_truncated_top_level_header(source: bytes) -> bytes:
    """Append one byte of a top-level box header after the complete still."""
    root_boxes = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in root_boxes] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError(
            "truncated-header source must have the baseline still's three root boxes"
        )
    return source + b"\x00"


def append_truncated_top_level_payload(source: bytes) -> bytes:
    """Append a complete box header whose declared payload is absent."""
    root_boxes = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in root_boxes] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError(
            "truncated-payload source must have the baseline still's three root boxes"
        )
    return source + struct.pack(">I4s", 16, b"free")


def append_ftyp_misaligned_compatible_brand_byte(source: bytes) -> bytes:
    """Append one byte to the ftyp compatible-brand list without shifting items."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path[-1] != b"ftyp":
            return payload
        changed += 1
        return payload + b"\x00"

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source) + 1:
        raise RuntimeError("misaligned ftyp mutation must grow the file by exactly one byte")
    return candidate


def extend_ftyp_compatible_brands(source: bytes, count: int) -> bytes:
    """Append unique compatible brands while preserving every following box."""
    root_boxes = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in root_boxes] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("brand-list source must have ftyp, meta and mdat root boxes")
    ftyp, meta, mdat = (complete for _, complete, _ in root_boxes)
    payload = ftyp[8:]
    if len(payload) < 8 or (len(payload) - 8) % 4:
        raise RuntimeError("brand-list source must have an aligned ftyp payload")
    brands = (len(payload) - 8) // 4
    if not brands < count <= 0xFFF:
        raise RuntimeError("requested brand count must extend the source and fit unique xNNN brands")
    additions = b"".join(
        f"x{index:03x}".encode("ascii") for index in range(count - brands)
    )
    new_ftyp = pack_box(b"ftyp", payload + additions)
    candidate = new_ftyp + meta + mdat
    if len(candidate) != len(source) + len(additions):
        raise RuntimeError("compatible-brand fixture changed bytes outside its ftyp growth")
    if candidate[len(new_ftyp) :] != source[len(ftyp) :]:
        raise RuntimeError("compatible-brand fixture changed metadata or media boxes")
    return candidate


def truncate_before_meta_with_partial_header(source: bytes) -> bytes:
    """Keep the AVIF ftyp box and append one byte of an incomplete next box."""
    root_boxes = read_boxes(source, 0, len(source))
    if [kind for kind, _, _ in root_boxes] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError(
            "incomplete-still source must have the baseline's three root boxes"
        )
    _, ftyp_box, _ = root_boxes[0]
    return ftyp_box + b"\x00"


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


def grid_descriptor_offset(source: bytes) -> int:
    """Return the pinned primary grid descriptor's bounded file offset."""
    top_level = read_boxes(source, 0, len(source))
    metas = [payload for kind, _, payload in top_level if kind == b"meta"]
    if len(metas) != 1 or metas[0][:4] != bytes(4):
        raise RuntimeError("grid source must have one version-zero meta box")
    children = read_boxes(metas[0], 4, len(metas[0]))
    locations = [payload for kind, _, payload in children if kind == b"iloc"]
    prefix = bytes.fromhex("00000000440000050001000000010000027300000008")
    if len(locations) != 1 or locations[0][: len(prefix)] != prefix:
        raise RuntimeError("grid source iloc no longer has its pinned primary extent")
    offset = int.from_bytes(locations[0][14:18], "big")
    length = int.from_bytes(locations[0][18:22], "big")
    if length != 8 or source[offset : offset + length] != bytes.fromhex("0000010000500050"):
        raise RuntimeError("grid source primary descriptor differs from its pinned bytes")
    return offset


def mutate_grid_descriptor(source: bytes, mutation: str) -> bytes:
    """Change one bounded primary-grid descriptor field without shifting media."""
    descriptor_offset = grid_descriptor_offset(source)
    fields = {
        "version": (0, b"\x01"),
        "zero_width": (4, bytes(2)),
        "zero_height": (6, bytes(2)),
    }
    try:
        relative_offset, value = fields[mutation]
    except KeyError as error:
        raise ValueError(f"unsupported grid descriptor mutation {mutation!r}") from error
    result = bytearray(source)
    start = descriptor_offset + relative_offset
    result[start : start + len(value)] = value
    if len(result) != len(source):
        raise RuntimeError("grid descriptor mutation must preserve the full file size")
    return bytes(result)


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


def unterminate_infe_first_item_name(source: bytes) -> bytes:
    """Replace the only terminator in the baseline's first AV1 item name."""
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
        expected = bytes.fromhex("020000000001000061763031436f6c6f7200")
        if item_info != expected:
            raise RuntimeError("file source infe item-name layout changed")
        changed += 1
        return payload[:6] + pack_box(kind, item_info[:-1] + b"X")

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("unterminated infe name mutation must preserve the file size")
    return candidate


def append_empty_duplicate_idat(source: bytes) -> bytes:
    """Append an empty second idat box without changing the active AV1 item."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path != (b"meta",):
            return payload
        if payload[:4] != bytes(4):
            raise RuntimeError("indexed-idat source meta must use a version-zero FullBox")
        children = read_boxes(payload, 4, len(payload))
        if sum(kind == b"idat" for kind, _, _ in children) != 1:
            raise RuntimeError("indexed-idat source must contain one idat box")
        changed += 1
        child_boxes = b"".join(pack_box(kind, child) for kind, _, child in children)
        return payload[:4] + child_boxes + pack_box(b"idat", b"")

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source) + 8:
        raise RuntimeError("duplicate idat mutation must append exactly one empty box")
    return candidate


def set_handler_nonzero_version(source: bytes) -> bytes:
    """Set the baseline meta handler's FullBox version to one."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path != (b"meta", b"hdlr"):
            return payload
        if len(payload) < 24 or payload[:4] != bytes(4):
            raise RuntimeError("file source hdlr no longer has its pinned version-zero layout")
        changed += 1
        return b"\x01" + payload[1:]

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("nonzero hdlr version mutation must preserve the file size")
    return candidate


def set_handler_nonzero_predefined(source: bytes) -> bytes:
    """Set the baseline meta handler's reserved pre_defined field to one."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path != (b"meta", b"hdlr"):
            return payload
        if len(payload) < 24 or payload[:8] != bytes(8):
            raise RuntimeError("file source hdlr no longer has its pinned zero-value fields")
        changed += 1
        return payload[:4] + struct.pack(">I", 1) + payload[8:]

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("nonzero hdlr pre_defined mutation must preserve the file size")
    return candidate


def move_meta_handler_after_primary_item(source: bytes) -> bytes:
    """Move the primary meta handler after pitm without changing other boxes."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path != (b"meta",):
            return payload
        children = read_boxes(payload, 4, len(payload))
        handler_indices = [
            index for index, (kind, _, _) in enumerate(children) if kind == b"hdlr"
        ]
        primary_indices = [
            index for index, (kind, _, _) in enumerate(children) if kind == b"pitm"
        ]
        if len(handler_indices) != 1 or len(primary_indices) != 1:
            raise RuntimeError("file source meta must contain one hdlr and one pitm")
        handler_index = handler_indices[0]
        primary_index = primary_indices[0]
        if handler_index >= primary_index:
            raise RuntimeError("file source meta must place hdlr before pitm")
        handler = children.pop(handler_index)
        primary_index -= 1
        children.insert(primary_index + 1, handler)
        changed += 1
        return payload[:4] + b"".join(
            pack_box(kind, child) for kind, _, child in children
        )

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("reordered meta handler mutation must preserve the file size")
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


def zero_primary_item_id(source: bytes) -> bytes:
    """Set the baseline still image's complete version-zero pitm ID to zero."""
    changed = 0

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        nonlocal changed
        if path != (b"meta", b"pitm"):
            return payload
        if payload != bytes.fromhex("000000000001"):
            raise RuntimeError("primary-item source pitm differs from its pinned layout")
        changed += 1
        return payload[:4] + bytes(2)

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source):
        raise RuntimeError("zero primary-item ID mutation must preserve the complete file")
    return candidate


def duplicate_iinf_first_item(source: bytes) -> bytes:
    """Duplicate the baseline's AV1 item-info entry without changing its ID."""
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
        if item_info != bytes.fromhex("020000000001000061763031436f6c6f7200"):
            raise RuntimeError("file source infe no longer has its pinned primary-item layout")
        changed += 1
        return payload[:4] + (2).to_bytes(2, "big") + pack_box(kind, item_info) * 2

    candidate = rewrite_boxes(source, transform)
    if changed != 1 or len(candidate) != len(source) + 26:
        raise RuntimeError("duplicate item-info mutation must append exactly one infe box")
    return candidate


def verify_open_failure(source: bytes, filename: str) -> None:
    """Require the exact pinned public open rejection for malformed metadata."""
    try:
        image = Image.open(BytesIO(source))
    except UnidentifiedImageError as error:
        if stable_error_message(str(error)) != "cannot identify image file <bytes>":
            raise RuntimeError(f"{filename}: Pillow open diagnostic changed") from error
        return
    image.close()
    raise RuntimeError(f"{filename}: Pillow unexpectedly opened an invalid association")


def verify_open_runtime_failure(source: bytes, filename: str) -> None:
    """Require Pillow's typed AVIF error when a zero pitm cannot select an item."""
    try:
        image = Image.open(BytesIO(source))
    except RuntimeError as error:
        expected = "Failed to decode image: Missing or empty image item"
        if stable_error_message(str(error)) != expected:
            raise RuntimeError(f"{filename}: Pillow primary-item diagnostic changed") from error
        return
    image.close()
    raise RuntimeError(f"{filename}: Pillow unexpectedly opened an image with a zero pitm ID")


def verify_decode_runtime_failure(source: bytes, filename: str, expected: str) -> None:
    """Require the pinned public decode rejection for malformed item metadata."""
    try:
        with Image.open(BytesIO(source)) as image:
            image.load()
    except RuntimeError as error:
        if stable_error_message(error) != expected:
            raise RuntimeError(f"{filename}: Pillow decode diagnostic changed") from error
        return
    raise RuntimeError(f"{filename}: Pillow unexpectedly decoded malformed item metadata")


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


def verify_grid_descriptor_rejection(
    source: bytes, filename: str, expected_message: str
) -> None:
    """Require Pillow's public rejection for an invalid grid descriptor."""
    try:
        with Image.open(BytesIO(source)) as image:
            image.load()
    except RuntimeError as error:
        if stable_error_message(error) != expected_message:
            raise RuntimeError(f"{filename}: Pillow grid diagnostic changed") from error
        return
    raise RuntimeError(f"{filename}: Pillow unexpectedly decoded an invalid grid descriptor")


def verify_pixi_rejection(source: bytes, filename: str, reason: str) -> None:
    """Require Pillow's pinned decode failure for a malformed pixi property."""
    try:
        with Image.open(BytesIO(source)) as image:
            image.load()
    except RuntimeError as error:
        if stable_error_message(error) != "Failed to decode image: Not implemented":
            raise RuntimeError(f"{filename}: Pillow pixi diagnostic changed") from error
        return
    raise RuntimeError(f"{filename}: Pillow unexpectedly decoded {reason}")


def encoded_media_payloads(source: bytes) -> tuple[tuple[bytes, bytes], ...]:
    """Retain complete mdat and idat bytes, including the active idat item."""
    boxes = read_boxes(source, 0, len(source))
    mdat = [payload for kind, _, payload in boxes if kind == b"mdat"]
    meta = [payload for kind, _, payload in boxes if kind == b"meta"]
    if len(mdat) != 1 or len(meta) != 1 or meta[0][:4] != bytes(4):
        raise RuntimeError("container-edge source must have one mdat and version-zero meta")
    idat = [payload for kind, _, payload in read_boxes(meta[0], 4, len(meta[0])) if kind == b"idat"]
    if len(idat) > 1 and any(idat[1:]):
        raise RuntimeError("additional idat boxes must be empty mutation witnesses")
    return ((b"mdat", mdat[0]),) + tuple((b"idat", payload) for payload in idat)


def omit_required_meta(source: bytes) -> bytes:
    """Keep the complete ftyp and mdat boxes while omitting required metadata."""
    boxes = read_boxes(source, 0, len(source))
    file_types = [payload for kind, _, payload in boxes if kind == b"ftyp"]
    metadata = [payload for kind, _, payload in boxes if kind == b"meta"]
    media = [payload for kind, _, payload in boxes if kind == b"mdat"]
    if len(file_types) != 1 or len(metadata) != 1 or len(media) != 1:
        raise RuntimeError("missing-meta source must contain one ftyp, meta and mdat")
    if boxes[0][0] != b"ftyp" or boxes[-1][0] != b"mdat":
        raise RuntimeError("missing-meta source must begin with ftyp and end with mdat")

    candidate = pack_box(b"ftyp", file_types[0]) + pack_box(b"mdat", media[0])
    retained = read_boxes(candidate, 0, len(candidate))
    if tuple(kind for kind, _, _ in retained) != (b"ftyp", b"mdat"):
        raise RuntimeError("missing-meta file must retain only complete ftyp and mdat boxes")
    if retained[0][2] != file_types[0] or retained[1][2] != media[0]:
        raise RuntimeError("missing-meta mutation changed file type or encoded media bytes")
    return candidate


def omit_required_movie(source: bytes) -> bytes:
    """Keep a complete AVIS ftyp and mdat while omitting meta and moov."""
    boxes = read_boxes(source, 0, len(source))
    file_types = [payload for kind, _, payload in boxes if kind == b"ftyp"]
    media = [payload for kind, _, payload in boxes if kind == b"mdat"]
    if len(file_types) != 1 or len(media) != 1:
        raise RuntimeError("missing-moov source must contain one ftyp and mdat")
    if tuple(kind for kind, _, _ in boxes) != (b"ftyp", b"meta", b"moov", b"mdat"):
        raise RuntimeError("missing-moov source boxes differ from the pinned sequence layout")

    source_ftyp = file_types[0]
    compatible = source_ftyp[8:]
    if source_ftyp[:4] != b"avis" or len(compatible) % 4 != 0:
        raise RuntimeError("missing-moov source must use an aligned AVIS file type")
    brands = tuple(compatible[index : index + 4] for index in range(0, len(compatible), 4))
    if b"avif" not in brands:
        raise RuntimeError("missing-moov source must have the pinned AVIF compatible brand")
    rewritten_brands = b"".join(b"avis" if brand == b"avif" else brand for brand in brands)
    file_type = source_ftyp[:8] + rewritten_brands
    candidate = pack_box(b"ftyp", file_type) + pack_box(b"mdat", media[0])
    retained = read_boxes(candidate, 0, len(candidate))
    if tuple(kind for kind, _, _ in retained) != (b"ftyp", b"mdat"):
        raise RuntimeError("missing-moov file must retain only complete ftyp and mdat boxes")
    if retained[0][2] != file_type or retained[1][2] != media[0]:
        raise RuntimeError("missing-moov mutation changed file type or encoded media bytes")
    if any(
        rewritten_brands[index : index + 4] == b"avif"
        for index in range(0, len(rewritten_brands), 4)
    ):
        raise RuntimeError("missing-moov ftyp must not retain an AVIF compatible brand")
    return candidate


def remove_avis_movie_track(source: bytes) -> bytes:
    """Keep the AVIS movie header while removing its sole picture track."""
    boxes = read_boxes(source, 0, len(source))
    if tuple(kind for kind, _, _ in boxes) != (b"ftyp", b"meta", b"moov", b"mdat"):
        raise RuntimeError("trackless-moov source boxes differ from the pinned sequence layout")

    rebuilt = []
    removed_tracks = 0
    for kind, _, payload in boxes:
        if kind == b"moov":
            children = read_boxes(payload, 0, len(payload))
            if sum(child_kind == b"mvhd" for child_kind, _, _ in children) != 1:
                raise RuntimeError("trackless-moov source must contain one movie header")
            kept_children = []
            for child_kind, _, child_payload in children:
                if child_kind == b"trak":
                    removed_tracks += 1
                    continue
                kept_children.append(pack_box(child_kind, child_payload))
            payload = b"".join(kept_children)
        rebuilt.append(pack_box(kind, payload))

    candidate = b"".join(rebuilt)
    if removed_tracks != 1:
        raise RuntimeError("trackless-moov mutation must remove exactly one picture track")
    retained = read_boxes(candidate, 0, len(candidate))
    movie_payloads = [payload for kind, _, payload in retained if kind == b"moov"]
    if len(movie_payloads) != 1:
        raise RuntimeError("trackless AVIS file must retain one moov box")
    movie_children = read_boxes(movie_payloads[0], 0, len(movie_payloads[0]))
    if not any(kind == b"mvhd" for kind, _, _ in movie_children) or any(
        kind == b"trak" for kind, _, _ in movie_children
    ):
        raise RuntimeError("trackless moov must retain mvhd and contain no trak")

    source_mdat = [payload for kind, _, payload in boxes if kind == b"mdat"]
    candidate_mdat = [payload for kind, _, payload in retained if kind == b"mdat"]
    if candidate_mdat != source_mdat:
        raise RuntimeError("trackless-moov mutation changed encoded AV1 media bytes")
    return candidate


def generate(
    output_dir: Path = DEFAULT_OUTPUT_DIR,
    file_source: Path = FILE_SOURCE,
    idat_source: Path = IDAT_SOURCE,
    sequence_source: Path = SEQUENCE_SOURCE,
    alpha_source: Path = ALPHA_SOURCE,
    check: bool = False,
) -> tuple[Path, ...]:
    """Generate or verify inputs against live pinned Pillow observations."""
    manifest = yaml.safe_load((ROOT / "manifest.yaml").read_bytes())
    verify_primary_oracle(manifest)
    sources = {
        "file": file_source.read_bytes(),
        "idat": idat_source.read_bytes(),
        "sequence": sequence_source.read_bytes(),
        "alpha": alpha_source.read_bytes(),
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
            "trailing_truncated_top_level_header.avif",
            "file",
            append_truncated_top_level_header(sources["file"]),
            None,
        ),
        (
            "trailing_truncated_top_level_payload.avif",
            "file",
            append_truncated_top_level_payload(sources["file"]),
            None,
        ),
        (
            "error_incomplete_still_truncated_top_level_header.avif",
            "file",
            truncate_before_meta_with_partial_header(sources["file"]),
            "open_error",
        ),
        (
            "error_avif_missing_meta.avif",
            "file",
            omit_required_meta(sources["file"]),
            "open_error",
        ),
        (
            "error_primary_item_missing_ispe.avif",
            "file",
            remove_primary_ispe_association(sources["file"]),
            "open_error",
        ),
        (
            "error_avis_missing_moov.avif",
            "sequence",
            omit_required_movie(sources["sequence"]),
            "open_error",
        ),
        (
            "error_avis_moov_without_trak.avif",
            "sequence",
            remove_avis_movie_track(sources["sequence"]),
            "open_error",
        ),
        (
            "error_pitm_zero_item_id.avif",
            "file",
            zero_primary_item_id(sources["file"]),
            "open_runtime_error",
        ),
        (
            "error_iinf_duplicate_item_id.avif",
            "file",
            duplicate_iinf_first_item(sources["file"]),
            "duplicate_iinf_decode_error",
        ),
        (
            "error_ftyp_misaligned_compatible_brand_payload.avif",
            "idat",
            append_ftyp_misaligned_compatible_brand_byte(sources["idat"]),
            "open_error",
        ),
        (
            "ftyp_1025_compatible_brands.avif",
            "idat",
            extend_ftyp_compatible_brands(sources["idat"], 1_025),
            None,
        ),
        (
            "error_iref_records_over_budget_trailing_byte.avif",
            "file",
            append_reference_records_over_budget(sources["file"]),
            "open_error",
        ),
        (
            "error_iref_zero_from_id.avif",
            "alpha",
            zero_iref_item_id(sources["alpha"], source_id=True),
            "open_error",
        ),
        (
            "error_iref_zero_to_id.avif",
            "alpha",
            zero_iref_item_id(sources["alpha"], source_id=False),
            "open_error",
        ),
        (
            "error_iref_trailing_payload.avif",
            "alpha",
            append_iref_trailing_payload(sources["alpha"]),
            "open_error",
        ),
        (
            "duplicate_alpha_auxl_reference_first_wins.avif",
            "alpha",
            duplicate_alpha_auxl_reference_first_wins(sources["alpha"]),
            None,
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
            "error_infe_missing_item_name_terminator.avif",
            "file",
            unterminate_infe_first_item_name(sources["file"]),
            "open_error",
        ),
        (
            "error_meta_duplicate_idat.avif",
            "idat",
            append_empty_duplicate_idat(sources["idat"]),
            "open_error",
        ),
        (
            "error_meta_hdlr_nonzero_version.avif",
            "file",
            set_handler_nonzero_version(sources["file"]),
            "open_error",
        ),
        (
            "error_meta_hdlr_nonzero_predefined.avif",
            "file",
            set_handler_nonzero_predefined(sources["file"]),
            "open_error",
        ),
        (
            "error_meta_reordered_handler.avif",
            "file",
            move_meta_handler_after_primary_item(sources["file"]),
            "open_error",
        ),
        (
            "error_auxc_nonzero_version.avif",
            "alpha",
            set_auxc_full_box_version(sources["alpha"], 1),
            "open_error",
        ),
        (
            "error_pixi_zero_depth.avif",
            "file",
            zero_pixi_first_channel_depth(sources["file"]),
            "open_error",
        ),
        (
            "error_pixi_nonzero_version.avif",
            "file",
            set_pixi_full_box_version(sources["file"], 1),
            "open_error",
        ),
        (
            "error_pixi_zero_planes.avif",
            "file",
            set_pixi_plane_count(sources["file"], 0),
            "pixi_zero_planes",
        ),
        (
            "error_pixi_depth_above_16.avif",
            "file",
            set_pixi_first_channel_depth(sources["file"], 17),
            "pixi_depth_above_16",
        ),
        (
            "error_pixi_channel_depth_mismatch.avif",
            "file",
            mismatch_pixi_second_channel_depth(sources["file"]),
            "pixi_channel_depth_mismatch",
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
            "error_grid_version_1.avif",
            "grid",
            mutate_grid_descriptor(sources["grid"], "version"),
            "grid_version_not_implemented",
        ),
        (
            "error_grid_zero_width.avif",
            "grid",
            mutate_grid_descriptor(sources["grid"], "zero_width"),
            "grid_dimensions_invalid",
        ),
        (
            "error_grid_zero_height.avif",
            "grid",
            mutate_grid_descriptor(sources["grid"], "zero_height"),
            "grid_dimensions_invalid",
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
            "ipma_trailing_payload.avif",
            "idat",
            append_ipma_trailing_payload(sources["idat"]),
            None,
        ),
        (
            "error_ipma_zero_item_id.avif",
            "idat",
            set_ipma_item_id_zero(sources["idat"]),
            "open_error",
        ),
        (
            "error_ipma_non_increasing_item_id.avif",
            "idat",
            append_duplicate_ipma_item_id(sources["idat"]),
            "open_error",
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
        if filename == "error_incomplete_still_truncated_top_level_header.avif":
            expected_media = ()
        elif filename in {
            "error_avif_missing_meta.avif",
            "error_avis_missing_moov.avif",
        }:
            expected_media = tuple(
                (kind, payload)
                for kind, _, payload in read_boxes(sources[source_name], 0, len(sources[source_name]))
                if kind == b"mdat"
            )
        else:
            expected_media = encoded_media_payloads(sources[source_name])
        if filename == "error_meta_duplicate_idat.avif":
            expected_media += ((b"idat", b""),)
        if filename == "trailing_truncated_top_level_payload.avif":
            complete_box_data = data[:-8]
        elif filename in {
            "trailing_truncated_top_level_header.avif",
            "error_incomplete_still_truncated_top_level_header.avif",
        }:
            complete_box_data = data[:-1]
        else:
            complete_box_data = data
        if filename in {
            "error_grid_version_1.avif",
            "error_grid_zero_width.avif",
            "error_grid_zero_height.avif",
        }:
            source = sources["grid"]
            descriptor_offset = grid_descriptor_offset(source)
            descriptor_end = descriptor_offset + 8
            if (
                len(data) != len(source)
                or data[:descriptor_offset] != source[:descriptor_offset]
                or data[descriptor_end:] != source[descriptor_end:]
            ):
                raise RuntimeError(f"{filename}: mutation changed bytes outside the grid descriptor")
            normalized = bytearray(data)
            normalized[descriptor_offset:descriptor_end] = source[descriptor_offset:descriptor_end]
            complete_box_data = bytes(normalized)
        if filename == "error_incomplete_still_truncated_top_level_header.avif":
            actual_media = ()
        elif filename in {
            "error_avif_missing_meta.avif",
            "error_avis_missing_moov.avif",
        }:
            actual_media = tuple(
                (kind, payload)
                for kind, _, payload in read_boxes(complete_box_data, 0, len(complete_box_data))
                if kind == b"mdat"
            )
        else:
            actual_media = encoded_media_payloads(complete_box_data)
        if actual_media != expected_media:
            raise RuntimeError(f"{filename}: mutation changed encoded AV1 media bytes")
        if oracle_error == "open_error":
            verify_open_failure(data, filename)
        elif oracle_error == "open_runtime_error":
            verify_open_runtime_failure(data, filename)
        elif oracle_error == "duplicate_iinf_decode_error":
            verify_decode_runtime_failure(
                data,
                filename,
                "Failed to decode frame 0: Decoding of color planes failed",
            )
        elif oracle_error == "decode_error":
            verify_invalid_grid_rejection(data, filename)
        elif oracle_error == "grid_version_not_implemented":
            verify_grid_descriptor_rejection(
                data, filename, "Failed to decode image: Not implemented"
            )
        elif oracle_error == "grid_dimensions_invalid":
            verify_grid_descriptor_rejection(
                data, filename, "Failed to decode image: Invalid image grid"
            )
        elif oracle_error == "pixi_depth_above_16":
            verify_pixi_rejection(data, filename, "a pixi channel depth above 16")
        elif oracle_error == "pixi_channel_depth_mismatch":
            verify_pixi_rejection(data, filename, "unequal pixi channel depths")
        elif oracle_error == "pixi_zero_planes":
            verify_pixi_rejection(data, filename, "a pixi property with zero planes")
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
    parser.add_argument("--alpha-source", type=Path, default=ALPHA_SOURCE)
    parser.add_argument("--check", action="store_true", help="verify fixtures without writing them")
    args = parser.parse_args()
    generate(
        output_dir=args.output_dir,
        file_source=args.file_source,
        idat_source=args.idat_source,
        sequence_source=args.sequence_source,
        alpha_source=args.alpha_source,
        check=args.check,
    )


if __name__ == "__main__":
    main()
