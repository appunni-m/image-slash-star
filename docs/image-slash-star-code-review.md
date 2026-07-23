# `image-slash-star` Code Review

Date: 2026-07-23

## Decision

The migration places the correct responsibilities in this crate:
`image-slash-star` owns signature detection, header inspection, structured
codec errors, format-specific decode/encode dispatch, and immutable encoded
source caching. The manifest-driven exact-byte tests and the use of
`Arc`/`OnceLock` for shared deterministic results are strong foundations.

No unsafe Rust or new FFI boundary was introduced in the reviewed source-cache
slice. The remaining upstream work is contract and target correctness, feature
test strength, and maintenance-gate cleanup. It does not require a new codec or
an expansion of the approved migration.

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
- The recorded Coverage MCP snapshot
  `bc41e67e-4be2-4eac-9444-abe318a0a151` reports 100% line, branch, and region
  coverage.
- The strict all-target/all-feature Clippy command was run and currently fails
  with more than two thousand diagnostics, primarily existing arithmetic/cast
  lint debt.

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

Recommendation:

- decide whether `verify()` means “validate the primary decoded image” or
  “validate the entire encoded container”;
- if full-container validation is required, route multi-image formats through
  sequence/container validation and add fixtures with a valid first image and
  corrupt later image;
- otherwise narrow the rustdoc and migration claims to the primary image;
- retain independent execution so verification does not populate the ordinary
  lazy decode cache.

### I2 — Feature tests do not exercise each codec in isolation

Severity: medium test gap.

`tests/feature_gate_tests.rs` correctly proves that disabled formats return the
exact `FeatureDisabled` error. The usual all-feature lane proves that all
formats work together. Those endpoints do not prove that each individual
feature enables exactly its intended codec.

Recommendation:

- run one manifest lane for each of `jpeg`, `png`, `gif`, `bmp`, `tiff`,
  `webp`, `ico`, and `avif`;
- in each lane, prove that the selected codec succeeds and unrelated codecs
  retain their exact disabled-feature errors;
- explicitly prove ICO's intentional transitive `bmp` and `png` features;
- retain the no-default and all-feature lanes as boundary checks.

### I3 — AVIF enabled on WASM reports malformed input instead of capability

Severity: medium.

The `avif` Cargo feature can be selected for `wasm32`, while AVIF decode and
sequence functions return `None` on that target. A valid AVIF can consequently
surface as malformed input instead of a structured target-unavailable or
unsupported-capability error.

Recommendation:

- either prevent the native AVIF feature from being selected on unsupported
  targets or return a structured target/capability error;
- add a WASM compile and behavior lane before exposing AVIF in the planned
  core/extra JS package split;
- preserve the current pinned native libavif behavior.

### I4 — The documented strict Clippy gate is permanently red

Severity: medium process risk.

The documented all-target/all-feature Clippy invocation currently emits more
than two thousand diagnostics when warnings are denied. Most are existing
arithmetic and cast warnings rather than regressions from `EncodedImage`.
A permanently failing gate cannot distinguish new defects from baseline debt.

Recommendation:

- record a lint-debt baseline and reject new diagnostics in touched code;
- burn down the existing diagnostics separately;
- do not weaken correctness lints globally or absorb the whole cleanup into
  this migration.

### I5 — `ImageFormat::from_path()` performs an avoidable allocation

Severity: optional.

`ImageFormat::from_path()` lowercases an extension into a new `String`, while
`ImageFormat::from_name()` already compares names case-insensitively.

Recommendation: pass the borrowed extension directly when this function is
next touched. This does not justify a standalone migration task.

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

1. Decide and prove the `verify()` contract in I1.
2. Add isolated feature lanes from I2.
3. Make AVIF target behavior explicit under I3.
4. Align migration and rustdoc claims with those decisions.
5. Establish the lint baseline from I4 without widening migration scope.
6. Apply I5 only alongside another relevant format-parsing change.
7. Rerun formatting, exact manifest tests, isolated feature lanes, and Coverage
   MCP line/branch/region coverage before declaring the upstream slice complete.

## Review completion rule

Each finding is complete when it is fixed with the named evidence or explicitly
accepted as a documented contract or release tradeoff. This review changes no
codec algorithm, public feature set, fixture output, or dependency.
