# image-slash-star Backend Migration Spec

Status: active migration; Phases 1 and 2 complete.

This spec describes the intended migration from local codec logic in
`pillow-rs` to `image-slash-star` as the shared codec and decoded-buffer
backend for `pillow-rs` and `pillow-rs-js`.

The API direction is Rust-specific. The internal design should not be shaped by
Pillow compatibility requirements. Pillow-like naming or compatibility methods
may exist at binding or adapter layers, but the core crate contracts should use
typed Rust concepts, explicit errors, and clear ownership boundaries.

## Goal

Use `~/work/image-slash-star` as the image codec, metadata, and typed decoded
buffer backend for `pillow-rs` and `pillow-rs-js`.

The migration should remove duplicated codec logic from `pillow-rs` while
keeping image manipulation semantics and operation execution in `pillow-rs`.

## Reviewed Worktree Findings

The July 2026 migration worktrees are design prototypes, not merge sources.
They predate the current AVIF implementation and package rename, and applying
them wholesale would regress AVIF decoding, sequence support, and compatible
brand detection. Their useful ideas are structured codec errors, explicit
feature forwarding, generic decoded-buffer conversion, palette-alpha
preservation, metadata caching, and persistent materialization.

The following parts must be redesigned rather than copied:

- metadata parsers must live under each feature-gated codec, not one module
  that pulls every parser into small builds
- ICO inspection must select the same best entry as decoding
- TIFF inspection must not claim a single frame after examining only one IFD
- loaded Pillow storage must retain mode, source format, metadata, palette, and
  palette alpha
- palette operations must be proven index-preserving before avoiding color
  expansion
- all behavior must be exercised by manifest fixtures and exact Pillow-oracle
  output before replacing an existing path

Each migration slice is accepted only when it preserves current AVIF behavior,
passes the relevant feature builds, matches active manifest references exactly,
and restores 100% line, branch, function, and region coverage through Coverage
MCP.

## Domain Boundary

### `image-slash-star`

`image-slash-star` is the codec-domain crate.

It owns:

- encoded image format detection
- metadata inspection
- image header parsing
- codec feature dispatch
- decode from encoded bytes into validated decoded image types
- encode from decoded image types into encoded bytes
- sequence decode and encode where supported
- codec-visible metadata:
  - color mode
  - bit depth
  - palette RGB
  - palette alpha
  - frame timing
  - frame disposal metadata
  - animation/container metadata

It must not:

- execute `pillow-rs` image operations
- know about `PipelineOp`
- choose eager versus lazy loading policy
- choose CPU, SIMD, parallel, or GPU execution backends
- encode Python, JS, or Pillow compatibility semantics into the core API

### `pillow-rs`

`pillow-rs` is the image manipulation crate.

It owns:

- Rust-native `Image` API
- lazy versus eager loading policy
- image operation construction
- deferred operation pipelines
- materialization strategy
- CPU/SIMD/parallel/GPU operation execution
- conversion from codec-domain decoded buffers into operation-ready storage
- user-facing error mapping
- save/load behavior at the library level
- compatibility adapters where they are intentionally exposed

`pillow-rs` may expose Pillow-like convenience methods, but those should wrap a
typed Rust core rather than define it.

### `pillow-rs-js`

`pillow-rs-js` is a binding and packaging layer.

It owns:

- WASM packaging
- JS-friendly wrappers
- selected codec feature set
- browser/runtime-specific API ergonomics
- release-size policy

It must not contain codec logic.

## Current Reality

### `image-slash-star`

The Cargo package is named `image-slash-star`, so downstream Rust imports use
Cargo's underscore-normalized crate name:

```rust
image_slash_star
```

Its centralized codec APIs are being migrated to this canonical contract:

```rust
pub fn detect_format(data: &[u8]) -> ImageResult<ImageFormat>;
pub fn decode(data: &[u8]) -> ImageResult<Decoded<DecodedImage>>;
pub fn decode_sequence(data: &[u8]) -> ImageResult<Decoded<DecodedSequence>>;
pub fn encode(
    img: &DecodedImage,
    format: ImageFormat,
    opts: &EncodeOptions,
) -> ImageResult<Vec<u8>>;
pub fn encode_sequence(
    sequence: &DecodedSequence,
    format: ImageFormat,
    opts: &EncodeOptions,
) -> ImageResult<Vec<u8>>;
pub fn encode_default(img: &DecodedImage, format: ImageFormat) -> ImageResult<Vec<u8>>;
```

Current features:

```toml
default = ["jpeg", "png", "gif", "bmp", "tiff", "webp", "ico"]
jpeg = []
png = []
gif = []
bmp = []
tiff = []
webp = []
ico = ["bmp", "png"]
avif = []
```

Current shared types include:

- `DecodedImage`
- `DecodedSequence`
- `DynamicImage`
- `ImageMode`
- `ImagePalette`

`DynamicImage` is already represented as an enum over concrete pixel buffers:

- `Luma8`
- `LumaA8`
- `Rgb8`
- `Rgba8`
- `Luma16`
- `LumaA16`
- `Rgb16`
- `Rgba16`
- `Rgb32F`
- `Rgba32F`

`DynamicImage::from_decoded` exists, but it only accepts decoded images whose
mode/color can be represented by those variants. It rejects paletted `P8` and
other modes that require side-channel state.

PNG decode already preserves indexed PNG as `ImageMode::P8` with
`ImagePalette` from `PLTE` and `tRNS`. That representation should become the
common contract for all indexed codecs.

Implemented today:

- canonical `Result` APIs and structured `ImageError` variants
- automatic decode envelopes retaining the detected `ImageFormat`
- exact `DecodedImage` mode, color type, pixels, palette RGB, and palette alpha
- metadata-only `ImageInfo` and feature-gated `inspect`
- header-only PNG, JPEG, GIF, BMP, WebP, TIFF, and ICO inspection
- native-parser AVIF inspection
- manifest-driven Pillow-oracle assertions for automatic detection, decoded
  format/mode, metadata, successful output, and structured errors

Still missing:

- complete paletted preservation across every naturally indexed codec
- narrow default features for WASM-oriented consumers

### `pillow-rs`

`pillow-rs::Image` currently owns lazy loading and deferred operations:

```rust
pub enum Image {
    Loaded(DynamicImage, Option<String>),
    Paletted(PalettedData),
    Path {
        path: PathBuf,
        format: Option<ImageFormat>,
        is_paletted: bool,
    },
    Bytes {
        data: Arc<Vec<u8>>,
        format: Option<ImageFormat>,
        is_paletted: bool,
    },
    Pipeline {
        source: Arc<Image>,
        ops: Vec<PipelineOp>,
        format: Option<ImageFormat>,
        explicit_mode: Option<String>,
        backend: Option<crate::compute::Backend>,
        palette: Option<Vec<u8>>,
    },
}
```

`materialize()` is the execution boundary.

Current behavior:

- lazy sources decode through `image_slash_star::decode`
- decoded images are converted through `DynamicImage::from_decoded`
- operation pipelines execute in `pillow-rs`
- `Image::open` and `Image::open_bytes` eagerly probe PNG palette state using
  the direct `png` crate
- `Image::load(&self)` validates materialization but does not cache pixels
- `size()` and much of `mode()` materialize because metadata is not cached
- `PalettedData` stores palette RGB but not palette alpha
- `apply_transparency()` expands palettes with alpha `255` instead of using
  codec-provided palette alpha
- sequence APIs are not integrated with `DecodedSequence`

Current feature forwarding is incomplete:

```toml
image-codecs-all = ["image-gif", "image-png", "image-tiff", "image-webp"]
image-gif = ["image-slash-star/gif"]
image-png = ["image-slash-star/png"]
image-tiff = ["image-slash-star/tiff"]
image-webp = ["image-slash-star/webp"]
```

