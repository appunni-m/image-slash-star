//! Internal lossless compression primitives used by image codecs.

#[cfg_attr(not(feature = "png"), allow(dead_code))]
pub(crate) mod deflate;
#[cfg(any(feature = "png", feature = "tiff"))]
#[cfg_attr(not(feature = "png"), allow(dead_code))]
mod zlib_ng;

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    deflate::__coverage_exercise_private_branches();

    #[cfg(any(feature = "png", feature = "tiff"))]
    zlib_ng::__coverage_exercise_private_branches();
}
