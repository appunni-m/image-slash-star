#!/usr/bin/env python3
"""Require immutable external actions and validate workflows with actionlint."""
from __future__ import annotations

import errno
import hashlib
import http.client
import io
import os
from collections.abc import Callable
from pathlib import Path
import platform
import re
import socket
import ssl
import subprocess
import sys
import tarfile
import tempfile
import textwrap
import time
import urllib.error
import urllib.request

from verify_ci_results import RESULT_ENV_BY_JOB


VERSION = "1.7.12"
DOWNLOAD_ATTEMPTS = 3
DOWNLOAD_RETRY_BACKOFF_SECONDS = 1
TRANSIENT_HTTP_STATUS_CODES = frozenset({408, 425, 429, 500, 502, 503, 504})
TRANSIENT_NETWORK_ERRNOS = frozenset(
    {
        errno.EAGAIN,
        errno.ECONNABORTED,
        errno.ECONNREFUSED,
        errno.ECONNRESET,
        errno.EHOSTUNREACH,
        errno.ENETDOWN,
        errno.ENETUNREACH,
        errno.EPIPE,
        errno.ETIMEDOUT,
        socket.EAI_AGAIN,
    }
)
# Official release checksums: https://github.com/rhysd/actionlint/releases/tag/v1.7.12
ARCHIVES = {
    ("Darwin", "arm64"): ("darwin_arm64", "aba9ced2dee8d27fecca3dc7feb1a7f9a52caefa1eb46f3271ea66b6e0e6953f"),
    ("Darwin", "x86_64"): ("darwin_amd64", "5b44c3bc2255115c9b69e30efc0fecdf498fdb63c5d58e17084fd5f16324c644"),
    ("Linux", "x86_64"): ("linux_amd64", "8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8"),
    ("Linux", "aarch64"): ("linux_arm64", "325e971b6ba9bfa504672e29be93c24981eeb1c07576d730e9f7c8805afff0c6"),
}
GITHUB_RELEASE_PUBLICATION_SCRIPT_SHA256 = (
    "dea46fa0fe054ca1319e4500f57d4d47bc0e4e0b55aeee391708236ce30a8012"
)
ROOT = Path(__file__).resolve().parent.parent
PINNED_ACTION_RE = re.compile(
    r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+(?:/[^@\s]+)?@[0-9a-f]{40}\Z"
)
USES_ENTRY_RE = re.compile(
    r"(?:^|[,{])\s*(?:-\s*)?(?:'uses'|\"uses\"|uses)\s*:\s*"
    r"(?P<value>'[^']*'|\"[^\"]*\"|[^,\s}#]+)"
)


def unpinned_action_uses(contents: str) -> list[tuple[int, str]]:
    """Return workflow `uses` references that are not local or SHA-pinned."""

    violations = []
    for line_number, line in enumerate(contents.splitlines(), start=1):
        for match in USES_ENTRY_RE.finditer(line):
            reference = match.group("value").strip()
            if (
                len(reference) >= 2
                and reference[0] == reference[-1]
                and reference[0] in "\"'"
            ):
                reference = reference[1:-1]
            if reference.startswith("./"):
                continue
            if not PINNED_ACTION_RE.fullmatch(reference):
                violations.append((line_number, reference))
    return violations


def verify_action_pins(workflows: list[Path]) -> bool:
    """Reject mutable external action refs before running actionlint."""

    violations = []
    for workflow in workflows:
        for line_number, reference in unpinned_action_uses(
            workflow.read_text(encoding="utf-8")
        ):
            violations.append(f"{workflow.relative_to(ROOT)}:{line_number}: {reference}")
    if not violations:
        return True
    print(
        "Workflow action pinning failed; use full 40-character commit SHAs:",
        file=sys.stderr,
    )
    for violation in violations:
        print(f"  {violation}", file=sys.stderr)
    return False


def run_script(step: str) -> str | None:
    """Return a workflow step's shell script without YAML block indentation."""

    run_match = re.search(r"(?m)^ {8}run:\s*(?P<value>.*)$", step)
    if run_match is None:
        return None

    first_line = run_match.group("value").strip()
    if first_line and first_line[0] not in "|>":
        return first_line

    script_lines = []
    for line in step[run_match.end():].splitlines():
        if not line.strip():
            script_lines.append("")
            continue
        if not line.startswith(" " * 10):
            break
        script_lines.append(line[10:])
    return "\n".join(script_lines)


def make_coverage_command_lines(script: str) -> list[str]:
    """Find actual `make coverage` command lines, excluding comments and text."""

    command = re.compile(
        r"^(?:(?:[A-Za-z_][A-Za-z0-9_]*=\S+)\s+)*"
        r"make\s+coverage(?=$|[\s;&|])"
    )
    return [
        line.strip()
        for line in script.splitlines()
        if line.strip()
        and not line.lstrip().startswith("#")
        and command.match(line.strip())
    ]


def workflow_jobs(contents: str) -> dict[str, str] | None:
    """Return job bodies from a workflow without depending on a YAML package."""

    jobs_match = re.search(r"(?m)^jobs:\s*$", contents)
    if jobs_match is None:
        return None

    jobs_body = contents[jobs_match.end():]
    headers = list(re.finditer(r"(?m)^  ([A-Za-z0-9_-]+):\s*$", jobs_body))
    jobs = {}
    for index, header in enumerate(headers):
        end = headers[index + 1].start() if index + 1 < len(headers) else len(jobs_body)
        jobs[header.group(1)] = jobs_body[header.end():end]
    return jobs


def workflow_job_condition(job: str) -> str | None:
    """Return a job-level `if` expression, unfolding a YAML block scalar."""

    lines = job.splitlines()
    for index, line in enumerate(lines):
        if not line.startswith("    if:"):
            continue
        value = line.split(":", maxsplit=1)[1].strip()
        if value not in {">", ">-", "|", "|-", "|+", ">+"}:
            return value

        expression = []
        for continuation in lines[index + 1:]:
            if not continuation.strip():
                continue
            indentation = len(continuation) - len(continuation.lstrip())
            if indentation < 6:
                break
            expression.append(continuation.strip())
        return " ".join(expression)
    return None


def normalized_expression(expression: str) -> str:
    """Collapse YAML line wrapping while preserving expression tokens."""

    return " ".join(expression.split())


CI_COVERAGE_CONDITION = (
    "github.event_name == 'schedule' || "
    "github.event_name == 'workflow_dispatch'"
)
CI_REQUIRED_CHECK_RESULTS = RESULT_ENV_BY_JOB
CI_REQUIRED_CHECK_JOB_ID = "required-checks"
CI_DOCUMENTATION_REQUIRED_COMMANDS = (
    "make docs-test docs-lint docs-build PYTHON=.venv-docs/bin/python",
    "make docs-examples",
    "make docs-registry-examples PYTHON=python",
)
APPLE_ARM64_ORACLE_JOBS = {
    "ci.yml": ("quality", "coverage"),
    "benchmark.yml": ("benchmark",),
    "release.yml": ("release-coverage",),
}


def apple_arm64_runner_violations(workflows: dict[str, str]) -> list[str]:
    """Pin oracle jobs to the Apple ARM64 image used for fixture capture."""

    violations = []
    for workflow_name, job_names in APPLE_ARM64_ORACLE_JOBS.items():
        contents = workflows.get(workflow_name)
        if contents is None:
            violations.append(f"missing {workflow_name} workflow")
            continue

        jobs = workflow_jobs(contents)
        if jobs is None:
            violations.append(f"{workflow_name} workflow has no jobs section")
            continue

        for job_name in job_names:
            job = jobs.get(job_name)
            if job is None:
                violations.append(
                    f"{workflow_name} workflow has no {job_name} job"
                )
                continue

            if not re.search(r"(?m)^    runs-on: macos-15\s*$", job):
                violations.append(
                    f"{workflow_name} {job_name} must run on macos-15"
                )

            steps = re.split(r"(?m)^      - ", job)
            architecture_step = next(
                (
                    index
                    for index, step in enumerate(steps)
                    if run_script(step) == 'test "$(uname -m)" = arm64'
                ),
                None,
            )
            if architecture_step is None:
                violations.append(
                    f"{workflow_name} {job_name} must verify an arm64 runner"
                )
                continue

            for step in steps[:architecture_step]:
                action = re.search(r"(?m)^(?:        )?uses:\s*(\S+)", step)
                if run_script(step) is not None or (
                    action and not action.group(1).startswith("actions/checkout@")
                ):
                    violations.append(
                        f"{workflow_name} {job_name} must verify arm64 "
                        "before substantive steps"
                    )
                    break

    return violations


