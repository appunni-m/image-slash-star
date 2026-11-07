# Pinned AVIF fixture encoder

The AV1 super-resolution fixture generators need encoder controls that stock
libavif 1.4.1 does not expose as codec-specific options. The small patch in
[`libavif-av1-fixture-controls.patch`](libavif-av1-fixture-controls.patch)
passes three super-resolution values to AOM before encoder initialization and
maps partition and transform-search controls to AOM encoder controls. It is
used only to build fixture-generation tools; the Rust library and runtime do
not depend on this patch.

## B32x64 inter-transform fixtures

The deep and topology profiles use one forced AV1 partition so each inter frame
keeps two vertical B32x64 blocks. The deep profile reaches the deep transform
tree; the topology profile reaches a mixed one-child split. The fixture-only patch
[`libaom-lossy-inter-i420-deep-b32x64.patch`](libaom-lossy-inter-i420-deep-b32x64.patch)
is applied to a separate copy of pinned libaom. It does not change the clean
oracle checkout or the Rust decoder.

Build patched libaom from the pinned source. The libavif archive from the
fixture-controls build below supplies the encoder API; link it against the
patched libaom archive for this fixture:

```sh
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-deep-b32x64
git -C target/oracle-staging/libaom-deep-b32x64 checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-deep-b32x64 apply "$PWD/scripts/avif_fixture_oracle/libaom-lossy-inter-i420-deep-b32x64.patch"
cmake -S target/oracle-staging/libaom-deep-b32x64 -B target/oracle-staging/libaom-deep-b32x64-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0
cmake --build target/oracle-staging/libaom-deep-b32x64-build -j 4

clang -std=c11 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-superres/include \
  -I target/oracle-staging/libavif-superres-build/include \
  scripts/avif_fixture_oracle/encode_lossy_inter_i420_b32x64.c \
  target/oracle-staging/libavif-superres-build/libavif.a \
  target/oracle-staging/libaom-deep-b32x64-build/libaom.a -lm \
  -o target/oracle-staging/encode_lossy_inter_i420_b32x64

target/oracle-staging/pillow122/bin/python3 \
  scripts/generate_avif_lossy_inter_i420_b32x64.py \
  --encoder target/oracle-staging/encode_lossy_inter_i420_b32x64
target/oracle-staging/pillow122/bin/python3 \
  scripts/generate_avif_lossy_inter_i420_b32x64.py --check
target/oracle-staging/pillow122/bin/python3 \
  scripts/generate_avif_lossy_inter_i420_b32x64.py \
  --profile topology --encoder target/oracle-staging/encode_lossy_inter_i420_b32x64
target/oracle-staging/pillow122/bin/python3 \
  scripts/generate_avif_lossy_inter_i420_b32x64.py --profile topology --check
```

The generator normalizes sequence timestamps, requires repeat-encode stability,
and verifies the exact two-frame RGB output with Pillow 12.2.0.

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

The 8-bit I422 companion is generated and checked with:

```sh
.oracle-venv/bin/python scripts/generate_avif_8bit_i422_superres_restoration.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc
.oracle-venv/bin/python scripts/generate_avif_8bit_i422_superres_restoration.py --check
```

It uses the same pinned encoder profile, with 4:2:2 chroma and the exact
largest-transform/SuperRes SGR bitstream checked before the Pillow RGB pins.

The 10-bit I444 companion uses this same pinned encoder and compatibility
patch. It disables transform-size search, retains the largest-transform mode,
and requires SGR restoration on all three full-resolution planes. Regenerate
it with:

```sh
.oracle-venv/bin/python scripts/generate_avif_hi10_i444_superres_restoration.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc
```

The generator checks that the inter frame is coded at 128x56, restored after
super-resolution to 160x56, and decoded by the pinned Pillow runtime to the
recorded RGB frames. Its `--check` mode verifies the retained AVIF without
rebuilding the fixture encoder.

## Active 8-bit I422 inter-frame loop filter

The 16x16 I422 loop-filter parity case needs an inter frame with nonzero
deblocking levels. Libaom's rate-distortion search selects zero for this small
profile, so the fixture-only patch
[`libaom-force-i422-inter-loopfilter.patch`](libaom-force-i422-inter-loopfilter.patch)
sets the levels to one for only 8-bit 16x16 I422 inter frames. The encoder then
applies the signaled filter normally. The generator also disables reference-frame
motion vectors and compound modes so the inter frame meets the decoder's bounded
profile. These controls change only the fixture producer; the checked AVIF is
decoded by the pinned Pillow oracle and the Rust decoder.

Build the pinned sources in separate staging copies, then generate and check
the fixture:

```sh
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-i422-loopfilter
git -C target/oracle-staging/libaom-i422-loopfilter checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-i422-loopfilter apply "$PWD/scripts/avif_fixture_oracle/libaom-force-i422-inter-loopfilter.patch"
cmake -S target/oracle-staging/libaom-i422-loopfilter -B target/oracle-staging/libaom-i422-loopfilter-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/aom-i422-loopfilter-prefix"
cmake --build target/oracle-staging/libaom-i422-loopfilter-build -j 4
cmake --install target/oracle-staging/libaom-i422-loopfilter-build

git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-i422-loopfilter
git -C target/oracle-staging/libavif-i422-loopfilter checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-i422-loopfilter apply "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
cmake -S target/oracle-staging/libavif-i422-loopfilter -B target/oracle-staging/libavif-i422-loopfilter-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=ON \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/aom-i422-loopfilter-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/aom-i422-loopfilter-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/aom-i422-loopfilter-prefix/include"
cmake --build target/oracle-staging/libavif-i422-loopfilter-build --target avifenc -j 4

.oracle-venv/bin/python scripts/generate_avif_inter_i422_loopfilter_16x16.py \
  --avifenc target/oracle-staging/libavif-i422-loopfilter-build/avifenc
.oracle-venv/bin/python scripts/generate_avif_inter_i422_loopfilter_16x16.py --check
```

## Combined 10-bit I420 CDEF and SGR restoration fixture

Libaom's normal rate-distortion search selects either CDEF or SGR restoration
for this tiny profile. The fixture-only patch
[`libaom-force-i420-cdef-restoration.patch`](libaom-force-i420-cdef-restoration.patch)
retains the SGR search for a single 16x16 10-bit I420 key frame and selects its
computed luma filter parameters. Runtime code and the pinned Pillow decoder are
unchanged.

Apply the patch to the pinned libaom checkout from the build steps above, then
build a separate archive and AVIF encoder:

```sh
git -C target/oracle-staging/libaom apply \
  "$PWD/scripts/avif_fixture_oracle/libaom-force-i420-cdef-restoration.patch"
cmake -S target/oracle-staging/libaom -B target/oracle-staging/libaom-force-i420-cdef-restoration-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/aom-force-i420-cdef-restoration-prefix"
cmake --build target/oracle-staging/libaom-force-i420-cdef-restoration-build -j 4
cmake --install target/oracle-staging/libaom-force-i420-cdef-restoration-build
cmake -S target/oracle-staging/libavif-superres -B target/oracle-staging/libavif-force-i420-cdef-restoration-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=ON \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/aom-force-i420-cdef-restoration-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/aom-force-i420-cdef-restoration-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/aom-force-i420-cdef-restoration-prefix/include"
cmake --build target/oracle-staging/libavif-force-i420-cdef-restoration-build --target avifenc -j 4
.oracle-venv/bin/python scripts/generate_avif_intra_filters.py \
  --variant i420_cdef_restoration \
  --avifenc target/oracle-staging/libavif-force-i420-cdef-restoration-build/avifenc
.oracle-venv/bin/python scripts/generate_avif_intra_filters.py \
  --variant i420_cdef_restoration --check
```

## Combined 10-bit I422 loop filtering and Wiener restoration

The 16x16 fixture `coverage_i422_intra_loopfilter_restoration_10bit_16x16.avif`
selects the bounded I422 intra postfilter profile with active Wiener luma
restoration and nonzero Y/U/V deblocking levels. Libaom's normal tiny-frame
search leaves these levels at zero, so the fixture-only companion patch
[`libaom-force-i422-intra-loopfilter.patch`](libaom-force-i422-intra-loopfilter.patch)
sets all four levels to 1 on a 10-bit 16x16 I422 key frame. Apply it after
[`libaom-force-i422-inter-loopfilter.patch`](libaom-force-i422-inter-loopfilter.patch)
to the clean pinned libaom 3.13.2 source tree, then build in a separate
directory.

```sh
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-i422-intra-loopfilter
git -C target/oracle-staging/libaom-i422-intra-loopfilter checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-i422-intra-loopfilter apply --check "$PWD/scripts/avif_fixture_oracle/libaom-force-i422-inter-loopfilter.patch"
git -C target/oracle-staging/libaom-i422-intra-loopfilter apply "$PWD/scripts/avif_fixture_oracle/libaom-force-i422-inter-loopfilter.patch"
git -C target/oracle-staging/libaom-i422-intra-loopfilter apply --check "$PWD/scripts/avif_fixture_oracle/libaom-force-i422-intra-loopfilter.patch"
git -C target/oracle-staging/libaom-i422-intra-loopfilter apply "$PWD/scripts/avif_fixture_oracle/libaom-force-i422-intra-loopfilter.patch"
cmake -S target/oracle-staging/libaom-i422-intra-loopfilter -B target/oracle-staging/libaom-i422-intra-loopfilter-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-i422-intra-loopfilter-prefix"
cmake --build target/oracle-staging/libaom-i422-intra-loopfilter-build -j 4
cmake --install target/oracle-staging/libaom-i422-intra-loopfilter-build
git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-i422-intra-loopfilter
git -C target/oracle-staging/libavif-i422-intra-loopfilter checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-i422-intra-loopfilter apply --check "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
git -C target/oracle-staging/libavif-i422-intra-loopfilter apply "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
cmake -S target/oracle-staging/libavif-i422-intra-loopfilter -B target/oracle-staging/libavif-i422-intra-loopfilter-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=ON \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-i422-intra-loopfilter-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-i422-intra-loopfilter-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-i422-intra-loopfilter-prefix/include"
cmake --build target/oracle-staging/libavif-i422-intra-loopfilter-build --target avifenc -j 4
.oracle-venv/bin/python scripts/generate_avif_intra_filters.py \
  --variant i422_loopfilter_restoration \
  --avifenc target/oracle-staging/libavif-i422-intra-loopfilter-build/avifenc
.oracle-venv/bin/python scripts/generate_avif_intra_filters.py \
  --variant i422_loopfilter_restoration --check
```

The generator checks the exact I422 key-frame syntax, all four active filter
levels, Wiener restoration, repeat-encode identity, and Pillow 12.2.0/libavif
1.4.1 pixel output. The AVIF SHA-256 is
`9366ceb13fc73ec12ff778d9f55a1924025cd166bd75e2cd8322358914b326fe`; its
768-byte RGB reference SHA-256 is
`93f7718993194043da301e4f78d6a36848058c1a18599546db3628ca579519dc`.

## Combined 10-bit I420 loop filtering, CDEF, and SGR restoration

The 16x16 fixture `coverage_i420_intra_loopfilter_cdef_restoration_10bit_16x16.avif`
exercises nonzero luma and chroma loop-filter levels together with the same
single-entry CDEF and SGR profile. Libaom's normal tiny-frame search does not
retain active deblocking for this profile, so the fixture-only companion patch
[`libaom-force-i420-loopfilter.patch`](libaom-force-i420-loopfilter.patch)
sets all four levels to 1 for a 10-bit 16x16 I420 key frame. Apply it together
with `libaom-force-i420-cdef-restoration.patch` to a clean pinned libaom 3.13.2
source tree. Build it in a separate directory so the existing zero-loop-filter
fixture remains reproducible.

```sh
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-i420-loop-cdef-restoration
git -C target/oracle-staging/libaom-i420-loop-cdef-restoration checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-i420-loop-cdef-restoration apply --check \
  "$PWD/scripts/avif_fixture_oracle/libaom-force-i420-cdef-restoration.patch"
git -C target/oracle-staging/libaom-i420-loop-cdef-restoration apply \
  "$PWD/scripts/avif_fixture_oracle/libaom-force-i420-cdef-restoration.patch"
git -C target/oracle-staging/libaom-i420-loop-cdef-restoration apply --check \
  "$PWD/scripts/avif_fixture_oracle/libaom-force-i420-loopfilter.patch"
git -C target/oracle-staging/libaom-i420-loop-cdef-restoration apply \
  "$PWD/scripts/avif_fixture_oracle/libaom-force-i420-loopfilter.patch"
cmake -S target/oracle-staging/libaom-i420-loop-cdef-restoration -B target/oracle-staging/libaom-i420-loop-cdef-restoration-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-i420-loop-cdef-restoration-prefix"
cmake --build target/oracle-staging/libaom-i420-loop-cdef-restoration-build -j 4
cmake --install target/oracle-staging/libaom-i420-loop-cdef-restoration-build
cmake -S target/oracle-staging/cdef-chroma-avif-source -B target/oracle-staging/libavif-i420-loop-cdef-restoration-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=ON \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-i420-loop-cdef-restoration-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-i420-loop-cdef-restoration-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-i420-loop-cdef-restoration-prefix/include"
cmake --build target/oracle-staging/libavif-i420-loop-cdef-restoration-build --target avifenc -j 4
.oracle-venv/bin/python scripts/generate_avif_intra_filters.py \
  --variant i420_cdef_restoration_loopfilter \
  --avifenc target/oracle-staging/libavif-i420-loop-cdef-restoration-build/avifenc
.oracle-venv/bin/python scripts/generate_avif_intra_filters.py \
  --variant i420_cdef_restoration_loopfilter --check
```

The deterministic source is the same 16x16 Y4M input as the zero-loop-filter
profile. The generator requires the exact 10-bit I420 key-frame syntax, nonzero
loop-filter levels, active CDEF and SGR restoration, repeat-encode identity,
and Pillow 12.2.0/libavif 1.4.1 pixel output. The AVIF SHA-256 is
`8901fba07e2d68e35baa03bd521d22048e7d2ff2bd969a02b195541f5958df54`; the
768-byte RGB reference SHA-256 is
`4d3f43960d12c77079790677f805b6deec2470a7c4deee5b498948d2f434dd30`.

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

The no-restoration companion uses the same deterministic 17x17 I420 source
with restoration disabled. It preserves actual luma and ceil-rounded chroma
upscaling while exercising the intra reconstruction path. The generator also
checks that all three AV1 restoration types are absent. Generate it with:

```sh
.oracle-venv/bin/python scripts/generate_avif_equal_width_superres.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc \
  --chroma 420-actual-no-restoration
```

The source Y4M, AVIF, and decoded-pixel SHA-256 values are respectively
`2b6a7a2db94663d55e4be325b666b9dbae21bf8ef368863772ebacdef37dcb86`,
`175f4eed6919458cd0f3af455f20e6a0be83bfd059b2e63e745ad8fab3e46c35`, and
`36b2cbf4958b72a92a936b63d8b905bfd49dfb49b18511cc01723e880576b57b`.


Generate the two-tile monochrome super-resolution case with the same pinned
encoder:

```sh
.oracle-venv/bin/python scripts/generate_avif_equal_width_superres.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc \
  --chroma 400-multitile-actual
```

This 288x64 4:0:0 still signals denominator 9, with coded width 256, two full-width tile
columns, and restoration disabled. The generator checks the AV1 sequence, frame and tile headers,
byte-for-byte re-encoding, and exact Pillow 12.2.0 pixels before writing
`multitile_monochrome_superres_288x64.avif`. The source Y4M, AVIF, and decoded
pixel SHA-256 values are respectively
`c8ebdc997141ba1718abe3b81819e6383f2d3724f54a646525f98e3059c30b60`,
`cc88ed36062a37989bef826649d01c2fa872f41c3668c59509f310ff081a93d4`, and
`30ba32d9eda130257562b74f7fa6a49025176071264bfca20a3d87cae6951479`.

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

The 10-bit animated I422 companion is generated by
`scripts/generate_avif_hi10_i422_superres_restoration.py`. Its inter frame is
coded at 128x56, super-resolved to 160x56, and uses SGR restoration on all
planes. Disabling transform-size search keeps the frame in
`TX_MODE_LARGEST`, the high-depth restoration profile exercised by its Rust
parity case. The generator normalizes sequence timestamps and checks both
repeat-encode stability and exact Pillow 12.2.0 RGB frames.

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

The `--lossless-roi-residual` mode keeps its existing intra-predicted residual
case. `--lossless-roi-impulse-residual` adds one residual at `(33,33)` while
preserving inter prediction for the qindex-zero ROI. The fixture-only libaom
patch selects 4x4 transforms for that leaf after state update, and the encoder
trace must report `inter=1` and `skip_txfm=0`.

Add `--bit-depth 10` to `scripts/generate_avif_mixed_lossless_inter_b32.py`
with `--lossless-roi-impulse-residual` to generate the corresponding high-bit-
depth parity fixture. Its AV1 sequence header must report 10-bit I420, while
the inter-frame segment map remains mixed and the B32 ROI remains lossless.
The same encoder trace, independent repeat runs, exact Pillow RGB frames, and
native loop evidence are checked.

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

.oracle-venv/bin/python scripts/generate_avif_mixed_lossless_inter_b32.py \
  --encoder target/oracle-staging/encode-avif-mixed-lossless-inter-b32 \
  --lossless-roi-impulse-residual
```

The generator writes a receipt under ignored `target/oracle-staging/` with
patch, encoder, bitstream, frame-header, trace, and Pillow identities. Only
the three version-one BMFF timestamp pairs are normalized; all other bytes
must be deterministic. Keep these patched builds apart from the clean pinned
libavif checkout used by the loop observer.

## Lossy 10-bit I420 B128x128 sequence

`animated_lossy_wide_i420_10bit_b128x128.avif` is encoded from deterministic
full-range 10-bit I420 planes by `encode_lossy_wide_i420_10bit.c`. The
128x128 sequence uses one encoder thread, 128x128 superblocks, fixed 128-pixel
partition limits, largest transforms, and active SGR restoration on all three
planes. The generator checks repeatable timestamp-normalized bytes, exact
Pillow 12.2.0 RGB frames, and the AV1 profile, bit depth, subsampling, frame
types, transform mode, and restoration headers. These checks establish the
configured encoder case and its public decode behavior; they do not claim a
per-block encoder trace.

Compile the helper against the static libraries from the pinned build above,
then generate the fixture:

```sh
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-superres/include \
  -I target/oracle-staging/libavif-superres-build/include \
  -I target/oracle-staging/aom-prefix/include \
  scripts/avif_fixture_oracle/encode_lossy_wide_i420_10bit.c \
  target/oracle-staging/libavif-superres-build/libavif.a \
  target/oracle-staging/aom-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-lossy-wide-i420-10bit
.oracle-venv/bin/python scripts/generate_avif_lossy_wide_i420_10bit.py \
  --encoder target/oracle-staging/encode-avif-lossy-wide-i420-10bit
```

## Lossy wide I422 B128x64 sequence

`animated_lossy_wide_i422_b128x64.avif` is encoded from deterministic 8-bit I422 planes by `encode_lossy_wide_i422_b128x64.c`. It uses a fixture-only libaom patch at the pinned 3.13.2 commit to select a valid horizontal split of the inter B128x128 root. It does not alter AV1 plane block-size mapping. The second frame therefore contains two I422 B128x64 leaves, and does not claim a B64x128 chroma block.

Build the fixture-only libraries and encoder from a fresh staging directory:

```sh
mkdir -p target/oracle-staging
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-i422-b128x64-source
git -C target/oracle-staging/libaom-i422-b128x64-source checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-i422-b128x64-source apply \
  "$PWD/scripts/avif_fixture_oracle/libaom-lossy-wide-i422-b128x64.patch"
cmake -S target/oracle-staging/libaom-i422-b128x64-source \
  -B target/oracle-staging/libaom-i422-b128x64-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-i422-b128x64-prefix"
cmake --build target/oracle-staging/libaom-i422-b128x64-build -j 4
cmake --install target/oracle-staging/libaom-i422-b128x64-build

git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-i422-b128x64-source
git -C target/oracle-staging/libavif-i422-b128x64-source checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-i422-b128x64-source apply \
  "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
cmake -S target/oracle-staging/libavif-i422-b128x64-source \
  -B target/oracle-staging/libavif-i422-b128x64-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=OFF \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-i422-b128x64-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-i422-b128x64-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-i422-b128x64-prefix/include"
cmake --build target/oracle-staging/libavif-i422-b128x64-build --target avif_static -j 4
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-i422-b128x64-source/include \
  -I target/oracle-staging/libavif-i422-b128x64-build/include \
  -I target/oracle-staging/libaom-i422-b128x64-prefix/include \
  scripts/avif_fixture_oracle/encode_lossy_wide_i422_b128x64.c \
  target/oracle-staging/libavif-i422-b128x64-build/libavif.a \
  target/oracle-staging/libaom-i422-b128x64-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-lossy-wide-i422-b128x64
.oracle-venv/bin/python scripts/generate_avif_lossy_wide_i422_b128x64.py \
  --encoder target/oracle-staging/encode-avif-lossy-wide-i422-b128x64
```

The generator normalizes only the version-one BMFF movie, track, and media creation/modification timestamps. It rejects a different normalized file digest, changed repeated encodes, or different Pillow RGB frame hashes. A separate clean pinned libavif checkout is used for native loop evidence.

## Lossy inter I420 B64x16 sequence

`animated_lossy_inter_420_b64x16_mode_largest_64x64.avif` and
`animated_lossy_inter_420_b64x16_mode_select_64x64.avif` are two-frame 8-bit
I420 sequences. The 10-bit and 12-bit companions,
`animated_lossy_inter_420_b64x16_10bit_mode_select_64x64.avif` and
`animated_lossy_inter_420_b64x16_12bit_mode_select_64x64.avif`, keep the
select-mode pattern at high bit depths. A fixture-only patch to pinned libaom
3.13.2 makes each inter frame's origin B64x64 choose `PARTITION_HORZ_4`,
yielding four coded B64x16 leaves with positive qindex. The 8-bit transform
modes are `TX_MODE_LARGEST` and `TX_MODE_SELECT`; both high-bit-depth
companions signal `TX_MODE_SELECT`. The pinned dav1d trace confirms the
inter-frame root partition for the 8-bit pair, and Pillow checks exact RGB
output for every frame.

The select-mode pattern gives its top leaf a low-frequency residual while
keeping texture in the other leaves, so the frame signals transform selection
while the top leaf can retain its maximum transform. A selected LLVM parity
report observes both guard outcomes in one decoder instantiation across the
8-bit and 10-bit select cases; seven related generic instantiations remain
uncovered. The patch only controls fixture encoding and does not change the
Rust decoder.

Build the fixture-only encoder from separate pinned source copies:

```sh
mkdir -p target/oracle-staging
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-i420-b64x16-source
git -C target/oracle-staging/libaom-i420-b64x16-source checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-i420-b64x16-source apply \
  "$PWD/scripts/avif_fixture_oracle/libaom-lossy-inter-i420-b64x16.patch"
cmake -S target/oracle-staging/libaom-i420-b64x16-source \
  -B target/oracle-staging/libaom-i420-b64x16-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-i420-b64x16-prefix"
cmake --build target/oracle-staging/libaom-i420-b64x16-build -j 4
cmake --install target/oracle-staging/libaom-i420-b64x16-build

git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-i420-b64x16-source
git -C target/oracle-staging/libavif-i420-b64x16-source checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-i420-b64x16-source apply \
  "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
cmake -S target/oracle-staging/libavif-i420-b64x16-source \
  -B target/oracle-staging/libavif-i420-b64x16-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=OFF \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-i420-b64x16-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-i420-b64x16-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-i420-b64x16-prefix/include"
cmake --build target/oracle-staging/libavif-i420-b64x16-build --target avif_static -j 4
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-i420-b64x16-source/include \
  -I target/oracle-staging/libavif-i420-b64x16-build/include \
  -I target/oracle-staging/libaom-i420-b64x16-prefix/include \
  scripts/avif_fixture_oracle/encode_lossy_inter_i420_b64x16.c \
  target/oracle-staging/libavif-i420-b64x16-build/libavif.a \
  target/oracle-staging/libaom-i420-b64x16-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-inter-i420-b64x16
.oracle-venv/bin/python scripts/generate_avif_inter_i420_b64x16.py \
  --mode largest --encoder target/oracle-staging/encode-avif-inter-i420-b64x16
.oracle-venv/bin/python scripts/generate_avif_inter_i420_b64x16.py \
  --mode select --encoder target/oracle-staging/encode-avif-inter-i420-b64x16
.oracle-venv/bin/python scripts/generate_avif_inter_i420_b64x16.py \
  --mode select --bit-depth 10 --encoder target/oracle-staging/encode-avif-inter-i420-b64x16
.oracle-venv/bin/python scripts/generate_avif_inter_i420_b64x16.py \
  --mode select --bit-depth 12 --encoder target/oracle-staging/encode-avif-inter-i420-b64x16
```

For each profile, the generator requires two identical normalized encodes, the
expected AV1 frame-header transform mode and bit depth, and exact Pillow 12.2.0
RGB frames. It normalizes only the version-one BMFF movie, track, and media
creation and modification timestamps. The retained file SHA-256 values are:

- Largest mode, 8-bit: `35f16973d0bda15b2a2038102b3f646f2b04d6e07157933bd03b8b32a6452daf`
- Select mode, 8-bit: `84eab6d96f97deb02fca49e66602f9cb507d2281cde5d4b52b2a50c54cb0433a`
- Select mode, 10-bit: `0d85dc1e3e8958e7375c3dcddd048f93cc5bd7ad12f50f82f1299f1c482c1880`
- Select mode, 12-bit: `8583340a327c7feffa803fa3b0a80267a49b6fa21c062f9b19799e2a82bf1dcc`
`make fixtures-check` validates both tracked fixtures using the pinned Pillow
oracle without rebuilding the fixture encoder.

## Lossy inter I420 B8x8 sequence

`animated_lossy_inter_420_b8x8_mode_select_64x64.avif` is a two-frame 8-bit
I420 sequence. A fixture-only patch to pinned libaom 3.13.2 leaves key-frame
search unchanged and forces inter square partitions down to 8x8. The inter
frame retains positive qindex and `TX_MODE_SELECT`; this fixture-only patch
does not alter the Rust decoder. The generator requires byte-identical
timestamp-normalized encodes, exact Pillow RGB pixels for both frames, and the
expected AV1 key/inter headers.

Build the fixture encoder from separate pinned source copies:

```sh
mkdir -p target/oracle-staging
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-i420-b8x8-source
git -C target/oracle-staging/libaom-i420-b8x8-source checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-i420-b8x8-source apply \
  "$PWD/scripts/avif_fixture_oracle/libaom-lossy-inter-i420-b8x8.patch"
cmake -S target/oracle-staging/libaom-i420-b8x8-source \
  -B target/oracle-staging/libaom-i420-b8x8-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-i420-b8x8-prefix"
cmake --build target/oracle-staging/libaom-i420-b8x8-build -j 4
cmake --install target/oracle-staging/libaom-i420-b8x8-build

git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-i420-b8x8-source
git -C target/oracle-staging/libavif-i420-b8x8-source checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-i420-b8x8-source apply \
  "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
cmake -S target/oracle-staging/libavif-i420-b8x8-source \
  -B target/oracle-staging/libavif-i420-b8x8-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=OFF \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-i420-b8x8-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-i420-b8x8-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-i420-b8x8-prefix/include"
cmake --build target/oracle-staging/libavif-i420-b8x8-build --target avif_static -j 4
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-i420-b8x8-source/include \
  -I target/oracle-staging/libavif-i420-b8x8-build/include \
  -I target/oracle-staging/libaom-i420-b8x8-prefix/include \
  scripts/avif_fixture_oracle/encode_lossy_inter_i420_b8x8.c \
  target/oracle-staging/libavif-i420-b8x8-build/libavif.a \
  target/oracle-staging/libaom-i420-b8x8-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-inter-i420-b8x8
.oracle-venv/bin/python scripts/generate_avif_inter_i420_b8x8.py \
  --encoder target/oracle-staging/encode-avif-inter-i420-b8x8
