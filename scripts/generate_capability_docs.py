#!/usr/bin/env python3
"""Generate the packaged runtime-capability and fixture-contract document.

The runtime capability fixture and the coverage matrix answer different
questions.  ``capability_tables.json`` records what each feature lane exposes
when the library is executed on a target.  ``coverage_matrix.json`` records
the active input/output contracts that have actually been exercised against
the pinned oracle.  This generator keeps those evidence domains separate in a
deterministic document that can be shipped with the crate.

``--check`` reads only the committed source fixtures, regenerates the document
in memory, and compares it byte-for-byte with ``docs/capabilities.md``.
``--generate`` writes that deterministic result.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CAPABILITY_FIXTURE = ROOT / "tests" / "fixtures" / "capability_tables.json"
COVERAGE_MATRIX = ROOT / "tests" / "fixtures" / "coverage_matrix.json"
OUTPUT = ROOT / "docs" / "capabilities.md"

MARKER = "<!-- generated-capability-docs:v1; do not edit -->"
CAPABILITY_FORMAT_VERSION = 1
TARGETS = ("native", "wasm32-wasip1")
LANES = (
    "none",
    "jpeg",
    "png",
    "gif",
    "bmp",
    "tiff",
    "webp",
    "ico",
    "avif",
    "default",
    "all",
)
FORMATS = ("avif", "bmp", "gif", "ico", "jpeg", "png", "tiff", "webp")
CAPABILITY_OPERATIONS = (
    "detection",
    "inspection",
    "still_decode",
    "still_encode",
    "sequence_decode",
    "sequence_encode",
)
MATRIX_OPERATIONS = {
    "decode": ("decode", "decode_sequence"),
    "encode": ("encode", "encode_sequence"),
}
MATRIX_OPERATION_FIELDS = {
    "decode": ("detect", "inspect", "verify", "decode", "decode_sequence"),
    "encode": ("encode", "encode_sequence"),
}
OUTCOMES = {"ok", "error", "not_applicable"}
VERIFICATION_SCOPES = {"header_only", "structure"}
TIMED_SEQUENCE_FORMATS = {"avif", "gif", "png", "webp"}

MODE_ALIASES = {
    "1": "L1",
    "P": "P8",
    "L": "L8",
    "L8": "L8",
    "LA": "La8",
    "La8": "La8",
    "RGB": "Rgb8",
    "Rgb8": "Rgb8",
    "RGBA": "Rgba8",
    "Rgba8": "Rgba8",
    "CMYK": "Cmyk8",
    "Cmyk8": "Cmyk8",
    "I;16": "L16",
    "I;16B": "L16",
    "I;16L": "L16",
    "L16": "L16",
    "La16": "La16",
    "Rgb16": "Rgb16",
    "Rgba16": "Rgba16",
    "Rgb32F": "Rgb32F",
    "Rgba32F": "Rgba32F",
    "F": "F32",
    "F32": "F32",
    "I": "I32",
    "I32": "I32",
}


class GenerationError(RuntimeError):
    """A source-fixture or generated-document contract violation."""


def fail(message: str) -> None:
    raise GenerationError(message)


def read_json(path: Path) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {path.relative_to(ROOT)}: {error}")
    if not isinstance(value, dict):
        fail(f"{path.relative_to(ROOT)} must contain a JSON object")
    return value


def require_dict(value: object, context: str) -> dict:
    if not isinstance(value, dict):
        fail(f"{context} must be an object")
    return value


def require_list(value: object, context: str) -> list:
    if not isinstance(value, list):
        fail(f"{context} must be an array")
    return value


def require_string(value: object, context: str) -> str:
    if not isinstance(value, str) or not value:
        fail(f"{context} must be a non-empty string")
    return value


def require_positive_int(value: object, context: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        fail(f"{context} must be a positive integer")
    return value


def normalize_mode(value: object, context: str) -> str:
    mode = require_string(value, context)
    try:
        return MODE_ALIASES[mode]
    except KeyError:
        fail(f"{context} has unknown image mode {mode!r}")
    raise AssertionError("unreachable")


def validate_capability_fixture(document: dict) -> None:
    expected_keys = {"format_version", *TARGETS}
    if set(document) != expected_keys:
        fail(
            "capability fixture top-level keys must be exactly "
            f"{sorted(expected_keys)}"
        )
    if document.get("format_version") != CAPABILITY_FORMAT_VERSION:
        fail("capability fixture format_version must be 1")

    for target in TARGETS:
        target_document = require_dict(document.get(target), f"capability target {target}")
        require_string(target_document.get("host_triple"), f"capability target {target}.host_triple")
        lanes = require_dict(target_document.get("lanes"), f"capability target {target}.lanes")
        if set(lanes) != set(LANES):
            fail(f"capability target {target} lanes differ from the committed lane set")
        for lane in LANES:
            lane_document = require_dict(lanes.get(lane), f"capability {target}/{lane}")
            if lane_document.get("target") != target or lane_document.get("lane") != lane:
                fail(f"capability {target}/{lane} has inconsistent row identity")
            features = require_list(lane_document.get("features"), f"capability {target}/{lane}.features")
            if any(not isinstance(feature, str) for feature in features):
                fail(f"capability {target}/{lane}.features must contain only strings")
            expected_features = {
                "none": set(),
                "default": set(FORMATS) - {"avif"},
                "all": set(FORMATS),
                "ico": {"ico", "bmp", "png"},
            }.get(lane, {lane})
            if set(features) != expected_features:
                fail(f"capability {target}/{lane}.features has inconsistent lane identity")
            formats = require_list(
                lane_document.get("formats"), f"capability {target}/{lane}.formats"
            )
            entries = {}
            for index, entry_value in enumerate(formats):
                entry = require_dict(entry_value, f"capability {target}/{lane}.formats[{index}]")
                fmt = require_string(entry.get("format"), "capability format name")
                if fmt in entries:
                    fail(f"capability {target}/{lane} repeats format {fmt}")
                entries[fmt] = entry
                if fmt not in FORMATS:
                    fail(f"capability {target}/{lane} has unknown format {fmt}")
                if not isinstance(entry.get("feature_enabled"), bool):
                    fail(f"capability {target}/{lane}/{fmt} feature_enabled must be boolean")
                operations = require_dict(
                    entry.get("operations"), f"capability {target}/{lane}/{fmt}.operations"
                )
                if set(operations) != set(CAPABILITY_OPERATIONS):
                    fail(
                        f"capability {target}/{lane}/{fmt} operations differ from the "
                        "published operation set"
                    )
                for operation in CAPABILITY_OPERATIONS:
                    require_string(
                        operations.get(operation),
                        f"capability {target}/{lane}/{fmt}/{operation}",
                    )
            if set(entries) != set(FORMATS):
                fail(f"capability {target}/{lane} formats differ from the published format set")


def validate_mode_fields(row: dict, context: str) -> None:
    for field in ("source_mode", "ref_mode"):
        if row.get(field) is not None:
            normalize_mode(row[field], f"{context}.{field}")
    parameters = row.get("params")
    if parameters is None:
        return
    parameters = require_dict(parameters, f"{context}.params")
    unsupported_modes = parameters.get("rust_unsupported_modes")
    if unsupported_modes is not None:
        for index, mode in enumerate(require_list(unsupported_modes, f"{context}.params.rust_unsupported_modes")):
            normalize_mode(mode, f"{context}.params.rust_unsupported_modes[{index}]")
    for field in ("sequence_frame_mode", "second_frame_mode"):
        if parameters.get(field) is not None:
            normalize_mode(parameters[field], f"{context}.params.{field}")


def validate_execution(row: dict, context: str) -> None:
    execution = require_dict(row.get("execution"), f"{context}.execution")
    require_string(execution.get("target"), f"{context}.execution.target")
    if execution.get("suite") != "native_all_features":
        fail(
            f"{context}.execution.suite must be native_all_features for the "
            "published active fixture evidence"
        )
    features = require_list(execution.get("features"), f"{context}.execution.features")
    if set(features) != set(FORMATS) or any(not isinstance(feature, str) for feature in features):
        fail(f"{context}.execution.features must list every all-feature codec")


def validate_active_row(row: dict, fmt: str, kind: str) -> None:
    context = f"active {kind} row {fmt}:{row.get('id')}"
    validate_execution(row, context)
    validate_mode_fields(row, context)
    operations = require_dict(row.get("operations"), f"{context}.operations")
    expected_operations = MATRIX_OPERATION_FIELDS[kind]
    if set(operations) != set(expected_operations):
        fail(f"{context}.operations differs from the {kind} operation set")
    errors = row.get("error_contracts", {})
    errors = require_dict(errors, f"{context}.error_contracts")
    for operation in expected_operations:
        outcome = operations[operation]
        if outcome not in OUTCOMES:
            fail(f"{context}.{operation} has unknown outcome {outcome!r}")
        if outcome == "error":
            error_contract = require_dict(
                errors.get(operation), f"{context}.error_contracts.{operation}"
            )
            require_string(
                error_contract.get("rust_kind"),
                f"{context}.error_contracts.{operation}.rust_kind",
            )

    if kind == "decode":
        scope = require_string(row.get("verification_scope"), f"{context}.verification_scope")
        if scope not in VERIFICATION_SCOPES:
            fail(f"{context}.verification_scope has unknown value {scope!r}")
    else:
        normalize_mode(row.get("source_mode"), f"{context}.source_mode")
        require_positive_int(row.get("source_frame_count"), f"{context}.source_frame_count")

    for operation in MATRIX_OPERATIONS[kind]:
        if operations[operation] != "ok":
            continue
        if kind == "decode":
            normalize_mode(row.get("ref_mode"), f"{context}.{operation}.ref_mode")
            if operation == "decode_sequence":
                require_positive_int(
                    row.get("ref_frame_count"), f"{context}.{operation}.ref_frame_count"
                )
                if not isinstance(row.get("ref_is_animated"), bool):
                    fail(f"{context}.{operation}.ref_is_animated must be boolean")
                if row["ref_is_animated"] != (row["ref_frame_count"] > 1):
                    fail(f"{context}.{operation} frame count and animation flag disagree")
        else:
            normalize_mode(row.get("ref_mode"), f"{context}.{operation}.ref_mode")
            require_positive_int(row.get("ref_frame_count"), f"{context}.{operation}.ref_frame_count")
            if not isinstance(row.get("ref_is_animated"), bool):
                fail(f"{context}.{operation}.ref_is_animated must be boolean")
            if row["ref_is_animated"] != (row["ref_frame_count"] > 1):
                fail(f"{context}.{operation} frame count and animation flag disagree")
        if operation in ("decode_sequence", "encode_sequence"):
            if kind == "decode" or row["ref_frame_count"] > 1:
                require_positive_int(
                    row.get("ref_frame_count"), f"{context}.{operation}.ref_frame_count"
                )
            if kind == "decode" and row["ref_frame_count"] > 1:
                sequence = require_dict(row.get("sequence"), f"{context}.sequence")
                frames = require_list(sequence.get("frames"), f"{context}.sequence.frames")
                if len(frames) != row["ref_frame_count"]:
                    fail(f"{context}.sequence.frames does not match ref_frame_count")


def validate_matrix(document: dict) -> tuple[list[tuple[str, str, dict]], dict, dict]:
    formats = require_dict(document.get("formats"), "coverage matrix.formats")
    if set(formats) != set(FORMATS):
        fail("coverage matrix formats differ from the published format set")
    summary = require_dict(document.get("summary"), "coverage matrix.summary")
    rows: list[tuple[str, str, dict]] = []
    seen: set[tuple[str, str, str]] = set()
    counts = defaultdict(int)
    scopes: dict[str, str] = {}

    for fmt in FORMATS:
        format_document = require_dict(formats.get(fmt), f"coverage matrix format {fmt}")
        for kind in ("decode", "encode"):
            entries = require_list(format_document.get(kind), f"coverage matrix {fmt}.{kind}")
            counts[f"{kind}_rows"] += len(entries)
            for index, row_value in enumerate(entries):
                row = require_dict(row_value, f"coverage matrix {fmt}.{kind}[{index}]")
                row_id = require_string(row.get("id"), f"coverage matrix {fmt}.{kind}[{index}].id")
                if row.get("format") != fmt or row.get("type") != kind:
                    fail(f"coverage matrix row {fmt}:{row_id} has inconsistent identity")
                key = (fmt, kind, row_id)
                if key in seen:
                    fail(f"coverage matrix repeats row identity {fmt}:{kind}:{row_id}")
                seen.add(key)
                status = row.get("status")
                if status not in {"active", "planned"}:
                    fail(f"coverage matrix row {fmt}:{kind}:{row_id} has unknown status {status!r}")
                counts[f"{kind}_{status}"] += 1
                if status == "planned":
                    validate_mode_fields(row, f"planned {kind} row {fmt}:{row_id}")
                    continue
                validate_active_row(row, fmt, kind)
                rows.append((fmt, kind, row))
                if kind == "decode":
                    scope = row["verification_scope"]
                    if fmt in scopes and scopes[fmt] != scope:
                        fail(f"active decode rows for {fmt} disagree on verification_scope")
                    scopes[fmt] = scope
                for operation in MATRIX_OPERATIONS[kind]:
                    if row["operations"][operation] == "not_applicable":
                        counts["not_applicable"] += 1
                    else:
                        counts["included_operation_entries"] += 1

    expected_summary = {
        "total_rows": counts["decode_rows"] + counts["encode_rows"],
        "decode_rows": counts["decode_rows"],
        "encode_rows": counts["encode_rows"],
        "decode_active": counts["decode_active"],
        "decode_planned": counts["decode_planned"],
        "encode_not_wired": counts["encode_planned"],
    }
    for field, expected in expected_summary.items():
        if summary.get(field) != expected:
            fail(
                f"coverage matrix summary {field} is {summary.get(field)!r}, "
                f"expected {expected}"
            )
    if set(scopes) != set(FORMATS):
        fail("every published format must have active decode verification scope evidence")
    return rows, dict(counts), scopes


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def format_label(fmt: str) -> str:
    return fmt.upper()


def outcome_label(row: dict, operation: str) -> tuple[str, str]:
    outcome = row["operations"][operation]
    if outcome == "ok":
        return "accepted", "—"
    if outcome == "error":
        error_contract = row["error_contracts"][operation]
        return "rejected", error_contract["rust_kind"]
    fail(f"cannot render not_applicable operation {operation} as evidence")
    raise AssertionError("unreachable")


def sequence_kind(fmt: str, row: dict, operation: str) -> str:
    frame_count = require_positive_int(
        row.get("ref_frame_count"), f"{fmt}:{row['id']}:{operation}.ref_frame_count"
    )
    if frame_count == 1:
        return "SingleFrame"
    if fmt in TIMED_SEQUENCE_FORMATS:
        return "TimedAnimation"
    if fmt == "tiff":
        return "UntimedPages"
    fail(
        f"successful multi-frame sequence {fmt}:{row['id']}:{operation} has no "
        "published sequence-kind rule"
    )
    raise AssertionError("unreachable")


def evidence_id(fmt: str, row: dict) -> str:
    return f"{fmt}:{row['id']}"


def evidence_cell(ids: list[str]) -> str:
    quoted = ", ".join(f"`{item}`" for item in sorted(ids))
    return f"{len(ids)}: {quoted}"


def table_cell(value: object) -> str:
    return str(value).replace("|", r"\|").replace("\n", " ")


def grouped_decode_evidence(matrix: dict) -> list[dict]:
    groups: dict[tuple[str, ...], list[str]] = defaultdict(list)
    for fmt in FORMATS:
        for row in matrix["formats"][fmt]["decode"]:
            if row.get("status") != "active":
                continue
            for operation in MATRIX_OPERATIONS["decode"]:
                if row["operations"][operation] == "not_applicable":
                    continue
                outcome, error_kind = outcome_label(row, operation)
                if outcome == "accepted":
                    output_mode = normalize_mode(row["ref_mode"], f"{fmt}:{row['id']}.ref_mode")
                    if operation == "decode_sequence":
                        returned_kind = sequence_kind(fmt, row, operation)
                    else:
                        returned_kind = "— (image result)"
                else:
                    output_mode = "—"
                    returned_kind = "— (no result)"
                key = (
                    fmt,
                    "still_decode" if operation == "decode" else "sequence_decode",
                    outcome,
                    output_mode,
                    returned_kind,
                    row["verification_scope"],
                    error_kind,
                )
                groups[key].append(evidence_id(fmt, row))

    result = []
    for key, ids in sorted(groups.items()):
        (
            fmt,
            operation,
            outcome,
            output_mode,
            returned_kind,
            verification_scope,
            error_kind,
        ) = key
        result.append(
            {
                "format": format_label(fmt),
                "operation": operation,
                "outcome": outcome,
                "output_mode": output_mode,
                "returned_kind": returned_kind,
                "verification_scope": verification_scope,
                "error_kind": error_kind,
                "evidence": evidence_cell(ids),
            }
        )
    return result


def encode_mode_witness(row: dict, context: str) -> str:
    mode = normalize_mode(row["source_mode"], f"{context}.source_mode")
    parameters = row.get("params") or {}
    witnesses = []
    unsupported = parameters.get("rust_unsupported_modes")
    if unsupported:
        witnesses.append(
            "invalid-mode witness: "
            + ", ".join(normalize_mode(value, f"{context}.params.rust_unsupported_modes") for value in unsupported)
        )
    for field, label in (("second_frame_mode", "second-frame mode"), ("sequence_frame_mode", "sequence-frame mode")):
        if parameters.get(field) is not None:
            witnesses.append(
                f"{label}: {normalize_mode(parameters[field], f'{context}.params.{field}')}"
            )
    return mode if not witnesses else f"{mode}; " + "; ".join(witnesses)


def frame_shape(row: dict, context: str) -> str:
    count = require_positive_int(row.get("source_frame_count"), f"{context}.source_frame_count")
    return f"{count} frame" if count == 1 else f"{count} frames"


def grouped_encode_evidence(matrix: dict) -> list[dict]:
    groups: dict[tuple[str, ...], list[str]] = defaultdict(list)
    for fmt in FORMATS:
        for row in matrix["formats"][fmt]["encode"]:
            if row.get("status") != "active":
                continue
            for operation in MATRIX_OPERATIONS["encode"]:
                if row["operations"][operation] == "not_applicable":
                    continue
                context = f"{fmt}:{row['id']}"
                outcome, error_kind = outcome_label(row, operation)
                input_mode = encode_mode_witness(row, context)
                input_shape = frame_shape(row, context)
                if outcome == "accepted":
                    output_mode = normalize_mode(row["ref_mode"], f"{context}.ref_mode")
                    if operation == "encode_sequence":
                        target_kind = sequence_kind(fmt, row, operation)
                    else:
                        target_kind = "— (still result)"
                else:
                    output_mode = "— (no Rust output)"
                    target_kind = "— (no result)"
                key = (
                    fmt,
                    "still_encode" if operation == "encode" else "sequence_encode",
                    outcome,
                    input_mode,
                    input_shape,
                    output_mode,
                    target_kind,
                    error_kind,
                )
                groups[key].append(evidence_id(fmt, row))

    result = []
    for key, ids in sorted(groups.items()):
        (
            fmt,
            operation,
            outcome,
            input_mode,
            input_shape,
            output_mode,
            target_kind,
            error_kind,
        ) = key
        result.append(
            {
                "format": format_label(fmt),
                "operation": operation,
                "outcome": outcome,
                "input_mode": input_mode,
                "input_shape": input_shape,
                "output_mode": output_mode,
                "target_kind": target_kind,
                "error_kind": error_kind,
                "evidence": evidence_cell(ids),
            }
        )
    return result


def render_capability_table(capability: dict) -> list[str]:
    lines = [
        "| Target | Feature lane | Format | Feature enabled | Detection | Inspection | Still decode | Still encode | Sequence decode | Sequence encode |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for target in TARGETS:
        lanes = capability[target]["lanes"]
        for lane in LANES:
            entries = {entry["format"]: entry for entry in lanes[lane]["formats"]}
            for fmt in FORMATS:
                entry = entries[fmt]
                operations = entry["operations"]
                values = [operations[operation] for operation in CAPABILITY_OPERATIONS]
                lines.append(
                    "| "
                    + " | ".join(
                        table_cell(value)
                        for value in (
                            target,
                            lane,
                            format_label(fmt),
                            "yes" if entry["feature_enabled"] else "no",
                            *values,
                        )
                    )
                    + " |"
                )
    return lines


def render_verification_table(scopes: dict, matrix: dict) -> list[str]:
    lines = [
        "| Format | Verification scope | Active decode rows |",
        "| --- | --- | ---: |",
    ]
    for fmt in FORMATS:
        count = sum(
            row.get("status") == "active" for row in matrix["formats"][fmt]["decode"]
        )
        lines.append(f"| {format_label(fmt)} | {scopes[fmt]} | {count:,} |")
    return lines


def render_decode_table(rows: list[dict]) -> list[str]:
    lines = [
        "| Format | Operation | Outcome | Observed Rust output mode | Returned sequence kind | Verification scope | Rust error kind | Evidence |",
        "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for row in rows:
        lines.append(
            "| "
            + " | ".join(
                table_cell(row[field])
                for field in (
                    "format",
                    "operation",
                    "outcome",
                    "output_mode",
                    "returned_kind",
                    "verification_scope",
                    "error_kind",
                    "evidence",
                )
            )
            + " |"
        )
    return lines


def render_encode_table(rows: list[dict]) -> list[str]:
    lines = [
        "| Target format | Operation | Outcome | Public input mode witness | Input frame shape | Oracle-decoded output mode | Target sequence semantics | Rust error kind | Evidence |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for row in rows:
        lines.append(
            "| "
            + " | ".join(
                table_cell(row[field])
                for field in (
                    "format",
                    "operation",
                    "outcome",
                    "input_mode",
                    "input_shape",
                    "output_mode",
                    "target_kind",
                    "error_kind",
                    "evidence",
                )
            )
            + " |"
        )
    return lines


def render_document(capability: dict, matrix: dict, counts: dict, scopes: dict) -> str:
    active_total = counts["decode_active"] + counts["encode_active"]
    planned_total = counts["decode_planned"] + counts["encode_planned"]
    not_applicable = counts["not_applicable"]
    decode_rows = grouped_decode_evidence(matrix)
    encode_rows = grouped_encode_evidence(matrix)
    capability_rows = len(TARGETS) * len(LANES) * len(FORMATS)
    lines = [
        MARKER,
        "# Runtime capabilities and observed fixture contracts",
        "",
        "This page is generated from committed runtime/cfg evidence and active "
        "fixture evidence. It is packaged with the crate so users can inspect "
        "the same bounded contract that the repository checks.",
        "",
        "## Provenance and scope",
        "",
        "- Generator: `scripts/generate_capability_docs.py`.",
        "- Regenerate: `python3 scripts/generate_capability_docs.py --generate`.",
        "- Runtime source: `tests/fixtures/capability_tables.json`.",
        "- Active fixture source: `tests/fixtures/coverage_matrix.json`.",
        f"- Runtime source SHA-256: `{sha256(CAPABILITY_FIXTURE)}`.",
        f"- Active fixture source SHA-256: `{sha256(COVERAGE_MATRIX)}`.",
        f"- Runtime capability fixture format version: {capability['format_version']}.",
        f"- Runtime targets represented: {len(TARGETS)} (`{TARGETS[0]}`, `{TARGETS[1]}`).",
        f"- Feature lanes represented per target: {len(LANES)}.",
        f"- Formats per lane: {len(FORMATS)}.",
        f"- Operations per runtime capability row: {len(CAPABILITY_OPERATIONS)}.",
        f"- Runtime capability rows rendered: {capability_rows:,}.",
        f"- Active fixture rows: {active_total:,} ({counts['decode_active']:,} decode; {counts['encode_active']:,} encode).",
        f"- Planned fixture rows excluded: {planned_total:,} ({counts['decode_planned']:,} decode; {counts['encode_planned']:,} encode).",
        f"- Active `not_applicable` operation outcomes excluded from evidence groups: {not_applicable:,}.",
        "",
        "Runtime evidence is present only for `native` and `wasm32-wasip1`; "
        "this page makes no runtime claim for `wasm32-unknown-unknown`. The "
        "fixture tables below are observed active contracts, not an exhaustive "
        "file-format specification or Cartesian mode-acceptance matrix.",
        "Active fixture evidence in the tables below is `native` / `all features`; "
        "it is evidence for the matrix run, not a claim that every target/lane "
        "has the same direct fixture coverage.",
        "",
        "## Runtime operation capabilities",
        "",
        "Each row below comes solely from the runtime capability fixture. A "
        "`feature_disabled` value means the operation is unavailable in that "
        "lane; it is not a claim about the codec when enabled.",
        "",
        *render_capability_table(capability),
        "",
        "## Observed active fixture contracts",
        "",
        "The following tables group identical observed evidence tuples and list "
        "their exact format-qualified fixture IDs. `accepted` means the active "
        "operation returned successfully; `rejected` means the active operation "
        "returned the recorded typed Rust error. A rejected or planned operation "
        "does not provide a sequence kind.",
        "",
        "### Verification scope by format",
        "",
        "Verification scope is a format-level property derived from active decode "
        "rows. It is intentionally separate from runtime feature-lane availability "
        "and is not inferred for encode-only evidence.",
        "",
        *render_verification_table(scopes, matrix),
        "",
        "### Decode output evidence",
        "",
        "`Observed Rust output mode` is the mode returned by a successful decode. "
        "A rejected decode has no output mode, even when the oracle row contains "
        "a reference mode. `Returned sequence kind` is derived only from a "
        "successful sequence result: one retained frame is `SingleFrame`, "
        "multi-frame GIF/APNG/WebP/AVIF is `TimedAnimation`, and multi-page TIFF "
        "is `UntimedPages`.",
        "",
        *render_decode_table(decode_rows),
        "",
        "### Encode input/output evidence",
        "",
        "`Public input mode witness` describes the source mode exercised by an "
        "active encode fixture; an invalid-mode parameter witness is shown when "
        "the row covers several rejected modes. `Oracle-decoded output mode` is "
        "shown only for an accepted output and is not a Rust result mode for a "
        "rejected operation. `Target sequence semantics` is derived only from a "
        "successful output, so it does not turn an error row into an accepted "
        "sequence-kind claim.",
        "",
        *render_encode_table(encode_rows),
        "",
        "`not_applicable` operation entries are omitted from the evidence tables "
        "because they are neither acceptance nor rejection. Planned rows are "
        "also omitted from active evidence and remain explicit roadmap work; the "
        "generated counts above make both exclusions visible.",
        "",
    ]
    return "\n".join(lines)


def generate_document() -> str:
    capability = read_json(CAPABILITY_FIXTURE)
    matrix = read_json(COVERAGE_MATRIX)
    validate_capability_fixture(capability)
    _, counts, scopes = validate_matrix(matrix)
    return render_document(capability, matrix, counts, scopes)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail when the generated document drifts")
    parser.add_argument("--generate", action="store_true", help="write the generated document")
    args = parser.parse_args()
    if args.check == args.generate:
        parser.error("exactly one of --check or --generate is required")
    try:
        generated = generate_document()
        if args.generate:
            OUTPUT.write_text(generated, encoding="utf-8")
            print(f"wrote {OUTPUT.relative_to(ROOT)}")
            return 0
        try:
            committed = OUTPUT.read_text(encoding="utf-8")
        except OSError as error:
            fail(f"cannot read {OUTPUT.relative_to(ROOT)}: {error}")
        if committed != generated:
            fail(
                "generated capability documentation drifted; run "
                "`python3 scripts/generate_capability_docs.py --generate` "
                "and commit the intentional result"
            )
        print("capability documentation OK: generated output matches committed fixtures")
        return 0
    except GenerationError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