def coverage_event_is_allowed(event_name: str) -> bool:
    """Allow aggregate CI coverage only on scheduled or manual runs."""

    return event_name in {"schedule", "workflow_dispatch"}


def ci_coverage_gate_violations(contents: str) -> list[str]:
    """Keep the expensive aggregate CI coverage lane opt-in or scheduled."""

    jobs = workflow_jobs(contents)
    if jobs is None:
        return ["CI workflow has no jobs section"]

    violations = []
    for event_name in ("schedule", "workflow_dispatch"):
        if not re.search(rf"(?m)^  {re.escape(event_name)}:\s*$", contents):
            violations.append(f"CI workflow must declare the {event_name} trigger")

    coverage_job = jobs.get("coverage")
    if coverage_job is None:
        return [*violations, "CI workflow has no aggregate coverage job"]

    condition = workflow_job_condition(coverage_job)
    if condition is None or normalized_expression(condition) != normalized_expression(
        CI_COVERAGE_CONDITION
    ):
        violations.append(
            "CI aggregate coverage must run only on schedule or workflow_dispatch"
        )

    coverage_commands = []
    for step in re.split(r"(?m)^      - ", coverage_job):
        script = run_script(step)
        if script is not None:
            coverage_commands.extend(make_coverage_command_lines(script))
    if not coverage_commands:
        violations.append("CI aggregate coverage job must execute make coverage")
    return violations


def ci_required_checks_violations(contents: str) -> list[str]:
    """Keep the stable required status tied to every mandatory CI result."""

    jobs = workflow_jobs(contents)
    if jobs is None:
        return ["CI workflow has no jobs section"]

    job = jobs.get(CI_REQUIRED_CHECK_JOB_ID)
    if job is None:
        return ["CI workflow has no required-checks job"]

    violations = []
    expected_job_ids = {*CI_REQUIRED_CHECK_RESULTS, CI_REQUIRED_CHECK_JOB_ID}
    unclassified_jobs = sorted(set(jobs) - expected_job_ids)
    if unclassified_jobs:
        violations.append(
            "CI workflow jobs are not classified by required-checks: "
            f"{unclassified_jobs}"
        )

    documentation_job = jobs.get("documentation")
    if documentation_job is None:
        violations.append("CI workflow has no documentation job")
    else:
        violations.extend(ci_documentation_job_violations(documentation_job))

    for required_job_name in CI_REQUIRED_CHECK_RESULTS:
        if required_job_name == "documentation":
            continue
        required_job = jobs.get(required_job_name)
        if required_job is not None and re.search(
            r"(?m)^ {4,8}continue-on-error:", required_job
        ):
            violations.append(
                f"CI required job {required_job_name} and steps must not allow errors"
            )

    condition = workflow_job_condition(job)
    if condition is None or normalized_expression(condition) != "always()":
        violations.append("CI required-checks job must run with if: always()")
    if re.search(r"(?m)^    continue-on-error:", job):
        violations.append("CI required-checks job must not allow errors")

    needs_match = re.search(
        r"(?m)^    needs:\s*\n(?P<items>(?:^      - [A-Za-z0-9_-]+\s*\n)+)",
        job,
    )
    expected_jobs = set(CI_REQUIRED_CHECK_RESULTS)
    if needs_match is None:
        violations.append("CI required-checks job must list all required jobs in needs")
        actual_jobs = set()
    else:
        needed_job_names = [
            match.group(1)
            for match in re.finditer(
                r"(?m)^      - ([A-Za-z0-9_-]+)\s*$", needs_match.group("items")
            )
        ]
        actual_jobs = set(needed_job_names)
        if len(needed_job_names) != len(actual_jobs):
            violations.append("CI required-checks dependencies must not contain duplicates")
        if actual_jobs != expected_jobs:
            missing = sorted(expected_jobs - actual_jobs)
            unexpected = sorted(actual_jobs - expected_jobs)
            violations.append(
                "CI required-checks dependencies differ: "
                f"missing={missing}, unexpected={unexpected}"
            )

    steps = re.split(r"(?m)^      - ", job)
    gate_steps = [
        step
        for step in steps
        if re.search(
            r"(?m)^name: Require every applicable CI job to pass$", step
        )
    ]
    if len(gate_steps) != 1:
        return [
            *violations,
            "CI required-checks job must have one result-gate step",
        ]

    gate_step = gate_steps[0]
    gate_index = steps.index(gate_step)
    checkout_indices = [
        index
        for index, step in enumerate(steps)
        if re.search(r"(?m)^uses: actions/checkout@", step)
    ]
    if not checkout_indices or min(checkout_indices) > gate_index:
        violations.append("CI required-checks must check out the script before running it")
    if re.search(r"(?m)^        if:", gate_step):
        violations.append("CI required-checks result-gate step must not be conditional")
    if re.search(r"(?m)^        continue-on-error:", gate_step):
        violations.append("CI required-checks result-gate step must not allow errors")
    for job_name, variable in CI_REQUIRED_CHECK_RESULTS.items():
        source = f"needs.{job_name}.result"
        if not re.search(
            rf"(?m)^          {variable}: \$\{{\{{ {re.escape(source)} \}}\}}$",
            gate_step,
        ):
            violations.append(
                f"CI required-checks must map {variable} from {source}"
            )
    if not re.search(
        r"(?m)^          EVENT_NAME: \$\{\{ github\.event_name \}\}$",
        gate_step,
    ):
        violations.append("CI required-checks must map EVENT_NAME from github.event_name")

    script = run_script(gate_step)
    if script != "python3 scripts/verify_ci_results.py":
        violations.append(
            "CI required-checks must invoke the tested result gate directly"
        )
    return violations


def ci_documentation_job_violations(job: str) -> list[str]:
    """Keep every required documentation check active and fail-closed on PRs."""

    violations = []
    condition = workflow_job_condition(job)
    if (
        condition is None
        or normalized_expression(condition)
        != "github.event_name == 'pull_request'"
    ):
        violations.append(
            "CI documentation job must run only for pull_request events"
        )

    if re.search(r"(?m)^ {4,8}continue-on-error:", job):
        violations.append("CI documentation job and steps must not allow errors")

    steps = re.split(r"(?m)^      - ", job)
    for command in CI_DOCUMENTATION_REQUIRED_COMMANDS:
        command_steps = [step for step in steps if run_script(step) == command]
        if len(command_steps) != 1:
            violations.append(
                "CI documentation job must execute exactly once: "
                f"{command}"
            )
        elif re.search(r"(?m)^        if:", command_steps[0]):
            violations.append(
                "CI documentation job must not conditionally skip: "
                f"{command}"
            )

    return violations


CI_X86_COVERAGE_PROFILES = (
    (
        "SSE2",
        "Run x86-64 public JPEG and AVIF parity with branch coverage",
        "",
    ),
    (
        "AVX2",
        "Run AVX2 public JPEG and AVIF parity with branch coverage",
        "-avx2",
    ),
)
CI_X86_COVERAGE_RUNS = (
    ("coverage-jpeg", "test_decode_matrix_jpeg"),
    ("coverage-jpeg-encode", "test_encode_matrix_jpeg"),
    ("coverage-avif", "test_decode_matrix_avif"),
    ("coverage-avif-encode", "test_encode_matrix_avif"),
    ("coverage-fault-contract", "test_fault_contract_matrix"),
)


