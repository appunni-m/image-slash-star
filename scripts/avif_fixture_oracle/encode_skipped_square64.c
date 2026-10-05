/* Fixture-only AV1 tile writer using unmodified pinned libaom primitives. */
#include "aom_dsp/bitwriter.h"
#include "av1/common/entropy.h"
#include "av1/common/entropymode.h"
#include <stdio.h>
#include <string.h>

int main(int argc, char **argv) {
  if (argc != 3 || (strcmp(argv[1], "color") &&
                    strcmp(argv[1], "monochrome"))) {
    fprintf(stderr, "usage: encode_skipped_square64 color|monochrome output\n");
    return 1;
  }
  FRAME_CONTEXT context;
  memset(&context, 0, sizeof(context));
  av1_init_mode_probs(&context);
  /* One complete 64x64 superblock at a tile origin. No neighbors, screen
     tools, selected transform sizes, restoration, or residual tokens. The
     frame header signals a largest-transform, lossy key frame with CDEF. */
  /* At most four nonzero-probability Q15 symbols plus range termination:
     4 * 15 + 32 bits fits comfortably in this fixed output buffer. */
  unsigned char bytes[64];
  aom_writer writer;
  memset(&writer, 0, sizeof(writer));
  writer.allow_update_cdf = 1;
  aom_start_encode(&writer, bytes);
  aom_write_symbol(&writer, PARTITION_NONE, context.partition_cdf[12],
                   EXT_PARTITION_TYPES);
  aom_write_symbol(&writer, 1, context.skip_txfm_cdfs[0], 2);
  /* skip_txfm suppresses CDEF-index and full-superblock delta-Q sentences. */
  aom_write_symbol(&writer, DC_PRED, context.kf_y_cdf[0][0], INTRA_MODES);
  if (!strcmp(argv[1], "color")) {
    /* A 64x64 luma partition cannot use CfL. */
    aom_write_symbol(&writer, UV_DC_PRED,
                     context.uv_mode_cdf[CFL_DISALLOWED][DC_PRED],
                     UV_INTRA_MODES - 1);
  }
  if (aom_stop_encode(&writer) < 0 || writer.pos > sizeof(bytes)) return 2;
  FILE *output = fopen(argv[2], "wb");
  if (!output) return 3;
  const size_t written = fwrite(bytes, 1, writer.pos, output);
  const int close_status = fclose(output);
  return written == writer.pos && !close_status ? 0 : 4;
}
