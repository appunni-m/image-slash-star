# Pinned AVIF fixture encoder

The AV1 super-resolution fixture generators need encoder controls that stock
libavif 1.4.1 does not expose as codec-specific options. The small patch in
[`libavif-av1-fixture-controls.patch`](libavif-av1-fixture-controls.patch)
passes three super-resolution values to AOM before encoder initialization and
maps partition and transform-search controls to AOM encoder controls. It is
used only to build fixture-generation tools; the Rust library and runtime do
not depend on this patch.

Build the pinned libaom and libavif versions with CMake and Ninja:

```sh
mkdir -p target/oracle-staging
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom
git -C target/oracle-staging/libaom checkout ad44980d7f3c7a2605c25d51ea96946949000841
git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-superres
git -C target/oracle-staging/libavif-superres checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-superres apply --check "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
git -C target/oracle-staging/libavif-superres apply "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"

cmake -S target/oracle-staging/libaom -B target/oracle-staging/libaom-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/aom-prefix"
cmake --build target/oracle-staging/libaom-build -j 4
cmake --install target/oracle-staging/libaom-build

cmake -S target/oracle-staging/libavif-superres -B target/oracle-staging/libavif-superres-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=ON \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/aom-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/aom-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/aom-prefix/include"
cmake --build target/oracle-staging/libavif-superres-build --target avifenc -j 4
```

Use the resulting `avifenc` with the generator. For example:

```sh
.oracle-venv/bin/python scripts/generate_avif_8bit_superres_restoration.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc
```

Generate the equal-width no-op edge case with the same pinned encoder:

```sh
.oracle-venv/bin/python scripts/generate_avif_equal_width_superres.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc
```

That still fixture sets the key-frame super-resolution denominator to 9 and
checks the emitted AV1 header, exact pinned Pillow pixels, and repeat-encode
stability. Its 16-pixel width exercises AV1's minimum coded-width behavior,
where the frame signals super-resolution but coded and upscaled widths match.

Generate the actual-upscaled 4:2:2 odd-edge case with the same pinned encoder:

```sh
.oracle-venv/bin/python scripts/generate_avif_equal_width_superres.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc \
  --chroma 422-actual
```

This full-range 33x17 I422 still signals denominator 9, with coded width 29
and upscaled width 33. Restoration is disabled so decoding exercises the
generic I422 super-resolution path. The generator checks the I422 sequence
header and CICP values, repeat-encode stability, and exact Pillow RGB pixels.

The clean pinned libavif checkout used by `scripts/generate_avif_loop_refs.py`
must remain separate: that oracle checks that its source tree is unmodified.

## Skipped CDEF maps in tiled color and alpha items

Generate the two-tile public still fixture with the same pinned encoder and
the clean libaom source/build directories above:

```sh
target/oracle-staging/pillow122/bin/python scripts/generate_avif_color_tile_split_fixture.py \
  --profile cdef-skipped-square64 \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc \
  --aom-source target/oracle-staging/libaom \
  --aom-build target/oracle-staging/libaom-build
```

This writes `multitile_skipped_cdef_color_alpha.avif`. The source is a 128x64
RGBA PNG with every component equal to 128. The encoder uses 8-bit 4:2:0,
color and alpha quality 99, speed 8, one worker, two tile columns, CDEF enabled,
and minimum/maximum partition size 64. Both AV1 items retain lossy quantizer 4,
enabled CDEF, and disabled screen-content tools.

Stock libaom deliberately writes `skip_txfm = 0` for intra blocks, including
blocks with zero residual coefficients. The fixture-only native writer
[`encode_skipped_square64.c`](encode_skipped_square64.c) instead constructs
one legal skipped 64x64 DC-predicted superblock through the unmodified pinned
libaom default CDFs and entropy primitives. Each color tile is `98 80`; each
monochrome alpha tile is `99`. Skipping the whole superblock omits its CDEF
index and delta-Q sentence. The generator replaces the bounded tile payloads
and updates item extents and the terminal `mdat` size while retaining the
independently encoded frame headers. It never calls Rust or modifies native
oracle sources.

The generator encodes and constructs the file twice, requires byte-for-byte
repeatability, and decodes each result through Pillow 12.2.0/libavif 1.4.1.
The 483-byte AVIF has SHA-256
`36c83e5e5104e74a8b3474b3ee313645a7bf86381d1578087e5cef3453d289eb`.
Pillow returns RGBA 128x64 with all 32,768 bytes equal to 128, pixel SHA-256
`67d47633eeb4ab9211bfaddc84e6d5c09a958588867dcdc4b2169ad74b73fa0e`.
The ordinary valid input leaves both per-tile CDEF active maps false and both
per-tile CDEF region indices absent. Public matrix execution measures the
Rust outcome and coverage separately.

The generator's original no-argument behavior still creates
`multitile_color_split_groups.avif`; `--profile color-split` selects it
explicitly. `--output PATH` redirects either fixture for an independent
reproduction check.