def ci_x86_coverage_violations(contents: str) -> list[str]:
    """Require every SSE2 and AVX2 parity report to be run, checked, and uploaded."""

    jobs = workflow_jobs(contents)
    if jobs is None:
        return ["CI workflow has no jobs section"]

    job = jobs.get("x86-simd-lint")
    if job is None:
        return ["CI workflow has no x86 SIMD job"]

    steps = re.split(r"(?m)^      - ", job)
    violations = []

    selector_checks = [
        step
        for step in steps
        if re.search(
            r"(?m)^name: Verify x86 coverage selectors resolve to runnable tests$",
            step,
        )
    ]
    if len(selector_checks) != 1:
        violations.append(
            "CI x86 coverage selector check must be defined exactly once"
        )
    else:
        selector_script = run_script(selector_checks[0])
        if (
            selector_script is None
            or "-- --list" not in selector_script
            or "grep -Fqx" not in selector_script
        ):
            violations.append(
                "CI x86 coverage selector check must list runnable tests exactly"
            )
        else:
            for _, test_name in CI_X86_COVERAGE_RUNS:
                if selector_script.count(test_name) != 1:
                    violations.append(
                        "CI x86 coverage selector check must require "
                        f"{test_name} exactly once"
                    )

    reports = []
    for profile, step_name, suffix in CI_X86_COVERAGE_PROFILES:
        matching_steps = [
            step
            for step in steps
            if re.search(rf"(?m)^name: {re.escape(step_name)}$", step)
        ]
        if len(matching_steps) != 1:
            violations.append(f"CI x86 SIMD job must define one {profile} parity step")
            continue

        script = run_script(matching_steps[0])
        if script is None:
            violations.append(f"CI {profile} parity step must run its coverage commands")
            continue
        normalized_script = " ".join(script.replace("\\\n", " ").split())
        for report_prefix, test_name in CI_X86_COVERAGE_RUNS:
            report = f"{report_prefix}-x86_64{suffix}.json"
            reports.append(report)
            expected_command = (
                'cargo +"$COVERAGE_TOOLCHAIN" llvm-cov test --all-features --branch '
                "--locked --json --output-path "
                f"target/release-evidence/{report} --test coverage_matrix_tests "
                f"{test_name} -- --exact"
            )
            if normalized_script.count(expected_command) != 1:
                violations.append(
                    f"CI {profile} parity step must produce exactly once: {report}"
                )

    report_check = next(
        (
            run_script(step)
            for step in steps
            if re.search(
                r"(?m)^name: Verify all x86 coverage reports were produced$",
                step,
            )
        ),
        None,
    )
    if report_check is None or 'test -s "$report"' not in report_check:
        violations.append("CI coverage verification must reject missing or empty reports")
    else:
        for report in reports:
            path = f"target/release-evidence/{report}"
            if report_check.count(path) != 1:
                violations.append(f"CI coverage verification must require {report}")

    upload_step = next(
        (
            step
            for step in steps
            if re.search(r"(?m)^uses: actions/upload-artifact@", step)
        ),
        None,
    )
    if upload_step is None:
        violations.append("CI x86 artifact upload step is missing")
    else:
        for report in reports:
            path = f"target/release-evidence/{report}"
            if upload_step.count(path) != 1:
                violations.append(f"CI x86 artifact must retain {report}")
    return violations


DOCS_TRUSTED_WORKFLOW_RUN_GATE = (
    "github.event.workflow_run.conclusion == 'success' && "
    "github.event.workflow_run.head_repository.full_name == github.repository && "
    "((github.event.workflow_run.name == 'Benchmark' && "
    "github.event.workflow_run.head_branch == 'main' && "
    "(github.event.workflow_run.event == 'push' || "
    "github.event.workflow_run.event == 'schedule' || "
    "github.event.workflow_run.event == 'workflow_dispatch')) || "
    "(github.event.workflow_run.name == 'Release' && "
    "github.event.workflow_run.event == 'push' && "
    "startsWith(github.event.workflow_run.head_branch, 'v')))"
)
DOCS_WORKFLOW_RUN_GUARD = (
    "github.event_name != 'workflow_run' || "
    f"({DOCS_TRUSTED_WORKFLOW_RUN_GATE})"
)
DOCS_INELIGIBLE_RUN_CONCURRENCY_GUARD = (
    "github.event_name == 'workflow_run' && "
    f"!({DOCS_TRUSTED_WORKFLOW_RUN_GATE}) && "
    "format('ineligible-workflow-run-{0}', github.run_id)"
)
DOCS_JOB_CONDITIONS = {
    "benchmark-data": (
        "github.event_name != 'pull_request' && "
        f"({DOCS_WORKFLOW_RUN_GUARD})"
    ),
    "build": (
        "github.event_name != 'pull_request' && "
        "always() && "
        "needs.benchmark-data.result == 'success' && "
        f"({DOCS_WORKFLOW_RUN_GUARD})"
    ),
    "examples": (
        "github.event_name != 'pull_request' && "
        f"({DOCS_WORKFLOW_RUN_GUARD})"
    ),
    "deploy": (
        "github.ref == 'refs/heads/main' && "
        "github.event_name != 'pull_request' && "
        f"({DOCS_WORKFLOW_RUN_GUARD})"
    ),
}


def docs_workflow_run_is_publishable(
    event_name: str, workflow_run: dict, repository: str
) -> bool:
    """Model the trusted workflow_run sources that may publish documentation."""

    head_repository = workflow_run.get("head_repository")
    if (
        event_name != "workflow_run"
        or workflow_run.get("status") != "completed"
        or workflow_run.get("conclusion") != "success"
        or not isinstance(head_repository, dict)
        or head_repository.get("full_name") != repository
    ):
        return False

    name = workflow_run.get("name")
    branch = workflow_run.get("head_branch")
    event = workflow_run.get("event")
    if name == "Benchmark":
        return branch == "main" and event in {
            "push",
            "schedule",
            "workflow_dispatch",
        }
    if name == "Release":
        return isinstance(branch, str) and branch.startswith("v") and event == "push"
    return False


def docs_workflow_run_publication_violations(contents: str) -> list[str]:
    """Require every docs workflow_run publication path to use trusted sources."""

    jobs = workflow_jobs(contents)
    if jobs is None:
        return ["documentation workflow has no jobs section"]

    violations = []
    if not re.search(
        r"(?m)^  workflow_run:\s*$", contents
    ) or not re.search(
        r"(?m)^    workflows:\s*\[Benchmark, Release\]\s*$", contents
    ) or not re.search(
        r"(?m)^    types:\s*\[completed\]\s*$", contents
    ):
        violations.append(
            "documentation workflow_run trigger must watch completed Benchmark and Release runs"
        )

    concurrency = re.search(
        r"(?ms)^concurrency:\s*\n(?P<body>.*?)(?=^jobs:\s*$)", contents
    )
    if concurrency is None or normalized_expression(
        DOCS_INELIGIBLE_RUN_CONCURRENCY_GUARD
    ) not in normalized_expression(concurrency.group("body")):
        violations.append(
            "documentation concurrency must isolate untrusted workflow_run events"
        )

    for job_name in ("benchmark-data", "build", "examples", "deploy"):
        job = jobs.get(job_name)
        if job is None:
            violations.append(f"documentation workflow has no {job_name} job")
            continue
        condition = normalized_expression(workflow_job_condition(job) or "")
        expected_condition = normalized_expression(DOCS_JOB_CONDITIONS[job_name])
        if condition != expected_condition:
            violations.append(
                f"documentation {job_name} job must gate workflow_run on trusted producers"
            )

    deploy_job = jobs.get("deploy", "")
    needs_match = re.search(
        r"(?m)^    needs:\s*\[(?P<list>[^\]]*)\]", deploy_job
    )
    dependencies = (
        set()
        if needs_match is None
        else {item.strip() for item in needs_match.group("list").split(",")}
    )
    if not {"build", "examples"}.issubset(dependencies):
        violations.append("documentation deploy must require the build and examples jobs")
    return violations


def verify_workflow_policy(
    workflow: Path, violations_for: Callable[[str], list[str]], label: str
) -> bool:
    """Print policy failures for one workflow and return whether it passed."""

    violations = violations_for(workflow.read_text(encoding="utf-8"))
    if not violations:
        return True
    print(f"{label} workflow policy validation failed:", file=sys.stderr)
    for violation in violations:
        print(f"  {workflow.relative_to(ROOT)}: {violation}", file=sys.stderr)
    return False


def verify_apple_arm64_runner_policy() -> bool:
    """Apply the Apple ARM64 runner policy to all oracle workflows."""

    workflow_dir = ROOT / ".github" / "workflows"
    workflows = {
        name: (workflow_dir / name).read_text(encoding="utf-8")
        for name in APPLE_ARM64_ORACLE_JOBS
    }
    violations = apple_arm64_runner_violations(workflows)
    if not violations:
        return True
    print("Apple ARM64 runner workflow policy validation failed:", file=sys.stderr)
    for violation in violations:
        print(f"  {violation}", file=sys.stderr)
    return False


