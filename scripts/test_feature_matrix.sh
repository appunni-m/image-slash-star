#!/bin/sh
set -eu

cargo clippy --workspace --all-targets --locked --no-default-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --no-default-features
cargo test --locked --test feature_gate_tests --no-default-features

for feature in jpeg png gif bmp tiff webp ico avif; do
    cargo clippy --workspace --all-targets --locked --no-default-features \
        --features "$feature" -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --no-default-features \
        --features "$feature"
    cargo test --locked --test feature_gate_tests --no-default-features --features "$feature"
done

cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo test --locked --test feature_gate_tests
cargo clippy --workspace --all-targets --locked --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --all-features
cargo test --locked --test feature_gate_tests --all-features

for features in none jpeg png gif bmp tiff webp ico avif default all; do
    case "$features" in
        none)
            cargo clippy --workspace --all-targets --locked \
                --target wasm32-unknown-unknown --no-default-features -- -D warnings
            RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked \
                --target wasm32-unknown-unknown --no-default-features
            ;;
        jpeg|png|gif|bmp|tiff|webp|ico|avif)
            cargo clippy --workspace --all-targets --locked \
                --target wasm32-unknown-unknown --no-default-features \
                --features "$features" -- -D warnings
            RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked \
                --target wasm32-unknown-unknown --no-default-features \
                --features "$features"
            ;;
        default)
            cargo clippy --workspace --all-targets --locked \
                --target wasm32-unknown-unknown -- -D warnings
            RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked \
                --target wasm32-unknown-unknown
            ;;
        all)
            cargo clippy --workspace --all-targets --locked \
                --target wasm32-unknown-unknown --all-features -- -D warnings
            RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked \
                --target wasm32-unknown-unknown --all-features
            ;;
    esac
done

cargo test --locked --target wasm32-unknown-unknown --test feature_gate_tests \
    --no-default-features --no-run
cargo test --locked --target wasm32-unknown-unknown --test feature_gate_tests \
    --no-default-features --features avif --no-run

# Execute the feature-gate contract in a real WASM runtime: every native lane
# is built for wasm32-wasip1 and run under Node's WASI preview1 runtime.
if command -v node >/dev/null 2>&1; then
    for features in none jpeg png gif bmp tiff webp ico avif default all; do
        case "$features" in
            none)
                cargo test --locked --target wasm32-wasip1 --test feature_gate_tests \
                    --no-default-features --no-run
                ;;
            jpeg|png|gif|bmp|tiff|webp|ico|avif)
                cargo test --locked --target wasm32-wasip1 --test feature_gate_tests \
                    --no-default-features --features "$features" --no-run
                ;;
            default)
                cargo test --locked --target wasm32-wasip1 --test feature_gate_tests \
                    --no-run
                ;;
            all)
                cargo test --locked --target wasm32-wasip1 --test feature_gate_tests \
                    --all-features --no-run
                ;;
        esac
        binary=$(ls -t target/wasm32-wasip1/debug/deps/feature_gate_tests-*.wasm | head -1)
        node scripts/wasm_test_runner.js "$binary"
    done
else
    echo "node is required for the wasm32-wasip1 runtime lanes" >&2
    exit 1
fi

# Regenerate the capability tables in memory and reject any drift between the
# committed fixture and the native or WASI runtime tables.
python3 scripts/generate_capability_tables.py --check
