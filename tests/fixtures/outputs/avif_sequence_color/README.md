# High-depth color conversion oracle

This bundle observes all five frames of the existing complete `10bit.avif`.
Despite its filename, the source declares 12-bit limited-range I422 color,
CICP 2/2/2, with a 12-bit full-range monochrome auxiliary alpha track. Each
frame is 64×64. The public high-depth and animation rows remain planned;
native observations do not establish Rust reconstruction or sequence parity.

The collector pins Pillow 12.2.0, libavif 1.4.1, dav1d 1.5.3 and libyuv 1922.
Source revisions, tree/file hashes, retained licenses, binary identities,
compiler arguments, CPU dispatch, sample ranges and per-frame timing are
recorded in [high_bitdepth/index.json](high_bitdepth/index.json).

It checks three boundaries without executing Rust:

1. Decode the complete file with Pillow's exported libavif API, compiled
   against its pinned header. Repeat each frame observation twice and require
   identical visible planes, RGBA bytes and timing. Require every RGBA byte
   to match Pillow's sequential frame decode.
2. Decode both complete AV1 tracks with separately built, unmodified scalar
   dav1d. Require identical color planes (81,920 bytes) and alpha planes
   (40,960 bytes) across all five frames.
3. Compile unmodified libyuv scalar primitives. Downshift color and alpha
   before row-local I422 interpolation, preserve the upstream endpoint rule,
   and apply the default I601 constants. Require identical RGBA (81,920 bytes).

The scalar build disables NEON/X86 dispatch and rejects the unlimited-data
or unlimited-BT.601 build options. The latter would change the blue chroma
coefficient from 128 to 129. CICP matrix 2 resolves to this limited-range
BT.601 conversion in the pinned native implementation; this is evidence for
the exact observed declaration, not for arbitrary unspecified CICP fields.

`frame_N.yuva` stores unpadded little-endian u16 planes in Y64×64, U32×64,
V32×64, A64×64 order. `frame_N.rgba` stores straight-alpha RGBA8. Partial-alpha
pixels distinguish straight from premultiplied output. There are no zero-alpha
pixels, so fully transparent RGB retention is not independently witnessed.
The source includes RGB clipping, but does not span every nominal sample or
color declaration. Native timing is retained as provenance only.

The deferred Rust regression compares every pixel of all five native frames
through the production color converter, plus 25 slices of complete real rows.
Defensive cases retain valid nominal sample depths and plane shapes while
rejecting unwitnessed declarations; separate cases reject malformed internal
sample buffers. These mutations are internal model checks, not Pillow
malformed-file observations. Rust execution and managed coverage are deferred.

The native harness is development-only and excluded from the Rust package.
Regenerate into a fresh ignored directory with clean pinned source checkouts:

```sh
CC=/usr/bin/clang .oracle-venv/bin/python scripts/generate_avif_sequence_color_refs.py \
  --dav1d-source /path/to/dav1d \
  --libavif-source /path/to/libavif \
  --libyuv-source /path/to/libyuv \
  --meson /path/to/meson --ninja /path/to/ninja \
  --output target/oracle-staging/avif-sequence-color/fresh
```

The source revisions are dav1d `b546257f770768b2c88258c533da38b91a06f737`,
libavif `6543b22b5bc706c53f038a16fe515f921556d9b3`, and libyuv
`6067afde563c3946eebd94f146b3824ab7a97a9c`. The collector rejects changed input,
dirty or incorrect sources, unexpected versions, unequal native observations,
existing output directories and output paths outside ignored staging.
