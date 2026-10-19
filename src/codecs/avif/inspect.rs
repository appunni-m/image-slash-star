//! Portable AVIF ISO-BMFF container inspection.

use crate::codecs::{CodecError, CodecResult};
use crate::types::ImageInfo;

/// Inspect AVIF dimensions, output mode, and presentation frame count.
pub fn inspect(data: &[u8]) -> CodecResult<ImageInfo> {
    let extracted = super::samples::inspected(data)
        .map_err(|error| error.context("AVIF container validation failed"))?;
    let mut info = super::container::inspect(data, extracted.grid_properties)?;

    // The primary-item count is authoritative for `avif` files, while `avis`
    // and other track-led brands present the sample-to-chunk expansion. The
    // lightweight container inspector reads `stsz`; use the already validated
    // track mapping for the presentation count when that track is preferred.
    let track_led = info
        .source
        .avif_file_type()
        .is_some_and(|file_type| file_type.major_brand() != *b"avif");
    if track_led && let Some(sequence) = extracted.sequence.as_ref() {
        let frame_count = u32::try_from(sequence.color.samples.len()).map_err(|_| {
            CodecError::Malformed(
                "AVIF sequence sample count exceeds the public frame-count range".to_owned(),
            )
        })?;
        info.frame_count = Some(frame_count);
        info.frame_count_complete = true;
        info.is_animated = frame_count > 1;
    }

    Ok(info)
}
