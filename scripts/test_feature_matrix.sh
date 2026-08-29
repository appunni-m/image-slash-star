#!/bin/sh
set -eu

# Give each bounded lane its own temporary Cargo target directory. Cargo
# serializes writers that share a target directory, so a shared target turns
# independent feature configurations into lock contention. The lane-local
# roots still reuse clippy, rustdoc, and test artifacts within that lane, and
# the capability-table probe is pointed at the same roots below.
# Clean roots benefit from roughly two logical CPUs per active lane because
# each feature/target variant is compiling independently. Retained roots have
# already paid that compilation cost, so they can use one worker per lane and
# finish more independent lanes concurrently. Set MATRIX_JOBS explicitly when
# a CI runner has a known capacity.
matrix_cpu_count=$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '%s\n' 1)
case "$matrix_cpu_count" in
    ''|*[!0-9]*|0)
        matrix_cpu_count=1
        ;;
esac
matrix_target_root=${MATRIX_TARGET_ROOT:-${CARGO_TARGET_DIR:-target}/feature-matrix}

# Directory existence alone does not prove that the retained lane artifacts
# match the current source. After a codec or test change, treating those
# directories as warm selects one compiler worker per lane even though every
# lane must rebuild its feature-specific crate and test binary. Fingerprint
# the inputs that can invalidate those artifacts so a changed revision uses
# the compile-oriented cold scheduler, while documentation-only commits keep
# the fast warm path.
matrix_source_signature() {
    matrix_source_files=$(git ls-files --cached --others --exclude-standard -- \
        Cargo.lock Cargo.toml src tests \
        scripts/test_feature_matrix.sh scripts/wasm_test_runner.js 2>/dev/null) || return 1
    if [ -z "$matrix_source_files" ]; then
        return 1
    fi
    # `cksum` accepts many paths in one process. The previous per-file loop
    # spawned one checksum process for every source file, which made the
    # signature itself a measurable part of every warm matrix invocation.
    # `git ls-files` emits paths in stable order; retain the path names in
    # cksum's output so additions, removals, and content changes all alter the
    # marker. A deleted tracked path is intentionally omitted from the checksum
    # input: its disappearance still changes the aggregate path/content list,
    # while avoiding noisy checksum errors during native-to-Rust cutovers.
    git ls-files --cached --others --exclude-standard -- \
        Cargo.lock Cargo.toml src tests \
        scripts/test_feature_matrix.sh scripts/wasm_test_runner.js 2>/dev/null |
        while IFS= read -r path; do
            if [ -f "$path" ]; then
                printf '%s\n' "$path"
            fi
        done |
        xargs -n 200 cksum |
        cksum
}

matrix_source_state=
if matrix_source_state=$(matrix_source_signature); then
    :
else
    # A missing VCS checkout or an unreadable input is safer as cold than as
    # a false warm hit; the explicit MATRIX_* overrides remain available.
    matrix_source_state=
fi
matrix_cache_marker="$matrix_target_root/.matrix-source-signature"
matrix_cache_state=cold
if [ -d "$matrix_target_root/target-native-all" ] \
    && [ -d "$matrix_target_root/target-wasm-unknown-all" ] \
    && [ -d "$matrix_target_root/target-wasm-wasi-all" ] \
    && [ -n "$matrix_source_state" ] \
    && [ -f "$matrix_cache_marker" ] \
    && [ "$(sed -n '1p' "$matrix_cache_marker")" = "$matrix_source_state" ]; then
    matrix_cache_state=warm
