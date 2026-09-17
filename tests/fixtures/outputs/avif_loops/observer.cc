
#include <avif/avif.h>
#include <dlfcn.h>
#include <cstdio>
extern "C" int observe(void * library, const uint8_t * input, size_t size,
                        int * fields, char * diagnostic, size_t capacity) {
#define LOAD(name) \
    auto p_##name = reinterpret_cast<decltype(&name)>(dlsym(library, #name)); \
    if (!p_##name) return 1
    LOAD(avifDecoderCreate); LOAD(avifDecoderDestroy);
    LOAD(avifDecoderSetIOMemory); LOAD(avifDecoderParse); LOAD(avifDecoderNextImage);
#undef LOAD
    avifDecoder * decoder = p_avifDecoderCreate();
    if (!decoder) return 2;
    decoder->codecChoice = AVIF_CODEC_CHOICE_DAV1D;
    decoder->maxThreads = 1;
    auto finish = [&](int result) { p_avifDecoderDestroy(decoder); return result; };
    avifResult result = p_avifDecoderSetIOMemory(decoder, input, size);
    if (result != AVIF_RESULT_OK) return finish(3);
    result = p_avifDecoderParse(decoder);
    fields[0] = result;
    if (result != AVIF_RESULT_OK) {
        std::snprintf(diagnostic, capacity, "%s", decoder->diag.error);
        return finish(0);
    }
    fields[1] = decoder->repetitionCount;
    fields[2] = decoder->imageCount;
    fields[3] = 0;
    while ((result = p_avifDecoderNextImage(decoder)) == AVIF_RESULT_OK) ++fields[3];
    fields[4] = result;
    fields[5] = decoder->imageSequenceTrackPresent;
    std::snprintf(diagnostic, capacity, "%s", decoder->diag.error);
    if (result != AVIF_RESULT_NO_IMAGES_REMAINING) return finish(4);
    return finish(0);
}
