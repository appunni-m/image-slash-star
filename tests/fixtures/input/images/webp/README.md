# WebP fixture provenance

`lossy_mode_filter_delta.webp` is a four-segment VP8 keyframe whose intra4x4
macroblocks use a `+5` loop-filter mode delta. It is generated from the
repository's 128x128 RGB pattern with libwebp 1.6.0. The fixture-only encoder
change sets `i4x4_lf_delta` in `src/enc/webp_enc.c`; the generator applies this
change to a temporary source copy and leaves the supplied upstream tree alone.

Regenerate it with an unpacked upstream libwebp 1.6.0 source tree, CMake, a C
compiler, and Pillow 12.2.0 in the repository oracle environment:

```sh
.oracle-venv/bin/python scripts/generate_webp_lf_mode_delta_fixture.py \
    /path/to/libwebp-1.6.0 \
    --output tests/fixtures/input/images/webp/lossy_mode_filter_delta.webp
```

The generator checks the source image, encoded asset, and Pillow-decoded pixels
against pinned SHA-256 values. Pillow 12.2.0 opens, verifies, and loads the
fixture as 128x128 RGB. The asset SHA-256 is
`86b97b8489b9ec2318858bef5d91be138e1222398dd2b8ffbde6dded0e598b55`; the
decoded-pixel SHA-256 is
`ce195285d0ae374bc2ae5538da590658fbe26846b27cf2b0d2300979319f4c99`.

`lossy_mode_filter_delta_no_update.webp` uses the same source pixels and
libwebp 1.6.0 encoder configuration, with a fixture-only change that keeps
loop-filter deltas enabled while clearing the delta-update bit. This exercises
the VP8 keyframe path that preserves its default zero deltas. Regenerate it
with an unpacked upstream libwebp 1.6.0 source tree:

```sh
.oracle-venv/bin/python scripts/generate_webp_lf_no_update_fixture.py \
    /path/to/libwebp-1.6.0
```

The generator pins the complete WebP bytes to
`39a3eeb4102f2b10bdb25f9cca0687345afa5ce3c27ff38a243b36b82ee78bd0` and the
Pillow 12.2.0 decoded pixels to
`6b2c202c55d0bf70d9f32b74c1b8705232ae41e0ed24203ea2b1b138dfd914fe`.
Pillow opens, verifies, and loads the fixture as 128x128 RGB.

`vp8x_reserved_flag_0x01.webp`, `vp8x_reserved_flag_0x40.webp`, and
`vp8x_reserved_flag_0x80.webp` are complete RIFF WebP files derived
independently from the pinned `icc.webp` source. The generator validates the
source hash and walks RIFF chunk extents to find the sole 10-byte `VP8X`
header, then sets one reserved flag bit while preserving ICC bit `0x20`. The
source SHA-256 is
`1b1872b2280483eefe2084ac09335d2ab40649204996df6a58bc14d3b3b15219`; the
mutant SHA-256 values are `0x01`:
`5f61cb5418ee6aac04b3c813ce4679bb286fc6c232fea0ec9ddd8a8f6532971a`, `0x40`:
`a7e995a2d38cb074be90bea6b4ea85c3b9f658239bcd11cfbfb836c3727ddde3`, and
`0x80`: `b2dc2020ec817a1e3f0bf477560c76b5b7cbb71cc577a0b1c5c44f3abd5ee8b9`.
Pinned Pillow 12.2.0 detects each file as WebP, then rejects open, verify,
load, and sequence decode with `OSError: could not create decoder object`.

`animated_top_level_image_chunk.webp` and
`animated_duplicate_top_level_image_chunks.webp` are complete animated RIFF
files derived from `animated_rgb_palette_background.webp`. The generator checks
the pinned source and output hashes, copies the full-canvas VP8L payload from
the first ANMF frame, and inserts one or two top-level VP8L chunks before
ANIM. Regenerate and check them with
`scripts/generate_webp_animated_top_level_image_fixtures.py`; Pillow 12.2.0
rejects both during image open with `OSError: could not create decoder object`.

`animated_alpha_origin_partial.webp` derives from the pinned
`animated_alpha.webp` source by changing only the two ANMF origin fields. The
frames start at raw offsets `(0, 0)` and `(0, 2)` while retaining their 16x16
dimensions on a 64x64 canvas. Regenerate it with
`scripts/generate_webp_animated_origin_partial_fixture.py`; the generator pins
the complete-file hash and verifies both composited RGBA frames with Pillow
12.2.0. The fixture reaches the partial-frame path after both origin checks
pass, then the width check fails.
