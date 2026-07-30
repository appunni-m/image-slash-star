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
- Coverage MCP run `65edd371-6a90-49d0-8ce1-51f4801c234e`, snapshot
  `4ae70741-3fc1-4b45-8154-4d1ed8c2d63b`, passed all seven test targets and
  reports 100% coverage: 38,652 lines, 5,532 branches, 2,044 functions, and
  62,400 regions.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D
  warnings` passes. The library-root and integration-test blanket allowances
  have been removed. The inherited `webp/native` blanket allowance is also
  removed after a file-by-file arithmetic and conversion audit.

## Findings

### I0 — Accepted Pillow error-contract hardening

Severity: high correctness and maintainability work.

The review found that the public root API returned `ImageResult` while
codec-private parsers, inspectors, decoders, encoders, compression routines,
AV1 readers, and native AVIF wrappers still used `Option` for operational
failure. Dispatch consequently collapsed distinct failures into a generic
“codec rejected” message. The accepted implementation now uses the private
`CodecError`/`CodecResult` contract across those fallible boundaries and
reserves `Option` for an explicitly reviewed absence contract.

The pinned Pillow 12.2.0 plugin predicates also expose detection differences:

- JPEG requires `FF D8 FF`, not only `FF D8`;
- GIF requires `GIF87a` or `GIF89a`, not every `GIF8` prefix;
- TIFF accepts its six registered classic/BigTIFF byte-order prefixes; and
- WebP requires `RIFF`, `WEBP`, and a recognized `VP8 `, `VP8L`, or `VP8X`
  first chunk.

`ImagePalette::new` validates RGB triplets, entry count, and alpha length. The
accepted implementation repeats those checks from `DecodedImage::validate`
because public fields permit direct construction. Pillow-tolerated PNG and TIFF
fixtures also prove that `P8` may legitimately retain no palette; absence is
therefore allowed, while every present palette and retained index is validated.

Accepted implementation:

1. Introduce one private typed codec error contract and convert every
   operationally fallible private helper from `Option` or `Result<T, ()>` to a
   meaningful `Result`.
2. Retain `Option` only for genuine absence, including optional metadata,
   transparency, animation values, and infallible searches.
3. Map codec errors into the existing canonical `ImageError` categories at the
   format dispatcher without adding `try_*` entry points or exposing algorithm
   modules.
4. Match the pinned Pillow detection predicates using mutated complete image
   fixtures, never synthetic prefix-only assertions.
5. Share palette structural validation between `ImagePalette::new` and
   `DecodedImage::validate`, and prove rejected encode inputs through the
   manifest.
6. Migrate shared compression and each independently feature-gated codec as
   separately reviewable slices, preserving exact pixels, frames, metadata,
   encoded bytes, feature behavior, WASM behavior, and native AVIF behavior.
7. Require strict native/WASM formatting, Clippy, and rustdoc gates plus
   Coverage MCP exact manifest parity and 100% line, branch, function, and
   region coverage before completion.

Final source audit:

- every public operational entry point returns `ImageResult`, and codec-private
  failure propagation uses `CodecResult` or a native error converted to it;
- no production source path calls `.ok()` to erase an error, and there are no
  duplicate `try_*` entry points;
- TIFF LZW short reads now return a classified codec error instead of using
  `Option` as an end-of-stream signal;
- AVIF detection is proved with complete `avif`, `avis`, `mif1`, `msf1`, and
  unsupported-major-brand files;
- TIFF detection is proved with complete files for all six Pillow-registered
  signatures. The pinned Pillow parser's asymmetric behavior is retained:
  `II+\0` selects unsupported BigTIFF layout and fails, while `MM\0+` continues
  through the classic big-endian parser for the manifest payload; and
- direct public palette mutation is represented by Pillow-oracle encode-error
  rows where Pillow has an equivalent operation: an oversized palette and a
  palette attached to non-indexed data. Other model-only validation branches
  are not described as Pillow behavior.

The remaining production `Option` signatures were reviewed rather than
mechanically renamed:

| Contract | Retained uses |
|---|---|
| Optional observable/container state | palette, palette alpha, loop/background metadata, uninitialized cache, AVIF main/alpha item or track absence, WebP background color |
| Infallible lookup/search | TIFF tag lookup, GIF palette color lookup, AVIF item/location and reconstructed-leaf lookup |
| Standard protocol | PNG chunk iterator `Iterator::next` |
| Closed implementation path | portable AVIF applicability and AV1 restoration applicability; `None` selects the complete native decoder or rejects only the portable reconstruction path |
| Speculative decoder optimization | WebP Huffman/VP8 fast reads; `None` rolls back and retries the checked decoder, while color-cache absence is `Result<Option<_>>` |
| Non-error algorithm choice | AV1 restoration disabled, VP8 per-block luma mode, zlib-ng optional tail match, and WebP histogram pruning |

Adjacent non-Pillow work is review-only in this slice. Logging must remain
application-owned and dependency-free; CPU benchmarks must use fair release
builds and fixed fixture classes; fuzzing, generic limits, security hardening,
governance, release automation, and image-processing ownership remain future
plans documented in `codec-only-productization-plan.md`. They must not be used
to broaden this parity migration.

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

1. Implement the I0 typed-error contract, exact Pillow signatures, palette
   validation, per-codec `Result` migration, and manifest error evidence.
2. Implement and prove the accepted codec-specific `verify()` contract in I1.
3. Implement the combined codec feature and target matrix from I2.
4. Align migration and rustdoc claims with those decisions.
5. Complete the strict Clippy implementation from I3 without suppressing
   diagnostics or changing parity.
6. Apply I4 only alongside another relevant format-parsing change.
7. Rerun formatting, exact manifest tests, isolated feature/target lanes, and Coverage
   MCP line/branch/region coverage before declaring the upstream slice complete.

## Review completion rule

Each finding is complete when it is fixed with the named evidence or explicitly
accepted as a documented contract or release tradeoff. This review changes no
codec algorithm, public feature set, fixture output, or dependency.
