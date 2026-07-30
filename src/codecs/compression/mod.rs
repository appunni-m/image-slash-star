//! Internal lossless compression primitives used by image codecs.

use crate::codecs::{CodecError, CodecResult};

type CompressionResult<T> = CodecResult<T>;

fn malformed(stage: &'static str) -> CodecError {
    CodecError::Malformed(format!("invalid compressed stream: {stage}"))
}

fn parameter(stage: &'static str) -> CodecError {
    CodecError::Parameter(format!("invalid compression input: {stage}"))
}

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
