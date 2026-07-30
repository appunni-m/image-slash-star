## Summary

<!-- Explain the user-visible change and the Pillow behavior it matches. -->

## Provenance

<!-- Identify copied/translated sources and licenses, or write "original". -->

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps --locked`
- [ ] `cargo test --locked --all-features --test coverage_matrix_tests`
- [ ] `scripts/test_feature_matrix.sh`
- [ ] Coverage MCP retains 100% line, branch, function, and region coverage
- [ ] Exact encoded bytes and decoded pixels were checked where applicable
