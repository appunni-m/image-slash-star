/* Included after pinned encodetxb.c headers. No encoder state is changed. */
#include "aom_dsp/oracle_semantic.h"
#include <stdlib.h>

static const uint16_t *semantic_eob_slot(const MACROBLOCK *x, int plane, int block) {
  return x->cb_coef_buff->eobs[plane] +
      x->mbmi_ext_frame->cb_offset[get_plane_type(plane)] / 16 + block;
}

static unsigned semantic_saved_context(const MACROBLOCK *x, int plane, int block) {
  return x->cb_coef_buff->entropy_ctx[plane][
      x->mbmi_ext_frame->cb_offset[get_plane_type(plane)] / 16 + block];
}

static void semantic_txb_fields(FILE *f, const AV1_COMMON *cm, const MACROBLOCK *x,
                                 int plane, int block, int row, int col, TX_SIZE tx,
                                 unsigned eob, const tran_low_t *coeff) {
  const MACROBLOCKD *xd = &x->e_mbd;
  (void)oracle_generation(cm);
  fprintf(f, ",\"mi\":[%d,%d],\"plane\":%d,\"block\":%d,\"tx_origin\":[%d,%d],"
          "\"tx_size\":%d,\"eob\":%u", xd->mi_col, xd->mi_row, plane, block, col, row, tx, eob);
  if (!eob) return; /* The corresponding coefficient storage need not be initialized. */
  const TX_TYPE type = av1_get_tx_type(xd, get_plane_type(plane), row, col, tx,
                                       cm->features.reduced_tx_set_used);
  int count = av1_get_tx_eob(&cm->seg, xd->mi[0]->segment_id, tx);
  if (count < 1 || count > 1024 || eob > (unsigned)count) abort();
  const int16_t *scan = get_scan(tx, type)->scan;
  fprintf(f, ",\"tx_type\":%d,\"coefficient_shape\":[%d,%d],\"coefficients\":[",
          type, get_txb_wide(tx), get_txb_high(tx));
  for (int i = 0; i < count; ++i) fprintf(f, "%s%d", i ? "," : "", (int)coeff[i]);
  fputs("],\"scan\":[", f);
  for (int i = 0; i < count; ++i) {
    if (scan[i] < 0 || scan[i] >= count) abort();
    fprintf(f, "%s%d", i ? "," : "", scan[i]);
  }
  fputc(']', f);
}

static void semantic_contexts(FILE *f, const MACROBLOCKD *xd, int plane,
                               int row, int col, TX_SIZE tx) {
  const struct macroblockd_plane *pd = &xd->plane[plane];
  fputs("{\"above\":", f);
  oracle_bytes(f, pd->above_entropy_context + col, tx_size_wide_unit[tx]);
  fputs(",\"left\":", f);
  oracle_bytes(f, pd->left_entropy_context + row, tx_size_high_unit[tx]);
  fputc('}', f);
}

static unsigned semantic_analysis_begin(const AV1_COMMON *cm, const MACROBLOCK *x,
                                         int plane, int block, int row, int col,
                                         BLOCK_SIZE plane_size, TX_SIZE tx,
                                         unsigned eob, const tran_low_t *coeff) {
  unsigned id = oracle_next_origin();
  FILE *f = oracle_semantic(0, "analysis_txb");
  fprintf(f, ",\"origin\":%u,\"plane_block_size\":%d", id, plane_size);
  semantic_txb_fields(f, cm, x, plane, block, row, col, tx, eob, coeff);
  fprintf(f, ",\"context_clip\":[%d,%d],\"edges\":[%d,%d],\"before\":",
          max_block_wide(&x->e_mbd, plane_size, plane) - col,
          max_block_high(&x->e_mbd, plane_size, plane) - row,
          x->e_mbd.mb_to_right_edge, x->e_mbd.mb_to_bottom_edge);
  semantic_contexts(f, &x->e_mbd, plane, row, col, tx);
  return id;
}

