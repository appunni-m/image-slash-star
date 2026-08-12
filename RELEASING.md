# Release checklist

This project is pre-release. A release is allowed only when the version,
source revision, generated fixtures, legal notices, and published claims all
describe the same tree.

## Before tagging

1. Read the open inventory and dependency order in
   [docs/roadmap-new.md](docs/roadmap-new.md). Do not call a planned or
   evidence-pending slice complete.
2. Update [CHANGELOG.md](CHANGELOG.md) and the README for user-visible
   behavior, supported targets, and known limitations.
3. Run the pinned toolchain checks:

   ```text
   cargo fmt --all -- --check
   cargo check --all-features --locked
   cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
   RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps --locked
   cargo test --doc --all-features --locked
   cargo test --all-features --locked --test coverage_matrix_tests -- --nocapture
   scripts/test_feature_matrix.sh
   ```

4. Run every repository verifier listed in the roadmap, including claim
   ledger, coverage origins, diagnostic provenance, package surface, and
   third-party licenses.
5. Run the managed Pillow parity and Coverage MCP workflows at the exact
   source revision. Record run IDs, snapshot ID, all four aggregate coverage
   metrics, and any known target-specific failure without relabeling it.
6. Run the production JPEG comparison when JPEG code or benchmark claims
   changed. Keep the complete same-machine TurboJPEG matrix and its metadata.
7. Build a clean package with `cargo package --locked`, inspect the archive,
   and confirm that the package-surface verifier passes from a clean checkout.

## Tag and publish

- Tag only the reviewed commit after the checks above are complete.
- Publish the generated package and source release together with the matching
  changelog entry and legal notices.
- Do not publish native AVIF binaries as if they were part of the dependency-
  free Rust/WASM artifact; document the separately pinned native stack.
- Keep the release artifacts, benchmark receipt, and evidence identifiers
  recoverable from the release notes.

## If a release is wrong

Pause further publication, mark the affected version clearly, and open a
private security report when the issue could affect confidentiality,
integrity, or availability. Otherwise publish a corrective changelog entry,
identify the first bad revision, and rerun the complete acceptance set before
retagging. Never rewrite a published tag to hide a failed result.
