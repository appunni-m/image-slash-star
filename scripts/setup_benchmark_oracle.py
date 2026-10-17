#!/usr/bin/env python3
"""Build the checksum-pinned TurboJPEG benchmark oracle, never a runtime dependency."""
from __future__ import annotations

import errno
import hashlib
import http.client
import json
import os
from pathlib import Path
import shutil
import socket
import ssl
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
VERSION = "3.2.0"
URL = f"https://github.com/libjpeg-turbo/libjpeg-turbo/releases/download/{VERSION}/libjpeg-turbo-{VERSION}.tar.gz"
SHA256 = "6f30092cef9fb839779646608f4ee14ae3cbac989c47fa05e841b0841f09878e"
BASE = ROOT / "target" / "benchmark-oracle" / VERSION
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


def sha256(path: Path) -> str:
    """Return the lowercase SHA-256 digest of an archive file."""

    digest = hashlib.sha256()
    with path.open("rb") as archive:
        while chunk := archive.read(128 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def is_transient_download_failure(error: BaseException) -> bool:
    """Return whether a download failure is likely to recover on retry."""

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


def download_archive(archive: Path) -> None:
    """Download, verify, and atomically cache the pinned source archive."""

    request = urllib.request.Request(
        URL, headers={"User-Agent": "image-slash-star-benchmark"}
    )
    for attempt in range(1, DOWNLOAD_ATTEMPTS + 1):
        temporary_path = None
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                with tempfile.NamedTemporaryFile(
                    dir=archive.parent,
                    prefix=f".{archive.name}.",
                    delete=False,
                ) as temporary:
                    temporary_path = Path(temporary.name)
                    shutil.copyfileobj(response, temporary)
                    temporary.flush()
                    os.fsync(temporary.fileno())

            actual = sha256(temporary_path)
            if actual != SHA256:
                raise SystemExit("official TurboJPEG archive checksum mismatch")
            os.replace(temporary_path, archive)
            return
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
        finally:
            if temporary_path is not None:
                temporary_path.unlink(missing_ok=True)

        delay = DOWNLOAD_RETRY_BACKOFF_SECONDS * (2 ** (attempt - 1))
        print(
            f"TurboJPEG archive download attempt {attempt}/{DOWNLOAD_ATTEMPTS} "
            f"failed ({failure}); retrying in {delay}s",
            file=sys.stderr,
        )
        time.sleep(delay)

    raise RuntimeError("TurboJPEG archive download retry loop exited unexpectedly")


def ensure_archive(archive: Path) -> None:
    """Reuse only a verified cache; replace a corrupt one from the pinned URL."""

    if archive.exists():
        try:
            cached_digest = sha256(archive)
        except FileNotFoundError:
            pass
        else:
            if cached_digest == SHA256:
                return
            archive.unlink(missing_ok=True)
    download_archive(archive)


def main() -> None:
    if not BASE.resolve().is_relative_to(ROOT.resolve() / "target"):
        raise SystemExit("benchmark oracle must remain under the repository target directory")
    BASE.mkdir(parents=True, exist_ok=True)
    archive = BASE / f"libjpeg-turbo-{VERSION}.tar.gz"
    source = BASE / f"libjpeg-turbo-{VERSION}"
    install = BASE / "install"
    ensure_archive(archive)
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
    cmake = os.environ.get("BENCHMARK_CMAKE", "cmake")
    commands = [
        [cmake, "-S", str(source), "-B", str(BASE / "build"),
         "-DCMAKE_BUILD_TYPE=Release", f"-DCMAKE_INSTALL_PREFIX={install}",
         "-DENABLE_SHARED=ON", "-DENABLE_STATIC=OFF", "-DWITH_TURBOJPEG=ON", "-DWITH_SIMD=ON"],
        [cmake, "--build", str(BASE / "build"), "--parallel", "2"],
        [cmake, "--install", str(BASE / "build")],
    ]
    for command in commands:
        subprocess.run(command, check=True)
    (BASE / "identity.json").write_text(json.dumps({"version": VERSION, "url": URL, "sha256": SHA256,
                                                  "commands": commands}, indent=2) + "\n")
    print(f"Pinned TurboJPEG {VERSION} installed at {install}")


if __name__ == "__main__":
    main()
