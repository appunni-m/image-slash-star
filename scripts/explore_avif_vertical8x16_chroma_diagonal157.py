#!/usr/bin/env python3
"""Search a fixed corpus for a rectangular AV1 chroma Diagonal157 leaf.

The campaign is deliberately bounded and input-driven. It creates exactly one
hundred deterministic 16x16 RGB candidates, encodes each twice through the
pinned Pillow/libavif/libaom oracle, and classifies two independent traces from
the pinned scalar dav1d executable. Generated files are temporary unless
``--retain-dir`` is supplied; no repository Rust code is invoked.
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

from explore_avif_chroma_diagonal113 import parse_trace
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
SUBSAMPLING = "4:2:0"
ADVANCED = {
    "min-partition-size": "8",
    "max-partition-size": "16",
    "use-intra-dct-only": "0",
    "enable-filter-intra": "0",
    "enable-intra-edge-filter": "0",
    "enable-smooth-intra": "0",
    "enable-paeth-intra": "0",
    "enable-directional-intra": "1",
    "enable-cfl-intra": "0",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "loopfilter-control": "0",
    "aq-mode": "0",
    "deltaq-mode": "0",
}
FAMILY_NAMES = (
    "rect_d157_saw_52",
    "rect_d157_saw_73",
    "rect_d157_saw_94",
    "rect_d157_step_52",
    "rect_d157_step_73",
    "rect_d157_right_bias",
    "rect_d157_edge_ramp",
    "rect_d157_luma_partition",
    "rect_d157_dual_ac",
    "rect_d157_mirror",
)
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


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of one byte sequence."""

    return hashlib.sha256(data).hexdigest()


def clamp(value: int) -> int:
    """Clamp one generated RGB component to the 8-bit range."""

    return max(0, min(255, value))


