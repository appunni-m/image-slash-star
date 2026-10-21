#!/usr/bin/env bash
set -euo pipefail

snapshot=${UBUNTU_APT_SNAPSHOT:?UBUNTU_APT_SNAPSHOT must name the pinned Ubuntu snapshot}
github_env=${GITHUB_ENV:?GITHUB_ENV must point to the workflow environment file}
snapshot_log=$(mktemp)
trap 'rm -f "$snapshot_log"' EXIT

packages=(cmake=3.28.3-1build7 nasm=2.16.01-1build1)

snapshot_failure_is_http_5xx() {
  python3 - "$snapshot_log" <<'PY'
from pathlib import Path
import re
import sys

lines = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace").splitlines()
for index, line in enumerate(lines):
    if "https://snapshot.ubuntu.com/ubuntu/" not in line:
        continue
    nearby = lines[index:index + 3]
    if any(re.search(r"(?<!\d)5\d{2}(?!\d)", candidate) for candidate in nearby):
        raise SystemExit(0)
raise SystemExit(1)
PY
}

show_snapshot_failure() {
  echo "Ubuntu snapshot update failed (attempt $1/3); final apt output follows:" >&2
  tail -n 40 "$snapshot_log" >&2
}

snapshot_updated=false
for attempt in 1 2 3; do
  echo "Refreshing Ubuntu package snapshot (attempt ${attempt}/3)"
  # Retain live Release metadata so Noble can detect snapshot-capable sources.
  if sudo apt-get \
    -o Acquire::Retries=3 \
    -o APT::Update::Error-Mode=any \
    update --snapshot "$snapshot" >"$snapshot_log" 2>&1; then
    tail -n 20 "$snapshot_log"
    snapshot_updated=true
    break
  fi

  show_snapshot_failure "$attempt"
  if [[ "$attempt" -lt 3 ]]; then
    sleep "$((attempt * 10))"
  fi
done

if [[ "$snapshot_updated" == true ]]; then
  echo "Verifying pinned CMake and NASM from Ubuntu snapshot $snapshot"
  if ! sudo apt-get --simulate --yes --snapshot "$snapshot" install "${packages[@]}"; then
    echo "Pinned CMake and NASM could not be resolved from the Ubuntu snapshot" >&2
    sudo apt-cache policy cmake nasm || true
    exit 1
  fi
  sudo apt-get -o Acquire::Retries=3 install --yes --snapshot "$snapshot" \
    "${packages[@]}"
  apt_source="ubuntu-apt-snapshot:$snapshot"
else
  if ! snapshot_failure_is_http_5xx; then
    echo "Ubuntu snapshot update failed without a confirmed snapshot-service HTTP 5xx; refusing fallback" >&2
    exit 1
  fi

  echo "Snapshot service returned HTTP 5xx; refreshing the runner's signed Ubuntu Noble sources"
  sudo apt-get \
    -o Acquire::Retries=3 \
    -o APT::Update::Error-Mode=any \
    update
  if ! sudo apt-get --simulate --yes install "${packages[@]}"; then
    echo "Pinned CMake and NASM are unavailable from the runner's signed Ubuntu sources" >&2
    sudo apt-cache policy cmake nasm || true
    exit 1
  fi
  sudo apt-get -o Acquire::Retries=3 install --yes "${packages[@]}"
  apt_source="ubuntu-noble-live-fallback"
fi

printf 'BENCHMARK_APT_SOURCE=%s\n' "$apt_source" >> "$github_env"
echo "Installed pinned benchmark packages from $apt_source"
