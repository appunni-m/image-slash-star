/* Generate a pinned 8-bit lossless I420 sequence with vertical B64x128 leaves. */
#include <avif/avif.h>

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

enum { FRAME_WIDTH = 128, FRAME_HEIGHT = 128 };

static uint8_t sample_value(unsigned frame, int plane, unsigned x, unsigned y) {
  const unsigned pixel_x = plane == AVIF_CHAN_Y ? x : 2 * x;
  const unsigned pixel_y = plane == AVIF_CHAN_Y ? y : 2 * y;
  const unsigned source_x = pixel_x >= frame ? pixel_x - frame : 0;
  const uint32_t pattern = source_x * 73 + pixel_y * 151 +
                           source_x * pixel_y * 13 + (unsigned)plane * 509;
  return (uint8_t)(16 + pattern % 224);
}

static void fill_frame(avifImage *image, unsigned frame) {
  for (int plane = AVIF_CHAN_Y; plane <= AVIF_CHAN_V; ++plane) {
    uint8_t *const data = avifImagePlane(image, (avifChannelIndex)plane);
    const unsigned width = avifImagePlaneWidth(image, (avifChannelIndex)plane);
    const unsigned height = avifImagePlaneHeight(image, (avifChannelIndex)plane);
    const unsigned row_bytes = avifImagePlaneRowBytes(image, (avifChannelIndex)plane);
    for (unsigned y = 0; y < height; ++y) {
      for (unsigned x = 0; x < width; ++x) {
        data[(size_t)y * row_bytes + x] = sample_value(frame, plane, x, y);
      }
    }
  }
}

static int set_option(avifEncoder *encoder, const char *name, const char *value) {
  const avifResult result = avifEncoderSetCodecSpecificOption(encoder, name, value);
  if (result == AVIF_RESULT_OK) {
    return 1;
  }
  fprintf(stderr, "option %s=%s failed: %s\n", name, value,
          avifResultToString(result));
  return 0;
}

static int write_sequence(const char *path) {
  avifEncoder *encoder = avifEncoderCreate();
  avifImage *image = avifImageCreate(FRAME_WIDTH, FRAME_HEIGHT, 8,
                                     AVIF_PIXEL_FORMAT_YUV420);
  avifRWData output = AVIF_DATA_EMPTY;
  FILE *file = NULL;
  int status = 1;
  if (encoder == NULL || image == NULL) {
    fprintf(stderr, "failed to allocate the encoder or I420 frame\n");
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
  encoder->timescale = 1000;
  encoder->repetitionCount = AVIF_REPETITION_COUNT_INFINITE;
  encoder->minQuantizer = 0;
  encoder->maxQuantizer = 0;

  const char *const options[][2] = {
      {"sb-size", "128"},
      {"min-partition-size", "64"},
      {"max-partition-size", "128"},
      {"enable-rect-partitions", "1"},
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
      fprintf(stderr, "encoding failed: %s (%s)\n", avifResultToString(result),
              encoder->diag.error);
      goto cleanup;
    }
  }

  file = fopen(path, "wb");
  if (file == NULL || fwrite(output.data, 1, output.size, file) != output.size) {
    fprintf(stderr, "failed to write output sequence\n");
    goto cleanup;
  }
  if (fclose(file) != 0) {
    file = NULL;
    fprintf(stderr, "failed to close output sequence\n");
    goto cleanup;
  }
  file = NULL;
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
    fprintf(stderr,
            "usage: encode_lossless_inter_i420_b64x128 OUTPUT.avif\n");
    return 2;
  }
  return write_sequence(argv[1]);
}
