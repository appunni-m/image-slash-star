//! AVIF decoding entry point.
//!
//! The AV1 bitstream decoder is not implemented yet, so the `avif` feature
//! deliberately reports AVIF input as unsupported instead of returning
//! placeholder pixels that could be mistaken for Pillow parity.

use crate::types::DecodedImage;

/// Decode AVIF bytes once the pure-Rust AV1 implementation is available.
#[must_use]
pub fn decode(_data: &[u8]) -> Option<DecodedImage> {
    None
}
