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
- Persistent lazy `EncodedImage` inspection and decode caching that retains
  exact source format and decoded mode.
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
  independent dav1d oracle now covers 86 complete reconstruction cases.

### Changed

- Renamed the package to `image-slash-star`.
- Made codec implementations and format dispatchers private; callers use one
  structured root API rather than public `Option`-returning codec helpers.
- Made every image format independently feature-gated, with ICO explicitly
  forwarding its PNG and BMP container requirements.
- Added portable AVIF container inspection and in-tree AV1 parsing groundwork.

### Removed

- Removed the general image-buffer and `DynamicImage` compatibility layer,
  including resize, crop, rotate, flip, conversion, blending, and other image
  processing behavior.
- Removed ICO's implicit resampling. ICO encoding now accepts only the
  source-sized entry supplied by the caller.
- Removed Serde and serde_json from development targets; manifest-driven tests
  use a strict project-owned test-only JSON reader.
