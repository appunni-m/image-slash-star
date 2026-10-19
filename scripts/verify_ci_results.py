#!/usr/bin/env python3
"""Fail the stable CI status when an applicable job did not pass."""

from __future__ import annotations

from collections.abc import Mapping
import os
import sys


RESULT_ENV_BY_JOB = {
    "fixture-integrity": "FIXTURE_RESULT",
    "quality": "QUALITY_RESULT",
    "x86-simd-lint": "X86_RESULT",
    "windows-msvc": "WINDOWS_RESULT",
    "supply-chain": "SUPPLY_CHAIN_RESULT",
    "coverage": "COVERAGE_RESULT",
}
JOB_LABELS = {
    "fixture-integrity": "Fixture and release metadata",
    "quality": "Format, lint, and parity",
    "x86-simd-lint": "x86 SSE2 and AVX2 checks",
    "windows-msvc": "Windows MSVC runtime and strict lint",
    "supply-chain": "Dependencies and licenses",
    "coverage": "Coverage report",
}
SUPPORTED_EVENTS = frozenset({"push", "pull_request", "schedule", "workflow_dispatch"})
FULL_COVERAGE_EVENTS = frozenset({"schedule", "workflow_dispatch"})


def check_ci_results(event_name: str, results: Mapping[str, str]) -> list[str]:
    """Return policy violations for GitHub Actions job conclusions."""

    violations = []
    if event_name not in SUPPORTED_EVENTS:
        violations.append(f"unsupported GitHub event {event_name!r}")

    for job_id in RESULT_ENV_BY_JOB:
        if job_id == "coverage":
            continue
        result = results.get(job_id, "missing")
        if result != "success":
            violations.append(
                f"Required CI job '{JOB_LABELS[job_id]}' concluded '{result}'."
            )

    coverage_result = results.get("coverage", "missing")
    if event_name in FULL_COVERAGE_EVENTS:
        if coverage_result != "success":
            violations.append(
                f"Required CI job '{JOB_LABELS['coverage']}' concluded "
                f"'{coverage_result}'."
            )
    elif coverage_result not in {"success", "skipped"}:
        violations.append(
            "Coverage report concluded unexpected state "
            f"'{coverage_result}'."
        )

    return violations


def main(environ: Mapping[str, str] | None = None) -> int:
    """Read job results from the workflow environment and enforce the gate."""

    environment = os.environ if environ is None else environ
    results = {
        job_id: environment.get(variable, "missing")
        for job_id, variable in RESULT_ENV_BY_JOB.items()
    }
    violations = check_ci_results(environment.get("EVENT_NAME", ""), results)
    if violations:
        for violation in violations:
            print(f"::error::{violation}", file=sys.stderr)
        return 1

    print("All applicable CI jobs passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
