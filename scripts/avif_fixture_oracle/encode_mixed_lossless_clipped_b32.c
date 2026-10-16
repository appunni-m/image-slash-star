#include <avif/avif.h>

#include <stdio.h>

static int write_image(const char * path)
{
    avifEncoder * encoder = avifEncoderCreate();
    avifImage * image = avifImageCreate(60, 60, 8, AVIF_PIXEL_FORMAT_YUV420);
    if ((encoder == NULL) || (image == NULL)) {
        fprintf(stderr, "failed to allocate the AVIF encoder or image\n");
        avifEncoderDestroy(encoder);
        avifImageDestroy(image);
        return 0;
    }

    image->yuvRange = AVIF_RANGE_FULL;
    image->colorPrimaries = AVIF_COLOR_PRIMARIES_BT709;
    image->transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_SRGB;
    image->matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_BT601;

    if (avifImageAllocatePlanes(image, AVIF_PLANES_YUV) != AVIF_RESULT_OK) {
        fprintf(stderr, "failed to allocate the AVIF image planes\n");
        avifEncoderDestroy(encoder);
        avifImageDestroy(image);
        return 0;
    }
    for (int plane = AVIF_CHAN_Y; plane <= AVIF_CHAN_V; ++plane) {
        uint8_t * const data = avifImagePlane(image, (avifChannelIndex)plane);
        const unsigned int width = avifImagePlaneWidth(image, (avifChannelIndex)plane);
        const unsigned int height = avifImagePlaneHeight(image, (avifChannelIndex)plane);
        const unsigned int rowBytes = avifImagePlaneRowBytes(image, (avifChannelIndex)plane);
        const uint8_t value = (plane == AVIF_CHAN_Y) ? 48 : 128;
        for (unsigned int y = 0; y < height; ++y) {
            for (unsigned int x = 0; x < width; ++x) {
                data[(size_t)y * rowBytes + x] = value;
            }
        }
    }

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
        { "fixture-roi-lossless-clipped-b32", "1" },
    };
    for (size_t i = 0; i < sizeof(options) / sizeof(options[0]); ++i) {
        const avifResult result =
            avifEncoderSetCodecSpecificOption(encoder, options[i][0], options[i][1]);
        if (result != AVIF_RESULT_OK) {
            fprintf(stderr, "codec option %s failed: %s\n", options[i][0], avifResultToString(result));
            avifEncoderDestroy(encoder);
            avifImageDestroy(image);
            return 0;
        }
    }

    avifResult result = avifEncoderAddImage(encoder, image, 100, AVIF_ADD_IMAGE_FLAG_NONE);
    if (result != AVIF_RESULT_OK) {
        fprintf(stderr, "adding the AVIF image failed: %s (%s)\n",
                avifResultToString(result), encoder->diag.error);
        avifEncoderDestroy(encoder);
        avifImageDestroy(image);
        return 0;
    }

    avifRWData output = AVIF_DATA_EMPTY;
    result = avifEncoderFinish(encoder, &output);
    if (result != AVIF_RESULT_OK) {
        fprintf(stderr, "finishing the AVIF image failed: %s (%s)\n",
                avifResultToString(result), encoder->diag.error);
        avifRWDataFree(&output);
        avifEncoderDestroy(encoder);
        avifImageDestroy(image);
        return 0;
    }

    FILE * const file = fopen(path, "wb");
    if (file == NULL) {
        fprintf(stderr, "cannot open output path %s\n", path);
        avifRWDataFree(&output);
        avifEncoderDestroy(encoder);
        avifImageDestroy(image);
        return 0;
    }
    const size_t bytesWritten = fwrite(output.data, 1, output.size, file);
    const int closeResult = fclose(file);
    if ((bytesWritten != output.size) || (closeResult != 0)) {
        fprintf(stderr, "writing AVIF output %s failed\n", path);
        avifRWDataFree(&output);
        avifEncoderDestroy(encoder);
        avifImageDestroy(image);
        return 0;
    }

    fprintf(stderr, "wrote %zu bytes\n", output.size);
    avifRWDataFree(&output);
    avifEncoderDestroy(encoder);
    avifImageDestroy(image);
    return 1;
}

int main(int argc, char ** argv)
{
    if (argc != 2) {
        fprintf(stderr, "usage: encode_mixed_lossless_clipped_b32 OUTPUT.avif\n");
        return 2;
    }
    return write_image(argv[1]) ? 0 : 1;
}