Missing forwarded features:

- `jpeg`
- `bmp`
- `ico`
- `avif`

`pillow-rs` still depends directly on:

```toml
png = "0.18"
```

That dependency should disappear once generic palette decode and metadata
support come from `image-slash-star`.

### `pillow-rs-js`

`pillow-rs-js` currently depends on `pillow-rs` with only PNG enabled:

```toml
pillow-rs = {
    path = "../pillow-rs",
    default-features = false,
    features = ["image-png"]
}
```

Non-PNG image support should not be assumed in JS/WASM until the feature set is
expanded intentionally.

Logging/debug dependencies currently include:

```toml
log = "0.4"
console_error_panic_hook = { version = "0.1", optional = true }
console_log = { version = "1.0", optional = true }
```

The actual WASM size impact of logging has not been measured.

## Design Direction

### Keep The Current `Image` Enum Initially

Do not replace the current `Image` representation immediately.

The first migration should be narrow:

1. strengthen `image-slash-star`
2. route all decode/encode/inspect through it
3. convert decoded images correctly into current `pillow-rs` image variants
4. remove direct codec dependencies from `pillow-rs`
5. make `load` persistent
6. add richer storage only after the current enum proves insufficient

### Use Rust-Specific Core APIs

The core API should prefer:

- typed metadata
- explicit errors
- feature-gated codecs
- clear ownership
- immutable inspection APIs
- mutating materialization APIs
- explicit operation pipelines
- explicit conversion boundaries

Do not make string mode names or Pillow behavior the internal contract.

Preferred Rust-native concepts include:

- `ImageMode`
- `ImageFormat`
- `ImageInfo`
- `ColorType`
- `BitDepth`
- `ImagePalette`
- `PaletteInfo`

Compatibility naming can be added at crate edges when needed.

## Proposed `image-slash-star` Contract

### Result APIs

The canonical APIs return structured `Result` values directly. This is an
intentional pre-1.0 breaking cleanup; permanent `try_*` duplicates and `Option`
compatibility wrappers are not retained.

```rust
pub type ImageResult<T> = Result<T, ImageError>;

pub fn detect_format(data: &[u8]) -> ImageResult<ImageFormat>;

pub fn decode(data: &[u8]) -> ImageResult<Decoded<DecodedImage>>;

pub fn decode_sequence(data: &[u8]) -> ImageResult<Decoded<DecodedSequence>>;

pub fn encode(
    image: &DecodedImage,
    format: ImageFormat,
    options: &EncodeOptions,
) -> ImageResult<Vec<u8>>;

pub fn encode_sequence(
    sequence: &DecodedSequence,
    format: ImageFormat,
    options: &EncodeOptions,
) -> ImageResult<Vec<u8>>;
```

Container format and decoded sample mode are different properties. Automatic
decode retains both through an envelope:

```rust
pub struct Decoded<T> {
    pub format: ImageFormat,
    pub content: T,
}
```

`DecodedImage` remains the format-independent pixel value and retains exact
`ImageMode`, `ColorType`, pixels, palette RGB, and palette alpha. Programmatic
images therefore do not need a fictitious source format. Encoding keeps an
explicit target `ImageFormat`; pixels alone cannot determine whether callers
want PNG, JPEG, WebP, or another container.

### Error Type

