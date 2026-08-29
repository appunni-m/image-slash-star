#!/usr/bin/env python3
"""Search a bounded oracle corpus for an AV1 R16x8 H_DCT luma witness.

This campaign reuses the deterministic 16x8 corpus and independent scalar
dav1d tracing from :mod:`explore_avif_horizontal16x8_identity`, but targets
the R16x8 H_DCT syntax pair specifically: transform CDF symbol 3 and dav1d
``txtp=11``.  A candidate also has to expose exactly one 128-value luma
dequantized coefficient dump with a non-zero value after DC.  That makes the
non-empty AC requirement a coefficient-level fact rather than an inference
from EOB.

The campaign encodes and decodes every candidate twice, records every
rejection, and never invokes repository Rust code.  Generated AVIF files are
temporary unless ``--retain-dir`` is supplied.
"""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path

from PIL import Image, _avif, features

import explore_avif_horizontal16x8_identity as identity


TARGET = {
    "h_dct": {"cdf_symbol": 3, "txtp": 11, "name": "h_dct16x8"}
}
BASE_CLASSIFY = identity.classify


def parse_dq_matrices(group: list[str]) -> list[list[int]]:
    """Extract each dequantized coefficient dump from one dav1d leaf."""

    matrices: list[list[int]] = []
    for index, line in enumerate(group):
        if line != "dq":
            continue
        matrix: list[int] = []
        for row in group[index + 1 :]:
            values = row.split()
            if not values:
                break
            try:
                matrix.extend(int(value) for value in values)
            except ValueError:
                break
        if matrix:
            matrices.append(matrix)
    return matrices


def classify_h_dct(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
) -> dict[str, object]:
    """Add coefficient-level H_DCT and skipped-chroma predicates."""

    result = BASE_CLASSIFY(blocks, groups, yuv, portable_color)
    parsed = [identity.parse_group(group) for group in groups]
    matrices = [matrix for group in groups for matrix in parse_dq_matrices(group)]
    luma_matrix = matrices[0] if len(matrices) == 1 else []
    nonzero_ac = [
        {"index": index, "value": value}
        for index, value in enumerate(luma_matrix)
        if index > 0 and value != 0
    ]
    chroma_payloads = [
        payload
        for group in parsed
        for payload in group["chroma_payloads"]
    ]
    predicates = result["target_predicates"]["h_dct"]
    predicates.update(
        {
            "two_skipped_tx8x4_dc_chroma_payloads": (
                len(chroma_payloads) == 2
                and {payload["plane"] for payload in chroma_payloads} == {0, 1}
                and all(
                    payload["tx"] == 6
                    and payload["txtp"] == 0
                    and payload["eob"] == -1
                    for payload in chroma_payloads
                )
            ),
            "one_r16x8_luma_dq_matrix": (
                len(matrices) == 1 and len(luma_matrix) == 128
            ),
            "luma_dequantized_ac": bool(nonzero_ac),
        }
    )
    predicates["qualifies"] = all(
        value for name, value in predicates.items() if name != "qualifies"
    )
    result["dq_matrices"] = matrices
    result["luma_dq_matrix"] = luma_matrix
    result["luma_dq_nonzero_ac"] = nonzero_ac
    result["target_predicates"]["h_dct"] = predicates
    result["qualifies"] = predicates["qualifies"] and not result["rejection_reasons"]
    result["qualified_targets"] = ["h_dct"] if result["qualifies"] else []
    return result


def main() -> None:
    """Run the pinned 100-case input-only H_DCT campaign."""

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

    identity.TARGETS = TARGET
    identity.classify = classify_h_dct
    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")

    with tempfile.TemporaryDirectory(prefix="image-star-avif-h16x8-h-dct-") as name:
        work = Path(name)
        if args.dav1d_source is not None:
            source = args.dav1d_source.resolve()
            identity.verify_source(source)
            executable, environment = identity.build_dav1d(
                source,
                work,
                identity.resolve_tool(args.meson, "Meson"),
                identity.resolve_tool(args.ninja, "Ninja"),
                args.python_path.resolve() if args.python_path else None,
            )
        else:
            executable = args.dav1d.resolve()
            environment = {}
        version_result = identity.run([str(executable), "--version"], env=environment)
        version = (version_result.stdout + version_result.stderr).strip()
        if not version.startswith("1.5.3-0-gb546257"):
            raise RuntimeError(f"unexpected dav1d executable version: {version}")
        reports = [
            identity.decode_candidate(
                executable, environment, work, candidate, args.retain_dir
            )
            for candidate in identity.candidates()
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
    target_predicate_names = tuple(
        name for name in reports[0]["target_predicates"]["h_dct"] if name != "qualifies"
    )
    report = {
        "format_version": 1,
        "oracle": {
            "pillow": Image.__version__,
            "libavif": features.version("avif"),
            "codecs": codecs,
            "dav1d": version,
            "dav1d_commit": identity.DAV1D_COMMIT,
        },
        "encoding": {
            "size": list(identity.SIZE),
            "subsampling": identity.SUBSAMPLING,
            "quality_by_candidate_index": list(identity.QUALITIES),
            "amplitude_by_candidate_index": list(identity.AMPLITUDES),
            "speed": 0,
            "max_threads": 1,
            "autotiling": False,
            "advanced": identity.ADVANCED,
        },
        "search": {
            "input_only": True,
            "repository_rust_invoked": False,
            "candidate_count": len(reports),
            "family_count": len(identity.FAMILY_NAMES),
            "candidates_per_family": 10,
            "target_id": "origin_horizontal16x8_h_dct",
            "target": (
                "16x8 4:2:0 origin Horizontal16x8 leaf with one unsplit TX16x8 "
                "luma payload; H_DCT requires CDF symbol 3/dav1d txtp 11, "
                "two skipped TX8x4 DC chroma payloads, and a direct nonzero "
                "AC coefficient in the 128-value luma dequantized dump"
            ),
            "targets": TARGET,
            "families": list(identity.FAMILY_NAMES),
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "qualified_by_target": {
                "h_dct": sum("h_dct" in report["qualified_targets"] for report in reports)
            },
            "qualified_candidates": [
                report["id"] for report in reports if report["qualifies"]
            ],
            "promotions": {
                "h_dct": next(
                    (
                        report["id"]
                        for report in sorted(
                            reports,
                            key=lambda item: (
                                int(item["entropy_operation_count"]),
                                int(item["encoded_item_length"]),
                                str(item["id"]),
                            ),
                        )
                        if "h_dct" in report["qualified_targets"]
                    ),
                    None,
                )
            },
            "by_common_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in common_reasons
            },
            "by_target_rejection_reason": {
                name: sum(
                    not report["target_predicates"]["h_dct"].get(name, False)
                    for report in reports
                )
                for name in target_predicate_names
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic Horizontal16x8 H_DCT traces: {args.output}")


if __name__ == "__main__":
    main()
