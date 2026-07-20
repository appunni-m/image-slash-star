//! Feature-gated image codec implementations.
//!
//! Each format owns its decoding and encoding implementation so enabling one
//! Cargo feature pulls in only that codec and its private support code.

use crate::encode_options::EncodeOptions;
use crate::types::{DecodedImage, DecodedSequence, ImageFormat};

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

#[cfg(any(feature = "png", feature = "tiff"))]
mod compression;

/// Dispatch decoding to the enabled format implementation.
pub fn decode_format(_data: &[u8], format: ImageFormat) -> Option<DecodedImage> {
    let image: DecodedImage = match format {
        #[cfg(feature = "jpeg")]
        ImageFormat::Jpeg => jpeg::decode::decode(_data),
        #[cfg(not(feature = "jpeg"))]
        ImageFormat::Jpeg => None,
        #[cfg(feature = "png")]
        ImageFormat::Png => png::decode::decode(_data),
        #[cfg(not(feature = "png"))]
        ImageFormat::Png => None,
        #[cfg(feature = "gif")]
        ImageFormat::Gif => gif::decode::decode(_data),
        #[cfg(not(feature = "gif"))]
        ImageFormat::Gif => None,
        #[cfg(feature = "bmp")]
        ImageFormat::Bmp => bmp::decode::decode(_data),
        #[cfg(not(feature = "bmp"))]
        ImageFormat::Bmp => None,
        #[cfg(feature = "tiff")]
        ImageFormat::Tiff => tiff::decode::decode(_data),
        #[cfg(not(feature = "tiff"))]
        ImageFormat::Tiff => None,
        #[cfg(feature = "webp")]
        ImageFormat::WebP => webp::decode::decode(_data),
        #[cfg(not(feature = "webp"))]
        ImageFormat::WebP => None,
        #[cfg(feature = "ico")]
        ImageFormat::Ico => ico::decode::decode(_data),
        #[cfg(not(feature = "ico"))]
        ImageFormat::Ico => None,
        #[cfg(feature = "avif")]
        ImageFormat::Avif => avif::decode::decode(_data),
        #[cfg(not(feature = "avif"))]
        ImageFormat::Avif => None,
    }?;
    image.validate().ok()?;
    Some(image)
}

/// Dispatch decoding while retaining every frame and its presentation data.
pub fn decode_sequence_format(data: &[u8], format: ImageFormat) -> Option<DecodedSequence> {
    #[cfg(feature = "gif")]
    if format == ImageFormat::Gif {
        return gif::decode::decode_sequence(data);
    }

    #[cfg(feature = "webp")]
    if format == ImageFormat::WebP {
        return webp::decode::decode_sequence(data);
    }

    decode_format(data, format).map(DecodedSequence::from_image)
}

/// Dispatch encoding to the enabled format implementation.
pub fn encode_format(
    _image: &DecodedImage,
    format: ImageFormat,
    _options: &EncodeOptions,
) -> Option<Vec<u8>> {
    _image.validate().ok()?;
    match format {
        #[cfg(feature = "jpeg")]
        ImageFormat::Jpeg => jpeg::encode::encode(_image, _options),
        #[cfg(not(feature = "jpeg"))]
        ImageFormat::Jpeg => None,
        #[cfg(feature = "png")]
        ImageFormat::Png => png::encode::encode(_image, _options),
        #[cfg(not(feature = "png"))]
        ImageFormat::Png => None,
        #[cfg(feature = "gif")]
        ImageFormat::Gif => gif::encode::encode(_image, _options),
        #[cfg(not(feature = "gif"))]
        ImageFormat::Gif => None,
        #[cfg(feature = "bmp")]
        ImageFormat::Bmp => bmp::encode::encode(_image, _options),
        #[cfg(not(feature = "bmp"))]
        ImageFormat::Bmp => None,
        #[cfg(feature = "tiff")]
        ImageFormat::Tiff => tiff::encode::encode(_image, _options),
        #[cfg(not(feature = "tiff"))]
        ImageFormat::Tiff => None,
        #[cfg(feature = "webp")]
        ImageFormat::WebP => webp::encode::encode(_image, _options),
        #[cfg(not(feature = "webp"))]
        ImageFormat::WebP => None,
        #[cfg(feature = "ico")]
        ImageFormat::Ico => ico::encode::encode(_image, _options),
        #[cfg(not(feature = "ico"))]
        ImageFormat::Ico => None,
        #[cfg(feature = "avif")]
        ImageFormat::Avif => avif::encode::encode(_image, _options),
        #[cfg(not(feature = "avif"))]
        ImageFormat::Avif => None,
    }
}

/// Dispatch encoding without collapsing an animation to its first frame.
pub fn encode_sequence_format(
    sequence: &DecodedSequence,
    format: ImageFormat,
    options: &EncodeOptions,
) -> Option<Vec<u8>> {
    sequence.validate().ok()?;

    #[cfg(feature = "gif")]
    if format == ImageFormat::Gif {
        return gif::encode::encode_sequence(sequence, options);
    }

    let image = sequence.first()?;
    (sequence.frames.len() == 1).then(|| encode_format(image, format, options))?
}
