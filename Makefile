PYTHON ?= python3
PACKAGE_NAME := image-slash-star
PACKAGE_VERSION = $(shell cargo metadata --locked --no-deps --format-version 1 | $(PYTHON) -c 'import json,sys; d=json.load(sys.stdin); print(next(p["version"] for p in d["packages"] if p["name"]=="image-slash-star"))')
RELEASE_DIR := target/release-artifacts
RELEASE_CRATE = $(RELEASE_DIR)/$(PACKAGE_NAME)-$(PACKAGE_VERSION).crate
REGISTRY_CRATE = $(RELEASE_DIR)/registry/$(PACKAGE_NAME)-$(PACKAGE_VERSION).crate
COVERAGE_TOOLCHAIN ?= nightly-2026-07-16
COVERAGE_REPORT ?= target/release-evidence/coverage.json
COVERAGE_TARGET_DIR ?= target/cargo-llvm-cov
COVERAGE_FULL_TARGET_DIR ?= $(COVERAGE_TARGET_DIR)/full
COVERAGE_SELECTED_TARGET_DIR ?= $(COVERAGE_TARGET_DIR)/selected
COVERAGE_CASES ?=
SELECTED_COVERAGE_REPORT ?=
COVERAGE_EXPORTER_PREFIX ?= $(CURDIR)/target/coverage-exporter/llvm22.1.8
COVERAGE_EXPORTER_SOURCE ?=
COVERAGE_EXPORTER_TARGET ?= coverage
# Quote optional launcher arguments for the recipe shell, including apostrophes.
coverage_exporter_quote = '$(subst ','"'"',$(1))'
# A literal line feed for rejecting recipe-breaking report names before the shell.
define coverage_exporter_newline


endef

.DEFAULT_GOAL := help
.NOTPARALLEL: ci release-verify

.PHONY: help
help:
	@printf "image-slash-star\n\n"
	@printf "  make build            Compile the locked default-feature crate\n"
	@printf "  make docs-setup       Install hash-locked documentation tools\n"
	@printf "  make docs-build       Build and validate the Pages site\n"
	@printf "  make docs-serve       Preview the site at localhost:8000\n"
	@printf "  make bench-setup      Build the pinned TurboJPEG benchmark oracle\n"
	@printf "  make bench            Run the complete JPEG comparison (BENCH_OUTPUT, BENCH_TARGET_DIR, BENCH_ROUNDS)\n"
	@printf "  make fmt              Check formatting\n"
	@printf "  make verify           Run generated, claim, roadmap, and legal checks\n"
	@printf "  make refresh-fixture-integrity Refresh current fixture receipts, then verify them\n"
	@printf "  make lint             Run formatting checks and strict Clippy for library and benchmark code\n"
	@printf "  make lint-x86-simd    Lint the x86-64 SSE2 baseline and AVX2 builds\n"
	@printf "  make test             Run docs, tests, target feature lanes, and WASI Clippy\n"
	@printf "  make supply-chain     Run cargo-deny\n"
	@printf "  make coverage         Require alpha floors: lines 59, branches 46, functions 52, regions 58 percent\n"
	@printf "  make coverage-selected COVERAGE_CASES='decode:webp:<id>' Run exact parity/fault cases with an isolated LLVM-Cov target\n"
	@printf "  make coverage-exporter-setup Build the optional pinned native exporter (macOS ARM64; Git/CMake/Ninja, network on first setup)\n"
	@printf "  make coverage-with-exporter Use the verified exporter; COVERAGE_EXPORTER_TARGET=coverage-complete retains strict 100%%\n"
	@printf "  make package-verify   Build and consume a reproducible package\n"
	@printf "  make ci               Run all local CI gates, including workflow validation\n"
	@printf "  make release-verify   Require a clean, complete release candidate\n"
	@printf "  make dependency-update Update one dependency in both lockfiles (DEPENDENCY, DEPENDENCY_VERSION)\n"
	@printf "  make release-lock-update Refresh both workspace lockfiles after a version bump\n"
	@printf "  make release-registry-verify Compare the candidate with its published archive\n"
	@printf "  make release-publish-request Frame the verified crate archive for registry upload\n"
	@printf "  make coverage-complete Require 100 percent coverage across all four metrics\n"

.PHONY: fmt
fmt:
	cargo fmt --all -- --check
	cargo fmt --manifest-path benchmarks/jpeg-production/rust/Cargo.toml -- --check

