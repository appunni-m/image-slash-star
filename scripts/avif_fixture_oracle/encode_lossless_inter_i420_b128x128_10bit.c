/* Generate a pinned 10-bit lossless I420 sequence with 128x128 inter blocks. */
#include <avif/avif.h>

#include <stdint.h>
#include <stdio.h>

enum { FRAME_WIDTH = 128, FRAME_HEIGHT = 128, BIT_DEPTH = 10 };

static uint16_t sample_value(unsigned frame, int plane, unsigned x, unsigned y) {
  uint16_t value = plane == AVIF_CHAN_Y ? 300 : 512;
  const unsigned pixel_x = plane == AVIF_CHAN_Y ? x : 2 * x;
  const unsigned pixel_y = plane == AVIF_CHAN_Y ? y : 2 * y;
  const unsigned patch_x = 8 + frame;
  const unsigned patch_y = 8 + frame;
  if (pixel_x >= patch_x && pixel_x < patch_x + 8 &&
      pixel_y >= patch_y && pixel_y < patch_y + 8) {
    if (plane == AVIF_CHAN_Y) {
      value = (uint16_t)(700 + (x + 3 * y) % 80);
    } else {
      value = plane == AVIF_CHAN_U ? 350 : 650;
    }
  }
  return value;
}

static int fill_frame(avifImage *image, unsigned frame) {
  for (int plane = AVIF_CHAN_Y; plane <= AVIF_CHAN_V; ++plane) {
    uint8_t *data = avifImagePlane(image, (avifChannelIndex)plane);
    const unsigned width = avifImagePlaneWidth(image, (avifChannelIndex)plane);
    const unsigned height = avifImagePlaneHeight(image, (avifChannelIndex)plane);
    const unsigned row_bytes = avifImagePlaneRowBytes(image, (avifChannelIndex)plane);
    for (unsigned y = 0; y < height; ++y) {
      for (unsigned x = 0; x < width; ++x) {
        const uint16_t value = sample_value(frame, plane, x, y);
        uint8_t *pixel = data + y * row_bytes + 2 * x;
        pixel[0] = (uint8_t)(value & 0xff);
        pixel[1] = (uint8_t)(value >> 8);
      }
    }
  }
  return 1;
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

int main(int argc, char **argv) {
  if (argc != 2) {
    fprintf(stderr,
            "usage: encode_lossless_inter_i420_b128x128_10bit OUTPUT.avif\n");
    return 2;
  }

  int status = 0;
  avifEncoder *encoder = avifEncoderCreate();
  avifImage *image = avifImageCreate(FRAME_WIDTH, FRAME_HEIGHT, BIT_DEPTH,
                                     AVIF_PIXEL_FORMAT_YUV420);
  avifRWData output = AVIF_DATA_EMPTY;
  FILE *file = NULL;
  if (encoder == NULL || image == NULL) {
    fprintf(stderr, "failed to allocate the encoder or image\n");
    status = 1;
    goto cleanup;
  }
  if (avifImageAllocatePlanes(image, AVIF_PLANES_YUV) != AVIF_RESULT_OK) {
    fprintf(stderr, "failed to allocate I420 planes\n");
    status = 1;
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
  encoder->speed = 8;
  encoder->autoTiling = AVIF_FALSE;
  encoder->timescale = 1000;
  encoder->repetitionCount = AVIF_REPETITION_COUNT_INFINITE;
  encoder->minQuantizer = 0;
  encoder->maxQuantizer = 0;

  if (!set_option(encoder, "sb-size", "128") ||
      !set_option(encoder, "min-partition-size", "128") ||
      !set_option(encoder, "max-partition-size", "128")) {
    status = 1;
    goto cleanup;
  }

  for (unsigned frame = 0; frame < 2; ++frame) {
    if (!fill_frame(image, frame)) {
      status = 1;
      goto cleanup;
    }
    const avifResult result =
        avifEncoderAddImage(encoder, image, 100, AVIF_ADD_IMAGE_FLAG_NONE);
    if (result != AVIF_RESULT_OK) {
      fprintf(stderr, "adding frame %u failed: %s (%s)\n", frame,
              avifResultToString(result), encoder->diag.error);
      status = 1;
      goto cleanup;
    }
  }

  {
    const avifResult result = avifEncoderFinish(encoder, &output);
    if (result != AVIF_RESULT_OK) {
      fprintf(stderr, "encoding failed: %s (%s)\n", avifResultToString(result),
              encoder->diag.error);
      status = 1;
      goto cleanup;
    }
  }

  file = fopen(argv[1], "wb");
  if (file == NULL || fwrite(output.data, 1, output.size, file) != output.size) {
    fprintf(stderr, "failed to write AVIF output\n");
    status = 1;
    goto cleanup;
  }
  if (fclose(file) != 0) {
    file = NULL;
    fprintf(stderr, "failed to close AVIF output\n");
    status = 1;
    goto cleanup;
  }
  file = NULL;

cleanup:
  if (file != NULL) {
    fclose(file);
  }
  avifRWDataFree(&output);
  avifImageDestroy(image);
  avifEncoderDestroy(encoder);
  return status;
}
