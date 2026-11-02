#!/usr/bin/env python3
"""Verify AVIF sequence references against the pinned native loop evidence."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys

from avif_loop_evidence import LoopEvidenceError, validate_loop_evidence


ROOT = Path(__file__).resolve().parent.parent
INPUT_PATH = ROOT / "tests" / "fixtures" / "input" / "jsons" / "Decode.avif.json"
OUTPUT_PATH = ROOT / "tests" / "fixtures" / "outputs" / "jsons" / "Decode.avif.json"
MATRIX_PATH = ROOT / "tests" / "fixtures" / "coverage_matrix.json"
ASSETS = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
LOOP_INDEX = ROOT / "tests" / "fixtures" / "outputs" / "avif_loops" / "index.json"


def read_cases(path: Path) -> dict[str, dict]:
    """Read a fixture document and reject missing or duplicate case IDs."""
    document = json.loads(path.read_text(encoding="utf-8"))
    cases = document.get("cases")
    if not isinstance(cases, list):
        raise ValueError(f"{path.relative_to(ROOT)} has no cases array")
    result = {}
    for case in cases:
        if not isinstance(case, dict) or not isinstance(case.get("id"), str):
            raise ValueError(f"{path.relative_to(ROOT)} contains a case without an ID")
        case_id = case["id"]
        if case_id in result:
            raise ValueError(f"{path.relative_to(ROOT)} repeats case {case_id}")
        result[case_id] = case
    return result


def matrix_cases() -> dict[str, dict]:
    """Read AVIF decode rows from the shared coverage matrix."""
    document = json.loads(MATRIX_PATH.read_text(encoding="utf-8"))
    try:
        rows = document["formats"]["avif"]["decode"]
    except (KeyError, TypeError) as error:
        raise ValueError("coverage matrix has no AVIF decode rows") from error
    if not isinstance(rows, list):
        raise ValueError("coverage matrix AVIF decode rows are invalid")
    result = {}
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get("id"), str):
            raise ValueError("coverage matrix contains an AVIF row without an ID")
        row_id = row["id"]
        if row_id in result:
            raise ValueError(f"coverage matrix repeats AVIF case {row_id}")
        result[row_id] = row
    return result


def validate_observer_builds(index: dict) -> None:
    """Bind every native observation to the build that produced it."""
    def is_sha256(value: object) -> bool:
        return (
            isinstance(value, str)
            and len(value) == 64
            and all(character in "0123456789abcdef" for character in value)
        )

    cases = index.get("cases")
    primary_build = index.get("build")
    if not isinstance(cases, list) or not isinstance(primary_build, dict):
        raise ValueError("AVIF loop index lacks cases or its primary build")
    primary_hash = primary_build.get("binary_sha256")
    if not is_sha256(primary_hash):
        raise ValueError("AVIF loop index has an invalid primary observer hash")

    cases_by_name = {}
    for case in cases:
        if not isinstance(case, dict) or not isinstance(case.get("name"), str):
            raise ValueError("AVIF loop index contains a case without a name")
        if case["name"] in cases_by_name:
            raise ValueError(f"AVIF loop index repeats case {case['name']}")
        cases_by_name[case["name"]] = case

    collections = index.get("additional_collections", [])
    if not isinstance(collections, list):
        raise ValueError("AVIF additional_collections must be an array")
    assigned_names = set()
    for collection in collections:
        if not isinstance(collection, dict):
            raise ValueError("AVIF additional collection must be an object")
        build = collection.get("build")
        names = collection.get("case_names")
        if not isinstance(build, dict) or not isinstance(names, list) or not names:
            raise ValueError("AVIF additional collection needs a build and case_names")
        binary_hash = build.get("binary_sha256")
        if not is_sha256(binary_hash):
            raise ValueError("AVIF additional collection has an invalid observer hash")
        for name in names:
            if not isinstance(name, str) or name in assigned_names:
                raise ValueError(
                    "AVIF additional collection case names are invalid or repeated"
                )
            case = cases_by_name.get(name)
            if case is None or case.get("observer_binary_sha256") != binary_hash:
                raise ValueError(f"AVIF additional collection build does not match {name}")
            assigned_names.add(name)

    for name, case in cases_by_name.items():
        observed_hash = case.get("observer_binary_sha256")
        if not is_sha256(observed_hash):
            raise ValueError(f"AVIF loop case {name} has an invalid observer hash")
        if observed_hash != primary_hash and name not in assigned_names:
            raise ValueError(f"AVIF loop case {name} has no matching observer build record")
        if observed_hash == primary_hash and name in assigned_names:
            raise ValueError(f"AVIF loop case {name} redundantly lists the primary build")


def validate() -> int:
    """Check every retained AVIF sequence loop observation and its digest."""
    loop_index = json.loads(LOOP_INDEX.read_text(encoding="utf-8"))
    validate_observer_builds(loop_index)
    inputs = read_cases(INPUT_PATH)
    outputs = read_cases(OUTPUT_PATH)
    matrix = matrix_cases()
    validated = 0

    for case_id, output in outputs.items():
        sequence = output.get("sequence")
        if not isinstance(sequence, dict) or not isinstance(sequence.get("loop_evidence"), dict):
            continue
        source = inputs.get(case_id)
        row = matrix.get(case_id)
        if source is None or row is None:
            raise ValueError(f"AVIF loop case {case_id} is missing from a manifest")
        if row.get("sequence") != sequence:
            raise ValueError(f"AVIF loop case {case_id} differs between reference and matrix")

        asset = source.get("asset")
        if not isinstance(asset, str) or Path(asset).name != asset:
            raise ValueError(f"AVIF loop case {case_id} has an invalid input asset")
        data = (ASSETS / asset).read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        if source.get("asset_sha256") != digest or output.get("asset_sha256") != digest:
            raise ValueError(f"AVIF loop case {case_id} has a stale input hash")
        try:
            validate_loop_evidence(sequence, digest)
        except LoopEvidenceError as error:
            raise ValueError(f"AVIF loop case {case_id}: {error}") from error
        validated += 1

    if validated == 0:
        raise ValueError("no AVIF sequence loop references were validated")
    return validated


def main() -> int:
    try:
        validated = validate()
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(f"AVIF loop evidence OK: {validated} sequence references match the pinned index")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