def yuv_to_rgb(y: int, u: int, v: int) -> tuple[int, int, int]:
    """Convert a bounded synthetic BT.601-like YUV sample to RGB."""

    du = u - 128
    dv = v - 128
    return (
        clamp(y + (358 * dv + 128) // 256),
        clamp(y - (88 * du + 183 * dv + 128) // 256),
        clamp(y + (453 * du + 128) // 256),
    )


def chroma_deltas(family: int, index: int, cx: int, cy: int) -> tuple[int, int]:
    """Return deterministic U/V deltas for one 4:2:0 chroma sample."""

    seed = 1000 + 10 * family + index
    phase = (11 * index + 7 * family + 3) % 32
    amplitude = 16 + (index % 5) * 3
    if family in (0, 3, 5, 6, 7, 8):
        coordinate = 5 * cx - 2 * cy + phase
    elif family in (1, 4):
        coordinate = 7 * cx - 3 * cy + phase
    elif family == 2:
        coordinate = 9 * cx - 4 * cy + phase
    else:
        coordinate = 2 * cx - 5 * cy + phase
    wrapped = coordinate % 32
    wave = wrapped - 16
    if family in (3, 4):
        chroma = amplitude if wrapped >= 16 else -amplitude
    else:
        chroma = (wave * amplitude) // 16
    if family == 5 and cx >= 4:
        chroma *= 2
    if family == 6:
        chroma += (3 * cy + phase) % 7 - 3
    if family == 5:
        return (
            chroma + ((37 * cx + 19 * cy + seed) % 121) - 60,
            chroma + ((23 * cx + 47 * cy + 3 * seed) % 121) - 60,
        )
    if family == 7:
        return (
            ((17 * cx + 31 * cy + seed) % 121) - 60,
            ((29 * cx + 13 * cy + 2 * seed) % 121) - 60,
        )
    if family == 8:
        chroma += ((3 * cx + 5 * cy + seed) % 7) - 3
    if family in (1, 4, 9):
        return chroma, -chroma
    if family == 9:
        return -chroma, chroma
    return chroma, chroma


def candidate_pixels(family: int, index: int) -> bytes:
    """Create one deterministic 16x16 RGB candidate from a YUV-shaped field."""

    seed = 1000 + 10 * family + index
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            cx, cy = x // 2, y // 2
            u_delta, v_delta = chroma_deltas(family, index, cx, cy)
            luma = 128
            if family in (5, 7, 8):
                luma += 14 if x >= 8 and ((x // 2 + y // 2 + seed) % 2) else 0
            elif family == 8:
                luma += ((7 * x + 11 * y + seed) % 17) - 8
            elif family == 9:
                luma += ((x // 2 + 3 * (y // 2) + seed) % 13) - 6
            if family == 6 and x >= 8:
                luma += 8
            pixels.extend(yuv_to_rgb(luma, 128 + u_delta, 128 + v_delta))
    return bytes(pixels)


def candidates() -> list[dict[str, object]]:
    """Return exactly ten deterministic families with ten cases each."""

    result = []
    for family, family_name in enumerate(FAMILY_NAMES):
        for index in range(10):
            result.append(
                {
                    "id": f"R157-F{family + 1:02d}-N{index:02d}",
                    "family": family_name,
                    "family_index": family,
                    "candidate_index": index,
                    "seed": 1000 + 10 * family + index,
                    "pixels": candidate_pixels(family, index),
                    "quality": 76,
                    "speed": 0,
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


def trace(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    item: bytes,
    stem: str,
    ordinal: int,
) -> tuple[str, list[dict[str, int]], list[list[str]], int]:
    """Trace one color item with the independent scalar dav1d executable."""

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
    return result.stdout, blocks, groups, entropy_count


def classify(
    blocks: list[dict[str, int]], groups: list[list[str]], left_edge_observable: bool
) -> dict[str, object]:
    """Apply exact predicates for a following Vertical8x16 mode-6 leaf."""

    root = next(
        (
            block
            for block in blocks
            if block["x"] == 0 and block["y"] == 0 and block["partition"] == 2
        ),
        None,
    )
    luma_groups = []
    chroma_groups = []
    uv_modes = []
    for group in groups:
        luma_payloads = []
        chroma_payloads = []
        for line in group:
            if line.startswith("Post-uvmode["):
                uv_modes.append(int(line.split("[", 1)[1].split("]", 1)[0]))
            if match := LUMA_PATTERN.match(line):
                luma_payloads.append({name: int(value) for name, value in match.groupdict().items()})
            if match := CHROMA_PATTERN.match(line):
                chroma_payloads.append({name: int(value) for name, value in match.groupdict().items()})
        luma_groups.append(luma_payloads)
        chroma_groups.append(chroma_payloads)
    right_luma = luma_groups[1] if len(luma_groups) == 2 else []
    right_chroma = chroma_groups[1] if len(chroma_groups) == 2 else []

    def is_vertical8x16_luma(payloads: list[dict[str, int]]) -> bool:
        """Accept the unsplit or legal two-TX8x8 luma form of Vertical8x16."""

        return (len(payloads) == 1 and payloads[0]["tx"] == 7) or (
            len(payloads) == 2 and all(payload["tx"] == 1 for payload in payloads)
        )

    predicates = {
        "vertical_split_root": root is not None,
        "two_visible_leaf_groups": len(groups) == 2,
        "both_vertical8x16_luma": (
            len(luma_groups) == 2
            and all(is_vertical8x16_luma(payloads) for payloads in luma_groups)
        ),
        "right_uv_mode_6": len(uv_modes) == 2 and uv_modes[1] == 6,
        "right_chroma_r4x8": (
            len(right_chroma) == 2
            and {payload["plane"] for payload in right_chroma} == {0, 1}
            and all(payload["tx"] == 5 for payload in right_chroma)
        ),
        "right_chroma_dct_adst": (
            len(right_chroma) == 2 and all(payload["txtp"] == 2 for payload in right_chroma)
        ),
        "right_chroma_nonempty_ac": (
            len(right_chroma) == 2 and all(payload["eob"] >= 1 for payload in right_chroma)
        ),
        "left_edge_observable": left_edge_observable,
        "no_filter_intra": not any(
            line.startswith("Post-filterintramode[") for line in groups[1]
        ) if len(groups) == 2 else False,
    }
    return {
        "root_partition": root,
        "group_count": len(groups),
        "uv_modes": uv_modes,
        "left_luma_payloads": luma_groups[0] if luma_groups else [],
        "right_luma_payloads": right_luma,
        "left_chroma_payloads": chroma_groups[0] if chroma_groups else [],
        "right_chroma_payloads": right_chroma,
        "predicates": predicates,
        "rejection_reasons": [name for name, passed in predicates.items() if not passed],
        "qualifies": all(predicates.values()),
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
    encoded_a = encode(pixels, quality, speed)
    encoded_b = encode(pixels, quality, speed)
    path_a = work / f"{candidate['id']}.avif"
    path_b = work / f"{candidate['id']}-second.avif"
    path_a.write_bytes(encoded_a)
    path_b.write_bytes(encoded_b)
    item_a, _ = extract_color_item(path_a)
    item_b, _ = extract_color_item(path_b)
    trace_a, blocks_a, groups_a, entropy_a = trace(
        executable, environment, work, item_a, str(candidate["id"]), 1
    )
    trace_b, blocks_b, groups_b, entropy_b = trace(
        executable, environment, work, item_b, str(candidate["id"]), 2
    )
    left_edge = any(
        delta != 0
        for cy in range(8)
        for delta in chroma_deltas(
            int(candidate["family_index"]), int(candidate["candidate_index"]), 3, cy
        )
    )
    classification = classify(blocks_a, groups_a, left_edge)
    classification["predicates"].update(
        {
            "double_encode_equal": encoded_a == encoded_b,
            "double_trace_equal": (
                trace_a == trace_b and blocks_a == blocks_b and groups_a == groups_b
            ),
        }
    )
    classification["rejection_reasons"] = [
        name for name, passed in classification["predicates"].items() if not passed
    ]
    classification["qualifies"] = all(classification["predicates"].values())
    portable_color = portable_color_reference(path_a)
    report = {
        "id": candidate["id"],
        "family": candidate["family"],
        "seed": candidate["seed"],
        "quality": quality,
        "speed": speed,
        "input_rgb_sha256": sha256(pixels),
        "encoded_file_sha256": sha256(encoded_a),
        "encoded_file_sha256_second": sha256(encoded_b),
        "encoded_item_sha256": sha256(item_a),
        "encoded_item_sha256_second": sha256(item_b),
        "encoded_item_length": len(item_a),
        "entropy_operation_count": entropy_a,
        "entropy_operation_count_second": entropy_b,
        "partition_blocks": blocks_a,
        "partition_blocks_second": blocks_b,
        "trace_sha256": sha256(trace_a.encode()),
        "trace_sha256_second": sha256(trace_b.encode()),
        "portable_color": portable_color,
        **classification,
    }
    if report["qualifies"] and retain_dir is not None:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path_a.name).write_bytes(encoded_a)
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
    with tempfile.TemporaryDirectory(prefix="image-star-avif-rectangular-157-") as name:
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
            "max_threads": 1,
            "autotiling": False,
            "advanced": ADVANCED,
        },
        "search": {
            "candidate_count": len(reports),
            "seed_formula": "1000 + 10*family_index + candidate_index",
            "target": "two side-by-side Vertical8x16 leaves with following right UV mode 6 (Diagonal157), R4x8 U/V, DctAdst, and non-empty AC",
            "families": list(FAMILY_NAMES),
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "by_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in sorted(reports[0]["predicates"])
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic Diagonal157 traces: {args.output}")


if __name__ == "__main__":
    main()
