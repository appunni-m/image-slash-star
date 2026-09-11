# Production release readiness

This checklist is the release boundary for `image-slash-star`. The first
registry package is version `0.1.0`; it remains a bounded codec pre-release
until the canonical roadmap and evidence say otherwise.

## Release identity

- [x] Cargo metadata, lockfile, README, changelog, legal files, and package
      surface name the same `image-slash-star` package.
- [x] `Cargo.toml` has an explicit include list and excludes fixtures, coverage
      state, build output, and release-only tooling.
- [x] `make verify`, `make lint`, and the nightly coverage-harness compile pass
      on the current local release candidate.
      The complete documented `make test` lane also passes on the current
      checkout: all doctests, 42 coverage-matrix/contract tests, workspace
      tests, and every native/wasm feature-matrix lane are green.
      The exact parity and aggregate coverage jobs use GitHub's `macos-14`
      ARM64 runner to match the pinned `aarch64-apple-darwin` fixture lane;
      x86 determinism remains tracked as QA-019.
- [ ] The full pinned nightly LLVM coverage run is green from a clean checkout.
      The maintained AV1 reconstruction index and five deterministic sidecars
      are now present, and all 45/45 coverage-matrix tests execute successfully.
      The strict verifier still reports 95,603/161,451 lines (59.2149%),
      14,912/32,262 branches (46.2216%), 4,892/9,244 functions (52.9208%), and
      140,738/241,503 regions (58.2759%), so this gate remains open until the
      complete metric totals meet the repository threshold.
- [x] The exact 208-file Cargo package list is recorded in
      `tests/fixtures/package_surface_manifest.json`; the package-surface
      check and isolated archive consumer pass. A clean worktree also passes
      the complete `make package-verify` target with the pinned Rust 1.96.1
      toolchain.
- [ ] The remote `main` history contains no generated fixture blob at or above
      GitHub's 100 MB limit. The verified candidate branch
      `codex/release-image-slash-star-clean` has the required 424-byte AV1
      reconstruction index plus five deterministic sidecars (7.7–29.1 MB each),
      no blob at or above 100 MB, and a passing `make package-verify`. GitHub
      rejected the old `main` push because earlier history retains three
      generated `av1_reconstruction.json` blobs above the hard limit
      (122,191,374; 113,296,026; and 105,260,021 bytes); two additional
      historical versions of 104,403,261 and 104,072,984 bytes also trigger
      large-file warnings. These are history-only artifacts; the current
      fixture set remains required and regeneratable from the pinned dav1d
      source. Updating `main` requires an explicit coordinated history rewrite.
- [ ] The exact clean commit has a successful pinned CI run.
- [ ] Managed Pillow parity and Coverage MCP receipts identify that commit.
- [ ] The four release coverage metrics and every planned codec class remain
      visible in the release notes.

## Local first publish

- [ ] Run `make release-verify` from a clean checkout.
- [ ] Run `cargo login` interactively on the reviewed machine.
- [ ] Run `RELEASE_APPROVED=1 RELEASE_CI_SHA="$(git rev-parse HEAD)" make release-bootstrap`.
- [ ] Run `cargo logout` after the bootstrap and compare the public crates.io
      archive with `target/release-artifacts/registry/`.
- [ ] Configure the crates.io Trusted Publisher for this repository,
      `.github/workflows/release.yml`, and environment `crates-io`.

## Later tag releases

- [ ] Create and push one annotated `v<version>` tag on the reviewed commit.
- [ ] The tag workflow verifies CI, rebuilds the package, authenticates with
      crates.io OIDC, compares the registry archive, attests the artifact, and
      creates the GitHub prerelease.
- [ ] GitHub environment protection and release rules require maintainer
      review.
- [ ] Never reuse or move a published tag; fix forward with a new version.

## Scope boundary

The package is codec-only. AVIF classes, hostile-input assurance, full WASM
semantic coverage, fuzzing, and other open roadmap items remain explicitly
tracked as planned or evidence-pending. Publication does not change those
claims.
