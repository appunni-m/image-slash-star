# Image Backend Migration: Full Code Review

Date: 2026-07-23

## Review decision

The migration is directionally correct: `image-slash-star` owns signature
detection, header inspection, structured codec errors, decoding, encoding, and
the immutable encoded-source cache; `pillow-rs` owns Pillow-facing state and
operations. The exact-byte, manifest-driven codec tests and the use of
`Arc`/`OnceLock` for deterministic shared results are strong foundations.

The current implementation should not yet be called complete. Three confirmed
runtime issues should be fixed before accepting the lazy-loading slice:

1. appending an operation can silently discard an explicitly selected compute
   backend;
2. `Image::verify()` materializes pipelines despite promising not to change
   state;
3. every read of an already loaded `P` image clones its entire index buffer.

The remaining findings are compatibility, verification-contract, feature-gate,
test-strength, documentation, and repository-hygiene issues. None requires a
new codec, a new image operation, or a dependency expansion. This review keeps
the approved migration scope unchanged.

## Scope reviewed

The review covers the uncommitted image-backend migration in both repositories:

- `image-slash-star`: the canonical `EncodedImage`, format parsing, lifecycle
  and feature-gate tests, and migration/correctness documentation;
- `pillow-rs`: codec feature forwarding, removal of the old format-handler
  layer, lazy path/byte/pipeline state, retained metadata and modes, palette-safe
  operations, structured error propagation, fixture generation, and migration
  tests;
- repository policy relevant to committing this slice.

The review does not reopen codec parity already proven by the existing
manifest, add new Pillow operations, redesign unrelated compute backends, or
address the unrelated FreeType scalar failures.

## Validation performed

The following read-only checks were used while reviewing:

- complete diff and status inspection in both repositories;
- targeted call-path tracing for open, inspect, decode, materialize, load,
  verify, copy, palette handling, pipeline extension, and backend routing;
- feature declaration and feature-test inspection in both crates;
- fixture manifest/generator/test cross-checks;
- `git diff --check` in both repositories, which passed;
- strict all-target/all-feature Clippy invocations in both repositories.

Both strict Clippy invocations currently fail with more than two thousand
diagnostics, primarily the repositories' existing arithmetic/cast lint debt.
This is not a reason to expand this migration into a whole-project lint rewrite,
but it means the documented strict lint commands are not usable acceptance
gates today.

The previously recorded upstream Coverage MCP snapshot is
`bc41e67e-4be2-4eac-9444-abe318a0a151` and reports exact 100% line, branch, and
region coverage. Per the project decision, Coverage MCP is not used for
`pillow-rs`; its migration tests are run manually.

## Acceptance blockers

### R1 — Extending a pipeline discards its backend lock

Severity: high, confirmed correctness defect.

`pillow-rs/pillow-rs/src/image.rs:726-743` destructures an existing
`Image::Pipeline`, appends the operation, and builds a replacement pipeline.
The existing pipeline's `backend` field is ignored. The replacement uses
`source.backend()`, where `source` is the underlying input node, not the
pipeline being extended. In the usual case this evaluates to `None`.

Consequently this sequence does not preserve the caller's choice:

```rust
let image = operation.use_backend(Backend::Cpu);
let image = Image::push_op(&image, another_operation);
```

The second line silently restores automatic routing. This can change output,
performance, or availability when backends do not have identical support.

Recommendation:

- bind and retain the existing pipeline's `backend` value when flattening and
  extending a pipeline;
- add a regression test that locks each available backend, appends at least one
  operation, and asserts the lock before materialization and the selected route
  during execution;
- include a palette-safe pipeline case because that path has a separate backend
  selection branch at `image.rs:638`.

### R2 — Pipeline verification mutates observable lazy state

Severity: high, confirmed contract defect.

`Image::verify()` documents that it validates “without changing its state” at
`pillow-rs/pillow-rs/src/image.rs:1552-1567`. Path and byte sources perform an
independent `EncodedImage::verify()`, but every other variant calls
`materialized_shared()`. For `Image::Pipeline`, that initializes the pipeline's
`OnceLock` and changes `is_materialized()` from `false` to `true`.

The current lifecycle fixtures prove non-mutation for encoded sources but do
not cover a deferred pipeline.

Recommendation:

- execute pipeline verification independently and discard its output rather
  than publishing it to the ordinary materialization cache;
- assert that a pipeline is not materialized before or after both successful
  and failing `verify()` calls;
- repeat the assertion through a clone to prove the shared cache remains
  untouched;
- keep the current public contract. Weakening the contract only for pipelines
  would make the API state-dependent and harder to reason about.

### R3 — `P`-mode read access performs an unbounded hidden copy

Severity: high, confirmed performance and lifecycle-contract defect.

