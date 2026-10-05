# AVIF fixture provenance

These files are fixed inputs for the Pillow 12.2.0 parity manifest. Copied
upstream inputs are listed first; repository-generated inputs and their exact
generation provenance follow.

| Local file | Upstream file | Tag / commit | SHA-256 | License |
| --- | --- | --- | --- | --- |
| `baseline.avif` | Pillow `Tests/images/avif/hopper.avif` | Pillow 12.2.0 / `3c41c09` | `d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2` | MIT-CMU (`third_party/pillow/LICENSE`) |
| `ipma_version_1_wide_item_ids.avif` | Baseline still with a version-1 `ipma` FullBox and widened 32-bit item ID | `scripts/generate_test_assets.py`; Pillow 12.2.0; 2-byte box growth with relocated `iloc` extent | `90ac8f0230ddede24a72b73fb120bd46680962d4c0a7facab31e77b088cd82c5` | MIT/Apache-2.0 (repository-generated mutation) |
| `iloc_version_2_wide_item_ids.avif` | Baseline still with version-2 item locations, 32-bit item count/ID, and file-backed construction method | `scripts/generate_test_assets.py`; Pillow 12.2.0; exact decoded pixels and image info match baseline; 6-byte box growth with relocated extent | `39a494da43edb3e25dcc60c1c51c987452c66a2590e5ac9563d416d922199d87` | MIT/Apache-2.0 (repository-generated mutation) |
| `ipma_wide_associations.avif` | Baseline still with 16-bit `ipma` property-association indices | `scripts/generate_test_assets.py`; Pillow 12.2.0; 4-byte box growth with relocated `iloc` extent | `7bef3a5c1db6196b199c03406719f5dcd4b0bb6a7320de44b39a7daa52e6116b` | MIT/Apache-2.0 (repository-generated mutation) |
| `primary_item_irot_pasp_4x3.avif` | `primary_item_irot.avif` with an associated 4:3 pixel-aspect-ratio property | `scripts/generate_test_assets.py`; Pillow 12.2.0; 17-byte metadata growth with relocated `iloc` extent | `2acc2e59572718a5fd1247aef7d18ab7666943533f068c221117df05378c4564` | MIT/Apache-2.0 (repository-generated mutation) |
| `icc_profile.avif` | Repository-generated 4x4 RGB portable-pattern still using the profile from `png/iccp.png` | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 8, 4:4:4, one thread, CICP 1/13/6, autotiling disabled | `a59749aa4985d4954cb8ef8f513e17b40f1051f969374eff7a654aeb0a629d95` | MIT/Apache-2.0 (repository-generated) |
| `icc_ricc_type.avif` | Full-file `prof` to `rICC` property-type variant; Pillow accepts the same profile bytes and RGB pixels | Pillow 12.2.0; one four-byte AVIF property mutation | `36e07c9ec8afd80be79396196b3fec7397caec1cec3bc5f89e0064d98a87ea9b` | MIT/Apache-2.0 (repository-generated) |
| `icc_mdcv.avif` | ICC still with one associated BT.2020/D65 mastering-display property | Pillow 12.2.0; deterministic `mdcv` property/association insertion with relocated item extent | `d06f268e6f7341d67e1bf62510681375bc40ea4815a27b6aa4e38172dde01882` | MIT/Apache-2.0 (repository-generated) |
| `icc_mdcv_trailing.avif` | ICC still whose associated `mdcv` property has one trailing byte | Pillow 12.2.0; deterministic bounded property extension with relocated item extent | `956465627904603da6df5c6ce0419cb401156dff57cf2a5ab28a332009d0e946` | MIT/Apache-2.0 (repository-generated) |
| `alpha.avif` | Pillow `Tests/images/avif/transparency.avif` | Pillow 12.2.0 / `3c41c09` | `b19f57d9421bbd3d0b0706c8fe79cef802aebf106afefa6bddde9de1a07509c9` | MIT-CMU (`third_party/pillow/LICENSE`) |
| `10bit.avif` | libavif `tests/data/colors-animated-12bpc-keyframes-0-2-3.avif` | libavif 1.4.1 / `6543b22` | `3bf9f91da471749e7df639ba7945d4d94c1c3e3968c26f3619fbbcfc92790576` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `high_bitdepth_still_12bit_444_lossless.avif` | Deterministic 16x16 RGB source encoded as a single-frame 12-bit 4:4:4 witness | libavif 1.4.1 / `6543b22`; libaom 3.13.2 fixture build | `8645ee1ecc437868c5842248444ea6c8400d983a03bcfe75710bb0a424915abd` | MIT/Apache-2.0 (repository-generated) |
| `profile2_identity_12bit_444.avif` | Deterministic 16x16 RGB source encoded as a profile-2 12-bit full-range 4:4:4 identity-CICP still | `scripts/generate_avif_profile2_identity.py`; libavif 1.4.1 / libaom 3.13.2; `--qcolor 100 --cicp 1/13/0 --range full --speed 8 --jobs 1` | `1ce2aae5539074e3a65092f185c4db34537462d637859d4fd85449bb15557931` | MIT/Apache-2.0 (repository-generated) |
| `high_bitdepth_still_10bit_420_lossy_16x16.avif` | Deterministic 16x16 planar 10-bit I420 source encoded as a single-frame lossy profile-0 AVIF | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980` | `60c40a86c0409add22d7ab678b579ea4ccf979624bf3b127a4448d0a83e18d18` | MIT/Apache-2.0 (repository-generated) |
| `high_bitdepth_still_10bit_420_alpha_lossless_16x16.avif` | Deterministic 16x16 planar 10-bit YUV420 still with a monochrome 10-bit auxiliary alpha item; both AV1 frames all-lossless | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980` | `8c3cb86055e1003d91897d986995dd2ee030d0278b82d3d7c9ddc783189eb76f` | MIT/Apache-2.0 (repository-generated) |
| `high_bitdepth_still_10bit_422_alpha_lossless_16x16.avif` | Deterministic 16x16 planar 10-bit YUV422 still with a monochrome 10-bit auxiliary alpha item; both AV1 frames all-lossless | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980` | `eef1ffae7961354032dad642c1ebfc146cbcb28f7147c4dce6d9b8a4de17d392` | MIT/Apache-2.0 (repository-generated) |
| `high_bitdepth_still_10bit_444_alpha_lossy_16x16.avif` | Deterministic 16x16 planar 10-bit YUV444 still with a monochrome 10-bit auxiliary alpha item | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980` | `468bd5af2f96693582d6618572dc353cac4ee16719cebebb8e20ff7e81049548` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_b32x32_10bit.avif` | Deterministic two-frame full-range 10-bit I420 lossless sequence with a translated patch | `scripts/generate_avif_10bit_lossless_inter_fixture.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; timestamp-normalized | `c90bf9e75b0c091e19ab6b8aa70c17243ae1dbbf818493e815015f998f14e73a` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b32x32_10bit_partition32_28x64.avif` | Deterministic two-frame 28x64 full-range 10-bit I420 lossless sequence with a translated textured 24x24 patch in the padded left B32x32 leaf | `scripts/generate_avif_10bit_lossless_inter_fixture.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; timestamp-normalized | `8f2b787faea151faf9c7f69c2b92e098dedd3bb70f28fd33546194c002b3456f` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b32x32_10bit_partition32_56x64.avif` | Deterministic two-frame 56x64 full-range 10-bit I420 lossless sequence with fixed 32-pixel partitions and a translated textured patch in the clipped 24x32 right-edge B32x32 leaf | `scripts/generate_avif_10bit_lossless_inter_fixture.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; timestamp-normalized | `a819711996c804d7c5e4dcc26cee2e6a35bb82c48a88a6db8e34bd61d3b5f9fe` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_superres_sgr_10bit_160x56.avif` | Deterministic two-frame full-range 10-bit I420 lossy sequence; frame 1 uses super-resolution and SGR restoration | `scripts/generate_avif_hi10_superres_restoration.py`; libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `016e4a8433002b60899744fba6f26a7c10af82c192f65b4c5c19233b15c3cb11` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_superres_sgr_8bit_160x56.avif` | Deterministic two-frame full-range 8-bit I420 lossy sequence; frame 1 uses super-resolution and SGR restoration with largest-transform mode and 16x16 partitions | `scripts/generate_avif_8bit_superres_restoration.py`; patched pinned fixture encoder; libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `c294163610d4a45852fe374e0345c878979bb81e5ea94596960ef64411180fd7` | MIT/Apache-2.0 (repository-generated) |
| `superres_equal_width_16x16.avif` | Deterministic 16x16 full-range 8-bit I420 lossy still; AV1 signals super-resolution denominator 9 while minimum coded width leaves the coded and upscaled widths equal | `scripts/generate_avif_equal_width_superres.py`; patched pinned fixture encoder; libavif 1.4.1 / libaom 3.13.2 | `2de74d720f8863be43049a3df776337c1cde494122a57093ffad532cea19a004` | MIT/Apache-2.0 (repository-generated) |
| `superres_equal_width_i444_16x16.avif` | Deterministic 16x16 full-range 8-bit I444 lossy still; AV1 signals super-resolution denominator 9 with equal coded and upscaled widths, exercising full-resolution chroma planes | `scripts/generate_avif_equal_width_superres.py --chroma 444`; patched pinned fixture encoder; libavif 1.4.1 / libaom 3.13.2 | `cd02b86f213b96f9d58bec3781f8855f325aa7b4c867a6fbf76f5802886420a4` | MIT/Apache-2.0 (repository-generated) |
| `superres_actual_upscaled_i444_32x16.avif` | Deterministic 32x16 full-range 8-bit I444 lossy still; AV1 signals denominator 9 and coded width 28, exercising real resizing of luma and both full-resolution chroma planes | `scripts/generate_avif_equal_width_superres.py --chroma 444-actual`; patched pinned fixture encoder; libavif 1.4.1 / libaom 3.13.2 | `c976ff5f3a4669557d6dc45780743ab462f1766de732c7914d5094663e323948` | MIT/Apache-2.0 (repository-generated) |
| `superres_actual_upscaled_i422_33x17.avif` | Deterministic 33x17 full-range 8-bit I422 lossy still; AV1 signals denominator 9 and coded width 29, with restoration disabled to exercise the generic I422 super-resolution path across odd visible edges | `scripts/generate_avif_equal_width_superres.py --chroma 422-actual`; patched pinned fixture encoder; libavif 1.4.1 / libaom 3.13.2 | `9095580af62e5003fcb8f68d62da1b6b0cbb6205578a2828d05a87ee2f367c1f` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_superres_select_10bit_160x56.avif` | Deterministic two-frame full-range 10-bit I420 lossy sequence; frame 1 uses super-resolution and TX_MODE_SELECT without active restoration | `scripts/generate_avif_hi10_superres_select.py`; libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `04e31a3ac36c25ef77061a2ed09b79fd8fb885a5ff14d6ab3a82e2d32ad3b02e` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_1x1.avif` | Deterministic 1x1 8-bit monochrome Luma still with sample 127 | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, 4:0:0, CICP 1/13/6 | `6c4212de07ead445c0b468c39b77f099cc8555e99edd6460806407d7536be305` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_limited_16x16.avif` | Deterministic 16x16 8-bit lossless 4:0:0 luma still with limited-range samples | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread, CICP 1/13/6 | `e14c3cce9943b6f4f12ad3f0a93624c702678a73fa55101ba61e61aabe3168be` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_limited_10bit_16x16.avif` | Deterministic 16x16 10-bit lossless 4:0:0 luma still with limited-range samples | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread, CICP 1/13/6 | `514a2b65aa84f2bd647a8c041cf20c7b59c8fb7e71f8303bc1a4a408defccd66` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_limited_12bit_16x16.avif` | Deterministic 16x16 12-bit lossless 4:0:0 luma still with limited-range samples | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread, CICP 1/13/6 | `3abcfd332dff4d6ccfc801b75430c1997e1738b1546006ab5590a6e1fcfd549e` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_limited_bt2020_pq_16x16.avif` | Deterministic 16x16 8-bit lossless limited-range 4:0:0 still with BT.2020/PQ CICP 9/16/9 | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread | `ad3ce0e1171561690b156149fa8761fcc8adf26670507b0d64cc4294994d344b` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_limited_alpha_16x16.avif` | Deterministic 16x16 8-bit lossless limited-range 4:0:0 luma still with a full-range alpha item | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread, CICP 1/13/6 | `b95edc757441fad6b2c051e1df78c888fab81aeab5c81e366d8fd4b5941a58fa` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_limited_alpha_10bit_16x16.avif` | Deterministic 16x16 10-bit lossless limited-range 4:0:0 luma still with a full-range alpha item | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread, CICP 1/13/6 | `2497017931a998c26e7328b0e940b736ccbaaab5f3dfc4d3e6637ebb7e7168f2` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_limited_alpha_12bit_16x16.avif` | Deterministic 16x16 12-bit lossless limited-range 4:0:0 luma still with a full-range alpha item | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread, CICP 1/13/6 | `27aed8594e5d60179e6596dd950800fdf43f6809038ed30c0b86aaf4c20ac3c7` | MIT/Apache-2.0 (repository-generated) |
| `portable_i444_quality100_64x64.avif` | Deterministic 64x64 8-bit quality-100 RGB 4:4:4 key frame with fixed 64-pixel partitions | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 64-pixel partitions, CICP 1/13/6 | `5ceb66b47bda43bec5c421222eeb3ef858c62702bc41833986fdf678f6867737` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_filmgrain_i444_64x64.avif` | Seeded 64x64 8-bit all-lossless RGB 4:4:4 key frame with film grain | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 64-pixel partitions, DCT only, CICP 1/13/6, film-grain test enabled | `5e40ca71068fdd232d11f356103f53c1fdd7b94356fa91b0004d7307de4cbd25` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_filmgrain_420_chroma_from_luma_64x64.avif` | Seeded 64x64 8-bit all-lossless I420 key frame using chroma scaling from luma for film grain | `scripts/generate_avif_filmgrain_chroma_from_luma_420.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, CICP 1/13/6, film-grain test enabled | `80c3a157f32f190a787588c48232113c4bbe915abd80812c5e0d1d62fd51c893` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_filmgrain_420_zero_y_points_64x64.avif` | Seeded 64x64 8-bit all-lossless I420 key frame whose update-grain syntax has zero Y points and omits conditional U/V points and coefficients | `scripts/generate_avif_filmgrain_zero_y_420.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; deterministic film-grain header rewrite | `c76ede4ac45599b182d023f8425744f09dc27e4011742d6295d3ec50e172ed9f` | MIT/Apache-2.0 (repository-generated) |
| `animated_filmgrain_reference_reuse_i444_64x64.avif` | Deterministic two-frame 64x64 I444 sequence whose inter frame reuses key-frame film-grain parameters with a new seed | `scripts/generate_avif_filmgrain_reference_reuse.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `bd947085ed6437edfd50a97b43506cc56e96af8b5450a8ef5cb8289b8ec62b34` | MIT/Apache-2.0 (repository-generated) |
| `hdr.avif` | libavif `tests/data/colors_hdr_rec2020.avif` | libavif 1.4.1 / `6543b22` | `9980e58ddf718a923f1738c34aad1c72f8e5795ec07e68f1a5f9bd216ca19740` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `grid.avif` | libavif `tests/data/color_grid_alpha_nogrid.avif` | libavif 1.4.1 / `6543b22` | `bae56368b348b1d847e2bfb662522599f0c63dfe62fb68826c9e42a300ff405d` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated.avif` | libavif `tests/data/colors-animated-8bpc.avif` | libavif 1.4.1 / `6543b22` | `2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_lossless_inter_420_clipped_b32x32_17x17.avif` | Deterministic two-frame 17x17 RGB sequence encoded as an MI-clipped lossless AV1 4:2:0 inter-grid witness | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 8, one thread | `b29bedb6d7486dae5bed2abb74986a65e47bc737bfd26f85b1f69fa8bd666819` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_i422_clipped_b32x32_49x64.avif` | Deterministic two-frame odd-width 49x64 RGB 4:2:2 sequence selecting a clipped lossless AV1 B32x32 edge grid | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 32x32 partitions, timestamp-normalized | `fd18cdcef915327fd4aab6992c901e27f2efc1411d1d2d01eee93dd3f277d7b1` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_i444_clipped_b32x32_49x64.avif` | Deterministic two-frame 49x64 RGB 4:4:4 sequence selecting a clipped 24x32 lossless AV1 B32x32 edge grid | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 32x32 partitions, timestamp-normalized | `d45defceb497272879b1093f76e6a6c645f0bde8a6d10a1903e2322af422caae` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_i422_clipped_b32x32_52x64.avif` | Deterministic two-frame 52x64 RGB 4:2:2 sequence selecting a clipped 20x32 lossless AV1 B32x32 edge grid | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 32x32 partitions, timestamp-normalized | `43bbcd3d92593e026d88ce4dea727074f74134175ee42a962886c6de697a4e05` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_i444_clipped_b32x32_64x56.avif` | Deterministic two-frame 64x56 RGB 4:4:4 sequence selecting a clipped 32x24 lossless AV1 B32x32 bottom-edge grid | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 32x32 partitions, timestamp-normalized | `9004ec840b2a413c0e15294f52c723ebe5c3689cbe3decbd4afd5575cad78e23` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_i422_clipped_b32x32_52x60.avif` | Deterministic two-frame 52x60 RGB 4:2:2 sequence selecting a simultaneously right-and-bottom-clipped 20x28 luma, 10x28 chroma lossless AV1 B32x32 grid | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 32x32 partitions, timestamp-normalized | `53e06d79c586151d920b94cc6173eebcc1f9869ebb0f1fa5bec673df11c45e50` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_i444_clipped_b32x32_52x60.avif` | Deterministic two-frame 52x60 RGB 4:4:4 sequence selecting a simultaneously right-and-bottom-clipped 20x28 lossless AV1 B32x32 grid | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 32x32 partitions, timestamp-normalized | `9fdd099eb28fed9eaff2760b154e42d91933758fd6b01e532e441646f564993a` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b32x32_60x64.avif` | Deterministic two-frame 60x64 RGB sequence with a translated patch selecting a partially visible lossless AV1 4:2:0 B32x32 inter grid | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 8, one thread | `a60dde64c6b7bf9f6fbb3afe7e75a74374d446f8106613b00837300df4963bbc` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b32x32_64x60.avif` | Deterministic two-frame 64x60 RGB sequence with a translated lower-edge patch for bottom-edge frame cropping | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 8, one thread | `7602fd062d0aefdaa0221b35a3a6c9403fecc27bf8a66cb00c0b4f9b8763c0d6` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b32x32_184x64.avif` | Deterministic two-frame textured 184x64 RGB sequence selecting a clipped lossless AV1 4:2:0 B32x32 inter grid at the right edge | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 32x32 partitions | `0a8c935fe67694fe576e7f21064eec4c428cffc2a05a3ff2d0be33e401a20c1d` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b16x16_60x64.avif` | Deterministic two-frame 60x64 RGB texture shifted one column to select a right-edge lossless AV1 4:2:0 B16x16 inter block clipped to 12 columns | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 16x16 partitions | `7c8bd0f88cd6654fb1150d2220ecb33c6ec5874b26a2250909b51cab4ed2e17f` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_b16_mixed_topology_inter_420_b16x16.avif` | Deterministic three-frame 16x16 4:2:0 sequence whose final B16x16 inter block splits only its top-right TX8 child to TX4 | `scripts/generate_avif_b16_mixed_topology_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 99, speed 0, one thread, fixed 16x16 partitions, timestamp-normalized | `3264482e2a50d80bd39be178b843fdfe9997e947d54b4466c745751837821d74` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_i444_obmc_mixed_b16x32.avif` | Deterministic 32x32 checker sequence encoded as an AV1 4:4:4 inter-prediction edge-case witness | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 75, speed 0, one thread, partitions 16–32 | `71e737f7b196173d98ae6447925cd8f6f77d95ced3eaa80d924ad81febcb3743` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_422_checker_random_b32x32.avif` | Deterministic two-frame 32x32 jittered-checker sequence encoded as a lossy AV1 4:2:2 inter-prediction witness | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 75, speed 0, one thread, partitions 16–32 | `e46aef4aed9076da48627a449e53c6f318a0494ba5d94a32a4f13edc548ab134` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_global_rotzoom_compound_i444_b128x128.avif` | Deterministic four-frame 256x256 4:4:4 affine-motion sequence encoded as a global-motion edge-case witness | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 80, speed 0, one thread | `14ffb1529c54f07a397a1fe94d02135f9def3a7730416cb80d5acbf3c54c144e` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_global_halfblend_spatial_i444_b256x256.avif` | Deterministic four-frame 256x256 4:4:4 sequence with identity and RotZoom global references paired in compound spatial neighbors | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 80, speed 0, one thread | `ec2f145f64fa87c5b7c255b13b6009fb58760be06070724ed367cd4ccd9a6d7b` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_wide_i444_mode2_split32_b128x128.avif` | Deterministic two-frame 256x256 I444 sequence selecting TX64-to-TX32 splits in each 128x128 mode-2 inter leaf | `scripts/generate_avif_mode2_split32_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 80, speed 0, one thread, fixed 128-pixel partitions | `f48a6d235c7ab245e8d57aedcc6135b897cea1bcbc579c3d630aa7c20e1a61aa` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_interintra_420_nowedge_mode3_b16x16_64x64.avif` | Deterministic three-frame 64x64 4:2:0 smooth inter-intra mode-3 witness | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 50, speed 0, one thread | `7e035cef7293dfff72e50ff8c729bc0fad360ee14d7bf80d663b82b96ca7c4d7` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_interintra_420_wedge_b16x16_64x64.avif` | Deterministic colored three-frame 64x64 4:2:0 sequence whose final B16x16 block selects AV1 inter-intra wedge index 4 and exercises chroma-mask downsampling | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 50, speed 0, one thread | `1510b764d7e642f2caf02fff53e6dc966cfb92df8eca2e8f2d91c0b3bead1791` | MIT/Apache-2.0 (repository-generated) |
| `animated_tx64_root_split_inter_420_64x64.avif` | Deterministic three-frame 64x64 4:2:0 sequence with a TX64 root split and two TX32 child offsets | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 80, speed 0, one thread, fixed 64x64 partitions | `9f4450d4d9c7c2738d4f9f34eafb02100c5fb15b85ac121a057c3eef367f6d7c` | MIT/Apache-2.0 (repository-generated) |
| `animated_motion_temporal_window_left_512x128.avif` | Deterministic four-frame 512x128 4:2:0 translated-patch sequence that misses the projected motion window on the left | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 95, speed 0, one thread | `29e856c6c8a117c764bb0a176ccc1d82bac982fdf7287eb461f827ebdc31b399` | MIT/Apache-2.0 (repository-generated) |
| `animated_motion_temporal_window_right_512x128.avif` | Deterministic four-frame 512x128 4:2:0 translated-patch sequence that misses the projected motion window on the right | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 95, speed 0, one thread | `10d8e5514c1d8d9cad355f9a912b3e3075bf8bd661ee6af5ce44a9ce62d7cad2` | MIT/Apache-2.0 (repository-generated) |

