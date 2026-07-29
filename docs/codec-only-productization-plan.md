# Codec-Only Productization Plan

Date: 2026-07-30

Status: accepted implementation plan. Codec-only scope separation is complete;
portable AVIF and the remaining WASM/dependency release gates are in progress.

Related review:
[image-rs comparison and gap analysis](image-rs-gap-analysis.md).

## Current Alignment Ledger

This ledger records repository evidence rather than inferred completion. It is
updated whenever an accepted slice changes the public or published contract.

| Requirement | Current evidence | State |
| --- | --- | --- |
| Codec-only public surface | The root API exposes byte detection, inspection, decode, encode, immutable encoded sources, decoded transfer models, and options. A source search finds no public processing operation or compatibility buffer/editor type. | aligned |
| Rust dependency graph | `cargo tree --all-features --locked` contains only `image-slash-star` and `bytemuck 1.25.1`. | aligned |
| Third-party provenance | The retained-license verifier inventories and verifies 22 legal/provenance files; `cargo-deny` passes advisories, bans, licenses, and sources. | aligned |
| Canonical structured API | Codec modules are private and the public detect/inspect/decode/encode paths use `ImageResult`. | aligned |
| Feature isolation | No-feature, individual-codec, default, and all-feature native/WASM compilation and strict-lint lanes pass; ICO alone intentionally enables BMP and PNG. | aligned for compilation |
| Portable AVIF | Detection, bounded inspection, and the manifest-bounded AV1 still-decode classes in `portable-avif-progress.md` run in-tree. Native decode outside those classes and all native encode still use the pinned C stack. | partial; release blocker |
| Executed WASM parity | The matrix cross-compiles, but the full semantic manifest is not executed in a WASM runtime. | missing; release blocker |
| Public status claims | README counts and the portable-AVIF boundary are synchronized with the Slice 38 manifest in this update. | aligned through Slice 38 |
| Caller-controlled limits | No `DecodeLimits` or `DecodeOptions` contract exists in `src/`. | missing |
| Typed encoder configuration | `EncodeOptions` still exposes string-valued subsampling, AVIF pairs, and a catch-all `HashMap`; metadata is passed through string/hex keys. | missing |
| Capability discovery | No public capability query describes format, operation, feature, and target support. | missing |
| Metadata preservation model | Format parsers retain selected palette/frame fields and some encoders accept opaque metadata, but `ImageInfo` and decoded envelopes do not yet expose the accepted opaque metadata contract. | partial |
| Publishable external release | Offline packaging verifies 135 files and 431.5 KiB compressed, but portable AVIF, limits, executed WASM parity, examples, and support policy remain incomplete. | not ready |

The highest-priority implementation path remains A3: finish portable AVIF and
remove the native stack from the published build contract. Limits, typed
options, metadata, and external-adoption work must not use the native bridge or
new dependencies as shortcuts.

## 1. Product Decision

`image-slash-star` is an image **encoding and decoding** crate. It is not an
image editor or image-processing crate.

The repository may implement everything required to make codecs complete,
safe, portable, interoperable, testable, and easy to adopt. That includes
format detection, header inspection, metadata, still images, animation,
streaming, resource limits, codec configuration, format expansion, buffer
layout, compatibility adapters, fuzzing, benchmarks, documentation, and
release engineering.

The repository must not grow operations whose purpose is to transform already
decoded pixels independently of a codec.

### 1.1 Hard constraints

Every implementation slice must preserve all three constraints:

1. **Codec-only scope.** Public and reusable internal behavior must directly
   serve image detection, inspection, decoding, or encoding. General image
   processing is prohibited even when retained only for compatibility.
2. **WASM portability.** Every advertised format feature and public API must
   compile and provide the same documented semantic contract on
   `wasm32-unknown-unknown`. A native-only implementation is incomplete.
3. **No dependency growth.** `bytemuck` is the one previously approved library
   dependency. Do not add runtime, optional, target-specific, build, or proc
   macro dependencies beyond its existing graph.

Pillow, Coverage MCP, compilers, fuzzers, and benchmark runners are development
tools. They may be used to generate or verify artifacts, but they must not
become dependencies of the published library or generated WASM module.

### 1.2 In scope

- signature detection and explicit format selection;
- cheap header inspection;
- still-image decode and encode;
- animated/multi-image decode and encode;
- exact source-format, pixel-mode, palette, alpha, and frame retention;
- metadata extraction, preservation, validation, and emission;
- caller-controlled resource limits;
- byte, slice, and portable in-memory reader/writer interfaces;
- streaming or incremental codec state;
- rectangular or partial decode when the format can support it;
- typed encoder configuration;
- structured codec errors;
- capability discovery;
- static codec abstractions and registration that work on WASM;
- additional image formats implemented in-tree;
- optional scalar/SIMD implementations with a portable scalar fallback;
- raw-layout and FFI-friendly views;
- fixture generation, Pillow-oracle parity, fuzzing, coverage, and benchmarks;
- crate documentation, examples, packaging, and releases.

### 1.3 Out of scope

- resize and resampling;
- crop as an image operation;
- rotate and flip as image operations;
- blur, sharpen, convolution, and filters;
- brightness, contrast, hue, and color enhancement;
- drawing, text rendering, compositing, and blending;
- a mutable image-editor API;
- filesystem ownership, path sandboxing, or network fetching;
- dynamic native plugins or shared-library loading;
- a direct dependency on `image`, `serde`, Rayon, or a native codec library.

Cropping that is an intrinsic part of decoding a tiled or region-addressable
format is in scope. A general `crop_imm` method over arbitrary decoded pixels
is not.

Codec-mandated frame disposal/blending, alpha composition, and YUV/RGB sample
conversion are also in scope because they are necessary to produce the decoded
representation. General compositing of unrelated decoded images is not.
Likewise, preserving and reporting an ICC profile is in scope; applying that
profile as a pixel transformation belongs downstream.

The deciding test is observable purpose, not the name of an algorithm:

- work needed to turn encoded bytes into the documented decoded samples, or
  documented samples into an encoded format, is codec implementation;
- work that accepts an already decoded image and returns different pixels for
  a caller-selected visual effect is image processing and is prohibited here;
- a container encoder must reject a convenience request that would require
  resampling, rotation, compositing, or color enhancement unless the caller
  supplies the already-transformed samples or container entries.

