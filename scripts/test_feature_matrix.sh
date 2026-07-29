#!/bin/sh
set -eu

cargo clippy --workspace --all-targets --locked --no-default-features -- -D warnings
cargo test --locked --test feature_gate_tests --no-default-features

for feature in jpeg png gif bmp tiff webp ico avif; do
    cargo clippy --workspace --all-targets --locked --no-default-features \
        --features "$feature" -- -D warnings
    cargo test --locked --test feature_gate_tests --no-default-features --features "$feature"
done

cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --locked --test feature_gate_tests
cargo clippy --workspace --all-targets --locked --all-features -- -D warnings
cargo test --locked --test feature_gate_tests --all-features

for features in none jpeg png gif bmp tiff webp ico avif default all; do
    case "$features" in
        none)
            cargo clippy --workspace --all-targets --locked \
                --target wasm32-unknown-unknown --no-default-features -- -D warnings
            ;;
        jpeg|png|gif|bmp|tiff|webp|ico|avif)
            cargo clippy --workspace --all-targets --locked \
                --target wasm32-unknown-unknown --no-default-features \
                --features "$features" -- -D warnings
            ;;
        default)
            cargo clippy --workspace --all-targets --locked \
                --target wasm32-unknown-unknown -- -D warnings
            ;;
        all)
            cargo clippy --workspace --all-targets --locked \
                --target wasm32-unknown-unknown --all-features -- -D warnings
            ;;
    esac
done

cargo test --locked --target wasm32-unknown-unknown --test feature_gate_tests \
    --no-default-features --no-run
cargo test --locked --target wasm32-unknown-unknown --test feature_gate_tests \
    --no-default-features --features avif --no-run
