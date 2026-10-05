#!/usr/bin/env python3
"""Prepare a pinned, opt-in native coverage exporter; never install a Rust dependency."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import shutil
import stat
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
DESCRIPTOR = ROOT / "scripts/coverage-exporter/llvm22.1.8.json"
PIN = json.loads(DESCRIPTOR.read_text())
PATCH = DESCRIPTOR.parent / PIN["patch"]["filename"]


def digest(path: Path) -> str:
    if not path.is_file():
        raise ValueError("expected a regular binding file: " + str(path))
    value = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024**2), b""):
            value.update(chunk)
    return value.hexdigest()


def capture(*argv: str, cwd: Path | None = None) -> str:
    return subprocess.check_output(argv, cwd=cwd, text=True).strip()


def rust_identity(toolchain: str) -> dict:
    host = PIN["platform"]
    if platform.system() != host["system"] or platform.machine() != host["machine"]:
        raise ValueError("the verified exporter recipe supports native macOS ARM64 only")
    if toolchain != PIN["rust"]["toolchain"]:
        raise ValueError("COVERAGE_TOOLCHAIN does not match the reviewed exporter descriptor")
    installed = capture("rustup", "toolchain", "list").splitlines()
    if not any(line.split()[0] == toolchain + "-" + host["rust_host"] for line in installed):
        raise ValueError("install the pinned toolchain and llvm-tools-preview through normal contributor setup")
    version = capture("rustc", "+" + toolchain, "-vV")
    fields = dict(line.split(": ", 1) for line in version.splitlines() if ": " in line)
    if (fields.get("commit-hash") != PIN["rust"]["commit"] or
            fields.get("host") != host["rust_host"] or
            fields.get("LLVM version") != PIN["rust"]["llvm_version"]):
        raise ValueError("installed Rust/LLVM/host identity differs from the reviewed pin")
    target = os.environ.get("CARGO_BUILD_TARGET")
    if target and target != host["rust_host"]:
        raise ValueError("the exporter recipe has no verified cross-target override")
    sysroot = Path(capture("rustc", "+" + toolchain, "--print", "sysroot"))
    profdata = (sysroot / "lib/rustlib" / host["rust_host"] / "bin/llvm-profdata").resolve()
    library = (sysroot / "lib/libLLVM.dylib").resolve()
    if "LLVM version " + PIN["rust"]["llvm_version"] not in capture(str(profdata), "--version"):
        raise ValueError("llvm-profdata is incompatible with the pinned Rust LLVM")
    return {"toolchain": toolchain, "rustc_verbose_version": version,
            "sysroot": str(sysroot.resolve()),
            "llvm_profdata": str(profdata), "llvm_profdata_sha256": digest(profdata),
            "llvm_library": str(library), "llvm_library_sha256": digest(library),
            "host_release": platform.release()}


def source_state(source: Path) -> str:
    if capture("git", "rev-parse", "HEAD", cwd=source) != PIN["llvm"]["commit"]:
        raise ValueError("LLVM checkout is not the pinned Rust LLVM commit")
    states = []
    for name, hashes in PIN["llvm"]["source_hashes"].items():
        value = digest(source / name)
        states.append(next((kind for kind in ("baseline", "candidate") if hashes[kind] == value), "invalid"))
    if len(set(states)) != 1 or states[0] == "invalid":
        raise ValueError("LLVM patch source hashes are inconsistent or unreviewed")
    status = subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=all"],
                                     cwd=source, text=True).rstrip("\n")
    changed = capture("git", "diff", "--name-only", cwd=source).splitlines()
    if states[0] == "baseline" and status:
        raise ValueError("unpatched LLVM source must be clean")
    if states[0] == "candidate":
        expected = sorted(PIN["llvm"]["source_hashes"])
        if sorted(changed) != expected or sorted(status.splitlines()) != sorted(" M " + f for f in expected):
            raise ValueError("LLVM checkout has changes beyond the exact reviewed patch")
    return states[0]


@contextmanager
def locked(prefix: Path):
    prefix.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(prefix / ".setup.lock", os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
    with os.fdopen(descriptor, "a") as handle:
        if not stat.S_ISREG(os.fstat(handle.fileno()).st_mode):
            raise ValueError("exporter lock must be an owned regular file")
        fcntl.flock(handle, fcntl.LOCK_EX)
        yield


def context(prefix: Path, toolchain: str) -> dict:
    return {"schema_version": 1, "prefix": str(prefix), "descriptor_sha256": digest(DESCRIPTOR),
            "recipe_sha256": digest(Path(__file__)), "patch_sha256": digest(PATCH),
            "rust": rust_identity(toolchain)}


def ready(prefix: Path, expected: dict) -> dict:
    if (prefix / "identity.json").is_symlink() or not (prefix / "identity.json").is_file():
        raise ValueError("exporter receipt must be an owned regular file")
    receipt = json.loads((prefix / "identity.json").read_text())
    if not isinstance(receipt, dict) or not isinstance(receipt.get("source"), str):
        raise ValueError("exporter receipt has an invalid structure")
    if receipt["context"] != expected:
        raise ValueError("existing exporter receipt differs; use a new isolated prefix")
    source = Path(receipt["source"])
    tool = prefix / "llvm-cov"
    required = {tool, prefix / "build/CMakeCache.txt", prefix / "build/build.ninja"}
    bindings = receipt["files"]
    if (receipt["llvm_cov"] != str(tool) or not isinstance(bindings, dict) or
            not {str(path) for path in required}.issubset(bindings)):
        raise ValueError("exporter receipt lacks the mandatory owned tool/cache bindings")
    for path in required:
        if path.is_symlink() or not path.is_file() or digest(path) != bindings[str(path)]:
            raise ValueError("exporter tool/build binding changed: " + str(path))
    if source_state(source) != "candidate":
        raise ValueError("exporter source is not the reviewed candidate")
    libraries = capture("otool", "-L", str(tool))
    external = {Path(line.strip().split(" (", 1)[0]).resolve() for line in libraries.splitlines()[1:]
                if Path(line.strip().split(" (", 1)[0]).is_file()}
    if set(bindings) != {str(path) for path in required | external}:
        raise ValueError("exporter receipt has missing or unexpected external-library bindings")
    for filename, sha256 in bindings.items():
        if digest(Path(filename)) != sha256:
            raise ValueError("exporter tool/build binding changed: " + filename)
    if (libraries != receipt["dynamic_libraries"] or
            capture(str(tool), "--version") != receipt["llvm_cov_version"]):
        raise ValueError("exporter version or linked-library identity changed")
    cache = {}
    for line in (prefix / "build/CMakeCache.txt").read_text().splitlines():
        if not line.startswith(("#", "//")) and ":" in line and "=" in line:
            key, value = line.split("=", 1)
            cache[key.split(":", 1)[0]] = value
    options = {flag[2:].split("=", 1)[0]: flag.split("=", 1)[1]
               for flag in PIN["build"]["cmake_options"] if flag.startswith("-D")}
    options.update(CMAKE_HOME_DIRECTORY=str(source / "llvm"),
                   CMAKE_CACHEFILE_DIR=str(prefix / "build"), CMAKE_GENERATOR="Ninja")
    if any(cache.get(key) != value for key, value in options.items()):
        raise ValueError("native cache differs from the pinned recipe/source/prefix")
    return receipt


def run_logged(argv: list[str], log: Path, prefix: Path, source: Path) -> None:
    environment = dict(os.environ, GIT_TERMINAL_PROMPT="0", CCACHE_DISABLE="1", SCCACHE_DISABLE="1")
    start = time.monotonic()
    roots = [prefix] if source.is_relative_to(prefix) else [prefix, source]
    command_receipt = {"argv": argv, "status": "running"}
    log.with_suffix(".json").write_text(json.dumps(command_receipt, indent=2) + "\n")
    with log.open("wb") as handle:
        proc = subprocess.Popen(argv, stdout=handle, stderr=subprocess.STDOUT, env=environment,
                                start_new_session=True)

        def stop_owned_group() -> None:
            if proc.poll() is None:
                os.killpg(proc.pid, signal.SIGTERM)
                try:
                    proc.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(proc.pid, signal.SIGKILL)
                    proc.wait()

        try:
            next_guard = start
            while proc.poll() is None:
                time.sleep(1)
                if time.monotonic() < next_guard:
                    continue
                next_guard = time.monotonic() + 15
                allocated = sum(int(capture("du", "-sk", str(path)).split()[0]) * 1024 for path in roots if path.exists())
                if allocated > PIN["build"]["allocated_limit_bytes"] or time.monotonic() - start > 14400:
                    stop_owned_group()
                    command_receipt["stopped_reason"] = "resource-or-duration-guard"
                    break
        except BaseException:
            stop_owned_group()
            command_receipt.update(status="interrupted", exit_code=proc.returncode,
                                  elapsed_seconds=time.monotonic() - start)
            log.with_suffix(".json").write_text(json.dumps(command_receipt, indent=2) + "\n")
            raise
    command_receipt.update(status="completed", exit_code=proc.returncode,
        elapsed_seconds=time.monotonic() - start, log_sha256=digest(log))
    log.with_suffix(".json").write_text(json.dumps(command_receipt, indent=2) + "\n")
    if proc.returncode or command_receipt.get("stopped_reason"):
        raise ValueError("native setup command failed or hit its guard; inspect " + str(log))


def setup(args, prefix: Path, expected: dict) -> None:
    custom = ("CFLAGS", "CXXFLAGS", "CPPFLAGS", "LDFLAGS", "CMAKE_TOOLCHAIN_FILE")
    if any(os.environ.get(name) for name in custom):
        raise ValueError("custom native build flags/toolchain are outside the pinned setup recipe")
    if (prefix / "identity.json").exists() or (prefix / "identity.json").is_symlink():
        receipt = ready(prefix, expected)
        if args.source and Path(args.source).resolve() != Path(receipt["source"]):
            raise ValueError("requested source differs from the bound exporter source")
        print("Verified existing exporter: " + receipt["llvm_cov"])
        return
    source = Path(args.source).resolve() if args.source else prefix / "source"
    build = prefix / "build"
    if build.exists() or build.is_symlink() or (prefix / "llvm-cov").exists() or (prefix / "llvm-cov").is_symlink():
        raise ValueError("native build directory has no bound receipt; use a fresh isolated prefix")
    if not args.source and source.is_symlink():
        raise ValueError("default LLVM source cannot be a symlink; select a prepared source explicitly")
    logs = prefix / ("setup-" + str(time.time_ns()))
    logs.mkdir()
    commands = []

    def execute(argv: list[str]) -> None:
        commands.append(argv)
        run_logged(argv, logs / f"{len(commands):02}.log", prefix, source)

    if not source.exists():
        execute(["git", "init", str(source)])
        execute(["git", "-C", str(source), "remote", "add", "origin", PIN["llvm"]["repository"]])
        execute(["git", "-C", str(source), "sparse-checkout", "init", "--cone"])
        execute(["git", "-C", str(source), "sparse-checkout", "set", *PIN["llvm"]["sparse_directories"]])
        execute(["git", "-c", "credential.helper=", "-C", str(source), "fetch", "--depth=1",
                 "--filter=blob:none", "origin", PIN["llvm"]["commit"]])
        execute(["git", "-c", "credential.helper=", "-C", str(source), "checkout", "--detach", PIN["llvm"]["commit"]])
    if source_state(source) == "baseline":
        execute(["git", "-C", str(source), "apply", "--check", str(PATCH)])
        execute(["git", "-C", str(source), "apply", "--whitespace=error-all", str(PATCH)])
    if source_state(source) != "candidate":
        raise ValueError("reviewed patch was not applied exactly")
    execute(["cmake", "-S", str(source / "llvm"), "-B", str(build), *PIN["build"]["cmake_options"]])
    execute(["cmake", "--build", str(build), "--target", "llvm-cov", "--parallel", str(PIN["build"]["jobs"])])
    tool = prefix / "llvm-cov"
    shutil.copy2(build / "bin/llvm-cov", tool)
    if "LLVM version " + PIN["rust"]["llvm_version"] not in capture(str(tool), "--version"):
        raise ValueError("built exporter has the wrong LLVM version")
    if source_state(source) != "candidate":
        raise ValueError("LLVM source changed during setup")
    files = [tool, build / "CMakeCache.txt", build / "build.ninja"]
    libraries = capture("otool", "-L", str(tool))
    files.extend(Path(line.strip().split(" (", 1)[0]).resolve() for line in libraries.splitlines()[1:]
                 if Path(line.strip().split(" (", 1)[0]).is_file())
    receipt = {"context": expected, "source": str(source), "llvm_cov": str(tool),
               "llvm_cov_version": capture(str(tool), "--version"), "dynamic_libraries": libraries,
               "commands": commands,
               "files": {str(path): digest(path) for path in files}, "logs": str(logs)}
    with tempfile.NamedTemporaryFile(mode="w", prefix="identity-", suffix=".pending", dir=prefix, delete=False) as handle:
        handle.write(json.dumps(receipt, indent=2) + "\n")
        pending = Path(handle.name)
    pending.replace(prefix / "identity.json")
    ready(prefix, expected)
    print("Pinned exporter ready: " + str(tool))


def validate_report(report: str) -> None:
    if any(character in report for character in ("$", "`", '"', "\\", "\r", "\n")):
        raise ValueError("COVERAGE_REPORT contains characters unsupported by the recursive Make recipes")


def coverage(args, prefix: Path, expected: dict) -> None:
    validate_report(args.report)
    receipt = ready(prefix, expected)
    environment = dict(os.environ)
    for variable, value in {"LLVM_COV": receipt["llvm_cov"], "LLVM_PROFDATA": expected["rust"]["llvm_profdata"]}.items():
        if environment.get(variable) and Path(environment[variable]).resolve() != Path(value).resolve():
            raise ValueError(variable + " conflicts with the verified exporter; caller setting was preserved")
        environment[variable] = value
    if environment.get("LLVM_COV_FLAGS") or environment.get("LLVM_PROFDATA_FLAGS"):
        raise ValueError("custom LLVM flags are outside this verified complete-source exporter recipe")
    jobserver = re.search(r"--jobserver(?:-auth|-fds)=(\d+),(\d+)", environment.get("MAKEFLAGS", ""))
    descriptors = tuple(int(fd) for fd in jobserver.groups()) if jobserver else ()
    for fd in descriptors:
        os.fstat(fd)
    subprocess.run([args.make, args.target, "COVERAGE_TOOLCHAIN=" + args.toolchain,
                    "COVERAGE_REPORT=" + args.report], cwd=ROOT, env=environment,
                   pass_fds=descriptors, check=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("setup", "run"))
    parser.add_argument("--prefix", type=Path, required=True)
    parser.add_argument("--source", default="", help="reuse an exact pinned LLVM checkout without downloading")
    parser.add_argument("--toolchain", default=PIN["rust"]["toolchain"])
    parser.add_argument("--make", default="make")
    parser.add_argument("--target", choices=("coverage", "coverage-complete"), default="coverage")
    parser.add_argument("--report", default="target/release-evidence/coverage.json")
    args = parser.parse_args()
    try:
        validate_report(args.report)
    except ValueError as error:
        parser.error(str(error))
    prefix = args.prefix.resolve()
    if prefix == Path("/") or prefix in (ROOT, ROOT / "target") or (prefix.is_relative_to(ROOT) and not prefix.is_relative_to(ROOT / "target")):
        parser.error("exporter prefix must be isolated under target/ or outside the checkout")
    if digest(PATCH) != PIN["patch"]["sha256"]:
        parser.error("reviewed native patch checksum mismatch")
    try:
        expected = context(prefix, args.toolchain)
        sysroot = Path(expected["rust"]["sysroot"])
        if prefix.is_relative_to(sysroot) or sysroot.is_relative_to(prefix) or ROOT.is_relative_to(prefix):
            raise ValueError("exporter prefix cannot overlap an installed toolchain or the checkout")
        if any(prefix.is_relative_to(Path(path)) for path in ("/usr", "/System", "/Library", "/opt/homebrew")):
            raise ValueError("exporter prefix cannot be a system or package-manager installation path")
        with locked(prefix):
            if args.mode == "setup":
                setup(args, prefix, expected)
            else:
                coverage(args, prefix, expected)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
