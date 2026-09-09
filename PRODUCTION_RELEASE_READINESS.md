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
      The split AV1 oracle now executes all 45/45 coverage-matrix tests, but
      the strict four-metric verifier remains red at the repository-wide
      100% release target. The remaining uncovered source is tracked in the
      roadmap; it is separate from the AV1 fixture layout.
- [x] The exact 170-file Cargo package list is recorded in
      `tests/fixtures/package_surface_manifest.json`; the package-surface
      check and isolated archive consumer pass. The full `make package-verify`
      target remains clean-source gated until the push-safe release checkout
      is prepared.
- [ ] The release candidate history contains no generated fixture blob at or
      above GitHub's 100 MiB limit; the candidate tree uses a 424-byte index
      and five regenerated AV1 case parts, with the canonical case hash
      unchanged. This remains open only until the candidate is pushed and
      checked remotely.
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
