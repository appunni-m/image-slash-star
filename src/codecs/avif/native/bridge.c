// SPDX-License-Identifier: MIT OR Apache-2.0
//
// A narrow ABI boundary around libavif 1.4.1. The call order and field values
// intentionally match Pillow 12.2.0's src/_avif.c, but this bridge owns its
// error handling and Rust-friendly buffer contract.

#include "avif/avif.h"

#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

enum {
    PRS_AVIF_OK = 0,
    PRS_AVIF_INVALID_ARGUMENT = 1,
    PRS_AVIF_VERSION_MISMATCH = 2,
    PRS_AVIF_CODEC_UNAVAILABLE = 3,
    PRS_AVIF_OUT_OF_MEMORY = 4,
    PRS_AVIF_LIBRARY_ERROR = 5,
    PRS_AVIF_BUFFER_TOO_SMALL = 6
};

enum {
    PRS_AVIF_NEEDS_DECODER = 1,
    PRS_AVIF_NEEDS_ENCODER = 2
};

typedef struct {
    const char *key;
    const char *value;
} prs_avif_codec_option;

typedef struct {
    uint32_t width;
    uint32_t height;
    uint32_t frame_count;
    uint32_t channels;
    uint64_t timescale;
    int32_t repetition_count;
} prs_avif_decode_info;

typedef struct {
    uint64_t pts_in_timescales;
    uint64_t duration_in_timescales;
} prs_avif_frame_timing;

typedef struct {
    uint32_t width;
    uint32_t height;
    int32_t yuv_format;
    int32_t yuv_range;
    int32_t quality;
    int32_t speed;
    int32_t max_threads;
    int32_t tile_rows_log2;
    int32_t tile_cols_log2;
    int32_t alpha_premultiplied;
    int32_t auto_tiling;
    uint64_t timescale;
    uint64_t creation_time;
    uint64_t modification_time;
    const uint8_t *icc;
    size_t icc_size;
    const uint8_t *exif;
    size_t exif_size;
    int32_t exif_orientation;
    const uint8_t *xmp;
    size_t xmp_size;
    const prs_avif_codec_option *advanced;
    size_t advanced_count;
} prs_avif_encode_config;

typedef struct {
    avifDecoder *decoder;
} prs_avif_decoder;

typedef struct {
    avifEncoder *encoder;
    avifImage *image;
    uint32_t width;
    uint32_t height;
    int first_frame;
} prs_avif_encoder;

static int
prs_avif_codec_version_contains(const char *expected) {
    char versions[256];
    const char *match;
    char suffix;

    avifCodecVersions(versions);
    match = strstr(versions, expected);
    if (match == NULL) {
        return 0;
    }
    suffix = match[strlen(expected)];
    return suffix == '\0' || suffix == ',' || suffix == '-';
}

int
prs_avif_runtime_status(uint32_t requirements) {
    if (strcmp(avifVersion(), "1.4.1") != 0) {
        return PRS_AVIF_VERSION_MISMATCH;
    }
    if ((requirements & PRS_AVIF_NEEDS_DECODER) != 0 &&
        !prs_avif_codec_version_contains("dav1d [dec]:1.5.3")) {
        return PRS_AVIF_CODEC_UNAVAILABLE;
    }
    if ((requirements & PRS_AVIF_NEEDS_ENCODER) != 0 &&
        !prs_avif_codec_version_contains("aom [enc]:3.13.2")) {
        return PRS_AVIF_CODEC_UNAVAILABLE;
    }
    return PRS_AVIF_OK;
}

static uint8_t
prs_avif_normalize_tiles_log2(int32_t value) {
    if (value < 0) {
        return 0;
    }
    if (value > 6) {
        return 6;
    }
    return (uint8_t)value;
}

