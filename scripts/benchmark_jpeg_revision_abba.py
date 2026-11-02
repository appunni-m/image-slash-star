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
import shutil
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
BUILD_SOURCE_PATHS = (
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


def prepare_revision_source(revision: str, destination: Path, archive_path: Path) -> dict:
    """Extract immutable build inputs inside the checkout without changing Git state."""
    paths = git("ls-tree", "-r", "--name-only", revision, "--", *BUILD_SOURCE_PATHS).splitlines()
    required = {"Cargo.toml", "Cargo.lock", "src/lib.rs", BENCHMARK_MANIFEST.as_posix()}
    if missing := sorted(required.difference(paths)):
        raise RuntimeError(f"revision omits required build inputs: {missing}")
    archived = subprocess.run(
        ["git", "archive", "--format=tar", revision, "--", *paths],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout
    if archive_path.exists() and archive_path.read_bytes() != archived:
        raise RuntimeError("source archive differs; use a fresh output directory")
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


def build_revision(
    source: Path, target_dir: Path, rustflags: str, log_path: Path
) -> tuple[Path, dict]:
    cargo_env = os.environ.copy()
    cargo_env["CARGO_TARGET_DIR"] = str(target_dir)
    cargo_env["RUSTFLAGS"] = rustflags
    # The declared variant flags must win over an inherited encoded override.
    cargo_env.pop("CARGO_ENCODED_RUSTFLAGS", None)
    cargo_env["CARGO_INCREMENTAL"] = "0"
    cargo_env["RUSTC_WRAPPER"] = ""
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
    started = datetime.now(timezone.utc).isoformat()
    log_path.parent.mkdir(parents=True, exist_ok=True)
    with log_path.open("w", encoding="utf-8") as log:
        completed = subprocess.run(command, cwd=source, env=cargo_env, stdout=log, stderr=subprocess.STDOUT)
    if completed.returncode != 0:
        raise RuntimeError(
            f"revision build failed ({completed.returncode}); {log_path}:\n"
            f"{log_path.read_text(encoding='utf-8')[-5000:]}"
        )
    binary = target_dir / "release" / BINARY_NAME
    if not binary.is_file():
        raise RuntimeError(f"revision benchmark binary was not produced: {binary}")
    return binary, {
        "command": command,
        "cwd": str(source),
        "rustflags": rustflags,
        "incremental": False,
        "compiler_wrapper": "",
        "log": str(log_path),
        "rustc": subprocess.check_output(
            ["rustc", *([f"+{toolchain}"] if toolchain else []), "-Vv"],
            cwd=source, env=cargo_env, text=True,
        ).strip(),
        "started_at_utc": started,
        "finished_at_utc": datetime.now(timezone.utc).isoformat(),
    }


def build_revision_pair(
    baseline_revision: str,
    candidate_revision: str,
    variant: str,
    rustflags: str,
    temporary_root: Path,
    output: Path,
) -> dict:
    """Build both immutable revisions at one source path, retaining timed binaries."""
    source = temporary_root / "source"
    pair = {}
    for subject, revision in (("control", baseline_revision), ("candidate", candidate_revision)):
        # Only this invocation's private source extraction is replaced. Each
        # subject gets a fresh target directory so archive mtimes cannot make
        # Cargo reuse the other revision's executable.
        if source.exists():
            shutil.rmtree(source)
        receipt = prepare_revision_source(revision, source, output / f"{subject}-source.tar")
        binary, build = build_revision(
            source, temporary_root / "targets" / variant / subject, rustflags,
            output / "build-logs" / variant / f"{subject}.log",
        )
        retained = output / "executables" / variant / subject / BINARY_NAME
        retained.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(binary, retained)
        pair[subject] = {
            "source": receipt,
            "build": build,
            "binary": str(retained),
            "binary_sha256": hashlib.sha256(retained.read_bytes()).hexdigest(),
        }
    source_identical = pair["control"]["source"]["files"] == pair["candidate"]["source"]["files"]
    binary_identical = pair["control"]["binary_sha256"] == pair["candidate"]["binary_sha256"]
    if source_identical and not binary_identical:
        raise RuntimeError(
            f"source-identical {variant} builds produced different executables; "
            "investigate build identity before timing"
        )
    pair["source_identical"] = source_identical
    pair["binary_identical"] = binary_identical
    return pair


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
    parser.add_argument(
        "--sse2-target-dir", type=Path,
        help="optional prebuilt SSE2 directory for untimed exact-output comparison",
    )
    parser.add_argument(
        "--avx2-target-dir", type=Path,
        help="optional prebuilt AVX2 directory for untimed exact-output comparison",
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cases", nargs="+", default=CASES, help="exact matrix workload names")
    parser.add_argument("--cohorts", type=int, default=1, help="A-B-B-A quartets per workload (1–20)")
    args = parser.parse_args()
    if not 1 <= args.cohorts <= 20:
        parser.error("--cohorts must be between 1 and 20")
    if len(set(args.cases)) != len(args.cases):
        parser.error("--cases cannot repeat a workload")
    if platform.machine().lower() not in {"x86_64", "amd64"}:
        raise RuntimeError("SSE2/AVX2 revision timing requires a native x86-64 host")

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

    prebuilt_targets = {
        variant: resolve_path(path)
        for variant, path in (("sse2", args.sse2_target_dir), ("avx2", args.avx2_target_dir))
        if path is not None
    }
    for variant, target_dir in prebuilt_targets.items():
        binary = target_dir / "release" / BINARY_NAME
        if not binary.is_file():
            raise RuntimeError(f"{variant} prebuilt benchmark binary is missing: {binary}")

    output = resolve_path(args.output)
    if not output.is_relative_to(ROOT):
        raise RuntimeError("benchmark output must stay inside the repository")
    if output.exists():
        raise RuntimeError("benchmark report already exists; use a fresh output directory")
    if output.parent.exists() and any(output.parent.iterdir()):
        raise RuntimeError("benchmark output directory is not empty; use a fresh directory")
    output.parent.mkdir(parents=True, exist_ok=True)
    producer_sources = {}
    for path in (
        Path(__file__), Path(matrix.__file__), ROOT / ".github/workflows/benchmark-x86.yml"
    ):
        relative = path.resolve().relative_to(ROOT)
        retained = output.parent / "producer" / relative
        retained.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, retained)
        producer_sources[relative.as_posix()] = hashlib.sha256(retained.read_bytes()).hexdigest()
    prebuilt_binaries = {}
    for variant, target_dir in prebuilt_targets.items():
        retained = output.parent / "executables" / variant / "prebuilt" / BINARY_NAME
        retained.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(target_dir / "release" / BINARY_NAME, retained)
        prebuilt_binaries[variant] = {
            "binary": str(retained),
            "binary_sha256": hashlib.sha256(retained.read_bytes()).hexdigest(),
        }

    started_at = datetime.now(timezone.utc).isoformat()
    records: list[dict[str, object]] = []
    summaries: list[dict[str, object]] = []
    exact_outputs: list[dict[str, object]] = []
    cohort_summaries: list[dict[str, object]] = []
    binary_identities: dict[str, dict[str, str]] = {}
    revision_builds = {}
    with tempfile.TemporaryDirectory(prefix="jpeg-abba-", dir=output.parent) as temporary:
        for variant, rustflags in VARIANTS.items():
            pair = build_revision_pair(
                baseline_revision, candidate_revision, variant, rustflags,
                Path(temporary), output.parent,
            )
            revision_builds[variant] = pair
            control_binary = Path(pair["control"]["binary"])
            candidate_binary = Path(pair["candidate"]["binary"])
            binary_identities[variant] = {
                subject: pair[subject]["binary_sha256"] for subject in ("control", "candidate")
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
                prebuilt_match = None
                if variant in prebuilt_binaries:
                    prebuilt_output = exact_dir / f"{case_name}-prebuilt.jpg"
                    prebuilt_bytes = emit_output(
                        Path(prebuilt_binaries[variant]["binary"]), case, prebuilt_output, matrix
                    )
                    if prebuilt_bytes != candidate_bytes:
                        raise RuntimeError(
                            f"prebuilt and archived candidate JPEG bytes differ for {variant}/{case_name}"
                        )
                    prebuilt_match = True
                exact_outputs.append(
                    {
                        "variant": variant,
                        "case": case_name,
                        "bytes": len(control_bytes),
                        "sha256": hashlib.sha256(control_bytes).hexdigest(),
                        "byte_for_byte_match": True,
                        "prebuilt_matches_archived_candidate": prebuilt_match,
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
        "schema": "image-slash-star/jpeg-revision-abba@3",
        "status": "completed",
        "scope": "public JPEG encoder, same-host interleaved A-B-B-A",
        "started_at_utc": started_at,
        "finished_at_utc": completed_at,
        "github_run_id": os.environ.get("GITHUB_RUN_ID"),
        "runner_argv": sys.orig_argv,
        "runner_cwd": str(ROOT),
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
        "build_policy": "both Git archives at one absolute source path, independent fresh target directories",
        "producer_sources": producer_sources,
        "revision_builds": revision_builds,
        "prebuilt_binaries": prebuilt_binaries,
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