The upstream libavif test-data README identifies the copied libavif files as
covered by libavif's own license. `10bit.avif` retains its historical manifest
name but is a 12-bit high-bit-depth animation; the manifest description states
the exact depth.

`animated_lossless_inter_420_b32x32_10bit.avif` is generated by
`scripts/generate_avif_10bit_lossless_inter_fixture.py` and its C helper in
`scripts/avif_fixture_oracle/encode_lossless_inter_10bit.c`. Pillow's bundled
AOM cannot encode 10-bit AVIF, so the generator validates clean local source
checkouts at libaom `ad44980d7f3c7a2605c25d51ea96946949000841` and libavif
`6543b22b5bc706c53f038a16fe515f921556d9b3`, including their retained license
files, then builds both under a fresh ignored `target/oracle-staging` directory.
Create or provide those checkouts and run:

```sh
git clone https://aomedia.googlesource.com/aom target/oracle-staging/libaom
git -C target/oracle-staging/libaom checkout ad44980d7f3c7a2605c25d51ea96946949000841
git clone https://github.com/AOMediaCodec/libavif target/oracle-staging/libavif
git -C target/oracle-staging/libavif checkout 6543b22b5bc706c53f038a16fe515f921556d9b3
.oracle-venv/bin/python scripts/generate_avif_10bit_lossless_inter_fixture.py \
  --aom-source target/oracle-staging/libaom \
  --libavif-source target/oracle-staging/libavif
```

The generator runs the native encoder twice, zeroes version-one `mvhd`, `tkhd`,
and `mdhd` timestamps, and requires byte-identical normalized outputs with the
listed hash. It also checks the two-frame 10-bit 4:2:0 key/inter all-lossless
bitstream shape and Pillow 12.2.0 RGB frame references. Each frame is 3,072
bytes, with SHA-256 values
`0a9a2996f570e2959cbd25e68991ceb2fbd153485af10d4ebfb2fed8fe3170a7` and
`a9346f58b101a9c058ad8271404b371773b0fea8fc3255c8c8befcf7104c4345`.

The 28x64 companion forces 32x32 partitions. AV1 pads this frame to 32 luma
columns for its partition tree, and its translated 24x24 patch stays inside the
left B32x32 leaf. The generator verifies key/inter frame types, repeatable AV1
samples, and exact Pillow 12.2.0 frame pixels. Its normalized file SHA-256 is
`8f2b787faea151faf9c7f69c2b92e098dedd3bb70f28fd33546194c002b3456f`; the
frame hashes are
`3dc1cdc564be835c5afbd6cac356fcda1b5f8c7b58be38b18331a5b1deec0f9e` and
`15b9ec6a29475ae276ce6c118088943f352ac2b383f83218a0ce133cbd0ac420`.

The 56x64 companion fixes partitions to 32x32 and places its translated
16x24 textured patch inside the right-edge B32x32 leaf, which has a 24x32
visible extent. Its normalized file SHA-256 is
`a819711996c804d7c5e4dcc26cee2e6a35bb82c48a88a6db8e34bd61d3b5f9fe`; the
Pillow frame hashes are
`8d14c698d8415e0e22c67a43b245c4a645cecd3e048ec1e8dcbf0bba1f112100` and
`3b24a0b82c7f7572d0fbf148d3eb205fdc251650f8d73258fbf16e4b371ef5b6`.

`animated_lossy_inter_420_superres_sgr_10bit_160x56.avif` is generated by
`scripts/generate_avif_hi10_superres_restoration.py` from two deterministic
160x56 full-range 10-bit I420 frames. The key frame is flat at sample 512; the
inter frame alternates 448/576 samples in 8x8 tiles. The generator requires
`avifenc` built with libavif 1.4.1 and libaom 3.13.2, double-encodes the
sequence, normalizes movie timestamps, and checks the AV1 syntax and exact
Pillow 12.2.0 RGB frames. Run it with the pinned encoder binary:

```sh
.oracle-venv/bin/python scripts/generate_avif_hi10_superres_restoration.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc
```

The inter frame is coded at 128x56 and super-resolved to 160x56 with SGR
restoration on all three planes. The normalized input hashes to
`016e4a8433002b60899744fba6f26a7c10af82c192f65b4c5c19233b15c3cb11`; the two
Pillow RGB frame hashes are
`69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed` and
`199a031bed1879efe5b17d90bacd44226ec2c05da120f4e981326695e6d493d0`.
Its super-resolution encoder controls require the fixture-only compatibility
patch and build recipe in
[`scripts/avif_fixture_oracle/README.md`](../../../../../scripts/avif_fixture_oracle/README.md).

