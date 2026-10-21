#!/usr/bin/env python3
"""Run exact coverage-matrix cases and retain local, unverified evidence."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path, PurePosixPath
import re
import shlex
import subprocess
import sys
import uuid
from typing import Any, Optional

ROOT = Path(__file__).resolve().parent.parent
MATRIX_PATH = Path("tests/fixtures/coverage_matrix.json")
SELECTOR_PREFIX = "__image_slash_star_matrix_row_selector__="
RECEIPT_SCHEMA = "image-slash-star.selected-coverage.local-evidence.v1"
REPORT_DIR = Path(".coverage-mcp/local-evidence/selected")

SELECTION_LINE = re.compile(
    r"^matrix row selection: selected=(\d+) active=(\d+) "
    r"planned-not-executed=(\d+); canonical=(.*)$",
    re.MULTILINE,
)
EXECUTED_LINE = re.compile(r"^MATRIX_ROW_EXECUTING (\S+)$", re.MULTILINE)
PASSED_LINE = re.compile(r"^MATRIX_ROW_PASSED (\S+)$", re.MULTILINE)
DECODE_SUMMARY = re.compile(
    r"^decode matrix \(selected\): (\d+)/(\d+) active rows passed, "
    r"(\d+) failed, (\d+) planned-not-executed$",
    re.MULTILINE,
)
ENCODE_SUMMARY = re.compile(
    r"^encode matrix \(selected\): (\d+)/(\d+) active rows passed, "
    r"(\d+) failed, (\d+) planned-not-executed(?:;.*)?$",
    re.MULTILINE,
)
FAULT_SUMMARY = re.compile(
    r"^Fault contracts: selected=(\d+) executed=(\d+) passed=(\d+) "
    r"failed=(\d+) planned-not-executed=(\d+) oracle=not_applicable$",
    re.MULTILINE,
)


@dataclass(frozen=True)
class SelectedRow:
    key: str
    operation: str
    format: str
    row_id: str
    status: str
    row: dict[str, Any]


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def parse_case_list(value: str) -> list[str]:
    cases = value.split()
    if not cases:
        raise ValueError("provide one or more exact operation:format:row-id selectors")
    seen: set[str] = set()
    for case in cases:
        fields = case.split(":")
        if (
            len(fields) != 3
            or fields[0] not in {"decode", "encode", "fault-contract"}
            or not fields[1]
            or not fields[2]
            or any(character.isspace() for character in case)
            or "," in case
            or "*" in case
        ):
            raise ValueError(
                f"malformed selector {case!r}; expected exact operation:format:row-id"
            )
        if case in seen:
            raise ValueError(f"duplicate selector {case!r}")
        seen.add(case)
    return cases


def load_selected_rows(cases: list[str], matrix_path: Path = ROOT / MATRIX_PATH) -> list[SelectedRow]:
    matrix = json.loads(matrix_path.read_text(encoding="utf-8"))
    if not isinstance(matrix, dict) or not isinstance(matrix.get("formats"), dict):
        raise ValueError("coverage matrix must contain a formats object")

    catalog: dict[str, SelectedRow] = {}
    for format_name, format_data in matrix["formats"].items():
        if not isinstance(format_name, str) or not isinstance(format_data, dict):
            raise ValueError(f"coverage matrix format {format_name!r} is invalid")
        for operation in ("decode", "encode"):
            rows = format_data.get(operation, [])
            if not isinstance(rows, list):
                raise ValueError(f"coverage matrix {operation}:{format_name} rows are invalid")
            for row in rows:
                if not isinstance(row, dict) or not isinstance(row.get("id"), str):
                    raise ValueError(f"coverage matrix contains an invalid {operation} row")
                if row.get("format") != format_name:
                    raise ValueError(f"matrix row {row['id']!r} has a mismatched format")
                key = f"{operation}:{format_name}:{row['id']}"
                if key in catalog:
                    raise ValueError(f"coverage matrix contains duplicate selector {key!r}")
                catalog[key] = SelectedRow(
                    key, operation, format_name, row["id"], _status(row, key), row
                )

    fault_rows = matrix.get("fault_contracts", [])
    if not isinstance(fault_rows, list):
        raise ValueError("coverage matrix fault_contracts must be an array")
    for row in fault_rows:
        if not isinstance(row, dict) or not all(
            isinstance(row.get(field), str) for field in ("format", "id")
        ):
            raise ValueError("coverage matrix contains an invalid fault-contract row")
        key = f"fault-contract:{row['format']}:{row['id']}"
        if key in catalog:
            raise ValueError(f"coverage matrix contains duplicate selector {key!r}")
        catalog[key] = SelectedRow(
            key, "fault-contract", row["format"], row["id"], _status(row, key), row
        )

    selected: list[SelectedRow] = []
    for key in cases:
        row = catalog.get(key)
        if row is None:
            raise ValueError(f"unknown selector {key!r}")
        selected.append(row)
    if not any(row.status == "active" for row in selected):
        planned = ", ".join(row.key for row in selected)
        raise ValueError(f"planned-only selection cannot execute: {planned}")
    return selected


def _status(row: dict[str, Any], key: str) -> str:
    status = row.get("status")
    if not isinstance(status, str) or status not in {"active", "planned"}:
        raise ValueError(f"matrix row {key!r} has unsupported status {status!r}")
    return status


def build_command(
    toolchain: str, report: Path, cases: list[str], target: str
) -> list[str]:
    command = [
        "cargo",
        f"+{toolchain}",
        "llvm-cov",
        "test",
        "--all-features",
        "--target",
        target,
        "--branch",
        "--locked",
        "--json",
        "--output-path",
        str(report),
        "--test",
        "coverage_matrix_tests",
        "test_coverage_matrix",
        "--",
        "--exact",
        "--nocapture",
    ]
    for case in cases:
        command.extend(("--skip", f"{SELECTOR_PREFIX}{case}"))
    return command


def parse_harness_output(output: str, selected: list[SelectedRow]) -> dict[str, Any]:
    matches = list(SELECTION_LINE.finditer(output))
    if len(matches) != 1:
        raise ValueError(f"expected one canonical selection line; found {len(matches)}")
    match = matches[0]
    selected_count, active_count, planned_count = map(int, match.groups()[:3])
    canonical_text = match.group(4)
    canonical_rows: list[tuple[str, str]] = []
    if canonical_text:
        for item in canonical_text.split(", "):
            row_match = re.fullmatch(r"(\S+) \[(active|planned-not-executed)\]", item)
            if row_match is None:
                raise ValueError(f"invalid canonical selector item {item!r}")
            canonical_rows.append((row_match.group(1), row_match.group(2)))

    expected_rows = [
        (row.key, "active" if row.status == "active" else "planned-not-executed")
        for row in selected
    ]
    if len(canonical_rows) != len(set(canonical_rows)):
        raise ValueError("Rust selector resolution emitted duplicate canonical rows")
    if set(canonical_rows) != set(expected_rows):
        raise ValueError(
            "Rust selector resolution differs from request: "
            f"{canonical_rows!r} != {expected_rows!r}"
        )
    expected_active = sum(row.status == "active" for row in selected)
    expected_planned = len(selected) - expected_active
    if (selected_count, active_count, planned_count) != (
        len(selected),
        expected_active,
        expected_planned,
    ):
        raise ValueError("Rust selector counts differ from the canonical matrix")

    executed = EXECUTED_LINE.findall(output)
    if len(executed) != len(set(executed)):
        raise ValueError("Rust harness emitted a duplicate execution marker")
    expected_executed = {row.key for row in selected if row.status == "active"}
    executed_set = set(executed)
    passed = PASSED_LINE.findall(output)
    if len(passed) != len(set(passed)):
        raise ValueError("Rust harness emitted a duplicate passing-result marker")
    passed_set = set(passed)
    if not passed_set.issubset(executed_set):
        raise ValueError("Rust harness reported a pass for a row without an execution marker")

    operations: dict[str, Any] = {}
    for operation, pattern in (
        ("decode", DECODE_SUMMARY),
        ("encode", ENCODE_SUMMARY),
    ):
        rows = [row for row in selected if row.operation == operation]
        if not rows:
            continue
        summaries = list(pattern.finditer(output))
        if len(summaries) != 1:
            raise ValueError(
                f"expected one {operation} summary for selected rows; found {len(summaries)}"
            )
        passed_count, total, failed_count, planned = map(int, summaries[0].groups())
        expected_total = sum(row.status == "active" for row in rows)
        expected_planned = len(rows) - expected_total
        if (total, planned) != (expected_total, expected_planned):
            raise ValueError(f"{operation} summary counts differ from selected rows")
        if passed_count + failed_count != total:
            raise ValueError(f"{operation} summary has inconsistent pass/fail totals")
        operation_keys = {row.key for row in rows if row.status == "active"}
        operation_passed = passed_set.intersection(operation_keys)
        if len(operation_passed) != passed_count:
            raise ValueError(f"{operation} pass markers differ from summary count")
        operations[operation] = {
            "selected": len(rows),
            "executed": total,
            "passed": passed_count,
            "failed": failed_count,
            "planned_not_executed": planned,
        }

    fault_rows = [row for row in selected if row.operation == "fault-contract"]
    if fault_rows:
        summaries = list(FAULT_SUMMARY.finditer(output))
        if len(summaries) != 1:
            raise ValueError(
                f"expected one fault-contract summary; found {len(summaries)}"
            )
        selected_total, executed_total, passed_total, failed_total, planned_total = map(
            int, summaries[0].groups()
        )
        expected_fault_active = sum(row.status == "active" for row in fault_rows)
        expected_fault_planned = len(fault_rows) - expected_fault_active
        if (selected_total, executed_total, planned_total) != (
            len(fault_rows),
            expected_fault_active,
            expected_fault_planned,
        ):
            raise ValueError("fault-contract summary counts differ from selected rows")
        if passed_total + failed_total != executed_total:
            raise ValueError("fault-contract summary has inconsistent pass/fail totals")
        fault_keys = {row.key for row in fault_rows if row.status == "active"}
        fault_passed = passed_set.intersection(fault_keys)
        if len(fault_passed) != passed_total:
            raise ValueError("fault-contract pass markers differ from summary count")
        operations["fault-contract"] = {
            "selected": selected_total,
            "executed": executed_total,
            "passed": passed_total,
            "failed": failed_total,
            "planned_not_executed": planned_total,
            "oracle": "not_applicable",
        }

    planned_keys = [row.key for row in selected if row.status == "planned"]
    if not executed_set.issubset(expected_executed):
        unexpected = sorted(executed_set - expected_executed)
        raise ValueError(f"Rust harness executed unrequested rows: {unexpected}")
    if executed_set != expected_executed:
        missing = sorted(expected_executed - executed_set)
        raise ValueError(f"Rust harness did not execute all selected active rows: {missing}")

    passed_count = len(passed_set)
    failed_count = expected_active - passed_count
    return {
        "selected_case_ids": [row.key for row in selected],
        "executed_case_ids": executed,
        "passed_case_ids": sorted(passed_set),
        "failed_case_ids": sorted(expected_executed - passed_set),
        "planned_not_executed_case_ids": planned_keys,
        "counts": {
            "selected": len(selected),
            "executed": len(executed_set),
            "passed": passed_count,
            "failed": failed_count,
            "planned_not_executed": len(planned_keys),
        },
        "operations": operations,
    }


def _git(args: list[str]) -> str:
    return subprocess.run(
        ["git", *args], cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout


def relevant_source_paths() -> list[Path]:
    candidates = _git(["ls-files", "-co", "--exclude-standard", "-z"]).split("\0")
    selected: set[Path] = set()
    for candidate in candidates:
        if not candidate:
            continue
        path = PurePosixPath(candidate)
        is_source = (
            (path.parts[0] == "src" and path.suffix == ".rs")
            or candidate == "tests/coverage_matrix_tests.rs"
            or (candidate.startswith("tests/support/") and path.suffix == ".rs")
            or candidate == "scripts/run_selected_coverage.py"
            or candidate in {"Cargo.toml", "Cargo.lock", "build.rs", "rust-toolchain.toml"}
            or candidate.startswith(".cargo/")
        )
        if is_source and (ROOT / candidate).is_file():
            selected.add(Path(candidate))
    return sorted(selected, key=lambda path: path.as_posix())


def source_state() -> dict[str, Any]:
    status = _git(["status", "--porcelain=v1", "--untracked-files=all"])
    sources = {
        path.as_posix(): sha256_file(ROOT / path) for path in relevant_source_paths()
    }
    return {
        "revision": _git(["rev-parse", "HEAD"]).strip(),
        "dirty": bool(status),
        "git_status": status,
        "git_status_sha256": sha256_bytes(status.encode()),
        "source_files": sources,
        "source_manifest_sha256": sha256_bytes(
            json.dumps(sources, sort_keys=True, separators=(",", ":")).encode()
        ),
    }


def _collect_fixture_paths(row: SelectedRow) -> set[Path]:
    paths: set[Path] = set()

    def visit(value: Any, format_name: str, source_format: Optional[str]) -> None:
        if isinstance(value, dict):
            nested_format = value.get("format", format_name)
            if not isinstance(nested_format, str):
                nested_format = format_name
            nested_source_format = value.get("source_format", source_format)
            if not isinstance(nested_source_format, str):
                nested_source_format = source_format
            for key, child in value.items():
                if isinstance(child, str) and key.endswith("_path"):
                    paths.add(Path(child))
                elif isinstance(child, str) and key == "asset":
                    paths.add(
                        Path("tests/fixtures/input/images") / nested_format / child
                    )
                elif isinstance(child, str) and key == "source_asset":
                    if nested_source_format is not None:
                        paths.add(
                            Path("tests/fixtures/input/images")
                            / nested_source_format
                            / child
                        )
                visit(child, nested_format, nested_source_format)
        elif isinstance(value, list):
            for child in value:
                visit(child, format_name, source_format)

    visit(row.row, row.format, row.row.get("source_format"))
    return paths


def fixture_hashes(selected: list[SelectedRow]) -> dict[str, Any]:
    paths = {MATRIX_PATH}
    for row in selected:
        paths.update(_collect_fixture_paths(row))
        if row.operation == "fault-contract":
            input_data = row.row.get("input")
            source_case = input_data.get("source_case") if isinstance(input_data, dict) else None
            if isinstance(source_case, str):
                source_rows = load_selected_rows([source_case])
                for source_row in source_rows:
                    paths.update(_collect_fixture_paths(source_row))
    hashes: dict[str, Any] = {}
    for path in sorted(paths, key=lambda item: item.as_posix()):
        absolute = (ROOT / path).resolve()
        if not absolute.is_relative_to(ROOT):
            raise ValueError(f"fixture path escapes repository root: {path}")
        hashes[path.as_posix()] = (
            {"sha256": sha256_file(absolute), "status": "present"}
            if absolute.is_file()
            else {"sha256": None, "status": "missing"}
        )
    return hashes


def discover_toolchain(toolchain: str) -> dict[str, str]:
    rustc = subprocess.run(
        ["rustc", f"+{toolchain}", "--version", "--verbose"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    llvm_cov = subprocess.run(
        ["cargo", f"+{toolchain}", "llvm-cov", "--version"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    host = re.search(r"^host: (\S+)$", rustc, re.MULTILINE)
    if host is None:
        raise ValueError("rustc --version --verbose did not report a host target")
    return {
        "rustc_verbose": rustc.strip(),
        "llvm_cov": llvm_cov,
        "target": host.group(1),
        "target_source": "explicit --target from rustc host",
    }


def report_totals(path: Path) -> dict[str, Any]:
    if not path.is_file() or path.stat().st_size == 0:
        raise ValueError("LLVM coverage report is missing or empty")
    report = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(report, dict) or not isinstance(report.get("data"), list):
        raise ValueError("LLVM coverage report has no data array")
    if len(report["data"]) != 1 or not isinstance(report["data"][0], dict):
        raise ValueError("LLVM coverage report must contain one data object")
    totals = report["data"][0].get("totals")
    if not isinstance(totals, dict):
        raise ValueError("LLVM coverage report has no totals")
    result: dict[str, Any] = {}
    for metric in ("lines", "branches", "functions", "regions"):
        value = totals.get(metric)
        if not isinstance(value, dict):
            raise ValueError(f"LLVM coverage report has no {metric} totals")
        covered = value.get("covered")
        count = value.get("count")
        percent = value.get("percent")
        if (
            not isinstance(covered, int)
            or isinstance(covered, bool)
            or not isinstance(count, int)
            or isinstance(count, bool)
            or count < 0
            or covered < 0
            or covered > count
            or not isinstance(percent, (int, float))
            or isinstance(percent, bool)
            or not math.isfinite(percent)
            or percent < 0
            or percent > 100
        ):
            raise ValueError(f"LLVM coverage report has invalid {metric} totals")
        result[metric] = {
            "covered": covered,
            "count": count,
            "percent": percent,
        }
    return result


def _absolute_report_path(value: Optional[str]) -> Path:
    if value is None:
        stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
        run_id = f"{stamp}-{uuid.uuid4().hex}"
        path = REPORT_DIR / run_id / "coverage.json"
    else:
        path = Path(value)
        if path.is_absolute():
            try:
                path = path.relative_to(ROOT)
            except ValueError as error:
                raise ValueError("coverage reports must be written inside the repository") from error
    absolute = (ROOT / path).resolve()
    if not absolute.is_relative_to(ROOT):
        raise ValueError("coverage report path escapes repository root")
    if absolute.exists():
        raise ValueError(f"refusing to overwrite existing coverage report: {path}")
    return absolute


def _relative_path(path: Path) -> str:
    return path.resolve().relative_to(ROOT).as_posix()


def _write_json(path: Path, value: Any) -> None:
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(path)


def run(toolchain: str, cases: list[str], report: Path) -> int:
    selected = load_selected_rows(cases)
    report.parent.mkdir(parents=True, exist_ok=True)
    log_path = report.with_suffix(".log")
    receipt_path = report.with_name(f"{report.stem}.local-evidence.json")
    if log_path.exists() or receipt_path.exists():
        raise ValueError("refusing to overwrite an existing selected-run log or evidence receipt")

    before = source_state()
    fixture_digests = fixture_hashes(selected)
    versions = discover_toolchain(toolchain)
    argv = build_command(toolchain, report, cases, versions["target"])
    environment = os.environ.copy()
    environment["RUSTC_WRAPPER"] = ""
    environment["CARGO_NET_OFFLINE"] = "true"
    environment["CARGO_TERM_COLOR"] = "never"
    environment["IMAGE_SLASH_STAR_VERBOSE_MATRIX"] = "1"
    environment_identity = {
        key: sha256_bytes(environment[key].encode())
        for key in (
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_BUILD_TARGET",
            "CARGO_TARGET_DIR",
            "CARGO_INCREMENTAL",
            "CFLAGS",
            "CXXFLAGS",
        )
        if environment.get(key)
    }

    receipt: dict[str, Any] = {
        "schema": RECEIPT_SCHEMA,
        "evidence_kind": "local-only; not a Coverage MCP source/build receipt",
        "started_at_utc": datetime.now(timezone.utc).isoformat(),
        "source_before": before,
        "fixtures": fixture_digests,
        "toolchain": {"name": toolchain, **versions},
        "coverage": {
            "features": ["all-features"],
            "instrumentation": ["branch", "llvm-cov"],
            "target": versions["target"],
            "target_source": versions["target_source"],
            "environment_variable_hashes": environment_identity,
        },
        "argv": argv,
        "report": {"path": _relative_path(report), "sha256": None, "totals": None},
        "log": {"path": _relative_path(log_path), "sha256": None},
        "execution": None,
        "coverage_mcp": {
            "status": "not-queried",
            "source": "unverified",
            "tests": "unknown",
            "note": "Query Coverage MCP after this local run; its available interface has no CLI bridge or receipt-registration action.",
        },
    }
    _write_json(receipt_path, receipt)

    output_parts: list[str] = []
    environment_line = " ".join(
        f"{name}={shlex.quote(value)}"
        for name, value in (
            ("RUSTC_WRAPPER", ""),
            ("CARGO_NET_OFFLINE", "true"),
            ("CARGO_TERM_COLOR", "never"),
            ("IMAGE_SLASH_STAR_VERBOSE_MATRIX", "1"),
        )
    )
    print(f"$ {environment_line} {shlex.join(argv)}", flush=True)
    try:
        with log_path.open("w", encoding="utf-8") as log:
            process = subprocess.Popen(
                argv,
                cwd=ROOT,
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
            )
            assert process.stdout is not None
            for line in process.stdout:
                output_parts.append(line)
                log.write(line)
                log.flush()
                print(line, end="", flush=True)
            return_code = process.wait()
    except OSError as error:
        return_code = 127
        output_parts.append(f"selected coverage process could not start: {error}\n")
        log_path.write_text("".join(output_parts), encoding="utf-8")

    output = "".join(output_parts)
    parse_error: Optional[str] = None
    execution: Optional[dict[str, Any]] = None
    try:
        execution = parse_harness_output(output, selected)
        execution["test_process_exit_code"] = return_code
        execution["test_process_passed"] = (
            return_code == 0
            and "test result: ok." in output
            and execution["counts"]["failed"] == 0
        )
    except ValueError as error:
        parse_error = str(error)
        execution = {
            "selected_case_ids": [row.key for row in selected],
            "executed_case_ids": EXECUTED_LINE.findall(output),
            "passed_case_ids": PASSED_LINE.findall(output),
            "parse_error": parse_error,
            "test_process_exit_code": return_code,
            "test_process_passed": False,
        }

    after = source_state()
    source_changed = before != after
    receipt["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
    receipt["source_after"] = after
    receipt["source_changed_during_run"] = source_changed
    fixture_changed = False
    try:
        fixtures_after = fixture_hashes(selected)
        receipt["fixtures_after"] = fixtures_after
        fixture_changed = fixture_digests != fixtures_after
    except (OSError, ValueError) as error:
        receipt["fixtures_after_error"] = str(error)
        fixture_changed = True
    receipt["fixtures_changed_during_run"] = fixture_changed
    receipt["execution"] = execution
    receipt["log"]["sha256"] = sha256_file(log_path)
    report_error: Optional[str] = None
    if report.is_file():
        receipt["report"]["sha256"] = sha256_file(report)
        try:
            receipt["report"]["totals"] = report_totals(report)
        except (OSError, ValueError, json.JSONDecodeError) as error:
            report_error = str(error)
            receipt["report"]["totals_error"] = report_error
    else:
        report_error = "LLVM coverage report was not produced"
        receipt["report"]["totals_error"] = report_error
    _write_json(receipt_path, receipt)

    print(f"Selected coverage report: {_relative_path(report)}")
    print(f"Local evidence receipt: {_relative_path(receipt_path)}")
    print(f"Captured command output: {_relative_path(log_path)}")
    if source_changed:
        print("selected coverage failed: source or working-tree identity changed during the run", file=sys.stderr)
    if fixture_changed:
        print(
            "selected coverage failed: fixture inputs changed or could not be rehashed during the run",
            file=sys.stderr,
        )
    if report_error:
        print(f"selected coverage failed: {report_error}", file=sys.stderr)
    if return_code != 0:
        print(f"selected coverage failed: cargo llvm-cov exited with {return_code}", file=sys.stderr)
    if parse_error:
        print(f"selected coverage failed: {parse_error}", file=sys.stderr)
    if return_code == 0 and execution and not execution.get("test_process_passed"):
        print("selected coverage failed: test process did not report success", file=sys.stderr)
    if (
        source_changed
        or fixture_changed
        or report_error
        or return_code != 0
        or parse_error
        or not execution
        or not execution.get("test_process_passed")
    ):
        return 1
    return 0


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--cases",
        required=True,
        help="space-separated exact operation:format:row-id selectors",
    )
    parser.add_argument(
        "--toolchain", default="nightly-2026-07-16", help="Rust coverage toolchain"
    )
    parser.add_argument("--report", help="new report path inside this repository")
    args = parser.parse_args()
    try:
        cases = parse_case_list(args.cases)
        selected = load_selected_rows(cases)
        report = _absolute_report_path(args.report)
        exit_code = run(args.toolchain, cases, report)
    except (OSError, ValueError, json.JSONDecodeError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"selected coverage setup failed: {error}") from error
    raise SystemExit(exit_code)


if __name__ == "__main__":
    main()
