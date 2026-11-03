/* Fixture-only producer: compile with FIXTURE_WIDTH, FIXTURE_HEIGHT,
 * FIXTURE_FORMAT and FIXTURE_QUALITY as documented in README.md. */
#include <avif/avif.h>

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef FIXTURE_ROOT_SIZE
#define FIXTURE_ROOT_SIZE 16
#endif
#if FIXTURE_ROOT_SIZE != 8 && FIXTURE_ROOT_SIZE != 16 && FIXTURE_ROOT_SIZE != 32 && FIXTURE_ROOT_SIZE != 64
#error "narrow inter fixtures require 8x8, 16x16, 32x32, or 64x64 partition roots"
#endif
#ifndef FIXTURE_NARROW_VERTICAL
#define FIXTURE_NARROW_VERTICAL 0
#endif
#if FIXTURE_NARROW_VERTICAL != 0 && FIXTURE_NARROW_VERTICAL != 1
#error "FIXTURE_NARROW_VERTICAL must be 0 or 1"
#endif
#ifndef FIXTURE_ENABLE_CHROMA_DELTAQ
#define FIXTURE_ENABLE_CHROMA_DELTAQ 0
#endif
#if FIXTURE_ENABLE_CHROMA_DELTAQ != 0 && FIXTURE_ENABLE_CHROMA_DELTAQ != 1
#error "FIXTURE_ENABLE_CHROMA_DELTAQ must be 0 or 1"
#endif
#define FIXTURE_STRING_INNER(value) #value
#define FIXTURE_STRING(value) FIXTURE_STRING_INNER(value)

static uint32_t hash32(uint32_t value)
{
    value ^= value >> 16;
    value *= 0x7feb352dU;
    value ^= value >> 15;
    value *= 0x846ca68bU;
    value ^= value >> 16;
    return value;
}

static uint8_t sample_value(unsigned frame, unsigned x, unsigned y, int plane,
                            int mode_select_content, int fine_content)
{
    // The first forced narrow leaf has a low-frequency residual so libaom can
    // keep its maximum transform. The other three leaves retain textured
    // residuals so the frame still signals TX_MODE_SELECT.
    const unsigned first_leaf_extent = FIXTURE_ROOT_SIZE / 4U;
    const int narrow_leaf_is_vertical =
        FIXTURE_NARROW_VERTICAL || FIXTURE_WIDTH < FIXTURE_HEIGHT;
    if (mode_select_content &&
        (narrow_leaf_is_vertical ? x < first_leaf_extent : y < first_leaf_extent)) {
        const unsigned base = plane == AVIF_CHAN_Y ? 48U :
                              plane == AVIF_CHAN_U ? 96U : 144U;
        const unsigned frame_delta = frame == 0 ? 0U :
                                     plane == AVIF_CHAN_Y ? 12U : 6U;
        return (uint8_t)(base + x / 2U + y / 2U + frame_delta);
    }
    const unsigned source_x = (x + (frame == 0 ? 0 : 3)) & 63U;
    const unsigned source_y = (y + (frame == 0 ? 0 : 2)) & 63U;
    const unsigned cell_x = fine_content ? source_x :
                           source_x / (plane == AVIF_CHAN_Y ? 4 : 2);
    const unsigned cell_y = fine_content ? source_y : source_y / 4;
    const uint32_t code = hash32(cell_x * 991U + cell_y * 113U +
                                 (unsigned)plane * 503U);
    if (fine_content) {
        // Per-pixel detail and a bounded second-frame perturbation encourage
        // small residual transforms while preserving temporal correlation.
        const int detail = frame == 0 ? 0 :
            (int)(hash32(x * 271U + y * 911U + (unsigned)plane * 53U) % 31U) - 15;
        return (uint8_t)(32 + (int)(code % 192U) + detail);
    }
    return (uint8_t)(24 + code % 208);
}

static void fill_frame(avifImage *image, unsigned frame, int mode_select_content,
                      unsigned bit_depth, int fine_content)
{
    for (int plane = AVIF_CHAN_Y; plane <= AVIF_CHAN_V; ++plane) {
        uint8_t *const data = avifImagePlane(image, (avifChannelIndex)plane);
        const unsigned width = avifImagePlaneWidth(image, (avifChannelIndex)plane);
        const unsigned height = avifImagePlaneHeight(image, (avifChannelIndex)plane);
        const unsigned row_bytes = avifImagePlaneRowBytes(image, (avifChannelIndex)plane);
        for (unsigned y = 0; y < height; ++y) {
            for (unsigned x = 0; x < width; ++x) {
                const uint16_t sample = (uint16_t)(
                    sample_value(frame, x, y, plane, mode_select_content, fine_content)
                    << (bit_depth - 8));
                uint8_t *const destination =
                    data + (size_t)y * row_bytes + (size_t)x * (bit_depth == 8 ? 1U : 2U);
                if (bit_depth == 8) {
                    *destination = (uint8_t)sample;
                } else {
                    memcpy(destination, &sample, sizeof(sample));
                }
            }
        }
    }
}