fi
MATRIX_JOBS=${MATRIX_JOBS:-}
if [ -z "$MATRIX_JOBS" ]; then
    if [ "$matrix_cache_state" = warm ]; then
        # Warm lanes still run native tests, target checks, and WASI processes;
        # each lane is not merely a cached fingerprint lookup. The lanes use
        # one test/compiler worker each, so admitting up to two independent
        # lanes per logical CPU overlaps the target families without creating
        # nested test/compiler fan-out. The default is capped for large hosts;
        # callers can lower it for a smaller or more heavily shared runner.
        MATRIX_JOBS=$((matrix_cpu_count * 2))
        if [ "$MATRIX_JOBS" -gt 24 ]; then
            MATRIX_JOBS=24
        fi
    else
        MATRIX_JOBS=$(( (matrix_cpu_count + 1) / 2 ))
        if [ "$MATRIX_JOBS" -gt 6 ]; then
            MATRIX_JOBS=6
        fi
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
# heavily oversubscribed matrix. Cold lanes keep the aggregate test-worker
# budget near the host CPU count; warm lanes use one worker per lane while the
# scheduler overlaps independent target/feature processes.
# Callers can override this when a runner has a different process/thread
# balance.
MATRIX_TEST_THREADS=${MATRIX_TEST_THREADS:-}
if [ -z "$MATRIX_TEST_THREADS" ]; then
    if [ "$matrix_cache_state" = warm ]; then
        MATRIX_TEST_THREADS=1
    else
        MATRIX_TEST_THREADS=$((matrix_cpu_count / MATRIX_JOBS))
        if [ "$MATRIX_TEST_THREADS" -lt 1 ]; then
            MATRIX_TEST_THREADS=1
        fi
        if [ "$MATRIX_TEST_THREADS" -gt 8 ]; then
            MATRIX_TEST_THREADS=8
        fi
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
    # Warm roots have already paid the dependency fan-out cost. Keep one
    # compiler worker per active lane: the warm scheduler can admit up to
    # twenty-four lanes, so increasing this to two would oversubscribe the
    # measured 12-logical-CPU host without changing any test coverage.
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
MATRIX_DEBUG=${MATRIX_DEBUG:-0}
case "$MATRIX_DEBUG" in
    0|1|2)
        ;;
    *)
        echo "MATRIX_DEBUG must be 0, 1, or 2" >&2
        exit 2
        ;;
esac
# Matrix lanes do not need debugger symbols: coverage and rustdoc run through
# their dedicated commands, while these lanes only need compile and runtime
# correctness. Removing debug info from the isolated dev/test artifacts cuts
# cold fan-out time and artifact volume without changing a production profile;
# callers can raise it for local debugging with MATRIX_DEBUG=1 or 2.
export CARGO_PROFILE_DEV_DEBUG="$MATRIX_DEBUG"
export CARGO_PROFILE_TEST_DEBUG="$MATRIX_DEBUG"
MATRIX_VERBOSE=${MATRIX_VERBOSE:-0}
case "$MATRIX_VERBOSE" in
    0|1)
        ;;
    *)
        echo "MATRIX_VERBOSE must be 0 or 1" >&2
        exit 2
        ;;
esac
echo "feature matrix: cache=$matrix_cache_state lanes=$MATRIX_JOBS test_threads=$MATRIX_TEST_THREADS build_jobs=$MATRIX_BUILD_JOBS debug=$MATRIX_DEBUG verbose=$MATRIX_VERBOSE"

# The feature-gate suite includes real codec work-budget and cancellation
# contracts, so the repository's level-2 test profile is the runtime baseline
# for the isolated matrix lanes as well. This keeps the codec-heavy native and
# WASI checks representative without changing a production profile; callers
# may still override this matrix-only setting explicitly when compile fan-out
# matters more than repeated runtime.
MATRIX_TEST_OPT_LEVEL=${MATRIX_TEST_OPT_LEVEL:-2}
case "$MATRIX_TEST_OPT_LEVEL" in
    0|1|2|3|s|z)
        ;;
    *)
        echo "MATRIX_TEST_OPT_LEVEL must be one of 0, 1, 2, 3, s, or z" >&2
        exit 2
        ;;
esac
export CARGO_PROFILE_TEST_OPT_LEVEL="$MATRIX_TEST_OPT_LEVEL"

