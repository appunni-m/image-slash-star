/*
 * Produce YCCK fixtures with the pinned libjpeg-turbo 3.2.0 benchmark-oracle
 * build. The default YCCK sampling factors exercise 2x2 luma/K and 1x1 chroma
 * sampling at odd image edges. Optional modes exercise independent luma/K
 * factors, integer upsampling fallbacks, and single-source-sample edges.
 */
#include <stdio.h>
#include <jpeglib.h>
#include <string.h>

int main(int argc, char **argv) {
    if (argc < 2 || argc > 4) return 2;
    int progressive = 0;
    int image_width = 17;
    int image_height = 9;
    int luma_horizontal_sampling = 2;
    int luma_vertical_sampling = 2;
    int black_horizontal_sampling = 2;
    int black_vertical_sampling = 2;
    int custom_sampling = 0;
    for (int argument = 2; argument < argc; ++argument) {
        if (strcmp(argv[argument], "progressive") == 0) {
            progressive = 1;
        } else if (strcmp(argv[argument], "h2v1") == 0) {
            luma_horizontal_sampling = 2;
            luma_vertical_sampling = 1;
            black_horizontal_sampling = 2;
            black_vertical_sampling = 1;
            custom_sampling = 1;
        } else if (strcmp(argv[argument], "h4v1") == 0) {
            luma_horizontal_sampling = 4;
            luma_vertical_sampling = 1;
            black_horizontal_sampling = 4;
            black_vertical_sampling = 1;
            custom_sampling = 1;
        } else if (strcmp(argv[argument], "v2") == 0 ||
                   strcmp(argv[argument], "v2_even_height") == 0) {
            if (strcmp(argv[argument], "v2_even_height") == 0) {
                image_height = 10;
            }
            luma_horizontal_sampling = 1;
            luma_vertical_sampling = 2;
            black_horizontal_sampling = 1;
            black_vertical_sampling = 2;
            custom_sampling = 1;
        } else if (strcmp(argv[argument], "luma_upsample") == 0) {
            luma_horizontal_sampling = 1;
            luma_vertical_sampling = 1;
            black_horizontal_sampling = 2;
            black_vertical_sampling = 2;
            custom_sampling = 1;
        } else if (strcmp(argv[argument], "black_upsample") == 0) {
            luma_horizontal_sampling = 2;
            luma_vertical_sampling = 2;
            black_horizontal_sampling = 1;
            black_vertical_sampling = 1;
            custom_sampling = 1;
        } else if (strcmp(argv[argument], "tiny_2x2") == 0) {
            image_width = 2;
            image_height = 2;
        } else {
            return 2;
        }
    }
    FILE *output = fopen(argv[1], "wb");
    if (!output) return 3;

    struct jpeg_compress_struct cinfo;
    struct jpeg_error_mgr jerr;
    cinfo.err = jpeg_std_error(&jerr);
    jpeg_create_compress(&cinfo);
    jpeg_stdio_dest(&cinfo, output);

    cinfo.image_width = (JDIMENSION)image_width;
    cinfo.image_height = (JDIMENSION)image_height;
    cinfo.input_components = 4;
    cinfo.in_color_space = JCS_CMYK;
    jpeg_set_defaults(&cinfo);
    jpeg_set_colorspace(&cinfo, JCS_YCCK);
    if (custom_sampling) {
        for (int component = 0; component < 4; ++component) {
            cinfo.comp_info[component].h_samp_factor = 1;
            cinfo.comp_info[component].v_samp_factor = 1;
        }
        cinfo.comp_info[0].h_samp_factor = luma_horizontal_sampling;
        cinfo.comp_info[0].v_samp_factor = luma_vertical_sampling;
        cinfo.comp_info[3].h_samp_factor = black_horizontal_sampling;
        cinfo.comp_info[3].v_samp_factor = black_vertical_sampling;
    }
    jpeg_set_quality(&cinfo, 85, TRUE);
    if (progressive) jpeg_simple_progression(&cinfo);
    jpeg_start_compress(&cinfo, TRUE);

    JSAMPLE row[17 * 4];
    while (cinfo.next_scanline < cinfo.image_height) {
        unsigned int y = cinfo.next_scanline;
        for (unsigned int x = 0; x < cinfo.image_width; ++x) {
            row[4 * x + 0] = (JSAMPLE)((x * 17 + y * 3) & 255);
            row[4 * x + 1] = (JSAMPLE)((x * 5 + y * 19) & 255);
            row[4 * x + 2] = (JSAMPLE)((x * 11 + y * 13) & 255);
            row[4 * x + 3] = (JSAMPLE)((x * 7 + y * 23) & 255);
        }
        JSAMPROW scanline = row;
        if (jpeg_write_scanlines(&cinfo, &scanline, 1) != 1) return 4;
    }

    jpeg_finish_compress(&cinfo);
    jpeg_destroy_compress(&cinfo);
    fclose(output);
    return 0;
}
