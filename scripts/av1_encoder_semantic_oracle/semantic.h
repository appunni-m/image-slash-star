/* Development-only native observations; never a Rust/runtime dependency. */
#ifndef IMAGE_SLASH_STAR_SEMANTIC_H
#define IMAGE_SLASH_STAR_SEMANTIC_H
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
unsigned oracle_position(void);
FILE *oracle_semantic(unsigned writer, const char *kind);
FILE *oracle_semantic_output(void);
void oracle_semantic_end(void);
void oracle_frame_begin(const void *frame, unsigned width, unsigned height);
unsigned oracle_generation(const void *frame);
unsigned oracle_next_origin(void);
void oracle_publish_origin(const void *frame, const void *slot, int kind, unsigned id);
unsigned oracle_find_origin(const void *frame, const void *slot, int kind);
void oracle_cdf_reset(unsigned writer);
void oracle_cdf_register(const char *name, const void *data, size_t bytes,
                         unsigned rank, const unsigned *shape);
void oracle_semantic_cdf(unsigned writer, const uint16_t *cdf, int symbols, int adaptive);
void oracle_bytes(FILE *f, const uint8_t *data, int count);
#endif