matrix_log_dir=$(mktemp -d "${TMPDIR:-/tmp}/image-slash-star-feature-matrix.XXXXXX")
# Cargo's package-cache lock lives under CARGO_HOME even when the dependency
# graph is already fetched and every lane uses a private target directory.
# Keep the fetched registry and git sources shared, but give each concurrent
# lane its own lock files so read-only offline builds do not queue behind one
# another. The lane homes are retained alongside the lane target roots so
# Cargo fingerprints remain stable across warm matrix invocations.
matrix_cargo_home_source=${CARGO_HOME:-$HOME/.cargo}
matrix_cargo_home_source=$(cd "$matrix_cargo_home_source" && pwd)
prepare_matrix_cargo_home() {
    lane_cargo_home=$1
    mkdir -p "$lane_cargo_home"
    for cargo_shared_directory in registry git; do
        if [ -e "$matrix_cargo_home_source/$cargo_shared_directory" ] \
            && [ ! -e "$lane_cargo_home/$cargo_shared_directory" ] \
            && [ ! -L "$lane_cargo_home/$cargo_shared_directory" ]; then
            ln -s "$matrix_cargo_home_source/$cargo_shared_directory" \
                "$lane_cargo_home/$cargo_shared_directory"
        fi
    done
    for cargo_shared_file in config config.toml credentials.toml; do
        if [ -e "$matrix_cargo_home_source/$cargo_shared_file" ] \
            && [ ! -e "$lane_cargo_home/$cargo_shared_file" ] \
            && [ ! -L "$lane_cargo_home/$cargo_shared_file" ]; then
            ln -s "$matrix_cargo_home_source/$cargo_shared_file" \
                "$lane_cargo_home/$cargo_shared_file"
        fi
    done
}
# Keep build artifacts between invocations by default. Every matrix lane still
# receives an isolated target root, so feature configurations never share
# Cargo's build-directory lock. Set MATRIX_TARGET_ROOT to a temporary path for
# a deliberately cold or disposable run.
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
    # The native test target compiles the complete feature-gated integration
    # suite for every lane. Repository CI separately runs all-feature Clippy
    # and rustdoc, so repeating those checks for every native feature subset
    # only duplicates compilation without adding a distinct runtime result.
    CAPABILITY_TABLE_OUTPUT="$capability_output" cargo test \
        --locked --test feature_gate_tests "$@" -- \
        --test-threads "$MATRIX_TEST_THREADS"
    native_status=$?
    cat "$capability_output"
    return "$native_status"
}

run_wasm_unknown_lane() {
    features=$1
    set -- $(feature_args "$features")
    # Native and WASI lanes already compile and execute the complete
    # feature-gated integration target for every feature selection. The
    # browser-style unknown target is compile/rustdoc-only, so lint its
    # target-specific library surface here instead of rebuilding every
    # integration target a second time in all eleven compile-only lanes.
    cargo clippy --workspace --lib --locked \
        --target wasm32-unknown-unknown "$@" -- -D warnings
    wasm_unknown_status=$?
    if [ "$wasm_unknown_status" -ne 0 ]; then
        return "$wasm_unknown_status"
    fi
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked \
        --target wasm32-unknown-unknown "$@"
    wasm_unknown_status=$?
    if [ "$wasm_unknown_status" -ne 0 ]; then
        return "$wasm_unknown_status"
    fi
    # These are target-contract compile checks, not a second matrix scope.
    # Run them in their matching feature lanes so they overlap with the other
    # target/feature work instead of extending the matrix's serial tail.
    if [ "$features" = none ] || [ "$features" = avif ]; then
        cargo test --locked --target wasm32-unknown-unknown \
            --test feature_gate_tests "$@" --no-run
        wasm_unknown_status=$?
        if [ "$wasm_unknown_status" -ne 0 ]; then
            return "$wasm_unknown_status"
        fi
    fi
    return 0
}