`DecodedImage` and `DecodedSequence` are validated codec data-transfer models.
They may expose constructors, validation, immutable sample access, palette and
frame metadata, but must not accumulate transformation methods.

### 1.4 Retired scope leak

The former compatibility layer exposed `DynamicImage`, generic image buffers
and traits, color conversion, blending, `fliph`, `flipv`, rotations, and
`crop_imm`. None was required by a codec path.

The image-slash-star side of this migration was completed on 2026-07-29:

- the entire general buffer/pixel/processing compatibility layer was removed;
- the public model was reduced to encoded containers, decoded samples,
  palettes, animation state, options, and structured codec errors;
- all 32 processing rows and their generated binary references were removed
  from the Pillow manifest harness;
- a follow-up ICO audit removed the private Lanczos resampler and 18 oracle
  rows that requested generated sizes, filtered size lists, or resizing-only
  failures;
- ICO still encoding now accepts exactly one source-sized entry and rejects
  any size request that would require processing;
- exact codec parity still passes; and
- Coverage MCP reports 100% line, branch, function, and region coverage.

Downstream ownership was completed on 2026-07-29 without restoring a
compatibility facade:

1. every downstream use was audited and retargeted;
2. `pillow-rs/src/raster/` now owns `DynamicImage`, generic buffers, pixels,
   views, conversions, and raster transforms;
3. `pillow-rs` calls `image-slash-star` only for codec models, feature-gated
   detection, inspection, decoding, encoding, and structured codec errors;
4. Pillow retains the same lazy source snapshot, source format, exact decoded
   mode, palette, alpha, and cached metadata across materialization;
5. Pillow's core, migration, feature-gate, exact parity, and core/extra WASM
   package tests pass with the separated ownership; and
6. `image-slash-star` retains only the validated decoded-sample model and
   accessors required to consume decoder output or provide encoder input.

Any sample conversion required by a specific codec must be private to that
codec or to a codec-only internal utility. It must not be exposed as a general
pixel-processing API.

An encoder option does not make general processing codec-intrinsic. The former
ICO `sizes` implementation generated a multi-resolution icon by applying a
Lanczos resampler to one caller-supplied raster. That behavior belonged to
Pillow's save convenience layer, not the ICO container encoder. Multi-resolution
ICO support may return only through an entry-oriented API that accepts one
already-sized raster per directory entry.

### 1.5 Current compliance gaps

The constraints describe the required destination, not the current state:

- AVIF detection and bounded container inspection are now portable, but pixel
  decode is portable only for the first proven closed AV1 class; other decode
  classes and encoding still skip the C bridge on WASM and rely on
  libavif/dav1d/libaom natively;
- the WASM matrix compiles selected lanes but does not execute codec parity;
- the build script still knows how to invoke C tools and link native AVIF.

The Rust dependency portion of K2 is complete: `bytemuck` is the only Cargo
dependency, including development targets. The remaining gaps are tracked in
A3, K3, and Phase 0. Do not market the complete AVIF/WASM contract as finished
while they remain.

## 2. Architecture Rules

### 2.1 Encoded bytes are the canonical input

The stable core remains:

```text
&[u8] -> detect/inspect/decode/decode_sequence
DecodedImage/DecodedSequence + ImageFormat -> encode
```

This contract is naturally usable in native Rust, browsers, workers, Node,
serverless WASM, and embedded runtimes. All convenience APIs must delegate to
this byte core rather than duplicating detection or codec logic.

Do not add a filesystem path API to the crate. A path API is not meaningful in
all WASM hosts and would mix host security policy into a codec library. Native
users can call `std::fs::read` and `std::fs::write` around the byte API.

The existing `ImageFormat::from_path` is only lexical extension parsing and
performs no I/O, canonicalization, or path resolution. It can remain portable,
although a future name such as `from_extension` would express the security
boundary more clearly.

Portable `std::io::Read`/`Write` adapters are allowed only if they:

- work with in-memory readers and writers on WASM;
- add no dependency;
- preserve the same limits and errors as the byte API;
- clearly state when they buffer the complete payload.

### 2.2 Feature means portable functionality

Each image type remains independently feature-gated. Enabling a feature must
mean that its advertised operations work on every supported target.

It is not sufficient for a feature to compile on WASM and return
`TargetUnsupported` for all useful operations. Target capability errors are
appropriate only for genuinely optional acceleration, never for the sole
implementation.

ICO may continue to enable PNG and BMP because its container can embed either
representation. Every other transitive feature relationship must be justified
by the format.

### 2.3 Portable implementation first

Every codec starts with a safe scalar Rust implementation in this repository.
Target-specific acceleration is a later internal detail and must have:

- the exact same observable result;
- a scalar fallback;
- a fixture proving both paths;
- no public behavior difference;
- no extra crate or native library.

WASM SIMD may be added only after measurement and must not become required.
WASM threads must not be assumed.

### 2.4 Exactness remains manifest-bounded

The pinned Pillow oracle remains the behavioral authority for formats and
operations Pillow supports. Exactness means exact values, not approximate
visual similarity:

- detected format;
- decoded mode and dimensions;
- pixel bytes;
- palette and alpha;
- frame state;
- metadata where the contract retains it;
- encoded bytes;
- structured error category and stable fields.

The promise applies to committed manifest rows. Adding a fixture expands the
proved surface; it does not imply that all possible Pillow inputs were already
covered.

Formats that Pillow does not support need a separate approved oracle. Do not
silently substitute image-rs output and call it Pillow parity.

### 2.5 Public fallibility uses `Result`

`Option` is valid for absence, such as “this file has no ICC profile.” It is
not valid for malformed data, an unsupported mode, a disabled feature, a
limit violation, or encoder failure.

There should be one canonical public path for each operation. Low-level codec
helpers must either be private or return the same structured `ImageResult`
contract as the root API. Do not create duplicate `try_*` APIs.

### 2.6 Validate before allocation and before output

All untrusted sizes, offsets, counts, and products must use checked arithmetic.
Limits must be charged before allocation or unbounded work. Every decoded
model must pass `validate()` before it is returned, and every image or sequence
must pass validation before encoding begins.

### 2.7 No dependency workaround

If an item cannot be implemented within the dependency and WASM constraints,
the acceptable outcomes are:

- implement the required algorithm in-tree under a compatible license;
- narrow the public promise;
- defer the feature;
- temporarily remove an advertised capability.

Adding an optional dependency, native-only feature, command invocation, or
WASM-incompatible fallback is not an acceptable shortcut.

## 3. Definition Of A Complete Slice

Every feature, format, or API slice is complete only when all applicable items
below pass:

1. The public contract is documented before or with the implementation.
2. The implementation is in the correct format module.
3. All public failure paths return structured errors.
4. No library/build dependency is added.
5. No unsafe code is introduced.
6. Native and `wasm32-unknown-unknown` compile with the same feature.
7. Behavior is represented in the fixture manifest, including errors.
8. Fixtures assert exact bytes and structured fields, never only byte length.
9. Pillow generated the expected result where Pillow is the oracle.
10. A non-Pillow format names its specification and oracle explicitly.
11. Coverage MCP reports 100% line, branch, and region coverage.
12. Strict Clippy passes for the relevant feature lanes.
13. Rustfmt and strict rustdoc pass.
14. Package verification proves the published archive contains what tests and
    docs need.
15. Any performance claim has a reproducible release-mode measurement.

The repository's prior testing decision still applies: parity and error
behavior belong in manifest-driven integration tests, not scattered unit
tests. Doctests may demonstrate the public API, and fuzz harnesses may test
invariants, but neither replaces manifest parity.

## 4. Workstream A — Correct The Current Scope And Claims

### A1. Rewrite the product boundary

**Problem**

The crate currently describes itself partly as an `image` API replacement and
exports processing methods. That encourages users to expect an image-processing
library.

**Approach**

- State “codec backend” in the first README paragraph.
- Show the canonical detect/inspect/decode/encode workflow.
- Explain that callers or `pillow-rs` own image operations.
- Replace “matches the image crate API” language with an exact list of buffer
  compatibility types.
- Add the scope boundary from this document to crate-level rustdoc.

**Acceptance**

An unfamiliar user can determine within the first README screen:

- why the crate exists;
- how it differs from `image`;
- what parity is proved;
- that all advertised features are WASM-compatible;
- that `bytemuck` is the only approved dependency;
- that image processing is intentionally elsewhere.

### A2. Retire processing compatibility safely

**Status**

Complete in both repositories.

**Problem**

Removing flip/rotate/crop before migrating `pillow-rs` can break the downstream
build, but leaving them in this repository contradicts the accepted scope.

**Approach**

- Search both repositories for each transformation method.
- Add downstream replacements first.
- Keep the removed methods and compatibility types out of image-slash-star.
- Move downstream storage and processing behind `pillow-rs`-owned types.
- Keep processing parity rows with the downstream operations, not the codec
  manifest.
- Re-measure package and WASM size after removal.

**Risks**

- conversions may accidentally depend on transformation helpers;
- removal can alter feature reachability and coverage;
- downstream and codec-only commits must remain independently buildable during
  the migration.

**Acceptance**

No processing method or general processing compatibility implementation
remains, no codec path depends on one, downstream migration is proved, and the
breaking-release notes list every removal.

### A3. Make current AVIF status honest

**Problem**

The present AVIF path uses pinned native libavif, dav1d, and libaom components.
That violates the final WASM and dependency contract.

**Approach**

AVIF must take one of two release-safe paths:

1. finish the in-tree portable AVIF/AV1 implementation before publication; or
2. withdraw the public AVIF encode/decode feature until that implementation is
   ready, while retaining format detection and an ordinary unsupported-feature
   error on every target.

Do not preserve a public “native AVIF exception.” Native libraries may remain
temporarily as development oracles during the port, but they must leave the
published dependency/build contract.

**Acceptance**

`cargo tree` has no AVIF dependency, no external AVIF library is required to
build any published feature, and the same AVIF manifest executes in the WASM
behavior harness.

## 5. Workstream B — External Adoption

### B1. Installation and release

**Problem**

The crate is not published, so users cannot add a stable registry dependency.

**Approach**

1. Push reviewed commits before documenting a Git revision.
2. Add a pinned Git installation example for the pre-release period.
3. Finish the WASM/dependency release gates.
4. Verify the generated `.crate` as a fresh consumer.
5. Publish a pre-1.0 release.
6. Confirm docs.rs renders the intended feature surface.
7. Add crates.io, docs.rs, license, CI, and WASM badges.

**Acceptance**

A fresh crate can install `image-slash-star`, select two formats with default
features disabled, compile natively and for `wasm32-unknown-unknown`, and run
the documented byte workflow without undeclared host libraries.

### B2. Five-minute quick start

**Problem**

The README has no complete copy-paste application and `examples/` is empty.

**Approach**

Add dependency declarations and complete examples for:

- inspect without materializing pixels;
- auto-detect and decode;
- encode to an explicit output format;
- decode and encode an animation;
- configure strict limits;
- select only required format features;
- pass browser/JS-owned bytes into a Rust-to-WASM caller.

The repository example should use bytes. Native file reading belongs only in a
small host wrapper around it.

**Acceptance**

Every Rust snippet is a doctest or compiled example. The WASM example must not
use `std::fs`, OS paths, environment variables, or a binding-generator crate.

### B3. Stability and support policy

**Problem**

Users do not know which API and byte outputs are stable.

**Approach**

Document:

- pre-1.0 API compatibility rules;
- MSRV;
- supported native targets;
- `wasm32-unknown-unknown` as a mandatory target;
- format/operation support by feature;
- exact-output stability rules;
- how fixture expansion can change previously unspecified behavior;
- native and WASM size measurement procedure;
- security-reporting and patch policy.

**Acceptance**

README, changelog, rustdoc, and Cargo metadata make no conflicting claims.

### B4. Complete rustdoc

**Status**

Complete for the current public surface. The crate-wide missing-documentation
allowance is removed, all reachable public items are documented, and strict
all-feature rustdoc passes with warnings denied. Feature-specific semantic
detail and compiled examples continue under B2/B3 as the API expands.

**Problem**

`#![allow(missing_docs)]` makes a successful rustdoc build weak evidence.

**Approach**

- Document crate and modules first.
- Document public models, invariants, errors, and byte layouts.
- Document every feature-dependent behavior.
- Add `# Errors` and `# Panics` sections where applicable.
- Hide internal codec implementation modules.
- Remove the broad allowance.
- Promote missing docs and broken links to hard failures.

