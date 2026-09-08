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
- [ ] The full pinned nightly LLVM coverage run is green from a clean checkout.
      The corrected coverage-only AV1 state probes now pass, and the current
      run executes 44/45 coverage-matrix tests. The remaining failure is the
      AV1 reconstruction test, which needs the maintained index sidecars.
      Keep this gate open until those inputs are restored in a push-safe
      history and the complete run produces a verified report.
- [x] The exact 170-file Cargo package list is recorded in
      `tests/fixtures/package_surface_manifest.json`; the package-surface
      check and isolated archive consumer pass. The full `make package-verify`
      target remains clean-source gated until the push-safe release checkout
      is prepared.
- [ ] The pushed history contains no generated fixture blob at or above
      GitHub's 100 MiB limit; the current local branch still has five older
      oversized AV1-oracle blobs.
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
