//! Decode dispatcher — decode_format routes to per-format decoders.

use crate::types::{DecodedImage, ImageFormat};

#[cfg(feature = "avif")]
pub mod avif;
#[cfg(feature = "bmp")]
pub mod bmp;
#[cfg(feature = "gif")]
pub mod gif;
#[cfg(feature = "ico")]
pub mod ico;
#[cfg(feature = "jpeg")]
pub mod jpeg;
#[cfg(feature = "png")]
pub mod png;
#[cfg(feature = "tiff")]
pub mod tiff;
#[cfg(feature = "webp")]
pub mod webp;

/// Dispatch decoding to the appropriate format-specific decoder.
pub fn decode_format(_data: &[u8], format: ImageFormat) -> Option<DecodedImage> {
    match format {
        #[cfg(feature = "jpeg")]
        ImageFormat::Jpeg => jpeg::decode(_data),
        #[cfg(not(feature = "jpeg"))]
        ImageFormat::Jpeg => None,
        #[cfg(feature = "png")]
        ImageFormat::Png => png::decode(_data),
        #[cfg(not(feature = "png"))]
        ImageFormat::Png => None,
        #[cfg(feature = "gif")]
        ImageFormat::Gif => gif::decode(_data),
        #[cfg(not(feature = "gif"))]
        ImageFormat::Gif => None,
        #[cfg(feature = "bmp")]
        ImageFormat::Bmp => bmp::decode(_data),
        #[cfg(not(feature = "bmp"))]
        ImageFormat::Bmp => None,
        #[cfg(feature = "tiff")]
        ImageFormat::Tiff => tiff::decode(_data),
        #[cfg(not(feature = "tiff"))]
        ImageFormat::Tiff => None,
        #[cfg(feature = "webp")]
        ImageFormat::WebP => webp::decode(_data),
        #[cfg(not(feature = "webp"))]
        ImageFormat::WebP => None,
        #[cfg(feature = "ico")]
        ImageFormat::Ico => ico::decode(_data),
        #[cfg(not(feature = "ico"))]
        ImageFormat::Ico => None,
        #[cfg(feature = "avif")]
        ImageFormat::Avif => avif::decode(_data),
        #[cfg(not(feature = "avif"))]
        ImageFormat::Avif => None,
    }
}
