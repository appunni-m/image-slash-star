# Release checklist

This project is pre-release. The complete go/no-go checklist is in
[PRODUCTION_RELEASE_READINESS.md](PRODUCTION_RELEASE_READINESS.md). A release is
allowed only when the version,
source revision, generated fixtures, legal notices, and published claims all
describe the same tree.

## Before tagging

1. Read the open inventory and dependency order in
   [roadmap.json](roadmap.json); use [docs/roadmap-new.md](docs/roadmap-new.md)
   as its human rendering. Do not call a planned or evidence-pending slice
   complete.
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
- Do not publish stale native AVIF binaries or build instructions. The
  dependency-free Rust/WASM artifact contains the only AVIF runtime path;
  pinned native projects are documented as oracle/provenance material only.
- Keep the release artifacts, benchmark receipt, and evidence identifiers
  recoverable from the release notes.

## Generated AV1 oracle files

The checked-in `tests/fixtures/outputs/av1_reconstruction.json` file is a small
index. Its five `av1_reconstruction.part-*.json` sidecars contain the current
case records consumed by the reconstruction test; they are release test inputs
and must remain available to clean source checkouts. The sidecars are generated
deterministically by the maintained
`scripts/generate_av1_reconstruction_refs.py` script from the pinned dav1d
`b546257f770768b2c88258c533da38b91a06f737` source, using the exact AVIF inputs
and Pillow oracle described in `tests/fixtures/input/images/avif/README.md`.
Regeneration produces the index and sidecars together and the test harness
joins them before validation. It is appropriate to regenerate and compare
these files during fixture maintenance; deleting them from the current tree
would remove the reproducible oracle needed by CI. Older monolithic revisions
remain only in Git history and are the source of the hosting-size blocker.

## First local crates.io bootstrap

After the clean release gate passes, authenticate interactively and publish the
single crate without creating a tag:

```text
cargo login
RELEASE_APPROVED=1 RELEASE_CI_SHA="$(git rev-parse HEAD)" make release-bootstrap
cargo logout
```

The helper builds twice, checks the extracted package and downstream consumer,
publishes only when the exact version is absent, and compares the resulting
crates.io archive byte-for-byte. It never creates or pushes a Git tag.

## Later tag releases

After the first bootstrap, create an annotated `v<version>` tag on each reviewed
commit and push that tag. The tag-driven
[`.github/workflows/release.yml`](.github/workflows/release.yml) repeats the
clean release gate, uses crates.io OIDC Trusted Publishing, verifies the
registry archive, attests the artifact, and creates the GitHub prerelease. No
long-lived registry token is stored in the workflow.

## If a release is wrong

Pause further publication, mark the affected version clearly, and open a
private security report when the issue could affect confidentiality,
integrity, or availability. Otherwise publish a corrective changelog entry,
identify the first bad revision, and rerun the complete acceptance set before
retagging. Never rewrite a published tag to hide a failed result.
