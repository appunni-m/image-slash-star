/* Included after the pinned bitstream.c headers; observation only. */
#include "aom_dsp/oracle_semantic.h"
#include <stdlib.h>

static void semantic_field(aom_writer *w, const char *phase, const char *name) {
  FILE *f = oracle_semantic(w->oracle_id, phase);
  fprintf(f, ",\"field\":\"%s\"", name);
  oracle_semantic_end();
}

static void semantic_tile(const AV1_COMMON *cm, const MACROBLOCKD *xd,
                           const TileInfo *tile, aom_writer *w) {
  (void)oracle_generation(cm);
  oracle_cdf_reset(w->oracle_id);
  const FRAME_CONTEXT *fc = xd->tile_ctx;
#define COUNT(a) ((unsigned)(sizeof(a) / sizeof((a)[0])))
#define REG1(a) do { unsigned s[] = { COUNT(fc->a) }; oracle_cdf_register(#a, &fc->a, sizeof(fc->a), 1, s); } while (0)
#define REG2(a) do { unsigned s[] = { COUNT(fc->a), COUNT(fc->a[0]) }; oracle_cdf_register(#a, &fc->a, sizeof(fc->a), 2, s); } while (0)
#define REG3(a) do { unsigned s[] = { COUNT(fc->a), COUNT(fc->a[0]), COUNT(fc->a[0][0]) }; oracle_cdf_register(#a, &fc->a, sizeof(fc->a), 3, s); } while (0)
#define REG4(a) do { unsigned s[] = { COUNT(fc->a), COUNT(fc->a[0]), COUNT(fc->a[0][0]), COUNT(fc->a[0][0][0]) }; oracle_cdf_register(#a, &fc->a, sizeof(fc->a), 4, s); } while (0)
  REG3(txb_skip_cdf); REG4(eob_extra_cdf); REG3(dc_sign_cdf);
  REG3(eob_flag_cdf16); REG3(eob_flag_cdf32); REG3(eob_flag_cdf64);
  REG3(eob_flag_cdf128); REG3(eob_flag_cdf256); REG3(eob_flag_cdf512); REG3(eob_flag_cdf1024);
  REG4(coeff_base_eob_cdf); REG4(coeff_base_cdf); REG4(coeff_br_cdf);
  REG2(skip_txfm_cdfs); REG2(filter_intra_cdfs); REG1(filter_intra_mode_cdf);
  REG3(uv_mode_cdf); REG2(partition_cdf); REG3(kf_y_cdf); REG2(angle_delta_cdf);
  REG3(tx_size_cdf); REG1(delta_q_cdf); REG4(intra_ext_tx_cdf);
  REG1(cfl_sign_cdf); REG2(cfl_alpha_cdf); REG1(intrabc_cdf);
#undef REG4
#undef REG3
#undef REG2
#undef REG1
#undef COUNT
  FILE *f = oracle_semantic(w->oracle_id, "tile_begin");
  fprintf(f, ",\"visible\":[%d,%d],\"mi_grid\":[%d,%d],\"tile\":[%d,%d],"
          "\"mi_bounds\":[%d,%d,%d,%d],\"sb_size\":%d,\"sb_mi\":%d,"
          "\"base_q\":%d,\"delta_q_present\":%d,\"delta_q_res\":%d,"
          "\"tx_mode\":%d,\"coded_lossless\":%d,\"reduced_tx_set\":%d,"
          "\"allow_screen_content\":%d,\"allow_intrabc\":%d,\"cdf_update\":%d,"
          "\"segmentation_enabled\":%d,\"cdef_bits\":%d,\"planes\":%d",
          cm->width, cm->height, cm->mi_params.mi_cols, cm->mi_params.mi_rows,
          tile->tile_col, tile->tile_row, tile->mi_col_start, tile->mi_col_end,
          tile->mi_row_start, tile->mi_row_end, cm->seq_params->sb_size, cm->seq_params->mib_size,
          cm->quant_params.base_qindex, cm->delta_q_info.delta_q_present_flag, cm->delta_q_info.delta_q_res,
          cm->features.tx_mode, cm->features.coded_lossless, cm->features.reduced_tx_set_used,
          cm->features.allow_screen_content_tools, cm->features.allow_intrabc, w->allow_update_cdf,
          cm->seg.enabled, cm->cdef_info.cdef_bits, av1_num_planes(cm));
  oracle_semantic_end();
  if (!frame_is_intra_only(cm) || cm->seg.enabled || cm->delta_q_info.delta_lf_present_flag) abort();
  for (int p = 0; p < av1_num_planes(cm); ++p)
    if (cm->rst_info[p].frame_restoration_type != RESTORE_NONE) abort();
}

