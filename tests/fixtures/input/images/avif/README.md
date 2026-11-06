# AVIF fixture provenance

These files are fixed inputs for the Pillow 12.2.0 parity manifest. Copied
upstream inputs are listed first; repository-generated inputs and their exact
generation provenance follow.

| Local file | Upstream file | Tag / commit | SHA-256 | License |
| --- | --- | --- | --- | --- |
| `baseline.avif` | Pillow `Tests/images/avif/hopper.avif` | Pillow 12.2.0 / `3c41c09` | `d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2` | MIT-CMU (`third_party/pillow/LICENSE`) |
| `error_avif_missing_meta.avif` | `baseline.avif` with its complete `meta` box omitted; the original `ftyp` and `mdat` boxes and encoded AV1 media bytes are retained | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 rejects the complete file at open | `5ed417664c2b5b74bd36591baf392c94a99a16724ee12454fa921d2ef1ff2a8b` | MIT-CMU (`third_party/pillow/LICENSE`) |
| `error_primary_item_missing_ispe.avif` | `baseline.avif` with the primary AV1 item's `ispe` association cleared; all box sizes and encoded AV1 media bytes are unchanged | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 rejects the complete file at open | `475d45c0c712b011983d6d946a2e3ce4512d7968664853b0a6953fa7da0c06f7` | MIT-CMU (`third_party/pillow/LICENSE`) |
| `error_avis_missing_moov.avif` | `animated.avif` with `meta` and `moov` omitted and compatible `avif` brands changed to `avis`; the complete `avis` `ftyp` and `mdat` media payload remain | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 rejects the complete file at open | `5831e8f2c248e9cec39f80cd7856f95135be889e13a138fe436587f061321cf4` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_avis_moov_without_trak.avif` | `animated.avif` with its sole `moov/trak` removed while `ftyp`, `meta`, `moov/mvhd`, and the complete `mdat` media payload remain | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 rejects the complete file at open | `b7bad81724df0c975c0ba70935f07ade291f5aa55f9b539b230bbb7bfe00562f` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_pitm_zero_item_id.avif` | `baseline.avif` with only the version-zero `meta/pitm` item identifier set to zero; all box sizes and encoded AV1 media bytes are unchanged | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 rejects it at open with `Failed to decode image: Missing or empty image item` | `c85de8c2432106246e3135cb873eaf4d48ec94ff7b1d34b69e77cb2e119c126d` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_iinf_duplicate_item_id.avif` | `baseline.avif` with its primary `infe` entry duplicated under `iinf`; both entries retain the same item ID and all AV1 media bytes are unchanged | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 opens the file, then fails frame decoding with `Failed to decode frame 0: Decoding of color planes failed` | `b80ecc8c82022162c98d17e84047f9c6a5988345eb702e41445faf33469994e1` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_animated_missing_leading_ftyp.avif` | `animated_sequence_only_no_meta.avif` with only the leading box type changed from `ftyp` to `free`; all remaining boxes and media bytes are unchanged | `scripts/generate_avif_missing_leading_ftyp_fixture.py`; Pillow 12.2.0 rejects the complete AVIS file at open | `d45842074621fd7572ef6331e44fd97691dfd74584e0a58ec198cf60524f8020` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_iprp_ipma_before_ipco.avif` | `baseline.avif` with the complete `meta/iprp/ipma` box moved before `ipco`; all box bytes and encoded AV1 samples are preserved | `scripts/generate_avif_iprp_order_error_fixture.py`; Pillow 12.2.0 rejects the complete file at open | `06d3fa832ab0b96b7108926c8973b30d578617e75aa57a7bbcaabdd53ac2cdd7` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_iprp_missing_ipma.avif` | `baseline.avif` with `ipma` removed from `iprp` and a size-matched ignored `free` box appended to `meta`; AV1 samples and file length are preserved | `scripts/generate_avif_missing_ipma_fixture.py`; Pillow 12.2.0 rejects the complete file at open | `b73571771c5418bf14f1a70989e0af0ed98aa59ee834b6b04c8dece86ff77392` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_iprp_unknown_child.avif` | `baseline.avif` with the `ipma` child renamed to `free`, preserving the box size and AV1 samples | `scripts/generate_avif_iprp_child_error_fixtures.py`; Pillow 12.2.0 rejects the complete file at open | `1d67de2af3d30be3ef552e461fb3a74c3badacd6c6fd84e1e8fb9937fa3d3580` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_iprp_duplicate_ipma.avif` | `baseline.avif` with a duplicate `ipma` appended to `iprp`; the primary-item extent is rebased and AV1 samples are preserved | `scripts/generate_avif_iprp_child_error_fixtures.py`; Pillow 12.2.0 rejects the complete file at open | `d6e09f8fcc13fd62817793463188f12b2583ec044a967368af9c977f4cf2b65f` | MIT/Apache-2.0 (repository-generated mutation) |
| `trailing_boxes_over_budget.avif` | `baseline.avif` followed by 4,094 empty top-level `free` boxes (4,097 root boxes total) | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 exact image info and RGB pixels; exercises the decoder's bounded trailing-box scan after complete metadata | `0a92d1c734bb48ab51ffe4aef1f0c5f5693ca06790058cb9a0c74c4c9eb73971` | MIT/Apache-2.0 (repository-generated mutation) |
| `trailing_truncated_top_level_header.avif` | `baseline.avif` followed by one zero byte, the incomplete start of another top-level box | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 ignores the trailing fragment and preserves exact image info and RGB pixels | `70def23bc6c03c8b3b49d79ca2f9fc3d01eeb60aa44fc00088e07ecb8ca7d616` | MIT/Apache-2.0 (repository-generated mutation) |
| `trailing_truncated_top_level_payload.avif` | `baseline.avif` followed by a complete `free` box header declaring an absent eight-byte payload | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 ignores the incomplete trailing box and preserves exact image info and RGB pixels | `78c60fa1cc0f453fbe9f6db714eb1f53001b0e7e57e7c7a401a4b4c3e7581816` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_incomplete_still_truncated_top_level_header.avif` | The valid 32-byte baseline `ftyp` box followed by one zero byte, with no image metadata or media box | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 recognizes the AVIF signature and rejects the incomplete image at open | `286ab06301e7cd9b2355a651876b6f9935ca37d60fc2de42ed1808b095c15e72` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_infe_missing_item_name_terminator.avif` | `baseline.avif` with the only zero terminator in its AV1 `infe` item name replaced by `X`; all box sizes and encoded AV1 media bytes are preserved | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 rejects the unterminated item name at open | `b8609cd7fd3acd8c4f7eacad072ed85dcce52c65df9ebd9760ab1e1a8328620d` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_iref_records_over_budget_trailing_byte.avif` | `baseline.avif` with 4,088 ignored `zzzz` item references followed by one truncated child byte | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 rejects malformed metadata; the AVIF sample parser rejects the over-budget reference table before reaching the trailing byte | `a160f1c9ddc2dd178b79ba152ac834d8887c6ec1921b4d5e904ec9df37c2c11c` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_meta_duplicate_idat.avif` | `iloc_idat_indexed_extent.avif` with an empty second `meta/idat` box | `scripts/generate_avif_container_edge_fixtures.py`; the complete original AV1 payload is preserved and Pillow 12.2.0 rejects the duplicate `idat` at open | `466e3ecd6e89e4cbd2dba8084642fc2ddbda83e196e005819a31aa54cc3c960b` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_meta_hdlr_nonzero_version.avif` | `baseline.avif` with only the `meta/hdlr` FullBox version set to one | `scripts/generate_avif_container_edge_fixtures.py`; complete AV1 payload and 3,077-byte file size are preserved; Pillow 12.2.0 rejects the input at open | `e5d7f45c33cc3aec2868d57bddaad27b62df23210326822382f81e2cb344dc62` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_meta_hdlr_nonzero_predefined.avif` | `baseline.avif` with only the `meta/hdlr` `pre_defined` field set to one | `scripts/generate_avif_container_edge_fixtures.py`; complete AV1 payload and 3,077-byte file size are preserved; Pillow 12.2.0 rejects the input at open | `b9119779779df0addfdfcd92927ae06127109846796ca6806c1a0bc20ce7391d` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_meta_reordered_handler.avif` | `baseline.avif` with the valid `meta/hdlr` child moved immediately after `pitm`; the file size and all AV1 samples are preserved | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 rejects the reordered metadata at open | `75d67ac340bfec40e5bce5e0f5d19d769053ec5fe774989fd56cb90b32392516` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_auxc_nonzero_version.avif` | `alpha.avif` with only the `meta/iprp/ipco/auxC` FullBox version set to one | `scripts/generate_avif_container_edge_fixtures.py`; complete AV1 payload and 6,441-byte file size are preserved; Pillow 12.2.0 rejects the input at open | `2357cdc2bc511390a97c6f84cdf1bbf03b0debe3b7bda4090b397d7bd20c3822` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_pixi_zero_depth.avif` | `baseline.avif` with only the first channel depth in the `meta/iprp/ipco/pixi` property set to zero | `scripts/generate_avif_container_edge_fixtures.py`; complete AV1 payload and 3,077-byte file size are preserved; Pillow 12.2.0 rejects the input at open | `7c815a023acc909df7a21d80d11aa1ca92aaae6ce0748bfc989288cae91d3c5f` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_pixi_nonzero_version.avif` | `baseline.avif` with only the FullBox version in the `meta/iprp/ipco/pixi` property changed to one | `scripts/generate_avif_container_edge_fixtures.py`; complete AV1 payload and 3,077-byte file size are preserved; Pillow 12.2.0 rejects the input at open | `a50bd4126788de5efdeefa05ec6583267311fe021af9168928850589d9fd5689` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_pixi_zero_planes.avif` | `baseline.avif` with only the plane count in the `meta/iprp/ipco/pixi` property set to zero | `scripts/generate_avif_container_edge_fixtures.py`; complete AV1 payload and 3,077-byte file size are preserved; Pillow 12.2.0 reports `RuntimeError: Failed to decode image: Not implemented` | `b29a15defd39037aebaa7dffdd878c828201d1d9adc852302e8c76b123036431` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_pixi_depth_above_16.avif` | `baseline.avif` with only the first channel depth in the `meta/iprp/ipco/pixi` property changed from eight to 17 | `scripts/generate_avif_container_edge_fixtures.py`; complete AV1 payload and 3,077-byte file size are preserved; Pillow 12.2.0 reports `RuntimeError: Failed to decode image: Not implemented` | `bdf5ecf32956f1c05ed93c9b3e5a9fb2381f5429c1996ccb6b5701201fdbd34d` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_pixi_channel_depth_mismatch.avif` | `baseline.avif` with only the second channel depth in the `meta/iprp/ipco/pixi` property changed from eight to seven | `scripts/generate_avif_container_edge_fixtures.py`; complete AV1 payload and 3,077-byte file size are preserved; Pillow 12.2.0 reports `RuntimeError: Failed to decode image: Not implemented` | `420568bd6387e4262a2b3076a466b0d39efe35f9c01c99a66364352389b92242` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_grid_without_dimg_references.avif` | `grid.avif` with its sole `dimg` reference box renamed to unknown type `zzzz`; all media payload bytes and box sizes are preserved | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 rejects the invalid image grid | `a283fb5590e31db1f491fe1e010a22fec3f55da99080dd4dcc61b95e89d6316a` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_grid_alpha_sample_count_mismatch.avif` | `grid.avif` with the second alpha cell's `auxl` target changed from color tile 3 to tile 2; box sizes and all AV1 payloads are unchanged | `scripts/generate_avif_grid_alpha_sample_count_mismatch.py`; Pillow 12.2.0 rejects the incomplete alpha grid with `Failed to decode image: Invalid image grid` | `e63f9dfda938353e27c05834e4b736fd855bbdb273d9b8c2d661348e710e3e50` | BSD-2-Clause (`third_party/libavif/LICENSE`; derived from `grid.avif`) |
| `error_grid_non_av1_color_tile.avif` | `grid.avif` with first color tile item 2 declared as `Exif` instead of `av01`; all box sizes and AV1 payload bytes are unchanged | `scripts/generate_avif_grid_non_av1_color_tile.py`; Pillow 12.2.0 rejects the complete file with `Failed to decode image: Invalid image grid` | `1721f82f4ea1f128a97041d867fc1a1c1c0a69663ef9e12d26b02e90b5285803` | BSD-2-Clause (`third_party/libavif/LICENSE`; derived from `grid.avif`) |
| `error_unsupported_primary_item_type.avif` | `grid.avif` with the primary item declared as `Exif` instead of `grid`; all box sizes and AV1 payload bytes are unchanged | `scripts/generate_avif_unsupported_primary_item_type.py`; Pillow 12.2.0 reports `Failed to decode image: Missing or empty image item` | `9285db1aa6df2d36c4d02fa8f2efedeac2f3ca48fc5f6aec64488893c3e3b9b8` | BSD-2-Clause (`third_party/libavif/LICENSE`; derived from `grid.avif`) |
| `error_grid_version_1.avif` | `grid.avif` with only the primary grid descriptor version changed to one; all other file bytes and encoded media payloads are preserved | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 reports `Failed to decode image: Not implemented` | `6d75c008f18bf3c509c79ba810656a0e71d0f1da05c890456af598e51172ddeb` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_grid_zero_width.avif` | `grid.avif` with only the primary grid descriptor output width changed to zero; all other file bytes and encoded media payloads are preserved | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 reports `Failed to decode image: Invalid image grid` | `37b53b695d8d8a97ddba92087ad15de6c75635c970c2dd94d037b6ad6f754bf2` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_grid_zero_height.avif` | `grid.avif` with only the primary grid descriptor output height changed to zero; all other file bytes and encoded media payloads are preserved | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 reports `Failed to decode image: Invalid image grid` | `3f2fd4d6f618c5527c25a1750b508c5ed7122f69b4a7c44a7c3bd9c4fb5292b6` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_ftyp_misaligned_compatible_brand_payload.avif` | `idat` source with one trailing byte appended to the `ftyp` compatible-brand list; all encoded `mdat` and `idat` payload bytes are preserved | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 rejects the file at open | `ada44e071b8609bc68b65ce123df4dac04b21e4b64d0865ec699bc7425400fc3` | MIT/Apache-2.0 (repository-generated mutation) |
| `ftyp_1025_compatible_brands.avif` | `idat` source with 1,021 unique four-character compatible brands appended, for 1,025 total; metadata and AV1 media boxes are unchanged | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 / libavif 1.4.1 decodes the same 128x128 RGB image | `4ded572af8b55afd4d876e8a129c651074a6b55e1cb4a01646a144a92fa9bcec` | MIT/Apache-2.0 (repository-generated mutation) |
| `mixed_lossless_clipped_b32_i420_60x60.avif` | Deterministic 60x60 full-range 8-bit I420 still with a qindex-zero lossless segment in the final bottom-right B32x32 leaf clipped to 28x28 | `scripts/generate_avif_mixed_lossless_clipped_b32.py`; libavif 1.4.1 / `6543b22`; patched libaom 3.13.2 / `ad44980`; Pillow 12.2.0 exact RGB reference | `ea8d484e1cde6819d1c98a64ff58db1bec7937fd929a835cf9c13c2da8430d1b` | MIT/Apache-2.0 (repository-generated) |
| `animated_mixed_lossless_inter_b32_i420_64x64.avif` | Deterministic two-frame 64x64 full-range 8-bit I420 sequence with lossy and qindex-zero lossless segments; the second frame's full-visible B32x32 leaf at MI (8,8) is inter-predicted | `scripts/generate_avif_mixed_lossless_inter_b32.py`; fixture-only libavif/libaom patches at `6543b22` / `ad44980`; encoder trace asserts block prediction mode; Pillow 12.2.0 exact RGB frames; timestamps normalized | `04c15f5bdc0e640fcfa09d22f63378d0655109db67a8f9236942c92dc0d8dbf6` | MIT/Apache-2.0 (repository-generated) |
| `animated_mixed_lossless_inter_b32_i420_60x60.avif` | Deterministic two-frame 60x60 full-range 8-bit I420 sequence with mixed lossy and qindex-zero lossless segments; the bottom-right B32x32 leaf is inter-predicted and clipped to 28x28 visible pixels | `scripts/generate_avif_mixed_lossless_inter_b32.py --canvas-size 60`; fixture-only libavif/libaom patches at `6543b22` / `ad44980`; encoder trace asserts block prediction mode; Pillow 12.2.0 exact RGB frames; timestamps normalized | `acdf18ae0fbbaf52cf6b888ab9c1f33b78b06e529bc545736d995c61d6d36ab2` | MIT/Apache-2.0 (repository-generated) |
| `animated_mixed_lossless_inter_b32_i420_residual_64x64.avif` | Deterministic two-frame 64x64 full-range 8-bit I420 sequence with an inter frame whose qindex-zero B32x32 ROI is intra-predicted and changes at all 1,024 pixels | `scripts/generate_avif_mixed_lossless_inter_b32.py --lossless-roi-residual`; fixture-only libavif/libaom patches at `6543b22` / `ad44980`; encoder trace asserts block prediction mode; Pillow 12.2.0 exact RGB frames; timestamps normalized | `58552db6049a5220936acccbb483fd0366224744e7a2f81058a099b4016d5d36` | MIT/Apache-2.0 (repository-generated) |
| `animated_mixed_lossless_inter_b32_i420_impulse_residual_64x64.avif` | Deterministic two-frame 64x64 full-range 8-bit I420 sequence with a non-skipped inter-coded qindex-zero B32x32 ROI and one lossless residual at pixel (33,33) | `scripts/generate_avif_mixed_lossless_inter_b32.py --lossless-roi-impulse-residual`; fixture-only libavif/libaom patches at `6543b22` / `ad44980`; encoder trace asserts `inter=1` and `skip_txfm=0`; Pillow 12.2.0 exact RGB frames; timestamps normalized | `321a0c17722c9e3739cf61e1bbc57bf52385b58a5588e185a760f81f00380547` | MIT/Apache-2.0 (repository-generated) |
| `animated_mixed_lossless_inter_b32_i420_10bit_impulse_residual_64x64.avif` | Deterministic two-frame 64x64 full-range 10-bit I420 sequence with lossy and qindex-zero lossless segments; the non-skipped inter-coded B32x32 ROI has one residual at pixel (33,33) | `scripts/generate_avif_mixed_lossless_inter_b32.py --bit-depth 10 --lossless-roi-impulse-residual`; fixture-only libavif/libaom patches at `6543b22` / `ad44980`; encoder trace asserts `inter=1` and `skip_txfm=0`; Pillow 12.2.0 exact RGB frames; timestamps normalized | `9a136b442ee7f742259cabc9bb1b2ce68004f9831eb4b169f08fa507f6dabbce` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_derived_skip_refs_clipped_b32x32_56x64.avif` | Deterministic five-frame 56x64 lossless 4:2:0 sequence with a clipped 24x32 right-edge B32 grid and derived skip references while frame skip mode is disabled | `scripts/generate_avif_derived_skip_reference_fixtures.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; fixed B32 controls; timestamps normalized | `b801ab1e84328dfe552543858fad84a59c7edeb7ac7497972c2499b0736d6eb5` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_skip_mode_enabled_64x64.avif` | Deterministic eight-frame lossy 64x64 4:2:0 sequence with four displayed inter frames enabling skip mode and deriving reference pairs | `scripts/generate_avif_skip_mode_enabled_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 45, speed 0, one thread, CICP 1/13/6; timestamps normalized | `98ead811784a1a8434582d6993fdc9891672543b59ed6135fbb0feabc205c8e3` | MIT/Apache-2.0 (repository-generated) |
| `ipma_version_1_wide_item_ids.avif` | Baseline still with a version-1 `ipma` FullBox and widened 32-bit item ID | `scripts/generate_test_assets.py`; Pillow 12.2.0; 2-byte box growth with relocated `iloc` extent | `90ac8f0230ddede24a72b73fb120bd46680962d4c0a7facab31e77b088cd82c5` | MIT/Apache-2.0 (repository-generated mutation) |
| `iloc_version_2_wide_item_ids.avif` | Baseline still with version-2 item locations, 32-bit item count/ID, and file-backed construction method | `scripts/generate_test_assets.py`; Pillow 12.2.0; exact decoded pixels and image info match baseline; 6-byte box growth with relocated extent | `39a494da43edb3e25dcc60c1c51c987452c66a2590e5ac9563d416d922199d87` | MIT/Apache-2.0 (repository-generated mutation) |
| `iloc_trailing_payload_ignored.avif` | Baseline still with one trailing byte after declared `meta/iloc` records | `scripts/generate_avif_iloc_trailing_payload_fixture.py`; Pillow 12.2.0 / libavif 1.4.1; exact image info and RGB pixels match baseline; `iloc` and `meta` grow by one byte with relocated extent | `d62c4cf7da0e986cf90225f63160fb4ff8bd851cf0d2a0c2957bc48ef7ce4504` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_iloc_file_extent_out_of_bounds.avif` | Baseline still whose file-backed `iloc` primary-item extent ends one byte past EOF; Pillow opens, inspects, and verifies it, then reports `Truncated data` on decode | `scripts/generate_avif_file_extent_fixture.py`; Pillow 12.2.0 / libavif 1.4.1; only the four-byte extent length changes | `764f4205c7b60acd296e37e9d2e45968c66aafd4fc84102366d67e9903d237ad` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_iloc_missing_idat.avif` | `iloc_idat_primary_item.avif` retains its method-one primary-item location while the required `idat` box is removed; Pillow opens, verifies, and reports `Failed to decode frame 0: No content` on load | `scripts/generate_avif_idat_fixture.py`; only the `idat` box is removed, while file-backed AV1 bytes remain unreferenced | `07936109dabbd3cec1a41922bcfedecb88f89b74c022f51bbdeb1eff460f7e75` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_iloc_empty_extent_list.avif` | `baseline.avif` with the primary item’s `iloc` record declaring zero extents; Pillow reports an empty image item | `scripts/generate_avif_idat_fixture.py`; removes the one eight-byte extent record while preserving all other boxes and encoded media | `8ba35ad56552287696f559c31565b8b013592f1c0b440a732cf0b8301bd023a6` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_primary_item_irot_invalid_value.avif` | `primary_item_irot.avif` with its rotation value changed from 3 to 4; Pillow rejects the invalid transform | `scripts/generate_avif_irot_edge_fixtures.py`; one-byte property mutation with encoded AV1 samples preserved | `feb3a381de36eac15916eaab142e4c4aff6e6243bb421565465fa98ab9ae376a` | MIT/Apache-2.0 (repository-generated mutation) |
| `ipma_wide_associations.avif` | Baseline still with 16-bit `ipma` property-association indices | `scripts/generate_test_assets.py`; Pillow 12.2.0; 4-byte box growth with relocated `iloc` extent | `7bef3a5c1db6196b199c03406719f5dcd4b0bb6a7320de44b39a7daa52e6116b` | MIT/Apache-2.0 (repository-generated mutation) |
| `primary_item_irot_pasp_4x3.avif` | `primary_item_irot.avif` with an associated 4:3 pixel-aspect-ratio property | `scripts/generate_test_assets.py`; Pillow 12.2.0; 17-byte metadata growth with relocated `iloc` extent | `2acc2e59572718a5fd1247aef7d18ab7666943533f068c221117df05378c4564` | MIT/Apache-2.0 (repository-generated mutation) |
| `primary_item_irot_trailing_payload.avif` | `primary_item_irot.avif` with one trailing zero byte on `irot`; Pillow ignores the extension and preserves exact pixels | `scripts/generate_avif_irot_edge_fixtures.py`; one-byte property growth with relocated `iloc` extent and unchanged AV1 samples | `49bb8805f73129fd022c97525f03faebfaf2517e74e90423180b3a06bdc6c5c9` | MIT/Apache-2.0 (repository-generated mutation) |
| `primary_item_pasp_zero_horizontal_spacing.avif` | 4:3 `pasp` property with zero horizontal spacing; Pillow ignores the unusable aspect ratio and decodes unchanged pixels | `scripts/generate_avif_pasp_compat_fixtures.py`; 342 bytes; encoded AV1 samples are unchanged | `1d8ad709f18d1bed97ca2e44178a8b9580aeb79763ea9543d4bc3ef73d62d799` | MIT/Apache-2.0 (repository-generated mutation) |
| `primary_item_pasp_zero_vertical_spacing.avif` | 4:3 `pasp` property with zero vertical spacing; Pillow ignores the unusable aspect ratio and decodes unchanged pixels | `scripts/generate_avif_pasp_compat_fixtures.py`; 342 bytes; encoded AV1 samples are unchanged | `3f8eef88f63387e8de633f5783d56e3ca8ae6d0ccc5a397969377c2bda3f220a` | MIT/Apache-2.0 (repository-generated mutation) |
| `primary_item_pasp_trailing_byte.avif` | `pasp` property with one trailing `0xff` byte; Pillow ignores the extra payload and decodes unchanged pixels | `scripts/generate_avif_pasp_compat_fixtures.py`; 343 bytes; encoded AV1 samples are unchanged and file-backed `iloc` is rebased | `e62a08cb8bcb753fcae35739316bfc5ee4c0943879f4d9f6d0ecf087dee2f5c7` | MIT/Apache-2.0 (repository-generated mutation) |
| `primary_item_duplicate_transforms_first_wins.avif` | `baseline.avif` with conflicting duplicate `irot`, `imir`, `pasp`, and `clap` properties; Pillow 12.2.0 retains the first value of each kind | `scripts/generate_avif_duplicate_transform_properties.py`; the eight property associations are essential and the primary-item extent is rebased | `c6b9cfc406f9fd210aa2817d6311791ce27929ef0b0173c06e9b2945b82eeae7` | MIT/Apache-2.0 (repository-generated mutation) |
| `primary_item_imir_top_bottom.avif` | `baseline.avif` with an essential primary-item `imir` axis 0 property; Pillow exposes EXIF orientation 4 and preserves decoded pixels | `scripts/generate_avif_imir_fixtures.py`; 10-byte metadata growth with relocated `iloc` extent; Pillow 12.2.0 exact reference | `b129f71ceda1490258d8fbdbec9885ab5ea29a82e9a453aa000290e366e8f3df` | MIT/Apache-2.0 (repository-generated mutation) |
| `primary_item_imir_left_right.avif` | `baseline.avif` with an essential primary-item `imir` axis 1 property; Pillow exposes EXIF orientation 2 and preserves decoded pixels | `scripts/generate_avif_imir_fixtures.py`; 10-byte metadata growth with relocated `iloc` extent; Pillow 12.2.0 exact reference | `5535e699843b31a97921316661e3cacb4826383638f88f0462a55da5aafe5d5d` | MIT/Apache-2.0 (repository-generated mutation) |
| `primary_item_imir_trailing_byte.avif` | `baseline.avif` with an essential primary-item `imir` axis 0 and trailing `0xff`; Pillow exposes EXIF orientation 4 and preserves decoded pixels | `scripts/generate_avif_imir_fixtures.py`; 11-byte metadata growth with relocated `iloc` extent; Pillow 12.2.0 exact reference | `891bd77c35c719432f2632f87c0f5633aa91dacdc3c068cf99d5834c8c5fc9f1` | MIT/Apache-2.0 (repository-generated mutation) |
| `colr_unknown_type_ignored.avif` | `baseline.avif` with the primary-item `nclx` type changed to unknown `abcd`; Pillow ignores the color property and returns exact baseline RGB pixels | `scripts/generate_avif_colr_edge_fixtures.py`; one four-byte property-type mutation | `fa2c4c6af711605a8f72893a3242abb49380ca276957b09c22516f49a0cbd8c8` | MIT/Apache-2.0 (repository-generated mutation) |
| `colr_nclx_trailing_payload_ignored.avif` | `clap_trailing_payload_byte_ignored.avif` with its trailing zero moved to the `nclx` property; Pillow returns exact control RGB pixels | `scripts/generate_avif_colr_edge_fixtures.py`; preserves `ipco`, parent-box, and file sizes | `7922155e4e99e6fca5c2fb7a8786c0507cb2e9e8552b80e389a7ab196388774a` | MIT/Apache-2.0 (repository-generated mutation) |
| `clli_trailing_payload_ignored.avif` | `hdr.avif` with one trailing zero byte on its primary-item CLLI property; Pillow preserves complete HDR image info and decoded RGB bytes | `scripts/generate_avif_clli_trailing_payload_fixture.py`; grows nested metadata by one byte and rebases all five file-backed item extents | `7862991cf4424c784f22d4a27ef128949eea8ad927c8ad05eed23a6e9effd108` | BSD-2-Clause (derived from libavif `hdr.avif`) |
| `error_colr_reserved_flag_bits.avif` | `baseline.avif` with reserved bit 0 set in the primary-item `nclx` flags; Pillow rejects it at open | `scripts/generate_avif_colr_edge_fixtures.py`; one flag-bit mutation | `638cf1dcfaedcd051f2b7250207e1fd38dcc3f622be048ddd1e2c3432933e51c` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_colr_empty_icc_profile.avif` | `icc_profile.avif` with an empty `prof` payload and a same-size trailing `free` property; Pillow rejects the empty profile at open | `scripts/generate_avif_colr_edge_fixtures.py`; preserves `ipco`, parent-box, and file sizes | `d9aa1a6a31622983ea3888c982a33cf5b82bb84a28d6e3b151ac41eac49ab111` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_ipco_property_size_below_header.avif` | `hdr.avif` with the first `ipco/ispe` property's declared size changed from 20 to 4 bytes; Pillow rejects the malformed nested box at open | `scripts/generate_avif_ipco_property_size_below_header.py`; one size-field mutation, same file length and unchanged AV1 media payload | `1376cc7d7360eae6b03c97b6d3808dc95bfa945aa8578e4443c540489ea2ed20` | BSD-2-Clause (derived from libavif `hdr.avif`) |
| `icc_profile.avif` | Repository-generated 4x4 RGB portable-pattern still using the profile from `png/iccp.png` | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 8, 4:4:4, one thread, CICP 1/13/6, autotiling disabled | `a59749aa4985d4954cb8ef8f513e17b40f1051f969374eff7a654aeb0a629d95` | MIT/Apache-2.0 (repository-generated) |
| `icc_ricc_type.avif` | Full-file `prof` to `rICC` property-type variant; Pillow accepts the same profile bytes and RGB pixels | Pillow 12.2.0; one four-byte AVIF property mutation | `36e07c9ec8afd80be79396196b3fec7397caec1cec3bc5f89e0064d98a87ea9b` | MIT/Apache-2.0 (repository-generated) |
| `icc_mdcv.avif` | ICC still with one associated BT.2020/D65 mastering-display property | Pillow 12.2.0; deterministic `mdcv` property/association insertion with relocated item extent | `d06f268e6f7341d67e1bf62510681375bc40ea4815a27b6aa4e38172dde01882` | MIT/Apache-2.0 (repository-generated) |
| `icc_mdcv_trailing.avif` | ICC still whose associated `mdcv` property has one trailing byte | Pillow 12.2.0; deterministic bounded property extension with relocated item extent | `956465627904603da6df5c6ce0419cb401156dff57cf2a5ab28a332009d0e946` | MIT/Apache-2.0 (repository-generated) |
| `duplicate_mdcv_first_wins.avif` | ICC still with conflicting duplicate primary-item `mdcv` properties; Pillow preserves pixels and ICC metadata | `scripts/generate_avif_duplicate_mdcv_property.py`; second property association and relocated item extent; first-property metadata projection is asserted against the specification-backed control | `a81d3f13b2412488464d461f9c81ab496ce5758bfba3f5f94de237b37b77faa8` | MIT/Apache-2.0 (repository-generated mutation) |
| `empty_xmp_metadata_item_extent.avif` | `baseline.avif` with an empty XMP MIME item and no extents; Pillow ignores the item and preserves decoded pixels | `scripts/generate_avif_empty_xmp_metadata_extent.py`; adds one `infe` entry and zero-extent `iloc` record, then rebases the unchanged primary AV1 item | `98c1a14618e4577354d82b758a8840d079990364b10ce94b6ef49debc87c1642` | MIT/Apache-2.0 (repository-generated mutation) |
| `xmp_metadata_item_pixi_only.avif` | `empty_xmp_metadata_item_extent.avif` with the existing `pixi` property associated with its empty XMP item, without an `ispe`; Pillow ignores the metadata item and preserves decoded pixels | `scripts/generate_avif_empty_xmp_metadata_extent.py`; adds one narrow `ipma` association and rebases the unchanged primary AV1 item | `2626a70cdd4b505b30e55245e9068ba17994b0c4bafdf57c00221c54940d8e12` | MIT/Apache-2.0 (repository-generated mutation) |
| `trailing_uuid_zero_size_ignored.avif` | `baseline.avif` followed by an open-ended unknown UUID box with a 16-byte user type; Pillow ignores the box and preserves decoded pixels | `scripts/generate_avif_trailing_uuid_fixture.py`; 24-byte top-level UUID box appended after all baseline bytes | `5fe9b4f99f994e4424ecc8467d12b42b1d6c7f41fa1ad48f1e78eafb4919df88` | MIT/Apache-2.0 (repository-generated mutation) |
| `trailing_uuid_extended_size_ignored.avif` | `baseline.avif` followed by an unknown UUID box using the extended 64-bit size form; Pillow ignores the box and preserves decoded pixels | `scripts/generate_avif_trailing_uuid_fixture.py`; 32-byte top-level extended-size UUID box appended after all baseline bytes | `2e3a871c9481898f7f6ab78fc0d79c6abc24924b00d8c65871b6fdee90e73966` | MIT/Apache-2.0 (repository-generated mutation) |
| `trailing_uuid_oversized_ignored.avif` | `baseline.avif` followed by a UUID box declaring a 4 GiB size with only its header and user type present; Pillow ignores the incomplete trailing box and preserves decoded pixels | `scripts/generate_avif_trailing_uuid_fixture.py`; 32-byte top-level UUID box with an oversized 64-bit declaration | `667587f4cdaecbf61f8be838eee71042a3205c276d229aa080f7c6fb32b3ffc3` | MIT/Apache-2.0 (repository-generated mutation) |
| `unknown_top_level_box_ignored.avif` | 8x8 lossless I420 still followed by a bounded `zunk` top-level box; Pillow preserves exact RGB pixels | `scripts/generate_avif_unknown_top_level_box.py`; 12-byte box appended after the complete still | `ebd1e037026559f6a2f7dc12271c94a7440b2c06c79843eed5380afb92072d64` | MIT/Apache-2.0 (repository-generated mutation) |
| `trailing_box_size_below_header_ignored.avif` | `baseline.avif` followed by a `free` box declaring size 4, smaller than its 8-byte header; Pillow ignores the incomplete trailing box and preserves decoded pixels | `scripts/generate_avif_trailing_box_size_below_header.py`; appends only the 8-byte invalid box header | `d99d5e1018f23c5251d163ea6352c2ac6f9a0559ea5f21748b3ddd0f7ba493bf` | MIT/Apache-2.0 (repository-generated mutation) |
| `alpha.avif` | Pillow `Tests/images/avif/transparency.avif` | Pillow 12.2.0 / `3c41c09` | `b19f57d9421bbd3d0b0706c8fe79cef802aebf106afefa6bddde9de1a07509c9` | MIT-CMU (`third_party/pillow/LICENSE`) |
| `duplicate_alpha_auxl_reference_first_wins.avif` | `alpha.avif` with a duplicate identical `auxl` relationship to its alpha item; Pillow keeps the first relationship and returns identical RGBA pixels | `scripts/generate_avif_container_edge_fixtures.py`; Pillow 12.2.0 exact RGBA reference; both encoded AV1 extents are unchanged | `680204d0a9bb9dd0f058b2539e0ccb527c265f6b2ee5e863f2f3674fbe9d03a6` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_alpha_auxiliary_missing_ispe.avif` | `alpha.avif` with item 2's `ispe` property association set to zero while its `pixi`, `av1C`, and `auxC` associations remain intact; Pillow rejects the complete file at open | `scripts/generate_avif_alpha_missing_auxiliary_ispe_fixture.py` | `15755192d922569479398381a910a6a73bc9d964e37acf89c26633439ccc0b0c` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_alpha_auxiliary_missing_ispe_and_pixi.avif` | `alpha.avif` with item 2's `ispe` and `pixi` property associations set to zero while its `av1C` and `auxC` associations remain intact; Pillow rejects the complete file at open | `scripts/generate_avif_alpha_missing_auxiliary_ispe_fixture.py` | `e28737256c72dd7b31b3377667fb3fabfc5efeb5cd8dcfed5d12847c0642fe48` | MIT/Apache-2.0 (repository-generated mutation) |
| `duplicate_non_primary_ispe_first_wins.avif` | `alpha.avif` with a second item 2 `ispe` declaring 32×64 after the original 64×64 extent; Pillow preserves the first extent and RGBA output | `scripts/generate_avif_non_primary_property_duplicate_fixtures.py` | `c1e7a5c8654652a117bb6e11d1e3ad1715ba18adb5191a1a53975be70ea86c9c` | MIT/Apache-2.0 (repository-generated mutation) |
| `duplicate_non_primary_pixi_first_wins.avif` | `alpha.avif` with a second item 2 `pixi` declaring 16-bit after the original 8-bit depth; Pillow preserves the first depth and RGBA output | `scripts/generate_avif_non_primary_property_duplicate_fixtures.py` | `ed581a79d3937c870b06bd1a830ddc31c4af20660da249059fc1b549c4194d52` | MIT/Apache-2.0 (repository-generated mutation) |
| `duplicate_non_primary_av1c_first_wins.avif` | `alpha.avif` with a second item 2 `av1C` declaring 10-bit depth after the original 8-bit configuration; Pillow preserves the first declaration and exact RGBA output | `scripts/generate_avif_non_primary_property_duplicate_fixtures.py` | `5c06b67ebc29379bc0dd58c61865dd28927ab3669d59d75a08afcdeb7bcd832f` | MIT/Apache-2.0 (repository-generated mutation) |
| `alpha_item_icc_profile_ignored.avif` | `alpha.avif` with a `prof` ICC color property associated only with auxiliary alpha item 2; Pillow preserves the exact RGBA pixels | `scripts/generate_avif_non_primary_property_duplicate_fixtures.py`; adds the pinned small profile from `icc_profile.avif` and relocates both file-backed item extents | `4dfa9e87d65e9365b3e38ab13e67bdceadd9b8eb5ddef273da03eb3057b553d3` | MIT/Apache-2.0 (repository-generated mutation) |
| `iref_version_1_alpha_auxl.avif` | `alpha.avif` with its direct `auxl` item reference widened to version 1 / 32-bit item IDs; both file-backed `iloc` extents are relocated after the four-byte `iref` growth | `scripts/generate_avif_iref_v1_alpha_fixture.py`; Pillow 12.2.0 exact 64×64 RGBA reference | `b6f3b78c98b33b32b679d07a91b3f3030aba7c49def8bc8bad8b64243eb2c710` | MIT/Apache-2.0 (repository-generated mutation) |
| `10bit.avif` | libavif `tests/data/colors-animated-12bpc-keyframes-0-2-3.avif` | libavif 1.4.1 / `6543b22` | `3bf9f91da471749e7df639ba7945d4d94c1c3e3968c26f3619fbbcfc92790576` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `sequence_alpha_duration_mismatch.avif` | `10bit.avif` with only the auxiliary alpha-track sample delta changed from 1 to 2; Pillow retains five RGBA frames with color-track durations | `scripts/generate_test_assets.py`; Pillow 12.2.0 / libavif 1.4.1 parity | `b70e900cbe163b4e394aaa0bab6f283fec8f7f0093cc8dbf9ea790b85103902d` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `sequence_alpha_timescale_mismatch.avif` | `10bit.avif` with only the auxiliary alpha track's `mdhd` timescale changed from 1 to 2; Pillow preserves five RGBA frames and uses color-track durations | `scripts/generate_test_assets.py`; Pillow 12.2.0 / libavif 1.4.1 parity | `4aa9b40d1971afcb9d26c5beb312f8973fc8f1f69c2cf64eafa4fcaeaf3acf69` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_duplicate_alpha_track.avif` | `10bit.avif` with its auxiliary alpha track duplicated under a distinct track ID and all file-backed item/chunk offsets rebased; Pillow preserves all five RGBA frames | `scripts/generate_avif_sequence_selection_fixtures.py`; Pillow 12.2.0 / libavif 1.4.1 parity | `a1635ad5da1194bed67af764f88f49801cc4afbe4711c71eb9b59404eca4c89a` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `sequence_alpha_sample_count_mismatch.avif` | `10bit.avif` with only the auxiliary alpha track's sample-to-chunk count changed from five to four; Pillow decodes the first four RGBA frames and errors while loading frame five | `scripts/generate_test_assets.py`; Pillow 12.2.0 / libavif 1.4.1 parity | `89a13baeb503ace10bea0f07d9c2aa296235d2ca8ed0471f3ee99751deccafcf` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_sequence_no_color_track_candidate.avif` | `10bit.avif` with its color handler changed to `soun` and its sample-entry type changed from `av01` to `zzzz`; Pillow rejects the file at open | `scripts/generate_avif_sequence_selection_fixtures.py`; two four-byte box-field mutations | `8cde6b3f382eb4bc9b19b9ebbb2256a42d2d98f2f2dc607ab1cf1884deacd5b8` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `high_bitdepth_still_12bit_444_lossless.avif` | Deterministic 16x16 RGB source encoded as a single-frame 12-bit 4:4:4 witness | libavif 1.4.1 / `6543b22`; libaom 3.13.2 fixture build | `8645ee1ecc437868c5842248444ea6c8400d983a03bcfe75710bb0a424915abd` | MIT/Apache-2.0 (repository-generated) |
| `high_bitdepth_still_12bit_444_alpha_9x9.avif` | Deterministic 9x9 12-bit YUV444 still with a monochrome 12-bit auxiliary alpha item; both AV1 frames are all-lossless | `scripts/generate_av1_encoder_refs.py` case `high12_444_alpha_9x9`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread, CICP 1/13/6 | `f7eb5b9edfdd7b11d00094aadb91320e7cc6cc01961470e3c2040d91e68610aa` | MIT/Apache-2.0 (repository-generated) |
| `high_bitdepth_still_12bit_420_alpha_odd_edge_17x9.avif` | Deterministic 17x9 full-range 12-bit YUV420 still with a 12-bit auxiliary alpha item; varied decoded Y/U/V low nibbles include the odd right and bottom chroma edges | `scripts/generate_avif_12bit420_alpha_odd_edge.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 0, one thread, CICP 1/13/6; Pillow 12.2.0 exact RGBA reference | `ce408e4a66fe57b734544c2f30f17e90f725e2f2bae75d748f9eda75bdf23761` | MIT/Apache-2.0 (repository-generated) |
| `error_12bit420_alpha_ispe_extent_mismatch.avif` | The pinned 17x9 12-bit YUV420 alpha AVIF with a separate 16x9 auxiliary `ispe`; encoded AV1 item bytes are preserved and item extents are rebased | `scripts/generate_avif_12bit420_alpha_ispe_mismatch.py`; Pillow 12.2.0 reports `Failed to decode frame 0: Decoding of alpha plane failed` | `9150d49f7f499f86c9e778a70d6525bd06a5509e409366d082097e6d7c4a2be6` | MIT/Apache-2.0 (repository-generated mutation) |
| `profile2_identity_12bit_444.avif` | Deterministic 16x16 RGB source encoded as a profile-2 12-bit full-range 4:4:4 identity-CICP still | `scripts/generate_avif_profile2_identity.py`; libavif 1.4.1 / libaom 3.13.2; `--qcolor 100 --cicp 1/13/0 --range full --speed 8 --jobs 1` | `1ce2aae5539074e3a65092f185c4db34537462d637859d4fd85449bb15557931` | MIT/Apache-2.0 (repository-generated) |
| `profile2_identity_12bit_444_9x9.avif` | The pinned 16x16 identity-CICP still with its AV1 sequence and item spatial extent reduced to 9x9 | `scripts/generate_avif_profile2_identity_tail.py`; Pillow 12.2.0 exact RGB reference; frame payload preserved | `2329ba1c0b8cb384727b1a3c0b817998fee4da5a3aa0254801afdcefb2f70c0d` | MIT/Apache-2.0 (repository-generated mutation) |
| `high_bitdepth_still_10bit_420_lossy_16x16.avif` | Deterministic 16x16 planar 10-bit I420 source encoded as a single-frame lossy profile-0 AVIF | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980` | `60c40a86c0409add22d7ab678b579ea4ccf979624bf3b127a4448d0a83e18d18` | MIT/Apache-2.0 (repository-generated) |
| `high_bitdepth_still_10bit_420_alpha_lossless_16x16.avif` | Deterministic 16x16 planar 10-bit YUV420 still with a monochrome 10-bit auxiliary alpha item; both AV1 frames all-lossless | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980` | `8c3cb86055e1003d91897d986995dd2ee030d0278b82d3d7c9ddc783189eb76f` | MIT/Apache-2.0 (repository-generated) |
| `high_bitdepth_still_10bit_422_alpha_lossless_16x16.avif` | Deterministic 16x16 planar 10-bit YUV422 still with a monochrome 10-bit auxiliary alpha item; both AV1 frames all-lossless | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980` | `eef1ffae7961354032dad642c1ebfc146cbcb28f7147c4dce6d9b8a4de17d392` | MIT/Apache-2.0 (repository-generated) |
| `high_bitdepth_still_10bit_444_alpha_lossy_16x16.avif` | Deterministic 16x16 planar 10-bit YUV444 still with a monochrome 10-bit auxiliary alpha item | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980` | `468bd5af2f96693582d6618572dc353cac4ee16719cebebb8e20ff7e81049548` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_b16x16_10bit.avif` | Deterministic two-frame full-range 10-bit I420 lossless sequence with a one-pixel-shifted near-full-frame texture | `scripts/generate_avif_10bit_lossless_inter_fixture.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; timestamp-normalized | `8eb14ad3c57916b9e7fdded28b08a9254302cfc76f0cb5a934b41f11dc4184b6` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_b4x4_leaves_64x64.avif` | Deterministic two-frame 64x64 8-bit I420 lossless sequence with fixed 4x4 inter partitions and a moving luma patch | `scripts/generate_avif_lossless_inter_b4x4_fixture.py` and `scripts/avif_fixture_oracle/encode_lossless_inter_i420_b4x4.c`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; timestamp-normalized | `8c588829bd49a125fee501c9cdcaa1e6f9c0e6f058084270eaa039688acc9748` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_b16x16_10bit_small_patch.avif` | Deterministic two-frame full-range 10-bit I420 lossless sequence with a moving 4x4 luma patch | `scripts/generate_avif_10bit_lossless_inter_fixture.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; timestamp-normalized | `99dd72e3dc39af9f7d9bcb89d5e3bb0553f7cc04db021c0107fe2f4dcd06ae7b` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_b32x32_10bit.avif` | Deterministic two-frame full-range 10-bit I420 lossless sequence with a translated patch | `scripts/generate_avif_10bit_lossless_inter_fixture.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; timestamp-normalized | `c90bf9e75b0c091e19ab6b8aa70c17243ae1dbbf818493e815015f998f14e73a` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_monochrome_b32x32_10bit.avif` | Deterministic two-frame 32x32 full-range 10-bit monochrome lossless sequence with a translated patch | `scripts/generate_avif_10bit_lossless_inter_fixture.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; timestamp-normalized | `28427120f6a0e9de71d0abcfe36bbeaae4d701dacefb62f852dfd71eeb82d624` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_qmatrix_level15_qindex0_16x64.avif` | Deterministic two-frame full-range 8-bit I420 lossless sequence with qindex zero and qmatrix level 15 | `scripts/generate_avif_lossless_qmatrix_sequence.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; speed 0, one thread, CICP 1/13/6; timestamp-normalized | `b76924ec8cf9fa01ad27e48dec5990332b089d75460fa7bf37d9fa00108100de` | MIT/Apache-2.0 (repository-generated) |
| `lossy_420_qmatrix_level15_qindex76_16x64.avif` | Deterministic single-frame full-range 8-bit I420 texture with qindex 76 and uniform qmatrix level 15 | `scripts/generate_avif_lossy_qmatrix_fixture.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 75, speed 0, one thread, CICP 1/13/6 | `23cdcad6c54eb49db6a5f8a28b29bb5504ec64cb0ef8489f8ba7216f2893a43d` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_b32x32_10bit_partition32_full_texture.avif` | Deterministic two-frame 32x32 full-range 10-bit I420 lossless sequence with fixed 32-pixel partitions and a translated 31x31 textured patch | `scripts/generate_avif_10bit_lossless_inter_fixture.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; timestamp-normalized | `992e7bfe661bcdc1390127ae716059acb0f5ecdd79f12b5869eaa7bb89831257` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b32x32_10bit_partition32_28x64.avif` | Deterministic two-frame 28x64 full-range 10-bit I420 lossless sequence with a translated textured 24x24 patch in the padded left B32x32 leaf | `scripts/generate_avif_10bit_lossless_inter_fixture.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; timestamp-normalized | `8f2b787faea151faf9c7f69c2b92e098dedd3bb70f28fd33546194c002b3456f` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b32x32_10bit_partition32_56x64.avif` | Deterministic two-frame 56x64 full-range 10-bit I420 lossless sequence with fixed 32-pixel partitions and a translated textured patch in the clipped 24x32 right-edge B32x32 leaf | `scripts/generate_avif_10bit_lossless_inter_fixture.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; timestamp-normalized | `a819711996c804d7c5e4dcc26cee2e6a35bb82c48a88a6db8e34bd61d3b5f9fe` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b32x32_10bit_partition32_63x64.avif` | Deterministic two-frame odd-width 63x64 full-range 10-bit I420 lossless sequence with fixed 32-pixel partitions and a translated textured patch in the 31-pixel right-edge B32x32 leaf | `scripts/generate_avif_10bit_lossless_inter_fixture.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; timestamp-normalized | `248ea28edff232b792c3f9e71b05ae6762f0f807b9981cd91ba9ea7ff19da42f` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b32x32_12bit_partition32_56x64.avif` | Deterministic two-frame 56x64 full-range 12-bit I420 lossless sequence with fixed 32-pixel partitions and a translated textured patch in the clipped 24x32 right-edge B32x32 leaf | `scripts/generate_avif_10bit_lossless_inter_fixture.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; timestamp-normalized | `cec5a564546c2ef8893c3f4e44c7f46c49ea9daa89d1f782a06717f11d183c87` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_wide_grid_b128x128.avif` | Deterministic two-frame 128x128 RGB sequence encoded as all-lossless I420 with 128x128 AV1 superblocks and a wide inter-grid witness | `scripts/generate_avif_lossless_wide_inter_grid_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread; timestamp-normalized | `e44c5a53a1cf185a29b4a6570836c82078d1cd2e1d0d0963e69ebf1028f7d47f` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_monochrome_clipped_b128x128_124x128.avif` | Deterministic two-frame 124x128 8-bit all-lossless monochrome sequence whose 128x128 inter block is clipped by four columns at the right edge | `scripts/generate_avif_lossless_monochrome_clipped_wide_inter_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread; timestamp-normalized | `7553404e35f1583e5bb2e785acb4908de5a4bbfa8d888054b163cc8da32391e8` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_compound_wide_i420_b128x128.avif` | Deterministic three-frame 128x128 8-bit I420 all-lossless sequence whose final B128x128 inter block selects compound prediction from both reference frames | `scripts/generate_avif_lossless_compound_wide_i420.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread; timestamp-normalized | `7e736d84c4a9477930c013517e4f3a8e4ad759ebbcf524ecbb67240b8bbfaefc` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_compound_vertical_i420_b64x128.avif` | Deterministic three-frame 128x128 8-bit I420 all-lossless sequence with a fixture-forced vertical B64x128 partition and a final inter frame referencing both prior frames | `scripts/generate_avif_lossless_compound_tall_i420.py`; fixture-only patched libaom/libavif at `ad44980` / `6543b22`; Pillow 12.2.0 exact RGB reference; timestamp-normalized | `eb40e16aa47a612c272e29c2ad2cd62a7dc6115cce8cd40da1f1a01f95ec166a` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_vertical_b64x128.avif` | Deterministic two-frame 128x128 full-range 8-bit I420 all-lossless sequence with a forced vertical partition of the AV1 root into two B64x128 leaves | `scripts/generate_avif_lossless_inter_i420_b64x128.py`; fixture-only patched libavif/libaom at `6543b22` / `ad44980`; Pillow 12.2.0 exact RGB reference; timestamp-normalized | `9ed8dd60feedcb8480bb8d6de7f8ea2ec23583da923e916e28a128c3746ca650` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_superres_sgr_10bit_160x56.avif` | Deterministic two-frame full-range 10-bit I420 lossy sequence; frame 1 uses super-resolution and SGR restoration | `scripts/generate_avif_hi10_superres_restoration.py`; libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `016e4a8433002b60899744fba6f26a7c10af82c192f65b4c5c19233b15c3cb11` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_444_superres_sgr_10bit_160x56.avif` | Deterministic two-frame full-range 10-bit I444 lossy sequence; frame 1 is coded at 128x56, super-resolved to 160x56, and uses SGR restoration on all planes | `scripts/generate_avif_hi10_i444_superres_restoration.py`; libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `d0f13c712995e0464efb51575ca353aabd26c8ff6e3001c415fd98f239c8221c` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_422_superres_sgr_10bit_160x56.avif` | Deterministic two-frame full-range 10-bit I422 lossy sequence; frame 1 is coded at 128x56, super-resolved to 160x56, uses `TX_MODE_LARGEST`, and applies SGR restoration to all planes | `scripts/generate_avif_hi10_i422_superres_restoration.py`; libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `60748f33355d8f8b54f48942e319c1791e75c75c94da55a1568fd6679edf578a` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_422_superres_12bit_160x56.avif` | Deterministic two-frame full-range 12-bit I422 lossy sequence; frame 1 is coded at 128x56, super-resolved to 160x56, uses `TX_MODE_LARGEST`, and has restoration disabled | `scripts/generate_avif_12bit_i422_superres.py`; libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `565ad0b866eb0fd25b37c5b23e0c3dba8ab791312e782cf4d58fd80a92684db9` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_superres_sgr_8bit_160x56.avif` | Deterministic two-frame full-range 8-bit I420 lossy sequence; frame 1 uses super-resolution and SGR restoration with largest-transform mode and 16x16 partitions | `scripts/generate_avif_8bit_superres_restoration.py`; patched pinned fixture encoder; libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `c294163610d4a45852fe374e0345c878979bb81e5ea94596960ef64411180fd7` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_422_superres_sgr_8bit_160x56.avif` | Deterministic two-frame full-range 8-bit I422 lossy sequence; frame 1 is coded at 128x56, super-resolved to 160x56, and uses largest-transform mode with SGR restoration on all planes | `scripts/generate_avif_8bit_i422_superres_restoration.py`; pinned libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `a3f55297651b8659e9a76fda1276a2a6330151d802af1e72cdb659858cfcb1a1` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_422_cdef_8bit_16x16.avif` | Deterministic two-frame full-range 8-bit I422 lossy sequence with a 16x16 inter frame using active CDEF and no restoration | `scripts/generate_avif_inter_422_cdef_fixture.py`; libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `84446e8a38a5ad9a742f6e4274a0b8982f07fd4ceb778787b10abe7dcc420c28` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_444_superres_sgr_8bit_160x56.avif` | Deterministic two-frame full-range 8-bit I444 lossy sequence; frame 1 is coded at 128x56, super-resolved to 160x56, and uses largest-transform mode with SGR restoration on all planes | `scripts/generate_avif_8bit_i444_superres_restoration.py`; pinned libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `8c3323e3971c653b41a16e6c12c6a12cf512c1247d8a399d97e2f1be2b544716` | MIT/Apache-2.0 (repository-generated) |
| `superres_equal_width_16x16.avif` | Deterministic 16x16 full-range 8-bit I420 lossy still; AV1 signals super-resolution denominator 9 while minimum coded width leaves the coded and upscaled widths equal | `scripts/generate_avif_equal_width_superres.py`; patched pinned fixture encoder; libavif 1.4.1 / libaom 3.13.2 | `2de74d720f8863be43049a3df776337c1cde494122a57093ffad532cea19a004` | MIT/Apache-2.0 (repository-generated) |
| `superres_equal_width_i444_16x16.avif` | Deterministic 16x16 full-range 8-bit I444 lossy still; AV1 signals super-resolution denominator 9 with equal coded and upscaled widths, exercising full-resolution chroma planes | `scripts/generate_avif_equal_width_superres.py --chroma 444`; patched pinned fixture encoder; libavif 1.4.1 / libaom 3.13.2 | `cd02b86f213b96f9d58bec3781f8855f325aa7b4c867a6fbf76f5802886420a4` | MIT/Apache-2.0 (repository-generated) |
| `superres_actual_upscaled_i444_32x16.avif` | Deterministic 32x16 full-range 8-bit I444 lossy still; AV1 signals denominator 9 and coded width 28, exercising real resizing of luma and both full-resolution chroma planes | `scripts/generate_avif_equal_width_superres.py --chroma 444-actual`; patched pinned fixture encoder; libavif 1.4.1 / libaom 3.13.2 | `c976ff5f3a4669557d6dc45780743ab462f1766de732c7914d5094663e323948` | MIT/Apache-2.0 (repository-generated) |
| `animated_mixed_lossless_superres_444_10bit_loopfilter_80x48.avif` | Two-frame 80x48 10-bit 4:4:4 mixed lossless/lossy sequence with coded inter width 64, denominator-10 super-resolution, and active Y/U/V deblocking on both frames; exact Pillow RGB frame references | `scripts/generate_avif_mixed_superres_fixtures.py --variant animated_mixed_lossless_superres_444_10bit_loopfilter_80x48`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 45, speed 10; timestamp-normalized | `e7c75ed78f3ba1ef3bcd4c56c32e7ea832901cfcc41b0282a05c1bce4cfda646` | MIT/Apache-2.0 (repository-generated) |
| `superres_actual_upscaled_i420_no_restoration_17x17.avif` | Deterministic 17x17 full-range 8-bit I420 lossy still; AV1 signals denominator 9 and has no active restoration, exercising intra reconstruction for actual super-resolution | `scripts/generate_avif_equal_width_superres.py --chroma 420-actual-no-restoration`; patched pinned fixture encoder; libavif 1.4.1 / libaom 3.13.2 | `175f4eed6919458cd0f3af455f20e6a0be83bfd059b2e63e745ad8fab3e46c35` | MIT/Apache-2.0 (repository-generated) |
| `superres_actual_upscaled_monochrome_17x16.avif` | Deterministic 17x16 full-range 8-bit 4:0:0 lossy still; AV1 signals denominator 9 and resizes its monochrome plane from coded width 16 to visible width 17 | `scripts/generate_avif_equal_width_superres.py --chroma 400-actual`; patched pinned fixture encoder; libavif 1.4.1 / libaom 3.13.2; Pillow 12.2.0 exact RGB reference | `d2c485bd5da5d3ea1a6c7af4c95807176f71949c312ca37756caed3e2eae6446` | MIT/Apache-2.0 (repository-generated) |
| `animated_monochrome_superres_sequence_17x16.avif` | Two-frame 17x16 full-range 8-bit monochrome AVIF sequence with single-unit SGR restoration; key and inter frames resize coded width 16 to visible width 17 with denominator 9 | `scripts/generate_avif_monochrome_superres_sequence.py`; pinned libavif 1.4.1 / libaom 3.13.2 with repeatable timestamps; Pillow 12.2.0 exact RGB frame references | `d7018788a6c11276df9721e556e726542baa7c6195acb3d42f8404ed4d2b0310` | MIT/Apache-2.0 (repository-generated) |
| `superres_actual_upscaled_i422_33x17.avif` | Deterministic 33x17 full-range 8-bit I422 lossy still; AV1 signals denominator 9 and coded width 29, with restoration disabled to exercise the generic I422 super-resolution path across odd visible edges | `scripts/generate_avif_equal_width_superres.py --chroma 422-actual`; patched pinned fixture encoder; libavif 1.4.1 / libaom 3.13.2 | `9095580af62e5003fcb8f68d62da1b6b0cbb6205578a2828d05a87ee2f367c1f` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_superres_select_10bit_160x56.avif` | Deterministic two-frame full-range 10-bit I420 lossy sequence; frame 1 uses super-resolution and TX_MODE_SELECT without active restoration | `scripts/generate_avif_hi10_superres_select.py`; libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `04e31a3ac36c25ef77061a2ed09b79fd8fb885a5ff14d6ab3a82e2d32ad3b02e` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_1x1.avif` | Deterministic 1x1 8-bit monochrome Luma still with sample 127 | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, 4:0:0, CICP 1/13/6 | `6c4212de07ead445c0b468c39b77f099cc8555e99edd6460806407d7536be305` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_superres_clamped_3x3.avif` | 3x3 8-bit all-lossless monochrome AV1 key frame with super-resolution enabled at the minimum-width clamp; the AV1 frame header adds a four-bit super-resolution syntax field while preserving its coded pixels | `scripts/generate_avif_lossless_monochrome_superres_small.py`; libavif 1.4.1 / libaom 3.13.2, quality 100, speed 0, one thread; Pillow 12.2.0 exact RGB reference | `b2e3a50cf5e71bbde9324c3b8f3b1fb8a050e088c25aa1029fd7bad93dd0c2d6` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_multitile_superres_256x64.avif` | 256x64 8-bit all-lossless monochrome AV1 key frame with two tile columns; coded width 228 is restored to 256 by denominator-9 super-resolution, exercising multi-tile lossless super-resolution reconstruction | `scripts/generate_avif_lossless_monochrome_multitile_superres.py`; libavif 1.4.1 / libaom 3.13.2, quality 100, speed 0, one thread; Pillow 12.2.0 exact RGB reference | `47d5096e64a5d61a6247575f645490813a55ba2ddd4c77379987c1a37ea6dd91` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_multitile_superres_narrow_edge_145x64.avif` | 145x64 8-bit all-lossless monochrome AV1 key frame coded at width 129 with two tile columns, leaving a one-pixel right tile before denominator-9 super-resolution | `scripts/generate_avif_lossless_monochrome_multitile_superres.py --profile narrow-edge-tile`; libavif 1.4.1 / libaom 3.13.2, quality 100, speed 0, one thread; Pillow 12.2.0 exact RGB reference | `aa73cdf640627b55ea1cc7a35af26a42f92bce39c504911c0e0fee6124905320` | MIT/Apache-2.0 (repository-generated) |
| `lossless_monochrome_above_right_superres_163x64.avif` | 163x64 8-bit all-lossless monochrome AV1 key frame coded at width 145 with two tile columns, leaving a 17-pixel cropped right tile before denominator-9 super-resolution | `scripts/generate_avif_lossless_monochrome_multitile_superres.py --profile above-right-context`; libavif 1.4.1 / libaom 3.13.2, quality 100, speed 0, one thread; Pillow 12.2.0 exact RGB reference | `b60d77de7232868f5c4796875641dd93c0ec00f0fc29ae911af06ca8a36c9f0a` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_noise_64x64.avif` | Deterministic 64x64 8-bit lossless 4:0:0 still with high-entropy luma samples | `scripts/generate_avif_monochrome_noise_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 6, one thread, full range | `508ad1f423c0bfa66c3983fb4dd198017cca9d5c2fc426be4b87d037b0a0dd28` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_i420_odd_17x17.avif` | Deterministic 17x17 8-bit full-range lossless I420 still with odd luma and ceil-sized 9x9 chroma planes | `scripts/generate_avif_portable_color_odd_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 6, one thread | `8b8ed2a65adb2dce764a2786b3d5c4332df59a00b0ba7e1dc3efc8a1e9036d1c` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_i420_aligned_width_odd_height_16x17.avif` | Deterministic 16x17 8-bit full-range lossless I420 still with aligned width and an odd ceil-sized chroma height | `scripts/generate_avif_portable_color_odd_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 6, one thread | `fd52d7f651bd18556811b6f2c4200302454011f37e013f15bdfcbcddf1983904` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossy_monochrome_noise_64x64.avif` | Deterministic 64x64 8-bit lossy 4:0:0 still with high-entropy luma samples and CDEF enabled | `scripts/generate_avif_lossy_monochrome_noise_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 70, speed 6, one thread, full range | `e6bdd362737c7bdbbabd407bf162a80d3d54a4584ec323e2d96e18c4a4d26155` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_limited_16x16.avif` | Deterministic 16x16 8-bit lossless 4:0:0 luma still with limited-range samples | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread, CICP 1/13/6 | `e14c3cce9943b6f4f12ad3f0a93624c702678a73fa55101ba61e61aabe3168be` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_limited_10bit_16x16.avif` | Deterministic 16x16 10-bit lossless 4:0:0 luma still with limited-range samples | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread, CICP 1/13/6 | `514a2b65aa84f2bd647a8c041cf20c7b59c8fb7e71f8303bc1a4a408defccd66` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_limited_12bit_16x16.avif` | Deterministic 16x16 12-bit lossless 4:0:0 luma still with limited-range samples | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread, CICP 1/13/6 | `3abcfd332dff4d6ccfc801b75430c1997e1738b1546006ab5590a6e1fcfd549e` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_fullrange_12bit_17x9.avif` | Deterministic 17x9 full-range 12-bit lossless 4:0:0 luma still; all 153 varied 12-bit codes round-trip exactly, including low nibbles 0–15 | `scripts/generate_avif_monochrome_12bit_scalar_tail.py`; clean libavif 1.4.1 / `6543b22`, libaom 3.13.2 / `ad44980`; quality 100, speed 0, one thread; Pillow 12.2.0 exact RGB reference | `07793973838382f7de986377c7ed2ea139144b1a4413092e65bb6713db10ae57` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_fullrange_alpha_12bit_17x9.avif` | Deterministic 17x9 full-range 12-bit lossless monochrome still with a varied 12-bit auxiliary alpha plane; color and alpha both include endpoints and low nibbles 0–15 | `scripts/generate_avif_monochrome_12bit_scalar_tail.py`; clean libavif 1.4.1 / `6543b22`, libaom 3.13.2 / `ad44980`; quality 100, speed 0, one thread; Pillow 12.2.0 exact RGBA reference | `3acad68f56c9eced8f975303cce603d3404fd00d10bb2f92b24573395b34c25b` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_limited_bt2020_pq_16x16.avif` | Deterministic 16x16 8-bit lossless limited-range 4:0:0 still with BT.2020/PQ CICP 9/16/9 | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread | `ad3ce0e1171561690b156149fa8761fcc8adf26670507b0d64cc4294994d344b` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_limited_alpha_16x16.avif` | Deterministic 16x16 8-bit lossless limited-range 4:0:0 luma still with a full-range alpha item | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread, CICP 1/13/6 | `b95edc757441fad6b2c051e1df78c888fab81aeab5c81e366d8fd4b5941a58fa` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_limited_alpha_10bit_16x16.avif` | Deterministic 16x16 10-bit lossless limited-range 4:0:0 luma still with a full-range alpha item | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread, CICP 1/13/6 | `2497017931a998c26e7328b0e940b736ccbaaab5f3dfc4d3e6637ebb7e7168f2` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_limited_alpha_12bit_16x16.avif` | Deterministic 16x16 12-bit lossless limited-range 4:0:0 luma still with a full-range alpha item | `scripts/generate_av1_encoder_refs.py`; libavif 1.4.1 / `6543b22`; libaom 3.13.2 / `ad44980`; quality 100, speed 6, one thread, CICP 1/13/6 | `27aed8594e5d60179e6596dd950800fdf43f6809038ed30c0b86aaf4c20ac3c7` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_monochrome_grid_73x64.avif` | Deterministic 73x64 8-bit lossless monochrome 2x1 grid with two 64x64 color cells, auxiliary alpha items, and a clipped right cell | `scripts/generate_avif_monochrome_grid_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 6, one thread, 4:0:0; metadata layout adapted from `grid.avif` | `3e69167291d846470673b297e9a30e51d7027e182918b0a8c1d95f24719e9828` | BSD-2-Clause (`third_party/libavif/LICENSE`; derived from `grid.avif`) |
| `portable_lossless_monochrome_grid_32bit_dimensions_73x64.avif` | Same exact 73x64 monochrome-plus-alpha grid using the AV1 grid descriptor's 32-bit output-dimension fields (`flags=1`) | `scripts/generate_avif_monochrome_grid_fixture.py`; derived from the pinned 16-bit-dimension grid and its encoded cells; Pillow 12.2.0 exact RGBA output | `40991aa9bee645ee683cb8c562d9db17e3c834e0c6998db90890dc0a498cd2f3` | BSD-2-Clause (`third_party/libavif/LICENSE`; derived from `grid.avif`) |
| `portable_lossless_monochrome_grid_no_alpha_73x64.avif` | Deterministic 73x64 8-bit lossless monochrome 2x1 grid with no auxiliary-alpha items or properties and a clipped right cell | `scripts/generate_avif_monochrome_grid_edges.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 6, one thread, 4:0:0; metadata adapted from `grid.avif` | `a33a40230675d388722a13e45849205e24ff7f36486502907472c5f9ebd719d0` | BSD-2-Clause (`third_party/libavif/LICENSE`; derived from `grid.avif`) |
| `portable_lossless_monochrome_grid_bottom_clip_64x73.avif` | Deterministic 64x73 8-bit lossless monochrome 1x2 grid with two 64x64 color cells, auxiliary alpha items, and a bottom cell clipped to nine rows | `scripts/generate_avif_monochrome_grid_edges.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 6, one thread, 4:0:0; metadata adapted from `grid.avif` | `453945fc2524dabc77162d97896b21d8399a7171a909d19babb908603c0a4201` | BSD-2-Clause (`third_party/libavif/LICENSE`; derived from `grid.avif`) |
| `portable_i444_quality100_64x64.avif` | Deterministic 64x64 8-bit quality-100 RGB 4:4:4 key frame with fixed 64-pixel partitions | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 64-pixel partitions, CICP 1/13/6 | `5ceb66b47bda43bec5c421222eeb3ef858c62702bc41833986fdf678f6867737` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_filmgrain_i444_64x64.avif` | Seeded 64x64 8-bit all-lossless RGB 4:4:4 key frame with film grain | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 64-pixel partitions, DCT only, CICP 1/13/6, film-grain test enabled | `5e40ca71068fdd232d11f356103f53c1fdd7b94356fa91b0004d7307de4cbd25` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_filmgrain_i444_overlap_64x64.avif` | The pinned I444 film-grain source with only the AV1 overlap flag enabled, covering all 32x32 block-edge blends | `scripts/generate_avif_filmgrain_edge_fixtures.py`; source `portable_lossless_filmgrain_i444_64x64.avif`; set the inspected frame-header overlap bit; preserve sample extents and tile payload; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2 | `345a3e16a1d9574a6ac39c4b14eaa93ba70b5d49714cae2188fdff03c252e03a` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_filmgrain_i444_restricted_clip_64x64.avif` | The pinned I444 film-grain source with restricted-range clipping enabled | `scripts/generate_avif_filmgrain_edge_fixtures.py`; source `portable_lossless_filmgrain_i444_64x64.avif`; set only the inspected frame-header clipping bit; preserve sample extents and tile payload; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2 | `a60b2f7ce2b1a2891d6627a5f568fe5ba5dde0ea96ff348d3271e4893a633fcb` | MIT/Apache-2.0 (repository-generated) |
| `malformed_filmgrain_y_point_non_increasing.avif` | Full-file I444 AVIF whose second film-grain Y point repeats the first point's x value | `scripts/generate_avif_filmgrain_edge_fixtures.py`; source `portable_lossless_filmgrain_i444_64x64.avif`; replace the checked second Y-point x field; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2 | `1a7811072b8e1e3b9ea0694aaf5415d61d00f19a4dcdbbcb0048f7fddd94fe71` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_filmgrain_420_chroma_from_luma_64x64.avif` | Seeded 64x64 8-bit all-lossless I420 key frame using chroma scaling from luma for film grain | `scripts/generate_avif_filmgrain_chroma_from_luma_420.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, CICP 1/13/6, film-grain test enabled | `80c3a157f32f190a787588c48232113c4bbe915abd80812c5e0d1d62fd51c893` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_filmgrain_420_chroma_from_luma_65x65.avif` | Seeded 65x65 8-bit all-lossless I420 key frame with chroma scaling from luma; the final 33x33 chroma block reaches partial grain blocks, clamps the right interpolation neighbor, and accesses the last luma row | `scripts/generate_avif_filmgrain_chroma_from_luma_odd.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, CICP 1/13/6, film-grain test enabled | `5f881e4c6eed5bb4c554004d72ca5e992f6b704bc39672b01d8b3a3236bb2330` | MIT/Apache-2.0 (repository-generated) |
| `portable_lossless_filmgrain_420_zero_y_points_64x64.avif` | Seeded 64x64 8-bit all-lossless I420 key frame whose update-grain syntax has zero Y points and omits conditional U/V points and coefficients | `scripts/generate_avif_filmgrain_zero_y_420.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; deterministic film-grain header rewrite | `c76ede4ac45599b182d023f8425744f09dc27e4011742d6295d3ec50e172ed9f` | MIT/Apache-2.0 (repository-generated) |
| `animated_filmgrain_reference_reuse_i444_64x64.avif` | Deterministic two-frame 64x64 I444 sequence whose inter frame reuses key-frame film-grain parameters with a new seed | `scripts/generate_avif_filmgrain_reference_reuse.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `bd947085ed6437edfd50a97b43506cc56e96af8b5450a8ef5cb8289b8ec62b34` | MIT/Apache-2.0 (repository-generated) |
| `hdr.avif` | libavif `tests/data/colors_hdr_rec2020.avif` | libavif 1.4.1 / `6543b22` | `9980e58ddf718a923f1738c34aad1c72f8e5795ec07e68f1a5f9bd216ca19740` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `grid.avif` | libavif `tests/data/color_grid_alpha_nogrid.avif` | libavif 1.4.1 / `6543b22` | `bae56368b348b1d847e2bfb662522599f0c63dfe62fb68826c9e42a300ff405d` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated.avif` | libavif `tests/data/colors-animated-8bpc.avif` | libavif 1.4.1 / `6543b22` | `2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_animated_duplicate_stbl.avif` | `animated.avif` with a second `stbl` box appended to the same `trak/mdia/minf`; both sample and item offsets are rebased | `scripts/generate_avif_media_box_edges.py`; Pillow 12.2.0 rejects the complete file at open | `82117e58d5518cead7a53bc76cb705b3b02261558c6d5b34cf90328700b8bb4d` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_sequence_only_trailing_meta_after_mdat.avif` | `animated_sequence_only_no_meta.avif` with a handler-only `meta` box appended after the complete `mdat` | `scripts/generate_avif_trailing_meta_sequence_fixture.py`; Pillow 12.2.0 and pinned libavif preserve all five RGB frames, 33 ms timing, and one total play | `a58d67aaece0bd6502b9f3bb63467e22de4452d4d99d9fb21d088d371f0e0c1f` | BSD-2-Clause (derived from libavif AVIS) |
| `animated_sequence_only_interstitial_meta_before_moov.avif` | `animated_sequence_only_no_meta.avif` reordered as `ftyp/free/mdat/meta/moov`, with the `stco` offset rebased and the same handler-only `meta` box between media data and movie boxes | `scripts/generate_avif_trailing_meta_sequence_fixture.py`; Pillow 12.2.0 and pinned libavif preserve all five RGB frames, 33 ms timing, and one total play | `2c60d420f540730712e700640cd01aa8d71657029ed4873321137eecf159fc59` | BSD-2-Clause (derived from libavif AVIS) |
| `error_animated_duplicate_sample_description_colr.avif` | `animated_sequence_only_no_meta.avif` with a conflicting second `nclx` color property in its AV1 track sample entry; sample bytes are unchanged and `stco` offsets are rebased | `scripts/generate_avif_duplicate_sample_description_colr.py`; Pillow 12.2.0 recognizes the AVIF signature but rejects the duplicate sample-entry property at open | `99f4051b60a85669670f9664d20a061a57a7e8ad00115219ab34bfb2ef65c396` | BSD-2-Clause (derived from libavif AVIS) |
| `animated_timing_without_decoder_model.avif` | `animated.avif` with AV1 timing information enabled and `decoder_model_info_present_flag` clear; Pillow preserves all five exact RGB frames | `scripts/generate_avif_sequence_header_parity.py`; timing syntax only is inserted into the first sequence header | `7978ad21887be7ec5e74b13ab9555e2c2caef9911b900947b67c231b74f1f5cf` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_equal_picture_interval_two_ticks.avif` | `animated.avif` with `num_ticks_per_picture_minus_1=1` encoded as UVLC `010`; Pillow preserves all five exact RGB frames and BMFF timing | `scripts/generate_avif_sequence_header_parity.py --case equal-picture-interval-two-ticks`; extends only the AV1 sequence timing syntax | `e49851e6d94c721525f3b4130d7279c2ed2329a68032ea81f3714ae68119db7d` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_order_hint_disabled.avif` | Five-frame 64×64 RGB animation encoded with ordinary AV1 sequence headers and `enable_order_hint=0`; creation and modification timestamps are zeroed for reproducibility | `scripts/generate_avif_order_hint_disabled_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / AOM 3.13.2; Pillow decodes all five frames with 33 ms durations | `565363788fd1a30cc51b2f3f3d4852a447292646de0b37ab81589590a93e706d` | MIT/Apache-2.0 (repository-generated) |
| `animated_screen_content_tools_disabled.avif` | Five-frame 64×64 RGB animation with AV1 screen-content tools fixed off; all five frame headers omit the screen-tools selector | `scripts/generate_avif_sequence_screen_tools.py --case screen-content-disabled`; mutation preserves tile payloads, Pillow 12.2.0 decodes all five exact frames at 33 ms | `d0c88a39ca31c30f56afa5540505d7e20892ed0d9d5339ff843cdf6d9040a357` | MIT/Apache-2.0 (repository-generated mutation) |
| `animated_integer_mv_disabled.avif` | Five-frame 64×64 RGB animation with screen-content tools selectable and AV1 integer motion vectors fixed off | `scripts/generate_avif_sequence_screen_tools.py --case integer-mv-disabled`; sequence-header mutation only, Pillow 12.2.0 decodes all five exact frames at 33 ms | `47b3767081913f190e0473842a466a9a81ae4acce5e5099c323c9793187cdcce` | MIT/Apache-2.0 (repository-generated mutation) |
| `animated_transfer_bt709.avif` | Five-frame 64×64 RGB animation with AV1 CICP values 1/1/6 (BT.709 primaries and transfer; BT.601 matrix); creation and modification timestamps are zeroed | `scripts/generate_avif_transfer_characteristics_bt709.py`; Pillow 12.2.0 / libavif 1.4.1 / AOM 3.13.2; Pillow decodes all five frames with 33 ms durations | `8b7c981b052241b55112e6ae3ce64b00681c101a25c7d9bd9c9091d727417af2` | MIT/Apache-2.0 (repository-generated) |
| `animated_identity_matrix_profile1_444.avif` | Five-frame 64×64 RGB AV1 profile-1 8-bit 4:4:4 animation with valid identity CICP values 1/13/0; creation and modification timestamps are zeroed | `scripts/generate_avif_identity_matrix_profile1_444.py`; Pillow 12.2.0 / libavif 1.4.1 / AOM 3.13.2; Pillow decodes all five frames with 33 ms durations | `4535db4548ddf82d23c72ef970b01f1632f4db4c9f7ddfd392bf0d5d011bdd4a` | MIT/Apache-2.0 (repository-generated) |
| `animated_identity_matrix_profile2_12bit_444.avif` | Five-frame 64×64 RGB AV1 profile-2 12-bit 4:4:4 animation with valid identity CICP values 1/13/0; creation and modification timestamps are zero | `scripts/generate_avif_identity_matrix_profile2_12bit_444.py`; pinned libavif 1.4.1 / AOM 3.13.2 `avifenc` with qcolor 100, speed 8, one worker, and 33 ms frame durations | `d0079f47a1f7709a373191d2248d42daec16d01132b6aaf1cdb1d7dc613fb1da` | MIT/Apache-2.0 (repository-generated) |
| `animated_initial_display_delay_10.avif` | `animated.avif` with AV1 timing information and the maximum valid initial display delay of 10; Pillow preserves all five exact RGB frames | `scripts/generate_avif_sequence_header_parity.py --case initial-display-delay-10`; inserts the display-model flag, per-operating-point flag, and delay | `aa95d8f4b6f2576f91a010fd0cc1cd5ed10ea826b89f8aba661c3dd6a9fa1b7d` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_operating_point_high_tier.avif` | `animated.avif` with AV1 operating-point level 8 and the main-tier flag present; Pillow preserves all five exact RGB frames | `scripts/generate_avif_sequence_header_parity.py --case operating-point-high-tier`; sets the level and inserts the tier bit | `bb06afad6c56b58389eede0bdda980ca42ee36ca045eb92e39de7f5a85e19fb0` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_decoder_model_without_operating_point_parameters.avif` | `animated.avif` with decoder-model timing enabled and `decoder_model_present_for_this_op` clear; Pillow preserves all five exact RGB frames | `scripts/generate_avif_sequence_header_parity.py --case decoder-model-without-operating-point-parameters`; adds valid decoder-model timing syntax and the required frame-header timing flags | `e220130a0614df8c5fce0acf525c2c69a9fccaa8ec140373eac1f91d424d757d` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_duplicate_top_level_meta_after_mdat.avif` | `animated.avif` with its complete top-level `meta` box copied after `mdat` | `scripts/generate_avif_trailing_top_level_duplicates.py`; Pillow 12.2.0 preserves the original five-frame metadata and pixels | `86ae1b02cbcec44de7ca7bf5c08815a1ac9ca4325f5d74fb89835fb37516156d` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `still_duplicate_top_level_meta_after_mdat.avif` | `grid.avif` with its complete top-level `meta` box copied after `mdat` | `scripts/generate_avif_still_duplicate_meta_after_mdat.py`; Pillow 12.2.0 preserves the original RGBA pixels and metadata | `2279761c577c985f0c3ccfbe19a02a89f05c5fa16fb3dc1516bd5ebeb3c6702d` | BSD-2-Clause (derived from libavif `tests/data/color_grid_alpha_nogrid.avif`) |
| `animated_duplicate_top_level_moov_after_mdat.avif` | `animated.avif` with its complete top-level `moov` box copied after `mdat` | `scripts/generate_avif_trailing_top_level_duplicates.py`; Pillow 12.2.0 preserves the original five-frame metadata and pixels | `f88aa18f6c19d1f29431dfec69eff4ac0f4cc5b91e56c4c84206d0ab629e59b5` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_duplicate_top_level_meta_before_mdat.avif` | `animated.avif` with its complete top-level `meta` box copied before `moov` and `mdat` | `scripts/generate_avif_trailing_top_level_duplicates.py`; Pillow 12.2.0 rejects the file at image open | `a409fdd42863ea2fcea4462589d034120b6529854320f3833fef47f0d836e9a2` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_duplicate_top_level_moov_before_mdat.avif` | `animated.avif` with its complete top-level `moov` box copied before `mdat` | `scripts/generate_avif_trailing_top_level_duplicates.py`; Pillow 12.2.0 opens and verifies the file, then rejects frame decoding | `9ca96808a42c259be912295f8286109e4cc6d7c2844abec252b1c36203ff0528` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_duplicate_track_tkhd_before_mdia.avif` | `animated.avif` with its `trak/tkhd` box duplicated immediately before `edts` and `mdia`; primary-item and chunk offsets plus containing box sizes are rebased | `scripts/generate_avif_track_box_parity_fixtures.py --case duplicate_tkhd`; Pillow 12.2.0 rejects the duplicate track header at image open | `06e8189946b910376e447dfb29db317f3c80b383a291a10799e45be78e6b2ab8` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_duplicate_track_mdia_after_mdia.avif` | `animated.avif` with its `trak/mdia` box duplicated after the original media box; primary-item and chunk offsets plus containing box sizes are rebased | `scripts/generate_avif_track_box_parity_fixtures.py --case duplicate_mdia`; Pillow 12.2.0 rejects the duplicate media box at image open | `83ba36a8ae5559dad80fcd4e45b7c54d55406cc1eb2368081f4fb4301004e54a` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_duplicate_track_edts_after_edts.avif` | `animated.avif` with its complete `trak/edts` edit-list box duplicated immediately after the original; primary-item and chunk offsets plus containing box sizes are rebased | `scripts/generate_avif_track_box_parity_fixtures.py --case duplicate_edts`; Pillow 12.2.0 rejects the duplicate edit-list box at image open | `1a184c2afd0529985cbc9df3737b40e2df8578de04e7dd9a8b38144adfbe2cc7` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_duplicate_track_tref_auxl_before_edts.avif` | `animated.avif` with two `trak/tref/auxl` relationships to the track ID before `edts`; primary-item and chunk offsets plus containing box sizes are rebased | `scripts/generate_avif_track_box_parity_fixtures.py --case duplicate_tref_auxl`; Pillow 12.2.0 rejects the duplicated auxiliary relationship at image open | `32f47d98b7e9406480d8a9f9351689ae43f1822cf7712931a0cdb433cef360a0` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_duplicate_track_tref_auxl_children_before_edts.avif` | `animated.avif` with one `trak/tref` containing two `auxl` children naming the same track ID before `edts`; file offsets and containing box sizes are rebased | `scripts/generate_avif_track_box_parity_fixtures.py --case duplicate_tref_auxl_children`; Pillow 12.2.0 rejects the duplicate children at image open | `bb9dc21acddc7b76ae65f81bbf07f12dfeeb75a1fd2d3696fb57c55238680be3` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_missing_track_tkhd.avif` | `animated.avif` with its `trak/tkhd` box replaced by a same-size `free` box | `scripts/generate_avif_track_box_parity_fixtures.py --case missing_tkhd`; Pillow 12.2.0 rejects the track without a header at image open | `67dc5859be58f020106cdaf9ede0648161583f723a43f82d0501f67c6f8526e2` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_track_tkhd_zero_id.avif` | `animated.avif` with its version-one `trak/tkhd` track identifier set to zero | `scripts/generate_avif_track_box_parity_fixtures.py --case zero_track_id_error`; Pillow 12.2.0 rejects the complete file at image open | `5b9718040db403bad0122fd59426e3e4d61652b88cea1a5654435075475180a3` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_missing_track_mdia.avif` | `animated.avif` with its `trak/mdia` box replaced by a same-size `free` box | `scripts/generate_avif_track_box_parity_fixtures.py --case missing_mdia`; Pillow 12.2.0 rejects the track without media at image open | `714dd98f5357e554fec337937e8f938a653490ceebb2be1f76af6be0644f68df` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_track_duration_repeating_5_of_5.avif` | `animated.avif` with the repeating edit-list flag set, track duration 5 and segment duration 5 | `scripts/generate_avif_track_box_parity_fixtures.py --case repeating_duration_finite`; Pillow 12.2.0 preserves the five exact frames; loop count is independently observed by pinned libavif | `076c6e24ac7dc4c91f82b94ed214e57da2e2d0392269782e3097427e128bcca1` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_track_duration_repeating_6_of_5.avif` | `animated.avif` with the repeating edit-list flag set, track duration 6 and segment duration 5 | `scripts/generate_avif_track_box_parity_fixtures.py --case repeating_duration_nondivisible`; Pillow 12.2.0 preserves the five exact frames; loop count is independently observed by pinned libavif | `fc517b7bd5850eff7f50332298ba8ac6772958c07965d2d6a5bb311c14e7f1b2` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_track_duration_repeating_large.avif` | `animated.avif` with the repeating edit-list flag set, track duration 2³¹+1 and segment duration 1 | `scripts/generate_avif_track_box_parity_fixtures.py --case repeating_duration_large`; Pillow 12.2.0 preserves the five exact frames; loop count is independently observed by pinned libavif | `65f39e3119b871fa15333c98f52c00c69d1064bcb0b9da89ae46d3871ff8ece3` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_track_duration_repeating_zero.avif` | `animated.avif` with the repeating edit-list flag set and track duration zero | `scripts/generate_avif_track_box_parity_fixtures.py --case repeating_duration_zero`; Pillow 12.2.0 rejects the file at image open | `4dbabf9ca1462df172a4509912c0799f92874a51fdd935ea29a39d76e1ea19b6` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_track_edit_list_entry_count.avif` | `animated.avif` with the repeating edit-list flag set and `elst` entry count increased from one to two without adding a second record | `scripts/generate_avif_track_box_parity_fixtures.py --case edit_list_entry_count_error`; Pillow 12.2.0 rejects the file at image open | `6c85c53210f2c251b432f82a14920b3592ff293a75af3cb8561cff05d6ab3c98` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_track_edit_list_segment_duration_zero.avif` | `animated.avif` with the repeating edit-list flag set and its version-one segment duration changed to zero | `scripts/generate_avif_track_box_parity_fixtures.py --case edit_list_segment_duration_zero_error`; Pillow 12.2.0 rejects the file at image open | `ff0d9a0a1588986bd66142fd1d02a36c5ff9732b2cba184a700e3580f2a17720` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_track_edit_list_duplicate_elst.avif` | `animated.avif` with its complete `elst` child duplicated inside `edts`; Pillow rejects the file at image open | `scripts/generate_avif_track_box_parity_fixtures.py --case edit_box_duplicate_elst_error` | `4f39c64e57f8d24d058b7f08d5fb6e76d1c455e790a24622187e5f58e5525b81` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_track_edit_box_missing_elst.avif` | `animated.avif` with the `elst` child type changed to `free`, leaving the edit box without an edit list; Pillow rejects the file at image open | `scripts/generate_avif_track_box_parity_fixtures.py --case edit_box_missing_elst_error` | `4b8d2110df3a049b39333d94d9533fbf4a1a987fd4a0cddc4e1be5e4a04915eb` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_duplicate_track_tref_hint_before_edts.avif` | `animated.avif` with two `trak/tref` boxes containing ignored `hint` references before `edts`; primary-item and chunk offsets plus containing box sizes are rebased | `scripts/generate_avif_track_box_parity_fixtures.py --case duplicate_tref_hint`; Pillow 12.2.0 preserves the original five-frame output | `f20b077c2b38c9e8e944f4a39549fe215a9d864374c34fa8b44180b86c60254e` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_zero_track_tref_auxl_before_edts.avif` | `animated.avif` with one `trak/tref/auxl` relationship targeting track ID zero before `edts`; primary-item and chunk offsets plus containing box sizes are rebased | `scripts/generate_avif_track_box_parity_fixtures.py --case zero_tref_auxl`; Pillow 12.2.0 preserves the original five-frame output | `00bd7d70c5c9285d612bfec8fcca43de555ee0d5f7deedd0cb0f52bc03971563` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_duplicate_track_av1c_conflicting_depth.avif` | `animated.avif` with a duplicate track sample-entry `av1C` whose depth changes from 8-bit to 10-bit | `scripts/generate_avif_track_box_parity_fixtures.py --case duplicate_av1c_conflicting_depth`; Pillow 12.2.0 and pinned libavif preserve the original five frames | `5325225cbcd2171dbc009a935e6318a995be90ea1641bcf6f4eecc277aa445c8` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_duplicate_track_auxi_conflicting_alpha.avif` | `animated.avif` with duplicate track sample-entry `auxi` properties: an ignored auxiliary type followed by alpha | `scripts/generate_avif_track_box_parity_fixtures.py --case duplicate_auxi_conflicting_alpha`; Pillow 12.2.0 and pinned libavif preserve the original five frames | `d549d98a957011df232993ab5605012110e173bb200a1af1f7a0773c74464cc5` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_stss_trailing_payload.avif` | `animated.avif` with a zero-entry `stss` table and its original sync-sample index retained as bounded trailing payload | `scripts/generate_avif_track_box_parity_fixtures.py --case stss_trailing_payload`; Pillow 12.2.0 preserves exact frame pixels and timing | `607f1a675785f65fe6579920e803969030a3b67e13152f7303b12b97c3c60768` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_animated_stss_truncated_record.avif` | `animated.avif` with the `stss` count increased from one to two while only one sample index remains | `scripts/generate_avif_track_box_parity_fixtures.py --case stss_truncated_record`; Pillow 12.2.0 rejects the complete file at open | `52cf735b88c44a975ebbf0d6f17ebe103a90eb59639f4de1f9bb089289d5bf20` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_animated_stss_nonzero_version.avif` | `animated.avif` with the `stss` FullBox version changed from zero to one | `scripts/generate_avif_track_box_parity_fixtures.py --case stss_nonzero_version`; Pillow 12.2.0 rejects the complete file at open | `2d6aad8fb2b8e247f3489d54bb30cf0f49e2cdb1f2e8632ae70144dc5a93fdac` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_co64_chunk_offsets.avif` | `animated.avif` with its one-entry `stco` widened to `co64`, and chunk / primary-item offsets rebased | `scripts/generate_avif_co64_chunk_offsets.py`; source SHA-256 `2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d`; +4-byte box growth preserves encoded samples | `589f8836e3e1fbe9f8524353b5bc56cc615927dc15848c1e4fb671c467c04021` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_stsc_two_chunk_mapping.avif` | `animated.avif` remapped into two track chunks: samples 1–2 and 3–5, with matching two-row `stsc` and two-entry `stco` tables | `scripts/generate_avif_multi_entry_stsc_fixture.py`; source SHA-256 `2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d`; +16-byte sample-table growth rebases chunk and primary-item offsets while preserving encoded samples | `117a8b2d61e01d9e9a5d911313524509c4b99d527b108e552a0b6e27d2a8500c` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_stsc_first_chunk_not_one.avif` | Two-chunk AVIF with the first `stsc` mapping changed from chunk one to chunk two; Pillow rejects it at open | `scripts/generate_avif_stsc_order_error_fixtures.py`; one four-byte field changes and encoded samples remain unchanged | `9b54a6a24989c3ca9feaffb1779ced9407a9cad3f864415d0742e9def89eb46a` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_stsc_non_increasing_chunk.avif` | Two-chunk AVIF with the second `stsc` mapping changed from chunk three to chunk one; Pillow rejects the non-increasing mapping at open | `scripts/generate_avif_stsc_order_error_fixtures.py`; one four-byte field changes and encoded samples remain unchanged | `a6032faac42c6646befcb10470541495702600e98d70fc56dcde3c7fde787f73` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_stsc_nonzero_version.avif` | Two-chunk AVIF with the `stsc` FullBox version set to one; Pillow rejects the unsupported version at open | `scripts/generate_avif_sample_table_edge_fixtures.py`; one version byte changes and encoded samples remain unchanged | `d8778ff338b70039d77390547f95423a70e44e31eb53f70d6d56202911c56296` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_stsc_empty_table.avif` | Two-chunk AVIF with no `stsc` entries and an ignored padding box preserving parent and file sizes; Pillow rejects the empty table | `scripts/generate_avif_sample_table_edge_fixtures.py`; encoded samples remain unchanged | `000ebcbdf17df55f3c1c2da392869f982250cd7d77ec149b3f9219dc25803180` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_stsc_zero_samples_per_chunk.avif` | Two-chunk AVIF whose first `stsc` mapping assigns zero samples to the chunk; Pillow rejects the invalid mapping | `scripts/generate_avif_sample_table_edge_fixtures.py`; one four-byte field changes and encoded samples remain unchanged | `89df914cacdf68e8ec3c1053fc8d0503d79a361d405312957db2f3265d1e2213` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_stco_empty_offsets.avif` | Two-chunk AVIF with an empty `stco` table and an ignored padding box preserving parent and file sizes; Pillow rejects the missing offsets | `scripts/generate_avif_sample_table_edge_fixtures.py`; encoded samples remain unchanged | `6ddb173daf11f397e82abb5fd46760ab8f24a46a66ecaf66877fdf93340fde2e` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_stco_nonzero_version.avif` | Two-chunk AVIF with its `stco` FullBox version set to one; Pillow rejects the unsupported version at open | `scripts/generate_avif_sample_table_edge_fixtures.py`; one version byte changes and encoded samples remain unchanged | `e0a2ce3fbe6306871a2b1db1165010adea9d658cf049e987d3f231db093351d1` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_stco_trailing_payload.avif` | Two-chunk AVIF with one trailing `stco` byte; media offsets are rebased and Pillow preserves all five exact frames | `scripts/generate_avif_sample_table_edge_fixtures.py`; one trailing byte and corresponding offsets are added | `e773e124d5c3954d8fd0ca5b7fde7f6ad4d50b198dddc20b86b45a1cc70e2d15` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_stsc_zero_description_index_first.avif` | Two-chunk AVIF with its first `stsc` description index set to zero; Pillow decodes all five frames using the first sample description | `scripts/generate_avif_sample_table_edge_fixtures.py`; one four-byte field changes and encoded samples remain unchanged | `c6f3ff4272a7ade6f51cd77a3c643fb7301e416e4e5ebbd75e4fca4397c5db64` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_stsc_trailing_payload.avif` | Two-chunk AVIF with one zero byte after its otherwise unchanged two-entry `stsc` table; `iloc` and `stco` offsets are rebased by one while encoded samples and five-frame output are preserved | `scripts/generate_avif_stsc_trailing_payload.py`; Pillow 12.2.0 and pinned libavif 1.4.1 decode five exact frames | `afd53fee7738e9b4fe280bb0ff1a19f938cdaad326977abf2cb4266c8e376e4b` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_stsz_trailing_payload.avif` | Two-chunk AVIF with one trailing byte after the declared `stsz` sample-size entries; `iloc` and `stco` offsets are rebased by one while encoded samples are preserved | `scripts/generate_avif_stsz_edge_fixtures.py`; Pillow 12.2.0 decodes five exact frames | `ec5869776d517f36dcbd9d19eb05e8a87dd709b3651a2b2ca00a826cf546f8c3` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_animated_stsz_empty_table.avif` | Two-chunk AVIF whose `stsz` declares zero samples and contains no entries; Pillow rejects it at open | `scripts/generate_avif_stsz_edge_fixtures.py`; the table shrinks by 20 bytes, offsets are rebased, and encoded samples remain unchanged | `00a12f15bb8a108afd79d4a8c3a38f03e2a00180b513c32ecc3c18a2544deffd` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_animated_stsz_zero_entry.avif` | Two-chunk AVIF whose first `stsz` entry declares a zero-length sample; Pillow rejects it at open | `scripts/generate_avif_stsz_edge_fixtures.py`; one four-byte entry changes and encoded samples remain unchanged | `6bf0b1208f1043014565adbfa368a46ac60462faaea837da36fe240604fd6b72` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_animated_stsz_nonzero_version.avif` | Two-chunk AVIF whose `stsz` FullBox version is one; Pillow rejects the complete file at open | `scripts/generate_avif_stsz_edge_fixtures.py`; only the version byte changes and encoded samples remain unchanged | `b4f1467cfb5dd7aa833de3e1fa54009540debaf53d12c01c901ed57bbcb14a22` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_animated_sequence_only_stsz_common_size_short_samples.avif` | Five-frame AVIS-only track whose `stsz` assigns a one-byte common size to all samples; Pillow inspects five frames but fails to decode frame zero | `scripts/generate_avif_stsz_edge_fixtures.py`; the explicit five-entry table shrinks by 20 bytes, the chunk offset is rebased, and the original media bytes remain unchanged | `fe2666ad9cf9fe32d6b4a35d64f1b9f1a101ae9a65e8055fab6cd62bc63e827c` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_animated_show_existing_unshowable_reference.avif` | `animated.avif` with its show-existing frame's reference slot changed from 2 to the non-showable initial key-frame slot 0; Pillow loads display frames 0–2 and fails while loading frame 3 | `scripts/generate_avif_show_existing_unshowable_reference.py`; one AV1 syntax byte changes | `bf8daa2a92e16a7c9b12cd5ef6d724b36c30e724d5f8a2952480387ff20ecf9d` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_animated_show_existing_empty_reference.avif` | `animated.avif` with its first color sample replaced by the original sequence header and a show-existing frame header for empty reference slot 7; Pillow opens the sequence then fails while loading frame 0 | `scripts/generate_avif_show_existing_empty_reference.py`; rewrites bytes only within the first sample extent | `4edf1ad3aca5683da9b01569d76381185f6dc057e089337466085f044e786f7c` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_animated_initial_display_delay_over_10.avif` | `animated_decoder_model_parameters_change.avif` with initial display delay set to 11 in its first AV1 sequence header; Pillow opens the file and fails while loading frame 0 | `scripts/generate_avif_sequence_header_errors.py --case initial-display-delay-over-10`; the display-model flags and delay field change | `5b1c2097577d75e116016f0760f6c589c12e1304fb42e7ef9a2c982f020b555a` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_animated_sequence_header_short_payload.avif` | `animated_decoder_model_parameters_change.avif` with the first sequence OBU shortened by two bytes; the reclaimed bytes form an empty `OBU_PADDING`, preserving the sample size while Pillow fails loading frame 0 | `scripts/generate_avif_sequence_header_errors.py --case sequence-header-short-payload`; the sequence payload length and two tail bytes change | `801608c817468a05bbf62f04ebee2cf5fbe324296c48e98560e7f74350357197` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_animated_frame_before_sequence_header.avif` | `animated_decoder_model_parameters_change.avif` with its first sequence-header OBU changed to OBU_PADDING, so the frame OBU arrives before sequence state exists; Pillow fails loading frame 0 | `scripts/generate_avif_sequence_header_errors.py --case sequence-header-omitted`; changes only the first OBU type bits and preserves the item extent | `4990da835025fe108dc99cea39b521e0ad9fb77e3461f71daa0fc70c2e4bb56d` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_animated_redundant_frame_header_with_pending.avif` | `animated_lossless_inter_420_derived_skip_refs_b32x32_64x64.avif` with its first OBU_FRAME replaced by a matching OBU_FRAME_HEADER and OBU_REDUNDANT_FRAME_HEADER followed by OBU_PADDING; the sample and file sizes are preserved and the pending frame has no tile data, so Pillow fails loading frame 0 | `scripts/generate_avif_frame_header_edge_fixtures.py`; changes only bytes within the first picture sample | `46c2a6f033820f828209f9a81ef8da4318c5f3255cee99d866321ce4635a86d5` | MIT/Apache-2.0 (repository-generated mutation of the pinned Pillow fixture) |
| `error_animated_redundant_frame_header_without_pending.avif` | `animated_lossless_inter_420_derived_skip_refs_b32x32_64x64.avif` with its first OBU_FRAME replaced by OBU_REDUNDANT_FRAME_HEADER and OBU_PADDING, so there is no original frame header; the sample and file sizes are preserved and Pillow fails loading frame 0 | `scripts/generate_avif_frame_header_edge_fixtures.py`; changes only bytes within the first picture sample | `9c29fa97ff57041eb15a7b49a067b2fb2f6f141f003a835d7a786089d1c2dee8` | MIT/Apache-2.0 (repository-generated mutation of the pinned Pillow fixture) |
| `error_animated_frame_header_before_tiles.avif` | `animated_lossless_inter_420_derived_skip_refs_b32x32_64x64.avif` with its first OBU_FRAME replaced by two identical OBU_FRAME_HEADER OBUs and OBU_PADDING; the sample and file sizes are preserved, but the second header arrives before any tile data, so Pillow fails loading frame 0 | `scripts/generate_avif_frame_header_edge_fixtures.py`; changes only bytes within the first picture sample | `380c025386ee770a6573d88fa1bdf12adbe345149026c46a3aee78c922f06c78` | MIT/Apache-2.0 (repository-generated mutation of the pinned Pillow fixture) |
| `error_animated_redundant_frame_header_mismatch.avif` | `animated_lossless_inter_420_derived_skip_refs_b32x32_64x64.avif` with its first OBU_FRAME replaced by an OBU_FRAME_HEADER and a redundant header whose `order_hint` differs; sample and file sizes are preserved and Pillow fails loading frame 0 | `scripts/generate_avif_frame_header_edge_fixtures.py`; changes only bytes within the first picture sample | `4e004fa44966ae7fc63f2ce5214283e4138e8cc6658e12fcf7b38fbd0b9897b0` | MIT/Apache-2.0 (repository-generated mutation of the pinned Pillow fixture) |
| `error_animated_tile_group_temporal_id_mismatch.avif` | `animated_opidc_0x101.avif` with its first operating-point IDC expanded to `0x103`, then its first OBU_FRAME split into a temporal-0 OBU_FRAME_HEADER and temporal-1 OBU_TILE_GROUP followed by OBU_PADDING; sample and file sizes are preserved and Pillow fails loading frame 0 | `scripts/generate_avif_frame_header_edge_fixtures.py`; changes the first sequence-header IDC and first picture sample only | `0c23f3e5db7b1d9d7150144f5204044c2bee0804edd6eb022148fc7a4ef4a1d9` | BSD-2-Clause (repository-generated mutation of the pinned Pillow fixture) |
| `error_animated_tile_group_spatial_id_mismatch.avif` | `animated_opidc_0x101.avif` with its first operating-point IDC expanded to `0x301`, then its first OBU_FRAME split into a spatial-0 OBU_FRAME_HEADER and spatial-1 OBU_TILE_GROUP followed by OBU_PADDING; sample and file sizes are preserved and Pillow fails loading frame 0 | `scripts/generate_avif_frame_header_edge_fixtures.py`; changes the first sequence-header IDC and first picture sample only | `2c23b60cc7f4e5951b24ba1309aa7ec1afda18fd97b74d66b54e982899a6963d` | BSD-2-Clause (repository-generated mutation of the pinned Pillow fixture) |
| `error_animated_equal_picture_interval_uvlc_32_leading_zeroes.avif` | `animated_equal_picture_interval_two_ticks.avif` with the equal-picture interval UVLC replaced by 32 leading zeroes; Pillow opens five frames and fails loading frame 0 | `scripts/generate_avif_sequence_header_errors.py --case uvlc-leading-zero-limit`; changes only bits in the sequence-header UVLC field | `842854d4e586bf3b84607f45c85479cb37c3357a78296b7bf39da5ad71ad74e8` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_nonuniform_tile_grid_tile_group_mismatch.avif` | `uniform_tile_rows_monochrome_256.avif` with `uniform_tile_spacing_flag` cleared; the frame header becomes a nonuniform 2×4 grid while the tile payload retains its original 1×4 layout, so Pillow fails loading frame 0 | `scripts/generate_avif_nonuniform_tile_grid_error.py`; changes one AV1 frame-header bit | `31f9bc09f7a52b56a9059a51d96c464bad715135557bfbf649c38501f55d6411` | MIT/Apache-2.0 (repository-generated mutation of the pinned Pillow fixture) |
| `error_nonuniform_tile_grid_ns_threshold_false.avif` | `uniform_tile_rows_monochrome_256.avif` with `uniform_tile_spacing_flag` cleared and the `ns(3)` prefix set to one; Pillow fails loading frame 0 after the nonuniform 2×3 grid takes the extended-value path | `scripts/generate_avif_nonuniform_tile_grid_error.py --case ns-threshold-false`; changes two AV1 frame-header bits | `b7a9dc5b14f8115b21f131700351a26cb679f6c23ded975a47489291e074f9fa` | MIT/Apache-2.0 (repository-generated mutation of the pinned Pillow fixture) |
| `error_animated_identity_matrix_subsampled.avif` | `animated_lossy_inter_420_b64x16_12bit_mode_select_64x64.avif` with matching AV1 and item/track `nclx` CICP set to 9/13/0 while 12-bit 4:2:0 remains; Pillow opens the file and fails loading frame 0 with `Conversion from YUV failed: Reformat failed` | `scripts/generate_avif_identity_matrix_subsampled.py`; the first sequence header's primaries and matrix fields plus both `nclx` properties change | `c20cb97af0fe69ddd1cfc9e38f223b1df5084655af6dacfd2b3e0c502e8d6162` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_animated_identity_matrix_profile0_8bit_420.avif` | `animated_transfer_bt709.avif` with matching AV1 sequence-header and item/track `nclx` CICP changed to 1/13/0 while profile 0 and 8-bit 4:2:0 remain; Pillow opens then fails loading frame 0 | `scripts/generate_avif_profile0_identity_matrix_error.py`; pinned Pillow reports `Failed to decode frame 0: Decoding of color planes failed` | `a88b8b552040ea93afb486c17efca9c7cdb0e7151854e1a0e350375489cd5526` | MIT/Apache-2.0 (repository-generated mutation) |
| `animated_sequence_header_cicp_mismatch_nclx.avif` | `animated_lossy_inter_420_b64x16_12bit_mode_select_64x64.avif` with only AV1 sequence-header primaries and matrix changed to 9/13/0; valid item and track `nclx` remain 1/13/6 and Pillow decodes both frames with the source pixels | `scripts/generate_avif_sequence_cicp_mismatch.py`; no sample payload or container color property changes | `74a0a8f8725cc7b44232c4e970859be1ace37ad1898f80da38dd5cc98b7ed628` | MIT/Apache-2.0 (repository-generated mutation) |
| `error_animated_invalid_operating_point_idc.avif` | `animated_decoder_model_parameters_change.avif` with first sequence-header `operating_point_idc` changed from zero to temporal-only value 1; Pillow opens the file and fails while loading frame 0 | `scripts/generate_avif_sequence_header_errors.py --case invalid-operating-point-idc`; one sequence-header bit changes | `d5553ed290d398d26ef3e731c98e4bee6b2d8d06b8a00dd4c876d851c61196c0` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_animated_reduced_header_nonstill.avif` | `animated_decoder_model_parameters_change.avif` with the reduced-still flag set only in the first AV1 sequence header although `still_picture` is clear; Pillow opens the file and fails while loading frame 0 | `scripts/generate_avif_sequence_header_errors.py --case reduced-header-nonstill`; one sequence-header bit changes | `61d55ddcaafde6347e60cc7b9ffe4e2c18ac448f85e14c8d3b6a9a2f0056ff6f` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `error_animated_sequence_color_primaries_change.avif` | `animated_decoder_model_parameters_change.avif` with only `color_primaries` changed from 1 to 9 in its repeated AV1 sequence header; Pillow decodes frame 0, then fails while loading frame 1 | `scripts/generate_avif_sequence_color_primaries_mismatch.py`; one bit in the repeated header changes | `3a4039604a7bf10739a7bcdf5a32c406c3ed5b772b9fdad58a88e73bc6571588` | BSD-2-Clause (derived from libavif `animated.avif`) |
| `animated_frame_id_outside_continuity.avif` | `animated_error_resilient.avif` with the second frame ID changed from 4,627 to 21,010, exactly 2^14 after its 15-bit predecessor; all other sample and container bytes are preserved | `scripts/generate_avif_frame_id_continuity.py`; Pillow 12.2.0 decodes frame 0 and rejects frame 1 | `d51d8d0ca98aa4cd24e7b464d71235bc89e767c5948a70952efc9df45cd2255d` | MIT/Apache-2.0 (repository-generated) |
| `animated_frame_id_wrapped_outside_continuity.avif` | `animated_error_resilient.avif` with the second frame ID changed from 4,627 to 4,625, wrapping below its 4,626 predecessor to a modulo difference of 32,767; all other sample and container bytes are preserved | `scripts/generate_avif_frame_id_continuity.py`; Pillow 12.2.0 decodes frame 0 and rejects frame 1 | `54329521e9374cd69c65026a99990ca9d8aae31a9a74f519f83fdb9228f5a07a` | MIT/Apache-2.0 (repository-generated) |
| `animated_frame_id_high_contiguous.avif` | `animated_error_resilient.avif` with both frame IDs advanced to 20,000 and 20,001, preserving their one-step reference delta while entering the high half of the 15-bit frame-ID range; Pillow preserves both exact frames | `scripts/generate_avif_frame_id_high_contiguous.py`; six bytes in the two AV1 frame-ID fields change | `25c5e52ca0759816ce94f4aa5d2be42ed99353a004da85e1cf80e8006925eecf` | MIT/Apache-2.0 (repository-generated) |
| `animated_frame_id_future_reference_invalidation.avif` | Three-frame sequence with IDs 30,000, 10,000, and 20,000; the valid wrapped inter frame refreshes one slot, leaving references at 30,000 for the final key frame to invalidate | `scripts/generate_avif_frame_id_future_reference_invalidation.py`; only the three frame IDs and seven reference deltas change | `ebe40d4c20e39723979ac8fc5968c846b54f30045739af26d8d91974d7492cbb` | MIT/Apache-2.0 (repository-generated) |
| `animated_frame_id_stale_reference_invalidation.avif` | Three-frame sequence with frame IDs 0, 8,000, and 17,000; the middle inter frame refreshes one slot, leaving ID 0 stale until the final independent key frame invalidates it; Pillow preserves all three exact frames | `scripts/generate_avif_frame_id_stale_reference_invalidation.py`; adds a cloned key-frame sample and updates bounded AVIF sample tables and offsets | `3b8ce1b8ddccf7b114f2ceac3505c254d73bc823d2138cae01106ce04029dd6d` | MIT/Apache-2.0 (repository-generated) |
| `animated_frame_id_wrap_contiguous.avif` | `animated_error_resilient.avif` with frame IDs 32,760 then 10 and all seven reference deltas changed to 18, preserving valid modulo continuity across the 15-bit wrap; Pillow preserves both exact frames | `scripts/generate_avif_frame_id_wrap_contiguous.py`; only AV1 frame-ID and reference-delta fields change | `8a53bf96c97b66d07b849ec6e4a6dcf18f37ab9db5b3edb045f2b357bd59e35e` | MIT/Apache-2.0 (repository-generated) |
| `animated_stsd_trailing_payload.avif` | Five-frame AVIS-only track with one trailing byte after its declared `stsd` entries; Pillow decodes all five frames unchanged | `scripts/generate_avif_stsd_edge_fixtures.py`; `stsd` and containing boxes grow by one byte, `stco` is rebased, and encoded `mdat` samples are preserved | `153ead664b729f1854c738ddd2e771e18c7488457f7716d8516df626eb2543ef` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_unknown_sample_entry_ignored_hint_track.avif` | Five-frame AVIS-only file with an ignored `hint` track whose bounded sample uses an unknown entry type; Pillow preserves every frame | `scripts/generate_avif_ignored_hint_track_fixture.py`; adds one sample pointing at existing `mdat` data, rebases the original chunk offset, and preserves media bytes | `dcc55d191d0f5e9cbabcd1dfbc9119b2f8e712ec1cea33880873739ed9987a49` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_animated_stsd_nonzero_version.avif` | Five-frame AVIS-only track whose `stsd` FullBox version is two; Pillow rejects it at open | `scripts/generate_avif_stsd_edge_fixtures.py`; one version byte changes and encoded samples remain unchanged | `aeace2695e5ce994805cc52b298cbed8f4354f025ff58431da591b83956a790b` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_animated_stsd_unknown_sample_entry.avif` | Five-frame AVIS-only track whose sole `stsd` entry type is `free`; Pillow rejects the unsupported sample description at open | `scripts/generate_avif_stsd_edge_fixtures.py`; four entry-type bytes change and encoded samples remain unchanged | `48318a79ccba0b03ec54fb991dc9125d6a79a2fba64a34fb710d97c36343c402` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `error_animated_stsd_short_av01_entry.avif` | Five-frame AVIS-only track whose `av01` entry declares an empty payload; Pillow rejects it at open | `scripts/generate_avif_stsd_edge_fixtures.py`; the entry size changes to its eight-byte header and encoded samples remain unchanged | `2fd1bb9882767e930ab717a2e8c970de48643788b11546af73cf91a09582af91` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated_lossless_inter_420_clipped_b32x32_17x17.avif` | Deterministic two-frame 17x17 RGB sequence encoded as an MI-clipped lossless AV1 4:2:0 inter-grid witness | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 8, one thread | `b29bedb6d7486dae5bed2abb74986a65e47bc737bfd26f85b1f69fa8bd666819` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_i422_clipped_b32x32_49x64.avif` | Deterministic two-frame odd-width 49x64 RGB 4:2:2 sequence selecting a clipped lossless AV1 B32x32 edge grid | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 32x32 partitions, timestamp-normalized | `fd18cdcef915327fd4aab6992c901e27f2efc1411d1d2d01eee93dd3f277d7b1` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_i444_clipped_b32x32_49x64.avif` | Deterministic two-frame 49x64 RGB 4:4:4 sequence selecting a clipped 24x32 lossless AV1 B32x32 edge grid | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 32x32 partitions, timestamp-normalized | `d45defceb497272879b1093f76e6a6c645f0bde8a6d10a1903e2322af422caae` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_i422_clipped_b32x32_52x64.avif` | Deterministic two-frame 52x64 RGB 4:2:2 sequence selecting a clipped 20x32 lossless AV1 B32x32 edge grid | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 32x32 partitions, timestamp-normalized | `43bbcd3d92593e026d88ce4dea727074f74134175ee42a962886c6de697a4e05` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_i444_clipped_b32x32_64x56.avif` | Deterministic two-frame 64x56 RGB 4:4:4 sequence selecting a clipped 32x24 lossless AV1 B32x32 bottom-edge grid | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 32x32 partitions, timestamp-normalized | `9004ec840b2a413c0e15294f52c723ebe5c3689cbe3decbd4afd5575cad78e23` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_i422_clipped_b32x32_52x60.avif` | Deterministic two-frame 52x60 RGB 4:2:2 sequence selecting a simultaneously right-and-bottom-clipped 20x28 luma, 10x28 chroma lossless AV1 B32x32 grid | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 32x32 partitions, timestamp-normalized | `53e06d79c586151d920b94cc6173eebcc1f9869ebb0f1fa5bec673df11c45e50` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_i422_fixed_b16x16_64x64.avif` | Deterministic two-frame 64x64 8-bit I422 all-lossless sequence with fixed 16x16 inter partitions and a moving textured patch | `scripts/generate_avif_lossless_inter_i422_b16x16.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; one thread, timestamp-normalized | `43866d1bf796b85eab547f3c7859e063599209bbec528d49e2fec74b681e4343` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_i444_fixed_b64x64_64x64.avif` | Deterministic two-frame 64x64 8-bit I444 all-lossless sequence with textured chroma residuals and fixed 64x64 partitions | `scripts/generate_avif_lossless_inter_i444_b64x64.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; one thread, timestamp-normalized | `89000cefae8e2c8c1a933be59cb42ee6749747cb62909c21378d54be54d5d0ff` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_i444_clipped_b32x32_52x60.avif` | Deterministic two-frame 52x60 RGB 4:4:4 sequence selecting a simultaneously right-and-bottom-clipped 20x28 lossless AV1 B32x32 grid | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 32x32 partitions, timestamp-normalized | `9fdd099eb28fed9eaff2760b154e42d91933758fd6b01e532e441646f564993a` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b32x32_60x64.avif` | Deterministic two-frame 60x64 RGB sequence with a translated patch selecting a partially visible lossless AV1 4:2:0 B32x32 inter grid | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 8, one thread | `a60dde64c6b7bf9f6fbb3afe7e75a74374d446f8106613b00837300df4963bbc` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b32x32_64x60.avif` | Deterministic two-frame 64x60 RGB sequence with a translated lower-edge patch for bottom-edge frame cropping | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 8, one thread | `7602fd062d0aefdaa0221b35a3a6c9403fecc27bf8a66cb00c0b4f9b8763c0d6` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b32x32_184x64.avif` | Deterministic two-frame textured 184x64 RGB sequence selecting a clipped lossless AV1 4:2:0 B32x32 inter grid at the right edge | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 32x32 partitions | `0a8c935fe67694fe576e7f21064eec4c428cffc2a05a3ff2d0be33e401a20c1d` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b16x16_60x64.avif` | Deterministic two-frame 60x64 RGB texture shifted one column to select a right-edge lossless AV1 4:2:0 B16x16 inter block clipped to 12 columns | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 16x16 partitions | `7c8bd0f88cd6654fb1150d2220ecb33c6ec5874b26a2250909b51cab4ed2e17f` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossless_inter_420_clipped_b16x16_10bit_partition16_60x64.avif` | Deterministic two-frame 60x64 10-bit full-range 4:2:0 lossless sequence with fixed B16x16 partitions and a translated texture at the right edge | `scripts/generate_avif_10bit_lossless_inter_fixture.py`; libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread; timestamp-normalized | `ce481c1839200b05932d69730174d5967973aa7f4b3af874a108f2660f01142f` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_b16_mixed_topology_inter_420_b16x16.avif` | Deterministic three-frame 16x16 4:2:0 sequence whose final B16x16 inter block splits only its top-right TX8 child to TX4 | `scripts/generate_avif_b16_mixed_topology_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 99, speed 0, one thread, fixed 16x16 partitions, timestamp-normalized | `3264482e2a50d80bd39be178b843fdfe9997e947d54b4466c745751837821d74` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_mode_largest_16x64.avif` | Deterministic two-frame 16x64 8-bit 4:2:0 sequence whose inter frame signals `TX_MODE_LARGEST` at positive qindex | `scripts/generate_avif_inter_i420_mode_largest_16x64.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 76, speed 0, one thread, partitions 16–64, timestamp-normalized | `4e2eba0851921d8629b8c3d1d1660652bf9b5d01e279737cbe7594e2e0a9ec40` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_mode_select_16x64.avif` | Deterministic two-frame 16x64 8-bit 4:2:0 sequence whose inter frame signals `TX_MODE_SELECT` at qindex 9 | `scripts/generate_avif_inter_i420_mode_select_16x64.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 99, speed 8, one thread, partitions 16–64, timestamp-normalized | `6de7ecf1d5ed5eab61c60fab3899cdc328cf3416ddcc1338bc7823412e64ddd2` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_b16x64_mode_select_64x64.avif` | Deterministic two-frame 64x64 8-bit 4:2:0 sequence whose inter root uses `PARTITION_VERT_4` to create four B16x64 leaves at positive qindex and `TX_MODE_SELECT`; the left leaf has a low-frequency residual while the other leaves retain texture | `scripts/generate_avif_inter_i420_b16x64_mode_select.py`; fixture-only libavif 1.4.1 and libaom 3.13.2 with `libaom-lossy-inter-i420-b16x64.patch`; Pillow 12.2.0 exact RGB frames; creation/modification timestamps normalized | `6241cd581ea930154ddf66554966a069d03ebf06c8d2a4880a4a0008398e1a4a` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_b16x64_qindex0_mode_select_64x64.avif` | Deterministic two-frame 64x64 8-bit 4:2:0 sequence whose lossless key frame is followed by an inter root using `PARTITION_VERT_4` to create four `TX_MODE_SELECT` B16x64 leaves at non-lossless base qindex zero with chroma deltas of 2 | `scripts/generate_avif_inter_i420_b16x64_qindex0_mode_select.py`; fixture-only libavif 1.4.1 and libaom 3.13.2 with `libaom-lossy-inter-i420-b16x64.patch` and `libaom-qindex0-nonlossless-inter.patch`; Pillow 12.2.0 exact RGB frames; timestamps normalized | `f9a69337b2eab0d0e6623aa874fdfcf58c35ada2154e8377a595b401c487317c` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_mode_largest_64x16.avif` | Deterministic two-frame 64x16 8-bit 4:2:0 sequence whose inter frame signals `TX_MODE_LARGEST` at positive qindex | `scripts/generate_avif_inter_i420_mode_largest_64x16.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 76, speed 0, one thread, partitions 16–64, timestamp-normalized | `74d3b5b9b06e208175d4ac533c05de73eb37f46fce8cfc2de581116689d59468` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_b64x16_mode_largest_64x64.avif` | Deterministic two-frame 64x64 8-bit 4:2:0 sequence whose inter root uses `PARTITION_HORZ_4` to create four coded B64x16 leaves at positive qindex and `TX_MODE_LARGEST` | `scripts/generate_avif_inter_i420_b64x16.py --mode largest`; fixture-only patched libaom 3.13.2; Pillow 12.2.0 / libavif 1.4.1; quality 70, speed 0, one thread, timestamp-normalized | `35f16973d0bda15b2a2038102b3f646f2b04d6e07157933bd03b8b32a6452daf` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_b64x16_mode_select_64x64.avif` | Deterministic two-frame 64x64 8-bit 4:2:0 sequence whose inter root uses `PARTITION_HORZ_4` to create four coded B64x16 leaves at positive qindex and `TX_MODE_SELECT`; the top leaf has a low-frequency residual while the other leaves retain texture | `scripts/generate_avif_inter_i420_b64x16.py --mode select`; fixture-only patched libaom 3.13.2; Pillow 12.2.0 / libavif 1.4.1; quality 70, speed 0, one thread, timestamp-normalized | `84eab6d96f97deb02fca49e66602f9cb507d2281cde5d4b52b2a50c54cb0433a` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_b8x8_mode_select_64x64.avif` | Deterministic two-frame 64x64 8-bit 4:2:0 sequence whose lossy inter-frame square partitions are forced down to B8x8 and signal `TX_MODE_SELECT`; both exact Pillow RGB frames are retained | `scripts/generate_avif_inter_i420_b8x8.py`; fixture-only patched libaom 3.13.2; Pillow 12.2.0 / libavif 1.4.1; quality 70, speed 0, one thread, timestamp-normalized | `a4963f38d42c857cbacd6306ab5585c0e42455fd6c3725dfdffda27b7553e6f1` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_i444_b8x16_mode_select_64x64.avif` | Deterministic two-frame 64x64 8-bit I444 sequence whose lossy inter partitions are forced into vertical B8x16 leaves and signal `TX_MODE_SELECT`; both exact Pillow RGB frames are retained | `scripts/generate_avif_inter_i444_b8x16.py`; fixture-only patched libaom 3.13.2; Pillow 12.2.0 / libavif 1.4.1; quality 70, speed 0, one thread, timestamp-normalized | `0187f7634e7ffb21019961fbb47091c3c2deaaccd88a04c043a7ffe1b05b8f93` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_i444_b16x64_mode_select_10bit_64x64.avif` | Deterministic two-frame 64x64 full-range 10-bit I444 sequence whose lossy inter root is forced into vertical B16x64 leaves and signals `TX_MODE_SELECT`; both exact Pillow RGB frames are retained | `scripts/generate_avif_inter_i444_b16x64_mode2.py`; fixture-only patched libaom 3.13.2; Pillow 12.2.0 / libavif 1.4.1; quality 75, speed 0, one thread, timestamp-normalized | `86fdcab5cc8355ec8706ea40ea3e9f75e8ee05a5b54c3043bf7416417bfe1de6` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_b64x16_10bit_mode_select_64x64.avif` | Deterministic two-frame 64x64 10-bit 4:2:0 sequence whose inter root uses `PARTITION_HORZ_4` to create four coded B64x16 leaves at positive qindex and `TX_MODE_SELECT`; the top leaf has a low-frequency residual while the other leaves retain texture | `scripts/generate_avif_inter_i420_b64x16.py --mode select --bit-depth 10`; fixture-only patched libaom 3.13.2; Pillow 12.2.0 / libavif 1.4.1; quality 70, speed 0, one thread, timestamp-normalized | `0d85dc1e3e8958e7375c3dcddd048f93cc5bd7ad12f50f82f1299f1c482c1880` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_b64x16_12bit_mode_select_64x64.avif` | Deterministic two-frame 64x64 12-bit profile-2 4:2:0 sequence whose inter root uses `PARTITION_HORZ_4` to create four coded B64x16 leaves at positive qindex and `TX_MODE_SELECT`; the top leaf has a low-frequency residual while the other leaves retain texture | `scripts/generate_avif_inter_i420_b64x16.py --mode select --bit-depth 12`; fixture-only patched libaom 3.13.2; Pillow 12.2.0 / libavif 1.4.1; quality 70, speed 0, one thread, timestamp-normalized | `8583340a327c7feffa803fa3b0a80267a49b6fa21c062f9b19799e2a82bf1dcc` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_i444_compound_split_b16x32_mode2.avif` | Deterministic three-frame 32x32 4:4:4 sequence whose final mode-2 B16x32 inter split permits compound reference selection from both earlier frames | `scripts/generate_avif_i444_compound_split_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 75, speed 0, one thread, partitions 16–32, timestamp-normalized | `bb43388411119ac10f7990ecb8523e8b62f85227b501e3402b962b5c2b624714` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_i444_obmc_mixed_b16x32.avif` | Deterministic 32x32 checker sequence encoded as an AV1 4:4:4 inter-prediction edge-case witness | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 75, speed 0, one thread, partitions 16–32 | `71e737f7b196173d98ae6447925cd8f6f77d95ced3eaa80d924ad81febcb3743` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_422_checker_random_b32x32.avif` | Deterministic two-frame 32x32 jittered-checker sequence encoded as a lossy AV1 4:2:2 inter-prediction witness | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 75, speed 0, one thread, partitions 16–32 | `e46aef4aed9076da48627a449e53c6f318a0494ba5d94a32a4f13edc548ab134` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_422_tx_context_checker_b64x32.avif` | Deterministic two-frame 64x32 jittered-checker sequence whose lossy I422 inter block has a subdivided transform tree adjacent to intra transform syntax | `scripts/generate_avif_inter_422_tx_context_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 95, speed 0, one thread, partitions 16–32, timestamp-normalized | `38c94eed681628437dd11423d62573fee3b1325a749ffc9f74dda1301c6acd56` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_422_compound_refs_b64x32.avif` | Deterministic three-frame 64x32 jittered-checker I422 sequence whose final inter frame uses compound prediction from both earlier frames | `scripts/generate_avif_inter_422_compound_refs_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 75, speed 0, one thread, partitions 16–32, timestamp-normalized | `58179bf4eec6cd6a78dc575646dc3db0fa30b0d8c844f74a1dbf02694c785350` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_422_compound_direct_grid_b64x64.avif` | Deterministic three-frame 64x64 textured I422 sequence whose largest-transform final inter block uses compound prediction and the direct chroma-grid path | `scripts/generate_avif_inter_422_compound_direct_grid_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 75, speed 0, one thread, partitions 64–64, timestamp-normalized | `5eef65a8499170972eb9edf607844b39292721f4e53ebbcdc4d61e78f2d7c51e` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_422_mode_select_b64x64.avif` | Deterministic three-frame 64x64 textured I422 sequence whose single-reference final inter frame signals TX_MODE_SELECT at positive qindex | `scripts/generate_avif_inter_422_mode_select_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 75, speed 4, one thread, partitions 64–64, timestamp-normalized | `eba71559d1bd1c34ebce8d3bd82ea3f9aea8935ac8f2749ed42784423a1126d6` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_422_direct_grid_mode_select_mixed_b128x64.avif` | Deterministic three-frame 128x64 I422 sequence with two 64x64 regions; its TX_MODE_SELECT final frame repeats the right source region and blends the left | `scripts/generate_avif_inter_422_direct_grid_mode_select_mixed_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 75, speed 4, one thread, partitions 64–64, timestamp-normalized | `7aee9a35ab56e6209dbc11b2feccd8d636edc4c6b7b448d128c66832d6da198e` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_444_direct_grid_mode_select_mixed_b128x64.avif` | Deterministic three-frame 128x64 I444 sequence with two 64x64 regions; its single-reference TX_MODE_SELECT final frame exercises direct full-resolution chroma grids | `scripts/generate_avif_inter_444_direct_grid_mode_select_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 75, speed 4, one thread, partitions 64–64, timestamp-normalized | `8eb2916792a241372d03d5ccf1802549363ad61325c3ec3489b36752aaa7cfc2` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_422_vertical_split_b32x64.avif` | Deterministic two-frame 32x64 lossy I422 sequence whose vertical texture selects inter B32x64 transform splits | `scripts/generate_avif_inter_422_vertical_split_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 95, speed 0, one thread, partitions 16–32, timestamp-normalized | `0eb45159064b3f1c30919d0062afa19056415f76be203278cd3ddd7174e8a798` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_deep_b32x64_64x64.avif` | Deterministic two-frame 64x64 lossy I420 sequence forced into vertical B32x64 inter blocks with deep transform splits | `scripts/generate_avif_lossy_inter_i420_b32x64.py`; fixture-only patched libaom 3.13.2; Pillow 12.2.0 exact RGB frames; timestamp-normalized | `a64648f0a78b83b679a0a09448688be51e4c933b84a0215b6af8a766b52e2e8b` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_topology_b32x64_64x64.avif` | Deterministic two-frame 64x64 lossy I420 sequence with B32x64 blocks using mixed one-child and deep transform splits | `scripts/generate_avif_lossy_inter_i420_b32x64.py --profile topology`; fixture-only patched libaom 3.13.2; Pillow 12.2.0 exact RGB frames; timestamp-normalized | `71e64835c2d78a25599a5b195c9a819dba8b0f61ac63136a64af2932f7bdddd6` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_422_small_partitions_32x64.avif` | Deterministic two-frame 32x64 lossy I422 sequence encoded with 8–16-pixel inter partition bounds | `scripts/generate_avif_inter_422_small_partitions_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 70, speed 0, one thread, timestamp-normalized | `f150bd51312eee0a9e7dccc57b3d26fc08899c400d82565b839d704e98c6c029` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_i422_loopfilter_active_16x16.avif` | Deterministic two-frame 16x16 8-bit I422 sequence with a lossy `TX_MODE_LARGEST` inter frame, nonzero Y/U/V loop filter, and reference-motion/compound tools disabled | `scripts/generate_avif_inter_i422_loopfilter_16x16.py`; fixture-only libaom loop-filter patch; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 70, speed 0, one thread, timestamp-normalized | `757c9fc631ecee8522b14e66fafc6aebcbe90aa2bd5a3cc2d27df472e7440481` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_422_qindex0_mode_select_64x64.avif` | Deterministic two-frame 64x64 8-bit I422 sequence with a lossless qindex-zero key and a non-lossless qindex-zero `TX_MODE_SELECT` inter frame using a luma DC delta | `scripts/generate_avif_inter_422_qindex0_mode_select.py`; fixture-only libaom 3.13.2 qindex-zero luma-delta patch; Pillow 12.2.0 exact RGB frames; timestamps normalized | `062cec9bf86cfa7e5d2130baec72884dec70d96d9621578f91388612d21a1fab` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_b8x16_mixed_topology_64x64.avif` | Deterministic two-frame 64x64 lossy I420 sequence with mixed TX4x4 and TX8x8 children in inter B8x16 transforms | `scripts/generate_avif_inter_i420_b8x16_mixed_topology.py`; pinned fixture-only libaom transform-tree patch; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 70, speed 0, one thread, timestamp-normalized | `07e1d146cf7a3c5f565fab5eddecd19ae53cca665a70faf6078e1b05baeebe12` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_420_single_extended_ref_128x128.avif` | Deterministic two-frame 128x128 lossy I420 sequence with one localized 16x16 changed patch; reaches the positive-reference lane of the AV1 single-reference extended motion-vector search | `scripts/generate_avif_single_extended_ref_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 60, speed 0, one thread, fixed 64-pixel superblocks, timestamp-normalized | `6fe9058db08a06e520922eb2d63d2dc1111d842e1f8d26e023acd560fbdde271` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_global_rotzoom_compound_i444_b128x128.avif` | Deterministic four-frame 256x256 4:4:4 affine-motion sequence encoded as a global-motion edge-case witness | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 80, speed 0, one thread | `14ffb1529c54f07a397a1fe94d02135f9def3a7730416cb80d5acbf3c54c144e` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_global_subexp_endpoint_i444_b128x128.avif` | Four-frame 256x256 I444 RotZoom sequence reaching the terminal AV1 subexponential `ns()` branch | `scripts/generate_avif_global_motion_subexp_endpoint_fixture.py`; deterministic same-length frame-header mutation; Pillow 12.2.0 exact RGB frames | `ff156ac8e1bfeb017b38846ef0dc4abca0312083523d317a7a892bde16ef3e97` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_global_halfblend_spatial_i444_b256x256.avif` | Deterministic four-frame 256x256 4:4:4 sequence with identity and RotZoom global references paired in compound spatial neighbors | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 80, speed 0, one thread | `ec2f145f64fa87c5b7c255b13b6009fb58760be06070724ed367cd4ccd9a6d7b` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_inter_i444_tx_largest_64x64.avif` | Deterministic three-frame 64x64 8-bit lossy I444 sequence whose final inter frame header signals transform mode `largest` and reference mode `select` | `scripts/generate_avif_inter_i444_frame_header_b64.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 50, speed 0, one thread; timestamp-normalized | `270ab1fb4f9e066d6898c52bdd8d94911502684d13eb7cdc62d1c034a57148ea` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_wide_i422_b128x64.avif` | Deterministic two-frame 128x128 RGB sequence whose second lossy I422 frame horizontally splits its 128x128 root into two 128x64 inter leaves | `scripts/generate_avif_lossy_wide_i422_b128x64.py`; fixture-only libaom patch at `ad44980`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 65, speed 0, one thread, timestamp-normalized | `dbfb8c364abaddaeab4894511ff28f42fa9512226a27af097da35975a4ee5dbc` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_wide_i420_b128x128.avif` | Deterministic two-frame 256x256 8-bit I420 sequence with four 128x128 inter blocks and 32x32 chroma chunks | `scripts/generate_avif_lossy_wide_i420_b128x128.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 80, speed 0, one thread, fixed 128-pixel partitions, timestamp-normalized | `a8cdebb85ccc240bbab9adde924e13b13d82e76d22600beb8511e62b58ae8bdd` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_wide_i420_10bit_b128x128.avif` | Deterministic two-frame 128x128 full-range 10-bit I420 sequence with a 128x128 superblock, fixed 128-pixel partition limits, largest transforms, and active SGR restoration | `scripts/generate_avif_lossy_wide_i420_10bit.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 75, speed 0, one thread, timestamp-normalized | `4167f64f7b7336c0f16cbac49379cae49671a988f80b2ba066fdec9fe54c32d4` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_compound_wide_i420_b128x128.avif` | Deterministic three-frame 256x256 8-bit I420 sequence with compound-selectable 128x128 inter blocks and transform residuals | `scripts/generate_avif_lossy_compound_wide_i420.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `d0491ed004f7e6d722ce55b7cbeaa6db1898073a9868b306ef412f89693b7f71` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_compound_wide_i420_unsplit_b128x128.avif` | Deterministic three-frame 256x256 8-bit I420 sequence whose final compound frame averages both reference frames | `scripts/generate_avif_lossy_compound_wide_i420.py --variant unsplit`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; timestamp-normalized | `8fed3a09dd26a5baf9aa52918f479ee8f83ebfe49642f8b1c021a86fca9abe35` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_interinter_wedge_i420_b16x16_64x64.avif` | Deterministic three-frame 64x64 8-bit I420 sequence whose final frame applies the AV1 B16x16 soft wedge mask to every pair of seeded grayscale references | `scripts/generate_avif_interinter_wedge_i420.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 99, speed 0, one thread, fixed 16-pixel partitions, timestamp-normalized | `48defb0aff134ef32ed8c2e0ade85619f7c0dc1d48bc2473b7b8a67b0e54ffed` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_interinter_wedge_i422_b16x16_64x64.avif` | Deterministic three-frame 64x64 8-bit I422 sequence whose final frame applies the AV1 B16x16 soft wedge mask to every pair of seeded grayscale references | `scripts/generate_avif_interinter_wedge_i420.py --variant i422`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 99, speed 0, one thread, fixed 16-pixel partitions, timestamp-normalized | `c8b77442e52d68206e6f4100a940a7e85a8deae6677371712ba6b0c760b565b6` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_interinter_wedge_i444_b16x16_64x64.avif` | Deterministic three-frame 64x64 8-bit I444 sequence whose final frame applies the AV1 B16x16 soft wedge mask to every pair of seeded grayscale references | `scripts/generate_avif_interinter_wedge_i420.py --variant i444`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 99, speed 0, one thread, fixed 16-pixel partitions, timestamp-normalized | `9525cb01d99c78fe0b479a37da45228ac168c468f4759e90b4cc3d37c3e51e66` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_interinter_diffwtd_i420_b16x16_64x64.avif` | Deterministic 64x64 I420 witness for the AV1 38-mask difference-weighted compound blend | `scripts/generate_avif_interinter_diffwtd.py --variant i420`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 16-pixel partitions, timestamp-normalized | `cfd5adb4e0c97a9c3d206f2de0b0a8ce195fd7aa4f69e3b937129cf581986ae0` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_interinter_distance_i420_b16x16_64x64.avif` | Deterministic 64x64 I420 witness with AV1 distance-compound syntax enabled, warped motion disabled, and an 11:5 final-frame blend of the two references | `scripts/generate_avif_interinter_distance_i420.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 16-pixel partitions, timestamp-normalized | `7abfec6d36ae819aaea9a9dc814c33c538488ba87362cd174b5cd62dfcaaab13` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_interinter_superres_distance_i420_b16x16_64x64.avif` | Deterministic three-frame 64x64 I420 witness whose final inter frame combines 51-pixel coded width, 64-pixel upscaled references, and selectable distance-compound prediction | `scripts/generate_avif_interinter_superres_distance_i420.py`; Pillow 12.2.0 / patched libavif 1.4.1 / libaom 3.13.2; quality 45, super-resolution denominator 10, fixed 16-pixel partitions, timestamp-normalized | `bf6049e6ed687af3367ec31f43517f5cbcd5633171d7dd381b6eb7a6ae9f36ad` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_interinter_diffwtd_i420_inverted_b16x16_64x64.avif` | Deterministic 64x64 I420 companion using the inverted AV1 difference-weighted mask | `scripts/generate_avif_interinter_diffwtd.py --variant i420-inverted`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 16-pixel partitions, timestamp-normalized | `23c64470703c11c0ecfedfd906cc7b5098db1cc4b707676464c8c8064f91dd45` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_interinter_diffwtd_i422_b16x16_64x64.avif` | Deterministic 64x64 I422 witness for horizontal reduction of the AV1 difference-weighted compound mask | `scripts/generate_avif_interinter_diffwtd.py --variant i422`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 16-pixel partitions, timestamp-normalized | `fad2e3b5264d0ec4d58dd53beea98c15462fbf36fd02e0a82a3f21447e291962` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_interinter_diffwtd_i444_b16x16_64x64.avif` | Deterministic 64x64 I444 witness for unreduced AV1 difference-weighted compound masks | `scripts/generate_avif_interinter_diffwtd.py --variant i444`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 100, speed 0, one thread, fixed 16-pixel partitions, timestamp-normalized | `abafc8fd5f727e83dbef669a3e43e57b283e780e0274a55f540da9070eae837b` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_wide_i444_mode2_split32_b128x128.avif` | Deterministic two-frame 256x256 I444 sequence selecting TX64-to-TX32 splits in each 128x128 mode-2 inter leaf | `scripts/generate_avif_mode2_split32_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 80, speed 0, one thread, fixed 128-pixel partitions | `f48a6d235c7ab245e8d57aedcc6135b897cea1bcbc579c3d630aa7c20e1a61aa` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_wide_i444_mode2_deep16_b128x128.avif` | Deterministic five-frame 256x256 I444 sequence selecting TX64-to-TX32-to-TX16 splits in each 128x128 mode-2 inter leaf | `scripts/generate_avif_mode2_deep16_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 95, speed 0, one thread, fixed 128-pixel partitions | `a806f4585aa11699bc831fff1b02b7f1eacbd2089a88a2578bb6933afbcbdaf0` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_wide_i444_mode2_mixed_b128x128.avif` | Deterministic two-frame 256x256 I444 sequence with mixed TX64 root splitting in one 128x128 mode-2 inter leaf | `scripts/generate_avif_mode2_mixed_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 80, speed 0, one thread, fixed 128-pixel partitions | `0e72f7d47e8e83e0a6e13a76a816cb98974cdb2976fff72857901712e2633d61` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_i420_mode2_variable_tx_loopfilter_b128x128.avif` | Deterministic two-frame 256x256 I420 sequence with one 128x128 mode-2 inter leaf using mixed luma transforms under nonzero loop-filter levels | `scripts/generate_avif_loopfilter_variable_tx_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 30, speed 0, one thread, fixed 128-pixel partitions, loop filtering enabled | `7be16326561aa3e67340b787cecfe9c5917ea282b312a28f665a03549fd62e31` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_interintra_420_nowedge_mode3_b16x16_64x64.avif` | Deterministic three-frame 64x64 4:2:0 smooth inter-intra mode-3 witness | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 50, speed 0, one thread | `7e035cef7293dfff72e50ff8c729bc0fad360ee14d7bf80d663b82b96ca7c4d7` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_interintra_420_wedge_b16x16_64x64.avif` | Deterministic colored three-frame 64x64 4:2:0 sequence whose final B16x16 block selects AV1 inter-intra wedge index 4 and exercises chroma-mask downsampling | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 50, speed 0, one thread | `1510b764d7e642f2caf02fff53e6dc966cfb92df8eca2e8f2d91c0b3bead1791` | MIT/Apache-2.0 (repository-generated) |
| `animated_lossy_interintra_420_wedge_idx2_no_flip_b16x16_64x64.avif` | Deterministic three-frame 64x64 4:2:0 sequence selecting the oblique117 inter-intra wedge index 2 with sign flip disabled | `scripts/generate_avif_interintra_oblique_wedge_no_flip_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 50, speed 0, one thread | `62f1f2704bdd6efcd3b936909dd3d9351116fb8d072add8a0194d9ca5459e275` | MIT/Apache-2.0 (repository-generated) |
| `animated_tx64_root_split_inter_420_64x64.avif` | Deterministic three-frame 64x64 4:2:0 sequence with a TX64 root split and two TX32 child offsets | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 80, speed 0, one thread, fixed 64x64 partitions | `9f4450d4d9c7c2738d4f9f34eafb02100c5fb15b85ac121a057c3eef367f6d7c` | MIT/Apache-2.0 (repository-generated) |
| `animated_motion_temporal_window_left_512x128.avif` | Deterministic four-frame 512x128 4:2:0 translated-patch sequence that misses the projected motion window on the left | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 95, speed 0, one thread | `29e856c6c8a117c764bb0a176ccc1d82bac982fdf7287eb461f827ebdc31b399` | MIT/Apache-2.0 (repository-generated) |
| `animated_motion_temporal_window_right_512x128.avif` | Deterministic four-frame 512x128 4:2:0 translated-patch sequence that misses the projected motion window on the right | Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 95, speed 0, one thread | `10d8e5514c1d8d9cad355f9a912b3e3075bf8bd661ee6af5ce44a9ce62d7cad2` | MIT/Apache-2.0 (repository-generated) |
| `animated_motion_temporal_mv_range_2048x128.avif` | Deterministic three-frame 2048x128 4:2:0 sequence translates a 16-pixel-run texture 600 pixels and back | `scripts/generate_avif_temporal_mv_range_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 95, speed 0, one thread, timestamp-normalized | `294468c2498c8fa494bfcc2e701a944e90f15ec4b5f08eb64fa34f907e5a6a1a` | MIT/Apache-2.0 (repository-generated) |
| `animated_motion_temporal_mv_range_positive_x_2048x128.avif` | Deterministic three-frame 2048x128 4:2:0 sequence translates the texture 600 pixels left and back | `scripts/generate_avif_temporal_mv_range_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 95, speed 0, one thread, timestamp-normalized | `f802a724db9dea26e0d71abb7d123c9338449e77008282874f58580c0b2163d1` | MIT/Apache-2.0 (repository-generated) |
| `animated_motion_temporal_mv_range_negative_y_128x2048.avif` | Deterministic three-frame 128x2048 4:2:0 sequence translates 16-pixel color bands 600 pixels down and back | `scripts/generate_avif_temporal_mv_range_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 95, speed 0, one thread, timestamp-normalized | `e32c9ff32e87922be280d26fef6d8c92635e84d2918df7c23f3f8ec3087b051c` | MIT/Apache-2.0 (repository-generated) |
| `animated_motion_temporal_mv_range_positive_y_128x2048.avif` | Deterministic three-frame 128x2048 4:2:0 sequence translates 16-pixel color bands 600 pixels up and back | `scripts/generate_avif_temporal_mv_range_fixture.py`; Pillow 12.2.0 / libavif 1.4.1 / libaom 3.13.2; quality 95, speed 0, one thread, timestamp-normalized | `307a001ecd6e10c4b840a3d0e0b30b24a6a90ac1c8f5f5ea1b8d9e209b1c3333` | MIT/Apache-2.0 (repository-generated) |

