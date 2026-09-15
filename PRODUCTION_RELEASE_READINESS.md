# Production release readiness

This checklist is the release boundary for `image-slash-star`. The first
registry package is version `0.1.0`; it remains a bounded codec pre-release
until the canonical roadmap and evidence say otherwise.

The current candidate is `0.1.1`. The archive and checksum are authoritative
only when produced by `make package-verify` from the exact clean commit.
The first upload is complete; all future publication uses GitHub OIDC.

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
- [x] Alpha coverage policy is explicit: lines 59%, branches 46%, functions
      52%, and regions 58%, approved on 2026-09-15. The 2026-09-14 full report
      measures 59.2149%, 46.2216%, 52.9208%, and 58.2759% respectively.
      The denominator is unchanged. Empty, invalid, or below-floor reports
      fail. `make coverage-complete` retains the separate 100% target.
- [ ] Collect a passing full coverage report on the exact 0.1.1 commit in CI.
- [x] The exact 208-file Cargo package list is recorded in
      `tests/fixtures/package_surface_manifest.json`; the package-surface
      check and isolated archive consumer pass. A clean worktree also passes
      the complete `make package-verify` target with the pinned Rust 1.96.1
      toolchain.
- [x] The pushed clean-history release branch and remote `main` contain no
      generated fixture blob at or above GitHub's 100 MB limit. Their current
      tree has the required 424-byte AV1 reconstruction index plus five
      deterministic sidecars (7.7–29.1 MB each), all below the limit. The
      pre-promotion remote history, including its oversized historical blobs,
      is preserved in the local-only bundle
      `/private/tmp/image-slash-star-pre-force-backup-20260913/repository.bundle`.
- [ ] The exact clean commit has a successful pinned CI run.
- [ ] Managed Pillow parity and Coverage MCP receipts identify that commit.
- [ ] The four release coverage metrics and every planned codec class remain
      visible in the release notes.

## Completed bootstrap and trusted publisher

- [ ] Run `make release-verify` from a clean checkout.
- [x] Run the owner-authorized first Cargo bootstrap from immutable tag
      `v0.1.0` (`35dd72808e6b2a8488b98caf685a3d48e4c97468`). The published
      crates.io checksum is
      `f35022079076b686716e61a8640b3e4bafb0004701486277cb95f004b769a178`.
- [x] Verify `cargo info image-slash-star@0.1.0` and the downloaded registry
      archive after the bootstrap.
- [x] The owner reports configuring the crates.io Trusted Publisher for this
      repository, workflow filename `release.yml`, and environment `crates-io`.
      Successful OIDC publication remains the acceptance evidence.

## Later tag releases

- [ ] Create and push one annotated `v<version>` tag on the reviewed commit.
- [ ] The tag workflow verifies CI, rebuilds the package, authenticates with
      crates.io OIDC, compares the registry archive, attests the artifact, and
      creates the GitHub prerelease.
- [ ] Verify the published checksum and OIDC identity after the workflow runs.
- [ ] Never reuse or move a published tag; fix forward with a new version.

## Scope boundary

The package is codec-only. AVIF classes, hostile-input assurance, full WASM
semantic coverage, fuzzing, and other open roadmap items remain explicitly
tracked as planned or evidence-pending. Publication does not change those
claims.