static void semantic_neighbor(FILE *f, const MB_MODE_INFO *mbmi, int available) {
  if (!available || !mbmi) { fputs("null", f); return; }
  fprintf(f, "{\"mode\":%d,\"skip\":%d,\"block_size\":%d,\"tx_size\":%d}",
          mbmi->mode, mbmi->skip_txfm, mbmi->bsize, mbmi->tx_size);
}

static void semantic_leaf(const AV1_COMMON *cm, const MACROBLOCKD *xd, aom_writer *w) {
  const MB_MODE_INFO *m = xd->mi[0];
  FILE *f = oracle_semantic(w->oracle_id, "leaf_begin");
  fprintf(f, ",\"mi\":[%d,%d],\"block_size\":%d,\"block_pixels\":[%d,%d],"
          "\"segment\":%u,\"skip\":%u,\"mode\":%d,\"tx_size\":%d,\"lossless\":%d,"
          "\"is_chroma_ref\":%d,\"use_intrabc\":%d,\"current_q\":%d,"
          "\"palette_sizes\":[%u,%u],\"above\":",
          xd->mi_col, xd->mi_row, m->bsize, block_size_wide[m->bsize], block_size_high[m->bsize],
          m->segment_id, m->skip_txfm, m->mode, m->tx_size, xd->lossless[m->segment_id],
          xd->is_chroma_ref, m->use_intrabc, m->current_qindex,
          m->palette_mode_info.palette_size[0], m->palette_mode_info.palette_size[1]);
  semantic_neighbor(f, xd->above_mbmi, xd->up_available);
  fputs(",\"left\":", f);
  semantic_neighbor(f, xd->left_mbmi, xd->left_available);
  fprintf(f, ",\"edges\":[%d,%d],\"subsampling\":[%d,%d],\"y_angle_delta\":",
          xd->mb_to_right_edge, xd->mb_to_bottom_edge,
          xd->plane[1].subsampling_x, xd->plane[1].subsampling_y);
  if (av1_use_angle_delta(m->bsize) && av1_is_directional_mode(m->mode)) fprintf(f, "%d", m->angle_delta[0]);
  else fputs("null", f);
  fputs(",\"filter_intra\":", f);
  if (av1_filter_intra_allowed(cm, m)) {
    fprintf(f, "{\"enabled\":%d,\"mode\":", m->filter_intra_mode_info.use_filter_intra);
    if (m->filter_intra_mode_info.use_filter_intra) fprintf(f, "%d", m->filter_intra_mode_info.filter_intra_mode);
    else fputs("null", f);
    fputc('}', f);
  } else fputs("null", f);
  fputs(",\"chroma\":", f);
  if (!cm->seq_params->monochrome && xd->is_chroma_ref) {
    fprintf(f, "{\"mode\":%d,\"cfl_allowed\":%d,\"angle_delta\":", m->uv_mode, is_cfl_allowed(xd));
    if (av1_use_angle_delta(m->bsize) && av1_is_directional_mode(get_uv_mode(m->uv_mode))) fprintf(f, "%d", m->angle_delta[1]);
    else fputs("null", f);
    fputs(",\"cfl\":", f);
    if (m->uv_mode == UV_CFL_PRED) fprintf(f, "{\"index\":%u,\"signs\":%d}", m->cfl_alpha_idx, m->cfl_alpha_signs);
    else fputs("null", f);
    fputc('}', f);
  } else fputs("null", f);
  if (m->skip_txfm) fprintf(f, ",\"reset_origin\":%u", oracle_find_origin(cm, m, 2));
  oracle_semantic_end();
  if (m->use_intrabc || m->palette_mode_info.palette_size[0] || m->palette_mode_info.palette_size[1]) abort();
}

static void semantic_partition(const AV1_COMMON *cm, const MACROBLOCKD *xd,
                                aom_writer *w, int row, int col, BLOCK_SIZE size,
                                BLOCK_SIZE subsize, PARTITION_TYPE partition) {
  int half = mi_size_wide[size] / 2;
  int rows = row + half < cm->mi_params.mi_rows, cols = col + half < cm->mi_params.mi_cols;
  FILE *f = oracle_semantic(w->oracle_id, "partition");
  fprintf(f, ",\"mi\":[%d,%d],\"block_size\":%d,\"subsize\":%d,\"partition\":%d,"
          "\"has_rows\":%d,\"has_cols\":%d,\"context\":", col, row, size, subsize, partition, rows, cols);
  if (size >= BLOCK_8X8) fprintf(f, "%d", partition_plane_context(xd, row, col, size)); else fputs("null", f);
  fprintf(f, ",\"syntax\":\"%s\"", size < BLOCK_8X8 || (!rows && !cols) ? "forced" : rows && cols ? "adaptive" : "raw");
  oracle_semantic_end();
}
