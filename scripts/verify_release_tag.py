#!/usr/bin/env python3
"""Verify that a release workflow's annotated tag still names its commit."""

from __future__ import annotations

import os
import re
import subprocess
import sys
from collections.abc import Callable


COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")


class ReleaseTagError(RuntimeError):
    """The release tag is absent, mutable, or points at another commit."""


def capture(command: list[str]) -> str:
    """Return stripped stdout from a successful local git command."""

    return subprocess.run(
        command,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def verify_remote_tag(
    tag: str,
    expected_commit: str,
    *,
    capture_command: Callable[[list[str]], str] | None = None,
) -> str:
    """Require an annotated remote tag that peels to the expected commit."""

    run = capture if capture_command is None else capture_command
    tag_ref = f"refs/tags/{tag}"
    peeled_ref = f"{tag_ref}^{{}}"
    if not tag or not COMMIT_RE.fullmatch(expected_commit):
        raise ReleaseTagError("release tag and expected commit are required")
    try:
        run(["git", "check-ref-format", tag_ref])
        output = run(["git", "ls-remote", "--tags", "origin", tag_ref, peeled_ref])
    except subprocess.CalledProcessError as error:
        raise ReleaseTagError(f"cannot verify remote release tag {tag}: {error}") from error

    refs: dict[str, str] = {}
    for line in output.splitlines():
        fields = line.split("\t", maxsplit=1)
        if len(fields) == 2 and fields[1] in {tag_ref, peeled_ref}:
            refs[fields[1]] = fields[0]
    if tag_ref not in refs or peeled_ref not in refs:
        raise ReleaseTagError(f"remote release tag {tag} must be an annotated tag")
    if not COMMIT_RE.fullmatch(refs[peeled_ref]):
        raise ReleaseTagError(f"remote release tag {tag} does not peel to a commit SHA")
    if refs[peeled_ref] != expected_commit:
        raise ReleaseTagError(
            f"remote release tag {tag} resolves to {refs[peeled_ref]}, "
            f"expected {expected_commit}"
        )
    return refs[peeled_ref]


def verify_current_release_tag() -> str:
    """Bind the checked-out source and remote tag to this workflow event."""

    tag = os.environ.get("GITHUB_REF_NAME", "")
    ref = os.environ.get("GITHUB_REF", "")
    expected_commit = os.environ.get("GITHUB_SHA", "")
    if not tag.startswith("v") or ref != f"refs/tags/{tag}":
        raise ReleaseTagError("release verification requires a v-prefixed tag event")
    if not COMMIT_RE.fullmatch(expected_commit):
        raise ReleaseTagError("GITHUB_SHA must be a full commit SHA")
    checkout_commit = capture(["git", "rev-parse", "HEAD"])
    if checkout_commit != expected_commit:
        raise ReleaseTagError(
            f"checked-out commit {checkout_commit} differs from workflow commit {expected_commit}"
        )
    return verify_remote_tag(tag, expected_commit)


def main() -> int:
    try:
        commit = verify_current_release_tag()
    except (OSError, ReleaseTagError, subprocess.CalledProcessError) as error:
        print(f"release tag verification failed: {error}", file=sys.stderr)
        return 1
    print(f"annotated release tag {os.environ['GITHUB_REF_NAME']} still resolves to {commit}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
