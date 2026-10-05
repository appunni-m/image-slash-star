/* Generate the 10-bit I420 sequence used by the public AVIF decode matrix. */
#include <avif/avif.h>

#include <errno.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int fill_frame(avifImage *image, unsigned frame, unsigned patch_origin_x,
                      unsigned patch_origin_y, unsigned patch_width,
                      unsigned patch_height, int patch_pattern) {
  if (avifImageAllocatePlanes(image, AVIF_PLANES_YUV) != AVIF_RESULT_OK) {
    return 0;
  }
  for (int plane = AVIF_CHAN_Y; plane <= AVIF_CHAN_V; ++plane) {
    uint8_t *data = avifImagePlane(image, plane);
    const unsigned width = avifImagePlaneWidth(image, plane);
    const unsigned height = avifImagePlaneHeight(image, plane);
    const unsigned row_bytes = avifImagePlaneRowBytes(image, plane);
    for (unsigned y = 0; y < height; ++y) {
      for (unsigned x = 0; x < width; ++x) {
        uint16_t value = plane == AVIF_CHAN_Y ? 300 : 512;
        const unsigned pixel_x = plane == AVIF_CHAN_Y ? x : 2 * x;
        const unsigned pixel_y = plane == AVIF_CHAN_Y ? y : 2 * y;
        const unsigned patch_x = patch_origin_x + frame;
        const unsigned patch_y = patch_origin_y + frame;
        if (pixel_x >= patch_x && pixel_x < patch_x + patch_width &&
            pixel_y >= patch_y && pixel_y < patch_y + patch_height) {
          if (plane == AVIF_CHAN_Y) {
            const unsigned local_x = pixel_x - patch_x;
            const unsigned local_y = pixel_y - patch_y;
            if (patch_pattern == 1) {
              value = 700;
            } else if (patch_pattern == 2) {
              uint32_t texture = local_x * 0x9e3779b1U ^ local_y * 0x85ebca77U;
              texture ^= texture >> 16;
              texture *= 0x7feb352dU;
              texture ^= texture >> 15;
              value = (uint16_t)(192 + (texture & 511));
            } else {
              value = (uint16_t)(700 + (x + 3 * y) % 80);
            }
          } else {
            value = plane == AVIF_CHAN_U ? 350 : 650;
          }
        }
        uint8_t *pixel = data + y * row_bytes + 2 * x;
        pixel[0] = (uint8_t)(value & 0xff);
        pixel[1] = (uint8_t)(value >> 8);
      }
    }
  }
  return 1;
}

static int parse_unsigned(const char *text, unsigned *value) {
  char *end = NULL;
  errno = 0;
  const unsigned long parsed = strtoul(text, &end, 10);
  if (errno != 0 || end == text || *end != '\0' || parsed > UINT_MAX) {
    return 0;
  }
  *value = (unsigned)parsed;
  return 1;
}