def release_coverage_gate_violations(contents: str) -> list[str]:
    """Require tag coverage to pass before the release publish job can run."""

    jobs = workflow_jobs(contents)
    if jobs is None:
        return ["release workflow has no jobs section"]
    violations = []

    coverage_job = jobs.get("release-coverage")
    if coverage_job is None:
        violations.append("release workflow has no release-coverage job")
    else:
        if not re.search(r"(?m)^    steps:\s*$", coverage_job):
            violations.append("release-coverage must define steps")
        steps = re.split(r"(?m)^      - ", coverage_job)
        checkout_step = next(
            (step for step in steps if step.startswith("uses: actions/checkout@")),
            None,
        )
        if checkout_step is None:
            violations.append("release-coverage must check out the tagged source")
        elif re.search(r"(?m)^\s+ref:", checkout_step):
            violations.append("release-coverage checkout must use the tag event commit")
        if "toolchain: ${{ env.COVERAGE_TOOLCHAIN }}" not in coverage_job:
            violations.append("release-coverage must use the configured coverage toolchain")
        if not re.search(
            r"(?m)^  COVERAGE_TOOLCHAIN:\s*[\"']?nightly-\d{4}-\d{2}-\d{2}[\"']?\s*$",
            contents,
        ):
            violations.append("release workflow must pin its nightly coverage toolchain")
        if "cargo-llvm-cov --locked --version 0.8.7" not in coverage_job:
            violations.append("release-coverage must install the pinned coverage driver")
        job_settings = coverage_job.split("    steps:", maxsplit=1)[0]
        if re.search(r"(?m)^    if:", job_settings):
            violations.append("release-coverage must not be conditionally skipped")
        if re.search(r"(?m)^    continue-on-error:\s*true\s*$", job_settings):
            violations.append("release-coverage must not ignore coverage failures")
        coverage_step = ""
        coverage_script = ""
        for step in re.split(r"(?m)^      - ", coverage_job):
            script = run_script(step)
            if script is not None and make_coverage_command_lines(script):
                coverage_step = step
                coverage_script = script
                break
        if not coverage_step:
            violations.append("release-coverage must execute make coverage")
        elif any("||" in line for line in make_coverage_command_lines(coverage_script)):
            violations.append("release-coverage must not ignore make coverage failures")
        if re.search(r"(?m)^\s*set\s+\+e\s*$", coverage_script):
            if (
                "PIPESTATUS[0]" not in coverage_script
                or not re.search(r"(?m)^\s*exit\s+\"?\$status\"?\s*$", coverage_script)
            ):
                violations.append(
                    "release-coverage must propagate failures while errexit is disabled"
                )
        if re.search(r"(?m)^(?:if:| {8}if:)", coverage_step):
            violations.append("make coverage step must not be conditionally skipped")
        if re.search(r"(?m)^(?:continue-on-error:| {8}continue-on-error:)\s*true\s*$", coverage_step):
            violations.append("make coverage step must not ignore failures")

    publish_job = jobs.get("publish")
    if publish_job is None:
        violations.append("release workflow has no publish job")
    else:
        publish_settings = publish_job.split("    steps:", maxsplit=1)[0]
        if re.search(r"(?m)^    if:", publish_settings):
            violations.append("publish must use the default successful-dependencies condition")
        needs_match = re.search(
            r"(?m)^    needs:\s*(?:\[(?P<list>[^\]]*)\]|(?P<scalar>[^\s#]+))",
            publish_job,
        )
        if needs_match is None:
            dependencies = set()
        elif needs_match.group("list") is not None:
            dependencies = {
                dependency.strip()
                for dependency in needs_match.group("list").split(",")
                if dependency.strip()
            }
        else:
            dependencies = {needs_match.group("scalar")}
        required = {"preflight", "release-coverage"}
        if not required.issubset(dependencies):
            missing = ", ".join(sorted(required - dependencies))
            violations.append(f"publish must depend on {missing}")

    return violations


def yaml_mapping(
    contents: str, key: str, indentation: int
) -> dict[str, str] | str | None:
    """Read a simple YAML mapping at one indentation level."""

    prefix = " " * indentation
    match = re.search(
        rf"(?m)^{prefix}{re.escape(key)}:[ \t]*(?P<inline>[^\n]*)$", contents
    )
    if match is None:
        return None
    if match.group("inline").strip():
        return match.group("inline").strip()

    values = {}
    child_prefix = prefix + "  "
    for line in contents[match.end():].splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if not line.startswith(child_prefix):
            break
        entry = line[len(child_prefix):]
        if entry.startswith(" ") or ":" not in entry:
            return "<invalid mapping>"
        name, separator, value = entry.partition(":")
        if not separator or not name.strip() or name.strip() in values:
            return "<invalid mapping>"
        values[name.strip()] = value.strip()
    return values


def has_quoted_yaml_key(contents: str, indentation: int) -> bool:
    """Detect quoted mapping keys whose decoded spelling needs YAML parsing."""

    prefix = " " * indentation
    return re.search(
        rf"(?m)^{prefix}(?:\"(?:[^\"\\]|\\.)*\"|'(?:[^']|'')*')[ \t]*:",
        contents,
    ) is not None


def has_yaml_indirect_key(contents: str, indentation: int) -> bool:
    """Reject YAML key syntax that can hide job or step controls."""

    prefix = " " * indentation
    return re.search(
        rf"(?m)^{prefix}(?:<<[ \t]*:|\?[ \t]+|!+[^\s]*[ \t]+|&[^\s]+[ \t]+|\*[^\s]+[ \t]*:)",
        contents,
    ) is not None


def has_indented_yaml_continuation(
    contents: str, key: str, indentation: int
) -> bool:
    """Detect a folded plain scalar extending a reviewed control value."""

    prefix = " " * indentation
    key_line = re.compile(rf"^{prefix}{re.escape(key)}[ \t]*:")
    lines = contents.splitlines()
    for index, line in enumerate(lines):
        if key_line.search(line) is None:
            continue
        for following in lines[index + 1:]:
            if not following.strip() or following.lstrip().startswith("#"):
                continue
            following_indentation = len(following) - len(following.lstrip(" "))
            if following_indentation > indentation:
                return True
            break
    return False


def reviewed_job_control_violations(job_name: str, settings: str) -> list[str]:
    """Require OIDC jobs to use normal success dependency handling."""

    prefix = " " * 4
    if (
        has_quoted_yaml_key(settings, 4)
        or has_yaml_indirect_key(settings, 4)
        or any(
            re.search(rf"(?m)^{prefix}{control}[ \t]*:", settings) is not None
            for control in ("if", "continue-on-error")
        )
    ):
        return [f"{job_name} job controls must match the reviewed policy"]
    return []


def reviewed_step_control_violations(
    job_name: str,
    steps: list[str],
    expected_conditions: dict[str, str],
) -> list[str]:
    """Require exact conditions and never permit a credentialed step to ignore failure."""

    violations = []
    for step in steps[1:]:
        name_match = re.search(r"(?m)^name:\s*(.+)$", step)
        step_name = name_match.group(1) if name_match is not None else None
        actual_conditions = re.findall(
            r"(?m)^ {8}if[ \t]*:[ \t]*(?P<condition>[^\r\n]*?)[ \t]*$",
            step,
        )
        expected_condition = expected_conditions.get(step_name)
        expected_conditions_for_step = (
            [expected_condition] if expected_condition is not None else []
        )
        if (
            actual_conditions != expected_conditions_for_step
            or has_indented_yaml_continuation(step, "if", 8)
            or has_quoted_yaml_key(step, 8)
            or has_yaml_indirect_key(step, 8)
            or re.search(
                r"(?m)^ {8}continue-on-error[ \t]*:",
                step,
            ) is not None
        ):
            violations.append(f"{job_name} step controls must match the reviewed policy")
            break
    return violations


def github_release_publication_script_violations(script: str) -> list[str]:
    """Require retry-safe draft publication and immutable release checks."""

    normalized_script = textwrap.dedent(script).strip()
    script_digest = hashlib.sha256(normalized_script.encode("utf-8")).hexdigest()
    if script_digest != GITHUB_RELEASE_PUBLICATION_SCRIPT_SHA256:
        violations = [
            "github-release recovery script differs from the reviewed command set"
        ]
    else:
        violations = []

    required = (
        'release_endpoint="repos/$repo/releases/tags/$tag"',
        "HTTP 404([^0-9]|$)",
        '--rawfile body "$expected_body"',
        '[[ "$VERIFIED_TAG_COMMIT" == "$GITHUB_SHA" ]] || {',
        ".tag_name == $tag",
        ".name == $title",
        ".body == $body",
        ".prerelease == true",
        'gh release download "$tag"',
        "cmp -s --",
        'gh release upload "$tag"',
        'gh release edit "$tag" --repo "$repo" --draft=false',
        '[[ "$tag_commit" == "$GITHUB_SHA" && "$tag_commit" == "$VERIFIED_TAG_COMMIT" ]]',
        'image-slash-star-release tag=%s commit=%s crate-sha256=%s checksums-sha256=%s',
        '([.assets[].name] | sort) == ([$crate, "SHA256SUMS"] | sort)',
    )
    violations.extend(
        f"github-release recovery script is missing required check: {marker}"
        for marker in required
        if marker not in normalized_script
    )

    create_index = script.find('gh release create "$tag"')
    create_end = script.find("--notes-file", create_index)
    if create_index < 0 or create_end < 0 or "--draft" not in script[create_index:create_end]:
        violations.append("github-release must create a draft before uploading assets")

    published_check = script.find('if [[ "$(jq -r \'.draft\' "$release_file")" == false ]]')
    upload_index = script.find('gh release upload "$tag"')
    edit_index = script.find('gh release edit "$tag" --repo "$repo" --draft=false')
    tag_recheck = script.rfind("verify_tag_commit\n")
    if not (0 <= published_check < upload_index < tag_recheck < edit_index):
        violations.append(
            "github-release must leave published releases untouched and recheck the tag before publishing a draft"
        )

    forbidden = (
        "--clobber",
        "gh release delete",
        "gh release delete-asset",
        "curl ",
        "wget ",
        "git push",
    )
    violations.extend(
        f"github-release must not use {marker.strip()}"
        for marker in forbidden
        if marker in script
    )
    return violations


