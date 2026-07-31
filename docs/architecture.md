# Architecture and public contract

Status: current implementation reference

Reviewed: 2026-07-31 against the working tree based on `b437123`

This document explains the stable mental model and ownership boundaries of
`image-slash-star`. The generated Rust API documentation remains the
declaration-level reference.

## What the crate owns

`image-slash-star` is a byte-oriented image codec library. It owns:

- encoded-image signature detection;
- header and container inspection;
- validated still-image and sequence decoding;
- explicit-format still-image and sequence encoding;
- source format, sample mode, color layout, palette, alpha, frame timing,
  disposal, and background transfer models;
- structured codec errors; and
- immutable encoded-byte snapshots with shared lazy materialization.

It does not own filesystem policy or general image processing. Resizing,
cropping, rotation, drawing, filtering, arbitrary compositing, color
adjustment, and mutable editing belong in downstream applications or
processing libraries.

Transforms required by a codec—JPEG IDCT, PNG filtering, AV1 prediction,
sample reconstruction, color conversion, animation disposal, and encoder
quantization—remain private implementation details.

## Mental model

Encoded format and decoded sample layout answer different questions:

```text
encoded bytes
    │
    ├─ detect_format() ───────────────► ImageFormat
    │
    ├─ inspect() ─────────────────────► ImageInfo
    │
    ├─ decode() ──────────────────────► Decoded<DecodedImage>
    │                                      ├─ source ImageFormat
    │                                      └─ ImageMode + ColorType + pixels
    │
    └─ decode_sequence() ─────────────► Decoded<DecodedSequence>

DecodedImage / DecodedSequence
    └─ encode*(explicit ImageFormat) ──► encoded bytes
```

`ImageFormat` identifies the encoded container, such as PNG or JPEG.
`ImageMode` identifies the observable decoded byte layout. `ColorType`
describes its unpacked channel representation. `Decoded<T>` retains the source
format separately so decoding never pretends that a pixel buffer has an
intrinsic output format.

Palette indices use `ImageMode::P8` and an optional `ImagePalette`. They are not
treated as luminance merely because both layouts use one byte per sample.

`ImageInfo::is_indexed()` classifies the `P8` sample mode, while
`ImageInfo::has_palette_table()` reports whether an explicit table was
retained. These can differ for a tolerated indexed container with a missing or
empty palette table.

The shared `ico` codec also recognizes CUR. `cursor_hotspot: Some(...)` on
`ImageInfo` and `DecodedImage` preserves CUR identity and its selected entry's
activation point; ordinary ICO uses `None`.

## Canonical public surface

Codec modules and dispatchers are private. Callers use the root API so format
detection, feature availability, decoded-buffer validation, and error
translation cannot be bypassed.

| API | Contract |
| --- | --- |
| `detect_format(&[u8])` | Identify a supported signature without invoking a codec |
| `inspect(&[u8])` | Read `ImageInfo` without materializing compressed pixels |
| `decode(&[u8])` | Auto-detect and decode the still/first-image view |
| `decode_sequence(&[u8])` | Auto-detect and retain every supported frame plus presentation metadata |
| `encode(&DecodedImage, ImageFormat, &EncodeOptions)` | Validate and encode one image to an explicit target |
| `encode_default(&DecodedImage, ImageFormat)` | Encode one image with format defaults |
| `encode_sequence(&DecodedSequence, ImageFormat, &EncodeOptions)` | Encode one frame to any enabled format or multiple frames to GIF, TIFF, WebP, or native AVIF |
| `EncodedImage::new(bytes)` | Snapshot encoded bytes, inspect immediately, and defer decoding |

`detect_format` recognizes all eight container signatures even when a codec
feature is disabled. An operation that requires a disabled codec returns
`ImageError::FeatureDisabled`. For AVIF, `avif` and `avis` major brands are
direct signatures; generic `mif1`/`msf1` majors additionally require an
`avif` or `avis` compatible brand in the complete bounded `ftyp` box.

## Decoded sample layouts

`DecodedImage::pixels` is tightly packed and row-major. There is no implicit
row stride.

