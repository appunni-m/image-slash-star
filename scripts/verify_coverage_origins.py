#!/usr/bin/env python3
"""Verify the provenance inventory for exact ``cfg(coverage)`` guards.

The inventory is a static source audit. Guard identities use per-file
ordinals so ordinary line insertions do not invalidate provenance. The verifier
still requires every guard to be accounted for and rejects source coverage
exclusions and ``pillow_fixture`` as an origin. Pillow parity is represented by
``coverage_matrix.json`` instead.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INVENTORY = ROOT / "tests" / "fixtures" / "coverage_origin_manifest.json"
GUARD = re.compile(r"^\s*#\[cfg\(coverage\)\]\s*$")
EXCLUSION = re.compile(
    r"^[ \t]*#\[[^\]]*\bcoverage\s*\(\s*off\s*\)[^\]]*\]", re.MULTILINE
)
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

    if document.get("format_version") != 2:
        fail("coverage origin manifest format_version must be 2")
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
        source = path.read_text(encoding="utf-8")
        exclusion = EXCLUSION.search(source)
        if exclusion is not None:
            line = source.count("\n", 0, exclusion.start()) + 1
            fail(f"{relative}:{line}: source coverage exclusions are forbidden")
        line_numbers = guards(path)
        if line_numbers:
            discovered[relative] = line_numbers

    if set(indexed) != set(discovered):
        missing = sorted(set(discovered) - set(indexed))
        extra = sorted(set(indexed) - set(discovered))
        fail(f"coverage origin file set differs: missing={missing}, extra={extra}")

    total = 0
    for path, source_lines in discovered.items():
        entry = indexed[path]
        if entry["guard_count"] != len(source_lines):
            fail(
                f"{path}: guard_count {entry['guard_count']} differs from source "
                f"count {len(source_lines)}"
            )
        total += len(source_lines)
        explicit = entry.get("guards")
        if explicit is None:
            if entry.get("origin") not in ORIGINS:
                fail(f"{path}: origin is required when guards are not listed individually")
            continue
        if not isinstance(explicit, list):
            fail(f"{path}: guards must be an array")
        by_index: dict[int, dict] = {}
        for guard in explicit:
            if not isinstance(guard, dict):
                fail(f"{path}: listed guards must be objects")
            if "line" in guard:
                fail(f"{path}: guard identities must use stable indexes, not source lines")
            index = guard.get("index")
            origin = guard.get("origin")
            if type(index) is not int or index < 1 or index in by_index:
                fail(f"{path}: invalid or duplicate guard index {index!r}")
            if origin not in ORIGINS:
                fail(f"{path}: guard {index}: unsupported coverage origin {origin!r}")
            by_index[index] = guard
        expected_indexes = set(range(1, len(source_lines) + 1))
        if set(by_index) != expected_indexes:
            fail(
                f"{path}: listed guard indexes {sorted(by_index)} differ from "
                f"source indexes {sorted(expected_indexes)}"
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
        f"across {files} files; no source exclusions or Pillow-parity origin assigned"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