def release_oidc_isolation_violations(contents: str) -> list[str]:
    """Keep repository code outside release jobs with OIDC permissions."""

    jobs = workflow_jobs(contents)
    if jobs is None:
        return ["release workflow has no jobs section"]
    violations = []
    if yaml_mapping(contents, "permissions", 0) != {"contents": "read"}:
        violations.append(
            "release workflow must use only the reviewed top-level contents: read permission"
        )
    if yaml_mapping(contents, "env", 0) != {
        "CARGO_TERM_COLOR": "always",
        "RUST_BACKTRACE": '"1"',
        "RUSTC_WRAPPER": '""',
        "RUST_TOOLCHAIN": '"1.96.1"',
        "PYTHON_VERSION": '"3.12.10"',
        "COVERAGE_TOOLCHAIN": '"nightly-2026-07-16"',
    }:
        violations.append("release workflow environment must not add execution hooks")
    if (
        yaml_mapping(contents, "defaults", 0) is not None
        or has_quoted_yaml_key(contents, 0)
        or has_yaml_indirect_key(contents, 0)
    ):
        violations.append("release workflow must not override default shell execution")

    expected_job_permissions = {
        "preflight": {"contents": "read", "actions": "read"},
        "release-coverage": None,
        "prepare-publish": {"contents": "read"},
        "publish": {"contents": "read", "id-token": "write"},
        "verify-registry": {"contents": "read"},
        "github-release": {
            "contents": "write",
            "attestations": "write",
            "artifact-metadata": "write",
            "id-token": "write",
        },
    }
    if set(jobs) != set(expected_job_permissions):
        violations.append("release workflow job set must match its reviewed permission map")
    for job_name, job in jobs.items():
        if job_name not in expected_job_permissions:
            continue
        actual_permissions = yaml_mapping(job, "permissions", 4)
        if actual_permissions != expected_job_permissions[job_name]:
            violations.append(
                f"{job_name} must use its reviewed least-privilege permissions"
            )
        if (
            job_name not in {"publish", "github-release"}
            and isinstance(actual_permissions, dict)
            and actual_permissions.get("id-token") == "write"
        ):
            violations.append(f"{job_name} must not have OIDC token permission")

    prepare_job = jobs.get("prepare-publish")
    if prepare_job is not None:
        if "needs: [preflight, release-coverage]" not in prepare_job:
            violations.append("prepare-publish must depend on preflight and release-coverage")
        if "make release-publish-prepare" not in prepare_job:
            violations.append("prepare-publish must reproduce and compile the candidate")
        if "make release-publish-request" not in prepare_job:
            violations.append("prepare-publish must frame the candidate for the crates.io API")

    publish_job = jobs.get("publish")
    if publish_job is None:
        violations.append("release workflow has no publish job")
    else:
        settings, _, _ = publish_job.partition("    steps:")
        violations.extend(reviewed_job_control_violations("publish", settings))
        if "needs: [preflight, release-coverage, prepare-publish]" not in publish_job:
            violations.append("publish must depend on the verified preparation and coverage gates")
        if not re.search(r"(?m)^    environment:\s*crates-io\s*$", settings):
            violations.append("publish must use the crates-io protected environment")
        if not re.search(r"(?m)^    runs-on:\s*ubuntu-24\.04\s*$", settings):
            violations.append("publish must use the reviewed hosted runner")
        if yaml_mapping(settings, "env", 4) is not None:
            violations.append("crates.io credentials must not be job-scoped")
        if any(
            yaml_mapping(settings, key, 4) is not None
            for key in ("container", "defaults")
        ):
            violations.append(
                "publish must not override its runner, shell, or environment"
            )
        if re.search(r"(?m)^      - uses: actions/checkout@", publish_job):
            violations.append("publish must not check out or execute repository source")

        steps = re.split(r"(?m)^      - ", publish_job)
        violations.extend(
            reviewed_step_control_violations(
                "publish",
                steps,
                {
                    "Authenticate to crates.io with OIDC": (
                        "steps.existing-version.outputs.present != 'true'"
                    ),
                    "Upload the verified archive to crates.io": (
                        "steps.existing-version.outputs.present != 'true'"
                    ),
                },
            )
        )
        if len(steps) != 7:
            violations.append(
                "publish must contain only artifact, version, tag, OIDC, and upload steps"
            )
        publish_actions = [
            action
            for step in steps[1:]
            if (action := re.search(r"(?m)^\s*uses:\s*(\S+)", step)) is not None
        ]
        if [action.group(1).partition(" #")[0] for action in publish_actions] != [
            "actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333",
            "rust-lang/crates-io-auth-action@c6f97d42243bad5fab37ca0427f495c86d5b1a18",
        ]:
            violations.append(
                "publish may use only the reviewed artifact and crates.io OIDC action revisions"
            )
        existing_index = next(
            (index for index, step in enumerate(steps)
             if step.startswith("name: Check whether this exact crate version is already published")),
            None,
        )
        tag_check_index = next(
            (index for index, step in enumerate(steps)
             if step.startswith("name: Verify the remote annotated tag")),
            None,
        )
        checksum_index = next(
            (index for index, step in enumerate(steps)
             if step.startswith("name: Verify publish request checksum")),
            None,
        )
        auth_index = next(
            (index for index, step in enumerate(steps)
             if "uses: rust-lang/crates-io-auth-action@" in step),
            None,
        )
        upload_index = next(
            (index for index, step in enumerate(steps)
             if step.startswith("name: Upload the verified archive to crates.io")),
            None,
        )
        if tag_check_index is None:
            violations.append("publish must recheck the remote annotated tag")
        elif auth_index is not None and tag_check_index >= auth_index:
            violations.append("remote tag verification must precede OIDC authentication")
        else:
            tag_script = run_script(steps[tag_check_index])
            if tag_script is None or not all(
                marker in tag_script
                for marker in (
                    "gh api",
                    "/git/ref/tags/",
                    "/git/tags/",
                    "object.type",
                    "$GITHUB_SHA",
                )
            ):
                violations.append("publish tag check must require an annotated tag at GITHUB_SHA")
        if checksum_index is None:
            violations.append("publish must verify the prepared request checksum")
        elif auth_index is not None and checksum_index >= auth_index:
            violations.append("publish request checksum verification must precede OIDC authentication")
        if existing_index is None:
            violations.append("publish must check for an identical already-published version")
        elif auth_index is not None and existing_index >= auth_index:
            violations.append("existing-version verification must precede OIDC authentication")
        else:
            existing_script = run_script(steps[existing_index])
            if existing_script is None or not all(
                marker in existing_script
                for marker in (
                    "https://crates.io/api/v1/crates/image-slash-star/",
                    "version.checksum",
                    "sha256sum",
                    "present=true",
                    "present=false",
                )
            ):
                violations.append("existing-version check must require exact crate archive bytes")
        if auth_index is None:
            violations.append("publish must authenticate to crates.io with OIDC")
        elif "if: steps.existing-version.outputs.present != 'true'" not in steps[auth_index]:
            violations.append("publish must skip OIDC when the exact version already exists")
        if upload_index is None:
            violations.append("publish must upload the prepared request body")
        elif auth_index is not None:
            if upload_index <= auth_index:
                violations.append("crates.io upload must follow OIDC authentication")
            if upload_index != len(steps) - 1:
                violations.append("only the crates.io upload may follow OIDC authentication")
            if "if: steps.existing-version.outputs.present != 'true'" not in steps[upload_index]:
                violations.append("publish must skip upload when the exact version already exists")
            upload_script = run_script(steps[upload_index])
            if upload_script is None or not all(
                marker in upload_script
                for marker in (
                    "curl --config /dev/null",
                    "--request PUT",
                    "--data-binary @target/crates-io-publish/crates-io-upload.bin",
                    "https://crates.io/api/v1/crates/new",
                )
            ):
                violations.append("publish must upload the prepared archive directly to crates.io")
            auth_id_match = re.search(r"(?m)^        id:\s*([A-Za-z0-9_-]+)\s*$", steps[auth_index])
            if auth_id_match is None:
                violations.append("crates.io OIDC action must have a stable step id")
            else:
                expected_token = (
                    "CRATES_IO_PUBLISH_TOKEN: ${{ "
                    f"steps.{auth_id_match.group(1)}.outputs.token }}"
                )
                if expected_token not in steps[upload_index]:
                    violations.append("crates.io token must be scoped to the upload step")

        expected_publish_scripts = {
            "Verify publish request checksum": (
                "(cd target/crates-io-publish && sha256sum --check SHA256SUMS)"
            ),
            "Check whether this exact crate version is already published": """\
set -euo pipefail
version="${GITHUB_REF_NAME#v}"
response_file="${RUNNER_TEMP}/crates-io-existing-version.json"
status="$(curl --config /dev/null \\
  --proto '=https' \\
  --silent --show-error --connect-timeout 10 --max-time 30 \\
  --output "$response_file" --write-out '%{http_code}' \\
  "https://crates.io/api/v1/crates/image-slash-star/${version}")"
if [[ "$status" == 404 ]]; then
  echo "present=false" >> "$GITHUB_OUTPUT"
elif [[ "$status" =~ ^2[0-9][0-9]$ ]]; then
  expected="$(sha256sum "target/crates-io-publish/image-slash-star-${version}.crate" | cut -d ' ' -f 1)"
  actual="$(jq -r '.version.checksum // empty' "$response_file")"
  [[ "$actual" == "$expected" ]] || {
    echo "crates.io already contains ${version} with different archive bytes" >&2
    exit 1
  }
  echo "present=true" >> "$GITHUB_OUTPUT"
else
  cat "$response_file" >&2
  echo "crates.io version lookup returned HTTP ${status}" >&2
  exit 1
fi""",
            "Verify the remote annotated tag still names this commit": """\
set -euo pipefail
[[ "$GITHUB_REF_NAME" == v* ]]
[[ "$GITHUB_SHA" =~ ^[0-9a-f]{40}$ ]]
tag_ref="$(gh api "repos/${GITHUB_REPOSITORY}/git/ref/tags/${GITHUB_REF_NAME}")"
[[ "$(jq -r '.object.type' <<< "$tag_ref")" == "tag" ]]
tag_object_sha="$(jq -r '.object.sha' <<< "$tag_ref")"
[[ "$tag_object_sha" =~ ^[0-9a-f]{40}$ ]]
tag_object="$(gh api "repos/${GITHUB_REPOSITORY}/git/tags/${tag_object_sha}")"
[[ "$(jq -r '.object.type' <<< "$tag_object")" == "commit" ]]
tag_commit="$(jq -r '.object.sha' <<< "$tag_object")"
[[ "$tag_commit" == "$GITHUB_SHA" ]]
echo "tag_commit=$tag_commit" >> "$GITHUB_OUTPUT"
echo "verified annotated tag ${GITHUB_REF_NAME} at ${tag_commit}\"""",
            "Upload the verified archive to crates.io": """\
set -euo pipefail
response_file="${RUNNER_TEMP}/crates-io-publish-response.json"
status="$(curl --config /dev/null \\
  --proto '=https' \\
  --silent --show-error --connect-timeout 10 --max-time 300 \\
  --output "$response_file" --write-out '%{http_code}' \\
  --request PUT \\
  --header "Authorization: ${CRATES_IO_PUBLISH_TOKEN}" \\
  --header 'Content-Type: application/octet-stream' \\
  --header 'Accept: application/json' \\
  --data-binary @target/crates-io-publish/crates-io-upload.bin \\
  https://crates.io/api/v1/crates/new)"
if [[ ! "$status" =~ ^2[0-9][0-9]$ ]]; then
  cat "$response_file" >&2
  echo "crates.io publish request returned HTTP ${status}" >&2
  exit 1
fi
if ! jq -e 'type == "object" and (has("errors") | not)' \\
  "$response_file" >/dev/null; then
  cat "$response_file" >&2
  echo "crates.io returned an error payload for the publish request" >&2
  exit 1
fi""",
        }
        for step in steps[1:]:
            step_name = re.search(r"(?m)^name:\s*(.+)$", step)
            script = run_script(step)
            shell = re.search(r"(?m)^ {8}shell:\s*(.+)$", step)
            shell_value = shell.group(1).strip() if shell is not None else None
            expected_shell = (
                "bash"
                if step_name is not None
                and step_name.group(1)
                in {
                    "Check whether this exact crate version is already published",
                    "Verify the remote annotated tag still names this commit",
                    "Upload the verified archive to crates.io",
                }
                else None
            )
            if shell_value != expected_shell:
                violations.append("publish must use only the reviewed step shells")
                break
            if re.search(r"(?m)^ {8}working-directory:", step):
                violations.append("publish must use the reviewed working directory")
                break
            if step_name is not None and script is not None:
                expected_script = expected_publish_scripts.get(step_name.group(1))
                if expected_script is None or textwrap.dedent(script).strip() != textwrap.dedent(
                    expected_script
                ).strip():
                    violations.append(
                        "publish shell steps must match the reviewed static command set"
                    )
                    break
            elif script is not None:
                violations.append(
                    "publish shell steps must have a reviewed name and command"
                )
                break
            expected_env = {
                "Verify the remote annotated tag still names this commit": {
                    "GH_TOKEN": "${{ github.token }}",
                },
                "Upload the verified archive to crates.io": {
                    "CRATES_IO_PUBLISH_TOKEN": (
                        "${{ steps.crates-io-auth.outputs.token }}"
                    ),
                },
            }.get(step_name.group(1) if step_name is not None else "")
            if yaml_mapping(step, "env", 8) != expected_env:
                violations.append("publish step environments must match the reviewed token scopes")
                break

        for step in steps:
            script = run_script(step)
            if script is not None and re.search(
                r"(?im)^\s*(?:cargo|make|python(?:3)?|scripts/|\.\/scripts/)\b",
                script,
            ):
                violations.append("publish must not run Cargo or repository-controlled scripts")
                break

    verify_job = jobs.get("verify-registry")
    if verify_job is not None and "needs: publish" not in verify_job:
        violations.append("verify-registry must follow a successful publish")

    github_release_job = jobs.get("github-release")
    if github_release_job is not None:
        if "needs: verify-registry" not in github_release_job:
            violations.append("github-release must follow registry byte verification")
        settings, _, _ = github_release_job.partition("    steps:")
        violations.extend(
            reviewed_job_control_violations("github-release", settings)
        )
        if not re.search(r"(?m)^    runs-on:\s*ubuntu-24\.04\s*$", settings):
            violations.append("github-release must use the reviewed hosted runner")
        if any(
            yaml_mapping(settings, key, 4) is not None
            for key in ("container", "defaults", "env", "environment")
        ):
            violations.append(
                "github-release must not override its runner, shell, or environment"
            )
        if re.search(r"(?m)^      - uses: actions/checkout@", github_release_job):
            violations.append("github-release must not check out repository source")
        steps = re.split(r"(?m)^      - ", github_release_job)
        violations.extend(
            reviewed_step_control_violations("github-release", steps, {})
        )
        if len(steps) != 6:
            violations.append(
                "github-release must contain only artifact, checksum, tag, attestation, and release steps"
            )
        release_actions = [
            action
            for step in steps[1:]
            if (action := re.search(r"(?m)^\s*uses:\s*(\S+)", step)) is not None
        ]
        if [action.group(1).partition(" #")[0] for action in release_actions] != [
            "actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333",
            "actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6",
        ]:
            violations.append(
                "github-release may use only the reviewed artifact and attestation action revisions"
            )
        attest_index = next(
            (index for index, step in enumerate(steps)
             if "uses: actions/attest@" in step),
            None,
        )
        tag_check_index = next(
            (index for index, step in enumerate(steps)
             if step.startswith("name: Verify the remote annotated tag")),
            None,
        )
        if attest_index is None:
            violations.append("github-release must attest the canonical registry archive")
        elif tag_check_index is None or tag_check_index >= attest_index:
            violations.append("github-release must recheck the tag before attestation")
        elif tag_check_index is not None:
            if re.search(r"(?m)^ {8}id:\s*verify-tag\s*$", steps[tag_check_index]) is None:
                violations.append("github-release tag check must expose the verified commit output")
            tag_script = run_script(steps[tag_check_index])
            if tag_script is None or not all(
                marker in tag_script
                for marker in (
                    "gh api",
                    "/git/ref/tags/",
                    "/git/tags/",
                    "object.type",
                    "$GITHUB_SHA",
                )
            ):
                violations.append("github-release tag check must require an annotated tag at GITHUB_SHA")
        for step in steps:
            script = run_script(step)
            if script is not None and re.search(
                r"(?im)^\s*(?:cargo|make|python(?:3)?|scripts/|\.\/scripts/)\b",
                script,
            ):
                violations.append("github-release must not execute repository code with OIDC permission")
                break
        if attest_index is not None:
            post_attestation_steps = steps[attest_index + 1:]
            if len(post_attestation_steps) != 1 or not post_attestation_steps[0].startswith(
                "name: Create GitHub prerelease"
            ):
                violations.append(
                    "only GitHub release creation may follow the attestation action"
                )
        expected_release_scripts = {
            "Verify canonical registry checksum": (
                "(cd registry-bundle && sha256sum --check SHA256SUMS)"
            ),
            "Verify the remote annotated tag still names this commit": """\
set -euo pipefail
[[ "$GITHUB_REF_NAME" == v* ]]
[[ "$GITHUB_SHA" =~ ^[0-9a-f]{40}$ ]]
tag_ref="$(gh api "repos/${GITHUB_REPOSITORY}/git/ref/tags/${GITHUB_REF_NAME}")"
[[ "$(jq -r '.object.type' <<< "$tag_ref")" == "tag" ]]
tag_object_sha="$(jq -r '.object.sha' <<< "$tag_ref")"
[[ "$tag_object_sha" =~ ^[0-9a-f]{40}$ ]]
tag_object="$(gh api "repos/${GITHUB_REPOSITORY}/git/tags/${tag_object_sha}")"
[[ "$(jq -r '.object.type' <<< "$tag_object")" == "commit" ]]
tag_commit="$(jq -r '.object.sha' <<< "$tag_object")"
[[ "$tag_commit" == "$GITHUB_SHA" ]]
echo "tag_commit=$tag_commit" >> "$GITHUB_OUTPUT"
echo "verified annotated tag ${GITHUB_REF_NAME} at ${tag_commit}\"""",
        }
        expected_release_scripts["Create GitHub prerelease"] = (
            ROOT / "scripts" / "github_release_publish.sh"
        ).read_text(encoding="utf-8").strip()
        for step in steps[1:]:
            step_name = re.search(r"(?m)^name:\s*(.+)$", step)
            script = run_script(step)
            shell = re.search(r"(?m)^ {8}shell:\s*(.+)$", step)
            shell_value = shell.group(1).strip() if shell is not None else None
            expected_shell = (
                "bash"
                if step_name is not None
                and step_name.group(1)
                == "Verify the remote annotated tag still names this commit"
                else None
            )
            if shell_value != expected_shell:
                violations.append("github-release must use only the reviewed step shells")
                break
            if re.search(r"(?m)^ {8}working-directory:", step):
                violations.append("github-release must use the reviewed working directory")
                break
            if step_name is not None and script is not None:
                expected_script = expected_release_scripts.get(step_name.group(1))
                if expected_script is None or textwrap.dedent(script).strip() != textwrap.dedent(
                    expected_script
                ).strip():
                    violations.append(
                        "github-release shell steps must match the reviewed commands and publication script"
                    )
                    break
                if step_name.group(1) == "Create GitHub prerelease":
                    violations.extend(github_release_publication_script_violations(script))
            elif script is not None:
                violations.append(
                    "github-release shell steps must have a reviewed name and command"
                )
                break
            expected_env = {
                "Verify the remote annotated tag still names this commit": {
                    "GH_TOKEN": "${{ github.token }}",
                },
                "Create GitHub prerelease": {
                    "GH_TOKEN": "${{ github.token }}",
                    "GH_REPO": "${{ github.repository }}",
                    "VERIFIED_TAG_COMMIT": "${{ steps.verify-tag.outputs.tag_commit }}",
                },
            }.get(step_name.group(1) if step_name is not None else "")
            if yaml_mapping(step, "env", 8) != expected_env:
                violations.append(
                    "github-release step environments must match the reviewed token scopes"
                )
                break

    return violations