`pillow-rs/pillow-rs/src/image.rs:584-588` returns shared storage for
`Image::Loaded`, but `Image::Paletted` constructs a new `DynamicImage` from
`data.indices.clone()` on every call. All read methods routed through
`materialized_shared()` therefore copy the complete index plane every time.
After `load()` converts a source or pipeline into `Image::Paletted`, repeated
`tobytes`, `getpixel`, `save`, statistics, or other reads keep paying that cost.

This contradicts the explicit invariant in
`docs/lazy-loading-correctness-proposal.md` that read-only access shares cached
pixel storage and only ownership-promising or copy-on-write APIs allocate a
full copy.

Recommendation:

- give paletted indices shared immutable storage, or give `PalettedData` its own
  once-initialized `Arc<DynamicImage>` view;
- ensure palette mutation uses copy-on-write and cannot mutate another clone;
- add an allocation- or pointer-identity regression test for repeated reads of
  loaded and pipeline-produced `P` images;
- retain the existing exact indices, palette, alpha, and encoded-byte oracle
  assertions.

## Correctness and contract findings

### R4 — “Fully validates” is stronger than the implemented codec contract

Severity: medium-high.

`image-slash-star/src/source.rs:87-96` implements `EncodedImage::verify()` by
calling the single-image `decode()` API. This validates the decoded image that
API selects, but it does not universally mean every frame or image directory:

- AVIF `decode()` explicitly decodes frame zero;
- WebP has a separate `decode_sequence()` path for all animation frames;
- TIFF `decode()` documents that it decodes the first IFD.

The source documentation and downstream migration documents currently call
this full snapshot validation. A later corrupt frame or IFD may therefore be
outside the work performed by `verify()`.

Recommendation:

- define the contract explicitly as either “validate the primary decoded
  image” or “validate the entire encoded container”;
- for full-container validation, route multi-image formats through their
  sequence/container validators and add fixtures whose first frame is valid but
  a later frame is corrupt;
- otherwise change only the wording and tests so callers are not promised more
  than the implementation performs.

This decision stays within the existing lifecycle API; it does not require a
new operation or format.

### R5 — Public enum fields make invalid lazy states constructible

Severity: medium-high API correctness risk.

`pillow-rs/pillow-rs/src/image.rs:87-160` publicly exposes `LoadedData`, every
field of `Image::{Path,Bytes,Pipeline}`, and `MaterializationCache`. External
code can construct states that the implementation assumes are consistent, for
example:

- an encoded `source` paired with a different `format` or `ImageInfo`;
- a materialization cache originating from an unrelated image;
- a pipeline whose retained mode, palette, alpha, or format does not describe
  its source and operations;
- arbitrarily nested or self-referential-looking public pipeline graphs.

This conflicts with the correctness proposal's goal of making invalid
combinations unrepresentable and preventing cycles at construction boundaries.
The internal constructors are coherent; the risk comes from the public data
model.

Recommendation:

- treat the representation as an internal implementation detail and expose
  read-only accessors/constructors;
- if immediate field privacy is too disruptive, mark the enum non-exhaustive,
  document the invariants, and schedule full encapsulation for the same breaking
  release as the other migration API changes;
- add constructor-level invariant tests rather than tests that directly build
  public cache graphs.

Do not make this as a silent patch release: downstream code may currently
pattern-match or construct these public variants.

### R6 — Palette safety classifies tuple `putpixel` more broadly than proved

Severity: medium.

`Image::is_palette_safe_op()` treats every `PipelineOp::PutPixel` as
index-preserving. The operation stores a generic RGBA tuple, while the P-mode
executor writes only `color.0` as the palette index. The oracle fixtures cover
`putpixel_mode(..., "P")` with a scalar index, including a chained case, but do
not cover public tuple-based `Image::putpixel()` on a P image.

Recommendation:

- distinguish an explicit palette-index write from a color-tuple write in the
  pipeline representation, or reject/convert tuple writes according to the
  Pillow contract;
- only classify the explicit index operation as palette-safe until tuple
  behavior has an oracle row;
- add success and error fixtures for the exact public bindings that can reach
  both forms.

### R7 — Encoded and decoded pixel buffers are retained twice downstream

Severity: medium, measurable resource risk.

The canonical `EncodedImage` correctly retains encoded bytes and caches a full
`DecodedImage`. Downstream `decoded_to_dynamic()` then clones the decoded pixel
vector into a separate `DynamicImage`, after which both full decoded buffers
remain alive as long as the encoded source is retained. P-mode load can add
another conversion copy.

This is correct but can roughly double decoded-pixel residency in addition to
the encoded source, which matters for large images and WASM. It should not be
optimized speculatively because ownership changes can damage cache semantics.

Recommendation:

