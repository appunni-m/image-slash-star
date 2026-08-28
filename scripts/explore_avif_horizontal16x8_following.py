#!/usr/bin/env python3
"""Search a bounded corpus for a following AV1 Horizontal16x8 leaf.

The campaign creates one hundred deterministic 32x8 RGB candidates in ten
families.  Each candidate is encoded twice through the pinned Pillow AVIF
oracle and the extracted color item is decoded twice through the independent
scalar dav1d diagnostic build.  A candidate qualifies only when the trace
contains a root PARTITION_SPLIT with two level-3 top-row PARTITION_HORZ
children (the two 16x8 leaves), both leaves use the bounded unsplit
TX16x8/DCT_DCT plus skipped TX8x4 4:2:0 sentence, and the right-hand leaf has
a non-empty luma residual.  The report records every candidate and rejection
reason.  It never invokes repository Rust code.
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

from generate_av1_reconstruction_refs import (
    DAV1D_COMMIT,
    build_dav1d,
    extract_color_item,
    portable_color_reference,
    resolve_tool,
    run,
    verify_source,
)


SIZE = (32, 8)
SUBSAMPLING = "4:2:0"
EXPECTED_YUV_BYTES = SIZE[0] * SIZE[1] + 2 * (SIZE[0] // 2) * (SIZE[1] // 2)
QUALITIES = (36, 44, 52, 60, 68, 76, 84, 90, 94, 98)
AMPLITUDES = (24, 32, 40, 48, 56, 64, 72, 80, 88, 96)
FAMILY_NAMES = (
    "F01_vertical_checker",
    "F02_horizontal_checker",
    "F03_left_right_ramps",
    "F04_column_prbs",
    "F05_row_prbs",
    "F06_sparse_edges",
    "F07_crosshatch",
    "F08_right_impulses",
    "F09_two_region_noise",
    "F10_mixed_bands",
)
ADVANCED = {
    "min-partition-size": "8",
    "max-partition-size": "16",
    "use-intra-dct-only": "0",
    "enable-filter-intra": "0",
    "enable-intra-edge-filter": "0",
    "enable-smooth-intra": "0",
    "enable-paeth-intra": "0",
    "enable-directional-intra": "0",
    "enable-cfl-intra": "0",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "loopfilter-control": "0",
    "aq-mode": "0",
    "deltaq-mode": "0",
}
BLOCK_PATTERN = re.compile(
    r"^poc=(?P<poc>-?\d+),y=(?P<y>-?\d+),x=(?P<x>-?\d+),"
    r"bl=(?P<level>\d+),ctx=(?P<context>\d+),bp=(?P<partition>\d+): "
    r"r=(?P<range>\d+)$"
)
LUMA_PATTERN = re.compile(
    r"^Post-y-cf-blk\[tx=(?P<tx>\d+),txtp=(?P<txtp>-?\d+),"
    r"eob=(?P<eob>-?\d+)\]"
)
CHROMA_PATTERN = re.compile(
    r"^Post-uv-cf-blk\[pl=(?P<plane>\d+),tx=(?P<tx>\d+),"
    r"txtp=(?P<txtp>-?\d+),eob=(?P<eob>-?\d+)\]"
)
YMODE_PATTERN = re.compile(r"^Post-ymode\[(?P<mode>\d+)\]")
UVMODE_PATTERN = re.compile(r"^Post-uvmode\[(?P<mode>\d+)\]")
TX_PATTERN = re.compile(r"^Post-tx\[(?P<tx>\d+)\]")
TXTP_PATTERN = re.compile(
    r"^Post-txtp-intra\[(?P<maximum>\d+)->(?P<minimum>\d+)\]\[0\]"
    r"\[(?P<symbol>\d+)->(?P<txtp>-?\d+)\]"
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of ``data``."""

    return hashlib.sha256(data).hexdigest()


def clamp(value: int) -> int:
    """Clamp one generated sample to the eight-bit range."""

    return max(0, min(255, value))


