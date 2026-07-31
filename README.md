# image-slash-star

[![CI](https://github.com/appunni-m/image-slash-star/actions/workflows/ci.yml/badge.svg)](https://github.com/appunni-m/image-slash-star/actions/workflows/ci.yml)
[![License: multi-license](https://img.shields.io/badge/license-see%20NOTICE-blue.svg)](#license-and-attribution)

Dependency-constrained Rust codecs for detecting, inspecting, decoding, and
encoding JPEG, PNG, GIF, BMP, TIFF, WebP, ICO/CUR, and AVIF bytes.

The crate targets exact observable compatibility with a pinned Pillow 12.2.0
oracle for every active manifest case: success or error, format, mode,
dimensions, metadata, frames, pixels, palettes, and deterministic encoded
bytes.

> **Pre-release status:** version 0.1.0 is not published to crates.io. The
> compatibility guarantee is limited to committed manifest cases, not every
> legal file in each format specification. Encoded-input bytes and inspected
> primary-canvas dimensions/pixels/decoded bytes, the inspected frame count,
> every later frame/page's decoded bytes, and cumulative sequence bytes can be
> bounded, but metadata, container nesting, and codec work are not yet fully
> limited. The current crate should not be treated as hardened for arbitrary
> hostile inputs.
> Breaking API changes may occur before 1.0.

## Why use it?

- One auto-detecting, structured `Result` API across all codecs.
- Rust-only default JPEG, PNG, GIF, BMP, TIFF, WebP, and ICO execution.
- `bytemuck` as the only Cargo dependency.
- Independent per-format Cargo features.
- Exact fixture-backed success, error, pixel, frame, and encoded-byte checks.
- Byte-buffer APIs that work without filesystem or networking assumptions.
- Default codec feature combinations cross-compile to
  `wasm32-unknown-unknown`.

The crate deliberately does not resize, crop, rotate, draw, filter, adjust, or
otherwise process decoded images. Applications keep image processing in a
downstream library.

## Quick start

Until the first registry release, depend on the repository:

```toml
[dependencies.image-slash-star]
git = "https://github.com/appunni-m/image-slash-star"
default-features = false
features = ["png", "jpeg"]
```

Cargo package names use hyphens; Rust imports use underscores.

```rust,no_run
use image_slash_star::{
    decode, encode_default, ImageError, ImageFormat, ImageMode, ImageResult,
};

fn opaque_rgb_png_to_jpeg(input: &[u8]) -> ImageResult<Vec<u8>> {
    let decoded = decode(input)?;
    if decoded.format != ImageFormat::Png {
        return Err(ImageError::Unsupported {
            format: Some(decoded.format),
            message: "expected a PNG source".to_owned(),
        });
    }
    if decoded.content.mode != ImageMode::Rgb8 {
        return Err(ImageError::Unsupported {
            format: Some(ImageFormat::Jpeg),
            message: "JPEG example requires opaque RGB8 input".to_owned(),
        });
    }
    encode_default(&decoded.content, ImageFormat::Jpeg)
}
```

`decode` detects the source format from the complete byte slice and preserves
that format separately from the decoded sample mode. Encoding always requires
an explicit output format.

The example deliberately accepts only opaque `Rgb8` PNG pixels. RGBA, indexed,
bilevel, and sixteen-bit PNG inputs need an explicit conversion policy in a
downstream processing library; this codec crate does not silently discard
alpha, expand palettes, or change sample depth. For a full program, read bytes
with `std::fs`, call the function above, and write the returned bytes. The crate
itself never opens paths.

## Supported features

Default features enable every codec except AVIF.

| Feature | Default | Native behavior | `wasm32-unknown-unknown` |
| --- | --- | --- | --- |
| `jpeg` | yes | Rust inspect/decode/encode | Build-verified Rust path |
| `png` | yes | Rust still/APNG sequence decode and still encode | Build-verified Rust path |
| `gif` | yes | Rust still/sequence decode and encode | Build-verified Rust path |
| `bmp` | yes | Rust inspect/decode/encode | Build-verified Rust path |
| `tiff` | yes | Rust still/multipage decode and encode | Build-verified Rust path |
| `webp` | yes | Rust still/sequence decode and still/keyframe-sequence encode | Build-verified Rust path |
| `ico` | yes | Rust ICO/CUR inspect/decode and source-sized ICO encode | Build-verified Rust path |
| `avif` | no | Fixed native inspect/decode/sequence/encode stack | Portable inspect and restricted still decode; sequence decode and encode unsupported |

The `ico` feature recognizes both ICO and CUR signatures and accepts `.ico`
and `.cur` aliases. Inspection and decode retain the selected CUR hotspot in
`ImageInfo::cursor_hotspot` and `DecodedImage::cursor_hotspot`; `None`
distinguishes ordinary ICO. The feature enables PNG and BMP because an entry
can use either representation. Encoding currently writes ICO only, with one
entry at the supplied raster dimensions, and never resizes pixels.

Feature evolution rule: format umbrella features (`jpeg`, `png`, `gif`, `bmp`,
`tiff`, `webp`, `ico`, `avif`) are stable public Cargo API. Any future
operation-level subfeature must be additive: it may only narrow an umbrella's
optional surface, never disable behavior that a subset of features already
enables, and Cargo's additive unification must compose subfeatures without
changing umbrella semantics. This rule is committed before any split is
accepted.

WASM feature combinations are cross-compiled in CI. Executing the complete
semantic fixture matrix in a WASM runtime remains planned.

AVIF is the remaining portability boundary. Native parity uses fixed
libavif 1.4.1, dav1d 1.5.3, and libaom 3.13.2 builds. The WASM path has a
growing in-tree AV1 subset. See [AVIF support](docs/avif.md) for exact
capabilities and setup.

## API and data model

| API | Purpose |
| --- | --- |
| `detect_format(&[u8])` | Identify a supported container signature |
| `inspect(&[u8])` | Read `ImageInfo` without decoding compressed pixels |
| `decode(&[u8])` | Decode the still/first-image view and retain source format |
| `decode_sequence(&[u8])` | Retain supported frames and presentation metadata |
| `inspect_with_policy`, `decode_with_policy`, `decode_sequence_with_policy` | Apply caller-controlled limits before the corresponding operation |
| `encode(&DecodedImage, ImageFormat, &EncodeOptions)` | Encode one image with explicit options |
| `encode_default(&DecodedImage, ImageFormat)` | Encode one image with defaults |
| `encode_sequence(&DecodedSequence, ImageFormat, &EncodeOptions)` | Encode one frame to any enabled format or multiple frames to GIF, TIFF, WebP, or native AVIF |
| `ImageFormat::capabilities()` | Query detection, inspection, still, and genuine multi-image support for the current feature set and target |
| `all_capabilities()` | Return the same typed capability record for every public format |
| `EncodedImage::new(bytes)` | Inspect an immutable source now and decode it lazily |
| `EncodedImage::*_with_policy(...)` | Enforce the same limits during source construction or lazy materialization |
| `EncodedImage::verify_with_scope(scope)` | Verify with an explicit requested strength; stronger requests fail instead of downgrading |

`Decoded::consumed_bytes` reports the encoded bytes of the container-defined
extent when the container defines one unambiguously (JPEG after EOI, PNG after
IEND, GIF after the trailer, WebP's RIFF size, TIFF's final IFD, and AVIF's
last top-level box). BMP and ICO report `None` because they declare no total
extent. Decoders ignore well-formed trailing bytes after that extent and never
let them change the decoded result; the trailing-input manifest pins this
behavior for all eight formats against Pillow 12.2.0.

Signature detection is feature-independent. Disabled codec operations report
`Unavailable(FeatureDisabled)` through capability discovery and return
`ImageError::FeatureDisabled` when attempted. Sequence capabilities mean
genuine multi-image decode or encode; the validated one-frame fallback follows
the corresponding still capability. On `wasm32`, AVIF inspection remains
manifest-bounded, still decode reports the restricted portable subset, and
encode plus sequence operations report target unavailability.

`ImageFormat::from_name` accepts canonical names and extension aliases
case-insensitively: JPEG `jpg`/`jpeg`/`jfif`/`jpe`, PNG `png`/`apng`, TIFF
`tiff`/`tif`, ICO/CUR `ico`/`cur`, and AVIF `avif`/`avifs`. Headerless `.dib`
remains an explicit-format scope decision, not an automatic BMP alias.
`mime_type()`, `canonical_extension()`, and `extensions()` expose stable,
dependency-free format metadata in canonical-first order; `from_path` uses the
same table without touching the filesystem.

`VerificationScope` orders `HeaderOnly` < `Structure` < `FullPixels`.
`EncodedImage::verify()` runs the format's Pillow-compatible default scope;
`verify_with_scope(requested)` fails with a format-qualified `Unsupported` when
the codec cannot provide the requested strength, so header-only success is
never silently reported as structural or full-pixel evidence. No codec
currently provides `FullPixels`.

The core model separates:

```text
ImageFormat                ImageMode + ColorType
encoded container         decoded sample-byte layout
PNG / JPEG / GIF / ...    P8 / L8 / RGB8 / RGBA8 / ...
             \             /
              Decoded<T>
```

`DecodedImage::pixels` is tightly packed and row-major. Indexed samples use
`ImageMode::P8` and retain an `ImagePalette` when the source exposes one; they
are not silently interpreted as grayscale. Caller-built images and sequences
are validated before encoding.

`ImageInfo::source` and `DecodedImage::source` retain structural source facts
without changing the transfer bytes. TIFF currently records its exact
`SourceByteOrder`; `I32`/`F32` pixels preserve that order, while normalized
modes keep their documented transfer layout. Other codecs currently return an
empty `SourceDescriptor`.

`DecodedSequence::first()` returns the complete `DecodedFrame`, including its
source and presentation metadata. `first_image()` is available when a caller
intentionally wants only the first frame's pixels and accepts that metadata
loss.

Codec/capability vocabulary enums are non-exhaustive, including `ImageFormat`,
`VerificationScope`, `ImageMode`, and animation presentation enums. Downstream
`match` expressions must include a fallback so a later format or transfer mode
does not become an accidental source break. Closed domains such as
`SourceByteOrder` remain exhaustive.

### Typed encoder options

`EncodeOptions` always identifies one target codec. Construct the corresponding
record directly or use `EncodeOptions::for_format` for that format's defaults:

```rust
use image_slash_star::{
    encode, EncodeOptions, ImageFormat, JpegEncodeOptions, JpegSubsampling,
};

# fn example(image: &image_slash_star::DecodedImage)
#     -> image_slash_star::ImageResult<Vec<u8>> {
let options = EncodeOptions::from(JpegEncodeOptions {
    quality: Some(90),
    subsampling: Some(JpegSubsampling::Cs444),
    ..JpegEncodeOptions::default()
});
encode(image, ImageFormat::Jpeg, &options)
# }
```

Passing JPEG options with a PNG target, for example, returns a
format-qualified `Parameter` error before codec dispatch. There is no
format-neutral `EncodeOptions::default()` because codec defaults and option
domains are not interchangeable.

`EncodeOptions::try_from_legacy_pairs` is a strict migration boundary for the
former string-pair configuration. It rejects unknown and duplicate keys,
validates each value, and produces a typed record; encoders never inspect
string keys. New integrations should construct codec records directly.

### Caller-controlled limits

The unlimited entry points remain convenient for trusted inputs.
`DecodePolicy` provides inclusive maxima for the complete encoded byte slice,
inspected canvas width, height, and pixel count, and the primary image's
decoded transfer-byte length, the inspected frame/page count, every later
frame/page's decoded byte length, and the cumulative retained sequence bytes:

```rust
use image_slash_star::{decode_with_policy, DecodePolicy, ImageResult};

fn decode_at_most_one_mebibyte(
    input: &[u8],
) -> ImageResult<image_slash_star::Decoded<image_slash_star::DecodedImage>> {
    let policy = DecodePolicy::new()
        .with_max_encoded_bytes(1024 * 1024)
        .with_max_width(4096)
        .with_max_height(4096)
        .with_max_pixels(16_000_000)
        .with_max_primary_decoded_bytes(64 * 1024 * 1024)
        .with_max_frames(1000)
        .with_max_frame_decoded_bytes(4 * 1024 * 1024)
        .with_max_sequence_decoded_bytes(256 * 1024 * 1024)
        .with_max_metadata_bytes(8 * 1024 * 1024);
    decode_with_policy(input, &policy)
}
```

The encoded-byte check occurs before signature detection and codec parsing. An
oversized input returns a typed `LimitExceeded` error with the operation,
`ResourceLimit::EncodedBytes`, configured maximum, and observed length. It has
no selected format because no format parsing occurred.

Canvas limits use exact `ImageInfo` width, height, `width × height`, mode, and
primary decoded byte length. Packed `L1` rows are byte-aligned; other modes use
their exact transfer bytes per pixel. They run after format-qualified
inspection and before primary pixel materialization, so their errors retain
the selected format. Policy-aware direct decode may inspect then parse again;
unlimited wrappers do not gain that additional pass.

`max_frames` uses the exact inspected frame/page count and runs after the
canvas checks but before sequence materialization. Inspection and sequence
decode reject a source whose declared count exceeds the maximum; still decode
and lazy still materialization retain exactly one frame, so only a zero frame
maximum rejects them. Sources whose inspection cannot prove an exact count
remain unlimited for this resource.

`max_frame_decoded_bytes` and `max_sequence_decoded_bytes` apply inside every
sequence decoder before the next frame's pixel work: the per-frame limit
rejects any later frame/page whose transfer-byte length exceeds the maximum,
and the cumulative limit charges the inspected primary first and rejects
before the frame whose addition would exceed the total. Both failures retain
the format and typed resource; the primary and still-only paths remain bounded
by the primary-canvas limits.

`max_metadata_bytes` bounds the encoded metadata extent — every encoded byte
that is not primary pixel payload data — measured by a per-format container
scan before inspection or pixel work on all five policy paths.

`inspect_with_policy`, `decode_sequence_with_policy`,
`EncodedImage::new_with_policy`, and `EncodedImage::decode_with_policy` use the
same boundary. A rejected lazy decode is not cached, and an already cached
decode cannot bypass a later stricter policy. This is not yet a complete
hostile-input budget: metadata, container nesting, codec work, other
allocations, and encoded output remain unbounded.

See [architecture and public contract](docs/architecture.md) for byte layouts,
validation invariants, lazy source lifecycle, memory behavior, feature
dispatch, and internal boundaries. Generate declaration-level API
documentation with:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked
```

## Errors

Every canonical fallible API returns `ImageResult<T>`.

| Error | Meaning |
| --- | --- |
| `UnknownFormat` | No supported signature matched |
| `FeatureDisabled` | The format is recognized but its Cargo feature is off |
| `Malformed` | The selected codec rejected the encoded bytes |
| `Unsupported` | The requested operation or valid input class is unavailable |
| `Dimensions` | Dimensions, frame bounds, or sample length are invalid |
| `Parameter` | An option, palette, mode combination, or other parameter is invalid |
| `LimitExceeded` | A caller-configured resource maximum was exceeded |

Codec-dispatched failures additionally report the public operation that
produced them through `ImageError::stage()` (`Inspection`, `StillDecode`,
`StillEncode`, `SequenceDecode`, `SequenceEncode`, or `Verification`).
Caller-built validation and option-construction errors remain stage-free;
`UnknownFormat`, `FeatureDisabled`, and `LimitExceeded` keep their existing
contracts (`LimitExceeded` already carries the typed operation).

Where the parser can name the failing container structure, codec-dispatched
errors also report the encoded-input byte offset (`ImageError::offset()`) and
a stable structure identity (`ImageError::identity()`, for example
`png_chunk`, `jpeg_marker`, or `tiff_ifd`). BMP, ICO, and WebP decode internals
intentionally remain detail-free.

`ImageError` is non-exhaustive; downstream `match` expressions need a fallback
arm. Unchanged malformed bytes should not be retried. Feature and unsupported
errors can usually be handled by selecting another compiled capability.

Use `error.kind()` for stable recovery policy and `error.format()` for the
selected input/output format when one is known. `error.message()` returns
retained high-level codec/parameter diagnostics; `LimitExceeded` instead
exposes typed fields directly and has no prose message. In particular,
`Dimensions` and `Parameter` retain both optional format and diagnostic
context; callers do not need to parse `Display` output. Diagnostic prose may
become more specific, so it is not a substitute for `ImageErrorKind`.

## Correctness evidence

The generated matrix in this tree contains 1,417 active cases:
1,024 decode/inspect/verify cases and 393 encode cases, with zero
planned or unwired rows. Expected errors are active fixture outcomes, and
every decode-error class is catalogued in a generated, CI-checked
malformed-class ledger with Pillow outcome, Rust error contract, evidence
origin, and specification status.

The accepted Coverage MCP snapshot for that implementation state reports 100%
line, branch, function, and region coverage. Coverage proves execution under
the retained suite; it does not prove complete format support or security.

The oracle identity, regeneration workflow, exact comparison contract, test
tiers, current run identifiers, and troubleshooting are in
[oracle, fixtures, tests, and coverage](docs/testing.md).

## Build from source

The Rust version and WASM target are pinned in `rust-toolchain.toml`.

```bash
git clone https://github.com/appunni-m/image-slash-star.git
cd image-slash-star
cargo check --locked
```

Native builds using default features need no external codec library. Enabling
`avif` requires the exact native stack described in
[AVIF support](docs/avif.md#native-setup).

## Documentation

- [Architecture and public contract](docs/architecture.md)
- [Oracle, fixtures, tests, and coverage](docs/testing.md)
- [AVIF support and portability boundary](docs/avif.md)
- [Roadmap](docs/roadmap.md)
- [Changelog](CHANGELOG.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [Third-party provenance](third_party/README.md)

Current behavior belongs in the README, architecture reference, rustdoc, and
testing contract. Planned work belongs only in the roadmap. Historical
investigation logs remain available through Git history rather than the active
documentation tree.

## Support, contributing, and security

Use [GitHub issues](https://github.com/appunni-m/image-slash-star/issues) for
questions, non-sensitive bugs, and feature proposals. Include the commit,
target, enabled features, smallest non-sensitive fixture, expected Pillow
result, and actual Rust result.

Read [CONTRIBUTING.md](CONTRIBUTING.md) before changing codecs, fixtures, or
ported code. The repository requires strict Clippy, exact manifest parity,
feature/target checks, complete retained coverage, and provenance updates.

Report vulnerabilities privately as described in
[SECURITY.md](SECURITY.md). Do not publish exploit details or malicious
fixtures in an issue.

## License and attribution

Original project code is available under your choice of Apache-2.0 or MIT.
The combined distribution also includes source-derived portions under
BSD-2-Clause, BSD-3-Clause, Zlib, IJG, and MIT-CMU terms.

[NOTICE.md](NOTICE.md) maps repository paths to applicable terms.
[third_party/README.md](third_party/README.md) records exact upstream versions,
revisions, hashes, roles, and retained texts. The root [PATENTS](PATENTS) file
contains the Alliance for Open Media patent license required for AV1
distribution.

This software is based in part on the work of the Independent JPEG Group.
