#!/usr/bin/env python3
"""Search a bounded corpus for AV1 R16x8 luma identity transforms.

The campaign creates exactly one hundred deterministic 16x8 RGB candidates in
ten families, encodes every candidate twice through the pinned Pillow AVIF
oracle, and decodes the extracted color item twice through the independent
scalar dav1d diagnostic build.  A candidate qualifies only when the public
bitstream has one origin Horizontal16x8 leaf, an unsplit TX16x8 luma payload,
and the requested luma transform syntax/payload pair:

* IDTX: transform CDF symbol 0 and dav1d ``txtp=9``;
* V_DCT: transform CDF symbol 2 and dav1d ``txtp=10``.

The report records every candidate and every rejection reason.  It never
invokes repository Rust code.  AVIF files are temporary unless
``--retain-dir`` is supplied.
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


SIZE = (16, 8)
EXPECTED_YUV_BYTES = SIZE[0] * SIZE[1] + 2 * (SIZE[0] // 2) * (SIZE[1] // 2)
SUBSAMPLING = "4:2:0"
QUALITIES = (36, 44, 52, 60, 68, 76, 84, 90, 94, 98)
AMPLITUDES = (24, 32, 40, 48, 56, 64, 72, 80, 88, 96)
FAMILY_NAMES = (
    "F01_checker_1px",
    "F02_checker_2px",
    "F03_full_frame_noise",
    "F04_sparse_impulses",
    "F05_crosshatch",
    "F06_columns_1px",
    "F07_columns_2px",
    "F08_sawtooth_columns",
    "F09_repeated_prbs_row",
    "F10_vertical_bars_ramp",
)
TARGETS = {
    "idtx": {"cdf_symbol": 0, "txtp": 9, "name": "identity16x8"},
    "v_dct": {"cdf_symbol": 2, "txtp": 10, "name": "identity_dct16x8"},
}
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
    r"^Post-txtp-intra\[8->(?P<minimum>\d+)\]\[0\]"
    r"\[(?P<symbol>\d+)->(?P<txtp>-?\d+)\]"
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of ``data``."""

    return hashlib.sha256(data).hexdigest()


def clamp(value: int) -> int:
    """Clamp one generated grayscale sample to the eight-bit range."""

    return max(0, min(255, value))


def signed(value: int, amplitude: int) -> int:
    """Return a signed signal with a bounded amplitude."""

    return amplitude if value >= 0 else -amplitude


