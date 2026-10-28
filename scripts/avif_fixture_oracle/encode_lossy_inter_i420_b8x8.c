#include <avif/avif.h>

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

static uint32_t hash32(uint32_t value)
{
    value ^= value >> 16;
    value *= 0x7feb352dU;
    value ^= value >> 15;
    value *= 0x846ca68bU;
    value ^= value >> 16;
    return value;
}

static uint8_t sample_value(unsigned frame, unsigned x, unsigned y, int plane)
{
    // A smooth top band and textured rows keep the inter frame in select mode.
    if (y < 16U) {
        const unsigned base = plane == AVIF_CHAN_Y ? 48U :
                              plane == AVIF_CHAN_U ? 96U : 144U;
        const unsigned frame_delta = frame == 0 ? 0U :
                                     plane == AVIF_CHAN_Y ? 12U : 6U;
        return (uint8_t)(base + x / 2U + y / 2U + frame_delta);
    }
    const unsigned source_x = (x + (frame == 0 ? 0 : 3)) & 63U;
    const unsigned source_y = (y + (frame == 0 ? 0 : 2)) & 63U;
    const unsigned cell_x = source_x / (plane == AVIF_CHAN_Y ? 4 : 2);
    const unsigned cell_y = source_y / 4;
    const uint32_t code = hash32(cell_x * 991U + cell_y * 113U +
                                 (unsigned)plane * 503U);
    return (uint8_t)(24 + code % 208);
}

static void fill_frame(avifImage *image, unsigned frame)
{
    for (int plane = AVIF_CHAN_Y; plane <= AVIF_CHAN_V; ++plane) {
        uint8_t *const data = avifImagePlane(image, (avifChannelIndex)plane);
        const unsigned width = avifImagePlaneWidth(image, (avifChannelIndex)plane);
        const unsigned height = avifImagePlaneHeight(image, (avifChannelIndex)plane);
        const unsigned row_bytes = avifImagePlaneRowBytes(image, (avifChannelIndex)plane);
        for (unsigned y = 0; y < height; ++y) {
            for (unsigned x = 0; x < width; ++x) {
                const uint16_t sample = (uint16_t)(
                    sample_value(frame, x, y, plane));
                uint8_t *const destination =
                    data + (size_t)y * row_bytes + (size_t)x;
                *destination = (uint8_t)sample;
            }
        }
    }
}

int main(int argc, char **argv)
{
    if (argc != 2) {
        fprintf(stderr, "usage: encode_lossy_inter_i420_b8x8 OUTPUT.avif\n");
        return 2;
    }
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
    encoder->quality = 70;
    encoder->qualityAlpha = 70;
    encoder->speed = 0;
    encoder->autoTiling = AVIF_FALSE;
    encoder->timescale = 1000;
    encoder->repetitionCount = 0;
    const char *const options[][2] = {
        {"sb-size", "64"},
        {"min-partition-size", "16"},
        {"max-partition-size", "64"},
        {"enable-rect-partitions", "1"},
        {"enable-ab-partitions", "0"},
        {"enable-1to4-partitions", "1"},
        {"enable-tx-size-search", "1"},
        {"enable-global-motion", "0"},
        {"aq-mode", "0"},
        {"deltaq-mode", "0"},
        {"enable-cdef", "0"},
        {"enable-restoration", "0"},
        {"loopfilter-control", "0"},
        {"enable-warped-motion", "0"},
    };
    for (size_t index = 0; index < sizeof(options) / sizeof(options[0]); ++index) {
        const avifResult result =
            avifEncoderSetCodecSpecificOption(encoder, options[index][0], options[index][1]);
        if (result != AVIF_RESULT_OK) {
            fprintf(stderr, "option %s failed: %s\n", options[index][0],
                    avifResultToString(result));
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
    const avifResult result = avifEncoderFinish(encoder, &output);
    if (result != AVIF_RESULT_OK) {
        fprintf(stderr, "encoding failed: %s (%s)\n", avifResultToString(result),
                encoder->diag.error);
        goto cleanup;
    }
    FILE *const file = fopen(argv[1], "wb");
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
