## Summary

<!-- Explain the user-visible change and the Pillow behavior it matches. -->

## Provenance

<!-- Identify copied/translated sources and licenses, or write "original". -->

## Verification

- [ ] `make fmt`
- [ ] `make lint`
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps --locked`
- [ ] `cargo test --locked --all-features --test coverage_matrix_tests`
- [ ] `scripts/test_feature_matrix.sh`
- [ ] `make coverage` passes the current alpha floors; coverage gaps are reviewed toward the 100% goal
- [ ] Exact encoded bytes and decoded pixels were checked where applicable