static void
prs_avif_set_orientation(avifImage *image, int32_t orientation) {
    switch (orientation) {
        case 2:
            image->transformFlags |= AVIF_TRANSFORM_IMIR;
            image->imir.axis = 1;
            break;
        case 3:
            image->transformFlags |= AVIF_TRANSFORM_IROT;
            image->irot.angle = 2;
            break;
        case 4:
            image->transformFlags |= AVIF_TRANSFORM_IMIR;
            break;
        case 5:
            image->transformFlags |= AVIF_TRANSFORM_IROT | AVIF_TRANSFORM_IMIR;
            image->irot.angle = 1;
            break;
        case 6:
            image->transformFlags |= AVIF_TRANSFORM_IROT;
            image->irot.angle = 3;
            break;
        case 7:
            image->transformFlags |= AVIF_TRANSFORM_IROT | AVIF_TRANSFORM_IMIR;
            image->irot.angle = 3;
            break;
        case 8:
            image->transformFlags |= AVIF_TRANSFORM_IROT;
            image->irot.angle = 1;
            break;
        default:
            break;
    }
}

int
prs_avif_decoder_create(
    const uint8_t *data,
    size_t size,
    int32_t max_threads,
    prs_avif_decoder **out_decoder,
    prs_avif_decode_info *out_info
) {
    avifCodecChoice codec;
    avifDecoder *decoder;
    prs_avif_decoder *wrapper;
    avifResult result;

    if (data == NULL || size == 0 || out_decoder == NULL || out_info == NULL ||
        max_threads < 0) {
        return PRS_AVIF_INVALID_ARGUMENT;
    }
    *out_decoder = NULL;
    if (prs_avif_runtime_status(PRS_AVIF_NEEDS_DECODER) != PRS_AVIF_OK) {
        return PRS_AVIF_CODEC_UNAVAILABLE;
    }

    codec = avifCodecChoiceFromName("dav1d");
    if (codec == AVIF_CODEC_CHOICE_AUTO ||
        avifCodecName(codec, AVIF_CODEC_FLAG_CAN_DECODE) == NULL) {
        return PRS_AVIF_CODEC_UNAVAILABLE;
    }

    decoder = avifDecoderCreate();
    if (decoder == NULL) {
        return PRS_AVIF_OUT_OF_MEMORY;
    }
    decoder->maxThreads = max_threads;
    decoder->strictFlags &= ~AVIF_STRICT_CLAP_VALID;
    decoder->strictFlags &= ~AVIF_STRICT_PIXI_REQUIRED;
    decoder->codecChoice = codec;

    result = avifDecoderSetIOMemory(decoder, data, size);
    if (result != AVIF_RESULT_OK) {
        avifDecoderDestroy(decoder);
        return PRS_AVIF_LIBRARY_ERROR;
    }
    result = avifDecoderParse(decoder);
    if (result != AVIF_RESULT_OK) {
        avifDecoderDestroy(decoder);
        return PRS_AVIF_LIBRARY_ERROR;
    }

    wrapper = (prs_avif_decoder *)malloc(sizeof(*wrapper));
    if (wrapper == NULL) {
        avifDecoderDestroy(decoder);
        return PRS_AVIF_OUT_OF_MEMORY;
    }
    wrapper->decoder = decoder;

    out_info->width = decoder->image->width;
    out_info->height = decoder->image->height;
    out_info->frame_count = decoder->imageCount;
    out_info->channels = decoder->alphaPresent ? 4u : 3u;
    out_info->timescale = decoder->timescale;
    out_info->repetition_count = decoder->repetitionCount;
    *out_decoder = wrapper;
    return PRS_AVIF_OK;
}

