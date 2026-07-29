#!/usr/bin/env python3
"""Generate exact scalar AV1 entropy traces from pinned dav1d 1.5.3.

The generated JSON is an implementation-boundary oracle. The script compiles a
small harness against dav1d's unmodified ``src/msac.c`` and records the decoded
value plus every externally comparable scalar decoder field after each call.
It does not import Pillow or invoke the Rust implementation.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT = ROOT / "tests" / "fixtures" / "outputs" / "av1_entropy.json"
DAV1D_COMMIT = "b546257f770768b2c88258c533da38b91a06f737"

CONFIG_H = """\
#define ARCH_AARCH64 0
#define ARCH_ARM 0
#define ARCH_LOONGARCH 0
#define ARCH_LOONGARCH64 0
#define ARCH_PPC64LE 0
#define ARCH_X86 0
#define ARCH_X86_32 0
#define ARCH_X86_64 0
#define HAVE_ASM 0
#define TRIM_DSP_FUNCTIONS 0
"""

HARNESS_C = r"""
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

#include "src/msac.h"

static const uint8_t input[] = {
    0x00, 0xff, 0x81, 0x7e, 0x55, 0xaa, 0x13, 0xec,
    0x42, 0xbd, 0x99, 0x66, 0x01, 0x80, 0xfe, 0x24,
    0xdb, 0x10, 0xef, 0x73, 0x8c, 0x31, 0xce, 0x5a,
    0xa5, 0x0f, 0xf0, 0x69, 0x96, 0x3c, 0xc3, 0x7f,
};

static const uint8_t partition_422_still_input[] = {
    0x00, 0xe2, 0x34, 0xfe, 0x35, 0xf6, 0xba,
    0x40, 0x26, 0xa9, 0xe0, 0xb7, 0x7e, 0x80,
};

static const uint8_t partition_422_frame_2_input[] = {
    0x0a, 0x05, 0x77, 0x97, 0xa7, 0xa0, 0x58,
    0x37, 0xfe, 0xb1, 0x1c, 0x88, 0x87,
};

static const uint8_t partition_422_frame_3_input[] = {
    0xf8, 0x3f, 0x9f, 0xfd, 0x73, 0xc0, 0x2f, 0xa5,
    0x59, 0x48, 0xfa, 0xc5, 0xe5, 0x74, 0x87, 0x85,
    0xca, 0xc6, 0x00, 0x81, 0x5d, 0xa5, 0x3a, 0x6e,
    0xfa, 0xf3, 0x7c, 0x24, 0x18, 0x0b, 0xfc, 0x69,
    0x2c, 0x41, 0x07, 0x3b, 0x72, 0x2e, 0xcf, 0xff,
    0xb0, 0x2a, 0x3b, 0x55, 0x45, 0x22, 0x47, 0xbb,
    0x8c, 0x3c, 0x03, 0xb2, 0x19, 0xe9, 0xdf, 0x68,
    0xca, 0xf0, 0x15, 0x6e, 0xc0, 0xe7, 0x9d, 0x21,
    0xff, 0x54, 0xf6, 0xce, 0x30, 0x93, 0x63, 0x6f,
    0x59, 0x97, 0x89, 0xba, 0x72,
};

static void dump(const char *name, unsigned step, int value,
                 const MsacContext *s, const uint8_t *base,
                 const uint16_t *cdf, unsigned cdf_len)
{
    printf("{\"case\":\"%s\",\"step\":%u,\"value\":%d,"
           "\"byte_position\":%td,\"difference\":%llu,"
           "\"range\":%u,\"count\":%d,\"cdf\":[",
           name, step, value, s->buf_pos - base,
           (unsigned long long)s->dif, s->rng, s->cnt);
    for (unsigned i = 0; i < cdf_len; i++) {
        if (i) putchar(',');
        printf("%u", cdf[i]);
    }
    puts("]}");
}

static MsacContext init(int disable_cdf_update) {
    MsacContext s;
    dav1d_msac_init(&s, input, sizeof(input), disable_cdf_update);
    return s;
}

static void dump_partition(const char *name,
                           const uint8_t *data, size_t size)
{
    uint16_t cdf[] =
        { 12631, 11221, 9690, 3202, 2931, 2507, 2244, 1876, 1044, 0 };
    MsacContext s;
    dav1d_msac_init(&s, data, size, 0);
    dump(name, 0, -1, &s, data, cdf, 10);
    dump(name, 1, (int)dav1d_msac_decode_symbol_adapt_c(&s, cdf, 9),
         &s, data, cdf, 10);
}

