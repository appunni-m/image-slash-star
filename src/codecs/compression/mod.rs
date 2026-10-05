//! Internal lossless compression primitives used by image codecs.

use crate::codecs::{CodecError, CodecResult};

type CompressionResult<T> = CodecResult<T>;

#[cfg(feature = "png")]
#[derive(Clone, Copy)]
pub(super) struct RepeatedInputChunks {
    row_len: usize,
    remaining: usize,
}

#[cfg(feature = "png")]
impl RepeatedInputChunks {
    pub(super) const fn new(row_len: usize, height: usize) -> Self {
        Self {
            row_len,
            remaining: height,
        }
    }
}

#[cfg(feature = "png")]
impl Iterator for RepeatedInputChunks {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining = self.remaining.saturating_sub(1);
        Some(self.row_len)
    }
}

fn malformed(stage: &'static str) -> CodecError {
    CodecError::Malformed(format!("invalid compressed stream: {stage}"))
}

#[cfg(feature = "png")]
fn parameter(stage: &'static str) -> CodecError {
    CodecError::Parameter(format!("invalid compression input: {stage}"))
}

#[cfg_attr(
    not(feature = "png"),
    allow(
        dead_code,
        reason = "The shared module's row-compression helpers are only used by the PNG feature."
    )
)]
pub(crate) mod deflate;
#[cfg(any(feature = "png", feature = "tiff"))]
#[cfg_attr(
    not(feature = "png"),
    allow(
        dead_code,
        reason = "The compressor extension is selected only by PNG's feature-gated encoder."
    )
)]
mod zlib_ng;
