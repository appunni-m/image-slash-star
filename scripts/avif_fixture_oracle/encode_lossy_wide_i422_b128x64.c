#include <avif/avif.h>

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static uint32_t hash32(uint32_t value)
{
    value ^= value >> 16;
    value *= 0x7feb352dU;
    value ^= value >> 15;
    value *= 0x846ca68bU;
    value ^= value >> 16;
    return value;
}

static uint8_t sample_value(unsigned frame, unsigned pixel_x, unsigned pixel_y, int plane)
{
    const unsigned half = pixel_x / 64;
    const unsigned local_x = pixel_x % 64;
    const int shift = half == 0 ? 8 : -8;
    const unsigned source_x = frame == 0
                                  ? local_x
                                  : (unsigned)((int)local_x - shift + 64) % 64;
    const unsigned band = (source_x / 8) & 7;
    const unsigned tile = (pixel_y / 8) & 15;
    const uint32_t code = hash32((half + 1) * 991U + band * 37U + tile * 113U +
                                 (unsigned)plane * 503U);
    return (uint8_t)(24 + code % 208);
}

static int fill_frame(avifImage *image, unsigned frame)
{
    for (int plane = AVIF_CHAN_Y; plane <= AVIF_CHAN_V; ++plane) {
        uint8_t *const data = avifImagePlane(image, (avifChannelIndex)plane);
        const unsigned width = avifImagePlaneWidth(image, (avifChannelIndex)plane);
        const unsigned height = avifImagePlaneHeight(image, (avifChannelIndex)plane);
        const unsigned row_bytes = avifImagePlaneRowBytes(image, (avifChannelIndex)plane);
        for (unsigned y = 0; y < height; ++y) {
            for (unsigned x = 0; x < width; ++x) {
                const unsigned pixel_x = plane == AVIF_CHAN_Y ? x : x * 2;
                data[(size_t)y * row_bytes + x] = sample_value(frame, pixel_x, y, plane);
            }
        }
    }
    return 1;
}

static int write_sequence(const char *path)
{
    avifEncoder *encoder = avifEncoderCreate();
    avifImage *image = avifImageCreate(128, 128, 8, AVIF_PIXEL_FORMAT_YUV422);
    avifRWData output = AVIF_DATA_EMPTY;
    int success = 0;
    if (encoder == NULL || image == NULL) {
        fprintf(stderr, "failed to allocate the encoder or I422 frame\n");
        goto cleanup;
    }
    if (avifImageAllocatePlanes(image, AVIF_PLANES_YUV) != AVIF_RESULT_OK) {
        fprintf(stderr, "failed to allocate I422 planes\n");
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
    encoder->repetitionCount = 0;

    const char *const options[][2] = {
        {"sb-size", "128"},
        {"min-partition-size", "64"},
        {"max-partition-size", "128"},
        {"enable-rect-partitions", "1"},
        {"enable-ab-partitions", "0"},
        {"enable-1to4-partitions", "0"},
        {"enable-tx-size-search", "0"},
        {"enable-global-motion", "0"},
        {"aq-mode", "0"},
        {"deltaq-mode", "0"},
        {"enable-cdef", "0"},
        {"enable-restoration", "0"},
        {"loopfilter-control", "0"},
        {"enable-warped-motion", "0"},
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

    FILE *const file = fopen(path, "wb");
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
    return success;
}

int main(int argc, char **argv)
{
    if (argc != 2) {
        fprintf(stderr, "usage: encode_lossy_wide_i422_b128x64 OUTPUT.avif\n");
        return 2;
    }
    return write_sequence(argv[1]) ? 0 : 1;
}