int main(int argc, char **argv)
{
    if (argc < 3 || argc > 6 ||
        (strcmp(argv[2], "largest") != 0 && strcmp(argv[2], "select") != 0)) {
        fprintf(stderr, "usage: encode_narrow_inter OUTPUT.avif {largest|select} [8|10|12] [coarse|fine] [0|1|2]\n");
        return 2;
    }
    unsigned bit_depth = 8;
    if (argc >= 4) {
        char *end = NULL;
        const unsigned long parsed = strtoul(argv[3], &end, 10);
        if (end == argv[3] || *end != '\0' ||
            (parsed != 8 && parsed != 10 && parsed != 12)) {
            fprintf(stderr, "bit depth must be 8, 10, or 12\n");
            return 2;
        }
        bit_depth = (unsigned)parsed;
    }
    if (argc >= 5 && strcmp(argv[4], "coarse") != 0 && strcmp(argv[4], "fine") != 0) {
        fprintf(stderr, "content pattern must be coarse or fine\n");
        return 2;
    }
    if (argc == 6 && strcmp(argv[5], "0") != 0 && strcmp(argv[5], "1") != 0 &&
        strcmp(argv[5], "2") != 0) {
        fprintf(stderr, "CDF update mode must be 0 (frozen), 1 (adaptive), or 2 (selective)\n");
        return 2;
    }
    const int fine_content = argc >= 5 && strcmp(argv[4], "fine") == 0;
    // libaom's chroma-size table has no 4x16/8x32/16x64 vertical leaf in 4:2:2.
    // Reject this unsupported fixture geometry before forcing its partition.
    if (FIXTURE_WIDTH < FIXTURE_HEIGHT && FIXTURE_FORMAT == AVIF_PIXEL_FORMAT_YUV422) {
        fprintf(stderr, "vertical 4:2:2 narrow leaves have no valid native chroma block size\n");
        return 2;
    }
    const char *const tx_size_search = strcmp(argv[2], "select") == 0 ? "1" : "0";
    const int mode_select_content = strcmp(argv[2], "select") == 0;
    avifEncoder *encoder = avifEncoderCreate();
    avifImage *image = avifImageCreate(FIXTURE_WIDTH, FIXTURE_HEIGHT, bit_depth, FIXTURE_FORMAT);
    avifRWData output = AVIF_DATA_EMPTY;
    int success = 0;
    if (encoder == NULL || image == NULL) {
        fprintf(stderr, "failed to allocate encoder or YUV frame\n");
        goto cleanup;
    }
    if (avifImageAllocatePlanes(image, AVIF_PLANES_YUV) != AVIF_RESULT_OK) {
        fprintf(stderr, "failed to allocate YUV planes\n");
        goto cleanup;
    }
    image->yuvRange = AVIF_RANGE_FULL;
    image->colorPrimaries = AVIF_COLOR_PRIMARIES_BT709;
    image->transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_SRGB;
    image->matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_BT601;
    encoder->codecChoice = AVIF_CODEC_CHOICE_AOM;
    encoder->maxThreads = 1;
    encoder->quality = FIXTURE_QUALITY;
    encoder->qualityAlpha = FIXTURE_QUALITY;
    encoder->speed = 0;
    encoder->autoTiling = AVIF_FALSE;
    encoder->timescale = 1000;
    encoder->repetitionCount = 0;
    const char *const options[][2] = {
        {"sb-size", "64"},
        {"min-partition-size", "4"},
        {"max-partition-size", FIXTURE_STRING(FIXTURE_ROOT_SIZE)},
        {"enable-rect-partitions", "1"},
        {"enable-ab-partitions", "0"},
        {"enable-1to4-partitions", "1"},
        {"enable-tx-size-search", tx_size_search},
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
#if FIXTURE_ENABLE_CHROMA_DELTAQ
    const avifResult chroma_delta_result =
        avifEncoderSetCodecSpecificOption(encoder, "enable-chroma-deltaq", "1");
    if (chroma_delta_result != AVIF_RESULT_OK) {
        fprintf(stderr, "option enable-chroma-deltaq failed: %s\n",
                avifResultToString(chroma_delta_result));
        goto cleanup;
    }
#endif
    // This is a normal encoder control. Omitting it preserves every existing
    // input; mode 0 freezes all frames, while mode 2 selects updates per frame.
    if (argc == 6) {
        const avifResult cdf_result =
            avifEncoderSetCodecSpecificOption(encoder, "cdf-update-mode", argv[5]);
        if (cdf_result != AVIF_RESULT_OK) {
            fprintf(stderr, "CDF update mode failed: %s\n", avifResultToString(cdf_result));
            goto cleanup;
        }
    }
    for (unsigned frame = 0; frame < 2; ++frame) {
        fill_frame(image, frame, mode_select_content, bit_depth, fine_content);
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
