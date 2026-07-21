//! In-repository VP8, VP8L, and WebP container implementation.
//!
//! Derived from image-webp 0.2.4 under MIT OR Apache-2.0. The distributed
//! license texts and upstream README are retained in `third_party/image-webp`.
//!
// Keep the imported codec algorithms stable and auditable against upstream;
// project-specific Clippy policy applies at the wrapper boundary.
#![allow(clippy::all, clippy::nursery, clippy::pedantic, clippy::restriction)]

pub(crate) use self::decoder::{LoopCount, WebPDecoder};
pub(crate) use self::encoder::encode_alpha;
pub(crate) use self::encoder::{ColorType, WebPEncoder};

mod alpha_blending;
mod byteorder_lite;
mod decoder;
mod encoder;
mod extended;
mod huffman;
mod loop_filter;
mod lossless;
mod lossless_transform;
mod transform;
pub(crate) mod vp8;
mod vp8_arithmetic_decoder;
mod yuv;

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    decoder::__coverage_exercise_private_branches();
    encoder::__coverage_exercise_private_branches();
    extended::__coverage_exercise_private_branches();
    huffman::__coverage_exercise_private_branches();
    lossless::__coverage_exercise_private_branches();
    lossless_transform::__coverage_exercise_private_branches();
    vp8::__coverage_exercise_private_branches();
}
