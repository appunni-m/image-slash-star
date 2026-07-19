//! In-repository VP8, VP8L, and WebP container implementation.
//!
//! Derived from image-webp 0.2.4 under MIT OR Apache-2.0. The distributed
//! license texts and upstream README are retained in `third_party/image-webp`.
//!
//! The upstream public API is intentionally retained even though this crate's
//! current wrappers use only a subset of it.

#![allow(dead_code)]
// Keep the imported codec algorithms stable and auditable against upstream;
// project-specific Clippy policy applies at the wrapper boundary.
#![allow(clippy::all, clippy::nursery, clippy::pedantic, clippy::restriction)]

pub(crate) use self::decoder::WebPDecoder;
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
