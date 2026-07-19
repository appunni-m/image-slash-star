//! Internal lossless compression primitives used by image codecs.

pub(crate) mod deflate;
#[cfg(any(feature = "png", feature = "tiff"))]
mod zlib_ng;