```

The normalized fixture SHA-256 is
`a4963f38d42c857cbacd6306ab5585c0e42455fd6c3725dfdffda27b7553e6f1`. Pillow
12.2.0 frame hashes are `547f53c99d9fc578e69c7cf37a55ef187ca316ce792d1641127de6f9c7ef58fa`
and `eec41a0b35aefcb11f78418d9e1fbd3617ae6dcadb7182de12ca4293f1794487`.
Independent pinned libavif evidence records repetition count zero (one total
play). `make fixtures-check` validates the retained Pillow references.

## Lossy inter I444 B8x16 TX_MODE_SELECT sequence

`animated_lossy_inter_i444_b8x16_mode_select_64x64.avif` is a deterministic
two-frame 64x64 8-bit I444 sequence. A fixture-only libaom patch forces the
second frame's 16x16 inter partitions into vertical splits, producing B8x16
leaves while retaining positive qindex and `TX_MODE_SELECT`. The Rust decoder
is unchanged. The fixture retains both exact Pillow RGB frames and an
independent pinned libavif one-play observation.

Build the fixture encoder with the same pinned oracle revisions:

```sh
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-i444-b8x16-source
git -C target/oracle-staging/libaom-i444-b8x16-source checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-i444-b8x16-source apply \
  "$PWD/scripts/avif_fixture_oracle/libaom-lossy-inter-i444-b8x16.patch"
cmake -S target/oracle-staging/libaom-i444-b8x16-source \
  -B target/oracle-staging/libaom-i444-b8x16-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-i444-b8x16-prefix"
cmake --build target/oracle-staging/libaom-i444-b8x16-build -j 4
cmake --install target/oracle-staging/libaom-i444-b8x16-build

git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-i444-b8x16-source
git -C target/oracle-staging/libavif-i444-b8x16-source checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-i444-b8x16-source apply \
  "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
cmake -S target/oracle-staging/libavif-i444-b8x16-source \
  -B target/oracle-staging/libavif-i444-b8x16-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=OFF \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-i444-b8x16-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-i444-b8x16-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-i444-b8x16-prefix/include"
cmake --build target/oracle-staging/libavif-i444-b8x16-build --target avif_static -j 4
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-i444-b8x16-source/include \
  -I target/oracle-staging/libavif-i444-b8x16-build/include \
  -I target/oracle-staging/libaom-i444-b8x16-prefix/include \
  scripts/avif_fixture_oracle/encode_lossy_inter_i444_b8x16.c \
  target/oracle-staging/libavif-i444-b8x16-build/libavif.a \
  target/oracle-staging/libaom-i444-b8x16-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-inter-i444-b8x16
.oracle-venv/bin/python scripts/generate_avif_inter_i444_b8x16.py \
  --encoder target/oracle-staging/encode-avif-inter-i444-b8x16
```

The normalized fixture SHA-256 is
`0187f7634e7ffb21019961fbb47091c3c2deaaccd88a04c043a7ffe1b05b8f93`. Pillow
12.2.0 frame hashes are
`88c35ff901b357eb27dd0b079351ce61ea088d2d8bd1ab4fb9a215f3fc6bf930` and
`24c6d0c2cbe9558c7ce24811c04af120ac2e6cc811e2bb0b89377b699185d157`.
`make verify-fixture-integrity` checks the retained references.

## High-depth lossy inter I444 B16x64 mode-2 sequence

`animated_lossy_inter_i444_b16x64_mode_select_10bit_64x64.avif` is a
deterministic two-frame 64x64 full-range 10-bit I444 sequence. The fixture-only
libaom patch forces a vertical four-way split at the second frame's 64x64
root, creating exact B16x64 leaves. It also keeps `TX_MODE_SELECT` on that
frame when libaom's normal no-split fallback would change the frame header to
`TX_MODE_LARGEST`. Positive qindex and non-lossless segments keep the lossy
transform path active. The patch does not change the Rust decoder. Both exact
Pillow RGB frames and an independent pinned libavif one-play observation are
retained.

Build the fixture encoder from the pinned oracle revisions:

```sh
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-i444-b16x64-mode2-source
git -C target/oracle-staging/libaom-i444-b16x64-mode2-source checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-i444-b16x64-mode2-source apply \
  "$PWD/scripts/avif_fixture_oracle/libaom-lossy-inter-i444-b16x64-mode2.patch"
cmake -S target/oracle-staging/libaom-i444-b16x64-mode2-source \
  -B target/oracle-staging/libaom-i444-b16x64-mode2-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-i444-b16x64-mode2-prefix"
cmake --build target/oracle-staging/libaom-i444-b16x64-mode2-build -j 4
cmake --install target/oracle-staging/libaom-i444-b16x64-mode2-build

git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-i444-b16x64-mode2-source
git -C target/oracle-staging/libavif-i444-b16x64-mode2-source checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-i444-b16x64-mode2-source apply \
  "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
cmake -S target/oracle-staging/libavif-i444-b16x64-mode2-source \
  -B target/oracle-staging/libavif-i444-b16x64-mode2-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_AOM_ENCODE=ON -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF \
  -DAVIF_BUILD_APPS=OFF -DAVIF_BUILD_TESTS=OFF -DAVIF_BUILD_EXAMPLES=OFF \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-i444-b16x64-mode2-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-i444-b16x64-mode2-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-i444-b16x64-mode2-prefix/include"
cmake --build target/oracle-staging/libavif-i444-b16x64-mode2-build --target avif_static -j 4
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-i444-b16x64-mode2-source/include \
  -I target/oracle-staging/libavif-i444-b16x64-mode2-build/include \
  -I target/oracle-staging/libaom-i444-b16x64-mode2-prefix/include \
  scripts/avif_fixture_oracle/encode_lossy_inter_i444_b16x64_mode2.c \
  target/oracle-staging/libavif-i444-b16x64-mode2-build/libavif.a \
  target/oracle-staging/libaom-i444-b16x64-mode2-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-inter-i444-b16x64-mode2
.oracle-venv/bin/python scripts/generate_avif_inter_i444_b16x64_mode2.py \
  --encoder target/oracle-staging/encode-avif-inter-i444-b16x64-mode2
```

The normalized fixture SHA-256 is
`86fdcab5cc8355ec8706ea40ea3e9f75e8ee05a5b54c3043bf7416417bfe1de6`. Pillow
12.2.0 frame hashes are
`6de4e80faf42899e53568d005b9fcadaae744531ef886f308641dda0d33de8cb` and
`e645285f52e17e263e9a67fe6b0aa9eb6d18b1cb4177e3857ec6092a348bff8d`.
`make coverage-selected COVERAGE_CASES='decode:avif:animated_lossy_inter_i444_b16x64_mode_select_10bit_64x64'`
executes the public parity row and retains its selected coverage report.

## Lossy inter I420 B16x64 TX_MODE_SELECT sequence

`animated_lossy_inter_420_b16x64_mode_select_64x64.avif` is a two-frame 8-bit
I420 sequence whose second frame's 64x64 root is forced to
`PARTITION_VERT_4`, creating four B16x64 leaves. The left leaf has a
low-frequency residual and the other leaves keep texture, so the frame
signals `TX_MODE_SELECT` while the first leaf selects the maximum 16x64
transform. The fixture-only controls are in
`libaom-lossy-inter-i420-b16x64.patch`; they do not alter the Rust decoder.
The qindex-zero companion uses the same layout and quality 100. Its key frame
stays lossless; the additional `libaom-qindex0-nonlossless-inter.patch` applies
chroma quantizer deltas of 2 only to inter frame 1. That legal AV1 frame has
base qindex zero but is non-lossless, reaching the decoder's single-terminal
transform path. Both patches affect only this fixture encoder.

Build a separate pinned libaom copy with that patch:

```sh
mkdir -p target/oracle-staging
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-i420-b16x64-source
git -C target/oracle-staging/libaom-i420-b16x64-source checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-i420-b16x64-source apply \
  "$PWD/scripts/avif_fixture_oracle/libaom-lossy-inter-i420-b16x64.patch"
git -C target/oracle-staging/libaom-i420-b16x64-source apply \
  "$PWD/scripts/avif_fixture_oracle/libaom-qindex0-nonlossless-inter.patch"
cmake -S target/oracle-staging/libaom-i420-b16x64-source \
  -B target/oracle-staging/libaom-i420-b16x64-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-i420-b16x64-prefix"
cmake --build target/oracle-staging/libaom-i420-b16x64-build -j 4
cmake --install target/oracle-staging/libaom-i420-b16x64-build
```

Build the pinned static libavif fixture library against that AOM copy, then
compile and run the deterministic encoder:

```sh
git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-i420-b16x64-source
git -C target/oracle-staging/libavif-i420-b16x64-source checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-i420-b16x64-source apply \
  "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
cmake -S target/oracle-staging/libavif-i420-b16x64-source \
  -B target/oracle-staging/libavif-i420-b16x64-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=OFF \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-i420-b16x64-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-i420-b16x64-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-i420-b16x64-prefix/include"
cmake --build target/oracle-staging/libavif-i420-b16x64-build --target avif_static -j 4
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-i420-b16x64-source/include \
  -I target/oracle-staging/libavif-i420-b16x64-build/include \
  -I target/oracle-staging/libaom-i420-b16x64-prefix/include \
  scripts/avif_fixture_oracle/encode_lossy_inter_i420_b16x64.c \
  target/oracle-staging/libavif-i420-b16x64-build/libavif.a \
  target/oracle-staging/libaom-i420-b16x64-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-inter-i420-b16x64
.oracle-venv/bin/python scripts/generate_avif_inter_i420_b16x64_mode_select.py \
  --encoder target/oracle-staging/encode-avif-inter-i420-b16x64
```

The generator checks two byte-identical timestamp-normalized encodes, the AV1
frame headers, exact Pillow 12.2.0 RGB pixels, and this pinned AVIF SHA-256:
`6241cd581ea930154ddf66554966a069d03ebf06c8d2a4880a4a0008398e1a4a`.
Generate and verify the qindex-zero companion with the same encoder:

```sh
.oracle-venv/bin/python scripts/generate_avif_inter_i420_b16x64_qindex0_mode_select.py \
  --encoder target/oracle-staging/encode-avif-inter-i420-b16x64
```

That generator verifies the lossless key frame, the inter frame's qindex zero,
nonzero chroma deltas and non-lossless `TX_MODE_SELECT`, byte-identical
timestamp-normalized encodes, and exact Pillow 12.2.0 pixels. Its retained AVIF
SHA-256 is `f9a69337b2eab0d0e6623aa874fdfcf58c35ada2154e8377a595b401c487317c`.
`make fixtures-check` validates the committed fixture against its generator and
Pillow observations without rebuilding the native encoder.

The reduced-transform-set companion uses quality 70 and the same B16x64
encoder with its `--reduced-transform-set` option. It passes the ordinary AOM
`reduced-tx-type-set=1` control; both picture headers carry the reduced set,
and the inter frame retains TX_MODE_SELECT and positive qindex. Generate and
check its two exact Pillow frames with:

```sh
.oracle-venv/bin/python scripts/generate_avif_inter_reduced_transform_set.py \
  --encoder target/oracle-staging/encode-avif-inter-i420-b16x64
.oracle-venv/bin/python scripts/generate_avif_inter_reduced_transform_set.py --check
```

The normalized input SHA-256 is
`3a52bdb0573571d5b9d6c5f79cbf4f41816fa84e4ec052034a9e418b9797511e`.
The native repetition collector uses
`--only lossy_inter_420_b16x64_reduced_transform_set_64x64` with its separate
clean pinned libavif checkout and a fresh output directory. `make verify`
checks the committed fixture without rebuilding the encoder.

The monochrome companion uses the same pinned sources and partition controls.
The qindex-zero patch sets a luma DC delta of 2 on its inter frame, keeping the
base qindex at zero while making that frame non-lossless. Compile the
monochrome-specific encoder and generate the fixture with:

```sh
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-i420-b16x64-source/include \
  -I target/oracle-staging/libavif-i420-b16x64-build/include \
  -I target/oracle-staging/libaom-i420-b16x64-prefix/include \
  scripts/avif_fixture_oracle/encode_lossy_inter_monochrome_b16x64_qindex0.c \
  target/oracle-staging/libavif-i420-b16x64-build/libavif.a \
  target/oracle-staging/libaom-i420-b16x64-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-inter-mono-b16x64-qzero
.oracle-venv/bin/python scripts/generate_avif_inter_monochrome_b16x64_qindex0_mode_select.py \
  --encoder target/oracle-staging/encode-avif-inter-mono-b16x64-qzero
```

The generator checks two byte-identical timestamp-normalized encodes, the
monochrome AV1 sequence header, the lossless key and non-lossless
`TX_MODE_SELECT` inter header, and exact Pillow RGB frames. The retained AVIF
SHA-256 is `972330af1b5c8bd691dae557ccb6e70532959c52009f5e840aca73989975bf3c`.

## Non-lossless qindex-zero I422 sequence

`animated_lossy_inter_422_qindex0_mode_select_64x64.avif` is a two-frame
64x64 8-bit I422 sequence. The quality-100 key frame remains lossless. A
fixture-only libaom patch sets a luma DC quantizer delta of 2 on inter frame 1,
so the second frame has base qindex zero but is not lossless. Chroma deltas
stay zero; this preserves Pillow decoding while exercising the I422
`TX_MODE_SELECT` qindex-zero input path.

Build the fixture encoder from fresh pinned source checkouts:

```sh
mkdir -p target/oracle-staging
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-i422-qindex0-source
git -C target/oracle-staging/libaom-i422-qindex0-source checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-i422-qindex0-source apply \
  "$PWD/scripts/avif_fixture_oracle/libaom-qindex0-nonlossless-i422.patch"
cmake -S target/oracle-staging/libaom-i422-qindex0-source \
  -B target/oracle-staging/libaom-i422-qindex0-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-i422-qindex0-prefix"
cmake --build target/oracle-staging/libaom-i422-qindex0-build -j 4
cmake --install target/oracle-staging/libaom-i422-qindex0-build

git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-i422-qindex0-source
git -C target/oracle-staging/libavif-i422-qindex0-source checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-i422-qindex0-source apply \
  "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
cmake -S target/oracle-staging/libavif-i422-qindex0-source \
  -B target/oracle-staging/libavif-i422-qindex0-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=OFF \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-i422-qindex0-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-i422-qindex0-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-i422-qindex0-prefix/include"
cmake --build target/oracle-staging/libavif-i422-qindex0-build --target avif_static -j 4
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-i422-qindex0-source/include \
  -I target/oracle-staging/libavif-i422-qindex0-build/include \
  -I target/oracle-staging/libaom-i422-qindex0-prefix/include \
  scripts/avif_fixture_oracle/encode_lossy_inter_i422_small_partitions.c \
  target/oracle-staging/libavif-i422-qindex0-build/libavif.a \
  target/oracle-staging/libaom-i422-qindex0-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-inter-i422-qindex0
