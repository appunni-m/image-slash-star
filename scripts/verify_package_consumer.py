#!/usr/bin/env python3
"""Compile and run a clean consumer against the exact Cargo package archive.

This verifies the published source boundary, not the repository workspace:
Cargo first creates the archive, then a separate temporary package depends on
the extracted archive directory with default features disabled and PNG enabled.
The embedded input and assertions mirror ``examples/package_smoke.rs``.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import tarfile
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
PACKAGE_DIR = ROOT / "target" / "package"
PACKAGE_COMMAND = ["cargo", "package", "--locked", "--no-verify"]
RELEASE_TOOLCHAIN = "1.96.1"
PNG_BYTES = """\
0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x68,
0x60, 0x60, 0x00, 0x00, 0x01, 0x84, 0x00, 0x81, 0xf9, 0xfe, 0x65, 0x88, 0x00, 0x00, 0x00,
0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
"""


def run(command: list[str], cwd: Path, env: dict[str, str] | None = None) -> None:
    process_env = os.environ.copy()
    process_env["RUSTUP_TOOLCHAIN"] = RELEASE_TOOLCHAIN
    if env:
        process_env.update(env)
    result = subprocess.run(command, cwd=cwd, env=process_env, check=False, text=True)
    if result.returncode:
        raise SystemExit(result.returncode)


def extract_package(archive_path: Path, destination: Path) -> None:
    """Extract Cargo's local archive without allowing path traversal."""

    root = destination.resolve()
    with tarfile.open(archive_path, "r:gz") as archive:
        for member in archive.getmembers():
            if not (member.isfile() or member.isdir()):
                raise RuntimeError(
                    f"package archive contains unsupported link or special entry: {member.name}"
                )
            member_path = (root / member.name).resolve()
            if member_path != root and root not in member_path.parents:
                raise RuntimeError(f"package archive escapes its extraction root: {member.name}")
        archive.extractall(root)


def package_version() -> str:
    """Return the release version from Cargo's normalized metadata."""

    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        cwd=ROOT,
        env={**os.environ, "RUSTUP_TOOLCHAIN": RELEASE_TOOLCHAIN},
        check=True,
        capture_output=True,
        text=True,
    )
    document = json.loads(result.stdout)
    packages = [
        package
        for package in document["packages"]
        if package["name"] == "image-slash-star"
    ]
    if len(packages) != 1:
        raise RuntimeError(f"expected one image-slash-star package, found {packages}")
    return str(packages[0]["version"])


def arguments() -> argparse.Namespace:
    """Parse an optional exact archive supplied by release verification."""

    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--archive",
        type=Path,
        help="verify this exact .crate archive instead of building one",
    )
    return parser.parse_args()


def main() -> int:
    args = arguments()
    version = package_version()
    expected_name = f"image-slash-star-{version}.crate"
    if args.archive is None:
        run(PACKAGE_COMMAND, ROOT)
        archive_path = PACKAGE_DIR / expected_name
    else:
        archive_path = args.archive.resolve()
    if archive_path.name != expected_name or not archive_path.is_file():
        raise RuntimeError(f"expected exact release archive {expected_name}: {archive_path}")

    with tempfile.TemporaryDirectory(prefix="image-slash-star-package-") as temporary:
        temporary_root = Path(temporary)
        extract_package(archive_path, temporary_root)
        package_roots = [path for path in temporary_root.iterdir() if path.is_dir()]
        if len(package_roots) != 1:
            raise RuntimeError(f"expected one extracted package root, found {package_roots}")
        package_root = package_roots[0]

        consumer = temporary_root / "consumer"
        (consumer / "src").mkdir(parents=True)
        (consumer / "Cargo.toml").write_text(
            """[package]\nname = \"image-slash-star-package-consumer\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nimage-slash-star = { path = %r, default-features = false, features = [\"png\"] }\n"""
            % str(package_root),
            encoding="utf-8",
        )
        (consumer / "src" / "main.rs").write_text(
            """#![allow(unused_crate_dependencies)]\n\nuse image_slash_star::{decode, detect_format, inspect, ImageFormat, ImageMode, ImageResult};\n\nconst PNG: &[u8] = &[\n%s];\n\nfn main() -> ImageResult<()> {\n    assert_eq!(detect_format(PNG)?, ImageFormat::Png);\n    let info = inspect(PNG)?;\n    assert_eq!((info.width, info.height, info.mode), (1, 1, ImageMode::Rgb8));\n    let decoded = decode(PNG)?;\n    assert_eq!(decoded.format, ImageFormat::Png);\n    assert_eq!(decoded.content.pixels, [128, 0, 0]);\n    Ok(())\n}\n""" % PNG_BYTES,
            encoding="utf-8",
        )
        target_dir = temporary_root / "consumer-target"
        environment = os.environ.copy()
        environment["CARGO_TARGET_DIR"] = str(target_dir)
        run(
            [
                "cargo",
                "generate-lockfile",
                "--offline",
                "--manifest-path",
                str(consumer / "Cargo.toml"),
            ],
            consumer,
            environment,
        )
        run(
            [
                "cargo",
                "run",
                "--offline",
                "--locked",
                "--quiet",
                "--manifest-path",
                str(consumer / "Cargo.toml"),
            ],
            consumer,
            environment,
        )

    print(
        "clean package consumer OK: exact packaged archive compiled and decoded PNG "
        f"({archive_path.name})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
