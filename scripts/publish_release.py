#!/usr/bin/env python3
"""Verify exact registry archive identity without publishing credentials."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
import hashlib
import http.client
import json
import math
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Callable, TypeVar

ROOT = Path(__file__).resolve().parent.parent
PACKAGE_NAME = "image-slash-star"
API_ROOT = "https://crates.io/api/v1/crates"
USER_AGENT = "image-slash-star-release-verifier/0.1 (release automation)"
REQUEST_ATTEMPTS = 4
MAX_RETRY_DELAY_SECONDS = 60.0
RETRYABLE_HTTP_STATUSES = {408, 425, 429, 500, 502, 503, 504}
Result = TypeVar("Result")


class PublishError(RuntimeError):
    """The registry or local release identity cannot be verified."""


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


def request(url: str, *, accept: str = "application/json") -> urllib.request.Request:
    """Request the appropriate metadata or archive representation."""

    return urllib.request.Request(
        url,
        headers={"Accept": accept, "User-Agent": USER_AGENT},
    )


def _retryable_http_error(error: urllib.error.HTTPError) -> bool:
    headers = error.headers or {}
    delay = _server_retry_delay(error)
    if error.code in RETRYABLE_HTTP_STATUSES:
        return delay is None or delay <= MAX_RETRY_DELAY_SECONDS
    if error.code != 403:
        return False
    if headers.get("Retry-After") is not None:
        return delay is None or delay <= MAX_RETRY_DELAY_SECONDS
    return headers.get("X-RateLimit-Remaining") == "0" and (
        delay is not None and delay <= MAX_RETRY_DELAY_SECONDS
    )


def _server_retry_delay(error: urllib.error.HTTPError) -> float | None:
    headers = error.headers or {}
    retry_after = headers.get("Retry-After")
    if retry_after:
        try:
            seconds = float(retry_after)
        except ValueError:
            try:
                retry_at = parsedate_to_datetime(retry_after)
                if retry_at.tzinfo is None:
                    retry_at = retry_at.replace(tzinfo=timezone.utc)
                seconds = (retry_at - datetime.now(timezone.utc)).total_seconds()
            except (TypeError, ValueError, OverflowError):
                seconds = float("nan")
        if math.isfinite(seconds):
            return max(seconds, 0.0)

    reset = headers.get("X-RateLimit-Reset")
    if reset is not None:
        try:
            reset_at = float(reset)
        except ValueError:
            return None
        if math.isfinite(reset_at):
            return max(reset_at - datetime.now(timezone.utc).timestamp(), 0.0)
    return None


def _retry_delay(error: BaseException, attempt: int) -> float:
    if isinstance(error, urllib.error.HTTPError):
        server_delay = _server_retry_delay(error)
        if server_delay is not None:
            return min(server_delay, MAX_RETRY_DELAY_SECONDS)
    return min(float(2 ** (attempt - 1)), MAX_RETRY_DELAY_SECONDS)


def _retry_registry_request(action: Callable[[], Result]) -> Result:
    """Retry bounded transient registry failures without weakening validation."""

    for attempt in range(1, REQUEST_ATTEMPTS + 1):
        try:
            return action()
        except urllib.error.HTTPError as error:
            if not _retryable_http_error(error) or attempt == REQUEST_ATTEMPTS:
                raise
            delay = _retry_delay(error, attempt)
        except (OSError, http.client.HTTPException, json.JSONDecodeError) as error:
            if attempt == REQUEST_ATTEMPTS:
                raise
            delay = _retry_delay(error, attempt)
        time.sleep(delay)
    raise AssertionError("unreachable registry retry state")


def registry_version(version: str) -> dict | None:
    """Return exact registry metadata, distinguishing absence from API failure."""

    url = f"{API_ROOT}/{PACKAGE_NAME}/{version}"
    try:
        def read_metadata() -> dict:
            with urllib.request.urlopen(request(url), timeout=30) as response:
                return json.load(response)

        document = _retry_registry_request(read_metadata)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise PublishError(f"crates.io metadata request failed: HTTP {error.code}") from error
    except (OSError, http.client.HTTPException, json.JSONDecodeError) as error:
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
        # crates.io returns {"url": ...} for application/json clients. Ask
        # for binary data so the endpoint redirects to the actual .crate.
        def download() -> None:
            with urllib.request.urlopen(
                request(url, accept="application/octet-stream"), timeout=60
            ) as response:
                with destination.open("wb") as output:
                    shutil.copyfileobj(response, output)

        _retry_registry_request(download)
    except (OSError, urllib.error.HTTPError) as error:
        destination.unlink(missing_ok=True)
        raise PublishError(f"cannot download registry archive: {error}") from error
    except http.client.HTTPException as error:
        destination.unlink(missing_ok=True)
        raise PublishError(f"cannot download registry archive: {error}") from error
    digest = sha256(destination)
    if digest != metadata["checksum"]:
        raise PublishError(
            f"download checksum {digest} differs from crates.io index "
            f"{metadata['checksum']}"
        )
    return digest


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
    """Parse registry probe or candidate-verification mode."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--probe", action="store_true")
    parser.add_argument("--wait-until-visible", action="store_true")
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
    """Probe or compare the exact candidate with crates.io."""

    try:
        args = arguments()
        version = package_version()
        present = registry_version(version) is not None
        if args.probe:
            if args.candidate or args.output or args.wait_until_visible:
                raise PublishError("--probe cannot be combined with verification arguments")
            print("present" if present else "absent")
            return 0
        if args.candidate is None or args.output is None:
            raise PublishError("--candidate and --output are required outside --probe")

        candidate_digest = verify_candidate(args.candidate, version)
        if not present:
            if args.wait_until_visible:
                wait_until_visible(version)
            else:
                raise PublishError(f"{PACKAGE_NAME} {version} is absent from crates.io")
        print(f"{PACKAGE_NAME} {version} is visible; verifying immutable bytes")

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
        print(f"release registry verification failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