The upstream libavif test-data README identifies the copied libavif files as
covered by libavif's own license. `10bit.avif` retains its historical manifest
name but is a 12-bit high-bit-depth animation; the manifest description states
the exact depth.

`mixed_lossless_clipped_b32_i420_60x60.avif` uses the fixture-only patches
and C helper documented in `scripts/avif_fixture_oracle/README.md`. Its
generator proves the fixed 32x32 partition and segment-1 qindex-zero leaf from
the pinned encoder trace, verifies the mixed AV1 frame header, requires
byte-for-byte repeated encoding, and writes its exact Pillow RGB reference
only after the pinned Pillow 12.2.0/libavif 1.4.1 hash check succeeds.

`animated_mixed_lossless_inter_b32_i420_64x64.avif` is a separate two-frame
sequence with lossy and qindex-zero lossless segments. The pinned libaom trace
proves the second frame's full-visible B32x32 leaf at MI `(8,8)` uses inter
prediction; the generator checks this block mode, AV1 headers, repeated
encoding, and both exact Pillow RGB frames.

`animated_mixed_lossless_inter_b32_i420_60x60.avif` uses the same deterministic
sequence with a 60x60 canvas. Its inter-predicted B32x32 at pixel origin
`(32,32)` is visible as 28x28 pixels; its lossless segment remains alongside
lossy segments. The generator checks this block mode, the clipped AV1 headers,
repeated encoding, and both exact Pillow RGB frames.

