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
        ColorType::Rgb8 => super::native::ColorType::Rgb8,
        ColorType::Rgba8 => super::native::ColorType::Rgba8,
        _ => return None,
    };

    let mut out = Cursor::new(Vec::new());
    let encoder = super::native::WebPEncoder::new(&mut out);
    encoder.encode(&img.pixels, width, height, color).ok()?;

    Some(out.into_inner())
}

/// Lossy VP8 encoding — own pure-Rust implementation.
///
/// Encodes VP8 keyframe bitstream in RIFF/WEBP container.
fn encode_lossy(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let quality = opts.quality.unwrap_or(80).min(100);
    let encoded = match img.color {
        ColorType::Rgb8 => {
            vp8::encoder::encode_vp8_lossy(&img.pixels, img.width, img.height, quality)
        }
        ColorType::Rgba8 => {
            let has_alpha = img.pixels.chunks_exact(4).any(|pixel| pixel[3] != u8::MAX);
            if has_alpha {
                let alpha = img
                    .pixels
                    .chunks_exact(4)
                    .map(|pixel| pixel[3])
                    .collect::<Vec<_>>();
                let alpha_chunk =
                    super::native::encode_alpha(&alpha, img.width, img.height).ok()?;
                vp8::encoder::encode_vp8_lossy_rgba(
                    &img.pixels,
                    img.width,
                    img.height,
                    quality,
                    &alpha_chunk,
                )
            } else {
                let rgb = img
                    .pixels
                    .chunks_exact(4)
                    .flat_map(|pixel| pixel[..3].iter().copied())
                    .collect::<Vec<_>>();
                vp8::encoder::encode_vp8_lossy(&rgb, img.width, img.height, quality)
            }
        }
        _ => return None,
    };
    Some(encoded)
}
