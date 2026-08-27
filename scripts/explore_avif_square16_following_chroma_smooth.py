#!/usr/bin/env python3
"""Search bounded following-Square16 4:2:0 chroma Smooth inputs.

The campaign generates exactly one hundred deterministic 32x16 RGB candidates
in ten named families. Each candidate is encoded twice with the pinned
Pillow/libavif/libaom oracle and traced twice through scalar dav1d. A case is
qualifying only when the clipped Square16 topology, predictor modes, transform
types, residuals, YUV planes, and Pillow RGB output satisfy the fixed contract.
Repository Rust is never invoked while searching.

This campaign is deliberately separate from the SmoothVertical and
SmoothHorizontal campaigns. Its right-hand chroma field follows the bilinear
shape of the AV1 Smooth predictor, while bounded perturbations preserve
meaningful AC in both chroma planes.
"""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path

from PIL import Image, _avif, features

import explore_avif_square16_following_chroma as horizontal
from generate_av1_reconstruction_refs import (
    DAV1D_COMMIT,
    build_dav1d,
    resolve_tool,
    run,
    verify_source,
)


SIZE = horizontal.SIZE
SUBSAMPLING = horizontal.SUBSAMPLING
QUALITY = horizontal.QUALITY
SPEED = horizontal.SPEED
ADVANCED = horizontal.ADVANCED
FAMILY_NAMES = (
    "positive_bilinear_ramp",
    "negative_bilinear_ramp",
    "two_level_bilinear_step",
    "four_level_bilinear_staircase",
    "bilinear_saw",
    "low_frequency_bilinear_ripple",
    "alternating_bilinear_bands",
    "opposed_uv_bilinear_rows",
    "edge_biased_bilinear_rows",
    "asymmetric_bilinear_texture",
)
SMOOTH_WEIGHTS_8 = (255, 197, 146, 105, 73, 50, 37, 32)


def clamp(value: int) -> int:
    """Clamp one generated component to the eight-bit range."""

    return max(0, min(255, value))