def candidates() -> list[dict[str, object]]:
    """Return ten deterministic families with ten candidates each."""

    result: list[dict[str, object]] = []
    for family, family_name in enumerate(FAMILY_NAMES):
        for index in range(10):
            seed = 328_000 + 100 * family + index
            state = random.Random(seed)
            amplitude = AMPLITUDES[index]
            column_prbs = [state.choice((-1, 1)) for _ in range(SIZE[0])]
            row_prbs = [state.choice((-1, 1)) for _ in range(SIZE[1])]
            impulses = {
                (state.randrange(SIZE[0]), state.randrange(SIZE[1]))
                for _ in range(4 + family % 4)
            }
            pixels = bytearray()
            for y in range(SIZE[1]):
                for x in range(SIZE[0]):
                    side = -1 if x < SIZE[0] // 2 else 1
                    if family == 0:
                        signal = amplitude * (1 if (x + index) % 2 else -1)
                    elif family == 1:
                        signal = amplitude * (1 if (y + index) % 2 else -1)
                    elif family == 2:
                        signal = side * (amplitude // 2) + (x % 4 - 2) * 3
                    elif family == 3:
                        signal = amplitude * column_prbs[x]
                    elif family == 4:
                        signal = amplitude * row_prbs[y]
                    elif family == 5:
                        signal = amplitude if (x, y) in impulses else side * 8
                    elif family == 6:
                        signal = amplitude // 2 * (
                            (1 if (x + index) % 2 else -1)
                            + (1 if (y + index) % 2 else -1)
                        )
                    elif family == 7:
                        signal = amplitude if (x, y) in impulses else 0
                    elif family == 8:
                        signal = state.randrange(-amplitude, amplitude + 1)
                    else:
                        signal = side * (amplitude // 2) + (y - 3) * 5
                    base = 128 + signal
                    pixels.extend((clamp(base),) * 3)
            result.append(
                {
                    "id": f"h16x8-following-f{family + 1:02d}-n{index:02d}",
                    "family": family_name,
                    "family_index": family,
                    "candidate_index": index,
                    "seed": seed,
                    "quality": QUALITIES[index],
                    "amplitude": amplitude,
                    "speed": 0,
                    "pixels": bytes(pixels),
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
        max_threads=1,
        subsampling=SUBSAMPLING,
        autotiling=False,
        advanced=ADVANCED,
    )
    return output.getvalue()


def parse_trace(output: str) -> tuple[list[dict[str, int]], list[list[str]], int]:
    """Parse partition records, leaf groups, and contiguous scalar entropy."""

    lines = [line.rstrip() for line in output.splitlines() if line.strip()]
    entropy = [
        json.loads(line.removeprefix("@MSAC "))
        for line in lines
        if line.startswith("@MSAC ")
    ]
    if not entropy or entropy[0].get("operation") != "init":
        raise RuntimeError("missing scalar MSAC trace")
    if [operation["step"] for operation in entropy] != list(range(len(entropy))):
        raise RuntimeError("non-contiguous scalar MSAC trace")
    debug = [line for line in lines if not line.startswith("@MSAC ")]
    blocks = []
    for line in debug:
        if match := BLOCK_PATTERN.fullmatch(line):
            blocks.append({name: int(value) for name, value in match.groupdict().items()})
    groups: list[list[str]] = []
    for line in debug:
        if line.startswith("Post-skip["):
            groups.append([])
        if groups:
            groups[-1].append(line)
    if not blocks:
        raise RuntimeError("missing partition trace")
    return blocks, groups, len(entropy)


def parse_group(group: list[str]) -> dict[str, object]:
    """Extract modes, transform syntax, and coefficient payloads."""

    y_modes = [
        int(match["mode"])
        for line in group
        if (match := YMODE_PATTERN.match(line)) is not None
    ]
    uv_modes = [
        int(match["mode"])
        for line in group
        if (match := UVMODE_PATTERN.match(line)) is not None
    ]
    tx_sizes = [
        int(match["tx"])
        for line in group
        if (match := TX_PATTERN.match(line)) is not None
    ]
    transform_records = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in group
        if (match := TXTP_PATTERN.match(line)) is not None
    ]
    luma_payloads = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in group
        if (match := LUMA_PATTERN.match(line)) is not None
    ]
    chroma_payloads = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in group
        if (match := CHROMA_PATTERN.match(line)) is not None
    ]
    forbidden = any(
        line.startswith(
            (
                "Post-filterintramode[",
                "Post-yangle-symbol[",
                "Post-uvangle-symbol[",
                "Post-y_pal[",
                "Post-pal[",
                "Post-y-pal-indices",
                "y-pal-pred",
                "Post-uv_pal[",
                "Post-uv-pal-indices",
                "uv-pal-pred",
            )
        )
        for line in group
    )
    return {
        "y_modes": y_modes,
        "uv_modes": uv_modes,
        "tx_sizes": tx_sizes,
        "transform_records": transform_records,
        "luma_payloads": luma_payloads,
        "chroma_payloads": chroma_payloads,
        "forbidden_syntax": forbidden,
    }


