#!/usr/bin/env python3
"""Regression checks for coverage floors and release identity guards."""

from __future__ import annotations

import contextlib
import copy
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import verify_llvm_coverage as coverage
import publish_release


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
    def test_local_or_wrong_tag_context_cannot_publish(self) -> None:
        context = {
            "GITHUB_ACTIONS": "true",
            "GITHUB_REPOSITORY": "appunni-m/image-slash-star",
            "GITHUB_REF": "refs/tags/v0.1.1",
            "ACTIONS_ID_TOKEN_REQUEST_URL": "https://example.invalid/oidc",
            "CARGO_REGISTRY_TOKEN": "test-placeholder",
            "RELEASE_CI_SHA": "a" * 40,
        }
        with patch.object(publish_release, "capture", side_effect=["", "a" * 40] * 3), \
             patch.object(publish_release, "package_version", return_value="0.1.1"):
            with patch.dict(os.environ, {}, clear=True):
                with self.assertRaises(publish_release.PublishError):
                    publish_release.require_publish_approval()
            with patch.dict(os.environ, context, clear=True):
                publish_release.require_publish_approval()
            context["GITHUB_REF"] = "refs/heads/main"
            with patch.dict(os.environ, context, clear=True):
                with self.assertRaises(publish_release.PublishError):
                    publish_release.require_publish_approval()


if __name__ == "__main__":
    unittest.main()
