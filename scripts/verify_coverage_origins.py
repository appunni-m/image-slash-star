#!/usr/bin/env python3
"""Verify the provenance inventory for exact ``cfg(coverage)`` guards.

The inventory is a static source audit. It deliberately does not execute Rust
tests and it rejects ``pillow_fixture`` as an origin for a coverage guard:
Pillow parity is represented by ``coverage_matrix.json`` instead.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INVENTORY = ROOT / "tests" / "fixtures" / "coverage_origin_manifest.json"
GUARD = re.compile(r"^\s*#\[cfg\(coverage\)\]\s*$")
ORIGINS = {
    "defensive_model",
    "independent_implementation",
    "specification_reference",
}


def source_files() -> list[Path]:
    return sorted(
        path
        for root in (ROOT / "src", ROOT / "tests")
        for path in root.rglob("*.rs")
        if path.is_file()
    )


def guards(path: Path) -> list[int]:
    return [
        line_number
        for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1)
        if GUARD.fullmatch(line)
    ]


def fail(message: str) -> None:
    raise RuntimeError(message)


def verify() -> tuple[int, int]:
    try:
        document = json.loads(INVENTORY.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {INVENTORY}: {error}")

    if document.get("format_version") != 1:
        fail("coverage origin manifest format_version must be 1")
    entries = document.get("entries")
    if not isinstance(entries, list) or not entries:
        fail("coverage origin manifest entries must be a non-empty array")

    indexed: dict[str, dict] = {}
    for entry in entries:
        if not isinstance(entry, dict):
            fail("coverage origin entries must be objects")
        path = entry.get("path")
        if not isinstance(path, str) or not path or path in indexed:
            fail(f"invalid or duplicate coverage origin path: {path!r}")
        if Path(path).is_absolute() or ".." in Path(path).parts or not path.endswith(".rs"):
            fail(f"coverage origin path is not a repository-relative Rust path: {path!r}")
        origin = entry.get("origin")
        if origin is not None and origin not in ORIGINS:
            fail(f"{path}: unsupported coverage origin {origin!r}")
        if origin == "pillow_fixture":
            fail(f"{path}: cfg(coverage) cannot have Pillow parity origin")
        guard_count = entry.get("guard_count")
        if not isinstance(guard_count, int) or guard_count < 1:
            fail(f"{path}: guard_count must be a positive integer")
        indexed[path] = entry

    discovered: dict[str, list[int]] = {}
    for path in source_files():
        relative = path.relative_to(ROOT).as_posix()
        line_numbers = guards(path)
        if line_numbers:
            discovered[relative] = line_numbers

    if set(indexed) != set(discovered):
        missing = sorted(set(discovered) - set(indexed))
        extra = sorted(set(indexed) - set(discovered))
        fail(f"coverage origin file set differs: missing={missing}, extra={extra}")

    total = 0
    for path, line_numbers in discovered.items():
        entry = indexed[path]
        if entry["guard_count"] != len(line_numbers):
            fail(
                f"{path}: guard_count {entry['guard_count']} differs from source "
                f"count {len(line_numbers)}"
            )
        total += len(line_numbers)
        explicit = entry.get("guards")
        if explicit is None:
            if entry.get("origin") not in ORIGINS:
                fail(f"{path}: origin is required when guards are not listed individually")
            continue
        if not isinstance(explicit, list):
            fail(f"{path}: guards must be an array")
        by_line: dict[int, dict] = {}
        for guard in explicit:
            if not isinstance(guard, dict):
                fail(f"{path}: listed guards must be objects")
            line = guard.get("line")
            origin = guard.get("origin")
            if not isinstance(line, int) or line in by_line:
                fail(f"{path}: invalid or duplicate listed guard line {line!r}")
            if origin not in ORIGINS:
                fail(f"{path}:{line}: unsupported coverage origin {origin!r}")
            by_line[line] = guard
        if set(by_line) != set(line_numbers):
            fail(
                f"{path}: listed guard lines {sorted(by_line)} differ from source "
                f"lines {line_numbers}"
            )

    return total, len(discovered)


def main() -> int:
    try:
        total, files = verify()
    except RuntimeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        f"coverage origin inventory OK: {total} exact cfg(coverage) guards "
        f"across {files} files; no Pillow-parity origin assigned"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
