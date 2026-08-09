#!/usr/bin/env python3
"""Run revision-bound fixture and artifact measurements.

This is a measurement protocol, not a parity oracle and not a unit-test
replacement.  The parity workload runs the generated Pillow-result matrix;
the Rust-only workload runs the existing feature-gate contracts separately.
Both workloads are pinned to the current clean revision and the current
manifest/matrix hashes in the emitted JSON.  The native release and WASM
compile workloads record artifact sizes, while resource fields remain null
when the host's ``time`` implementation does not expose them.

The result is intentionally an observation for one host, toolchain, cache,
and feature selection.  It is suitable for comparing two revision-bound runs
with the same protocol, not for a universal speed or memory claim.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
MANIFEST_PATH = ROOT / "manifest.yaml"
MATRIX_PATH = ROOT / "tests" / "fixtures" / "coverage_matrix.json"
SCHEMA = "image-slash-star/fixture-benchmark@3"
# Keep the harness parallel enough to represent normal local execution while
# fixing its fan-out across revisions and hosts. The command records this
# value, so comparisons never silently change their test-thread budget.
BENCHMARK_TEST_THREADS = "4"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def command_output(command: list[str]) -> str:
    result = subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def matrix_summary() -> dict:
    document = json.loads(MATRIX_PATH.read_text(encoding="utf-8"))
    summary = document.get("summary")
    if not isinstance(summary, dict):
        raise RuntimeError("coverage matrix has no summary object")
    active = {}
    for format_name, format_data in document.get("formats", {}).items():
        if not isinstance(format_data, dict):
            continue
        active[format_name] = {
            operation: sum(
                1
                for row in format_data.get(operation, [])
                if isinstance(row, dict) and row.get("status") == "active"
            )
            for operation in ("decode", "encode")
        }
    return {
        "summary": summary,
        "active_rows_by_format": active,
    }


def _peak_resident_bytes(max_rss: int) -> int:
    """Normalize ``wait4``'s platform-specific ``ru_maxrss`` unit."""

    # Darwin reports bytes; Linux and the other POSIX platforms report KiB.
    return int(max_rss) if sys.platform == "darwin" else int(max_rss) * 1024


def _unmeasured_resources() -> dict:
    return {
        "reported_user_seconds": None,
        "reported_sys_seconds": None,
        "peak_resident_bytes": None,
        "peak_resident_source": None,
    }


def run_measured_command(
    command: list[str], stdout_path: Path, stderr_path: Path
) -> tuple[int, dict]:
    """Run one workload and collect direct-child POSIX resource usage.

    ``/usr/bin/time -l`` is not portable and can be denied by the macOS
    sandbox while querying sysctls.  ``wait4`` gives the parent the direct
    child's peak RSS and CPU usage without depending on that host utility.
    Non-POSIX hosts retain the benchmark's timing behavior and report the
    resource fields as unavailable rather than fabricating values.
    """

    if not (hasattr(os, "fork") and hasattr(os, "wait4")):
        with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
            result = subprocess.run(
                command,
                cwd=ROOT,
                stdout=stdout,
                stderr=stderr,
                check=False,
            )
        return result.returncode, _unmeasured_resources()

    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        pid = os.fork()
        if pid == 0:
            try:
                os.chdir(ROOT)
                os.dup2(stdout.fileno(), 1)
                os.dup2(stderr.fileno(), 2)
                os.execvpe(command[0], command, os.environ.copy())
            except BaseException as error:  # pragma: no cover - child fallback
                message = f"benchmark child exec failed: {error}\n".encode()
                os.write(2, message)
                os._exit(127)

        _, status, usage = os.wait4(pid, 0)

    return os.waitstatus_to_exitcode(status), {
        "reported_user_seconds": round(usage.ru_utime, 6),
        "reported_sys_seconds": round(usage.ru_stime, 6),
        "peak_resident_bytes": _peak_resident_bytes(usage.ru_maxrss),
        "peak_resident_source": "posix_wait4_direct_child",
    }


def run_workload(workload_id: str, provenance: str, command: list[str]) -> dict:
    started = time.perf_counter()
    with tempfile.TemporaryDirectory(prefix="image-slash-star-benchmark-") as temp_dir:
        stdout_path = Path(temp_dir) / "stdout.log"
        stderr_path = Path(temp_dir) / "stderr.log"
        exit_code, measurements = run_measured_command(command, stdout_path, stderr_path)
        timing_text = stderr_path.read_text(encoding="utf-8", errors="replace")
        stdout_text = stdout_path.read_text(encoding="utf-8", errors="replace")
    elapsed = time.perf_counter() - started
    measurements["reported_real_seconds"] = round(elapsed, 6)
    record = {
        "id": workload_id,
        "provenance": provenance,
        "command": command,
        "status": "passed" if exit_code == 0 else "failed",
        "exit_code": exit_code,
        "wall_seconds": round(elapsed, 6),
        **measurements,
    }
    if exit_code != 0:
        record["stdout_tail"] = stdout_text[-2000:]
        record["stderr_tail"] = timing_text[-4000:]
    return record


