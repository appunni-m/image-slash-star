#include <avif/avif.h>

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static uint32_t mix32(uint32_t value)
{
    value ^= value >> 16;
    value *= 0x7feb352dU;
    value ^= value >> 15;
    value *= 0x846ca68bU;
    value ^= value >> 16;
    return value;
}

static uint16_t sample_value(unsigned frame, unsigned x, unsigned y, int plane)
{
    const unsigned pixel_x = plane == AVIF_CHAN_Y ? x : 2U * x;
    const unsigned pixel_y = plane == AVIF_CHAN_Y ? y : 2U * y;
    const unsigned local_x = (pixel_x + (frame == 0 ? 0U : 4U)) % 64U;
    const unsigned band = local_x / 8U;
    const unsigned tile = (pixel_y / 8U) & 7U;
    const uint32_t code = mix32(band * 991U + tile * 113U + (unsigned)plane * 503U);
    uint16_t value = plane == AVIF_CHAN_Y
        ? (uint16_t)(180U + code % 680U)
        : (uint16_t)(420U + code % 180U);

    const unsigned local_block_x = pixel_x % 128U;
    const unsigned local_block_y = pixel_y % 128U;
    const unsigned marker_x = frame == 0 ? 16U : 28U;
    if (plane == AVIF_CHAN_Y && local_block_x >= marker_x && local_block_x < marker_x + 20U &&
        local_block_y >= 20U && local_block_y < 44U) {
        value = 940U;
    }
    return value;
}

static int fill_frame(avifImage *image, unsigned frame)
{
    if (avifImageAllocatePlanes(image, AVIF_PLANES_YUV) != AVIF_RESULT_OK) {
        return 0;
    }
    for (int plane = AVIF_CHAN_Y; plane <= AVIF_CHAN_V; ++plane) {
        uint8_t *const data = avifImagePlane(image, (avifChannelIndex)plane);
        const unsigned width = avifImagePlaneWidth(image, (avifChannelIndex)plane);
        const unsigned height = avifImagePlaneHeight(image, (avifChannelIndex)plane);
        const unsigned row_bytes = avifImagePlaneRowBytes(image, (avifChannelIndex)plane);
        for (unsigned y = 0; y < height; ++y) {
            for (unsigned x = 0; x < width; ++x) {
                const uint16_t value = sample_value(frame, x, y, plane);
                uint8_t *const pixel = data + (size_t)y * row_bytes + 2U * x;
                pixel[0] = (uint8_t)(value & 0xffU);
                pixel[1] = (uint8_t)(value >> 8);
            }
        }
    }
    return 1;
}

int main(int argc, char **argv)
{
    if (argc != 2) {
        fprintf(stderr, "usage: %s OUTPUT.avif\n", argv[0]);
        return 2;
    }
    avifEncoder *encoder = avifEncoderCreate();
    avifImage *image = avifImageCreate(128, 128, 10, AVIF_PIXEL_FORMAT_YUV420);
    avifRWData output = AVIF_DATA_EMPTY;
    int status = 1;
    if (encoder == NULL || image == NULL) {
        fprintf(stderr, "failed to allocate the AVIF encoder or image\n");
        goto cleanup;
    }
    image->yuvRange = AVIF_RANGE_FULL;
    image->colorPrimaries = AVIF_COLOR_PRIMARIES_BT709;
    image->transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_SRGB;
    image->matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_BT601;
    encoder->codecChoice = AVIF_CODEC_CHOICE_AOM;
    encoder->maxThreads = 1;
    encoder->quality = 75;
    encoder->qualityAlpha = 75;
    encoder->speed = 0;
    encoder->autoTiling = AVIF_FALSE;
    encoder->timescale = 1000;
    encoder->repetitionCount = AVIF_REPETITION_COUNT_INFINITE;

    const char *const options[][2] = {
        {"sb-size", "128"},
        {"min-partition-size", "128"},
        {"max-partition-size", "128"},
        {"aq-mode", "0"},
        {"deltaq-mode", "0"},
        {"enable-cdef", "0"},
        {"enable-restoration", "1"},
        {"loopfilter-control", "0"},
        {"enable-warped-motion", "0"},
        {"enable-tx-size-search", "0"},
    };
    for (size_t i = 0; i < sizeof(options) / sizeof(options[0]); ++i) {
        const avifResult result = avifEncoderSetCodecSpecificOption(encoder, options[i][0], options[i][1]);
        if (result != AVIF_RESULT_OK) {
            fprintf(stderr, "option %s failed: %s\n", options[i][0], avifResultToString(result));
            goto cleanup;
        }
    }

    for (unsigned frame = 0; frame < 2; ++frame) {
        if (!fill_frame(image, frame)) {
            fprintf(stderr, "failed to allocate image planes\n");
            goto cleanup;
        }
        const avifResult result = avifEncoderAddImage(encoder, image, 100, AVIF_ADD_IMAGE_FLAG_NONE);
        if (result != AVIF_RESULT_OK) {
            fprintf(stderr, "frame %u failed: %s (%s)\n", frame, avifResultToString(result), encoder->diag.error);
            goto cleanup;
        }
    }
    {
        const avifResult result = avifEncoderFinish(encoder, &output);
        if (result != AVIF_RESULT_OK) {
            fprintf(stderr, "encoding failed: %s (%s)\n", avifResultToString(result), encoder->diag.error);
            goto cleanup;
        }
    }
    FILE *const file = fopen(argv[1], "wb");
    if (file == NULL) {
        fprintf(stderr, "could not open output\n");
        goto cleanup;
    }
    const int write_ok = fwrite(output.data, 1, output.size, file) == output.size;
    const int close_ok = fclose(file) == 0;
    if (!write_ok || !close_ok) {
        fprintf(stderr, "could not write output\n");
        goto cleanup;
    }
    status = 0;

cleanup:
    avifRWDataFree(&output);
    avifImageDestroy(image);
    avifEncoderDestroy(encoder);
    return status;
}
