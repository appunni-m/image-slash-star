# Third-party notices

Original work in this repository is licensed under either Apache-2.0 or MIT,
at your option. Portions are modified translations or source-derived
implementations of upstream open-source projects and remain subject to their
original terms. The exact revisions and source hashes are recorded in
`third_party/README.md`.

## Code and data incorporated into the crate

| Repository scope | Fixed upstream | Terms and retained text |
| --- | --- | --- |
| `src/codecs/webp/native/` | image-webp 0.2.4 | MIT OR Apache-2.0; `third_party/image-webp/` |
| WebP code under `src/codecs/webp/`, including marked native helpers and VP8 encoder code | libwebp 1.6.0 | BSD-3-Clause and the WebM patent grant; `third_party/libwebp/` |
| VP8 quantization tables in `src/codecs/webp/encode/vp8/quant.rs` | libvpx 1.15.2 | BSD-3-Clause and the WebM patent grant; `third_party/libvpx/` |
| `src/codecs/jpeg/` | libjpeg-turbo 3.1.4.1 and IJG libjpeg | IJG and BSD-style terms; `third_party/libjpeg-turbo/` |
| `src/codecs/compression/zlib_ng.rs` and marked related DEFLATE code | zlib-ng 2.3.3 | Zlib; `third_party/zlib-ng/LICENSE.md` |
| AVIF container and sample-table ports under `src/codecs/avif/`, plus copied libavif fixtures | libavif 1.4.1 | BSD-2-Clause and the complete upstream notice bundle; `third_party/libavif/` |
| Portable AV1 implementation under `src/codecs/avif/av1/` | dav1d 1.5.3 and libaom 3.13.2 | BSD-2-Clause; `third_party/dav1d/COPYING`, `third_party/libaom/LICENSE`, and the root `PATENTS` |
| Explicitly marked Pillow/libImaging source-derived portions and copied Pillow AVIF fixtures | Pillow 12.2.0 | MIT-CMU; `third_party/pillow/LICENSE` |
| GIF RGBA FASTOCTREE quantization in `src/codecs/gif/encode.rs` | Pillow 12.2.0 `QuantOctree.c`, Oliver Tonnhofer / Omniscale | MIT; `third_party/pillow/QUANT-OCTREE-LICENSE` |
| GIF palette-bucket ordering in `src/codecs/gif/encode.rs` | Apple Libc `stdlib/FreeBSD/qsort.c` | BSD-3-Clause; `third_party/apple-libc/LICENSE` |

All C-to-Rust ports in this repository are altered translations. Their
corresponding module comments and this notice identify that modification; no
ported Rust file should be represented as unmodified upstream source.

## Cargo dependency

The sole Cargo dependency is bytemuck 1.25.1, licensed under
Zlib OR Apache-2.0 OR MIT. Exact license texts are retained in
`third_party/bytemuck/`, and the package checksum is pinned in `Cargo.lock` and
`third_party/README.md`.

## AVIF oracle and source provenance

The `avif` feature is implemented in safe Rust and has no native build or
linking path. The following pinned projects identify the oracle and
source-derived reference material:

| Component | Fixed revision | Terms |
| --- | --- | --- |
| libavif | 1.4.1 | Complete upstream bundle in `third_party/libavif/LICENSE` |
| libaom | 3.13.2 | BSD-2-Clause plus Alliance for Open Media Patent License 1.0 in `third_party/libaom/` and root `PATENTS` |
| dav1d | 1.5.3 | BSD-2-Clause in `third_party/dav1d/COPYING` |
| libyuv | commit `6067afde563c3946eebd94f146b3824ab7a97a9c` | BSD-3-Clause in `third_party/libyuv/LICENSE` |
| libwebp SharpYUV | 1.6.0 | BSD-3-Clause and the WebM patent grant in `third_party/libwebp/` |

These projects are not Cargo dependencies and are not linked by the crate.
A source distribution retains the AOM patent license at its root. Distributors
of an AV1 implementation in another form should retain that license in the
documentation, legal notices, or other written materials provided with the
implementation, as specified by `PATENTS`.

## Fixtures, attribution, and names

Committed oracle outputs are observations produced by the pinned Pillow
binary described in `pillow-oracle.lock.yaml`; they are not linked into the
library. Copied AVIF input files have per-file source, revision, hash, and
license records in `tests/fixtures/input/images/avif/README.md`.

This software is based in part on the work of the Independent JPEG Group.

Format specifications, interoperability targets, and project names belong to
their respective owners. No endorsement is implied.