int
prs_avif_decoder_decode(
    prs_avif_decoder *wrapper,
    uint32_t frame_index,
    uint8_t *output,
    size_t output_size,
    prs_avif_frame_timing *out_timing
) {
    avifDecoder *decoder;
    avifRGBImage rgb;
    avifResult result;
    uint32_t channels;
    size_t row_bytes;
    size_t required_size;

    if (wrapper == NULL || wrapper->decoder == NULL || output == NULL ||
        out_timing == NULL) {
        return PRS_AVIF_INVALID_ARGUMENT;
    }
    decoder = wrapper->decoder;
    result = avifDecoderNthImage(decoder, frame_index);
    if (result != AVIF_RESULT_OK) {
        return PRS_AVIF_LIBRARY_ERROR;
    }

    channels = decoder->alphaPresent ? 4u : 3u;
    if (decoder->image->width > SIZE_MAX / channels) {
        return PRS_AVIF_INVALID_ARGUMENT;
    }
    row_bytes = (size_t)decoder->image->width * channels;
    if (row_bytes > UINT32_MAX ||
        decoder->image->height > SIZE_MAX / row_bytes) {
        return PRS_AVIF_INVALID_ARGUMENT;
    }
    required_size = row_bytes * decoder->image->height;
    if (output_size < required_size) {
        return PRS_AVIF_BUFFER_TOO_SMALL;
    }

    avifRGBImageSetDefaults(&rgb, decoder->image);
    rgb.depth = 8;
    rgb.format = decoder->alphaPresent ? AVIF_RGB_FORMAT_RGBA : AVIF_RGB_FORMAT_RGB;
    rgb.pixels = output;
    rgb.rowBytes = (uint32_t)row_bytes;
    result = avifImageYUVToRGB(decoder->image, &rgb);
    if (result != AVIF_RESULT_OK) {
        return PRS_AVIF_LIBRARY_ERROR;
    }

    out_timing->pts_in_timescales = decoder->imageTiming.ptsInTimescales;
    out_timing->duration_in_timescales = decoder->imageTiming.durationInTimescales;
    return PRS_AVIF_OK;
}

void
prs_avif_decoder_destroy(prs_avif_decoder *wrapper) {
    if (wrapper == NULL) {
        return;
    }
    if (wrapper->decoder != NULL) {
        avifDecoderDestroy(wrapper->decoder);
    }
    free(wrapper);
}

