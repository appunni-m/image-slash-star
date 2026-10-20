//! TIFF codec.

pub mod decode;
pub mod encode;
pub mod inspect;

pub(super) const PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS: u64 = 178_956_970;