def verify_release_coverage_gate(workflow: Path) -> bool:
    """Reject release workflows that can publish without tagged-source coverage."""

    violations = release_coverage_gate_violations(workflow.read_text(encoding="utf-8"))
    if not violations:
        return True
    print("Release coverage gate validation failed:", file=sys.stderr)
    for violation in violations:
        print(f"  {workflow.relative_to(ROOT)}: {violation}", file=sys.stderr)
    return False


def release_candidate_fallback_violations(contents: str) -> list[str]:
    """Require a verified source rebuild when the retained CI artifact is absent."""

    jobs = workflow_jobs(contents)
    if jobs is None:
        return ["release workflow has no jobs section"]
    preflight = jobs.get("preflight")
    if preflight is None:
        return ["release workflow has no preflight job"]

    steps = re.split(r"(?m)^      - ", preflight)
    violations = []

    def step_index(marker: str) -> int | None:
        return next(
            (index for index, step in enumerate(steps) if marker in step),
            None,
        )

    ci_index = step_index("Require successful main CI for this commit")
    download_index = step_index("actions/download-artifact@")
    fallback_index = step_index("Rebuild and verify the candidate when the CI artifact is unavailable")
    checksum_index = step_index("Verify candidate bundle")
    upload_index = step_index("Retain verified candidate bundle")

    if ci_index is None:
        violations.append("release preflight must require successful main CI")
    else:
        ci_step = steps[ci_index]
        if (
            "head_sha=${GITHUB_SHA}" not in ci_step
            or 'select(.event == "push" and .head_branch == "main"' not in ci_step
            or '.status == "completed" and .conclusion == "success"' not in ci_step
            or 'test -n "$run_id"' not in ci_step
        ):
            violations.append(
                "release preflight must select successful main CI for the exact tagged commit"
            )

    if download_index is None:
        violations.append("release preflight must download the exact main CI candidate")
    else:
        download_step = steps[download_index]
        if "id: candidate-artifact" not in download_step:
            violations.append("candidate download must expose its step outcome")
        if not re.search(r"(?m)^        continue-on-error:\s*true\s*$", download_step):
            violations.append("candidate download failure must enable the verified rebuild")
        if "run-id: ${{ steps.ci-run.outputs.run-id }}" not in download_step:
            violations.append("candidate download must use the successful main CI run")

    if fallback_index is None:
        violations.append("release preflight must rebuild a missing candidate")
    else:
        fallback_step = steps[fallback_index]
        if "if: steps.candidate-artifact.outcome == 'failure'" not in fallback_step:
            violations.append("candidate rebuild must run only when artifact download fails")
        script = run_script(fallback_step) or ""
        if 'rm -rf target/release-artifacts' not in script:
            violations.append("candidate rebuild must clear partial artifact contents")
        if 'RELEASE_TAG="$GITHUB_REF_NAME" make package-verify PYTHON=python3' not in script:
            violations.append("candidate rebuild must run tagged package verification")
        if re.search(r"(?m)^        continue-on-error:\s*true\s*$", fallback_step):
            violations.append("candidate rebuild must not ignore verification failures")

    if checksum_index is None:
        violations.append("release preflight must verify the candidate checksum")
    else:
        checksum_step = steps[checksum_index]
        script = run_script(checksum_step) or ""
        if "sha256sum --check SHA256SUMS" not in script:
            violations.append("release candidate checksum must be checked")
        if re.search(r"(?m)^        continue-on-error:\s*true\s*$", checksum_step):
            violations.append("release candidate checksum failures must block publication")
        if re.search(r"(?m)^        if:", checksum_step):
            violations.append("release candidate checksum must not be conditionally skipped")

    if upload_index is None:
        violations.append("release preflight must retain a tag-specific candidate")
    if (
        download_index is not None
        and fallback_index is not None
        and checksum_index is not None
        and not download_index < fallback_index < checksum_index
    ):
        violations.append("candidate checksum must run after the download/rebuild paths")
    if checksum_index is not None and upload_index is not None and checksum_index >= upload_index:
        violations.append("candidate must pass checksum verification before retention")
    if ci_index is not None and download_index is not None and ci_index >= download_index:
        violations.append("successful main CI must be checked before candidate download")

    return violations