`animated_mixed_lossless_inter_b32_i420_residual_64x64.avif` uses an inter
frame whose full-visible, unsplit qindex-zero B32x32 ROI is intra-predicted
while every pixel in its 32x32 region changes. The pinned encoder trace proves
the prediction mode and partition; the generator verifies repeated encoding,
both exact Pillow RGB frames, and all 1,024 changed ROI pixels.

`animated_mixed_lossless_inter_b32_i420_impulse_residual_64x64.avif` keeps the
same mixed-lossless segment map but adds one residual at pixel `(33,33)` inside
the full-visible B32x32 ROI. The pinned encoder trace proves this leaf remains
inter-predicted with `skip_txfm=0`; the AV1 bitstream reports qindex zero and a
lossless segment, and the generator checks repeatability plus exact Pillow
frames.

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

`animated_lossless_inter_monochrome_b32x32_10bit.avif` uses the same pinned
generator and encoder helper, with the helper's `mono` option selecting the
AV1 monochrome plane layout. It is a 32x32 full-range 10-bit all-lossless
key/inter sequence whose second frame translates a patch inside a full-size
B32x32 inter leaf. Its normalized file SHA-256 is
`28427120f6a0e9de71d0abcfe36bbeaae4d701dacefb62f852dfd71eeb82d624`; the
Pillow 12.2.0 RGB frame hashes are
`9eb4dc1f2ea4eb4cd5bc4dca1bc035a1eeee1d08248a527348e3be8cb5d25129` and
`c40bb457a52128f753525937c909cd08ebd8f96d7a5431a4e9751392dc5b487f`. This
parity case exercises high-depth inter reconstruction while excluding the
generic I444 path for monochrome input.

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

