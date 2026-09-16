# Releasing image-slash-star

<!-- release:summary -->
**Latest release: [0.1.3](https://github.com/appunni-m/image-slash-star/releases/tag/v0.1.3).**
<!-- /release:summary -->

There is one Cargo crate and no npm or PyPI distribution. Releases use this
repository's `release.yml` workflow and GitHub OIDC.

## Prepare and tag

1. Update the package version, changelog, documentation source-version field, and both
   workspace lockfiles through `make release-lock-update`.
2. Read [maturity](docs/MATURITY.md) and the [public roadmap](docs/roadmap-new.md).
   Keep planned or unmeasured behavior explicit.
3. Run `make ci`, including strict lint, contract checks, feature/target tests,
   source coverage, package extraction, rustdoc, and isolated consumers.
4. Inspect the distributable archive and its licenses. Use
   `make release-verify` from a clean checkout for release acceptance.
5. Commit and push to main. Require successful CI for that exact commit.
6. Push a new annotated `v<version>` tag on the validated commit.

The alpha source-coverage floors are 59% lines, 46% branches, 52% functions,
and 58% regions. Every executed test must pass. Full completion still requires
`make coverage-complete`; neither a package release nor a floor pass closes
the coverage goal.

## Publish and verify

The crates.io trusted publisher identifies repository
`appunni-m/image-slash-star`, workflow filename `release.yml`, and environment
`crates-io`. Only the publishing job receives OIDC permission. No local login
or long-lived registry token is used by subsequent releases.

The workflow builds and validates before publishing. It checks registry
artifact identity and creates GitHub assets only after successful publication.
Use `make release-registry-verify` for a read-only candidate/registry comparison.
Binary downloads must be hashed as archive bytes, not JSON redirect metadata.

[Release 0.1.3](https://github.com/appunni-m/image-slash-star/actions/runs/35098565255)
completed this flow. Its crate SHA-256 is
`3e7755f83b15f0fc3f763276dc172ac21c6da153e9f5dc3abebbd456d91ac8b0`.
See [Evidence](docs/EVIDENCE.md) for accepted source and test boundaries.

## Recovery and immutable artifacts

Retry only the same source and unchanged artifacts after a transient failure.
An uploaded registry version or released tag must never be overwritten.
Changed source, packaging, or artifact bytes require a new version and tag.
Verify which job actually failed before changing trusted-publisher settings.

The completed local bootstrap target has been removed. The maintainer controls
releases through repository review and tag permissions; no maintainer-succession
guarantee or support SLA is implied.

## Required source assets

AV1's reconstruction index and its five sidecars form a reproducible oracle
set consumed by clean test checkouts. They can be regenerated from the exact
licensed inputs and pinned dav1d/Pillow tools described in
[fixture provenance](tests/fixtures/input/images/avif/README.md). Do not delete
required oracle outputs while retaining tests that consume them.

The crate keeps public guides, generated capability tables, package examples,
licenses, notices, and source needed by consumers. Internal release diaries and
local benchmark output stay out of the package.

## Documentation publication

Main documentation CI publishes GitHub Pages from this same repository,
independently of registry versions. See [documentation maintenance](docs/DOCUMENTATION.md).

## Refresh user documentation after publication

Wait for every registry job and the GitHub release to succeed, then run:

```sh
make docs-release-refresh
make docs-release-check docs-registry-examples
```

Review and commit the refreshed release record and installation blocks. The
Documentation workflow checks freshness after a successful tag release and
on its daily schedule. Keep source-candidate versions separate from published
installation versions; do not send users to a version that is still building.
