# image-slash-star

[![CI](https://github.com/appunni-m/image-slash-star/actions/workflows/ci.yml/badge.svg)](https://github.com/appunni-m/image-slash-star/actions/workflows/ci.yml)
[![License: multi-license](https://img.shields.io/badge/license-see%20NOTICE-blue.svg)](#license)

Image codec implementation with byte-exact parity against a pinned Pillow
oracle.

The Cargo package is `image-slash-star`; Rust source imports it as
`image_slash_star`.

The default JPEG, PNG, GIF, BMP, TIFF, WebP, and ICO codecs are 100% Rust:
zero Pillow imports and zero native codec libraries. `bytemuck` remains the
only third-party Rust runtime dependency. The opt-in `avif` feature uses the
exact native library stack used by the oracle because a different AV1 encoder
cannot produce libaom-identical bytes.

The crate publishes three API surfaces:

- A high-level byte API: format detection, still-image decode/encode, and
  sequence decode/encode.
- Feature-scoped codec modules under `codecs::<format>` for callers that
  already know the image format.
- Pillow-observable image types that retain pixels, modes, palettes, frame
  timing, disposal, background metadata, and encoder options.

Project goal: exact Pillow 12.2.0 parity across public image behavior — success
or error, mode, dimensions, metadata, frame data, decoded pixels, and
deterministic encoded file bytes. Pillow itself remains fixture-only; the
explicitly enabled AVIF feature is the sole native runtime boundary.

## Status

The manifest-driven parity matrix is the source of truth.

| Metric | Count |
| --- | ---: |
| Manifest rows | 1,037 |
| Active manifest rows | 1,037 |
| Active decode rows | 709 |
| Active encode rows | 296 |
| Operation rows | 32 |
| Planned or skipped rows | 0 |
| Formats tracked | 8 |

All rows compare exact decoded pixels, exact sequence frames, exact encoded
files, or an exact oracle success/error outcome. AVIF contributes six decode
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
| `avif` | no | parity rows active | libavif 1.4.1 / dav1d 1.5.3 / libaom 3.13.2 |

Select only the formats an application needs by disabling default features and
enabling the relevant format features.

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
export PILLOW_RS_AVIF_LIB_DIR=/path/to/the/exact/libavif/lib
cargo test --all-features --test coverage_matrix_tests
```

The build also accepts an exact `pkg-config` `libavif` installation. Every
operation checks the loaded libavif and codec versions at runtime. On macOS
arm64, the pinned oracle environment described below supplies the same bundled
library used to create the references. AVIF compiles on `wasm32` so feature
unification remains safe, but its operations return unsupported there.

Linux contributors can build the complete pinned stack with the same flags as
Pillow's wheel build:

```bash
scripts/build_avif_stack.sh /tmp/image-star-avif /tmp/image-star-avif-build
PILLOW_RS_AVIF_LIB_DIR=/tmp/image-star-avif/lib \
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
| `decode(&[u8])` | Decode one still image from auto-detected bytes. |
| `decode_sequence(&[u8])` | Decode retained frames and animation metadata. |
| `encode(&DecodedImage, ImageFormat, &EncodeOptions)` | Encode with explicit format options. |
| `encode_default(&DecodedImage, ImageFormat)` | Encode a still image with default options. |
| `encode_sequence(&DecodedSequence, ImageFormat, &EncodeOptions)` | Encode still or animated sequences while retaining frame metadata. |

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
cargo clippy --workspace --all-targets --all-features
cargo test --all-features --test coverage_matrix_tests
cargo check --no-default-features
```

Coverage work should first add manifest-backed Pillow fixtures when a missing
path is public image behavior. `cfg(coverage)` hooks are reserved for private
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

- Keep default runtime codec execution pure Rust and AVIF confined to its
  fixed, opt-in native boundary.
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
MIT-CMU terms for ported, derived, and retained portions. `NOTICE.md` maps
repository paths to exact upstream versions and retained license files under
`third_party/`.

This software is based in part on the work of the Independent JPEG Group.
