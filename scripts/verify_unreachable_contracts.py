#!/usr/bin/env python3
"""Verify the bounded v1 map for behavior Pillow cannot prove.

The map is deliberately smaller than the roadmap. It names existing
fixture-backed integration contracts and typed target-only fault cases, or
records a category as planned when no such contract exists. It is a static
audit: it does not execute Rust tests, collect coverage, or turn a Pillow
parity row into evidence for a Rust-only field.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "tests" / "fixtures" / "unreachable_contract_manifest.json"
DOCUMENT = ROOT / "docs" / "EVIDENCE.md"
SOURCE_MANIFEST = ROOT / "manifest.yaml"

TOP_LEVEL_KEYS = {
    "format_version",
    "scope",
    "pillow_parity",
    "fault_contract_cases",
    "categories",
}
PARITY_KEYS = {"status", "matrix", "reason"}
CATEGORY_KEYS = {
    "id",
    "label",
    "status",
    "scope",
    "pillow_parity",
    "evidence",
    "planned_reason",
    "planned_context",
}
EVIDENCE_KEYS = {"kind", "path", "fixture", "tests", "command"}
FAULT_CONTRACT_CASE_KEYS = {"id", "category", "selector", "test"}
FAULT_CONTRACTS = {
    "avif_segment_map_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_segment_map_reservation_failure",
        "source_case": "decode:avif:mixed_lossless_clipped_b32_i420_60x60",
        "point": "av1.segment_map.reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_monochrome_alpha_plane_length_mismatch": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_monochrome_alpha_plane_length_mismatch",
        "source_case": "decode:avif:portable_lossless_monochrome_limited_alpha_16x16",
        "point": "avif.monochrome_alpha.plane_length_mismatch",
        "contract": "decode_unsupported_then_retry_succeeds",
    },
    "avif_10bit_i444_alpha_plane_length_mismatch": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_10bit_i444_alpha_plane_length_mismatch",
        "source_case": "decode:avif:high_bitdepth_still_10bit_444_alpha_lossy_16x16",
        "point": "avif.i444_10bit_alpha.plane_length_mismatch",
        "contract": "decode_unsupported_then_retry_succeeds",
    },
    "avif_12bit_i444_generic_alpha_plane_length_mismatch": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_12bit_i444_generic_alpha_plane_length_mismatch",
        "source_case": "decode:avif:high_bitdepth_still_12bit_444_alpha_9x9",
        "point": "avif.i444_generic_alpha.plane_length_mismatch",
        "contract": "decode_unsupported_then_retry_succeeds",
    },
    "avif_12bit_i444_generic_u_plane_length_mismatch": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_12bit_i444_generic_u_plane_length_mismatch",
        "source_case": "decode:avif:high_bitdepth_still_12bit_444_alpha_9x9",
        "point": "avif.i444_generic_rgb.u_plane_length_mismatch",
        "contract": "decode_unsupported_then_retry_succeeds",
    },
    "avif_12bit_i444_generic_v_plane_length_mismatch": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_12bit_i444_generic_v_plane_length_mismatch",
        "source_case": "decode:avif:high_bitdepth_still_12bit_444_alpha_9x9",
        "point": "avif.i444_generic_rgb.v_plane_length_mismatch",
        "contract": "decode_unsupported_then_retry_succeeds",
    },
    "avif_12bit_i444_generic_invalid_sample": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_12bit_i444_generic_invalid_sample",
        "source_case": "decode:avif:high_bitdepth_still_12bit_444_alpha_9x9",
        "point": "avif.i444_generic_rgb.invalid_sample",
        "contract": "decode_unsupported_then_retry_succeeds",
    },
    "avif_12bit_i444_generic_rgb_output_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_12bit_i444_generic_rgb_output_reservation_failure",
        "source_case": "decode:avif:high_bitdepth_still_12bit_444_alpha_9x9",
        "point": "avif.i444_generic_rgb.output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_subsampled_rgb_output_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_subsampled_rgb_output_reservation_failure",
        "source_case": "decode:avif:baseline",
        "point": "avif.subsampled_rgb.output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_10bit_i444_alpha_invalid_sample": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_10bit_i444_alpha_invalid_sample",
        "source_case": "decode:avif:high_bitdepth_still_10bit_444_alpha_lossy_16x16",
        "point": "avif.i444_10bit_alpha.invalid_sample",
        "contract": "decode_unsupported_then_retry_succeeds",
    },
    "avif_10bit_i444_alpha_output_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_10bit_i444_alpha_output_reservation_failure",
        "source_case": "decode:avif:high_bitdepth_still_10bit_444_alpha_lossy_16x16",
        "point": "avif.i444_10bit_alpha.output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_10bit_i444_u_plane_length_mismatch": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_10bit_i444_u_plane_length_mismatch",
        "source_case": "decode:avif:high_bitdepth_still_10bit_444_alpha_lossy_16x16",
        "point": "avif.i444_10bit_alpha.u_plane_length_mismatch",
        "contract": "decode_unsupported_then_retry_succeeds",
    },
    "avif_10bit_i444_v_plane_length_mismatch": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_10bit_i444_v_plane_length_mismatch",
        "source_case": "decode:avif:high_bitdepth_still_10bit_444_alpha_lossy_16x16",
        "point": "avif.i444_10bit_alpha.v_plane_length_mismatch",
        "contract": "decode_unsupported_then_retry_succeeds",
    },
    "avif_display_plane_copy_allocation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_display_plane_copy_allocation_failure",
        "source_case": "decode:avif:baseline",
        "point": "av1.display_plane.reconstructed_copy_allocation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_display_plane_copy_sequence_allocation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_display_plane_copy_sequence_allocation_failure",
        "source_case": "decode:avif:animated_motion_chroma_422",
        "point": "av1.display_plane.reconstructed_copy_allocation",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_film_grain_output_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_film_grain_output_reservation_failure",
        "source_case": "decode:avif:portable_lossless_filmgrain_i444_64x64",
        "point": "av1.film_grain.output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_film_grain_points_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_film_grain_points_reservation_failure",
        "source_case": "decode:avif:portable_lossless_filmgrain_i444_64x64",
        "point": "av1.film_grain.points_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_temporal_motion_field_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_temporal_motion_field_reservation_failure",
        "source_case": "decode:avif:baseline",
        "point": "av1.temporal_motion_field.reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_projected_temporal_field_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_projected_temporal_field_reservation_failure",
        "source_case": "decode:avif:animated_motion_chroma",
        "point": "av1.temporal_projection.field_reservation",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_grid_cell_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_grid_cell_reservation_failure",
        "source_case": "decode:avif:grid",
        "point": "av1.grid.cell_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_monochrome_grid_cell_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_monochrome_grid_cell_reservation_failure",
        "source_case": "decode:avif:portable_lossless_monochrome_grid_73x64",
        "point": "av1.grid.monochrome_cell_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_monochrome_loop_filter_metadata_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_monochrome_loop_filter_metadata_reservation_failure",
        "source_case": "decode:avif:multitile_monochrome_split_groups",
        "point": "av1.frame.assembled_monochrome_loop_filter_metadata_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_monochrome_cdef_region_map_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_monochrome_cdef_region_map_reservation_failure",
        "source_case": "decode:avif:multitile_monochrome_split_groups",
        "point": "av1.frame.assembled_monochrome_cdef_region_map_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_monochrome_cdef_active_map_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_monochrome_cdef_active_map_reservation_failure",
        "source_case": "decode:avif:multitile_monochrome_split_groups",
        "point": "av1.frame.assembled_monochrome_cdef_active_map_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_monochrome_group_buffer_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_monochrome_group_buffer_reservation_failure",
        "source_case": "decode:avif:multitile_monochrome",
        "point": "av1.frame.monochrome_group_buffer_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_sequence_frames_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_sequence_frames_reservation_failure",
        "source_case": "decode:avif:animated_motion_chroma",
        "point": "avif.sequence_frames.output_reservation",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_superresolution_positions_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_superresolution_positions_reservation_failure",
        "source_case": "decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56",
        "point": "av1.superres.position_reservation",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_superresolution_plane_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_superresolution_plane_reservation_failure",
        "source_case": "decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56",
        "point": "av1.superres.plane_reservation",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_superresolution_header_geometry_mismatch": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_superresolution_header_geometry_mismatch",
        "source_case": "decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56",
        "point": "av1.superres.header_geometry_mismatch",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_superresolution_chroma_u_plane_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_superresolution_chroma_u_plane_reservation_failure",
        "source_case": "decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56",
        "point": "av1.superres.chroma_u_plane_reservation",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_superresolution_chroma_v_plane_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_superresolution_chroma_v_plane_reservation_failure",
        "source_case": "decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56",
        "point": "av1.superres.chroma_v_plane_reservation",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_sgr_source_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_sgr_source_reservation_failure",
        "source_case": "decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56",
        "point": "av1.restoration.sgr_source_reservation",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_item_table_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_item_table_reservation_failure",
        "source_case": "decode:avif:baseline",
        "point": "avif.metadata.item_table_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_item_location_table_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_item_location_table_reservation_failure",
        "source_case": "decode:avif:baseline",
        "point": "avif.metadata.item_location_table_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_item_extent_span_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_item_extent_span_reservation_failure",
        "source_case": "decode:avif:baseline",
        "point": "avif.metadata.item_extent_span_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "jpeg_multiscan_coefficient_reservation_failure": {
        "format": "jpeg",
        "category": "structured-diagnostics",
        "selector": "fault-contract:jpeg:jpeg_multiscan_coefficient_reservation_failure",
        "source_case": "decode:jpeg:baseline_444_multiscan_duplicate_component",
        "point": "jpeg.multiscan.coefficient_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "jpeg_huffman_table_slot_reservation_failure": {
        "format": "jpeg",
        "category": "structured-diagnostics",
        "selector": "fault-contract:jpeg:jpeg_huffman_table_slot_reservation_failure",
        "source_case": "decode:jpeg:baseline_444_multiscan_late_dht_after_default_scan",
        "point": "jpeg.baseline.huffman_table_slot_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "jpeg_ycck_output_reservation_failure": {
        "format": "jpeg",
        "category": "structured-diagnostics",
        "selector": "fault-contract:jpeg:jpeg_ycck_output_reservation_failure",
        "source_case": "decode:jpeg:color_ycck_app14_transform_2",
        "point": "jpeg.ycck.output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "jpeg_h1v2_upsample_plane_reservation_failure": {
        "format": "jpeg",
        "category": "structured-diagnostics",
        "selector": "fault-contract:jpeg:jpeg_h1v2_upsample_plane_reservation_failure",
        "source_case": "decode:jpeg:baseline_420_chroma_component_sampling_fallback",
        "point": "jpeg.upsample.h1v2_plane_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "jpeg_h1v2_upsample_empty_geometry_rejection": {
        "format": "jpeg",
        "category": "structured-diagnostics",
        "selector": "fault-contract:jpeg:jpeg_h1v2_upsample_empty_geometry_rejection",
        "source_case": "decode:jpeg:baseline_420_chroma_component_sampling_fallback",
        "point": "jpeg.upsample.h1v2_empty_geometry",
        "contract": "decode_error_then_retry_succeeds",
    },
    "jpeg_h1v2_upsample_zero_width_rejection": {
        "format": "jpeg",
        "category": "structured-diagnostics",
        "selector": "fault-contract:jpeg:jpeg_h1v2_upsample_zero_width_rejection",
        "source_case": "decode:jpeg:baseline_420_chroma_component_sampling_fallback",
        "point": "jpeg.upsample.h1v2_zero_width",
        "contract": "decode_error_then_retry_succeeds",
    },
    "jpeg_h1v2_upsample_source_length_mismatch": {
        "format": "jpeg",
        "category": "structured-diagnostics",
        "selector": "fault-contract:jpeg:jpeg_h1v2_upsample_source_length_mismatch",
        "source_case": "decode:jpeg:baseline_420_chroma_component_sampling_fallback",
        "point": "jpeg.upsample.h1v2_source_length_mismatch",
        "contract": "decode_error_then_retry_succeeds",
    },
    "jpeg_fancy_upsample_row_invalid_geometry": {
        "format": "jpeg",
        "category": "structured-diagnostics",
        "selector": "fault-contract:jpeg:jpeg_fancy_upsample_row_invalid_geometry",
        "source_case": "decode:jpeg:color_ycck_app14_transform_2",
        "point": "jpeg.upsample.row_invalid_geometry",
        "contract": "decode_error_then_retry_succeeds",
    },
    "jpeg_fancy_upsample_row_short_stride": {
        "format": "jpeg",
        "category": "structured-diagnostics",
        "selector": "fault-contract:jpeg:jpeg_fancy_upsample_row_short_stride",
        "source_case": "decode:jpeg:color_ycck_app14_transform_2",
        "point": "jpeg.upsample.row_short_stride",
        "contract": "decode_error_then_retry_succeeds",
    },
    "jpeg_row_upsample_source_length_mismatch": {
        "format": "jpeg",
        "category": "structured-diagnostics",
        "selector": "fault-contract:jpeg:jpeg_row_upsample_source_length_mismatch",
        "source_case": "decode:jpeg:color_ycck_app14_transform_2",
        "point": "jpeg.upsample.row_source_length_mismatch",
        "contract": "decode_error_then_retry_succeeds",
    },
    "bmp_palette_reservation_failure": {
        "format": "bmp",
        "category": "structured-diagnostics",
        "selector": "fault-contract:bmp:bmp_palette_reservation_failure",
        "source_case": "decode:bmp:rle4_odd_absolute_eol",
        "point": "bmp.palette.output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "bmp_decoded_pixels_reservation_failure": {
        "format": "bmp",
        "category": "structured-diagnostics",
        "selector": "fault-contract:bmp:bmp_decoded_pixels_reservation_failure",
        "source_case": "decode:bmp:depth_24",
        "point": "bmp.decoded_pixels.output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_strip_assembly_reservation_failure": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_strip_assembly_reservation_failure",
        "source_case": "decode:tiff:compression_none",
        "point": "tiff.strip_assembly.output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_unaligned_strip_assembly_reservation_failure": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_unaligned_strip_assembly_reservation_failure",
        "source_case": "decode:tiff:ycbcr_subsampled_lzw_2x2_unaligned_rows_per_strip_3",
        "point": "tiff.strip_assembly.output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_rgb16_output_reservation_failure": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_rgb16_output_reservation_failure",
        "source_case": "decode:tiff:rgb16_le_uncompressed_strips",
        "point": "tiff.rgb16.output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_ycbcr_clump_length_mismatch": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_ycbcr_clump_length_mismatch",
        "source_case": "decode:tiff:ycbcr_subsampled_lzw_2x2",
        "point": "tiff.ycbcr.clump_length_mismatch",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_ycbcr_general_clump_length_mismatch": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_ycbcr_general_clump_length_mismatch",
        "source_case": "decode:tiff:ycbcr_subsampled_lzw_4x2",
        "point": "tiff.ycbcr.clump_length_mismatch",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_ycbcr_full_resolution_length_mismatch": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_ycbcr_full_resolution_length_mismatch",
        "source_case": "decode:tiff:ycbcr_full_resolution_lzw_1x1",
        "point": "tiff.ycbcr.full_resolution_length_mismatch",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_ycbcr_full_resolution_length_mismatch_custom_tags": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_ycbcr_full_resolution_length_mismatch_custom_tags",
        "source_case": "decode:tiff:ycbcr_custom_tags_full_resolution_lzw_1x1",
        "point": "tiff.ycbcr.full_resolution_length_mismatch",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_ycbcr_rgb_output_reservation_failure": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_ycbcr_rgb_output_reservation_failure",
        "source_case": "decode:tiff:ycbcr_subsampled_lzw_2x2",
        "point": "tiff.ycbcr.rgb_output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_ycbcr_custom_table_clump_length_mismatch": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_ycbcr_custom_table_clump_length_mismatch",
        "source_case": "decode:tiff:ycbcr_custom_coefficients_reference_black_white_lzw_2x2",
        "point": "tiff.ycbcr.clump_length_mismatch",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_ycbcr_custom_table_rgb_output_reservation_failure": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_ycbcr_custom_table_rgb_output_reservation_failure",
        "source_case": "decode:tiff:ycbcr_custom_coefficients_reference_black_white_lzw_2x2",
        "point": "tiff.ycbcr.rgb_output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_ycbcr_tile_assembly_reservation_failure": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_ycbcr_tile_assembly_reservation_failure",
        "source_case": "decode:tiff:ycbcr_subsampled_deflate_tiled_2x2",
        "point": "tiff.ycbcr.tile_assembly.output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_ycbcr_tile_prefix_limit_failure": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_ycbcr_tile_prefix_limit_failure",
        "source_case": "decode:tiff:ycbcr_subsampled_deflate_tiled_2x2",
        "point": "tiff.ycbcr.tile_prefix.output_limit",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_ycbcr_general_rgb_output_reservation_failure": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_ycbcr_general_rgb_output_reservation_failure",
        "source_case": "decode:tiff:ycbcr_subsampled_lzw_4x2",
        "point": "tiff.ycbcr.rgb_output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_ycbcr_1x2_clump_length_mismatch": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_ycbcr_1x2_clump_length_mismatch",
        "source_case": "decode:tiff:ycbcr_subsampled_lzw_1x2",
        "point": "tiff.ycbcr.clump_length_mismatch",
        "contract": "decode_error_then_retry_succeeds",
    },
    "tiff_ycbcr_1x2_rgb_output_reservation_failure": {
        "format": "tiff",
        "category": "structured-diagnostics",
        "selector": "fault-contract:tiff:tiff_ycbcr_1x2_rgb_output_reservation_failure",
        "source_case": "decode:tiff:ycbcr_subsampled_lzw_1x2",
        "point": "tiff.ycbcr.rgb_output_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "png_decoded_sample_buffer_reservation_failure": {
        "format": "png",
        "category": "structured-diagnostics",
        "selector": "fault-contract:png:png_decoded_sample_buffer_reservation_failure",
        "source_case": "decode:png:size_1x1",
        "point": "png.scanline.sample_buffer_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "png_apng_sequence_counter_exhaustion": {
        "format": "png",
        "category": "structured-diagnostics",
        "selector": "fault-contract:png:png_apng_sequence_counter_exhaustion",
        "source_case": "decode:png:apng_sequence_controls",
        "point": "png.apng.sequence_counter_exhaustion",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "png_apng_inspection_sequence_counter_exhaustion": {
        "format": "png",
        "category": "structured-diagnostics",
        "selector": "fault-contract:png:png_apng_inspection_sequence_counter_exhaustion",
        "source_case": "decode:png:apng_sequence_controls",
        "point": "png.apng.sequence_counter_exhaustion",
        "contract": "inspection_error_then_retry_succeeds",
        "operation": "inspect",
    },
    "avif_monochrome_tile_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_monochrome_tile_reservation_failure",
        "source_case": "decode:avif:multitile_monochrome_split_groups",
        "point": "av1.frame.monochrome_tile_state_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_tile_group_temporal_sample_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_tile_group_temporal_sample_reservation_failure",
        "source_case": "decode:avif:animated_motion_chroma",
        "point": "av1.frame.tile_group_temporal_sample_reservation",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_retained_temporal_sample_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_retained_temporal_sample_reservation_failure",
        "source_case": "decode:avif:animated_motion_multitile_split_groups",
        "point": "av1.frame.retained_temporal_sample_reservation",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_color_tile_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_color_tile_reservation_failure",
        "source_case": "decode:avif:multitile_color_split_groups",
        "point": "av1.frame.color_tile_state_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_sgr_restoration_output_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_sgr_restoration_output_reservation_failure",
        "source_case": "decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56",
        "point": "av1.restoration.sgr_output_reservation",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_sgr_intermediate_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_sgr_intermediate_reservation_failure",
        "source_case": "decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56",
        "point": "av1.restoration.sgr_intermediate_reservation",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_restoration_stripe_scratch_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_restoration_stripe_scratch_reservation_failure",
        "source_case": "decode:avif:high_bitdepth",
        "point": "av1.restoration.stripe_scratch_reservation",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_restoration_stripe_output_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_restoration_stripe_output_reservation_failure",
        "source_case": "decode:avif:high_bitdepth",
        "point": "av1.restoration.stripe_output_reservation",
        "contract": "sequence_decode_error_then_retry_succeeds",
    },
    "avif_tile_cell_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_tile_cell_reservation_failure",
        "source_case": "decode:avif:baseline",
        "point": "av1.frame.tile_cell_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_tile_block_metadata_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_tile_block_metadata_reservation_failure",
        "source_case": "decode:avif:baseline",
        "point": "av1.frame.tile_block_metadata_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_chroma_tile_cell_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_chroma_tile_cell_reservation_failure",
        "source_case": "decode:avif:baseline",
        "point": "av1.frame.chroma_tile_cell_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_assembled_loop_filter_metadata_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_assembled_loop_filter_metadata_reservation_failure",
        "source_case": "decode:avif:multitile_color_split_groups",
        "point": "av1.frame.assembled_loop_filter_metadata_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_assembled_cdef_region_map_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_assembled_cdef_region_map_reservation_failure",
        "source_case": "decode:avif:multitile_color_split_groups",
        "point": "av1.frame.assembled_cdef_region_map_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_tile_cdef_region_map_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_tile_cdef_region_map_reservation_failure",
        "source_case": "decode:avif:multitile_color_split_groups",
        "point": "av1.tile.cdef_region_map_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_tile_cdef_active_map_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_tile_cdef_active_map_reservation_failure",
        "source_case": "decode:avif:multitile_color_split_groups",
        "point": "av1.tile.cdef_active_map_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_assembled_cdef_active_map_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_assembled_cdef_active_map_reservation_failure",
        "source_case": "decode:avif:multitile_color_split_groups",
        "point": "av1.frame.assembled_cdef_active_map_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_partition_node_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_partition_node_reservation_failure",
        "source_case": "decode:avif:baseline",
        "point": "av1.partition_tree.node_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
    "avif_partition_context_above_reservation_failure": {
        "format": "avif",
        "category": "structured-diagnostics",
        "selector": "fault-contract:avif:avif_partition_context_above_reservation_failure",
        "source_case": "decode:avif:baseline",
        "point": "av1.partition_context.above_reservation",
        "contract": "decode_error_then_retry_succeeds",
    },
}
EXPECTED_CATEGORIES = (
    ("decode-encode-policy-limits", "DecodePolicy and EncodePolicy limits"),
    ("cancellation-work-budgets", "Cancellation and work budgets"),
    ("output-sink-delivery", "OutputSink delivery"),
    ("caller-owned-destination-buffers", "Caller-owned destination buffers"),
    ("source-provenance", "Source provenance"),
    ("structured-diagnostics", "Structured diagnostics"),
    ("feature-target-capability", "Feature and target capability"),
    ("cache-concurrency-api-lifecycle", "Cache/concurrency/API lifecycle"),
    ("release-package-surface", "Release package surface"),
    ("allocation-stack-coverage-models", "Allocation/stack/coverage models"),
)
STATUSES = {"covered", "planned"}
EVIDENCE_KINDS = {"integration_contract", "fixture_verifier"}
TEST_DECLARATION = re.compile(r"#\[test\]\s*(?:\r?\n\s*)+fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")


def fail(message: str) -> None:
    raise RuntimeError(message)


def read_json() -> dict:
    try:
        value = json.loads(MANIFEST.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {MANIFEST}: {error}")
    if not isinstance(value, dict):
        fail(f"{MANIFEST} must contain a JSON object")
    return value


def repository_file(value: object, field: str) -> Path:
    if not isinstance(value, str) or not value:
        fail(f"{field} must be a non-empty repository-relative path")
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts:
        fail(f"{field} must be a repository-relative path: {value!r}")
    path = ROOT / relative
    if not path.is_file():
        fail(f"{field} does not exist: {value}")
    return relative


def string_array(value: object, field: str, *, allow_empty: bool) -> list[str]:
    if not isinstance(value, list) or (not allow_empty and not value):
        fail(f"{field} must be a non-empty string array")
    if any(not isinstance(item, str) or not item for item in value):
        fail(f"{field} must contain only non-empty strings")
    if len(set(value)) != len(value):
        fail(f"{field} must not contain duplicates")
    return value


def verify_test_symbols(path: Path, names: list[str], field: str) -> None:
    source = (ROOT / path).read_text(encoding="utf-8")
    discovered = set(TEST_DECLARATION.findall(source))
    missing = sorted(set(names) - discovered)
    if missing:
        fail(f"{field} names are not #[test] functions in {path}: {missing}")


def active_fault_contract_ids() -> set[str]:
    """Read the fault-contract IDs without adding a YAML runtime dependency."""

    try:
        lines = SOURCE_MANIFEST.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        fail(f"cannot read active fault rows from {SOURCE_MANIFEST}: {error}")

    try:
        section_start = lines.index("fault_contracts:") + 1
    except ValueError:
        fail(f"{SOURCE_MANIFEST} is missing the top-level fault_contracts section")

    case_ids = []
    current_status = None
    in_section = False
    for line in lines[section_start:]:
        if line and not line[0].isspace():
            break
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if line.startswith("  - "):
            if case_ids and current_status != "active":
                fail("every manifest fault contract must have status: active")
            match = re.fullmatch(r"  - id: ([a-z0-9_]+)", line)
            if match is None:
                fail("manifest fault-contract rows must begin with a simple id field")
            case_ids.append(match.group(1))
            current_status = None
            in_section = True
            continue
        if in_section:
            match = re.fullmatch(r"    status: ([a-z]+)", line)
            if match is not None:
                current_status = match.group(1)

    if current_status != "active":
        fail("every manifest fault contract must have status: active")
    if not case_ids:
        fail(f"{SOURCE_MANIFEST} has no active fault-contract IDs")
    if len(case_ids) != len(set(case_ids)):
        fail("manifest fault_contracts contains duplicate active ids")
    return set(case_ids)


def verify_fault_contract_case_index(entries: object) -> None:
    if not isinstance(entries, list):
        fail("fault_contract_cases must be an array")
    if len(entries) != len(FAULT_CONTRACTS):
        fail(
            f"fault_contract_cases must contain exactly {len(FAULT_CONTRACTS)} active fault rows"
        )

    source_id_set = active_fault_contract_ids()
    if source_id_set != set(FAULT_CONTRACTS):
        missing = sorted(source_id_set - set(FAULT_CONTRACTS))
        stale = sorted(set(FAULT_CONTRACTS) - source_id_set)
        fail(
            "verifier fault mappings differ from active manifest rows: "
            f"missing={missing}, stale={stale}"
        )
    try:
        coverage = json.loads(
            (ROOT / "tests" / "fixtures" / "coverage_matrix.json").read_text(
                encoding="utf-8"
            )
        )
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read target-only fault rows from coverage matrix: {error}")
    rows = coverage.get("fault_contracts")
    if not isinstance(rows, list):
        fail("coverage matrix fault_contracts must be an array")
    matrix_rows = {row.get("id"): row for row in rows if isinstance(row, dict)}
    if len(matrix_rows) != len(rows):
        fail("coverage matrix fault_contracts contains duplicate or invalid row ids")
    if set(matrix_rows) != source_id_set:
        missing = sorted(source_id_set - set(matrix_rows))
        unexpected = sorted(set(matrix_rows) - source_id_set)
        fail(
            "coverage matrix fault_contracts differ from active manifest rows: "
            f"missing={missing}, unexpected={unexpected}"
        )

    entry_ids = []
    seen_entry_ids = set()
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict) or set(entry) != FAULT_CONTRACT_CASE_KEYS:
            fail(
                f"fault_contract_cases[{index}] must use exactly {sorted(FAULT_CONTRACT_CASE_KEYS)}"
            )
        case_id = entry.get("id")
        if not isinstance(case_id, str) or case_id in seen_entry_ids:
            fail(f"fault_contract_cases[{index}] has an invalid or duplicate id {case_id!r}")
        entry_ids.append(case_id)
        seen_entry_ids.add(case_id)
        expected = FAULT_CONTRACTS.get(case_id)
        if expected is None or case_id not in matrix_rows:
            fail(f"fault_contract_cases[{index}] names an unknown fault case {case_id!r}")
        for field in ("category", "selector"):
            if entry.get(field) != expected[field]:
                fail(f"{case_id}: {field} differs from its allow-listed mapping")
        if entry.get("test") != "test_fault_contract_matrix":
            fail(f"{case_id}: test must name the shared fault-contract matrix runner")
        verify_test_symbols(
            Path("tests/coverage_matrix_tests.rs"),
            [entry["test"]],
            f"{case_id}.test",
        )

        row = matrix_rows[case_id]
        fault = row.get("fault")
        source = row.get("input")
        if (
            row.get("type") != "fault-contract"
            or row.get("verification") != "fault-contract"
            or row.get("oracle_status") != "not_applicable"
            or row.get("operation") != expected.get("operation", "decode")
            or row.get("status") != "active"
            or row.get("format") != expected["format"]
            or entry.get("selector")
            != f"fault-contract:{row.get('format')}:{case_id}"
            or entry.get("category") not in row.get("requirements", [])
            or not isinstance(source, dict)
            or source.get("source_case") != expected["source_case"]
            or not isinstance(fault, dict)
            or fault.get("point") != expected["point"]
            or fault.get("contract") != expected["contract"]
        ):
            fail(f"{case_id}: shared matrix row violates the target-only fault contract")
        parity_rows = (
            coverage.get("formats", {}).get(expected["format"], {}).get("decode", [])
        )
        if not any(
            candidate.get("id") == expected["source_case"].split(":", 2)[2]
            and candidate.get("status") == "active"
            and candidate.get("oracle_status") == "ok"
            for candidate in parity_rows
        ):
            fail(f"{case_id}: source_case is not a live Pillow parity row")

    if set(entry_ids) != set(FAULT_CONTRACTS):
        missing = sorted(set(FAULT_CONTRACTS) - set(entry_ids))
        unexpected = sorted(set(entry_ids) - set(FAULT_CONTRACTS))
        fail(f"fault_contract_cases differs from the allowlist: missing={missing}, unexpected={unexpected}")

def verify_evidence(entry: object, category_id: str, index: int) -> None:
    if not isinstance(entry, dict):
        fail(f"{category_id}: evidence entry {index} must be an object")
    if set(entry) != EVIDENCE_KEYS:
        fail(
            f"{category_id}: evidence entry {index} keys differ: "
            f"expected {sorted(EVIDENCE_KEYS)}, got {sorted(entry)}"
        )

    kind = entry.get("kind")
    if kind not in EVIDENCE_KINDS:
        fail(f"{category_id}: unsupported evidence kind {kind!r}")
    path = repository_file(entry.get("path"), f"{category_id}.evidence[{index}].path")
    if "coverage_matrix_tests" in path.as_posix():
        fail(f"{category_id}: Pillow parity test path cannot be evidence")
    if "pillow_fixture" in json.dumps(entry, sort_keys=True):
        fail(f"{category_id}: Pillow fixture origin cannot be evidence")

    fixture = entry.get("fixture")
    if fixture is not None:
        fixture_path = repository_file(
            fixture, f"{category_id}.evidence[{index}].fixture"
        )
        if fixture_path.as_posix() == "tests/fixtures/coverage_matrix.json":
            fail(f"{category_id}: Pillow parity matrix cannot be evidence")

    tests = string_array(
        entry.get("tests"),
        f"{category_id}.evidence[{index}].tests",
        allow_empty=kind == "fixture_verifier",
    )
    command = string_array(
        entry.get("command"),
        f"{category_id}.evidence[{index}].command",
        allow_empty=False,
    )
    if any(token in {";", "&&", "||", "|"} for token in command):
        fail(f"{category_id}: evidence command must not contain shell syntax")

    if kind == "integration_contract":
        if not path.as_posix().startswith("tests/") or path.suffix != ".rs":
            fail(f"{category_id}: integration evidence must be a Rust test path")
        if path.as_posix() == "tests/coverage_matrix_tests.rs":
            fail(f"{category_id}: coverage matrix is not a Rust-only contract")
        if entry.get("fixture") is not None and not path.as_posix().startswith("tests/"):
            fail(f"{category_id}: integration fixture path is outside tests")
        if command[0] != "cargo" or "test" not in command:
            fail(f"{category_id}: integration evidence must use cargo test")
        verify_test_symbols(path, tests, f"{category_id}.evidence[{index}].tests")
    else:
        if not path.as_posix().startswith("scripts/") or path.suffix != ".py":
            fail(f"{category_id}: fixture verifier must be a repository script")
        if entry.get("fixture") is None:
            fail(f"{category_id}: fixture verifier must name its fixture")
        if tests:
            fail(f"{category_id}: fixture verifier cannot name Rust tests")
        if command != ["python3", path.as_posix()]:
            fail(
                f"{category_id}: fixture verifier command must invoke its script "
                f"directly"
            )


def markdown_table_cells(line: str) -> list[str] | None:
    stripped = line.strip()
    if not stripped.startswith("|") or not stripped.endswith("|"):
        return None
    return [cell.strip() for cell in stripped[1:-1].split("|")]


def markdown_cell_value(cell: str) -> str:
    if len(cell) >= 2 and cell.startswith("`") and cell.endswith("`"):
        return cell[1:-1]
    return cell


def verify_catalog_table(document_text: str, categories: list[dict]) -> None:
    """Keep the human-facing catalog tied to the machine-facing manifest."""

    expected_ids = {category_id for category_id, _ in EXPECTED_CATEGORIES}
    rows: dict[str, list[str]] = {}
    for line in document_text.splitlines():
        cells = markdown_table_cells(line)
        if cells is None or len(cells) != 6:
            continue
        category_id = markdown_cell_value(cells[0])
        if category_id not in expected_ids:
            continue
        if category_id in rows:
            fail(f"{DOCUMENT} contains duplicate catalog table row for `{category_id}`")
        rows[category_id] = cells

    missing = sorted(expected_ids - set(rows))
    if missing:
        fail(f"{DOCUMENT} is missing catalog table rows: {missing}")

    by_id = {category["id"]: category for category in categories}
    labels = dict(EXPECTED_CATEGORIES)
    for category_id in expected_ids:
        category = by_id[category_id]
        cells = rows[category_id]
        if cells[1].replace("`", "") != labels[category_id]:
            fail(
                f"{DOCUMENT} catalog label for `{category_id}` does not match "
                "the manifest"
            )
        if markdown_cell_value(cells[3]) != category["status"]:
            fail(
                f"{DOCUMENT} status for `{category_id}` does not match the "
                "manifest"
            )
        if markdown_cell_value(cells[5]) != category["pillow_parity"]:
            fail(
                f"{DOCUMENT} Pillow-parity status for `{category_id}` does not "
                "match the manifest"
            )

        evidence_cell = cells[4]
        if category_id == "structured-diagnostics":
            documented_selectors = re.findall(r"`(fault-contract:[^`]+)`", evidence_cell)
            expected_selectors = {
                fault["selector"] for fault in FAULT_CONTRACTS.values()
            }
            if len(documented_selectors) != len(set(documented_selectors)):
                fail(f"{DOCUMENT} repeats a target-only fault selector")
            if set(documented_selectors) != expected_selectors:
                missing = sorted(expected_selectors - set(documented_selectors))
                unexpected = sorted(set(documented_selectors) - expected_selectors)
                fail(
                    f"{DOCUMENT} structured-diagnostics fault selectors differ: "
                    f"missing={missing}, unexpected={unexpected}"
                )
        if category["status"] == "covered":
            for evidence in category["evidence"]:
                path = evidence["path"]
                if f"`{path}`" not in evidence_cell:
                    fail(
                        f"{DOCUMENT} evidence cell for `{category_id}` omits "
                        f"manifest path `{path}`"
                    )
        else:
            marker = "No category-specific evidence is claimed"
            if marker not in evidence_cell:
                fail(
                    f"{DOCUMENT} planned evidence cell for `{category_id}` must "
                    f"say `{marker}`"
                )
            for path in category["planned_context"]:
                if f"`{path}`" not in evidence_cell:
                    fail(
                        f"{DOCUMENT} planned context for `{category_id}` omits "
                        f"manifest path `{path}`"
                    )


def verify_document(document_text: str, categories: list[dict]) -> None:
    required = (
        "tests/fixtures/unreachable_contract_manifest.json",
        "make verify",
        "Pillow parity",
        "fault-contract",
        "excluded",
    )
    missing = [phrase for phrase in required if phrase not in document_text]
    if missing:
        fail(f"{DOCUMENT} is missing mapping references: {missing}")
    plain_document = document_text.replace("`", "")
    for category_id, label in EXPECTED_CATEGORIES:
        if document_text.count(f"`{category_id}`") != 1:
            fail(f"{DOCUMENT} must contain exactly one mapping row for `{category_id}`")
        if label not in plain_document:
            fail(f"{DOCUMENT} is missing catalog label {label!r}")
    verify_catalog_table(document_text, categories)


def verify() -> tuple[int, int]:
    document = read_json()
    if set(document) != TOP_LEVEL_KEYS:
        fail(
            f"manifest keys differ: expected {sorted(TOP_LEVEL_KEYS)}, "
            f"got {sorted(document)}"
        )
    if document.get("format_version") != 1:
        fail("unreachable contract manifest format_version must be 1")
    if document.get("scope") != "pillow_unreachable_contract_v1":
        fail("unreachable contract manifest scope must be pillow_unreachable_contract_v1")

    parity = document.get("pillow_parity")
    if not isinstance(parity, dict) or set(parity) != PARITY_KEYS:
        fail("pillow_parity must have exactly status, matrix, and reason")
    if parity.get("status") != "excluded":
        fail("pillow_parity status must be excluded")
    if parity.get("matrix") != "tests/fixtures/coverage_matrix.json":
        fail("pillow_parity matrix must identify the canonical Pillow matrix")
    repository_file(parity.get("matrix"), "pillow_parity.matrix")
    if not isinstance(parity.get("reason"), str) or "Pillow" not in parity["reason"]:
        fail("pillow_parity reason must explain the Pillow boundary")
    verify_fault_contract_case_index(document.get("fault_contract_cases"))

    categories = document.get("categories")
    if not isinstance(categories, list) or len(categories) != len(EXPECTED_CATEGORIES):
        fail(f"categories must contain exactly {len(EXPECTED_CATEGORIES)} entries")
    expected_ids = [category_id for category_id, _ in EXPECTED_CATEGORIES]
    actual_ids = [category.get("id") if isinstance(category, dict) else None for category in categories]
    if actual_ids != expected_ids:
        fail(f"category IDs differ: expected {expected_ids}, got {actual_ids}")

    covered = 0
    for category, (expected_id, expected_label) in zip(categories, EXPECTED_CATEGORIES):
        if not isinstance(category, dict):
            fail(f"{expected_id}: category must be an object")
        if set(category) != CATEGORY_KEYS:
            fail(
                f"{expected_id}: category keys differ: expected {sorted(CATEGORY_KEYS)}, "
                f"got {sorted(category)}"
            )
        if category.get("label") != expected_label:
            fail(f"{expected_id}: label does not match the canonical catalog")
        if category.get("status") not in STATUSES:
            fail(f"{expected_id}: status must be covered or planned")
        if category.get("pillow_parity") != "excluded":
            fail(f"{expected_id}: Pillow parity must be explicitly excluded")
        if not isinstance(category.get("scope"), str) or not category["scope"]:
            fail(f"{expected_id}: scope must be a non-empty string")

        evidence = category.get("evidence")
        if not isinstance(evidence, list):
            fail(f"{expected_id}: evidence must be an array")
        evidence_keys: set[tuple[str, str]] = set()
        for index, entry in enumerate(evidence):
            verify_evidence(entry, expected_id, index)
            evidence_key = (entry["kind"], entry["path"])
            if evidence_key in evidence_keys:
                fail(f"{expected_id}: duplicate evidence path {evidence_key}")
            evidence_keys.add(evidence_key)

        planned_reason = category.get("planned_reason")
        planned_context = category.get("planned_context")
        if not isinstance(planned_context, list):
            fail(f"{expected_id}: planned_context must be an array")
        if category["status"] == "covered":
            covered += 1
            if not evidence:
                fail(f"{expected_id}: covered category must name evidence")
            if planned_reason is not None or planned_context:
                fail(f"{expected_id}: covered category cannot carry planned fields")
        else:
            if evidence:
                fail(f"{expected_id}: planned category cannot claim evidence")
            if not isinstance(planned_reason, str) or not planned_reason:
                fail(f"{expected_id}: planned category needs a reason")
            if not planned_context:
                fail(f"{expected_id}: planned category needs bounded context paths")
            for index, path in enumerate(planned_context):
                repository_file(path, f"{expected_id}.planned_context[{index}]")

    try:
        verify_document(DOCUMENT.read_text(encoding="utf-8"), categories)
    except OSError as error:
        fail(f"cannot read {DOCUMENT}: {error}")
    return len(categories), covered


def main() -> int:
    try:
        total, covered = verify()
    except RuntimeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        f"unreachable contract mapping OK: {covered}/{total} categories have "
        "existing Rust evidence; remaining categories are explicitly planned; "
        "Pillow parity excluded"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
