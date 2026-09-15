#!/usr/bin/env python3
"""Enforce explicit alpha coverage floors without excluding measured code."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

# Owner-approved alpha acceptance, below the 2026-09-14 full all-feature run:
# lines 59.2149%, branches 46.2216%, functions 52.9208%, regions 58.2759%.
# Strict completeness remains available via make coverage-complete.
RELEASE_FLOORS = {"lines": 59, "branches": 46, "functions": 52, "regions": 58}
METRICS = tuple(RELEASE_FLOORS)


def load_totals(path: Path) -> dict[str, Any]:
    report = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(report, dict):
        raise ValueError("LLVM report must be an object")
    data = report.get("data")
    if not isinstance(data, list) or len(data) != 1 or not isinstance(data[0], dict):
        raise ValueError("expected exactly one LLVM coverage data object")
    totals = data[0].get("totals")
    if not isinstance(totals, dict):
        raise ValueError("LLVM report has no aggregate totals")
    return totals


def verify(totals: dict[str, Any], *, strict: bool = False) -> list[str]:
    failures = []
    for metric, release_floor in RELEASE_FLOORS.items():
        value = totals.get(metric)
        if not isinstance(value, dict):
            raise ValueError(f"LLVM report has no {metric} totals")
        covered, count = value.get("covered"), value.get("count")
        if type(covered) is not int or type(count) is not int:
            raise ValueError(f"LLVM {metric} totals are not integers")
        if count <= 0 or not 0 <= covered <= count:
            raise ValueError(f"invalid {metric} coverage: {covered}/{count}")
        floor = 100 if strict else release_floor
        print(f"{metric}: {covered}/{count} = {100 * covered / count:.6f}% (required {floor}%)")
        # Compare counts, never rounded or caller-supplied percentages.
        if 100 * covered < floor * count:
            failures.append(f"{metric}: {covered}/{count} is below {floor}%")
    return failures


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("--strict", action="store_true", help="require 100 percent in every metric")
    args = parser.parse_args()
    try:
        failures = verify(load_totals(args.report), strict=args.strict)
        if failures:
            raise ValueError("; ".join(failures))
    except (OSError, ValueError) as error:
        raise SystemExit(f"coverage verification failed: {error}") from error


if __name__ == "__main__":
    main()