The 12-bit companion exercises the same clipped right-edge B32x32 inter grid
with AV1 profile 2. The generator scales the deterministic 10-bit source
samples by four, verifies the 12-bit sequence header and repeated encoded
samples, and checks the exact Pillow 12.2.0 RGB frames. Its normalized input
hash is `cec5a564546c2ef8893c3f4e44c7f46c49ea9daa89d1f782a06717f11d183c87`;
the frame hashes are the same RGB8 outputs as the 10-bit companion:
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

`animated_lossy_inter_444_superres_sgr_10bit_160x56.avif` extends this
super-resolution profile to full-resolution 4:4:4 chroma. The key frame is flat
at sample 512; the inter frame alternates 448/576 in 8x8 tiles, with the V
plane inverted. The generator disables transform-size search to retain
`TX_MODE_LARGEST`, enables SGR on all three planes, double-encodes, normalizes
BMFF timestamps, and checks AV1 syntax plus exact Pillow 12.2.0 RGB frames.
Regenerate it with the same pinned `avifenc` binary:

```sh
.oracle-venv/bin/python scripts/generate_avif_hi10_i444_superres_restoration.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc
```

The normalized 1,656-byte AVIF has SHA-256
`d0f13c712995e0464efb51575ca353aabd26c8ff6e3001c415fd98f239c8221c`. Its Y4M
source hashes are `e1dc6934f9e04e6837ad84fe8a55f5dc691f5426b9fa5c07ff4235776d364d60`
and `23ba56c49956deee3cc6d1778847a448d15c2023a0546c0d0bbf5ce4afb6064e`; the
RGB frame hashes are `69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed`
and `855f30260049bc5f85a057fa91223c85259acc4a55c4ae2d4ec66e45d88e4be6`.
Verify the retained fixture without rebuilding the encoder with:

```sh
.oracle-venv/bin/python scripts/generate_avif_hi10_i444_superres_restoration.py --check
```

`animated_lossy_inter_422_superres_sgr_10bit_160x56.avif` carries the same
10-bit super-resolution and all-plane SGR profile with 4:2:2 chroma. Its key
frame is flat at sample 512; the inter frame alternates 448/576 samples in
8x8 luma regions and inverts the V plane. The generator disables transform
size search to retain `TX_MODE_LARGEST`, then repeat-encodes, timestamp
normalizes, and verifies AV1 syntax plus exact Pillow 12.2.0 RGB pixels.
Regenerate or check the fixture with:

```sh
.oracle-venv/bin/python scripts/generate_avif_hi10_i422_superres_restoration.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc
.oracle-venv/bin/python scripts/generate_avif_hi10_i422_superres_restoration.py --check
```

The normalized 1,855-byte AVIF hashes to
`60748f33355d8f8b54f48942e319c1791e75c75c94da55a1568fd6679edf578a`. Its Y4M
source hashes are `38d38ec2544747c5d09d8a68548619fe5e8ec55417a5b6eb84e3ba263d0c1395`
and `896a665decff46a6912b5a0fc283ea310e0de67b52bc5fe9c003756fdb1f124f`; the
RGB frames hash to `69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed`
and `8b3079d8e82b1eda72b002a51cc3dfafc9e38344dc6a262d4e3c5e8c9493e940`.

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

