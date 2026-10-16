#!/usr/bin/env python3
"""Reject enabled Cargo unit-test targets rooted in workspace source trees."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def enabled_source_test_targets() -> list[str]:
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "cargo metadata failed")

    metadata = json.loads(result.stdout)
    findings: list[str] = []
    for package in metadata["packages"]:
        source_root = Path(package["manifest_path"]).resolve().parent / "src"
        for target in package["targets"]:
            if not target.get("test", False):
                continue
            source_path = Path(target["src_path"]).resolve()
            try:
                source_path.relative_to(source_root)
            except ValueError:
                continue
            findings.append(f"{package['name']}:{target['name']} ({source_path})")
    return findings


def main() -> int:
    try:
        findings = enabled_source_test_targets()
    except (OSError, RuntimeError, json.JSONDecodeError) as error:
        print(f"source test-target check failed: {error}", file=sys.stderr)
        return 2

    if findings:
        print("enabled unit-test targets found under workspace src/ trees:", file=sys.stderr)
        for finding in findings:
            print(f"  {finding}", file=sys.stderr)
        print("Keep codec coverage in public manifest parity targets.", file=sys.stderr)
        return 1

    print("No Cargo unit-test targets are enabled under workspace src/ trees.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