**Acceptance**

Strict rustdoc and doctests pass for no features, each format, defaults, and all
portable features.

## 6. Workstream C — Public API Hardening

### C1. Separate supported API from implementation

**Status**

Complete. `codecs` and its dispatchers are private, the duplicate public
decode/encode facades were removed, and the root `ImageResult` functions are
the only public codec operations. Internal `Option` helpers remain
implementation details and cannot be named by downstream crates.

**Problem**

`pub mod codecs` exposes helpers returning `Option`, internal native WebP
types, and format implementation details.

**Approach**

- Make dispatcher and algorithm modules private.
- Re-export only deliberate public codec configuration or state types.
- Keep root detect/inspect/decode/encode functions canonical.
- If advanced callers need codec-specific state, design a stable public wrapper
  rather than exposing implementation modules.
- Convert any intentionally public fallible helper to `ImageResult`.

**Acceptance**

Malformed input cannot lose its error category merely because the caller chose
a format-specific entry point.

### C2. Replace stringly encoder options

**Problem**

`HashMap<String, String>` does not express valid formats, ranges, conflicts, or
metadata byte ownership. Irrelevant fields can be silently ignored.

**Approach**

Introduce typed non-exhaustive format options:

```rust,ignore
pub enum FormatEncodeOptions {
    Jpeg(JpegEncodeOptions),
    Png(PngEncodeOptions),
    Gif(GifEncodeOptions),
    Bmp(BmpEncodeOptions),
    Tiff(TiffEncodeOptions),
    WebP(WebPEncodeOptions),
    Ico(IcoEncodeOptions),
    Avif(AvifEncodeOptions),
}
```

Use small copied values for numeric options and borrowed byte slices for
metadata when ownership is unnecessary. Reject:

- options for the wrong format;
- values outside Pillow's accepted range;
- impossible option combinations;
- unknown keys;
- unsupported target behavior.

AVIF ordered advanced pairs can remain a typed ordered collection if Pillow
order and duplicate-key behavior are observable. Do not turn it into a map.

Migrate in two stages:

1. add typed validation behind the current dispatcher;
2. deprecate public construction of stringly extras after every manifest row
   has a typed equivalent.

**Acceptance**

Every accepted and rejected option combination has a fixture row with exact
output or exact structured error.

### C3. Capability discovery

**Problem**

Callers currently discover compiled capabilities by attempting an operation.

**Approach**

Add a compact value returned entirely from compile-time feature information:

```rust,ignore
pub struct FormatCapabilities {
    pub inspect: bool,
    pub decode: bool,
    pub encode: bool,
    pub decode_sequence: bool,
    pub encode_sequence: bool,
}
```

Expose either `ImageFormat::capabilities()` or a root `capabilities(format)`.
Do not include “native only”; a finished advertised capability must work on
WASM. If an implementation is unavailable on WASM, its capability is not ready
to ship.

ICO's transitive PNG/BMP relationship and every animation asymmetry must be
represented exactly.

**Acceptance**

The feature-matrix script derives its expectations from the same documented
capability table and proves them for native and WASM builds.

### C4. Byte and sample layout contract

**Problem**

Raw `Vec<u8>` fields are not self-explanatory for packed, indexed, 16-bit,
floating-point, and CMYK modes.

**Approach**

Document and validate:

- row-major ordering;
- row stride;
- channel order;
- packed `L1` bit order and row padding;
- `P8` palette index behavior;
- palette RGB and alpha lengths;
- endianness of 16-bit, integer, and float samples;
- exact relationship between `ImageMode` and `ColorType`;
- alignment guarantees, if any;
- zero-sized and maximum-sized image behavior.

Prefer slice accessors and checked constructors over public mutation of fields.
Use `bytemuck` only where its type and alignment contract is proved.

**Acceptance**

Every mode has a manifest fixture and a doctest showing how to interpret its
samples.

### C5. Error model

**Problem**

The structured error foundation is good, but future limits, metadata, options,
streaming, and format expansion need stable categories.

**Approach**

Use a non-exhaustive public enum or non-exhaustive variants for:

- unknown format;
- codec disabled;
- malformed input;
- unsupported operation/mode/option;
- invalid decoded buffer;
- resource limit exceeded;
- truncated incremental input;
- metadata rejected;
- output buffer too small;
- internal invariant violation.

Stable fields should include the format, operation, and limit kind where useful.
Do not place entire input buffers or secrets in errors. Implement
`std::error::Error` manually to preserve the dependency rule.

**Acceptance**

Every public error category has a manifest-driven fixture. Error tests compare
category and stable fields, not only display text.

## 7. Workstream D — Resource And Security Boundaries

### D1. Caller-controlled limits

**Problem**

Checked arithmetic prevents overflow but does not prevent a valid compressed
input from requesting unacceptable memory or work.

**Approach**

Add:

```rust,ignore
pub struct DecodeLimits {
    pub max_input_bytes: Option<u64>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub max_pixels: Option<u64>,
    pub max_output_bytes: Option<u64>,
    pub max_frames: Option<u32>,
    pub max_sequence_bytes: Option<u64>,
    pub max_metadata_bytes: Option<u64>,
    pub max_chunks: Option<u32>,
}

pub struct DecodeOptions {
    pub limits: DecodeLimits,
}
```

Use one internal budget object for all codecs. It must:

- validate input length immediately;
- charge dimensions during header parsing;
- charge allocations before reserving;
- charge every frame and accumulated sequence storage;
- charge decompressed metadata;
- avoid double-charging buffers that move rather than copy;
- return the same limit category in every codec.

Thread options through:

- `inspect_with_options`;
- `decode_with_options`;
- `decode_sequence_with_options`;
- lazy `EncodedImage` construction and materialization;
- future reader and streaming APIs.

Keep current convenience functions as documented-default wrappers.

**Default decision**

Use conservative finite defaults for public decode entry points. Provide an
explicit `DecodeLimits::unbounded()` only for trusted inputs. “Unbounded” still
uses checked arithmetic and container validity checks.

**Acceptance**

Each limit has fixtures for exactly-at-limit and one-over-limit behavior,
including malformed inputs that try to bypass the early check.

### D2. Work limits

**Problem**

Byte limits alone do not constrain pathological Huffman trees, excessive
chunks, nested containers, repeated frame updates, or decompression loops.

