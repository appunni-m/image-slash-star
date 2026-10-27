#!/usr/bin/env python3
"""Compare JPEG encoder revisions in an interleaved same-host benchmark."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import platform
import statistics
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BENCHMARK_MANIFEST = Path("benchmarks/jpeg-production/rust/Cargo.toml")
BINARY_NAME = "jpeg-production-matrix-rust"
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
    return path if path.is_absolute() else (ROOT / path).resolve()


def build_control(worktree: Path, target_dir: Path, rustflags: str) -> Path:
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
        str(worktree / BENCHMARK_MANIFEST),
        "--release",
        "--locked",
    ]
    subprocess.run(command, cwd=worktree, env=cargo_env, check=True)
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
    args = parser.parse_args()

    candidate_revision = git("rev-parse", "HEAD")
    if git("status", "--porcelain"):
        raise RuntimeError("candidate checkout must be clean for revision comparison")
    baseline_revision = git("rev-parse", "--verify", f"{args.baseline_revision}^{{commit}}")
    if baseline_revision != args.baseline_revision:
        raise RuntimeError("baseline revision must be an immutable commit ID")

    matrix = load_benchmark_matrix()
    case_by_name = {case["case"]: case for case in matrix.CASES}
    missing_cases = sorted(set(CASES).difference(case_by_name))
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
    output.parent.mkdir(parents=True, exist_ok=True)
    build_root = output.parent / "control-targets"
    build_root.mkdir(parents=True, exist_ok=True)

    started_at = datetime.now(timezone.utc).isoformat()
    records: list[dict[str, object]] = []
    summaries: list[dict[str, object]] = []
    exact_outputs: list[dict[str, object]] = []
    temporary_directory = tempfile.TemporaryDirectory(prefix="jpeg-abba-")
    control_worktree = Path(temporary_directory.name) / "control"
    worktree_added = False
    try:
        subprocess.run(
            ["git", "worktree", "add", "--quiet", "--detach", str(control_worktree), baseline_revision],
            cwd=ROOT,
            check=True,
        )
        worktree_added = True

        for variant, rustflags in VARIANTS.items():
            control_binary = build_control(
                control_worktree,
                build_root / variant,
                rustflags,
            )
            candidate_binary = candidate_targets[variant] / "release" / BINARY_NAME
            for case_name in CASES:
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

                samples: dict[str, list[dict[str, str]]] = {
                    "control": [],
                    "candidate": [],
                }
                for order, subject in enumerate(
                    ("control", "candidate", "candidate", "control"),
                    start=1,
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
                            "order": order,
                            "subject": subject,
                            "revision": baseline_revision
                            if subject == "control"
                            else candidate_revision,
                            "binary": str(binary),
                            "command": matrix.encode_arguments(binary, case),
                            "load_before": load_before,
                            "load_after": load_average(),
                            "started_unix_ns": sample_started,
                            "finished_unix_ns": sample_finished,
                            "report": report,
                        }
                    )
                    samples[subject].append(report)

                identities = {
                    tuple(
                        report.get(field, "")
                        for field in ("input_fnv1a", "output_bytes", "output_fnv1a")
                    )
                    for report in samples["control"] + samples["candidate"]
                }
                if len(identities) != 1:
                    raise RuntimeError(
                        f"baseline and candidate output differ for {variant}/{case_name}"
                    )

                control_ns = int(
                    statistics.median(int(report["median_ns"]) for report in samples["control"])
                )
                candidate_ns = int(
                    statistics.median(int(report["median_ns"]) for report in samples["candidate"])
                )
                summaries.append(
                    {
                        "variant": variant,
                        "case": case_name,
                        "iterations_per_sample": case["iterations"],
                        "control_median_ns": control_ns,
                        "candidate_median_ns": candidate_ns,
                        "candidate_over_control": candidate_ns / control_ns,
                        "speedup": control_ns / candidate_ns,
                        "matching_input_and_output_identity": True,
                    }
                )
    finally:
        if worktree_added:
            subprocess.run(
                ["git", "worktree", "remove", "--force", str(control_worktree)],
                cwd=ROOT,
                check=True,
            )
        temporary_directory.cleanup()

    completed_at = datetime.now(timezone.utc).isoformat()
    report = {
        "schema": "image-slash-star/jpeg-revision-abba@1",
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
        "workloads": CASES,
        "correctness_policy": "Baseline and candidate JPEG outputs are byte-for-byte compared before timing; x86 coverage CI separately runs exact Pillow parity cases.",
        "exact_outputs": exact_outputs,
        "summaries": summaries,
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