`animated_lossy_inter_420_superres_sgr_8bit_160x56.avif` exercises the same
inter-frame super-resolution and SGR restoration profile in 8-bit I420. Its
key frame is flat at sample 128; the inter frame alternates 112/144 samples in
8x8 tiles. The generator fixes 16x16 partitions and disables TX size search,
which selects the largest transform mode required by this decoder path. It
normalizes BMFF timestamps, double-encodes the input, and checks AV1 syntax
plus Pillow 12.2.0 frame pixels. The pinned libavif fixture-controls patch and
build steps are documented in
[`scripts/avif_fixture_oracle/README.md`](../../../../../scripts/avif_fixture_oracle/README.md).
Regenerate it with:

```sh
.oracle-venv/bin/python scripts/generate_avif_8bit_superres_restoration.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc
```

The normalized asset hash is
`c294163610d4a45852fe374e0345c878979bb81e5ea94596960ef64411180fd7`; Pillow
RGB frame hashes are
`69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed` and
`0bb7e3278f7cad0535a87e433d644434074b207396bf668ab3f5007dfca98061`.

`animated_lossy_inter_420_superres_select_10bit_160x56.avif` uses the same
deterministic source frames but a pinned quality-39, speed-5 encode. The inter
frame is coded at 128x56, super-resolved to 160x56, and uses TX_MODE_SELECT
with no active restoration. Run
`.oracle-venv/bin/python scripts/generate_avif_hi10_superres_select.py` with
the same pinned `avifenc` binary:

```sh
.oracle-venv/bin/python scripts/generate_avif_hi10_superres_select.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc
```

The normalized asset hashes to
`04e31a3ac36c25ef77061a2ed09b79fd8fb885a5ff14d6ab3a82e2d32ad3b02e`; Pillow
RGB frame hashes are
`69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed` and
`7aa2455358aa4a1d51b3a4a263a425229b1be524efd68979a26dad18c8250fef`.

`animated_lossy_inter_i444_obmc_mixed_b16x32.avif` is a retained 32x32,
three-frame, 8-bit 4:4:4 candidate from the deterministic `checker` seed-490
AV1 search recipe. It uses quality 75, speed 0, one encoder thread, 16–32
pixel partitions, and disables AQ, delta-Q, CDEF, restoration, and loopfilter
control. Its terminal B16x32 combines a single-reference target matching the
second lane of a compound spatial neighbor with OBMC and mixed transform-split
topology. The exact encoded witness is retained at the listed SHA-256; the
three pinned Pillow RGB frame references are recorded by the decode manifest.

`animated_lossy_global_rotzoom_compound_i444_b128x128.avif` is a retained
256x256, four-frame, 8-bit 4:4:4 affine-motion candidate from the deterministic
global-motion search. It uses quality 80, speed 0, and one encoder thread. Its
AV1 frame headers carry non-identity global RotZoom references, and frame 3
contains four 128x128 compound GlobalGlobal blocks. The exact encoded witness
is retained at the listed SHA-256; pinned Pillow RGB frames, durations, and
independent libavif loop evidence are recorded by the decode manifest and AVIF
loop bundle.

`animated_lossy_global_halfblend_spatial_i444_b256x256.avif` is a retained
256x256 four-frame 8-bit 4:4:4 sequence generated from seeded rectangles,
small rotations, and a half blend. The pinned encoder produces a compound
spatial neighbor whose identity-reference lane has no affine vector while the
RotZoom lane does. This reaches both outcomes of the public motion projection
condition. The generator double-encodes the sequence, normalizes movie
timestamps, and checks the exact input hash; Pillow frame outputs and
independent libavif loop evidence are recorded with the decode matrix.

`animated_lossy_wide_i444_mode2_split32_b128x128.avif` is generated by
`scripts/generate_avif_mode2_split32_fixture.py`. Its flat gray key frame is
followed by a row-alternating, 32-pixel grayscale sinusoid. With fixed
128-pixel partitions, the inter frame splits each TX64 root once to TX32 and
leaves the TX32 children unsplit in all four 128x128 I444 blocks. The generator
checks deterministic timestamp-normalized bytes and both pinned Pillow frame
hashes; public decode and complete-file loop references are retained.

`animated_lossy_interintra_420_nowedge_mode3_b16x16_64x64.avif` extends the
non-wedge smooth inter-intra sequence to mask mode 3. Its final frame contains
a deterministic 16x16 grayscale patch generated from the smooth mask weights.
Pinned dav1d syntax tracing confirms mode 3 on the final-frame B16x16 at
`(16,16)`. All three public Rust RGB frames and their 100ms durations match
Pillow exactly. The normalized input hash is
`7e035cef7293dfff72e50ff8c729bc0fad360ee14d7bf80d663b82b96ca7c4d7`.

`animated_lossy_interintra_420_wedge_b16x16_64x64.avif` uses a colored
reference and final-frame wedge ramp so 4:2:0 chroma-mask downsampling affects
decoded pixels. Pinned dav1d syntax tracing confirms wedge index 4 on the final
B16x16 at `(16,16)`. The three public Rust RGB frames and their 100ms durations
match Pillow exactly. Its normalized input hash is
`1510b764d7e642f2caf02fff53e6dc966cfb92df8eca2e8f2d91c0b3bead1791`.

`animated_lossy_inter_422_checker_random_b32x32.avif` is generated by
`scripts/generate_test_assets.py` from two 32x32 RGB frames with per-frame
seeded checker jitter and a translated patch. It uses Pillow 12.2.0, libavif
1.4.1, libaom 3.13.2, quality 75, speed 0, one thread, 16–32 pixel
partitions, and timestamp-normalized AVIF sequence boxes. The generator
double-encodes the sequence and checks the exact input hash. The two Pillow RGB
references are each 3,072 bytes, with hashes
`64a05a10f3a33ccd351e0c529f56cff96fabb33e0fc81a5f96f43c3f4c1c6c6e` and
`c98fa755c2b5a8ba48a7e821d9ecad46d4ebbc4cc59535b70c24255d3ffe2437`.

`animated_tx64_root_split_inter_420_64x64.avif` uses a deterministic low-range
luma texture with two larger checkerboard changes. Pinned AOM selects a B64
inter root split into two TX32 children, and public Rust decoding reaches both
child x offsets. The generator fixes the encoder settings, normalizes sequence
timestamps, double-encodes the source, and checks the listed input hash.

`animated_motion_temporal_window_left_512x128.avif` and
`animated_motion_temporal_window_right_512x128.avif` use a deterministic
96x64 textured patch on a flat 512x128 background. The four-frame patch path
alternates between x=192 and x=32; reversing the path gives the second file.
The pinned encoder selects temporal motion candidates whose projected sample
falls left of the window in the first fixture and at or beyond its right edge
in the second. Both retain exact Pillow frame references and native AVIF loop
observations; the generator double-encodes and checks each normalized hash.

`animated_lossless_inter_420_clipped_b32x32_17x17.avif` is generated by
`scripts/generate_test_assets.py` from two 17x17 RGB frames with a diagonally
translated 4x4 patch. Pinned Pillow 12.2.0/libavif 1.4.1/libaom 3.13.2 encode
it at quality 100, speed 8, and one thread as 8-bit lossless 4:2:0. The
inter-frame B32x32 residual grid exercises the spec's MI-padded 24x24 boundary
while Pillow's exact two-frame RGB output is retained in the decode manifest.
The generator double-encodes the input and checks the exact SHA-256 above.

`animated_lossless_inter_i444_clipped_b32x32_49x64.avif` uses a deterministic
49x64 RGB texture and a second frame shifted left by one pixel. Pillow
12.2.0/libavif 1.4.1/libaom 3.13.2 encode it losslessly at quality 100 and
speed 0 as 4:4:4 with fixed 32x32 partitions. The second frame's clipped
right-edge B32x32 block has a 24x32 visible extent. The generator encodes the
sequence twice, zeroes version-one movie timestamps, and checks the listed
hash; the decode manifest retains exact Pillow pixels for both frames.

`animated_lossless_inter_i422_clipped_b32x32_49x64.avif` is generated from a
flat 49x64 RGB key frame and a second frame with a 16x16 patch at `(32,40)`.
Pillow 12.2.0/libavif 1.4.1/libaom 3.13.2 encode it losslessly at quality 100
and speed 0 as 4:2:2 with fixed 32x32 partitions. Its right-edge B32x32 block
touches the final column of an odd-width frame. The key frame has exact
Rust-to-Pillow pixel parity; Rust sequence decoding retains an explicit
Unsupported/NotImplemented expectation because the final chroma sample
geometry is not parity-safe for this layout. The generator encodes the input
twice, zeroes version-one movie timestamps, and checks the listed hash.

`animated_lossless_inter_i422_clipped_b32x32_52x64.avif` is generated from a
flat 52x64 RGB key frame and a second frame with a 16x16 patch at `(36,40)`.
Pillow 12.2.0/libavif 1.4.1/libaom 3.13.2 encode it losslessly at quality 100
and speed 0 as 4:2:2 with fixed 32x32 partitions. Its right-edge B32x32 block
has a 20x32 visible luma extent; the generator encodes it twice, zeroes
version-one movie timestamps, and checks the listed hash.

`animated_lossless_inter_i444_clipped_b32x32_64x56.avif` uses a deterministic
64x56 RGB texture below a flat top half, with the second frame's lower texture
shifted upward by one row. Pillow 12.2.0/libavif 1.4.1/libaom 3.13.2 encode it
losslessly at quality 100 and speed 0 as 4:4:4 with fixed 32x32 partitions.
The bottom B32x32 inter block has a 32x24 visible extent. The generator
double-encodes the sequence, zeroes version-one movie timestamps, and checks
the listed hash; exact Pillow pixels for both frames are retained.

The two `animated_lossless_inter_{i422,i444}_clipped_b32x32_52x60.avif`
fixtures use the same deterministic lower-right RGB texture in 4:2:2 and
4:4:4. Their final B32x32 block is clipped on both axes to 20x28 luma; I422
also uses 10x28 chroma. The second frame shifts the texture upward by one row.
Both variants use Pillow 12.2.0/libavif 1.4.1/libaom 3.13.2 at quality 100
and speed 0 with fixed 32x32 partitions. Their generators double-encode,
zero version-one movie timestamps, and verify the listed hashes; exact Pillow
pixels for both frames are retained.

`animated_lossless_inter_420_clipped_b32x32_60x64.avif` is generated from two
60x64 RGB frames whose 28x32 edge patch moves one pixel left. The pinned
libavif/libaom encoder deterministically selects an inter B32x32 leaf at
pixel `(32,32)` in frame 1, with a 28x32 visible extent at the right edge.
This adds a larger clipped-grid boundary to the 17x17 MI-padded witness; the
decode manifest retains Pillow's exact frame pixels.

`animated_lossless_inter_420_clipped_b32x32_64x60.avif` is generated from two
64x60 RGB frames whose 32x28 lower-edge patch moves upward by one pixel. The
pinned libavif/libaom encoder runs at quality 100 and speed 8, and exact Pillow
frame pixels are retained in the decode manifest. A selected Rust coverage run
with these settings reached the bottom-only clipped B32 geometry predicate.

`animated_lossless_inter_420_clipped_b32x32_184x64.avif` is generated by
`scripts/generate_test_assets.py` from two deterministic 184x64 RGB textures.
Pinned Pillow 12.2.0/libavif 1.4.1/libaom 3.13.2 encode it at quality 100,
speed 0, and one thread with fixed 32x32 partitions. The final-frame inter
B32x32 leaf at `(160,0)` has a 24x32 visible extent and selects the clipped
lossless transform plan. The generator double-encodes the input, normalizes
sequence timestamps, and checks the exact SHA-256 above; exact Pillow RGB frame
hashes are retained by the decode manifest and native AVIF loop bundle.

`animated_lossless_inter_420_clipped_b16x16_60x64.avif` fixes 16x16
partitions and shifts deterministic 60x64 texture one column left between
frames. The pinned libavif/libaom encoder selects an inter B16x16 leaf at
`(48,16)` in frame 1; only its leftmost 12 columns are visible. The generator
double-encodes the sequence, normalizes its timestamps, and checks the exact
SHA-256 above.

`animated_track_only.avif` is deterministically derived from the pinned
`animated.avif` input by changing its one top-level `meta` box type to `free`
without changing the box size or any sample-table offsets. It therefore retains
the upstream BSD-2-Clause fixture license and proves that pinned Pillow/libavif
rejects an otherwise intact `avis` track sequence when item metadata is absent.
Its SHA-256 is
`45c85bd7d08261cfcb1a4563150e993d606b5e309b4767d922988ce700587fe4`.

`high_bitdepth_still_12bit_444_lossless.avif` is repository-generated from a
deterministic 16x16 RGB pattern with the pinned libavif 1.4.1 `avifenc`
toolchain and libaom 3.13.2, using `--depth 12 --yuv 444 --qcolor 100`,
`--cicp 1/13/6`, full range, speed 8, and one encoder job. The resulting AV1
frame is independently inspected as AV1 profile 2, 12-bit 4:4:4,
still-picture, one tile, all-lossless, with base qindex 0. The exact Pillow
12.2.0 RGB8 reference is 768 bytes with SHA-256
`9bb7dfcac6b47a80ec62d5d1732dc2d5954390e55c8462b312f5eb2ccb332661`.
`avifenc` and its native libraries are fixture-generation tools only; the
crate's runtime path remains pure safe Rust with no native AVIF dependency.

`profile2_identity_12bit_444.avif` uses the same deterministic 16x16 RGB
pattern but selects identity CICP 1/13/0. Its AV1 sequence declares profile 2,
12-bit, full-range 4:4:4, for which the Y, Cb, and Cr planes carry G, B, and R
samples directly. `scripts/generate_avif_profile2_identity.py` checks the
sequence declaration and Pillow 12.2.0 open/verify/load behavior. Pillow
returns 768 RGB8 bytes with SHA-256
`45114693c3cde135ae810d49fbfb8bcf54204da4b9bf11fa3094b5e177686343`.
Generate it with the pinned local `avifenc` build:

