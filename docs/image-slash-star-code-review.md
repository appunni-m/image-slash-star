# `image-slash-star` Code Review

Date: 2026-07-23

## Decision

The migration places the correct responsibilities in this crate:
`image-slash-star` owns signature detection, header inspection, structured
codec errors, format-specific decode/encode dispatch, and immutable encoded
source caching. The manifest-driven exact-byte tests and the use of
`Arc`/`OnceLock` for shared deterministic results are strong foundations.

No unsafe Rust or new FFI boundary was introduced in the reviewed source-cache
slice. The accepted contract, target-capability, feature-test, path-boundary,
and maintenance-gate findings are now implemented. It did not require a new
codec or an expansion of the approved migration.

Downstream lazy pipeline, palette, compatibility, and repository findings are
owned by `docs/image-backend-migration-code-review.md` in the `pillow-rs`
repository.

## Scope reviewed

- `src/source.rs` and the public `EncodedImage` lifecycle;
- format-name and path parsing in `src/types/mod.rs`;
- codec auto-detection, inspection, decode, and sequence boundaries;
- codec feature declarations and reduced-feature tests;
- migration, completion, and lazy-loading documentation;
- the current repository lint and Coverage MCP evidence.

This review does not reopen codec parity already proven by the coverage
manifest, add Pillow operations, or redesign the downstream compute pipeline.

## Validation evidence

