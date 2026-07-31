#![cfg_attr(coverage, feature(coverage_attribute))]

//! Byte-oriented, dependency-constrained image codecs.
//!
//! `image-slash-star` detects, inspects, decodes, and encodes JPEG, PNG, GIF,
//! BMP, TIFF, WebP, ICO, and AVIF bytes. Its compatibility target is the exact
//! observable behavior recorded from the pinned Pillow oracle in the
//! repository manifest. The guarantee is manifest-bounded rather than a claim
//! to implement every legal file in every format specification.
//!
//! # Scope
//!
//! The crate owns encoded containers and validated decoded transfer models. It
//! does not open files or provide resizing, cropping, drawing, filtering,
//! arbitrary compositing, or another general image-processing layer.
//! Applications supply and receive byte buffers and keep host I/O policy
//! outside the codec boundary.
//!
//! # Quick start
//!
//! ```
//! use image_slash_star::{
//!     decode, encode_default, ImageError, ImageFormat, ImageMode, ImageResult,
//! };
//!
//! fn opaque_rgb_png_to_jpeg(input: &[u8]) -> ImageResult<Vec<u8>> {
//!     let decoded = decode(input)?;
//!     if decoded.format != ImageFormat::Png {
//!         return Err(ImageError::Unsupported {
//!             format: Some(decoded.format),
//!             message: "expected a PNG source".to_owned(),
//!         });
//!     }
//!     if decoded.content.mode != ImageMode::Rgb8 {
//!         return Err(ImageError::Unsupported {
//!             format: Some(ImageFormat::Jpeg),
//!             message: "JPEG example requires opaque RGB8 input".to_owned(),
//!         });
//!     }
//!     encode_default(&decoded.content, ImageFormat::Jpeg)
//! }
//! ```
//!
//! This intentionally narrow example performs no hidden image processing.
//! Alpha, palette, bilevel, and sixteen-bit PNG sources require an explicit
//! conversion policy in a downstream processing library before JPEG encoding.
//!
//! # Data model
//!
//! [`decode`] and [`decode_sequence`] auto-detect the input and return a
//! [`Decoded`] envelope. [`Decoded::format`] retains the source
//! [`ImageFormat`], while [`DecodedImage::mode`] and
//! [`DecodedImage::color`] describe the decoded sample bytes. Indexed images
//! retain their [`ImagePalette`] rather than becoming ambiguous luminance
//! buffers. [`ImageInfo::source`] and [`DecodedImage::source`] retain proved
//! structural source facts such as TIFF byte order without changing transfer
//! bytes. Every encode API requires an explicit target [`ImageFormat`].
//!
//! [`EncodedImage`] is an immutable source snapshot. It inspects at
//! construction and shares a once-initialized decode result across clones.
//!
//! # Features and targets
//!
//! Default features enable the Rust-only `jpeg`, `png`, `gif`, `bmp`, `tiff`,
//! `webp`, and `ico` codecs. `ico` also enables `png` and `bmp`. The `avif`
//! feature is opt-in: native builds use a version-locked libavif stack, while
//! `wasm32-unknown-unknown` currently supports portable inspection and a
//! documented, manifest-bounded still-decode subset. Portable AVIF sequence
//! decode and encoding are not complete.
//!
//! # Errors
//!
//! Canonical fallible operations return [`ImageResult`]. [`ImageError`]
//! distinguishes unknown signatures, disabled features, malformed data,
//! unsupported operations, invalid dimensions, invalid parameters, and
//! caller-controlled resource-limit failures.

// Retained as the project's one explicitly approved byte-layout utility.
use bytemuck as _;

pub mod capabilities;
mod codecs;
pub mod decode_policy;
pub mod encode_options;
pub mod source;
pub mod types;

pub use capabilities::{
    CODEC_OPERATIONS, Capability, CapabilityRestriction, CapabilityTarget,
    CapabilityUnavailableReason, CodecOperation, FormatCapabilities, all_capabilities,
};
pub(crate) use decode_policy::SequenceDecodeBudget;
pub use decode_policy::{DecodeLimits, DecodePolicy};
pub use encode_options::*;
pub use source::EncodedImage;
pub use types::*;