static void dump_restoration_partition(const char *name,
                                       const uint8_t *data, size_t size)
{
    static const uint8_t sgr_activity[16][2] = {
        { 1, 1 }, { 1, 1 }, { 1, 1 }, { 1, 1 },
        { 1, 1 }, { 1, 1 }, { 1, 1 }, { 1, 1 },
        { 1, 1 }, { 1, 1 }, { 0, 1 }, { 0, 1 },
        { 0, 1 }, { 0, 1 }, { 1, 0 }, { 1, 0 },
    };
    uint16_t sgr_cdf[] = { 15913, 0 };
    uint16_t partition_cdf[] =
        { 12631, 11221, 9690, 3202, 2931, 2507, 2244, 1876, 1044, 0 };
    MsacContext s;
    unsigned step = 0;
    dav1d_msac_init(&s, data, size, 0);
    dump(name, step++, -1, &s, data, sgr_cdf, 2);
    for (unsigned plane = 0; plane < 3; plane++) {
        const unsigned enabled = dav1d_msac_decode_bool_adapt_c(&s, sgr_cdf);
        dump(name, step++, (int)enabled, &s, data, sgr_cdf, 2);
        if (!enabled) continue;
        const unsigned index = dav1d_msac_decode_bools(&s, 4);
        dump(name, step++, (int)index, &s, data, NULL, 0);
        if (sgr_activity[index][0]) {
            const int weight = dav1d_msac_decode_subexp(&s, 64, 128, 4) - 96;
            dump(name, step++, weight, &s, data, NULL, 0);
        }
        if (sgr_activity[index][1]) {
            const int weight = dav1d_msac_decode_subexp(&s, 63, 128, 4) - 32;
            dump(name, step++, weight, &s, data, NULL, 0);
        }
    }
    dump(name, step,
         (int)dav1d_msac_decode_symbol_adapt_c(&s, partition_cdf, 9),
         &s, data, partition_cdf, 10);
}

