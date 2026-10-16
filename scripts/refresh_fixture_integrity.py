#!/usr/bin/env python3
"""Refresh current fixture-integrity receipts without changing measured evidence."""

from __future__ import annotations

import copy
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import verify_claim_ledger as claim_ledger

ROOT = Path(__file__).resolve().parent.parent
LEDGER_PATH = ROOT / "tests" / "fixtures" / "claim_ledger.json"
WEBP_MAP_PATH = ROOT / "tests" / "fixtures" / "webp_vp8l_property_map.json"
EVIDENCE_PATH = ROOT / "docs" / "EVIDENCE.md"
SHA_RE = re.compile(r"^[0-9a-f]{40}$")
HISTORICAL_FIELDS = ("base_revision", "coverage", "measured_inputs")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_object(path: Path) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {path.relative_to(ROOT)}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{path.relative_to(ROOT)} must contain a JSON object")
    return value


def validate_historical_evidence(ledger: dict, roadmap: dict) -> dict:
    """Reject refreshes that would hide missing or changed historical evidence."""
    if ledger.get("format_version") != 2:
        raise ValueError("claim ledger format_version must be 2")

    revision = ledger.get("base_revision")
    if not isinstance(revision, str) or not SHA_RE.fullmatch(revision):
        raise ValueError("base_revision must be a 40-character git commit SHA-1")

    revision_check = subprocess.run(
        ["git", "rev-parse", "--verify", f"{revision}^{{commit}}"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if revision_check.returncode != 0:
        raise ValueError(f"base_revision {revision} is not a commit in this repository")

    errors: list[str] = []
    claim_ledger.verify_measured_inputs(ledger, errors)
    if errors:
        raise ValueError("historical inputs are invalid: " + "; ".join(errors))

    if roadmap.get("claim_ledger_base_revision") != revision:
        raise ValueError(
            "roadmap claim_ledger_base_revision differs from the measured ledger; "
            "fixture receipt refresh cannot change the historical baseline"
        )

    coverage = ledger.get("coverage")
    current_state = roadmap.get("current_state")
    if not isinstance(current_state, dict):
        raise ValueError("roadmap current_state must be an object")
    roadmap_coverage = current_state.get("coverage")
    if not isinstance(roadmap_coverage, dict):
        raise ValueError("roadmap current_state.coverage must be an object")
    measurement = roadmap_coverage.get("measurement")
    if not isinstance(coverage, dict) or not isinstance(measurement, dict):
        raise ValueError("historical coverage measurement is missing")
    for field in ("run_id", "snapshot_id"):
        value = coverage.get(field)
        if not isinstance(value, str) or not claim_ledger.UUID_RE.fullmatch(value):
            raise ValueError(f"coverage.{field} must be a UUID")
        if value != measurement.get(field):
            raise ValueError(f"coverage.{field} differs from the roadmap measurement")
    if not isinstance(coverage.get("suite"), str) or not coverage["suite"]:
        raise ValueError("coverage.suite must be a non-empty string")
    if measurement.get("suite") != coverage["suite"]:
        raise ValueError("coverage.suite differs from the roadmap measurement")
    if measurement.get("managed_coverage_mcp") is not True:
        raise ValueError("roadmap measurement is not managed Coverage MCP evidence")
    if measurement.get("commit_sha") != revision:
        raise ValueError("roadmap measurement commit_sha differs from base_revision")

    return measurement


def refreshed_documents(
    ledger: dict, roadmap: dict, webp_map: dict
) -> tuple[str, str, str]:
    """Build updated JSON receipts and the current-hash line in the evidence guide."""
    inputs = webp_map.get("inputs")
    if not isinstance(inputs, dict):
        raise ValueError("WebP property map inputs must be an object")
    manifest_sha = sha256(ROOT / claim_ledger.HASHED_FILES["manifest_sha256"])
    matrix_sha = sha256(ROOT / claim_ledger.HASHED_FILES["matrix_sha256"])
    inputs["manifest_sha256"] = manifest_sha
    inputs["matrix_sha256"] = matrix_sha

    ledger["manifest_sha256"] = manifest_sha
    ledger["matrix_sha256"] = matrix_sha

    fixture_manifests = ledger.get("fixture_manifests")
    if not isinstance(fixture_manifests, dict):
        raise ValueError("claim ledger fixture_manifests must be an object")
    webp_map_text = (
        json.dumps(webp_map, ensure_ascii=True, indent=2, sort_keys=True) + "\n"
    )
    for field, relative in claim_ledger.HASHED_FILES.items():
        if field not in ("manifest_sha256", "matrix_sha256"):
            if field == "webp_property_map":
                digest = hashlib.sha256(webp_map_text.encode("utf-8")).hexdigest()
            else:
                digest = sha256(ROOT / relative)
            fixture_manifests[field] = digest

    evidence = EVIDENCE_PATH.read_text(encoding="utf-8")
    begin = claim_ledger.CURRENT_CLAIM_BEGIN
    end = claim_ledger.CURRENT_CLAIM_END
    if evidence.count(begin) != 1 or evidence.count(end) != 1:
        raise ValueError("docs/EVIDENCE.md must contain one current claim-ledger block")
    block_start = evidence.index(begin) + len(begin)
    block_end = evidence.index(end, block_start)
    block = evidence[block_start:block_end]

    current_state = roadmap.get("current_state")
    if not isinstance(current_state, dict):
        raise ValueError("roadmap current_state must be an object")
    coverage = current_state.get("coverage")
    if not isinstance(coverage, dict):
        raise ValueError("roadmap current_state.coverage must be an object")
    expected_lines = claim_ledger.current_claim_lines(
        ledger, coverage, coverage["measurement"]
    )
    for line in expected_lines[:-1]:
        if line not in block.splitlines():
            raise ValueError(
                "the current evidence block differs from the historical baseline; "
                "receipt refresh will not rewrite coverage claims"
            )

    current_hash_lines = [
        line
        for line in block.splitlines()
        if line.startswith("- Current fixture integrity only:")
    ]
    if len(current_hash_lines) != 1:
        raise ValueError(
            "the current evidence block must contain one fixture-integrity line"
        )
    updated_block = block.replace(current_hash_lines[0], expected_lines[-1], 1)
    updated_evidence = evidence[:block_start] + updated_block + evidence[block_end:]

    ledger_text = json.dumps(ledger, ensure_ascii=True, indent=2, sort_keys=True) + "\n"
    return webp_map_text, ledger_text, updated_evidence


def write_atomically(path: Path, content: str) -> None:
    """Replace a repository file atomically from a temporary sibling."""
    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            newline="\n",
            dir=path.parent,
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as temporary:
            temporary_path = Path(temporary.name)
            temporary.write(content)
            temporary.flush()
            os.fsync(temporary.fileno())
        os.replace(temporary_path, path)
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)


def refresh() -> list[str]:
    ledger = read_object(LEDGER_PATH)
    roadmap = read_object(ROOT / "roadmap.json")
    webp_map = read_object(WEBP_MAP_PATH)
    validate_historical_evidence(ledger, roadmap)

    historical_before = {
        field: copy.deepcopy(ledger.get(field)) for field in HISTORICAL_FIELDS
    }
    webp_map_text, ledger_text, evidence_text = refreshed_documents(
        ledger, roadmap, webp_map
    )
    for field, original in historical_before.items():
        if ledger.get(field) != original:
            raise ValueError(f"refresh attempted to change historical {field}")

    updates = (
        (WEBP_MAP_PATH, webp_map_text),
        (LEDGER_PATH, ledger_text),
        (EVIDENCE_PATH, evidence_text),
    )
    changed: list[str] = []
    for path, content in updates:
        if path.read_text(encoding="utf-8") != content:
            write_atomically(path, content)
            changed.append(path.relative_to(ROOT).as_posix())

    errors = claim_ledger.failures()
    if errors:
        raise ValueError("refreshed claim ledger is invalid: " + "; ".join(errors))
    return changed


def main() -> int:
    try:
        changed = refresh()
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    if changed:
        print("refreshed fixture-integrity receipts: " + ", ".join(changed))
    else:
        print("fixture-integrity receipts are already current")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
