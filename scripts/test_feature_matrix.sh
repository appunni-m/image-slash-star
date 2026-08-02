#!/bin/sh
set -eu

# Cargo lanes share a target directory, so use bounded process-level
# concurrency instead of allowing every feature configuration to contend at
# once. Set MATRIX_JOBS higher on machines with more spare CPU and memory.
MATRIX_JOBS=${MATRIX_JOBS:-3}
case "$MATRIX_JOBS" in
    ''|*[!0-9]*|0)
        echo "MATRIX_JOBS must be a positive integer" >&2
        exit 2
        ;;
esac
CAPABILITY_JOBS=${CAPABILITY_JOBS:-$MATRIX_JOBS}
case "$CAPABILITY_JOBS" in
    ''|*[!0-9]*|0)
        echo "CAPABILITY_JOBS must be a positive integer" >&2
        exit 2
        ;;
esac
export CAPABILITY_JOBS

matrix_log_dir=$(mktemp -d "${TMPDIR:-/tmp}/image-slash-star-feature-matrix.XXXXXX")
cleanup_matrix_logs() {
    rm -rf "$matrix_log_dir"
}
trap cleanup_matrix_logs EXIT

feature_args() {
    case "$1" in
        none)
            printf '%s\n' --no-default-features
            ;;
        jpeg|png|gif|bmp|tiff|webp|ico|avif)
            printf '%s\n' --no-default-features --features "$1"
            ;;
        default)
            ;;
        all)
            printf '%s\n' --all-features
            ;;
        *)
            echo "unknown feature lane: $1" >&2
            return 2
            ;;
    esac
}

run_native_lane() {
    features=$1
    set -- $(feature_args "$features")
    cargo clippy --workspace --all-targets --locked "$@" -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked "$@"
    cargo test --locked --test feature_gate_tests "$@"
}

run_wasm_unknown_lane() {
    features=$1
    set -- $(feature_args "$features")
    cargo clippy --workspace --all-targets --locked \
        --target wasm32-unknown-unknown "$@" -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked \
        --target wasm32-unknown-unknown "$@"
}

run_wasi_lane() {
    features=$1
    set -- $(feature_args "$features")
    build_log="$matrix_log_dir/wasm-wasi-$features.jsonl"
    build_status=0
    cargo test --locked --target wasm32-wasip1 --test feature_gate_tests \
        --no-run --message-format=json --color never "$@" \
        >"$build_log" 2>&1 || build_status=$?
    cat "$build_log"
    if [ "$build_status" -ne 0 ]; then
        return "$build_status"
    fi
    binary=$(python3 -c '
import json
import sys

for line in open(sys.argv[1], encoding="utf-8"):
    try:
        message = json.loads(line)
    except json.JSONDecodeError:
        continue
    if (
        message.get("reason") == "compiler-artifact"
        and message.get("target", {}).get("name") == "feature_gate_tests"
        and (message.get("executable") or "").endswith(".wasm")
    ):
        print(message["executable"])
        break
else:
    raise SystemExit("cargo did not report a feature_gate_tests WASM executable")
' "$build_log")
    node scripts/wasm_test_runner.js "$binary"
}

wait_matrix_batch() {
    batch_status=0
    for pid in $matrix_pending_pids; do
        if ! wait "$pid"; then
            batch_status=1
        fi
    done
    for lane in $matrix_pending_lanes; do
        cat "$matrix_log_dir/$matrix_group-$lane.log"
    done
    matrix_pending_pids=
    matrix_pending_lanes=
    matrix_pending_count=0
    return "$batch_status"
}

run_parallel_lanes() {
    matrix_group=$1
    matrix_runner=$2
    shift 2
    matrix_pending_pids=
    matrix_pending_lanes=
    matrix_pending_count=0
    for lane in "$@"; do
        "$matrix_runner" "$lane" \
            >"$matrix_log_dir/$matrix_group-$lane.log" 2>&1 &
        matrix_pending_pids="$matrix_pending_pids $!"
        matrix_pending_lanes="$matrix_pending_lanes $lane"
        matrix_pending_count=$((matrix_pending_count + 1))
        if [ "$matrix_pending_count" -ge "$MATRIX_JOBS" ]; then
            wait_matrix_batch || return 1
        fi
    done
    if [ "$matrix_pending_count" -gt 0 ]; then
        wait_matrix_batch || return 1
    fi
}

run_parallel_lanes native run_native_lane \
    none jpeg png gif bmp tiff webp ico avif default all

run_parallel_lanes wasm-unknown run_wasm_unknown_lane \
    none jpeg png gif bmp tiff webp ico avif default all

cargo test --locked --target wasm32-unknown-unknown --test feature_gate_tests \
    --no-default-features --no-run
cargo test --locked --target wasm32-unknown-unknown --test feature_gate_tests \
    --no-default-features --features avif --no-run

# Execute the feature-gate contract in a real WASM runtime: every native lane
# is built for wasm32-wasip1 and run under Node's WASI preview1 runtime.
if command -v node >/dev/null 2>&1; then
    # Keep the real runtime coverage, but do not serialize independent lanes.
    # Each lane extracts its own Cargo-reported executable so concurrent builds
    # cannot accidentally run a neighboring feature configuration.
    run_parallel_lanes wasm-wasi run_wasi_lane \
        none jpeg png gif bmp tiff webp ico avif default all
else
    echo "node is required for the wasm32-wasip1 runtime lanes" >&2
    exit 1
fi

# Cross-target determinism: encoded bytes and decoded pixels executed in the
# WASM runtime must match the golden hashes committed from the native host.
cargo test --locked --target wasm32-wasip1 --test determinism_tests \
    --all-features --no-run
binary=$(ls -t target/wasm32-wasip1/debug/deps/determinism_tests-*.wasm | head -1)
node scripts/wasm_test_runner.js "$binary"

# Regenerate the capability tables in memory and reject any drift between the
# committed fixture and the native or WASI runtime tables.
python3 scripts/generate_capability_tables.py --check
