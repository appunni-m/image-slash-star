#!/usr/bin/env python3
"""Measure an explicit JPEG candidate patch without changing shipping Rust code."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def repository_path(path: Path) -> Path:
    resolved = (ROOT / path).resolve()
    if not resolved.is_relative_to(ROOT):
        raise RuntimeError("experiment inputs and output must stay inside the repository")
    return resolved


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    plan_path = repository_path(args.plan)
    output = repository_path(args.output)
    if output.exists() or (output.parent.exists() and any(output.parent.iterdir())):
        raise RuntimeError("experiment output must use a fresh empty directory")
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    if plan["schema"] != "image-slash-star/jpeg-patch-experiment@1":
        raise RuntimeError("unsupported JPEG experiment plan")
    if plan["variants"] != ["sse2", "avx2"]:
        raise RuntimeError("native experiment requires both SSE2 and AVX2")
    patch = repository_path(Path(plan["patch"]))
    if digest(patch) != plan["patch_sha256"]:
        raise RuntimeError("experiment patch differs from the predeclared plan")
    for name, expected in plan["source_guards"].items():
        if digest(repository_path(Path(name))) != expected:
            raise RuntimeError(f"experiment source guard changed: {name}")
    revision = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True,
    ).strip()
    committed_plan = subprocess.check_output(
        ["git", "show", f"{revision}:{plan_path.relative_to(ROOT)}"], cwd=ROOT,
    )
    if committed_plan != plan_path.read_bytes():
        raise RuntimeError("experiment plan differs from its committed input")
    cases = [plan["primary"], *plan["guards"]]
    if len(set(cases)) != len(cases) or not plan["guards"] or plan["cohorts"] < 6:
        raise RuntimeError("experiment needs distinct primary/guard cases and at least six cohorts")
    argv = [
        sys.executable, str(ROOT / "scripts/benchmark_jpeg_revision_abba.py"),
        "--baseline-revision", revision,
        "--candidate-patch", str(patch),
        "--variants", *plan["variants"],
        "--cohorts", str(plan["cohorts"]),
        "--cases", *cases,
        "--output", str(output),
    ]
    try:
        subprocess.run(argv, cwd=ROOT, check=True)
    finally:
        # The runner requires an empty destination. Attach the predeclared
        # producer inputs afterwards, including when a build or byte check fails.
        if output.parent.is_dir():
            for path in (plan_path, Path(__file__)):
                retained = output.parent / "producer" / path.relative_to(ROOT)
                retained.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, retained)
    report = json.loads(output.read_text(encoding="utf-8"))
    if report["schema"] != "image-slash-star/jpeg-revision-abba@4":
        raise RuntimeError("experiment did not produce an explicit patched-source report")
    if report["baseline_revision"] != revision or report["candidate_revision"] != revision:
        raise RuntimeError("experiment baseline and candidate must share one committed source")
    if report["candidate_patch"]["sha256"] != plan["patch_sha256"]:
        raise RuntimeError("measured patch differs from the predeclared plan")
    if report["host"]["machine"].lower() not in {"x86_64", "amd64"}:
        raise RuntimeError("experiment was not executed on native x86-64 hardware")
    if set(report["variants"]) != set(plan["variants"]) or report["workloads"] != cases:
        raise RuntimeError("experiment workload or variant selection differs from its plan")
    expected = {(variant, case) for variant in plan["variants"] for case in cases}
    exact = {(row["variant"], row["case"]) for row in report["exact_outputs"]}
    if exact != expected or not all(row["byte_for_byte_match"] for row in report["exact_outputs"]):
        raise RuntimeError("experiment omits a complete JPEG output comparison")
    summaries = {(row["variant"], row["case"]): row for row in report["summaries"]}
    if summaries.keys() != expected:
        raise RuntimeError("experiment summaries omit declared workloads")
    if report["cohorts_per_workload"] != plan["cohorts"]:
        raise RuntimeError("experiment cohort count differs from the predeclared plan")
    for variant, case in expected:
        cohorts = [row for row in report["cohort_summaries"] if row["variant"] == variant and row["case"] == case]
        if len(cohorts) != plan["cohorts"] or {row["cohort"] for row in cohorts} != set(range(1, plan["cohorts"] + 1)):
            raise RuntimeError("experiment omits or repeats declared ABBA cohorts")
    admission = plan["admission"]
    gates = []
    for variant in plan["variants"]:
        pair = report["revision_builds"][variant]
        if set(pair["candidate"]["source"]["patched_files"]) != set(plan["source_guards"]):
            raise RuntimeError("patched implementation differs from the guarded source set")
        for name, expected_digest in plan["source_guards"].items():
            if pair["control"]["source"]["files"][name] != expected_digest:
                raise RuntimeError("baseline archive differs from the guarded source")
            if pair["candidate"]["source"]["compiled_files"][name] != plan["candidate_guards"][name]:
                raise RuntimeError("compiled candidate differs from the predeclared implementation")
        primary = summaries[(variant, plan["primary"])]
        cohorts = [row for row in report["cohort_summaries"] if row["variant"] == variant and row["case"] == plan["primary"]]
        if len(cohorts) != plan["cohorts"]:
            raise RuntimeError("experiment omits primary ABBA cohorts")
        minimum_guard_gain = min(summaries[(variant, case)]["paired_gain_percent"] for case in plan["guards"])
        passed = (
            not pair["binary_identical"]
            and primary["paired_gain_percent"] >= admission["primary_gain_minimum_percent"]
            and all(row["candidate_over_control"] < 1 for row in cohorts)
            and minimum_guard_gain >= -admission["guard_slowdown_limit_percent"]
        )
        gates.append({
            "variant": variant,
            "primary_paired_gain_percent": primary["paired_gain_percent"],
            "primary_cohorts": len(cohorts),
            "primary_cohorts_faster": primary["cohorts_faster"],
            "minimum_guard_paired_gain_percent": minimum_guard_gain,
            "performance_gate_passed": passed,
        })
    passed = all(row["performance_gate_passed"] for row in gates)
    result = {
        "schema": "image-slash-star/jpeg-patch-experiment-result@1",
        "scope": "Native x86 diagnostic patch experiment; shipping implementation unchanged",
        "producer_revision": revision,
        "plan_sha256": digest(plan_path),
        "producer_sha256": digest(Path(__file__)),
        "patch_sha256": plan["patch_sha256"],
        "summary_sha256": digest(output),
        "argv": argv,
        "variants": gates,
        "native_performance_gate_passed": passed,
        "adoption": "pending exact Pillow parity and architecture policy" if passed else "reject",
    }
    result_path = output.parent / "experiment-result.json"
    result_path.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))
    # A rejected diagnostic candidate is a valid completed experiment. Only
    # build, provenance or correctness failures fail the workflow.
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"JPEG experiment error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
