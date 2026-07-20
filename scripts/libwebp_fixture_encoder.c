/* Build against pinned libwebp 1.6.0 to create VP8 token-partition fixtures.
 * Usage: encoder input.rgb width height partition_bits output.webp
 */
#include <stdio.h>
#include <stdlib.h>
#include <webp/encode.h>

int main(int argc, char **argv) {
    if (argc != 6) return 2;
    const int width = atoi(argv[2]);
    const int height = atoi(argv[3]);
    const int partitions = atoi(argv[4]);
    const size_t size = (size_t)width * (size_t)height * 3;
    FILE *input = fopen(argv[1], "rb");
    if (!input) return 3;
    unsigned char *rgb = malloc(size);
    if (!rgb || fread(rgb, 1, size, input) != size) return 4;
    fclose(input);

    WebPConfig config;
    WebPPicture picture;
    WebPMemoryWriter writer;
    if (!WebPConfigInit(&config) || !WebPPictureInit(&picture)) return 5;
    config.quality = 75.0f;
    config.method = 4;
    config.partitions = partitions;
    config.low_memory = 1;
    if (!WebPValidateConfig(&config)) return 6;
    picture.width = width;
    picture.height = height;
    if (!WebPPictureImportRGB(&picture, rgb, width * 3)) return 7;
    free(rgb);

    WebPMemoryWriterInit(&writer);
    picture.writer = WebPMemoryWrite;
    picture.custom_ptr = &writer;
    if (!WebPEncode(&config, &picture)) return 8;
    WebPPictureFree(&picture);

    FILE *output = fopen(argv[5], "wb");
    if (!output || fwrite(writer.mem, 1, writer.size, output) != writer.size) return 9;
    fclose(output);
    WebPMemoryWriterClear(&writer);
    return 0;
}
