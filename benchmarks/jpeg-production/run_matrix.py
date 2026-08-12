#!/usr/bin/env python3
"""Run an alternating public-API JPEG comparison on one machine."""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
TARGET = ROOT / "target" / "jpeg-production-matrix"
RUST_MANIFEST = HERE / "rust" / "Cargo.toml"
RUST_BINARY = TARGET / "release" / "jpeg-production-matrix-rust"
TURBO_BINARY = TARGET / "turbojpeg-matrix"
INPUT_DIR = TARGET / "inputs"

CASES = [
    {"case": "rgb-8-q85-420", "width": 8, "height": 8, "mode": "rgb", "quality": 85, "subsampling": "420", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 20_000},
    {"case": "rgb-32-q85-420", "width": 32, "height": 32, "mode": "rgb", "quality": 85, "subsampling": "420", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 15_000},
    {"case": "rgb-63x65-q85-420", "width": 63, "height": 65, "mode": "rgb", "quality": 85, "subsampling": "420", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 8_000},
    {"case": "rgb-128-q10-420", "width": 128, "height": 128, "mode": "rgb", "quality": 10, "subsampling": "420", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 4_000},
    {"case": "rgb-128-q50-420", "width": 128, "height": 128, "mode": "rgb", "quality": 50, "subsampling": "420", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 4_000},
    {"case": "rgb-128-q85-420", "width": 128, "height": 128, "mode": "rgb", "quality": 85, "subsampling": "420", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 4_000},
    {"case": "rgb-128-q100-420", "width": 128, "height": 128, "mode": "rgb", "quality": 100, "subsampling": "420", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 3_000},
    {"case": "rgb-512-q10-420", "width": 512, "height": 512, "mode": "rgb", "quality": 10, "subsampling": "420", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 300},
    {"case": "rgb-512-q85-420", "width": 512, "height": 512, "mode": "rgb", "quality": 85, "subsampling": "420", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 200},
    {"case": "rgb-1024-q85-420", "width": 1024, "height": 1024, "mode": "rgb", "quality": 85, "subsampling": "420", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 50},
    {"case": "rgb-128-q85-422", "width": 128, "height": 128, "mode": "rgb", "quality": 85, "subsampling": "422", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 3_000},
    {"case": "rgb-128-q85-444", "width": 128, "height": 128, "mode": "rgb", "quality": 85, "subsampling": "444", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 2_000},
    {"case": "rgb-128-q85-optimized", "width": 128, "height": 128, "mode": "rgb", "quality": 85, "subsampling": "420", "progressive": 0, "optimize": 1, "restart": 0, "iterations": 1_500},
    {"case": "rgb-128-q85-progressive", "width": 128, "height": 128, "mode": "rgb", "quality": 85, "subsampling": "420", "progressive": 1, "optimize": 0, "restart": 0, "iterations": 500},
    {"case": "rgb-128-q85-restart4", "width": 128, "height": 128, "mode": "rgb", "quality": 85, "subsampling": "420", "progressive": 0, "optimize": 0, "restart": 4, "iterations": 3_000},
    {"case": "gray-128-q85", "width": 128, "height": 128, "mode": "gray", "quality": 85, "subsampling": "420", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 3_000},
    {"case": "gray-512-q85", "width": 512, "height": 512, "mode": "gray", "quality": 85, "subsampling": "420", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 300},
    {"case": "cmyk-128-q85", "width": 128, "height": 128, "mode": "cmyk", "quality": 85, "subsampling": "444", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 1_000},
    {"case": "cmyk-512-q85", "width": 512, "height": 512, "mode": "cmyk", "quality": 85, "subsampling": "444", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 100},
    {"case": "rgb-127x129-q85-420", "width": 127, "height": 129, "mode": "rgb", "quality": 85, "subsampling": "420", "progressive": 0, "optimize": 0, "restart": 0, "iterations": 3_000},
]


