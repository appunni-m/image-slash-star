# libavif reference material

`include/avif/avif.h` is copied without modification from libavif tag
`v1.4.1` (`6543b22`) so the optional AVIF bridge is compiled against the exact
ABI used by the pinned Pillow 12.2.0 oracle.

- Upstream: <https://github.com/AOMediaCodec/libavif>
- Header SHA-256:
  `2fcde09bb0124f4c1d1fbc5dfbf06ade08a66d8c58854fd3fe3411a6483bd26e`
- License: BSD-2-Clause; retained in `LICENSE`.

No libavif implementation object code is stored in this repository. Enabling
the `avif` feature links the separately installed exact native library.