.PHONY: verify-fixture-integrity
verify-fixture-integrity:
	$(PYTHON) scripts/verify_claim_ledger.py
	$(PYTHON) scripts/generate_gif_rgb_token_checkpoint_fixture.py --check
	$(PYTHON) scripts/generate_gif_rgba_single_color_token_fixture.py --check
	$(PYTHON) scripts/generate_gif_rgba_transparent_black_fixture.py --check
	$(PYTHON) scripts/generate_webp_alpha_filter_no_win_fixture.py --check
	$(PYTHON) scripts/generate_webp_box_chain_max_match_fixture.py --check
	$(PYTHON) scripts/generate_webp_cost_manager_shifted_rows_fixture.py --check
	$(PYTHON) scripts/generate_webp_cost_manager_fragment_stream_fixture.py --check
	$(PYTHON) scripts/generate_webp_cost_manager_deep_fragment_fixture.py --check
	$(PYTHON) scripts/generate_webp_singleton_copy_group_fixture.py --check
	$(PYTHON) scripts/generate_webp_tiny_predictor_grid_fixture.py --check
	$(PYTHON) scripts/generate_webp_sampling_row_mismatch_fixture.py --check
	$(PYTHON) scripts/generate_webp_dimension_limit_fixture.py --check
	$(PYTHON) scripts/generate_webp_method_two_fixtures.py --check
	$(PYTHON) scripts/generate_webp_alpha_effort_fixtures.py --check
	$(PYTHON) scripts/generate_webp_crunch_fixtures.py --check
	$(PYTHON) scripts/generate_webp_bool_carry_noise_fixture.py --check
	$(PYTHON) scripts/generate_webp_sampling_copy_checkpoint_fixture.py --check
	$(PYTHON) scripts/generate_webp_palette_alpha_tail_fixture.py --check
	$(PYTHON) scripts/generate_webp_alpha_zero_palette_checkpoint_fixture.py --check
	$(PYTHON) scripts/generate_malformed_ledger.py --check
	$(PYTHON) scripts/generate_avif_sequence_still_brand.py --check
	$(PYTHON) scripts/generate_avif_colr_edge_fixtures.py --check
	$(PYTHON) scripts/generate_avif_frame_id_continuity.py --check
	$(PYTHON) scripts/generate_avif_frame_id_high_contiguous.py --check
	$(PYTHON) scripts/generate_avif_frame_id_stale_reference_invalidation.py --check
	$(PYTHON) scripts/generate_avif_frame_id_future_reference_invalidation.py --check
	$(PYTHON) scripts/generate_avif_frame_id_wrap_contiguous.py --check
	$(PYTHON) scripts/generate_avif_duplicate_mdcv_property.py --check
	$(PYTHON) scripts/generate_avif_duplicate_sample_description_colr.py --check
	$(PYTHON) scripts/generate_avif_empty_xmp_metadata_extent.py --check
	$(PYTHON) scripts/generate_avif_trailing_uuid_fixture.py --check
	$(PYTHON) scripts/generate_avif_unknown_top_level_box.py --check
	$(PYTHON) scripts/generate_avif_trailing_box_size_below_header.py --check
	$(PYTHON) scripts/generate_avif_clli_trailing_payload_fixture.py --check
	$(PYTHON) scripts/generate_avif_ipco_property_size_below_header.py --check
	$(PYTHON) scripts/generate_avif_lossless_420_gray_step_split.py --check
	$(PYTHON) scripts/generate_avif_lossless_monochrome_clipped_wide_inter_fixture.py --check
	$(PYTHON) scripts/generate_avif_8bit_i422_superres_restoration.py --check
	$(PYTHON) scripts/generate_avif_8bit_i444_superres_restoration.py --check
	$(PYTHON) scripts/generate_avif_monochrome_superres_sequence.py --check
	$(PYTHON) scripts/generate_avif_order_hint_disabled_fixture.py --check
	$(PYTHON) scripts/generate_avif_sequence_screen_tools.py --check
	$(PYTHON) scripts/generate_avif_frame_header_edge_fixtures.py --check
	$(PYTHON) scripts/generate_avif_transfer_characteristics_bt709.py --check
	$(PYTHON) scripts/generate_avif_identity_matrix_profile1_444.py --check
	$(PYTHON) scripts/generate_avif_identity_matrix_profile2_12bit_444.py --check
	$(PYTHON) scripts/generate_avif_profile0_identity_matrix_error.py --check
	$(PYTHON) scripts/generate_avif_monochrome_palette_fixture.py --check
	$(PYTHON) scripts/generate_avif_lossless_monochrome_superres_small.py --check
	$(PYTHON) scripts/generate_avif_nonuniform_tile_fixture.py --check
	$(PYTHON) scripts/generate_avif_nonuniform_tile_limit_error.py --check
	$(PYTHON) scripts/generate_avif_lossless_monochrome_multitile_superres.py --check
	$(PYTHON) scripts/generate_avif_lossless_monochrome_multitile_superres.py --profile narrow-edge-tile --check
	$(PYTHON) scripts/generate_avif_lossless_monochrome_multitile_superres.py --profile above-right-context --check
	$(PYTHON) scripts/generate_avif_lossless_monochrome_multitile_superres.py --profile adaptive-above-right-128x128 --check
	$(PYTHON) scripts/generate_avif_lossless_monochrome_multitile_superres.py --profile padded-right-context-1x128 --check
	$(PYTHON) scripts/generate_avif_intra_filters.py --variant i420 --check
	$(PYTHON) scripts/generate_avif_intra_filters.py --variant i422 --check
	$(PYTHON) scripts/generate_avif_intra_filters.py --variant i422_loopfilter_restoration --check
	$(PYTHON) scripts/generate_avif_intra_filters.py --variant i422_loopfilter_no_restoration --check
	$(PYTHON) scripts/generate_avif_intra_filters.py --variant i420_cdef --check
	$(PYTHON) scripts/generate_avif_intra_filters.py --variant i420_cdef_low_variance --check
	$(PYTHON) scripts/generate_avif_intra_filters.py --variant i420_cdef_odd_edge --check
	$(PYTHON) scripts/generate_avif_intra_filters.py --variant i420_cdef_odd_width --check
	$(PYTHON) scripts/generate_avif_intra_filters.py --variant i420_cdef_chroma_aligned_right_edge --check
	$(PYTHON) scripts/generate_avif_intra_filters.py --variant i420_cdef_restoration --check
	$(PYTHON) scripts/generate_avif_intra_filters.py --variant i420_cdef_restoration_striped --check
	$(PYTHON) scripts/generate_avif_intra_filters.py --variant i422_cdef --check
	$(PYTHON) scripts/generate_avif_intra_filters.py --variant i422_cdef_restoration --check
	$(PYTHON) scripts/generate_avif_inter_422_cdef_fixture.py --check
	$(PYTHON) scripts/generate_avif_monochrome_multitile_cdef.py --variant cdef --check
	$(PYTHON) scripts/generate_avif_color_multitile_superres.py --check
	$(PYTHON) scripts/generate_avif_monochrome_multitile_cdef.py --variant loop_cdef --check
	$(PYTHON) scripts/generate_avif_monochrome_multitile_cdef.py --variant loop_cdef_active --check
	$(PYTHON) scripts/generate_avif_12bit_i422_superres.py --check
	$(PYTHON) scripts/generate_avif_12bit420_alpha_ispe_mismatch.py --check
	$(PYTHON) scripts/generate_avif_identity_matrix_subsampled.py --check
	$(PYTHON) scripts/generate_avif_sequence_cicp_mismatch.py --check
	$(PYTHON) scripts/generate_avif_portable_color_odd_fixture.py --check
	$(PYTHON) scripts/generate_avif_inter_444_direct_grid_mode_select_fixture.py --check
	$(PYTHON) scripts/generate_avif_interinter_wedge_i420.py --variant i420 --check
	$(PYTHON) scripts/generate_avif_interinter_wedge_i420.py --variant i422 --check
	$(PYTHON) scripts/generate_avif_interinter_wedge_i420.py --variant i444 --check
	$(PYTHON) scripts/generate_avif_interintra_oblique_wedge_no_flip_fixture.py --check
	$(PYTHON) scripts/generate_avif_interinter_distance_ratio_edge.py --check
	$(PYTHON) scripts/generate_avif_lossless_420_gray_texture_8x8.py --check
	$(PYTHON) scripts/generate_avif_lossless_420_color_texture_8x8.py --check
	$(PYTHON) scripts/generate_avif_lossless_inter_b4x4_fixture.py --check
	$(PYTHON) scripts/verify_avif_loop_evidence.py
	$(PYTHON) scripts/generate_avif_short_reference_fixtures.py --check
	$(PYTHON) scripts/generate_avif_wide_mode2_fixtures.py --check
	$(PYTHON) scripts/generate_avif_monochrome_wide_fixtures.py --check
	$(PYTHON) scripts/generate_avif_narrow_inter_fixtures.py --check
	$(PYTHON) scripts/generate_avif_square64_neighbor_fixtures.py --check
	$(PYTHON) scripts/generate_avif_mixed_superres_fixtures.py --check
	$(PYTHON) scripts/generate_avif_loopfilter_sharpness.py --check
	$(PYTHON) scripts/generate_avif_monochrome_superres_fixtures.py --check
	$(PYTHON) scripts/generate_avif_i422_neutral_superres_fixtures.py --check
	$(PYTHON) scripts/generate_avif_monochrome_mixed_key_fixtures.py --check
	$(PYTHON) scripts/generate_avif_color_mixed_key_fixtures.py --check
	$(PYTHON) scripts/verify_coverage_origins.py
	$(PYTHON) scripts/verify_webp_vp8l_property_map.py
	$(PYTHON) scripts/generate_webp_hash_chain_window_cutoff_fixture.py --check
	$(PYTHON) scripts/generate_webp_hash_chain_base_zero_fixture.py --check
	$(PYTHON) scripts/verify_roadmap.py
	$(PYTHON) scripts/generate_capability_docs.py --check