**Approach**

- Identify the dominant work unit for every codec.
- Charge bounded chunk/table/frame counts where a direct count exists.
- Add recursion-depth limits for recursive parsers.
- Prefer iterative parsing where practical.
- Ensure progress in every decode loop.
- Treat integer overflow while accounting as a limit violation or malformed
  input, never wrapping.

Avoid a misleading universal “CPU milliseconds” limit; deterministic unit
budgets are portable across native and WASM.

**Acceptance**

Adversarial fixtures terminate deterministically with the expected structured
error on native and WASM.

### D3. Security contract

**Approach**

Update `SECURITY.md` with:

- untrusted-input threat model;
- what limits do and do not guarantee;
- no filesystem access in the crate;
- no native codec loading;
- maximum supported dimensions and counts;
- panic policy;
- vulnerability reporting and supported versions.

**Acceptance**

No README claim says “fully validates” or “safe for arbitrary input” without
the corresponding bounded contract.

## 8. Workstream E — Metadata

### E1. Opaque preservation first

**Problem**

Pixel parity does not preserve ICC, EXIF, XMP, orientation, density, comments,
or container-specific application data.

**Approach**

Start with a byte-preserving envelope:

```rust,ignore
pub struct ImageMetadata {
    pub icc_profile: Option<Vec<u8>>,
    pub exif: Option<Vec<u8>>,
    pub xmp: Option<Vec<u8>>,
    pub orientation: Option<Orientation>,
    pub density: Option<PixelDensity>,
    pub entries: Vec<MetadataEntry>,
}
```

`MetadataEntry` should use a typed kind and bytes, not a string-to-string map.
Preserve order and duplicates where the container makes them observable.

Attach metadata to `ImageInfo` when inspection can retrieve it cheaply and to
decoded results when full parsing is needed. Keep source format separate.

### E2. Format-by-format implementation order

Implement one feature slice at a time:

1. PNG chunks: ICC, EXIF, XMP/text, gamma/chromaticity, density.
2. JPEG APP segments: ICC, EXIF, XMP, comments, density.
3. GIF comments and application extensions.
4. BMP/DIB profile and density fields.
5. WebP RIFF metadata chunks.
6. TIFF tag-backed opaque values and orientation.
7. ICO embedded-image metadata policy.
8. AVIF item/property metadata after the portable codec exists.

For every format, specify:

- whether decode preserves bytes exactly;
- whether encode reproduces bytes exactly;
- whether duplicate order is retained;
- whether orientation is reported or applied;
- which fields inspection returns without pixel decode;
- how metadata consumes the decode budget.

Orientation should be reported, not automatically applied. Applying it would
transform pixels and belongs downstream.

### E3. Metadata acceptance

Fixtures must include:

- absence;
- one entry;
- duplicate/order-sensitive entries;
- empty and maximum accepted values;
- truncated data;
- conflicting values;
- one-over-limit data;
- decode followed by encode preservation.

If Pillow normalizes or discards a field, record that observable behavior in
the manifest instead of assuming byte preservation.

## 9. Workstream F — Portable I/O And Incremental Codecs

### F1. Reader and writer adapters

**Approach**

Keep `&[u8]` and `Vec<u8>` canonical. Add dependency-free generic adapters only
where they provide value:

```rust,ignore
pub fn decode_reader<R: std::io::Read>(
    reader: R,
    options: &DecodeOptions,
) -> ImageResult<Decoded<DecodedImage>>;
```

An initial reader adapter may buffer into a limited `Vec<u8>`. It must document
that behavior and charge `max_input_bytes` while reading. A writer adapter can
write the already encoded vector and map I/O errors without changing codec
output.

Do not add path helpers. They are host policy and do not work uniformly on
WASM.

### F2. Incremental input

**Problem**

Buffering every source duplicates memory and prevents progressive network
consumption.

**Approach**

Design a small state machine API after limits are stable:

```rust,ignore
pub enum DecodeProgress {
    NeedMoreInput,
    Info(ImageInfo),
    Frame(DecodedFrame),
    Complete,
}
```

Rules:

- callers provide input chunks;
- the decoder reports consumed bytes;
- internal retained bytes count against the budget;
- `NeedMoreInput` is distinct from malformed end-of-stream;
- state is format-specific but the public lifecycle is uniform;
- no async runtime is required;
- the same state machine works in browser fetch streams.

Start with formats whose containers naturally support incremental parsing.
Do not force a fake streaming API over codecs that still require complete
random access; report the actual buffering contract.

### F3. Output sinks

After typed options are stable, allow encoders to target a caller-provided
portable sink. Use static dispatch over `std::io::Write` or a crate-owned small
sink trait. Measure code size before choosing a broad trait surface.

Encoded bytes must remain identical to `encode()` regardless of sink chunking.

## 10. Workstream G — Codec Abstractions And Extensibility

### G1. Repository-owned decoder and encoder traits

**Problem**

Generic codec consumers currently depend on root dispatch functions and cannot
hold format-specific state.

**Approach**

Design traits around codec needs, not image-rs compatibility:

```rust,ignore
pub trait ImageDecoder {
    fn format(&self) -> ImageFormat;
    fn info(&self) -> &ImageInfo;
    fn decode(self) -> ImageResult<DecodedImage>;
}

pub trait ImageEncoder {
    fn format(&self) -> ImageFormat;
    fn encode(
        &mut self,
        image: &DecodedImage,
        sink: &mut dyn ByteSink,
    ) -> ImageResult<()>;
}
```

The final signatures should prefer static dispatch in hot paths. Use dynamic
dispatch only where runtime codec selection requires it and after measuring
WASM size. Keep object safety deliberate.

Do not expose internal parser structs until their lifetime and buffering
contracts are stable.

### G2. Static registration only

Codec extensibility is fair game, but dynamic shared-library plugins violate
WASM portability.

Allowed model:

- compile-time registration;
- a static table of signatures and function pointers;
- caller-provided codec implementations linked into the same WASM module;
- no global mutation requirement;
- deterministic detection precedence.

Before adding registration, solve how external formats identify themselves
without making `ImageFormat` an unstable closed enum. Possible designs include
a built-in enum plus validated custom format identifier.

**Acceptance**

Registration works without `dlopen`, threads, filesystem access, inventory
crates, linker-section tricks, or target-specific behavior.

