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
> legal file in each format specification. Caller-controlled decode limits are
> not implemented, so the current crate should not be treated as hardened for
> arbitrary hostile inputs. Breaking API changes may occur before 1.0.

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
| `encode(&DecodedImage, ImageFormat, &EncodeOptions)` | Encode one image with explicit options |
| `encode_default(&DecodedImage, ImageFormat)` | Encode one image with defaults |
| `encode_sequence(&DecodedSequence, ImageFormat, &EncodeOptions)` | Encode one frame to any enabled format or multiple frames to GIF, TIFF, WebP, or native AVIF |
| `EncodedImage::new(bytes)` | Inspect an immutable source now and decode it lazily |

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

`ImageError` is non-exhaustive; downstream `match` expressions need a fallback
arm. Unchanged malformed bytes should not be retried. Feature and unsupported
errors can usually be handled by selecting another compiled capability.

Use `error.kind()` for stable recovery policy and `error.format()` for the
selected input/output format when one is known. `error.message()` returns the
retained high-level diagnostic for logs. In particular, `Dimensions` and
`Parameter` retain both optional format and diagnostic context; callers do not
need to parse `Display` output. Diagnostic prose may become more specific, so
it is not a substitute for `ImageErrorKind`.

## Correctness evidence

The generated matrix in this tree contains 1,417 active cases:
1,024 decode/inspect/verify cases and 393 encode cases, with zero
planned or unwired rows. Expected errors are active fixture outcomes.

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
