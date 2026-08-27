#!/usr/bin/env python3
"""Search a bounded corpus for AV1 Vertical4x16 transform witnesses.

The campaign creates exactly one hundred deterministic 16x16 RGB candidates
in ten families.  Each candidate is encoded twice through the pinned Pillow
AVIF oracle and its color item is decoded twice through an independently
instrumented scalar dav1d build.  A candidate qualifies only when it has one
origin ``PARTITION_V4`` frame with four unsplit ``Vertical4x16`` luma leaves,
no optional prediction syntax, and a non-empty luma transform pair whose
symbol and dav1d transform type agree.

The input corpus is the existing predictor-oriented ten-family corpus with
its raster transposed.  This changes the public raster only; it does not
encode expected decoder output or invoke repository Rust.  Files are
temporary unless ``--retain-dir`` is supplied.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from explore_avif_horizontal16x4_predictor import (
    ADVANCED,
    FAMILY_NAMES,
    candidates as horizontal_candidates,
)
from generate_av1_reconstruction_refs import (
    DAV1D_COMMIT,
    build_dav1d,
    extract_color_item,
    portable_color_reference,
    resolve_tool,
    run,
    verify_source,
)


SIZE = (16, 16)
EXPECTED_YUV_BYTES = SIZE[0] * SIZE[1] + 2 * (SIZE[0] // 2) * (SIZE[1] // 2)
SUBSAMPLING = "4:2:0"
TARGETS = {
    "adst_dct": {"cdf_symbol": 5, "txtp": 1, "name": "inverse_adst_dct4x16"},
    "adst_adst": {"cdf_symbol": 4, "txtp": 3, "name": "inverse_adst_adst4x16"},
}
PREDICTOR_PROFILES = {
    "dc-only": (0,),
    "dc-horizontal-paeth": (0, 2, 12),
}
OBSERVED_MAPPINGS = {
    "idtx": {"cdf_symbol": 0, "txtp": 9},
    "dct_dct": {"cdf_symbol": 1, "txtp": 0},
    "v_dct": {"cdf_symbol": 2, "txtp": 10},
    "h_dct": {"cdf_symbol": 3, "txtp": 11},
    "adst_adst": {"cdf_symbol": 4, "txtp": 3},
    "adst_dct": {"cdf_symbol": 5, "txtp": 1},
    "dct_adst": {"cdf_symbol": 6, "txtp": 2},
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
    r"^Post-txtp-intra\[(?P<maximum>\d+)->(?P<minimum>\d+)\]"
    r"\[(?P<mode>\d+)\]\[(?P<symbol>\d+)->(?P<txtp>-?\d+)\]"
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of ``data``."""

    return hashlib.sha256(data).hexdigest()


def orient_rgb(pixels: bytes, orientation: str) -> bytes:
    """Orient a packed RGB16x16 raster while preserving its channels.

    The transpose and rotation variants make the source's four-row horizontal
    bands into four-column vertical bands.  The vertical reflection is kept as
    a separate deterministic search variant because ADST basis functions are
    direction-sensitive and a reversal can change the encoder's chosen
    transform without changing the target geometry.
    """

    width, height = SIZE
    result = bytearray(len(pixels))
    for y in range(height):
        for x in range(width):
            if orientation == "transpose":
                source_x, source_y = y, x
            elif orientation == "rotate-cw":
                source_x, source_y = y, height - 1 - x
            elif orientation == "rotate-ccw":
                source_x, source_y = width - 1 - y, x
            elif orientation == "vertical-flip":
                source_x, source_y = x, height - 1 - y
            else:
                raise ValueError(f"unknown orientation: {orientation}")
            source = (source_y * width + source_x) * 3
            target = (y * width + x) * 3
            result[target : target + 3] = pixels[source : source + 3]
    return bytes(result)


