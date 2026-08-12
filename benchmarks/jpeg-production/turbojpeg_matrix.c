#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include <turbojpeg.h>

enum { WARMUP = 100 };

typedef struct {
    int width, height, channels, pixel_format, quality, subsampling;
    int progressive, optimize, restart_rows;
    const char *mode;
    unsigned char *pixels;
} encode_case;

static void fail(const char *message) { fprintf(stderr, "%s\n", message); exit(EXIT_FAILURE); }
static void fail_tj(tjhandle handle, const char *operation) {
    fprintf(stderr, "%s failed: %s\n", operation, tj3GetErrorStr(handle));
    exit(EXIT_FAILURE);
}

static uint64_t fnv1a(const unsigned char *bytes, size_t length) {
    uint64_t hash = UINT64_C(0xcbf29ce484222325);
    for (size_t i = 0; i < length; i++) hash = (hash ^ bytes[i]) * UINT64_C(0x00000100000001b3);
    return hash;
}

static uint64_t now_ns(void) {
    struct timespec value;
    if (clock_gettime(CLOCK_MONOTONIC_RAW, &value) != 0) fail("clock_gettime failed");
    return (uint64_t)value.tv_sec * UINT64_C(1000000000) + (uint64_t)value.tv_nsec;
}

static int compare_u64(const void *left, const void *right) {
    uint64_t a = *(const uint64_t *)left, b = *(const uint64_t *)right;
    return (a > b) - (a < b);
}

static void report(const char *operation, size_t iterations, uint64_t total, uint64_t *samples) {
    qsort(samples, iterations, sizeof(*samples), compare_u64);
    size_t p95 = (iterations * 95U + 99U) / 100U - 1U;
    printf("operation=%s\nimplementation=libjpeg-turbo-3.2.0\nboundary=public-api-fresh-operation-owned-output\n", operation);
    printf("iterations=%zu\nwarmup=%d\navg_ns=%.3f\nmedian_ns=%" PRIu64 "\np95_ns=%" PRIu64 "\nmin_ns=%" PRIu64 "\n",
           iterations, WARMUP, (double)total / (double)iterations,
           samples[(iterations - 1U) / 2U], samples[p95], samples[0]);
}

static unsigned char *generated_pixels(size_t length) {
    unsigned char *pixels = malloc(length);
    if (!pixels) fail("pixel allocation failed");
    uint32_t state = UINT32_C(0x12345678);
    for (size_t i = 0; i < length; i++) {
        state ^= state << 13; state ^= state >> 17; state ^= state << 5;
        pixels[i] = (unsigned char)state;
    }
    return pixels;
}

static unsigned char *read_file(const char *path, size_t *length) {
    FILE *file = fopen(path, "rb"); if (!file) fail("cannot open input");
    if (fseek(file, 0, SEEK_END) || (*length = (size_t)ftell(file), fseek(file, 0, SEEK_SET))) fail("cannot size input");
    unsigned char *bytes = malloc(*length); if (!bytes) fail("input allocation failed");
    if (fread(bytes, 1, *length, file) != *length) fail("cannot read input");
    fclose(file); return bytes;
}

static void configure_compress(tjhandle handle, const encode_case *value) {
    if (tj3Set(handle, TJPARAM_QUALITY, value->quality) ||
        tj3Set(handle, TJPARAM_SUBSAMP, value->subsampling) ||
        tj3Set(handle, TJPARAM_PROGRESSIVE, value->progressive) ||
        tj3Set(handle, TJPARAM_OPTIMIZE, value->optimize) ||
        tj3Set(handle, TJPARAM_RESTARTROWS, value->restart_rows) ||
        tj3Set(handle, TJPARAM_FASTDCT, 0)) fail_tj(handle, "tj3Set");
    if (value->channels == 4 && tj3Set(handle, TJPARAM_COLORSPACE, TJCS_CMYK))
        fail_tj(handle, "tj3Set(CMYK)");
}

static size_t encode_once(const encode_case *value, uint64_t *hash, const char *output_path) {
    tjhandle handle = tj3Init(TJINIT_COMPRESS); if (!handle) fail_tj(NULL, "tj3Init(compress)");
    configure_compress(handle, value);
    unsigned char *output = NULL; size_t output_size = 0;
    if (tj3Compress8(handle, value->pixels, value->width, 0, value->height, value->pixel_format, &output, &output_size)) fail_tj(handle, "tj3Compress8");
    if (hash) *hash = fnv1a(output, output_size);
    if (output_path) {
        FILE *file = fopen(output_path, "wb"); if (!file || fwrite(output, 1, output_size, file) != output_size || fclose(file)) fail("cannot write JPEG");
    }
    volatile unsigned char sink = output[0] ^ output[output_size - 1U]; (void)sink;
    tj3Free(output); tj3Destroy(handle); return output_size;
}

