#include <avif/avif.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

static void print_version(void)
{
    char codec_versions[256];
    avifCodecVersions(codec_versions);
    printf("libavif %s (%s)\n", avifVersion(), codec_versions);
}

int main(int argc, char **argv)
{
    if (argc == 2 && strcmp(argv[1], "--version") == 0) {
        print_version();
        return 0;
    }
    if (argc != 2) {
        fprintf(stderr, "usage: %s OUTPUT.avif\n", argv[0]);
        return 2;
    }

    avifImage *image = avifImageCreate(128, 128, 8, AVIF_PIXEL_FORMAT_YUV420);
    avifEncoder *encoder = avifEncoderCreate();
    avifRWData output = AVIF_DATA_EMPTY;
    int success = 0;
    if (image == NULL || encoder == NULL ||
        avifImageAllocatePlanes(image, AVIF_PLANES_YUV) != AVIF_RESULT_OK) {
        goto cleanup;
    }

    image->yuvRange = AVIF_RANGE_FULL;
    image->colorPrimaries = AVIF_COLOR_PRIMARIES_BT709;
    image->transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_SRGB;
    image->matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_BT601;
    encoder->codecChoice = AVIF_CODEC_CHOICE_AOM;
    encoder->maxThreads = 1;
    encoder->quality = 75;
    encoder->speed = 6;
    encoder->autoTiling = AVIF_FALSE;
    encoder->tileColsLog2 = 0;
    encoder->tileRowsLog2 = 0;

    static const char *const options[][2] = {
        {"min-partition-size", "8"},
        {"max-partition-size", "32"},
        {"loopfilter-control", "1"},
        {"sharpness", "7"},
        {"enable-cdef", "0"},
        {"enable-restoration", "0"},
        {"superres-mode", "0"},
    };
    for (size_t index = 0; index < sizeof(options) / sizeof(options[0]); ++index) {
        if (avifEncoderSetCodecSpecificOption(encoder, options[index][0], options[index][1]) !=
            AVIF_RESULT_OK) {
            fprintf(stderr, "option %s rejected: %s\n", options[index][0], encoder->diag.error);
            goto cleanup;
        }
    }

    for (int plane = AVIF_CHAN_Y; plane <= AVIF_CHAN_V; ++plane) {
        uint8_t *data = avifImagePlane(image, (avifChannelIndex)plane);
        const unsigned width = avifImagePlaneWidth(image, (avifChannelIndex)plane);
        const unsigned height = avifImagePlaneHeight(image, (avifChannelIndex)plane);
        const size_t stride = avifImagePlaneRowBytes(image, (avifChannelIndex)plane);
        for (unsigned y = 0; y < height; ++y) {
            for (unsigned x = 0; x < width; ++x) {
                const unsigned block = ((x / 8) + (y / 8) + (unsigned)plane) & 1U;
                data[(size_t)y * stride + x] =
                    (uint8_t)(48U + block * 96U +
                              ((x * 17U + y * 11U + (unsigned)plane * 13U) & 31U));
            }
        }
    }

    avifResult result = avifEncoderAddImage(
        encoder, image, 1, AVIF_ADD_IMAGE_FLAG_SINGLE);
    if (result != AVIF_RESULT_OK) {
        fprintf(stderr, "add image: %s (%s)\n", avifResultToString(result), encoder->diag.error);
        goto cleanup;
    }
    result = avifEncoderFinish(encoder, &output);
    if (result != AVIF_RESULT_OK) {
        fprintf(stderr, "finish: %s (%s)\n", avifResultToString(result), encoder->diag.error);
        goto cleanup;
    }

    FILE *file = fopen(argv[1], "wb");
    if (file == NULL) {
        goto cleanup;
    }
    const size_t written = fwrite(output.data, 1, output.size, file);
    const int closed = fclose(file);
    if (written != output.size || closed != 0) {
        goto cleanup;
    }
    fprintf(stderr, "wrote %zu bytes\n", output.size);
    success = 1;

cleanup:
    avifRWDataFree(&output);
    avifImageDestroy(image);
    avifEncoderDestroy(encoder);
    return success ? 0 : 1;
}
