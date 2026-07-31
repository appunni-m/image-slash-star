# Changelog

All notable changes will be documented in this file. This project follows
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Manifest-driven Pillow 12.2.0 parity suite with exact decoded-pixel and
  encoded-file comparisons.
- Feature-gated JPEG, PNG, GIF, BMP, TIFF, WebP, ICO, and native AVIF codec
  modules.
- Exact AVIF parity through libavif 1.4.1, dav1d 1.5.3, and libaom 3.13.2,
  including still images, animations, metadata, color modes, and save options.
- Manifest fixtures with zero planned or skipped rows and 100% LLVM line,
  function, branch, and region coverage.
- Pinned native oracle identities, a checksummed third-party provenance
  inventory, complete upstream license texts, and the AOM patent notice at the
  source-package root.
- Structured `ImageResult` failures across the canonical detect, inspect,
  decode, sequence, and encode APIs.
- A shared `DecodePolicy` with inclusive limits for pre-detection encoded
  bytes, inspected canvas width/height/pixels, and the primary decoded
  transfer-byte length, the inspected frame/page count, every later
  frame/page's decoded bytes, and the cumulative retained sequence bytes, plus
  typed `LimitExceeded` failures and retry-safe lazy-source behavior.
- A sequence-policy defensive manifest with 32 frame-count, later-frame-byte,
  cumulative-byte, and precedence cases across inspection, still/sequence
  decode, immutable-source construction, and lazy materialization.
- Explicit verification-strength requests through `verify_with_scope`, with a
  never-provided `FullPixels` scope and format-qualified `Unsupported` failure
  instead of silently downgrading stronger requests.
- Container-defined consumed extents on `Decoded` (`consumed_bytes`) with a
  pinned per-format trailing-input policy: well-formed trailing bytes are
  ignored by every decoder, and AVIF container validation now accepts trailing
  bytes exactly as Pillow 12.2.0/libavif do.
- Format-qualified typed encoder option records for every codec, including a
  strict legacy-pair migration adapter and ordered AVIF advanced options.
- Persistent lazy `EncodedImage` inspection and decode caching that retains
  exact source format and decoded mode.
- A consolidated open-source documentation set covering architecture, the
  public contract, AVIF portability, oracle testing, and the release roadmap.
- Portable AV1 tile-boundary validation with exact multi-tile success/error
  fixtures and pinned dav1d scalar-entropy trace vectors.
- Portable lossless AVIF materialization for the first closed 4:4:4
  single-leaf classes, including square/padded leaves through 16x16, one-axis
  16x8 and 8x16 rectangular leaves, exact DC/vertical/horizontal luma
  prediction, and nonzero DC-only or zero-residual transform paths.
- Portable lossless AVIF materialization for the first closed two-leaf
  recursive split in 12x4, 16x4, 12x8, 16x8, 4x12, 4x16, 8x12, and 8x16
  frames, with shared partition/block CDF mutation, spatial luma-mode
  contexts, all-skip second leaves, exact reconstructed left/top edge
  prediction, and partial or full visibility on both axes. The pinned
  independent dav1d oracle now covers 92 complete reconstruction cases.
- Portable lossless AVIF materialization for the first closed 12x12 and 16x16
  four-leaf square splits, with interleaved child partition symbols, shared
  adaptive block state, DC-only spatial-neighbor restrictions, boundary
  transform prediction, direct high-token magnitudes below 15, the token-15
  Golomb extension, declared-frame visibility, and exact Y/U/V reconstruction
  for positive and negative residual signs. The 12x12 class also includes the
  first luma-only EOB-1 AC coefficient path and its complete inverse 4x4 WHT.

### Changed

- Renamed the package to `image-slash-star`.
- Made codec implementations and format dispatchers private; callers use one
  structured root API rather than public `Option`-returning codec helpers.
- Made every image format independently feature-gated, with ICO explicitly
  forwarding its PNG and BMP container requirements.
- Removed target-free encoder defaults and catch-all string maps; explicit
  encode targets now reject option records for another codec.
- Made public codec/capability vocabulary enums non-exhaustive while retaining
  exhaustive closed value domains such as source byte order.
- Made `DecodedSequence::first()` return the complete frame and added the
  explicitly lossy `first_image()` convenience.
- Added Pillow-recognized extension aliases and public MIME/canonical/alias
  queries on `ImageFormat`.
- Added portable AVIF container inspection and in-tree AV1 parsing groundwork.

### Removed

- Removed the general image-buffer and `DynamicImage` compatibility layer,
  including resize, crop, rotate, flip, conversion, blending, and other image
  processing behavior.
- Removed ICO's implicit resampling. ICO encoding now accepts only the
  source-sized entry supplied by the caller.
- Removed Serde and serde_json from development targets; manifest-driven tests
  use a strict project-owned test-only JSON reader.
- Removed per-sweep coverage logs and downstream `pillow-rs` migration plans
  from the maintained documentation tree; their binding decisions now live in
  four current project documents and historical detail remains in Git.
