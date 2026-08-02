#!/bin/sh
set -eu

# Give each bounded lane its own temporary Cargo target directory. Cargo
# serializes writers that share a target directory, so a shared target turns
# independent feature configurations into lock contention. The lane-local
# roots still reuse clippy, rustdoc, and test artifacts within that lane, and
# the capability-table probe is pointed at the same roots below.
# Use roughly two logical CPUs per active lane by default, capped so a large
# host does not turn the matrix into an unbounded process fan-out. Set
# MATRIX_JOBS explicitly when a CI runner has a known capacity.
matrix_cpu_count=$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '%s\n' 1)
case "$matrix_cpu_count" in
    ''|*[!0-9]*|0)
        matrix_cpu_count=1
        ;;
esac
MATRIX_JOBS=${MATRIX_JOBS:-}
if [ -z "$MATRIX_JOBS" ]; then
    MATRIX_JOBS=$(( (matrix_cpu_count + 1) / 2 ))
    if [ "$MATRIX_JOBS" -gt 6 ]; then
        MATRIX_JOBS=6
    fi
fi
case "$MATRIX_JOBS" in
    ''|*[!0-9]*|0)
        echo "MATRIX_JOBS must be a positive integer" >&2
        exit 2
        ;;
esac
# The Rust test harness otherwise starts one worker per logical CPU in every
# active lane. With several lanes running concurrently that multiplies into a
# heavily oversubscribed matrix. Keep both the default test-worker total and
# the concurrent Cargo compiler-job total close to the host CPU count while
# allowing a caller to tune either budget explicitly.
MATRIX_TEST_THREADS=${MATRIX_TEST_THREADS:-}
if [ -z "$MATRIX_TEST_THREADS" ]; then
    MATRIX_TEST_THREADS=$((matrix_cpu_count / MATRIX_JOBS))
    if [ "$MATRIX_TEST_THREADS" -lt 1 ]; then
        MATRIX_TEST_THREADS=1
    fi
    if [ "$MATRIX_TEST_THREADS" -gt 8 ]; then
        MATRIX_TEST_THREADS=8
    fi
fi
case "$MATRIX_TEST_THREADS" in
    ''|*[!0-9]*|0)
        echo "MATRIX_TEST_THREADS must be a positive integer" >&2
        exit 2
        ;;
esac
MATRIX_BUILD_JOBS=${MATRIX_BUILD_JOBS:-${CARGO_BUILD_JOBS:-}}
if [ -z "$MATRIX_BUILD_JOBS" ]; then
    MATRIX_BUILD_JOBS=$((matrix_cpu_count / MATRIX_JOBS))
    if [ "$MATRIX_BUILD_JOBS" -lt 1 ]; then
        MATRIX_BUILD_JOBS=1
    fi
    if [ "$MATRIX_BUILD_JOBS" -gt 8 ]; then
        MATRIX_BUILD_JOBS=8
    fi
fi
case "$MATRIX_BUILD_JOBS" in
    ''|*[!0-9]*|0)
        echo "MATRIX_BUILD_JOBS must be a positive integer" >&2
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
echo "feature matrix: lanes=$MATRIX_JOBS test_threads=$MATRIX_TEST_THREADS build_jobs=$MATRIX_BUILD_JOBS"

matrix_log_dir=$(mktemp -d "${TMPDIR:-/tmp}/image-slash-star-feature-matrix.XXXXXX")
# Keep build artifacts between invocations by default. Every matrix lane still
# receives an isolated target root, so feature configurations never share
# Cargo's build-directory lock. Set MATRIX_TARGET_ROOT to a temporary path for
# a deliberately cold or disposable run.
matrix_target_root=${MATRIX_TARGET_ROOT:-${CARGO_TARGET_DIR:-target}/feature-matrix}
export CAPABILITY_TARGET_ROOT="$matrix_target_root"
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
    capability_output="$matrix_log_dir/native-$features.capability"
    cargo clippy --workspace --all-targets --locked "$@" -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked "$@"
    CAPABILITY_TABLE_OUTPUT="$capability_output" cargo test \
        --locked --test feature_gate_tests "$@" -- \
        --test-threads "$MATRIX_TEST_THREADS"
    cat "$capability_output"
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
    capability_output="$matrix_log_dir/wasm-wasi-$features.capability"
    CAPABILITY_TABLE_OUTPUT="$capability_output" node scripts/wasm_test_runner.js "$binary" \
        --test-threads "$MATRIX_TEST_THREADS"
    cat "$capability_output"
}

run_matrix_lane() {
    matrix_job=$1
    matrix_group=${matrix_job%%:*}
    matrix_lane=${matrix_job#*:}
    case "$matrix_group" in
        native)
            run_native_lane "$matrix_lane"
            ;;
        wasm-unknown)
            run_wasm_unknown_lane "$matrix_lane"
            ;;
        wasm-wasi)
            run_wasi_lane "$matrix_lane"
            ;;
        *)
            echo "unknown matrix group: $matrix_group" >&2
            return 2
            ;;
    esac
}