```sh
.oracle-venv/bin/python scripts/generate_avif_profile2_identity.py \
  --avifenc target/oracle-staging/avif-profile-probe/libavif-build/avifenc
```

The existing `error_sequence_identity_color_matrix_profile2_8bit.avif` is a
separate malformed case: it mutates the pinned sequence to profile 2, 8-bit,
4:2:0, identity CICP. Pillow opens and verifies its container, then reports a
decode error when loading the frame. Its generator checks that the original
sequence has 8-bit 4:2:0 color planes before applying those mutations.

`high_bitdepth_still_10bit_420_lossy_16x16.avif` is the
`high10_420_16x16` deterministic planar-input case in
`scripts/generate_av1_encoder_refs.py`. Its 16x16 Y plane and 8x8 U and V
planes use the generator's fixed arithmetic sample pattern. The pinned native
encoder uses quality 75, speed 6, one thread, full range, CICP 1/13/6, and one
tile. The fixture is one still-picture AV1 item, profile 0, 10-bit 4:2:0;
plain and instrumented encoders and two native repetitions produce identical
AVIF bytes. Generate the candidate with a clean libaom checkout at
`ad44980d7f3c7a2605c25d51ea96946949000841` and the pinned libavif checkout:

```sh
.oracle-venv/bin/python scripts/generate_av1_encoder_refs.py \
  --aom-source target/oracle-staging/libaom \
  --libavif-source target/oracle-staging/libavif \
  --output target/oracle-staging/av1-encoder-run
```

Copy `high10_420_16x16/encoded.avif` from that generated bundle into the
fixture directory. Pillow 12.2.0 decodes it to 16x16 RGB8 (768 bytes), with
SHA-256 `55820e29e26b25634c402e57e8b743bb9560666861f5ea07c14a607f31eea38f`.
The encoder collector independently pins libaom/libavif source identity and
the Pillow decoder wheel; it records native repeat parity and the exact AVIF
and pixel-plane digests in its generated index.

`high_bitdepth_still_10bit_422_alpha_lossless_16x16.avif` is the
`high10_422_alpha_lossless_16x16` case from the same collector. It has 16x16
10-bit Y, 8x16 U/V, and 16x16 alpha planes, encoded as full-range profile 2
4:2:2 plus a profile 0 monochrome alpha item. Both frame headers declare
all-lossless with base qindex 0. The collector's plain and instrumented
outputs and two native repetitions agree byte-for-byte. The AVIF input hash
is `eef1ffae7961354032dad642c1ebfc146cbcb28f7147c4dce6d9b8a4de17d392`;
Pillow 12.2.0 returns RGBA8 bytes with SHA-256
`b7731cca14b3da564883533a5cfd6ddc75aba189b60dbe7795de93711237a457`.
The complete tile, trace, replay, and plane evidence is retained under
`tests/fixtures/outputs/av1_encoder/high10_422_alpha_lossless_16x16/`.

The active 10-bit alpha stills extend that same collector with 4:2:0 and
4:4:4 color planes. The 4:2:0 case is all-lossless; the 4:4:4 case uses
quality 75 to exercise the full-resolution 10-bit alpha conversion. The
collector validates full-range CICP 1/13/6 color declarations and 10-bit
monochrome alpha items, then checks plain/instrumented output, two native
repetitions, Pillow pixels, and replayed entropy traces. Their input hashes
are `8c3cb86055e1003d91897d986995dd2ee030d0278b82d3d7c9ddc783189eb76f` and
`468bd5af2f96693582d6618572dc353cac4ee16719cebebb8e20ff7e81049548`; their
1,024-byte Pillow RGBA8 output hashes are
`a3eec8f3c1e9539ccee71ec2e9d514a46d20d0b93a7b8e5d562379ef4a794dd9` and
`02e871ddbaa576836e9d486cf36be09efce4f2fef164c7f66af2a579dfb1e67a`.
Their planes and full AV1 evidence are under
`tests/fixtures/outputs/av1_encoder/`.

`forbidden_422_partition.avif` is deterministically derived from the pinned
BSD-2-Clause `10bit.avif` input. It replaces only the 14-byte first color-item
tile with the same-length prefix from the independently pinned scalar dav1d
4:2:2 entropy oracle; no container, item, sequence-header, frame-header, or
tile-boundary field changes. Its SHA-256 is
`de34b2dc5855166b32e61aadffbead4989db3787e6db26fab77ae7129ec93381`.
Pillow opens and verifies the complete container, then rejects frame
materialization because the entropy selects a partition forbidden for
vertically unsampled chroma. The derivative retains libavif's BSD-2-Clause
fixture license.

`multitile.avif`
(`28bd09d7f17a15fcf3457eb21d2bebc36054718b20338191793e2d5faa61f253`)
is deterministically generated by
`scripts/generate_test_assets.py` with the pinned Pillow 12.2.0/libavif
1.4.1/libaom 3.13.2 oracle. It is a 256x128 RGB source encoded with two tile
columns, quality 75, speed 6, and one encoder thread.
`multitile_color_split_groups.avif`
(`654ef88dd8ebc71a554c03f721ad97581fcf2d2df0d825020d267939eb8f5e34`)
uses `scripts/generate_avif_color_tile_split_fixture.py` to preserve that
source's frame header and tile payloads while delivering each color tile in
its own OBU tile group. Pinned Pillow 12.2.0 produces the same image metadata
and RGB pixels as `multitile.avif`; the separate groups exercise incremental
color-tile state retention.

`multitile_skipped_cdef_color_alpha.avif`
(`36c83e5e5104e74a8b3474b3ee313645a7bf86381d1578087e5cef3453d289eb`)
is an original 128×64 RGBA raster with every component equal to 128.
`scripts/generate_avif_color_tile_split_fixture.py --profile cdef-skipped-square64`
builds its two-column color and monochrome-alpha container using the pinned
Pillow/libavif/libaom oracle, then replaces the tile payloads with independent
libaom default-CDF entropy construction from
`scripts/avif_fixture_oracle/encode_skipped_square64.c`. Each tile selects an
unpartitioned 64×64 DC-predicted block with skipped transforms. The frame
headers retain enabled CDEF syntax with zero strengths; skipped blocks omit
the CDEF region index. The generator checks identical repetitions, container
extents, frame controls, and Pillow's exact 32,768 RGBA bytes (SHA-256
`67d47633eeb4ab9211bfaddc84e6d5c09a958588867dcdc4b2169ad74b73fa0e`).
The original raster and generated file use the repository's MIT/Apache
licensing. This is an ordinary public parity input without fault injection.

`invalid_tile_size.avif`
(`82a07a29a8631d60a2d83bd9973afac0e494882580d3542bb953875817eb0f67`)
differs only in the most-significant byte of the first tile's little-endian
`tile_size_minus_1`; the declared tile consequently crosses the frame OBU
payload. Both generated files are covered by this repository's MIT/Apache
licensing, while their encoded AV1 behavior is verified against the pinned
oracle.

`coverage_square16_chroma_smooth_vertical_01.avif`
(`66ed5a57015730ce80eb529483102fbe781d1d073e3443fa041177e38be8e380`) is a
repository-generated 32x16 8-bit 4:2:0 quality-76/speed-0 witness. Its clipped
root split has origin/following Square16 leaves; the following chroma selects
SmoothVertical mode 10 with ADST-DCT TX8x8 U/V and non-empty AC. The input-only
campaign report is
`tests/fixtures/outputs/av1_search/coverage_square16_following_chroma_smooth_vertical_campaign_01.json`
(`d7155bcb67dd01c23ec7ddf7286dbf5530547d35ad8acf72d55ae907645996d9`); it
searched 100 candidates across 10 families, qualified 9, promoted
`SV16-F06-N03`, and invoked no repository Rust. The fixture was double-encoded
and its item, trace, Y/U/V, and Pillow RGB outputs were checked for
determinism.

`coverage_square16_chroma_smooth_01.avif`
(`1d663c7f7e3d65f12062124880ab1ae4d3eee5eaada570d9a38504aa58093080`) is a
repository-generated 32x16 8-bit 4:2:0 quality-76/speed-0 witness. Its clipped
root split has origin/following Square16 leaves; the following chroma selects
Smooth mode 9 with ADST-ADST TX8x8 U/V and non-empty AC. The input-only
campaign report is
`tests/fixtures/outputs/av1_search/coverage_square16_following_chroma_smooth_campaign_01.json`
(`e6fd49048f42a9ed36ea54d527a70d1a67cc40908a3db2d6767905bad77769e7`); it
searched 100 candidates across 10 families, qualified 10, promoted
`SS16-F06-N01`, and invoked no repository Rust. The fixture was double-encoded
and its item, trace, Y/U/V, and Pillow RGB outputs were checked for
determinism.

`portable_lossless_a.avif`
(`ccc84752237af0549d7310af7a5b948435b07c78f9b20c322240a18f1667c411`)
and `portable_lossless_b.avif`
(`4d319cc51aee3d79d5fb8a7c1fba1b42b42303ffa8baef1f7a1511fa4ec031ee`)
are deterministically generated by `scripts/generate_test_assets.py` from
constant 4x4 RGB inputs `(17, 91, 203)` and `(199, 37, 83)`. Encoding is pinned
to Pillow 12.2.0, libavif 1.4.1, libaom 3.13.2, quality 100, speed 8, one
thread, 4:4:4 subsampling, and disabled autotiling. The generator encodes each
fixture twice and refuses differing bytes.

`portable_lossless_monochrome_1x1.avif` is generated as a one-pixel Pillow
`L` image with sample 127. The pinned AVIF encoder uses quality 100, speed 0,
one thread, 4:0:0 subsampling, CICP 1/13/6, and disabled warped motion. The
pure-Rust monochrome reconstruction crops the coded lossless block to this
nonzero sub-4-pixel canvas; Pillow's exact RGB output is `7f7f7f` (SHA-256
`4977021028c4a74c2bca8061786cd88afc96837ef6007265acc4f8cda2d945e0`). The
generator double-encodes the input and checks its listed hash.

`portable_lossless_monochrome_limited_16x16.avif` uses the
`mono_limited_lossless_16x16` case in `scripts/generate_av1_encoder_refs.py`.
The deterministic Y plane is mapped to limited-range codes, then encoded as
lossless 8-bit 4:0:0 with CICP 1/13/6. The collector verifies the limited-range
sequence flag, plain/instrumented byte equality, repeat encoding, replayed
entropy, and Pillow RGB output. Pillow's 768-byte result has SHA-256
`81a84ee60e5a6b01ffeb8007cf087a5956fbb03da7e734b6f2326bb801750150`; the
complete input planes and trace evidence are under
`tests/fixtures/outputs/av1_encoder/mono_limited_lossless_16x16/`.

The companion 10-bit and 12-bit RGB fixtures use the same full-file collector
to check depth-scaled studio ranges. Three paired RGBA fixtures add full-range
8-bit, 10-bit, or 12-bit alpha planes. For RGB, pinned libavif normalizes and
rounds limited-range luma; for RGBA it uses the limited-range I400 matrix for
color while converting alpha over the full sample range. The five additional
Pillow output hashes are `756fe3346d631980ef8ca651824dc321b5ec3a35deab90e8a43eca0b2945393c`,
`e1cd06c4130804d0274ee4d76338cd4cab94f09195cd82515b13443ab932d57e`,
`aff3a7277b7b21f5ec06e4f4d85b1309eee554a086042cc52da99e2215f8f578`,
`ed3fe784a495990c92c60ed29c188917e0c6c68c8672f3770e406b1fd7691dd9`, and
`ff702f5efd66fbe66bdbf67c5f683ac042ab5e3ae4067a40a3f6e14f8760ca09`.

`portable_lossless_monochrome_limited_bt2020_pq_16x16.avif` changes the
limited-range RGB witness to BT.2020 primaries, PQ transfer, and BT.2020
non-constant-luminance matrix coefficients (CICP 9/16/9). Its Pillow RGB8
output is byte-identical to the BT.709/sRGB/BT.601 case, confirming that the
monochrome luma conversion does not depend on the chroma matrix.

`portable_i444_quality100_64x64.avif` uses deterministic RGB pixels
`((4x) mod 256, (4y) mod 256, (2(x+y)) mod 256)` over a 64×64 canvas. Pillow
12.2.0's AOM encoder uses quality 100, speed 0, one thread, 4:4:4
subsampling, fixed 64×64 partitions, CICP 1/13/6, intra DCT only, and disabled
CDEF, restoration, AQ, and delta-Q. The generator encodes it twice and checks
the pinned input hash. The decoder's RGB bytes match Pillow exactly: the
12,288-byte output has SHA-256
`5e74b862ca69314d8ae9a3aa4b8f3af5651dfbd238ce922a7973c193e338f64a`.

`portable_lossless_filmgrain_i444_64x64.avif` uses deterministic random RGB
pixels from seed `0x21108` over a 64×64 canvas. Pillow 12.2.0's AOM encoder
uses quality 100, speed 0, one thread, fixed 64×64 partitions, DCT-only
transforms, CICP 1/13/6, and its film-grain test mode. The AV1 item is 8-bit
full-resolution 4:4:4 and all-lossless. Its 12,288 Pillow RGB bytes have
SHA-256 `44f85ee642e036ae8646b40b2a71643f1e74a27d2a5a6af719c938d8aba2dceb`.

`portable_lossless_filmgrain_420_chroma_from_luma_64x64.avif` starts from
deterministic random RGB pixels (seed `0x4672`) encoded losslessly as I420 by
Pillow 12.2.0's AOM encoder with film-grain test mode. Its AV1 frame header is
rewritten to set `chroma_scaling_from_luma`, omit the independent U/V point
tables and their scaling parameters, and preserve the original tile payload. A
valid padding OBU keeps the AV1 item extent unchanged. The pinned generator
verifies the syntax with the OBU inspector, repeats the mutation deterministically, and
checks Pillow's RGB output. The 7,715-byte AVIF has SHA-256
`80c3a157f32f190a787588c48232113c4bbe915abd80812c5e0d1d62fd51c893`; its
12,288 Pillow RGB bytes have SHA-256
`ee0b8557ef74499434382e1776a76492adc857b142f0481f4ef29c4e5a9d82cd`.

