#!/usr/bin/env python3
"""Verify that Rust diagnostic evidence stays separate from Pillow parity.

The diagnostic manifest records Rust-only recovery fields.  Some cases reuse an
unchanged asset that also has a Pillow parity row; the remaining cases build a
runtime mutation from a parity baseline.  Neither category makes Pillow a
source of the diagnostic kind, stage, offset, or identity.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DIAGNOSTIC_MANIFEST = ROOT / "tests/fixtures/diagnostic_manifest.json"
COVERAGE_MATRIX = ROOT / "tests/fixtures/coverage_matrix.json"
MAINTAINED_DOCS = (ROOT / "docs/testing.md", ROOT / "docs/roadmap.md")
FORBIDDEN_MATRIX_FIELDS = {
    "diagnostic",
    "diagnostics",
    "diagnostic_identity",
    "diagnostic_kind",
    "diagnostic_offset",
    "diagnostic_stage",
    "mutation",
}


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


def matrix_rows(document: dict) -> list[dict]:
    formats = document.get("formats")
    if not isinstance(formats, dict) or not formats:
        fail("coverage matrix formats must be a non-empty object")
    rows: list[dict] = []
    for format_name, format_data in formats.items():
        if not isinstance(format_data, dict):
            fail(f"coverage matrix format {format_name!r} must be an object")
        decode = format_data.get("decode")
        if not isinstance(decode, list):
            fail(f"coverage matrix format {format_name!r} decode rows must be an array")
        for row in decode:
            if not isinstance(row, dict):
                fail(f"coverage matrix format {format_name!r} has a non-object decode row")
            rows.append(row)
    return rows


def has_active_parity_row(rows: list[dict], case: dict) -> bool:
    asset_path = case.get("asset_path")
    operation = case.get("operation")
    if not isinstance(asset_path, str) or not isinstance(operation, str):
        return False
    asset_name = Path(asset_path).name
    return any(
        row.get("status") == "active"
        and row.get("format") == case.get("format")
        and row.get("asset") == asset_name
        and isinstance(row.get("operations"), dict)
        and operation in row["operations"]
        for row in rows
    )


def verify() -> tuple[int, int, int]:
    diagnostic = read_json(DIAGNOSTIC_MANIFEST)
    matrix = read_json(COVERAGE_MATRIX)

    if diagnostic.get("format_version") != 1:
        fail("diagnostic manifest format_version must be 1")
    if diagnostic.get("assertion_origin") != "defensive_model":
        fail("diagnostic manifest must be defensive_model evidence")
    if not isinstance(diagnostic.get("pillow_version"), str):
        fail("diagnostic manifest must record the Pillow version used as supporting evidence")

    cases = diagnostic.get("cases")
    if not isinstance(cases, list) or not cases:
        fail("diagnostic manifest cases must be a non-empty array")
    rows = matrix_rows(matrix)

    matrix_fields = set().union(*(row.keys() for row in rows))
    forbidden = sorted(matrix_fields & FORBIDDEN_MATRIX_FIELDS)
    if forbidden:
        fail(f"Pillow parity matrix contains Rust-only diagnostic fields: {forbidden}")

    unmodified = 0
    mutations = 0
    seen_ids: set[str] = set()
    for case in cases:
        if not isinstance(case, dict):
            fail("diagnostic manifest cases must be objects")
        case_id = case.get("id")
        if not isinstance(case_id, str) or not case_id or case_id in seen_ids:
            fail(f"diagnostic case id is missing or duplicated: {case_id!r}")
        seen_ids.add(case_id)
        if case.get("pillow_outcome") != "ok":
            fail(f"{case_id}: non-fatal diagnostic cases must have Pillow outcome ok")
        if case.get("operation") not in {"decode", "decode_sequence"}:
            fail(f"{case_id}: diagnostic operation is not a decode operation")
        mutation = case.get("mutation")
        if mutation == "none":
            unmodified += 1
            if not has_active_parity_row(rows, case):
                fail(f"{case_id}: unchanged diagnostic asset has no active parity row")
        elif isinstance(mutation, str) and mutation:
            mutations += 1
        else:
            fail(f"{case_id}: mutation must be `none` or a named runtime mutation")

    total = len(cases)
    for doc in MAINTAINED_DOCS:
        try:
            text = doc.read_text(encoding="utf-8")
        except OSError as error:
            fail(f"cannot read {doc}: {error}")
        normalized = " ".join(text.split())
        required = (
            f"{total} diagnostic cases",
            f"{unmodified} use committed bytes that also have a Pillow parity row",
            f"{mutations} cases construct runtime mutations",
        )
        missing = [phrase for phrase in required if phrase not in normalized]
        if missing:
            fail(f"{doc} has stale diagnostic provenance counts: {missing}")

    if total != unmodified + mutations:
        fail("diagnostic provenance counts do not add up")
    return total, unmodified, mutations


def main() -> int:
    try:
        total, unmodified, mutations = verify()
    except RuntimeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        "diagnostic provenance OK: "
        f"{total} defensive-model cases ({unmodified} unchanged parity baselines, "
        f"{mutations} runtime mutations); parity matrix has no diagnostic fields"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
