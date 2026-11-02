#!/usr/bin/env python3
"""Compare JPEG encoder revisions in an interleaved same-host benchmark."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import io
import json
import os
import platform
import statistics
import subprocess
import sys
import tarfile
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BENCHMARK_MANIFEST = Path("benchmarks/jpeg-production/rust/Cargo.toml")
BINARY_NAME = "jpeg-production-matrix-rust"
CONTROL_SOURCE_PATHS = (
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo/config.toml",
    "README.md",
    "src",
    "benchmarks/jpeg-production/rust",
)
CASES = (
    "rgb-128-q10-420",
    "rgb-512-q10-420",
    "rgb-128-q85-420",
)
VARIANTS = {
    "sse2": "-C target-cpu=x86-64",
    "avx2": "-C target-cpu=x86-64 -C target-feature=+avx2",
}


def load_benchmark_matrix():
    path = ROOT / "benchmarks/jpeg-production/run_matrix.py"
    spec = importlib.util.spec_from_file_location("jpeg_production_matrix", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load benchmark workload definitions from {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def git(*arguments: str) -> str:
    result = subprocess.run(
        ["git", *arguments],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return result.stdout.strip()


def resolve_path(path: Path) -> Path:
    return (path if path.is_absolute() else ROOT / path).resolve()


def prepare_control_source(revision: str, destination: Path, archive_path: Path) -> dict:
    """Extract immutable build inputs inside the checkout without changing Git state."""
    paths = git("ls-tree", "-r", "--name-only", revision, "--", *CONTROL_SOURCE_PATHS).splitlines()
    required = {"Cargo.toml", "Cargo.lock", "src/lib.rs", BENCHMARK_MANIFEST.as_posix()}
    if missing := sorted(required.difference(paths)):
        raise RuntimeError(f"control revision omits required build inputs: {missing}")
    archived = subprocess.run(
        ["git", "archive", "--format=tar", revision, "--", *paths],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout
    if archive_path.exists() and archive_path.read_bytes() != archived:
        raise RuntimeError("control source archive differs; use a fresh output directory")
    archive_path.write_bytes(archived)
    with tarfile.open(fileobj=io.BytesIO(archived), mode="r:") as source:
        source.extractall(destination, filter="data")
    files = {
        path.relative_to(destination).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(destination.rglob("*"))
        if path.is_file()
    }
    return {
        "revision": revision,
        "tree": git("rev-parse", f"{revision}^{{tree}}"),
        "archive": str(archive_path.relative_to(ROOT)),
        "archive_sha256": hashlib.sha256(archived).hexdigest(),
        "paths": paths,
        "files": files,
    }


def build_control(source: Path, target_dir: Path, rustflags: str) -> Path:
    cargo_env = os.environ.copy()
    cargo_env["CARGO_TARGET_DIR"] = str(target_dir)
    cargo_env["RUSTFLAGS"] = rustflags
    toolchain = os.environ.get("RUST_TOOLCHAIN")
    cargo_command = ["cargo"]
    if toolchain:
        cargo_command.append(f"+{toolchain}")
    command = [
        *cargo_command,
        "build",
        "--manifest-path",
        str(source / BENCHMARK_MANIFEST),
        "--release",
        "--locked",
    ]
    subprocess.run(command, cwd=source, env=cargo_env, check=True)
    binary = target_dir / "release" / BINARY_NAME
    if not binary.is_file():
        raise RuntimeError(f"baseline benchmark binary was not produced: {binary}")
    return binary


def run_sample(binary: Path, case: dict[str, object], matrix) -> dict[str, str]:
    command = matrix.encode_arguments(binary, case)
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"benchmark command failed ({completed.returncode}): {completed.stderr.strip()}"
        )
    return matrix.parse_report(completed.stdout)


def emit_output(binary: Path, case: dict[str, object], destination: Path, matrix) -> bytes:
    command = matrix.encode_arguments(binary, case)
    command[1] = "emit"
    command[-1] = str(destination)
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"exact-output command failed ({completed.returncode}): {completed.stderr.strip()}"
        )
    return destination.read_bytes()


def load_average() -> list[float] | None:
    try:
        return list(os.getloadavg())
    except (AttributeError, OSError):
        return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-revision", required=True)
    parser.add_argument("--sse2-target-dir", type=Path, required=True)
    parser.add_argument("--avx2-target-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cases", nargs="+", default=CASES, help="exact matrix workload names")
    parser.add_argument("--cohorts", type=int, default=1, help="A-B-B-A quartets per workload (1–20)")
    args = parser.parse_args()
    if not 1 <= args.cohorts <= 20:
        parser.error("--cohorts must be between 1 and 20")
    if len(set(args.cases)) != len(args.cases):
        parser.error("--cases cannot repeat a workload")

    candidate_revision = git("rev-parse", "HEAD")
    if git("status", "--porcelain"):
        raise RuntimeError("candidate checkout must be clean for revision comparison")
    baseline_revision = git("rev-parse", "--verify", f"{args.baseline_revision}^{{commit}}")
    if baseline_revision != args.baseline_revision:
        raise RuntimeError("baseline revision must be an immutable commit ID")

    matrix = load_benchmark_matrix()
    case_by_name = {case["case"]: case for case in matrix.CASES}
    missing_cases = sorted(set(args.cases).difference(case_by_name))
    if missing_cases:
        raise RuntimeError(f"benchmark matrix is missing required cases: {missing_cases}")

    candidate_targets = {
        "sse2": resolve_path(args.sse2_target_dir),
        "avx2": resolve_path(args.avx2_target_dir),
    }
    for variant, target_dir in candidate_targets.items():
        binary = target_dir / "release" / BINARY_NAME
        if not binary.is_file():
            raise RuntimeError(f"{variant} candidate benchmark binary is missing: {binary}")

    output = resolve_path(args.output)
    if not output.is_relative_to(ROOT):
        raise RuntimeError("benchmark output must stay inside the repository")
    output.parent.mkdir(parents=True, exist_ok=True)
    build_root = output.parent / "control-targets"
    build_root.mkdir(parents=True, exist_ok=True)

    started_at = datetime.now(timezone.utc).isoformat()
    records: list[dict[str, object]] = []
    summaries: list[dict[str, object]] = []
    exact_outputs: list[dict[str, object]] = []
    cohort_summaries: list[dict[str, object]] = []
    binary_identities: dict[str, dict[str, str]] = {}
    with tempfile.TemporaryDirectory(prefix="jpeg-abba-", dir=output.parent) as temporary:
        control_source = Path(temporary) / "control"
        control_receipt = prepare_control_source(
            baseline_revision, control_source, output.parent / "control-source.tar"
        )

        for variant, rustflags in VARIANTS.items():
            control_binary = build_control(
                control_source,
                build_root / variant,
                rustflags,
            )
            candidate_binary = candidate_targets[variant] / "release" / BINARY_NAME
            binary_identities[variant] = {
                "control": hashlib.sha256(control_binary.read_bytes()).hexdigest(),
                "candidate": hashlib.sha256(candidate_binary.read_bytes()).hexdigest(),
            }
            for case_name in args.cases:
                case = case_by_name[case_name]
                exact_dir = output.parent / "correctness" / variant
                exact_dir.mkdir(parents=True, exist_ok=True)
                control_output = exact_dir / f"{case_name}-control.jpg"
                candidate_output = exact_dir / f"{case_name}-candidate.jpg"
                control_bytes = emit_output(control_binary, case, control_output, matrix)
                candidate_bytes = emit_output(candidate_binary, case, candidate_output, matrix)
                if control_bytes != candidate_bytes:
                    raise RuntimeError(
                        f"baseline and candidate JPEG bytes differ for {variant}/{case_name}"
                    )
                exact_outputs.append(
                    {
                        "variant": variant,
                        "case": case_name,
                        "bytes": len(control_bytes),
                        "sha256": hashlib.sha256(control_bytes).hexdigest(),
                        "byte_for_byte_match": True,
                    }
                )

                case_cohorts = []
                for cohort in range(1, args.cohorts + 1):
                    samples: dict[str, list[dict[str, str]]] = {
                        "control": [],
                        "candidate": [],
                    }
                    for order, subject in enumerate(
                        ("control", "candidate", "candidate", "control"), start=1
                    ):
                        binary = control_binary if subject == "control" else candidate_binary
                        load_before = load_average()
                        sample_started = time.time_ns()
                        report = run_sample(binary, case, matrix)
                        sample_finished = time.time_ns()
                        records.append(
                            {
                                "variant": variant,
                                "case": case_name,
                                "cohort": cohort,
                                "order": order,
                                "subject": subject,
                                "revision": baseline_revision
                                if subject == "control"
                                else candidate_revision,
                                "binary": str(binary),
                                "binary_sha256": binary_identities[variant][subject],
                                "command": matrix.encode_arguments(binary, case),
                                "load_before": load_before,
                                "load_after": load_average(),
                                "started_unix_ns": sample_started,
                                "finished_unix_ns": sample_finished,
                                "report": report,
                            }
                        )
                        samples[subject].append(report)

                    reports = samples["control"] + samples["candidate"]
                    fields = ("input_fnv1a", "output_bytes", "output_fnv1a")
                    if any(field not in report for report in reports for field in fields):
                        raise RuntimeError("timed sample omits input/output identity")
                    identities = {tuple(report[field] for field in fields) for report in reports}
                    if len(identities) != 1:
                        raise RuntimeError(
                            f"baseline and candidate output differ for {variant}/{case_name}"
                        )
                    control_ns = int(
                        statistics.median(
                            int(report["median_ns"]) for report in samples["control"]
                        )
                    )
                    candidate_ns = int(
                        statistics.median(
                            int(report["median_ns"]) for report in samples["candidate"]
                        )
                    )
                    measured = {
                        "variant": variant,
                        "case": case_name,
                        "cohort": cohort,
                        "control_median_ns": control_ns,
                        "candidate_median_ns": candidate_ns,
                        "candidate_over_control": candidate_ns / control_ns,
                    }
                    cohort_summaries.append(measured)
                    case_cohorts.append(measured)

                ratio = statistics.median(
                    cohort["candidate_over_control"] for cohort in case_cohorts
                )
                summaries.append(
                    {
                        "variant": variant,
                        "case": case_name,
                        "iterations_per_sample": case["iterations"],
                        "cohorts": args.cohorts,
                        "control_median_ns": int(
                            statistics.median(
                                cohort["control_median_ns"] for cohort in case_cohorts
                            )
                        ),
                        "candidate_median_ns": int(
                            statistics.median(
                                cohort["candidate_median_ns"] for cohort in case_cohorts
                            )
                        ),
                        "candidate_over_control": ratio,
                        "paired_gain_percent": 100 * (1 - ratio),
                        "speedup": 1 / ratio,
                        "cohorts_faster": sum(
                            cohort["candidate_over_control"] < 1 for cohort in case_cohorts
                        ),
                        "matching_input_and_output_identity": True,
                    }
                )

    completed_at = datetime.now(timezone.utc).isoformat()
    report = {
        "schema": "image-slash-star/jpeg-revision-abba@2",
        "status": "completed",
        "scope": "public JPEG encoder, same-host interleaved A-B-B-A",
        "started_at_utc": started_at,
        "finished_at_utc": completed_at,
        "github_run_id": os.environ.get("GITHUB_RUN_ID"),
        "baseline_revision": baseline_revision,
        "candidate_revision": candidate_revision,
        "candidate_dirty": False,
        "host": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "cpu": matrix.cpu_brand(),
            "rustc": matrix.capture(["rustc", "-Vv"]),
        },
        "variants": VARIANTS,
        "workloads": args.cases,
        "cohorts_per_workload": args.cohorts,
        "aggregation": "median of within-cohort candidate/control ratios",
        "control_source": control_receipt,
        "binary_sha256": binary_identities,
        "correctness_policy": "Baseline and candidate JPEG outputs are byte-for-byte compared before timing; x86 coverage CI separately runs exact Pillow parity cases.",
        "exact_outputs": exact_outputs,
        "summaries": summaries,
        "cohort_summaries": cohort_summaries,
        "samples": records,
    }
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    for summary in summaries:
        print(
            "{variant}/{case}: control={control_median_ns}ns candidate={candidate_median_ns}ns "
            "candidate/control={candidate_over_control:.4f} speedup={speedup:.4f}x".format(
                **summary
            )
        )
    print(f"Wrote matched A-B-B-A evidence to {output}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"benchmark error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
