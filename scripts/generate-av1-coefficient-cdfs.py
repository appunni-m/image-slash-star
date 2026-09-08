#!/usr/bin/env python3
"""Generate the complete portable AV1 coefficient and transform-CDF defaults.

The input is the pinned rav1d-safe 0.5.7 `src/cdf.rs` source. rav1d-safe is a
safe Rust transcription of dav1d's normative default tables; this generator
extracts the four `CdfCoefContext` values plus the intra transform-type mode
tables and removes alignment wrappers. The emitted values retain
rav1d/dav1d's complemented range-decoder form and adaptive-count padding.
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path


PINNED_SOURCE = "rav1d-safe 0.5.7"


def balanced_array(source: str, start: int) -> tuple[str, int]:
    if source[start] != "[":
        raise ValueError("coefficient initializer does not begin with '['")
    depth = 0
    for index in range(start, len(source)):
        character = source[index]
        if character == "[":
            depth += 1
        elif character == "]":
            depth -= 1
            if depth == 0:
                return source[start : index + 1], index + 1
    raise ValueError("unterminated coefficient initializer")


def extract_defaults(source: str) -> str:
    root = source.index("static default_cdf: CdfDefaultContext = CdfDefaultContext {")
    marker = "    coef: ["
    marker_start = source.index(marker, root)
    array_start = marker_start + len(marker) - 1
    initializer, end = balanced_array(source, array_start)
    if not source[end:].lstrip().startswith(",\n    m:"):
        raise ValueError("coefficient initializer ended at an unexpected field")
    initializer = initializer.replace("CdfCoefContext", "CoefficientCdfs")
    initializer = re.sub(
        r"Aligned\((cdf[123]d\(.*?\))\)",
        r"\1",
        initializer,
        flags=re.DOTALL,
    )
    if "Aligned(" in initializer:
        raise ValueError("unconverted alignment wrapper in coefficient defaults")
    if initializer.count("CoefficientCdfs {") != 4:
        raise ValueError("expected exactly four quantizer-category defaults")
    return initializer


def extract_mode_array(source: str, field: str) -> str:
    root = source.index("static default_cdf: CdfDefaultContext = CdfDefaultContext {")
    marker = f"        {field}: Aligned(cdf2d("
    marker_start = source.index(marker, root)
    array_start = source.index("[", marker_start + len(marker))
    initializer, end = balanced_array(source, array_start)
    if not source[end:].startswith("))"):
        raise ValueError(f"{field} initializer ended at an unexpected token")
    return initializer


def render(coefficient_initializer: str, intra1: str, intra2: str) -> str:
    return f'''//! Generated AV1 coefficient-CDF defaults for the portable decoder.
//!
//! Source: {PINNED_SOURCE} `src/cdf.rs`, whose values transcribe dav1d's
//! `default_coef_cdf`. Regenerate with
//! `scripts/generate-av1-coefficient-cdfs.py`; do not edit the table by hand.

#[derive(Clone)]
pub(super) struct CoefficientCdfs {{
    pub(super) eob_bin_16: [[[u16; 8]; 2]; 2],
    pub(super) eob_bin_32: [[[u16; 8]; 2]; 2],
    pub(super) eob_bin_64: [[[u16; 8]; 2]; 2],
    pub(super) eob_bin_128: [[[u16; 8]; 2]; 2],
    pub(super) eob_bin_256: [[[u16; 16]; 2]; 2],
    pub(super) eob_bin_512: [[u16; 16]; 2],
    pub(super) eob_bin_1024: [[u16; 16]; 2],
    pub(super) eob_base_tok: [[[[u16; 4]; 4]; 2]; 5],
    pub(super) base_tok: [[[[u16; 4]; 41]; 2]; 5],
    pub(super) br_tok: [[[[u16; 4]; 21]; 2]; 4],
    pub(super) eob_hi_bit: [[[[u16; 2]; 11]; 2]; 5],
    pub(super) skip: [[[u16; 2]; 13]; 5],
    pub(super) dc_sign: [[[u16; 2]; 3]; 2],
}}

#[derive(Clone)]
pub(super) struct TransformTypeCdfs {{
    pub(super) intra1: [[[u16; 8]; 13]; 2],
    pub(super) intra2: [[[u16; 8]; 13]; 3],
}}

const fn cdf0d<const P: usize, const N: usize>(probabilities: [u16; P]) -> [u16; N] {{
    assert!(P < N);
    let mut cdf = [0; N];
    let mut index = 0_usize;
    while index < P {{
        cdf[index] = 32_768_u16.wrapping_sub(probabilities[index]) & !32_768;
        index = index.saturating_add(1);
    }}
    cdf
}}

const fn cdf1d<const P: usize, const N: usize, const M: usize>(
    probabilities: [[u16; P]; M],
) -> [[u16; N]; M] {{
    let mut cdf = [[0; N]; M];
    let mut index = 0_usize;
    while index < M {{
        cdf[index] = cdf0d(probabilities[index]);
        index = index.saturating_add(1);
    }}
    cdf
}}

const fn cdf2d<const P: usize, const N: usize, const M: usize, const L: usize>(
    probabilities: [[[u16; P]; M]; L],
) -> [[[u16; N]; M]; L] {{
    let mut cdf = [[[0; N]; M]; L];
    let mut index = 0_usize;
    while index < L {{
        cdf[index] = cdf1d(probabilities[index]);
        index = index.saturating_add(1);
    }}
    cdf
}}

const fn cdf3d<
    const P: usize,
    const N: usize,
    const M: usize,
    const L: usize,
    const K: usize,
>(probabilities: [[[[u16; P]; M]; L]; K]) -> [[[[u16; N]; M]; L]; K] {{
    let mut cdf = [[[[0; N]; M]; L]; K];
    let mut index = 0_usize;
    while index < K {{
        cdf[index] = cdf2d(probabilities[index]);
        index = index.saturating_add(1);
    }}
    cdf
}}

#[rustfmt::skip]
pub(super) static DEFAULT_COEFFICIENT_CDFS: [CoefficientCdfs; 4] = {coefficient_initializer};

#[rustfmt::skip]
pub(super) const DEFAULT_TRANSFORM_TYPE_CDFS: TransformTypeCdfs = TransformTypeCdfs {{
    intra1: cdf2d({intra1}),
    intra2: cdf2d({intra2}),
}};
'''


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    arguments = parser.parse_args()
    source = arguments.source.read_text(encoding="utf-8")
    if "pub struct CdfCoefContext" not in source:
        raise ValueError(f"{arguments.source} is not a compatible rav1d CDF source")
    arguments.output.write_text(
        render(
            extract_defaults(source),
            extract_mode_array(source, "txtp_intra1"),
            extract_mode_array(source, "txtp_intra2"),
        ),
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