## 11. Workstream H — Interoperability Without Dependencies

### H1. Raw-layout boundary

**Approach**

Expose stable borrowed views:

- dimensions and row stride;
- mode and channel layout;
- immutable/mutable sample bytes where valid;
- typed `[u8]`, `[u16]`, and `[f32]` views when alignment and endianness permit;
- palette views;
- frame rectangles and timing.

Keep constructors checked. A foreign caller must not be able to create an
invalid `DecodedImage` by setting unrelated public fields.

### H2. `image` crate interoperability

A direct optional `image` dependency is prohibited. Interoperability should use
one of these external patterns:

- documented conversion code in the consuming application;
- a separately published companion crate outside the dependency-free core;
- stable raw-layout traits that another crate can implement.

The core repository may document mappings but must not claim lossless mapping
for Pillow modes that `image::DynamicImage` cannot represent, including packed
`L1`, indexed `P8`, and some integer/float/CMYK modes.

### H3. Serde and JavaScript

Do not add Serde or a binding-generator dependency. Prefer:

- plain Rust structs and enums;
- stable integer discriminants only where an FFI contract explicitly needs
  them;
- caller-owned serialization;
- a small manually written WASM harness for project verification;
- upstream `pillow-rs-js` bindings outside this crate.

The public Rust API must remain usable without JavaScript, and the WASM binary
must not require generated glue merely to execute codec algorithms.

## 12. Workstream I — Format Expansion

### I1. Oracle inventory

Pinned Pillow 12.2.0 currently registers these relevant codec families:

- registered for open and save: AVIF, BLP, BMP/DIB, BUFR, DDS, EPS, GIF, GRIB,
  HDF5, ICNS, ICO, IM, JPEG, JPEG 2000, MSP, PCX, PNG/PPM, QOI, SGI, Spider,
  TGA, TIFF, WebP, WMF, and XBM;
- registered for open only: CUR, DCX, FITS, FLI, FTEX, GBR, IPTC, MCIDAS,
  MPEG, PCD, PIXAR, PSD, Sun raster, XPM, XVThumb, and others;
- registered for save only: MPO, PALM, and PDF.

This registry is an inventory, not an automatic parity claim. Some Pillow
plugins are stubs or delegate to external interpreters/native libraries.
Registration alone is not proof that Pillow provides a usable oracle. Such
delegation cannot be copied into this crate.

### I2. Admission gate for a new format

A format enters implementation only after its design note records:

1. consumer need and supported operations;
2. authoritative specification;
3. Pillow support and exact oracle version;
4. whether Pillow itself requires an external component;
5. in-tree implementation feasibility;
6. WASM memory and code-size estimate;
7. license/provenance plan;
8. feature name and transitive feature needs;
9. mode, palette, alpha, animation, and metadata mapping;
10. malformed-input corpus;
11. manifest rows and structured errors;
12. encoder determinism policy.

If an external executable or library is necessary for correctness, the format
does not pass the gate until that functionality is implemented in-tree.

### I3. Recommended order

Order formats by useful coverage per implementation risk:

1. **PNM/PBM/PGM/PPM** — simple container family, broad mode fixtures, and a
   valuable parser/limits exercise. Admit PAM separately with a
   specification-based oracle if Pillow does not cover the required variant.
2. **QOI** — compact lossless still codec, Pillow read/write oracle, good WASM
   and exact-output slice.
3. **TGA** — common legacy still format with RLE and orientation cases.
4. **DDS** — container plus selected uncompressed/block modes; start with an
   explicit supported subset.
5. **Radiance HDR** — floating-point mode and run-length coverage; approve a
   non-Pillow oracle first.
6. **Farbfeld** — simple 16-bit format; Pillow is not the oracle, so approve a
   specification-based oracle first.
7. **OpenEXR** — high complexity and size; implement only after limits,
   metadata, and floating layouts are mature.
8. Other Pillow formats — demand-driven through the admission gate.

This order is not a promise to implement every format before release. It is
the order to use when format expansion begins.

### I4. AVIF portable port

AVIF is its own program because it combines:

- ISO BMFF item/track parsing;
- AV1 bitstream decoding;
- AV1 encoding;
- YUV/RGB conversion;
- alpha auxiliary images;
- still and sequence timing;
- metadata item/property handling.

Tackle it as separately testable layers:

1. in-tree bounded bit reader and arithmetic primitives;
2. ISO BMFF parser with item/property/reference fixtures;
3. AV1 sequence/frame header parsing;
4. scalar inverse transforms, prediction, filtering, and reconstruction;
5. color conversion and alpha composition;
6. still decode parity;
7. sequence decode parity;
8. deterministic encoder primitives;
9. still encode parity;
10. sequence encode parity;
11. metadata;
12. WASM size and memory reduction.

Use the pinned native stack only to generate intermediate oracle values during
development. Add reverse-mapped fixtures at the first divergence point, as was
done for JPEG and other codecs. Do not link the native stack into the final
crate.

AVIF is complete only after the identical manifest passes natively and through
the WASM behavior harness with no native installation.

## 13. Workstream J — Animation And Partial Decode

### J1. Streaming frames

The current eager `DecodedSequence` remains the simple ownership API. Add a
frame iterator/state machine only after limits are available.

The streaming contract must specify:

- whether frames are raw subrectangles or composited canvases;
- disposal and blend application;
- frame buffer lifetime;
- loop count and background;
- whether seeking is possible;
- retained compressed/input memory;
- total and per-frame budget charging.

GIF, WebP, and AVIF require independent manifest rows for raw frame state and
presentation output. Do not hide compositing transformations behind an
ambiguous `next_frame`.

### J2. Rectangular decode

Partial decode is in scope only as a codec operation. Implement it when a codec
can avoid decoding the complete image or when the API is necessary for tiled
data.

The method must return whether work was truly bounded to the requested region.
A convenience that decodes the whole image and then crops should not be called
partial decode.

## 14. Workstream K — Testing, Coverage, And Fuzzing

### K1. Manifest schema

Extend the generated fixture manifest so every row can express:

- operation;
- input bytes;
- selected or detected format;
- enabled feature;
- options;
- limits;
- expected info;
- expected mode and pixel bytes;
- expected palette;
- expected frames;
- expected metadata;
- expected encoded bytes;
- expected error category and stable fields;
- oracle name and version;
- native/WASM applicability, which should normally be identical.

