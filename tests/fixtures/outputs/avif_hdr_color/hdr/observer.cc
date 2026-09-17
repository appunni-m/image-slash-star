
#include <avif/avif.h>
#include <libyuv/row.h>
#include <libyuv/convert_argb.h>
#include <dlfcn.h>
#include <cstring>
#include <vector>

extern "C" int observe(void * library, const uint8_t * input, size_t size,
                       uint16_t * planes, uint8_t * rgb, uint8_t * scalar,
                       uint32_t * strides) {
#define LOAD(name) \
    auto p_##name = reinterpret_cast<decltype(&name)>(dlsym(library, #name)); \
    if (!p_##name) return 1
    LOAD(avifImageCreateEmpty);
    LOAD(avifImageDestroy);
    LOAD(avifDecoderCreate);
    LOAD(avifDecoderDestroy);
    LOAD(avifDecoderReadMemory);
    LOAD(avifRGBImageSetDefaults);
    LOAD(avifImageYUVToRGB);
#undef LOAD
    avifImage * image = p_avifImageCreateEmpty();
    avifDecoder * decoder = p_avifDecoderCreate();
    auto finish = [&](int status) {
        if (image) p_avifImageDestroy(image);
        if (decoder) p_avifDecoderDestroy(decoder);
        return status;
    };
    if (!image || !decoder) return finish(2);
    decoder->codecChoice = AVIF_CODEC_CHOICE_DAV1D;
    decoder->maxThreads = 1;
    if (p_avifDecoderReadMemory(decoder, image, input, size) != AVIF_RESULT_OK)
        return finish(3);
    if (image->width != 200 || image->height != 200 || image->depth != 10 ||
        image->yuvFormat != AVIF_PIXEL_FORMAT_YUV444 || image->yuvRange != AVIF_RANGE_FULL ||
        image->colorPrimaries != 9 || image->transferCharacteristics != 16 ||
        image->matrixCoefficients != 9 || image->alphaPlane || image->icc.size)
        return finish(4);
    constexpr size_t count = 200 * 200;
    for (size_t plane = 0; plane < 3; ++plane) {
        if (!image->yuvPlanes[plane] || image->yuvRowBytes[plane] < 400)
            return finish(5);
        strides[plane] = image->yuvRowBytes[plane];
        for (size_t y = 0; y < 200; ++y) {
            for (size_t x = 0; x < 200; ++x) {
                uint16_t sample;
                std::memcpy(&sample, image->yuvPlanes[plane] + y * strides[plane] + x * 2, 2);
                if (sample > 1023) return finish(6);
                planes[plane * count + y * 200 + x] = sample;
            }
        }
    }
    avifRGBImage output;
    p_avifRGBImageSetDefaults(&output, image);
    output.depth = 8;
    output.format = AVIF_RGB_FORMAT_RGB;
    output.pixels = rgb;
    output.rowBytes = 200 * 3;
    if (p_avifImageYUVToRGB(image, &output) != AVIF_RESULT_OK) return finish(7);
    std::vector<uint8_t> downshifted(count * 3);
    for (size_t plane = 0; plane < 3; ++plane) {
        libyuv::Convert16To8Row_C(planes + plane * count,
                                  downshifted.data() + plane * count, 16384, count);
    }
    // libavif RGB uses the RGB24 row with U/V and the matrix both reversed.
    libyuv::I444ToRGB24Row_C(downshifted.data(), downshifted.data() + count * 2,
                             downshifted.data() + count, scalar,
                             &libyuv::kYvuV2020Constants, count);
    return finish(0);
}
