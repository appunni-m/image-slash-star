#include <avif/avif.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

static void fill_luma(avifImage *image, unsigned frame)
{
    uint8_t *plane = avifImagePlane(image, AVIF_CHAN_Y);
    const size_t stride = avifImagePlaneRowBytes(image, AVIF_CHAN_Y);

    for (unsigned y = 0; y < image->height; ++y) {
        for (unsigned x = 0; x < image->width; ++x) {
            const unsigned band = ((x / 16U) ^ (y / 32U)) * 43U;
            plane[(size_t)y * stride + x] =
                (uint8_t)((x * 31U + y * 17U + band + (x * y) / 13U + frame * 7U) & 255U);
        }
    }
}

int main(int argc, char **argv)
{
    unsigned width = 256;
    unsigned height = 256;
    const char *output_path = NULL;

    if (argc == 2) {
        output_path = argv[1];
    } else if (argc == 3 && strcmp(argv[1], "--tile-limit") == 0) {
        width = 4097;
        height = 1;
        output_path = argv[2];
    } else {
        fprintf(stderr, "usage: %s [--tile-limit] OUTPUT.avif\n", argv[0]);
        return 2;
    }

    avifImage *image = avifImageCreate(width, height, 8, AVIF_PIXEL_FORMAT_YUV400);
    avifEncoder *encoder = avifEncoderCreate();
    avifRWData output = AVIF_DATA_EMPTY;
    int status = 1;

    if (image == NULL || encoder == NULL ||
        avifImageAllocatePlanes(image, AVIF_PLANES_YUV) != AVIF_RESULT_OK) {
        fprintf(stderr, "failed to allocate image planes or encoder\n");
        goto cleanup;
    }

    image->yuvRange = AVIF_RANGE_FULL;
    image->colorPrimaries = AVIF_COLOR_PRIMARIES_BT709;
    image->transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_SRGB;
    image->matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_BT709;
    encoder->codecChoice = AVIF_CODEC_CHOICE_AOM;
    encoder->maxThreads = 1;
    encoder->quality = 100;
    encoder->speed = 8;
    encoder->autoTiling = AVIF_FALSE;
    encoder->tileColsLog2 = 0;
    encoder->tileRowsLog2 = 0;
    encoder->minQuantizer = 0;
    encoder->maxQuantizer = 0;

    fill_luma(image, 0);
    avifResult result = avifEncoderAddImage(
        encoder, image, 100, AVIF_ADD_IMAGE_FLAG_SINGLE);
    if (result != AVIF_RESULT_OK) {
        fprintf(stderr, "add image failed: %s (%s)\n",
                avifResultToString(result), encoder->diag.error);
        goto cleanup;
    }

    result = avifEncoderFinish(encoder, &output);
    if (result != AVIF_RESULT_OK) {
        fprintf(stderr, "finish failed: %s (%s)\n",
                avifResultToString(result), encoder->diag.error);
        goto cleanup;
    }

    FILE *file = fopen(output_path, "wb");
    if (file == NULL) {
        perror("fopen");
        goto cleanup;
    }

    const size_t written = fwrite(output.data, 1, output.size, file);
    const int close_status = fclose(file);
    if (written != output.size || close_status != 0) {
        fprintf(stderr, "failed to write complete AVIF output\n");
        goto cleanup;
    }

    fprintf(stderr, "wrote %zu bytes\n", output.size);
    status = 0;

cleanup:
    avifRWDataFree(&output);
    avifImageDestroy(image);
    avifEncoderDestroy(encoder);
    return status;
}