`animated_lossy_inter_422_superres_sgr_8bit_160x56.avif` carries the same
largest-transform SGR profile in 8-bit I422. Its key frame is flat at sample
128; the inter frame alternates 112/144 values in 8x8 luma regions and the
V-plane pattern is inverted. The generator pins the AV1 sequence and inter
frame syntax, double-encodes the deterministic source, normalizes BMFF
timestamps, and verifies exact Pillow RGB frame hashes. Run it with:

```sh
.oracle-venv/bin/python scripts/generate_avif_8bit_i422_superres_restoration.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc
```

The normalized 1,601-byte asset hashes to
`a3f55297651b8659e9a76fda1276a2a6330151d802af1e72cdb659858cfcb1a1`; the RGB
frames hash to
`69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed` and
`06f17713c7758faaac74cd5cabd7d441aed55a41e8e03322a3e339b4a345ac2f`.

`animated_lossy_inter_422_superres_12bit_160x56.avif` retains the same
two-frame I422 super-resolution profile at 12-bit depth, with restoration
disabled on the inter frame. The key frame is flat at sample 2048; the inter
frame alternates 1792/2304 over 8x8 luma regions and inverts the V-plane
pattern. The generator pins the AV1 sequence and inter-frame syntax, repeats
the encode, normalizes timestamps, and checks exact Pillow 12.2.0 RGB frames.
Regenerate it with the pinned `avifenc` binary, or verify the retained fixture
with `--check`:

```sh
.oracle-venv/bin/python scripts/generate_avif_12bit_i422_superres.py \
  --avifenc target/oracle-staging/libavif-superres-build/avifenc
.oracle-venv/bin/python scripts/generate_avif_12bit_i422_superres.py --check
```

The 1,675-byte input hashes to
`565ad0b866eb0fd25b37c5b23e0c3dba8ab791312e782cf4d58fd80a92684db9`. The two
Pillow RGB frame hashes are
`69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed` and
`3acbd991ecc3cdbea09be468ef346e16f3413c9766e2cc6ebb8690b8e2f69ab6`.

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

`animated_lossy_inter_i444_compound_split_b16x32_mode2.avif` is generated by
`scripts/generate_avif_i444_compound_split_fixture.py` from two deterministic
checker frames and their blend. The final frame header permits compound
reference selection and names both earlier frames. The generator pins
timestamp-normalized bytes and exact Pillow RGB hashes; complete-file native
repetition and frame references are retained.

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

`animated_lossy_global_subexp_endpoint_i444_b128x128.avif` is a four-frame
256x256 I444 derivative of that RotZoom sequence. It changes two same-length
global-motion parameter codes in frame 2: one reaches the terminal AV1
subexponential `ns()` branch while preserving the frame-header and tile-data
boundaries. `scripts/generate_avif_global_motion_subexp_endpoint_fixture.py`
recreates the complete AVIF and verifies the changed matrix; Pillow frame
references and independent libavif loop evidence are recorded by the decode
manifest and AVIF loop bundle.

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

`animated_lossy_wide_i444_mode2_deep16_b128x128.avif` is generated by
`scripts/generate_avif_mode2_deep16_fixture.py`. Its flat gray key frame is
followed by checkerboards with 2-, 4-, 8-, and 16-pixel periods. With fixed
128-pixel partitions, all four I444 inter leaves split each TX64 root to TX32
and every TX32 child to TX16 in mode 2. The generator checks deterministic
timestamp-normalized bytes and all five pinned Pillow frame hashes; public
decode and complete-file loop references are retained.

`animated_lossy_wide_i444_mode2_mixed_b128x128.avif` is generated by
`scripts/generate_avif_mode2_mixed_fixture.py`. Its flat gray key frame is
followed by a localized row-alternating 32-pixel sinusoid. With fixed
128-pixel partitions, only the top-left 128x128 I444 inter leaf splits one
TX64 root to TX32. Its other three roots and the remaining 128x128 leaves stay
unsplit; no TX32 child splits. The generator checks deterministic bytes and
both pinned Pillow frame hashes; public decode and complete-file loop
references are retained.

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

`animated_lossy_interintra_420_wedge_idx2_no_flip_b16x16_64x64.avif` adds the
oblique117 wedge index-2 template with sign flipping disabled. Its deterministic
final-frame target uses the complementary wedge weight at B16x16 `(16,16)`;
a public Rust decode trace observed 256 mask samples for that template. The
generator checks the complete normalized input hash and all three exact Pillow
RGB frame hashes.

The inter-inter difference-weighted companions use the same seeded grayscale
references and form the final frame with AV1's 38-mask difference-weighted
formula. They include normal and inverted I420 masks plus I422 and I444 layouts.
Their selected public parity run checks all twelve decoded RGB frames against
Pillow. The native loop bundle retains each complete encoded input and frame.

`animated_lossy_inter_422_checker_random_b32x32.avif` is generated by
`scripts/generate_test_assets.py` from two 32x32 RGB frames with per-frame
seeded checker jitter and a translated patch. It uses Pillow 12.2.0, libavif
1.4.1, libaom 3.13.2, quality 75, speed 0, one thread, 16–32 pixel
partitions, and timestamp-normalized AVIF sequence boxes. The generator
double-encodes the sequence and checks the exact input hash. The two Pillow RGB
references are each 3,072 bytes, with hashes
`64a05a10f3a33ccd351e0c529f56cff96fabb33e0fc81a5f96f43c3f4c1c6c6e` and
`c98fa755c2b5a8ba48a7e821d9ecad46d4ebbc4cc59535b70c24255d3ffe2437`.

`animated_lossy_inter_422_compound_refs_b64x32.avif` is generated by
`scripts/generate_avif_inter_422_compound_refs_fixture.py` from two seeded
64x32 RGB checker frames and their pixelwise blend. It uses Pillow 12.2.0,
libavif 1.4.1, libaom 3.13.2, quality 75, speed 0, one thread, and 16–32
pixel partitions. The generator requires an 8-bit I422 sequence header, a
final inter frame that exposes both earlier references, repeat-encode
stability, and exact Pillow RGB frame hashes. Selected branch-instrumented
public parity confirms the final inter frame reaches compound reconstruction.
The normalized input hash is
`58179bf4eec6cd6a78dc575646dc3db0fa30b0d8c844f74a1dbf02694c785350`.

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

The `animated_motion_temporal_mv_range_*` fixtures are three-frame A/B/A
sequences. The two 2048x128 inputs translate a deterministic texture made from
16-pixel horizontal-color runs in opposite directions. The 128x2048 inputs
translate 16-pixel color bands vertically in opposite directions. Each
translation is 600 pixels, exposing the signed horizontal and vertical temporal
motion-vector bounds while preserving large inter-predicted regions. Each
retains exact Pillow frame references and a native AVIF loop observation. The
pinned generator double-encodes every sequence and checks its normalized file
and frame hashes.

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

`animated_lossless_inter_i422_fixed_b16x16_64x64.avif` uses two deterministic
64x64 RGB frames with a moving textured 16x16 patch. The pinned Pillow
12.2.0/libavif 1.4.1/libaom 3.13.2 generator encodes an 8-bit I422 all-lossless
sequence with fixed 16x16 inter partitions, normalizes movie timestamps, and
checks the input and both exact Pillow frame hashes. The public sequence
decoder matches both frames byte-for-byte; independent pinned native loop
evidence confirms the two-frame sequence.

`animated_lossless_inter_i444_fixed_b64x64_64x64.avif` uses two 64x64 RGB
frames with independently textured color channels. The pinned Pillow
12.2.0/libavif 1.4.1/libaom 3.13.2 generator encodes an 8-bit I444
all-lossless sequence with fixed 64x64 partitions and normalizes movie
timestamps. Both frame headers are checked as lossless key/inter pictures;
Pillow's exact frame pixels are retained for public sequence parity.

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

`animated_lossless_inter_420_clipped_b16x16_10bit_partition16_60x64.avif`
fixes 16x16 partitions in a full-range 10-bit 4:2:0 lossless sequence. Its
translated textured patch occupies the right edge of the 60x64 canvas. Pillow
retains exact RGB pixels for both frames, and the public Rust sequence decoder
matches both frames exactly.

`animated_track_only.avif` is deterministically derived from the pinned
`animated.avif` input by changing its one top-level `meta` box type to `free`
without changing the box size or any sample-table offsets. It therefore retains
the upstream BSD-2-Clause fixture license and proves that pinned Pillow/libavif
rejects an otherwise intact `avis` track sequence when item metadata is absent.
Its SHA-256 is
`45c85bd7d08261cfcb1a4563150e993d606b5e309b4767d922988ce700587fe4`.

`animated_missing_stco.avif`, `animated_missing_stsc.avif`, and
`animated_missing_stsd.avif` are generated by
`scripts/generate_avif_missing_sample_table_fixtures.py` from `animated.avif`.
Each replaces exactly one required sample-table box with a same-size `free`
box, preserving the encoded samples and all absolute offsets. The fixtures
retain the upstream BSD-2-Clause license. Pinned Pillow rejects each full file
with `UnidentifiedImageError: cannot identify image file <bytes>`. Their
SHA-256 values, in filename order, are
`71148f861ce9b342c4f9642216b2344a568c088adc06f6f7c1df6bac0fab60fe`,
`d09ef902eacd76da8ece326e0aeb5464e68caac10c2b3408461df271bf35be41`, and
`50e398a37b90e7217117bd573b4529a530b479442bc756738e3c5fbce0fdcdf6`.

`animated_stsc_first_chunk_not_one.avif` and
`animated_stsc_non_increasing_chunk.avif` are generated by
`scripts/generate_avif_stsc_order_error_fixtures.py` from the pinned
`animated_stsc_two_chunk_mapping.avif`. Each changes only one four-byte
`stsc` first-chunk field, preserving file length and encoded samples. Pillow
12.2.0 rejects both complete AVIF inputs at open. Their SHA-256 values, in
filename order, are
`9b54a6a24989c3ca9feaffb1779ced9407a9cad3f864415d0742e9def89eb46a` and
`a6032faac42c6646befcb10470541495702600e98d70fc56dcde3c7fde787f73`.

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

`high_bitdepth_still_12bit_444_alpha_9x9.avif` is generated by the full-file
`scripts/generate_av1_encoder_refs.py` collector from four constant 9x9
little-endian 12-bit planes: Y, U, V, and alpha use samples 464, 1633, 2802,
and 3971. The pinned libavif 1.4.1 commit `6543b22b5bc706c53f038a16fe515f921556d9b3`
and libaom 3.13.2 commit `ad44980d7f3c7a2605c25d51ea96946949000841` encode
full-range profile-2 YUV444 color and monochrome auxiliary alpha at quality
100, speed 6, one thread, and CICP 1/13/6. The collector confirms both AV1
items are 12-bit, one-tile, all-lossless frames and repeats the complete file
byte-for-byte. The input SHA-256 is
`f7eb5b9edfdd7b11d00094aadb91320e7cc6cc01961470e3c2040d91e68610aa`.
Pinned Pillow 12.2.0 opens, verifies, and loads one 9x9 RGBA8 frame; its exact
324-byte reference has SHA-256
`4a644e661e4cc9eb2ab3b4548d7f2275061b058e14a9370cffb63826f5c7a014`.
The odd 81-pixel sample count exercises ten full eight-lane conversion batches
and one scalar tail, including alpha output. Regenerate the collector bundle
with the pinned clean native source checkouts:

```sh
.oracle-venv/bin/python scripts/generate_av1_encoder_refs.py \
  --aom-source target/oracle-staging/libaom \
  --libavif-source target/oracle-staging/libavif \
  --output target/oracle-staging/av1-encoder-repro
cp -R target/oracle-staging/av1-encoder-repro/. tests/fixtures/outputs/av1_encoder/
cp target/oracle-staging/av1-encoder-repro/high12_444_alpha_9x9/encoded.avif \
  tests/fixtures/input/images/avif/high_bitdepth_still_12bit_444_alpha_9x9.avif
.oracle-venv/bin/python scripts/generate_decode_refs.py --format avif
```
The committed `high12_444_alpha_9x9` bundle retains the complete planes,
Pillow pixels, AV1 syntax, native traces, and deterministic replay under
`tests/fixtures/outputs/av1_encoder/high12_444_alpha_9x9/`.

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

`profile2_identity_12bit_444_9x9.avif` is a deterministic full-file mutation
of the 16x16 identity-CICP still above. The generator updates the AV1 sequence
dimensions and primary item's `ispe` spatial extent to 9x9 while preserving the
frame payload and all identity-color declarations. It checks the source and
output hashes, verifies the AV1 sequence fields, and checks Pillow 12.2.0
open, verify, and load behavior. Pillow returns 243 RGB8 bytes with SHA-256
`081589477332f1b32b6175028a983d88a15fc1b995b32abcfe4280cfcdb8d2d8`. The 81
color samples exercise ten complete eight-lane batches and a one-sample scalar
remainder in the identity RGB converter. Regenerate it with:

```sh
.oracle-venv/bin/python scripts/generate_avif_profile2_identity_tail.py
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
`coverage_monochrome_multitile_cdef_8bit_128x64.avif`
(`ea6c16c86c07414bfa3f71f1132af6494c4d7f6cb5015d5b98bfc94e09e5d73b`)
is a separate 128x64 monochrome key frame encoded as two 64-pixel tile
columns, with CDEF enabled and loop filtering and restoration disabled. The
deterministic Cmono Y4M source, pinned libaom bitstream, and Pillow RGB bytes
are checked by `scripts/generate_avif_monochrome_multitile_cdef.py`; the exact
24,576-byte RGB reference has SHA-256
`0d9fa687322831cf61ae6cd8b2cba87ee78bbb071641aada0f2879667bebd3b8`.
`coverage_color_multitile_superres_8bit_256x64.avif`
(`5a1c05d8f225ef1c73f73010089ab0d5ddf7e495620f117673321a248e71b044`)
is a full-range 8-bit 4:2:0 color key frame encoded with two tile columns.
Its coded width of 228 pixels is restored to 256 by denominator-9
super-resolution, exercising color-tile assembly followed by super-resolution.
The deterministic Y4M source, pinned libaom bitstream, and Pillow RGB bytes are
checked by `scripts/generate_avif_color_multitile_superres.py`; the exact
49,152-byte RGB reference has SHA-256
`42703be2059382a9c3dce70c4795ea5e085fcd87e30ca361d9bfc443213273f3`.
`coverage_monochrome_multitile_loop_cdef_8bit_128x64.avif`
(`f81ad82e033b3d36cd1c92ab97e296a1b4c3ea5308cd12c919e748a8304ce282`)
uses the same deterministic source and two-tile layout with nonzero luma
deblocking, CDEF syntax, and restoration disabled. The pinned encoder profile
and 24,576-byte Pillow RGB reference are checked by the same generator; the
reference SHA-256 is
`2b228a77b35dad05b440b250ee3eee71d858a8561dde92a71634e8cccfffbb57`.
`coverage_monochrome_multitile_loop_cdef_active_8bit_128x64.avif`
(`229af13b8ae6f1e69ce316e74c42d2d225d3ce479bececd45c5ff2059502d763`)
uses the same source and tile layout with active CDEF strength 44, luma
deblocking level 56, and no restoration. The deterministic generator validates
the pinned AV1 syntax and exact 24,576-byte Pillow RGB output, whose SHA-256 is
`04fe662bfb299b49c6f8117c02278c3b94a80c18f388ca2c5f7370732af460e5`.
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

`portable_lossless_monochrome_grid_73x64.avif` is generated by
`scripts/generate_avif_monochrome_grid_fixture.py`. The generator double-encodes
two deterministic 64x64 RGBA cells as lossless 4:0:0 AVIF with auxiliary alpha,
then places their AV1 item payloads into the pinned 2x1 BMFF layout from
`grid.avif`. The grid descriptor is `0000000100490040`: it exposes all 64 rows
of the first cell and the first 9 columns of the second on a 73x64 canvas. The
script verifies every rewritten item extent, then uses Pillow 12.2.0 to verify,
load, and compare the full RGBA output to the composed raw cell pixels. The
18,688-byte Pillow output SHA-256 is
`a553e8c265e29b0644101cff0da8cb54ac351ac4541b86d095ad960445cea987`.

`portable_lossless_monochrome_grid_32bit_dimensions_73x64.avif` uses the same
two encoded cells and canvas, but sets descriptor flag bit 0 and stores both
73 and 64 as 32-bit fields (`000100010000004900000040`). This exercises the
large-dimension descriptor form with ordinary image dimensions. The fixture
generator derives it from the pinned grid and verifies that its Pillow RGBA
pixels match the 16-bit-form fixture exactly.

The companion `portable_lossless_monochrome_grid_no_alpha_73x64.avif` uses
two deterministic 64x64 `L` cells in a clean 2x1 grid with no auxiliary-alpha
items, references, or properties. The pinned generator verifies the BMFF item
inventory and extents, then checks the exact 14,016-byte RGB result from
Pillow 12.2.0; its pixel SHA-256 is
`1b4a716a8f76555d37d08901f0582e195b2bdcedad46b00f781995d42e4cb1b6`.

`portable_lossless_monochrome_grid_bottom_clip_64x73.avif` reuses the
deterministic alpha-bearing 64x64 cells in a 1x2 grid with descriptor
`0000010000400049`. The canvas contains all 64 rows of the top cell and the
first nine rows of the bottom cell. The generator verifies each BMFF extent
and the exact 18,688-byte Pillow RGBA output, whose pixel SHA-256 is
`20b867857750340cae5d682bb6f5a162508395c242dfe331321cbff0bae90bb4`.

`portable_lossless_monochrome_1x1.avif` is generated as a one-pixel Pillow
`L` image with sample 127. The pinned AVIF encoder uses quality 100, speed 0,
one thread, 4:0:0 subsampling, CICP 1/13/6, and disabled warped motion. The
pure-Rust monochrome reconstruction crops the coded lossless block to this
nonzero sub-4-pixel canvas; Pillow's exact RGB output is `7f7f7f` (SHA-256
`4977021028c4a74c2bca8061786cd88afc96837ef6007265acc4f8cda2d945e0`). The
generator double-encodes the input and checks its listed hash.

`portable_lossless_monochrome_noise_64x64.avif` is generated from 4,096
deterministic luma samples produced by a fixed 32-bit xorshift stream. The
generator double-encodes with the pinned Pillow AVIF oracle, verifies exact
RGB pixels, and checks that the AV1 sequence is monochrome and all-lossless.

`portable_lossy_monochrome_noise_64x64.avif` uses the same deterministic luma
source at quality 70. Its generator pins the Pillow RGB output and verifies
that the AV1 sequence is monochrome, lossy, and CDEF-enabled.


The `portable_lossless_monochrome_fullrange_12bit_17x9.avif` and
`portable_lossless_monochrome_fullrange_alpha_12bit_17x9.avif` fixtures use
17x9 full-range 12-bit lossless monochrome items. Their color planes include
both endpoints and low-nibble values 0–15; the alpha fixture has a separate
varied full-range alpha item with the same sample coverage. The generator
checks repeatable encodes, AV1 profile/depth/range/lossless declarations, the
no-alpha fixture's exact 153-sample Y4M round-trip, and pinned Pillow RGB8 and
RGBA pixel hashes. It uses clean libavif `6543b22` and libaom `ad44980` builds,
without the fixture-only super-resolution patch. After preparing those pinned
builds, regenerate or verify both assets with:

```sh
.oracle-venv/bin/python scripts/generate_avif_monochrome_12bit_scalar_tail.py \
  --avifenc target/oracle-staging/libavif-kpzg-clean-build/avifenc \
  --avifdec target/oracle-staging/libavif-kpzg-clean-build/avifdec
.oracle-venv/bin/python scripts/generate_avif_monochrome_12bit_scalar_tail.py \
  --avifenc target/oracle-staging/libavif-kpzg-clean-build/avifenc \
  --avifdec target/oracle-staging/libavif-kpzg-clean-build/avifdec --check
```

`high_bitdepth_still_12bit_420_alpha_odd_edge_17x9.avif` uses a generated
16-bit RGBA source encoded as full-range 12-bit YUV420 with a 12-bit auxiliary
alpha item. The generator checks repeated byte-identical encodes, AV1 profile,
depth, range, subsampling and lossless frame declarations, decoded Y/U/V sample
depth and low-nibble variation along both odd chroma edges, and the pinned
Pillow RGBA8 hash. Build the pinned clean libavif tools above, then regenerate
or verify the fixture with:

```sh
.oracle-venv/bin/python scripts/generate_avif_12bit420_alpha_odd_edge.py \
  --avifenc target/oracle-staging/libavif-kpzg-clean-build/avifenc \
  --avifdec target/oracle-staging/libavif-kpzg-clean-build/avifdec
.oracle-venv/bin/python scripts/generate_avif_12bit420_alpha_odd_edge.py \
  --avifenc target/oracle-staging/libavif-kpzg-clean-build/avifenc \
  --avifdec target/oracle-staging/libavif-kpzg-clean-build/avifdec --check
