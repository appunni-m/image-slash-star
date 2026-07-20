# Third-party notices

Original work in this repository is licensed under either Apache-2.0 or MIT,
at your option. Portions are derived from or are behavioral ports of upstream
open source projects and remain subject to their original licenses.

| Repository paths | Upstream and fixed version | License and retained text |
| --- | --- | --- |
| `src/codecs/webp/native/` | image-webp 0.2.4 | MIT OR Apache-2.0; see `third_party/image-webp/` |
| `src/codecs/jpeg/` | libjpeg-turbo 3.1.4.1 and IJG libjpeg | IJG and BSD-style terms; see `third_party/libjpeg-turbo/` |
| `src/codecs/webp/` | libwebp 1.6.0 | BSD-3-Clause; see `third_party/libwebp/COPYING` |
| `src/codecs/compression/zlib_ng.rs` and related DEFLATE code | zlib-ng 2.3.3 | Zlib; see `third_party/zlib-ng/LICENSE.md` |
| Pillow-compatible codec and color behavior identified in source comments | Pillow 12.2.0 / libImaging | MIT-CMU; see `third_party/pillow/LICENSE` |
| GIF RGBA FASTOCTREE quantization in `src/codecs/gif/encode.rs` | Pillow 12.2.0 `QuantOctree.c`, Oliver Tonnhofer / Omniscale | MIT; see `third_party/pillow/QUANT-OCTREE-LICENSE` |
| GIF palette bucket ordering in `src/codecs/gif/encode.rs` | Apple Libc / FreeBSD `qsort.c`, Regents of the University of California | BSD-3-Clause; see `third_party/apple-libc/LICENSE` |

The committed oracle fixtures are observations produced by the pinned Pillow
binary described in `pillow-oracle.lock.yaml`; they are not linked into the
library. Format specifications, interoperability targets, and project names
belong to their respective owners. No endorsement is implied.

This software is based in part on the work of the Independent JPEG Group.
