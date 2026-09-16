# image-slash-star

Rust codecs for detecting, inspecting, decoding, and encoding image bytes.
The library covers selected JPEG, PNG, GIF, BMP, TIFF, WebP, ICO/CUR, and
opt-in AVIF paths, with observable behavior compared against Pillow 12.2.0.

[Documentation](https://appunni-m.github.io/image-slash-star/) ·
[Capabilities](https://appunni-m.github.io/image-slash-star/capabilities/) ·
[Benchmarks](https://appunni-m.github.io/image-slash-star/benchmarks/) ·
[Rust API](https://docs.rs/image-slash-star/0.1.2/image_slash_star/)

**Current release: 0.1.2.** This is an early codec release. Compatibility is
limited to the selected manifest cases, with substantial planned API and AVIF
work. It is not a complete implementation of every image-format specification.
See [maturity](docs/MATURITY.md) before adopting it.

## Install

```toml
[dependencies]
image-slash-star = { version = "=0.1.2", default-features = false, features = ["png", "jpeg"] }
```

Rust 1.96.1 is required. There is one Cargo package and no npm or PyPI package.
Applications own filesystem and network I/O; the codec API consumes and returns
bytes and Rust values.

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
encoding requires an explicit output format. Other modes, palettes, metadata,
and sequences have their own contracts.

Continue with [API usage](docs/USAGE.md) and the
[Generated capability and direct-mode tables](docs/capabilities.md).

## Choose features and scope

| Feature | Default | Scope |
| --- | --- | --- |
| `jpeg`, `png`, `gif`, `bmp`, `tiff`, `webp` | Yes | Selected decode, encode, metadata, and sequence paths; see capabilities |
| `ico` | Yes | ICO/CUR handling; enables PNG and BMP dependencies |
| `avif` | No | Partial safe Rust still decoder and container inspection; planned paths remain explicit |
| `jpeg-wide-color` | No | Optional safe SIMD color candidate; enabled by deliberate choice |

Runtime code is Rust. `bytemuck` is a utility dependency; JPEG and AVIF enable
the optional `wide` dependency. The committed Cargo lockfile fixes CI's
resolved graph. C codec libraries are test/benchmark oracles, not runtime
fallbacks.

This crate does not resize, crop, rotate, draw, filter, or otherwise edit images.
Keep image processing in the consuming application or a library such as
[pillow-rs](https://github.com/appunni-m/pillow-rs).

## Errors and resource limits

<!-- image-error-policy: typed-recovery-diagnostic-prose -->

Recover using typed error kinds, stages, and reasons. Diagnostic `message()`
and `Display` wording are not a parsing or equality contract.
`DecodePolicy` bounds the documented input/result dimensions and work;
`EncodePolicy` includes an encoded-output length limit. These limits do not
bound all transient allocation, wall-clock time, or recoverable out-of-memory
behavior. Defaults are compatibility-oriented and unlimited.

The library is not advertised as hardened for arbitrary hostile inputs.
Read [API usage and limits](docs/USAGE.md) and [security](SECURITY.md).

## Evidence and performance

The [evidence guide](docs/EVIDENCE.md) separates active/planned fixture rows,
dated coverage, and accepted release checks. A compiled feature or present
function name does not establish every operation and target combination.

The [benchmark site](https://appunni-m.github.io/image-slash-star/benchmarks/)
shows the complete recorded JPEG/TurboJPEG operation matrix with source and
host identity. It does not represent other codecs or general image processing.
[Methodology](docs/BENCHMARKING.md) explains the timing boundary and CMYK caveat.

## Contribute

Start with [Contributing](CONTRIBUTING.md), the
[command reference](docs/testing.md), and [architecture](docs/architecture.md).

```sh
make help
make build
make verify
```

For the public website, use `make docs-setup`, `make docs-build`, and
`make docs-serve`. Main CI publishes GitHub Pages from this repository.

## Project information

[Support](SUPPORT.md) · [Releases](RELEASING.md) · [Changelog](CHANGELOG.md) ·
[Code of conduct](CODE_OF_CONDUCT.md) · [Public roadmap](docs/roadmap-new.md)

This project contains original and translated work under multiple licenses.
Read [NOTICE.md](NOTICE.md), the retained license files, and
[third-party attribution](third_party/README.md) before redistributing it.

## Acknowledgements

Thank you to [Pillow](https://python-pillow.org/) and its contributors for the
reference codec behavior, and [Puhu](https://github.com/bgunebakan/puhu) for the
Rust/Python image-processing work that informed the parent project's early
exploration. Thank you also to the codec authors credited in
[NOTICE.md](NOTICE.md) for their implementations, research, and test material.
