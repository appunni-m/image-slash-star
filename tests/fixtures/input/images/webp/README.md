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
