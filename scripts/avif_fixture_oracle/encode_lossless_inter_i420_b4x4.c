/* Generate an all-lossless 4:2:0 sequence with 4x4 AV1 inter blocks. */
#include <avif/avif.h>

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

enum { FRAME_WIDTH = 64, FRAME_HEIGHT = 64 };

static uint8_t luma_sample(unsigned frame, unsigned x, unsigned y) {
  const unsigned base = 32U + (x * 13U + y * 7U + x * y * 3U) % 96U;
  const unsigned patch_x = frame == 0 ? 28U : 29U;
  if (x >= patch_x && x < patch_x + 4U && y >= 28U && y < 32U) {
    return (uint8_t)(base + 96U);
  }
  return (uint8_t)base;
}

static void fill_frame(avifImage *image, unsigned frame) {
  uint8_t *const luma = avifImagePlane(image, AVIF_CHAN_Y);
  uint8_t *const chroma_u = avifImagePlane(image, AVIF_CHAN_U);
  uint8_t *const chroma_v = avifImagePlane(image, AVIF_CHAN_V);
  const unsigned luma_stride = avifImagePlaneRowBytes(image, AVIF_CHAN_Y);
  const unsigned chroma_stride = avifImagePlaneRowBytes(image, AVIF_CHAN_U);
  const unsigned chroma_width = avifImagePlaneWidth(image, AVIF_CHAN_U);
  const unsigned chroma_height = avifImagePlaneHeight(image, AVIF_CHAN_U);

  for (unsigned y = 0; y < FRAME_HEIGHT; ++y) {
    for (unsigned x = 0; x < FRAME_WIDTH; ++x) {
      luma[(size_t)y * luma_stride + x] = luma_sample(frame, x, y);
    }
  }
  for (unsigned y = 0; y < chroma_height; ++y) {
    for (unsigned x = 0; x < chroma_width; ++x) {
      const size_t offset = (size_t)y * chroma_stride + x;
      chroma_u[offset] = (uint8_t)(100U + y + x);
      chroma_v[offset] = (uint8_t)(150U + y + x);
    }
  }
}

static int set_option(avifEncoder *encoder, const char *name,
                      const char *value) {
  const avifResult result = avifEncoderSetCodecSpecificOption(encoder, name, value);
  if (result == AVIF_RESULT_OK) {
    return 1;
  }
  fprintf(stderr, "codec option %s=%s failed: %s\n", name, value,
          avifResultToString(result));
  return 0;
}

static int write_sequence(const char *path) {
  static const char *const options[][2] = {
      {"sb-size", "64"},
      {"min-partition-size", "4"},
      {"max-partition-size", "4"},
      {"enable-rect-partitions", "0"},
      {"enable-ab-partitions", "0"},
      {"enable-1to4-partitions", "0"},
      {"enable-tx-size-search", "0"},
      {"aq-mode", "0"},
      {"deltaq-mode", "0"},
      {"enable-cdef", "0"},
      {"enable-restoration", "0"},
      {"loopfilter-control", "0"},
      {"enable-warped-motion", "0"},
      {"enable-global-motion", "0"},
      {"enable-interintra-comp", "0"},
  };
  avifEncoder *encoder = avifEncoderCreate();
  avifImage *image = avifImageCreate(FRAME_WIDTH, FRAME_HEIGHT, 8,
                                     AVIF_PIXEL_FORMAT_YUV420);
  avifRWData output = AVIF_DATA_EMPTY;
  FILE *file = NULL;
  int status = 1;

  if (encoder == NULL || image == NULL) {
    fprintf(stderr, "failed to allocate the AVIF encoder or image\n");
    goto cleanup;
  }
  if (avifImageAllocatePlanes(image, AVIF_PLANES_YUV) != AVIF_RESULT_OK) {
    fprintf(stderr, "failed to allocate I420 planes\n");
    goto cleanup;
  }

  image->yuvRange = AVIF_RANGE_FULL;
  image->colorPrimaries = AVIF_COLOR_PRIMARIES_BT709;
  image->transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_SRGB;
  image->matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_BT601;
  encoder->codecChoice = AVIF_CODEC_CHOICE_AOM;
  encoder->maxThreads = 1;
  encoder->quality = 100;
  encoder->qualityAlpha = 100;
  encoder->speed = 0;
  encoder->autoTiling = AVIF_FALSE;
  encoder->tileColsLog2 = 0;
  encoder->tileRowsLog2 = 0;
  encoder->timescale = 1000;
  encoder->repetitionCount = AVIF_REPETITION_COUNT_INFINITE;
  encoder->minQuantizer = 0;
  encoder->maxQuantizer = 0;

  for (size_t index = 0; index < sizeof(options) / sizeof(options[0]); ++index) {
    if (!set_option(encoder, options[index][0], options[index][1])) {
      goto cleanup;
    }
  }
  for (unsigned frame = 0; frame < 2; ++frame) {
    fill_frame(image, frame);
    const avifResult result =
        avifEncoderAddImage(encoder, image, 100, AVIF_ADD_IMAGE_FLAG_NONE);
    if (result != AVIF_RESULT_OK) {
      fprintf(stderr, "adding frame %u failed: %s (%s)\n", frame,
              avifResultToString(result), encoder->diag.error);
      goto cleanup;
    }
  }

  {
    const avifResult result = avifEncoderFinish(encoder, &output);
    if (result != AVIF_RESULT_OK) {
      fprintf(stderr, "encoding failed: %s (%s)\n",
              avifResultToString(result), encoder->diag.error);
      goto cleanup;
    }
  }

  file = fopen(path, "wb");
  if (file == NULL || fwrite(output.data, 1, output.size, file) != output.size) {
    fprintf(stderr, "failed to write sequence to %s\n", path);
    goto cleanup;
  }
  if (fclose(file) != 0) {
    file = NULL;
    fprintf(stderr, "failed to close sequence %s\n", path);
    goto cleanup;
  }
  file = NULL;
  fprintf(stderr, "wrote %zu bytes\n", output.size);
  status = 0;

cleanup:
  if (file != NULL) {
    fclose(file);
  }
  avifRWDataFree(&output);
  avifImageDestroy(image);
  avifEncoderDestroy(encoder);
  return status;
}

int main(int argc, char **argv) {
  if (argc != 2) {
    fprintf(stderr, "usage: encode_lossless_inter_i420_b4x4 OUTPUT.avif\n");
    return 2;
  }
  return write_sequence(argv[1]);
}
