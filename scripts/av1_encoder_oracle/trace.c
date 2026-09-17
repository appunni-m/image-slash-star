/* Development-only observations of pinned libaom; never linked into Rust. */
#include "oracle_trace.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

static FILE *trace_file;
static unsigned trace_events;
static size_t trace_bytes;

static FILE *output(void) {
  if (!trace_file) {
    const char *path = getenv("IMAGE_SLASH_STAR_ENCODER_TRACE");
    if (!path || !(trace_file = fopen(path, "wb"))) abort();
  }
  return trace_file;
}

static void hex(const uint8_t *data, unsigned size) {
  trace_bytes += size;
  if (size > 65536 || trace_bytes > 16 * 1024 * 1024) abort();
  fputc('"', output());
  for (unsigned i = 0; i < size; ++i) fprintf(output(), "%02x", data[i]);
  fputc('"', output());
}

static void state(const od_ec_enc *ec) {
  if (ec->error) abort();
  fprintf(output(), "{\"low\":%" PRIu64 ",\"range\":%u,\"count\":%d,"
          "\"offset\":%u,\"tell\":%d,\"tell_frac\":%u,\"buffer\":",
          ec->low, ec->rng, ec->cnt, ec->offs,
          od_ec_enc_tell(ec), od_ec_enc_tell_frac(ec));
  /* Flushes write eight physical bytes; only offs bytes are initialized output. */
  hex(ec->buf, ec->offs);
  fputc('}', output());
}

static void cdf_values(const uint16_t *cdf, int length) {
  if (length < 0 || length > 17) abort();
  fputc('[', output());
  for (int i = 0; i < length; ++i)
    fprintf(output(), "%s%u", i ? "," : "", cdf[i]);
  fputc(']', output());
}

static void event(void) {
  if (++trace_events > 20000) abort();
}

void oracle_start(unsigned id, const od_ec_enc *ec) {
  event();
  fprintf(output(), "{\"writer\":%u,\"kind\":\"start\",\"after\":", id);
  state(ec);
  fputs("}\n", output());
}

void oracle_before(unsigned id, const char *kind, const od_ec_enc *ec,
                   int value, unsigned probability, const uint16_t *cdf,
                   int length, int update) {
  event();
  fprintf(output(), "{\"writer\":%u,\"kind\":\"%s\",\"value\":%d,"
          "\"probability\":%u,\"update\":%d,\"cdf_before\":",
          id, kind, value, probability, update);
  cdf_values(cdf, length);
  fputs(",\"before\":", output());
  state(ec);
}

void oracle_after(const od_ec_enc *ec, const uint16_t *cdf, int length) {
  fputs(",\"after\":", output());
  state(ec);
  fputs(",\"cdf_after\":", output());
  cdf_values(cdf, length);
  fputs("}\n", output());
}

void oracle_finish_before(unsigned id, const od_ec_enc *ec) {
  event();
  fprintf(output(), "{\"writer\":%u,\"kind\":\"finish\",\"before\":", id);
  state(ec);
}

void oracle_finish_after(const od_ec_enc *ec, const uint8_t *bytes, unsigned size) {
  if (!bytes) abort();
  fputs(",\"after\":", output());
  state(ec);
  fputs(",\"bytes\":", output());
  hex(bytes, size);
  fprintf(output(), ",\"length\":%u}\n", size);
  if (fflush(output())) abort();
}