int main(int argc, char **argv) {
  if (argc != 2 && argc != 5 && argc != 6 && argc != 8 && argc != 10 &&
      argc != 11) {
    fprintf(stderr,
            "usage: %s OUTPUT.avif [DIMENSION SPEED PARTITION_SIZE] "
            "or [WIDTH HEIGHT SPEED PARTITION_SIZE [PATCH_X PATCH_Y "
            "[PATCH_WIDTH PATCH_HEIGHT [ramp|flat|texture]]]]\n",
            argv[0]);
    return 1;
  }

  unsigned width = 32;
  unsigned height = 32;
  unsigned speed = 8;
  unsigned partition_size = 0;
  unsigned patch_origin_x = 8;
  unsigned patch_origin_y = 8;
  unsigned patch_width = 8;
  unsigned patch_height = 8;
  int patch_pattern = 0;
  const int parse_square =
      argc == 5 &&
      (!parse_unsigned(argv[2], &width) || !parse_unsigned(argv[3], &speed) ||
       !parse_unsigned(argv[4], &partition_size));
  const int parse_rectangle =
      (argc == 6 || argc == 8 || argc == 10 || argc == 11) &&
      (!parse_unsigned(argv[2], &width) || !parse_unsigned(argv[3], &height) ||
       !parse_unsigned(argv[4], &speed) ||
       !parse_unsigned(argv[5], &partition_size));
  const int parse_patch_origin =
      (argc == 8 || argc == 10 || argc == 11) &&
      (!parse_unsigned(argv[6], &patch_origin_x) ||
       !parse_unsigned(argv[7], &patch_origin_y));
  const int parse_patch_extent =
      (argc == 10 || argc == 11) &&
      (!parse_unsigned(argv[8], &patch_width) ||
       !parse_unsigned(argv[9], &patch_height));
  const int parse_patch_pattern =
      argc == 11 && strcmp(argv[10], "flat") != 0 &&
      strcmp(argv[10], "ramp") != 0 && strcmp(argv[10], "texture") != 0;
  if (parse_square || parse_rectangle || parse_patch_origin ||
      parse_patch_extent || parse_patch_pattern || width < 9 ||
      height < 9 || width > 64 || height > 64 || width % 4 != 0 ||
      height % 4 != 0 || speed > 10 || patch_width == 0 ||
      patch_height == 0 || patch_width >= width || patch_height >= height ||
      patch_origin_x > width - patch_width - 1 ||
      patch_origin_y > height - patch_height - 1 ||
      (partition_size != 0 && partition_size != 32)) {
    fprintf(stderr, "invalid dimensions, speed, or AV1 partition size\n");
    return 1;
  }
  if (argc == 5) {
    height = width;
  }
  if (argc == 11) {
    patch_pattern = strcmp(argv[10], "flat") == 0
                        ? 1
                        : (strcmp(argv[10], "texture") == 0 ? 2 : 0);
  }

  int status = 0;
  avifEncoder *encoder = avifEncoderCreate();
  avifImage *image = avifImageCreate(width, height, 10, AVIF_PIXEL_FORMAT_YUV420);
  avifRWData output = AVIF_DATA_EMPTY;
  FILE *file = NULL;
  if (!encoder || !image) {
    status = 2;
    goto cleanup;
  }

  encoder->codecChoice = AVIF_CODEC_CHOICE_AOM;
  encoder->maxThreads = 1;
  encoder->quality = 100;
  encoder->qualityAlpha = 100;
  encoder->speed = (int)speed;
  encoder->tileColsLog2 = 0;
  encoder->tileRowsLog2 = 0;
  encoder->autoTiling = AVIF_FALSE;
  encoder->timescale = 1000;
  encoder->repetitionCount = AVIF_REPETITION_COUNT_INFINITE;
  encoder->minQuantizer = 0;
  encoder->maxQuantizer = 0;

  if (partition_size != 0) {
    char partition_value[4];
    snprintf(partition_value, sizeof(partition_value), "%u", partition_size);
    if (avifEncoderSetCodecSpecificOption(encoder, "min-partition-size",
                                         partition_value) != AVIF_RESULT_OK ||
        avifEncoderSetCodecSpecificOption(encoder, "max-partition-size",
                                         partition_value) != AVIF_RESULT_OK) {
      fprintf(stderr, "failed to constrain AV1 partition size\n");
      status = 8;
      goto cleanup;
    }
  }

  image->yuvRange = AVIF_RANGE_FULL;
  image->colorPrimaries = AVIF_COLOR_PRIMARIES_BT709;
  image->transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_SRGB;
  image->matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_BT601;
  for (unsigned frame = 0; frame < 2; ++frame) {
    if (!fill_frame(image, frame, patch_origin_x, patch_origin_y, patch_width,
                    patch_height, patch_pattern)) {
      status = 3;
      goto cleanup;
    }
    const avifResult result =
        avifEncoderAddImage(encoder, image, 100, AVIF_ADD_IMAGE_FLAG_NONE);
    if (result != AVIF_RESULT_OK) {
      fprintf(stderr, "AddImage: %s (%s)\n", avifResultToString(result),
              encoder->diag.error);
      status = 4;
      goto cleanup;
    }
  }

  {
    const avifResult result = avifEncoderFinish(encoder, &output);
    if (result != AVIF_RESULT_OK) {
      fprintf(stderr, "Finish: %s (%s)\n", avifResultToString(result),
              encoder->diag.error);
      status = 5;
      goto cleanup;
    }
  }

  file = fopen(argv[1], "wb");
  if (!file || fwrite(output.data, 1, output.size, file) != output.size) {
    status = 6;
    goto cleanup;
  }
  if (fclose(file) != 0) {
    file = NULL;
    status = 7;
    goto cleanup;
  }
  file = NULL;

cleanup:
  if (file) {
    fclose(file);
  }
  avifRWDataFree(&output);
  avifImageDestroy(image);
  avifEncoderDestroy(encoder);
  return status;
}