- add a focused native/WASM memory benchmark for open, first read, repeated
  read, clone, mutation, and drop;
- then prefer a shared/owned decoded-pixel handoff or a downstream operation
  representation that can directly share the backend buffer;
- keep deterministic failure caching and immutable clone behavior intact.

## Feature and target findings

### R8 — Feature tests do not prove one-codec-at-a-time forwarding

Severity: medium-high test gap.

The upstream and downstream feature tests correctly prove disabled-codec
errors. The downstream Makefile runs migration parity with all features and the
feature-error test with no default features. Those two endpoints cannot detect
an individual forwarding mistake: an `image-png` feature wired to the wrong
upstream feature is masked by `--all-features`, while `--no-default-features`
enables neither side.

Recommendation:

- add isolated manifest lanes for each downstream feature:
  `image-jpeg`, `image-png`, `image-gif`, `image-bmp`, `image-tiff`,
  `image-webp`, `image-ico`, and `image-avif`;
- in each lane, prove that the selected format succeeds and unrelated formats
  return the exact `FeatureDisabled` error;
- explicitly assert ICO's intentional transitive PNG/BMP features;
- keep the no-default and all-feature lanes as boundary checks.

### R9 — AVIF enabled on WASM degrades into a malformed-input error

Severity: medium.

The `avif` feature is available in `image-slash-star/Cargo.toml`, but the AVIF
decode and sequence implementations return `None` on `wasm32`. A valid AVIF
with the feature enabled can therefore surface as malformed input rather than a
target-unavailable/unsupported capability. This is especially confusing for
the planned core/extra WASM split.

Recommendation:

- either make native AVIF impossible to select for unsupported targets or
  return a structured target/capability error;
- add a WASM compile/behavior lane for the AVIF feature before advertising it
  in a JS package split;
- preserve the current native pinned-libavif behavior.

## Compatibility findings

### R10 — The migration contains public breaking API changes

Severity: medium-high release risk.

The change is not source-compatible for all Rust consumers:

- `PilError::Io` changes its payload from `std::io::Error` to
  `Arc<std::io::Error>` so errors can be cloned and cached;
- `LoadedData.image` changes from `DynamicImage` to `Arc<DynamicImage>`;
- public `Image` variants add or change source, metadata, palette-alpha, backend,
  and materialization fields;
- the old direct format-handler modules are removed.

These choices are reasonable for persistent shared state, but callers that
pattern-match public variants or construct errors/data directly will break.

Recommendation:

- ship the migration under an explicitly breaking version boundary;
- add a concise migration guide covering error matching, pixel access,
  construction, feature selection, and removed format handlers;
- distinguish semantic behavior changes from mechanical `Arc` dereferencing;
- avoid adding compatibility aliases named `pillow-rs-image`; the canonical
  crate/package name should remain `image-slash-star`.

## Test and fixture findings

### R11 — Palette-safe parameter boundaries are under-sampled

Severity: medium test-strength gap.

The 19 exact oracle rows are valuable and should remain the authority. Some
operation classifications cover larger parameter spaces than their current
proof: arbitrary rotate angles/expand choices, nearest-neighbor output shapes,
affine coefficients/fill behavior, and boundary coordinates.

Recommendation:

- add a compact boundary matrix rather than combinatorial random tests: zero,
  right-angle, negative, and non-right-angle rotations; expand on/off; 1-pixel
  resize/thumbnail dimensions; affine identity/translation/out-of-bounds; and
  first/last pixel writes;
- keep every row generated by the pinned Pillow oracle and compare exact mode,
  dimensions, indices, palette, alpha, and encoded bytes;
- allow failing fixture rows while investigating, as already agreed, but keep
  them explicit in the manifest.

### R12 — The operation fixture generator can leave stale files

Severity: low-medium repository/test risk.

`scripts/generate_image_backend_operation_fixtures.py` writes the current rows
but does not enforce a bijection between manifest entries and files in
`outputs/operations`. Renaming or removing an operation can leave obsolete
binary/PNG artifacts committed indefinitely.

Recommendation:

- calculate the expected output set and fail on or remove stale generated files
  within that exact fixture directory;
- add a manifest-to-files completeness assertion to the Rust test;
- keep regeneration deterministic under the pinned Pillow oracle.

### R13 — Temporary path lifecycle is not panic-safe

Severity: low.

The path-backed migration test uses a process-derived temporary filename and
manually removes it at the end. A panic leaves the file behind, and a repeated
case within the same process can collide.

Recommendation:

- use an RAII temporary-file guard with a unique suffix;
- avoid adding a production dependency solely for this test convenience.

## Documentation and repository findings

### R14 — Acceptance documents overstate current guarantees

Severity: medium.

