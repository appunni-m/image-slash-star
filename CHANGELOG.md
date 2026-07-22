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
- Pinned native oracle identities and retained third-party license notices.