def artifact_upload_rerun_violations(contents: str) -> list[str]:
    """Require every uploaded workflow artifact to be replaceable on retries."""

    violations = []
    pages_artifact_names = []
    pages_deployment_names = []
    pages_step_output = "${{ steps.pages-artifact.outputs.name }}"
    pages_job_output = "${{ needs.build.outputs.pages_artifact }}"
    steps = re.finditer(
        r"(?ms)^      - (?P<header>[^\n]*)\n"
        r"(?P<body>.*?)(?=^      - |\Z)",
        contents,
    )
    for match in steps:
        header = match.group("header")
        body = match.group("body")
        uses = header.removeprefix("uses: ") if header.startswith("uses: ") else None
        if uses is None:
            uses_match = re.search(r"(?m)^        uses:\s*(.+)$", body)
            uses = uses_match.group(1) if uses_match is not None else None
        if uses is None:
            continue

        artifact_match = re.search(r"(?m)^          name:\s*(.+)$", body)
        artifact_name = artifact_match.group(1) if artifact_match is not None else "<unnamed>"
        if uses.startswith("actions/upload-pages-artifact@"):
            pages_artifact_names.append(artifact_name)
            if artifact_name.strip() != pages_step_output:
                violations.append(
                    "GitHub Pages upload must use the attempt-specific build output"
                )
        elif uses.startswith("actions/deploy-pages@"):
            deployment_match = re.search(
                r"(?m)^          artifact_name:\s*(.+)$", body
            )
            pages_deployment_names.append(
                deployment_match.group(1).strip() if deployment_match else None
            )
        elif uses.startswith("actions/upload-artifact@") and re.search(
            r"(?m)^          overwrite:\s*true\s*$", body
        ) is None:
            violations.append(
                f"artifact {artifact_name!r} must set overwrite: true for same-run retries"
            )

    if pages_deployment_names and not pages_artifact_names:
        violations.append("GitHub Pages deployment must have an uploaded artifact")
    if pages_artifact_names and not pages_deployment_names:
        violations.append("GitHub Pages artifact must have a deployment step")
    if len(pages_artifact_names) > 1:
        violations.append("workflow must upload exactly one GitHub Pages artifact")
    if pages_artifact_names:
        if f"pages_artifact: {pages_step_output}" not in contents:
            violations.append(
                "GitHub Pages attempt-specific name must be exposed as a build output"
            )
        if not re.search(
            r'(?m)^      - name: Name the Pages artifact for this run attempt\n'
            r'        id: pages-artifact\n'
            r'        run: echo "name=github-pages-\$\{GITHUB_RUN_ATTEMPT\}" '
            r'>> "\$GITHUB_OUTPUT"$',
            contents,
        ):
            violations.append(
                "GitHub Pages artifact name must include GITHUB_RUN_ATTEMPT"
            )
        for deployment_name in pages_deployment_names:
            if deployment_name != pages_job_output:
                violations.append(
                    "GitHub Pages deployment must use the successful build's artifact output"
                )
    return violations


