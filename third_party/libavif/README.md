# libavif provenance

The fixed source is libavif tag `v1.4.1`, commit
`6543b22b5bc706c53f038a16fe515f921556d9b3`.

Rust AVIF container and sample-table logic under `src/codecs/avif/` is also a
safe-Rust, modified source-derived port of libavif 1.4.1 behavior, principally
the container and sample-table rules represented by `src/read.c` and
`src/stream.c`. Several AVIF fixture inputs are unmodified copies from
libavif's test data; their individual provenance is documented in
`tests/fixtures/input/images/avif/README.md`.

No libavif implementation object code or public header is stored in this
repository. The pinned libavif identity is an oracle/provenance reference;
the crate does not build or link it.

- Upstream: <https://github.com/AOMediaCodec/libavif>
- License: complete upstream license bundle retained in `LICENSE`
- Repository-wide mapping: `NOTICE.md` and `third_party/README.md`
