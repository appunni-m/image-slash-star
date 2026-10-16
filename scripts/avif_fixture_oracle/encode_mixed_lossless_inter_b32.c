#include <avif/avif.h>

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static uint8_t luma_value(unsigned frame, unsigned x, unsigned y)
{
    if (frame == 0) {
        return 48;
    }

    if (x < 32 && y < 32) {
        return 96;
    }
    return 48;
}

static int fill_frame(avifImage *image, unsigned frame)
{
    if (avifImageAllocatePlanes(image, AVIF_PLANES_YUV) != AVIF_RESULT_OK) {
        return 0;
    }

    for (int plane = AVIF_CHAN_Y; plane <= AVIF_CHAN_V; ++plane) {
        uint8_t * const data = avifImagePlane(image, (avifChannelIndex)plane);
        const unsigned int width = avifImagePlaneWidth(image, (avifChannelIndex)plane);
        const unsigned int height = avifImagePlaneHeight(image, (avifChannelIndex)plane);
        const unsigned int row_bytes = avifImagePlaneRowBytes(image, (avifChannelIndex)plane);
        for (unsigned int y = 0; y < height; ++y) {
            for (unsigned int x = 0; x < width; ++x) {
                const unsigned int pixel_x = plane == AVIF_CHAN_Y ? x : x * 2;
                const unsigned int pixel_y = plane == AVIF_CHAN_Y ? y : y * 2;
                uint8_t value = 128;
                if (plane == AVIF_CHAN_Y) {
                    value = luma_value(frame, pixel_x, pixel_y);
                }
                data[(size_t)y * row_bytes + x] = value;
            }
        }
    }
    return 1;
}

static int write_sequence(const char *path)
{
    avifEncoder *encoder = avifEncoderCreate();
    avifImage *image = avifImageCreate(64, 64, 8, AVIF_PIXEL_FORMAT_YUV420);
    avifRWData output = AVIF_DATA_EMPTY;
    int success = 0;
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
    encoder->quality = 80;
    encoder->qualityAlpha = 80;
    encoder->speed = 8;
    encoder->autoTiling = AVIF_FALSE;
    encoder->tileColsLog2 = 0;
    encoder->tileRowsLog2 = 0;
    encoder->timescale = 1000;
    encoder->repetitionCount = AVIF_REPETITION_COUNT_INFINITE;

    static const char * const options[][2] = {
        { "min-partition-size", "32" },
        { "max-partition-size", "32" },
        { "enable-cdef", "0" },
        { "enable-restoration", "0" },
        { "loopfilter-control", "0" },
        { "enable-warped-motion", "0" },
        { "fixture-roi-lossless-inter-b32", "1" },
    };
    for (size_t i = 0; i < sizeof(options) / sizeof(options[0]); ++i) {
        const avifResult result =
            avifEncoderSetCodecSpecificOption(encoder, options[i][0], options[i][1]);
        if (result != AVIF_RESULT_OK) {
            fprintf(stderr, "codec option %s failed: %s\n", options[i][0], avifResultToString(result));
            goto cleanup;
        }
    }

    for (unsigned frame = 0; frame < 2; ++frame) {
        if (!fill_frame(image, frame)) {
            fprintf(stderr, "failed to fill frame %u\n", frame);
            goto cleanup;
        }
        const avifResult result =
            avifEncoderAddImage(encoder, image, 100, AVIF_ADD_IMAGE_FLAG_NONE);
        if (result != AVIF_RESULT_OK) {
            fprintf(stderr, "adding frame %u failed: %s (%s)\n", frame,
                    avifResultToString(result), encoder->diag.error);
            goto cleanup;
        }
    }

    {
        const avifResult result = avifEncoderFinish(encoder, &output);
        if (result != AVIF_RESULT_OK) {
            fprintf(stderr, "finishing the AVIF sequence failed: %s (%s)\n",
                    avifResultToString(result), encoder->diag.error);
            goto cleanup;
        }
    }

    FILE * const file = fopen(path, "wb");
    if (file == NULL) {
        fprintf(stderr, "cannot open output path %s\n", path);
        goto cleanup;
    }
    const size_t bytes_written = fwrite(output.data, 1, output.size, file);
    const int close_result = fclose(file);
    if (bytes_written != output.size || close_result != 0) {
        fprintf(stderr, "writing AVIF output %s failed\n", path);
        goto cleanup;
    }

    fprintf(stderr, "wrote %zu bytes\n", output.size);
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
        fprintf(stderr, "usage: encode_mixed_lossless_inter_b32 OUTPUT.avif\n");
        return 2;
    }
    return write_sequence(argv[1]) ? 0 : 1;
}