`portable_lossless_filmgrain_420_zero_y_points_64x64.avif` uses the same
pinned lossless I420 source. Its AV1 header keeps `apply_grain` set, sets
`num_y_points` to zero, and clears chroma-from-luma; 4:2:0 syntax therefore
omits U/V points, all AR coefficients, and U/V scaling parameters. The
generator preserves the key-frame prefix and tile payload, verifies the
rewritten syntax with the independent OBU inspector, and pins repeatable
Pillow 12.2.0 output. The 7,715-byte AVIF has SHA-256
`c76ede4ac45599b182d023f8425744f09dc27e4011742d6295d3ec50e172ed9f`; its
12,288 Pillow RGB bytes have SHA-256
`5d35ab50438b9f1536ef7de3cbca8ac3d01360efa9d77446dfe0d69786d2fb62`.

`animated_filmgrain_reference_reuse_i444_64x64.avif` starts from a pinned
two-frame Pillow 12.2.0 I444 sequence, then updates the inter-frame AV1 header
to reuse film-grain parameters from reference slot 0 with a new seed. The
generator validates the inter-frame syntax with the independent OBU inspector,
preserves the sample extent with a valid padding OBU, normalizes BMFF
timestamps, and checks Pillow's decoded RGB bytes for both frames. Its 18,004
bytes have SHA-256
`bd947085ed6437edfd50a97b43506cc56e96af8b5450a8ef5cb8289b8ec62b34`; frame
RGB hashes are `a4e4fa07369777b09a15c680d4812441a5509a7662740c039127a998bdf3d9ab`
and `d9902750b3685e4c451df39a76c6f18848dc7bb6cb01055174d34d8d0def0052`.

`portable_lossless_filmgrain_monochrome_64x64.avif` uses grayscale RGB pixels
uses `scripts/generate_avif_monochrome_filmgrain.py`, seeded grayscale RGB
pixels, and Pillow 12.2.0's AOM encoder with quality 100, speed 0, one thread,
4:0:0 subsampling, fixed 64×64 partitions, DCT-only transforms, CICP 1/13/6,
and film-grain test mode. The AV1 item is 8-bit all-lossless monochrome with
luma film grain. Its 6,210 AVIF bytes have SHA-256
`0bc3fe81f320d7f55853d53ec7b7fa20f099bf8af7e5e7ccbaa69556ccd980d4`; its
12,288 Pillow RGB bytes have SHA-256
`853bfb557b4ab4960d708f4ecfeda145ed9feab8c987214d82ee6a20665e93f4`.

`icc_profile.avif` is deterministically generated from a 4x4 RGB still using
the ICC profile in the existing `png/iccp.png` source and CICP 1/13/6 values
matching the portable AV1 subset. The generator double-encodes the still,
checks its pinned input hash, and reopens it with Pillow to verify the profile
survives. The AVIF decode parity row compares Pillow's RGB pixels and retained
profile metadata.

`icc_ricc_type.avif` changes only the `colr` property type from `prof` to
`rICC` in that complete AVIF file. The pinned Pillow oracle accepts it, returns
the same profile payload, and produces the same RGB pixels. This row covers
the second ICC property spelling without claiming the marker payload is a
color-conversion profile.

`icc_mdcv.avif` starts from the same ICC still and inserts a non-essential
primary-image `mdcv` property with BT.2020 primaries and a D65 white point.
The generator updates the property association and file-based item extent,
then checks the pinned SHA-256. Pillow accepts the result, preserves the ICC
profile, and returns pixels identical to the control. Pillow does not expose
`mdcv` in `Image.info`, so its ten exact 16-bit coordinate and 32-bit luminance
fields are asserted against the ISO/IEC 23008-12 property definition while the
pixel/profile assertions retain Pillow provenance.

`icc_mdcv_trailing.avif` appends one bounded byte to that associated property.
Pillow 12.2.0 still verifies and decodes it to the same pixels and profile.
The Rust parser retains the fixed `mdcv` fields and ignores that bounded tail,
matching the oracle's behavior without changing image samples.

`portable_lossless_420_a.avif`
(`640d19800ff27dbd1cd28e881736e923a48eb46e8223bed9d52bfb624b85e6a7`),
`portable_lossless_420_b.avif`
(`bd6427ce4848cb4d65f83b1621ffda46a4614e6a8b316998b69234298077ffba`),
`portable_lossless_420_8x8_a.avif`
(`21d453da436be1bbb47238e35d919499c7814a2a8073550b9ae958cafe78d15e`),
and `portable_lossless_420_8x8_b.avif`
(`311de615cc4f0f7cbd9f6c136170c383f5263659c07dcaa8fabb1877f87f415e`)
use the same two constant RGB sources and pinned settings with explicit 4:2:0
subsampling. The 4x4 and 8x8 pairs prove one chroma transform per plane,
declared subsampled plane geometry, and private I420-to-RGB materialization.
Their same-source 4:4:4 fixtures are the adjacent controls.

The quality-99 files below use the same pinned, double-encode-verified path
with explicit 4:2:0 subsampling. Each input has one coded 8x8 luma transform
and one 4x4 transform per chroma plane. Gray 127 and 129 skip every residual;
gray 126 and 130 add one direct-token DC-only luma residual; gray 125 and 131
use token 15 with Golomb extensions zero and one; and gray 124 and 132 use
Golomb extension nine and final token 24. Gray 123 and 133 extend the same
DC-only class through final tokens 32 and 33. An exhaustive gray-0-through-255
sweep proves the same coefficient rule from final token 8 through 1,047.
Gray 0, 64, 122, 134, 192, and 255 retain its endpoints and interiors.
Deterministic coded-tile mutations extend that same closed class through the
ten-to-eleven-bit Golomb boundary, both coefficient clamps, and a non-clamped
20-bit token-mask wrap. Gray 128 changes the luma predictor and remains a
separate non-portable control.

| Fixture | Source RGB | SHA-256 |
| --- | --- | --- |
| `portable_lossy_420_q99_gray_126.avif` | `(126,126,126)` | `f82b264295ffb7ea9e357a352e674200ed89138a182b0de7c4002fbc55fade4d` |
| `portable_lossy_420_q99_8x8_gray_126.avif` | `(126,126,126)` | `90d415cfd1292d211e6b3874837853f8a7690f27de93c28a133e18f4af986ad1` |
| `portable_lossy_420_q99_gray_127.avif` | `(127,127,127)` | `c232a943aef1ec71422567e9c00a137a70576c63a383621a4417a9637ee08732` |
| `portable_lossy_420_q99_8x8_gray_127.avif` | `(127,127,127)` | `947d6326cc09f88e50e0aba60d9cb468970d793ac323003a5d2452934998dcf1` |
| `portable_lossy_420_q99_gray_129.avif` | `(129,129,129)` | `79e3d72995eb382d5462e4309fec24e37111cd039a10bc9b28bd370b9b26fa64` |
| `portable_lossy_420_q99_8x8_gray_129.avif` | `(129,129,129)` | `ca48aaddde1310eecde25c24c24314089a5e62164c8dbd36b0c64b2ef9812507` |
| `portable_lossy_420_q99_gray_130.avif` | `(130,130,130)` | `cf98497c2b678d67bbb9327f7816b9ef9d3d186ffee51b24ee10ec50e8e8d776` |
| `portable_lossy_420_q99_8x8_gray_130.avif` | `(130,130,130)` | `a579a6a3f85a4b5574d237c3c06f1cff79404bb565ece13e099c3611bac7b39f` |
| `portable_lossy_420_q99_gray_0.avif` | `(0,0,0)` | `7f1485129fd93e4318cf21bcf59934963c1a84b3bcb0d74f3e7555b3bad20b38` |
| `portable_lossy_420_q99_8x8_gray_0.avif` | `(0,0,0)` | `75df02eb1a44eb478b17910a79179dcc563a4b1b72db2b6b25d229ba377320eb` |
| `portable_lossy_420_q99_gray_64.avif` | `(64,64,64)` | `6f4d9be7282279fdaaf38c1a464c49e44fb1373be0cfb83bb632f85167d1022e` |
| `portable_lossy_420_q99_8x8_gray_64.avif` | `(64,64,64)` | `350a8eca70ae23d2e4981c3a4f0e31c5edf060e6da940c56750fa5b4dbed3ff8` |
| `portable_lossy_420_q99_gray_122_control.avif` | `(122,122,122)` | `17c312d10c6cd7ecd6a1bf1fb6b1bfff07aa970ff2ff3e722f2dd984c714a80a` |
| `portable_lossy_420_q99_8x8_gray_122_control.avif` | `(122,122,122)` | `7163cc6aee6597f1792a6b963fb2777758fdbe7096bcfe5712df0e150f5c4d49` |
| `portable_lossy_420_q99_gray_123_control.avif` | `(123,123,123)` | `1e0f1f2ae4da78ca2cee5af734916106bb822d2d780f44111f257beed7c05890` |
| `portable_lossy_420_q99_8x8_gray_123_control.avif` | `(123,123,123)` | `842883fdf557bb56f02454da1f5e5fe91a87f4afa21b87ba4155abd51396687f` |
| `portable_lossy_420_q99_gray_124_control.avif` | `(124,124,124)` | `f2c1d46376a93d91baa784dfd69615bb1d334471ac997515612366085e2cb781` |
| `portable_lossy_420_q99_8x8_gray_124_control.avif` | `(124,124,124)` | `4d1fc957ddb0e368fe179d7f93c8d64afb01bb24f3b876bd6c8cc7d2b337c033` |
| `portable_lossy_420_q99_gray_125_control.avif` | `(125,125,125)` | `43e09f9447cb94aaa979956887dad091ec1f630f6dab5e33eb68dfbc989537fa` |
| `portable_lossy_420_q99_8x8_gray_125_control.avif` | `(125,125,125)` | `70b97a8ecfdca48dadf67624fe03db3fa4672dfd921f11f7da1396c393c0b7be` |
| `portable_lossy_420_q99_gray_128_control.avif` | `(128,128,128)` | `9c89f8ca4506897154f40d89c62cd7beb5b810f2d70b0381c419142f19a7f02f` |
| `portable_lossy_420_q99_gray_131_control.avif` | `(131,131,131)` | `ff49a749f44a139b697671a2c21032ff0a3298a0fb749ed7d9a7c193fbbeacfb` |
| `portable_lossy_420_q99_8x8_gray_131_control.avif` | `(131,131,131)` | `f238b91f4c6b225691933fc5a46a1c2b42dd2460bdc3567a92dcd907fb8ac7bb` |
| `portable_lossy_420_q99_gray_132_control.avif` | `(132,132,132)` | `98ee27816a74ee14b345e4a3c39856a328f18d77c7bbba95e40630b335bf44dd` |
| `portable_lossy_420_q99_8x8_gray_132_control.avif` | `(132,132,132)` | `adeca8ec9e6cbe47fc2a7a046d631be33772e385f868d7d943d99175e6535c32` |
| `portable_lossy_420_q99_gray_133_control.avif` | `(133,133,133)` | `536cd711fe24a5c63489ecefc3f53d3a732aa606ebb3cb94a00789a5b4d9798d` |
| `portable_lossy_420_q99_8x8_gray_133_control.avif` | `(133,133,133)` | `6abbf10ccf33392f217a6db1e1b9a66cd6b0cea9e95d06845252a0389beaa029` |
| `portable_lossy_420_q99_gray_134_control.avif` | `(134,134,134)` | `88a3a51f1107ca20a77bd70db89891e9431dd932914a2e4494d017e11018ca68` |
| `portable_lossy_420_q99_8x8_gray_134_control.avif` | `(134,134,134)` | `65fe71943e62a346b20249a420f323dab9601ba99cbb5bf9782074d0d16a6331` |
| `portable_lossy_420_q99_gray_192.avif` | `(192,192,192)` | `8b517a977c091cbe56ec1997907c27706ba9bdd6c660e646d49df8a6dd16677f` |
| `portable_lossy_420_q99_8x8_gray_192.avif` | `(192,192,192)` | `5edccf35d44da2f17d41b106681b7535f264863d44e26c0c1e16d1a67bd6e8f9` |
| `portable_lossy_420_q99_gray_255.avif` | `(255,255,255)` | `e1c3b423417b18795071054196ce1f95e6cf19a841a632c616ab3a96969d6e3f` |
| `portable_lossy_420_q99_8x8_gray_255.avif` | `(255,255,255)` | `cf7660907939a12972c8ba2def48cb0b8b6014cc24bd75ab82cd0ffe1162f6c5` |
| `portable_lossy_420_q99_token_1048_control.avif` | gray-zero AV1 item offset 28, `0x42` to `0x43` | `1097067dca85e499768a40e15232dce3602afbb1cabcbf485e8a14bf83e9bb73` |
| `portable_lossy_420_q99_token_2061.avif` | final ten-bit Golomb token | `bc97b1f2ca96f6072239101e096e1b18fe87cb6ecf13b48188b37b52a50d761e` |
| `portable_lossy_420_q99_token_2988.avif` | eleven-bit Golomb interior | `0153d56609f86e637159836af94d103523853c9002c92dc7411925d97a919250` |
| `portable_lossy_420_q99_token_7940.avif` | positive coefficient clamp | `503ca52689395ec769b5453f7a30b4340f4234132338b1dd16e6a945ab34c37a` |
| `portable_lossy_420_q99_token_7764.avif` | negative coefficient clamp | `15822dfb32fea6432adf1c7ddb9ea648dd6d2e028b12c9f117c6031420760367` |
| `portable_lossy_420_q99_token_2097724_masked_572.avif` | raw token 2,097,724 masked to 572 | `d492c364655cad1f950bd37fbf63b1b9eecc42dff0bae3f95d2d15d8f0f86f63` |
| `portable_lossy_420_q99_eob_bin_control.avif` | gray-126 AV1 item offset 24, `0x72` to `0x73` | `0ff53f82624ab0c9e213a7398251aef6d14af7a91ca3a31ba757d1fe36f8cdea` |
| `portable_lossy_420_q99_eob_base_control.avif` | gray-126 AV1 item offset 25, `0xe1` to `0x1e` | `ebf00b9dc914982bd698af0413a0e26a6a849208871abbeccc6789541efb08f5` |