- The complete migration diff and relevant call paths were inspected.
- `git diff --check` passed.
- Coverage MCP run `5b0f1ca0-0ecf-433b-a159-722387249757`, snapshot
  `2a9e4148-d559-44db-8368-57df58bf21fc`, passed all six test targets and
  reports 100% coverage: 30,616 lines, 3,924 branches, 1,837 functions, and
  51,578 regions.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D
  warnings` passes. The library-root and integration-test blanket allowances
  have been removed. The inherited `webp/native` blanket allowance is also
  removed after a file-by-file arithmetic and conversion audit.

## Findings

### I1 — “Fully validates” is stronger than the implemented codec contract

Severity: medium-high.

`src/source.rs:87-96` implements `EncodedImage::verify()` by calling the
single-image `decode()` API. This independently validates the image selected by
that API and correctly avoids populating the ordinary decode cache, but it does
not universally validate every image in the container:

- AVIF `decode()` explicitly decodes frame zero;
- WebP has a separate `decode_sequence()` path for all animation frames;
- TIFF `decode()` documents that it decodes the first IFD.

A later corrupt frame or image directory can therefore be outside the work
performed by `verify()`. The public rustdoc and migration documents currently
call this full snapshot validation.

Accepted implementation:

The accepted implementation is recorded in
`docs/lazy-loading-correctness-proposal.md`: match the pinned Pillow oracle's
codec-specific `Image.verify()` behavior rather than imposing one generic
primary-image or full-container definition. Dispatch verification by detected
format, use primary decode, sequence decode, or structural validation only when
oracle fixtures prove it, and retain independent execution so verification
never populates the ordinary lazy decode cache.

### I2 — Feature and target capability combinations are not proved

Severity: medium correctness and test gap.

`tests/feature_gate_tests.rs` correctly proves that disabled formats return the
exact `FeatureDisabled` error. The usual all-feature lane proves that all
formats work together. Those endpoints do not prove that each individual
feature enables exactly its intended codec. The same availability contract is
incomplete across targets: the `avif` feature can be enabled on `wasm32`, but
its native implementation is unavailable there, causing valid AVIF input to be
reported as malformed instead of target-unsupported.

Accepted implementation:

The combined feature and target capability matrix is recorded in
`docs/lazy-loading-correctness-proposal.md`. Run the same manifest-driven test
with no features, every individual codec feature, the default set, and all
features. Enabled rows must prove successful detection, inspection, source
construction, and decode behavior; disabled rows must prove exact structured
feature errors. ICO must prove its intentional PNG/BMP transitive features, and
AVIF must prove native success, disabled-feature errors, and one consistent
`Unsupported` capability error across every core-WASM entry point. Valid AVIF
must never be labeled malformed solely because the target lacks libavif.

Implemented (July 2026): the shared dispatcher applies the target capability
check to inspect, verify, still/sequence decode, and still/sequence encode.
`scripts/test_feature_matrix.sh` executes the accepted native matrix and
compiles both core-WASM lanes from the same fixture-driven integration test.

### I3 — Strict Clippy did not pass

Severity: high acceptance-gate defect.

The documented all-target/all-feature invocation currently fails on two denied
`map_unwrap_or` diagnostics in the integration test. Running the accepted
strict form with `-D warnings` also promotes the existing arithmetic, cast, and
style warnings into thousands of errors. A release cannot claim strict lint
quality while that command is red.

Accepted implementation:

The strict implementation contract is recorded in
`docs/lazy-loading-correctness-proposal.md`. Fix every diagnostic across
production code, integration tests, and coverage hooks; do not accept a warning
baseline as completion. Arithmetic and conversion fixes must use checked,
explicitly wrapping, or proven-bounded behavior according to codec semantics,
and any observable error/output change requires a Pillow-oracle manifest row.
The authoritative command must pass with `-D warnings`.

Progress (July 2026):

- the authoritative command passes with warnings denied;
- the coverage-matrix and feature-matrix integration crates no longer suppress
  `unwrap`, `expect`, or unused-dependency diagnostics;
- the library root no longer suppresses those lints across every codec;
- explicit wrapping, saturation, checked conversion, and invariant-scoped
  assertions replaced the exposed diagnostics without changing oracle bytes;
- `byteorder_lite.rs`, `alpha_blending.rs`, `transform.rs`, `yuv.rs`, and
  `encoder/predictor.rs`, plus `huffman.rs` and `extended.rs`, now override the
  inherited WebP allowance with strict child-module policy; `loop_filter.rs`
  and `encoder/cross_color.rs` also replace the blanket policy with documented
  RFC/reference-kernel arithmetic exceptions. `vp8_arithmetic_decoder.rs`
  similarly confines bit-coder arithmetic and one padded-chunk invariant, while
  `encoder/histogram.rs` confines fixed-point entropy and deterministic
  clustering arithmetic. `lossless_transform.rs` now confines inverse VP8L
  arithmetic and fixed-width slice conversion invariants, and
  `encoder/backward_refs.rs` confines its hash-chain, fixed-point, and LZ77
  interval arithmetic. `decoder.rs` confines RIFF/geometry arithmetic and
  documents the three constructor-proven chunk/canvas invariants that retain
  infallible access. `encoder.rs`, `lossless.rs`, and `vp8.rs` complete the
  orchestrator and decoder audits with only reference arithmetic and exact
  collection/block invariants scoped locally. Other exceptions are confined to
  packed fields and validated geometry traversals;
- the inherited blanket allowance in `src/codecs/webp/native/mod.rs` has been
  removed. Every child module now owns its explicit strict policy, and the
  authoritative command plus exact WebP parity and Coverage MCP gates pass.

### I4 — `ImageFormat::from_path()` performs an avoidable allocation

Severity: optional.

`ImageFormat::from_path()` lowercases an extension into a new `String`, while
`ImageFormat::from_name()` already compares names case-insensitively.

Implemented (July 2026): successful parsing now passes the borrowed extension
directly to the existing case-insensitive parser. Allocation occurs only while
constructing the normalized structured error for an unknown extension; exact
uppercase-success, uppercase-error, and missing-extension behavior is covered.

Security boundary: this API only inspects the final extension. It performs no
filesystem access, path resolution, canonicalization, or root-containment
check, so it is not itself a path-traversal surface. Downstream code that opens
an untrusted path must enforce allowed-root and symlink policy at the actual I/O
boundary; successful format inference must not be treated as path validation.
The accepted boundary and its nonexistent traversal-looking path regression
case are recorded in `docs/lazy-loading-correctness-proposal.md`.

## Positive findings

- Magic-byte auto-detection, inspection, decode/encode dispatch, and codec
  errors are centralized in this crate rather than duplicated downstream.
- `EncodedImage` snapshots immutable bytes and shares inspected metadata and
  decode state across clones with thread-safe standard-library primitives.
- Successful and deterministic failed decodes use one initialization path.
- `ImageFormat`, exact `ImageMode`, `ColorType`, pixels, palette, and alpha are
  retained in the canonical decoded envelope.
- Structured `Result` APIs are used without duplicate `try_*` entry points.
- Codec features are explicit, ICO declares its transitive requirements, and
  native AVIF remains outside the default feature set.
- Manifest tests compare exact values and bytes rather than byte lengths.
- The source-cache slice has exact 100% line, branch, and region coverage.

## Resolution order

1. Implement and prove the accepted codec-specific `verify()` contract in I1.
2. Implement the combined codec feature and target matrix from I2.
3. Align migration and rustdoc claims with those decisions.
4. Complete the strict Clippy implementation from I3 without suppressing
   diagnostics or changing parity.
5. Apply I4 only alongside another relevant format-parsing change.
6. Rerun formatting, exact manifest tests, isolated feature/target lanes, and Coverage
   MCP line/branch/region coverage before declaring the upstream slice complete.

## Review completion rule

Each finding is complete when it is fixed with the named evidence or explicitly
accepted as a documented contract or release tradeoff. This review changes no
codec algorithm, public feature set, fixture output, or dependency.