Use a non-exhaustive Rust error enum:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImageError {
    UnknownFormat,

    FeatureDisabled {
        format: ImageFormat,
        feature: &'static str,
    },

    Malformed {
        format: ImageFormat,
        message: String,
    },

    Unsupported {
        format: Option<ImageFormat>,
        message: String,
    },

    Dimensions,

    Parameter(String),

    IoError(String),
}
```

Semantics:

- `UnknownFormat`: no supported signature/container match was found
- `FeatureDisabled`: support exists, but the required Cargo feature is disabled
- `Malformed`: the encoded bytes are invalid
- `Unsupported`: the encoded bytes may be valid, but a mode/variant is not
  implemented
- `Dimensions`: dimensions overflow or violate decoded-buffer invariants
- `Parameter`: caller-provided encode/decode options are invalid
- `IoError`: file or stream access failed

### Metadata Inspection

Add metadata-only inspection:

```rust
pub fn inspect(data: &[u8]) -> ImageResult<ImageInfo>;
```

Proposed type:

```rust
#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    pub mode: Option<ImageMode>,
    pub bit_depth: Option<u8>,
    pub has_palette: bool,
    pub palette: Option<ImagePalette>,
    pub is_animated: bool,
    pub frame_count: Option<u32>,
}
```

Stable requirements:

- dimensions should be available without full pixel decode for common formats
- mode should be available when headers define it
- palette tables should be available when stored separately
- animation metadata should be available when a cheap container scan can provide
  it

Expected no-full-decode behavior:

- PNG: parse `IHDR`, `PLTE`, `tRNS`; no `IDAT` decompression
- GIF: parse logical screen, color tables, extensions, frame descriptors; no
  LZW decode
- BMP: parse DIB header and color table; no pixel decode
- JPEG: parse SOF dimensions/components; no entropy decode
- WebP: parse `VP8`, `VP8L`, `VP8X`, and animation chunks where practical
- TIFF/ICO: inspect directory/container metadata where practical

Reader/path variants can be added later if path-based inspection should avoid
reading entire files:

```rust
pub fn inspect_reader<R: Read + Seek>(reader: R) -> ImageResult<ImageInfo>;
pub fn inspect_path(path: impl AsRef<Path>) -> ImageResult<ImageInfo>;
```

### Paletted Decode Contract

All naturally indexed formats should decode to:

```text
mode = ImageMode::P8
pixels = one byte per palette index
palette.rgb = RGB triples
palette.alpha = optional alpha values
```

This applies to:

- PNG
- GIF
- BMP
- TIFF
- ICO when relevant
- future indexed sources

`image-slash-star` should not silently expand indexed input to RGB when palette
preservation is possible.

Palette alpha should be normalized:

- `None` means no alpha table or all entries fully opaque
- `Some(Vec<u8>)` contains one alpha byte per palette entry

Partial alpha tables, such as PNG `tRNS`, should be expanded to full palette
length with missing entries treated as `255`.

### Decoded Buffer Validation

Decoded image constructors or validation helpers should guarantee:

- dimensions are valid for the format and buffer shape
- pixel length matches width, height, mode, and bit depth
- palette index buffers are valid for the declared mode
- palette RGB length is valid
- palette alpha length matches palette length when present
- sequence frames have valid dimensions and timing metadata

Suggested helper:

```rust
impl DecodedImage {
    pub fn validate(&self) -> ImageResult<()>;
}
```

### Feature Contract

Long-term feature shape:

```toml
[features]
default = []
png = []
jpeg = []
gif = []
bmp = []
tiff = []
webp = []
ico = ["bmp", "png"]
avif = []
all = ["png", "jpeg", "gif", "bmp", "tiff", "webp", "ico", "avif"]
```

During migration, downstream crates should use:

```toml
default-features = false
```

Do not rely on broad codec defaults for WASM.

## Proposed `pillow-rs` Changes

### Dependency And Feature Wiring

Point `image-slash-star` at `~/work/image-slash-star` during local migration:

```toml
image-slash-star = {
    path = "../image-slash-star",
    default-features = false
}
```

Forward every supported codec feature:

```toml
image-codecs-all = [
    "image-jpeg",
    "image-png",
    "image-gif",
    "image-bmp",
    "image-tiff",
    "image-webp",
    "image-ico",
]

