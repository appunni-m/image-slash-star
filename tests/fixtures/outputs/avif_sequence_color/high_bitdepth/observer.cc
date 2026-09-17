
#include <avif/avif.h>
#include <libyuv/row.h>
#include <libyuv/scale_row.h>
#include <libyuv/convert_argb.h>
#include <dlfcn.h>
#include <cstring>
#include <array>
#if defined(LIBYUV_UNLIMITED_DATA) || defined(LIBYUV_UNLIMITED_BT601)
#error This oracle requires the pinned default I601 constants
#endif

extern "C" int observe(void * library, const uint8_t * input, size_t size,
                       uint32_t frame, uint16_t * planes, uint8_t * rgba,
                       uint64_t * timing) {
#define LOAD(name) \
    auto p_##name = reinterpret_cast<decltype(&name)>(dlsym(library, #name)); \
    if (!p_##name) return 1
    LOAD(avifDecoderCreate);
    LOAD(avifDecoderDestroy);
    LOAD(avifDecoderSetIOMemory);
    LOAD(avifDecoderParse);
    LOAD(avifDecoderNthImage);
    LOAD(avifRGBImageSetDefaults);
    LOAD(avifImageYUVToRGB);
#undef LOAD
    avifDecoder * decoder = p_avifDecoderCreate();
    if (!decoder) return 2;
    auto finish = [&](int status) { p_avifDecoderDestroy(decoder); return status; };
    decoder->codecChoice = AVIF_CODEC_CHOICE_DAV1D;
    decoder->maxThreads = 1;
    if (p_avifDecoderSetIOMemory(decoder, input, size) != AVIF_RESULT_OK ||
        p_avifDecoderParse(decoder) != AVIF_RESULT_OK) return finish(3);
    if (decoder->imageCount != 5 || frame >= 5 ||
        p_avifDecoderNthImage(decoder, frame) != AVIF_RESULT_OK) return finish(4);
    const avifImage * image = decoder->image;
    if (image->width != 64 || image->height != 64 || image->depth != 12 ||
        image->yuvFormat != AVIF_PIXEL_FORMAT_YUV422 || image->yuvRange != AVIF_RANGE_LIMITED ||
        image->colorPrimaries != 2 || image->transferCharacteristics != 2 ||
        image->matrixCoefficients != 2 || !image->alphaPlane || image->alphaPremultiplied ||
        image->icc.size || image->transformFlags) return finish(5);
    size_t offset = 0;
    for (size_t plane = 0; plane < 4; ++plane) {
        const uint8_t * pixels = plane == 3 ? image->alphaPlane : image->yuvPlanes[plane];
        const uint32_t stride = plane == 3 ? image->alphaRowBytes : image->yuvRowBytes[plane];
        const size_t width = (plane == 1 || plane == 2) ? 32 : 64;
        if (!pixels || stride < width * 2) return finish(6);
        for (size_t y = 0; y < 64; ++y) {
            for (size_t x = 0; x < width; ++x) {
                uint16_t sample;
                std::memcpy(&sample, pixels + y * stride + x * 2, 2);
                if (sample > 4095) return finish(7);
                planes[offset++] = sample;
            }
        }
    }
    avifRGBImage output;
    p_avifRGBImageSetDefaults(&output, image);
    output.depth = 8;
    output.format = AVIF_RGB_FORMAT_RGBA;
    output.rowBytes = 64 * 4;
    output.pixels = rgba;
    if (p_avifImageYUVToRGB(image, &output) != AVIF_RESULT_OK) return finish(8);
    // Call the pinned scalar primitives with libyuv's endpoint convention.
    // Arithmetic and expected pixels are supplied by upstream, not a port.
    std::array<uint8_t, 12288> downshifted;
    libyuv::Convert16To8Row_C(planes, downshifted.data(), 4096, 12288);
    for (size_t row = 0; row < 64; ++row) {
        std::array<std::array<uint8_t, 64>, 2> chroma;
        for (size_t plane = 0; plane < 2; ++plane) {
            const uint8_t * src = downshifted.data() + 4096 + plane * 2048 + row * 32;
            chroma[plane][0] = src[0];
            libyuv::ScaleRowUp2_Linear_C(src, chroma[plane].data() + 1, 62);
            chroma[plane][63] = src[31];
        }
        std::array<uint8_t, 256> scalar;
        libyuv::I444AlphaToARGBRow_C(downshifted.data() + row * 64,
                                     chroma[1].data(), chroma[0].data(),
                                     downshifted.data() + 8192 + row * 64,
                                     scalar.data(), &libyuv::kYvuI601Constants, 64);
        if (std::memcmp(scalar.data(), rgba + row * 256, 256)) return finish(9);
    }
    timing[0] = decoder->imageTiming.timescale;
    timing[1] = decoder->imageTiming.ptsInTimescales;
    timing[2] = decoder->imageTiming.durationInTimescales;
    // Preserve the signed repetition value as two fields, without narrowing.
    timing[3] = decoder->repetitionCount < 0;
    timing[4] = timing[3] ? -int64_t(decoder->repetitionCount) : decoder->repetitionCount;
    return finish(0);
}