def leaf_predicates(leaf: dict[str, object]) -> dict[str, bool]:
    """Return the exact bounded syntax predicates for one leaf."""

    luma = leaf["luma_payloads"]
    chroma = leaf["chroma_payloads"]
    transforms = leaf["transform_records"]
    return {
        "no_forbidden_syntax": not leaf["forbidden_syntax"],
        "dc_luma_and_chroma": leaf["y_modes"] == [0] and leaf["uv_modes"] == [0],
        "unsplit_tx16x8": leaf["tx_sizes"] == [8],
        "dct_dct_transform": transforms == [{"maximum": 8, "minimum": 1, "symbol": 1, "txtp": 0}],
        "one_luma_tx16x8": (
            len(luma) == 1 and luma[0]["tx"] == 8 and luma[0]["txtp"] == 0
        ),
        "two_skipped_tx8x4_dc_chroma": (
            len(chroma) == 2
            and [item["plane"] for item in chroma] == [0, 1]
            and all(
                item["tx"] == 6 and item["txtp"] == 0 and item["eob"] == -1
                for item in chroma
            )
        ),
    }


def classify(
    blocks: list[dict[str, int]], groups: list[list[str]], yuv: bytes, color: dict[str, object]
) -> dict[str, object]:
    """Apply exact frame, topology, and target-leaf predicates."""

    parsed = [parse_group(group) for group in groups]
    root = next(
        (
            block
            for block in blocks
            if block["level"] == 2
            and block["x"] == 0
            and block["y"] == 0
            and block["partition"] == 3
        ),
        None,
    )
    child_blocks = [
        block
        for block in blocks
        if block["level"] == 3 and block["y"] == 0 and block["partition"] == 1
    ]
    left = parsed[0] if len(parsed) == 2 else {}
    right = parsed[1] if len(parsed) == 2 else {}
    left_predicates = leaf_predicates(left) if left else {}
    right_predicates = leaf_predicates(right) if right else {}
    common = {
        "frame_is_32x8_8bit_420": (
            color.get("width") == SIZE[0]
            and color.get("height") == SIZE[1]
            and color.get("bit_depth") == 8
            and color.get("monochrome") is False
            and color.get("subsampling_x") is True
            and color.get("subsampling_y") is True
        ),
        "root_partition_split": root is not None,
        "root_has_two_horizontal_children": sorted(
            (block["x"], block["context"]) for block in child_blocks
        )
        == [(0, 0), (4, 2)],
        "two_leaf_groups": len(groups) == 2,
        "both_leaves_bounded": bool(left_predicates) and bool(right_predicates)
        and all(left_predicates.values())
        and all(right_predicates.values()),
        "right_luma_nonempty": bool(right.get("luma_payloads"))
        and right["luma_payloads"][0]["eob"] >= 0,
        "complete_yuv_output": len(yuv) == EXPECTED_YUV_BYTES,
    }
    reasons = [name for name, passed in common.items() if not passed]
    reasons.extend(f"left_{name}" for name, passed in left_predicates.items() if not passed)
    reasons.extend(f"right_{name}" for name, passed in right_predicates.items() if not passed)
    return {
        "root_partition": root,
        "group_count": len(groups),
        "left": left,
        "right": right,
        "left_predicates": left_predicates,
        "right_predicates": right_predicates,
        "common_predicates": common,
        "rejection_reasons": reasons,
        "qualifies": not reasons,
    }


