#!/usr/bin/env python3
"""Search a fixed corpus for a right-hand Square8 Diagonal67 AVIF leaf.

The search is deliberately bounded and input-driven. It creates exactly one
hundred deterministic 16x8 RGB candidates, encodes each twice through the
pinned Pillow/libavif/libaom oracle, and classifies an independently
instrumented scalar dav1d trace. Generated files are temporary unless
``--retain-dir`` is supplied; no repository Rust code is invoked.
"""

from __future__ import annotations

import argparse
import json
import re
import tempfile
from pathlib import Path

from PIL import Image, _avif, features

from explore_avif_chroma_diagonal113 import (
    ADVANCED,
    SIZE,
    SUBSAMPLING,
    candidates,
    encode,
    parse_trace,
    sha256,
)
from generate_av1_reconstruction_refs import (
    DAV1D_COMMIT,
    build_dav1d,
    extract_color_item,
    resolve_tool,
    run,
    verify_source,
)


BLOCK_PATTERN = re.compile(
    r"^poc=(?P<poc>-?\d+),y=(?P<y>-?\d+),x=(?P<x>-?\d+),"
    r"bl=(?P<level>\d+),ctx=(?P<context>\d+),bp=(?P<partition>\d+): "
    r"r=(?P<range>\d+)$"
)
YMODE_PATTERN = re.compile(r"^Post-ymode\[(?P<mode>\d+)\]")
LUMA_PATTERN = re.compile(
    r"^Post-y-cf-blk\[tx=(?P<tx>\d+),txtp=(?P<txtp>-?\d+),"
    r"eob=(?P<eob>-?\d+)\]"
)
CHROMA_PATTERN = re.compile(
    r"^Post-uv-cf-blk\[pl=(?P<plane>\d+),tx=(?P<tx>\d+),"
    r"txtp=(?P<txtp>-?\d+),eob=(?P<eob>-?\d+)\]"
)


def classify(blocks: list[dict[str, int]], groups: list[list[str]]) -> dict[str, object]:
    """Apply exact predicates for the right-hand Square8 mode-8 leaf."""

    root = next(
        (
            block
            for block in blocks
            if block["level"] == 3 and block["x"] == 0 and block["y"] == 0
        ),
        None,
    )
    visible = blocks[1:3] if len(blocks) >= 3 else []
    right = groups[1] if len(groups) == 2 else []
    y_modes = [
        int(match["mode"])
        for line in right
        if (match := YMODE_PATTERN.match(line)) is not None
    ]
    luma_payloads = []
    chroma_payloads = []
    for line in right:
        if match := LUMA_PATTERN.match(line):
            luma_payloads.append({name: int(value) for name, value in match.groupdict().items()})
        if match := CHROMA_PATTERN.match(line):
            chroma_payloads.append({name: int(value) for name, value in match.groupdict().items()})
    predicates = {
        "visible_split_root": root is not None and root["partition"] == 3,
        "two_visible_square8_leaves": (
            len(visible) == 2
            and visible[0]["level"] == 4
            and visible[1]["level"] == 4
            and [(block["x"], block["y"]) for block in visible] == [(0, 0), (2, 0)]
        ),
        "two_visible_block_groups": len(groups) == 2,
        "right_uv_mode_8": y_modes == [3]
        and any(line == "Post-uvmode[8]" for line in right),
        "right_square8_luma_tx4x4": len(luma_payloads) == 4
        and all(payload["tx"] == 0 for payload in luma_payloads),
        "right_luma_nonempty": any(payload["eob"] >= 0 for payload in luma_payloads),
        "right_adst_dct_chroma": len(chroma_payloads) == 2
        and {payload["plane"] for payload in chroma_payloads} == {0, 1}
        and all(payload["tx"] == 0 and payload["txtp"] == 1 for payload in chroma_payloads),
        "right_chroma_nonempty_ac": all(payload["eob"] >= 1 for payload in chroma_payloads),
        "top_missing_left_available": (
            len(visible) == 2
            and visible[1]["x"] == 2
            and visible[1]["y"] == 0
            and visible[0]["x"] == 0
            and visible[0]["y"] == 0
        ),
    }
    return {
        "root_partition": root,
        "visible_blocks": visible,
        "group_count": len(groups),
        "right_y_modes": y_modes,
        "right_luma_payloads": luma_payloads,
        "right_chroma_payloads": chroma_payloads,
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
    """Encode, independently decode, and classify one candidate."""

    pixels = candidate["pixels"]
    if not isinstance(pixels, bytes):
        raise TypeError("candidate pixels must be bytes")
    quality = int(candidate["quality"])
    speed = int(candidate["speed"])
    encoded = encode(pixels, quality, speed)
    if encoded != encode(pixels, quality, speed):
        raise RuntimeError(f"nondeterministic encoding for {candidate['id']}")
    path = work / f"{candidate['id']}.avif"
    path.write_bytes(encoded)
    item, _ = extract_color_item(path)
    item_path = work / f"{candidate['id']}.obu"
    yuv_path = work / f"{candidate['id']}.yuv"
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
    report = {
        "id": candidate["id"],
        "family": candidate["family"],
        "seed": candidate["seed"],
        "quality": quality,
        "speed": speed,
        "input_rgb_sha256": sha256(pixels),
        "encoded_file_sha256": sha256(encoded),
        "encoded_item_sha256": sha256(item),
        "encoded_item_length": len(item),
        "entropy_operation_count": entropy_count,
        "partition_blocks": blocks,
        **classify(blocks, groups),
    }
    if report["qualifies"] and retain_dir is not None:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path.name).write_bytes(encoded)
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
    with tempfile.TemporaryDirectory(prefix="image-star-avif-diagonal67-") as name:
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
            "target": "right-hand Square8 leaf with coded UV mode 8 (Diagonal67), ADST-DCT chroma transform, and non-skipped AC chroma residuals",
            "families": [
                "F01_rgb_noise",
                "F02_diagonal_chroma_ramp",
                "F03_hue_ramp",
                "F04_diagonal_two_color",
                "F05_antidiagonal_two_color",
                "F06_blue_ramp",
                "F07_red_ramp",
                "F08_luma_chroma",
                "F09_mosaic",
                "F10_smooth_noise",
            ],
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "by_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in next(iter(reports))["predicates"]
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic Diagonal67 traces: {args.output}")


if __name__ == "__main__":
    main()
