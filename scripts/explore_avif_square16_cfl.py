#!/usr/bin/env python3
"""Search a fixed public corpus for an origin Square16 I444 CFL leaf.

The campaign is deliberately bounded and input-driven. It creates exactly
one hundred deterministic 16x16 RGB candidates, encodes every candidate twice
through the pinned Pillow/libavif/libaom oracle, and classifies an independent
instrumented scalar dav1d trace. Generated AVIF files are temporary unless
``--retain-dir`` is provided. The JSON report retains every rejection reason,
the complete trace and YUV hashes, and never invokes repository Rust code.
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
SUBSAMPLING = "4:4:4"
ADVANCED = {
    "min-partition-size": "16",
    "max-partition-size": "16",
    "use-intra-dct-only": "1",
    "enable-filter-intra": "0",
    "enable-intra-edge-filter": "0",
    "enable-smooth-intra": "0",
    "enable-paeth-intra": "0",
    "enable-directional-intra": "0",
    "enable-cfl-intra": "1",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "loopfilter-control": "0",
    "aq-mode": "0",
    "deltaq-mode": "0",
}
FAMILY_NAMES = (
    "F01_horizontal_positive",
    "F02_horizontal_opposed",
    "F03_vertical_positive",
    "F04_vertical_opposed",
    "F05_diagonal_positive",
    "F06_diagonal_opposed",
    "F07_checker_mosaic",
    "F08_quadrant_steps",
    "F09_cross_radial",
    "F10_mixed_frequency",
)
BLOCK_PATTERN = re.compile(
    r"^poc=(?P<poc>-?\d+),y=(?P<y>-?\d+),x=(?P<x>-?\d+),"
    r"bl=(?P<level>\d+),ctx=(?P<context>\d+),bp=(?P<partition>\d+): "
    r"r=(?P<range>\d+)$"
)
YMODE_PATTERN = re.compile(r"^Post-ymode\[(?P<mode>\d+)\]")
UVMODE_PATTERN = re.compile(r"^Post-uvmode\[(?P<mode>\d+)\]")
ALPHA_PATTERN = re.compile(
    r"^Post-uvalphas\[(?P<alpha_u>-?\d+)/(?P<alpha_v>-?\d+)\]"
)
TX_PATTERN = re.compile(r"^Post-tx\[(?P<tx>\d+)\]")
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
    """Convert a bounded synthetic full-range YUV sample to RGB."""

    du = u - 128
    dv = v - 128
    return (
        clamp(y + (358 * dv + 128) // 256),
        clamp(y - (88 * du + 183 * dv + 128) // 256),
        clamp(y + (453 * du + 128) // 256),
    )


def signal(family: int, index: int, x: int, y: int) -> int:
    """Return a deterministic signed luma/chroma correlation field."""

    phase = (7 * family + 11 * index) % 16
    horizontal = ((x * 17 + phase) % 32) - 16
    vertical = ((y * 19 + phase) % 32) - 16
    diagonal = (((x + y) * 13 + phase) % 32) - 16
    opposing = (((x - y) * 11 + phase) % 32) - 16
    if family == 0:
        return horizontal * 4
    if family == 1:
        return (horizontal if y % 2 == 0 else -horizontal) * 4
    if family == 2:
        return vertical * 4
    if family == 3:
        return (vertical if x % 2 == 0 else -vertical) * 4
    if family == 4:
        return diagonal * 4
    if family == 5:
        return opposing * 4
    if family == 6:
        return (38 if (x // 2 + y // 2 + index) % 2 == 0 else -38) + horizontal
    if family == 7:
        quadrant = (x // 4) + 4 * (y // 4)
        return ((quadrant * 23 + phase) % 128) - 64
    if family == 8:
        distance = abs(x - 7) + abs(y - 7)
        return 80 - 10 * distance + (18 if x == 7 or y == 7 else 0)
    return diagonal * 2 + horizontal + (28 if (x + 2 * y + index) % 5 == 0 else -14)


def candidate_pixels(family: int, index: int) -> bytes:
    """Create one deterministic RGB candidate from a synthetic YUV field."""

    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            value = signal(family, index, x, y)
            orthogonal = ((23 * x + 29 * y + 7 * index + family) % 13) - 6
            luma = clamp(128 + value // 3)
            u = clamp(128 + value // 5 + orthogonal)
            v = clamp(128 - (value * (3 + index % 3)) // 20 - orthogonal)
            pixels.extend(yuv_to_rgb(luma, u, v))
    return bytes(pixels)


def candidates() -> list[dict[str, object]]:
    """Return exactly ten deterministic families with ten cases each."""

    result = []
    for family, name in enumerate(FAMILY_NAMES):
        for index in range(10):
            result.append(
                {
                    "id": f"CFL-F{family + 1:02d}-N{index:02d}",
                    "family": name,
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


def parse_groups(output: str) -> tuple[list[dict[str, int]], list[list[str]], int]:
    """Parse the independent partition, leaf-group, and entropy trace."""

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
        match = BLOCK_PATTERN.fullmatch(line)
        if match is not None:
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


def parse_dq_matrices(lines: list[str]) -> list[list[int]]:
    """Extract complete 16x16 dequantized matrices from a debug trace."""

    matrices: list[list[int]] = []
    for index, line in enumerate(lines):
        if line != "dq":
            continue
        rows = []
        for row in lines[index + 1 : index + 17]:
            values = row.split()
            if len(values) != 16:
                break
            try:
                rows.extend(int(value) for value in values)
            except ValueError:
                break
        if len(rows) == 256:
            matrices.append(rows)
    return matrices


def plane_ac(y_plane: bytes) -> list[int]:
    """Apply the exact 16x16 CFL centering arithmetic to decoded luma."""

    scaled = [sample * 8 for sample in y_plane]
    mean = (128 + sum(scaled)) >> 8
    return [value - mean for value in scaled]


def classify(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
) -> dict[str, object]:
    """Apply exact predicates for the origin full-resolution CFL witness."""

    root = blocks[0] if len(blocks) == 1 else None
    leaf = groups[0] if len(groups) == 1 else []
    y_modes = [
        int(match["mode"])
        for line in leaf
        if (match := YMODE_PATTERN.match(line)) is not None
    ]
    uv_modes = [
        int(match["mode"])
        for line in leaf
        if (match := UVMODE_PATTERN.match(line)) is not None
    ]
    alphas = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in leaf
        if (match := ALPHA_PATTERN.match(line)) is not None
    ]
    transforms = [
        int(match["tx"])
        for line in leaf
        if (match := TX_PATTERN.match(line)) is not None
    ]
    luma_payloads = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in leaf
        if (match := LUMA_PATTERN.match(line)) is not None
    ]
    chroma_payloads = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in leaf
        if (match := CHROMA_PATTERN.match(line)) is not None
    ]
    matrices = parse_dq_matrices(leaf)
    y_plane = yuv[:256]
    ac = plane_ac(y_plane) if len(yuv) >= 768 else []
    alpha_nonzero = (
        len(alphas) == 1
        and all(
            value != 0 and -16 <= value <= 16
            for value in (alphas[0]["alpha_u"], alphas[0]["alpha_v"])
        )
    )
    chroma_planes = {payload["plane"] for payload in chroma_payloads}
    predicates = {
        "single_origin_square16_root": (
            root is not None
            and root["poc"] == 0
            and root["x"] == 0
            and root["y"] == 0
            and root["level"] == 3
            and root["context"] == 0
            and root["partition"] == 0
        ),
        "single_leaf_group": len(groups) == 1,
        "eight_bit_full_resolution": (
            portable_color.get("width") == 16
            and portable_color.get("height") == 16
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is False
            and portable_color.get("subsampling_y") is False
        ),
        "one_origin_skip": sum(line.startswith("Post-skip[") for line in leaf) == 1,
        "origin_ymode_zero": y_modes == [0],
        "origin_uvmode_cfl": uv_modes == [13],
        "nonzero_cfl_alphas": alpha_nonzero,
        "one_tx16_transform": transforms == [2],
        "one_nonempty_luma_tx16": (
            len(luma_payloads) == 1
            and luma_payloads[0]["tx"] == 2
            and luma_payloads[0]["txtp"] == 0
            and luma_payloads[0]["eob"] >= 1
        ),
        "two_nonempty_chroma_tx16": (
            len(chroma_payloads) == 2
            and chroma_planes == {0, 1}
            and all(
                payload["tx"] == 2
                and payload["txtp"] == 0
                and payload["eob"] >= 1
                for payload in chroma_payloads
            )
        ),
        "no_extra_coefficient_blocks": (
            len(luma_payloads) == 1 and len(chroma_payloads) == 2
        ),
        "luma_cfl_ac_has_both_signs": bool(ac) and min(ac) < 0 < max(ac),
        "both_chroma_dq_have_ac": (
            len(matrices) == 3
            and all(any(value != 0 for value in matrix[1:]) for matrix in matrices[1:])
        ),
    }
    return {
        "root_partition": root,
        "group_count": len(groups),
        "luma_modes": y_modes,
        "uv_modes": uv_modes,
        "alphas": alphas,
        "transform_symbols": transforms,
        "luma_payloads": luma_payloads,
        "chroma_payloads": chroma_payloads,
        "dq_matrix_count": len(matrices),
        "dq_matrix_sha256": [
            sha256(json.dumps(matrix, separators=(",", ":")).encode())
            for matrix in matrices
        ],
        "cfl_ac_min": min(ac) if ac else None,
        "cfl_ac_max": max(ac) if ac else None,
        "predicates": predicates,
        "rejection_reasons": [name for name, passed in predicates.items() if not passed],
        "qualifies": all(predicates.values()),
    }


def trace(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    item: bytes,
    stem: str,
    ordinal: int,
) -> tuple[str, list[dict[str, int]], list[list[str]], int, bytes]:
    """Trace one AV1 color item with the independent scalar dav1d binary."""

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
    blocks, groups, entropy_count = parse_groups(result.stdout)
    return result.stdout, blocks, groups, entropy_count, yuv_path.read_bytes()


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
    trace_a, blocks_a, groups_a, entropy_a, yuv_a = trace(
        executable, environment, work, item_a, str(candidate["id"]), 1
    )
    trace_b, blocks_b, groups_b, entropy_b, yuv_b = trace(
        executable, environment, work, item_b, str(candidate["id"]), 2
    )
    portable_color = portable_color_reference(path_a)
    with Image.open(BytesIO(encoded_a)) as decoded:
        decoded.load()
        if decoded.mode != "RGB" or decoded.size != SIZE:
            raise RuntimeError(f"{candidate['id']} Pillow decode is not 16x16 RGB")
        pillow_rgb = decoded.tobytes()
    classification = classify(blocks_a, groups_a, yuv_a, portable_color)
    classification["predicates"].update(
        {
            "double_encode_equal": encoded_a == encoded_b,
            "double_item_equal": item_a == item_b,
            "double_trace_equal": (
                trace_a == trace_b
                and blocks_a == blocks_b
                and groups_a == groups_b
                and yuv_a == yuv_b
            ),
            "yuv_is_three_256_byte_planes": len(yuv_a) == 768,
        }
    )
    classification["rejection_reasons"] = [
        name for name, passed in classification["predicates"].items() if not passed
    ]
    classification["qualifies"] = all(classification["predicates"].values())
    report = {
        "id": candidate["id"],
        "family": candidate["family"],
        "family_index": candidate["family_index"],
        "candidate_index": candidate["candidate_index"],
        "seed": candidate["seed"],
        "quality": quality,
        "speed": speed,
        "input_rgb_sha256": sha256(pixels),
        "encoded_file_sha256": sha256(encoded_a),
        "encoded_file_sha256_second": sha256(encoded_b),
        "encoded_item_sha256": sha256(item_a),
        "encoded_item_sha256_second": sha256(item_b),
        "encoded_item_length": len(item_a),
        "pillow_rgb_sha256": sha256(pillow_rgb),
        "pillow_rgb_length": len(pillow_rgb),
        "entropy_operation_count": entropy_a,
        "entropy_operation_count_second": entropy_b,
        "yuv_sha256": sha256(yuv_a),
        "yuv_sha256_second": sha256(yuv_b),
        "y_plane_sha256": sha256(yuv_a[:256]),
        "u_plane_sha256": sha256(yuv_a[256:512]),
        "v_plane_sha256": sha256(yuv_a[512:768]),
        "yuv_length": len(yuv_a),
        "trace_sha256": sha256(trace_a.encode()),
        "trace_sha256_second": sha256(trace_b.encode()),
        "portable_color": portable_color,
        "trace": trace_a,
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
    with tempfile.TemporaryDirectory(prefix="image-star-avif-square16-cfl-") as name:
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
            "target": (
                "origin Square16 8-bit 4:4:4 with coded CFL UV mode 13, nonzero U/V "
                "alpha, one nonempty TX16x16 luma block, and two nonempty DCT-DCT "
                "TX16x16 chroma blocks"
            ),
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
    print(f"Written {len(reports)} deterministic Square16 CFL traces: {args.output}")


if __name__ == "__main__":
    main()
