# Third-party provenance

This directory retains the license, attribution, and patent texts needed for
code, data, and pinned oracle material used by `image-slash-star`.
`NOTICE.md` maps those components to repository paths and explains the
distribution cases.

The project uses three distinct kinds of third-party material:

1. Rust translations or source-derived implementations compiled into the
   crate.
2. Separately licensed Cargo dependencies.
3. Pinned AVIF/Pillow oracle and source-reference material. These files are
   not compiled or linked into the crate.

Behavior observed through the pinned Pillow oracle is not treated as copied
source. A Pillow license is retained for source-derived portions explicitly
identified in comments and for copied Pillow fixtures.

## Audited source inventory

| Component | Fixed source | Repository role | Retained terms |
| --- | --- | --- | --- |
| Apple Libc `qsort.c` | commit `71bbe350ab79eef58113991d817ccc6165061a64`; source SHA-256 `0f4692a3a9177e1dcd35f5a73552359e644ff13489896fe2309840048b7f575e` | GIF palette-bucket ordering port | `apple-libc/LICENSE` (BSD-3-Clause) |
| bytemuck | crate 1.25.1; crates.io package SHA-256 `d6aedf8ae72766347502cf3cb4f41cf5e9cc37d28bee90f1fdaaae15f9cf9424` | Sole Cargo dependency | `bytemuck/LICENSE-*` (Zlib OR Apache-2.0 OR MIT) |
| dav1d | 1.5.3, commit `b546257f770768b2c88258c533da38b91a06f737` | Portable AV1 decoder source reference and fixture oracle identity | `dav1d/COPYING` (BSD-2-Clause) |
| image-webp | crate 0.2.4; crates.io package SHA-256 `525e9ff3e1a4be2fbea1fdf0e98686a6d98b4d8f937e1bf7402245af1909e8c3` | Pure-Rust WebP decoder base | `image-webp/LICENSE-*` (MIT OR Apache-2.0) |
| libaom | 3.13.2, commit `ad44980d7f3c7a2605c25d51ea96946949000841` | Portable AV1 encoder source reference and fixture oracle identity | `libaom/LICENSE` (BSD-2-Clause) and `libaom/PATENTS`; `PATENTS` is also retained at the crate root |
| libavif | 1.4.1, commit `6543b22b5bc706c53f038a16fe515f921556d9b3` | AVIF container/parser source reference and copied fixtures | `libavif/LICENSE` (complete upstream license bundle) |
| libjpeg-turbo | 3.1.4.1, commit `9217719d3a58633923b096af4c1d50d304768a64` | JPEG encoder/decoder ports | `libjpeg-turbo/LICENSE.md` and `libjpeg-turbo/README.ijg` (IJG and BSD-style terms) |
| libvpx | 1.15.2, commit `d168454ecd099805c675d4a98c66f4891373302a`; `vp8/common/quant_common.c` SHA-256 `fc8471698f061e5dcb9e7caef0df3bceec9921ec3aada2e1dc4c3348a5d7e8f2` | VP8 quantization-table source named by `src/codecs/webp/encode/vp8/quant.rs` | `libvpx/LICENSE` (BSD-3-Clause) and `libvpx/PATENTS` (WebM patent grant) |
| libwebp | 1.6.0, commit `4fa21912338357f89e4fd51cf2368325b59e9bd9` | WebP encoder/decoder ports and optional native SharpYUV component | `libwebp/COPYING` (BSD-3-Clause) and `libwebp/PATENTS` (WebM patent grant) |
| libyuv | commit `6067afde563c3946eebd94f146b3824ab7a97a9c` (libavif revision 1922) | AVIF color-conversion reference for the safe-Rust port | `libyuv/LICENSE` (BSD-3-Clause) |
| Pillow | 12.2.0, commit `3c41c095064200a02672d89cc5ff629eaf4b0d4f` | Explicit source-derived behavior and copied AVIF fixtures | `pillow/LICENSE` (MIT-CMU) |
| Pillow `QuantOctree.c` | Pillow 12.2.0 at the commit above | GIF FASTOCTREE port | `pillow/QUANT-OCTREE-LICENSE` (MIT; Oliver Tonnhofer / Omniscale) |
| zlib-ng | 2.3.3, commit `12731092979c6d07f42da27da673a9f6c7b13586` | Altered Rust DEFLATE compressor ports | `zlib-ng/LICENSE.md` (Zlib) |

Upstream repositories:

- Apple Libc: <https://github.com/apple-oss-distributions/Libc>
- bytemuck: <https://github.com/Lokathor/bytemuck>
- dav1d: <https://code.videolan.org/videolan/dav1d>
- image-webp: <https://github.com/image-rs/image-webp>
- libaom: <https://aomedia.googlesource.com/aom>
- libavif: <https://github.com/AOMediaCodec/libavif>
- libjpeg-turbo: <https://github.com/libjpeg-turbo/libjpeg-turbo>
- libvpx: <https://chromium.googlesource.com/webm/libvpx>
- libwebp: <https://chromium.googlesource.com/webm/libwebp>
- libyuv: <https://chromium.googlesource.com/libyuv/libyuv>
- Pillow: <https://github.com/python-pillow/Pillow>
- zlib-ng: <https://github.com/zlib-ng/zlib-ng>

## Retention details

License files are byte-exact copies from the fixed source unless described
below:

- `apple-libc/LICENSE` is the license block extracted from the pinned
  `stdlib/FreeBSD/qsort.c`; only C comment leaders were removed.
- `pillow/QUANT-OCTREE-LICENSE` is the license block extracted from
  `src/libImaging/QuantOctree.c`; only C comment leaders were removed.
- `libwebp/PATENTS` and `libvpx/PATENTS` are byte-exact copies from the pinned
  upstream commits. `libwebp/COPYING` and `libvpx/LICENSE` each have one
  redundant terminal empty line removed.
- `libavif/LICENSE` is intentionally the complete upstream file, including
  notices for files that are not copied into this repository. It is retained
  because the AVIF fixtures and source-derived parser references use that
  provenance.
- `libaom/PATENTS` and the root `PATENTS` file are byte-identical. The root
  copy follows the source-distribution placement required by that patent
  license.
- `image-webp/README.md` is the upstream README retained with the copied source
  base; it describes image-webp, not this crate.

The old copied public libavif header and C bridge were removed from the
runtime and package surface. AVIF code in `src/codecs/avif/` is safe Rust;
source-derived translations are modified Rust, not linked upstream object
code.

Run `python3 scripts/verify_third_party_licenses.py` to verify retained text
hashes, the root patent-license copy, and required notice references.
