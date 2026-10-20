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

Generate an actual-upscaled odd-width 4:2:0 case with the same pinned encoder:

```sh
.oracle-venv/bin/python scripts/generate_avif_equal_width_superres.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc \
  --chroma 420-actual
```

This 17x17 lossy I420 still signals denominator 9, with coded luma width 16
and upscaled width 17; ceil-rounded chroma width grows from 8 to 9. The
generator checks the AV1 header, two byte-identical encodes, and exact pinned
Pillow verify/load results and RGB pixels before writing
`superres_actual_upscaled_i420_17x17.avif`.
The source Y4M, AVIF, and decoded-pixel SHA-256 values are respectively
`2b6a7a2db94663d55e4be325b666b9dbae21bf8ef368863772ebacdef37dcb86`,
`5824f2bd3c3fa431a0c0c57509f34e11ec148e36bac9531bff083394eab59d2f`, and
`0140c8dab75efad711db2af0b7f1f837e76e2a8d578ac43052fea15b7693d926`.

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

## Mixed lossless segment in a clipped B32 block

The public still
`mixed_lossless_clipped_b32_i420_60x60.avif` uses a fixture-only ROI map to
assign qindex zero to the bottom-right B32x32 block. The other segments retain
the lossy frame qindex. At 60x60, the B32 leaf at MI origin (8, 8) exposes
28x28 pixels. The encoder trace patch requires libaom to report that exact
B32x32 as an unsplit leaf in segment 1 at qindex zero. The generator also
checks the serialized frame header, byte-for-byte re-encoding, and exact
Pillow pixels before writing either fixture.

Build separate work copies of the pinned sources so the clean oracle
checkouts remain unchanged:

```sh
mkdir -p target/oracle-staging
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-mixed-lossless-clipped-b32
git -C target/oracle-staging/libaom-mixed-lossless-clipped-b32 checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-mixed-lossless-clipped-b32 apply "$PWD/scripts/avif_fixture_oracle/libaom-mixed-lossless-clipped-b32.patch"
cmake -S target/oracle-staging/libaom-mixed-lossless-clipped-b32 -B target/oracle-staging/libaom-mixed-lossless-clipped-b32-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-mixed-lossless-clipped-b32-prefix"
cmake --build target/oracle-staging/libaom-mixed-lossless-clipped-b32-build -j 4
cmake --install target/oracle-staging/libaom-mixed-lossless-clipped-b32-build

git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-mixed-lossless-clipped-b32
git -C target/oracle-staging/libavif-mixed-lossless-clipped-b32 checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-mixed-lossless-clipped-b32 apply "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
git -C target/oracle-staging/libavif-mixed-lossless-clipped-b32 apply "$PWD/scripts/avif_fixture_oracle/libavif-mixed-lossless-clipped-b32.patch"
cmake -S target/oracle-staging/libavif-mixed-lossless-clipped-b32 -B target/oracle-staging/libavif-mixed-lossless-clipped-b32-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=OFF \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-mixed-lossless-clipped-b32-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-mixed-lossless-clipped-b32-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-mixed-lossless-clipped-b32-prefix/include"
cmake --build target/oracle-staging/libavif-mixed-lossless-clipped-b32-build --target avif_static -j 4
cc -O2 \
  -I target/oracle-staging/libavif-mixed-lossless-clipped-b32/include \
  -I target/oracle-staging/libavif-mixed-lossless-clipped-b32-build/include \
  -I target/oracle-staging/libaom-mixed-lossless-clipped-b32-prefix/include \
  scripts/avif_fixture_oracle/encode_mixed_lossless_clipped_b32.c \
  target/oracle-staging/libavif-mixed-lossless-clipped-b32-build/libavif.a \
  target/oracle-staging/libaom-mixed-lossless-clipped-b32-prefix/lib/libaom.a \
  -lz -lm -lpthread \
  -o target/oracle-staging/encode-avif-mixed-lossless-clipped-b32
```

Run the pinned generator and Pillow reference check:

```sh
.oracle-venv/bin/python scripts/generate_avif_mixed_lossless_clipped_b32.py \
  --encoder target/oracle-staging/encode-avif-mixed-lossless-clipped-b32
```

The patches are fixture-generation controls only. They do not modify the
runtime codec or the clean pinned source checkouts.

## Mixed-lossless inter B32 sequences

