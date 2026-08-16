#!/usr/bin/env python3
"""Compile and run a clean consumer against the exact Cargo package archive.

This verifies the published source boundary, not the repository workspace:
Cargo first creates the archive, then a separate temporary package depends on
the extracted archive directory with default features disabled and PNG enabled.
The embedded input and assertions mirror ``examples/package_smoke.rs``.
"""

from __future__ import annotations

import os
import subprocess
import tarfile
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
PACKAGE_DIR = ROOT / "target" / "package"
PACKAGE_COMMAND = ["cargo", "package", "--allow-dirty", "--locked", "--no-verify"]
PNG_BYTES = """\
0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x68,
0x60, 0x60, 0x00, 0x00, 0x01, 0x84, 0x00, 0x81, 0xf9, 0xfe, 0x65, 0x88, 0x00, 0x00, 0x00,
0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
"""


def run(command: list[str], cwd: Path, env: dict[str, str] | None = None) -> None:
    result = subprocess.run(command, cwd=cwd, env=env, check=False, text=True)
    if result.returncode:
        raise SystemExit(result.returncode)


def extract_package(archive_path: Path, destination: Path) -> None:
    """Extract Cargo's local archive without allowing path traversal."""

    root = destination.resolve()
    with tarfile.open(archive_path, "r:gz") as archive:
        for member in archive.getmembers():
            member_path = (root / member.name).resolve()
            if member_path != root and root not in member_path.parents:
                raise RuntimeError(f"package archive escapes its extraction root: {member.name}")
        archive.extractall(root)


def main() -> int:
    run(PACKAGE_COMMAND, ROOT)
    archives = sorted(PACKAGE_DIR.glob("image-slash-star-*.crate"), key=lambda path: path.stat().st_mtime)
    if not archives:
        raise RuntimeError(f"Cargo did not create an archive in {PACKAGE_DIR}")

    with tempfile.TemporaryDirectory(prefix="image-slash-star-package-") as temporary:
        temporary_root = Path(temporary)
        extract_package(archives[-1], temporary_root)
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

    print("clean package consumer OK: packaged archive compiled and decoded PNG")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