These three legal lossy controls exercise the first non-DC 8×8 coefficient
classes admitted by the safe Rust decoder. They are deliberately different
from the rejected byte mutations above: each has an independent Pillow pixel
oracle and remains a normal active matrix row.

| Fixture | Source pattern | SHA-256 |
| --- | --- | --- |
| `portable_lossy_420_q99_rampx_eob5.avif` | 4×4 luma ramp, `96 + 8*x`, EOB-bin five | `24ad87c6b33fc5178d3ff662bcb84d5893dfef0fd46ed738d59663153a866262` |
| `portable_lossy_420_q99_rampy_eob6.avif` | 4×4 luma ramp, `96 + 8*y`, EOB-bin six | `581705cc684dc5154896387dd9bbaac6b3c407af3ad46bf21ae505196a23c73c` |
| `portable_lossy_420_q99_8x8_diag_eob6.avif` | 8×8 diagonal impulse, `129` on the diagonal and `127` elsewhere, EOB-bin six | `006c41743bcf6b1990981cabc01f301b97ea8424c13e6c9e837375ab2792eb3c` |
| `portable_lossy_420_q99_luma_eob_bin2_eob3.avif` | 4×4 luma impulse at `(3,0)`, legal TX8×8 EOB-bin two / EOB three / EOB-base zero | `1e8f492d54742c0662595952247b15cd98054d4f6e11346041d1d7db4cf5b34` |

The complete scalar traces, extracted AV1-item hashes, reconstructed planes,
and Pillow RGB hashes are pinned in `docs/avif.md` and the indexed
`tests/fixtures/outputs/av1_reconstruction.json` oracle. Its
`av1_reconstruction.part-*.json` sidecars hold the case records so each
tracked blob stays below common hosting limits; the harness joins them before
validation.

The fixture `coverage_r32x16_filter_intra_tx8x8_01.avif` is an origin
Horizontal32x16 TX8x8 split witness. Its `Post-filterintramode[0/0]` trace
entry is dav1d's filter-intra-disabled sentinel; it must not be described as
filter-intra mode 0. Following-leaf split filter-intra remains an open target.

The fixture `coverage_square16_filter_intra_mode0_01.avif`
(`2fb3de2676b560d379d05782b3e57c7af028b2fdac0350364389b3f9ceb77bcc`) is a
16x16 8-bit 4:2:0 origin `Square16` witness. It selects
`FILTER_PRED[13/0]`, an unsplit TX16x16 luma transform, and TX8x8 U/V
transforms. The pinned trace has partition range `62320` and 1,116 entropy
operations; the safe-Rust reconstruction contract matches the exact Y/U/V
planes and Pillow RGB8 output. Its Pillow RGB SHA-256 is
`4090aed7681e287536328b3ec8ee9235c8e32979b8a249824d258fd57145b008`.
This is bounded origin Square16 evidence, not general filter-intra support.

The fixture `coverage_vertical8x16_filter_intra_mode0_01.avif`
(`da511e016e1e8720cb21af34b4cf41001a97af0f0380576dc47355dcd630f39a`) is an
8x16 8-bit 4:2:0 origin `Vertical8x16` witness. It selects
`FILTER_PRED[13/0]`, an unsplit TX8x16 luma transform, and TX4x8 U/V
transforms. The pinned trace has partition range `42232` and 584 entropy
operations; the safe-Rust reconstruction contract matches the exact Y/U/V
planes and Pillow RGB8 output. Its encoded-item SHA-256 is
`e86cc0fdfc27ec55e542a581bb22b4c619f5dfac793593ec7b276a13df6d8224`, and its
Pillow RGB SHA-256 is
`82b2100ac5f6f02e88ea931a90b2abab261b7486209ee4f63c538464c52b5c30`.
This is bounded origin Vertical8x16 evidence, not general filter-intra
support.

The fixture `coverage_vertical8x16_filter_intra_mode1_01.avif`
(`7c04bf5be19e0e1acf757dbdda04b3fd48419a2df1dcf7a12871cdefbce99917`) is an
8x16 8-bit 4:2:0 origin `Vertical8x16` witness. It selects
`FILTER_PRED[13/1]`, an unsplit TX8x16 luma transform, and TX4x8 U/V
transforms. The pinned trace has partition range `42232` and 559 entropy
operations; the safe-Rust reconstruction contract matches the exact Y/U/V
planes and Pillow RGB8 output. Its encoded-item SHA-256 is
`2ce5e66bfed511611e28f06c13f3014e6863e026b9e22ea6fd2c2145e36adbde`, and its
Pillow RGB SHA-256 is
`6051c012bac9735f10fb18bfe680fc9e3582ef6acfaa295a028f02ead7a642fe`.
The reconstructed Y/U/V plane SHA-256 values are
`d5f1f32b7f3bc6d635a7a9bd89b9efa59670ffb723b6f5ff8f7d65a0eca940c9`,
`f3238ddee04bccf67e555675f978da2a2cd114f0eac6cf751f355763e84dde85`, and
`ce452bca9cac19f45e3e2257f2ae531197097512d1bd6f76cb914c7eb34f9615`.
This is bounded origin Vertical8x16 mode-1 evidence, not general filter-intra
support.

The fixture `coverage_vertical8x16_filter_intra_mode2_01.avif`
(`a9a4a6ccb31aaed0164ce68ca9988fab9d8e8b0407e3e4e93de5dd0d53b48c41`) is an
8x16 8-bit 4:2:0 origin `Vertical8x16` witness. It selects
`FILTER_PRED[13/2]`, an unsplit TX8x16 luma transform, and TX4x8 U/V
transforms. The pinned trace has partition range `42232` and 578 entropy
operations; the safe-Rust reconstruction contract matches the exact Y/U/V
planes and Pillow RGB8 output. Its encoded-item SHA-256 is
`f275334de5da1405864b4570d137fe24b05d7e4d07a569cea361fa5833b37f8f`, and its
Pillow RGB SHA-256 is
`5bf4eb2849056ecbba6885bbab1852d39449dec94909f05f6b26657b74104b8d`.
This is bounded origin Vertical8x16 mode-2 evidence, not general filter-intra
support.

The following three repository-generated fixtures form one bounded 8x32,
8-bit 4:2:0 following-Vertical8x16 luma smooth family. Their lower leaf uses
qindex 16/qcat zero, matrix 10, an unsplit TX8x16 DCT-DCT luma transform, and
skipped TX4x8 U/V transforms. Generation is pinned to Pillow 12.2.0,
libavif 1.4.1, libaom 3.13.2, and scalar dav1d 1.5.3; each promoted input was
double-encoded and its AV1 item, trace, decoded YUV, and Pillow RGB output were
checked for determinism without invoking repository Rust.

| Fixture | Lower luma mode | Fixture SHA-256 | Campaign report and SHA-256 | Pillow RGB SHA-256 |
| --- | --- | --- | --- | --- |
| `coverage_vertical8x16_following_luma_smooth_01.avif` | Smooth (9) | `54fcb046a23c062c08a7a1ed75637bb43bc497bcea59a8ae10db8c093a8d8d24` | `coverage_vertical8x16_following_luma_smooth_campaign_01.json`, `ee10e865a3acfcb2d716af436d1501f51896ae4e70e7fbe2342ace515211364c` | `6a6ed4c75f6257de2ae215a5fa812f323ad28391de8dfba0627e2a45ac1cece5` |
| `coverage_vertical8x16_following_luma_smooth_vertical_01.avif` | SmoothVertical (10) | `6e7c4d5abba0c58777ffd3203889aae5f4a189fcdf7e0eb07fbab85436cb12d6` | `coverage_vertical8x16_following_luma_smooth_vertical_campaign_06.json`, `977167794eaae213b6ae5a9bf39a7495c9c36b5ee06331c7dabbcf4172d99799` | `f1abc727013b268d1ba37d61091868c50889462a8c43c769117ae92931992f46` |
| `coverage_vertical8x16_following_luma_smooth_horizontal_01.avif` | SmoothHorizontal (11) | `ffe831f5142199707be7f6b9596219aa646423f123f8282ae03d90aef4f2402e` | `coverage_vertical8x16_following_luma_smooth_horizontal_campaign_01.json`, `6199718ea2f3f4e579feb0319000db455c70022e039071f90c8f7682394b5422` | `e443dfd18a60122c283ea8bf277d64527380a63e2742eff4c7bf19fa037214b6` |

This fixture set proves only the declared smooth-family class; it is not a
general AV1/AVIF or performance claim.

The following four repository-generated fixtures extend the same 8x32,
8-bit 4:2:0 following-Vertical8x16 topology to the lower leaf's 4x8 U/V
planes. The lower luma leaf remains DC with two skipped TX8x8 DCT-DCT
children. Both chroma planes carry non-empty TX4x8 residuals: DC uses
DCT-DCT, Smooth uses ADST-ADST, SmoothVertical uses ADST-DCT, and
SmoothHorizontal uses DCT-ADST. Generation is pinned to Pillow 12.2.0,
libavif 1.4.1, libaom 3.13.2, and scalar dav1d 1.5.3; each 100-candidate
campaign retained deterministic AV1 item, trace, decoded YUV, and Pillow RGB
evidence without invoking repository Rust.

| Fixture | Lower chroma mode / transform | Fixture SHA-256 | Campaign report and SHA-256 | Pillow RGB SHA-256 |
| --- | --- | --- | --- | --- |
| `coverage_vertical8x16_following_chroma_dc_01.avif` | DC (0) / DCT-DCT | `7ff17319c3b2e5c7306908618ecaaa823c734391af286b81e0a68db6af01d35a` | `coverage_vertical8x16_following_chroma_dc_campaign_01.json`, `e16c6410c48f4b22bb884cdad4b609431ef6604359714bb63bb1e0ccee93d282` | `46cd23709b17164ec6ae3017f5f9c5f5f499fd8d1584a7ad0221f4b957ed8bb6` |
| `coverage_vertical8x16_following_chroma_smooth_01.avif` | Smooth (9) / ADST-ADST | `be6f22b1988333c303f63a7dddb3d5bbade9211bbfc519c9be51db3b510d0ccd` | `coverage_vertical8x16_following_chroma_smooth_campaign_01.json`, `22734ed045a34209876ff4ce984b4f9209cd28e1df2d07deffbd74a100dd7432` | `f8185c7fbfe11910c203c94003e30a02dc976320bd820c75a1e0708d1a82eb18` |
| `coverage_vertical8x16_following_chroma_smooth_vertical_01.avif` | SmoothVertical (10) / ADST-DCT | `a29134747ab2e6cb9602b06398fa9b48f7f4bdb2e7f0193e568d0474f54a782c` | `coverage_vertical8x16_following_chroma_smooth_vertical_campaign_01.json`, `e510f2669a6dd6f8ed21555d17fd097e6e238c8ebf4df196907476e87b65dfd0` | `ff8af413ad18331674a069195872a5e25a2545a05459332312c156d6c681248a` |
| `coverage_vertical8x16_following_chroma_smooth_horizontal_01.avif` | SmoothHorizontal (11) / DCT-ADST | `c3dd3717c4c639b3558b87344532650caf7f6b4d0f8c6e030250aef7efe3ccee` | `coverage_vertical8x16_following_chroma_smooth_horizontal_campaign_01.json`, `c834f57c1c1cf320b549fe6c4fc81a3631ca883fa84d3a90f7ed1a2aacf0e00e` | `f07fc781bd26776947d6d73abc5d4f1b50d9c3cdac661de79efecc56c9b5271a` |

The varying upper-leaf chroma rows make the lower predictor's true top edge
observable: it is row 7 of the 4x8 upper plane, not row 6 or a synthetic
rectangular endpoint. Because the lower leaf has no left neighbor, DC is
one-sided top DC and the smooth modes repeat the top-left sample for the
missing left edge. These 32-pixel predictors intentionally use checked fixed
arrays; this evidence does not claim that SIMD setup would improve them.

`coverage_i444_palette2_square8_four_leaves.avif`
(`7d13f753585fd646426ed1d8900c38ea95c7b06ada9c9204e4b8e6d47e1e4a56`)
is a deterministic 16x16, 8-bit, single-tile, lossy 4:4:4 witness generated
through the pinned Pillow 12.2.0/libavif 1.4.1/libaom 3.13.2 environment
with quality 99, speed 8, one encoder thread, and autotiling disabled. Its
4x4 checker cells alternate RGB `(17,91,203)` and `(0,255,0)`. The coded
frame selects four terminal 8x8 leaves; each leaf carries paired Y and UV
palette size 2 syntax, and later leaves reuse the spatial palette cache. It
proves only this narrow palette class: it does not establish Y-only or
UV-only palettes, palette sizes above two, other subsampling or bit depths,
cropped/non-8x8 leaves, or multi-tile support. Its Pillow RGB SHA-256 is
`ae90d60419a44e909e312e762e05d6f73d70d32c43366eb8885aabe4d2c7725b`.

The asymmetric-gradient public family also includes seven active 8-bit,
full-range, lossy 4:4:4 witnesses at quality 99. They are intentionally
different frame dimensions, so the public decoder is checked against the
same safe Rust materializer across narrow, wide, tall, and square rasters.
Each row has an independent Pillow RGB oracle; the private reconstruction
oracle additionally pins the scalar dav1d Y/U/V planes, entropy operations,
and the bounded partition records that its instrumentation observes. These
fixtures prove these exact rows only; they do not close the broader AV1
partition, predictor, high-bit-depth, color, tile, or encode roadmap gaps.

