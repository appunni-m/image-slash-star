# Production release readiness

This checklist is the release boundary for `image-slash-star`. The first
registry package is version `0.1.0`; it remains a bounded codec pre-release
until the canonical roadmap and evidence say otherwise.

The current clean-history release candidate is the pushed branch
`codex/release-image-slash-star-clean-current` at commit
`96fa1bccbcd8c87e40d99d7ade895b72d8607864`. Its package archive was rebuilt
and verified locally with SHA-256
`2e0be81e5453c4d22244ac665cb90ffefe861e73c9b24118a96c920f69b28138`.

## Release identity

- [x] Cargo metadata, lockfile, README, changelog, legal files, and package
      surface name the same `image-slash-star` package.
- [x] `Cargo.toml` has an explicit include list and excludes fixtures, coverage
      state, build output, and release-only tooling.
- [x] `make verify`, `make lint`, and the nightly coverage-harness compile pass
      on the current local release candidate.
      The complete documented `make test` lane also passes on the current
      checkout: all doctests, 40 coverage-matrix/contract tests, workspace
      tests, and every native/wasm feature-matrix lane are green.
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
- [x] The pushed clean-history release branch contains no generated fixture
      blob at or above GitHub's 100 MB limit. Its current tree has the required
      424-byte AV1 reconstruction index plus five deterministic sidecars
      (7.7–29.1 MB each), all below the limit. The default `main` branch still
      retains three historical `av1_reconstruction.json` blobs above the hard
      limit (122,191,374; 113,296,026; and 105,260,021 bytes), with two more
      historical versions of 104,403,261 and 104,072,984 bytes. Those objects
      are why a direct `main` push remains rejected; the release candidate uses
      the clean branch and leaves `main` unchanged.
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
