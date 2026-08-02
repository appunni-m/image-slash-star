#!/bin/sh
set -eu

# Give each bounded lane its own temporary Cargo target directory. Cargo
# serializes writers that share a target directory, so a shared target turns
# independent feature configurations into lock contention. The lane-local
# roots still reuse clippy, rustdoc, and test artifacts within that lane, and
# the capability-table probe is pointed at the same roots below.
# Set MATRIX_JOBS higher on machines with more spare CPU and memory.
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
export CAPABILITY_TARGET_ROOT="$matrix_log_dir"
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

run_parallel_lanes() {
    matrix_group=$1
    matrix_runner=$2
    shift 2
    matrix_pending_jobs=
    matrix_pending_count=0

    # Keep the concurrency bound, but admit work whenever any lane finishes
    # instead of waiting for the slowest lane in a batch. The status marker is
    # written by the child before it exits because POSIX sh has no portable
    # non-blocking wait primitive.
    wait_matrix_lane() {
        while :; do
            for matrix_job in $matrix_pending_jobs; do
                matrix_lane=${matrix_job#*:}
                matrix_status_path="$matrix_log_dir/$matrix_group-$matrix_lane.status"
                if [ -f "$matrix_status_path" ]; then
                    matrix_pid=${matrix_job%%:*}
                    matrix_status=$(sed -n '1p' "$matrix_status_path")
                    wait "$matrix_pid" || :

                    cat "$matrix_log_dir/$matrix_group-$matrix_lane.log"

                    matrix_remaining_jobs=
                    for matrix_remaining_job in $matrix_pending_jobs; do
                        if [ "$matrix_remaining_job" != "$matrix_job" ]; then
                            matrix_remaining_jobs="$matrix_remaining_jobs $matrix_remaining_job"
                        fi
                    done
                    matrix_pending_jobs=$matrix_remaining_jobs
                    matrix_pending_count=$((matrix_pending_count - 1))
                    return "$matrix_status"
                fi
            done
            sleep 0.1
        done
    }

    launch_matrix_lane() {
        matrix_lane=$1
        matrix_status_path="$matrix_log_dir/$matrix_group-$matrix_lane.status"
        matrix_status_tmp="$matrix_status_path.tmp"
        matrix_target_dir="$matrix_log_dir/target-$matrix_group-$matrix_lane"
        (
            export CARGO_TARGET_DIR="$matrix_target_dir"
            matrix_status=0
            "$matrix_runner" "$matrix_lane" \
                >"$matrix_log_dir/$matrix_group-$matrix_lane.log" 2>&1 \
                || matrix_status=$?
            printf '%s\n' "$matrix_status" >"$matrix_status_tmp"
            mv "$matrix_status_tmp" "$matrix_status_path"
            exit "$matrix_status"
        ) &
        matrix_pending_jobs="$matrix_pending_jobs $!:$matrix_lane"
        matrix_pending_count=$((matrix_pending_count + 1))
    }

    matrix_failed=0
    for lane in "$@"; do
        while [ "$matrix_pending_count" -ge "$MATRIX_JOBS" ]; do
            if ! wait_matrix_lane; then
                matrix_failed=1
            fi
        done
        if [ "$matrix_failed" -ne 0 ]; then
            break
        fi
        launch_matrix_lane "$lane"
    done
    if [ "$matrix_pending_count" -gt 0 ]; then
        while [ "$matrix_pending_count" -gt 0 ]; do
            if ! wait_matrix_lane; then
                matrix_failed=1
            fi
        done
    fi
    return "$matrix_failed"
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