/// Detect an encoded image format from its magic bytes.
///
/// AVIF uses `avif`/`avis` major brands directly. Generic `mif1`/`msf1`
/// containers are identified as AVIF only when their complete bounded
/// `ftyp` box also declares an `avif` or `avis` compatible brand.
///
/// # Errors
///
/// Returns [`ImageError::UnknownFormat`] when the signature is incomplete or
/// does not identify a supported container.
pub fn detect_format(data: &[u8]) -> ImageResult<ImageFormat> {
    if data.starts_with(b"\xff\xd8\xff") {
        return Ok(ImageFormat::Jpeg);
    }
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(ImageFormat::Png);
    }
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        return Ok(ImageFormat::Gif);
    }
    if data.starts_with(b"BM") {
        return Ok(ImageFormat::Bmp);
    }
    if data.starts_with(b"RIFF")
        && data.get(8..12) == Some(b"WEBP")
        && matches!(data.get(12..16), Some(b"VP8 " | b"VP8L" | b"VP8X"))
    {
        return Ok(ImageFormat::WebP);
    }
    if matches!(
        data.get(..4),
        Some(
            b"MM\x00\x2a"
                | b"II\x2a\x00"
                | b"MM\x2a\x00"
                | b"II\x00\x2a"
                | b"MM\x00\x2b"
                | b"II\x2b\x00"
        )
    ) {
        return Ok(ImageFormat::Tiff);
    }
    if matches!(
        data.get(..4),
        Some(b"\x00\x00\x01\x00" | b"\x00\x00\x02\x00")
    ) {
        return Ok(ImageFormat::Ico);
    }
    if is_avif_signature(data) {
        return Ok(ImageFormat::Avif);
    }
    Err(ImageError::UnknownFormat)
}

fn is_avif_signature(data: &[u8]) -> bool {
    match data.get(4..12) {
        Some(b"ftypavif" | b"ftypavis") => true,
        Some(b"ftypmif1" | b"ftypmsf1") => {
            let size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
            size >= 20
                && size <= data.len()
                && size.is_multiple_of(4)
                && data[16..size]
                    .chunks_exact(4)
                    .any(|brand| matches!(brand, b"avif" | b"avis"))
        }
        _ => false,
    }
}

/// Auto-detect encoded image data and retain both its source format and pixels.
///
/// # Errors
///
/// Returns a structured error for an unknown signature, disabled codec feature,
/// malformed payload, or invalid decoded buffer.
pub fn decode(data: &[u8]) -> ImageResult<Decoded<DecodedImage>> {
    decode_with_policy(data, &DecodePolicy::default())
}

/// Decode with an explicit caller-controlled policy.
///
/// The encoded-input byte limit is checked before format detection. Configured
/// primary-canvas limits trigger a format-qualified inspection preflight
/// before pixel materialization.
///
/// # Errors
///
/// Returns [`ImageError::LimitExceeded`] when the complete input, inspected
/// primary canvas, its decoded transfer-byte length, or the materialized frame
/// count exceeds a configured maximum. Otherwise returns the same errors as
/// [`decode`].
pub fn decode_with_policy(
    data: &[u8],
    policy: &DecodePolicy,
) -> ImageResult<Decoded<DecodedImage>> {
    policy.check_encoded_input(data, CodecOperation::StillDecode)?;
    let format = detect_format(data)?;
    if policy.requires_image_info() {
        let info = codecs::inspect_format(data, format)?;
        policy.check_image_info(&info, CodecOperation::StillDecode)?;
    }
    codecs::decode_format(data, format).map(|image| Decoded::new(format, image))
}

/// Auto-detect the format and decode every retained image frame.
///
/// # Errors
///
/// Returns a structured error for an unknown signature, disabled codec feature,
/// malformed payload, unsupported sequence, or invalid decoded frame data.
pub fn decode_sequence(data: &[u8]) -> ImageResult<Decoded<DecodedSequence>> {
    decode_sequence_with_policy(data, &DecodePolicy::default())
}