static void semantic_analysis_selected(int skip, int sign) {
  fprintf(oracle_semantic_output(), ",\"derived_skip_context\":%d,\"derived_dc_sign_context\":%d", skip, sign);
}

static void semantic_analysis_end(unsigned id, const AV1_COMMON *cm, const MACROBLOCK *x,
                                   int plane, int block, int row, int col, TX_SIZE tx) {
  fprintf(oracle_semantic_output(), ",\"saved_context\":%u,\"after\":", semantic_saved_context(x, plane, block));
  semantic_contexts(oracle_semantic_output(), &x->e_mbd, plane, row, col, tx);
  oracle_semantic_end();
  oracle_publish_origin(cm, semantic_eob_slot(x, plane, block), 1, id);
}

static void semantic_packed_txb(const AV1_COMMON *cm, const MACROBLOCK *x, aom_writer *w,
                                 int plane, int block, int row, int col, TX_SIZE tx, unsigned eob) {
  const tran_low_t *coeff = NULL;
  if (eob) coeff = x->cb_coef_buff->tcoeff[plane] +
      x->mbmi_ext_frame->cb_offset[get_plane_type(plane)] + BLOCK_OFFSET(block);
  FILE *f = oracle_semantic(w->oracle_id, "txb_begin");
  fprintf(f, ",\"origin\":%u,\"saved_context\":%u",
          oracle_find_origin(cm, semantic_eob_slot(x, plane, block), 1), semantic_saved_context(x, plane, block));
  semantic_txb_fields(f, cm, x, plane, block, row, col, tx, eob, coeff);
  oracle_semantic_end();
  f = oracle_semantic(w->oracle_id, "field_begin");
  fputs(",\"field\":\"coefficients\"", f);
  oracle_semantic_end();
}

static void semantic_packed_end(aom_writer *w) {
  FILE *f = oracle_semantic(w->oracle_id, "field_end");
  fputs(",\"field\":\"coefficients\"", f);
  oracle_semantic_end();
  (void)oracle_semantic(w->oracle_id, "txb_end");
  oracle_semantic_end();
}

static void semantic_skip_contexts(FILE *f, const AV1_COMMON *cm, const MACROBLOCKD *xd, BLOCK_SIZE size) {
  int planes = 1 + (av1_num_planes(cm) - 1) * xd->is_chroma_ref;
  fputc('[', f);
  for (int plane = 0; plane < planes; ++plane) {
    const struct macroblockd_plane *pd = &xd->plane[plane];
    BLOCK_SIZE plane_size = get_plane_block_size(size, pd->subsampling_x, pd->subsampling_y);
    fprintf(f, "%s{\"plane\":%d,\"above\":", plane ? "," : "", plane);
    oracle_bytes(f, pd->above_entropy_context, mi_size_wide[plane_size]);
    fputs(",\"left\":", f);
    oracle_bytes(f, pd->left_entropy_context, mi_size_high[plane_size]);
    fputc('}', f);
  }
  fputc(']', f);
}

static unsigned semantic_skip_begin(const AV1_COMMON *cm, const MACROBLOCKD *xd, BLOCK_SIZE size) {
  unsigned id = oracle_next_origin();
  FILE *f = oracle_semantic(0, "analysis_skip");
  fprintf(f, ",\"origin\":%u,\"mi\":[%d,%d],\"block_size\":%d,\"before\":", id, xd->mi_col, xd->mi_row, size);
  semantic_skip_contexts(f, cm, xd, size);
  return id;
}

static void semantic_skip_end(unsigned id, const AV1_COMMON *cm, const MACROBLOCKD *xd, BLOCK_SIZE size) {
  fputs(",\"after\":", oracle_semantic_output());
  semantic_skip_contexts(oracle_semantic_output(), cm, xd, size);
  oracle_semantic_end();
  oracle_publish_origin(cm, xd->mi[0], 2, id);
}
