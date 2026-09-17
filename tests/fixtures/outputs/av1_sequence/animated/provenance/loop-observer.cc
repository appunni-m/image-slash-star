
#include <avif/avif.h>
#include <dlfcn.h>
extern "C" int observe_loop(void * library, const uint8_t * input, size_t size, int * repetition) {
#define LOAD(name) \
    auto p_##name = reinterpret_cast<decltype(&name)>(dlsym(library, #name)); \
    if (!p_##name) return 1
    LOAD(avifDecoderCreate);
    LOAD(avifDecoderDestroy);
    LOAD(avifDecoderSetIOMemory);
    LOAD(avifDecoderParse);
    LOAD(avifDecoderNthImage);
#undef LOAD
    avifDecoder * decoder = p_avifDecoderCreate();
    if (!decoder) return 2;
    decoder->codecChoice = AVIF_CODEC_CHOICE_DAV1D;
    decoder->maxThreads = 1;
    auto finish = [&](int result) { p_avifDecoderDestroy(decoder); return result; };
    if (p_avifDecoderSetIOMemory(decoder, input, size) != AVIF_RESULT_OK ||
        p_avifDecoderParse(decoder) != AVIF_RESULT_OK ||
        p_avifDecoderNthImage(decoder, 0) != AVIF_RESULT_OK) return finish(3);
    if (decoder->imageCount != 5 || !decoder->imageSequenceTrackPresent) return finish(4);
    *repetition = decoder->repetitionCount;
    return finish(0);
}