The adjacent `coverage_i444_rect_01.avif` and `coverage_i444_rect_02.avif`
witnesses hold the 16x16 split-root/four-leaf geometry constant while changing
the gradient orientation and residual sentences. `rect_01` has a pinned
499-operation trace; `rect_02` has 553 operations and a filter-intra leaf. Both
are full-resolution lossy 4:4:4 cases with exact dav1d Y/U/V and Pillow RGB
references. Their evidence is intentionally bounded to these observed syntax
classes, not a claim of general I444 support.

The `coverage_i444_square8_01.avif` through `_10.avif` batch keeps that
16x16 full-range 4:4:4 split-root/four-Square8 topology fixed while varying
all four row-major leaves. Every leaf uses DC chroma and TX8x8 DCT-DCT on
all coded planes. The batch proves origin, left-neighbor, top-neighbor with
top-right extension, and combined top/left/upper-left contexts; luma covers
DC, Vertical, Horizontal, and Smooth, including the zero-delta directional
angle symbol. Cases 05-10 enable screen-content tools and consume only
palette-use false: all four UV decisions are false, while luma decisions are
present only on palette-eligible DC leaves. No fixture claims a nonzero
palette, palette cache entry, intra block copy, CFL, non-DC chroma,
filter-intra selection, transform split, or non-DCT transform. Exact
partition ranges, adaptive entropy operations, coefficient endpoints, Y/U/V
planes, and Pillow RGB bytes are pinned by the reconstruction oracle.

| Fixture | Fixture SHA-256 | Pillow RGB SHA-256 |
| --- | --- | --- |
| `coverage_i444_square8_01.avif` | `29a9a67c2719046b5d9aa6ebe9e6666377c298a1f60e2f1b4cbf56aa757d0d61` | `e2d9ba964c5ec53a4032198999f2d96a6c04f764827521c4d8266dfd63183a8d` |
| `coverage_i444_square8_02.avif` | `c76fd9908087d9025e5eac621d2fa7dc3e5aa2cbbe902e7df9baec31934a16fe` | `52cf14c15d3016015816a5097d48ed7b32210911f00e6533d65fc07aad401360` |
| `coverage_i444_square8_03.avif` | `bf79a86725d4e78286972e0688a6e9551850f7b476c7febe06c5c62b7d27cadc` | `23e0828c4691405b5616f2d3d1ce2452c8643ef941ff888fdcdb08d9ddbae07b` |
| `coverage_i444_square8_04.avif` | `7fe339ea07a4efc8592250f973f37eebd91878b64b48ef5da6ff0d928b259212` | `6af78ef081a21691dac3dbe080e0e74a4666df7c401975857a88d31be170c8d2` |
| `coverage_i444_square8_05.avif` | `88ad2e5488e80cbeba53625826b0f90a6fb96a8f9ac9f314f11ec8b4b505f2bc` | `956047973e698d18fe70a45f57a797c94f38bdf12ee2c6b5dcbf706971763cbf` |
| `coverage_i444_square8_06.avif` | `ce43e1768fa0d92d6821c4971ea071dedb6aeaa92b054e5cfb368a2ea903af67` | `69d96e28e665d2570868fce3d2e30aaa891a46afffc55146c8511fd3e2fe1f7d` |
| `coverage_i444_square8_07.avif` | `f7780936d03e09920e206942151ae9378abbf4100216644316da0624f5bf437e` | `ed89a1e09548a12cf5953f812af87b33d7047922a81f71359a949fdad1378b9b` |
| `coverage_i444_square8_08.avif` | `7ea976064f08dde24c28842e3fe3d3af3d01310f896b346fe179622d0da5322c` | `861d107c5e7958cf4bb38cc63f8c19d6460e16e7418f2e3fe4856c74d82910a2` |
| `coverage_i444_square8_09.avif` | `f1cf6c7fa5ddec16583f99e1ad5318f9c731386f9038071e1ba51f0b2d854737` | `7df3e53c1af05ddc0e53f6c59a2e0b3433da621fc44f0c0f4714d66fe4876aaa` |
| `coverage_i444_square8_10.avif` | `a8942600752ed77d7ecbca6b726e589d1c106963bd4bca4eca6bbbb18cc9978c` | `c9f06d709276d78fc43bc11d9712d4ea29faea7b0d52655175e827d15b1d3ced` |

| Fixture | Size | File SHA-256 | Pillow RGB SHA-256 |
| --- | ---: | --- | --- |
| `coverage_adst_public_03.avif` | 4×16 | `b6d15fa1ceb3eedcd3636ed660c0ed6755ce3a2af3ff6a3b2dcf6fa0b1adcc25` | `c4cbd418d7f72de0fd778268c0a4c40ac6c30b982987a3a4bfa84372c3c102e9` |
| `coverage_adst_public_05.avif` | 8×16 | `b398d1af52e414bee7e6d2a5ff071b8dd8d9af16d84dc301765f7fd05968537e` | `ccf631ee65a05977a2020995f5dc442905ad0c21450f3e3e0df3bd0f0d2b8e11` |
| `coverage_adst_public_06.avif` | 16×8 | `d4dc8bcf2e10acc54d24712def595d16a1550b7eacb44eefb58089a50a6b8ce8` | `988aef43dcf1c4eeaa0cffee66f3ba32e9c127c0b07996830900b4a79ed07cd6` |
| `coverage_adst_public_07.avif` | 8×32 | `0afade55d9a04a29af287c04e2f16a0cfc05758e3531658fd3be569948abe8d0` | `a40858233036b25f36900bd39be40e6eda843493ac27b767448b891ac8437492` |
| `coverage_adst_public_08.avif` | 32×8 | `0f59e5943381edf9361311a39d6e73a726cb028f9003fed675835104392abe5a` | `8b308e80e0a1a904072657a1f8b3472b5b89e37dc01238c8dc6066689a9ebf6a` |
| `coverage_adst_public_09.avif` | 16×16 | `866fc5bee5d19bf06df56b90c7b92d72c7725a1ed8aa7384a0cadb931c945a5e` | `e0e5a1ae7b7aef892258e7f7f2332f13f959b419ba0f9b14c8edcc9a298e487d` |
| `coverage_adst_public_10.avif` | 32×16 | `018eecce2e1f068cfe6ae022bd5e0f48f4a547c736bd946edb6ba45bc6663bcf` | `93047df7e452ceca5c0cf243100db0b2e1508e7db35d86dc00ad34b70069db4e` |

The EOB controls are selected by
`scripts/explore_avif_sample_mutations.py`, which exhaustively replaces every
AV1 item byte with every other byte value and retains only exact scalar syntax
prefix matches. The EOB-bin mutation is a Pillow error fixture; the EOB-base
mutation is a second Pillow error fixture that changes only coded-tile data.

The following additional 4:2:0 fixtures are generated by that same pinned,
double-encode-verified path. The constant-color files extend visible geometry
from 4x4/8x8 through cropped 8x8 leaves, 16x8 or 8x16 rectangular leaves,
two-child recursive splits, and cropped or complete 16x16 leaves. The final
two files start with constant `(17,91,203)` and replace the bottom-right 8x8
quadrant with the listed color, selecting one square split and four coded 8x8
children.

| Fixture | Source or replacement RGB | SHA-256 |
| --- | --- | --- |
| `portable_lossless_420_leaf_4x8_a.avif` | `(17,91,203)` | `31aae6e6395da7d749786b00c339ace12d29af7acbfa7d9710bca10d9d92346e` |
| `portable_lossless_420_leaf_8x4_a.avif` | `(17,91,203)` | `7108ddc6197b99e99d89f1327108cf070ff051d32cac02b82eae1531feb0daf7` |
| `portable_lossless_420_rect_12x4_gray_127.avif` | `(127,127,127)` | `d8bf37e044015315531fa44a412619bba0eede149b2caae9baeae3e0175d9b3f` |
| `portable_lossless_420_rect_16x4_gray_127.avif` | `(127,127,127)` | `bf3bc36ebd94d157ea028e41d12077ffec574d5b9ca6e115b3947a55f81f7580` |
| `portable_lossless_420_rect_12x8_gray_127.avif` | `(127,127,127)` | `b58a1b66e5dcd33c1686f072634c0e5f0662eb67dd0a8e3833303d4d7ad57808` |
| `portable_lossless_420_rect_16x8_gray_127.avif` | `(127,127,127)` | `ae83d9122ffad59a687f03e74e4dd2d78a08b6a5693cae0e67e299545584fe2b` |
| `portable_lossless_420_rect_4x12_gray_127.avif` | `(127,127,127)` | `1020c7340e5d9079777e7522229f30b1058817138acefc83330ad3e22c6a9010` |
| `portable_lossless_420_rect_4x16_gray_127.avif` | `(127,127,127)` | `8f5adda734549c4e0f7b88055f3819c553cb62f4ce902c5bb4e4a952cdf1f2d4` |
| `portable_lossless_420_rect_8x12_gray_127.avif` | `(127,127,127)` | `ad7c2d567edfa34b7988a64a18b19efe129c4f19ae67ab0265d58fecb654a10c` |
| `portable_lossless_420_rect_8x16_gray_127.avif` | `(127,127,127)` | `076d56c74ee714f01d26d21177c95c88be85157ad0a5d612ab94c3365f3a8520` |
| `portable_lossless_420_split_12x4_a.avif` | `(17,91,203)` | `5fadea5fcf4a48c7b77ea0a89761263516a6fde5472f1ab1b42d85e4a8bc1782` |
| `portable_lossless_420_split_16x4_a.avif` | `(17,91,203)` | `c262f88b8bae4ee384b69c705dfbe42d2dae6601c61b23b1d64b1e59db25be73` |
| `portable_lossless_420_split_12x8_a.avif` | `(17,91,203)` | `25ab515d0bdde387c97d6bf9b44b33e8327bf8642c4738c3ba424297b5a41ccb` |
| `portable_lossless_420_split_16x8_a.avif` | `(17,91,203)` | `325a6e737bd018076105cd3a22cc48d6b9c1d7b9dc0d9b29d6d749e6295de0b8` |
| `portable_lossless_420_split_4x12_a.avif` | `(17,91,203)` | `633040bba8ebb2c38a0201783869474b2867a79c19759eb70e8930ffb517c2cd` |
| `portable_lossless_420_split_4x16_a.avif` | `(17,91,203)` | `fc8ad1c44445df13afee7f176501fa754b1ea094cef9734ea8258281897b795b` |
| `portable_lossless_420_split_8x12_a.avif` | `(17,91,203)` | `539783b6e6c6ad4b54ef8e0f1f445c5f6e38b82b37529ab88a891c86958d17fd` |
| `portable_lossless_420_split_8x16_a.avif` | `(17,91,203)` | `995ac2a192f5e08af7535ee8151cb98386e82bb696abd99afd0204665b0b1da0` |
| `portable_lossless_420_square_12x12_a.avif` | `(17,91,203)` | `d1f328bb548b6d0911ed6c2125fa8d26ed2a72738c081040a9e64990c916adf3` |
| `portable_lossless_420_square_12x16_a.avif` | `(17,91,203)` | `9b93917ebc8120ce0d3f7ed5c8e9b41f1d5dc4afb248647c694ff1b634d4623b` |
| `portable_lossless_420_square_16x12_a.avif` | `(17,91,203)` | `1124115b0edb90a5b751e11b502f07788d03edf0bc3305ca6fb3f1a018ce4f9e` |
| `portable_lossless_420_square_16x16_a.avif` | `(17,91,203)` | `bde1f73324f6b1bd1ec41ed68ecf9a15d0ada9d7e3508ef70e54fe9216ebd73a` |
| `partitioned_square_420_16x16_rgb_delta.avif` | replacement `(22,96,208)` | `9cb30c2c2391c414c5dfef0a0ed27d9409089f88cdd05aad45103e720b6b12f7` |
| `partitioned_square_420_16x16_g96.avif` | replacement `(17,96,203)` | `7e66769bff63133cbab59a6d93aa143f4d2f0982fa142567dfc4727783c3330a` |
| `coverage_lossy_420_square8_four_leaves_01.avif` | same lower-right replacement, quality 99 | `c0465a00209870571f58be71cb122d5c42ae8d19e91ae001d8f3b706e7205255` |

`coverage_lossy_420_square8_four_leaves_01.avif` is generated by
`scripts/generate_avif_lossy_420_square8.py` with pinned libavif 1.4.1 and
libaom 3.13.2. The generator encodes the listed quadrant source as lossy
4:2:0 at quality 99, speed 8, one thread, and fixed minimum and maximum
partition sizes of 8 pixels. Two encodes must match the pinned AVIF hash; the
generator also checks the AV1 profile/frame controls and exact Pillow RGB
output (`a8e0fdcf9fc9fde209db6dbb71c23dae9a25996b6cef24808e64e03ff5e38e64`).
Pinned scalar dav1d 1.5.3 confirms the root split and four terminal Square8
leaves. Regenerate it with
`.oracle-venv/bin/python scripts/generate_avif_lossy_420_square8.py --avifenc target/oracle-staging/libavif-superres-build/avifenc`.

`portable_lossy_420_square8_gradient.avif` uses the same generator with
`--profile gradient`. Its original 16×16 RGB source is
`(24 + 5*x + 2*y, 40 + 3*y + x, 180 - 2*x - 3*y)` for coordinates
`0 <= x,y < 16`. The same pinned encoder, quality, subsampling, partition
limits, and single-thread settings produce a 381-byte AVIF with SHA-256
`8e7e7bcdfc9cd34e88e49c0b18b0e010529d62d3c526266d286cf32746f41671`.
The generator checks two identical encodes, disabled screen-content tools and
intra-block copy, the lossy frame controls, and Pillow's exact 16×16 RGB
output SHA-256
`82bc76a521907851c6cd20dace4a458d98387a5786b1e9551add7c0cec14d3f3`.
The default `quadrant` profile preserves the earlier fixture.

