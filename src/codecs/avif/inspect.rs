//! Portable AVIF ISO-BMFF container inspection.

use crate::types::ImageInfo;

/// Inspect AVIF dimensions, output mode, and presentation frame count.
pub fn inspect(data: &[u8]) -> Option<ImageInfo> {
    let _ = super::samples::validated(data)?;
    super::container::inspect(data)
}