`animated_mixed_lossless_inter_b32_i420_64x64.avif` keeps a 64x64 full-range
I420 canvas and places a qindex-zero ROI segment over the bottom-right B32
region. The top-left patch changes between frames so frame 1 is inter-coded;
the bottom-right leaf remains unchanged and the encoder trace proves it stays
an unsplit B32x32 at MI `(8,8)`, segment 1, qindex 0. Segment 0 and 2 retain
nonzero qindices, and neither frame is all-lossless. The generator validates
the serialized key/inter headers, normalized repeat-encode output, trace, and
both exact Pillow RGB frames.

The generator also accepts `--canvas-size 60` to create
`animated_mixed_lossless_inter_b32_i420_60x60.avif`. Its bottom-right
inter-coded B32x32 remains at MI `(8,8)` and pixel origin `(32,32)`, with a
28x28 visible extent. Segment 1 remains qindex zero while surrounding
segments stay lossy; both frames remain non-all-lossless. The bitstream check
validates the clipped dimensions and mixed segment map before exact Pillow
frames are written.

Use isolated copies of the pinned sources, applying the existing clipped-B32
fixture patch before the sequence-specific patches:

```sh
mkdir -p target/oracle-staging
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-mixed-lossless-inter-b32-source
git -C target/oracle-staging/libaom-mixed-lossless-inter-b32-source checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-mixed-lossless-inter-b32-source apply "$PWD/scripts/avif_fixture_oracle/libaom-mixed-lossless-clipped-b32.patch"
git -C target/oracle-staging/libaom-mixed-lossless-inter-b32-source apply "$PWD/scripts/avif_fixture_oracle/libaom-mixed-lossless-inter-b32.patch"
cmake -S target/oracle-staging/libaom-mixed-lossless-inter-b32-source -B target/oracle-staging/libaom-mixed-lossless-inter-b32-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-mixed-lossless-inter-b32-prefix"
cmake --build target/oracle-staging/libaom-mixed-lossless-inter-b32-build -j 4
cmake --install target/oracle-staging/libaom-mixed-lossless-inter-b32-build

git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-mixed-lossless-inter-b32-source
git -C target/oracle-staging/libavif-mixed-lossless-inter-b32-source checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-mixed-lossless-inter-b32-source apply "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
git -C target/oracle-staging/libavif-mixed-lossless-inter-b32-source apply "$PWD/scripts/avif_fixture_oracle/libavif-mixed-lossless-clipped-b32.patch"
git -C target/oracle-staging/libavif-mixed-lossless-inter-b32-source apply "$PWD/scripts/avif_fixture_oracle/libavif-mixed-lossless-inter-b32.patch"
cmake -S target/oracle-staging/libavif-mixed-lossless-inter-b32-source -B target/oracle-staging/libavif-mixed-lossless-inter-b32-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=OFF \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-mixed-lossless-inter-b32-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-mixed-lossless-inter-b32-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-mixed-lossless-inter-b32-prefix/include"
cmake --build target/oracle-staging/libavif-mixed-lossless-inter-b32-build --target avif_static -j 4
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-mixed-lossless-inter-b32-source/include \
  -I target/oracle-staging/libavif-mixed-lossless-inter-b32-build/include \
  -I target/oracle-staging/libaom-mixed-lossless-inter-b32-prefix/include \
  scripts/avif_fixture_oracle/encode_mixed_lossless_inter_b32.c \
  target/oracle-staging/libavif-mixed-lossless-inter-b32-build/libavif.a \
  target/oracle-staging/libaom-mixed-lossless-inter-b32-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-mixed-lossless-inter-b32
.oracle-venv/bin/python scripts/generate_avif_mixed_lossless_inter_b32.py \
  --encoder target/oracle-staging/encode-avif-mixed-lossless-inter-b32

.oracle-venv/bin/python scripts/generate_avif_mixed_lossless_inter_b32.py \
  --encoder target/oracle-staging/encode-avif-mixed-lossless-inter-b32 \
  --canvas-size 60
```

The generator writes a receipt under ignored `target/oracle-staging/` with
patch, encoder, bitstream, frame-header, trace, and Pillow identities. Only
the three version-one BMFF timestamp pairs are normalized; all other bytes
must be deterministic. Keep these patched builds apart from the clean pinned
libavif checkout used by the loop observer.
