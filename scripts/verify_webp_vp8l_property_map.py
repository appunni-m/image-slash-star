#!/usr/bin/env python3
"""Verify the revision-pinned WebP VP8L property-to-fixture map.

The map is an audit of fixture intent, not a second parity runner.  Every
named witness must resolve to an active WebP row whose asserted operation has
Pillow provenance, and the referenced input/output artifacts must still have
the hashes recorded by the generated matrix.  The map deliberately rejects
an internal VP8L property claim as proven by a Pillow row alone.
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

try:
    from inspect_webp_vp8l_structure import ParseError, inspect_path
except ImportError as error:  # pragma: no cover - only reached outside the repo script path
    raise RuntimeError("VP8L structural inspector is unavailable") from error

ROOT = Path(__file__).resolve().parent.parent
MAP_PATH = ROOT / "tests" / "fixtures" / "webp_vp8l_property_map.json"
MATRIX_PATH = ROOT / "tests" / "fixtures" / "coverage_matrix.json"
MANIFEST_PATH = ROOT / "manifest.yaml"
INSPECTOR_PATH = ROOT / "scripts" / "inspect_webp_vp8l_structure.py"
SHA_RE = re.compile(r"^[0-9a-f]{40}$")
HEX64_RE = re.compile(r"^[0-9a-f]{64}$")
OPERATIONS = ("decode", "encode")
STATUSES = {"candidate", "unmapped", "witnessed"}
BOUNDARIES = {"pillow_outer_result", "pillow_outer_result_and_specification", "rust_only"}


def fail(message: str) -> None:
    raise RuntimeError(message)


def read_json(path: Path) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {path}: {error}")
    if not isinstance(value, dict):
        fail(f"{path} must contain a JSON object")
    return value


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require_sha(value: object, label: str) -> str:
    if not isinstance(value, str) or not HEX64_RE.fullmatch(value):
        fail(f"{label} must be a 64-character lowercase SHA-256")
    return value


def verify_revision(value: object) -> str:
    if not isinstance(value, str) or not SHA_RE.fullmatch(value):
        fail("source_revision must be a 40-character git commit SHA-1")
    result = subprocess.run(
        ["git", "rev-parse", "--verify", value],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        fail(f"source_revision {value} is not a commit in this repository")
    return value


def verify_inputs(document: dict) -> None:
    if document.get("format_version") != 1:
        fail("property map format_version must be 1")
    if document.get("schema") != "image-slash-star/webp-vp8l-property-map@1":
        fail("property map schema is not recognized")
    if document.get("scope") != "webp/vp8l":
        fail("property map scope must be webp/vp8l")
    verify_revision(document.get("source_revision"))

    oracle = document.get("oracle")
    if not isinstance(oracle, dict):
        fail("oracle must be an object")
    if oracle.get("name") != "Pillow" or oracle.get("version") != "12.2.0":
        fail("property map must remain pinned to Pillow 12.2.0")
    if oracle.get("codec_backend") != "libwebp 1.6.0":
        fail("property map must remain pinned to libwebp 1.6.0")

    inputs = document.get("inputs")
    if not isinstance(inputs, dict):
        fail("inputs must be an object")
    if inputs.get("manifest_path") != "manifest.yaml":
        fail("inputs.manifest_path must be manifest.yaml")
    if inputs.get("matrix_path") != "tests/fixtures/coverage_matrix.json":
        fail("inputs.matrix_path must name the generated coverage matrix")
    if inputs.get("inspector_path") != "scripts/inspect_webp_vp8l_structure.py":
        fail("inputs.inspector_path must name the committed VP8L inspector")
    if require_sha(inputs.get("manifest_sha256"), "inputs.manifest_sha256") != sha256(
        MANIFEST_PATH
    ):
        fail("property map manifest SHA-256 does not match manifest.yaml")
    if require_sha(inputs.get("matrix_sha256"), "inputs.matrix_sha256") != sha256(
        MATRIX_PATH
    ):
        fail("property map matrix SHA-256 does not match coverage_matrix.json")
    if require_sha(inputs.get("inspector_sha256"), "inputs.inspector_sha256") != sha256(
        INSPECTOR_PATH
    ):
        fail("property map inspector SHA-256 does not match the committed inspector")

    policy = document.get("evidence_policy")
    if not isinstance(policy, dict):
        fail("evidence_policy must be an object")
    for key in (
        "pillow_outer_result",
        "candidate",
        "structural_witness_needed",
        "malformed_parser",
    ):
        if not isinstance(policy.get(key), str) or not policy[key].strip():
            fail(f"evidence_policy.{key} must be a non-empty explanation")


def matrix_rows(matrix: dict) -> dict[tuple[str, str], dict]:
    formats = matrix.get("formats")
    if not isinstance(formats, dict):
        fail("coverage matrix formats must be an object")
    webp = formats.get("webp")
    if not isinstance(webp, dict):
        fail("coverage matrix must contain a webp format")

    indexed: dict[tuple[str, str], dict] = {}
    for operation in OPERATIONS:
        rows = webp.get(operation)
        if not isinstance(rows, list):
            fail(f"coverage matrix webp.{operation} must be an array")
        for row in rows:
            if not isinstance(row, dict):
                fail(f"coverage matrix webp.{operation} rows must be objects")
            row_id = row.get("id")
            if not isinstance(row_id, str) or not row_id:
                fail(f"coverage matrix webp.{operation} row has no id")
            key = (operation, row_id)
            if key in indexed:
                fail(f"duplicate WebP matrix row {operation}:{row_id}")
            if row.get("format") != "webp" or row.get("status") != "active":
                continue
            indexed[key] = row
    return indexed


def fixture_path(operation: str, row: dict) -> tuple[Path, str, str]:
    if operation == "decode":
        asset = row.get("asset")
        digest = row.get("asset_sha256")
        path = ROOT / "tests" / "fixtures" / "input" / "images" / "webp" / str(asset)
    else:
        asset = row.get("source_asset")
        digest = row.get("source_sha256")
        source_format = row.get("source_format")
        if not isinstance(source_format, str) or not source_format:
            fail(f"{operation}:{row.get('id')}: source_format is required")
        path = ROOT / "tests" / "fixtures" / "input" / "images" / source_format / str(asset)
    if not isinstance(asset, str) or not asset:
        fail(f"{operation}:{row.get('id')}: fixture asset is required")
    if not isinstance(digest, str) or not HEX64_RE.fullmatch(digest):
        fail(f"{operation}:{row.get('id')}: fixture asset hash is required")
    return path, asset, digest


def verify_witness(
    witness: object,
    property_id: str,
    rows: dict[tuple[str, str], dict],
    seen: set[tuple[str, str]],
) -> tuple[str, str]:
    if not isinstance(witness, dict):
        fail(f"{property_id}: witnesses must be objects")
    operation = witness.get("operation")
    row_id = witness.get("row_id")
    if operation not in OPERATIONS or not isinstance(row_id, str) or not row_id:
        fail(f"{property_id}: witness must contain a decode/encode operation and row_id")
    key = (operation, row_id)
    row = rows.get(key)
    if row is None:
        fail(f"{property_id}: witness {operation}:{row_id} is not an active WebP row")
    if row.get("type") != operation:
        fail(f"{property_id}: witness {operation}:{row_id} has the wrong row type")

    origins = row.get("assertion_origins")
    if not isinstance(origins, dict) or origins.get(operation) != "pillow_fixture":
        fail(f"{property_id}: witness {operation}:{row_id} is not Pillow-origin for {operation}")
    if operation == "decode":
        if row.get("oracle_status") not in {"ok", "error"}:
            fail(f"{property_id}: witness {operation}:{row_id} has no live oracle status")
    elif row.get("operations", {}).get("encode") != "ok":
        fail(f"{property_id}: witness {operation}:{row_id} has no live encode status")

    path, asset, expected_digest = fixture_path(operation, row)
    if not path.is_file():
        fail(f"{property_id}: witness {operation}:{row_id} asset is missing: {path}")
    if sha256(path) != expected_digest:
        fail(f"{property_id}: witness {operation}:{row_id} asset hash differs from matrix")

    artifact_path = row.get("encoded_ref_path")
    artifact_digest = row.get("encoded_ref_sha256")
    if operation == "encode":
        if not isinstance(artifact_path, str) or not artifact_path:
            fail(f"{property_id}: encode witness {row_id} has no encoded_ref_path")
        if not isinstance(artifact_digest, str) or not HEX64_RE.fullmatch(artifact_digest):
            fail(f"{property_id}: encode witness {row_id} has no encoded_ref_sha256")
        artifact = ROOT / artifact_path
        if not artifact.is_file() or sha256(artifact) != artifact_digest:
            fail(f"{property_id}: encode witness {row_id} encoded artifact hash differs")

    seen.add(key)
    return operation, asset


def contains_all(actual: object, expected: object, label: str) -> None:
    if not isinstance(actual, list) or not isinstance(expected, list):
        fail(f"{label}: expected list-valued structural evidence")
    missing = [value for value in expected if value not in actual]
    if missing:
        fail(f"{label}: structural evidence is missing {missing!r}")


def verify_structure(expect: object, structure: dict, label: str) -> None:
    if not isinstance(expect, dict):
        fail(f"{label}: expect must be an object")
    if "width" in expect and structure.get("width") != expect["width"]:
        fail(f"{label}: width differs from structural witness")
    if "height" in expect and structure.get("height") != expect["height"]:
        fail(f"{label}: height differs from structural witness")
    if "alpha_used" in expect and structure.get("alpha_used") != expect["alpha_used"]:
        fail(f"{label}: alpha_used differs from structural witness")
    if "transforms" in expect and structure.get("transforms") != expect["transforms"]:
        fail(f"{label}: transform sequence differs from structural witness")
    if "transforms_contains" in expect:
        actual = structure.get("transforms")
        contains_all(actual, expect["transforms_contains"], f"{label}: transforms")

    streams = structure.get("image_streams")
    if not isinstance(streams, list):
        fail(f"{label}: inspector returned no image streams")

    def verify_stream(stream: object, stream_expect: object, stream_label: str) -> None:
        if not isinstance(stream, dict) or not isinstance(stream_expect, dict):
            fail(f"{stream_label}: stream evidence must be an object")
        for key in ("color_cache_bits", "meta_huffman_bits", "entropy_image_size", "huffman_groups"):
            if key in stream_expect and stream.get(key) != stream_expect[key]:
                fail(f"{stream_label}: {key} differs from structural witness")
        for key in ("green_values_contains", "distance_prefixes_contains", "plane_codes_contains", "mapped_distances_contains"):
            if key in stream_expect:
                source_key = key.removesuffix("_contains")
                contains_all(stream.get(source_key), stream_expect[key], f"{stream_label}: {source_key}")
        if "cache_lookups_at_least" in stream_expect:
            actual = stream.get("cache_lookups")
            if not isinstance(actual, int) or actual < stream_expect["cache_lookups_at_least"]:
                fail(f"{stream_label}: cache lookup count is below the structural witness")
        if "tree_forms_at_least" in stream_expect:
            actual = stream.get("tree_forms")
            if not isinstance(actual, dict):
                fail(f"{stream_label}: tree_forms are absent")
            for form, count in stream_expect["tree_forms_at_least"].items():
                if not isinstance(count, int) or not isinstance(actual.get(form), int) or actual[form] < count:
                    fail(f"{stream_label}: tree form {form!r} is below the structural witness")

    if "stream_index" in expect:
        index = expect["stream_index"]
        if not isinstance(index, int) or not 0 <= index < len(streams):
            fail(f"{label}: stream_index is outside the inspected stream list")
        verify_stream(streams[index], expect.get("stream", {}), f"{label}: stream[{index}]")
    if "any_stream" in expect:
        stream_expect = expect["any_stream"]
        if not any(
            isinstance(stream, dict)
            and all(
                key in stream and stream[key] == value
                for key, value in stream_expect.items()
                if key in {"color_cache_bits", "meta_huffman_bits", "entropy_image_size", "huffman_groups"}
            )
            and all(
                isinstance(stream.get(key.removesuffix("_contains")), list)
                and all(value in stream[key.removesuffix("_contains")] for value in values)
                for key, values in stream_expect.items()
                if key.endswith("_contains")
            )
            and all(
                isinstance(stream.get("tree_forms"), dict)
                and stream["tree_forms"].get(form, 0) >= count
                for form, count in stream_expect.get("tree_forms_at_least", {}).items()
            )
            for stream in streams
        ):
            fail(f"{label}: no image stream matches any_stream structural witness")


def verify_structural_witnesses(
    document: dict,
    rows: dict[tuple[str, str], dict],
    seen: set[tuple[str, str]],
) -> int:
    witnesses = document.get("structural_witnesses")
    if not isinstance(witnesses, list) or not witnesses:
        fail("structural_witnesses must be a non-empty array")
    property_ids = {
        entry.get("id")
        for entry in document.get("properties", [])
        if isinstance(entry, dict)
    }
    count = 0
    for witness in witnesses:
        if not isinstance(witness, dict):
            fail("structural witnesses must be objects")
        property_id = witness.get("property_id")
        if property_id not in property_ids:
            fail(f"structural witness references unknown property {property_id!r}")
        operation = witness.get("operation")
        row_id = witness.get("row_id")
        if operation not in OPERATIONS or not isinstance(row_id, str):
            fail(f"{property_id}: structural witnesses must reference active decode/encode rows")
        verify_witness(
            {"operation": operation, "row_id": row_id},
            str(property_id),
            rows,
            seen,
        )
        row = rows[(operation, row_id)]
        if operation == "decode":
            path, _, _ = fixture_path(operation, row)
        else:
            artifact_path = row.get("encoded_ref_path")
            if not isinstance(artifact_path, str) or not artifact_path:
                fail(f"{property_id}: encode structural witness {row_id} has no encoded artifact")
            path = ROOT / artifact_path
            if not path.is_file():
                fail(f"{property_id}: encode structural witness {row_id} artifact is missing")
        try:
            structure = inspect_path(path)
        except (OSError, ParseError) as error:
            fail(f"{property_id}: structural parse failed for {row_id}: {error}")
        verify_structure(witness.get("expect"), structure, f"{property_id}:{row_id}")
        count += 1
    return count


def verify_malformed_witnesses(
    document: dict,
    rows: dict[tuple[str, str], dict],
    seen: set[tuple[str, str]],
) -> int:
    groups = document.get("malformed_witnesses")
    if not isinstance(groups, list) or not groups:
        fail("malformed_witnesses must be a non-empty array")
    property_ids = {
        entry.get("id")
        for entry in document.get("properties", [])
        if isinstance(entry, dict)
    }
    accounted: set[tuple[str, str]] = set()
    count = 0
    for group in groups:
        if not isinstance(group, dict):
            fail("malformed witness groups must be objects")
        property_id = group.get("property_id")
        if property_id not in property_ids:
            fail(f"malformed witness references unknown property {property_id!r}")
        witnesses = group.get("witnesses")
        if not isinstance(witnesses, list) or not witnesses:
            fail(f"{property_id}: malformed witnesses must be a non-empty array")
        for witness in witnesses:
            if not isinstance(witness, dict):
                fail(f"{property_id}: malformed witness must be an object")
            operation = witness.get("operation")
            row_id = witness.get("row_id")
            if operation != "decode" or not isinstance(row_id, str) or not row_id:
                fail(f"{property_id}: malformed witnesses must reference decode rows")
            key = (operation, row_id)
            if key in accounted:
                fail(f"malformed witness row is listed more than once: {operation}:{row_id}")
            verify_witness(witness, str(property_id), rows, seen)
            accounted.add(key)
            row = rows[key]
            path, _, _ = fixture_path(operation, row)
            expect = witness.get("expect")
            if not isinstance(expect, dict):
                fail(f"{property_id}:{row_id}: malformed expect must be an object")
            expected_status = expect.get("status")
            if expected_status == "error":
                expected_code = expect.get("error_code")
                if not isinstance(expected_code, str) or not expected_code:
                    fail(f"{property_id}:{row_id}: error_code is required")
                try:
                    inspect_path(path)
                except ParseError as error:
                    if error.code != expected_code:
                        fail(
                            f"{property_id}:{row_id}: parser code {error.code!r} "
                            f"differs from {expected_code!r}"
                        )
                    if "error_phase" in expect and error.phase != expect["error_phase"]:
                        fail(
                            f"{property_id}:{row_id}: parser phase {error.phase!r} "
                            f"differs from {expect['error_phase']!r}"
                        )
                    if "bit_offset" in expect and error.bit_offset != expect["bit_offset"]:
                        fail(
                            f"{property_id}:{row_id}: parser bit offset {error.bit_offset!r} "
                            f"differs from {expect['bit_offset']!r}"
                        )
                except (OSError, ValueError) as error:
                    fail(f"{property_id}:{row_id}: independent parser could not classify error: {error}")
                else:
                    fail(f"{property_id}:{row_id}: malformed parser unexpectedly accepted fixture")
            elif expected_status == "ok":
                try:
                    structure = inspect_path(path)
                except (OSError, ParseError) as error:
                    fail(f"{property_id}:{row_id}: tolerated fixture did not parse: {error}")
                verify_structure(
                    expect.get("structure", {}),
                    structure,
                    f"{property_id}:{row_id}",
                )
            else:
                fail(f"{property_id}:{row_id}: malformed expect.status must be error or ok")
            count += 1

    expected_rows = {
        key
        for key, row in rows.items()
        if key[0] == "decode"
        and "vp8l" in key[1]
        and (
            key[1].startswith("error_malformed_container_")
            or key[1].startswith("pillow_tolerated_malformed_")
        )
    }
    if accounted != expected_rows:
        missing = sorted(expected_rows - accounted)
        extra = sorted(accounted - expected_rows)
        fail(f"VP8L malformed witness coverage differs: missing={missing}, extra={extra}")
    return count


def verify_lossless_success_corpus(rows: dict[tuple[str, str], dict]) -> int:
    parsed = 0
    for (operation, row_id), row in rows.items():
        if operation != "decode" or row.get("category") != "lossless" or row.get("oracle_status") != "ok":
            continue
        path, _, _ = fixture_path(operation, row)
        try:
            inspect_path(path)
        except (OSError, ParseError) as error:
            fail(f"lossless VP8L success row {row_id} cannot be structurally parsed: {error}")
        parsed += 1
    if parsed == 0:
        fail("no active lossless VP8L success rows were found")
    return parsed


def verify_properties(document: dict, rows: dict[tuple[str, str], dict]) -> tuple[int, int, Counter[str], set[tuple[str, str]]]:
    properties = document.get("properties")
    if not isinstance(properties, list) or not properties:
        fail("properties must be a non-empty array")

    ids: set[str] = set()
    seen: set[tuple[str, str]] = set()
    status_counts: Counter[str] = Counter()
    witness_count = 0
    for property_entry in properties:
        if not isinstance(property_entry, dict):
            fail("property entries must be objects")
        property_id = property_entry.get("id")
        if not isinstance(property_id, str) or not property_id or property_id in ids:
            fail(f"invalid or duplicate property id: {property_id!r}")
        ids.add(property_id)

        status = property_entry.get("status")
        if status not in STATUSES:
            fail(f"{property_id}: unsupported status {status!r}")
        status_counts[status] += 1
        boundary = property_entry.get("claim_boundary")
        if boundary not in BOUNDARIES:
            fail(f"{property_id}: unsupported claim_boundary {boundary!r}")
        if not isinstance(property_entry.get("gap"), str) or not property_entry["gap"].strip():
            fail(f"{property_id}: gap must be a non-empty explanation")
        internal_proven = property_entry.get("internal_state_proven")
        if not isinstance(internal_proven, bool):
            fail(f"{property_id}: internal_state_proven must be boolean")
        if status == "candidate" and internal_proven:
            fail(f"{property_id}: candidate properties cannot claim internal proof")
        if status == "witnessed" and not internal_proven:
            fail(f"{property_id}: witnessed properties must declare internal proof")
        if boundary == "rust_only" and status != "witnessed":
            fail(f"{property_id}: rust_only boundaries require a witnessed structural contract")

        witnesses = property_entry.get("minimal_witnesses")
        if not isinstance(witnesses, list):
            fail(f"{property_id}: minimal_witnesses must be an array")
        if status != "unmapped" and not witnesses:
            fail(f"{property_id}: mapped/candidate properties require a named witness")
        if status == "unmapped" and witnesses:
            fail(f"{property_id}: unmapped properties cannot name unverified fixtures")
        for witness in witnesses:
            verify_witness(witness, property_id, rows, seen)
            witness_count += 1
        if status == "witnessed":
            structural_entries = document.get("structural_witnesses")
            structural_keys = {
                (entry.get("operation"), entry.get("row_id"))
                for entry in structural_entries
                if isinstance(entry, dict) and entry.get("property_id") == property_id
            } if isinstance(structural_entries, list) else set()
            malformed_entries = document.get("malformed_witnesses")
            malformed_keys = {
                (witness.get("operation"), witness.get("row_id"))
                for group in malformed_entries
                if isinstance(group, dict) and group.get("property_id") == property_id
                for witness in group.get("witnesses", [])
                if isinstance(witness, dict)
            } if isinstance(malformed_entries, list) else set()
            covered_keys = structural_keys | malformed_keys
            missing = [
                (witness.get("operation"), witness.get("row_id"))
                for witness in witnesses
                if (witness.get("operation"), witness.get("row_id")) not in covered_keys
            ]
            if missing:
                fail(
                    f"{property_id}: witnessed property is missing structural or "
                    f"malformed-parser coverage for {missing!r}"
                )

    return len(properties), witness_count, status_counts, seen


def main() -> int:
    try:
        document = read_json(MAP_PATH)
        matrix = read_json(MATRIX_PATH)
        verify_inputs(document)
        rows = matrix_rows(matrix)
        property_count, witness_count, status_counts, seen = verify_properties(document, rows)
        structural_count = verify_structural_witnesses(document, rows, seen)
        malformed_count = verify_malformed_witnesses(document, rows, seen)
        corpus_count = verify_lossless_success_corpus(rows)
    except RuntimeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    print(
        "webp VP8L property map OK: "
        f"{property_count} properties, {witness_count} named witnesses, "
        f"{len(seen)} distinct active WebP rows, {structural_count} structural witnesses checked; "
        f"{malformed_count} malformed parser witnesses checked; "
        f"parsed all {corpus_count} active lossless success rows; "
        f"statuses={dict(sorted(status_counts.items()))}; "
        "row claims remain Pillow outer-result evidence; structural facts are independently parsed"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
