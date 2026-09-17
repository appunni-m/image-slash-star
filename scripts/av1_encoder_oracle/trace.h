/* Development-only observations of pinned libaom; never linked into Rust. */
#ifndef IMAGE_SLASH_STAR_ORACLE_TRACE_H
#define IMAGE_SLASH_STAR_ORACLE_TRACE_H
#include "aom_dsp/entenc.h"
void oracle_start(unsigned id, const od_ec_enc *ec);
void oracle_before(unsigned id, const char *kind, const od_ec_enc *ec,
                   int value, unsigned probability, const uint16_t *cdf,
                   int length, int update);
void oracle_after(const od_ec_enc *ec, const uint16_t *cdf, int length);
void oracle_finish_before(unsigned id, const od_ec_enc *ec);
void oracle_finish_after(const od_ec_enc *ec, const uint8_t *bytes, unsigned size);
#endif