def yuv_to_rgb(y: int, u: int, v: int) -> tuple[int, int, int]:
    """Convert a bounded full-range synthetic YUV sample to RGB."""

    du = u - 128
    dv = v - 128
    return (
        clamp(y + (358 * dv + 128) // 256),
        clamp(y - (88 * du + 183 * dv + 128) // 256),
        clamp(y + (453 * du + 128) // 256),
    )


def row_signal(family: int, index: int, row: int) -> int:
    """Return a deterministic chroma edge signal for one family."""

    phase = (3 * family + index) % 8
    if family == 0:
        return (row - 3) * (8 + index % 4)
    if family == 1:
        return -(row - 3) * (8 + index % 4)
    if family == 2:
        return 28 if row < 4 else -28
    if family == 3:
        return (row // 2 - 2) * 13
    if family == 4:
        return ((5 * row + phase) % 9 - 4) * (7 + index % 3)
    if family == 5:
        return (((row * 7 + phase) % 16) - 8) * (3 + index % 3)
    if family == 6:
        return (22 if (row + phase) % 2 == 0 else -22) + (index % 3 - 1) * 3
    if family == 7:
        return (row - 3) * (6 + index % 3)
    if family == 8:
        return (3 - abs(row - (2 + index % 4))) * (8 + index % 3)
    return (((11 * row + 5 * index + phase) % 17) - 8) * 3


def smooth_base(family: int, index: int, cx: int, cy: int) -> int:
    """Return the mode-9 bilinear prediction in the following leaf."""

    top = row_signal(family, index, 0)
    bottom = row_signal(family, index, 7)
    left = row_signal(family, index, cy)
    right = top
    vertical_weight = SMOOTH_WEIGHTS_8[cy]
    horizontal_weight = SMOOTH_WEIGHTS_8[cx - 8]
    return (
        vertical_weight * top
        + (256 - vertical_weight) * bottom
        + horizontal_weight * left
        + (256 - horizontal_weight) * right
        + 256
    ) >> 9


def chroma_sample(family: int, index: int, cx: int, cy: int) -> tuple[int, int]:
    """Return U/V deltas shaped for the following Smooth predictor."""

    edge = row_signal(family, index, cy)
    if cx < 8:
        # Keep the origin right edge equal to the modeled neighboring edge;
        # only the interior gets a small horizontal continuation.
        horizontal_delta = (cx - 7) * (1 + family % 3)
        u = edge + horizontal_delta
        v = edge - horizontal_delta if family in (1, 4, 7) else edge + horizontal_delta // 2
        return u, v

    perturb = ((3 * cx + 5 * cy + index + family) % 5) - 2
    if family in (2, 6):
        perturb *= 2
    base = smooth_base(family, index, cx, cy)
    if family in (1, 4, 7):
        return base + perturb, base - perturb
    if family in (5, 9):
        return base + 2 * perturb, base + perturb
    return base + perturb, base + perturb // 2


def candidate_pixels(family: int, index: int) -> bytes:
    """Create one deterministic RGB candidate from a synthetic YUV field."""

    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            cx, cy = x // 2, y // 2
            u_delta, v_delta = chroma_sample(family, index, cx, cy)
            # The two textured families retain the neighboring campaigns'
            # small luma residual. Without it libaom chooses a split/skipped
            # luma transform, so the chroma witness would no longer isolate
            # the intended following-Square16 class.
            luma = 128
            if family in (5, 9):
                luma += ((7 * x + 11 * y + index + family) % 7) - 3
            pixels.extend(yuv_to_rgb(luma, 128 + u_delta, 128 + v_delta))
    return bytes(pixels)


def candidates() -> list[dict[str, object]]:
    """Return exactly ten deterministic families with ten cases each."""

    result = []
    for family, family_name in enumerate(FAMILY_NAMES):
        for index in range(10):
            result.append(
                {
                    "id": f"SS16-F{family + 1:02d}-N{index:02d}",
                    "family": family_name,
                    "family_index": family,
                    "candidate_index": index,
                    "seed": 10000 + 10 * family + index,
                    "pixels": candidate_pixels(family, index),
                    "quality": QUALITY,
                    "speed": SPEED,
                }
            )
    if len(result) != 100:
        raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
    return result


def classify(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
) -> dict[str, object]:
    """Apply exact predicates for the following Smooth class."""

    parsed = [horizontal.parse_group(group) for group in groups]
    shape = [
        (
            block["poc"],
            block["x"],
            block["y"],
            block["level"],
            block["context"],
            block["partition"],
        )
        for block in blocks
    ]
    y_modes = [mode for group in parsed for mode in group["y_modes"]]
    uv_modes = [mode for group in parsed for mode in group["uv_modes"]]
    y_payloads = [group["luma_payloads"] for group in parsed]
    chroma_payloads = [group["chroma_payloads"] for group in parsed]
    all_lines = [line for group in groups for line in group]
    u_edge = horizontal.plane_edge(yuv, 1) if len(yuv) == 768 else []
    v_edge = horizontal.plane_edge(yuv, 2) if len(yuv) == 768 else []

    def is_tx16x16(payloads: list[dict[str, int]]) -> bool:
        return (
            len(payloads) == 1
            and payloads[0]["tx"] == 2
            and payloads[0]["txtp"] == 0
            and payloads[0]["eob"] >= 1
        )

    def is_chroma_tx8x8(
        payloads: list[dict[str, int]], transform: int, cbx4: int
    ) -> bool:
        return (
            len(payloads) == 2
            and {payload["plane"] for payload in payloads} == {0, 1}
            and all(
                payload["tx"] == 1
                and payload["txtp"] == transform
                and payload["eob"] >= 1
                and payload["cbx4"] == cbx4
                for payload in payloads
            )
        )

    forbidden_prefixes = (
        "Post-filterintramode[",
        "Post-y_pal[",
        "Post-pal[",
        "Post-y-pal-indices",
        "y-pal-pred",
        "Post-uv_pal[",
        "Post-uv-pal-indices",
        "uv-pal-pred",
    )
    predicates = {
        "exact_clipped_split_shape": shape == [
            (0, 0, 0, 2, 0, 3),
            (0, 0, 0, 3, 0, 0),
            (0, 4, 0, 3, 0, 0),
        ],
        "eight_bit_420_frame": (
            portable_color.get("width") == SIZE[0]
            and portable_color.get("height") == SIZE[1]
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "two_visible_square16_groups": len(groups) == 2,
        "both_luma_modes_dc": y_modes == [0, 0],
        "following_uv_mode_smooth": uv_modes == [0, 9],
        "no_angle_symbols": not any(
            line.startswith("Post-")
            and ("angle-symbol" in line or "angle_delta" in line)
            for line in all_lines
        ),
        "no_palette_or_filter_intra": not any(
            line.startswith(forbidden_prefixes) for line in all_lines
        ),
        "both_luma_are_unsplit_tx16x16": (
            len(y_payloads) == 2 and all(is_tx16x16(payloads) for payloads in y_payloads)
        ),
        "left_chroma_is_dct_dct_tx8x8": (
            len(chroma_payloads) == 2 and is_chroma_tx8x8(chroma_payloads[0], 0, 0)
        ),
        "right_chroma_is_adst_adst_tx8x8": (
            len(chroma_payloads) == 2 and is_chroma_tx8x8(chroma_payloads[1], 3, 2)
        ),
        "right_chroma_has_ac": (
            len(chroma_payloads) == 2
            and all(payload["eob"] >= 1 for payload in chroma_payloads[1])
        ),
        "decoded_yuv_has_expected_size": len(yuv) == 768,
        "origin_u_edge_varies": len(u_edge) == 8 and len(set(u_edge)) > 1,
        "origin_v_edge_varies": len(v_edge) == 8 and len(set(v_edge)) > 1,
        "origin_chroma_edge_is_not_midpoint": (
            bool(u_edge) and bool(v_edge) and (u_edge[0] != 128 or v_edge[0] != 128)
        ),
    }
    return {
        "target": "following_square16_chroma_smooth",
        "root_partition": blocks[0] if blocks else None,
        "partition_shape": shape,
        "group_count": len(groups),
        "y_modes": y_modes,
        "uv_modes": uv_modes,
        "origin_u_right_edge": u_edge,
        "origin_v_right_edge": v_edge,
        "left_luma_payloads": y_payloads[0] if y_payloads else [],
        "right_luma_payloads": y_payloads[1] if len(y_payloads) == 2 else [],
        "left_chroma_payloads": chroma_payloads[0] if chroma_payloads else [],
        "right_chroma_payloads": chroma_payloads[1] if len(chroma_payloads) == 2 else [],
        "predicates": predicates,
        "rejection_reasons": [name for name, passed in predicates.items() if not passed],
        "qualifies": all(predicates.values()),
    }


def main() -> None:
    """Run the deterministic oracle campaign and write its JSON report."""

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

    # decode_candidate resolves encode/classify from the shared module.
    horizontal.classify = classify
    with tempfile.TemporaryDirectory(prefix="image-star-avif-square16-following-smooth-") as name:
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
                broaden_vertical_following=False,
                broaden_horizontal_square16=True,
            )
        else:
            executable = args.dav1d.resolve()
            environment = {}
        version_result = run([str(executable), "--version"], env=environment)
        version = (version_result.stdout + version_result.stderr).strip()
        if not version.startswith("1.5.3-0-gb546257"):
            raise RuntimeError(f"unexpected dav1d executable version: {version}")
        reports = [
            horizontal.decode_candidate(
                executable,
                environment,
                work,
                candidate,
                args.retain_dir,
            )
            for candidate in candidates()
        ]

    rejection_reasons = list(reports[0]["predicates"])
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
            "max_threads": 1,
            "autotiling": False,
            "advanced": ADVANCED,
        },
        "search": {
            "candidate_count": len(reports),
            "seed_formula": "10000 + 10*family_index + candidate_index",
            "target_id": "following_square16_chroma_smooth",
            "target": (
                "32x16 8-bit 4:2:0 clipped root split with origin/following "
                "Square16 leaves; following UV mode 9 Smooth, ADST-ADST "
                "TX8x8 U/V, non-empty AC"
            ),
            "families": list(FAMILY_NAMES),
            "repository_rust_invoked": False,
        },
        "counts": {
            "qualified": sum(bool(item["qualifies"]) for item in reports),
            "by_rejection_reason": {
                reason: sum(reason in item["rejection_reasons"] for item in reports)
                for reason in rejection_reasons
            },
        },
        "qualified_candidates": [item["id"] for item in reports if item["qualifies"]],
        "promoted_candidate": next(
            (item["id"] for item in reports if item["qualifies"]), None
        ),
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic following-Square16 traces: {args.output}")


if __name__ == "__main__":
    main()
