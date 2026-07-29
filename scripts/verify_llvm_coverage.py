#!/usr/bin/env python3
"""Require complete LLVM line, branch, function, and region coverage."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any


METRICS = ("lines", "branches", "functions", "regions")


def fail(message: str) -> None:
    raise SystemExit(f"coverage verification failed: {message}")


def load_totals(path: Path) -> dict[str, Any]:
    try:
        report = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {path}: {error}")

    data = report.get("data")
    if not isinstance(data, list) or len(data) != 1:
        fail("expected exactly one LLVM coverage data object")
    totals = data[0].get("totals")
    if not isinstance(totals, dict):
        fail("LLVM report has no aggregate totals")
    return totals


def main(arguments: list[str]) -> None:
    if len(arguments) != 2:
        fail("usage: verify_llvm_coverage.py <llvm-coverage.json>")

    totals = load_totals(Path(arguments[1]))
    for metric in METRICS:
        value = totals.get(metric)
        if not isinstance(value, dict):
            fail(f"LLVM report has no {metric} totals")
        covered = value.get("covered")
        count = value.get("count")
        if not isinstance(covered, int) or not isinstance(count, int):
            fail(f"LLVM {metric} totals are not integers")
        if count == 0:
            fail(f"LLVM report contains zero {metric}")
        if covered != count:
            fail(f"{metric}: {covered}/{count}")
        print(f"{metric}: {covered}/{count}")


if __name__ == "__main__":
    main(sys.argv)
