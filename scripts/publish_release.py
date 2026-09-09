#!/usr/bin/env python3
"""Publish the first crate idempotently and prove registry archive identity."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
PACKAGE_NAME = "image-slash-star"
API_ROOT = "https://crates.io/api/v1/crates"
USER_AGENT = "image-slash-star-release-verifier/0.1 (release automation)"


class PublishError(RuntimeError):
    """The registry or local release identity is unsafe to publish."""


def capture(command: list[str]) -> str:
    """Return stripped stdout from a successful local command."""

    return subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def package_version() -> str:
    """Read the exact local package version through Cargo metadata."""

    document = json.loads(
        capture(
            [
                "cargo",
                "metadata",
                "--locked",
                "--no-deps",
                "--format-version",
                "1",
            ]
        )
    )
    matches = [
        package
        for package in document["packages"]
        if package["name"] == PACKAGE_NAME
    ]
    if len(matches) != 1:
        raise PublishError(f"expected one {PACKAGE_NAME} package")
    return str(matches[0]["version"])


def request(url: str) -> urllib.request.Request:
    """Build one crates.io request with a registry-identifying user agent."""

    return urllib.request.Request(
        url,
        headers={"Accept": "application/json", "User-Agent": USER_AGENT},
    )


def registry_version(version: str) -> dict | None:
    """Return exact registry metadata, distinguishing absence from API failure."""

    url = f"{API_ROOT}/{PACKAGE_NAME}/{version}"
    try:
        with urllib.request.urlopen(request(url), timeout=30) as response:
            document = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise PublishError(f"crates.io metadata request failed: HTTP {error.code}") from error
    except (OSError, json.JSONDecodeError) as error:
        raise PublishError(f"crates.io metadata request failed: {error}") from error
    item = document.get("version")
    if not isinstance(item, dict) or item.get("num") != version:
        raise PublishError(f"crates.io returned unexpected metadata for {version}")
    checksum = item.get("checksum")
    if not isinstance(checksum, str) or len(checksum) != 64:
        raise PublishError(f"crates.io metadata is missing the {version} checksum")
    return item


def sha256(path: Path) -> str:
    """Return the lowercase SHA-256 digest of one archive."""

    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(128 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def download_registry_archive(version: str, destination: Path) -> str:
    """Download the immutable registry archive and verify its index checksum."""

    metadata = registry_version(version)
    if metadata is None:
        raise PublishError(f"{PACKAGE_NAME} {version} is not visible on crates.io")
    url = f"{API_ROOT}/{PACKAGE_NAME}/{version}/download"
    try:
        with urllib.request.urlopen(request(url), timeout=60) as response:
            with destination.open("wb") as output:
                shutil.copyfileobj(response, output)
    except (OSError, urllib.error.HTTPError) as error:
        raise PublishError(f"cannot download registry archive: {error}") from error
    digest = sha256(destination)
    if digest != metadata["checksum"]:
        raise PublishError(
            f"download checksum {digest} differs from crates.io index "
            f"{metadata['checksum']}"
        )
    return digest


def require_publish_approval() -> None:
    """Require explicit approval bound to the clean source commit."""

    status = capture(["git", "status", "--porcelain=v1", "--untracked-files=all"])
    if status:
        raise PublishError(f"publish checkout is not clean:\n{status}")
    commit = capture(["git", "rev-parse", "HEAD"])
    if os.environ.get("RELEASE_APPROVED") != "1":
        raise PublishError("set RELEASE_APPROVED=1 only after reviewing release-verify")
    if os.environ.get("RELEASE_CI_SHA") != commit:
        raise PublishError(
            "RELEASE_CI_SHA must equal the exact clean commit being published"
        )


def publish(version: str) -> None:
    """Ask Cargo to publish from the clean exact source tree."""

    require_publish_approval()
    print(f"+ cargo publish --locked ({PACKAGE_NAME} {version})", flush=True)
    subprocess.run(["cargo", "publish", "--locked"], cwd=ROOT, check=True)


def wait_until_visible(version: str, timeout_seconds: int = 300) -> None:
    """Wait for the immutable version metadata to become registry-visible."""

    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        if registry_version(version) is not None:
            return
        time.sleep(5)
    raise PublishError(
        f"{PACKAGE_NAME} {version} was not visible within {timeout_seconds} seconds"
    )


def arguments() -> argparse.Namespace:
    """Parse registry probe or publish-and-verify mode."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--probe", action="store_true")
    parser.add_argument("--publish-if-missing", action="store_true")
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def verify_candidate(candidate: Path, version: str) -> str:
    """Require an exact candidate filename and return its digest."""

    candidate = candidate.resolve()
    expected_name = f"{PACKAGE_NAME}-{version}.crate"
    if candidate.name != expected_name or not candidate.is_file():
        raise PublishError(f"expected verified candidate {expected_name}: {candidate}")
    return sha256(candidate)


def main() -> int:
    """Probe or publish, then compare the exact candidate with crates.io."""

    try:
        args = arguments()
        version = package_version()
        present = registry_version(version) is not None
        if args.probe:
            if args.candidate or args.output or args.publish_if_missing:
                raise PublishError("--probe cannot be combined with publish arguments")
            print("present" if present else "absent")
            return 0
        if args.candidate is None or args.output is None:
            raise PublishError("--candidate and --output are required outside --probe")

        candidate_digest = verify_candidate(args.candidate, version)
        if not present:
            if not args.publish_if_missing:
                raise PublishError(f"{PACKAGE_NAME} {version} is absent from crates.io")
            publish(version)
            wait_until_visible(version)
        else:
            print(f"{PACKAGE_NAME} {version} already exists; verifying immutable bytes")

        args.output.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="image-slash-star-registry-") as temporary:
            downloaded = Path(temporary) / f"{PACKAGE_NAME}-{version}.crate"
            registry_digest = download_registry_archive(version, downloaded)
            if registry_digest != candidate_digest:
                raise PublishError(
                    "registry archive differs from the tag-built candidate: "
                    f"registry={registry_digest} candidate={candidate_digest}"
                )
            shutil.copyfile(downloaded, args.output)
        print(
            f"registry archive identity OK: {args.output.name} sha256={registry_digest}"
        )
        return 0
    except (OSError, PublishError, subprocess.CalledProcessError) as error:
        print(f"release publication failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
