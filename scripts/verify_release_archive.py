#!/usr/bin/env python3
"""Build and verify the exact reproducible crates.io release archive."""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path, PurePosixPath


ROOT = Path(__file__).resolve().parent.parent
PACKAGE_NAME = "image-slash-star"
ARTIFACT_DIR = ROOT / "target" / "release-artifacts"
# The release archive is intentionally checked from a temporary extracted
# directory, so Cargo cannot discover the repository's rust-toolchain.toml.
# Keep the package and cross-target checks on the same pinned toolchain as CI.
RELEASE_TOOLCHAIN = "1.96.1"


class ReleaseError(RuntimeError):
    """A deterministic release invariant was not satisfied."""


def run(
    command: list[str],
    cwd: Path = ROOT,
    env: dict[str, str] | None = None,
) -> None:
    """Run one visible release command and fail on any non-zero status."""

    print("+ " + " ".join(command), flush=True)
    process_env = os.environ.copy()
    process_env["RUSTUP_TOOLCHAIN"] = RELEASE_TOOLCHAIN
    if env:
        process_env.update(env)
    subprocess.run(command, cwd=cwd, env=process_env, check=True)


def capture(
    command: list[str], cwd: Path = ROOT, env: dict[str, str] | None = None
) -> str:
    """Return stripped stdout from one read-only command."""

    process_env = os.environ.copy()
    process_env["RUSTUP_TOOLCHAIN"] = RELEASE_TOOLCHAIN
    if env:
        process_env.update(env)
    return subprocess.run(
        command,
        cwd=cwd,
        env=process_env,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def require_clean_source() -> str:
    """Require every tracked, staged, modified, and untracked input to be clean."""

    status = capture(["git", "status", "--porcelain=v1", "--untracked-files=all"])
    if status:
        raise ReleaseError(f"release source is not clean:\n{status}")
    run(["git", "diff", "--check"])
    return capture(["git", "rev-parse", "HEAD"])


def package_version(manifest_path: Path = ROOT / "Cargo.toml") -> str:
    """Read the package version through Cargo rather than a partial TOML parser."""

    document = json.loads(
        capture(
            [
                "cargo",
                "metadata",
                "--locked",
                "--no-deps",
                "--format-version",
                "1",
                "--manifest-path",
                str(manifest_path),
            ],
            cwd=manifest_path.parent,
        )
    )
    matches = [
        package
        for package in document["packages"]
        if package["name"] == PACKAGE_NAME
    ]
    if len(matches) != 1:
        raise ReleaseError(f"expected one {PACKAGE_NAME} package, found {matches}")
    return str(matches[0]["version"])


def lockfile_version(lockfile: Path) -> str:
    """Return the root package version recorded in one Cargo.lock."""

    text = lockfile.read_text(encoding="utf-8")
    pattern = re.compile(
        rf'\[\[package\]\]\nname = "{re.escape(PACKAGE_NAME)}"\nversion = "([^"]+)"'
    )
    matches = pattern.findall(text)
    if len(matches) != 1:
        raise ReleaseError(f"expected one {PACKAGE_NAME} row in {lockfile}")
    return matches[0]


def verify_release_identity() -> tuple[str, str]:
    """Verify source, manifest, lockfile, changelog, README, and optional tag."""

    commit = require_clean_source()
    version = package_version()
    if version != "0.1.0":
        raise ReleaseError(f"unexpected first-release version: {version}")
    if lockfile_version(ROOT / "Cargo.lock") != version:
        raise ReleaseError("Cargo.lock package version differs from Cargo metadata")
    changelog_heading = f"## [{version}] - 2026-09-08"
    if changelog_heading not in (ROOT / "CHANGELOG.md").read_text(encoding="utf-8"):
        raise ReleaseError(f"CHANGELOG.md is missing {changelog_heading!r}")
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    if f'version = "={version}"' not in readme:
        raise ReleaseError("README.md does not pin the exact release version")
    release_tag = os.environ.get("RELEASE_TAG")
    if release_tag and release_tag != f"v{version}":
        raise ReleaseError(
            f"release tag {release_tag!r} does not equal manifest tag v{version}"
        )
    return version, commit


def sha256(path: Path) -> str:
    """Return the lowercase SHA-256 digest of a file."""

    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(128 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def build_archive(target_dir: Path, version: str) -> Path:
    """Create the exact archive in an isolated Cargo target directory."""

    run(
        ["cargo", "package", "--locked", "--no-verify"],
        env={"CARGO_TARGET_DIR": str(target_dir)},
    )
    expected = target_dir / "package" / f"{PACKAGE_NAME}-{version}.crate"
    archives = list((target_dir / "package").glob(f"{PACKAGE_NAME}-*.crate"))
    if archives != [expected] or not expected.is_file():
        raise ReleaseError(f"Cargo did not create only the exact archive {expected}")
    return expected


def extract_archive(archive_path: Path, destination: Path, version: str) -> Path:
    """Extract only regular package entries beneath one exact archive root."""

    expected_root = f"{PACKAGE_NAME}-{version}"
    with tarfile.open(archive_path, "r:gz") as archive:
        members = archive.getmembers()
        if not members:
            raise ReleaseError("release archive is empty")
        for member in members:
            relative = PurePosixPath(member.name)
            if relative.is_absolute() or ".." in relative.parts:
                raise ReleaseError(f"unsafe archive path: {member.name}")
            if not relative.parts or relative.parts[0] != expected_root:
                raise ReleaseError(f"archive entry has the wrong root: {member.name}")
            if not (member.isfile() or member.isdir()):
                raise ReleaseError(
                    f"archive contains a link or special entry: {member.name}"
                )
        archive.extractall(destination)
    package_root = destination / expected_root
    if not package_root.is_dir():
        raise ReleaseError(f"archive did not contain {expected_root}")
    return package_root


def verify_vcs_identity(package_root: Path, commit: str) -> None:
    """Require Cargo's embedded VCS identity to match the reviewed clean commit."""

    path = package_root / ".cargo_vcs_info.json"
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ReleaseError(f"cannot read {path}: {error}") from error
    git = document.get("git")
    if not isinstance(git, dict) or git.get("sha1") != commit:
        raise ReleaseError(f"archive VCS identity does not match {commit}")
    if git.get("dirty", False) is not False:
        raise ReleaseError("archive VCS identity records a dirty source")


def verify_extracted_archive(package_root: Path, version: str, commit: str) -> None:
    """Compile, document, and inspect the exact extracted source package."""

    manifest = package_root / "Cargo.toml"
    if package_version(manifest) != version:
        raise ReleaseError("normalized archive manifest has the wrong version")
    if lockfile_version(package_root / "Cargo.lock") != version:
        raise ReleaseError("archive Cargo.lock has the wrong package version")
    verify_vcs_identity(package_root, commit)

    run(["python3", "scripts/verify_third_party_licenses.py"], cwd=package_root)
    with tempfile.TemporaryDirectory(prefix="image-slash-star-archive-build-") as build:
        env = {"CARGO_TARGET_DIR": build}
        common = ["cargo", "check", "--locked", "--manifest-path", str(manifest)]
        run(common + ["--no-default-features", "--lib"], cwd=package_root, env=env)
        run(common + ["--lib"], cwd=package_root, env=env)
        run(common + ["--all-targets", "--all-features"], cwd=package_root, env=env)
        for target in ("wasm32-unknown-unknown", "wasm32-wasip1"):
            run(
                common + ["--all-features", "--lib", "--target", target],
                cwd=package_root,
                env=env,
            )
        doc_env = {**env, "RUSTDOCFLAGS": "-D warnings"}
        run(
            [
                "cargo",
                "doc",
                "--locked",
                "--all-features",
                "--no-deps",
                "--manifest-path",
                str(manifest),
            ],
            cwd=package_root,
            env=doc_env,
        )
        run(
            [
                "cargo",
                "test",
                "--locked",
                "--doc",
                "--all-features",
                "--manifest-path",
                str(manifest),
            ],
            cwd=package_root,
            env=env,
        )


def retain_artifacts(archive_path: Path, version: str, commit: str) -> Path:
    """Retain the verified candidate and deterministic checksum/release notes."""

    ARTIFACT_DIR.mkdir(parents=True, exist_ok=True)
    retained = ARTIFACT_DIR / f"{PACKAGE_NAME}-{version}.crate"
    shutil.copyfile(archive_path, retained)
    digest = sha256(retained)
    (ARTIFACT_DIR / "SHA256SUMS").write_text(
        f"{digest}  {retained.name}\n", encoding="utf-8"
    )
    (ARTIFACT_DIR / "release-notes.md").write_text(
        "\n".join(
            [
                f"# {PACKAGE_NAME} {version}",
                "",
                "First registry release of the dependency-constrained Rust codec library.",
                "It is not a production-readiness claim: the documented AVIF, hostile-input,",
                "WASM semantic-matrix, and other roadmap boundaries remain explicit.",
                "",
                f"Source commit: `{commit}`",
                "",
                "See `CHANGELOG.md`, `README.md`, and `PRODUCTION_RELEASE_READINESS.md`",
                "in the tagged source for the complete scope and post-release checks.",
                "",
            ]
        ),
        encoding="utf-8",
    )
    return retained


def main() -> int:
    """Verify identity, reproducibility, package contents, and clean consumption."""

    try:
        version, commit = verify_release_identity()
        run(["python3", "scripts/verify_package_surface.py"])
        with tempfile.TemporaryDirectory(prefix="image-slash-star-release-") as temporary:
            temporary_root = Path(temporary)
            first = build_archive(temporary_root / "first-target", version)
            second = build_archive(temporary_root / "second-target", version)
            first_digest = sha256(first)
            second_digest = sha256(second)
            if first_digest != second_digest:
                raise ReleaseError(
                    "two clean package builds are not reproducible: "
                    f"{first_digest} != {second_digest}"
                )
            package_root = extract_archive(
                first, temporary_root / "extracted", version
            )
            verify_extracted_archive(package_root, version, commit)
            run(
                [
                    "python3",
                    "scripts/verify_package_consumer.py",
                    "--archive",
                    str(first),
                ]
            )
            retained = retain_artifacts(first, version, commit)
        print(
            f"release archive OK: {retained.name} sha256={sha256(retained)} "
            f"commit={commit}"
        )
        return 0
    except (OSError, ReleaseError, subprocess.CalledProcessError) as error:
        print(f"release archive verification failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
