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
import platform
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
MANIFEST_PATH = ROOT / "manifest.yaml"
MATRIX_PATH = ROOT / "tests" / "fixtures" / "coverage_matrix.json"
SCHEMA = "image-slash-star/fixture-benchmark@1"


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


def time_command() -> list[str]:
    # POSIX `-p` is available on both macOS and Linux and preserves the child
    # exit status.  The verbose variants can fail in restricted sandboxes
    # while trying to query host sysctls; peak RSS remains explicitly null
    # unless a future runner supplies it through the same parser.
    return ["/usr/bin/time", "-p"]


def parse_resource_measurements(text: str) -> dict:
    def time_value(label: str) -> float | None:
        patterns = (
            rf"^\s*([0-9]+(?:\.[0-9]+)?)\s+{label}\s*$",
            rf"^\s*{label}\s+([0-9]+(?:\.[0-9]+)?)\s*$",
        )
        for pattern in patterns:
            match = re.search(pattern, text, re.MULTILINE)
            if match:
                return float(match.group(1))
        return None

    peak = re.search(r"maximum resident set size:\s+([0-9]+)", text, re.IGNORECASE)
    if peak is not None:
        peak_bytes = int(peak.group(1))
        peak_source = "usr_bin_time"
    else:
        peak_kib = re.search(
            r"Maximum resident set size \(kbytes\):\s+([0-9]+)",
            text,
            re.IGNORECASE,
        )
        peak_bytes = int(peak_kib.group(1)) * 1024 if peak_kib else None
        peak_source = "usr_bin_time" if peak_kib else None
    return {
        "reported_real_seconds": time_value("real"),
        "reported_user_seconds": time_value("user"),
        "reported_sys_seconds": time_value("sys"),
        "peak_resident_bytes": peak_bytes,
        "peak_resident_source": peak_source,
    }


def run_workload(workload_id: str, provenance: str, command: list[str]) -> dict:
    started = time.perf_counter()
    with tempfile.TemporaryDirectory(prefix="image-slash-star-benchmark-") as temp_dir:
        stdout_path = Path(temp_dir) / "stdout.log"
        stderr_path = Path(temp_dir) / "stderr.log"
        with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open(
            "w", encoding="utf-8"
        ) as stderr:
            result = subprocess.run(
                time_command() + command,
                cwd=ROOT,
                stdout=stdout,
                stderr=stderr,
                text=True,
            )
        timing_text = stderr_path.read_text(encoding="utf-8", errors="replace")
        stdout_text = stdout_path.read_text(encoding="utf-8", errors="replace")
    elapsed = time.perf_counter() - started
    measurements = parse_resource_measurements(timing_text)
    record = {
        "id": workload_id,
        "provenance": provenance,
        "command": command,
        "status": "passed" if result.returncode == 0 else "failed",
        "exit_code": result.returncode,
        "wall_seconds": round(elapsed, 6),
        **measurements,
    }
    if result.returncode != 0:
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
            "observation": "Timings and memory are host/cache/toolchain observations, not universal performance claims.",
        },
        "workloads": [],
        "unmeasured_dimensions": [
            "allocation_count",
            "retained_encoded_and_decoded_cache_bytes",
            "caller_buffer_reuse",
            "peak_resident_memory",
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
                "--test-threads=1",
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
                "--test-threads=1",
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