.oracle-venv/bin/python scripts/generate_avif_inter_422_qindex0_mode_select.py \
  --encoder target/oracle-staging/encode-avif-inter-i422-qindex0
```

The generator requires two identical timestamp-normalized encodes, an
8-bit I422 sequence header, a lossless key frame, a non-lossless qindex-zero
`TX_MODE_SELECT` inter frame, exact Pillow 12.2.0 RGB frames, and this AVIF
SHA-256: `062cec9bf86cfa7e5d2130baec72884dec70d96d9621578f91388612d21a1fab`.
Run `make verify-fixture-integrity` to check the retained fixture and
references without rebuilding the native encoder.

### Horizontal B8x4 I422 at qindex zero

`animated_lossy_inter_422_b8x4_qindex0_mode_select_64x64.avif` is a two-frame
64x64 8-bit I422 sequence. Its inter frame has base qindex zero, luma DC delta
two, zero chroma deltas, and `TX_MODE_SELECT`. The fixture-only AOM patch
[`libaom-i422-b8x4-qindex0-witness.patch`](libaom-i422-b8x4-qindex0-witness.patch)
sets the first inter B16x16 root to four B8x8 blocks, then retains a valid
horizontal B8x4 partition at the top-left B8x8 root. These controls are limited
to that root in the second inter frame. Apply it after
`libaom-qindex0-nonlossless-i422.patch`. This input reaches the I422 B8x4
qindex-zero decoder path while leaving the Rust decoder unchanged.

The complete AVIF hash is
`5c2fd79d69b2fb1a96542c30ea4eb6d103993982f9f322b1e64d040730f946f9`.
Pillow 12.2.0 produces two 64x64 RGB frames, both with 100 ms duration; their
hashes are `246f2904b49d857c90295a1a379d1eadea4058a6e04c8df8639aac4392732d50`
and `2daef02f6cde1d7914f04e930bf65f36a0bc74273f1a1029a5ec054434611b14`.

Start with the pinned AOM source, apply the I422 qindex-zero quantizer patch
and then the partition-witness patch. Build that source as a separate generic
AOM library. Reuse the pinned static libavif build from the preceding I422
qindex-zero section, configured against this AOM library. Then compile the
narrow-inter producer for 64x64 I422 at root size 8 and quality 100:

```sh
mkdir -p target/oracle-staging/libaom-i422-b8x4-qindex0-source
git -C target/oracle-staging/libaom archive ad44980d7f3c7a2605c25d51ea96946949000841 |
  tar -x -C target/oracle-staging/libaom-i422-b8x4-qindex0-source
git apply --directory=target/oracle-staging/libaom-i422-b8x4-qindex0-source \
  scripts/avif_fixture_oracle/libaom-qindex0-nonlossless-i422.patch
git apply --directory=target/oracle-staging/libaom-i422-b8x4-qindex0-source \
  scripts/avif_fixture_oracle/libaom-i422-b8x4-qindex0-witness.patch
cmake -S target/oracle-staging/libaom-i422-b8x4-qindex0-source \
  -B target/oracle-staging/libaom-i422-b8x4-qindex0-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0
cmake --build target/oracle-staging/libaom-i422-b8x4-qindex0-build -j 4
cc -std=c99 -O2 -Wall -Wextra -Werror -DFIXTURE_ROOT_SIZE=8 \
  -DFIXTURE_WIDTH=64 -DFIXTURE_HEIGHT=64 \
  -DFIXTURE_FORMAT=AVIF_PIXEL_FORMAT_YUV422 -DFIXTURE_QUALITY=100 \
  -I target/oracle-staging/libavif-narrow-b4x8-qindex0-source/include \
  -I target/oracle-staging/libavif-narrow-b4x8-qindex0-build/include \
  -I target/oracle-staging/libaom-i422-b8x4-qindex0-source \
  scripts/avif_fixture_oracle/encode_narrow_inter.c \
  target/oracle-staging/libavif-narrow-b4x8-qindex0-build/libavif.a \
  target/oracle-staging/libaom-i422-b8x4-qindex0-build/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-i422-b8x4-qzero
.oracle-venv/bin/python scripts/generate_avif_inter_422_qindex0_mode_select.py \
  --variant b8x4 --encoder target/oracle-staging/encode-avif-i422-b8x4-qzero
.oracle-venv/bin/python scripts/generate_avif_inter_422_qindex0_mode_select.py \
  --variant b8x4 --check
```

For this variant the generator repeats `encode_narrow_inter.c` with `select 8
coarse 0` (selective transform mode, coarse input, frozen CDF updates),
normalizes timestamps, verifies identical encodes and checks the
qindex-zero I422 AV1 headers plus both Pillow RGB frames. `--check` validates
the retained file without rebuilding the fixture encoder; `make verify` checks
the committed input and references as part of the normal fixture workflow.

## Lossless 10-bit I420 B128 sequence

`animated_lossless_inter_420_b128x128_10bit.avif` uses a 128x128 AV1
superblock, 10-bit full-range I420, and two all-lossless frames. The second
frame moves an 8x8 luma patch one pixel. Build the encoder against the pinned
static libavif and libaom artifacts described above:

```sh
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-i420-b16x64-source/include \
  -I target/oracle-staging/libavif-i420-b16x64-build/include \
  -I target/oracle-staging/libaom-i420-b16x64-prefix/include \
  scripts/avif_fixture_oracle/encode_lossless_inter_i420_b128x128_10bit.c \
  target/oracle-staging/libavif-i420-b16x64-build/libavif.a \
  target/oracle-staging/libaom-i420-b16x64-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-lossless-i420-b128x128
.oracle-venv/bin/python scripts/generate_avif_lossless_inter_i420_b128x128_10bit.py \
  --encoder target/oracle-staging/encode-avif-lossless-i420-b128x128
```

The generator requires two encodes to agree after normalizing only BMFF
creation and modification timestamps. It validates the 128x128 superblock,
key/inter all-lossless frame headers, and exact Pillow 12.2.0 RGB frames. Its
`--check` mode runs from `make verify` without rebuilding the native encoder.

## Lossless 8-bit I420 B4 partitions

`animated_lossless_inter_420_b4x4_leaves_64x64.avif` contains two all-lossless
64x64 I420 frames. The fixture encoder sets both AOM partition bounds to four
pixels, disables rectangular partition forms, and moves a four-pixel luma
patch between frames. The pinned Pillow decode and the Rust parity case retain
both complete RGB frames.

Build clean pinned libaom and libavif sources, applying only the repository's
fixture-control patch to libavif:

```sh
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-b4x4-source
git -C target/oracle-staging/libaom-b4x4-source checkout ad44980d7f3c7a2605c25d51ea96946949000841
cmake -S target/oracle-staging/libaom-b4x4-source -B target/oracle-staging/libaom-b4x4-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-b4x4-prefix"
cmake --build target/oracle-staging/libaom-b4x4-build -j 4
cmake --install target/oracle-staging/libaom-b4x4-build

git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-b4x4-source
git -C target/oracle-staging/libavif-b4x4-source checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-b4x4-source apply \
  "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
cmake -S target/oracle-staging/libavif-b4x4-source -B target/oracle-staging/libavif-b4x4-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=OFF \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-b4x4-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-b4x4-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-b4x4-prefix/include"
cmake --build target/oracle-staging/libavif-b4x4-build --target avif_static -j 4

clang -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-b4x4-source/include \
  -I target/oracle-staging/libavif-b4x4-build/include \
  -I target/oracle-staging/libaom-b4x4-prefix/include \
  scripts/avif_fixture_oracle/encode_lossless_inter_i420_b4x4.c \
  target/oracle-staging/libavif-b4x4-build/libavif.a \
  target/oracle-staging/libaom-b4x4-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-lossless-inter-i420-b4x4
.oracle-venv/bin/python scripts/generate_avif_lossless_inter_b4x4_fixture.py \
  --encoder target/oracle-staging/encode-lossless-inter-i420-b4x4
```

The generator requires two timestamp-normalized encodes to match, verifies the
all-lossless key/inter headers, and checks both exact Pillow 12.2.0 RGB frames.
`make verify-fixture-integrity` checks the retained input without rebuilding
the native encoder. Its AVIF SHA-256 is
`8c588829bd49a125fee501c9cdcaa1e6f9c0e6f058084270eaa039688acc9748`.

## Lossless 8-bit I420 B128 partition witnesses

The wide-grid witness
`animated_lossless_inter_420_wide_grid_b128x128.avif` is generated directly
with the pinned Pillow 12.2.0/libavif 1.4.1/libaom 3.13.2 stack:

```sh
.oracle-venv/bin/python scripts/generate_avif_lossless_wide_inter_grid_fixture.py
```

The generator checks the 128x128 superblock, all-lossless key/inter frames,
the fixed AVIF digest, and exact Pillow RGB frame hashes. Its `--check` mode is
included in `make verify` and needs no fixture-only native build.

The vertical witness
`animated_lossless_inter_420_vertical_b64x128.avif` requires a fixture-only
libaom override because the stock encoder does not select this tall partition.
The override at `libaom-lossless-wide-i420-b64x128.patch` forces the inter
4:2:0 128x128 root to `PARTITION_VERT`, producing two valid B64x128 leaves.
It changes only the fixture encoder; the Rust decoder and runtime are not
patched. Build from pinned libaom commit
`ad44980d7f3c7a2605c25d51ea96946949000841` and libavif commit
`6543b22b5bc706c53f038a16fe515f921556d9b3`:

```sh
mkdir -p target/oracle-staging
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-b64x128-source
git -C target/oracle-staging/libaom-b64x128-source checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-b64x128-source apply \
  "$PWD/scripts/avif_fixture_oracle/libaom-lossless-wide-i420-b64x128.patch"
cmake -S target/oracle-staging/libaom-b64x128-source \
  -B target/oracle-staging/libaom-b64x128-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-b64x128-prefix"
cmake --build target/oracle-staging/libaom-b64x128-build -j 4
cmake --install target/oracle-staging/libaom-b64x128-build

git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-b64x128-source
git -C target/oracle-staging/libavif-b64x128-source checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-b64x128-source apply \
  "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
cmake -S target/oracle-staging/libavif-b64x128-source \
  -B target/oracle-staging/libavif-b64x128-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=OFF \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-b64x128-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-b64x128-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-b64x128-prefix/include"
cmake --build target/oracle-staging/libavif-b64x128-build --target avif_static -j 4

cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-b64x128-source/include \
  -I target/oracle-staging/libavif-b64x128-build/include \
  -I target/oracle-staging/libaom-b64x128-prefix/include \
  scripts/avif_fixture_oracle/encode_lossless_inter_i420_b64x128.c \
  target/oracle-staging/libavif-b64x128-build/libavif.a \
  target/oracle-staging/libaom-b64x128-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-lossless-i420-b64x128
.oracle-venv/bin/python scripts/generate_avif_lossless_inter_i420_b64x128.py \
  --encoder target/oracle-staging/encode-avif-lossless-i420-b64x128
```

The generator requires two normalized encodes to match, validates the AV1
superblock and all-lossless key/inter headers, and checks both exact Pillow
RGB frames. `make verify` runs its `--check` mode without rebuilding the
fixture encoder. The retained AVIF SHA-256 is
`9ed8dd60feedcb8480bb8d6de7f8ea2ec23583da923e916e28a128c3746ca650`.

## Invalid lossless I422 B64x128 partition

`animated_lossless_inter_i422_invalid_vertical_b64x128.avif` uses the same
128x128 inter-root override with I422 sampling. The forced vertical split
creates B64x128 leaves whose chroma geometry is invalid for vertically
unsampled chroma. Pillow 12.2.0 decodes frame 0 and raises
`RuntimeError: Failed to decode frame 1: Decoding of color planes failed`.
The parity case checks that Rust reports a structured malformed sequence
error. Its normalized file SHA-256 is
`610a7eb024ac94f7576f583766a9fd3f9adf035292f5f1c40757ebba3fc455ea`.

Build the fixture-only encoder from the pinned libaom and libavif sources:

```sh
mkdir -p target/oracle-staging
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-i422-b64x128-source
git -C target/oracle-staging/libaom-i422-b64x128-source checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-i422-b64x128-source apply \
  "$PWD/scripts/avif_fixture_oracle/libaom-invalid-i422-vertical-b64x128.patch"
cmake -S target/oracle-staging/libaom-i422-b64x128-source \
  -B target/oracle-staging/libaom-i422-b64x128-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-i422-b64x128-prefix"
cmake --build target/oracle-staging/libaom-i422-b64x128-build -j 4
cmake --install target/oracle-staging/libaom-i422-b64x128-build

git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-i422-b64x128-source
git -C target/oracle-staging/libavif-i422-b64x128-source checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-i422-b64x128-source apply \
  "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
cmake -S target/oracle-staging/libavif-i422-b64x128-source \
  -B target/oracle-staging/libavif-i422-b64x128-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=OFF \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-i422-b64x128-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-i422-b64x128-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-i422-b64x128-prefix/include"
cmake --build target/oracle-staging/libavif-i422-b64x128-build --target avif_static -j 4

cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-i422-b64x128-source/include \
  -I target/oracle-staging/libavif-i422-b64x128-build/include \
  -I target/oracle-staging/libaom-i422-b64x128-prefix/include \
  scripts/avif_fixture_oracle/encode_lossless_inter_invalid_i422_b64x128.c \
  target/oracle-staging/libavif-i422-b64x128-build/libavif.a \
  target/oracle-staging/libaom-i422-b64x128-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-lossless-i422-b64x128
.oracle-venv/bin/python scripts/generate_avif_invalid_lossless_i422_b64x128.py \
  --encoder target/oracle-staging/encode-avif-lossless-i422-b64x128
