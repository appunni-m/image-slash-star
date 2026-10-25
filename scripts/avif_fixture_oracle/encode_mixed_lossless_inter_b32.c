#include <avif/avif.h>

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint8_t luma_value(unsigned frame, unsigned x, unsigned y, int residual_roi)
{
    if (frame != 0 && x < 32 && y < 32) {
        return 96;
    }

    if (residual_roi && x >= 32 && y >= 32) {
        return frame == 0 ? 48 : 49;
    }

    return 48;
}

static int fill_frame(avifImage *image, unsigned frame, int residual_roi)
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
                    value = luma_value(frame, pixel_x, pixel_y, residual_roi);
                }
                data[(size_t)y * row_bytes + x] = value;
            }
        }
    }
    return 1;
}

static int write_sequence(const char *path, unsigned canvas_size, int residual_roi)
{
    avifEncoder *encoder = avifEncoderCreate();
    avifImage *image = avifImageCreate(canvas_size, canvas_size, 8, AVIF_PIXEL_FORMAT_YUV420);
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
        if (!fill_frame(image, frame, residual_roi)) {
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
    if (argc < 2) {
        fprintf(stderr, "usage: %s OUTPUT.avif [60|64] [--residual-roi]\n", argv[0]);
        return 2;
    }

    unsigned canvas_size = 64;
    int residual_roi = 0;
    int canvas_size_seen = 0;
    for (int index = 2; index < argc; ++index) {
        if (strcmp(argv[index], "--residual-roi") == 0) {
            if (residual_roi) {
                fprintf(stderr, "--residual-roi may be provided only once\n");
                return 2;
            }
            residual_roi = 1;
        } else if (strcmp(argv[index], "60") == 0) {
            if (canvas_size_seen) {
                fprintf(stderr, "canvas size may be provided only once\n");
                return 2;
            }
            canvas_size = 60;
            canvas_size_seen = 1;
        } else if (strcmp(argv[index], "64") == 0) {
            if (canvas_size_seen) {
                fprintf(stderr, "canvas size may be provided only once\n");
                return 2;
            }
            canvas_size = 64;
            canvas_size_seen = 1;
        } else {
            fprintf(stderr, "expected one canvas size (60 or 64) and --residual-roi\n");
            return 2;
        }
    }
    if (residual_roi && canvas_size != 64) {
        fprintf(stderr, "--residual-roi requires a 64-pixel canvas\n");
        return 2;
    }
    return write_sequence(argv[1], canvas_size, residual_roi) ? 0 : 1;
}
