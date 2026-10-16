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
import time
import urllib.error
import urllib.request


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
        "always() && "
        "(needs.benchmark-data.result == 'success' || "
        "github.event_name == 'pull_request') && "
        f"({DOCS_WORKFLOW_RUN_GUARD})"
    ),
    "examples": DOCS_WORKFLOW_RUN_GUARD,
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
        if uses is None or not uses.startswith("actions/upload-artifact@"):
            continue

        artifact_match = re.search(r"(?m)^          name:\s*(.+)$", body)
        artifact_name = artifact_match.group(1) if artifact_match is not None else "<unnamed>"
        if re.search(r"(?m)^          overwrite:\s*true\s*$", body) is None:
            violations.append(
                f"artifact {artifact_name!r} must set overwrite: true for same-run retries"
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