def artifact_sizes(paths: list[Path]) -> list[dict]:
    result = []
    seen: set[Path] = set()
    for path in paths:
        if path in seen or not path.is_file():
            continue
        seen.add(path)
        result.append(
            {
                "path": str(path.relative_to(ROOT)),
                "size_bytes": path.stat().st_size,
            }
        )
    return sorted(result, key=lambda entry: entry["path"])


def release_artifacts() -> list[dict]:
    candidates = sorted((ROOT / "target" / "release").glob("libimage_slash_star*"))
    return artifact_sizes([path for path in candidates if path.suffix in {".rlib", ".a", ".dylib", ".so"}])


def wasm_artifacts() -> list[dict]:
    candidates = sorted(
        (ROOT / "target" / "wasm32-unknown-unknown" / "debug" / "deps").glob(
            "determinism_tests-*.wasm"
        )
    )
    return artifact_sizes(candidates)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--workload",
        action="append",
        choices=("parity", "non-parity", "release", "wasm", "all"),
        help="run only the selected workload class; repeat for multiple classes",
    )
    parser.add_argument("--output", type=Path, help="write JSON to this path instead of stdout")
    parser.add_argument(
        "--allow-dirty",
        action="store_true",
        help="permit a dirty worktree; the result will record dirty=true",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    selected = set(args.workload or ["all"])
    if "all" in selected:
        selected = {"parity", "non-parity", "release", "wasm"}

    status_text = command_output(["git", "status", "--porcelain", "--untracked-files=all"])
    dirty = bool(status_text)
    if dirty and not args.allow_dirty:
        print(
            "benchmark requires a clean worktree; use --allow-dirty to record a non-release observation",
            file=sys.stderr,
        )
        return 2

    matrix = matrix_summary()
    result = {
        "schema": SCHEMA,
        "source_revision": command_output(["git", "rev-parse", "HEAD"]),
        "dirty": dirty,
        "host": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
            "python": platform.python_version(),
            "rustc": command_output(["rustc", "-Vv"]),
            "cargo": command_output(["cargo", "-V"]),
        },
        "inputs": {
            "manifest_path": str(MANIFEST_PATH.relative_to(ROOT)),
            "manifest_sha256": sha256(MANIFEST_PATH),
            "matrix_path": str(MATRIX_PATH.relative_to(ROOT)),
            "matrix_sha256": sha256(MATRIX_PATH),
            "matrix_summary": matrix,
        },
        "evidence_policy": {
            "pillow_parity": "The parity workload measures only the existing Pillow-observable fixture matrix.",
            "rust_non_parity": "The non-parity workload measures existing feature-gated Rust contracts; it adds no parity claim.",
            "resource_measurement": "Peak RSS and CPU time are direct-child POSIX wait4 observations; other resource dimensions remain unmeasured.",
            "observation": "Timings and memory are host/cache/toolchain observations, not universal performance claims.",
        },
        "workloads": [],
        "unmeasured_dimensions": [
            "allocation_count",
            "retained_encoded_and_decoded_cache_bytes",
            "caller_buffer_reuse",
            "peak_stack_and_recursion_depth",
            "wasm_runtime_time_and_memory",
        ],
    }

    workloads = [
        (
            "parity",
            "pillow_parity_fixture_suite",
            "pillow_parity",
            [
                "cargo",
                "test",
                "--locked",
                "--all-features",
                "--test",
                "coverage_matrix_tests",
                "--",
                f"--test-threads={BENCHMARK_TEST_THREADS}",
            ],
        ),
        (
            "non-parity",
            "rust_non_parity_feature_gate_suite",
            "rust_only_feature_gate",
            [
                "cargo",
                "test",
                "--locked",
                "--all-features",
                "--test",
                "feature_gate_tests",
                "--",
                f"--test-threads={BENCHMARK_TEST_THREADS}",
            ],
        ),
        (
            "release",
            "native_all_features_release_build",
            "compiled_artifact",
            ["cargo", "build", "--locked", "--all-features", "--release"],
        ),
        (
            "wasm",
            "wasm32_unknown_unknown_determinism_compile",
            "wasm_compile_artifact",
            [
                "cargo",
                "test",
                "--locked",
                "--all-features",
                "--target",
                "wasm32-unknown-unknown",
                "--test",
                "determinism_tests",
                "--no-run",
            ],
        ),
    ]
    for kind, workload_id, provenance, command in workloads:
        if kind not in selected:
            continue
        record = run_workload(workload_id, provenance, command)
        if kind == "release":
            record["artifacts"] = release_artifacts()
        elif kind == "wasm":
            record["artifacts"] = wasm_artifacts()
        result["workloads"].append(record)

    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
        print(f"wrote {args.output}")
    else:
        print(encoded, end="")
    return 0 if all(item["status"] == "passed" for item in result["workloads"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