/// Decode every retained frame with an explicit caller-controlled policy.
///
/// # Errors
///
/// Returns [`ImageError::LimitExceeded`] when the complete input, inspected
/// primary canvas, its decoded transfer-byte length, or the inspected frame
/// count exceeds a configured maximum, or when a later frame or the cumulative
/// retained sequence exceeds the configured decoded-byte maxima. Frame-count
/// and primary checks run before sequence materialization; later-frame and
/// cumulative byte checks run before each later frame's pixel work. Otherwise
/// returns the same errors as [`decode_sequence`].
pub fn decode_sequence_with_policy(
    data: &[u8],
    policy: &DecodePolicy,
) -> ImageResult<Decoded<DecodedSequence>> {
    policy.check_encoded_input(data, CodecOperation::SequenceDecode)?;
    let format = detect_format(data)?;
    let mut budget = policy.sequence_budget(format);
    if policy.requires_image_info() {
        let info = codecs::inspect_format(data, format)?;
        policy.check_image_info(&info, CodecOperation::SequenceDecode)?;
        budget.charge_primary(&info)?;
    }
    codecs::decode_sequence_format(data, format, &mut budget)
        .map(|sequence| Decoded::new(format, sequence))
}

/// Inspect encoded image headers without decoding compressed pixel payloads.
///
/// # Errors
///
/// Returns a structured error for an unknown signature, disabled codec feature,
/// malformed header, or metadata that the selected inspector cannot represent.
pub fn inspect(data: &[u8]) -> ImageResult<ImageInfo> {
    inspect_with_policy(data, &DecodePolicy::default())
}

/// Inspect encoded image headers with an explicit caller-controlled policy.
///
/// # Errors
///
/// Returns [`ImageError::LimitExceeded`] when the complete input, inspected
/// primary canvas, its decoded transfer-byte length, or the inspected frame
/// count exceeds a configured maximum. Otherwise returns the same errors as
/// [`inspect`].
pub fn inspect_with_policy(data: &[u8], policy: &DecodePolicy) -> ImageResult<ImageInfo> {
    policy.check_encoded_input(data, CodecOperation::Inspection)?;
    let format = detect_format(data)?;
    let info = codecs::inspect_format(data, format)?;
    policy.check_image_info(&info, CodecOperation::Inspection)?;
    Ok(info)
}

/// Encode a decoded still image into an explicitly selected output format.
///
/// # Errors
///
/// Returns a structured error for invalid pixels, a disabled codec feature, or
/// input/options unsupported by the selected encoder. `opts` must name the
/// same target as `format`; a mismatch returns a format-qualified
/// [`ImageError::Parameter`].
pub fn encode(
    img: &DecodedImage,
    format: ImageFormat,
    opts: &EncodeOptions,
) -> ImageResult<Vec<u8>> {
    codecs::encode_format(img, format, opts)
}

/// Encode a still image or animation while retaining every source frame.
///
/// Multi-frame output is currently supported for GIF, TIFF, WebP, and native
/// AVIF. Other formats accept a one-frame sequence and reject additional
/// retained frames.
///
/// # Errors
///
/// Returns a structured error for an invalid sequence, disabled codec feature,
/// unsupported multi-frame target, invalid encoder options, or an option value
/// whose target differs from `format`.
pub fn encode_sequence(
    sequence: &DecodedSequence,
    format: ImageFormat,
    opts: &EncodeOptions,
) -> ImageResult<Vec<u8>> {
    codecs::encode_sequence_format(sequence, format, opts)
}

/// Encode with default options.
///
/// # Errors
///
/// Returns the same structured validation, feature, and codec errors as
/// [`encode`].
pub fn encode_default(img: &DecodedImage, format: ImageFormat) -> ImageResult<Vec<u8>> {
    encode(img, format, &EncodeOptions::for_format(format))
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
pub fn __coverage_av1_entropy_reference_trace() -> ImageResult<Vec<Av1EntropyTraceState>> {
    codecs::into_image_result(
        codecs::__coverage_av1_entropy_reference_trace(),
        ImageFormat::Avif,
    )
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
pub fn __coverage_av1_reconstruction(data: &[u8]) -> ImageResult<Option<Av1ReconstructionTrace>> {
    codecs::into_image_result(
        codecs::__coverage_av1_reconstruction(data),
        ImageFormat::Avif,
    )
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
    capabilities::__coverage_exercise_private_branches();
    codecs::__coverage_exercise_private_branches();
    decode_policy::__coverage_exercise_private_branches();
    types::__coverage_exercise_private_branches();
}