int
prs_avif_encoder_create(
    const prs_avif_encode_config *config,
    prs_avif_encoder **out_encoder
) {
    avifCodecChoice codec;
    avifImage *image;
    avifEncoder *encoder;
    prs_avif_encoder *wrapper;
    avifResult result;
    int32_t speed;

    if (config == NULL || out_encoder == NULL || config->width == 0 ||
        config->height == 0 ||
        config->quality < AVIF_QUALITY_WORST ||
        config->quality > AVIF_QUALITY_BEST ||
        (config->icc_size != 0 && config->icc == NULL) ||
        (config->exif_size != 0 && config->exif == NULL) ||
        (config->xmp_size != 0 && config->xmp == NULL) ||
        (config->advanced_count != 0 && config->advanced == NULL)) {
        return PRS_AVIF_INVALID_ARGUMENT;
    }
    if (config->yuv_format < AVIF_PIXEL_FORMAT_YUV444 ||
        config->yuv_format > AVIF_PIXEL_FORMAT_YUV400 ||
        (config->yuv_range != AVIF_RANGE_FULL &&
         config->yuv_range != AVIF_RANGE_LIMITED)) {
        return PRS_AVIF_INVALID_ARGUMENT;
    }
    *out_encoder = NULL;
    if (prs_avif_runtime_status(PRS_AVIF_NEEDS_ENCODER) != PRS_AVIF_OK) {
        return PRS_AVIF_CODEC_UNAVAILABLE;
    }

    codec = avifCodecChoiceFromName("aom");
    if (codec == AVIF_CODEC_CHOICE_AUTO ||
        avifCodecName(codec, AVIF_CODEC_FLAG_CAN_ENCODE) == NULL) {
        return PRS_AVIF_CODEC_UNAVAILABLE;
    }

    image = avifImageCreateEmpty();
    if (image == NULL) {
        return PRS_AVIF_OUT_OF_MEMORY;
    }
    image->width = config->width;
    image->height = config->height;
    image->depth = 8;
    image->yuvFormat = (avifPixelFormat)config->yuv_format;
    image->yuvRange = (avifRange)config->yuv_range;
    image->alphaPremultiplied = config->alpha_premultiplied ? AVIF_TRUE : AVIF_FALSE;

    encoder = avifEncoderCreate();
    if (encoder == NULL) {
        avifImageDestroy(image);
        return PRS_AVIF_OUT_OF_MEMORY;
    }
    encoder->codecChoice = codec;
    encoder->maxThreads = config->max_threads > 64 ? 64 : config->max_threads;
    encoder->quality = config->quality;
    speed = config->speed;
    if (speed < AVIF_SPEED_SLOWEST) {
        speed = AVIF_SPEED_SLOWEST;
    } else if (speed > AVIF_SPEED_FASTEST) {
        speed = AVIF_SPEED_FASTEST;
    }
    encoder->speed = speed;
    encoder->timescale = config->timescale;
    encoder->creationTime = config->creation_time;
    encoder->modificationTime = config->modification_time;
    encoder->autoTiling = config->auto_tiling ? AVIF_TRUE : AVIF_FALSE;
    if (!encoder->autoTiling) {
        encoder->tileRowsLog2 = prs_avif_normalize_tiles_log2(config->tile_rows_log2);
        encoder->tileColsLog2 = prs_avif_normalize_tiles_log2(config->tile_cols_log2);
    }

    for (size_t option_index = 0; option_index < config->advanced_count;
         ++option_index) {
        const prs_avif_codec_option *option = &config->advanced[option_index];
        if (option->key == NULL || option->value == NULL ||
            avifEncoderSetCodecSpecificOption(
                encoder,
                option->key,
                option->value
            ) != AVIF_RESULT_OK) {
            avifEncoderDestroy(encoder);
            avifImageDestroy(image);
            return PRS_AVIF_LIBRARY_ERROR;
        }
    }

    if (config->icc_size != 0) {
        result = avifImageSetProfileICC(image, config->icc, config->icc_size);
        if (result != AVIF_RESULT_OK) {
            avifEncoderDestroy(encoder);
            avifImageDestroy(image);
            return PRS_AVIF_LIBRARY_ERROR;
        }
        image->colorPrimaries = AVIF_COLOR_PRIMARIES_UNSPECIFIED;
        image->transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_UNSPECIFIED;
    } else {
        image->colorPrimaries = AVIF_COLOR_PRIMARIES_BT709;
        image->transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_SRGB;
    }
    image->matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_BT601;

    if (config->exif_size != 0) {
        result = avifImageSetMetadataExif(image, config->exif, config->exif_size);
        if (result != AVIF_RESULT_OK) {
            avifEncoderDestroy(encoder);
            avifImageDestroy(image);
            return PRS_AVIF_LIBRARY_ERROR;
        }
    }
    if (config->xmp_size != 0) {
        result = avifImageSetMetadataXMP(image, config->xmp, config->xmp_size);
        if (result != AVIF_RESULT_OK) {
            avifEncoderDestroy(encoder);
            avifImageDestroy(image);
            return PRS_AVIF_LIBRARY_ERROR;
        }
    }
    if (config->exif_orientation > 1) {
        prs_avif_set_orientation(image, config->exif_orientation);
    }

    wrapper = (prs_avif_encoder *)malloc(sizeof(*wrapper));
    if (wrapper == NULL) {
        avifEncoderDestroy(encoder);
        avifImageDestroy(image);
        return PRS_AVIF_OUT_OF_MEMORY;
    }
    wrapper->encoder = encoder;
    wrapper->image = image;
    wrapper->width = config->width;
    wrapper->height = config->height;
    wrapper->first_frame = 1;
    *out_encoder = wrapper;
    return PRS_AVIF_OK;
}

