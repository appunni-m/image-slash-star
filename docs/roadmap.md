# Roadmap

Status: accepted direction; items below are planned unless marked implemented

Reviewed: 2026-07-31

This roadmap contains future product work only. Current behavior belongs in the
[README](../README.md), [architecture](architecture.md), generated rustdoc, and
[testing contract](testing.md).

## Non-negotiable constraints

Every roadmap item must preserve:

- codec-only scope: detection, inspection, decoding, and encoding;
- no public image-processing API;
- `bytemuck` as the only Cargo dependency;
- independently feature-gated formats;
- Rust-only default codecs;
- eventual full WASM functionality for every published codec feature;
- explicit output format selection;
- structured `ImageResult` failures;
- exact manifest-driven Pillow success and error parity; and
- 100% line, branch, function, and region coverage.

Native AVIF is migration debt, not an exception to the final WASM constraint.

## Implemented foundation

The following decisions are already implemented and are not roadmap work:

- canonical auto-detecting root APIs;
- private codec dispatchers;
- separate source `ImageFormat` and decoded `ImageMode`;
- exact palette, alpha, frame, timing, disposal, and background transfer
  models;
- structured errors rather than fallible `Option`;
- immutable encoded snapshots and shared lazy decode results;
- one Cargo feature per format, with ICO forwarding PNG and BMP;
- exact fixture-backed errors and byte outputs;
- no general image-processing layer; and
- complete retained third-party provenance and legal texts.

## Priority 0: release blockers

### Portable AVIF

Replace the native AVIF runtime path with a repository-owned AV1/AVIF
implementation for every active still, sequence, metadata, option, success, and
error case.

Acceptance is defined in [AVIF support](avif.md#completion-criteria).

### Runtime WASM parity

Run the semantic manifest in a real WASM runtime. Cross-compiling Clippy,
rustdoc, and test binaries is not runtime evidence.

Acceptance:

- default and individually selected codec features execute in WASM;
- supported output and structured errors match native fixture expectations;
- AVIF capability differences are removed or explicitly fail the release gate;
  and
- JS/WASM artifact sizes are revision-bound and reproducible.

### Caller-controlled resource limits

Add typed limits before recommending the crate for arbitrary hostile inputs.
Limits need to cover at least:

- dimensions and decoded bytes;
- frame count and total sequence bytes;
- metadata/container nesting;
- codec work that can grow independently of output size; and
- output allocation.

Limit failures must be distinguishable from malformed input and must be covered
by complete fixtures where observable through Pillow, or clearly labeled
model-only defensive cases otherwise.

### Stability and support contract

Before the first registry release:

- define MSRV and supported-target policy;
- define what is semver-public;
- state pre-1.0 breaking-change expectations;
- document deprecation and migration behavior;
- define support and end-of-life windows; and
- establish a release and failed-release recovery procedure.

## Priority 1: integration quality

### Typed encode options

Replace catch-all string configuration with format-specific typed options.
Compatibility keys may remain temporarily at an adapter boundary, but new
public behavior should be discoverable from types and rustdoc.

Acceptance:

- ranges, defaults, interactions, target availability, and errors are explicit;
- unsupported options are not silently accepted by the wrong codec;
- existing manifest output remains exact; and
- migration from `EncodeOptions::extra` is documented.

### Metadata preservation

Define an opaque metadata model for ICC, EXIF, XMP, textual chunks, orientation,
and format-specific blocks.

Preserve bytes first. Parsed semantic metadata should be added only where the
format contract and round-trip behavior are clear.

### Capability discovery

Expose a dependency-free way to ask whether a format/operation is compiled and
available on the current target. It must distinguish:

- signature recognition;
- inspection;
- still decode/encode;
- sequence decode/encode; and
- manifest-bounded or restricted capability classes such as portable AVIF.

### Incremental I/O

Add reader/writer or incremental adapters without adding filesystem policy or
new dependencies. The byte-slice API remains the simplest path.

Acceptance must define partial input, buffering, cancellation/early exit,
output errors, and resource limits.

## Priority 2: assurance and release evidence

### Fuzzing

Add format-aware parser and round-trip fuzz targets. Fuzzing complements the
oracle matrix; it does not establish Pillow parity or security.

### Performance and size

Establish reproducible native and WASM benchmarks before making performance
claims. Every result must name the revision, toolchain, target, hardware,
features, workload, sample count, aggregation, and artifact form.

### API compatibility

Add a release-time public API diff and semver review once the surface is stable
enough to publish.

### Package policy

Resolve Cargo's warning about repository-only integration targets without
shipping the full oracle corpus or publishing an incomplete test target.
Verify the exact `.crate` contents and first-use path outside the source
checkout.

## Possible format expansion

New formats are lower priority than completing the current contracts. A format
is eligible only when:

- it is independently feature-gated;
- decode and encode are codec work rather than image processing;
- it works natively and on WASM;
- it adds no dependency;
- a fixed oracle or specification-backed reference exists;
- errors and exact outputs have manifest fixtures; and
- licenses and provenance are complete.

## Explicit non-goals

The roadmap does not include:

- resizing, cropping, rotating, drawing, filters, or general compositing;
- a `DynamicImage` compatibility layer;
- path traversal or filesystem sandbox policy inside the crate;
- runtime Python or Pillow;
- plugin-loaded or dynamically registered codecs;
- logging infrastructure;
- weakening exact byte/error assertions to broaden nominal support; or
- replacing dependency-free implementations with native libraries.

## Documentation maintenance

Maintain only four files under `docs/`:

- `architecture.md` for current contracts and boundaries;
- `testing.md` for oracle and verification workflow;
- `avif.md` for the one target-dependent codec boundary; and
- `roadmap.md` for planned work.

When an item ships, remove it from the roadmap and update the current contract.
Do not create per-sweep, per-branch, completion-audit, or downstream-project
documents here. Git history, commits, Coverage MCP artifacts, and the owning
downstream repository retain that evidence.
