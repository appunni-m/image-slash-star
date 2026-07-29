# image-rs Comparison And External-Adoption Gap Analysis

Date: 2026-07-29

Status: review document only. This document records missing capabilities and
documentation; it does not make every image-rs feature part of this project's
scope.

Update after the accepted codec-boundary implementation:

- the README now includes dependency selection and a complete byte-to-byte
  transcode example;
- codec modules are private and the root structured API is canonical;
- the missing-documentation allowance is removed and strict rustdoc passes;
- every individual codec feature is compiled in both native and WASM Clippy
  lanes; and
- package verification currently produces 120 files, approximately 1.8 MiB
  unpacked and 379.9 KiB compressed.

Accepted implementation direction:
[codec-only productization plan](codec-only-productization-plan.md). That plan
supersedes earlier prioritization language in this review. Codec-related gaps
are fair game, but every shipped feature must work on
`wasm32-unknown-unknown`, no dependency may be added beyond the previously
approved `bytemuck` exception, and decoded-pixel processing remains out of
scope.

## Executive Conclusion

`image-slash-star` already has a credible technical reason to exist:

- exact observable compatibility with a pinned Pillow oracle;
- deterministic encoded bytes for the committed manifest;
- Rust-only default codecs with `bytemuck` as the sole runtime utility
  dependency;
- explicit per-format features;
- exact source-format, pixel-mode, palette, and animation-frame retention;
- structured feature/capability failures;
- immutable encoded-byte snapshots with shared lazy decoding.

Those are real differentiators from image-rs. The project should not erase
them by becoming a second general-purpose `image` crate.

The remaining weakness is release-level external adoption. A user can now
follow dependency selection and the canonical byte workflow, but the crate is
not published on crates.io, its `examples/` directory is empty, rustdoc has no
doctests, and important operational contracts such as caller-controlled
allocation limits and retained metadata are absent.

The right target is:

> A focused Pillow-parity codec backend with an excellent byte API, documented
> resource controls, mandatory WASM portability, and no dependency growth—not
> a clone of image-rs image processing.

## Comparison Baseline

The image-rs baseline is the published `image` crate version **0.25.10**:

- [crates.io package](https://crates.io/crates/image/0.25.10)
- [versioned rustdoc](https://docs.rs/image/0.25.10/image/)
- [image-rs repository](https://github.com/image-rs/image)
- [ImageReader documentation](https://docs.rs/image/0.25.10/image/struct.ImageReader.html)
- [decoder Limits documentation](https://docs.rs/image/0.25.10/image/struct.Limits.html)
- [0.25.10 release notes](https://docs.rs/crate/image/0.25.10/source/CHANGES.md)

Registry metadata on 2026-07-29 reports:

| Property | image 0.25.10 | image-slash-star 0.1.0 |
| --- | --- | --- |
| Published | crates.io and docs.rs | not published |
| Rust version | 1.88.0 | 1.96.1 |
| Default feature policy | `rayon` plus 15 format features | seven Rust codec features |
| Always-on runtime crates | `bytemuck`, `byteorder-lite`, `moxcms`, `num-traits` | `bytemuck` |
| License expression | MIT OR Apache-2.0 | combined multi-license distribution |
| Primary compatibility goal | general Rust image API | pinned Pillow observable parity |

Local evidence used for this review:

- `README.md`, `Cargo.toml`, `CHANGELOG.md`, and `SECURITY.md`;
- `src/lib.rs`, `src/source.rs`, `src/encode_options.rs`;
- `src/types/` and the public codec dispatcher;
- `manifest.yaml` and the generated fixture matrix;
- `cargo metadata --no-deps`;
- `cargo tree --edges normal`;
- strict rustdoc generation and doctest discovery;
- `cargo package --allow-dirty`;
- crates.io lookup with `cargo info` and `cargo search`.

The local package verifies successfully and contains 120 files: approximately
1.8 MiB unpacked and 379.9 KiB compressed. Strict rustdoc builds with warnings
denied and no missing-documentation suppression; the crate still discovers
zero doctests.

## Product Positioning

### What image-rs offers

image-rs presents itself as a general image encoding, decoding, buffer, and
basic processing library. Its README immediately shows path and in-memory
loading, saving, the central image types, codec traits, supported formats, and
processing operations.

### What image-slash-star should offer

This crate should present itself as a codec and compatibility backend:

- input and output are encoded or decoded bytes;
- callers own filesystem, network, and async I/O;
- source format and exact Pillow pixel mode are retained separately;
- the manifest defines the tested compatibility boundary;
- deterministic Pillow output is more important than matching image-rs output;
- every advertised format works without native libraries;
- every advertised format provides its documented behavior on
  `wasm32-unknown-unknown`.

The current native AVIF implementation is migration debt under this newer
constraint, not a permanent exception. It must be replaced by an in-tree
portable implementation or withdrawn from the published feature surface until
that work is complete.

The current README describes much of the implementation but does not state this
user contract soon or plainly enough.

## Capability Matrix

| Area | image 0.25.10 | image-slash-star today | Assessment |
| --- | --- | --- | --- |
| Install from registry | `image = "0.25"` | unavailable | release blocker |
| Hosted API docs | docs.rs, 100% documented | strict local rustdoc; not yet on docs.rs | release blocker |
| Copy-paste quick start | path, memory, save examples | dependency and byte-transcode example | complete for current API |
| Compiled examples | README/rustdoc and repository examples | empty `examples/` directory | release blocker |
| Path loading | `open`, `ImageReader::open` | caller uses `std::fs::read` | intentional host/WASM boundary |
| Reader input | `Read + Seek`, buffered reader API | complete `&[u8]` required | intentional core boundary |
| Writer output | `Write + Seek`, save/write helpers | returns `Vec<u8>` | intentional core boundary |
| Magic-byte detection | yes | yes | complete |
| Explicit format selection | reader and codec APIs | format dispatcher and encoders | complete |
| Header-only inspection | dimensions/decoder metadata | `inspect` and `ImageInfo` | complete for the current contract |
| Lazy materialization | reader constructs a decoder | shared `EncodedImage` decode cache | image-slash-star advantage |
| Source format retained after decode | generally separate from `DynamicImage` | `Decoded<T>::format` | image-slash-star advantage |
| Exact Pillow mode retained | no Pillow contract | `ImageMode`, including packed/indexed modes | image-slash-star advantage |
| Palette/index retention | codec-specific and often converted | `P8` indices plus RGB/alpha palette | image-slash-star advantage |
| Unified dynamic buffer | mature `DynamicImage` | intentionally absent | out of codec scope |
| Generic image traits | mature public traits | intentionally absent | out of codec scope |
| Decoder trait | `ImageDecoder`, rectangular decode | no public decoder trait | integration gap |
| Encoder trait | `ImageEncoder`, codec encoders | canonical root functions; private implementation helpers | trait remains an integration gap |
| Resource limits | width, height, and allocation limits | checked arithmetic but no caller policy API | high-priority safety gap |
| Partial/rectangular decode | `ImageDecoderRect` where supported | no | defer unless demanded |
| Format plugins | detection/decoder hooks and image-extras | closed `ImageFormat` dispatch | static WASM-safe extension gap |
| Structured errors | detailed category hierarchy | compact structured enum | good foundation; needs richer contracts |
| ICC/EXIF/orientation readback | decoder methods for supported codecs | pixels are tested, but no generic retained profile model | metadata gap |
| Metadata writing | selected typed codec methods | stringly `extra` values for some codecs | public-API gap |
| Animation iteration | `AnimationDecoder`/`Frames` | eager retained `DecodedSequence` | different valid design |
| Animation encode | codec-specific | GIF and AVIF multi-frame encode | focused strength |
| Image processing | broad basic `imageops` and `DynamicImage` methods | intentionally absent | codec boundary complete |
| Format coverage | 15 default format features | eight tracked formats | demand-driven gap |
| Parallel processing | optional/default Rayon | none | intentional dependency choice |
| Serde support | optional | none | external adapter only; dependency prohibited |
| Flat/FFI sample views | `FlatSamples` and sample layouts | raw decoded byte vectors | interoperability gap |
| Fuzzing | libFuzzer and AFL trees | no maintained fuzz target | assurance gap |
| Benchmarks | maintained benchmark tree | no public benchmark suite/results | evidence gap |
| Cross-platform native CI | mature project matrix | Ubuntu plus every isolated WASM feature compile lane | release-confidence gap |
| Byte-identical Pillow output | not a goal | manifest-enforced goal | primary differentiator |

## Format Coverage

### image 0.25.10

Default format features cover AVIF, BMP, DDS, OpenEXR, Farbfeld, GIF, HDR,
ICO, JPEG, PNG, PNM, QOI, TGA, TIFF, and WebP. The default `avif` feature
provides the Rust encoder; AVIF decoding requires the separate
`avif-native` feature. The default feature set also enables Rayon.

### image-slash-star

Default features cover JPEG, PNG, GIF, BMP, TIFF, WebP, and ICO. AVIF is
opt-in and uses pinned libavif, dav1d, and libaom versions for Pillow parity.
ICO encoding is deliberately source-sized: unlike Pillow's save convenience
and image-rs helpers, it does not generate an icon pyramid by resampling one
input. Multi-resolution ICO output therefore remains an API gap until callers
can provide independently prepared entries.

Formats present in image-rs but absent here:

- DDS;
- OpenEXR;
- Farbfeld;
- HDR/Radiance RGBE;
- PNM/PBM/PGM/PPM/PAM;
- QOI;
- TGA.

These formats are valid roadmap candidates, but each still requires an
admission review covering oracle support, in-tree implementation feasibility,
WASM size, licensing, and manifest cost. A format that requires a native
library or new crate dependency cannot ship until that functionality exists
in-tree.

### Animation differences

`image-slash-star` eagerly retains animation state:

- GIF, WebP, and AVIF sequence decode;
- GIF and AVIF multi-frame encode;
- frame rectangles, duration, disposal, loop count, and background state.

image-rs exposes iterator-oriented animation traits and frame types. A
streaming frame interface could reduce peak memory, but it would introduce a
different lifecycle contract. It is not required for the current persistent
Pillow backend and should be designed separately.

## Missing External-Adoption Documentation

### 1. Installation

The crate is not on crates.io. `cargo search image-slash-star` returns no
published package. Until publication, the README must show an HTTPS Git
dependency pinned to a commit:

```toml
[dependencies]
image-slash-star = {
    git = "https://github.com/appunni-m/image-slash-star",
    rev = "<published-commit>"
}
```

After publication:

```toml
[dependencies]
image-slash-star = "0.1"
```

The README should also show a minimal feature selection:

```toml
[dependencies]
image-slash-star = {
    version = "0.1",
    default-features = false,
    features = ["jpeg", "png"]
}
```

### 2. One complete program

There must be a compiled example that reads bytes, detects and decodes them,
prints format/mode/dimensions, and writes a different format:

```rust
use image_slash_star::{ImageFormat, decode, encode_default};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let encoded = std::fs::read("input.jpg")?;
    let decoded = decode(&encoded)?;

    println!(
        "{} {}x{} {:?}",
        decoded.format.as_str(),
        decoded.content.width,
        decoded.content.height,
        decoded.content.mode,
    );

    let png = encode_default(&decoded.content, ImageFormat::Png)?;
    std::fs::write("output.png", png)?;
    Ok(())
}
```

This should exist as both a README example and `examples/transcode.rs`, with a
CI compile/run lane using committed input.

### 3. Byte and pixel layout contract

The README needs to state:

- pixels are stored row-major;
- `ImageMode`, not only `ColorType`, defines the exact byte/sample layout;
- packed `L1`, indexed `P8`, integer, float, and CMYK modes do not all map to
  `DynamicImage`;
- palette RGB and alpha live in `ImagePalette`;
- source `ImageFormat` belongs to `Decoded<T>`, not `DecodedImage`;
- dimensions and buffer lengths must pass `validate`;
- endianness for multi-byte integer and float modes.

The last item is particularly important: a raw `Vec<u8>` without an endianness
contract is not sufficient documentation for an interoperable public type.

### 4. Scope of the parity promise

The statement “exact Pillow parity across public image behavior” is broader
than the evidence. The manifest proves its committed rows, not every possible
Pillow image, metadata combination, malformed input, or option.

The README should promise:

> Exact parity for the inputs, modes, options, errors, and outputs represented
> by the committed manifest.

It should then link directly to the manifest and explain how unsupported
combinations fail.

### 5. Stability and support

External users need:

- an explicit pre-1.0 API stability statement;
- MSRV 1.96.1 displayed near installation;
- supported native targets;
- supported WASM targets and AVIF behavior there;
- a release/version policy;
- whether exact encoded bytes are stable across crate patch releases;
- whether fixture additions may intentionally change previously untracked
  behavior.

### 6. “Use this / do not use this” guidance

Use this crate when:

- Pillow-compatible pixels or deterministic encoded bytes matter;
- a WASM-portable Rust codec backend with no dependency growth is needed;
- an application wants to compile only selected formats;
- source format, packed modes, palettes, or frame presentation data must be
  retained.

Do not choose it solely as an image-rs replacement when:

- path, reader, writer, async, or streaming APIs are required;
- processing operations are the primary need;
- an absent format is required;
- the application depends on the image-rs decoder/encoder traits;
- a stable crates.io release is mandatory.

## Missing Public API Contracts

### 1. Resource limits

This is the most important functional gap for external decoding.

Checked arithmetic prevents many overflows, but it does not let an application
set policy such as:

- maximum width and height;
- maximum decoded pixel bytes;
- maximum frame count;
- maximum total sequence bytes;
- maximum palette/chunk/metadata bytes;
- maximum compressed input bytes;
- maximum work or recursion for adversarial streams.

image-rs exposes `Limits`, including strict width/height constraints and a
best-effort allocation budget. Its own release notes still warn about malicious
input, so its API is not proof of complete safety; it is nevertheless a useful
caller-controlled boundary that this crate lacks.

Recommended direction:

```rust,ignore
pub struct DecodeLimits {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub max_pixels: Option<u64>,
    pub max_output_bytes: Option<u64>,
    pub max_frames: Option<u32>,
    pub max_sequence_bytes: Option<u64>,
}
```

Limits must be enforced before large allocations and threaded through inspect,
still decode, sequence decode, and lazy `EncodedImage` materialization.
Defaults and strictness must be explicit.

### 2. Consistent public codec errors

Resolved. The root API returns structured `ImageResult`; format-specific
implementation modules and their `Option` helpers are private. The README
documents one public path for each codec operation.

### 3. Typed encode configuration

`EncodeOptions` combines global optional fields with:

```text
HashMap<String, String>
```

for codec-specific behavior. This creates several undocumented questions:

- which options apply to which formats;
- whether irrelevant options are ignored or rejected;
- accepted ranges and aliases;
- whether duplicate/order-sensitive values are possible;
- whether a key is stable API;
- why some metadata uses hex strings instead of bytes.

image-rs exposes format-specific encoder types for detailed configuration. This
crate does not need to copy those types, but should replace stringly public
configuration with one of:

- typed per-format option structs;
- a non-exhaustive typed option enum;
- explicit validated builder methods over the current representation.

`advanced` can remain ordered for AVIF, but its ownership and validation rules
must be documented.

### 4. Reader and writer adapters

The byte-only core is a valid intentional boundary. The README must say so.

Small `std` adapters would still improve adoption without moving codec logic:

- `decode_reader(Read)` if buffering the complete stream is acceptable;
- writer forms that avoid one extra caller-visible copy where possible.

Path helpers should remain outside the crate because they are not meaningful
across all WASM hosts. Publish a native wrapper example using
`std::fs::{read, write}` and a portable adapter example using
`std::io::Read::read_to_end`.

### 5. Metadata model

`ImageInfo` currently retains:

- format;
- dimensions;
- mode and bit depth;
- palette;
- animation flag and optional frame count.

The generic decoded model does not expose a stable readback contract for:

- ICC profiles;
- EXIF bytes;
- XMP;
- orientation;
- density/DPI;
- gamma/chromaticities;
- textual chunks/comments;
- container-specific metadata.

Some encoders accept metadata through string keys, and fixtures prove that
metadata-bearing files decode to correct pixels, but that is not metadata
retention.

Define whether metadata preservation is a goal. If yes, add an opaque
byte-preserving metadata envelope before adding high-level parsing. If no,
state clearly that metadata may be discarded.

### 6. Decoder and encoder abstractions

image-rs has `ImageDecoder`, `ImageDecoderRect`, and `ImageEncoder`. These enable
generic codec consumers and codec-specific control.

This crate has high-level dispatch functions but no equivalent trait contract.
Possible directions:

- keep the simpler closed API and document it;
- add repository-owned traits for generic backend use;
- expose dependency-free layout/conversion contracts that a separate companion
  crate can adapt to image-rs traits.

Do not add an optional or always-on `image` dependency merely to claim
compatibility; either would conflict with the dependency goal.

### 7. Capability introspection

Applications cannot currently ask whether a format is:

- compiled in;
- readable;
- writable;
- sequence-readable;
- sequence-writable;
- available in the selected feature set.

Structured failures are good, but static capability methods would improve UI
and negotiation:

```rust,ignore
ImageFormat::reading_enabled()
ImageFormat::writing_enabled()
ImageFormat::sequence_reading_enabled()
ImageFormat::sequence_writing_enabled()
```

This must account for portable AVIF readiness and ICO's transitive PNG/BMP
features. A shipped capability must not change merely because the target is
WASM.

### 8. Interoperability and raw layouts

Missing or incomplete compared with image-rs:

- flat sample/matrix views;
- `SubImage`;
- rectangular decoding;
- broad `From`/`TryFrom` conversions;
- public traits for foreign buffers;
- external serialization adapters over dependency-free layouts;
- explicit zero-copy/bytemuck views where layouts permit;
- documented conversion mappings for an external image-rs adapter.

Only flat-layout and conversion contracts are directly relevant to a codec
backend. Subimages and processing-oriented views can remain downstream work.

### 9. Documentation completeness

The crate-wide missing-documentation allowance has been removed. Strict
all-feature rustdoc now proves the current reachable public API is documented.
Further feature-specific semantics and compiled examples remain useful, but
they no longer hide behind a lint suppression.

Required release gate:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo test --all-features --doc
```

Remove the broad allowance and fix missing documentation rather than moving it
to another scope.

## Functionality Present In image-rs But Not Required Here

The following remain intentional non-goals:

- a broad image processing module;
- blur, resize, resampling, color adjustment, convolution, and compositing;
- a mutable path-oriented `DynamicImage` application API;
- matching image-rs encoded output.

Additional formats, portable codec abstractions, static registration, and
in-tree optimization are in scope when they pass the WASM, dependency, oracle,
and maintenance gates. Dynamic native plugins and dependency-based
interoperability are not.

`pillow-rs` already owns higher-level image operations. Duplicating those
operations in this codec crate would create two parity surfaces and undermine
the migration architecture.

The former `DynamicImage` and image-buffer compatibility layer was audited and
removed on 2026-07-29 because no codec path required it. Processing behavior
belongs downstream. `pillow-rs/src/raster/` now owns the materialized buffer,
pixel, view, conversion, and transform layer, while its feature-gated codec
calls continue to use this crate. The remaining decoded-sample model does not
claim to match the image crate API.

## Assurance And Maintenance Gaps

### Fuzzing

image-rs keeps libFuzzer and AFL targets. This repository has a large
fixture-oracle matrix and exact coverage, but no maintained fuzz harness.
Coverage and fuzzing answer different questions.

Add format-specific fuzz targets for:

- detection and inspection;
- still decode;
- sequence decode;
- decode followed by validate;
- encode for arbitrary valid `DecodedImage` models;
- decoder limit enforcement.

Seed corpora should include the committed Pillow fixtures. Fuzzing must assert
no panic, no excessive allocation under configured limits, and structured
failure for invalid input.

### Cross-platform CI

Current native CI is Ubuntu-based, with a core WASM target compile matrix.
Before a broad release, add:

- macOS native default-feature checks;
- Windows native default-feature checks;
- explicit supported WASM checks;
- package verification from the produced `.crate`;
- rustdoc and doctest lanes;
- an MSRV lane if 1.96.1 is intended as a minimum rather than merely the
  development toolchain.

The pinned native AVIF environment may remain an oracle while the portable
implementation is developed, but it cannot be a build or runtime requirement
of the published crate. Every advertised codec must compile and execute its
documented contract on WASM.

### Benchmarks

The project makes dependency and WASM-size claims but provides no maintained
native decode/encode benchmark comparison.

Add benchmarks only for decisions they can guide:

- throughput and peak memory per format;
- inspect versus full decode;
- eager `decode` versus cached `EncodedImage`;
- sequence peak memory;
- feature-specific binary size;
- comparison with Pillow and image-rs on the same source images.

Performance need not match image-rs to release, but external users need to know
the intended tradeoff for exact Pillow bytes.

### API stability and compatibility tests

Before publishing:

- record the initial public API with `cargo-semver-checks` or equivalent;
- compile the README/example as a consumer crate;
- test no features, every individual feature, defaults, and all features from
  the packaged `.crate`;
- decide which output bytes are semver-stable;
- define how manifest expansion affects compatibility claims.

## Prioritized Missing-Work Register

### P0 — Required before presenting this as an externally consumable crate

1. Push the reviewed commits so documented Git revisions are reachable.
2. Rewrite the README around installation and a compiled five-minute example.
3. Add `examples/transcode.rs` and at least one real doctest.
4. Publish 0.1.0 to crates.io and confirm docs.rs builds all intended features.
5. State pre-1.0 stability, MSRV, supported targets, and byte-stability policy.
6. Narrow the Pillow-parity promise to the committed manifest contract.
7. Document the byte/mode/palette/sequence model completely.
8. **Complete:** remove `allow(missing_docs)` and make strict rustdoc meaningful.
9. **Complete:** make codec modules internal and remove their public `Option`
   error surface.
10. Add caller-controlled decode/sequence resource limits.

### P1 — High-value integration work

1. Replace stringly codec options with typed, validated configuration.
2. Define a stable opaque metadata preservation model or declare metadata loss.
3. Add format capability introspection.
4. Add byte-reader/writer examples or thin adapters.
5. Add dependency-free raw-layout contracts for external image-rs adapters.
6. Add maintained fuzz targets seeded by parity fixtures.
7. Add macOS and Windows default-codec CI.
8. Add targeted performance and memory measurements.

### P2 — Codec extensions after the portability and safety foundation

1. Streaming frame decode.
2. Rectangular/partial decode.
3. Flat sample and foreign-buffer views.
4. Dependency-free external serialization/layout adapters.
5. Additional formats requested by real consumers.
6. WASM-compatible static codec registration hooks.

### Explicit non-goals

1. Reimplement image-rs image processing.
2. Add Rayon, a native library, or any dependency beyond the approved
   `bytemuck` exception.
3. Match image-rs encoded bytes.
4. Move Pillow-level operations out of `pillow-rs`.
5. Add a format without an in-tree WASM implementation and an approved oracle.

## Recommended First External-Release Slice

The smallest credible release slice is:

1. push current `main`;
2. add the installation, scope, and quick-start sections to the README;
3. add a compiled `examples/transcode.rs`;
4. document all root entry points, public model types, and option semantics;
5. make codec implementation modules internal unless explicitly supported;
6. add a conservative default `DecodeLimits` API;
7. run package-consumer tests against the generated `.crate`;
8. publish 0.1.0 and confirm docs.rs;
9. link crates.io and docs.rs badges from the README.

After that slice, an unfamiliar user can answer all essential questions:

- Why would I choose this instead of image-rs?
- How do I install it?
- How do I decode, inspect, and encode?
- What exact bytes and modes do I receive?
- Which formats and targets are enabled?
- How do I constrain untrusted input?
- What compatibility does the project actually promise?
