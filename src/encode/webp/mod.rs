//! Pure-Rust WebP encoder: internal VP8L lossless and VP8 lossy pipelines.

use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};
use std::io::Cursor;

pub mod vp8;

/// Encode a DecodedImage to WebP format.
///
/// Lossless uses the internal VP8L encoder.
/// Lossy: uses our own pure-Rust VP8 intra-frame encoder.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    if opts.lossless == Some(true) {
        encode_lossless(img, opts)
    } else {
        encode_lossy(img, opts)
    }
}

/// Lossless VP8L encoding via the internal `WebPEncoder`.
fn encode_lossless(img: &DecodedImage, _opts: &EncodeOptions) -> Option<Vec<u8>> {
    let (width, height) = (img.width, img.height);
    let color = match img.color {
        ColorType::Rgb8 => crate::webp_native::ColorType::Rgb8,
        ColorType::Rgba8 => crate::webp_native::ColorType::Rgba8,
        _ => return None,
    };

    let mut out = Cursor::new(Vec::new());
    let encoder = crate::webp_native::WebPEncoder::new(&mut out);
    encoder.encode(&img.pixels, width, height, color).ok()?;

    Some(out.into_inner())
}

/// Lossy VP8 encoding — own pure-Rust implementation.
///
/// Encodes VP8 keyframe bitstream in RIFF/WEBP container.
fn encode_lossy(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let quality = opts.quality.unwrap_or(80).min(100);
    let encoded = vp8::encoder::encode_vp8_lossy(&img.pixels, img.width, img.height, quality);
    Some(encoded)
}
