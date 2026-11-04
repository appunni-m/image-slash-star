#!/usr/bin/env python3
"""Measure a digest-guarded JPEG candidate against native x86 controls."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def repository_path(path: Path) -> Path:
    resolved = (ROOT / path).resolve()
    if not resolved.is_relative_to(ROOT):
        raise RuntimeError("experiment inputs and output must stay inside the repository")
    return resolved


def verify_committed_candidate_patch(
    baseline_revision: str,
    candidate_revision: str,
    source_guards: dict[str, str],
    candidate_guards: dict[str, str],
    patch: Path,
) -> None:
    """Verify a retained patch reconstructs the committed candidate source."""
    source_paths = sorted(source_guards)
    if set(candidate_guards) != set(source_paths):
        raise RuntimeError("candidate and baseline guards must cover the same source files")
    if any(Path(name).is_absolute() or ".." in Path(name).parts for name in source_paths):
        raise RuntimeError("guarded source paths must stay inside the repository")

    baseline_sources = {}
    for name, expected in source_guards.items():
        source = subprocess.check_output(
            ["git", "show", f"{baseline_revision}:{name}"], cwd=ROOT,
        )
        if hashlib.sha256(source).hexdigest() != expected:
            raise RuntimeError(f"baseline source guard changed: {name}")
        baseline_sources[name] = source

        candidate = subprocess.check_output(
            ["git", "show", f"{candidate_revision}:{name}"], cwd=ROOT,
        )
        if hashlib.sha256(candidate).hexdigest() != candidate_guards[name]:
            raise RuntimeError(f"committed candidate source changed: {name}")

    patch_bytes = patch.read_bytes()
    numstat = subprocess.check_output(
        ["git", "apply", "--numstat"], cwd=ROOT, input=patch_bytes,
    ).decode("utf-8")
    patch_paths = {line.split("\t", 2)[2] for line in numstat.splitlines()}
    if patch_paths != set(source_paths):
        raise RuntimeError("experiment patch touches a different source set than its guards")

    try:
        with tempfile.TemporaryDirectory(prefix="jpeg-candidate-source-") as directory:
            candidate_root = Path(directory)
            for name, source in baseline_sources.items():
                destination = candidate_root / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(source)
            subprocess.run(
                ["git", "apply", "--unidiff-zero", "--check"],
                cwd=candidate_root,
                input=patch_bytes,
                check=True,
            )
            subprocess.run(
                ["git", "apply", "--unidiff-zero"],
                cwd=candidate_root,
                input=patch_bytes,
                check=True,
            )
            for name, expected in candidate_guards.items():
                if digest(candidate_root / name) != expected:
                    raise RuntimeError(
                        f"experiment patch does not reconstruct the candidate source: {name}"
                    )
    except subprocess.CalledProcessError as error:
        raise RuntimeError("experiment patch does not apply to the guarded baseline") from error


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--baseline-revision",
        help="immutable control revision for candidate source committed in HEAD",
    )
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
    operation = plan.get("operation", "encode")
    if operation not in {"encode", "decode"}:
        raise RuntimeError("unsupported JPEG experiment operation")
    patch = repository_path(Path(plan["patch"]))
    if digest(patch) != plan["patch_sha256"]:
        raise RuntimeError("experiment patch differs from the predeclared plan")
    candidate_source_mode = (
        "committed_revision" if args.baseline_revision is not None else "explicit_patch"
    )
    revision = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True,
    ).strip()
    baseline_revision = revision
    if candidate_source_mode == "committed_revision":
        baseline_revision = subprocess.check_output(
            ["git", "rev-parse", "--verify", f"{args.baseline_revision}^{{commit}}"],
            cwd=ROOT,
            text=True,
        ).strip()
        if baseline_revision != args.baseline_revision:
            raise RuntimeError("baseline revision must be an immutable commit ID")
        verify_committed_candidate_patch(
            baseline_revision,
            revision,
            plan["source_guards"],
            plan["candidate_guards"],
            patch,
        )
    else:
        for name, expected in plan["source_guards"].items():
            if digest(repository_path(Path(name))) != expected:
                raise RuntimeError(f"experiment source guard changed: {name}")
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
        "--baseline-revision", baseline_revision,
    ]
    if candidate_source_mode == "explicit_patch":
        argv.extend(("--candidate-patch", str(patch)))
    argv.extend((
        "--variants", *plan["variants"],
        "--operation", operation,
        "--cohorts", str(plan["cohorts"]),
        "--cases", *cases,
        "--output", str(output),
    ))
    try:
        subprocess.run(argv, cwd=ROOT, check=True)
    finally:
        # The runner requires an empty destination. Attach the predeclared
        # producer inputs afterwards, including when a build or byte check fails.
        if output.parent.is_dir():
            for path in (plan_path, patch, Path(__file__)):
                retained = output.parent / "producer" / path.relative_to(ROOT)
                retained.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, retained)
    report = json.loads(output.read_text(encoding="utf-8"))
    expected_report_schema = (
        "image-slash-star/jpeg-revision-abba@3"
        if candidate_source_mode == "committed_revision"
        else "image-slash-star/jpeg-revision-abba@4"
    )
    if report["schema"] != expected_report_schema:
        raise RuntimeError("experiment report does not match the candidate source mode")
    if (
        report["baseline_revision"] != baseline_revision
        or report["candidate_revision"] != revision
    ):
        raise RuntimeError("measured revisions differ from the predeclared experiment")
    if candidate_source_mode == "explicit_patch":
        if report["candidate_patch"]["sha256"] != plan["patch_sha256"]:
            raise RuntimeError("measured patch differs from the predeclared plan")
    elif report["candidate_patch"] is not None:
        raise RuntimeError("committed candidate was unexpectedly patched a second time")
    if report.get("operation", "encode") != operation:
        raise RuntimeError("measured operation differs from the predeclared plan")
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
        candidate_source = pair["candidate"]["source"]
        if candidate_source_mode == "explicit_patch":
            if set(candidate_source["patched_files"]) != set(plan["source_guards"]):
                raise RuntimeError("patched implementation differs from the guarded source set")
            candidate_files = candidate_source["compiled_files"]
        else:
            candidate_files = candidate_source["files"]
        for name, expected_digest in plan["candidate_guards"].items():
            if candidate_files.get(name) != expected_digest:
                raise RuntimeError("candidate source differs from its guarded implementation")
        for name, expected_digest in plan["source_guards"].items():
            if pair["control"]["source"]["files"][name] != expected_digest:
                raise RuntimeError("baseline archive differs from the guarded source")
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
        "schema": "image-slash-star/jpeg-patch-experiment-result@2",
        "scope": f"Native x86 diagnostic {operation} patch experiment",
        "operation": operation,
        "producer_revision": revision,
        "baseline_revision": baseline_revision,
        "candidate_source_mode": candidate_source_mode,
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
    # An explicit patch is diagnostic only; committed source is the shipping
    # candidate and must meet its declared performance gates before merging.
    if candidate_source_mode == "committed_revision" and not passed:
        return 1
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"JPEG experiment error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