```

Pillow's pinned libavif 1.4.1 uses automatic chroma upsampling for the RGBA
decode. Its 12-bit 4:2:0 dispatch selects the high-depth `I012ToARGBMatrix`
route, which samples nearest chroma and preserves luma low bits through the
matrix; libavif converts the auxiliary alpha plane separately with full-range
round-half-up scaling. The pinned libyuv source is commit
`6067afde563c3946eebd94f146b3824ab7a97a9c`. The generator's `avifdec` checks
encoded plane shape and edge variation only; the exact Pillow RGBA8 reference
is the pixel oracle.

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

`portable_lossless_filmgrain_i444_overlap_64x64.avif` changes only the
inspected frame-header overlap bit in that pinned source. The independent AV1
OBU inspector confirms the updated grain syntax and unchanged sample geometry;
the generator also checks that all 15,555 tile-payload bytes are identical.
The fixture remains 15,960 bytes (SHA-256
`345a3e16a1d9574a6ac39c4b14eaa93ba70b5d49714cae2188fdff03c252e03a`). Its
12,288 Pillow RGB bytes have SHA-256
`46bfc877578f36fc8d3b376c3ac41e6be9ff98bb14fda447a0b0baeeba0b97d7`.
Because the image spans four 32×32 grain blocks, decoding covers top-only,
left-only, and top-left overlap blending.

`portable_lossless_filmgrain_i444_restricted_clip_64x64.avif` sets only the
final inspected frame-header flag, enabling restricted-range clipping during
grain synthesis. The generator verifies that the parsed grain parameters
differ only in this flag, the sample extents are unchanged, and all 15,555
tile-payload bytes match the pinned source. The 15,960-byte fixture has SHA-256
`a60b2f7ce2b1a2891d6627a5f568fe5ba5dde0ea96ff348d3271e4893a633fcb`; its
12,288 Pillow RGB bytes have SHA-256
`fb80a8679a38c9a13044b2b67b92e01d75761d4436b149b459eb55f35cf9e476`.

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

`portable_lossless_filmgrain_420_chroma_from_luma_65x65.avif` uses the same
seeded, lossless I420 film-grain recipe at odd 65x65 dimensions. Its 33x33
chroma planes leave a one-pixel final grain block. Chroma scaling reaches the
last luma column and row, with the right interpolation neighbor clamped at the
image edge. The deterministic
generator preserves the AV1 tile payload, checks the syntax with the OBU
inspector, and verifies repeatable Pillow 12.2.0 RGB output. The 8,189-byte
fixture has SHA-256
`5f881e4c6eed5bb4c554004d72ca5e992f6b704bc39672b01d8b3a3236bb2330`; its
12,675 Pillow RGB bytes have SHA-256
`d3c620ef7742e8702d1351c33b4ae9386a67fa1a761b6c4b9ea695a6fe08f0c9`.

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

`colr_unknown_type_ignored.avif` changes only the primary item's `colr` type to
`abcd`. Pillow ignores this unknown color-property type and returns the same
RGB bytes as `baseline.avif`. `error_colr_reserved_flag_bits.avif` sets one of
the reserved `nclx` flag bits; Pillow rejects that file during identification.
`error_colr_empty_icc_profile.avif` removes the ICC payload from `prof` and
moves same-sized padding into an unassociated `free` property so box bounds and
file offsets stay fixed; Pillow rejects the empty profile at open.
`colr_nclx_trailing_payload_ignored.avif` moves a bounded trailing zero byte
from `clap` to `nclx` without changing the enclosing box sizes. Pillow decodes
the same RGB pixels; the pinned libavif parser reads the required `nclx` fields
and ignores the bounded tail.
`clli_trailing_payload_ignored.avif` appends one zero byte to the primary CLLI
property in `hdr.avif`, grows its enclosing metadata boxes, and shifts all five
file-backed `iloc` extents by one byte. The pinned Pillow oracle preserves the
complete HDR image information and exact decoded RGB bytes.
`error_ipco_property_size_below_header.avif` changes only the first `ispe`
box's 32-bit size field from 20 to 4, below its 8-byte header. The complete
file length and `mdat` payload remain unchanged; Pillow rejects the file at
open.

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

`duplicate_mdcv_first_wins.avif` appends a second, conflicting non-essential
`mdcv` property and associates it with the primary item. Its generator rebases
the file-backed item extent and confirms Pillow returns the same pixels and ICC
profile as the one-property control. Pillow does not expose `mdcv` in
`Image.info`; the parity row therefore checks the retained value against the
same specification-backed fields as the control. Rust follows the existing
first-associated-property policy used for AVIF color metadata.

`empty_xmp_metadata_item_extent.avif` adds an XMP MIME item with no `iloc`
extents to `baseline.avif` and rebases its file-backed primary item. Pillow
ignores the empty item and returns the same decoded pixels and image
information. The Rust decoder drops metadata items that declare no extents.

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
| `portable_lossless_420_gray_step_split_16x8.avif` | grayscale step at x=8, values 64 and 128 | `387d206379f144e7904d4aaa2636c7b37e084109f5600b852734d5af003bbd3b` |
| `portable_lossless_420_gray_texture_8x8.avif` | deterministic grayscale luma texture with neutral chroma | `4d4caeb830d6775b0c82172b60cacae07b0945a419cecb44680394f4337eef65` |
| `portable_lossless_420_color_texture_8x8.avif` | 2x2 cells cycling red, green, blue, and yellow | `eba2565967dbab65b88d58c6855ee920d06f69ef39ed610306cff3e322beb0a2` |
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

`portable_lossless_420_gray_step_split_16x8.avif` is generated by
`scripts/generate_avif_lossless_420_gray_step_split.py`. Its two grayscale
levels meet at the 8-pixel block boundary while its 4:2:0 chroma remains
neutral. The generator checks deterministic bytes and exact Pillow RGB
reconstruction.

`portable_lossless_420_gray_texture_8x8.avif` is generated by
`scripts/generate_avif_lossless_420_gray_texture_8x8.py`. It encodes a
deterministic full-range 8-bit I420 image with nonzero luma residuals and
neutral chroma. The generator checks repeated byte-identical encodes, the AV1
lossless/I420 headers, and exact Pillow RGB reconstruction.

`portable_lossless_420_color_texture_8x8.avif` is generated by
`scripts/generate_avif_lossless_420_color_texture_8x8.py`. Its 2x2 source cells
cycle through red, green, blue, and yellow, producing non-neutral 4:2:0 color.
The generator checks repeated byte-identical encodes, the full-range 8-bit
lossless I420 headers, and exact Pillow RGB reconstruction.

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

The `coverage_h16x8_following_chroma_dct_dct_01.avif` fixture promotes
candidate F03/N09 (seed 328209) from the input-only search in
`scripts/explore_avif_horizontal16x8_following.py`. The focused generator
`scripts/generate_test_assets.py --avif-h16x8-following-chroma` uses Pillow
12.2.0, libavif 1.4.1, dav1d 1.5.3, and AOM 3.13.2 with quality 99, speed 0,
one thread, and 4:2:0 subsampling. The 32x8 frame has AV1 base qindex 4 and a
following Horizontal16x8 pair: both leaves use unsplit TX16x8 DCT_DCT luma;
the left U/V transforms are skipped, while the right U/V TX8x4 DCT_DCT
transforms are coded with EOB values 25 and 30. Its exact Pillow RGB output is
stored in the decode reference, and the fixture SHA-256 is
`09f28c50a75e29b87a7862aa3d74431707622b9bd5c597e19b05aa20df941f85`; the RGB
SHA-256 is
`58932efed7fe326d1de67786f8d595ec317ee217ed23a107b670a74fe1dd6892`.

## AVIF sample timing mutations

The `animated_stts_*` cases are deterministic mutations of the pinned
`animated.avif` source (`2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d`).
`scripts/avif_sample_table_fixtures.py` pins every output hash and preserves the
encoded media payloads. The zero-delta, empty-table, zero-count, under-filled,
and over-filled variants keep the complete 1,235-byte file; the empty table
uses a size-16 `stts` box and an 8-byte `free` sibling. Pillow 12.2.0 and pinned
libavif 1.4.1 decode five frames with unchanged pixels and 1/30-second timing
for the empty table, zero-count, under-filled, and over-filled cases. Pillow
12.2.0 also accepts the trailing-payload case. A nonzero `stts` FullBox version
is rejected by Pillow during identification.

The wrapped-count case changes the original `stts` run to `[1, 1]` and adds
`[u32::MAX, 2]`. Its 1,243-byte file adjusts enclosing box sizes and the
`stco`/`iloc` offsets while preserving the original media payload. The
libavif-compatible cumulative count wraps, giving frame zero a 1/30-second
duration and the remaining four frames 2/30 seconds. Pillow reports rounded
durations of 33 ms and 67 ms; all five RGB pixel hashes match the source.

The trailing-payload case appends four zero bytes to `stts`, grows its enclosing
boxes, and adjusts `stco` and `iloc` offsets. Pillow ignores those bounded bytes
and returns the same five RGB frames and 33 ms frame durations.

- `animated_stts_zero_delta.avif`: `5b2c438346c66b7ee1cb165742e639bc98e3180d5da42dd8e479f48b99598105`
- `animated_stts_empty.avif`: `c23b9d465406ba51f46d5fd6c3cf8d7a71dde68110bb0ccc7512d48977208b30`
- `animated_stts_zero_sample_count.avif`: `2e7fb956b121ced30abf59cc70a30fd7dc2dd8146484252883553651a86107d6`
- `animated_stts_underfilled.avif`: `0c8b7e231c455711581f0eed1272f2be2b32a764fc9eb782c955678ba64e9738`
- `animated_stts_overfilled.avif`: `fecedb638aaad110abfa0234edad9850ffaee9452b1633703b2426653d6e0bba`
- `animated_stts_wrapped_sample_count.avif`: `877d9aa62c8d5ad750d6f4c60a293a2025fbe6ade96f4b8957301a73c37a4e1b`
- `animated_stts_nonzero_version.avif`: `7723b92c39ed4477a37b37245f0f4b828ebd68543f89cc11edb14d4716fdb123`
- `animated_stts_trailing_payload.avif`: `315886b7d98357707dac59164052be3b44b735c25c0ccdbe0b35c4a87d8289c4`

## AVIF item-information child-kind error

`iinf_child_not_infe.avif` is generated by
`scripts/generate_avif_iinf_child_kind_fixture.py` from `baseline.avif`
(`d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2`). The
generator validates the version-zero `iinf` entry count and sole `infe` child,
then changes only that child's four-byte type from `infe` to `free` without
changing the 3,077-byte file. It pins the output SHA-256 to
`ee95c49866ddcf2bf1fd4f2f3a0e4c1d2c768bb5ccbf8f675555691298225d76` and
checks that Pillow 12.2.0 with libavif 1.4.1 rejects the complete file during
open with `cannot identify image file <bytes>`. Run the generator under the
pinned oracle environment to reproduce and check that rejection. Its `--check`
mode verifies the exact tracked mutation and hash without requiring Pillow.

## AVIF item-location trailing payload

`iloc_trailing_payload_ignored.avif` is generated from `baseline.avif`
(`d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2`) by
`scripts/generate_avif_iloc_trailing_payload_fixture.py`. The generator appends
one `0xff` byte after the declared version-zero `iloc` record, increases the
`iloc` and enclosing `meta` box sizes, and advances the file-backed extent by
one byte because `mdat` moves. It pins the fixture SHA-256 to
`d62c4cf7da0e986cf90225f63160fb4ff8bd851cf0d2a0c2957bc48ef7ce4504` and
requires Pillow 12.2.0 with libavif 1.4.1 to verify and decode the same image
information and exact RGB pixels as the baseline. The generator's `--check`
mode validates the tracked bytes without requiring Pillow.

## Lossless 10-bit I420 B128 sequence

`animated_lossless_inter_420_b128x128_10bit.avif` contains two full-range
128x128 I420 frames encoded losslessly with 128x128 superblocks. An 8x8 luma
patch moves by one pixel between frames. Its generator checks repeatable
timestamp-normalized encoding, AV1 key/inter headers, and exact Pillow 12.2.0
RGB pixels. The asset SHA-256 is
`a1056a8b40154eab74c1571cce2c39832e18a37de4b8bd1a3f808c6df231e851`.

## Lossless 8-bit I420 B128 partition witnesses

`animated_lossless_inter_420_wide_grid_b128x128.avif` is generated by
`scripts/generate_avif_lossless_wide_inter_grid_fixture.py`. It covers the wide
inter-grid path with two all-lossless frames and verifies the 128x128
superblock, AV1 key/inter headers, and exact Pillow 12.2.0 RGB frames. Its
SHA-256 is
`e44c5a53a1cf185a29b4a6570836c82078d1cd2e1d0d0963e69ebf1028f7d47f`.

`animated_lossless_inter_420_vertical_b64x128.avif` uses a fixture-only
libaom partition override to split the 128x128 inter root vertically into two
valid B64x128 leaves. This covers the tall-block path independently of the
wide-grid case. Its generator checks repeatable timestamp-normalized encodes,
all-lossless key/inter headers, and exact Pillow 12.2.0 RGB frames. The asset
SHA-256 is
`9ed8dd60feedcb8480bb8d6de7f8ea2ec23583da923e916e28a128c3746ca650`.

`animated_lossy_compound_wide_i420_b128x128.avif` and
`animated_lossy_compound_wide_i420_unsplit_b128x128.avif` use two seeded
full-range reference frames. The first final frame is their average with one
colored residual patch in each 128x128 region; the second is their exact pixel
average. Pillow's pinned AOM encoder emits three-frame 8-bit I420 sequences
with 128x128 superblocks and partitions. The generator double-encodes both
variants, checks AV1 headers and exact Pillow RGB frame hashes, and pins their
normalized file hashes to
`d0491ed004f7e6d722ce55b7cbeaa6db1898073a9868b306ef412f89693b7f71` and
`8fed3a09dd26a5baf9aa52918f479ee8f83ebfe49642f8b1c021a86fca9abe35`.

`animated_lossy_wide_i422_b128x64.avif` is generated by `scripts/generate_avif_lossy_wide_i422_b128x64.py` with the fixture-only libaom partition patch documented in `scripts/avif_fixture_oracle/README.md`. The inter frame's I422 128x128 root uses `PARTITION_HORZ`, producing two 128x64 leaves; independent AV1 OBU parsing confirms `transform_mode=largest`, while the pinned dav1d block trace confirms the two I422 leaves. The generator repeats the encode and verifies the normalized file hash and both exact Pillow RGB frames. This is a valid wide-I422 parity case; AV1 does not define the invalid B64x128 chroma plane.

## Distance-weighted AV1 compound ratio edge

`animated_lossy_interinter_distance_ratio_edge_i420_b16x16_64x64.avif` is
generated by `scripts/generate_avif_interinter_distance_ratio_edge.py`. Its
final shown frame has order hint 5 and selectable references at hints 3 and 6,
with absolute temporal distances 2 and 1. The AOM sequence enables
distance-weighted compound prediction, and the generator pins the normalized
AVIF bytes and all six Pillow 12.2.0 RGB frame hashes.

## 10-bit I420 intra Wiener restoration

`coverage_i420_intra_restoration_10bit_16x16.avif` and
`coverage_i422_intra_restoration_10bit_16x16.avif` are 16x16 still pictures
with one Wiener-restored luma plane, no CDEF, no quantization matrices, and
the largest transform mode. Their deterministic Y4M sources and pinned
libaom encodes are checked by `scripts/generate_avif_intra_filters.py`;
the generator also verifies each AV1 OBU profile and exact RGB bytes from
Pillow 12.2.0 with libavif 1.4.1. The I420 AVIF SHA-256 is
`c508ed4754d696d6b3240ca552dd61a1481c062c9537904183c4657b13bc5f7f`; the
I422 AVIF SHA-256 is
`2dfdaa9d17c34e5656e5fa458ef951abe28cec6452d17eef1ca7820022a6c19f`.

`coverage_i420_intra_cdef_10bit_16x16.avif` and
`coverage_i422_intra_cdef_10bit_16x16.avif` are 16x16 still pictures with a
single-entry CDEF strength set, no loop filtering or restoration, no
quantization matrices, and the largest transform mode. Their deterministic
Y4M sources and pinned libaom encodes are checked by the same generator. The
I420 AVIF SHA-256 is
`3407c458add1348d2e0f0a73f8aaaa862fd435a18eb7c563b8461d89541656d2`; the
I422 AVIF SHA-256 is
`40e0090f85088bdef2e7cf08910002dd2dc48e95637c976173e71a9326a22352`.

`coverage_i420_intra_cdef_10bit_9x9.avif` is a 9x9 full-range 10-bit I420
key frame with active single-entry CDEF. It is an active parity case for
partial CDEF blocks at both the right and bottom frame edges. Its reconstructed
16x16 coded canvas is retained through CDEF before the final 9x9 crop. The
deterministic Y4M source and pinned libaom encode are checked by
`scripts/generate_avif_intra_filters.py`;
RGB bytes come from Pillow 12.2.0 with libavif 1.4.1. The AVIF SHA-256 is
`11c149dcf954268012c565a586cb1a64af8c86b6eb5cd1463ec6ff6567ba4917`, and the
RGB reference SHA-256 is
`cb5a5b6e5760a49068aa7d3863634600c82c5fa6500295082d524633d959efd1`.

`coverage_i420_intra_cdef_10bit_9x16.avif` is a 9x16 full-range 10-bit I420
key frame with active single-entry CDEF. It covers a partial right CDEF edge
while the bottom edge is aligned, complementing the 9x9 case where both axes
are partial. The deterministic Y4M source, repeated pinned libaom encodes,
AV1 frame header, and Pillow 12.2.0/libavif 1.4.1 RGB bytes are checked by
`scripts/generate_avif_intra_filters.py`. The AVIF SHA-256 is
`9349dd957c42e2f39e8863893e1e86eb996bd112611f5543b2c4fa3e28c312fc`, and the
RGB reference SHA-256 is
`c99bfa3e07e5637aaf1b41db938b495bc8c43067c674be61753957f12d12e119`.

`coverage_i420_intra_cdef_10bit_15x16.avif` is a 15x16 full-range 10-bit
I420 key frame with active single-entry CDEF. The luma plane has a partial
right CDEF edge, while each visible 8-sample chroma row ends on a 4-sample
chroma CDEF boundary. This exercises edge sampling that must stop at the
visible chroma extent even though luma extends its partial edge. The
deterministic Y4M source, repeated pinned libaom encodes, AV1 frame header,
and Pillow 12.2.0/libavif 1.4.1 RGB bytes are checked by
`scripts/generate_avif_intra_filters.py`. The AVIF SHA-256 is
`43fef444f1e0144c74f60752ea255ac2d3f36d594c8172dbf09b57a6c5497b96`, and the
RGB reference SHA-256 is
`53e32b2c9de70cf7f2bf975e136c0e4f9517dbe73b755010de3b839c1ba799b3`.

`coverage_i420_intra_cdef_10bit_low_variance_16x16.avif` is a 16x16
full-range 10-bit I420 key frame with active single-entry CDEF and low but
nonzero luma direction variance. The decoded luma exercises the low-variance
primary-strength adjustment while neutral chroma keeps the input deterministic.
The fixture generator verifies the sequence and frame syntax, repeats the
pinned libaom encode, and checks the Pillow 12.2.0/libavif 1.4.1 RGB reference.
The AVIF SHA-256 is
`ce827c0c80e25f416bc5c03b9a577dd2cf71d2ab1a08715e7f7891a462210056`, and the
RGB reference SHA-256 is
`1f975d71bfac9a6becec7fb48dd1713abb6d24be0758bdbb53d61aaac09286fe`.

## Film-grain depth and inactive-plane inputs

The 34 `filmgrain_*_64x64.avif` inputs in
`scripts/generate_avif_filmgrain_depth_fixtures.py` cover full-range 10/12-bit
monochrome, I420, I422 and I444 displays, overlap with restricted-range
clipping, and 8/10/12-bit U-only, V-only and zero-active-point controls.
Deterministic planar Y4M sources include saturated endpoints and fractional
scaling indices. Repeated libavif 1.4.1/libaom 3.13.2 encodes supply the
lossless tiles; checked header rewrites preserve all tile bytes and sample
extents. Exact RGB references come independently from pinned Pillow 12.2.0
with dav1d 1.5.3.

Regenerate with the pinned native encoder:

```bash
python3 scripts/generate_avif_filmgrain_depth_fixtures.py --encoder target/oracle-staging/libavif-superres-build/avifenc
python3 scripts/generate_decode_refs.py --format avif
```

Use the pinned oracle Python environment for these commands. The generator's
`--check` mode verifies retained input hashes, AV1 headers and live Pillow
display metadata without invoking the encoder or rewriting fixture files.

## 10-bit I420 intra CDEF and striped restoration

`coverage_i420_intra_cdef_restoration_10bit_16x64.avif` combines active luma
CDEF with Wiener restoration across the first 56-row stripe boundary. CDEF
strengths are 47 for Y and 0 for U/V, with damping 5. Deblocking and
super-resolution are disabled. The deterministic Y4M source and repeated
libavif 1.4.1/libaom 3.13.2 encodes are checked by
`scripts/generate_avif_intra_filters.py --variant i420_cdef_restoration_striped`.
The AVIF SHA-256 is
`ccf7cb2dc6c81749955e09853587f7b1047e3c89b17f2832a4196303d8302727`;
the 3,072-byte Pillow 12.2.0 RGB reference SHA-256 is
`a6535b59e9da0eb66ee1bf768e080739bc8d88b2afc9554388b7620849900f1e`.

## 10-bit I420 intra CDEF and restoration

`coverage_i420_intra_cdef_restoration_10bit_16x16.avif` combines a single-entry
CDEF strength set with luma SGR restoration on a 16x16 10-bit I420 key frame.
The fixture uses the dedicated fixture-only libaom picker patch documented in
`scripts/avif_fixture_oracle/README.md`; the decoded pixel reference comes from
pinned Pillow 12.2.0/libavif 1.4.1. The AVIF SHA-256 is
`faff193dc1d3c62835490e949ed83314578abea8eb14357dffe361044914bb92`, and the
RGB reference SHA-256 is
`4d3f43960d12c77079790677f805b6deec2470a7c4deee5b498948d2f434dd30`.

## 10-bit I420 loop filtering with CDEF and restoration

`coverage_i420_intra_loopfilter_cdef_restoration_10bit_16x16.avif` uses
nonzero luma and chroma deblocking levels together with a single-entry CDEF
strength set and luma SGR restoration. It is a 16x16 full-range 10-bit I420
key frame; the loop-filter levels are 1 for both luma directions and for U/V.
The fixture-only libaom override and reproducible encoder build are documented
in `scripts/avif_fixture_oracle/README.md`. The generator checks the exact AV1
profile and repeat-encode identity. Its AVIF SHA-256 is
`8901fba07e2d68e35baa03bd521d22048e7d2ff2bd969a02b195541f5958df54`; the
768-byte Pillow 12.2.0 RGB reference SHA-256 is
`4d3f43960d12c77079790677f805b6deec2470a7c4deee5b498948d2f434dd30`.

## Short reference signaling and wrapped order hints

`short_refs_q{100,99}_*.avif` contains 24 complete 64x64 I444 sequences,
with two or three frames at 100 ms per frame. Quality 100 supplies lossless
controls; quality 99 supplies lossy controls. The variants cover aliased and
distinct LAST/GOLDEN slots, tied hints, both sides of the seven-bit half-range
boundary, and wraparound. Three-frame variants retain different key/inter
images, including reordered past and future hints.

`scripts/generate_avif_short_reference_fixtures.py` encodes each source twice
using pinned Pillow 12.2.0, libavif 1.4.1 and libaom 3.13.2. It replaces the
final frame's explicit reference indices with compact short signaling and
checks every encoded tile byte and sample extent. Legal padding OBUs or an
extended size LEB128 retain following physical offsets. Input digests are
pinned independently of expected pixels. Its `--check` mode reads retained
inputs without encoding or rewriting them. Exact public outputs come from
the normal live-Pillow reference workflow; repeated native libavif loop
observations are recorded separately in `outputs/avif_loops/index.json`.

## 10-bit I422 intra CDEF and restoration

`coverage_i422_intra_cdef_restoration_10bit_16x16.avif` combines a
single-entry CDEF strength set with Wiener luma restoration on a 16x16 10-bit
I422 key frame. The pinned libaom encoder selects both filters without a
fixture-only patch; the generator checks the AV1 syntax and exact Pillow
12.2.0/libavif 1.4.1 RGB bytes. The AVIF SHA-256 is
`aa368cf810ef5d48ce300f3582f9574744a1929beeb5525497675176b6b8beac`, and the
RGB reference SHA-256 is
`afb7a71a8c86bf53df63a3735752e299ced6712e8b718cf4725fb3a2b61eaec6`.

## Wide color OBMC regression inputs

`generate_avif_wide_mode2_fixtures.py` reproduces 30 complete 256x256 sequences:
8-bit gray/colored I420 and I422, and fractional-sample 10/12-bit I420, I422
and I444. Each layout uses sinusoid, increasing-checker and localized-change
sources. Repeated pinned Pillow 12.2.0 or libavif 1.4.1/libaom 3.13.2 encodes
must reproduce the complete-input digest. `--check` only reads retained files.
Construction never reads expected pixels or executes Rust. The ordinary live
Pillow workflow records exact frames; separate native observations record loops.

The three `12bit_mode2_deep16` inputs regress parsed OBMC being discarded by
wide color reconstruction. Presented frame 3 differed before the fix. Nearby
8/10-bit and other-source inputs preserve exact pixels across all frames.
Recipe names describe encoder stimuli; actual per-frame transform choices,
hidden frames and show-existing pictures are independently inspected.

## Monochrome multi-superblock and partial-edge inputs

`generate_avif_monochrome_wide_fixtures.py` reproduces 21 complete native-encoded
8/10/12-bit monochrome sequences. Nine 256x256 inputs use sinusoid, checker and
localized-change sources; twelve more cover 132x128, 128x132, 252x256 and
256x252 frames with rounded coded edges. All six earlier high-depth 256x256
inputs are reused byte for byte. Source Y4M and complete-input hashes are pinned;
every native encode repeats. Construction reads no expected outputs or Rust.

The single-tile neutral-filter profile now uses the existing checked walker
and canvas through 256 pixels per frame axis. Grain and active post-filter or
independently decoded tile profiles retain their previous admission limits.
Live Pillow supplies exact public frames; separate native observations supply
loop counts. `--check` reads retained inputs without encoding or rewriting.

## Horizontal B8x4 qindex-zero non-lossless 4:2:2 sequence

`animated_lossy_inter_422_b8x4_qindex0_mode_select_64x64.avif` is a 64x64
8-bit I422 key/inter sequence. The pinned fixture-only AOM patches create a
non-lossless qindex-zero inter frame and retain a horizontal B8x4 leaf at its
top-left B8x8 root. The encoder uses frozen CDF updates. The generator repeats
the encoder, normalizes timestamps,
and validates the AV1 headers plus both Pillow 12.2.0 RGB frames. AVIF
SHA-256: `5c2fd79d69b2fb1a96542c30ea4eb6d103993982f9f322b1e64d040730f946f9`.
The 100 ms frame hashes are
`246f2904b49d857c90295a1a379d1eadea4058a6e04c8df8639aac4392732d50` and
`2daef02f6cde1d7914f04e930bf65f36a0bc74273f1a1029a5ec054434611b14`.
The reproducible patch order and producer command are documented in
`scripts/avif_fixture_oracle/README.md`.

## Vertical B4x8 qindex-zero non-lossless 4:2:0 sequence

`animated_lossy_inter_420_b4x8_qindex0_mode_select_64x64.avif` is produced
with the fixture-only vertical B4x8 and qindex-zero AOM patches documented in
`scripts/avif_fixture_oracle/README.md`. The inter frame has base qindex zero,
zero luma DC delta, and chroma DC/AC deltas of two; the key frame is
all-lossless. The native producer is repeat-encoded and the generator pins the
normalized complete-file digest. SHA-256:
`bfc6c0756fcf2bceda38f6830b89c2e72b55d067e5b3816bd2661007d2fa41ee`.
Pillow 12.2.0 decodes two 64x64 RGB frames with 100 ms durations; their hashes
are `2279a056bd14eb2e81ea15baa98bef134aa0d0cfc5ae960ffb142c40b4960ea4` and
`ef83c285e5d4ddf2f3ad70cc98493568b6ecc5da664cffbc2b416ee2bfc0432f`.

## Odd-width 10-bit lossless inter stimulus

`animated_lossless_inter_420_clipped_b32x32_10bit_partition32_31x32_moving_1x1_patch.avif`
is a 31x32 full-range 10-bit I420 key/inter sequence whose one-pixel texture
patch moves diagonally from (28, 8) to (29, 9) at the clipped right edge. The
pinned fixture generator repeats the encode, normalizes timestamps, and checks
both exact Pillow frames and the all-lossless AV1 headers. The AVIF SHA-256 is
`a755bb3e7d9e6209a5c5760099bff623a0f3be0c19ce008569da75522503fe4c`; the
frame hashes are `43bd6195fe30acdd0d3c1f8454d02be3e9af547725ef0caa69b627c6cd6a7f8b`
and `58307f1a4682252aae8245b38d0128f0f4d804102078534daf75aea595241481`.

## Clipped-height lossy B16x8 partitions

`animated_lossy_inter_420_b16x8_clipped_height_16x15_q99.avif` is a two-frame
16x15 8-bit 4:2:0 sequence. Its fixture-only pinned libaom patch selects a
horizontal partition into two B16x8 leaves in a 16x16 root; the lower leaf has
seven visible rows after clipping to the coded image height. The `clipped`
producer pattern adds deterministic second-frame detail below that edge. The
AVIF SHA-256 is
`6aa8245b99fa1c8a8eafe0d9ee5f7c400d68a3fa49413cfb01f3d650fde287e4`. Pillow
12.2.0 decodes two 16x15 RGB frames at 100 ms; their hashes are
`e8da116bc8cef2ec21428242b17d42b133751047040d1b937e953d69965032a1` and
`5a2df0a8dd5460d572056952713e10c15db63565afadd68ae10fc97f62c67b9f`.

The quality-70 companion,
`animated_lossy_inter_420_b16x8_clipped_height_16x15_q70.avif`, uses the same
geometry with stronger quantization. Its AVIF SHA-256 is
`58a8248ca1e753cda32b55646e9121a2245907b5e495726596122582bcbaea94`; its
100 ms RGB frame hashes are
`65e1e811ae7e6f725a0b0f5cf344122ec312b5314584260d8dca19e40eeee1e1` and
`0adf0daf3c1b8389ded15f5ea8c4e5a5586b5d6b552f846238be436bc2e9727f`.
