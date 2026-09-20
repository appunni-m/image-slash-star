#!/usr/bin/env python3
"""Verify historical measurement provenance and current fixture integrity.

Measured input hashes are checked against the measured Git revision. Separate
working-tree hashes detect current fixture drift; they never extend historical
coverage to new source or inputs.
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LEDGER_PATH = ROOT / "tests" / "fixtures" / "claim_ledger.json"

HASHED_FILES = {
    "manifest_sha256": "manifest.yaml",
    "matrix_sha256": "tests/fixtures/coverage_matrix.json",
    "option_acceptance": "tests/fixtures/encode_option_acceptance_manifest.json",
    "option_error": "tests/fixtures/encode_option_error_manifest.json",
    "decode_policy": "tests/fixtures/decode_policy_manifest.json",
    "sequence_policy": "tests/fixtures/sequence_policy_manifest.json",
    "trailing_input": "tests/fixtures/trailing_input_manifest.json",
    "metadata_policy": "tests/fixtures/metadata_policy_manifest.json",
    "malformed_ledger": "tests/fixtures/malformed_ledger.json",
    "capability_tables": "tests/fixtures/capability_tables.json",
    "incremental_input": "tests/fixtures/incremental_input_manifest.json",
    "diagnostic": "tests/fixtures/diagnostic_manifest.json",
    "coverage_origins": "tests/fixtures/coverage_origin_manifest.json",
    "unreachable_contract": "tests/fixtures/unreachable_contract_manifest.json",
    "webp_property_map": "tests/fixtures/webp_vp8l_property_map.json",
    "webp_property_inspector": "scripts/inspect_webp_vp8l_structure.py",
    "roadmap": "roadmap.json",
}

DOCS = ["roadmap.json", "docs/EVIDENCE.md"]

CURRENT_CLAIM_DOCS = ["docs/EVIDENCE.md"]
CURRENT_CLAIM_BEGIN = "<!-- current-claim-ledger:begin -->"
CURRENT_CLAIM_END = "<!-- current-claim-ledger:end -->"

UUID_RE = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
SHA_RE = re.compile(r"^[0-9a-f]{40}$")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_measured_inputs(ledger: dict, errors: list[str]) -> None:
    revision = ledger.get("base_revision")
    if not isinstance(revision, str) or not SHA_RE.fullmatch(revision):
        errors.append("cannot verify measured inputs without a valid base revision")
        return
    measured = ledger.get("measured_inputs")
    if not isinstance(measured, dict):
        errors.append("measured_inputs must contain the historical input hashes")
        return
    for field in ("manifest_sha256", "matrix_sha256"):
        relative = HASHED_FILES[field]
        result = subprocess.run(
            ["git", "show", f"{revision}:{relative}"],
            cwd=ROOT, capture_output=True,
        )
        if result.returncode:
            errors.append(f"measured revision has no {relative}")
            continue
        actual = hashlib.sha256(result.stdout).hexdigest()
        if measured.get(field) != actual:
            errors.append(f"measured_inputs.{field} does not match measured revision")


def current_claim_lines(ledger: dict, coverage: dict, measurement: dict) -> list[str]:
    plural = {"line": "lines", "branch": "branches", "function": "functions"}
    metrics = ", ".join(
        f"{coverage[name]['covered']:,}/{coverage[name]['total']:,} {plural[name]} "
        f"({coverage[name]['percent']:.4f}%)"
        for name in ("line", "branch", "function")
    )
    region = coverage["region"]
    metrics += (
        f", and {region['covered']:,}/{region['total']:,} regions "
        f"({region['percent']:.4f}%)"
    )
    return [
        "Current claim-ledger baseline (not current `HEAD`):",
        f"- Measured revision: `{ledger['base_revision']}`.",
        f"- Coverage MCP run: `{measurement['run_id']}`; snapshot: `{measurement['snapshot_id']}`.",
        f"- Coverage: {metrics}.",
        f"- Measured manifest SHA-256: `{ledger['measured_inputs'].get('manifest_sha256', 'missing')}`; measured matrix SHA-256: `{ledger['measured_inputs'].get('matrix_sha256', 'missing')}`.",
        f"- Current fixture integrity only: manifest `{ledger['manifest_sha256']}`; matrix `{ledger['matrix_sha256']}`.",
    ]


def verify_current_claim_blocks(
    ledger: dict, coverage: dict, measurement: dict, errors: list[str]
) -> None:
    expected = current_claim_lines(ledger, coverage, measurement)
    for relative in CURRENT_CLAIM_DOCS:
        path = ROOT / relative
        text = path.read_text()
        if text.count(CURRENT_CLAIM_BEGIN) != 1 or text.count(CURRENT_CLAIM_END) != 1:
            errors.append(
                f"{relative} must contain exactly one current claim-ledger block"
            )
            continue
        begin = text.index(CURRENT_CLAIM_BEGIN)
        end = text.index(CURRENT_CLAIM_END, begin)
        block = text[begin:end]
        for line in expected:
            if line not in block:
                errors.append(f"{relative} current claim block is missing: {line}")


def failures() -> list[str]:
    errors: list[str] = []
    try:
        ledger = json.loads(LEDGER_PATH.read_text())
    except (OSError, json.JSONDecodeError) as error:
        return [f"cannot read {LEDGER_PATH}: {error}"]

    if ledger.get("format_version") != 2:
        errors.append("claim ledger format_version must be 2")

    revision = ledger.get("base_revision")
    if not isinstance(revision, str) or not SHA_RE.fullmatch(revision):
        errors.append("base_revision must be a 40-character git commit SHA-1")
    else:
        check = subprocess.run(
            ["git", "rev-parse", "--verify", f"{revision}^{{commit}}"],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        if check.returncode != 0:
            errors.append(f"base_revision {revision} is not a commit in this repository")

    verify_measured_inputs(ledger, errors)

    roadmap_path = ROOT / "roadmap.json"
    try:
        roadmap = json.loads(roadmap_path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        errors.append(f"cannot read {roadmap_path}: {error}")
    else:
        if roadmap.get("claim_ledger_base_revision") != revision:
            errors.append("roadmap.json claim_ledger_base_revision does not match base_revision")

    fixture_manifests = ledger.get("fixture_manifests")
    if not isinstance(fixture_manifests, dict):
        errors.append("fixture_manifests must be an object")
        fixture_manifests = {}
    for field, relative in HASHED_FILES.items():
        if field in ("manifest_sha256", "matrix_sha256"):
            expected = ledger.get(field)
        else:
            expected = fixture_manifests.get(field)
        actual = sha256(ROOT / relative)
        if expected != actual:
            errors.append(f"{field} mismatch: ledger {expected}, tree {actual}")

    coverage = ledger.get("coverage")
    if not isinstance(coverage, dict):
        errors.append("coverage must be an object")
    else:
        for field in ("run_id", "snapshot_id"):
            value = coverage.get(field)
            if not isinstance(value, str) or not UUID_RE.fullmatch(value):
                errors.append(f"coverage.{field} must be a UUID")

    try:
        measurement = roadmap["current_state"]["coverage"]["measurement"]
    except (KeyError, TypeError):
        errors.append("roadmap.json current_state.coverage.measurement is missing")
        measurement = None
    if not isinstance(measurement, dict):
        errors.append("roadmap.json current coverage measurement must be an object")
    else:
        if measurement.get("managed_coverage_mcp") is not True:
            errors.append("roadmap current coverage measurement is not managed Coverage MCP evidence")
        for field in ("run_id", "snapshot_id", "suite"):
            if isinstance(coverage, dict) and coverage.get(field) != measurement.get(field):
                errors.append(f"coverage.{field} does not match roadmap current measurement")
        if revision != measurement.get("commit_sha"):
            errors.append("base_revision does not match roadmap current measurement commit_sha")
        current_coverage = roadmap.get("current_state", {}).get("coverage")
        if isinstance(current_coverage, dict) and isinstance(ledger.get("measured_inputs"), dict):
            verify_current_claim_blocks(ledger, current_coverage, measurement, errors)
        else:
            errors.append("roadmap current coverage metrics are missing")

    if "[evidence guide](docs/EVIDENCE.md)" not in (ROOT / "README.md").read_text():
        errors.append("README must link the canonical evidence guide")

    for doc in DOCS:
        text = (ROOT / doc).read_text()
        if revision not in text and revision[:12] not in text and revision[:7] not in text:
            errors.append(f"{doc} does not reference base_revision {revision}")

    return errors


def main() -> int:
    problems = failures()
    if problems:
        for problem in problems:
            print(f"error: {problem}", file=sys.stderr)
        return 1
    print("claim ledger OK: revision, hashes, coverage identifiers, and docs agree")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