.PHONY: refresh-fixture-integrity
refresh-fixture-integrity:
	$(PYTHON) scripts/refresh_fixture_integrity.py
	$(MAKE) verify-fixture-integrity

.PHONY: verify
verify: release-tools-test docs-lint docs-test verify-fixture-integrity
	$(PYTHON) scripts/verify_no_source_unit_tests.py
	$(PYTHON) scripts/verify_third_party_licenses.py
	$(PYTHON) scripts/generate_png_srgb_intent_fixtures.py --check
	$(PYTHON) scripts/generate_png_1bit_odd_width_fixture.py --check
	$(PYTHON) scripts/generate_png_adam7_rgb16_fixture.py --check
	$(PYTHON) scripts/generate_png_signature_nearmiss_fixture.py --check
	$(PYTHON) scripts/generate_png_chunk_type_parity_fixtures.py --check
	$(PYTHON) scripts/generate_png_itxt_missing_keyword_separator.py --check
	$(PYTHON) scripts/generate_png_idat_split_zlib_header_fixture.py --check
	$(PYTHON) scripts/generate_png_deflate_header_edges.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_trailing_dri_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_422_restart_trailing_dri_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_422_restart_markers_without_dri_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_missing_eoi_marker_fill_fixture.py --check
	$(PYTHON) scripts/generate_png_interrupted_idat_apng_fixture.py --check
	$(PYTHON) scripts/generate_png_apng_duplicate_actl.py --check
	$(PYTHON) scripts/generate_png_apng_fdat_sequence_mismatch_before_idat.py --check
	$(PYTHON) scripts/generate_png_apng_gray16_partial_fixture.py --check
	$(PYTHON) scripts/generate_ico_equal_area_tiebreak_fixture.py --check
	$(PYTHON) scripts/generate_ico_equal_area_same_depth_fixture.py --check
	$(PYTHON) scripts/generate_ico_png_invalid_signature_fixture.py --check
	$(PYTHON) scripts/generate_cur_equal_size_y_hotspots_fixture.py --check
	$(PYTHON) scripts/generate_cur_larger_dimensions_fixture.py --check
	$(PYTHON) scripts/generate_ico_zero_bpp_depth_fallbacks_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_long_huffman_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_grayscale_subsampling_fixtures.py --check
	$(PYTHON) scripts/generate_jpeg_avx2_420_edge_batch_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_grayscale_duplicate_scan_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_trailing_dht_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_trailing_dht_no_eoi_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_trailing_app1_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_com_tail_junk_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_com_tail_junk_then_com_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_com_oversized_length_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_multiscan_com_oversized_length_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_progressive_oversized_com_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_app1_oversized_length_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_dht_oversized_length_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_dht_missing_length_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_multiscan_dht_after_second_scan_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_com_eoi_marker_fill_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_444_multiscan_marker_fill_stuffed_byte_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_restart_only_scan_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_com_ff00_tail_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_two_trailing_com_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_trailing_app2_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_single_scan_trailing_app0_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_post_scan_app14_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_trailing_dht_malformed_fixtures.py --check
	$(PYTHON) scripts/generate_jpeg_oversubscribed_dht_fallback_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_progressive_oversubscribed_dht_fixture.py --check
	$(PYTHON) scripts/generate_bmp_gif_edge_fixtures.py --check
	$(PYTHON) scripts/generate_gif_finite_loop_fixture.py --check
	$(PYTHON) scripts/generate_gif_split_loop_subblocks_fixture.py --check
	$(PYTHON) scripts/generate_bmp_top_down_24_odd_width_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_unused_custom_dc_chroma_table_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_custom_active_ac_chroma_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_progressive_correction_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_progressive_partial_mcu_fixtures.py --check
	$(PYTHON) scripts/generate_jpeg_cmyk_subsampling_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_sampling_ratio_fixtures.py --check
	$(PYTHON) scripts/generate_jpeg_rgb_sampling_fixtures.py --check
	$(PYTHON) scripts/generate_jpeg_adobe_precedence_fixtures.py --check
	$(PYTHON) scripts/generate_jpeg_jfif_boundaries.py --check
	$(PYTHON) scripts/generate_jpeg_jcoef_boundary_fixtures.py --check
	$(PYTHON) scripts/generate_jpeg_idct_workspace_fixtures.py --check
	$(PYTHON) scripts/generate_jpeg_partial_smoothing_fixtures.py --check
	$(PYTHON) scripts/generate_jpeg_empty_progressive_scans.py --check
	$(PYTHON) scripts/generate_jpeg_dequant_width_fixtures.py --check
	$(PYTHON) scripts/generate_jpeg_fdct_boundary_sources.py --check
	$(PYTHON) scripts/generate_jpeg_ycck_subsampled_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_ycck_h2v2_singleton_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_ycck_h2v1_singleton_fixture.py --check
	$(PYTHON) scripts/generate_jpeg_ycck_even_upsample_fixtures.py --check
	$(PYTHON) scripts/generate_tiff_duplicate_rational_fill_order_fixture.py --check
	$(PYTHON) scripts/generate_tiff_compressed_fill_order2_gray8.py --check
	$(PYTHON) scripts/generate_tiff_rgb16_fixtures.py --check
	$(PYTHON) scripts/generate_tiff_gray16_tiled_fixture.py --check
	$(PYTHON) scripts/generate_tiff_gray32_tiled_predictor_fixture.py --check
	$(PYTHON) scripts/generate_tiff_palette4_fillorder2_fixtures.py --check
	$(PYTHON) scripts/generate_tiff_ycbcr_subsampled_fixtures.py --check
	$(PYTHON) scripts/generate_tiff_signed_private_tags_fixture.py --check
	$(PYTHON) scripts/generate_tiff_later_ifd_bits_fixtures.py --check
	$(PYTHON) scripts/generate_webp_animated_top_level_image_fixtures.py --check
	$(PYTHON) scripts/generate_webp_animated_origin_partial_fixture.py --check
	$(PYTHON) scripts/generate_webp_lossless_run_cap_fixture.py --check
	$(PYTHON) scripts/generate_webp_lossless_distance2_match_fixture.py --check
	$(PYTHON) scripts/generate_webp_method0_sampling_boundary_fixture.py --check
	$(PYTHON) scripts/generate_webp_intra4_macroblock_grid_fixture.py --check
	$(PYTHON) scripts/generate_webp_vp8_header_budget_fixture.py --check
	$(PYTHON) scripts/generate_webp_segment_alpha_full_span_fixture.py --check
	$(PYTHON) scripts/generate_avif_iinf_child_kind_fixture.py --check
	$(PYTHON) scripts/generate_avif_iloc_trailing_payload_fixture.py --check
	$(PYTHON) scripts/generate_avif_multi_entry_stsc_fixture.py --check
	$(PYTHON) scripts/generate_avif_stsc_order_error_fixtures.py --check
	$(PYTHON) scripts/generate_avif_iprp_order_error_fixture.py --check
	$(PYTHON) scripts/generate_avif_sample_table_edge_fixtures.py --check
	$(PYTHON) scripts/generate_avif_stsz_edge_fixtures.py --check
	$(PYTHON) scripts/generate_avif_stsd_edge_fixtures.py --check
	$(PYTHON) scripts/generate_avif_ignored_hint_track_fixture.py --check
	$(PYTHON) scripts/generate_avif_missing_leading_ftyp_fixture.py --check
	$(PYTHON) scripts/generate_avif_trailing_meta_sequence_fixture.py --check
	$(PYTHON) scripts/generate_avif_grid_alpha_sample_count_mismatch.py --check
	$(PYTHON) scripts/generate_avif_grid_non_av1_color_tile.py --check
	$(PYTHON) scripts/generate_avif_grid_unreferenced_alpha.py --check
	$(PYTHON) scripts/generate_avif_grid_orphan_dimg.py --check
	$(PYTHON) scripts/generate_avif_padding_count_fixtures.py --check
	$(PYTHON) scripts/generate_avif_obu_header_fixtures.py --check
	$(PYTHON) scripts/generate_avif_operating_point_fixtures.py --check
	$(PYTHON) scripts/generate_avif_frame_timing_fixtures.py --check
	$(PYTHON) scripts/generate_avif_sequence_truncation_fixtures.py --check
	$(PYTHON) scripts/generate_avif_unsupported_primary_item_type.py --check
	$(PYTHON) scripts/generate_avif_top_level_skip_fixture.py --check
	$(PYTHON) scripts/generate_avif_missing_ipma_fixture.py --check
	$(PYTHON) scripts/generate_avif_media_box_edges.py --check
	$(PYTHON) scripts/generate_avif_missing_sample_table_fixtures.py --check
	$(PYTHON) scripts/generate_avif_imir_fixtures.py --check
	$(PYTHON) scripts/generate_avif_duplicate_transform_properties.py --check
	$(PYTHON) scripts/generate_avif_pasp_compat_fixtures.py --check
	$(PYTHON) scripts/generate_avif_irot_edge_fixtures.py --check
	$(PYTHON) scripts/generate_avif_trailing_top_level_duplicates.py --check
	$(PYTHON) scripts/generate_avif_still_duplicate_meta_after_mdat.py --check
	$(PYTHON) scripts/generate_avif_iref_v1_alpha_fixture.py --check
	$(PYTHON) scripts/generate_avif_alpha_missing_auxiliary_ispe_fixture.py --check
	$(PYTHON) scripts/generate_avif_non_primary_property_duplicate_fixtures.py --check
	$(PYTHON) scripts/generate_avif_monochrome_grid_fixture.py --check
	$(PYTHON) scripts/generate_avif_monochrome_grid_edges.py --check
	$(PYTHON) scripts/generate_avif_monochrome_noise_fixture.py --check
	$(PYTHON) scripts/generate_avif_lossy_monochrome_noise_fixture.py --check
	$(PYTHON) scripts/generate_avif_container_edge_fixtures.py --check
	$(PYTHON) scripts/generate_avif_idat_fixture.py --check
	$(PYTHON) scripts/generate_avif_track_box_parity_fixtures.py --case edit_list_entry_count_error --check
	$(PYTHON) scripts/generate_avif_track_box_parity_fixtures.py --case edit_list_segment_duration_zero_error --check
	$(PYTHON) scripts/generate_avif_track_box_parity_fixtures.py --case edit_box_duplicate_elst_error --check
	$(PYTHON) scripts/generate_avif_track_box_parity_fixtures.py --case edit_box_missing_elst_error --check
	$(PYTHON) scripts/generate_avif_track_box_parity_fixtures.py --case zero_track_id_error --check
	$(PYTHON) scripts/generate_avif_sequence_selection_fixtures.py --check
	$(PYTHON) scripts/generate_avif_show_existing_unshowable_reference.py --check
	$(PYTHON) scripts/generate_avif_show_existing_empty_reference.py --check
	$(PYTHON) scripts/generate_avif_zero_decoder_tick_fixture.py --check
	$(PYTHON) scripts/generate_avif_decoder_model_parity_fixture.py --check
	$(PYTHON) scripts/generate_avif_sequence_header_parity.py --check
	$(PYTHON) scripts/generate_avif_sequence_header_parity.py --case initial-display-delay-10 --check
	$(PYTHON) scripts/generate_avif_sequence_header_parity.py --case equal-picture-interval-two-ticks --check
	$(PYTHON) scripts/generate_avif_sequence_header_parity.py --case operating-point-high-tier --check
	$(PYTHON) scripts/generate_avif_sequence_header_parity.py --case decoder-model-without-operating-point-parameters --check
	$(PYTHON) scripts/generate_avif_sequence_color_primaries_mismatch.py --check
	$(PYTHON) scripts/generate_avif_sequence_header_errors.py --case reduced-header-nonstill --check
	$(PYTHON) scripts/generate_avif_sequence_header_errors.py --case invalid-operating-point-idc --check
	$(PYTHON) scripts/generate_avif_sequence_header_errors.py --case initial-display-delay-over-10 --check
	$(PYTHON) scripts/generate_avif_sequence_header_errors.py --case sequence-header-short-payload --check
	$(PYTHON) scripts/generate_avif_sequence_header_errors.py --case sequence-header-omitted --check
	$(PYTHON) scripts/generate_avif_sequence_header_errors.py --case uvlc-leading-zero-limit --check
	$(PYTHON) scripts/generate_avif_nonuniform_tile_grid_error.py --case tile-group-mismatch --check
	$(PYTHON) scripts/generate_avif_nonuniform_tile_grid_error.py --case ns-threshold-false --check
	$(PYTHON) scripts/generate_avif_global_motion_subexp_endpoint_fixture.py --check
	$(PYTHON) scripts/generate_avif_filmgrain_chroma_from_luma_odd.py --check
	$(PYTHON) scripts/generate_avif_filmgrain_edge_fixtures.py --check
	$(PYTHON) scripts/generate_avif_filmgrain_depth_fixtures.py --check
	$(PYTHON) scripts/generate_avif_inter_i420_mode_largest_16x64.py --check
	$(PYTHON) scripts/generate_avif_inter_i420_mode_select_16x64.py --check
	$(PYTHON) scripts/generate_avif_inter_i420_b16x64_mode_select.py --check
	$(PYTHON) scripts/generate_avif_inter_reduced_transform_set.py --check
	$(PYTHON) scripts/generate_avif_inter_i420_b16x64_qindex0_mode_select.py --check
	$(PYTHON) scripts/generate_avif_inter_monochrome_b16x64_qindex0_mode_select.py --check
	$(PYTHON) scripts/generate_avif_lossless_inter_i420_b128x128_10bit.py --check
	$(PYTHON) scripts/generate_avif_inter_i444_b8x16.py --check
	$(PYTHON) scripts/generate_avif_inter_i444_b16x64_mode2.py --check
	$(PYTHON) scripts/generate_avif_inter_i420_b8x16_mixed_topology.py --check
	$(PYTHON) scripts/generate_avif_lossless_wide_inter_grid_fixture.py --check
	$(PYTHON) scripts/generate_avif_lossless_compound_wide_i420.py --check
	$(PYTHON) scripts/generate_avif_lossless_compound_tall_i420.py --check
	$(PYTHON) scripts/generate_avif_lossless_inter_i420_b64x128.py --check
	$(PYTHON) scripts/generate_avif_invalid_lossless_i422_b64x128.py --check
	$(PYTHON) scripts/generate_avif_10bit_lossless_inter_fixture.py --fixture clipped_b16x16_10bit_partition16_60x64 --check
	$(PYTHON) scripts/generate_avif_inter_i420_mode_largest_64x16.py --check
	$(PYTHON) scripts/generate_avif_inter_i420_b64x16.py --mode largest --check
	$(PYTHON) scripts/generate_avif_inter_i420_b64x16.py --mode select --check
	$(PYTHON) scripts/generate_avif_inter_i420_b64x16.py --mode select --bit-depth 10 --check
	$(PYTHON) scripts/generate_avif_inter_i420_b64x16.py --mode select --bit-depth 12 --check
	$(PYTHON) scripts/generate_avif_hi10_i444_superres_restoration.py --check
	$(PYTHON) scripts/generate_avif_hi10_i422_superres_restoration.py --check
	$(PYTHON) scripts/generate_avif_inter_i444_frame_header_b64.py --check
	$(PYTHON) scripts/generate_avif_lossy_inter_i420_b32x64.py --check
	$(PYTHON) scripts/generate_avif_lossy_inter_i420_b32x64.py --profile topology --check
	$(PYTHON) scripts/generate_avif_lossy_wide_i420_b128x128.py --check
	$(PYTHON) scripts/generate_avif_lossy_wide_i420_10bit.py --check
	$(PYTHON) scripts/generate_avif_lossy_compound_wide_i420.py --check
	$(PYTHON) scripts/generate_avif_lossy_compound_wide_i420.py --variant unsplit --check
	$(PYTHON) scripts/generate_avif_interinter_wedge_i420.py --check
	$(PYTHON) scripts/generate_avif_interinter_diffwtd.py --variant i420 --check
	$(PYTHON) scripts/generate_avif_interinter_diffwtd.py --variant i420-inverted --check
	$(PYTHON) scripts/generate_avif_interinter_diffwtd.py --variant i422 --check
	$(PYTHON) scripts/generate_avif_interinter_diffwtd.py --variant i444 --check
	$(PYTHON) scripts/generate_avif_lossless_qmatrix_sequence.py --check
	$(PYTHON) scripts/generate_avif_lossy_qmatrix_fixture.py --check
	$(PYTHON) scripts/generate_avif_equal_width_superres.py --chroma 420-actual-no-restoration --check
	$(PYTHON) scripts/generate_avif_temporal_mv_range_fixture.py --check
	$(PYTHON) scripts/generate_avif_inter_422_vertical_split_fixture.py --check
	$(PYTHON) scripts/generate_avif_skip_mode_enabled_fixture.py --check
	$(PYTHON) scripts/generate_avif_inter_422_small_partitions_fixture.py --check
	$(PYTHON) scripts/generate_avif_lossless_inter_i422_b16x16.py --check
	$(PYTHON) scripts/generate_avif_inter_i422_loopfilter_16x16.py --check
	$(PYTHON) scripts/generate_avif_inter_422_qindex0_mode_select.py --check
	$(PYTHON) scripts/generate_avif_inter_422_qindex0_mode_select.py --variant b8x4 --check
	$(PYTHON) scripts/generate_avif_inter_422_compound_direct_grid_fixture.py --check
	$(PYTHON) scripts/verify_diagnostic_provenance.py
	$(PYTHON) scripts/verify_unreachable_contracts.py