def candidate_pixels(family: int, index: int) -> bytes:
    """Generate one deterministic grayscale-as-RGB candidate."""

    amplitude = AMPLITUDES[index]
    seed = 168_000 + 100 * family + index
    state = random.Random(seed)
    prbs = [state.choice((-1, 1)) for _ in range(SIZE[0])]
    impulses = {
        (state.randrange(SIZE[0]), state.randrange(SIZE[1]))
        for _ in range(5 + family % 4)
    }
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if family == 0:
                signal = signed(1 if (x + y + index) % 2 else -1, amplitude)
            elif family == 1:
                signal = signed(1 if (x // 2 + y // 2 + index) % 2 else -1, amplitude)
            elif family == 2:
                signal = state.randrange(-amplitude, amplitude + 1)
            elif family == 3:
                signal = signed(1, amplitude) if (x, y) in impulses else 0
            elif family == 4:
                signal = amplitude // 2 * (
                    (1 if (x + index) % 2 else -1)
                    + (1 if (y + index) % 2 else -1)
                )
            elif family == 5:
                signal = signed(1 if (x + index) % 2 else -1, amplitude)
            elif family == 6:
                signal = signed(1 if (x // 2 + index) % 2 else -1, amplitude)
            elif family == 7:
                levels = (-amplitude, -amplitude // 3, amplitude // 3, amplitude)
                signal = levels[(x + index) % len(levels)]
            elif family == 8:
                signal = amplitude * prbs[x]
            else:
                levels = (-amplitude, -amplitude // 3, amplitude // 3, amplitude)
                signal = levels[(x // 4 + index) % len(levels)] + (y - 3) * 2
            value = clamp(128 + signal)
            pixels.extend((value, value, value))
    return bytes(pixels)


def candidates() -> list[dict[str, object]]:
    """Return exactly ten deterministic families with ten cases each."""

    result = []
    for family, family_name in enumerate(FAMILY_NAMES):
        for index in range(10):
            result.append(
                {
                    "id": f"h16x8-f{family + 1:02d}-n{index:02d}",
                    "family": family_name,
                    "family_index": family,
                    "candidate_index": index,
                    "seed": 168_000 + 100 * family + index,
                    "quality": QUALITIES[index],
                    "amplitude": AMPLITUDES[index],
                    "pixels": candidate_pixels(family, index),
                    "speed": 0,
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
    return blocks, groups, len(entropy)


def parse_group(group: list[str]) -> dict[str, object]:
    """Extract public transform and payload records from one leaf."""

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
    syntax_markers = [
        line.split(":", 1)[0]
        for line in group
        if line.startswith(
            (
                "Post-skip[",
                "Post-cdef_idx[",
                "Post-ymode[",
                "Post-uvmode[",
                "Post-tx[",
                "Post-txtp-intra[",
                "Post-y-cf-blk[",
                "Post-uv-cf-blk[",
            )
        )
    ]
    return {
        "y_modes": y_modes,
        "uv_modes": uv_modes,
        "tx_sizes": tx_sizes,
        "transform_records": transform_records,
        "luma_payloads": luma_payloads,
        "chroma_payloads": chroma_payloads,
        "syntax_markers": syntax_markers,
    }


def classify(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
) -> dict[str, object]:
    """Apply exact public geometry and transform predicates."""

    parsed = [parse_group(group) for group in groups]
    root = blocks[0] if len(blocks) == 1 else None
    group = parsed[0] if len(parsed) == 1 else {}
    no_forbidden_syntax = not any(
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
        for leaf in groups
        for line in leaf
    )
    y_modes = group.get("y_modes", [])
    uv_modes = group.get("uv_modes", [])
    tx_sizes = group.get("tx_sizes", [])
    transform_records = group.get("transform_records", [])
    luma_payloads = group.get("luma_payloads", [])
    chroma_payloads = group.get("chroma_payloads", [])
    common = {
        "frame_is_16x8_8bit_420": (
            portable_color.get("width") == SIZE[0]
            and portable_color.get("height") == SIZE[1]
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "one_origin_horizontal16x8_root": (
            len(blocks) == 1
            and root is not None
            and root["poc"] == 0
            and root["x"] == 0
            and root["y"] == 0
            and root["level"] == 3
            and root["context"] == 0
            and root["partition"] == 1
        ),
        "single_leaf_group": len(groups) == 1,
        "no_filter_palette_or_angle_syntax": no_forbidden_syntax,
        "dc_luma_and_chroma_modes": y_modes == [0] and uv_modes == [0],
        "one_unsplit_tx16x8": tx_sizes == [8],
        "one_nonempty_luma_payload": (
            len(luma_payloads) == 1
            and luma_payloads[0]["tx"] == 8
            and luma_payloads[0]["eob"] >= 0
        ),
        "two_tx8x4_dc_chroma_payloads": (
            len(chroma_payloads) == 2
            and {payload["plane"] for payload in chroma_payloads} == {0, 1}
            and all(payload["tx"] == 6 and payload["txtp"] == 0 for payload in chroma_payloads)
        ),
        "full_yuv_output": len(yuv) == EXPECTED_YUV_BYTES,
    }
    transform = transform_records[0] if len(transform_records) == 1 else None
    target_predicates = {}
    for target, expected in TARGETS.items():
        target_predicates[target] = {
            "one_luma_transform_record": transform is not None,
            "transform_cdf_symbol": (
                transform is not None and transform["symbol"] == expected["cdf_symbol"]
            ),
            "transform_dav1d_type": (
                transform is not None and transform["txtp"] == expected["txtp"]
            ),
            "luma_payload_dav1d_type": (
                len(luma_payloads) == 1 and luma_payloads[0]["txtp"] == expected["txtp"]
            ),
        }
        target_predicates[target]["qualifies"] = all(target_predicates[target].values())
    common_rejections = [name for name, passed in common.items() if not passed]
    return {
        "root_partition": root,
        "group_count": len(groups),
        "y_modes": y_modes,
        "uv_modes": uv_modes,
        "tx_sizes": tx_sizes,
        "transform_records": transform_records,
        "luma_payloads": luma_payloads,
        "chroma_payloads": chroma_payloads,
        "syntax_markers": group.get("syntax_markers", []),
        "yuv_bytes": len(yuv),
        "common_predicates": common,
        "target_predicates": target_predicates,
        "rejection_reasons": common_rejections,
        "qualifies": bool(common) and not common_rejections and any(
            predicates["qualifies"] for predicates in target_predicates.values()
        ),
        "qualified_targets": [
            target
            for target, predicates in target_predicates.items()
            if not common_rejections and predicates["qualifies"]
        ],
    }


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
    portable_color = portable_color_reference(path)
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
    if len(yuv_a) != EXPECTED_YUV_BYTES:
        raise RuntimeError(
            f"unexpected 4:2:0 YUV length for {candidate['id']}: {len(yuv_a)}"
        )
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
        "decoded_y_plane_sha256": sha256(yuv_a[: SIZE[0] * SIZE[1]]),
        "decoded_u_plane_sha256": sha256(yuv_a[SIZE[0] * SIZE[1] : SIZE[0] * SIZE[1] + 32]),
        "decoded_v_plane_sha256": sha256(yuv_a[SIZE[0] * SIZE[1] + 32 :]),
        "entropy_operation_count": entropy_count,
        "partition_blocks": blocks,
        **classify(blocks, groups, yuv_a, portable_color),
    }


def choose_promotions(reports: list[dict[str, object]]) -> dict[str, str | None]:
    """Choose the smallest deterministic witness for each transform target."""

    selected = {}
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
    args = parser.parse_args()

    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")

    with tempfile.TemporaryDirectory(prefix="image-star-avif-h16x8-transform-") as name:
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
        reports = [
            decode_candidate(executable, environment, work, candidate, args.retain_dir)
            for candidate in candidates()
        ]

    common_reasons = (
        "frame_is_16x8_8bit_420",
        "one_origin_horizontal16x8_root",
        "single_leaf_group",
        "no_filter_palette_or_angle_syntax",
        "dc_luma_and_chroma_modes",
        "one_unsplit_tx16x8",
        "one_nonempty_luma_payload",
        "two_tx8x4_dc_chroma_payloads",
        "full_yuv_output",
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
            "target_id": "transform_rect_16x8",
            "target": (
                "16x8 4:2:0 origin Horizontal16x8 leaf with one unsplit TX16x8 "
                "luma payload; IDTX requires CDF symbol 0/txtp 9 and V_DCT "
                "requires CDF symbol 2/txtp 10"
            ),
            "targets": TARGETS,
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
            "by_common_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in common_reasons
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic Horizontal16x8 transform traces: {args.output}")


if __name__ == "__main__":
    main()