static size_t decode_once(const unsigned char *jpeg, size_t jpeg_size, uint64_t *hash,
                          int *width_out, int *height_out, int *channels_out) {
    tjhandle handle = tj3Init(TJINIT_DECOMPRESS); if (!handle) fail_tj(NULL, "tj3Init(decompress)");
    if (tj3DecompressHeader(handle, jpeg, jpeg_size)) fail_tj(handle, "tj3DecompressHeader");
    int width = tj3Get(handle, TJPARAM_JPEGWIDTH), height = tj3Get(handle, TJPARAM_JPEGHEIGHT);
    int colorspace = tj3Get(handle, TJPARAM_COLORSPACE);
    int pixel_format = colorspace == TJCS_GRAY ? TJPF_GRAY : (colorspace == TJCS_CMYK || colorspace == TJCS_YCCK ? TJPF_CMYK : TJPF_RGB);
    int channels = pixel_format == TJPF_GRAY ? 1 : (pixel_format == TJPF_CMYK ? 4 : 3);
    size_t output_size = (size_t)width * (size_t)height * (size_t)channels;
    unsigned char *output = malloc(output_size); if (!output) fail("decode output allocation failed");
    if (tj3Set(handle, TJPARAM_FASTDCT, 0) || tj3Decompress8(handle, jpeg, jpeg_size, output, 0, pixel_format)) fail_tj(handle, "tj3Decompress8");
    if (hash) *hash = fnv1a(output, output_size);
    volatile unsigned char sink = output[0] ^ output[output_size - 1U]; (void)sink;
    free(output); tj3Destroy(handle);
    *width_out = width; *height_out = height; *channels_out = channels;
    return output_size;
}

static encode_case parse_encode(int argc, char **argv) {
    if (argc < 11) fail("encode/emit WIDTH HEIGHT rgb|gray QUALITY 444|422|420 PROGRESSIVE OPTIMIZE RESTART_ROWS ITERATIONS|OUTPUT");
    encode_case value = {0};
    value.width = atoi(argv[2]); value.height = atoi(argv[3]); value.mode = argv[4];
    value.channels = strcmp(value.mode, "gray") == 0 ? 1 : (strcmp(value.mode, "cmyk") == 0 ? 4 : 3);
    value.pixel_format = value.channels == 1 ? TJPF_GRAY : (value.channels == 4 ? TJPF_CMYK : TJPF_RGB);
    value.quality = atoi(argv[5]);
    value.subsampling = strcmp(argv[6], "444") == 0 ? TJSAMP_444 : (strcmp(argv[6], "422") == 0 ? TJSAMP_422 : TJSAMP_420);
    if (value.channels == 1) value.subsampling = TJSAMP_GRAY;
    value.progressive = atoi(argv[7]); value.optimize = atoi(argv[8]); value.restart_rows = atoi(argv[9]);
    value.pixels = generated_pixels((size_t)value.width * (size_t)value.height * (size_t)value.channels);
    return value;
}

int main(int argc, char **argv) {
    if (argc < 2) fail("operation required");
    if (strcmp(argv[1], "encode") == 0 || strcmp(argv[1], "emit") == 0) {
        encode_case value = parse_encode(argc, argv);
        printf("width=%d\nheight=%d\nmode=%s\nquality=%d\nsubsampling=%s\nprogressive=%d\noptimize=%d\nrestart_rows=%d\ninput_fnv1a=%016" PRIx64 "\n",
               value.width, value.height, value.mode, value.quality, argv[6], value.progressive, value.optimize, value.restart_rows,
               fnv1a(value.pixels, (size_t)value.width * (size_t)value.height * (size_t)value.channels));
        if (strcmp(argv[1], "emit") == 0) {
            uint64_t hash = 0; size_t length = encode_once(&value, &hash, argv[10]);
            printf("output_bytes=%zu\noutput_fnv1a=%016" PRIx64 "\n", length, hash); free(value.pixels); return 0;
        }
        size_t iterations = strtoull(argv[10], NULL, 10); if (!iterations) fail("iterations must be positive");
        for (int i = 0; i < WARMUP; i++) (void)encode_once(&value, NULL, NULL);
        uint64_t hash = 0; size_t length = encode_once(&value, &hash, NULL);
        printf("output_bytes=%zu\noutput_fnv1a=%016" PRIx64 "\n", length, hash);
        uint64_t *samples = malloc(iterations * sizeof(*samples)); if (!samples) fail("sample allocation failed");
        uint64_t total_start = now_ns();
        for (size_t i = 0; i < iterations; i++) { uint64_t start = now_ns(); (void)encode_once(&value, NULL, NULL); samples[i] = now_ns() - start; }
        report("encode", iterations, now_ns() - total_start, samples); free(samples); free(value.pixels); return 0;
    }
    if (strcmp(argv[1], "decode") == 0) {
        if (argc != 4) fail("decode JPEG ITERATIONS");
        size_t jpeg_size = 0; unsigned char *jpeg = read_file(argv[2], &jpeg_size);
        size_t iterations = strtoull(argv[3], NULL, 10); if (!iterations) fail("iterations must be positive");
        int width = 0, height = 0, channels = 0;
        for (int i = 0; i < WARMUP; i++) (void)decode_once(jpeg, jpeg_size, NULL, &width, &height, &channels);
        uint64_t hash = 0; size_t length = decode_once(jpeg, jpeg_size, &hash, &width, &height, &channels);
        printf("input_bytes=%zu\ninput_fnv1a=%016" PRIx64 "\nwidth=%d\nheight=%d\nchannels=%d\noutput_bytes=%zu\noutput_fnv1a=%016" PRIx64 "\n", jpeg_size, fnv1a(jpeg, jpeg_size), width, height, channels, length, hash);
        uint64_t *samples = malloc(iterations * sizeof(*samples)); if (!samples) fail("sample allocation failed");
        uint64_t total_start = now_ns();
        for (size_t i = 0; i < iterations; i++) { uint64_t start = now_ns(); (void)decode_once(jpeg, jpeg_size, NULL, &width, &height, &channels); samples[i] = now_ns() - start; }
        report("decode", iterations, now_ns() - total_start, samples); free(samples); free(jpeg); return 0;
    }
    fail("operation must be encode, emit, or decode");
}