int
prs_avif_encoder_add(
    prs_avif_encoder *wrapper,
    const uint8_t *pixels,
    size_t pixel_size,
    uint32_t width,
    uint32_t height,
    uint32_t channels,
    uint64_t duration_in_timescales,
    int32_t is_single_frame
) {
    avifImage *frame;
    avifRGBImage rgb;
    avifResult result;
    size_t row_bytes;
    size_t expected_size;
    uint32_t add_flags;

    if (wrapper == NULL || wrapper->encoder == NULL || wrapper->image == NULL ||
        pixels == NULL || width != wrapper->width || height != wrapper->height ||
        (channels != 3 && channels != 4)) {
        return PRS_AVIF_INVALID_ARGUMENT;
    }
    if (width > SIZE_MAX / channels) {
        return PRS_AVIF_INVALID_ARGUMENT;
    }
    row_bytes = (size_t)width * channels;
    if (row_bytes > UINT32_MAX || height > SIZE_MAX / row_bytes) {
        return PRS_AVIF_INVALID_ARGUMENT;
    }
    expected_size = row_bytes * height;
    if (pixel_size != expected_size) {
        return PRS_AVIF_INVALID_ARGUMENT;
    }

    if (wrapper->first_frame) {
        frame = wrapper->image;
    } else {
        frame = avifImageCreateEmpty();
        if (frame == NULL) {
            return PRS_AVIF_OUT_OF_MEMORY;
        }
        frame->width = width;
        frame->height = height;
        frame->depth = wrapper->image->depth;
        frame->yuvFormat = wrapper->image->yuvFormat;
        frame->yuvRange = wrapper->image->yuvRange;
        frame->colorPrimaries = wrapper->image->colorPrimaries;
        frame->transferCharacteristics = wrapper->image->transferCharacteristics;
        frame->matrixCoefficients = wrapper->image->matrixCoefficients;
        frame->alphaPremultiplied = wrapper->image->alphaPremultiplied;
    }

    avifRGBImageSetDefaults(&rgb, frame);
    rgb.format = channels == 4 ? AVIF_RGB_FORMAT_RGBA : AVIF_RGB_FORMAT_RGB;
    rgb.pixels = (uint8_t *)pixels;
    rgb.rowBytes = (uint32_t)row_bytes;
    result = avifImageRGBToYUV(frame, &rgb);
    if (result != AVIF_RESULT_OK) {
        if (!wrapper->first_frame) {
            avifImageDestroy(frame);
        }
        return PRS_AVIF_LIBRARY_ERROR;
    }

    add_flags = is_single_frame ? AVIF_ADD_IMAGE_FLAG_SINGLE : AVIF_ADD_IMAGE_FLAG_NONE;
    result = avifEncoderAddImage(
        wrapper->encoder,
        frame,
        duration_in_timescales,
        add_flags
    );
    if (!wrapper->first_frame) {
        avifImageDestroy(frame);
    }
    if (result != AVIF_RESULT_OK) {
        return PRS_AVIF_LIBRARY_ERROR;
    }
    wrapper->first_frame = 0;
    return PRS_AVIF_OK;
}

int
prs_avif_encoder_finish(
    prs_avif_encoder *wrapper,
    uint8_t **out_data,
    size_t *out_size
) {
    avifRWData raw = AVIF_DATA_EMPTY;
    avifResult result;
    uint8_t *copy;

    if (wrapper == NULL || wrapper->encoder == NULL || out_data == NULL ||
        out_size == NULL) {
        return PRS_AVIF_INVALID_ARGUMENT;
    }
    *out_data = NULL;
    *out_size = 0;
    result = avifEncoderFinish(wrapper->encoder, &raw);
    if (result != AVIF_RESULT_OK) {
        avifRWDataFree(&raw);
        return PRS_AVIF_LIBRARY_ERROR;
    }
    if (raw.size == 0) {
        avifRWDataFree(&raw);
        return PRS_AVIF_LIBRARY_ERROR;
    }
    copy = (uint8_t *)malloc(raw.size);
    if (copy == NULL) {
        avifRWDataFree(&raw);
        return PRS_AVIF_OUT_OF_MEMORY;
    }
    memcpy(copy, raw.data, raw.size);
    *out_data = copy;
    *out_size = raw.size;
    avifRWDataFree(&raw);
    return PRS_AVIF_OK;
}

void
prs_avif_encoder_destroy(prs_avif_encoder *wrapper) {
    if (wrapper == NULL) {
        return;
    }
    if (wrapper->encoder != NULL) {
        avifEncoderDestroy(wrapper->encoder);
    }
    if (wrapper->image != NULL) {
        avifImageDestroy(wrapper->image);
    }
    free(wrapper);
}

void
prs_avif_bytes_free(uint8_t *data) {
    free(data);
}