The migration/correctness/status documents currently state or imply all of the
following as complete: non-mutating verification, no hidden read copies,
cycle-safe construction, full encoded-snapshot validation, and feature
forwarding. R2, R3, R4, R5, and R8 show that those claims are either false or
stronger than the tests.

Recommendation:

- after resolving each finding, update the status table with the exact proven
  contract and test lane;
- until then mark these rows partial rather than accepted;
- reconcile the migration spec's Coverage MCP checkpoint wording with the
  explicit decision not to use Coverage MCP for `pillow-rs`.

### R15 — Strict Clippy is documented but not currently enforceable

Severity: medium process risk.

Both repositories document strict Clippy commands, but current all-feature,
all-target invocations fail with thousands of warnings promoted to errors. A
gate that is permanently red cannot distinguish migration regressions from
baseline debt.

Recommendation:

- record a lint-debt baseline and reject new diagnostics in touched code/files;
- separately burn down the existing repository debt;
- do not weaken correctness lints globally and do not absorb the entire lint
  cleanup into this migration.

### R16 — Untracked generated/tool state should not enter the commit

Severity: low-medium.

The `pillow-rs` worktree currently contains an untracked `.coverage-mcp/`
directory, but its root `.gitignore` does not ignore that directory. This is
both unnecessary generated state and contrary to the decision not to use
Coverage MCP for that repository.

Recommendation:

- add `/.coverage-mcp/` to the `pillow-rs` root ignore rules;
- verify that no database or report from it is staged;
- do not delete user data as part of the migration commit—only prevent accidental
  inclusion.

### R17 — A one-off palette analysis script remains in the worktree

Severity: low.

`pillow-rs/scripts/analyze_palette_rotate.py` is an untracked diagnostic script
and is not referenced by a maintained generator or test target. Repository
instructions say one-off debugging scripts should not remain.

Recommendation:

- exclude it from the migration commit, or promote it only if it becomes a
  documented deterministic diagnostic with a maintained invocation;
- keep `generate_image_backend_operation_fixtures.py`, which is a permanent
  fixture generator tied to the oracle manifest.

### R18 — Unrelated formatting noise should be separated

Severity: low.

`pillow-rs/tests/imagingft_matrix_tests.rs` contains formatting-only changes
unrelated to the image backend migration. Keeping unrelated edits in this slice
makes review and later regression attribution harder.

Recommendation:

- exclude that file from the migration commit unless its formatting change is
  intentionally part of a repository-wide formatting commit;
- preserve the existing unrelated FreeType test state.

### R19 — Small avoidable allocation in extension parsing

Severity: optional.

`ImageFormat::from_path()` lowercases the extension into a new `String`, while
`ImageFormat::from_name()` already compares case-insensitively. Passing the
borrowed extension directly would avoid a tiny allocation.

Recommendation: simplify when touching this function for another reason; do
not create a standalone migration task for it.

## Positive findings

- Auto-detection, inspection, decode/encode dispatch, and codec-specific errors
  are correctly centralized in `image-slash-star`, not duplicated in
  `pillow-rs`.
- `EncodedImage` snapshots input bytes and shares inspection/decode state across
  clones with thread-safe standard-library primitives.
- Successful and deterministic failed decodes use one shared initialization
  path.
- Downstream path opening snapshots bytes, so replacing the filesystem path
  after open does not change the image source.
- Source format, exact decoded mode, palette, palette alpha, and inspected
  metadata are retained through the main decode/load path.
- Structured `Result` APIs are used without duplicate `try_*` APIs.
- Codec feature forwarding is explicit and default features exclude AVIF, which
  is appropriate while AVIF remains an optional native stack.
- Exact fixture assertions are used rather than byte-length-only comparisons.
- No new unsafe Rust or FFI boundary was introduced by this migration slice.
- `git diff --check` is clean in both repositories.

## Recommended resolution order

This ordering fixes correctness first without expanding the migration:

1. fix R1, R2, and R3 and add focused lifecycle tests;
2. decide and test the verification contract in R4;
3. narrow/prove palette writes in R6 and add the compact R11 boundary matrix;
4. add isolated feature lanes from R8 and target-aware AVIF behavior from R9;
5. make the public compatibility decision in R5 and document all breaks in R10;
6. measure R7 before changing buffer ownership;
7. align claims and gates under R14 and R15;
8. clean the eventual commit using R12, R13, R16, R17, R18, and R19;
9. rerun formatting, exact migration tests, isolated feature lanes, and upstream
   Coverage MCP line/branch/region coverage before marking the slice complete.

## Completion criteria for this review

The review itself changes no production code, public API, feature set, fixture,
or dependency. Its suggestions are complete when every numbered item is either
fixed with the named evidence or explicitly accepted as a documented tradeoff
at the appropriate release boundary.