def verify_orientation_mapping() -> None:
    """Guard the search corpus against a silent packed-RGB axis swap."""

    source = bytes(
        (x * 17 + y * 31 + channel * 73) % 251
        for y in range(SIZE[1])
        for x in range(SIZE[0])
        for channel in range(3)
    )
    oriented = orient_rgb(source, "vertical-flip")

    def pixel(data: bytes, x: int, y: int) -> bytes:
        start = (y * SIZE[0] + x) * 3
        return data[start : start + 3]

    for output, expected in (
        ((0, 0), (0, 15)),
        ((15, 0), (15, 15)),
        ((0, 15), (0, 0)),
        ((15, 15), (15, 0)),
    ):
        if pixel(oriented, *output) != pixel(source, *expected):
            raise AssertionError(
                "vertical-flip mapping must use output[x,y] = source[x,15-y]"
            )


def candidates(orientation: str = "transpose") -> list[dict[str, object]]:
    """Return one oriented ten-family corpus with stable candidate IDs."""

    result = []
    for candidate in horizontal_candidates():
        family_index = int(candidate["family_index"])
        candidate_index = int(candidate["candidate_index"])
        pixels = candidate["pixels"]
        if not isinstance(pixels, bytes):
            raise TypeError("source candidate pixels must be bytes")
        result.append(
            {
                "id": f"v4x16-f{family_index + 1:02d}-n{candidate_index:02d}",
                "family": FAMILY_NAMES[family_index],
                "family_index": family_index,
                "candidate_index": candidate_index,
                "seed": 264_000 + 100 * family_index + candidate_index,
                "quality": int(candidate["quality"]),
                "amplitude": int(candidate["amplitude"]),
                "pixels": orient_rgb(pixels, orientation),
                "speed": int(candidate["speed"]),
            }
        )
    if len(result) != 100:
        raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
    if any(
        sum(candidate["family"] == family for candidate in result) != 10
        for family in FAMILY_NAMES
    ):
        raise AssertionError("each campaign family must contain exactly ten cases")
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
    """Parse partition blocks, leaf groups, and contiguous scalar entropy."""

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
    if not groups:
        raise RuntimeError("missing leaf groups")
    return blocks, groups, len(entropy)


def parse_group(group: list[str]) -> dict[str, object]:
    """Extract modes, transform decisions, and coefficient payloads."""

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
    transform_records = []
    for line in group:
        if match := TXTP_PATTERN.match(line):
            transform_records.append(
                {name: int(value) for name, value in match.groupdict().items()}
            )
    luma_payloads = []
    chroma_payloads = []
    for line in group:
        if match := LUMA_PATTERN.match(line):
            luma_payloads.append(
                {name: int(value) for name, value in match.groupdict().items()}
            )
        if match := CHROMA_PATTERN.match(line):
            chroma_payloads.append(
                {name: int(value) for name, value in match.groupdict().items()}
            )
    return {
        "y_modes": y_modes,
        "uv_modes": uv_modes,
        "tx_sizes": tx_sizes,
        "transform_records": transform_records,
        "luma_payloads": luma_payloads,
        "chroma_payloads": chroma_payloads,
    }


