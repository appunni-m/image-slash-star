#!/usr/bin/env python3
"""Search a bounded oracle corpus for a full-resolution AV1 chroma witness.

This campaign is intentionally input-only.  It creates one hundred
deterministic 32x32 RGB candidates (ten families with ten cases each),
encodes every candidate twice through the pinned Pillow/libavif/libaom oracle,
and decodes each encoded item twice with the independently built scalar dav1d
debug oracle.  No repository Rust code is invoked while searching.

The target is the following 8x8 leaf at pixel ``(8, 8)`` in an 8-bit 4:4:4
frame.  Its top, left, and upper-left 8x8 neighbors are all distinct leaves,
and the target must use coded AV1 chroma Paeth (mode 12).  The target is
admitted only when the public encoded bytes, scalar traces, YUV output, and
Pillow RGB output are deterministic.  The report records every rejection
reason; a caller can promote the lowest-cost qualifying candidate without
changing a bitstream or widening a production hook.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import re
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from explore_avif_chroma_diagonal113 import parse_group, parse_trace
from generate_av1_reconstruction_refs import (
    DAV1D_COMMIT,
    build_dav1d,
    extract_color_item,
    portable_color_reference,
    resolve_tool,
    run,
    verify_source,
)


SIZE = (32, 32)
SUBSAMPLING = "4:4:4"
QUALITY = 76
SPEED = 0
MAX_THREADS = 1

# Keep the encoder controls fixed for the complete corpus.  In particular,
# these controls make the witness about neighbor selection rather than
# filtering, CDEF, restoration, delta-Q, palette, or another optional mode.
ADVANCED = {
    "min-partition-size": "8",
    "max-partition-size": "16",
    "use-intra-dct-only": "0",
    "enable-filter-intra": "0",
    "enable-intra-edge-filter": "0",
    "enable-smooth-intra": "0",
    "enable-paeth-intra": "1",
    "enable-directional-intra": "0",
    "enable-cfl-intra": "0",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "loopfilter-control": "0",
    "aq-mode": "0",
    "deltaq-mode": "0",
}

FAMILY_NAMES = (
    "u_only_quadrant_edges",
    "v_only_quadrant_edges",
    "opposed_uv_edges",
    "lower_left_corner_ramp",
    "diagonal_corner_edges",
    "checker_corner_edges",
    "nested_square8_edges",
    "crossing_edge_stripes",
    "corner_noise_edges",
    "four_quadrant_mosaic_noise",
)

BLOCK_PATTERN = re.compile(
    r"^poc=(?P<poc>-?\d+),y=(?P<y>-?\d+),x=(?P<x>-?\d+),"
    r"bl=(?P<level>\d+),ctx=(?P<context>\d+),bp=(?P<partition>\d+): "
    r"r=(?P<range>\d+)$"
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of ``data``."""

    return hashlib.sha256(data).hexdigest()


def clamp(value: int) -> int:
    """Clamp one synthetic RGB component to the 8-bit range."""

    return max(0, min(255, value))


