# HDR color conversion oracle

This bundle observes the existing complete `hdr.avif` input. It does not add a
matrix row or claim that Rust reconstructs that image. The HDR decode row and
`AVF-COLOR-001` remain planned until the deferred public parity campaign passes.

The input is 200×200, 10-bit I444, full range, CICP 9/16/9 (BT.2020 primaries,
PQ transfer, non-constant-luminance matrix), with no alpha. The collector pins
Pillow 12.2.0, libavif 1.4.1, dav1d 1.5.3 and libyuv 1922 to the source revisions
recorded in [hdr/index.json](hdr/index.json). Licenses are retained under
`third_party`; the native observer is development-only provenance.

The collector establishes three independent boundaries:

1. Decode the unchanged AVIF through Pillow's exported libavif API, using the
   exact pinned C header for ABI layout. Retain visible planes and native RGB.
2. Decode the primary AV1 item with an independently built, unmodified scalar
   dav1d. Require byte-identical visible planes (240,000 bytes).
3. Compile the pinned, unmodified libyuv `row_common.cc`. Its scalar downshift
   and RGB24 conversion must match both libavif and Pillow (120,000 bytes).

The libavif observation repeats twice and must be deterministic. The index
records source tree and file hashes, native binary identities, compiler,
strides, byte order, CPU dispatch, and complete artifact hashes. Expected
pixels come only from native sources; the Rust converter is never executed by
this script. `display.yuv` is planar Y/U/V in little-endian u16 without padding;
`display.rgb` is interleaved RGB8.

The native path downshifts 10-bit samples before applying libyuv's V2020
fixed-point matrix. It preserves PQ-encoded values: there is no additional
tone mapping or conversion to sRGB. `observer.cc` is a copy of the exact
collector harness. Its library calls run only inside the development oracle
process; they are not linked into the Rust crate or included in its package.

The Rust regression feeds these native planes through the production color
boundary and compares every RGB byte. It also compares 48 slices of the same
real pixels, covering vector batches and scalar tails without claiming new
encoded input classes. Separate internal defensive cases reject alpha,
subsampling, other depths, limited range, constant-luminance matrix, altered
primaries/transfer, invalid sample values and truncated planes. Those internal
mutations are not Pillow malformed-file observations. Rust execution and
managed coverage remain deferred; compilation alone does not prove parity.

The observed nominal sample ranges are Y 455–686, U 404–513 and V 469–630.
This input does not independently witness nominal extrema or RGB clipping;
those remain separate coverage gaps despite the bounded arithmetic proof.

Regenerate into a fresh ignored staging directory using clean pinned source
checkouts (substitute the local source and tool paths):

```sh
CC=/usr/bin/clang .oracle-venv/bin/python scripts/generate_avif_hdr_color_refs.py \
  --dav1d-source /path/to/dav1d \
  --libavif-source /path/to/libavif \
  --libyuv-source /path/to/libyuv \
  --meson /path/to/meson --ninja /path/to/ninja \
  --output target/oracle-staging/avif-hdr-color/fresh
```

The source revisions are dav1d `b546257f770768b2c88258c533da38b91a06f737`,
libavif `6543b22b5bc706c53f038a16fe515f921556d9b3`, and libyuv
`6067afde563c3946eebd94f146b3824ab7a97a9c`. The collector refuses dirty or
incorrect source checkouts, unexpected oracle versions, changed full-file
inputs, unequal native observations, canonical output paths, and existing
staging directories.
