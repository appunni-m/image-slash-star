# pillow-rs-image

`pillow-rs-image` contains the standalone image codec layer used by pillow-rs.
Its fixture suite is pixel-parity based: generated PIL reference pixels are
stored as raw byte files, and Rust tests compare decoded pixels byte-for-byte.

## Project Goal

The goal of this project is to provide pure Rust image codecs with no external
codec dependencies and pixel- and byte-exact compatibility with one pinned
Pillow distribution. Each codec should reproduce Pillow's public observable
behavior without calling into Pillow or C code. `bytemuck` remains the sole
foundational utility dependency.

## Codec Features

Every image format is independently selectable through Cargo features. The
default feature set preserves the common JPEG, PNG, GIF, BMP, TIFF, WebP, and
ICO codecs; AVIF remains opt-in. Consumers that need a smaller build can disable
defaults and select only the required formats:

```toml
pillow-rs-image = {
  version = "0.1.0",
  default-features = false,
  features = ["jpeg", "png"]
}
```

The available format features are `jpeg`, `png`, `gif`, `bmp`, `tiff`, `webp`,
`ico`, and `avif`. Full ICO support enables its BMP and PNG container payload
dependencies automatically.

## Reference Oracles

One exact Pillow 12.2.0 CPython 3.12 macOS arm64 wheel is the authoritative
oracle. Its wheel hash, extension hash, and bundled codec versions are pinned in
`pillow-oracle.lock.yaml` and mirrored under `reference_oracles` in
`manifest.yaml`. Parity means matching Pillow's public success/error behavior,
mode, dimensions, frames, metadata, decoded pixels, and deterministic encoded
file bytes. The committed fixtures are cached observations from that oracle;
they are not a substitute for the pinned oracle identity.

## Fixture Workflow

Run the full local workflow from the repository root:

```bash
.oracle-venv/bin/python scripts/generate_test_assets.py
.oracle-venv/bin/python scripts/generate_decode_refs.py
cargo test --all-features --test coverage_matrix_tests
```

To regenerate only one format:

```bash
.oracle-venv/bin/python scripts/generate_decode_refs.py --format png
cargo test --all-features --test coverage_matrix_tests test_decode_matrix
```

The workflow is self-contained except for the Python tooling used to generate
PIL references:

- `Pillow`
- `PyYAML`

Create the pinned oracle environment with CPython 3.12 on macOS arm64:

```bash
python3.12 -m venv .oracle-venv
.oracle-venv/bin/python -m pip install --require-hashes -r oracle-requirements.txt
```

The generator refuses to rewrite references unless both the Pillow release and
its JPEG, zlib-ng, TIFF, WebP, and AVIF backend versions match `manifest.yaml`.
Use `.oracle-venv/bin/python`; a different OS or wheel is a different oracle and
must not silently regenerate the committed references.

## Fixture Layout

```text
tests/fixtures/
  coverage_matrix.json          Codec coverage and active/planned rows
  input/images/<format>/        Deterministic source images
  input/jsons/                  Per-format input case lists
  outputs/jsons/                Per-format expected metadata
  outputs/raws/                 Raw PIL pixel references
  outputs/encoded/              Exact files emitted by Pillow encoders
```

Each active reference row records:

- `ref_path`: raw byte file relative to this repository root
- `ref_bytes`: exact expected byte count
- `ref_mode`: PIL mode mapped to the Rust-facing mode name where possible
- `ref_size`: `[width, height]`
- `encoded_ref_path` and `encoded_ref_bytes` for encode rows: exact Pillow file
  output, compared byte-for-byte before the pixel roundtrip assertion

Input and output JSON files use schema version 3 and embed the exact Pillow
profile plus the normalized `Image.open`/`Image.save` call. Active error rows
record Pillow's exception type and a deterministic message with runtime object
addresses normalized.

`ref_sha256` is intentionally not part of newly generated fixture metadata.
Hashes can prove that bytes changed, but they do not show which pixel changed.
The Rust test harness compares the actual bytes directly and reports the first
pixel coordinates, channel indexes, and byte values that differ.

## Asset Coverage

`scripts/generate_test_assets.py` creates deterministic source images with
gradients, checker patterns, hard edges, alpha ramps, odd sizes, large sizes,
palette variants, compression variants, metadata chunks, and corrupted/truncated
files. The manifest can still declare cases that Pillow cannot generate in a
portable way, such as AVIF variants; those remain skipped until matching source
assets are added.

The generated decode corpus currently covers:

- JPEG quality levels, chroma subsampling, grayscale/RGB/CMYK inputs,
  progressive/restart-marker files, trailing data, multiple EOI markers,
  1x1/8x8/odd-size images, and corrupt/truncated inputs.
- PNG color types, 1/2/4/8/16-bit samples, palette and transparency variants,
  Adam7, compression levels, metadata chunks, APNG-compatible files, odd
  dimensions, narrow/tall images, and CRC/signature failures.
- GIF static/animated/interlaced/transparent files, local/global color tables,
  Graphic Control Extension coverage, and empty inputs.
- BMP bit depths, row-padding widths, bottom-up/top-down fixtures, V3/V4/V5 and
  OS/2 headers, bitfields, and actual RLE4/RLE8 streams built from the format
  specification and decoded by Pillow.
- WebP lossy quality levels, lossless, alpha, odd dimensions, animation, and
  ICC/XMP/EXIF containers.
- TIFF endian, compression, grayscale/RGBA, strip/tile/multipage, palette/CMYK
  planned gaps, and bad IFD inputs.
- ICO single/multi-size and PNG/BMP-entry style fixtures.

As of the current generated matrix, decode has 168 rows: 156 active, 12 planned,
and 146 exact pixel references (10 active error cases intentionally have no
pixel file). Encode has 156 unique rows: 94 active, 62 explicit planned gaps,
and 94 exact Pillow roundtrip pixel references. Active lossy and lossless rows
both carry references because the exact Pillow wheel makes generation
deterministic.