def yuv_to_rgb(y: int, u: int, v: int) -> tuple[int, int, int]:
    """Convert one bounded synthetic full-range YUV sample to RGB."""

    du = u - 128
    dv = v - 128
    return (
        clamp(y + (358 * dv + 128) // 256),
        clamp(y - (88 * du + 183 * dv + 128) // 256),
        clamp(y + (453 * du + 128) // 256),
    )


def paeth(left: int, top: int, top_left: int) -> int:
    """Return the AV1 Paeth choice for one sample."""

    prediction = left + top - top_left
    left_distance = abs(prediction - left)
    top_distance = abs(prediction - top)
    top_left_distance = abs(prediction - top_left)
    if left_distance <= top_distance and left_distance <= top_left_distance:
        return left
    if top_distance <= top_left_distance:
        return top
    return top_left


def edge_fields(family: int, index: int) -> tuple[list[int], list[int], list[int], list[int]]:
    """Return deterministic top/left U/V edges for one family.

    The first family is the stable control that found the original witness.
    Other families apply small, named edge perturbations so the campaign
    exercises different corner relationships while retaining the same
    public topology constraints.
    """

    mode = family
    top_u = [125 + mode * 4 + ((i * 3 + mode) % 5) * 7 for i in range(8)]
    left_u = [95 + mode * 3 + ((i * 5 + mode) % 5) * 6 for i in range(8)]
    top_v = [185 - mode * 3 - ((i * 2 + mode) % 5) * 7 for i in range(8)]
    left_v = [155 - mode * 2 - ((i * 3 + mode) % 5) * 5 for i in range(8)]
    if family == 1:
        top_u = [value + (i % 3) * 3 for i, value in enumerate(top_u)]
    elif family == 2:
        left_u = [value + (8 - i) * 2 for i, value in enumerate(left_u)]
        top_v = [value - (8 - i) * 2 for i, value in enumerate(top_v)]
    elif family == 3:
        left_u = [value + i * (2 + index % 2) for i, value in enumerate(left_u)]
        left_v = [value - i * (1 + index % 3) for i, value in enumerate(left_v)]
    elif family == 4:
        top_u = [value + ((i * i + index) % 9) - 4 for i, value in enumerate(top_u)]
        top_v = [value - ((i * i + 2 * index) % 9) + 4 for i, value in enumerate(top_v)]
    elif family == 5:
        top_u = [value + (18 if i % 2 else -18) for i, value in enumerate(top_u)]
        left_v = [value + (14 if i % 2 else -14) for i, value in enumerate(left_v)]
    elif family == 6:
        top_u = [value + (i // 2) * 4 for i, value in enumerate(top_u)]
        left_u = [value - (i // 2) * 3 for i, value in enumerate(left_u)]
    elif family == 7:
        top_v = [value + (i % 4) * 5 for i, value in enumerate(top_v)]
        left_v = [value - (i % 4) * 4 for i, value in enumerate(left_v)]
    elif family == 8:
        top_u = [value + ((17 * i + index) % 11) - 5 for i, value in enumerate(top_u)]
        left_u = [value + ((13 * i + 2 * index) % 11) - 5 for i, value in enumerate(left_u)]
    elif family == 9:
        top_u = list(reversed(top_u))
        left_u = list(reversed(left_u))
        top_v = list(reversed(top_v))
        left_v = list(reversed(left_v))
    return top_u, left_u, top_v, left_v


def candidate_pixels(family: int, index: int) -> bytes:
    """Create one deterministic 32x32 RGB candidate."""

    seed = 7000 + 10 * family + index
    random_state = random.Random(seed)
    top_u, left_u, top_v, left_v = edge_fields(family, index)
    # These values deliberately cover the range in which the reference
    # encoder selects top-left-sensitive Paeth instead of DC for some cases.
    top_left_u = (70, 85, 100, 115, 130, 145, 160, 175, 100, 70)[index]
    top_left_v = 255 - top_left_u
    amplitude = index % 4
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if x < 8 and y < 8:
                u = 100 + 2 * x + 3 * y
                v = 150 - 2 * x + y
            elif y < 8 and 8 <= x < 16:
                u = top_u[x - 8]
                v = top_v[x - 8]
            elif x < 8 and 8 <= y < 16:
                u = left_u[y - 8]
                v = left_v[y - 8]
            elif 8 <= x < 16 and 8 <= y < 16:
                u = paeth(left_u[y - 8], top_u[x - 8], top_left_u)
                v = paeth(left_v[y - 8], top_v[x - 8], top_left_v)
                u += amplitude * (((13 * x + 7 * y + seed) % 7) - 3)
                v -= amplitude * (((11 * x + 5 * y + seed) % 7) - 3)
            else:
                u = 128 + random_state.randrange(-45, 46)
                v = 128 + random_state.randrange(-45, 46)
            # High-frequency luma makes the top-left 16x16 quadrant split
            # into four terminal Square8 leaves without mutating the encoded
            # AV1 item after Pillow has produced it.
            luma = 128 + random_state.randrange(-20, 21)
            if family in (6, 7):
                luma += ((17 * x + 13 * y + seed) % 9) - 4
            if x == 7 and y == 7:
                u = top_left_u
                v = top_left_v
            pixels.extend(yuv_to_rgb(luma, u, v))
    return bytes(pixels)


def candidates() -> list[dict[str, object]]:
    """Return exactly ten deterministic families with ten cases each."""

    result = []
    for family, family_name in enumerate(FAMILY_NAMES):
        for index in range(10):
            result.append(
                {
                    "id": f"i444-tl-f{family + 1:02d}-n{index:02d}",
                    "family": family_name,
                    "family_index": family,
                    "candidate_index": index,
                    "seed": 7000 + 10 * family + index,
                    "pixels": candidate_pixels(family, index),
                    "quality": QUALITY,
                    "speed": SPEED,
                }
            )
    if len(result) != 100:
        raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
    return result


def encode(pixels: bytes, quality: int, speed: int) -> bytes:
    """Encode one candidate through the pinned Pillow AVIF oracle."""

    output = BytesIO()
    Image.frombytes("RGB", SIZE, pixels).save(
        output,
        format="AVIF",
        quality=quality,
        speed=speed,
        max_threads=MAX_THREADS,
        subsampling=SUBSAMPLING,
        autotiling=False,
        advanced=ADVANCED,
    )
    return output.getvalue()


def trace(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    item: bytes,
    stem: str,
    ordinal: int,
) -> tuple[str, list[dict[str, int]], list[list[str]], int, bytes]:
    """Trace one AV1 color item with the independent scalar dav1d oracle."""

    item_path = work / f"{stem}-{ordinal}.obu"
    yuv_path = work / f"{stem}-{ordinal}.yuv"
    item_path.write_bytes(item)
    result = run(
        [
            str(executable),
            "--input",
            str(item_path),
            "--demuxer",
            "section5",
            "--output",
            str(yuv_path),
            "--muxer",
            "yuv",
            "--threads",
            "1",
            "--framedelay",
            "1",
            "--cpumask",
            "0",
            "--quiet",
        ],
        env=environment,
    )
    blocks, groups, entropy_count = parse_trace(result.stdout)
    return result.stdout, blocks, groups, entropy_count, yuv_path.read_bytes()


def leaf_records(output: str) -> list[dict[str, int] | None]:
    """Associate each traced ``Post-skip`` group with its latest block header."""

    records: list[dict[str, int] | None] = []
    current: dict[str, int] | None = None
    for line in output.splitlines():
        line = line.rstrip()
        if match := BLOCK_PATTERN.fullmatch(line):
            current = {name: int(value) for name, value in match.groupdict().items()}
        elif line.startswith("Post-skip["):
            records.append(current.copy() if current is not None else None)
    return records


def block_key(block: dict[str, int] | None) -> tuple[int, int, int, int] | None:
    """Return the geometry key used by the target topology predicates."""

    if block is None:
        return None
    return block["x"], block["y"], block["level"], block["partition"]


def no_optional_predictors(groups: list[list[str]]) -> bool:
    """Reject palette, filter-intra, and CFL syntax in the oracle trace."""

    prefixes = (
        "Post-filterintramode[",
        "Post-y_pal[",
        "Post-pal[",
        "Post-y-pal-indices",
        "y-pal-pred",
        "Post-uv_pal[",
        "Post-uv-pal-indices",
        "uv-pal-pred",
        "Post-cfl-alpha",
    )
    return not any(line.startswith(prefixes) for group in groups for line in group)


def planes(yuv: bytes) -> tuple[bytes, bytes, bytes] | None:
    """Split an 8-bit 4:4:4 32x32 YUV frame into three planes."""

    plane_length = SIZE[0] * SIZE[1]
    if len(yuv) != 3 * plane_length:
        return None
    return (
        yuv[:plane_length],
        yuv[plane_length : 2 * plane_length],
        yuv[2 * plane_length :],
    )


def predictor_evidence(plane: bytes) -> dict[str, object]:
    """Record target D's true and fallback Paeth predictions for one plane."""

    top = [plane[7 * SIZE[0] + x] for x in range(8, 16)]
    left = [plane[y * SIZE[0] + 7] for y in range(8, 16)]
    top_left = plane[7 * SIZE[0] + 7]
    true_prediction = [
        paeth(left[y], top[x], top_left) for y in range(8) for x in range(8)
    ]
    top_fallback = [
        paeth(left[y], top[x], top[0]) for y in range(8) for x in range(8)
    ]
    left_fallback = [
        paeth(left[y], top[x], left[0]) for y in range(8) for x in range(8)
    ]
    actual = [plane[y * SIZE[0] + x] for y in range(8, 16) for x in range(8, 16)]
    return {
        "top": top,
        "left": left,
        "top_left": top_left,
        "actual_target": actual,
        "true_prediction": true_prediction,
        "top_fallback_prediction": top_fallback,
        "left_fallback_prediction": left_fallback,
        "neighbors_pairwise_distinct": len({top_left, top[0], left[0]}) == 3,
        "differs_from_top_fallback": true_prediction != top_fallback,
        "differs_from_left_fallback": true_prediction != left_fallback,
        "actual_matches_true_prediction": actual == true_prediction,
    }


def classify(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
    output: str,
) -> dict[str, object]:
    """Apply exact admission predicates for the full-resolution target."""

    records = leaf_records(output)
    target_block = {
        "x": 2,
        "y": 2,
        "level": 4,
        "partition": 0,
    }
    target_index = next(
        (
            index
            for index, block in enumerate(records)
            if block is not None
            and all(block[key] == value for key, value in target_block.items())
        ),
        None,
    )
    target_group = parse_group(groups[target_index]) if target_index is not None else {}
    chroma_payloads = target_group.get("chroma_payloads", [])
    uv_modes = target_group.get("uv_modes", [])
    target_planes = planes(yuv)
    evidence = (
        {
            "u": predictor_evidence(target_planes[1]),
            "v": predictor_evidence(target_planes[2]),
        }
        if target_planes is not None
        else {}
    )
    topology = {block_key(block) for block in blocks}
    required_topology = {
        (0, 0, 2, 3),
        (0, 0, 3, 3),
        (0, 0, 4, 0),
        (2, 0, 4, 0),
        (0, 2, 4, 0),
        (2, 2, 4, 0),
    }
    parsed_chroma = [payload for payload in chroma_payloads if isinstance(payload, dict)]
    predicates = {
        "eight_bit_i444_32x32": (
            portable_color.get("width") == 32
            and portable_color.get("height") == 32
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is False
            and portable_color.get("subsampling_y") is False
        ),
        "required_top_left_square8_topology": required_topology.issubset(topology),
        "target_group_is_fourth_top_left_leaf": target_index == 3,
        "target_group_is_square8": target_group != {},
        "target_uv_mode_is_paeth": uv_modes == [12],
        "target_has_two_chroma_planes": (
            len(parsed_chroma) == 2
            and {payload.get("plane") for payload in parsed_chroma} == {0, 1}
        ),
        "target_chroma_is_square8": (
            len(parsed_chroma) == 2
            and all(payload.get("tx") == 1 for payload in parsed_chroma)
        ),
        "target_chroma_payloads_are_valid": (
            len(parsed_chroma) == 2
            and all(
                int(payload.get("eob", -2)) >= -1
                and 0 <= int(payload.get("txtp", -1)) <= 3
                for payload in parsed_chroma
            )
        ),
        "no_optional_predictors": no_optional_predictors(groups),
        "full_yuv_planes_are_available": target_planes is not None,
        "u_neighbors_pairwise_distinct": bool(evidence)
        and bool(evidence.get("u", {}).get("neighbors_pairwise_distinct")),
        "v_neighbors_pairwise_distinct": bool(evidence)
        and bool(evidence.get("v", {}).get("neighbors_pairwise_distinct")),
        "u_prediction_changes_with_wrong_fallback": bool(evidence)
        and bool(evidence.get("u", {}).get("differs_from_top_fallback"))
        and bool(evidence.get("u", {}).get("differs_from_left_fallback")),
        "u_decoded_target_matches_true_paeth": bool(evidence)
        and bool(evidence.get("u", {}).get("actual_matches_true_prediction")),
        "v_prediction_changes_with_wrong_fallback": bool(evidence)
        and bool(evidence.get("v", {}).get("differs_from_top_fallback"))
        and bool(evidence.get("v", {}).get("differs_from_left_fallback")),
        "v_decoded_target_matches_true_paeth": bool(evidence)
        and bool(evidence.get("v", {}).get("actual_matches_true_prediction")),
    }
    return {
        "partition_blocks": blocks,
        "leaf_records": records,
        "target_block": target_block,
        "target_group_index": target_index,
        "target_group": target_group,
        "target_chroma_payloads": parsed_chroma,
        "target_predictor_evidence": evidence,
        "predicates": predicates,
        "rejection_reasons": [name for name, passed in predicates.items() if not passed],
        "qualifies": all(predicates.values()),
    }


def pillow_rgb(encoded: bytes) -> bytes:
    """Decode one encoded AVIF through Pillow's public RGB path."""

    with Image.open(BytesIO(encoded)) as image:
        image.load()
        return image.convert("RGB").tobytes()


def decode_candidate(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    candidate: dict[str, object],
    retain_dir: Path | None,
) -> dict[str, object]:
    """Double-encode, double-trace, and classify one candidate."""

    pixels = candidate["pixels"]
    if not isinstance(pixels, bytes):
        raise TypeError("candidate pixels must be bytes")
    encoded_a = encode(pixels, int(candidate["quality"]), int(candidate["speed"]))
    encoded_b = encode(pixels, int(candidate["quality"]), int(candidate["speed"]))
    path_a = work / f"{candidate['id']}.avif"
    path_b = work / f"{candidate['id']}-second.avif"
    path_a.write_bytes(encoded_a)
    path_b.write_bytes(encoded_b)
    item_a, _ = extract_color_item(path_a)
    item_b, _ = extract_color_item(path_b)
    trace_a, blocks_a, groups_a, entropy_a, yuv_a = trace(
        executable, environment, work, item_a, str(candidate["id"]), 1
    )
    trace_b, blocks_b, groups_b, entropy_b, yuv_b = trace(
        executable, environment, work, item_b, str(candidate["id"]), 2
    )
    portable_color = portable_color_reference(path_a)
    pillow_rgb_a = pillow_rgb(encoded_a)
    pillow_rgb_b = pillow_rgb(encoded_b)
    classification = classify(blocks_a, groups_a, yuv_a, portable_color, trace_a)
    predicates = classification["predicates"]
    if not isinstance(predicates, dict):
        raise TypeError("classification predicates must be a dictionary")
    predicates.update(
        {
            "double_encode_equal": encoded_a == encoded_b,
            "double_item_equal": item_a == item_b,
            "double_trace_equal": (
                trace_a == trace_b
                and blocks_a == blocks_b
                and groups_a == groups_b
                and yuv_a == yuv_b
            ),
            "double_pillow_rgb_equal": pillow_rgb_a == pillow_rgb_b,
        }
    )
    classification["rejection_reasons"] = [
        name for name, passed in predicates.items() if not passed
    ]
    classification["qualifies"] = all(predicates.values())
    report = {
        "id": candidate["id"],
        "family": candidate["family"],
        "family_index": candidate["family_index"],
        "candidate_index": candidate["candidate_index"],
        "seed": candidate["seed"],
        "quality": candidate["quality"],
        "speed": candidate["speed"],
        "input_rgb_sha256": sha256(pixels),
        "encoded_file_sha256": sha256(encoded_a),
        "encoded_file_second_sha256": sha256(encoded_b),
        "encoded_item_sha256": sha256(item_a),
        "encoded_item_second_sha256": sha256(item_b),
        "encoded_item_length": len(item_a),
        "entropy_operation_count": entropy_a,
        "entropy_operation_count_second": entropy_b,
        "decoded_yuv_sha256": sha256(yuv_a),
        "decoded_yuv_second_sha256": sha256(yuv_b),
        "decoded_yuv_length": len(yuv_a),
        "decoded_plane_sha256": (
            [sha256(plane) for plane in planes(yuv_a)]
            if planes(yuv_a) is not None
            else None
        ),
        "pillow_rgb_sha256": sha256(pillow_rgb_a),
        "pillow_rgb_second_sha256": sha256(pillow_rgb_b),
        "partition_blocks": blocks_a,
        "partition_blocks_second": blocks_b,
        "trace_sha256": sha256(trace_a.encode()),
        "trace_second_sha256": sha256(trace_b.encode()),
        "portable_color": portable_color,
        "repository_rust_invoked": False,
        **classification,
    }
    if report["qualifies"] and retain_dir is not None:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path_a.name).write_bytes(encoded_a)
        (retain_dir / f"{candidate['id']}.trace.txt").write_text(trace_a)
        (retain_dir / f"{candidate['id']}.yuv").write_bytes(yuv_a)
    return report


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument("--dav1d-source", type=Path)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--python-path", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--retain-dir", type=Path)
    args = parser.parse_args()

    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")

    with tempfile.TemporaryDirectory(prefix="image-star-avif-i444-top-left-") as name:
        work = Path(name)
        if args.dav1d_source is not None:
            source = args.dav1d_source.resolve()
            verify_source(source)
            executable, environment = build_dav1d(
                source,
                work,
                resolve_tool(args.meson, "Meson"),
                resolve_tool(args.ninja, "Ninja"),
                args.python_path.resolve() if args.python_path else None,
                include_block_angles=False,
            )
        else:
            executable = args.dav1d.resolve()
            environment = {}
        version_result = run([str(executable), "--version"], env=environment)
        version = (version_result.stdout + version_result.stderr).strip()
        if not version.startswith("1.5.3-0-gb546257"):
            raise RuntimeError(f"unexpected dav1d executable version: {version}")
        reports = [
            decode_candidate(executable, environment, work, candidate, args.retain_dir)
            for candidate in candidates()
        ]

    qualified = [report for report in reports if report["qualifies"]]
    promoted = (
        min(
            qualified,
            key=lambda report: (
                int(report["entropy_operation_count"]),
                int(report["encoded_item_length"]),
                str(report["id"]),
            ),
        )["id"]
        if qualified
        else None
    )
    predicate_names = sorted(
        {
            name
            for report in reports
            for name in report["predicates"]
        }
    )
    report = {
        "format_version": 1,
        "oracle": {
            "pillow": Image.__version__,
            "libavif": features.version("avif"),
            "codecs": codecs,
            "dav1d": version,
            "dav1d_commit": DAV1D_COMMIT,
        },
        "encoding": {
            "size": list(SIZE),
            "subsampling": SUBSAMPLING,
            "quality": QUALITY,
            "speed": SPEED,
            "max_threads": MAX_THREADS,
            "autotiling": False,
            "advanced": ADVANCED,
        },
        "search": {
            "candidate_count": len(reports),
            "seed_formula": "7000 + 10*family_index + candidate_index",
            "target_id": "i444_full_chroma_top_left_paeth",
            "target": (
                "32x32 8-bit 4:4:4 following Square8 at pixel (8,8), with "
                "top/left/upper-left Square8 neighbors and coded chroma Paeth"
            ),
            "families": list(FAMILY_NAMES),
            "repository_rust_invoked": False,
            "admission_sort": [
                "entropy_operation_count",
                "encoded_item_length",
                "id",
            ],
        },
        "counts": {
            "qualified": len(qualified),
            "promoted": promoted,
            "by_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in predicate_names
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(
        f"Written {len(reports)} deterministic I444 top-left traces: {args.output}; "
        f"qualified={len(qualified)} promoted={promoted}"
    )


if __name__ == "__main__":
    main()
