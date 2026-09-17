/* Independent native fixture harness. Prepared planes are little-endian u16. */
#include <avif/avif.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
  if (argc != 11) return 1;
  const unsigned width = (unsigned)atoi(argv[3]), height = (unsigned)atoi(argv[4]);
  const unsigned depth = (unsigned)atoi(argv[5]);
  const int format = atoi(argv[6]), alpha = atoi(argv[7]);
  if (!width || !height || width > 128 || height > 64 ||
      (depth != 8 && depth != 10 && depth != 12) || format < 1 || format > 4)
    return 2;
  avifImage *image = avifImageCreate(width, height, depth, (avifPixelFormat)format);
  avifEncoder *encoder = avifEncoderCreate();
  if (!image || !encoder) return 3;
  image->yuvRange = AVIF_RANGE_FULL;
  image->colorPrimaries = AVIF_COLOR_PRIMARIES_BT709;
  image->transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_SRGB;
  image->matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_BT601;
  if (avifImageAllocatePlanes(image, alpha ? AVIF_PLANES_ALL : AVIF_PLANES_YUV)) return 4;
  FILE *input = fopen(argv[1], "rb");
  if (!input) return 5;
  for (int plane = 0; plane < 4; ++plane) {
    uint8_t *data = avifImagePlane(image, plane);
    if (!data) continue;
    for (unsigned y = 0; y < avifImagePlaneHeight(image, plane); ++y) {
      for (unsigned x = 0; x < avifImagePlaneWidth(image, plane); ++x) {
        int lo = fgetc(input), hi = fgetc(input);
        if (lo == EOF || hi == EOF) return 6;
        uint16_t value = (uint16_t)(lo | (hi << 8));
        if (value >= (1u << depth)) return 7;
        uint8_t *pixel = data + y * avifImagePlaneRowBytes(image, plane) + x * (depth > 8 ? 2 : 1);
        if (depth > 8) memcpy(pixel, &value, 2); else *pixel = (uint8_t)value;
      }
    }
  }
  if (fgetc(input) != EOF || ferror(input) || fclose(input)) return 8;
  encoder->codecChoice = AVIF_CODEC_CHOICE_AOM;
  encoder->maxThreads = 1;
  encoder->quality = atoi(argv[8]);
  encoder->qualityAlpha = encoder->quality;
  encoder->speed = atoi(argv[9]);
  encoder->tileColsLog2 = atoi(argv[10]);
  encoder->tileRowsLog2 = 0;
  encoder->autoTiling = AVIF_FALSE;
  avifRWData output = AVIF_DATA_EMPTY;
  avifResult result = avifEncoderWrite(encoder, image, &output);
  if (result != AVIF_RESULT_OK) {
    fprintf(stderr, "%s: %s\n", avifResultToString(result), encoder->diag.error);
    return 9;
  }
  FILE *file = fopen(argv[2], "wb");
  if (!file || fwrite(output.data, 1, output.size, file) != output.size || fclose(file)) return 10;
  avifRWDataFree(&output);
  avifEncoderDestroy(encoder);
  avifImageDestroy(image);
  return 0;
}
