# image/*

[![CI](https://github.com/appunni-m/image-slash-star/actions/workflows/ci.yml/badge.svg)](https://github.com/appunni-m/image-slash-star/actions/workflows/ci.yml)
[![License: MIT or Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#licensing)

`pillow-rs-image` is a safe-Rust image codec library built for exact observable
compatibility with a pinned Pillow distribution. It has no native runtime
dependencies, works on WASM, keeps every image format behind a Cargo feature,
and uses `bytemuck` as its only runtime utility dependency.

The project is pre-release software. All 504 currently active manifest rows
compare exact decoded pixels or exact encoded files successfully. Six AVIF
decode rows remain explicitly planned; AVIF is not represented as complete.

## Why this project exists

Ordinary compatibility tests often stop at successful decoding, dimensions,
or output length. This project treats Pillow 12.2.0 as a pinned behavioral
oracle and checks the public result byte-for-byte:

- decode status, error behavior, mode, dimensions, frames, metadata, and pixels;
- deterministic encoded file bytes, followed by decoded-pixel roundtrips;
- image operations such as conversion, flipping, rotation, and cropping.

No runtime call is made into Pillow or a C codec. Native libraries bundled in
the Pillow wheel define the reference behavior only; the crate implements that
behavior in Rust.

## Format features

Default features enable JPEG, PNG, GIF, BMP, TIFF, WebP, and ICO. AVIF is
opt-in and incomplete.

| Feature | Default | Status | Pinned oracle implementation |
| --- | --- | --- | --- |
| `jpeg` | yes | parity rows active | libjpeg-turbo 3.1.4.1 |
| `png` | yes | parity rows active | Pillow libImaging 12.2.0 / zlib-ng 2.3.3 |
| `gif` | yes | parity rows active | Pillow libImaging 12.2.0 |
| `bmp` | yes | parity rows active | Pillow libImaging 12.2.0 |
| `tiff` | yes | parity rows active | libtiff 4.7.1 |
| `webp` | yes | parity rows active | libwebp 1.6.0 |
| `ico` | yes | parity rows active | Pillow libImaging 12.2.0 |
| `avif` | no | 6 decode rows planned | libavif 1.4.1 / dav1d 1.5.3 / libaom 3.13.2 |

Select only the formats an application needs:

```toml
[dependencies]
pillow-rs-image = {
    version = "0.1.0",
    default-features = false,
    features = ["jpeg", "png"]
}
```

## API

The format is detected from the input bytes. Still-image and sequence APIs are
separate so animation frames are never silently discarded.

```rust,no_run
use pillow_rs_image::{ImageFormat, decode, encode_default};

let input = std::fs::read("input.png")?;
let image = decode(&input).ok_or("unsupported or invalid image")?;
let output = encode_default(&image, ImageFormat::Png)
    .ok_or("image cannot be encoded as PNG")?;
std::fs::write("output.png", output)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Use `decode_sequence` and `encode_sequence` for animated images. Lower-level
format modules are available under `codecs::<format>` when a caller already
knows the format.

## Reproducing parity

The authoritative oracle is the Pillow 12.2.0 CPython 3.12 macOS arm64 wheel.
Its wheel hash, extension hash, codec versions, and public API contract are
pinned in `pillow-oracle.lock.yaml` and `manifest.yaml`.

Create the oracle environment on macOS arm64:

```bash
python3.12 -m venv .oracle-venv
.oracle-venv/bin/python -m pip install --require-hashes -r oracle-requirements.txt
```

Generate deterministic assets and exact Pillow references, then run the one
manifest-driven integration suite:

```bash
.oracle-venv/bin/python scripts/generate_test_assets.py
.oracle-venv/bin/python scripts/generate_decode_refs.py
cargo test --all-features --test coverage_matrix_tests
```

The generator refuses to rewrite references if the Python, platform, Pillow
wheel, extension hash, or bundled codec versions differ from the lock. A
different wheel is a different oracle.

The fixture tree contains deterministic source images, normalized expected
metadata, raw Pillow pixels, and exact Pillow encoder output. Failed byte
comparisons report the first differing pixel coordinate, channel, and values;
hashes and file sizes are not accepted as parity substitutes.

## Development

The required Rust release and components are pinned in `rust-toolchain.toml`.
Before submitting a change, run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features
cargo test --all-features --test coverage_matrix_tests
cargo check --no-default-features
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the source-provenance and parity
workflow. Security issues should follow [SECURITY.md](SECURITY.md).

## Architecture

Each format owns its implementation under `src/codecs/<format>/`, including
format-specific encode and decode modules. Cargo features select these modules
at compile time. Shared primitives are limited to image types and compression
code that genuinely crosses format boundaries.

The AVIF boundary is deliberately strict. Pillow uses libavif, dav1d, and
libaom as its oracle stack, while this crate must implement the observable
ISOBMFF, AV1 decode, deterministic AV1 encode, color, alpha, grid, metadata,
and sequence behavior in Rust. Substituting a different encoder cannot prove
byte-identical libaom output.

## Licensing

Original project code is available under your choice of
[Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT). The crate as a combined
distribution is also subject to BSD-3-Clause, Zlib, IJG, and MIT-CMU terms for
ported and derived portions. [NOTICE.md](NOTICE.md) maps repository paths to
exact upstream versions and retained license files under `third_party/`.

This software is based in part on the work of the Independent JPEG Group.