| Mode | Layout |
| --- | --- |
| `L1` | One bit per sample, most-significant bit first, each row byte-aligned |
| `P8` | One palette index per byte |
| `L8`, `La8` | Interleaved 8-bit luminance and optional alpha |
| `Rgb8`, `Rgba8`, `Cmyk8` | Interleaved 8-bit channels |
| `L16`, `La16`, `Rgb16`, `Rgba16` | Interleaved little-endian 16-bit channels |
| `F32`, `I32` | Exact Pillow-observable 32-bit luminance bytes. These are byte-preserving modes, not portable typed-scalar views: TIFF `I`/`F` can retain the file byte order. |
| `Rgb32F`, `Rgba32F` | Native-endian 32-bit floating-point RGB(A) samples |

Code that needs numeric `I32`/`F32` values must determine the originating
codec's sample byte order before casting or parsing the bytes. This distinction
is required for Pillow 12.2.0 parity: on a little-endian host, Pillow
`tobytes()` still returns big-endian bytes for a big-endian TIFF `I` or `F`
source. TIFF encoding treats the supplied `I32`/`F32` buffer as the exact
Pillow-observable transfer bytes. The manifest proves uncompressed, Deflate
plus horizontal-predictor, and LZW plus horizontal-predictor output from
detached big-endian source bytes.

`DecodedImage::new` and `DecodedImage::with_mode` record caller-supplied
buffers without validating them. `DecodedImage::validate`, every encoder, and
sequence validation reject:

- zero dimensions;
- arithmetic overflow;
- a byte length inconsistent with dimensions and mode;
- a `ColorType` inconsistent with `ImageMode`;
- palettes on non-indexed images;
- invalid RGB or alpha table lengths; and
- palette indices outside the retained palette.

`DecodedSequence` validates its canvas, requires at least one frame, validates
each frame image, and rejects frame rectangles outside the canvas.

## Errors and recovery

All canonical fallible operations return `ImageResult<T>`. `ImageError` is
non-exhaustive so downstream matches need a fallback arm.

| Error | Meaning | Typical recovery |
| --- | --- | --- |
| `UnknownFormat` | No supported signature matched | Check the complete input or select another parser |
| `FeatureDisabled` | The format is recognized but its Cargo feature is off | Enable that feature or reject the format |
| `Malformed` | The selected decoder rejected the encoded bytes | Do not retry unchanged bytes |
| `Unsupported` | The format is valid enough to identify, but the requested operation/class is unavailable | Choose another target, option, or implementation |
| `Dimensions` | Dimensions, frame bounds, or sample length are invalid | Correct or constrain the caller-supplied data |
| `Parameter` | An encoder option, palette, mode combination, or other parameter is invalid | Correct the named input |

`ImageError::kind()` is the stable recovery category and
`ImageError::format()` identifies the selected codec when one exists.
`Dimensions` and `Parameter` retain optional format plus the high-level
diagnostic that crossed the codec boundary. `ImageError::message()` exposes
that diagnostic for logs; its prose may become more specific and is not a
commitment to preserve every internal parser phrase as public API.

## Immutable source lifecycle

`EncodedImage::new` converts input into an `Arc<[u8]>`, detects the format, and
inspects the header. It does not decode pixels.

The first call to `decode()` initializes a shared `OnceLock`:

- clones observe the same encoded-byte snapshot and metadata;
- successful materialization is reused;
- deterministic decode failures are cached too;
- `is_decoded()` is true only for a cached success; and
- `verify()` runs independently and does not populate or modify the decode
  cache.

`ImageFormat::verification_scope()` and
`EncodedImage::verification_scope()` distinguish `Structure` from
`HeaderOnly`. Header-only is Pillow 12.2.0's base `ImageFile.verify` behavior:
successful construction/inspection is the complete check, so later pixel
decompression can still fail. PNG has Pillow's structural scan; this crate
also retains independently proved JPEG and WebP structural verifiers.

The crate performs no filesystem or network I/O and emits no logs. Applications
decide where bytes come from and how errors are recorded.