Errors remain fixture-based. Do not add ad hoc tests that only manufacture a
prefix and assert `is_err()`.

### K2. Remove Rust dev-dependency pressure

The shipped library already keeps Serde out of normal dependencies, but the
strictest dependency interpretation should also reduce Cargo dev dependencies.

Generate a Rust fixture table or a simple project-owned binary manifest from
the Pillow/Python tooling, then include/read it without Serde. This allows
removing `serde` and `serde_json` from `Cargo.toml` while retaining
manifest-driven tests.

Generated artifacts must be deterministic and reviewable. The human-authored
source manifest remains the source of intent.

#### Accepted implementation

Use one test-only, project-owned strict JSON reader shared by the coverage and
feature-matrix integration targets:

- keep `manifest.yaml`, `tests/fixtures/coverage_matrix.json`, and the pinned
  AV1 JSON traces as the human-readable sources of truth;
- support the complete JSON grammar used by those files, including UTF-8
  strings, escapes and surrogate pairs, signed/unsigned integer conversion,
  arrays, objects, booleans, null, and trailing-input rejection;
- retain JSON numbers in their original spelling so encoder-option string
  forwarding remains byte-for-byte equivalent to `serde_json::Value`;
- use ordered object storage for deterministic AVIF advanced-option iteration;
- convert parsed values into the existing typed fixture structures and reject
  missing fields or incompatible types with contextual errors;
- share only the parser implementation, not codec behavior or expected values,
  between integration targets; and
- remove `serde`, `serde_json`, their derive toolchain, and the library-root
  dummy imports from Cargo after both manifests use the project-owned reader.

This reader is development infrastructure, not a public JSON API or a shipped
runtime dependency. It must not parse untrusted image data or move fixture
expectations into Rust source.

Acceptance requires an unchanged active-row count and exact parity result,
strict native and WASM Clippy, a dependency tree containing only the approved
`bytemuck`, and Coverage MCP at exact 100% line, branch, function, and region
coverage.

#### Accepted result

Completed on 2026-07-29:

- `tests/support/json.rs` provides the shared strict test-only JSON reader;
- the main coverage matrix, reduced-feature matrix, AV1 entropy trace, and AV1
  reconstruction trace all use typed project-owned conversion;
- all 1,030 active rows and both pinned AV1 documents retain their exact
  expectations without copying expected values into Rust source;
- `serde`, `serde_json`, their derive/proc-macro graph, and the crate-root dummy
  imports are removed;
- `cargo tree --all-features --locked` contains only
  `image-slash-star -> bytemuck`;
- `cargo package --list --allow-dirty` proves the test-only reader and fixture
  corpus are not shipped in the crate archive;
- strict native all-target/all-feature Clippy, AVIF-only WASM Clippy, and
  all-feature WASM Clippy pass; and
- Coverage MCP run `a5c7565e-18ad-4940-a8c5-6c21fad2f54c`, snapshot
  `ed12cf52-dd5e-478f-8d92-5a66ec6f2a0d`, passes all seven test binaries with
  35,673/35,673 lines, 5,300/5,300 branches, 1,792/1,792 functions, and
  59,138/59,138 regions.

### K3. WASM behavior harness

Compilation alone does not prove semantic compatibility.

Build a dependency-free harness that:

- compiles `image-slash-star` for `wasm32-unknown-unknown`;
- embeds a bounded representative fixture set;
- invokes detection, inspection, decode, encode, sequences, limits, and errors;
- exports a compact status and deterministic result digest;
- can be instantiated by a minimal JavaScript runner using the platform
  WebAssembly API;
- requires no binding generator or npm package.

Run the full native manifest and a size-conscious WASM subset on every change.
Run the complete WASM manifest in scheduled/release CI if CI duration or module
size makes it unsuitable for every commit.

The subset must cover every feature and public branch; it cannot be only a
smoke test.

### K4. Coverage

Use Coverage MCP only for coverage execution and analysis. Always collect:

- line coverage;
- branch coverage;
- region coverage.

Restore all three to 100% for each accepted slice. Coverage hooks must exercise
real reachable behavior or deliberately isolated invariants; do not keep dead
helpers solely to satisfy a percentage.

The WASM harness is a portability gate even when native instrumentation
produces the formal coverage numbers.

### K5. Fuzzing

Coverage and fixtures prove known behavior; fuzzing searches unknown input
space.

Create per-format harnesses for:

- detection;
- inspection;
- still decode;
- sequence decode;
- incremental state;
- decode under strict limits;
- encode arbitrary valid models;
- decode/encode round trips where byte parity is not the relevant invariant.

Seed with manifest inputs and minimized historical failures. The invariant is:

- no panic;
- no out-of-budget allocation;
- no infinite loop;
- no invalid decoded model;
- structured failure on rejected input.

Fuzzing tools remain external developer tools, not crate dependencies.

## 15. Workstream L — Performance And WASM Size

### L1. Establish baselines

Measure release builds before optimizing:

- native encode/decode throughput;
- native peak memory;
- WASM module size per isolated format feature;
- core module size with no formats;
- inspect versus decode;
- eager versus lazy decode;
- sequence peak memory;
- scalar versus any proposed SIMD path.

Use representative manifest fixtures plus larger public-domain images. Record
toolchain, flags, target, CPU/runtime, and exact commit.

### L2. Optimization rules

- Optimize only measured hot paths.
- Preserve exact output and errors.
- Prefer borrowing over cloning large buffers.
- Avoid intermediate collections.
- Keep decoder state bounded.
- Do not add a dependency for performance.
- Do not make WASM behavior a slower or less complete fallback.
- Every optimization with observable risk receives a fixture before merging.

### L3. Size budget

Track:

- raw `.wasm`;
- optimized `.wasm` if an external post-build optimizer is used;
- gzip and Brotli transfer sizes;
- native release library/binary examples;
- marginal size added by each format.

External optimizers can be release tooling, but correctness and basic
functionality must not depend on them.

## 16. Workstream M — CI, Packaging, And Supply Chain

### M1. Feature matrix

Required lanes:

- no default features;
- each individual format feature;
- default features;
- all portable features;
- native Rust target;
- `wasm32-unknown-unknown`;
- strict Clippy where the target supports it;
- rustdoc and doctests;
- package-consumer build from the generated `.crate`.

