#!/usr/bin/env python3
"""Require immutable external actions and validate workflows with actionlint."""
from __future__ import annotations

import errno
import hashlib
import http.client
import io
import os
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


def release_coverage_gate_violations(contents: str) -> list[str]:
    """Require tag coverage to pass before the release publish job can run."""

    jobs_match = re.search(r"(?m)^jobs:\s*$", contents)
    if jobs_match is None:
        return ["release workflow has no jobs section"]

    jobs_body = contents[jobs_match.end():]
    headers = list(re.finditer(r"(?m)^  ([A-Za-z0-9_-]+):\s*$", jobs_body))
    jobs = {}
    for index, header in enumerate(headers):
        end = headers[index + 1].start() if index + 1 < len(headers) else len(jobs_body)
        jobs[header.group(1)] = jobs_body[header.end():end]
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


def verify_candidate_artifact_rerun_safety(contents: str) -> bool:
    """Require CI's release candidate artifact to be replaceable on reruns."""

    step_match = re.search(
        r"(?ms)^      - name: Retain verified candidate bundle\s*$"
        r"(?P<step>.*?)(?=^      - |\Z)",
        contents,
    )
    step = step_match.group("step") if step_match is not None else ""
    if (
        re.search(
            r"(?m)^          name:\s*image-slash-star-candidate\s*$",
            step,
        )
        is None
        or re.search(r"(?m)^          overwrite:\s*true\s*$", step) is None
    ):
        print(
            "CI candidate artifact must keep its release name and overwrite prior uploads",
            file=sys.stderr,
        )
        return False
    return True


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
    release_workflow = ROOT / ".github" / "workflows" / "release.yml"
    if not verify_release_coverage_gate(release_workflow):
        return 1
    ci_workflow = ROOT / ".github" / "workflows" / "ci.yml"
    if not verify_candidate_artifact_rerun_safety(
        ci_workflow.read_text(encoding="utf-8")
    ):
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
