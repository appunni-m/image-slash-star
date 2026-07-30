//! Portable AVIF ISO-BMFF container inspection.

use crate::codecs::CodecResult;
use crate::types::ImageInfo;

/// Inspect AVIF dimensions, output mode, and presentation frame count.
pub fn inspect(data: &[u8]) -> CodecResult<ImageInfo> {
    let _ = super::samples::validated(data)
        .map_err(|error| error.context("AVIF container validation failed"))?;
    super::container::inspect(data)
}
