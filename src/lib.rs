#![cfg_attr(coverage, feature(coverage_attribute))]

//! image-slash-star — dependency-light pixel-perfect image decoders and encoders.
//!
//! Goal: produce bit-exact observable output against the pinned Pillow oracle
//! in `manifest.yaml`. `bytemuck` is the sole third-party Rust runtime utility
//! dependency. Default codecs are Rust-only and work on WASM; opt-in AVIF uses
//! the fixed native libavif stack on supported native targets.
//!
//! Architecture:
//!   &[u8] → decode() → `Decoded<DecodedImage>` { format, content }
//!   &[u8] → decode_sequence() → `Decoded<DecodedSequence>` { format, content }
//!   downstream consumers own any processing of `DecodedImage`.

// Retained as the project's one explicitly approved byte-layout utility.
use bytemuck as _;

mod codecs;
pub mod source;
pub mod types;

pub use source::EncodedImage;
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

/// One exact scalar AV1 entropy state used by the fixture-backed coverage gate.
#[cfg(all(coverage, feature = "avif", not(target_arch = "wasm32")))]
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Av1EntropyTraceState {
    /// Oracle case name.
    pub case: &'static str,
    /// Operation index inside the case.
    pub step: u32,
    /// Decoded boolean, symbol, or integer.
    pub value: i32,
    /// Logical bytes consumed from the tile.
    pub byte_position: usize,
    /// Arithmetic difference window.
    pub difference: u64,
    /// Normalized arithmetic range.
    pub range: u32,
    /// Refill bit count.
    pub count: i32,
    /// Complete adaptive CDF after the operation.
    pub cdf: Vec<u16>,
}

/// Produce scalar AV1 entropy states for the pinned fixture operation sequence.
#[cfg(all(coverage, feature = "avif", not(target_arch = "wasm32")))]
#[doc(hidden)]
pub fn __coverage_av1_entropy_reference_trace() -> Result<Vec<Av1EntropyTraceState>, &'static str> {
    codecs::__coverage_av1_entropy_reference_trace()
}

/// Codec-private Y/U/V samples reconstructed by the first supported AV1 leaf.
#[cfg(all(coverage, feature = "avif", not(target_arch = "wasm32")))]
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Av1ReconstructionTrace {
    /// Visible decoded width.
    pub width: u32,
    /// Visible decoded height.
    pub height: u32,
    /// AV1 decoded sample depth.
    pub bit_depth: u32,
    /// Whether the AV1 sequence is monochrome.
    pub monochrome: bool,
    /// CICP color-primaries value.
    pub color_primaries: u32,
    /// CICP transfer-characteristics value.
    pub transfer_characteristics: u32,
    /// CICP matrix-coefficients value.
    pub matrix_coefficients: u32,
    /// Whether AV1 declares full-range YUV.
    pub color_range: bool,
    /// Whether chroma is subsampled horizontally.
    pub subsampling_x: bool,
    /// Whether chroma is subsampled vertically.
    pub subsampling_y: bool,
    /// Three visible planes in Y, U, V order.
    pub planes: [Vec<u16>; 3],
    /// Every scalar range-decoder operation that produced the retained leaf.
    pub entropy_operations: Vec<Av1EntropyOperationState>,
}

/// One scalar range-decoder operation from a reconstructed AV1 leaf.
#[cfg(all(coverage, feature = "avif", not(target_arch = "wasm32")))]
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Av1EntropyOperationState {
    /// Scalar decoder operation.
    pub operation: &'static str,
    /// Probability, symbol-count-minus-one, or sentinel parameter.
    pub parameter: i32,
    /// Operation index in the tile decoder.
    pub step: u32,
    /// Decoded value.
    pub value: i32,
    /// Logical bytes consumed from the start of the tile.
    pub byte_position: usize,
    /// Arithmetic difference window.
    pub difference: u64,
    /// Normalized arithmetic range.
    pub range: u32,
    /// Refill bit count.
    pub count: i32,
    /// Complete adaptive CDF after the operation.
    pub cdf: Vec<u16>,
}

/// Return the retained production-path AV1 reconstruction for a fixture.
#[cfg(all(coverage, feature = "avif", not(target_arch = "wasm32")))]
#[doc(hidden)]
pub fn __coverage_av1_reconstruction(data: &[u8]) -> Option<Av1ReconstructionTrace> {
    codecs::__coverage_av1_reconstruction(data)
}

/// Exercise unsupported closed-leaf syntax by mutating one fixture's AV1 item.
#[cfg(all(coverage, feature = "avif", not(target_arch = "wasm32")))]
#[doc(hidden)]
pub fn __coverage_sweep_av1_first_leaf(data: &[u8]) {
    codecs::__coverage_sweep_av1_first_leaf(data);
}

#[cfg(coverage)]
#[doc(hidden)]
pub fn __coverage_exercise_private_branches() {
    let _ = decode_sequence(b"not an image");
    let image = DecodedImage::new(1, 1, vec![0], ColorType::L8);
    let _ = encode_default(&image, ImageFormat::Png);
    codecs::__coverage_exercise_private_branches();
    types::__coverage_exercise_private_branches();
}
pub mod encode_options;

use crate::encode_options::EncodeOptions;