def decode_candidate(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    candidate: dict[str, object],
    retain_dir: Path | None,
) -> dict[str, object]:
    """Double-encode and double-decode one candidate."""

    pixels = candidate["pixels"]
    if not isinstance(pixels, bytes):
        raise TypeError("candidate pixels must be bytes")
    quality = int(candidate["quality"])
    speed = int(candidate["speed"])
    encoded = encode(pixels, quality, speed)
    encoded_second = encode(pixels, quality, speed)
    if encoded != encoded_second:
        raise RuntimeError(f"nondeterministic encoding for {candidate['id']}")
    path = work / f"{candidate['id']}.avif"
    second_path = work / f"{candidate['id']}-second.avif"
    path.write_bytes(encoded)
    second_path.write_bytes(encoded_second)
    if retain_dir is not None:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path.name).write_bytes(encoded)
    item, _ = extract_color_item(path)
    second_item, _ = extract_color_item(second_path)
    if item != second_item:
        raise RuntimeError(f"nondeterministic color item for {candidate['id']}")
    color = portable_color_reference(path)
    item_path = work / f"{candidate['id']}.obu"
    item_path.write_bytes(item)

    def trace_once(ordinal: int) -> tuple[str, bytes]:
        yuv_path = work / f"{candidate['id']}-{ordinal}.yuv"
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
        return result.stdout, yuv_path.read_bytes()

    trace_a, yuv_a = trace_once(1)
    trace_b, yuv_b = trace_once(2)
    if trace_a != trace_b or yuv_a != yuv_b:
        raise RuntimeError(f"nondeterministic dav1d trace or YUV for {candidate['id']}")
    blocks, groups, entropy_count = parse_trace(trace_a)
    with Image.open(BytesIO(encoded)) as decoded:
        pillow_rgb = decoded.convert("RGB").tobytes()
    with Image.open(BytesIO(encoded_second)) as decoded:
        pillow_rgb_second = decoded.convert("RGB").tobytes()
    if pillow_rgb != pillow_rgb_second:
        raise RuntimeError(f"nondeterministic Pillow RGB decode for {candidate['id']}")
    luma_bytes = SIZE[0] * SIZE[1]
    chroma_bytes = (SIZE[0] // 2) * (SIZE[1] // 2)
    return {
        "id": candidate["id"],
        "family": candidate["family"],
        "family_index": candidate["family_index"],
        "candidate_index": candidate["candidate_index"],
        "seed": candidate["seed"],
        "quality": quality,
        "amplitude": candidate["amplitude"],
        "input_rgb_sha256": sha256(pixels),
        "encoded_file_sha256": sha256(encoded),
        "encoded_file_second_sha256": sha256(encoded_second),
        "encoded_item_sha256": sha256(item),
        "encoded_item_second_sha256": sha256(second_item),
        "encoded_item_length": len(item),
        "pillow_rgb_sha256": sha256(pillow_rgb),
        "pillow_rgb_second_sha256": sha256(pillow_rgb_second),
        "dav1d_trace_sha256": sha256(trace_a.encode()),
        "dav1d_trace_second_sha256": sha256(trace_b.encode()),
        "decoded_yuv_sha256": sha256(yuv_a),
        "decoded_y_plane_sha256": sha256(yuv_a[:luma_bytes]),
        "decoded_u_plane_sha256": sha256(yuv_a[luma_bytes : luma_bytes + chroma_bytes]),
        "decoded_v_plane_sha256": sha256(yuv_a[luma_bytes + chroma_bytes :]),
        "entropy_operation_count": entropy_count,
        "partition_blocks": blocks,
        "repository_rust_invoked": False,
        **classify(blocks, groups, yuv_a, color),
    }


def main() -> None:
    """Run the pinned one-hundred-candidate input-only campaign."""

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

    with tempfile.TemporaryDirectory(prefix="image-star-avif-h16x8-following-") as name:
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
                broaden_horizontal_rect_following=True,
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

    common_reasons = (
        "frame_is_32x8_8bit_420",
        "root_partition_split",
        "root_has_two_horizontal_children",
        "two_leaf_groups",
        "both_leaves_bounded",
        "right_luma_nonempty",
        "complete_yuv_output",
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
            "quality_by_candidate_index": list(QUALITIES),
            "amplitude_by_candidate_index": list(AMPLITUDES),
            "speed": 0,
            "max_threads": 1,
            "autotiling": False,
            "advanced": ADVANCED,
        },
        "search": {
            "input_only": True,
            "repository_rust_invoked": False,
            "candidate_count": len(reports),
            "family_count": len(FAMILY_NAMES),
            "candidates_per_family": 10,
            "target_id": "following_horizontal16x8_dct_dct",
            "target": (
                "32x8 4:2:0 root PARTITION_SPLIT with two level-3 top-row "
                "PARTITION_HORZ Horizontal16x8 leaves; both unsplit TX16x8 "
                "DCT_DCT luma and skipped TX8x4 U/V chroma, right luma "
                "non-empty"
            ),
            "families": list(FAMILY_NAMES),
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "qualified_candidates": [report["id"] for report in reports if report["qualifies"]],
            "by_common_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in common_reasons
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic following Horizontal16x8 traces: {args.output}")


if __name__ == "__main__":
    main()
