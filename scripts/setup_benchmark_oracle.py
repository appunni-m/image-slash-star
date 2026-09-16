#!/usr/bin/env python3
"""Build the checksum-pinned TurboJPEG benchmark oracle, never a runtime dependency."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
VERSION = "3.2.0"
URL = f"https://github.com/libjpeg-turbo/libjpeg-turbo/releases/download/{VERSION}/libjpeg-turbo-{VERSION}.tar.gz"
SHA256 = "6f30092cef9fb839779646608f4ee14ae3cbac989c47fa05e841b0841f09878e"
BASE = ROOT / "target" / "benchmark-oracle" / VERSION


def main() -> None:
    if not BASE.resolve().is_relative_to(ROOT.resolve() / "target"):
        raise SystemExit("benchmark oracle must remain under the repository target directory")
    BASE.mkdir(parents=True, exist_ok=True)
    archive = BASE / f"libjpeg-turbo-{VERSION}.tar.gz"
    source = BASE / f"libjpeg-turbo-{VERSION}"
    install = BASE / "install"
    if not archive.exists():
        request = urllib.request.Request(URL, headers={"User-Agent": "image-slash-star-benchmark"})
        with urllib.request.urlopen(request, timeout=60) as response:
            data = response.read()
        if hashlib.sha256(data).hexdigest() != SHA256:
            raise SystemExit("official TurboJPEG archive checksum mismatch")
        archive.write_bytes(data)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
        raise SystemExit("cached TurboJPEG archive checksum mismatch")
    if not source.exists():
        with tempfile.TemporaryDirectory(prefix="extract-", dir=BASE) as directory:
            destination = Path(directory).resolve()
            with tarfile.open(archive, "r:gz") as bundle:
                for member in bundle.getmembers():
                    target = (destination / member.name).resolve()
                    if not (member.isfile() or member.isdir()) or not target.is_relative_to(destination):
                        raise SystemExit(f"unexpected source archive entry: {member.name}")
                bundle.extractall(destination)
            extracted = destination / f"libjpeg-turbo-{VERSION}"
            (extracted / ".source-sha256").write_text(SHA256 + "\n")
            extracted.rename(source)
    if (source / ".source-sha256").read_text().strip() != SHA256:
        raise SystemExit("oracle source is not the verified extraction")
    commands = [
        ["cmake", "-S", str(source), "-B", str(BASE / "build"),
         "-DCMAKE_BUILD_TYPE=Release", f"-DCMAKE_INSTALL_PREFIX={install}",
         "-DENABLE_SHARED=ON", "-DENABLE_STATIC=OFF", "-DWITH_TURBOJPEG=ON", "-DWITH_SIMD=ON"],
        ["cmake", "--build", str(BASE / "build"), "--parallel", "2"],
        ["cmake", "--install", str(BASE / "build")],
    ]
    for command in commands:
        subprocess.run(command, check=True)
    (BASE / "identity.json").write_text(json.dumps({"version": VERSION, "url": URL, "sha256": SHA256,
                                                  "commands": commands}, indent=2) + "\n")
    print(f"Pinned TurboJPEG {VERSION} installed at {install}")


if __name__ == "__main__":
    main()