def classify(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
    allowed_luma_modes: tuple[int, ...],
) -> dict[str, object]:
    """Apply exact Vertical4x16 geometry and transform predicates."""

    parsed = [parse_group(group) for group in groups]
    root = next(
        (
            block
            for block in blocks
            if block["x"] == 0
            and block["y"] == 0
            and block["level"] == 3
            and block["context"] == 0
        ),
        None,
    )
    no_optional_syntax = not any(
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
                "y-cfl-pred",
                "u-cfl-pred",
                "v-cfl-pred",
            )
        )
        for leaf in groups
        for line in leaf
    )
    y_modes = [mode for leaf in parsed for mode in leaf["y_modes"]]
    uv_modes = [mode for leaf in parsed for mode in leaf["uv_modes"]]
    tx_sizes = [tx for leaf in parsed for tx in leaf["tx_sizes"]]
    luma_payloads = [payload for leaf in parsed for payload in leaf["luma_payloads"]]
    chroma_payloads = [payload for leaf in parsed for payload in leaf["chroma_payloads"]]
    common = {
        "frame_is_16x16_8bit_420": (
            portable_color.get("width") == SIZE[0]
            and portable_color.get("height") == SIZE[1]
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "origin_partition_v4": (
            root is not None
            and root["partition"] == 9
            and root["level"] == 3
            and root["x"] == 0
            and root["y"] == 0
        ),
        "four_terminal_groups": len(groups) == 4,
        "no_optional_prediction_syntax": no_optional_syntax,
        "four_dc_luma_modes": y_modes == [0, 0, 0, 0],
        "luma_modes_in_profile": len(y_modes) == 4
        and all(mode in allowed_luma_modes for mode in y_modes),
        "four_unsplit_vertical4x16_luma": tx_sizes == [13, 13, 13, 13],
        "four_luma_payloads": (
            len(luma_payloads) == 4
            and all(payload["tx"] == 13 for payload in luma_payloads)
        ),
        "four_chroma_payloads": (
            len(chroma_payloads) == 4
            and {payload["plane"] for payload in chroma_payloads} == {0, 1}
            and all(payload["tx"] == 5 and payload["txtp"] == 0 for payload in chroma_payloads)
        ),
        "full_yuv_output": len(yuv) == EXPECTED_YUV_BYTES,
        "nonconstant_luma": len(yuv[: SIZE[0] * SIZE[1]]) > 0
        and len(set(yuv[: SIZE[0] * SIZE[1]])) > 1,
    }
    target_predicates = {}
    for target, expected in TARGETS.items():
        matches = []
        for leaf_index, leaf in enumerate(parsed):
            for transform in leaf["transform_records"]:
                for payload in leaf["luma_payloads"]:
                    if (
                        transform["maximum"] == 13
                        and transform["symbol"] == expected["cdf_symbol"]
                        and transform["txtp"] == expected["txtp"]
                        and payload["tx"] == 13
                        and payload["txtp"] == expected["txtp"]
                        and payload["eob"] > 0
                    ):
                        matches.append(
                            {
                                "leaf": leaf_index,
                                "symbol": transform["symbol"],
                                "txtp": transform["txtp"],
                                "eob": payload["eob"],
                            }
                        )
        target_predicates[target] = {
            "matched_pairs": matches,
            "matched_leaf_groups": sorted({match["leaf"] for match in matches}),
            "matched_leaf_count": len({match["leaf"] for match in matches}),
            "qualifies": bool(matches),
        }
    common_rejections = [
        name
        for name, passed in common.items()
        if not passed and name not in {"four_dc_luma_modes", "luma_modes_in_profile"}
    ]
    profile_predicate = (
        "four_dc_luma_modes"
        if allowed_luma_modes == PREDICTOR_PROFILES["dc-only"]
        else "luma_modes_in_profile"
    )
    if not common[profile_predicate]:
        common_rejections.append(profile_predicate)
    qualified_targets = [
        target
        for target, predicates in target_predicates.items()
        if not common_rejections and predicates["qualifies"]
    ]
    return {
        "root_partition": root,
        "group_count": len(groups),
        "y_modes": y_modes,
        "uv_modes": uv_modes,
        "tx_sizes": tx_sizes,
        "transform_records_by_leaf": [leaf["transform_records"] for leaf in parsed],
        "luma_payloads_by_leaf": [leaf["luma_payloads"] for leaf in parsed],
        "chroma_payloads_by_leaf": [leaf["chroma_payloads"] for leaf in parsed],
        "yuv_bytes": len(yuv),
        "common_predicates": common,
        "target_predicates": target_predicates,
        "rejection_reasons": common_rejections,
        "qualifies": not common_rejections and bool(qualified_targets),
        "qualified_targets": qualified_targets,
    }


def trace_candidate(
    executable: Path,
    environment: dict[str, str],
    item_path: Path,
    work: Path,
    candidate_id: str,
    ordinal: int,
) -> tuple[str, bytes]:
    """Run one scalar dav1d decode and return its trace plus YUV bytes."""

    yuv_path = work / f"{candidate_id}-{ordinal}.yuv"
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


def decode_candidate(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    candidate: dict[str, object],
    retain_dir: Path | None,
    allowed_luma_modes: tuple[int, ...],
) -> dict[str, object]:
    """Double-encode, double-trace, and classify one candidate."""

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
    item, _ = extract_color_item(path)
    second_item, _ = extract_color_item(second_path)
    if item != second_item:
        raise RuntimeError(f"nondeterministic color item for {candidate['id']}")
    item_path = work / f"{candidate['id']}.obu"
    item_path.write_bytes(item)
    trace_a, yuv_a = trace_candidate(executable, environment, item_path, work, str(candidate["id"]), 1)
    trace_b, yuv_b = trace_candidate(executable, environment, item_path, work, str(candidate["id"]), 2)
    if trace_a != trace_b or yuv_a != yuv_b:
        raise RuntimeError(f"nondeterministic dav1d trace or YUV for {candidate['id']}")
    blocks, groups, entropy_count = parse_trace(trace_a)
    with Image.open(BytesIO(encoded)) as decoded:
        pillow_rgb = decoded.convert("RGB").tobytes()
    with Image.open(BytesIO(encoded_second)) as decoded:
        pillow_rgb_second = decoded.convert("RGB").tobytes()
    if pillow_rgb != pillow_rgb_second:
        raise RuntimeError(f"nondeterministic Pillow RGB decode for {candidate['id']}")
    if len(yuv_a) != EXPECTED_YUV_BYTES:
        raise RuntimeError(f"unexpected 4:2:0 YUV length for {candidate['id']}: {len(yuv_a)}")
    portable_color = portable_color_reference(path)
    classification = classify(
        blocks,
        groups,
        yuv_a,
        portable_color,
        allowed_luma_modes,
    )
    if retain_dir is not None and classification["qualifies"]:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path.name).write_bytes(encoded)
    y_plane_bytes = SIZE[0] * SIZE[1]
    chroma_plane_bytes = (SIZE[0] // 2) * (SIZE[1] // 2)
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
        "encoded_file_sha256_second": sha256(encoded_second),
        "encoded_item_sha256": sha256(item),
        "encoded_item_sha256_second": sha256(second_item),
        "encoded_item_length": len(item),
        "pillow_rgb_sha256": sha256(pillow_rgb),
        "pillow_rgb_sha256_second": sha256(pillow_rgb_second),
        "dav1d_trace_sha256": sha256(trace_a.encode()),
        "dav1d_trace_sha256_second": sha256(trace_b.encode()),
        "decoded_yuv_sha256": sha256(yuv_a),
        "decoded_y_plane_sha256": sha256(yuv_a[:y_plane_bytes]),
        "decoded_u_plane_sha256": sha256(yuv_a[y_plane_bytes : y_plane_bytes + chroma_plane_bytes]),
        "decoded_v_plane_sha256": sha256(yuv_a[y_plane_bytes + chroma_plane_bytes :]),
        "entropy_operation_count": entropy_count,
        "partition_blocks": blocks,
        "portable_color": portable_color,
        "repository_rust_invoked": False,
        **classification,
    }


