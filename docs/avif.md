# AVIF support and portability boundary

Status: native manifest parity retained; portable implementation incomplete

Reviewed: 2026-08-04 on the committed tree based on revision
`15a4f02809bb2ae5be6bee817cad1bb72e1a2fb6`; the claim-ledger baseline remains
`f1048bc0399fad9801559ca7fcfd3163427b5832`.

AVIF is the only codec feature with different native and
`wasm32-unknown-unknown` capabilities. The WASM behavior below executes at
runtime on `wasm32-wasip1` (Node's WASI preview1) in every feature lane;
`wasm32-unknown-unknown` remains build/rustdoc-verified.

## Current behavior

| Operation | Native `avif` feature | `wasm32-unknown-unknown` |
| --- | --- | --- |
| Signature detection | Portable Rust | Portable Rust |
| Container inspection | Portable parser with native-compatible results | Portable Rust |
| Still decode | Fixed libavif/dav1d path | Closed, manifest-bounded portable AV1 subset |
| Sequence decode | Fixed libavif/dav1d path | Unsupported |
| Still encode | Fixed libavif/libaom path | Unsupported |
| Sequence encode | Fixed libavif/libaom path | Unsupported |

The generated matrix is the exact supported-case inventory. In the current
working tree, AVIF has 197 decode/inspect/error cases and 32 encode/error cases.
In a repository checkout, list them directly:

```bash
jq '.formats.avif' tests/fixtures/coverage_matrix.json
```

This is a case inventory, not a claim of complete AVIF or AV1 specification
support.

## Why native output is version-locked

The Pillow 12.2.0 wheel used as the oracle selects:

| Layer | Fixed implementation |
| --- | --- |
| Container and RGB/YUV behavior | libavif 1.4.1 |
| Decoder | dav1d 1.5.3 |
| Encoder | libaom 3.13.2 |
| Color helpers | fixed libyuv and SharpYUV revisions recorded in provenance |

Different AV1 encoders can produce valid but byte-different files. Exact
Pillow-encoded output therefore requires the same encoder and configuration.
The native bridge is retained as an intermediate compatibility path while the
repository-owned portable implementation is developed.

Runtime checks reject incompatible native codec versions rather than silently
weakening parity.

## Native setup

Native AVIF requires a C11 compiler, archiver, and the exact library stack.
The build script searches in this order:

1. `IMAGE_SLASH_STAR_AVIF_LIB_DIR`, optionally with
   `IMAGE_SLASH_STAR_AVIF_LIB_NAME`;
2. an exact `pkg-config` libavif 1.4.1 installation; or
3. the pinned `.oracle-venv` used for references.

Linux contributors can build the pinned stack:

```bash
scripts/build_avif_stack.sh /tmp/image-star-avif /tmp/image-star-avif-build
```

Then select it:

```bash
export IMAGE_SLASH_STAR_AVIF_LIB_DIR=/tmp/image-star-avif/lib
cargo test --locked --all-features --test coverage_matrix_tests
```

The script clones fixed upstream sources and verifies the libavif commit and
installed version. It requires `cmake`, `git`, `meson`, `ninja`, and
`pkg-config`.

## Portable implementation

The WASM path contains repository-owned:

- ISO-BMFF brand and box parsing;
- item, extent, property, grid, alpha, and sample extraction;
- bounded retention of recognized AVIF EXIF and XMP item extents as raw
  `OpaqueMetadata` records;
- AV1 sequence and frame-header parsing;
- tile-boundary validation;
- scalar entropy decoding with adaptive CDF state;
- partition and block traversal;
- prediction, residual reconstruction, and required color conversion for
  accepted classes; and
- exact independent entropy/reconstruction references generated from pinned
  upstream behavior.

The accepted still-decode subset includes the closed manifest classes for:

- lossless full-range YUV 4:4:4 and 4:2:0;
- selected single-, two-, and four-leaf geometries from 4×4 through 16×16;
- supported DC, vertical, and horizontal prediction paths;
- zero, direct-token, and accepted token-15 Golomb DC-only residual paths; and
- initial 4×4 and 8×8 lossy 4:2:0 directional-predictor cases.

The manifest and independent reconstruction JSON are authoritative when this
summary becomes too coarse. Inputs outside a proven class return a structured
`Unsupported` or `Malformed` error; they must not fall through to a partial
decode. On every `wasm32` target the unavailable operations return staged,
codec-level `Unsupported` errors ("AVIF sequence decoding requires the native
AVIF stack" for sequence decode, "AVIF encoding requires the native extra
module" for still and sequence encode) that match the capability table;
out-of-subset still decode returns "AVIF input is outside the portable WASM
decode subset" at the `StillDecode` stage. When an AVIF item declares an
alpha auxiliary item, `SourceDescriptor::alpha()` reports `Auxiliary`,
identifying that the alpha samples are carried by a separate image.
`SourceDescriptor::avif_auxiliary_relationship()` additionally retains the
direct source-local `auxl` relationship for `alpha.avif` (auxiliary item `2`
targets primary item `1`) while
`SourceDescriptor::avif_auxiliary_relationships()` retains the bounded alpha
links for the supported grid fixture (`5`→`2` and `6`→`3`). Both are present
on inspection, still decode, and the still-sequence fallback. Decoded transfer
bytes remain the documented normalized unassociated layout; these relationships
are source provenance only.
When an AVIF `prem` `iref` edge declares that an alpha item qualifies a color
item, `SourceDescriptor::avif_premultiplied_relationships()` retains that
source-local edge alongside the auxiliary relationship. The field reports
container provenance only; it does not premultiply or unpremultiply decoded
transfer bytes.
For the same grid, `SourceDescriptor::avif_grid_item_ids()` retains the ordered
derived color-item list (`[2, 3]`) on those three surfaces. It does not expose
tile placement or compose the grid.

The primary item's `colr`/`nclx` CICP declaration, `av1C` chroma sample position,
`clli` content-light-level
property, `mdcv` mastering-display color volume, and `colr`/`prof` or `rICC` ICC
profile are retained as source provenance in `SourceColor` on inspection, still
decode, and the still-sequence fallback. The record contains color primaries,
chroma sample position, transfer characteristics, matrix coefficients, the full-range flag, maxCLL,
maxPALL, exact mastering-display coordinates/luminance fields, and the exact
ICC profile kind and bytes; it never applies color conversion or tone mapping.
This is a bounded specification/defensive-model contract rather than Pillow
parity evidence, because Pillow's observable result has no equivalent item-level
structured color field. The test uses a committed Pillow-generated encoded
metadata output only as a source witness for ICC and does not add a parity row.
Typed non-primary `colr`/`nclx` declarations are retained separately through
`SourceDescriptor::avif_item_color_properties()` as source-local item ID/CICP
records on inspection, still decode, and the still-sequence fallback. They do
not merge into the primary `SourceColor`, perform conversion, or change
decoded pixels. The contract uses the in-memory `alpha.avif` association
mutation described in [testing](testing.md); Pillow exposes no equivalent
item-level field, so it remains Rust source-provenance evidence with no parity
row. Raw non-primary `prof`/`rICC` profiles are retained through
`SourceDescriptor::avif_item_icc_profiles()` as source-local item ID and exact
profile-kind/bytes records, without replacing primary `SourceColor` or changing
decoded pixels. Other item color/property forms remain open.
Recognized `Exif` items and `mime` items with content type exactly
`application/rdf+xml` now follow the same raw-retention boundary as the decoded
metadata records. The EXIF record preserves the item payload exactly, including
the four-byte AVIF TIFF-header offset prefix; the XMP record uses kind `XMP `.
Other non-primary item profiles, track-only and non-alpha auxiliary item
properties, item color/property forms beyond typed CICP and raw ICC, and other
non-primary item relationships remain future slices. Direct and
supported grid-derived item and auxiliary-alpha provenance is represented
through `SourceAlpha::Auxiliary`, the scalar
`SourceDescriptor::avif_auxiliary_relationship()` getter, and the bounded
`SourceDescriptor::avif_auxiliary_relationships()`,
`SourceDescriptor::avif_premultiplied_relationships()`, and
`SourceDescriptor::avif_grid_item_ids()` lists.

The primary item's `irot`, `imir`, `pasp`, and `clap` properties are retained
in `SourceDescriptor::avif_transform()` as `AvifTransformProperties`. `irot`
accepts the four legal counter-clockwise quarter-turn values, `imir` accepts
the top/bottom or left/right axis, and `pasp` retains its positive horizontal
and vertical spacing values through `AvifPixelAspectRatio`. `clap` retains its
positive width/height fractions and signed horizontal/vertical offsets through
`AvifCleanAperture`. These declarations are source provenance only: decoded
pixels are never rotated, mirrored, rescaled, or cropped. Other non-primary
item-level color/properties, non-alpha auxiliary relationships, full grid
topology, and other item metadata remain open; bounded direct/grid item,
auxiliary-alpha, and
premultiplication provenance is represented through `SourceAlpha::Auxiliary`
plus the source-local item relationships.

## Native FFI boundary

`src/codecs/avif/native.rs` is the only Rust module allowed to use unsafe code.
It owns libavif handles, retains input bytes for the required lifetime, checks
frame and buffer bounds, pairs bridge allocations with the matching free
function, and prevents unwinding across a foreign callback boundary.

`src/codecs/avif/native/bridge.c` exposes a narrow status-code API. Other codec
modules do not call C or expose the FFI surface publicly.

Licenses and patent grants for libavif, dav1d, libaom, libyuv, libwebp, and
copied/translated sources are retained under `third_party/`, `NOTICE.md`, and
the root `PATENTS` file.

## Completion criteria

Portable AVIF is complete only when:

1. every retained native AVIF manifest success and error has an equivalent
   portable result;
2. still and sequence decode retain exact format, mode, metadata, timing, and
   pixels;
3. still and sequence encode match the pinned output contract where exact bytes
   are required;
4. native and WASM semantic tests execute, not merely compile;
5. line, branch, function, and region coverage remain 100%;
6. no Cargo dependency is added beyond `bytemuck`; and
7. the native stack can be removed from the published runtime contract without
   weakening any active case.

Progress should update this document's current boundary, the manifest, and the
[roadmap](roadmap.md). Run-by-run exploration belongs in commits and Coverage
MCP artifacts rather than another permanent progress log.
