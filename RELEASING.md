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
3. Run the maintained checks on the pinned toolchain:

   ```bash
   make ci
   ```

   The alpha release coverage floors, approved on 2026-09-15, are 59% lines,
   46% branches, 52% functions, and 58% regions. `make coverage` measures the
   entire existing all-feature suite and enforces each floor from raw counts.
   It rejects malformed or empty reports. `make coverage-complete` retains
   the 100% completeness goal. Every executed test must still pass.

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

The owner-authorized first upload completed on 2026-09-13 from immutable tag
`v0.1.0` at commit `35dd72808e6b2a8488b98caf685a3d48e4c97468`. The exact
crates.io archive is visible as `image-slash-star@0.1.0` with checksum
`f35022079076b686716e61a8640b3e4bafb0004701486277cb95f004b769a178`.
The current clean-history branch was promoted to remote `main` with an exact
`--force-with-lease` after creating and verifying a local-only Git bundle at
`/private/tmp/image-slash-star-pre-force-backup-20260913/repository.bundle`.

The local bootstrap is complete. Subsequent versions publish exclusively
through GitHub OIDC; `make release-bootstrap` now explains that boundary and
refuses a local upload.

## Later tag releases

Configure the existing crate's Trusted Publisher with repository
`appunni-m/image-slash-star`, workflow filename `release.yml`, and environment
`crates-io`. The GitHub publish job has `id-token: write`; no long-lived registry
secret or local Cargo login is needed. A configured environment reviewer rule
will pause the job until that review completes.

Increment the version in Cargo metadata, both lockfiles, README, and the dated
changelog. The next candidate is `0.1.1`. Commit and push to `main`, then wait
for successful CI on that exact revision before pushing an unused annotated
`v<version>` tag. The workflow checks the original annotated tag object through
a separately fetched ref, so a peeled Actions checkout cannot invalidate it.

CI runs quality, full native/WASM feature tests, aggregate coverage, dependency
audits, and reproducible archive/consumer checks. Release preflight consumes
the candidate from that exact successful main run. The publish job rebuilds
and compiles it before authentication, verifies byte identity, and only then
requests the short-lived OIDC token. After upload, the downloaded registry
archive must match the candidate's checksum. The final job attests that archive
and creates a GitHub prerelease with notes and `SHA256SUMS`.

This follows the verification/artifact/OIDC separation used by
[coverage-mcp](https://github.com/appunni-m/coverage-mcp/blob/v0.16.0/.github/workflows/release.yml).
A skipped publishing job says nothing about publisher configuration: inspect
its failed prerequisite first. Verify the registry checksum and GitHub assets
after the workflow succeeds. No npm or PyPI package is defined for this repo.

## If a release is wrong

Pause further publication, mark the affected version clearly, and open a
private security report when the issue could affect confidentiality,
integrity, or availability. Otherwise publish a corrective changelog entry,
identify the first bad revision, and rerun the complete acceptance set before
retagging. Never rewrite a published tag to hide a failed result.
