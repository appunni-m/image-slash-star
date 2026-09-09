#!/usr/bin/env python3
"""Generate the packed AV1 inverse-quantization matrix table.

The input is libaom's ``av1/common/quant_common.c``.  The generated Rust is
data-only and intentionally retains libaom's level/plane/transform packing so
the decoder can select a matrix with one checked slice instead of maintaining
fixture-specific copies.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


LEVELS = 15
PLANES = 2
MATRIX_VALUES = 3360
# The pinned libaom initializer omits the final 16-value chroma padding
# row.  The decoder never indexes that padding, but the Rust carrier keeps a
# uniform stride so all level/plane entries remain allocation-free.
SOURCE_VALUES = (LEVELS * PLANES - 1) * MATRIX_VALUES + (MATRIX_VALUES - 16)
TOTAL_VALUES = LEVELS * PLANES * MATRIX_VALUES
DECLARATION = (
    "static const qm_val_t iwt_matrix_ref"
    "[NUM_QM_LEVELS - 1][2][QM_TOTAL_SIZE] ="
)


def initializer(source: str) -> str:
    """Return the balanced initializer following ``DECLARATION``."""

    declaration = source.index(DECLARATION)
    opening = source.index("{", declaration + len(DECLARATION))
    depth = 0
    for index in range(opening, len(source)):
        character = source[index]
        if character == "{":
            depth += 1
        elif character == "}":
            depth -= 1
            if depth == 0:
                return source[opening : index + 1]
    raise ValueError("unterminated iwt_matrix_ref initializer")


def values(source: str) -> list[int]:
    body = initializer(source)
    body = re.sub(r"/\*.*?\*/", "", body, flags=re.DOTALL)
    body = re.sub(r"//.*", "", body)
    parsed = [int(value) for value in re.findall(r"\b\d+\b", body)]
    if len(parsed) == SOURCE_VALUES:
        parsed.extend([32] * (TOTAL_VALUES - SOURCE_VALUES))
    if len(parsed) != TOTAL_VALUES:
        raise ValueError(f"expected {SOURCE_VALUES} or {TOTAL_VALUES} values, found {len(parsed)}")
    if any(value > 255 for value in parsed):
        raise ValueError("inverse quantization matrix value exceeds u8")
    return parsed


def render(parsed: list[int]) -> str:
    lines = [
        "//! Generated AV1 inverse-quantization matrices.",
        "//!",
        "//! Source: libaom `av1/common/quant_common.c`, commit",
        "//! `5f31d477bfee2ddbfb6a182067f4be961872c3e2`.",
        "//! Regenerate with `scripts/generate-av1-inverse-quant-matrices.py`.",
        "",
        "// This is data from the normative reference implementation. Keeping",
        "// its exact packing makes transform-size selection allocation-free.",
        "#[rustfmt::skip]",
        "pub(super) static AV1_INVERSE_QUANT_MATRICES: [[[u8; 3344]; 2]; 15] = [",
    ]
    cursor = 0
    for level in range(LEVELS):
        lines.append(f"    [ // level {level}")
        for plane in range(PLANES):
            label = "luma" if plane == 0 else "chroma"
            lines.append(f"        [ // {label}")
            end = cursor + MATRIX_VALUES
            for offset in range(cursor, end, 32):
                row = ", ".join(str(value) for value in parsed[offset : offset + 32])
                lines.append(f"            {row},")
            cursor = end
            lines.append("        ],")
        lines.append("    ],")
    lines.append("];")
    lines.append("")
    if cursor != len(parsed):
        raise ValueError("generator did not consume the complete table")
    return "\n".join(lines)


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(f"usage: {sys.argv[0]} quant_common.c output.rs")
    source_path = Path(sys.argv[1])
    output_path = Path(sys.argv[2])
    output_path.write_text(render(values(source_path.read_text())), encoding="utf-8")


if __name__ == "__main__":
    main()