image-jpeg = ["image-slash-star/jpeg"]
image-png = ["image-slash-star/png"]
image-gif = ["image-slash-star/gif"]
image-bmp = ["image-slash-star/bmp"]
image-tiff = ["image-slash-star/tiff"]
image-webp = ["image-slash-star/webp"]
image-ico = ["image-slash-star/ico"]
image-avif = ["image-slash-star/avif"]
```

Recommendation: keep AVIF explicit opt-in until implementation maturity,
binary size, and platform behavior are known.

### Generic Decoded Conversion

Add one conversion boundary from codec-domain data into `pillow-rs` image-domain
data:

```rust
fn image_from_decoded(
    decoded: image_slash_star::DecodedImage,
    format: Option<ImageFormat>,
) -> Result<Image, ImageError>;
```

Required behavior:

```text
DynamicImage-compatible modes
-> Image::Loaded(dynamic, metadata)

P8 with palette
-> Image::Paletted(PalettedData)

P8 without palette
-> error

Unsupported modes
-> explicit unsupported error
```

This should replace format-specific decode logic in `pillow-rs`.

The target flow for every still-image codec is:

```text
encoded bytes
-> image-slash-star decode
-> DecodedImage
-> pillow-rs Image storage
```

### Palette Storage

Extend `PalettedData` to retain codec-provided alpha:

```rust
pub struct PalettedData {
    pub width: u32,
    pub height: u32,
    pub indices: Vec<u8>,
    pub palette_rgb: Vec<u8>,
    pub palette_alpha: Option<Vec<u8>>,
}
```

If `palette_alpha` is present, it must contain one entry per palette color.

### Metadata Cache

After `image-slash-star` exposes `ImageInfo`, add metadata to lazy variants:

```rust
Path {
    path: PathBuf,
    format: Option<ImageFormat>,
    info: Option<ImageInfo>,
    is_paletted: bool,
}

Bytes {
    data: Arc<Vec<u8>>,
    format: Option<ImageFormat>,
    info: Option<ImageInfo>,
    is_paletted: bool,
}
```

Later, remove `is_paletted` after callers use `info.has_palette` or decoded
state.

Use metadata for Rust-native inspection methods:

- `width`
- `height`
- `dimensions`
- `format`
- `image_info`
- `color_mode`
- `palette`
- `is_animated`
- `frame_count`

Pixel access, operation execution, and saving still require decode or
materialization.

### Pipeline Metadata

Do not blindly reuse source metadata for pipelines.

Some operations preserve dimensions and mode. Some operations change them.

Examples:

- `blur`: preserves dimensions and mode
- `resize`: changes dimensions
- `crop`: changes dimensions
- `convert`: changes mode
- `rotate`: may change dimensions
- `thumbnail`: changes dimensions

First implementation should be conservative:

```text
Image::Path / Image::Bytes metadata
-> use cached ImageInfo

Image::Pipeline metadata
-> use source metadata only when all ops are known metadata-preserving
-> otherwise materialize or return no cheap metadata
```

Avoid returning stale metadata.

### Persistent Load

Change `load` to true mutating materialization:

```rust
pub fn load(&mut self) -> Result<(), ImageError>;
pub fn is_materialized(&self) -> bool;
```

Expected behavior:

```text
Image::Loaded
-> already materialized

Image::Paletted
-> already materialized

Image::Path / Image::Bytes
-> decode once
-> replace self with Loaded or Paletted

Image::Pipeline
-> materialize source and operations
-> replace self with Loaded or Paletted when representable
```

Keep a non-mutating validation helper separately if useful:

```rust
pub fn verify(&self) -> Result<(), ImageError>;
```

`load(&self)` should not remain the primary API if it discards decoded pixels.

### Materialization Boundary

Keep a non-mutating execution helper for existing call sites:

```rust
pub fn materialize(&self) -> Result<DynamicImage, ImageError>;
```

Callers that want caching should use `load(&mut self)`.

The pipeline remains:

```text
open/open_bytes
-> store encoded source and metadata
-> append operations lazily
-> materialize once when pixels are needed
-> decode through image-slash-star
-> execute PipelineOp in pillow-rs
```

## Proposed `pillow-rs-js` Changes

Do not enable all codecs by default.

Candidate feature sets:

```text
minimum:
png

