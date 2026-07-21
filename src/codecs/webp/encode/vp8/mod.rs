//! VP8 intra-frame encoder modules (RFC 6386).
//!
//! These modules implement the building blocks for a lossy VP8 keyframe encoder:
//!
//! * `dct` — 4×4 forward DCT + Walsh-Hadamard Transform
//! * `quant` — Quantization tables, quality mapping, RGB→YUV conversion
//! * `tokenize` — DCT coefficient tokenization + probability tables
//! * `bool_enc` — VP8 boolean entropy encoder (range coder)

mod analysis;
mod bool_enc;
mod chroma;
mod cost;
mod dct;
pub(super) mod encoder;
mod frame;
mod intra16;
mod intra4;
mod mode_probability;
mod partition;
mod probability;
mod quant;
mod residual;
mod tokenize;

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    cost::__coverage_exercise_private_branches();
    encoder::__coverage_exercise_private_branches();
    intra16::__coverage_exercise_private_branches();
    probability::__coverage_exercise_private_branches();
}