```

The generator requires two identical normalized encodes and verifies the
profile-2 I422 key/inter headers, all-lossless flags, exact frame-0 RGB hash,
and the pinned Pillow frame-1 error. `make verify` runs its `--check` mode
without rebuilding the native fixture encoder.

The compound-prediction witness
`animated_lossless_compound_vertical_i420_b64x128.avif` uses the same pinned
libaom/libavif revisions and fixture-only vertical-partition patch. Reuse the
`libaom-b64x128-prefix` and `libavif-b64x128-build` outputs built above, then
compile its dedicated encoder and run the pinned generator:

```sh
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-b64x128-source/include \
  -I target/oracle-staging/libavif-b64x128-build/include \
  -I target/oracle-staging/libaom-b64x128-prefix/include \
  scripts/avif_fixture_oracle/encode_lossless_compound_i420_b64x128.c \
  target/oracle-staging/libavif-b64x128-build/libavif.a \
  target/oracle-staging/libaom-b64x128-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-lossless-compound-i420-b64x128
.oracle-venv/bin/python scripts/generate_avif_lossless_compound_tall_i420.py \
  --encoder target/oracle-staging/encode-avif-lossless-compound-i420-b64x128
```

The fixture contains three 128x128, 8-bit I420 frames. The final all-lossless
inter frame is the exact per-sample average of its two reference frames, and
the fixture-only libaom patch forces the 128x128 root into two vertical
B64x128 leaves. The generator normalizes only BMFF creation/modification
timestamps when checking two encodes, then validates the AV1 headers and exact
Pillow 12.2.0 pixels. Its `--check` mode is part of `make verify` and does not
need the native encoder. The AVIF SHA-256 is
`eb40e16aa47a612c272e29c2ad2cd62a7dc6115cce8cd40da1f1a01f95ec166a`.


## Lossy inter I420 mixed B8x16 transform topology

`animated_lossy_inter_420_b8x16_mixed_topology_64x64.avif` is a deterministic
two-frame 64x64 8-bit I420 sequence. A fixture-only libaom patch forces
vertical 16x16 inter partitions and a mixed transform tree inside each B8x16
leaf: one 8x8 child splits into TX4x4 blocks while the other remains TX8x8.
The sequence retains positive qindex and `TX_MODE_SELECT`; production Rust is
unchanged. Its exact Pillow RGB frames and native libavif repetition evidence
are retained with the parity row.

Build the fixture encoder from the pinned libaom and libavif revisions:

```sh
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-i420-small-topology-source
git -C target/oracle-staging/libaom-i420-small-topology-source checkout ad44980d7f3c7a2605c25d51ea96946949000841
git -C target/oracle-staging/libaom-i420-small-topology-source apply \
  "$PWD/scripts/avif_fixture_oracle/libaom-lossy-inter-i420-b8x16-mixed-topology.patch"
cmake -S target/oracle-staging/libaom-i420-small-topology-source \
  -B target/oracle-staging/libaom-i420-small-topology-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-i420-small-topology-prefix"
cmake --build target/oracle-staging/libaom-i420-small-topology-build -j 4
cmake --install target/oracle-staging/libaom-i420-small-topology-build

git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-i420-small-topology-source
git -C target/oracle-staging/libavif-i420-small-topology-source checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-i420-small-topology-source apply \
  "$PWD/scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch"
cmake -S target/oracle-staging/libavif-i420-small-topology-source \
  -B target/oracle-staging/libavif-i420-small-topology-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=OFF \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-i420-small-topology-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-i420-small-topology-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-i420-small-topology-prefix/include"
cmake --build target/oracle-staging/libavif-i420-small-topology-build --target avif_static -j 4
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-i420-small-topology-source/include \
  -I target/oracle-staging/libavif-i420-small-topology-build/include \
  -I target/oracle-staging/libaom-i420-small-topology-prefix/include \
  scripts/avif_fixture_oracle/encode_lossy_inter_i420_b8x16_mixed_topology.c \
  target/oracle-staging/libavif-i420-small-topology-build/libavif.a \
  target/oracle-staging/libaom-i420-small-topology-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-inter-i420-b8x16-mixed-topology
.oracle-venv/bin/python scripts/generate_avif_inter_i420_b8x16_mixed_topology.py \
  --encoder target/oracle-staging/encode-avif-inter-i420-b8x16-mixed-topology
```

The normalized fixture SHA-256 is
`07e1d146cf7a3c5f565fab5eddecd19ae53cca665a70faf6078e1b05baeebe12`.
Pillow 12.2.0 frame hashes are
`ab8e6ce0ad869b8fc5e75f48f4200167244d2358e269843964ceee22ed541255` and
`a6e8b64042b5adf6f2c9dc444c35e6d96c0151b59e91ae5b6eabcbeea8be8cbf`.

## Narrow inter chroma and transform-context regressions

`encode_narrow_inter.c` and `libaom-narrow-inter.patch` produce complete
64x16 and 16x64 two-frame AVIF sequences. The input-only patch forces 16x16
inter roots into four horizontal or vertical strips; libaom chooses the valid
prediction modes, motion vectors, transform trees and coefficients. No Rust
output or expected pixel data enters the producer. Qualities 45, 70 and 99
exercise different quantization and split choices. Fifteen 8-bit inputs cover
4:2:0 in both orientations, horizontal 4:2:2, and 4:4:4 in both orientations.
Ten quality-99 companions cover the same geometry at 10 and 12 bits. Their
native transform traces include deep horizontal and mixed vertical trees.
Four additional 10/12-bit vertical inputs use the `fine` content pattern to
reach fully split 4x16 inter transform trees in both 4:2:0 and 4:4:4.
Nine further inputs freeze probability CDFs on both frames across 8, 10 and
12 bits in 4:2:0, horizontal 4:2:2 and 4:4:4. Three selective-update inputs
retain adaptive key-frame CDFs and freeze the following inter-frame CDFs.
They exercise valid entropy state transitions through ordinary libaom controls;
the generator verifies both complete frame headers and exact pinned input hashes.
Vertical 4:2:2 probes are retained in local investigation evidence because the
pinned Pillow decoder rejects their second frame; they are not successful parity
cases. The native leaf observations establish 16x4 and 4x16 inter geometry.

The 4:2:0 cases regress chroma carriers shared by adjacent luma blocks with
different motion vectors. The horizontal quality-99 4:4:4 case also regresses a
mixed narrow transform tree: a split first child cannot lower the four-pixel
axis context for the next child. Public parity checks every frame's exact RGB
pixels, dimensions, mode and sequence metadata against live pinned Pillow.
Native loop observations are collected independently by
`scripts/generate_avif_loop_refs.py`.

Use clean pinned native source repositories at the paths below. Archive copies
keep fixture instrumentation separate without creating branches or worktrees:

```sh
mkdir -p target/oracle-staging/libaom-narrow-source
git -C target/oracle-staging/libaom archive ad44980d7f3c7a2605c25d51ea96946949000841 |
  tar -x -C target/oracle-staging/libaom-narrow-source
git apply --directory=target/oracle-staging/libaom-narrow-source \
  scripts/avif_fixture_oracle/libaom-narrow-inter.patch
cmake -S target/oracle-staging/libaom-narrow-source \
  -B target/oracle-staging/libaom-narrow-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0
cmake --build target/oracle-staging/libaom-narrow-build -j 4

mkdir -p target/oracle-staging/libavif-narrow-source
git -C target/oracle-staging/libavif-loop-clean-source \
  archive 6543b22b5bc706c53f038a16fe515f921556d9b3 |
  tar -x -C target/oracle-staging/libavif-narrow-source
git apply --directory=target/oracle-staging/libavif-narrow-source \
  scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch
cmake -S target/oracle-staging/libavif-narrow-source \
  -B target/oracle-staging/libavif-narrow-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=OFF \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-narrow-build/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-narrow-source"
cmake --build target/oracle-staging/libavif-narrow-build --target avif_static -j 4

mkdir -p target/oracle-staging/narrow-inter-tools
for orientation in horizontal vertical; do
  case "$orientation" in
    horizontal) fixture_width=64; fixture_height=16 ;;
    vertical) fixture_width=16; fixture_height=64 ;;
  esac
  for layout in 420 422 444; do
    if [ "$orientation:$layout" = vertical:422 ]; then continue; fi
    for quality in 45 70 99; do
      cc -std=c99 -O2 -Wall -Wextra -Werror \
        -DFIXTURE_WIDTH="$fixture_width" -DFIXTURE_HEIGHT="$fixture_height" \
        -DFIXTURE_FORMAT="AVIF_PIXEL_FORMAT_YUV$layout" -DFIXTURE_QUALITY="$quality" \
        -I target/oracle-staging/libavif-narrow-source/include \
        -I target/oracle-staging/libavif-narrow-build/include \
        -I target/oracle-staging/libaom-narrow-source \
        scripts/avif_fixture_oracle/encode_narrow_inter.c \
        target/oracle-staging/libavif-narrow-build/libavif.a \
        target/oracle-staging/libaom-narrow-build/libaom.a -lz -lm -lpthread \
        -o "target/oracle-staging/narrow-inter-tools/encode-$orientation-$layout-$quality"
    done
  done
done
target/oracle-pillow312-venv/bin/python scripts/generate_avif_narrow_inter_fixtures.py \
  --encoder-dir target/oracle-staging/narrow-inter-tools
target/oracle-pillow312-venv/bin/python scripts/generate_avif_narrow_inter_fixtures.py --check
```

The generator normalizes BMFF creation timestamps, requires two independent
encodes to agree, and pins every complete source digest. `--check` needs only
the committed inputs and pinned Pillow; `make verify-fixture-integrity` includes
that check. Pixel expectations remain in the ordinary oracle reference files.
Each producer accepts `OUTPUT.avif select BIT_DEPTH [coarse|fine|clipped] [CDF_MODE]`, where
`BIT_DEPTH` is 8, 10 or 12. The default `coarse` pattern preserves the original
inputs. The `fine` pattern uses per-pixel texture and a bounded second-frame
perturbation while retaining temporal correlation. The `clipped` pattern uses
the fine base and adds stronger second-frame detail below the halfway row of
the 16x16 clipped-height fixture, adding detail in its lower leaf. These are
source pixel controls; libaom chooses the residual transforms. The
generator selects the registered depth and pattern while reusing the same
orientation, layout and quality binary.

Omitting `CDF_MODE` preserves the original inputs. Mode `0` disables updates
on every frame, mode `1` requests adaptive updates, and mode `2` selects updates
per frame. The pinned libaom encoder enables updates on the key frame and
disables them on the shown inter frame for these mode-2 sequences. For example,
`encode-vertical-420-99 frozen.avif select 12 fine 0` produces a high-depth
frozen-CDF input; replacing the final `0` with `2` selects the transition case.
The generator's `cdf_mode` field supplies this input control, never expected
pixels or target decoder state. Exact pixels and sequence observations are
collected separately from live pinned Pillow and the native loop oracle.

## Horizontal B8x4 TX_MODE_SELECT inter sequence

`animated_lossy_inter_420_b8x4_mode_select_64x64.avif` is a two-frame, 64x64,
8-bit 4:2:0 lossy sequence. Its fixture-only patch
[`libaom-narrow-inter-b8x4.patch`](libaom-narrow-inter-b8x4.patch) forces the
second frame's B8x8 partition roots into horizontal B8x4 leaves. The normal
key-frame path is unchanged, and libaom still chooses the prediction modes,
coefficients and valid transform sizes. The source uses the coarse pattern,
quality 70, root size 8 and TX_MODE_SELECT. The generator checks normalized
complete-file identity across two native encodes and validates both frame
headers with the pinned AV1 inspector. Public expected pixels are collected
separately by pinned Pillow.

Archive the pinned AOM source into a separate staging directory, apply the
fixture-only patch, and build it with the same generic configuration as the
narrow inter profile above:

```sh
mkdir -p target/oracle-staging/libaom-narrow-b8x4-source
git -C target/oracle-staging/libaom archive ad44980d7f3c7a2605c25d51ea96946949000841 |
  tar -x -C target/oracle-staging/libaom-narrow-b8x4-source
git apply --directory=target/oracle-staging/libaom-narrow-b8x4-source \
  scripts/avif_fixture_oracle/libaom-narrow-inter-b8x4.patch
cmake -S target/oracle-staging/libaom-narrow-b8x4-source \
  -B target/oracle-staging/libaom-narrow-b8x4-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0
cmake --build target/oracle-staging/libaom-narrow-b8x4-build -j 4
mkdir -p target/oracle-staging/narrow-inter-b8x4-tools
cc -std=c99 -O2 -Wall -Wextra -Werror -DFIXTURE_ROOT_SIZE=8 \
  -DFIXTURE_WIDTH=64 -DFIXTURE_HEIGHT=64 \
  -DFIXTURE_FORMAT=AVIF_PIXEL_FORMAT_YUV420 -DFIXTURE_QUALITY=70 \
  -I target/oracle-staging/libavif-narrow-source/include \
  -I target/oracle-staging/libavif-narrow-build/include \
  -I target/oracle-staging/libaom-narrow-b8x4-source \
  scripts/avif_fixture_oracle/encode_narrow_inter.c \
  target/oracle-staging/libavif-narrow-build/libavif.a \
  target/oracle-staging/libaom-narrow-b8x4-build/libaom.a -lz -lm -lpthread \
  -o target/oracle-staging/narrow-inter-b8x4-tools/encode-square-420-70
target/oracle-pillow312-venv/bin/python scripts/generate_avif_narrow_inter_fixtures.py \
  --variant animated_lossy_inter_420_b8x4_mode_select_64x64 \
  --encoder-dir target/oracle-staging/narrow-inter-b8x4-tools
```

The libavif archive uses the fixture-control patch documented above for the
`enable-tx-size-search` encoder option. The independent loop-count observation
and exact Pillow frames are retained through the regular AVIF oracle workflows.

## Horizontal B8x4 4:4:4 TX_MODE_SELECT inter sequence

`animated_lossy_inter_444_b8x4_mode_select_64x64.avif` reuses the horizontal
B8x4 libaom patch and producer from the 4:2:0 case, with a full 4:4:4 plane
set. It is a two-frame, 64x64, 8-bit lossy sequence using the coarse pattern,
quality 70, root size 8 and TX_MODE_SELECT. The key-frame path remains
unchanged. The generator verifies repeat-encode stability and both frame
headers; Pillow RGB references and libavif loop evidence are collected
independently.

Compile the producer against the patched horizontal B8x4 libaom build and the
pinned libavif build, using the full-resolution 4:4:4 pixel format:

```sh
mkdir -p target/oracle-staging/narrow-inter-b8x4-tools
cc -std=c99 -O2 -Wall -Wextra -Werror -DFIXTURE_ROOT_SIZE=8 \
  -DFIXTURE_WIDTH=64 -DFIXTURE_HEIGHT=64 \
  -DFIXTURE_FORMAT=AVIF_PIXEL_FORMAT_YUV444 -DFIXTURE_QUALITY=70 \
  -I target/oracle-staging/libavif-narrow-source/include \
  -I target/oracle-staging/libavif-narrow-build/include \
  -I target/oracle-staging/libaom-narrow-b8x4-source \
  scripts/avif_fixture_oracle/encode_narrow_inter.c \
  target/oracle-staging/libavif-narrow-build/libavif.a \
  target/oracle-staging/libaom-narrow-b8x4-build/libaom.a -lz -lm -lpthread \
  -o target/oracle-staging/narrow-inter-b8x4-tools/encode-square-444-70
