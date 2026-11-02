#include <avif/avif.h>

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static uint32_t hash32(uint32_t value)
{
    value ^= value >> 16;
    value *= 0x7feb352dU;
    value ^= value >> 15;
    value *= 0x846ca68bU;
    value ^= value >> 16;
    return value;
}

static uint8_t sample_value(unsigned frame, unsigned x, unsigned y,
                            unsigned plane, int topology_pattern)
{
    unsigned source_x = x & 31U;
    unsigned source_y = y & 31U;
    if (frame != 0) {
        source_x = (source_x + 2U) & 31U;
        source_y = (source_y + (topology_pattern ? 1U : 2U)) & 31U;
    }
    const unsigned cell_x = source_x / (topology_pattern ? 2U : 4U);
    const unsigned cell_y = source_y / (topology_pattern ? 8U : 4U);
    const unsigned region = x / 32U;
    const uint32_t key = cell_x * 991U + cell_y * 113U + region * 10007U +
                         plane * 503U + (topology_pattern ? 7919U : 0U);
    return (uint8_t)(24U + hash32(key) % 208U);
}

static void fill_frame(avifImage *image, unsigned frame, int topology_pattern)
{
    for (int plane = AVIF_CHAN_Y; plane <= AVIF_CHAN_V; ++plane) {
        uint8_t *const data =
            avifImagePlane(image, (avifChannelIndex)plane);
        const unsigned width =
            avifImagePlaneWidth(image, (avifChannelIndex)plane);
        const unsigned height =
            avifImagePlaneHeight(image, (avifChannelIndex)plane);
        const unsigned row_bytes =
            avifImagePlaneRowBytes(image, (avifChannelIndex)plane);
        for (unsigned y = 0; y < height; ++y) {
            for (unsigned x = 0; x < width; ++x) {
                data[(size_t)y * row_bytes + x] =
                    sample_value(frame, x, y, (unsigned)plane, topology_pattern);
            }
        }
    }
}

int main(int argc, char **argv)
{
    if (argc != 3 ||
        (strcmp(argv[1], "deep") != 0 && strcmp(argv[1], "topology") != 0)) {
        fprintf(stderr,
                "usage: encode_lossy_inter_i420_b32x64 deep|topology OUTPUT.avif\n");
        return 2;
    }
    const int topology_pattern = strcmp(argv[1], "topology") == 0;

    avifEncoder *const encoder = avifEncoderCreate();
    avifImage *const image = avifImageCreate(64, 64, 8, AVIF_PIXEL_FORMAT_YUV420);
    avifRWData output = AVIF_DATA_EMPTY;
    int success = 0;
    if (encoder == NULL || image == NULL) {
        fprintf(stderr, "failed to allocate encoder or I420 frame\n");
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
    encoder->quality = 65;
    encoder->qualityAlpha = 65;
    encoder->speed = 0;
    encoder->autoTiling = AVIF_FALSE;
    encoder->timescale = 1000;
    encoder->repetitionCount = AVIF_REPETITION_COUNT_INFINITE;

    const char *const options[][2] = {
        {"sb-size", "64"},
        {"min-partition-size", "32"},
        {"max-partition-size", "64"},
        {"enable-rect-partitions", "1"},
        {"enable-ab-partitions", "0"},
        {"enable-1to4-partitions", "0"},
        {"enable-tx-size-search", "1"},
        {"enable-global-motion", "0"},
        {"aq-mode", "0"},
        {"deltaq-mode", "0"},
        {"enable-cdef", "0"},
        {"enable-restoration", "0"},
        {"loopfilter-control", "0"},
        {"enable-warped-motion", "0"},
    };
    for (size_t index = 0; index < sizeof(options) / sizeof(options[0]);
         ++index) {
        const avifResult result = avifEncoderSetCodecSpecificOption(
            encoder, options[index][0], options[index][1]);
        if (result != AVIF_RESULT_OK) {
            fprintf(stderr, "option %s failed: %s\n", options[index][0],
                    avifResultToString(result));
            goto cleanup;
        }
    }

    for (unsigned frame = 0; frame < 2; ++frame) {
        fill_frame(image, frame, topology_pattern);
        const avifResult result =
            avifEncoderAddImage(encoder, image, 100, AVIF_ADD_IMAGE_FLAG_NONE);
        if (result != AVIF_RESULT_OK) {
            fprintf(stderr, "adding frame %u failed: %s (%s)\n", frame,
                    avifResultToString(result), encoder->diag.error);
            goto cleanup;
        }
    }

    const avifResult result = avifEncoderFinish(encoder, &output);
    if (result != AVIF_RESULT_OK) {
        fprintf(stderr, "encoding failed: %s (%s)\n",
                avifResultToString(result), encoder->diag.error);
        goto cleanup;
    }

    FILE *const file = fopen(argv[2], "wb");
    if (file == NULL) {
        fprintf(stderr, "could not open output path\n");
        goto cleanup;
    }
    const int write_ok = fwrite(output.data, 1, output.size, file) == output.size;
    const int close_ok = fclose(file) == 0;
    if (!write_ok || !close_ok) {
        fprintf(stderr, "could not write output sequence\n");
        goto cleanup;
    }
    success = 1;

cleanup:
    avifRWDataFree(&output);
    avifImageDestroy(image);
    avifEncoderDestroy(encoder);
    return success ? 0 : 1;
}
