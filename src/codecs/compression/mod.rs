//! Internal lossless compression primitives used by image codecs.

#[cfg_attr(not(feature = "png"), allow(dead_code))]
pub(crate) mod deflate;
#[cfg(any(feature = "png", feature = "tiff"))]
#[cfg_attr(not(feature = "png"), allow(dead_code))]
mod zlib_ng;
