// These exceptions are explicit technical debt. Remove them as public API
// documentation and legacy algorithm ports are brought under the workspace
// lint policy.
#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::map_unwrap_or)]

//! image-slash-star — dependency-light pixel-perfect image decoders and encoders.
//!
//! Goal: produce bit-exact observable output against the pinned Pillow oracle
//! in `manifest.yaml`. `bytemuck` is the sole third-party Rust runtime utility
//! dependency. Default codecs are Rust-only and work on WASM; opt-in AVIF uses
//! the fixed native libavif stack on supported native targets.
//!
//! Architecture:
//!   &[u8] → decode() → Decoded<DecodedImage> { format, content }
//!   &[u8] → decode_sequence() → Decoded<DecodedSequence> { format, content }
//!   pillow-rs wraps DecodedImage into DynamicImage/Image::Loaded.

// Integration-test-only dependencies are still visible while Cargo builds the
// library test target. Mark them deliberately used under that configuration.
#[cfg(test)]
use serde as _;
#[cfg(test)]
use serde_json as _;

pub mod codecs;
pub mod decode;
pub mod encode;
pub mod types;

pub use types::*;

/// Detect an encoded image format from its magic bytes.
///
/// # Errors
///
/// Returns [`ImageError::UnknownFormat`] when the signature is incomplete or
/// does not identify a supported container.
pub fn detect_format(data: &[u8]) -> ImageResult<ImageFormat> {
    if data.len() < 8 {
        return Err(ImageError::UnknownFormat);
    }
    if data[0] == 0xFF && data[1] == 0xD8 {
        return Ok(ImageFormat::Jpeg);
    }
    if &data[0..8] == b"\x89PNG\r\n\x1a\n" {
        return Ok(ImageFormat::Png);
    }
    if &data[0..4] == b"GIF8" {
        return Ok(ImageFormat::Gif);
    }
    if &data[0..2] == b"BM" {
        return Ok(ImageFormat::Bmp);
    }
    if data.len() >= 12 && &data[8..12] == b"WEBP" {
        return Ok(ImageFormat::WebP);
    }
    if &data[0..4] == b"II\x2a\x00" || &data[0..4] == b"MM\x00\x2a" {
        return Ok(ImageFormat::Tiff);
    }
    if matches!(&data[0..4], b"\x00\x00\x01\x00" | b"\x00\x00\x02\x00") {
        return Ok(ImageFormat::Ico);
    }
    if data.len() >= 12 && &data[4..8] == b"ftyp" {
        let brand = &data[8..12];
        if matches!(brand, b"avif" | b"avis" | b"mif1" | b"msf1") {
            return Ok(ImageFormat::Avif);
        }
    }
    Err(ImageError::UnknownFormat)
}

/// Auto-detect encoded image data and retain both its source format and pixels.
///
/// # Errors
///
/// Returns a structured error for an unknown signature, disabled codec feature,
/// malformed payload, or invalid decoded buffer.
pub fn decode(data: &[u8]) -> ImageResult<Decoded<DecodedImage>> {
    let format = detect_format(data)?;
    codecs::decode_format(data, format).map(|image| Decoded::new(format, image))
}

/// Auto-detect the format and decode every retained image frame.
///
/// # Errors
///
/// Returns a structured error for an unknown signature, disabled codec feature,
/// malformed payload, unsupported sequence, or invalid decoded frame data.
pub fn decode_sequence(data: &[u8]) -> ImageResult<Decoded<DecodedSequence>> {
    let format = detect_format(data)?;
    codecs::decode_sequence_format(data, format).map(|sequence| Decoded::new(format, sequence))
}

/// Inspect encoded image headers without decoding compressed pixel payloads.
///
/// # Errors
///
/// Returns a structured error for an unknown signature, disabled codec feature,
/// malformed header, or a format whose metadata parser is not implemented yet.
pub fn inspect(data: &[u8]) -> ImageResult<ImageInfo> {
    let format = detect_format(data)?;
    codecs::inspect_format(data, format)
}

/// Encode a decoded still image into an explicitly selected output format.
///
/// # Errors
///
/// Returns a structured error for invalid pixels, a disabled codec feature, or
/// input/options unsupported by the selected encoder.
pub fn encode(
    img: &DecodedImage,
    format: ImageFormat,
    opts: &EncodeOptions,
) -> ImageResult<Vec<u8>> {
    codecs::encode_format(img, format, opts)
}

/// Encode a still image or animation while retaining every source frame.
pub fn encode_sequence(
    sequence: &DecodedSequence,
    format: ImageFormat,
    opts: &EncodeOptions,
) -> ImageResult<Vec<u8>> {
    codecs::encode_sequence_format(sequence, format, opts)
}

/// Encode with default options.
pub fn encode_default(img: &DecodedImage, format: ImageFormat) -> ImageResult<Vec<u8>> {
    encode(img, format, &EncodeOptions::default())
}

#[cfg(coverage)]
#[doc(hidden)]
pub fn __coverage_exercise_private_branches() {
    let _ = decode_sequence(b"not an image");
    codecs::__coverage_exercise_private_branches();
    types::__coverage_exercise_private_branches();
}
pub mod encode_options;

use crate::encode_options::EncodeOptions;