.PHONY: lint
# All-features can hide cfg branches used by the crate's default feature set.
lint: fmt
	cargo clippy --workspace --all-targets --locked -- -D warnings
	cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
	cargo clippy --release --workspace --all-targets --all-features --locked -- -D warnings
	CARGO_TARGET_DIR=target/cfg-coverage-clippy cargo +"$(COVERAGE_TOOLCHAIN)" clippy --workspace --all-targets --all-features --locked -- -D warnings --cfg coverage --cfg coverage_nightly
	# AVIF-only coverage must also compile with the GIF constructor disabled.
	CARGO_TARGET_DIR=target/cfg-coverage-clippy cargo +"$(COVERAGE_TOOLCHAIN)" clippy --workspace --all-targets --no-default-features --features avif --locked -- -D warnings --cfg coverage --cfg coverage_nightly
	cargo clippy --manifest-path benchmarks/jpeg-production/rust/Cargo.toml --release --locked -- -D warnings

.PHONY: lint-x86-simd
lint-x86-simd:
	$(MAKE) fmt
	CARGO_TARGET_DIR=target/x86_64-sse2 RUSTFLAGS="-C target-cpu=x86-64" cargo clippy --workspace --all-targets --all-features --locked --target x86_64-unknown-linux-gnu -- -D warnings
	CARGO_TARGET_DIR=target/x86_64-avx2 RUSTFLAGS="-C target-cpu=x86-64 -C target-feature=+avx2" cargo clippy --workspace --all-targets --all-features --locked --target x86_64-unknown-linux-gnu -- -D warnings