def verify_artifact_upload_rerun_safety(workflows: list[Path]) -> bool:
    """Reject artifact uploads that would collide when a workflow is retried."""

    violations = []
    for workflow in workflows:
        for violation in artifact_upload_rerun_violations(
            workflow.read_text(encoding="utf-8")
        ):
            violations.append(f"{workflow.relative_to(ROOT)}: {violation}")
    if not violations:
        return True

    print("Workflow artifact retry validation failed:", file=sys.stderr)
    for violation in violations:
        print(f"  {violation}", file=sys.stderr)
    return False


def binary_bytes(archive: bytes, checksum: str) -> bytes:
    if hashlib.sha256(archive).hexdigest() != checksum:
        raise ValueError("actionlint archive checksum mismatch")
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as contents:
        member = contents.getmember("actionlint")
        if not member.isfile():
            raise ValueError("actionlint archive does not contain a regular binary")
        # Copy this single verified member; never extract archive paths or links.
        source = contents.extractfile(member)
        if source is None:
            raise ValueError("actionlint binary is missing")
        return source.read()


def write_archive_atomically(archive: Path, data: bytes) -> None:
    """Persist a verified archive without exposing a partially written cache."""

    temporary_path = None
    try:
        with tempfile.NamedTemporaryFile(
            dir=archive.parent, prefix=f".{archive.name}.", delete=False
        ) as temporary:
            temporary_path = Path(temporary.name)
            temporary.write(data)
            temporary.flush()
            os.fsync(temporary.fileno())
        os.replace(temporary_path, archive)
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)


def cached_actionlint_binary(archive: Path, url: str, checksum: str) -> bytes:
    """Reuse a verified archive or replace a corrupt cache from the pinned URL."""

    try:
        data = archive.read_bytes()
    except FileNotFoundError:
        pass
    else:
        try:
            return binary_bytes(data, checksum)
        except (EOFError, KeyError, OSError, tarfile.TarError, ValueError) as error:
            print(
                f"Discarding invalid cached actionlint archive ({error}); downloading again",
                file=sys.stderr,
            )
            archive.unlink(missing_ok=True)

    data = download_archive(url)
    executable = binary_bytes(data, checksum)
    write_archive_atomically(archive, data)
    return executable


def is_transient_download_failure(error: BaseException) -> bool:
    """Return whether a failed release download is likely to recover."""

    if isinstance(error, urllib.error.HTTPError):
        return error.code in TRANSIENT_HTTP_STATUS_CODES
    if isinstance(error, urllib.error.URLError):
        error = error.reason
    if isinstance(
        error,
        (http.client.IncompleteRead, http.client.RemoteDisconnected, ssl.SSLEOFError),
    ):
        return True
    if isinstance(error, ssl.SSLError):
        return False
    if isinstance(error, TimeoutError):
        return True
    return isinstance(error, OSError) and error.errno in TRANSIENT_NETWORK_ERRNOS


def download_archive(url: str) -> bytes:
    """Download a release archive, retrying only transient failures."""

    for attempt in range(1, DOWNLOAD_ATTEMPTS + 1):
        try:
            with urllib.request.urlopen(url, timeout=60) as response:
                return response.read()
        except (http.client.HTTPException, OSError) as error:
            retryable = is_transient_download_failure(error)
            if isinstance(error, urllib.error.HTTPError):
                if error.fp is not None:
                    error.close()
                failure = f"HTTP {error.code}"
            else:
                failure = str(error)
            if not retryable or attempt == DOWNLOAD_ATTEMPTS:
                raise

        delay = DOWNLOAD_RETRY_BACKOFF_SECONDS * (2 ** (attempt - 1))
        print(
            f"actionlint download attempt {attempt}/{DOWNLOAD_ATTEMPTS} failed "
            f"({failure}); retrying in {delay}s",
            file=sys.stderr,
        )
        time.sleep(delay)

    raise RuntimeError("actionlint download retry loop exited unexpectedly")


def main() -> int:
    workflows = sorted((ROOT / ".github" / "workflows").glob("*.y*ml"))
    if not workflows:
        raise SystemExit("No GitHub workflows found")
    if not verify_action_pins(workflows):
        return 1
    ci_workflow = ROOT / ".github" / "workflows" / "ci.yml"
    if not verify_workflow_policy(
        ci_workflow, ci_coverage_gate_violations, "CI coverage"
    ):
        return 1
    if not verify_workflow_policy(
        ci_workflow,
        ci_x86_coverage_violations,
        "CI x86 parity coverage",
    ):
        return 1
    if not verify_workflow_policy(
        ci_workflow,
        ci_required_checks_violations,
        "CI required-check aggregation",
    ):
        return 1
    if not verify_apple_arm64_runner_policy():
        return 1
    docs_workflow = ROOT / ".github" / "workflows" / "docs.yml"
    if not verify_workflow_policy(
        docs_workflow,
        docs_workflow_run_publication_violations,
        "Documentation publication",
    ):
        return 1
    release_workflow = ROOT / ".github" / "workflows" / "release.yml"
    if not verify_release_coverage_gate(release_workflow):
        return 1
    if not verify_workflow_policy(
        release_workflow,
        release_oidc_isolation_violations,
        "Release OIDC isolation",
    ):
        return 1
    if not verify_workflow_policy(
        release_workflow,
        release_candidate_fallback_violations,
        "Release candidate fallback",
    ):
        return 1
    if not verify_artifact_upload_rerun_safety(workflows):
        return 1

    target = (platform.system(), platform.machine())
    if target not in ARCHIVES:
        raise SystemExit(f"Workflow lint supports Linux/macOS x86-64 and ARM64; got {target}")
    suffix, checksum = ARCHIVES[target]
    directory = ROOT / "target" / "workflow-tools" / f"actionlint-{VERSION}-{suffix}"
    directory.mkdir(parents=True, exist_ok=True)
    archive = directory / "archive.tar.gz"
    url = (
        f"https://github.com/rhysd/actionlint/releases/download/v{VERSION}/"
        f"actionlint_{VERSION}_{suffix}.tar.gz"
    )
    executable = cached_actionlint_binary(archive, url, checksum)
    binary = directory / "actionlint"
    binary.write_bytes(executable)
    binary.chmod(0o755)
    # Validate YAML, expressions, action inputs, and job dependencies. Shell and
    # Python linters are separate tools, not silently host-dependent checks.
    result = subprocess.run(
        [str(binary), "-shellcheck=", "-pyflakes=", *(str(path) for path in workflows)],
        cwd=ROOT, check=False,
    )
    if result.returncode == 0:
        print(f"Workflow lint passed: {len(workflows)} files, actionlint {VERSION}")
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
