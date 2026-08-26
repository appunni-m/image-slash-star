#!/usr/bin/env python3
"""Verify the machine-backed status claims in ``roadmap.json``.

``roadmap.json`` is the canonical status and dependency source. The Markdown
roadmap is a human rendering that must mirror its current counts and required
sections. The generated coverage matrix remains the machine source for
fixture status; this check keeps all three representations from drifting.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
ROADMAP_JSON = ROOT / "roadmap.json"
ROADMAP = ROOT / "docs" / "roadmap-new.md"
MATRIX = ROOT / "tests" / "fixtures" / "coverage_matrix.json"
ORIGIN_MANIFEST = ROOT / "tests" / "fixtures" / "coverage_origin_manifest.json"


def fail(message: str) -> None:
    raise RuntimeError(message)


def read_json(path: Path) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {path}: {error}")
    if not isinstance(value, dict):
        fail(f"{path} must contain a JSON object")
    return value


def matrix_rows(document: object) -> list[dict]:
    rows: list[dict] = []

    def visit(value: object) -> None:
        if isinstance(value, dict):
            if {"id", "format", "type", "status"}.issubset(value):
                rows.append(value)
            for child in value.values():
                visit(child)
        elif isinstance(value, list):
            for child in value:
                visit(child)

    visit(document)
    return rows


def exact_match(document: str, pattern: str, label: str) -> None:
    if re.search(pattern, document, flags=re.MULTILINE) is None:
        fail(f"roadmap is missing the current {label} claim")


def verify_avif_safe_rust_cutover() -> None:
    """Reject a roadmap that silently regresses to a native AVIF backend."""

    avif_root = ROOT / "src" / "codecs" / "avif"
    forbidden_paths = (
        ROOT / "build.rs",
        ROOT / "src" / "codecs" / "avif" / "native.rs",
        ROOT / "src" / "codecs" / "avif" / "native",
        ROOT / "src" / "codecs" / "avif" / "native" / "bridge.c",
    )
    for path in forbidden_paths:
        if path.exists():
            fail(f"pure-Rust AVIF cutover has a native path: {path.relative_to(ROOT)}")

    avif_sources = list(avif_root.rglob("*.rs"))
    if any(path.suffix == ".c" for path in avif_root.rglob("*")):
        fail("pure-Rust AVIF source tree contains a C source file")
    for path in avif_sources:
        source = path.read_text(encoding="utf-8")
        if re.search(r"\bunsafe\b|extern\s+\"C\"", source):
            fail(f"AVIF source is not safe Rust: {path.relative_to(ROOT)}")

    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    if re.search(r"(?im)^\s*(?:build|links)\s*=", cargo):
        fail("Cargo.toml still declares a native build or link path")
    if re.search(r"libavif|dav1d|libaom", cargo):
        fail("Cargo.toml still names a native AVIF/AV1 dependency")


def verify() -> str:
    roadmap_data = read_json(ROADMAP_JSON)
    roadmap = ROADMAP.read_text(encoding="utf-8")
    matrix = read_json(MATRIX)
    origin_manifest = read_json(ORIGIN_MANIFEST)
    if roadmap_data.get("schema") != "image-slash-star-roadmap-v1":
        fail("roadmap.json has the wrong schema")
    if roadmap_data.get("source_policy", {}).get("authoritative") != "roadmap.json":
        fail("roadmap.json does not declare itself authoritative")
    if roadmap_data.get("human_rendering") != "docs/roadmap-new.md":
        fail("roadmap.json names an unexpected human rendering")
    if roadmap_data.get("source_policy", {}).get("historical_finding_context") != "docs/roadmap.md":
        fail("roadmap.json names an unexpected historical finding source")
    current = roadmap_data.get("current_state")
    if not isinstance(current, dict):
        fail("roadmap.json has no current_state object")
    matrix_state = current.get("matrix")
    avif_state = current.get("avif")
    if not isinstance(matrix_state, dict) or not isinstance(avif_state, dict):
        fail("roadmap.json current_state is missing matrix or AVIF state")
    avif_document = roadmap_data.get("avif")
    if not isinstance(avif_document, dict):
        fail("roadmap.json has no AVIF ledger")
    planned_decode_document = avif_document.get("planned_decode")
    planned_encode_document = avif_document.get("planned_encode")
    if not isinstance(planned_decode_document, list) or not isinstance(
        planned_encode_document, list
    ):
        fail("roadmap.json AVIF ledger must contain planned decode and encode arrays")
    expected_planned = {
        row["id"]: row["work_item"]
        for row in planned_decode_document
        if isinstance(row, dict) and isinstance(row.get("id"), str)
    }
    expected_encode = {
        row["id"]
        for row in planned_encode_document
        if isinstance(row, dict) and isinstance(row.get("id"), str)
    }
    if len(expected_planned) != len(planned_decode_document):
        fail("roadmap.json contains duplicate planned AVIF decode IDs")
    if len(expected_encode) != len(planned_encode_document):
        fail("roadmap.json contains duplicate planned AVIF encode IDs")
    rows = matrix_rows(matrix)
    if len(rows) != len({(row["format"], row["type"], row["id"]) for row in rows}):
        fail("coverage matrix contains duplicate format/type/id rows")

    summary = matrix.get("summary")
    if not isinstance(summary, dict):
        fail("coverage matrix has no summary object")
    expected_summary = {
        "total_rows": matrix_state.get("total_rows"),
        "decode_rows": matrix_state.get("decode_rows"),
        "encode_rows": matrix_state.get("encode_rows"),
        "decode_active": matrix_state.get("decode_active"),
        "decode_planned": matrix_state.get("decode_planned"),
        "encode_not_wired": matrix_state.get("encode_planned"),
    }
    if any(not isinstance(value, int) for value in expected_summary.values()):
        fail("roadmap.json matrix counts must be integers")
    for key, expected in expected_summary.items():
        if summary.get(key) != expected:
            fail(f"coverage matrix summary {key} is {summary.get(key)!r}, expected {expected}")
    if len(rows) != expected_summary["total_rows"]:
        fail(f"coverage matrix has {len(rows)} rows, expected {expected_summary['total_rows']}")

    avif_decode = [
        row for row in rows if row["format"] == "avif" and row["type"] == "decode"
    ]
    avif_encode = [
        row for row in rows if row["format"] == "avif" and row["type"] == "encode"
    ]
    if len(avif_decode) != avif_state.get("decode_rows") or len(avif_encode) != avif_state.get("encode_rows"):
        fail(f"AVIF row counts drifted: decode={len(avif_decode)}, encode={len(avif_encode)}")

    planned = {
        row["id"]: row
        for row in avif_decode + avif_encode
        if row["status"] == "planned"
    }
    if set(planned) != set(expected_planned) | expected_encode:
        fail("planned AVIF row identity differs from the explicit roadmap ledger")
    for row_id, work_item in expected_planned.items():
        row = planned[row_id]
        if row.get("pure_rust_work_item") != work_item:
            fail(f"planned AVIF row {row_id} has the wrong pure-Rust work item")
        if row.get("former_native_only") is not True:
            fail(f"planned AVIF row {row_id} lacks former-native-only provenance")
        if not isinstance(row.get("gap"), str) or not row["gap"].strip():
            fail(f"planned AVIF row {row_id} has no concrete gap reason")
        if any(row.get(key) is not None for key in ("ref_path", "ref_bytes", "ref_sha256")):
            fail(f"planned AVIF row {row_id} claims pixel or byte evidence")
    if any(row.get("pure_rust_work_item") != "AVF-ENCODE-001" for row in avif_encode):
        fail("planned AVIF encoder rows must all map to AVF-ENCODE-001")
    if any(row.get("former_native_only") is not True for row in avif_encode):
        fail("planned AVIF encoder rows must identify former native-only provenance")
    if any(row.get("former_native_only") for row in rows if row["status"] != "planned"):
        fail("active rows must not retain former-native-only planned provenance")

    open_inventory = roadmap_data.get("open_inventory")
    if not isinstance(open_inventory, dict) or not isinstance(open_inventory.get("groups"), dict):
        fail("roadmap.json has no grouped open-task inventory")
    open_ids = [
        task_id
        for group_ids in open_inventory["groups"].values()
        if isinstance(group_ids, list)
        for task_id in group_ids
    ]
    if len(open_ids) != open_inventory.get("count") or len(open_ids) != len(set(open_ids)):
        fail("roadmap.json open-task count or IDs are inconsistent")
    finding_details = roadmap_data.get("finding_details")
    if not isinstance(finding_details, list):
        fail("roadmap.json has no per-finding detail catalog")
    detail_ids = [
        detail.get("id")
        for detail in finding_details
        if isinstance(detail, dict)
    ]
    if len(detail_ids) != len(finding_details) or len(detail_ids) != len(set(detail_ids)):
        fail("roadmap.json finding detail IDs are missing or duplicated")
    detail_by_id = {detail["id"]: detail for detail in finding_details}
    open_id_set = set(open_ids)
    if not open_id_set.issubset(detail_by_id):
        fail("roadmap.json is missing detail records for active finding IDs")
    for task_id in open_id_set:
        detail = detail_by_id[task_id]
        if detail.get("status") != "open":
            fail(f"roadmap.json finding detail {task_id} is not marked open")
        if not isinstance(detail.get("finding"), str) or not detail["finding"].strip():
            fail(f"roadmap.json finding detail {task_id} has no caller finding")
        if not isinstance(detail.get("next"), str) or not detail["next"].strip():
            fail(f"roadmap.json finding detail {task_id} has no next action")

    exact_match(
        roadmap,
        rf"AVIF decode/inspect/verify: {avif_state['decode_rows']} rows total, "
        rf"{avif_state['decode_active']} active, {avif_state['decode_planned']} explicit planned",
        "AVIF decode count",
    )
    exact_match(
        roadmap,
        rf"AVIF encode: {avif_state['encode_rows']} rows total, all "
        rf"{avif_state['encode_planned']} explicit planned",
        "AVIF encode count",
    )
    exact_match(
        roadmap,
        rf"Whole matrix: {matrix_state['total_rows']:,} rows total, "
        rf"{matrix_state['decode_active']} active decode rows, {matrix_state['encode_active']} active encode\s+rows, "
        rf"{matrix_state['decode_planned']} planned decode rows, and {matrix_state['encode_planned']} planned encode rows",
        "whole-matrix count",
    )
    exact_match(
        roadmap,
        rf"contains\s+\*\*{roadmap_data['open_inventory']['count']} active\s+finding rows\*\*",
        "open-task count",
    )
    exact_match(
        roadmap,
        r"\| Documentation \| 5 \| `DOC-003`, `DOC-005`–`DOC-008` \|",
        "documentation open-task count",
    )
    inventory = roadmap.split("## Complete open-task inventory", 1)[1].split(
        "## Parked, not pending", 1
    )[0]
    if re.search(r"\bDOC-00[24]\b", inventory):
        fail("resolved documentation work is still listed as an active task")
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    exact_match(
        readme,
        rf"contains {matrix_state['total_rows']:,} total rows: {matrix_state['decode_rows']:,} decode /\s+inspect / verify rows and {matrix_state['encode_rows']:,} encode rows\. Of those, {matrix_state['decode_active']:,} decode rows",
        "README matrix count",
    )
    current_section = roadmap.split("## Current pure-Rust AVIF cutover", 1)[1].split(
        "## Latest API-038 implementation candidate", 1
    )[0]
    test_counts = current.get("tests")
    if not isinstance(test_counts, dict):
        fail("roadmap.json current_state has no test counts")
    exact_match(
        current_section,
        re.escape(
            f"{test_counts.get('coverage_matrix_tests')} and "
            f"{test_counts.get('feature_gate_tests')}"
        ).replace(r"\ ", r"\s+"),
        "current Rust contract test count",
    )
    coverage = current.get("coverage")
    if not isinstance(coverage, dict):
        fail("roadmap.json current_state has no coverage object")
    for metric in ("line", "branch", "function", "region"):
        if not isinstance(coverage.get(metric), dict):
            fail(f"roadmap.json coverage is missing {metric} metrics")
    origin_state = coverage.get("coverage_origin_inventory")
    origin_entries = origin_manifest.get("entries")
    if not isinstance(origin_state, dict) or not isinstance(origin_entries, list):
        fail("roadmap.json or coverage-origin manifest is missing origin inventory")
    actual_origin_guards = sum(
        entry.get("guard_count", 0)
        for entry in origin_entries
        if isinstance(entry, dict)
    )
    if (
        actual_origin_guards != origin_state.get("guards")
        or len(origin_entries) != origin_state.get("files")
    ):
        fail("roadmap.json coverage-origin inventory disagrees with its manifest")
    coverage_pattern = (
        rf"{coverage['line']['covered']:,}/{coverage['line']['total']:,} lines\s+"
        rf"\({coverage['line']['percent']:.4f}%\),\s+"
        rf"{coverage['branch']['covered']:,}/{coverage['branch']['total']:,} branches\s+"
        rf"\({coverage['branch']['percent']:.4f}%\),\s+"
        rf"{coverage['function']['covered']:,}/{coverage['function']['total']:,} functions\s+"
        rf"\({coverage['function']['percent']:.4f}%\),\s+and\s+"
        rf"{coverage['region']['covered']:,}/{coverage['region']['total']:,} regions\s+"
        rf"\({coverage['region']['percent']:.4f}%\)"
    )
    exact_match(current_section, coverage_pattern, "current LLVM coverage result")
    exact_match(
        roadmap,
        rf"origin verifier passes for {origin_state['guards']} exact `cfg\(coverage\)` guards across {origin_state['files']} files",
        "coverage-origin count",
    )
    if re.search(r"native runtime\s+fallback", current_section) is None:
        fail("current AVIF section does not state the native-fallback prohibition")
    if "28/28" in current_section:
        fail("current AVIF section contains a stale 28/28 test count")
    exact_match(
        current_section,
        r"eleven Rust tests prove[\s\S]*complete-canvas enforcement",
        "atomic raster implementation evidence",
    )
    exact_match(
        current_section,
        r"8×16[\s\S]*block coordinates `\(6, 0\)`[\s\S]*rectangular\s+transform",
        "exact next AVIF rectangular-transform gap",
    )
    if "place_cells" not in current_section:
        fail("current AVIF section does not name the atomic cell-batch boundary")
    verify_avif_safe_rust_cutover()
    for heading in (
        "## Current pure-Rust AVIF cutover",
        "## AVIF planned-gap ledger (current tree)",
        "### Former native-only cases: explicit Rust work, never hidden fallback",
        "## Complete open-task inventory",
        "## Required acceptance commands",
    ):
        if heading not in roadmap:
            fail(f"roadmap is missing required section: {heading}")
    for work_item in sorted(set(expected_planned.values()) | {"AVF-ENCODE-001"}):
        if work_item not in roadmap:
            fail(f"roadmap is missing planned work item {work_item}")
    if "former_native_only" not in roadmap:
        fail("roadmap does not name the machine-readable former_native_only contract")
    if "zero aggregate coverage gaps" in roadmap:
        fail("roadmap still claims zero aggregate coverage gaps")

    return (
        f"roadmap status OK: {len(rows)} matrix rows, {len(expected_planned)} "
        f"planned AVIF decode gaps, and {len(expected_encode)} planned AVIF encoder rows"
    )


def main() -> int:
    try:
        print(verify())
    except RuntimeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
