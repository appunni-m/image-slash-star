# libavif provenance

The fixed source is libavif tag `v1.4.1`, commit
`6543b22b5bc706c53f038a16fe515f921556d9b3`.

`include/avif/avif.h` is copied without modification so the optional AVIF
bridge is compiled against the exact ABI used by the pinned Pillow 12.2.0
oracle. Its SHA-256 is
`2fcde09bb0124f4c1d1fbc5dfbf06ade08a66d8c58854fd3fe3411a6483bd26e`.

Rust AVIF container and sample-table logic under `src/codecs/avif/` is also a
modified port of libavif 1.4.1 code, principally `src/read.c` and
`src/stream.c`. Several AVIF fixture inputs are unmodified copies from
libavif's test data; their individual provenance is documented in
`tests/fixtures/input/images/avif/README.md`.

No libavif implementation object code is stored in this repository. Enabling
the native AVIF path links a separately installed libavif 1.4.1 library.

- Upstream: <https://github.com/AOMediaCodec/libavif>
- License: complete upstream license bundle retained in `LICENSE`
- Repository-wide mapping: `NOTICE.md` and `third_party/README.md`
