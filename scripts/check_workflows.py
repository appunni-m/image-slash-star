#!/usr/bin/env python3
"""Require immutable external actions and validate workflows with actionlint."""
from __future__ import annotations

import hashlib
import io
from pathlib import Path
import platform
import re
import subprocess
import sys
import tarfile
import urllib.request


VERSION = "1.7.12"
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


def main() -> int:
    workflows = sorted((ROOT / ".github" / "workflows").glob("*.y*ml"))
    if not workflows:
        raise SystemExit("No GitHub workflows found")
    if not verify_action_pins(workflows):
        return 1

    target = (platform.system(), platform.machine())
    if target not in ARCHIVES:
        raise SystemExit(f"Workflow lint supports Linux/macOS x86-64 and ARM64; got {target}")
    suffix, checksum = ARCHIVES[target]
    directory = ROOT / "target" / "workflow-tools" / f"actionlint-{VERSION}-{suffix}"
    directory.mkdir(parents=True, exist_ok=True)
    archive = directory / "archive.tar.gz"
    if archive.exists():
        data = archive.read_bytes()
    else:
        url = f"https://github.com/rhysd/actionlint/releases/download/v{VERSION}/actionlint_{VERSION}_{suffix}.tar.gz"
        with urllib.request.urlopen(url, timeout=60) as response:
            data = response.read()
    executable = binary_bytes(data, checksum)
    archive.write_bytes(data)
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