target/oracle-pillow312-venv/bin/python scripts/generate_avif_narrow_inter_fixtures.py \
  --variant animated_lossy_inter_444_b8x4_mode_select_64x64 \
  --encoder-dir target/oracle-staging/narrow-inter-b8x4-tools
```

## Vertical B4x8 TX_MODE_SELECT inter sequence

`animated_lossy_inter_420_b4x8_mode_select_64x64.avif` is a two-frame, 64x64,
8-bit 4:2:0 lossy sequence. Its fixture-only patch
[`libaom-narrow-inter-b4x8.patch`](libaom-narrow-inter-b4x8.patch) forces the
second frame's B8x8 partition roots into vertical B4x8 leaves. The key-frame
path and libaom's prediction, coefficient and valid transform-size choices
remain unchanged. The input uses the coarse pattern, quality 70, root size 8,
and TX_MODE_SELECT. `FIXTURE_NARROW_VERTICAL=1` orients the low-frequency
residual region with the vertical leaves. The generator checks normalized
complete-file identity across two native encodes and validates both frame
headers with the pinned AV1 inspector. Pillow frame references are collected
separately.

Archive the pinned AOM source, apply the fixture-only patch, and build it with
the generic narrow inter configuration described above. Link the producer to
the pinned patched libavif build:

```sh
mkdir -p target/oracle-staging/libaom-narrow-b4x8-source
git -C target/oracle-staging/libaom archive ad44980d7f3c7a2605c25d51ea96946949000841 |
  tar -x -C target/oracle-staging/libaom-narrow-b4x8-source
git apply --directory=target/oracle-staging/libaom-narrow-b4x8-source \
  scripts/avif_fixture_oracle/libaom-narrow-inter-b4x8.patch
cmake -S target/oracle-staging/libaom-narrow-b4x8-source \
  -B target/oracle-staging/libaom-narrow-b4x8-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0
cmake --build target/oracle-staging/libaom-narrow-b4x8-build -j 4
mkdir -p target/oracle-staging/narrow-inter-b4x8-tools
cc -std=c99 -O2 -Wall -Wextra -Werror -DFIXTURE_ROOT_SIZE=8 \
  -DFIXTURE_NARROW_VERTICAL=1 -DFIXTURE_WIDTH=64 -DFIXTURE_HEIGHT=64 \
  -DFIXTURE_FORMAT=AVIF_PIXEL_FORMAT_YUV420 -DFIXTURE_QUALITY=70 \
  -I target/oracle-staging/libavif-narrow-source/include \
  -I target/oracle-staging/libavif-narrow-build/include \
  -I target/oracle-staging/libaom-narrow-b4x8-source \
  scripts/avif_fixture_oracle/encode_narrow_inter.c \
  target/oracle-staging/libavif-narrow-build/libavif.a \
  target/oracle-staging/libaom-narrow-b4x8-build/libaom.a -lz -lm -lpthread \
  -o target/oracle-staging/narrow-inter-b4x8-tools/encode-square-420-70
.oracle-venv/bin/python scripts/generate_avif_narrow_inter_fixtures.py \
  --variant animated_lossy_inter_420_b4x8_mode_select_64x64 \
  --encoder-dir target/oracle-staging/narrow-inter-b4x8-tools
```

## Vertical B4x8 qindex-zero non-lossless I420 sequence

`animated_lossy_inter_420_b4x8_qindex0_mode_select_64x64.avif` uses the same
vertical B4x8 partition control with an 8-bit I420 inter frame at base qindex
zero. Its key frame remains all-lossless; the inter frame has zero luma DC
delta and chroma DC/AC deltas of two, so it exercises the non-lossless
qindex-zero path. The fixture-only
[`libaom-i420-qindex0-nonlossless-inter.patch`](libaom-i420-qindex0-nonlossless-inter.patch)
permits this standards-valid chroma-delta configuration in the pinned AOM
producer. `FIXTURE_ENABLE_CHROMA_DELTAQ=1` enables the corresponding libavif
encoder option. The generator checks both normalized complete-file identity
and the key/inter quantization headers.

Start from the pinned AOM and libavif sources used by the narrow-inter profile,
then apply both AOM patches before building. Keep this build separate from the
positive-qindex producer so its fixture-only controls cannot affect other
inputs:

```sh
mkdir -p target/oracle-staging/libaom-narrow-b4x8-qindex0-source
git -C target/oracle-staging/libaom archive ad44980d7f3c7a2605c25d51ea96946949000841 |
  tar -x -C target/oracle-staging/libaom-narrow-b4x8-qindex0-source
git apply --directory=target/oracle-staging/libaom-narrow-b4x8-qindex0-source \
  scripts/avif_fixture_oracle/libaom-narrow-inter-b4x8.patch
git apply --directory=target/oracle-staging/libaom-narrow-b4x8-qindex0-source \
  scripts/avif_fixture_oracle/libaom-i420-qindex0-nonlossless-inter.patch
cmake -S target/oracle-staging/libaom-narrow-b4x8-qindex0-source \
  -B target/oracle-staging/libaom-narrow-b4x8-qindex0-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0
cmake --build target/oracle-staging/libaom-narrow-b4x8-qindex0-build -j 4

mkdir -p target/oracle-staging/libavif-narrow-b4x8-qindex0-source
git -C target/oracle-staging/libavif-loop-clean-source \
  archive 6543b22b5bc706c53f038a16fe515f921556d9b3 |
  tar -x -C target/oracle-staging/libavif-narrow-b4x8-qindex0-source
git apply --directory=target/oracle-staging/libavif-narrow-b4x8-qindex0-source \
  scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch
cmake -S target/oracle-staging/libavif-narrow-b4x8-qindex0-source \
  -B target/oracle-staging/libavif-narrow-b4x8-qindex0-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF -DAVIF_BUILD_APPS=OFF \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-narrow-b4x8-qindex0-build/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-narrow-b4x8-qindex0-source"
cmake --build target/oracle-staging/libavif-narrow-b4x8-qindex0-build \
  --target avif_static -j 4

mkdir -p target/oracle-staging/narrow-inter-b4x8-qindex0-tools
cc -std=c99 -O2 -Wall -Wextra -Werror -DFIXTURE_ROOT_SIZE=8 \
  -DFIXTURE_NARROW_VERTICAL=1 -DFIXTURE_ENABLE_CHROMA_DELTAQ=1 \
  -DFIXTURE_WIDTH=64 -DFIXTURE_HEIGHT=64 \
  -DFIXTURE_FORMAT=AVIF_PIXEL_FORMAT_YUV420 -DFIXTURE_QUALITY=100 \
  -I target/oracle-staging/libavif-narrow-b4x8-qindex0-source/include \
  -I target/oracle-staging/libavif-narrow-b4x8-qindex0-build/include \
  -I target/oracle-staging/libaom-narrow-b4x8-qindex0-source \
  scripts/avif_fixture_oracle/encode_narrow_inter.c \
  target/oracle-staging/libavif-narrow-b4x8-qindex0-build/libavif.a \
  target/oracle-staging/libaom-narrow-b4x8-qindex0-build/libaom.a \
  -lz -lm -lpthread \
  -o target/oracle-staging/narrow-inter-b4x8-qindex0-tools/encode-square-420-100
.oracle-venv/bin/python scripts/generate_avif_narrow_inter_fixtures.py \
  --variant animated_lossy_inter_420_b4x8_qindex0_mode_select_64x64 \
  --encoder-dir target/oracle-staging/narrow-inter-b4x8-qindex0-tools
.oracle-venv/bin/python scripts/generate_avif_narrow_inter_fixtures.py \
  --variant animated_lossy_inter_420_b4x8_qindex0_mode_select_64x64 --check
```

## Odd-width 10-bit lossless B4x8 inter edge

`animated_lossless_inter_420_b4x8_10bit_odd_65x64.avif` is a two-frame,
full-range 10-bit I420 sequence. The pinned AOM producer forces vertical
B4x8 leaves; its all-lossless frames use `ONLY_4X4`, and the final right-edge
leaf has one visible luma column. Pillow 12.2.0 decodes both frames. The Rust
sequence decoder now admits this bounded 10-bit I420 profile, reconstructs
the TX4x4 lossless terminals that overlap the clipped B4x8 edge, and clips
publication to the frame's visible width. The manifest compares both frames
against their exact Pillow references. This fixture also covers the
sub-transform-width luma edge while chroma keeps its complete 4x4 transform
footprint.

The selected parity command is:

```sh
make coverage-selected \
  COVERAGE_CASES='decode:avif:animated_lossless_inter_420_b4x8_10bit_odd_65x64'
```

It passed for the 12,480-byte still-image reference and both sequence-frame
references. The selected LLVM report is retained as local evidence; it does
not establish whole-suite coverage or source-bound Coverage MCP attribution.

Reuse the pinned narrow-B4x8 AOM and libavif build from the preceding section.
Build a separate producer with odd dimensions and q100, then verify the
complete-file digest and AV1 headers:

```sh
mkdir -p target/oracle-staging/narrow-inter-b4x8-odd-edge-tools
cc -std=c99 -O2 -Wall -Wextra -Werror -DFIXTURE_ROOT_SIZE=8 \
  -DFIXTURE_NARROW_VERTICAL=1 -DFIXTURE_WIDTH=65 -DFIXTURE_HEIGHT=64 \
  -DFIXTURE_FORMAT=AVIF_PIXEL_FORMAT_YUV420 -DFIXTURE_QUALITY=100 \
  -I target/oracle-staging/libavif-narrow-source/include \
  -I target/oracle-staging/libavif-narrow-build/include \
  -I target/oracle-staging/libaom-narrow-b4x8-source \
  scripts/avif_fixture_oracle/encode_narrow_inter.c \
  target/oracle-staging/libavif-narrow-build/libavif.a \
  target/oracle-staging/libaom-narrow-b4x8-build/libaom.a -lz -lm -lpthread \
  -o target/oracle-staging/narrow-inter-b4x8-odd-edge-tools/encode-odd65x64-420-100
.oracle-venv/bin/python scripts/generate_avif_narrow_inter_fixtures.py \
  --variant animated_lossless_inter_420_b4x8_10bit_odd_65x64 \
  --encoder-dir target/oracle-staging/narrow-inter-b4x8-odd-edge-tools
.oracle-venv/bin/python scripts/generate_avif_narrow_inter_fixtures.py \
  --variant animated_lossless_inter_420_b4x8_10bit_odd_65x64 --check
```

## Odd-width 10-bit lossless B4x8 inter edge after a full block

`animated_lossless_inter_420_b4x8_10bit_odd_69x64.avif` is a two-frame,
full-range 10-bit I420 sequence. The pinned AOM producer forces vertical
B4x8 leaves. A complete leaf starts at x=64, then the final leaf at x=68 has
one visible luma column. Pinned Pillow and libavif decode both frames, and the
Rust public sequence decoder matches both frames byte-for-byte. The clipped
TX4x4 reconstruction handles this block-aligned edge without adding work to
the transform loops.

Build the matching producer and generate the deterministic input:

```sh
mkdir -p target/oracle-staging/narrow-inter-b4x8-odd-edge-tools
cc -std=c99 -O2 -Wall -Wextra -Werror -DFIXTURE_ROOT_SIZE=8 \
  -DFIXTURE_NARROW_VERTICAL=1 -DFIXTURE_WIDTH=69 -DFIXTURE_HEIGHT=64 \
  -DFIXTURE_FORMAT=AVIF_PIXEL_FORMAT_YUV420 -DFIXTURE_QUALITY=100 \
  -I target/oracle-staging/libavif-narrow-source/include \
  -I target/oracle-staging/libavif-narrow-build/include \
  -I target/oracle-staging/libaom-narrow-b4x8-source \
  scripts/avif_fixture_oracle/encode_narrow_inter.c \
  target/oracle-staging/libavif-narrow-build/libavif.a \
  target/oracle-staging/libaom-narrow-b4x8-build/libaom.a -lz -lm -lpthread \
  -o target/oracle-staging/narrow-inter-b4x8-odd-edge-tools/encode-odd69x64-420-100
.oracle-venv/bin/python scripts/generate_avif_narrow_inter_fixtures.py \
  --variant animated_lossless_inter_420_b4x8_10bit_odd_69x64 \
  --encoder-dir target/oracle-staging/narrow-inter-b4x8-odd-edge-tools
.oracle-venv/bin/python scripts/generate_avif_narrow_inter_fixtures.py \
  --variant animated_lossless_inter_420_b4x8_10bit_odd_69x64 --check
```

The public parity case is:

```sh
make coverage-selected \
  COVERAGE_CASES='decode:avif:animated_lossless_inter_420_b4x8_10bit_odd_69x64'
```

## Vertical B4x8 4:4:4 TX_MODE_SELECT inter sequence

`animated_lossy_inter_444_b4x8_mode_select_64x64.avif` reuses the vertical
B4x8 partition patch and producer from the 4:2:0 case above, with a full
4:4:4 input plane set. It is a two-frame, 64x64, 8-bit lossy sequence using
the coarse pattern, quality 70, and TX_MODE_SELECT. The generator checks
complete-file identity across two native encodes and validates both frame
headers with the pinned AV1 inspector; Pillow references are collected
independently.

Build the producer with `FIXTURE_FORMAT=AVIF_PIXEL_FORMAT_YUV444`,
`FIXTURE_ROOT_SIZE=8`, and `FIXTURE_NARROW_VERTICAL=1`, then run:

```sh
.oracle-venv/bin/python scripts/generate_avif_narrow_inter_fixtures.py \
  --variant animated_lossy_inter_444_b4x8_mode_select_64x64 \
  --encoder-dir target/oracle-staging/narrow-inter-b4x8-tools