wasm_binary_from_log() {
    python3 -c '
import json
import sys

for line in open(sys.argv[1], encoding="utf-8"):
    try:
        message = json.loads(line)
    except json.JSONDecodeError:
        continue
    if (
        message.get("reason") == "compiler-artifact"
        and message.get("target", {}).get("name") == sys.argv[2]
        and (message.get("executable") or "").endswith(".wasm")
    ):
        print(message["executable"])
        break
else:
    raise SystemExit(f"cargo did not report a {sys.argv[2]} WASM executable")
' "$1" "$2"
}

run_wasi_determinism_lane() {
    determinism_log="$matrix_log_dir/wasm-wasi-determinism.jsonl"
    determinism_status=0
    cargo test --locked --target wasm32-wasip1 --test determinism_tests \
        --all-features --no-run --message-format=json --color never \
        >"$determinism_log" 2>&1 || determinism_status=$?
    cat "$determinism_log"
    if [ "$determinism_status" -ne 0 ]; then
        return "$determinism_status"
    fi
    determinism_binary=$(wasm_binary_from_log "$determinism_log" determinism_tests)
    node scripts/wasm_test_runner.js "$determinism_binary"
    determinism_status=$?
    return "$determinism_status"
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
    binary=$(wasm_binary_from_log "$build_log" feature_gate_tests)
    capability_output="$matrix_log_dir/wasm-wasi-$features.capability"
    CAPABILITY_TABLE_OUTPUT="$capability_output" node scripts/wasm_test_runner.js "$binary" \
        --test-threads "$MATRIX_TEST_THREADS"
    wasi_status=$?
    cat "$capability_output"
    if [ "$wasi_status" -ne 0 ]; then
        return "$wasi_status"
    fi
    if [ "$features" = all ]; then
        run_wasi_determinism_lane
        wasi_status=$?
        if [ "$wasi_status" -ne 0 ]; then
            return "$wasi_status"
        fi
    fi
    return 0
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

                    if [ "$MATRIX_VERBOSE" -eq 1 ] || [ "$matrix_pending_status" -ne 0 ]; then
                        cat "$matrix_log_dir/$matrix_pending_group-$matrix_pending_lane.log"
                    else
                        echo "matrix lane $matrix_pending_group/$matrix_pending_lane passed"
                    fi
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
        matrix_launch_cargo_home="$matrix_target_root/cargo-home-$matrix_launch_group-$matrix_launch_lane"
        prepare_matrix_cargo_home "$matrix_launch_cargo_home"
        (
            export CARGO_TARGET_DIR="$matrix_launch_target_dir"
            export CARGO_HOME="$matrix_launch_cargo_home"
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

# Resolve the locked dependency graph once for every target family before
# concurrent lanes start. Once the cache is complete, keep lane Cargo
# processes offline so they do not contend on the shared package-cache lock;
# each lane still uses its own target directory for independent feature builds.
cargo fetch --locked
cargo fetch --locked --target wasm32-unknown-unknown
cargo fetch --locked --target wasm32-wasip1
export CARGO_NET_OFFLINE=true

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

# Regenerate the capability tables in memory and reject any drift between the
# committed fixture and the native or WASI runtime tables.
python3 scripts/generate_capability_tables.py --check \
    --matrix-log-dir "$matrix_log_dir"

# Keep the packaged capability page synchronized with the committed runtime
# table and active fixture contracts.
python3 scripts/generate_capability_docs.py --check

# Publish the signature only after every lane and the capability-table check
# pass. This marker is local build state, not a repository artifact. Recompute
# it before writing so a source edit made while the matrix was running cannot
# turn the next invocation into a false warm hit.
if [ -n "$matrix_source_state" ]; then
    matrix_current_source_state=$(matrix_source_signature || true)
    if [ "$matrix_current_source_state" = "$matrix_source_state" ]; then
        matrix_cache_marker_tmp="$matrix_cache_marker.tmp"
        printf '%s\n' "$matrix_source_state" >"$matrix_cache_marker_tmp"
        mv "$matrix_cache_marker_tmp" "$matrix_cache_marker"
    fi
fi