run_parallel_jobs() {
    matrix_runner=$1
    shift
    matrix_pending_jobs=
    matrix_pending_count=0

    # Keep the concurrency bound, but admit work whenever any lane finishes
    # instead of waiting for the slowest lane in a batch. The status marker is
    # written by the child before it exits because POSIX sh has no portable
    # non-blocking wait primitive.
    wait_matrix_lane() {
        while :; do
            for matrix_pending_entry in $matrix_pending_jobs; do
                matrix_pending_spec=${matrix_pending_entry#*|}
                matrix_pending_group=${matrix_pending_spec%%:*}
                matrix_pending_lane=${matrix_pending_spec#*:}
                matrix_pending_status_path="$matrix_log_dir/$matrix_pending_group-$matrix_pending_lane.status"
                if [ -f "$matrix_pending_status_path" ]; then
                    matrix_pending_pid=${matrix_pending_entry%%|*}
                    matrix_pending_status=$(sed -n '1p' "$matrix_pending_status_path")
                    wait "$matrix_pending_pid" || :

                    cat "$matrix_log_dir/$matrix_pending_group-$matrix_pending_lane.log"
                    if [ "$matrix_pending_status" -ne 0 ]; then
                        echo "matrix lane $matrix_pending_group/$matrix_pending_lane failed with status $matrix_pending_status" >&2
                    fi

                    matrix_remaining_jobs=
                    for matrix_remaining_entry in $matrix_pending_jobs; do
                        if [ "$matrix_remaining_entry" != "$matrix_pending_entry" ]; then
                            matrix_remaining_jobs="$matrix_remaining_jobs $matrix_remaining_entry"
                        fi
                    done
                    matrix_pending_jobs=$matrix_remaining_jobs
                    matrix_pending_count=$((matrix_pending_count - 1))
                    return "$matrix_pending_status"
                fi
            done
            sleep 0.1
        done
    }

    launch_matrix_lane() {
        matrix_launch_spec=$1
        matrix_launch_group=${matrix_launch_spec%%:*}
        matrix_launch_lane=${matrix_launch_spec#*:}
        matrix_launch_status_path="$matrix_log_dir/$matrix_launch_group-$matrix_launch_lane.status"
        matrix_launch_status_tmp="$matrix_launch_status_path.tmp"
        matrix_launch_target_dir="$matrix_target_root/target-$matrix_launch_group-$matrix_launch_lane"
        (
            export CARGO_TARGET_DIR="$matrix_launch_target_dir"
            export CARGO_BUILD_JOBS="$MATRIX_BUILD_JOBS"
            matrix_status=0
            "$matrix_runner" "$matrix_launch_spec" \
                >"$matrix_log_dir/$matrix_launch_group-$matrix_launch_lane.log" 2>&1 \
                || matrix_status=$?
            printf '%s\n' "$matrix_status" >"$matrix_launch_status_tmp"
            mv "$matrix_launch_status_tmp" "$matrix_launch_status_path"
            exit "$matrix_status"
        ) &
        matrix_pending_jobs="$matrix_pending_jobs $!|$matrix_launch_spec"
        matrix_pending_count=$((matrix_pending_count + 1))
    }

    matrix_failed=0
    for matrix_requested_job in "$@"; do
        while [ "$matrix_pending_count" -ge "$MATRIX_JOBS" ]; do
            if ! wait_matrix_lane; then
                matrix_failed=1
            fi
        done
        if [ "$matrix_failed" -ne 0 ]; then
            break
        fi
        launch_matrix_lane "$matrix_requested_job"
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

if command -v node >/dev/null 2>&1; then
    :
else
    echo "node is required for the wasm32-wasip1 runtime lanes" >&2
    exit 1
fi

# Resolve the locked dependency graph once before concurrent lanes start. This
# keeps Cargo's shared package-cache lock out of the lane fan-out, while each
# lane still uses its own target directory for independent feature builds.
cargo fetch --locked

run_parallel_jobs run_matrix_lane \
    native:none wasm-unknown:none wasm-wasi:none \
    native:jpeg wasm-unknown:jpeg wasm-wasi:jpeg \
    native:png wasm-unknown:png wasm-wasi:png \
    native:gif wasm-unknown:gif wasm-wasi:gif \
    native:bmp wasm-unknown:bmp wasm-wasi:bmp \
    native:tiff wasm-unknown:tiff wasm-wasi:tiff \
    native:webp wasm-unknown:webp wasm-wasi:webp \
    native:ico wasm-unknown:ico wasm-wasi:ico \
    native:avif wasm-unknown:avif wasm-wasi:avif \
    native:default wasm-unknown:default wasm-wasi:default \
    native:all wasm-unknown:all wasm-wasi:all

cargo test --locked --target wasm32-unknown-unknown --test feature_gate_tests \
    --no-default-features --no-run
cargo test --locked --target wasm32-unknown-unknown --test feature_gate_tests \
    --no-default-features --features avif --no-run

# Cross-target determinism: encoded bytes and decoded pixels executed in the
# WASM runtime must match the golden hashes committed from the native host.
cargo test --locked --target wasm32-wasip1 --test determinism_tests \
    --all-features --no-run
binary=$(ls -t target/wasm32-wasip1/debug/deps/determinism_tests-*.wasm | head -1)
node scripts/wasm_test_runner.js "$binary"

# Regenerate the capability tables in memory and reject any drift between the
# committed fixture and the native or WASI runtime tables.
python3 scripts/generate_capability_tables.py --check \
    --matrix-log-dir "$matrix_log_dir"
