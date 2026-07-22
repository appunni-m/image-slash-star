//! AVIF container inspection through the pinned libavif parser.

use crate::types::{ImageFormat, ImageInfo, ImageMode};

/// Inspect AVIF dimensions, output mode, and presentation frame count.
#[cfg(not(target_arch = "wasm32"))]
pub fn inspect(data: &[u8]) -> Option<ImageInfo> {
    let decoder = super::native::Decoder::new(data)?;
    let info = decoder.info();
    Some(ImageInfo {
        format: ImageFormat::Avif,
        width: info.width,
        height: info.height,
        mode: if info.has_alpha {
            ImageMode::Rgba8
        } else {
            ImageMode::Rgb8
        },
        bit_depth: 8,
        palette: None,
        is_animated: info.frame_count > 1,
        frame_count: Some(info.frame_count),
    })
}

/// AVIF inspection is unavailable in the core-only WebAssembly build.
#[cfg(target_arch = "wasm32")]
pub fn inspect(_data: &[u8]) -> Option<ImageInfo> {
    None
}
