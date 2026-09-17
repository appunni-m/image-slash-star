# image-slash-star

[![CI](https://github.com/appunni-m/image-slash-star/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/appunni-m/image-slash-star/actions/workflows/ci.yml)
[![Documentation](https://github.com/appunni-m/image-slash-star/actions/workflows/docs.yml/badge.svg?branch=main)](https://github.com/appunni-m/image-slash-star/actions/workflows/docs.yml)
[![Benchmarks](https://github.com/appunni-m/image-slash-star/actions/workflows/benchmark.yml/badge.svg?branch=main)](https://github.com/appunni-m/image-slash-star/actions/workflows/benchmark.yml)
[![Release](https://github.com/appunni-m/image-slash-star/actions/workflows/release.yml/badge.svg)](https://github.com/appunni-m/image-slash-star/actions/workflows/release.yml)
[![Latest release](https://img.shields.io/github/v/release/appunni-m/image-slash-star?include_prereleases&sort=semver)](https://github.com/appunni-m/image-slash-star/releases)

<!-- release:summary -->
**Latest release: [0.1.3](https://github.com/appunni-m/image-slash-star/releases/tag/v0.1.3).**
<!-- /release:summary -->

Rust codecs for detecting, inspecting, decoding, and encoding image bytes.
Supports selected JPEG, PNG, GIF, BMP, TIFF, WebP, and ICO/CUR operations,
with optional partial AVIF decoding.

[Documentation](https://appunni-m.github.io/image-slash-star/) ·
[Supported formats](https://appunni-m.github.io/image-slash-star/maturity/) ·
[Benchmark results](https://appunni-m.github.io/image-slash-star/benchmarks/)

## Install

Add the crates.io package to your application's `Cargo.toml`:

<!-- release:cargo -->
```toml
[dependencies]
image-slash-star = "=0.1.3"
```
<!-- /release:cargo -->

Requires Rust 1.96.1 or newer. There is one Cargo package and no npm or PyPI
package. Applications own filesystem and network I/O; the API uses bytes and
Rust values.

## Encode and decode a PNG

```rust
use image_slash_star::{
    ColorType, DecodedImage, ImageFormat, ImageResult, decode, encode_default,
};

fn main() -> ImageResult<()> {
    let image = DecodedImage::try_new(
        3, 2, [255, 12, 34].repeat(6), ColorType::Rgb8,
    )?;
    let png = encode_default(&image, ImageFormat::Png)?;
    let decoded = decode(&png)?;
    assert_eq!(decoded.format, ImageFormat::Png);
    assert_eq!(decoded.content.pixels, image.pixels);
    Ok(())
}
```

The checked constructor validates dimensions and pixel layout. RGB8 data is
tightly packed, row-major RGB bytes. `decode` detects the input format;
encoding requires an explicit output format. Continue with [API usage](docs/USAGE.md).

## Choose formats

| Feature | Enabled by default | Scope |
| --- | --- | --- |
| `jpeg`, `png`, `gif`, `bmp`, `tiff`, `webp` | Yes | Selected decoding, encoding, and metadata operations |
| `ico` | Yes | ICO/CUR; also enables PNG and BMP |
| `avif` | No | Partial still-image decoder and container inspection; no supported encoder |
| `jpeg-wide-color` | No | Optional SIMD JPEG color conversion |

Disable unused formats with `default-features = false` and an explicit Cargo
`features` list. See [supported formats and limitations](docs/MATURITY.md).
Resizing, drawing, filtering, and other image editing are outside this crate;
use an image-processing library such as [pillow-rs](https://github.com/appunni-m/pillow-rs).

## Errors and resource limits

<!-- image-error-policy: typed-recovery-diagnostic-prose -->

Handle typed error kinds, stages, and reasons. Diagnostic text can change.
Decode and encode policies offer input, result, and work limits; defaults are
unlimited. They do not bound every intermediate allocation or wall-clock time.
Read [error recovery and limits](docs/USAGE.md) before processing untrusted files.

## Performance

[View benchmark results](https://appunni-m.github.io/image-slash-star/benchmarks/)
for JPEG comparisons with TurboJPEG. Results identify their source revision
and hardware; they do not represent every codec or general image processing.

## Contribute and get help

[Contributing](CONTRIBUTING.md) covers source builds, tests, and benchmark work.
The contributor references include the
[Generated capability and direct-mode tables](docs/capabilities.md) and
[evidence guide](docs/EVIDENCE.md).
[Support](SUPPORT.md) · [Security](SECURITY.md) ·
[Releases](https://github.com/appunni-m/image-slash-star/releases) ·
[Changelog](CHANGELOG.md) · [Code of conduct](CODE_OF_CONDUCT.md)

This project contains original and translated work under multiple licenses.
Read [NOTICE.md](NOTICE.md) and [third-party attribution](third_party/README.md)
before redistributing it.

## Acknowledgements

Thank you to the codec authors credited in [NOTICE.md](NOTICE.md) for their
implementations, research, and test material.