browser-common:
png + jpeg + gif + webp

full-web:
png + jpeg + gif + webp + bmp + ico

experimental:
png + jpeg + gif + webp + bmp + ico + avif
```

Recommended initial default: keep PNG-only until size has been measured, then
consider `png + jpeg + gif + webp` if the release artifact remains acceptable.

### Logging Size Policy

Measure before deciding.

Variants:

- no logging dependencies
- `log` dependency only
- `console_error_panic_hook` enabled
- `console_log` enabled
- full debug logging enabled

Record:

- uncompressed WASM size
- optimized WASM size
- gzip size
- brotli size

Keep logging in normal release builds only if the compressed size impact is
within the target budget.

Target:

```text
<= 500 bytes compressed
```

## Phased Migration

### Phase 1: Strengthen `image-slash-star`

1. Add `ImageError`.
2. Add `ImageResult<T>`.
3. Convert the canonical detect/decode/encode APIs to `Result`.
4. Add `Decoded<T>` so automatic decode retains source format.
5. Return `FeatureDisabled` for disabled codecs.
6. Preserve current AVIF still, sequence, and brand behavior.
7. Add validation for `DecodedImage`.
8. Normalize paletted decode representation.

### Phase 2: Add Metadata Inspection

1. Add `ImageInfo`.
2. Add canonical `inspect` returning `ImageResult<ImageInfo>`.
3. Implement cheap metadata paths format by format.
4. Preserve palette RGB/alpha in metadata where available.
5. Avoid full pixel decode in inspect paths.
6. Add tests proving inspection does not decode pixel payloads where practical.

Suggested order:

1. PNG (complete)
2. JPEG (complete)
3. GIF (complete)
4. BMP (complete)
5. WebP (complete)
6. TIFF (complete)
7. ICO (complete)
8. AVIF (complete; native parser)

BMP acceptance evidence includes OS/2 and Windows headers, 1/4/8-bit
palettes, implicit palette sizes, RLE, bitfields, top-down rows, malformed
headers, and exact Pillow mode/palette parity. The oracle also establishes two
non-obvious compatibility rules: grayscale 4-bit BMP materialization is
rejected, while a pixel offset one byte before the declared DIB end is accepted.

### Phase 3: Wire `pillow-rs` To `image-slash-star`

1. Point dependency at `~/work/image-slash-star`.
2. Use `default-features = false`.
3. Add missing forwarded codec features.
4. Keep AVIF explicit opt-in.
5. Replace local decode paths with `decode`.
6. Map `image-slash-star::ImageError` into the `pillow-rs` error type, or reuse
   it if practical.

### Phase 4: Replace PNG-Specific Palette Logic

1. Add generic `DecodedImage -> Image` conversion.
2. Preserve `P8` as `Image::Paletted`.
3. Add `palette_alpha` to `PalettedData`.
4. Use codec-provided alpha in transparency expansion.
5. Remove `decode_paletted_png_reader`.
6. Remove direct `png = "0.18"` from `pillow-rs`.
7. Add regression tests for indexed PNG with alpha.

### Phase 5: Metadata Cache In `pillow-rs`

1. Store `ImageInfo` on lazy path/byte images.
2. Update cheap APIs to use metadata.
3. Avoid full decode for JS width/height.
4. Be conservative for pipeline metadata.
5. Remove `is_paletted` once `ImageInfo` fully replaces it.

### Phase 6: Persistent Load

1. Change core `load` to `&mut self`.
2. Replace lazy images with materialized storage after successful load.
3. Add `is_materialized`.
4. Add non-mutating `verify` if needed.
5. Update JS and Python bindings.
6. Ensure adding new operations after load produces correct pipeline behavior.

### Phase 7: WASM Measurement

1. Build release WASM for each candidate codec feature set.
2. Measure uncompressed size.
3. Measure optimized size.
4. Measure gzip size.
5. Measure brotli size.
6. Measure logging variants separately.
7. Record results in the repo.

Suggested maintained targets:

```bash
make build-wasm-release
make size-wasm
```

### Phase 8: Animated Images

Treat animated images as a separate migration.

`image-slash-star` already has `DecodedSequence`, but `pillow-rs` needs a
Rust-native frame model before wiring it in.

Needed concepts:

- `ImageSequence`
- `ImageFrame`
- `FrameDelay`
- `DisposalMethod`
- `BlendMethod`

Support should include:

- frame count
- current frame index
- seeking
- frame decoding
- frame metadata
- sequence encoding

Do not block still-image migration on this.

## Acceptance Criteria

### Codec Backend

- `image-slash-star` exposes `Result` APIs.
- automatic decoding retains both source `ImageFormat` and exact pixel mode
- encoding requires an explicit target format
- Disabled features return structured errors.
- Unknown formats, malformed files, and unsupported modes are distinguishable.
- Paletted formats decode as `P8 + ImagePalette`.
- Palette alpha is preserved when available.
- Metadata inspection works without full decode for common formats.

### `pillow-rs`

- `pillow-rs` loads non-PNG formats when their features are enabled.
- `pillow-rs` no longer depends directly on the `png` crate.
- All still-image decoding goes through `image-slash-star`.
- Generic `DecodedImage -> Image` conversion exists.
- Paletted images preserve RGB and alpha.
- `load(&mut self)` persists materialized state.
- `is_materialized()` reports lazy versus loaded state correctly.
- Cheap metadata APIs avoid pixel decode when `ImageInfo` is available.
- Pipeline operations remain owned and executed by `pillow-rs`.

### `pillow-rs-js`

- JS/WASM supports exactly the selected codec feature set.
- Width/height access avoids full decode when metadata is available.
- WASM size measurements are recorded.
- Logging remains in release builds only if measured size cost is acceptable.

## Verification

For `pillow-rs`:

```bash
make test-core
make test-wasm
make build-wasm-release
make fmt
make clippy
```

For `image-slash-star`, use maintained targets if present. If they do not exist,
add them before relying on repeated manual commands:

```bash
make test
make fmt
make clippy
```

## Open Decisions

1. Should `image-codecs-all` include AVIF?

   Recommendation: no. Keep AVIF explicit opt-in initially.

2. Should `Image::open(path)` inspect immediately?

   Recommendation: inspect immediately for byte sources. Consider lazy
   inspection for paths unless size/mode/info is requested.

3. Should `ImageInfo.frame_count` always be exact?

   Recommendation: allow `None` when exact count requires expensive scanning.

4. Should unsupported modes like CMYK, I32, F32 grayscale, or YCbCr get native
   operation support?

   Recommendation: preserve them in `image-slash-star` metadata first. Add
   operation support only when `pillow-rs` has a lossless internal
   representation.

5. Should `pillow-rs-js` default to browser-common codecs?

   Recommendation: only after size measurement. Until then, keep PNG-only or
   make the default explicit and documented.

## Recommended Implementation Order

1. Add `image-slash-star` `Result` APIs and errors.
2. Add `DecodedImage` validation and paletted normalization.
3. Add `pillow-rs` feature forwarding.
4. Add generic `DecodedImage -> Image` conversion in `pillow-rs`.
5. Remove direct PNG-specific palette decode from `pillow-rs`.
6. Add `ImageInfo` and `inspect` to `image-slash-star`.
7. Use `ImageInfo` in `pillow-rs`.
8. Make `load(&mut self)` persistent.
9. Measure WASM size for codec feature sets and logging variants.
10. Add animated image support separately.