def invoke(command: list[str], *, env: dict[str, str] | None = None) -> str:
    completed = subprocess.run(
        command,
        cwd=ROOT,
        env=env,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return completed.stdout


def capture(command: list[str]) -> str:
    try:
        return invoke(command).strip()
    except (OSError, subprocess.CalledProcessError) as error:
        return f"unavailable: {error}"


def parse_report(output: str) -> dict[str, str]:
    values: dict[str, str] = {}
    for line in output.splitlines():
        key, separator, value = line.partition("=")
        if separator:
            values[key] = value
    required = {"operation", "implementation", "iterations", "median_ns", "p95_ns", "min_ns"}
    missing = required.difference(values)
    if missing:
        raise RuntimeError(f"benchmark report omitted {sorted(missing)}: {output}")
    return values


def build(prefix: Path) -> dict[str, object]:
    TARGET.mkdir(parents=True, exist_ok=True)
    cargo_env = os.environ.copy()
    cargo_env["CARGO_BUILD_RUSTC_WRAPPER"] = ""
    cargo_env["CARGO_TARGET_DIR"] = str(TARGET)
    cargo_command = [
        "cargo",
        "build",
        "--manifest-path",
        str(RUST_MANIFEST),
        "--release",
        "--locked",
    ]
    subprocess.run(cargo_command, cwd=ROOT, env=cargo_env, check=True)

    include = prefix / "include"
    library = prefix / "lib"
    cc = os.environ.get("CC", "cc")
    cc_command = [
        cc,
        "-std=c11",
        "-O3",
        "-DNDEBUG",
        "-Wall",
        "-Wextra",
        "-Werror",
        f"-I{include}",
        f"-L{library}",
        f"-Wl,-rpath,{library}",
        str(HERE / "turbojpeg_matrix.c"),
        "-lturbojpeg",
        "-o",
        str(TURBO_BINARY),
    ]
    subprocess.run(cc_command, cwd=ROOT, check=True)
    return {"cargo": cargo_command, "cc": cc_command}


def encode_arguments(binary: Path, case: dict[str, object]) -> list[str]:
    return [
        str(binary),
        "encode",
        str(case["width"]),
        str(case["height"]),
        str(case["mode"]),
        str(case["quality"]),
        str(case["subsampling"]),
        str(case["progressive"]),
        str(case["optimize"]),
        str(case["restart"]),
        str(case["iterations"]),
    ]


def emit_decode_input(case: dict[str, object]) -> Path:
    INPUT_DIR.mkdir(parents=True, exist_ok=True)
    path = INPUT_DIR / f"{case['case']}.jpg"
    command = encode_arguments(RUST_BINARY, case)
    command[1] = "emit"
    command[-1] = str(path)
    invoke(command)
    return path


def benchmark_case(
    case: dict[str, object],
    operation: str,
    rounds: int,
    raw_file,
) -> list[dict[str, object]]:
    decode_input = emit_decode_input(case) if operation == "decode" else None
    records: list[dict[str, object]] = []
    for round_index in range(rounds):
        order = ["image-slash-star", "libjpeg-turbo"]
        if round_index % 2:
            order.reverse()
        for order_index, implementation in enumerate(order):
            binary = RUST_BINARY if implementation == "image-slash-star" else TURBO_BINARY
            if operation == "encode":
                command = encode_arguments(binary, case)
            else:
                command = [str(binary), "decode", str(decode_input), str(case["iterations"])]
            load_before = os.getloadavg()
            started = time.time_ns()
            report = parse_report(invoke(command))
            record: dict[str, object] = {
                "case": case["case"],
                "operation": operation,
                "round": round_index + 1,
                "order": order_index + 1,
                "implementation": implementation,
                "command": command,
                "load_before": load_before,
                "load_after": os.getloadavg(),
                "started_unix_ns": started,
                "finished_unix_ns": time.time_ns(),
                "report": report,
            }
            raw_file.write(json.dumps(record, sort_keys=True) + "\n")
            raw_file.flush()
            records.append(record)
    return records


def median_report(records: list[dict[str, object]], implementation: str) -> dict[str, str]:
    reports = [record["report"] for record in records if record["implementation"] == implementation]
    medians = [int(report["median_ns"]) for report in reports]
    selected = reports[medians.index(int(statistics.median(medians)))]
    return selected


def summarize(case: dict[str, object], operation: str, records: list[dict[str, object]], rounds: int) -> dict[str, object]:
    rust = median_report(records, "image-slash-star")
    turbo = median_report(records, "libjpeg-turbo")
    rust_ns = int(rust["median_ns"])
    turbo_ns = int(turbo["median_ns"])
    hashes_match = rust.get("output_fnv1a") == turbo.get("output_fnv1a")
    bytes_match = rust.get("output_bytes") == turbo.get("output_bytes")
    return {
        "operation": operation,
        "case": case["case"],
        "width": case["width"],
        "height": case["height"],
        "mode": case["mode"],
        "quality": case["quality"],
        "subsampling": case["subsampling"],
        "progressive": case["progressive"],
        "optimize": case["optimize"],
        "restart_rows": case["restart"],
        "iterations_per_round": case["iterations"],
        "rounds": rounds,
        "rust_median_ns": rust_ns,
        "turbo_median_ns": turbo_ns,
        "rust_over_turbo": f"{rust_ns / turbo_ns:.6f}",
        "output_bytes_match": bytes_match,
        "output_hash_match": hashes_match,
        "rust_output_bytes": rust.get("output_bytes", ""),
        "turbo_output_bytes": turbo.get("output_bytes", ""),
        "rust_output_fnv1a": rust.get("output_fnv1a", ""),
        "turbo_output_fnv1a": turbo.get("output_fnv1a", ""),
        "input_fnv1a": rust.get("input_fnv1a", ""),
    }


def metadata(prefix: Path, commands: dict[str, object], rounds: int) -> dict[str, object]:
    return {
        "schema": 1,
        "timestamp_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "repository": str(ROOT),
        "git_head": capture(["git", "rev-parse", "HEAD"]),
        "git_status": capture(["git", "status", "--short"]),
        "platform": platform.platform(),
        "machine": platform.machine(),
        "python": sys.version,
        "uname": capture(["uname", "-a"]),
        "macos": capture(["sw_vers"]),
        "cpu_brand": capture(["sysctl", "-n", "machdep.cpu.brand_string"]),
        "hardware_model": capture(["sysctl", "-n", "hw.model"]),
        "memory_bytes": capture(["sysctl", "-n", "hw.memsize"]),
        "rustc": capture(["rustc", "-Vv"]),
        "cargo": capture(["cargo", "-V"]),
        "cc": capture([os.environ.get("CC", "cc"), "--version"]),
        "turbojpeg_prefix": str(prefix),
        "turbojpeg_linkage": capture(["otool", "-L", str(TURBO_BINARY)]),
        "rounds": rounds,
        "case_count": len(CASES),
        "build_commands": commands,
        "compiler_flags_note": "ordinary Cargo --release; C harness -O3; no LTO, target-cpu, PGO, or disabled TurboJPEG SIMD",
        "threading": "single-threaded operation loops",
        "initial_load_average": os.getloadavg(),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--turbojpeg-prefix",
        type=Path,
        default=Path(os.environ.get("TURBOJPEG_PREFIX", "/opt/homebrew")),
    )
    args = parser.parse_args()
    if args.rounds < 3 or args.rounds % 2 == 0:
        parser.error("--rounds must be an odd number of at least three")
    if args.output.exists() and any(args.output.iterdir()):
        parser.error(f"output directory is not empty: {args.output}")
    args.output.mkdir(parents=True, exist_ok=True)
    if shutil.which("cargo") is None or shutil.which("cc") is None:
        parser.error("cargo and cc are required")

    commands = build(args.turbojpeg_prefix)
    (args.output / "metadata.json").write_text(
        json.dumps(metadata(args.turbojpeg_prefix, commands, args.rounds), indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    summaries: list[dict[str, object]] = []
    with (args.output / "raw.jsonl").open("w", encoding="utf-8") as raw_file:
        for operation in ("encode", "decode"):
            for case in CASES:
                print(f"{operation:6} {case['case']}", flush=True)
                records = benchmark_case(case, operation, args.rounds, raw_file)
                summaries.append(summarize(case, operation, records, args.rounds))

    with (args.output / "summary.csv").open("w", newline="", encoding="utf-8") as summary_file:
        writer = csv.DictWriter(summary_file, fieldnames=list(summaries[0]))
        writer.writeheader()
        writer.writerows(summaries)


if __name__ == "__main__":
    main()