int main(void) {
    MsacContext s = init(1);
    dump("equal", 0, -1, &s, input, NULL, 0);
    for (unsigned i = 0; i < 16; i++)
        dump("equal", i + 1, (int)dav1d_msac_decode_bool_equi_c(&s),
             &s, input, NULL, 0);

    static const unsigned probabilities[] =
        { 0, 1, 4096, 8192, 16384, 24576, 32767 };
    s = init(1);
    dump("fixed", 0, -1, &s, input, NULL, 0);
    for (unsigned i = 0; i < sizeof(probabilities) / sizeof(*probabilities); i++)
        dump("fixed", i + 1,
             (int)dav1d_msac_decode_bool_c(&s, probabilities[i]),
             &s, input, NULL, 0);

    uint16_t bool_cdf[] = { 16384, 0 };
    s = init(0);
    dump("adaptive_bool", 0, -1, &s, input, bool_cdf, 2);
    for (unsigned i = 0; i < 16; i++)
        dump("adaptive_bool", i + 1,
             (int)dav1d_msac_decode_bool_adapt_c(&s, bool_cdf),
             &s, input, bool_cdf, 2);

    uint16_t symbol_cdf[] = { 24576, 16384, 8192, 0 };
    s = init(0);
    dump("adaptive_symbol", 0, -1, &s, input, symbol_cdf, 4);
    for (unsigned i = 0; i < 16; i++)
        dump("adaptive_symbol", i + 1,
             (int)dav1d_msac_decode_symbol_adapt_c(&s, symbol_cdf, 3),
             &s, input, symbol_cdf, 4);

    uint16_t frozen_symbol_cdf[] = { 24576, 16384, 8192, 0 };
    s = init(1);
    dump("frozen_symbol", 0, -1, &s, input, frozen_symbol_cdf, 4);
    for (unsigned i = 0; i < 8; i++)
        dump("frozen_symbol", i + 1,
             (int)dav1d_msac_decode_symbol_adapt_c(&s, frozen_symbol_cdf, 3),
             &s, input, frozen_symbol_cdf, 4);

    uint16_t high_cdf[] = { 24576, 16384, 8192, 0 };
    s = init(0);
    dump("high_token", 0, -1, &s, input, high_cdf, 4);
    for (unsigned i = 0; i < 8; i++)
        dump("high_token", i + 1,
             (int)dav1d_msac_decode_hi_tok_c(&s, high_cdf),
             &s, input, high_cdf, 4);

    static const unsigned uniform_counts[] = { 2, 3, 5, 17, 255 };
    s = init(1);
    dump("uniform", 0, -1, &s, input, NULL, 0);
    for (unsigned i = 0; i < sizeof(uniform_counts) / sizeof(*uniform_counts); i++)
        dump("uniform", i + 1,
             dav1d_msac_decode_uniform(&s, uniform_counts[i]),
             &s, input, NULL, 0);

    static const int subexp_refs[] = { 0, 63, 127, 200 };
    s = init(1);
    dump("subexponential", 0, -1, &s, input, NULL, 0);
    for (unsigned i = 0; i < sizeof(subexp_refs) / sizeof(*subexp_refs); i++)
        dump("subexponential", i + 1,
             dav1d_msac_decode_subexp(&s, subexp_refs[i], 256, 5),
             &s, input, NULL, 0);

    dump_partition("partition_422_still", partition_422_still_input,
                   sizeof(partition_422_still_input));
    dump_partition("partition_422_frame_2", partition_422_frame_2_input,
                   sizeof(partition_422_frame_2_input));
    dump_restoration_partition("restoration_422_frame_3",
                               partition_422_frame_3_input,
                               sizeof(partition_422_frame_3_input));

    return 0;
}
"""


def command_output(command: list[str]) -> str:
    return subprocess.run(
        command,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout.strip()


def verify_source(source: Path) -> None:
    required = [
        source / "src" / "msac.c",
        source / "src" / "msac.h",
        source / "src" / "decode.c",
        source / "src" / "cdf.c",
        source / "src" / "tables.c",
    ]
    if not all(path.is_file() for path in required):
        raise RuntimeError(f"{source} is not a dav1d source checkout")
    commit = command_output(["git", "-C", str(source), "rev-parse", "HEAD"])
    if commit != DAV1D_COMMIT:
        raise RuntimeError(
            f"dav1d source must be exact commit {DAV1D_COMMIT}, found {commit}"
        )


def generate(source: Path, output: Path, compiler: str) -> None:
    verify_source(source)
    with tempfile.TemporaryDirectory(prefix="image-star-msac-") as directory:
        temporary = Path(directory)
        (temporary / "config.h").write_text(CONFIG_H)
        harness = temporary / "harness.c"
        harness.write_text(HARNESS_C)
        executable = temporary / "msac-trace"
        subprocess.run(
            [
                compiler,
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-I",
                str(temporary),
                "-I",
                str(source),
                "-I",
                str(source / "include"),
                str(source / "src" / "msac.c"),
                str(harness),
                "-o",
                str(executable),
            ],
            check=True,
        )
        records = [
            json.loads(line)
            for line in command_output([str(executable)]).splitlines()
        ]

    document = {
        "format_version": 2,
        "oracle": {
            "implementation": "dav1d",
            "version": "1.5.3",
            "commit": DAV1D_COMMIT,
            "source_files": [
                "src/msac.c",
                "src/msac.h",
                "src/decode.c",
                "src/cdf.c",
                "src/tables.c",
            ],
        },
        "input_hex": (
            "00ff817e55aa13ec42bd99660180fe24"
            "db10ef738c31ce5aa50ff069963cc37f"
        ),
        "partition_422_inputs": {
            "still": "00e234fe35f6ba4026a9e0b77e80",
            "frame_2": "0a057797a7a05837feb11c8887",
            "frame_3": (
                "f83f9ffd73c02fa55948fac5e5748785cac600815da53a6efaf37c24180bfc69"
                "2c41073b722ecfffb02a3b55452247bb8c3c03b219e9df68caf0156ec0e79d21"
                "ff54f6ce3093636f599789ba72"
            ),
        },
        "records": records,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(document, indent=2) + "\n")
    print(f"wrote {len(records)} dav1d entropy states to {output}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--dav1d-source",
        type=Path,
        required=True,
        help=f"dav1d checkout at exact commit {DAV1D_COMMIT}",
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--cc", default="cc")
    arguments = parser.parse_args()
    generate(arguments.dav1d_source.resolve(), arguments.output.resolve(), arguments.cc)


if __name__ == "__main__":
    main()