These rasters and encoded files are original project material under the
repository's MIT/Apache licensing. The generated AV1 behavior is verified
against pinned scalar dav1d 1.5.3 and the exact Pillow oracle; no third-party
fixture bytes are copied into this set.

`portable_lossless_gray_32.avif`
(`f57c5df28dc28add5b9913c9d3cc0c0aae2e69e0087e7a8614674c8658987875`)
and `portable_lossless_gray_127.avif`
(`40de5aecb3fb4c8b6ad9e242beda63204aae00f6be34694c14554aab91c330f8`)
use the same deterministic encoder settings for constant RGB `(32,32,32)` and
`(127,127,127)`. They extend the closed portable class with mixed and
all-plane zero-residual transforms while retaining the already accepted luma
and chroma prediction modes.

`portable_probe_gray_128.avif`
(`26713256cc2769ab320d6017dca8b0f822dbdb03e66351e8ebc37bda64e440dc`)
and `portable_probe_gray_129.avif`
(`649c4e452a350a51a9230070d768e432ff03bf7b3fb5700a50653f5f2887fc7a`)
are generated with the same settings. They select luma prediction modes 0 and
2 respectively and extend the portable class with exact first-block DC and
horizontal prediction.

`portable_lossless_8x8_a.avif`
(`b7f758da88a2a9835bcda1a709b1de1ce47e232113d7d67f027ec430bb089714`)
and `portable_lossless_8x8_gray_127.avif`
(`1b6a333257e226a63b6b33b34da54917a02518cf91864618222f60c160a883b7`)
use the same deterministic encoder settings at 8x8 for constant RGB
`(17,91,203)` and `(127,127,127)`. They isolate the geometry change from one
visible 4x4 WHT transform per plane to all four transforms of the same coded
8x8 leaf.

`portable_probe_8x8_gray_128.avif`
(`6a4f26af5a873630c21a85fa1b7a1337026991acac0a305ecd6dd059a84fba63`)
and `portable_probe_8x8_gray_129.avif`
(`43ab3fc61b01b3b173323da584abe8a07c06eb4fcf4254cf8e04f3333f654237`)
apply the same 8x8 geometry to the already accepted luma DC and horizontal
predictor modes.

The eight `portable_*_4x8_*.avif` and `portable_*_8x4_*.avif` fixtures are
generated with the same settings from RGB `(17,91,203)`, `(127,127,127)`,
`(128,128,128)`, and `(129,129,129)`. They prove both visible orientations of
the padded 8x8 coded leaf, including nonzero and zero residuals and all three
accepted luma predictors. Their exact file and AV1-item hashes are pinned in
`scripts/generate_av1_reconstruction_refs.py` and
`docs/avif.md`.

The eight `portable_*_12x12_*.avif` and `portable_*_16x16_*.avif` fixtures
use the same four RGB sources and deterministic settings. They select the
level-3 padded 16x16 coded leaf, isolate its 4x4 transform-context grid, and
prove both partial 12x12 visibility and complete 16x16 reconstruction. Their
exact file, AV1-item, decoded-plane, and Pillow RGB hashes are pinned in the
same generator and progress document.

The eight `portable_*_12x16_*.avif` and `portable_*_16x12_*.avif` fixtures
retain that level-3 padded 16x16 leaf while proving both remaining
`PARTITION_NONE` visibility orientations. The eight
`partitioned_4x12_*.avif` and `partitioned_12x4_*.avif` fixtures use the same
deterministic settings but select partition symbols 1, 2, and 3. The six
`partitioned_4x16_*.avif` and `partitioned_16x4_*.avif` fixtures retain the
accepted recursive partition-symbol-3 syntax while making both 4x4 children
fully visible on the long axis. The twelve `partitioned_8x12_*.avif`,
`partitioned_12x8_*.avif`, `partitioned_8x16_*.avif`, and
`partitioned_16x8_*.avif` fixtures preserve that exact two-child syntax while
making all eight coded samples visible on the short axis.

The gray-127 partition-symbol-1 and partition-symbol-2 files, the twenty-two
`portable_rect_*_gray_*.avif` files, and four representative speed-0 residual
files prove the complete one-axis rectangular leaf class. The speed-8 A,
gray-32, and green partition-symbol-3 files prove the smallest recursive
two-leaf class in both orientations: nonzero YUV residuals, luma-only
residuals, horizontal and vertical spatial mode contexts, neighbor coefficient
contexts, and left/top edge prediction. The full-width cases additionally
prove that the same private coded-plane composition preserves all sixteen
declared samples without adding a public crop or image-processing operation.
The eight-pixel cases also coexist with the accepted partition-symbol-1 and
partition-symbol-2 leaves at identical dimensions, proving that decoded syntax
rather than geometry chooses the reconstruction path.

`partitioned_square_12x12_g96_direct_tokens.avif`
(`b61f62f12306af9744ea06ac8c68bfd86f8b10f27caca820405b295756a3f194`),
`partitioned_square_16x16_g64.avif`
(`4a8703a56c56a2d6cbcdbec90e12d266fc28603db1f84e725f7f1a75f504fed7`),
`partitioned_square_16x16_g96_direct_tokens.avif`
(`1fcdc276a8521a7d248fa9382aca518c880921615a392d6116e3fff28320032d`),
`partitioned_square_16x16_r64.avif`
(`fe7610630b212d87a5b9b9650fa156be9729e1bd49d8c01df5df416e5e524898`),
and `partitioned_square_16x16_g127.avif`
(`4085fdb230e1bcc93a3a3be408d5fbbf0a5c740590df3983c07b191d3b59ba08`)
are repository-generated from a constant `(17,91,203)` square source. The
16x16 fixtures replace the bottom-right 8x8 quadrant with `(17,64,203)`,
`(17,96,203)`, `(64,91,203)`, or `(17,127,203)`; the 12x12 fixture replaces
the visible rectangle beginning at coordinate `(8,8)` with `(17,96,203)`.
The g96 fixtures prove the AV1 direct high-token DC magnitudes 4, 8, and 12,
including both coefficient signs, without a Golomb extension. They use the
same pinned deterministic lossless 4:4:4 encoder settings and select one
level-3 square split with four level-4 8x8 leaves. Their encoded AV1 syntax and
Pillow output are observations of the pinned oracle; the generator and source
rasters are original project material under the repository's MIT/Apache
licensing.

The 12x12 direct-token fixture's complete entropy trace is identical to the
16x16 g96 fixture, and it proves declared-frame visibility after the same
coded 16x16 reconstruction.
`partitioned_square_12x12_midpoint_g96_ac.avif`
(`d10972f944777129121ef100ee66903959138ae946295bb5fe271cef8035b258`)
changes at the declared-frame midpoint `(6,6)`. It is the one-hundred-second
independent reconstruction positive. It composes horizontal and vertical
prediction, luma EOB-4 token-three residuals, chroma EOB values 1, 2, and 4,
and the complete one- and two-neighbor residual-context propagation.
`partitioned_square_12x12_top_left_luma_eob4.avif`
(`fbc5e3cec5da21a1c1095ecf82525dac5d6ae60ff4a71b101502392de754cc45`)
changes `(17,91,203)` to `(22,96,208)` beginning at `(6,6)`. It uses the
top-left 2x2 transform-grid sequence DC-only/skipped/skipped/EOB-4 while U and
V remain DC-only/skipped/skipped/skipped and all later transforms remain
skipped. Its closed reconstruction also exercises horizontal right-edge
propagation, vertical bottom-edge propagation, two cross-leaf skip contexts,
and the bottom-right two-neighbor DC mode table.
`partitioned_square_12x12_top_left_luma_eob12_control.avif`
(`b8b703ee9e1f2d8200fea338ee85f7ada1b905539bb163712209f60d83af0713`)
uses the same replacement beginning at `(7,6)`. It preserves the grid
transition and selects the admitted alternate EOB-12 coefficient body. It is
the one-hundred-first independent reconstruction positive and proves the
base-context-seven value-three branch, its adaptive high-token updates,
DC high-token context four, and the complete nonzero sign chain.

`partitioned_square_12x12_luma_eob1.avif`
(`db9102a9b302387df2214814ac2cd02c8414beaf4751f3f374370237a210e9bc`)
changes the rectangle beginning at `(10,8)` to `(22,96,208)`. It retains the
same five-node square partition tree while isolating two luma-only EOB-1
lossless transforms; chroma remains skipped.
`partitioned_square_12x12_luma_eob2_control.avif`
(`89842483e159b7d9d98f58282679f9b6d09f4e164576270e9546b13df176c986`)
uses the same replacement at `(8,10)`. It is an active Pillow/native decode
fixture that isolates two luma-only EOB-2 transforms admitted by Slice 22.
`partitioned_square_12x12_luma_eob4_control.avif`
(`307512d55df127d8546273a57dedd182fbdb5282aa830191f7fe201b8eff419f`)
uses the same replacement at `(10,10)`. It isolates the luma-only EOB-4
transform admitted by Slice 23.
`partitioned_square_12x12_luma_eob6_control.avif`
(`90583dd6d88fce42d0cfdb8f9e7217d02d5a711f873955a04526dd99f5886efa`)
uses the same replacement at `(9,8)`. It isolates two luma-only EOB-6
transforms admitted by Slice 24.
`partitioned_square_12x12_luma_eob9_control.avif`
(`d57ddb0c7dbcdfc63aa77f3bdbd64a793246451528dfec553c0bf800f8137d4b`)
uses the same replacement at `(8,9)`. It isolates two luma-only EOB-9
transforms admitted by Slice 25.
`partitioned_square_12x12_luma_eob10_control.avif`
(`2cb6cfd94fb6cfaf62375d0c7c9dd51b9193d4b3740b31a62750b14ddc39e072`)
uses the same replacement at `(10,9)`. It isolates the luma-only EOB-10
transform admitted by Slice 26.
`partitioned_square_12x12_luma_eob12_control.avif`
(`52293589be5deed92756c5e11447b571686381ec3c873dbb9e5a221b91eb820c`)
uses the same replacement at `(9,10)`. It isolates the luma-only EOB-12
transform admitted by Slice 27.
`partitioned_square_12x12_luma_eob15_control.avif`
(`3265cf40613523eab69cba5ae73af453f781a29ab3b36f13c21b6720a4d42d7a`)
uses the same replacement at `(9,9)`. It isolates the luma-only EOB-15
transform admitted by Slice 28. All ten are original project material
generated by `scripts/generate_test_assets.py` under this repository's
MIT/Apache licensing.

The `coverage_entropy_mosaic_01.avif` through
`coverage_entropy_mosaic_10.avif` family is an original deterministic 32x32
4:2:0 quality-99 corpus generated from two low-contrast rectangles. Every file
selects one matrix-10 Square32 DC/DC leaf with a TX32x32 luma transform and
TX16x16 chroma transforms. Cases 03 and 06 through 10 enable screen-content
tools and prove the ordered `y_pal=0`, `uv_pal=0` syntax before filter-intra;
04 and 05 are screen-content-disabled controls. Case 06 reaches luma EOB 526,
and cases 06 and 07 retain extended coefficient magnitudes. This evidence does
not admit a nonzero palette, palette index map, palette neighbor context, or
intra block copy. The promoted file hashes are:

- 03: `bbf49002958d8b836d30ef5f837168a841b138e67f12ef4f6e73c072b71e65d9`
- 04: `233617a50cfd0a8b2dbd5976e1d4296bd9f6b26b36f416970fe812ba00f79d73`
- 05: `0509df3919b43398bd7e2bf6d812796113c750094cf7a973d58aa19fbc8d2dc7`
- 06: `ea7d7dc634b9ef96069030b6b62b4c5c499152982dc5524f17f5dfe5b58a3028`
- 07: `dcb3689dd4ca134fb7c221140c4e75b13abc39f6fba611ee285f7a003f1c5f2a`
- 08: `561dcfe17d6583e0d9051cd221ea93be152c59479074bc69ff9b038848eb5451`
- 09: `6d4b2b591d77fa312ac8f98b3478364f01c80852745dae498c48692a1e3a60f8`
- 10: `aefbe6aab6da76fe7d51bb0fb3e9d7e83fb0622da44d6b1f835e0868662d6558`

`scripts/generate_av1_reconstruction_refs.py` checks every positive file hash,
builds an instrumented scalar copy of exact dav1d 1.5.3 commit
`b546257f770768b2c88258c533da38b91a06f737` outside the repository, and writes
the indexed `tests/fixtures/outputs/av1_reconstruction.json` plus its
`av1_reconstruction.part-*.json` case files. That oracle records all
partition-block headers, scalar entropy operations, reconstructed Y/U/V plane
rows and hashes, and Pillow RGB rows and hashes. The Rust integration test
joins the index and parts, then consumes the resulting document and the
positive AVIF fixtures directly.

The bounded vertical Diagonal67 fixture
`coverage_square8_chroma_diagonal67_vertical_01.avif` is the promoted
`D67V-F01-N00` result from the input-only 100-candidate,
10-family campaign in
`scripts/explore_avif_chroma_diagonal67_vertical.py`. The campaign qualified
59 candidates without invoking repository Rust. The 8x16 4:2:0 fixture uses a
clipped vertical split and a following bottom Square8 with coded UV mode 8,
angle symbol 3, ADST-DCT TX4x4 chroma, split TX4x4 luma, and non-empty
residuals. It is regenerated by `scripts/generate_test_assets.py`, checked by
`scripts/generate_av1_reconstruction_refs.py`, and proved against pinned
dav1d 1.5.3 with 118 entropy operations, exact Y/U/V planes, and exact Pillow
RGB output. The fixture SHA-256 is
`7251e37d120b6cd170d0f2de705b2e56cccda3dfbd3ea4384369132bd0ea0f3f`.
