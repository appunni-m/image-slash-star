//! WebP codec.

pub mod decode;
pub mod encode;
pub mod inspect;
mod native;

pub(crate) const WEBP_MAX_ENCODER_DIMENSION: u32 = 16_383;
pub(crate) const VP8X_RESERVED_FLAG_MASK: u8 = 0b1100_0001;
