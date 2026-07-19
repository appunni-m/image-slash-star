//! Encode dispatcher — encode_format routes to per-format encoders.

use crate::encode_options::EncodeOptions;
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

/// Dispatch encoding to the appropriate format-specific encoder.
pub fn encode_format(
    _img: &DecodedImage,
    format: ImageFormat,
    _opts: &EncodeOptions,
) -> Option<Vec<u8>> {
    match format {
        #[cfg(feature = "jpeg")]
        ImageFormat::Jpeg => jpeg::encode(_img, _opts),
        #[cfg(not(feature = "jpeg"))]
        ImageFormat::Jpeg => None,
        #[cfg(feature = "png")]
        ImageFormat::Png => png::encode(_img, _opts),
        #[cfg(not(feature = "png"))]
        ImageFormat::Png => None,
        #[cfg(feature = "gif")]
        ImageFormat::Gif => gif::encode(_img, _opts),
        #[cfg(not(feature = "gif"))]
        ImageFormat::Gif => None,
        #[cfg(feature = "bmp")]
        ImageFormat::Bmp => bmp::encode(_img, _opts),
        #[cfg(not(feature = "bmp"))]
        ImageFormat::Bmp => None,
        #[cfg(feature = "tiff")]
        ImageFormat::Tiff => tiff::encode(_img, _opts),
        #[cfg(not(feature = "tiff"))]
        ImageFormat::Tiff => None,
        #[cfg(feature = "webp")]
        ImageFormat::WebP => webp::encode(_img, _opts),
        #[cfg(not(feature = "webp"))]
        ImageFormat::WebP => None,
        #[cfg(feature = "ico")]
        ImageFormat::Ico => ico::encode(_img, _opts),
        #[cfg(not(feature = "ico"))]
        ImageFormat::Ico => None,
        #[cfg(feature = "avif")]
        ImageFormat::Avif => avif::encode(_img, _opts),
        #[cfg(not(feature = "avif"))]
        ImageFormat::Avif => None,
    }
}
