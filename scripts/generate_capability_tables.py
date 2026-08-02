#!/usr/bin/env python3
"""Generate or verify the runtime capability-table fixture (FTR-028, QA-029).

The probe test in ``tests/capability_table.rs`` emits one
``CAPABILITY_TABLE_JSON`` line per feature lane. This script executes the
probe in every native lane and in every ``wasm32-wasip1`` lane under Node's
WASI runtime, then assembles ``tests/fixtures/capability_tables.json``.
Independent probe jobs run concurrently with the bounded
``CAPABILITY_JOBS`` setting so this acceptance check does not serialize every
feature lane.

``--generate`` rewrites the committed fixture. ``--check`` regenerates it in
memory and fails on any semantic diff, so capability drift between feature
lanes or targets cannot be merged. The native ``host_triple`` field is
informational: capability values are host-agnostic across native targets, so
it is excluded from the comparison.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURE = ROOT / "tests" / "fixtures" / "capability_tables.json"
PROBE = "emit_runtime_capability_table"
LANES = ["none", "jpeg", "png", "gif", "bmp", "tiff", "webp", "ico", "avif", "default", "all"]
OPERATIONS = [
    "detection",
    "inspection",
    "still_decode",
    "still_encode",
    "sequence_decode",
    "sequence_encode",
]
FORMATS = ["jpeg", "png", "gif", "bmp", "tiff", "webp", "ico", "avif"]
MARKER = "CAPABILITY_TABLE_JSON "


def capability_jobs() -> int:
    raw = os.environ.get("CAPABILITY_JOBS", "3")
    try:
        jobs = int(raw)
    except ValueError as error:
        raise RuntimeError("CAPABILITY_JOBS must be a positive integer") from error
    if jobs < 1:
        raise RuntimeError("CAPABILITY_JOBS must be a positive integer")
    return jobs


def host_triple() -> str:
    output = subprocess.run(
        ["rustc", "-vV"], cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout
    match = re.search(r"^host: (.+)$", output, re.MULTILINE)
    if not match:
        raise RuntimeError("cannot read the host triple from `rustc -vV`")
    return match.group(1).strip()


def lane_features(lane: str) -> list[str]:
    if lane == "none":
        return []
    if lane == "all":
        return FORMATS
    if lane == "default":
        return [feature for feature in FORMATS if feature != "avif"]
    return [lane]


def cargo_args(lane: str, target: str | None) -> list[str]:
    args = ["cargo", "test", "--locked", "--test", "capability_table"]
    if target:
        args += ["--target", target]
    args += ["--no-default-features"]
    features = lane_features(lane)
    if lane == "default":
        args += ["--features", "default"]
    elif features:
        args += ["--features", ",".join(features)]
    if target:
        args += ["--no-run", "--message-format=json"]
    return args


def run_native_probe(lane: str, triple: str) -> dict:
    args = cargo_args(lane, None) + [PROBE, "--", "--exact", "--nocapture"]
    env = {"CAPABILITY_TRIPLE": triple}
    return run_probe(args, env, lane, "native")


def wasi_executable(args: list[str]) -> Path:
    output = subprocess.run(
        args, cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout
    for line in output.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if (
            message.get("reason") == "compiler-artifact"
            and message.get("target", {}).get("name") == "capability_table"
            and message.get("executable", "").endswith(".wasm")
        ):
            return Path(message["executable"])
    raise RuntimeError("cargo did not report a capability_table WASM executable")


def run_wasi_probe(lane: str, triple: str) -> dict:
    build_args = cargo_args(lane, "wasm32-wasip1")
    executable = wasi_executable(build_args)
    node = shutil.which("node")
    if not node:
        raise RuntimeError("node is required for the wasm32-wasip1 runtime lane")
    env = {"CAPABILITY_TRIPLE": triple}
    args = [
        node,
        str(ROOT / "scripts" / "wasm_test_runner.js"),
        str(executable),
        PROBE,
        "--exact",
        "--nocapture",
    ]
    return run_probe(args, env, lane, "wasm32")


def run_probe(args: list[str], env: dict, lane: str, target: str) -> dict:
    process_env = dict(os.environ)
    process_env.update(env)
    output = subprocess.run(
        args, cwd=ROOT, check=True, capture_output=True, text=True, env=process_env
    )
    for line in output.stdout.splitlines():
        if MARKER in line:
            row = json.loads(line.split(MARKER, 1)[1])
            if row.get("lane") != lane:
                raise RuntimeError(
                    f"{target} probe for lane {lane} reported lane {row.get('lane')}"
                )
            if row.get("target") != target:
                raise RuntimeError(
                    f"probe for lane {lane} reported target {row.get('target')}, "
                    f"expected {target}"
                )
            return row
    raise RuntimeError(
        f"no {MARKER!r} line in the {target} probe output for lane {lane}\n"
        f"stdout: {output.stdout[-2000:]}"
    )


def normalize_row(row: dict) -> dict:
    row = dict(row)
    row.pop("triple", None)
    return row


def generate() -> dict:
    triple = host_triple()
    table: dict = {"format_version": 1, "native": {}, "wasm32-wasip1": {}}
    jobs = capability_jobs()
    native_rows: dict[str, dict] = {}
    wasi_rows: dict[str, dict] = {}
    with ThreadPoolExecutor(max_workers=jobs) as executor:
        native_futures = {}
        wasi_futures = {}
        for lane in LANES:
            native_futures[lane] = executor.submit(run_native_probe, lane, triple)
            wasi_futures[lane] = executor.submit(
                run_wasi_probe, lane, "wasm32-wasip1"
            )
        for lane in LANES:
            native_rows[lane] = normalize_row(native_futures[lane].result())
            wasi_rows[lane] = normalize_row(wasi_futures[lane].result())

    for lane in LANES:
        native = native_rows[lane]
        wasi = wasi_rows[lane]
        table["native"].setdefault("host_triple", triple)
        table["wasm32-wasip1"].setdefault("host_triple", "wasm32-wasip1")
        table["native"].setdefault("lanes", {})[lane] = native
        table["wasm32-wasip1"].setdefault("lanes", {})[lane] = wasi
        print(f"recorded {lane}: native={triple}, wasm32-wasip1")
    validate(table)
    return table


def validate(table: dict) -> None:
    if table.get("format_version") != 1:
        raise RuntimeError("capability table format_version must be 1")
    for target in ("native", "wasm32-wasip1"):
        lanes = table.get(target, {}).get("lanes")
        if lanes is None or set(lanes) != set(LANES):
            raise RuntimeError(f"{target} lanes must be exactly {LANES}")
        for lane, row in lanes.items():
            expected_target = "native" if target == "native" else "wasm32"
            if row.get("lane") != lane or row.get("target") != expected_target:
                raise RuntimeError(f"{target}/{lane} row identity mismatch")
            formats = {entry.get("format") for entry in row.get("formats", [])}
            if formats != set(FORMATS):
                raise RuntimeError(f"{target}/{lane} formats mismatch: {formats}")
            for entry in row.get("formats", []):
                operations = entry.get("operations", {})
                if list(operations) != OPERATIONS:
                    raise RuntimeError(f"{target}/{lane} operation order mismatch")


def comparison_table(table: dict) -> dict:
    return {
        "format_version": table.get("format_version"),
        "native": {"lanes": table.get("native", {}).get("lanes")},
        "wasm32-wasip1": {"lanes": table.get("wasm32-wasip1", {}).get("lanes")},
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail on any drift")
    parser.add_argument("--generate", action="store_true", help="rewrite the fixture")
    args = parser.parse_args()
    if args.check == args.generate:
        parser.error("exactly one of --check or --generate is required")

    regenerated = generate()
    if args.generate:
        FIXTURE.write_text(
            json.dumps(regenerated, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
        return 0

    try:
        committed = json.loads(FIXTURE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"error: cannot read {FIXTURE}: {error}", file=sys.stderr)
        return 1
    if comparison_table(committed) != comparison_table(regenerated):
        print(
            "error: capability tables drifted; run "
            "`python3 scripts/generate_capability_tables.py --generate` "
            "and commit the intentional change",
            file=sys.stderr,
        )
        return 1
    print("capability tables OK: every native and wasm32-wasip1 lane agrees")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
