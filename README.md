# image-slash-star

[![CI](https://github.com/appunni-m/image-slash-star/actions/workflows/ci.yml/badge.svg)](https://github.com/appunni-m/image-slash-star/actions/workflows/ci.yml)
[![License: multi-license](https://img.shields.io/badge/license-see%20NOTICE-blue.svg)](#license)

Image codec implementation with byte-exact parity against a pinned Pillow
oracle.

The Cargo package is `image-slash-star`; Rust source imports it as
`image_slash_star`.

This crate is intentionally limited to encoded-image detection, inspection,
decoding, and encoding. It does not provide resizing, cropping, rotation,
flipping, compositing, drawing, color adjustment, filters, or a mutable
`DynamicImage`-style processing layer. Applications should keep those concerns
in a downstream crate.

The default JPEG, PNG, GIF, BMP, TIFF, WebP, and ICO codecs are 100% Rust:
zero Pillow imports and zero native codec libraries. `bytemuck` remains the
only Cargo dependency, including development targets. The opt-in `avif`
feature uses the
exact native library stack used by the oracle because a different AV1 encoder
cannot produce libaom-identical bytes. That native AVIF implementation is
current compatibility behavior, not the final portable release design: it
compiles on `wasm32-unknown-unknown`, where detection, bounded container
inspection, and a growing manifest-bounded still-decode subset are portable.
That subset includes closed lossless full-range YUV 4:4:4 and 4:2:0
single-leaf, two-leaf, and four-leaf classes over the documented 4x4 through
16x16 geometries, plus the first 4x4 and 8x8 lossy 4:2:0 directional-predictor
classes with skipped, direct-token, or token-15 Golomb DC-only luma residuals.
That closed residual map currently reaches final tokens 32 and 33; adjacent
final tokens 40 and 41 remain explicit non-portable controls.
Other recursive partitions and AVIF pixel decode classes, plus AVIF encoding,
still report an unsupported codec operation on that target. The exact accepted
AV1 boundary is tracked in `docs/portable-avif-progress.md`; it is deliberately
narrower than general AVIF support.

The crate publishes one canonical codec API: format detection, inspection,
still-image decode/encode, and sequence decode/encode over encoded bytes and
decoded samples. Its public result types retain pixels, modes, palettes, frame
timing, disposal, background metadata, and encoder options. Codec algorithms
and format dispatchers are private so malformed input cannot lose its
structured error merely because a caller selected a lower-level entry point.

Project goal: exact Pillow 12.2.0 parity across public codec behavior — success
or error, mode, dimensions, metadata, frame data, decoded pixels, and
deterministic encoded file bytes. Pillow itself remains fixture-only; the
explicitly enabled AVIF feature is the sole native runtime boundary.

## Status

The manifest-driven parity matrix is the source of truth.

| Metric | Count |
| --- | ---: |
| Manifest rows | 1,187 |
| Active manifest rows | 1,187 |
| Active decode rows | 909 |
| Active encode rows | 278 |
| Planned or skipped rows | 0 |
| Formats tracked | 8 |

All rows compare exact decoded pixels, exact sequence frames, exact encoded
files, or an exact oracle success/error outcome. AVIF contributes 167 decode
rows and 23 encode rows, including five-frame animation and invalid-input
behavior.

## Format features

Default features enable JPEG, PNG, GIF, BMP, TIFF, WebP, and ICO. AVIF is
opt-in because it links a fixed native stack.

| Feature | Default | Status | Pinned oracle implementation |
| --- | --- | --- | --- |
| `jpeg` | yes | parity rows active | libjpeg-turbo 3.1.4.1 |
| `png` | yes | parity rows active | Pillow libImaging 12.2.0 / zlib-ng 2.3.3 |
| `gif` | yes | parity rows active | Pillow libImaging 12.2.0 |
| `bmp` | yes | parity rows active | Pillow libImaging 12.2.0 |
| `tiff` | yes | parity rows active | libtiff 4.7.1 |
| `webp` | yes | parity rows active | libwebp 1.6.0 |
| `ico` | yes | parity rows active | Pillow libImaging 12.2.0 |
| `avif` | no | parity rows active; portable inspection plus closed lossless and initial lossy 4:2:0 still-decode classes | libavif 1.4.1 / dav1d 1.5.3 / libaom 3.13.2 / libyuv 1922 |

Select only the formats an application needs by disabling default features and
enabling the relevant format features.

Until the first crates.io release, add the repository directly:

```toml
[dependencies.image-slash-star]
git = "https://github.com/appunni-m/image-slash-star"
default-features = false
features = ["jpeg", "png"]
```

The Cargo package name contains hyphens; Rust code imports it as
`image_slash_star`.

ICO still-image encoding writes one entry at the source raster's existing
dimensions. If the `sizes` compatibility option is supplied, it must name that
same single size. The codec rejects different, multiple, empty, or
over-256-pixel requests instead of resizing pixels. A future multi-resolution
ICO API must accept independently supplied, already-sized entries.

## From source

```bash
git clone git@github.com:appunni-m/image-slash-star.git
cd image-slash-star
cargo check --all-targets
```

The repository uses the Rust 2024 edition. The required Rust release and
components are pinned in `rust-toolchain.toml`.

To enable AVIF on a native target, install libavif 1.4.1 built with dav1d
1.5.3 and libaom 3.13.2, or point the build at its library directory:

```bash
export IMAGE_SLASH_STAR_AVIF_LIB_DIR=/path/to/the/exact/libavif/lib
cargo test --all-features --test coverage_matrix_tests
```

The build also accepts an exact `pkg-config` `libavif` installation. Every
operation checks the loaded libavif and codec versions at runtime. On macOS
arm64, the pinned oracle environment described below supplies the same bundled
library used to create the references. AVIF compiles on `wasm32` so feature
unification remains safe. Detection and inspection use the portable in-tree
parser there. The closed portable AV1 single-, two-, and four-leaf decode
classes described above are active; inputs outside those proven classes and
all AVIF encoding remain unsupported pending later portable slices.

Linux contributors can build the complete pinned stack with the same flags as
Pillow's wheel build:

```bash
scripts/build_avif_stack.sh /tmp/image-star-avif /tmp/image-star-avif-build
IMAGE_SLASH_STAR_AVIF_LIB_DIR=/tmp/image-star-avif/lib \
  cargo test --all-features --test coverage_matrix_tests
```

## API at a glance

The high-level API detects image format from input bytes, decodes still images
without discarding animation-aware metadata, and keeps sequence APIs separate so
frames are never silently dropped.

Primary entry points:

| Function | Purpose |
| --- | --- |
| `detect_format(&[u8])` | Detect JPEG, PNG, GIF, BMP, WebP, TIFF, ICO, or AVIF from magic bytes. |
| `inspect(&[u8])` | Read format, dimensions, mode, depth, palette, and frame count without decoding pixels. |
| `decode(&[u8])` | Decode one still image from auto-detected bytes. |
| `decode_sequence(&[u8])` | Decode retained frames and animation metadata. |
| `encode(&DecodedImage, ImageFormat, &EncodeOptions)` | Encode with explicit format options. |
| `encode_default(&DecodedImage, ImageFormat)` | Encode a still image with default options. |
| `encode_sequence(&DecodedSequence, ImageFormat, &EncodeOptions)` | Encode still or animated sequences while retaining frame metadata. |

```rust,no_run
use image_slash_star::{decode, encode_default, inspect, ImageFormat, ImageResult};

fn transcode_png_to_jpeg(input: &[u8]) -> ImageResult<Vec<u8>> {
    let info = inspect(input)?;
    assert_eq!(info.format, ImageFormat::Png);

    let decoded = decode(input)?;
    assert_eq!(decoded.format, info.format);
    encode_default(&decoded.content, ImageFormat::Jpeg)
}
```

All inputs and outputs are byte buffers. Native applications may wrap the API
with `std::fs`; browser and worker applications may pass fetched or uploaded
bytes directly. The crate itself does not open paths or apply host filesystem
policy.

## Parity harness

The authoritative oracle is the Pillow 12.2.0 CPython 3.12 macOS arm64 wheel.
Its wheel hash, extension hash, bundled codec versions, and public comparison
contract are pinned in `pillow-oracle.lock.yaml` and `manifest.yaml`.

```
manifest.yaml
       ↓
scripts/generate_test_assets.py
       ↓
tests/fixtures/input/
       ↓
scripts/generate_decode_refs.py
       ↓
tests/fixtures/outputs/
       ↓
tests/coverage_matrix_tests.rs
```

The generated fixture tree contains deterministic source images, normalized
expected metadata, raw Pillow pixels, and exact Pillow encoder output. Hashes,
file sizes, and approximate visual similarity are not accepted as parity
substitutes.

### Running the parity gate

Create the pinned oracle environment on macOS arm64:

```bash
python3.12 -m venv .oracle-venv
.oracle-venv/bin/python -m pip install --require-hashes -r oracle-requirements.txt
```

Regenerate deterministic assets and references, then run the parity suite:

```bash
.oracle-venv/bin/python scripts/generate_test_assets.py
.oracle-venv/bin/python scripts/generate_decode_refs.py
cargo test --all-features --test coverage_matrix_tests
```

The matrix target requires every format feature by design; partial-feature
builds are checked independently and never reinterpret unavailable codecs as
passing or skipped parity rows.

The generator refuses to rewrite references if the Python version, platform,
Pillow wheel hash, extension hash, or bundled codec versions differ from the
lock file. A different wheel is a different oracle.

## Development

Before submitting a change, run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --all-features --test coverage_matrix_tests
cargo check --no-default-features
python3 scripts/verify_third_party_licenses.py
```

Coverage work should first add manifest-backed Pillow fixtures when a missing
path is public codec behavior. `cfg(coverage)` hooks are reserved for private
state machines, generated helper states, or defensive limits that cannot be
represented as a public Pillow fixture.

## Architecture

Each format owns its implementation under `src/codecs/<format>/`, with
format-local encode and decode modules. Cargo features select those modules at
compile time. Shared code is limited to image types and compression primitives
that genuinely cross format boundaries.

```
&[u8]
  ├─ detect_format()
  ├─ decode()          → DecodedImage { dimensions, mode, palette, pixels }
  └─ decode_sequence() → DecodedSequence { frames, timing, disposal, background }

DecodedImage / DecodedSequence
  └─ encode*()         → exact Pillow-observable container bytes
```

Transforms intrinsic to a codec—such as JPEG IDCT, PNG filtering, YUV/RGB
sample reconstruction, and animation disposal needed to reproduce decoded
frames—remain private implementation details. They do not constitute a general
image-processing API. Container convenience behavior that transforms an
arbitrary source raster is excluded: in particular, ICO encoding never creates
smaller entries by resampling the input.

The AVIF boundary is deliberately strict. A small repository-owned C bridge
uses libavif 1.4.1 for container/color behavior, dav1d 1.5.3 for decoding, and
libaom 3.13.2 for encoding. Unsafe Rust is isolated to one ownership wrapper;
all other code remains under the crate-wide unsafe-code denial. Substituting a
different encoder or version is rejected rather than treated as parity.

## Fixtures

Fixture inputs are generated from `manifest.yaml`. Generated references are
stored under `tests/fixtures/outputs/` and are version-controlled because they
define the byte contract.

When adding or changing fixtures:

- Add the public behavior to `manifest.yaml`.
- Regenerate assets and oracle outputs with the pinned Python environment.
- Keep the row only if Rust matches the exact Pillow status, metadata, pixels,
  or encoded bytes required by the row.
- Document non-fixture coverage work in `docs/coverage-branch-attack-plan.md`.

## Contributing

Start with `CONTRIBUTING.md`. The short version:

- Keep default runtime codec execution pure Rust and do not add public image
  processing.
- Treat the fixed, opt-in native AVIF boundary as migration debt until a
  portable in-tree AV1 implementation replaces it.
- Keep Pillow as offline oracle tooling and native codec calls confined to
  the AVIF feature.
- Prefer manifest-driven fixtures over narrow implementation probes.
- Do not weaken byte expectations, fixture metadata, or failure checks.
- Run the parity gate before claiming correctness.

Security issues should follow `SECURITY.md`.

## License

Original project code is available under your choice of
[Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT). The crate as a combined
distribution is also subject to BSD-2-Clause, BSD-3-Clause, Zlib, IJG, and
MIT-CMU terms for ported, derived, and retained portions.
[NOTICE.md](NOTICE.md) maps repository paths to those terms, and the
[third-party provenance inventory](third_party/README.md) records exact
versions, revisions, hashes, roles, and retained files.

The portable AV1 implementation and optional native AVIF stack are also
distributed with the [Alliance for Open Media Patent License 1.0](PATENTS).
Keep that file with source distributions; follow its written-notice
requirements when distributing an AV1 implementation in another form.

This software is based in part on the work of the Independent JPEG Group.