```

## Narrow 8x32 and 32x8 inter transform trees

The same producer accepts `-DFIXTURE_ROOT_SIZE=32` at compile time. Its default
remains 16, preserving all earlier inputs. With
`libaom-wide8-inter.patch`, 32x32 roots form four 8x32 or 32x8 inter leaves in
a 32x64 or 64x32 sequence. Forty-two inputs cover monochrome, 4:2:0, horizontal
4:2:2 and 4:4:4 at 8/10/12 bits and qualities 70/99. Fine residual detail lets
libaom choose shallow, mixed and fully split child transforms. Independent
scalar dav1d traces witness those choices; public parity checks exact live
Pillow RGB frames and sequence metadata.

The pinned native chroma-size table maps an 8x32 vertical leaf in 4:2:2 to
`BLOCK_INVALID`. The fixture patch preserves that table's validity checks, and
the producer rejects vertical 4:2:2 requests before encoding. Six exploratory
native producer failures are retained as investigation evidence; they produced
no public input and are not Pillow parity failures or fault-contract cases.
The same validity restriction applies to the earlier 4x16 vertical leaves.

Build a separate native source archive with the wide-leaf patch, keeping the
16x16-root library for the earlier fixtures. These are ordinary source/build
directories under `target/`, not Git worktrees:

```sh
mkdir -p target/oracle-staging/libaom-wide8-source
git -C target/oracle-staging/libaom archive ad44980d7f3c7a2605c25d51ea96946949000841 |
  tar -x -C target/oracle-staging/libaom-wide8-source
git apply --directory=target/oracle-staging/libaom-wide8-source \
  scripts/avif_fixture_oracle/libaom-wide8-inter.patch
cmake -S target/oracle-staging/libaom-wide8-source \
  -B target/oracle-staging/libaom-wide8-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0
cmake --build target/oracle-staging/libaom-wide8-build -j 4

for orientation in horizontal vertical; do
  case "$orientation" in
    horizontal) fixture_width=64; fixture_height=32 ;;
    vertical) fixture_width=32; fixture_height=64 ;;
  esac
  for layout in 400 420 422 444; do
    if [ "$orientation:$layout" = vertical:422 ]; then continue; fi
    for quality in 70 99; do
      cc -std=c99 -O2 -Wall -Wextra -Werror -DFIXTURE_ROOT_SIZE=32 \
        -DFIXTURE_WIDTH="$fixture_width" -DFIXTURE_HEIGHT="$fixture_height" \
        -DFIXTURE_FORMAT="AVIF_PIXEL_FORMAT_YUV$layout" -DFIXTURE_QUALITY="$quality" \
        -I target/oracle-staging/libavif-narrow-source/include \
        -I target/oracle-staging/libavif-narrow-build/include \
        -I target/oracle-staging/libaom-wide8-source \
        scripts/avif_fixture_oracle/encode_narrow_inter.c \
        target/oracle-staging/libavif-narrow-build/libavif.a \
        target/oracle-staging/libaom-wide8-build/libaom.a -lz -lm -lpthread \
        -o "target/oracle-staging/narrow-inter-tools/encode-wide8-$orientation-$layout-$quality"
    done
  done
done
target/oracle-pillow312-venv/bin/python scripts/generate_avif_narrow_inter_fixtures.py \
  --encoder-dir target/oracle-staging/narrow-inter-tools
target/oracle-pillow312-venv/bin/python scripts/generate_avif_narrow_inter_fixtures.py --check
```

These cases bring the shared generator to 83 inputs. Its optional `--variant` selector
can regenerate one case; every generated input requires two independent encodes
with equal normalized bytes. Header validation includes monochrome signaling,
bit depth, chroma layout and CDF-control metadata where specified. Expected
pixels remain independently collected Pillow observations.

## Narrow 16x64 and 64x16 inter transform trees

`libaom-wide16-inter.patch` forces valid 64x64 inter roots into four 16x64 or
64x16 leaves. Compile the same producer with `-DFIXTURE_ROOT_SIZE=64` and use
128x64 horizontal or 64x128 vertical dimensions. Forty-two additional inputs
cover monochrome, 4:2:0, horizontal 4:2:2 and 4:4:4 at 8/10/12 bits and qualities
70/99, with the `fine` content pattern. The default root size remains 16.

Repeat the preceding wide8 archive and CMake steps with `wide16` paths and
`libaom-wide16-inter.patch`. In the producer loop, replace the root-size macro
with 64 and use the dimensions above; name its outputs
`encode-wide16-$orientation-$layout-$quality` in the shared encoder directory.
Keep the existing narrow and wide8 tools there so the generator can reproduce
all 125 maintained inputs. The generator requires two byte-identical normalized
encodes for every input; the previous 83 source digests remain unchanged.

The pinned native chroma-size table also rejects vertical 16x64 leaves in
4:2:2. Both the partition patch and producer preserve that restriction; the
producer returns an error before allocating or writing output. Such requests
are producer diagnostics, not public parity or fault-contract cases.

Independent scalar dav1d traces witness shallow, mixed and fully split trees.
Public parity compares both complete frames and sequence metadata with live
pinned Pillow. These inputs exposed two Rust reconstruction defects: thin
64-pixel subsampled chroma omitted the existing inherited-transform mapping,
and mixed 4:4:4 trees loaded only the first terminal's neighbor contexts.
Both are covered by the ordinary manifest rows, without private unit tests or
target-specific expected pixels in the producer.

## Adjacent square-64 chroma neighbor regressions

`scripts/generate_avif_square64_neighbor_fixtures.py` maintains eighteen
five-frame 128x128 sequences in 4:2:2 and 4:4:4, at 8/10/12 bits and qualities
70/95/99. A gray key frame precedes colored checker periods of 2, 4, 8 and
16 pixels. Ordinary encoder controls hold partition roots at 64x64 while
libaom chooses transform sizes and references. No native partition patch is
needed. The 10/12-bit planar sources include nonzero low sample bits.

Pinned Pillow produces the 8-bit inputs. The pinned libavif 1.4.1/libaom 3.13.2
`avifenc` consumes the independent high-depth Y4M sources:

```sh
target/oracle-pillow312-venv/bin/python scripts/generate_avif_square64_neighbor_fixtures.py \
  --encoder target/oracle-staging/libavif-superres-build/avifenc
target/oracle-pillow312-venv/bin/python scripts/generate_avif_square64_neighbor_fixtures.py --check
```

Regeneration requires two normalized encodes to agree with each pinned input
digest. `--output-dir` keeps investigation copies outside the maintained
fixture directory; `--variant` selects one source recipe. `--check` reads the
retained files and verifies AV1 headers and live Pillow sequence metadata,
without encoding or writing fixtures. The standard fixture-integrity target
includes this check.

These cases regress propagation of the complete chroma neighbor grid between
adjacent blocks when luma transforms split deeply. All eighteen fail on the
preceding Rust decoder and match exact live Pillow frames after the fix.
Native transform/loop observations and expected pixels are collected
independently by the existing reference workflows; this producer reads no
target output or expected pixels.

## Mixed lossless/lossy super-resolution inter sequences

`scripts/generate_avif_mixed_superres_fixtures.py` produces thirty-four
two-frame inputs. The original twenty-four cover monochrome, I420, I422 and
I444 at 8/10/12 bits with neutral filters, active luma SGR restoration at
8/10 bits, and the existing monochrome/I420 film-grain combinations. Eight
additional active-grain inputs cover 400 10/12-bit, 420 12-bit, and 422
8/10/12-bit at 80x48, plus 444 10/12-bit at 128x64. The I444 grain inputs use
super-resolution denominator 16 and coded width 64 to stay within the
decoder's bounded I444 grain geometry. Most inputs use an ordinary lossy key
followed by an inter frame coded at width 64 and displayed at width 80.
The added 8-bit I420 loop-filter case enables nonzero Y, U and V deblocking on
both frames while retaining mixed-segment horizontal super-resolution.
Segment 1 has qindex zero; the other segments retain qindex 140. Independent
planar samples retain nonzero low bits at high depth. The input generator reads
neither Rust outputs nor reference pixels; standard reference collectors own
the exact Pillow frames and native sequence observations.

The native fixture patches forward super-resolution configuration and a
bounded ROI map, permit its use in the ordinary RD encoder path, retain an
explicit non-temporal map, and allow qindex zero. They apply the ROI to inter
frames only. All arithmetic and AV1 encoding still run in pinned libaom.
Pinned libavif disables 12-bit encoder restoration; the generator preserves
that control and omits the four byte-identical restoration requests. It does
not claim 12-bit restoration inputs. All thirty-four inputs reproduce twice
and independently parsed headers confirm their actual segment, scaling,
restoration, film-grain, CDEF and loop-filter features before they are written.

Using the already available clean pinned sources, build without installation:

```sh
avif_case_dir="$PWD/target/oracle-staging/mixed-superres-producer"
mkdir -p "$avif_case_dir/aom-source" "$avif_case_dir/avif-source"
git -C target/oracle-staging/libaom archive ad44980d7f3c7a2605c25d51ea96946949000841 | tar -x -C "$avif_case_dir/aom-source"
git -C target/oracle-staging/libavif-loop-clean-source archive 6543b22b5bc706c53f038a16fe515f921556d9b3 | tar -x -C "$avif_case_dir/avif-source"
patch -d "$avif_case_dir/aom-source" -p1 < scripts/avif_fixture_oracle/libaom-mixed-superres.patch
patch -d "$avif_case_dir/avif-source" -p1 < scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch
patch -d "$avif_case_dir/avif-source" -p1 < scripts/avif_fixture_oracle/libavif-mixed-superres.patch
cmake -S "$avif_case_dir/aom-source" -B "$avif_case_dir/aom-build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic -DENABLE_TESTS=OFF \
  -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0
cmake --build "$avif_case_dir/aom-build" -j 4
cmake -S "$avif_case_dir/avif-source" -B "$avif_case_dir/avif-build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DAVIF_CODEC_AOM=SYSTEM \
  -DAVIF_CODEC_AOM_DECODE=OFF -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF \
  -DAVIF_BUILD_APPS=OFF -DAOM_LIBRARY="$avif_case_dir/aom-build/libaom.a" \
  -DAOM_INCLUDE_DIR="$avif_case_dir/aom-source"
cmake --build "$avif_case_dir/avif-build" --target avif_static -j 4
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I "$avif_case_dir/avif-source/include" -I "$avif_case_dir/avif-build/include" \
  scripts/avif_fixture_oracle/encode_mixed_superres.c \
  "$avif_case_dir/avif-build/libavif.a" "$avif_case_dir/aom-build/libaom.a" \
  -lz -lm -lpthread -o "$avif_case_dir/produce"
target/oracle-pillow312-venv/bin/python scripts/generate_avif_mixed_superres_fixtures.py \
  --encoder "$avif_case_dir/produce"
target/oracle-pillow312-venv/bin/python scripts/generate_avif_mixed_superres_fixtures.py --check
```

Only the input producer uses these patches. No runtime dependency, native
fallback, unsafe Rust, fault hook or private unit test is added. The ordinary
fixture-verification target checks the thirty-four complete input digests and AV1
features. The investigation also retains public inputs that originally exposed
unsupported monochrome/mixed-key and ordinary nonsegmented super-resolution
paths. Subsequent sections describe their separate parity cases; they do not
change these twenty-four inputs.

## Ordinary monochrome super-resolution inter sequences

The same producer accepts an optional final `ordinary` argument. This skips
only the fixture ROI option and emits ordinary lossy key/inter samples with
no segmentation. Its default mixed-segment invocation and all twenty-four
pinned mixed input digests remain unchanged. Build it with the pinned recipe above,
then run:

```sh
target/oracle-pillow312-venv/bin/python scripts/generate_avif_monochrome_superres_fixtures.py \
  --encoder "$avif_case_dir/produce"
target/oracle-pillow312-venv/bin/python scripts/generate_avif_monochrome_superres_fixtures.py --check
```

This generalized input-only generator retains the three neutral 8/10/12-bit
recipes unchanged and adds ordinary 8/10-bit sequences with active luma SGR
restoration. All five use inter width 64 displayed at 80x48, with ordinary
lossy key frames. Each complete file reproduces twice and must match its
pinned digest and independently parsed AV1 features before writing. The two
SGR inputs are the complete public workloads that exposed the restoration
performance follow-up; they now also participate in the normal parity matrix.
The earlier `generate_avif_monochrome_neutral_superres_fixtures.py` is renamed
to reflect both profiles. Existing case IDs and input bytes do not change.

The input generator owns no expected pixels or target results. Live
Pillow/native collectors and the standard public matrix own observations.

## Ordinary I422 neutral super-resolution inter sequences

The same pinned producer and final `ordinary` argument also generate 8-bit
4:2:2 key/inter sequences with restoration, deblocking, CDEF and segmentation
inactive. Build the producer with the pinned recipe above, then run:

```sh
target/oracle-pillow312-venv/bin/python scripts/generate_avif_i422_neutral_superres_fixtures.py \
  --encoder "$avif_case_dir/produce"