.PHONY: test
test:
	cargo check --all-features --locked
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
	cargo test --doc --all-features --locked
	cargo test --workspace --all-features --locked
	$(MAKE) test-feature-matrix

.PHONY: test-feature-matrix
test-feature-matrix: ## Run native, WASI, and WASM lanes with strict target Clippy
	scripts/test_feature_matrix.sh

.PHONY: supply-chain
supply-chain:
	cargo deny check

.PHONY: coverage
coverage:
	mkdir -p "$(dir $(COVERAGE_REPORT))"
	CARGO_TARGET_DIR=$(call coverage_exporter_quote,$(COVERAGE_FULL_TARGET_DIR)) \
		cargo +"$(COVERAGE_TOOLCHAIN)" llvm-cov --all-features --branch --locked --json \
		--output-path "$(COVERAGE_REPORT)" --no-fail-fast
	$(MAKE) coverage-check

.PHONY: coverage-selected
coverage-selected:
	@test -n "$(value COVERAGE_CASES)" || { printf "Set COVERAGE_CASES to one or more exact operation:format:row-id selectors.\\n" >&2; exit 2; }
	$(if $(findstring $$,$(value COVERAGE_CASES)),$(error COVERAGE_CASES contains a dollar sign unsupported by the Make recipe))
	$(if $(findstring `,$(value COVERAGE_CASES)),$(error COVERAGE_CASES contains a backtick unsupported by the Make recipe))
	$(if $(findstring ",$(value COVERAGE_CASES)),$(error COVERAGE_CASES contains a double quote unsupported by the Make recipe))
	$(if $(findstring \, $(value COVERAGE_CASES)),$(error COVERAGE_CASES contains a backslash unsupported by the Make recipe))
	$(if $(findstring $(coverage_exporter_newline),$(value COVERAGE_CASES)),$(error COVERAGE_CASES contains a newline unsupported by the Make recipe))
	$(if $(findstring $$,$(value SELECTED_COVERAGE_REPORT)),$(error SELECTED_COVERAGE_REPORT contains a dollar sign unsupported by the Make recipe))
	$(if $(findstring `,$(value SELECTED_COVERAGE_REPORT)),$(error SELECTED_COVERAGE_REPORT contains a backtick unsupported by the Make recipe))
	$(if $(findstring ",$(value SELECTED_COVERAGE_REPORT)),$(error SELECTED_COVERAGE_REPORT contains a double quote unsupported by the Make recipe))
	$(if $(findstring \, $(value SELECTED_COVERAGE_REPORT)),$(error SELECTED_COVERAGE_REPORT contains a backslash unsupported by the Make recipe))
	$(if $(findstring $(coverage_exporter_newline),$(value SELECTED_COVERAGE_REPORT)),$(error SELECTED_COVERAGE_REPORT contains a newline unsupported by the Make recipe))
	CARGO_TARGET_DIR=$(call coverage_exporter_quote,$(COVERAGE_SELECTED_TARGET_DIR)) \
		$(PYTHON) scripts/run_selected_coverage.py --cases $(call coverage_exporter_quote,$(value COVERAGE_CASES)) \
		--toolchain $(call coverage_exporter_quote,$(value COVERAGE_TOOLCHAIN)) \
		$(if $(value SELECTED_COVERAGE_REPORT),--report $(call coverage_exporter_quote,$(value SELECTED_COVERAGE_REPORT)))

.PHONY: coverage-exporter-setup coverage-with-exporter
coverage-exporter-setup:
	$(PYTHON) scripts/setup_coverage_exporter.py setup \
		--prefix $(call coverage_exporter_quote,$(COVERAGE_EXPORTER_PREFIX)) --source $(call coverage_exporter_quote,$(COVERAGE_EXPORTER_SOURCE)) \
		--toolchain $(call coverage_exporter_quote,$(COVERAGE_TOOLCHAIN))

coverage-with-exporter:
	$(if $(findstring $$,$(value COVERAGE_REPORT)),$(error COVERAGE_REPORT contains characters unsupported by the recursive Make recipes))
	$(if $(findstring $(coverage_exporter_newline),$(value COVERAGE_REPORT)),$(error COVERAGE_REPORT contains characters unsupported by the recursive Make recipes))
	+$(PYTHON) scripts/setup_coverage_exporter.py run \
		--prefix $(call coverage_exporter_quote,$(COVERAGE_EXPORTER_PREFIX)) --toolchain $(call coverage_exporter_quote,$(COVERAGE_TOOLCHAIN)) \
		--make $(call coverage_exporter_quote,$(MAKE)) --target $(call coverage_exporter_quote,$(COVERAGE_EXPORTER_TARGET)) --report $(call coverage_exporter_quote,$(value COVERAGE_REPORT))

.PHONY: coverage-check
coverage-check:
	$(PYTHON) scripts/verify_llvm_coverage.py "$(COVERAGE_REPORT)"

.PHONY: coverage-complete
coverage-complete: coverage
	$(PYTHON) scripts/verify_llvm_coverage.py "$(COVERAGE_REPORT)" --strict

.PHONY: release-tools-test
release-tools-test:
	$(PYTHON) scripts/test_release_checks.py

.PHONY: package-verify
package-verify:
	$(PYTHON) scripts/verify_release_archive.py

.PHONY: ci
ci: workflows-check fmt verify lint test supply-chain coverage package-verify

.PHONY: release-verify
release-verify:
	@test -z "$$(git status --porcelain=v1 --untracked-files=all)" || { \
		printf "release-verify requires a clean worktree.\n" >&2; exit 2; \
	}
	$(MAKE) ci

.PHONY: ci-quality
ci-quality: workflows-check fmt verify lint test package-verify

.PHONY: release-check-version
release-check-version:
	$(PYTHON) scripts/verify_release_archive.py --metadata-only

.PHONY: dependency-update
dependency-update:
	@test -n "$(DEPENDENCY)" -a -n "$(DEPENDENCY_VERSION)" || { \
		printf "Set DEPENDENCY and DEPENDENCY_VERSION to the reviewed package and exact version.\n" >&2; exit 2; \
	}
	cargo update --package "$(DEPENDENCY)" --precise "$(DEPENDENCY_VERSION)"
	cargo update --manifest-path benchmarks/jpeg-production/rust/Cargo.toml --package "$(DEPENDENCY)" --precise "$(DEPENDENCY_VERSION)"

.PHONY: release-lock-update
release-lock-update:
	cargo update --workspace --offline
	cargo update --manifest-path benchmarks/jpeg-production/rust/Cargo.toml --workspace --offline

.PHONY: release-registry-verify
release-registry-verify:
	$(PYTHON) scripts/publish_release.py --candidate "$(RELEASE_CRATE)" --output "$(REGISTRY_CRATE)"

.PHONY: release-publish-prepare
release-publish-prepare:
	cargo package --locked
	cmp "target/package/$(PACKAGE_NAME)-$(PACKAGE_VERSION).crate" "$(VERIFIED_CRATE)"

.PHONY: release-publish-request
release-publish-request:
	@test -n "$(VERIFIED_CRATE)" && test -n "$(PUBLISH_REQUEST)" || { \
		printf "Set VERIFIED_CRATE and PUBLISH_REQUEST to the verified crate and upload request paths.\n" >&2; exit 2; \
	}
	$(PYTHON) scripts/prepare_crates_io_publish.py --candidate "$(VERIFIED_CRATE)" --output "$(PUBLISH_REQUEST)"

.PHONY: build fmt-fix example bench bench-setup
build:
	cargo build --locked

fmt-fix:
	cargo fmt --all
	cargo fmt --manifest-path benchmarks/jpeg-production/rust/Cargo.toml

example:
	cargo run --locked --example package_smoke

BENCH_OUTPUT ?= target/benchmarks/jpeg/latest
BENCH_ROUNDS ?= 5
BENCH_TARGET_DIR ?= target/jpeg-production-matrix
TURBOJPEG_PREFIX ?= $(CURDIR)/target/benchmark-oracle/3.2.0/install
bench-setup:
	$(PYTHON) scripts/setup_benchmark_oracle.py

bench:
	JPEG_PRODUCTION_TARGET_DIR="$(BENCH_TARGET_DIR)" \
	  $(PYTHON) benchmarks/jpeg-production/run_matrix.py --rounds "$(BENCH_ROUNDS)" \
	  --output "$(BENCH_OUTPUT)" --turbojpeg-prefix "$(TURBOJPEG_PREFIX)"

include docs.mk

.PHONY: package-surface-check
package-surface-check: ## Check the exact declared Cargo archive file list
	$(PYTHON) scripts/verify_package_surface.py

.PHONY: workflows-check
workflows-check: ## Require SHA-pinned actions and validate workflows with actionlint
	$(PYTHON) scripts/check_workflows.py
