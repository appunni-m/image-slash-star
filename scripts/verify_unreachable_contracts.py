#!/usr/bin/env python3
"""Verify the bounded v1 map for behavior Pillow cannot prove.

The map is deliberately smaller than the roadmap.  It names existing
fixture-backed integration contracts, or records a category as planned when no
such contract exists.  It is a static audit: it does not execute Rust tests,
collect coverage, or turn a Pillow parity row into evidence for a Rust-only
field.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "tests" / "fixtures" / "unreachable_contract_manifest.json"
DOCUMENT = ROOT / "docs" / "roadmap-new.md"

TOP_LEVEL_KEYS = {"format_version", "scope", "pillow_parity", "categories"}
PARITY_KEYS = {"status", "matrix", "reason"}
CATEGORY_KEYS = {
    "id",
    "label",
    "status",
    "scope",
    "pillow_parity",
    "evidence",
    "planned_reason",
    "planned_context",
}
EVIDENCE_KEYS = {"kind", "path", "fixture", "tests", "command"}
EXPECTED_CATEGORIES = (
    ("decode-encode-policy-limits", "DecodePolicy and EncodePolicy limits"),
    ("cancellation-work-budgets", "Cancellation and work budgets"),
    ("output-sink-delivery", "OutputSink delivery"),
    ("caller-owned-destination-buffers", "Caller-owned destination buffers"),
    ("source-provenance", "Source provenance"),
    ("structured-diagnostics", "Structured diagnostics"),
    ("feature-target-capability", "Feature and target capability"),
    ("cache-concurrency-api-lifecycle", "Cache/concurrency/API lifecycle"),
    ("allocation-stack-coverage-models", "Allocation/stack/coverage models"),
)
STATUSES = {"covered", "planned"}
EVIDENCE_KINDS = {"integration_contract", "fixture_verifier"}
TEST_DECLARATION = re.compile(r"#\[test\]\s*(?:\r?\n\s*)+fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")


def fail(message: str) -> None:
    raise RuntimeError(message)


def read_json() -> dict:
    try:
        value = json.loads(MANIFEST.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {MANIFEST}: {error}")
    if not isinstance(value, dict):
        fail(f"{MANIFEST} must contain a JSON object")
    return value


def repository_file(value: object, field: str) -> Path:
    if not isinstance(value, str) or not value:
        fail(f"{field} must be a non-empty repository-relative path")
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts:
        fail(f"{field} must be a repository-relative path: {value!r}")
    path = ROOT / relative
    if not path.is_file():
        fail(f"{field} does not exist: {value}")
    return relative


def string_array(value: object, field: str, *, allow_empty: bool) -> list[str]:
    if not isinstance(value, list) or (not allow_empty and not value):
        fail(f"{field} must be a non-empty string array")
    if any(not isinstance(item, str) or not item for item in value):
        fail(f"{field} must contain only non-empty strings")
    if len(set(value)) != len(value):
        fail(f"{field} must not contain duplicates")
    return value


def verify_test_symbols(path: Path, names: list[str], field: str) -> None:
    source = (ROOT / path).read_text(encoding="utf-8")
    discovered = set(TEST_DECLARATION.findall(source))
    missing = sorted(set(names) - discovered)
    if missing:
        fail(f"{field} names are not #[test] functions in {path}: {missing}")


def verify_evidence(entry: object, category_id: str, index: int) -> None:
    if not isinstance(entry, dict):
        fail(f"{category_id}: evidence entry {index} must be an object")
    if set(entry) != EVIDENCE_KEYS:
        fail(
            f"{category_id}: evidence entry {index} keys differ: "
            f"expected {sorted(EVIDENCE_KEYS)}, got {sorted(entry)}"
        )

    kind = entry.get("kind")
    if kind not in EVIDENCE_KINDS:
        fail(f"{category_id}: unsupported evidence kind {kind!r}")
    path = repository_file(entry.get("path"), f"{category_id}.evidence[{index}].path")
    if "coverage_matrix_tests" in path.as_posix():
        fail(f"{category_id}: Pillow parity test path cannot be evidence")
    if "pillow_fixture" in json.dumps(entry, sort_keys=True):
        fail(f"{category_id}: Pillow fixture origin cannot be evidence")

    fixture = entry.get("fixture")
    if fixture is not None:
        fixture_path = repository_file(
            fixture, f"{category_id}.evidence[{index}].fixture"
        )
        if fixture_path.as_posix() == "tests/fixtures/coverage_matrix.json":
            fail(f"{category_id}: Pillow parity matrix cannot be evidence")

    tests = string_array(
        entry.get("tests"),
        f"{category_id}.evidence[{index}].tests",
        allow_empty=kind == "fixture_verifier",
    )
    command = string_array(
        entry.get("command"),
        f"{category_id}.evidence[{index}].command",
        allow_empty=False,
    )
    if any(token in {";", "&&", "||", "|"} for token in command):
        fail(f"{category_id}: evidence command must not contain shell syntax")

    if kind == "integration_contract":
        if not path.as_posix().startswith("tests/") or path.suffix != ".rs":
            fail(f"{category_id}: integration evidence must be a Rust test path")
        if path.as_posix() == "tests/coverage_matrix_tests.rs":
            fail(f"{category_id}: coverage matrix is not a Rust-only contract")
        if entry.get("fixture") is not None and not path.as_posix().startswith("tests/"):
            fail(f"{category_id}: integration fixture path is outside tests")
        if command[0] != "cargo" or "test" not in command:
            fail(f"{category_id}: integration evidence must use cargo test")
        verify_test_symbols(path, tests, f"{category_id}.evidence[{index}].tests")
    else:
        if not path.as_posix().startswith("scripts/") or path.suffix != ".py":
            fail(f"{category_id}: fixture verifier must be a repository script")
        if entry.get("fixture") is None:
            fail(f"{category_id}: fixture verifier must name its fixture")
        if tests:
            fail(f"{category_id}: fixture verifier cannot name Rust tests")
        if command != ["python3", path.as_posix()]:
            fail(
                f"{category_id}: fixture verifier command must invoke its script "
                f"directly"
            )


def markdown_table_cells(line: str) -> list[str] | None:
    stripped = line.strip()
    if not stripped.startswith("|") or not stripped.endswith("|"):
        return None
    return [cell.strip() for cell in stripped[1:-1].split("|")]


def markdown_cell_value(cell: str) -> str:
    if len(cell) >= 2 and cell.startswith("`") and cell.endswith("`"):
        return cell[1:-1]
    return cell


def verify_catalog_table(document_text: str, categories: list[dict]) -> None:
    """Keep the human-facing catalog tied to the machine-facing manifest."""

    expected_ids = {category_id for category_id, _ in EXPECTED_CATEGORIES}
    rows: dict[str, list[str]] = {}
    for line in document_text.splitlines():
        cells = markdown_table_cells(line)
        if cells is None or len(cells) != 6:
            continue
        category_id = markdown_cell_value(cells[0])
        if category_id not in expected_ids:
            continue
        if category_id in rows:
            fail(f"{DOCUMENT} contains duplicate catalog table row for `{category_id}`")
        rows[category_id] = cells

    missing = sorted(expected_ids - set(rows))
    if missing:
        fail(f"{DOCUMENT} is missing catalog table rows: {missing}")

    by_id = {category["id"]: category for category in categories}
    labels = dict(EXPECTED_CATEGORIES)
    for category_id in expected_ids:
        category = by_id[category_id]
        cells = rows[category_id]
        if cells[1].replace("`", "") != labels[category_id]:
            fail(
                f"{DOCUMENT} catalog label for `{category_id}` does not match "
                "the manifest"
            )
        if markdown_cell_value(cells[3]) != category["status"]:
            fail(
                f"{DOCUMENT} status for `{category_id}` does not match the "
                "manifest"
            )
        if markdown_cell_value(cells[5]) != category["pillow_parity"]:
            fail(
                f"{DOCUMENT} Pillow-parity status for `{category_id}` does not "
                "match the manifest"
            )

        evidence_cell = cells[4]
        if category["status"] == "covered":
            for evidence in category["evidence"]:
                path = evidence["path"]
                if f"`{path}`" not in evidence_cell:
                    fail(
                        f"{DOCUMENT} evidence cell for `{category_id}` omits "
                        f"manifest path `{path}`"
                    )
        else:
            marker = "No category-specific evidence is claimed"
            if marker not in evidence_cell:
                fail(
                    f"{DOCUMENT} planned evidence cell for `{category_id}` must "
                    f"say `{marker}`"
                )
            for path in category["planned_context"]:
                if f"`{path}`" not in evidence_cell:
                    fail(
                        f"{DOCUMENT} planned context for `{category_id}` omits "
                        f"manifest path `{path}`"
                    )


def verify_document(document_text: str, categories: list[dict]) -> None:
    required = (
        "tests/fixtures/unreachable_contract_manifest.json",
        "python3 scripts/verify_unreachable_contracts.py",
        "Pillow parity",
        "excluded",
    )
    missing = [phrase for phrase in required if phrase not in document_text]
    if missing:
        fail(f"{DOCUMENT} is missing mapping references: {missing}")
    plain_document = document_text.replace("`", "")
    for category_id, label in EXPECTED_CATEGORIES:
        if document_text.count(f"`{category_id}`") != 1:
            fail(f"{DOCUMENT} must contain exactly one mapping row for `{category_id}`")
        if label not in plain_document:
            fail(f"{DOCUMENT} is missing catalog label {label!r}")
    verify_catalog_table(document_text, categories)


def verify() -> tuple[int, int]:
    document = read_json()
    if set(document) != TOP_LEVEL_KEYS:
        fail(
            f"manifest keys differ: expected {sorted(TOP_LEVEL_KEYS)}, "
            f"got {sorted(document)}"
        )
    if document.get("format_version") != 1:
        fail("unreachable contract manifest format_version must be 1")
    if document.get("scope") != "pillow_unreachable_contract_v1":
        fail("unreachable contract manifest scope must be pillow_unreachable_contract_v1")

    parity = document.get("pillow_parity")
    if not isinstance(parity, dict) or set(parity) != PARITY_KEYS:
        fail("pillow_parity must have exactly status, matrix, and reason")
    if parity.get("status") != "excluded":
        fail("pillow_parity status must be excluded")
    if parity.get("matrix") != "tests/fixtures/coverage_matrix.json":
        fail("pillow_parity matrix must identify the canonical Pillow matrix")
    repository_file(parity.get("matrix"), "pillow_parity.matrix")
    if not isinstance(parity.get("reason"), str) or "Pillow" not in parity["reason"]:
        fail("pillow_parity reason must explain the Pillow boundary")

    categories = document.get("categories")
    if not isinstance(categories, list) or len(categories) != len(EXPECTED_CATEGORIES):
        fail(f"categories must contain exactly {len(EXPECTED_CATEGORIES)} entries")
    expected_ids = [category_id for category_id, _ in EXPECTED_CATEGORIES]
    actual_ids = [category.get("id") if isinstance(category, dict) else None for category in categories]
    if actual_ids != expected_ids:
        fail(f"category IDs differ: expected {expected_ids}, got {actual_ids}")

    covered = 0
    for category, (expected_id, expected_label) in zip(categories, EXPECTED_CATEGORIES):
        if not isinstance(category, dict):
            fail(f"{expected_id}: category must be an object")
        if set(category) != CATEGORY_KEYS:
            fail(
                f"{expected_id}: category keys differ: expected {sorted(CATEGORY_KEYS)}, "
                f"got {sorted(category)}"
            )
        if category.get("label") != expected_label:
            fail(f"{expected_id}: label does not match the canonical catalog")
        if category.get("status") not in STATUSES:
            fail(f"{expected_id}: status must be covered or planned")
        if category.get("pillow_parity") != "excluded":
            fail(f"{expected_id}: Pillow parity must be explicitly excluded")
        if not isinstance(category.get("scope"), str) or not category["scope"]:
            fail(f"{expected_id}: scope must be a non-empty string")

        evidence = category.get("evidence")
        if not isinstance(evidence, list):
            fail(f"{expected_id}: evidence must be an array")
        evidence_keys: set[tuple[str, str]] = set()
        for index, entry in enumerate(evidence):
            verify_evidence(entry, expected_id, index)
            evidence_key = (entry["kind"], entry["path"])
            if evidence_key in evidence_keys:
                fail(f"{expected_id}: duplicate evidence path {evidence_key}")
            evidence_keys.add(evidence_key)

        planned_reason = category.get("planned_reason")
        planned_context = category.get("planned_context")
        if not isinstance(planned_context, list):
            fail(f"{expected_id}: planned_context must be an array")
        if category["status"] == "covered":
            covered += 1
            if not evidence:
                fail(f"{expected_id}: covered category must name evidence")
            if planned_reason is not None or planned_context:
                fail(f"{expected_id}: covered category cannot carry planned fields")
        else:
            if evidence:
                fail(f"{expected_id}: planned category cannot claim evidence")
            if not isinstance(planned_reason, str) or not planned_reason:
                fail(f"{expected_id}: planned category needs a reason")
            if not planned_context:
                fail(f"{expected_id}: planned category needs bounded context paths")
            for index, path in enumerate(planned_context):
                repository_file(path, f"{expected_id}.planned_context[{index}]")

    try:
        verify_document(DOCUMENT.read_text(encoding="utf-8"), categories)
    except OSError as error:
        fail(f"cannot read {DOCUMENT}: {error}")
    return len(categories), covered


def main() -> int:
    try:
        total, covered = verify()
    except RuntimeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        f"unreachable contract mapping OK: {covered}/{total} categories have "
        "existing Rust evidence; remaining categories are explicitly planned; "
        "Pillow parity excluded"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