target/oracle-pillow312-venv/bin/python scripts/generate_avif_i422_neutral_superres_fixtures.py --check
```

The four input-only recipes cover display extents 40x16, 80x48 and 128x64,
super-resolution denominators 9, 10 and 16, and an odd coded width of 71.
The 64-pixel-high case exercises geometry beyond the separate active-restoration
profile's height limit. All four complete files reproduce twice and are checked
against pinned digests and independently parsed AV1 features. These inputs own
no expected pixels or Rust results; the standard live Pillow/native collectors
supply frame and sequence observations for public manifest parity.

## Monochrome sequences with mixed lossless/lossy key segments

`generate_avif_monochrome_mixed_key_fixtures.py` retains the five input-only 80x48
sequences with a mixed-segment key frame followed by a 64x48 inter frame
upscaled to 80x48. Four recipes vary the key pixels, including 8-, 10- and
12-bit input and two native speed settings. The flat 8-bit recipe covers the
same key admission failure as the retained investigation input; it is a new
complete file, not a byte reproduction of that earlier input.

Use a separate input producer: apply `libaom-mixed-key-segmentation.patch`
after `libaom-mixed-superres.patch` to enable the existing fixture ROI controls
on key frames as well as inter frames. This profile changes the AOM library
used to generate inputs. Keep the preceding producer for its twenty mixed,
five ordinary monochrome and four ordinary I422 recipes. The updated C
program reproduced all twenty-nine preceding recipes twice against their
unchanged complete digests when linked to that preceding library.

The following recipe exports the already available pinned sources into a new
build directory without creating a Git checkout or installing dependencies:

```sh
avif_key_dir="$PWD/target/oracle-staging/mixed-key-superres-producer"
mkdir -p "$avif_key_dir/aom-source" "$avif_key_dir/avif-source"
git -C target/oracle-staging/libaom archive ad44980d7f3c7a2605c25d51ea96946949000841 | tar -x -C "$avif_key_dir/aom-source"
git -C target/oracle-staging/libavif-loop-clean-source archive 6543b22b5bc706c53f038a16fe515f921556d9b3 | tar -x -C "$avif_key_dir/avif-source"
patch -d "$avif_key_dir/aom-source" -p1 < scripts/avif_fixture_oracle/libaom-mixed-superres.patch
patch -d "$avif_key_dir/aom-source" -p1 < scripts/avif_fixture_oracle/libaom-mixed-key-segmentation.patch
patch -d "$avif_key_dir/avif-source" -p1 < scripts/avif_fixture_oracle/libavif-av1-fixture-controls.patch
patch -d "$avif_key_dir/avif-source" -p1 < scripts/avif_fixture_oracle/libavif-mixed-superres.patch
cmake -S "$avif_key_dir/aom-source" -B "$avif_key_dir/aom-build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DENABLE_TESTS=OFF \
  -DENABLE_EXAMPLES=OFF -DENABLE_TOOLS=OFF -DENABLE_DOCS=OFF -DCONFIG_AV1_DECODER=1
cmake --build "$avif_key_dir/aom-build" --target aom -j 4
cmake -S "$avif_key_dir/avif-source" -B "$avif_key_dir/avif-build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DAVIF_BUILD_APPS=OFF \
  -DAVIF_BUILD_TESTS=OFF -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_DAV1D=OFF \
  -DAVIF_CODEC_LIBGAV1=OFF -DAVIF_LIBYUV=OFF \
  -DAOM_LIBRARY="$avif_key_dir/aom-build/libaom.a" -DAOM_INCLUDE_DIR="$avif_key_dir/aom-source"
cmake --build "$avif_key_dir/avif-build" --target avif_static -j 4
cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I "$avif_key_dir/avif-source/include" -I "$avif_key_dir/avif-build/include" \
  scripts/avif_fixture_oracle/encode_mixed_superres.c \
  "$avif_key_dir/avif-build/libavif.a" "$avif_key_dir/aom-build/libaom.a" \
  -lz -lm -lpthread -o "$avif_key_dir/produce"
target/oracle-pillow312-venv/bin/python scripts/generate_avif_monochrome_mixed_key_fixtures.py \
  --encoder "$avif_key_dir/produce"
target/oracle-pillow312-venv/bin/python scripts/generate_avif_monochrome_mixed_key_fixtures.py --check
```

This profile keeps the AOM decoder enabled because static libavif also refers
to its decoder entry point. Its producer version is
`libavif 1.4.1 (aom [enc/dec]:v3.13.2)`. The optional final `mixed-key`
argument varies the key-frame pixels; omitting it retains the flat-key input
pattern. The generator owns no expected pixels or Rust outcomes. It encodes
each recipe twice, normalizes only sequence timestamps, and validates complete
input digests and independently parsed AV1 features before writing. All twenty-six
actual sequence headers disable restoration and all frame restoration types
are NONE, including the fast flat recipe that requests restoration from AOM.
The public parity matrix and live Pillow/native observers own the expected
frame pixels and timing. The Rust decoder uses no native fallback.

The same generator now includes nine boundary recipes: varied 8/10/12-bit
key/inter sequences at 40x16, 60x20 and 128x128. Their inter coded widths are
32, 48 and 64 respectively. The first two use denominator 10; the largest uses
16. The 60x20 inputs exercise four-pixel key-frame and bottom-edge tails;
the 128x128 key covers the admitted size bound. Named input recipes preserve
the original five IDs, parameters and complete file bytes. All twenty-six
recipes reproduce twice using the unchanged producer and must pass the same
complete-digest and independent-header checks.

Twelve additional varied-key recipes exercise inter coded widths 71, 100 and
102 at 8/10/12 bits. Display sizes are 80x20 (denominator 9), 112x20
(denominator 9), and 128x20/128x128 (denominator 10). The logical resize width
sets the phase, while native taps read the complete reconstructed coded tail,
rounded up to eight pixels. The 20-row cases also check a padded bottom edge.
The public Rust candidate matches live pinned Pillow for still output and both
frames in every sequence. The standard reference collectors retain pixels;
the producer supplies inputs only.

The earlier width-admission-only experiment remains rejected evidence in the
boundary checkpoint. The follow-up separates physical reconstruction from
logical resize geometry and removes an unnecessary 8-bit monochrome exclusion
from the checked segment-lossless WHT grid. Neither change adds a native Rust
runtime fallback or establishes every legal mixed-segment sequence profile.

## Color mixed-segment key and coded-tail sequences

`generate_avif_color_mixed_key_fixtures.py` uses the same separately patched
mixed-key producer described above. Its 72 input-only recipes cover I420, I422
and I444 at 8/10/12 bits, with flat and varied key pixels. Both frames carry
mixed lossless/lossy segment maps. Complete file digests bind these recipes;
Pillow and the independent native observer supply expected frames and timing.

The dimensions are 128x48 with denominator 10 (inter width 102), 80x20 with
denominator 9 (width 71), and 112x20 with denominator 9 (width 100). These
exercise non-four-aligned logical widths, rounded physical coded tails,
subsampled chroma and 20-row bottom edges. Varied keys require nonconstant
residual reconstruction; flat keys retain the same segmentation syntax.
The producer version remains `libavif 1.4.1 (aom [enc/dec]:v3.13.2)`.

Nine recipes pass the optional `motion-key` argument. Their 128x48 second
source frame samples the pattern four luma pixels to the right and two rows
down, with the corresponding chroma offsets for each layout. This supplies
distinct temporal source pixels at coded width 102; the encoder chooses the
motion vectors and residuals. The original 54 recipes retain their complete
bytes. No recipe stores expected decoded pixels.

Nine further recipes pass `integer-key`, which enables libaom's public
`tune-content=screen` option and disables palette coding. Equal varied source
frames let its native content detector select integer-only motion on the inter
frame; IntraBC remains disabled. The generator independently requires both
`allow_screen_content_tools` and `force_integer_mv` in the parsed frame headers.
These I420/I422/I444 8/10/12-bit inputs retain mixed segments and super-resolution
from coded width 102. All preceding 63 recipes reproduce their complete bytes.

```sh
target/oracle-pillow312-venv/bin/python scripts/generate_avif_color_mixed_key_fixtures.py \
  --encoder "$avif_key_dir/produce"
target/oracle-pillow312-venv/bin/python scripts/generate_avif_color_mixed_key_fixtures.py --check
```

Every recipe is encoded twice with only timestamp normalization; both complete
files must match the pinned digest and independently parsed header features.
The existing twenty inter-only recipes and twenty-six monochrome mixed-key
recipes retain their parameters and complete bytes. The Makefile's ordinary
fixture verification includes the color recipe check. This producer changes
no Rust runtime dependency, native fallback, fault hook or unit-test policy.

## AVIF still with active loop-filter sharpness

`encode_avif_loopfilter_sharpness.c` produces the public 128x128 8-bit 4:2:0
still fixture with AOM loop-filter sharpness 7 and nonzero Y/U/V levels. Build
the pinned libavif 1.4.1/AOM 3.13.2 libraries using the source and patch steps
in “Mixed-segment super-resolution fixtures”, then compile and reproduce it:

```sh
sharpness_dir="$PWD/target/oracle-staging/loopfilter-sharpness-producer"
cc -std=c99 -O2 -Wall -Wextra -Werror   -I "$sharpness_dir/avif-source/include"   -I "$sharpness_dir/avif-build/include"   scripts/avif_fixture_oracle/encode_avif_loopfilter_sharpness.c   "$sharpness_dir/avif-build/libavif.a"   "$sharpness_dir/aom-build/libaom.a"   -lz -lm -lpthread -o "$sharpness_dir/produce"
.oracle-venv/bin/python scripts/generate_avif_loopfilter_sharpness.py   --encoder "$sharpness_dir/produce"
.oracle-venv/bin/python scripts/generate_avif_loopfilter_sharpness.py --check
```

The checker verifies repeatable bytes, the encoded frame's sharpness and
deblocking levels, and Pillow's 128x128 RGB decode.

## Valid nonuniform AV1 tile columns

`portable_lossless_monochrome_nonuniform_tiles_256x256.avif` is an 8-bit,
full-range, all-lossless monochrome still. Its frame has four 64x64
superblocks in each direction, with nonuniform column widths `[1, 3]` and one
full-height row. This valid bitstream executes the completed-column path in
the Rust AV1 tile parser. A pinned Pillow decode and the independent AV1 OBU
parser check the exact pixels and tile starts.

Libaom 3.13.2 exposes explicit tile widths, but libavif 1.4.1 does not map
those controls. The fixture-only patch
[`libavif-av1-nonuniform-tile-fixture.patch`](libavif-av1-nonuniform-tile-fixture.patch)
sets them only for this 256x256 monochrome producer profile. It does not
change the Rust decoder or a runtime dependency. Build the pinned libaom and
libavif sources in fresh staging directories; the base revisions are
`ad44980d7f3c7a2605c25d51ea96946949000841` and
`6543b22b5bc706c53f038a16fe515f921556d9b3`, respectively:

```sh
mkdir -p target/oracle-staging
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom-nonuniform-source
git -C target/oracle-staging/libaom-nonuniform-source checkout ad44980d7f3c7a2605c25d51ea96946949000841
cmake -S target/oracle-staging/libaom-nonuniform-source \
  -B target/oracle-staging/libaom-nonuniform-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DAOM_TARGET_CPU=generic \
  -DENABLE_TESTS=OFF -DENABLE_DOCS=OFF -DENABLE_TOOLS=OFF -DENABLE_EXAMPLES=OFF \
  -DCONFIG_AV1_DECODER=0 -DCONFIG_MULTITHREAD=0 \
  -DCMAKE_INSTALL_PREFIX="$PWD/target/oracle-staging/libaom-nonuniform-prefix"
cmake --build target/oracle-staging/libaom-nonuniform-build -j 4
cmake --install target/oracle-staging/libaom-nonuniform-build

git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif-nonuniform-source
git -C target/oracle-staging/libavif-nonuniform-source checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
git -C target/oracle-staging/libavif-nonuniform-source apply \
  "$PWD/scripts/avif_fixture_oracle/libavif-av1-nonuniform-tile-fixture.patch"
cmake -S target/oracle-staging/libavif-nonuniform-source \
  -B target/oracle-staging/libavif-nonuniform-build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=OFF \
  -DAVIF_CODEC_AOM_ENCODE=ON -DAVIF_CODEC_DAV1D=OFF -DAVIF_LIBYUV=OFF \
  -DAVIF_BUILD_APPS=OFF -DAVIF_BUILD_TESTS=OFF -DAVIF_BUILD_EXAMPLES=OFF \
  -DCMAKE_PREFIX_PATH="$PWD/target/oracle-staging/libaom-nonuniform-prefix" \
  -DAOM_LIBRARY="$PWD/target/oracle-staging/libaom-nonuniform-prefix/lib/libaom.a" \
  -DAOM_INCLUDE_DIR="$PWD/target/oracle-staging/libaom-nonuniform-prefix/include"
cmake --build target/oracle-staging/libavif-nonuniform-build --target avif_static -j 4

cc -std=c99 -O2 -Wall -Wextra -Werror \
  -I target/oracle-staging/libavif-nonuniform-source/include \
  -I target/oracle-staging/libavif-nonuniform-build/include \
  -I target/oracle-staging/libaom-nonuniform-prefix/include \
  scripts/avif_fixture_oracle/encode_lossless_monochrome_nonuniform_tiles.c \
  target/oracle-staging/libavif-nonuniform-build/libavif.a \
  target/oracle-staging/libaom-nonuniform-prefix/lib/libaom.a \
  -lz -lm -lpthread -o target/oracle-staging/encode-avif-nonuniform-tiles
.oracle-venv/bin/python scripts/generate_avif_nonuniform_tile_fixture.py \
  --encoder target/oracle-staging/encode-avif-nonuniform-tiles
.oracle-venv/bin/python scripts/generate_avif_nonuniform_tile_fixture.py --check
```

The checked fixture has SHA-256
`fa46345a828f46277562a5324c18066d6129b73b223e36a4214d492d5d6c344f`; its
Pillow RGB output has SHA-256
`b5cc944c73c13c1b613895869a0219be3449dfbe99be281984884be08cfc8293`. To run
only its public parity row and retain branch-coverage evidence, use:

```sh
make coverage-selected \
  COVERAGE_CASES='decode:avif:portable_lossless_monochrome_nonuniform_tiles_256x256'
```

## AV1 nonuniform tile-column cap parity

`error_nonuniform_tile_column_cap_4097x1.avif` is a still 4097x1 monochrome
AVIF with 65 superblocks and a declared 64-column nonuniform grid that leaves
the last superblock uncovered. The generator flips the final tile-width bit,
then changes one bit in the first tile payload. That payload corruption makes
the pinned Pillow oracle reject the file; the Rust parser reaches the earlier
tile-column cap check and returns the same malformed-input category. This
keeps the cap boundary in a passing public parity row without relying on a
target-only sequence expectation.

Build the pinned libaom/libavif producer as described above. Reproduce and
check the retained input with:

```sh
target/oracle-staging/encode-avif-nonuniform-tiles \
  --tile-limit target/oracle-staging/nonuniform-tile-limit-seed.avif
.oracle-venv/bin/python scripts/generate_avif_nonuniform_tile_limit_error.py \
  --encoder target/oracle-staging/encode-avif-nonuniform-tiles
.oracle-venv/bin/python scripts/generate_avif_nonuniform_tile_limit_error.py --check
```

The checked AVIF SHA-256 is
`08da0817962bf18a5bdf2eba90c35c0ce82997649f393599ccf6ad2e4bb4c955`. Run the
valid nonuniform-tile and cap-edge parity rows together with:

```sh
make coverage-selected \
  COVERAGE_CASES='decode:avif:portable_lossless_monochrome_nonuniform_tiles_256x256 decode:avif:error_nonuniform_tile_column_cap_4097x1'
```
