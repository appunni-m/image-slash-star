#!/usr/bin/env python3
"""Verify that committed JPEG code matches a measured patch candidate."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def repository_path(path: str) -> Path:
    resolved = (ROOT / path).resolve()
    if not resolved.is_relative_to(ROOT):
        raise RuntimeError("plan paths must remain inside the repository")
    return resolved


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", required=True)
    args = parser.parse_args()

    plan_path = repository_path(args.plan)
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    if plan.get("schema") != "image-slash-star/jpeg-patch-experiment@1":
        raise RuntimeError("unsupported JPEG patch experiment plan")
    candidates = plan.get("candidate_guards")
    if not isinstance(candidates, dict) or not candidates:
        raise RuntimeError("plan omits compiled candidate source hashes")
    for relative, expected in candidates.items():
        actual = hashlib.sha256(repository_path(relative).read_bytes()).hexdigest()
        if actual != expected:
            raise RuntimeError(f"source does not match measured patch candidate: {relative}")
    print(f"verified {len(candidates)} committed source file(s) against {plan_path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