Every new feature must add itself to this matrix in the same change.

### M2. Cross-platform native checks

Add Linux, macOS, and Windows for portable default codecs. These do not replace
WASM; they catch assumptions about endianness, filesystem-independent build
scripts, integer widths, and compiler/linker behavior.

The library must not download or build native dependencies in CI.

### M3. License and provenance

Every copied or translated algorithm must record:

- upstream project and exact version/commit;
- source file or component;
- original license;
- local modification summary;
- applicable notice;
- compatibility with the crate's combined license expression.

Do this before copying code. Generated oracle fixtures should record their
source and generator rather than pretending to be independently authored
algorithm code.

### M4. Package cleanliness

The `.crate` should contain:

- source needed to build every advertised feature;
- required license and notice files;
- README and changelog;
- scripts required by the published source/provenance contract.

It should not contain:

- coverage output;
- `.coverage-mcp`;
- `.DS_Store`;
- oracle virtual environments;
- native build products;
- downloaded test datasets not required by the package;
- Git internals;
- temporary generated diagnostics.

## 17. Dependency Order

Implement in this order because later work depends on earlier contracts:

### Phase 0 — Make the constraints true

1. Update README and rustdoc scope. **Complete.**
2. Move processing behavior downstream and remove it from this repository.
   **Complete.**
3. Prune image-rs compatibility types not required by codecs. **Complete.**
4. Decide portable AVIF versus temporary withdrawal.
5. Remove native AVIF from the shipped dependency/build contract.
6. Add all-feature WASM compilation gates.
7. Define the dependency audit gate.

### Phase 1 — Stabilize the current codec surface

1. Hide implementation modules.
2. Finish structured public errors.
3. Document byte layouts.
4. Add capability discovery.
5. Add compiled examples and doctests.
6. Remove broad missing-doc allowances.

### Phase 2 — Secure untrusted decoding

1. Add `DecodeLimits` and `DecodeOptions`.
2. Thread budgets through inspect/decode/sequence/lazy decode.
3. Add deterministic work limits.
4. Add manifest error fixtures.
5. Add the WASM behavior harness.

### Phase 3 — Make encoding configuration explicit

1. Add typed per-format options.
2. Validate wrong-format and conflicting values.
3. Migrate metadata bytes out of string maps.
4. Deprecate stringly construction.

### Phase 4 — Preserve metadata

Implement PNG, JPEG, GIF, BMP, WebP, TIFF, ICO, then portable AVIF. Each format
is separately feature-gated and accepted before moving to the next.

### Phase 5 — Portable streaming and generic integration

1. Reader/writer adapters.
2. Incremental decoder lifecycle.
3. Streaming frames.
4. Encoder sinks.
5. Repository-owned codec traits.
6. Static registration if a real consumer requires it.

### Phase 6 — Assurance and performance

1. Remove Rust dev dependencies from the fixture consumer.
2. Add maintained fuzz harnesses.
3. Add cross-platform native lanes.
4. Add native/WASM performance and size tracking.
5. Add public API compatibility checks without adding a shipped dependency.

### Phase 7 — Format expansion

Implement admitted formats one at a time, beginning with PNM, QOI, and TGA.
Portable AVIF may move earlier because it corrects an existing advertised
feature.

## 18. Tracking Table

| ID | Deliverable | Depends on | Main proof |
| --- | --- | --- | --- |
| A1 | codec-only positioning | none | README external-user review |
| A2 | processing API migration | downstream replacements | compile + parity |
| A3 | portable/withdrawn AVIF | none | all-feature WASM |
| B1 | publishable crate | A3, B2-B4, D1 | package consumer |
| B2 | quick starts | C1-C5 | doctests/examples |
| B3 | stability policy | A1, A3 | documentation consistency |
| B4 | full rustdoc | C1-C5 | strict rustdoc |
| C1 | private internals | none | public API compile tests |
| C2 | typed options | C1, C5 | exact encode/error fixtures |
| C3 | capabilities | A3 | feature matrix |
| C4 | layout contract | none | per-mode fixtures |
| C5 | error model | none | error manifest |
| D1 | allocation limits | C5 | boundary fixtures |
| D2 | work limits | D1 | adversarial fixtures |
| E1-E3 | metadata | C2, D1 | round-trip fixtures |
| F1 | portable I/O adapters | D1 | byte-equivalence fixtures |
| F2-F3 | incremental I/O | D2, G1 | chunk-boundary fixtures |
| G1 | codec traits | C1, D1 | generic consumer |
| G2 | static registration | G1, C3 | WASM harness |
| H1-H3 | dependency-free interop | C4 | external consumer examples |
| I1-I4 | formats and AVIF | C2, D2, E1 | oracle manifests + WASM |
| J1-J2 | streaming/partial decode | D2, F2 | frame/region manifests |
| K1-K5 | test system | C5, D1 | Coverage MCP + fuzz |
| L1-L3 | performance/size | stable codecs | reproducible reports |
| M1-M4 | release engineering | all release slices | CI/package audit |

## 19. First Execution Sweep

The first sweep should not start by adding another codec. It should make the
current project contract coherent:

1. update README and crate docs to the codec-only/WASM/dependency boundary;
2. move every processing operation to `pillow-rs` and remove it here;
3. remove image-rs compatibility APIs not required by codec input/output;
4. audit every feature for `wasm32-unknown-unknown`;
5. decide whether AVIF is withdrawn temporarily or ported before release;
6. hide public implementation modules and eliminate public `Option` failures;
7. define `DecodeLimits`, the error additions, and fixture schema together;
8. generate dependency-free Rust fixture data to remove Rust dev dependencies;
9. build the native/WASM semantic harness;
10. run strict formatting, Clippy, feature matrix, package verification, and
   Coverage MCP line/branch/region coverage;
11. only then begin metadata and additional formats.

This order prevents new formats from copying unstable option, error, limit,
metadata, and portability contracts.

## 20. Final Product Test

The roadmap is successful when an external user can truthfully say:

> I can compile only the image formats I need, use the same byte API in native
> Rust and `wasm32-unknown-unknown`, constrain untrusted inputs, retain Pillow's
> observable mode/palette/frame/metadata model, and obtain fixture-proved exact
> results without installing a codec library or adding a dependency graph.

If a proposed feature weakens any part of that statement, it must be redesigned
before implementation.
