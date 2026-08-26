#!/usr/bin/env python3
"""Run a second, deliberately chroma-biased Diagonal67 search.

The first Diagonal67 campaign reused the Diagonal113 corpus and produced no
mode-8 sentence. This separately reviewed campaign keeps the encoder controls
and acceptance predicates fixed, but generates ten deterministic YUV-shaped
RGB families intended to expose both likely 67-degree chroma orientations.
It still evaluates exactly one hundred candidates, records every rejection,
and never invokes repository Rust code.
"""

from __future__ import annotations

import argparse
import json
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from explore_avif_chroma_diagonal67 import classify
from explore_avif_chroma_diagonal113 import (
    ADVANCED,
    SIZE,
    SUBSAMPLING,
    parse_trace,
    sha256,
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


FAMILY_NAMES = (
    "uv67_wave_25",
    "uv67_wave_37_antiphase",
    "uv67_wave_49",
    "uv67_step_25",
    "uv67_step_37_antiphase",
    "uv67_right_chroma_contrast",
    "uv67_mirror_orientation",
    "uv67_wave_right_luma_checker",
    "uv67_step_right_luma_stripes",
    "uv67_wave_dual_ac",
)


def clamp(value: int) -> int:
    """Clamp one generated RGB component to an 8-bit sample."""

    return max(0, min(255, value))


def yuv_to_rgb(y: int, u: int, v: int) -> tuple[int, int, int]:
    """Convert one limited-amplitude synthetic YUV sample to RGB."""

    du = u - 128
    dv = v - 128
    return (
        clamp(y + (358 * dv + 128) // 256),
        clamp(y - (88 * du + 183 * dv + 128) // 256),
        clamp(y + (453 * du + 128) // 256),
    )


def candidate_pixels(family: int, index: int) -> bytes:
    """Create one deterministic 16x8 RGB candidate from a synthetic YUV field."""

    seed = 700 + 10 * family + index
    phase = (11 * index + 7 * family + 3) % 32
    amplitude = 14 + (index % 5) * 3
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            cx = x // 2
            cy = y // 2
            if family in (0, 5, 9):
                coordinate = 2 * cx + 5 * cy + phase
            elif family in (1, 4, 8):
                coordinate = 3 * cx + 7 * cy + phase
            elif family == 2:
                coordinate = 4 * cx + 9 * cy + phase
            elif family == 3:
                coordinate = 2 * cx + 5 * cy + phase
            else:
                coordinate = (5 * cx + 2 * cy + phase) if x < 8 else (7 * cx + 3 * cy + phase)
            wave = (coordinate % 32) - 16
            step = amplitude if (coordinate % 32) >= 16 else -amplitude
            if family in (3, 4, 8):
                chroma = step
            else:
                chroma = (wave * amplitude) // 16
            if family == 5 and x >= 8:
                chroma *= 2
            if family == 6:
                chroma = ((5 * cx + 2 * cy + phase) % 32) - 16
            if family in (1, 4, 6):
                u_delta, v_delta = chroma, -chroma
            else:
                u_delta, v_delta = chroma, chroma
            if family == 9:
                u_delta += ((3 * cx + 5 * cy + seed) % 7) - 3
                v_delta -= ((5 * cx + 2 * cy + seed) % 7) - 3
            luma = 128
            if family == 7 and x >= 8:
                luma += 12 if ((x // 2 + y // 2 + seed) % 2) else -12
            elif family == 8 and x >= 8:
                luma += 10 if ((x // 4 + seed) % 2) else -10
            elif family == 9 and x >= 8:
                luma += ((7 * x + 11 * y + seed) % 17) - 8
            pixels.extend(yuv_to_rgb(luma, 128 + u_delta, 128 + v_delta))
    return bytes(pixels)


def candidates() -> list[dict[str, object]]:
    """Return exactly ten deterministic families with ten cases each."""

    result = []
    for family, family_name in enumerate(FAMILY_NAMES):
        for index in range(10):
            result.append(
                {
                    "id": f"S2-F{family + 1:02d}-N{index:02d}",
                    "family": family_name,
                    "seed": 700 + 10 * family + index,
                    "family_index": family,
                    "candidate_index": index,
                    "pixels": candidate_pixels(family, index),
                    "quality": 76,
                    "speed": 0,
                }
            )
    if len(result) != 100:
        raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
    return result


def trace(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    item: bytes,
    stem: str,
    ordinal: int,
) -> tuple[str, list[dict[str, int]], list[list[str]], int]:
    """Decode one encoded item through the independent scalar trace."""

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


def left_edge_observable(family: int, index: int) -> bool:
    """Check the generated left chroma edge is intentionally non-neutral."""

    values = []
    for cy in range(4):
        coordinate = (2 * 3 + 5 * cy + (11 * index + 7 * family + 3)) % 32
        wave = (coordinate % 32) - 16
        amplitude = 14 + (index % 5) * 3
        values.append((wave * amplitude) // 16)
    return any(value != 0 for value in values)


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
    encoded_a = Image.frombytes("RGB", SIZE, pixels)
    first = BytesIO()
    encoded_a.save(
        first,
        format="AVIF",
        quality=int(candidate["quality"]),
        speed=int(candidate["speed"]),
        max_threads=1,
        subsampling=SUBSAMPLING,
        autotiling=False,
        advanced=ADVANCED,
    )
    second = BytesIO()
    Image.frombytes("RGB", SIZE, pixels).save(
        second,
        format="AVIF",
        quality=int(candidate["quality"]),
        speed=int(candidate["speed"]),
        max_threads=1,
        subsampling=SUBSAMPLING,
        autotiling=False,
        advanced=ADVANCED,
    )
    encoded_bytes_a = first.getvalue()
    encoded_bytes_b = second.getvalue()
    path = work / f"{candidate['id']}.avif"
    path.write_bytes(encoded_bytes_a)
    item_a, container = extract_color_item(path)
    second_path = work / f"{candidate['id']}-second.avif"
    second_path.write_bytes(encoded_bytes_b)
    item_b, _ = extract_color_item(second_path)
    trace_a, blocks_a, groups_a, entropy_a = trace(
        executable, environment, work, item_a, str(candidate["id"]), 1
    )
    trace_b, blocks_b, groups_b, entropy_b = trace(
        executable, environment, work, item_b, str(candidate["id"]), 2
    )
    classification = classify(blocks_a, groups_a)
    portable_color = portable_color_reference(path)
    dimensions_ok = (
        portable_color["width"] == SIZE[0]
        and portable_color["height"] == SIZE[1]
        and portable_color["bit_depth"] == 8
        and portable_color["subsampling_x"]
        and portable_color["subsampling_y"]
        and not portable_color["monochrome"]
    )
    trace_equal = trace_a == trace_b and blocks_a == blocks_b and groups_a == groups_b
    edge_observable = left_edge_observable(
        int(candidate["family_index"]), int(candidate["candidate_index"])
    )
    classification["predicates"].update(
        {
            "dimensions_8bit_420": dimensions_ok,
            "double_encode_equal": encoded_bytes_a == encoded_bytes_b,
            "double_trace_equal": trace_equal,
            "left_edge_observable": edge_observable,
        }
    )
    classification["rejection_reasons"] = [
        name for name, passed in classification["predicates"].items() if not passed
    ]
    classification["qualifies"] = all(classification["predicates"].values())
    report = {
        "id": candidate["id"],
        "family": candidate["family"],
        "seed": candidate["seed"],
        "input_rgb_sha256": sha256(pixels),
        "encoded_file_sha256": sha256(encoded_bytes_a),
        "encoded_file_sha256_second": sha256(encoded_bytes_b),
        "encoded_item_sha256": sha256(item_a),
        "encoded_item_sha256_second": sha256(item_b),
        "encoded_item_length": len(item_a),
        "entropy_operation_count": entropy_a,
        "entropy_operation_count_second": entropy_b,
        "partition_blocks": blocks_a,
        "partition_blocks_second": blocks_b,
        "portable_color": portable_color,
        "trace_equal": trace_equal,
        "trace_sha256": sha256(trace_a.encode()),
        "trace_sha256_second": sha256(trace_b.encode()),
        **classification,
    }
    if report["qualifies"] and retain_dir is not None:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path.name).write_bytes(encoded_bytes_a)
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
    with tempfile.TemporaryDirectory(prefix="image-star-avif-diagonal67-biased-") as name:
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
            "families": list(FAMILY_NAMES),
            "seed_formula": "700 + 10*family_index + candidate_index",
            "phase_formula": "(11*candidate_index + 7*family_index + 3) mod 32",
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
    print(f"Written {len(reports)} deterministic biased Diagonal67 traces: {args.output}")


if __name__ == "__main__":
    main()
