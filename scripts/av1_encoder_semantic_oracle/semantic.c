/* Included after oracle_trace.c in libaom's development-only bitwriter build. */
#include "oracle_semantic.h"
#include <stdlib.h>
#include <string.h>

static FILE *semantic_file;
static unsigned semantic_records, frame_generation, origin_counter, cdf_writer;
static const void *frame_identity;
static struct { const void *slot; int kind; unsigned id; } origins[8192];
static size_t origin_count;
static struct {
  const char *name;
  uintptr_t address;
  size_t bytes;
  unsigned rank, shape[4];
} families[48];
static size_t family_count;
static struct { const void *address; int symbols; } initial_cdfs[4096];
static size_t initial_count;

unsigned oracle_position(void) { return trace_events; }
FILE *oracle_semantic_output(void) { if (!semantic_file) abort(); return semantic_file; }

FILE *oracle_semantic(unsigned writer, const char *kind) {
  if (!semantic_file) {
    const char *path = getenv("IMAGE_SLASH_STAR_SEMANTIC_TRACE");
    if (!path || !(semantic_file = fopen(path, "wb"))) abort();
  }
  if (++semantic_records > 50000) abort();
  fprintf(semantic_file, "{\"kind\":\"%s\",\"writer\":%u,\"generation\":%u,\"cursor\":%u",
          kind, writer, frame_generation, oracle_position());
  return semantic_file;
}

void oracle_semantic_end(void) {
  if (fputs("}\n", semantic_file) < 0 || fflush(semantic_file)) abort();
  if (ftell(semantic_file) > 16 * 1024 * 1024) abort();
}

void oracle_frame_begin(const void *frame, unsigned width, unsigned height) {
  frame_identity = frame;
  ++frame_generation;
  origin_count = 0;
  FILE *f = oracle_semantic(0, "frame_begin");
  fprintf(f, ",\"visible\":[%u,%u]", width, height);
  oracle_semantic_end();
}

unsigned oracle_generation(const void *frame) {
  if (!frame_generation || frame != frame_identity) abort();
  return frame_generation;
}

unsigned oracle_next_origin(void) {
  if (++origin_counter > 50000) abort();
  return origin_counter;
}

void oracle_publish_origin(const void *frame, const void *slot, int kind, unsigned id) {
  (void)oracle_generation(frame);
  size_t i;
  for (i = 0; i < origin_count; ++i)
    if (origins[i].slot == slot && origins[i].kind == kind) break;
  if (i == origin_count && ++origin_count > 8192) abort();
  origins[i].slot = slot;
  origins[i].kind = kind;
  origins[i].id = id;
}

unsigned oracle_find_origin(const void *frame, const void *slot, int kind) {
  (void)oracle_generation(frame);
  for (size_t i = 0; i < origin_count; ++i)
    if (origins[i].slot == slot && origins[i].kind == kind) return origins[i].id;
  abort();
}

void oracle_cdf_reset(unsigned writer) {
  cdf_writer = writer;
  family_count = initial_count = 0;
}

void oracle_cdf_register(const char *name, const void *data, size_t bytes,
                         unsigned rank, const unsigned *shape) {
  if (family_count >= 48 || !rank || rank > 4) abort();
  size_t product = 2;
  for (unsigned i = 0; i < rank; ++i) {
    if (!shape[i] || product > SIZE_MAX / shape[i]) abort();
    product *= shape[i];
  }
  if (product != bytes) abort();
  families[family_count].name = name;
  families[family_count].address = (uintptr_t)data;
  families[family_count].bytes = bytes;
  families[family_count].rank = rank;
  memcpy(families[family_count].shape, shape, rank * sizeof(*shape));
  ++family_count;
}

void oracle_semantic_cdf(unsigned writer, const uint16_t *cdf, int symbols, int adaptive) {
  if (writer != cdf_writer || symbols < 2 || symbols > 16) abort();
  if (!adaptive) return; /* Raw partition CDFs can be temporary stack arrays. */
  for (size_t i = 0; i < initial_count; ++i) {
    if (initial_cdfs[i].address == cdf) {
      if (initial_cdfs[i].symbols != symbols) abort();
      return;
    }
  }
  if (initial_count >= 4096) abort();
  initial_cdfs[initial_count].address = cdf;
  initial_cdfs[initial_count++].symbols = symbols;
  const uintptr_t address = (uintptr_t)cdf;
  for (size_t i = 0; i < family_count; ++i) {
    const uintptr_t base = families[i].address;
    if (address < base || address - base >= families[i].bytes) continue;
    size_t offset = (size_t)(address - base);
    if (offset % 2 || (size_t)(symbols + 1) * 2 > families[i].bytes - offset) abort();
    FILE *f = oracle_semantic(writer, "initial_cdf");
    fprintf(f, ",\"family\":\"%s\",\"offset_u16\":%zu,\"shape\":[", families[i].name, offset / 2);
    for (unsigned j = 0; j < families[i].rank; ++j)
      fprintf(f, "%s%u", j ? "," : "", families[i].shape[j]);
    fputs("],\"values\":[", f);
    for (int j = 0; j <= symbols; ++j) fprintf(f, "%s%u", j ? "," : "", cdf[j]);
    fputc(']', f);
    oracle_semantic_end();
    return;
  }
  abort(); /* An unregistered syntax family needs an explicit evidence extension. */
}

void oracle_bytes(FILE *f, const uint8_t *data, int count) {
  if (count < 0 || count > 32) abort();
  fputc('[', f);
  for (int i = 0; i < count; ++i) fprintf(f, "%s%u", i ? "," : "", data[i]);
  fputc(']', f);
}
