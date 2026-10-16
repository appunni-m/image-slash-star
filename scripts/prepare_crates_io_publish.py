#!/usr/bin/env python3
"""Frame a verified crate archive as a crates.io publish request."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import struct
import subprocess
import sys
from typing import Any


ROOT = Path(__file__).resolve().parent.parent
PACKAGE_NAME = "image-slash-star"
DEPENDENCY_KINDS = {"build", "dev", "normal"}
MAX_U32 = (1 << 32) - 1


class PublishPayloadError(ValueError):
    """The candidate or package metadata cannot form a publish request."""


def cargo_metadata() -> dict[str, Any]:
    """Read package metadata without compiling or running package code."""

    process = subprocess.run(
        [
            "cargo",
            "metadata",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
            str(ROOT / "Cargo.toml"),
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    try:
        document = json.loads(process.stdout)
    except json.JSONDecodeError as error:
        raise PublishPayloadError("cargo metadata returned invalid JSON") from error

    packages = [
        package
        for package in document.get("packages", [])
        if package.get("name") == PACKAGE_NAME
    ]
    if len(packages) != 1:
        raise PublishPayloadError(
            f"expected one {PACKAGE_NAME} package, found {len(packages)}"
        )
    return packages[0]


def registry_metadata(
    package: dict[str, Any], *, package_root: Path = ROOT
) -> dict[str, Any]:
    """Convert Cargo metadata to the documented registry publish fields."""

    dependencies = []
    for dependency in package.get("dependencies", []):
        kind = dependency.get("kind") or "normal"
        if kind not in DEPENDENCY_KINDS:
            raise PublishPayloadError(f"unsupported dependency kind: {kind!r}")
        dependencies.append(
            {
                "name": dependency["name"],
                "version_req": dependency["req"],
                "features": dependency.get("features", []),
                "optional": dependency.get("optional", False),
                "default_features": dependency.get("uses_default_features", True),
                "target": dependency.get("target"),
                "kind": kind,
                "registry": dependency.get("registry"),
                "explicit_name_in_toml": dependency.get("rename"),
            }
        )

    readme_path = package.get("readme")
    if readme_path is None:
        readme_file = None
        readme_contents = None
    else:
        readme_file = str(readme_path)
        resolved_readme = (package_root / readme_file).resolve()
        if not resolved_readme.is_relative_to(package_root.resolve()):
            raise PublishPayloadError("package readme resolves outside the package")
        try:
            readme_contents = resolved_readme.read_text(encoding="utf-8")
        except OSError as error:
            raise PublishPayloadError(
                f"cannot read package readme {readme_file}: {error}"
            ) from error

    return {
        "name": package["name"],
        "vers": package["version"],
        "deps": dependencies,
        "features": package.get("features", {}),
        "authors": package.get("authors", []),
        "description": package.get("description"),
        "documentation": package.get("documentation"),
        "homepage": package.get("homepage"),
        "readme": readme_contents,
        "readme_file": readme_file,
        "keywords": package.get("keywords", []),
        "categories": package.get("categories", []),
        "license": package.get("license"),
        "license_file": package.get("license_file"),
        "repository": package.get("repository"),
        "badges": package.get("badges") or {},
        "links": package.get("links"),
        "rust_version": package.get("rust_version"),
    }


def encode_publish_request(metadata: dict[str, Any], archive: bytes) -> bytes:
    """Encode Cargo's little-endian length-prefixed registry request body."""

    metadata_bytes = json.dumps(
        metadata,
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode("utf-8")
    if len(metadata_bytes) > MAX_U32 or len(archive) > MAX_U32:
        raise PublishPayloadError("publish request fields exceed the registry limit")
    return (
        struct.pack("<I", len(metadata_bytes))
        + metadata_bytes
        + struct.pack("<I", len(archive))
        + archive
    )


def prepare(candidate: Path, output: Path, package: dict[str, Any]) -> None:
    """Write a request body containing metadata and the exact candidate bytes."""

    expected_name = f"{PACKAGE_NAME}-{package['version']}.crate"
    if package.get("name") != PACKAGE_NAME or candidate.name != expected_name:
        raise PublishPayloadError(f"expected verified candidate named {expected_name}")
    try:
        archive = candidate.read_bytes()
    except OSError as error:
        raise PublishPayloadError(f"cannot read candidate archive {candidate}: {error}") from error
    body = encode_publish_request(registry_metadata(package), archive)
    output.parent.mkdir(parents=True, exist_ok=True)
    try:
        output.write_bytes(body)
    except OSError as error:
        raise PublishPayloadError(f"cannot write publish request {output}: {error}") from error


def arguments() -> argparse.Namespace:
    """Parse a candidate archive and destination request-body path."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    """Create a publish request before the OIDC publishing job starts."""

    try:
        args = arguments()
        package = cargo_metadata()
        prepare(args.candidate, args.output, package)
        print(
            f"prepared crates.io request for {package['name']} {package['version']} "
            f"from {args.candidate.name}"
        )
        return 0
    except (OSError, KeyError, TypeError, PublishPayloadError, subprocess.CalledProcessError) as error:
        print(f"crates.io publish request preparation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
