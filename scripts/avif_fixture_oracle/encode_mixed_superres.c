#include <avif/avif.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

static int number(const char *s, unsigned low, unsigned high, unsigned *out)
{
    char *end = NULL;
    const unsigned long v = strtoul(s, &end, 10);
    if (*s == '\0' || *end != '\0' || v < low || v > high) { return 0; }
    *out = (unsigned)v;
    return 1;
}

int main(int argc, char **argv)
{
    if (argc == 2 && !strcmp(argv[1], "--version")) {
        char versions[256];
        avifCodecVersions(versions);
        printf("libavif %s (%s)\n", avifVersion(), versions);
        return 0;
    }
    unsigned width, height, depth, quality, speed, denominator, restoration;
    const char *mode = argc == 11 ? argv[10] : NULL;
    const int valid_mode = mode == NULL || !strcmp(mode, "ordinary") ||
        !strcmp(mode, "mixed-key") || !strcmp(mode, "motion-key") ||
        !strcmp(mode, "integer-key") || !strcmp(mode, "film-grain") ||
        !strcmp(mode, "film-grain-cdef") || !strcmp(mode, "loopfilter");
    if ((argc != 10 && argc != 11) ||
        !valid_mode ||
        !number(argv[2],32,256,&width) || !number(argv[3],16,128,&height) ||
        !number(argv[4],8,12,&depth) || (depth != 8 && depth != 10 && depth != 12) ||
        !number(argv[6],1,99,&quality) || !number(argv[7],0,10,&speed) ||
        !number(argv[8],8,16,&denominator) || !number(argv[9],0,1,&restoration)) {
        fprintf(stderr,"usage: %s OUTPUT WIDTH HEIGHT DEPTH {400|420|422|444} QUALITY SPEED DEN RESTORATION [ordinary|mixed-key|motion-key|integer-key|film-grain]\n",argv[0]);
        return 2;
    }
    avifPixelFormat layout;
    if (!strcmp(argv[5],"400")) { layout=AVIF_PIXEL_FORMAT_YUV400; }
    else if (!strcmp(argv[5],"420")) { layout=AVIF_PIXEL_FORMAT_YUV420; }
    else if (!strcmp(argv[5],"422")) { layout=AVIF_PIXEL_FORMAT_YUV422; }
    else if (!strcmp(argv[5],"444")) { layout=AVIF_PIXEL_FORMAT_YUV444; }
    else { return 2; }
    avifImage *image=avifImageCreate(width,height,depth,layout);
    avifEncoder *encoder=avifEncoderCreate();
    avifRWData bytes=AVIF_DATA_EMPTY;
    int success=0;
    if (image==NULL || encoder==NULL || avifImageAllocatePlanes(image,AVIF_PLANES_YUV)!=AVIF_RESULT_OK) { goto cleanup; }
    image->yuvRange=AVIF_RANGE_FULL;
    image->colorPrimaries=AVIF_COLOR_PRIMARIES_BT709;
    image->transferCharacteristics=AVIF_TRANSFER_CHARACTERISTICS_SRGB;
    image->matrixCoefficients=AVIF_MATRIX_COEFFICIENTS_BT601;
    encoder->codecChoice=AVIF_CODEC_CHOICE_AOM;
    encoder->maxThreads=1;
    encoder->quality=(int)quality;
    encoder->speed=(int)speed;
    encoder->autoTiling=AVIF_FALSE;
    encoder->tileColsLog2=0;
    encoder->tileRowsLog2=0;
    encoder->timescale=1000;
    encoder->repetitionCount=AVIF_REPETITION_COUNT_INFINITE;
    char den[4]; (void)snprintf(den,sizeof(den),"%u",denominator);
    static const char * const controls[][2]={
        {"min-partition-size","16"},{"max-partition-size","16"},
        {"superres-mode","1"},{"superres-kf-denominator","8"},
        {"enable-cdef","0"},{"loopfilter-control","0"},{"enable-global-motion","0"},
        {"enable-warped-motion","0"},{"enable-masked-comp","0"},{"enable-dist-wtd-comp","0"},
        {"enable-onesided-comp","0"},{"enable-interintra-comp","0"},{"enable-diff-wtd-comp","0"},
        {"enable-interinter-wedge","0"},{"enable-interintra-wedge","0"},{"enable-obmc","0"},
        {"enable-ref-frame-mvs","0"},{"enable-intrabc","0"},{"aq-mode","0"},
        {"deltaq-mode","0"},{"enable-chroma-deltaq","0"},{"enable-qm","0"},
        {"fixture-roi-mixed-superres","1"},{"film-grain-test","1"}
    };
    const int ordinary = mode != NULL && !strcmp(mode, "ordinary");
    const int film_grain = mode != NULL &&
        (!strcmp(mode, "film-grain") || !strcmp(mode, "film-grain-cdef"));
    const int film_grain_cdef = mode != NULL && !strcmp(mode, "film-grain-cdef");
    const int active_loop_filter = mode != NULL && !strcmp(mode, "loopfilter");
    for (size_t n=0;n<sizeof(controls)/sizeof(controls[0]);++n) {
        if (ordinary &&
            !strcmp(controls[n][0], "fixture-roi-mixed-superres")) { continue; }
        if (!film_grain &&
            !strcmp(controls[n][0], "film-grain-test")) { continue; }
        const char *value = film_grain_cdef && !strcmp(controls[n][0], "enable-cdef")
            ? "1" : controls[n][1];
        if (active_loop_filter && !strcmp(controls[n][0], "loopfilter-control")) {
            value = "1";
        }
        if (avifEncoderSetCodecSpecificOption(encoder,controls[n][0],value)!=AVIF_RESULT_OK) { goto cleanup; }
    }
    if (avifEncoderSetCodecSpecificOption(encoder,"superres-denominator",den)!=AVIF_RESULT_OK ||
        avifEncoderSetCodecSpecificOption(encoder,"enable-restoration",restoration ? "1":"0")!=AVIF_RESULT_OK) { goto cleanup; }
    const int integer_key = mode != NULL && !strcmp(mode, "integer-key");
    if (integer_key &&
        (avifEncoderSetCodecSpecificOption(encoder, "tune-content", "screen") != AVIF_RESULT_OK ||
         avifEncoderSetCodecSpecificOption(encoder, "enable-palette", "0") != AVIF_RESULT_OK)) { goto cleanup; }
    const int motion_key = mode != NULL && !strcmp(mode, "motion-key");
    const int mixed_key = mode != NULL && !strcmp(mode, "mixed-key");
    const int varied_key = integer_key || motion_key || mixed_key;
    for (unsigned frame=0;frame<2;++frame) {
        for (int plane=AVIF_CHAN_Y;plane<=AVIF_CHAN_V;++plane) {
            uint8_t *data=avifImagePlane(image,(avifChannelIndex)plane);
            const unsigned pw=avifImagePlaneWidth(image,(avifChannelIndex)plane);
            const unsigned ph=avifImagePlaneHeight(image,(avifChannelIndex)plane);
            const size_t stride=avifImagePlaneRowBytes(image,(avifChannelIndex)plane);
            for (unsigned y=0;y<ph;++y) {
                for (unsigned x=0;x<pw;++x) {
                    const int flat_key = frame == 0 && !varied_key;
                    const unsigned shift_x = plane == AVIF_CHAN_Y || layout == AVIF_PIXEL_FORMAT_YUV444 ? 4U : 2U;
                    const unsigned shift_y = plane != AVIF_CHAN_Y && layout == AVIF_PIXEL_FORMAT_YUV420 ? 1U : 2U;
                    const unsigned source_x = x + (motion_key ? frame * shift_x : 0U);
                    const unsigned source_y = y + (motion_key ? frame * shift_y : 0U);
                    const unsigned base=flat_key ? 128 : (64+((source_x*17+source_y*23+((source_x/8+source_y/8)&1)*61+(unsigned)plane*29)%128));
                    const uint16_t value=(uint16_t)((base<<(depth-8))+(flat_key ? 0 : ((source_x+source_y*3+(unsigned)plane)&((1U<<(depth-8))-1U))));
                    const size_t offset=y*stride+x*(depth==8 ? 1U:2U);
                    data[offset]=(uint8_t)(value&255U);
                    if (depth>8) { data[offset+1]=(uint8_t)(value>>8); }
                }
            }
        }
        const avifResult result=avifEncoderAddImage(encoder,image,100,AVIF_ADD_IMAGE_FLAG_NONE);
        if (result!=AVIF_RESULT_OK) { fprintf(stderr,"frame %u: %s (%s)\n",frame,avifResultToString(result),encoder->diag.error); goto cleanup; }
    }
    {
        const avifResult result=avifEncoderFinish(encoder,&bytes);
        if (result!=AVIF_RESULT_OK) { fprintf(stderr,"finish: %s (%s)\n",avifResultToString(result),encoder->diag.error); goto cleanup; }
        FILE *file=fopen(argv[1],"wb");
        if (file==NULL) { goto cleanup; }
        const size_t written=fwrite(bytes.data,1,bytes.size,file);
        const int closed=fclose(file);
        if (written!=bytes.size || closed!=0) { goto cleanup; }
        fprintf(stderr,"wrote %zu bytes\n",bytes.size);
        success=1;
    }
cleanup:
    avifRWDataFree(&bytes);
    avifImageDestroy(image);
    avifEncoderDestroy(encoder);
    return success ? 0:1;
}