## Features and targets

Default features are `jpeg`, `png`, `gif`, `bmp`, `tiff`, `webp`, and `ico`.
`avif` is opt-in. ICO intentionally enables PNG and BMP because entries can use
either representation.

| Feature | Native | `wasm32-unknown-unknown` |
| --- | --- | --- |
| `jpeg` | Rust inspect/decode/encode | Build-verified Rust path |
| `png` | Rust still/APNG sequence decode and still encode | Build-verified Rust path |
| `gif` | Rust still/sequence decode and encode | Build-verified Rust path |
| `bmp` | Rust inspect/decode/encode | Build-verified Rust path |
| `tiff` | Rust still/multipage decode and encode | Build-verified Rust path |
| `webp` | Rust still/sequence decode and still/keyframe-sequence encode | Build-verified Rust path |
| `ico` | Rust inspect/decode and source-sized encode | Build-verified Rust path |
| `avif` | Fixed native inspect/decode/sequence/encode stack | Portable inspect and a manifest-bounded still-decode subset; sequence decode and encode unsupported |

The compatibility guarantee is limited to active manifest cases. Default
native and WASM builds select the same Rust codec code, but the complete
semantic matrix has not yet executed in a WASM runtime.

See [AVIF support](avif.md) for the native dependency and portable boundary.

## Internal source organization

| Path | Responsibility |
| --- | --- |
| `src/lib.rs` | crate contract, signature detection, canonical root API |
| `src/source.rs` | immutable encoded snapshots and lazy decode cache |
| `src/encode_options.rs` | shared and codec-specific compatibility options |
| `src/types/` | formats, modes, palettes, images, frames, sequences, errors, validation |
| `src/codecs/mod.rs` | private feature dispatch and availability checks |
| `src/codecs/error.rs` | private codec failures and public error translation |
| `src/codecs/compression/` | DEFLATE/zlib behavior shared by PNG and TIFF |
| `src/codecs/<format>/` | format-local inspection, decoding, and encoding |
| `src/codecs/avif/native.rs` | safe ownership model around the opt-in unsafe FFI calls |
| `src/codecs/avif/native/bridge.c` | narrow libavif C bridge |
| `src/codecs/avif/av1/` | portable AV1 parsing, entropy, partition, and reconstruction work |

The folder `src/codecs/webp/native/` is a Rust port organized around upstream
algorithm boundaries. It does not link a native WebP library.

## Dependency and unsafe boundary

`bytemuck` is the only Cargo dependency. Default codecs do not link native
codec libraries.

Crate-wide unsafe Rust is denied. The only module-level exception is
`src/codecs/avif/native.rs`, which owns the optional libavif handles and
documents pointer, buffer, and deallocation invariants adjacent to each unsafe
operation. The separately compiled C bridge is enabled only for native builds
with the `avif` feature.

## Resource behavior and current limit

The public API is whole-buffer based:

- inputs are borrowed byte slices or immutable encoded snapshots;
- decoded pixels and encoded output allocate complete buffers;
- sequence APIs retain complete supported frame data; and
- no streaming reader/writer interface exists.

Caller-controlled decode limits are not implemented. The current crate should
not be described as hardened for arbitrary hostile inputs. Resource limits are
a release-blocking item in the [roadmap](roadmap.md).

## Retained and removed scope

Retained by design:

- exact, version-controlled Pillow-oracle fixtures;
- source-derived codec implementations with complete provenance;
- private coverage hooks for defensive states no valid fixture can reach;
- explicit target formats for encoding; and
- the fixed native AVIF path until portable parity replaces it.

Removed from earlier iterations:

- the `pillow-rs-image` package identity;
- a mutable/general image buffer and processing layer;
- public format-specific codec entry points;
- duplicate `try_*` and `Option`-returning fallible APIs;
- Serde development dependencies;
- isolated unit tests in place of the manifest harness;
- size-only or approximate output comparisons; and
- ICO resizing and implicit multi-resolution generation.

Downstream projects may add processing or bindings, but those contracts do not
expand this crate's public scope.
