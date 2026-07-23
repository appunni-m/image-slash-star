//! In-repository VP8, VP8L, and WebP container implementation.
//!
//! Derived from image-webp 0.2.4 under MIT OR Apache-2.0. The distributed
//! license texts and upstream README are retained in `third_party/image-webp`.
//!
//! Every child module has completed its file-level arithmetic and conversion
//! audit against the pinned Pillow/libwebp fixtures. Exceptions are scoped to
//! the exact reference kernels or invariants that require them.

#![warn(clippy::all)]
#![deny(
    clippy::clone_on_copy,
    clippy::expect_used,
    clippy::large_enum_variant,
    clippy::map_unwrap_or,
    clippy::needless_borrow,
    clippy::needless_collect,
    clippy::needless_range_loop,
    clippy::redundant_clone,
    clippy::todo,
    clippy::unnecessary_cast,
    clippy::unnecessary_to_owned,
    clippy::unwrap_in_result,
    clippy::unwrap_used
)]

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
    byteorder_lite::__coverage_exercise_private_branches();
    decoder::__coverage_exercise_private_branches();
    encoder::__coverage_exercise_private_branches();
    extended::__coverage_exercise_private_branches();
    huffman::__coverage_exercise_private_branches();
    lossless::__coverage_exercise_private_branches();
    lossless_transform::__coverage_exercise_private_branches();
    vp8::__coverage_exercise_private_branches();
}
