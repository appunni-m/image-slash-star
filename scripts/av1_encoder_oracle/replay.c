/* Input-only replay through unmodified pinned libaom entropy primitives. */
#include "aom_dsp/prob.h"
#include "oracle_trace.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void boolean(unsigned id, od_ec_enc *ec, int bit, unsigned probability) {
  oracle_before(id, "bool", ec, bit, probability, NULL, 0, 0);
  od_ec_encode_bool_q15(ec, bit, probability);
  oracle_after(ec, NULL, 0);
}

static void symbol(unsigned id, od_ec_enc *ec, int value, uint16_t *cdf,
                   int symbols, int adaptive, int update) {
  int length = symbols + adaptive;
  oracle_before(id, adaptive ? "symbol" : "cdf", ec, value, 0, cdf, length, update);
  od_ec_encode_cdf_q15(ec, value, cdf, symbols);
  if (update) update_cdf(cdf, (int8_t)value, symbols);
  oracle_after(ec, cdf, length);
}

static void start(unsigned id, od_ec_enc *ec) {
  /* Tiny initial storage independently exercises native growth. */
  od_ec_enc_init(ec, 1);
  oracle_start(id, ec);
}

static void finish(unsigned id, od_ec_enc *ec) {
  uint32_t size = 0;
  oracle_finish_before(id, ec);
  uint8_t *bytes = od_ec_enc_done(ec, &size);
  oracle_finish_after(ec, bytes, size);
  od_ec_enc_clear(ec);
}

static void boundaries(void) {
  od_ec_enc ec;
  unsigned id = 1;
  start(id, &ec);
  finish(id++, &ec);
  for (int count = 1; count <= 8; ++count) {
    start(id, &ec);
    for (int bit = 0; bit < count; ++bit) boolean(id, &ec, bit & 1, 16384);
    finish(id++, &ec);
  }
  for (int symbols = 2; symbols <= 16; ++symbols) {
    uint16_t cdf[17] = { 0 };
    for (int i = 0; i < symbols - 1; ++i)
      cdf[i] = (uint16_t)(32768 - ((i + 1) * 32768 / symbols));
    cdf[symbols] = 15;
    start(id, &ec);
    for (int step = 0; step < 40; ++step)
      symbol(id, &ec, step % symbols, cdf, symbols, 1, 1);
    /* Same mutable context with adaptation disabled. */
    symbol(id, &ec, 0, cdf, symbols, 1, 0);
    if (symbols > 2) cdf[1] = cdf[0];
    for (int value = 0; value < symbols; ++value)
      symbol(id, &ec, value, cdf, symbols, 0, 0);
    finish(id++, &ec);
  }
  start(id, &ec);
  uint32_t seed = 0x89392a3;
  for (unsigned step = 0; step < 2048; ++step) {
    seed = seed * 1664525u + 1013904223u;
    unsigned probability = step % 5 == 0 ? 1 : step % 5 == 1 ? 32767 : 1 + seed % 32767;
    boolean(id, &ec, (seed >> 31) & 1, probability);
  }
  finish(id, &ec);
}

int main(int argc, char **argv) {
  if (argc == 2 && !strcmp(argv[1], "--boundaries")) {
    boundaries();
    return 0;
  }
  if (argc != 2) return 1;
  FILE *input = fopen(argv[1], "r");
  if (!input) return 2;
  od_ec_enc ec;
  unsigned id = 0;
  int active = 0;
  char command;
  while (fscanf(input, " %c", &command) == 1) {
    if (command == 'S') {
      if (active || fscanf(input, "%u", &id) != 1) return 3;
      start(id, &ec);
      active = 1;
    } else if (!active) return 4;
    else if (command == 'F') { finish(id, &ec); active = 0; }
    else if (command == 'B') {
      int bit;
      unsigned probability;
      if (fscanf(input, "%d %u", &bit, &probability) != 2 ||
          bit < 0 || bit > 1 || probability < 1 || probability > 32767) return 5;
      boolean(id, &ec, bit, probability);
    } else if (command == 'C' || command == 'A') {
      int value, symbols, update;
      uint16_t cdf[17];
      if (fscanf(input, "%d %d %d", &value, &symbols, &update) != 3 ||
          symbols < 2 || symbols > 16 || value < 0 || value >= symbols ||
          update < 0 || update > 1 || (command == 'C' && update)) return 6;
      int length = symbols + (command == 'A');
      for (int i = 0; i < length; ++i) {
        unsigned probability;
        if (fscanf(input, "%u", &probability) != 1 || probability > 32767) return 7;
        cdf[i] = (uint16_t)probability;
      }
      if (cdf[symbols - 1] || (command == 'A' && cdf[symbols] > 32)) return 8;
      for (int i = 1; i < symbols; ++i) if (cdf[i] > cdf[i - 1]) return 9;
      symbol(id, &ec, value, cdf, symbols, command == 'A', update);
    } else return 10;
  }
  if (active || ferror(input) || fclose(input)) return 11;
  return 0;
}
