#!/usr/bin/env python3
"""Regression checks for coverage floors and release identity guards."""

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
import ssl
import sys
import tempfile
import subprocess
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import verify_llvm_coverage as coverage
import check_workflows
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

    def test_local_or_wrong_tag_context_cannot_publish(self) -> None:
        context = {
            "GITHUB_ACTIONS": "true",
            "GITHUB_REPOSITORY": "appunni-m/image-slash-star",
            "GITHUB_REF": "refs/tags/v0.1.1",
            "GITHUB_REF_NAME": "v0.1.1",
            "GITHUB_SHA": "a" * 40,
            "ACTIONS_ID_TOKEN_REQUEST_URL": "https://example.invalid/oidc",
            "CARGO_REGISTRY_TOKEN": "test-placeholder",
            "RELEASE_CI_SHA": "a" * 40,
        }
        with patch.object(publish_release, "capture", side_effect=["", "a" * 40] * 3), \
             patch.object(publish_release, "package_version", return_value="0.1.1"), \
             patch.object(publish_release, "verify_remote_tag") as verify_tag:
            with patch.dict(os.environ, {}, clear=True):
                with self.assertRaises(publish_release.PublishError):
                    publish_release.require_publish_approval()
            with patch.dict(os.environ, context, clear=True):
                publish_release.require_publish_approval()
            verify_tag.assert_called_once_with(
                "v0.1.1", "a" * 40, capture_command=publish_release.capture
            )
            context["GITHUB_REF"] = "refs/heads/main"
            with patch.dict(os.environ, context, clear=True):
                with self.assertRaises(publish_release.PublishError):
                    publish_release.require_publish_approval()


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
