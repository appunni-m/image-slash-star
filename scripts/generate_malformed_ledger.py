#!/usr/bin/env python3
"""Generate the per-format malformed-class ledger from the coverage matrix.

QA-018 requires a maintained per-format malformed-class ledger with Pillow
outcome, specification status, Rust error, and evidence origin for every
active error class. The ledger is generated from
``tests/fixtures/coverage_matrix.json`` so it cannot drift from the fixture
matrix: run with ``--check`` in CI and fail on any diff.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MATRIX_PATH = ROOT / "tests" / "fixtures" / "coverage_matrix.json"
LEDGER_PATH = ROOT / "tests" / "fixtures" / "malformed_ledger.json"

# Curated specification-status overrides for classes whose status is not
# derivable from the class id/description text. Values:
#   spec_violation | truncated | not_the_format | tolerated | ambiguous
SPEC_STATUS_OVERRIDES: dict[str, str] = {
    "jpeg:malformed_markers_fill_marker_only": "ambiguous",
    "gif:error_malformed_near_miss_version": "ambiguous",
    "png:error_malformed_structure_ihdr_trailing_byte": "spec_violation",
    "png:error_malformed_structure_short_chunk_kind": "truncated",
}

TRUNCATED_HINTS = (
    "truncated",
    "short",
    "near_miss",
    "partial",
    "missing",
    "no_length",
    "no_payload",
)

VALID_STATUSES = ("spec_violation", "truncated", "not_the_format", "tolerated", "ambiguous")


def derive_spec_status(fmt: str, class_id: str, description: str) -> str:
    key = f"{fmt}:{class_id}"
    if key in SPEC_STATUS_OVERRIDES:
        return SPEC_STATUS_OVERRIDES[key]
    text = f"{class_id} {description}".lower()
    if any(hint in text for hint in TRUNCATED_HINTS):
        return "truncated"
    if "tolerat" in text or "lenien" in text:
        return "tolerated"
    return "spec_violation"


def contract_summary(contracts: dict) -> dict | None:
    if not contracts:
        return None
    return {
        "pillow_type": contracts.get("pillow_type"),
        "pillow_message": contracts.get("pillow_message"),
        "rust_kind": contracts.get("rust_kind"),
        "rust_format": contracts.get("rust_format"),
        "rust_message": contracts.get("rust_message"),
        "origin": contracts.get("origin"),
    }


def build_ledger() -> dict:
    matrix = json.loads(MATRIX_PATH.read_text())
    formats: dict[str, list[dict]] = {}
    total = 0
    status_counts: dict[str, int] = {status: 0 for status in VALID_STATUSES}
    origin_counts: dict[str, int] = {}
    format_counts: dict[str, int] = {}

    for fmt_name, fmt_data in sorted(matrix["formats"].items()):
        classes = []
        rows = [
            row
            for row in fmt_data.get("decode", [])
            if row.get("expect_error") or row.get("rust_expect_error")
        ]
        rows.sort(key=lambda row: row["id"])
        for row in rows:
            class_id = row["id"]
            description = row.get("description") or ""
            detected = row.get("oracle_detects_format")
            detect_contract = (row.get("error_contracts") or {}).get("detect")
            not_the_format = detected is False or (
                detect_contract is not None
                and detect_contract.get("rust_kind") == "unknown_format"
            )
            spec_status = (
                "not_the_format"
                if not_the_format
                else derive_spec_status(fmt_name, class_id, description)
            )
            contracts = {
                operation: contract_summary(contract)
                for operation, contract in sorted((row.get("error_contracts") or {}).items())
            }
            origins = sorted(set((row.get("assertion_origins") or {}).values()))
            for origin in origins:
                origin_counts[origin] = origin_counts.get(origin, 0) + 1
            classes.append(
                {
                    "class": class_id,
                    "asset": row.get("asset"),
                    "description": description,
                    "spec_status": spec_status,
                    "pillow_status": row.get("oracle_status"),
                    "pillow_type": row.get("oracle_error_type"),
                    "pillow_message": row.get("oracle_error_message"),
                    "inspect_status": row.get("inspect_status"),
                    "verify_status": row.get("verify_status"),
                    "contracts": contracts,
                    "origins": origins,
                }
            )
            status_counts[spec_status] += 1
            total += 1
        formats[fmt_name] = classes
        format_counts[fmt_name] = len(classes)

    return {
        "format_version": 1,
        "source": str(MATRIX_PATH.relative_to(ROOT)),
        "generated_by": "scripts/generate_malformed_ledger.py",
        "summary": {
            "classes": total,
            "formats": len(formats),
            "by_format": format_counts,
            "by_spec_status": status_counts,
            "by_origin": dict(sorted(origin_counts.items())),
        },
        "formats": formats,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail instead of writing when the committed ledger is stale",
    )
    args = parser.parse_args()

    ledger = build_ledger()
    serialized = json.dumps(ledger, indent=2, sort_keys=True) + "\n"
    if args.check:
        committed = LEDGER_PATH.read_text()
        if committed != serialized:
            print(
                f"error: {LEDGER_PATH.relative_to(ROOT)} is stale; "
                "run scripts/generate_malformed_ledger.py",
                file=sys.stderr,
            )
            return 1
        summary = ledger["summary"]
        print(
            f"malformed ledger OK: {summary['classes']} classes across "
            f"{summary['formats']} formats"
        )
        return 0

    LEDGER_PATH.write_text(serialized)
    print(f"wrote {LEDGER_PATH.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
