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

static uint16_t sample_value(unsigned frame, unsigned x, unsigned y, int plane)
{
    const unsigned stripe = x / 16U;
    const uint32_t seed = x * 977U + y * 313U + (unsigned)plane * 1901U;
    const int texture = (int)(hash32(seed) % 25U) - 12;
    const int plane_offset = plane == AVIF_CHAN_Y ? 0 :
                             plane == AVIF_CHAN_U ? 240 : -240;
    int value = 160 + (int)stripe * 180 + plane_offset + texture;
    if (frame != 0 && stripe == 1U) {
        value += plane == AVIF_CHAN_Y ? 72 : plane == AVIF_CHAN_U ? 28 : -28;
    }
    if (value < 0) value = 0;
    if (value > 1023) value = 1023;
    return (uint16_t)value;
}

static void fill_frame(avifImage *image, unsigned frame)
{
    for (int plane = AVIF_CHAN_Y; plane <= AVIF_CHAN_V; ++plane) {
        uint8_t *const data = avifImagePlane(image, (avifChannelIndex)plane);
        const unsigned width = avifImagePlaneWidth(image, (avifChannelIndex)plane);
        const unsigned height = avifImagePlaneHeight(image, (avifChannelIndex)plane);
        const unsigned row_bytes = avifImagePlaneRowBytes(image, (avifChannelIndex)plane);
        for (unsigned y = 0; y < height; ++y) {
            uint16_t *const row = (uint16_t *)(data + (size_t)y * row_bytes);
            for (unsigned x = 0; x < width; ++x) {
                row[x] = sample_value(frame, x, y, plane);
            }
        }
    }
}

int main(int argc, char **argv)
{
    if (argc != 2) {
        fprintf(stderr,
                "usage: encode_lossy_inter_i444_b16x64_mode2 OUTPUT.avif\n");
        return 2;
    }
    avifEncoder *const encoder = avifEncoderCreate();
    avifImage *const image = avifImageCreate(64, 64, 10, AVIF_PIXEL_FORMAT_YUV444);
    avifRWData output = AVIF_DATA_EMPTY;
    int success = 0;
    if (encoder == NULL || image == NULL) {
        fprintf(stderr, "failed to allocate encoder or 10-bit I444 frame\n");
        goto cleanup;
    }
    if (avifImageAllocatePlanes(image, AVIF_PLANES_YUV) != AVIF_RESULT_OK) {
        fprintf(stderr, "failed to allocate I444 planes\n");
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
        {"enable-warped-motion", "0"},
        {"enable-masked-comp", "0"},
        {"enable-dist-wtd-comp", "0"},
        {"enable-interintra-comp", "0"},
        {"enable-diff-wtd-comp", "0"},
        {"enable-obmc", "0"},
        {"enable-ref-frame-mvs", "0"},
        {"enable-intrabc", "0"},
        {"aq-mode", "0"},
        {"deltaq-mode", "0"},
        {"enable-cdef", "0"},
        {"enable-restoration", "0"},
        {"loopfilter-control", "0"},
    };
    for (size_t index = 0; index < sizeof(options) / sizeof(options[0]); ++index) {
        const avifResult result = avifEncoderSetCodecSpecificOption(
            encoder, options[index][0], options[index][1]);
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