def choose_promotions(reports: list[dict[str, object]]) -> dict[str, str | None]:
    """Choose the smallest deterministic witness for each transform target."""

    selected: dict[str, str | None] = {}
    for target in TARGETS:
        qualifying = [
            report for report in reports if target in report["qualified_targets"]
        ]
        winner = min(
            qualifying,
            key=lambda report: (
                int(report["entropy_operation_count"]),
                int(report["encoded_item_length"]),
                str(report["id"]),
            ),
            default=None,
        )
        selected[target] = winner["id"] if winner is not None else None
    return selected


def main() -> None:
    """Run the pinned 100-case input-only campaign."""

    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument("--dav1d-source", type=Path)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--python-path", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--retain-dir", type=Path)
    parser.add_argument(
        "--orientation",
        choices=("transpose", "rotate-cw", "rotate-ccw", "vertical-flip"),
        default="transpose",
        help="orientation of the horizontal predictor corpus (default: transpose)",
    )
    parser.add_argument(
        "--predictor-profile",
        choices=tuple(PREDICTOR_PROFILES),
        default="dc-only",
        help=(
            "explicit terminal luma predictor set; dc-horizontal-paeth is a "
            "bounded non-DC search profile"
        ),
    )
    args = parser.parse_args()
    verify_orientation_mapping()

    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")

    with tempfile.TemporaryDirectory(prefix="image-star-avif-v4x16-transform-") as name:
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
            )
        else:
            executable = args.dav1d.resolve()
            environment = {}
        version_result = run([str(executable), "--version"], env=environment)
        version = (version_result.stdout + version_result.stderr).strip()
        if not version.startswith("1.5.3-0-gb546257"):
            raise RuntimeError(f"unexpected dav1d executable version: {version}")
        allowed_luma_modes = PREDICTOR_PROFILES[args.predictor_profile]
        reports = [
            decode_candidate(
                executable,
                environment,
                work,
                candidate,
                args.retain_dir,
                allowed_luma_modes,
            )
            for candidate in candidates(args.orientation)
        ]

    rejection_reasons = sorted(
        {
            reason
            for report in reports
            for reason in report["rejection_reasons"]
        }
    )
    transform_records = [
        record
        for report in reports
        for leaf in report["transform_records_by_leaf"]
        for record in leaf
    ]
    mapping_counts = {
        f"{symbol}->{txtp}": sum(
            record["symbol"] == symbol and record["txtp"] == txtp
            for record in transform_records
        )
        for symbol, txtp in sorted(
            {
                (mapping["cdf_symbol"], mapping["txtp"])
                for mapping in OBSERVED_MAPPINGS.values()
            }
        )
    }
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
            "quality_and_amplitude_source": "explore_avif_horizontal16x4_predictor.candidates",
            "orientation": args.orientation,
            "orientation_mapping": {
                "transpose": "output[x,y] = source[y,x]",
                "rotate-cw": "output[x,y] = source[y,15-x]",
                "rotate-ccw": "output[x,y] = source[15-y,x]",
                "vertical-flip": "output[x,y] = source[x,15-y]",
            }[args.orientation],
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
            "target_id": "transform_rect_4x16_v4",
            "target": (
                "16x16 4:2:0 origin PARTITION_V4 frame with four unsplit "
                "Vertical4x16 luma leaves and same-leaf 2D transform pairs; "
                "ADST_DCT requires CDF symbol 5/txtp 1 and ADST_ADST "
                "requires CDF symbol 4/txtp 3; terminal luma modes are "
                f"restricted to {list(allowed_luma_modes)}"
            ),
            "predictor_profile": args.predictor_profile,
            "allowed_luma_modes": list(allowed_luma_modes),
            "targets": TARGETS,
            "observed_transform_mappings": OBSERVED_MAPPINGS,
            "families": list(FAMILY_NAMES),
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "qualified_by_target": {
                target: sum(target in report["qualified_targets"] for report in reports)
                for target in TARGETS
            },
            "qualified_candidates": [report["id"] for report in reports if report["qualifies"]],
            "promotions": choose_promotions(reports),
            "observed_mapping_record_counts": mapping_counts,
            "by_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in rejection_reasons
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic Vertical4x16 transform traces: {args.output}")


if __name__ == "__main__":
    main()
