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
//!             stage: None,
//!             reason: None,
//!             offset: None,
//!             identity: None,
//!         });
//!     }
//!     if decoded.content.mode != ImageMode::Rgb8 {
//!         return Err(ImageError::Unsupported {
//!             format: Some(ImageFormat::Jpeg),
//!             message: "JPEG example requires opaque RGB8 input".to_owned(),
//!             stage: None,
//!             reason: None,
//!             offset: None,
//!             identity: None,
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

mod cancel;
pub mod capabilities;
mod codecs;
pub mod decode_policy;
mod diagnostic;
pub mod encode_options;
pub mod encode_policy;
pub mod source;
pub mod types;

pub use cancel::CancellationToken;
pub use capabilities::{
    CODEC_OPERATIONS, Capability, CapabilityRestriction, CapabilityTarget,
    CapabilityUnavailableReason, CodecOperation, FormatCapabilities, all_capabilities,
};
pub(crate) use decode_policy::SequenceDecodeBudget;
pub use decode_policy::{DecodeLimits, DecodePolicy};
pub use diagnostic::{DiagnosticKind, ImageDiagnostic};
pub use encode_options::*;
pub use encode_policy::EncodePolicy;
pub use source::{EncodedImage, EncodedImageView};
pub use types::*;

fn work_budget_token(
    policy: &EncodePolicy,
    source: Option<&CancellationToken>,
) -> Option<CancellationToken> {
    policy.max_work_units().map(|maximum| match source {
        Some(source) => CancellationToken::with_work_budget_from(source, maximum),
        None => CancellationToken::with_work_budget(maximum),
    })
}

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

/// Incremental signature detection for callers that are still receiving
/// encoded input.
///
/// Unlike [`detect_format`], an *incomplete prefix* of a supported signature
/// is not reported as [`ImageError::UnknownFormat`]. It returns
/// [`ImageError::NeedMoreData`] with an exact total byte `minimum`: append
/// enough input to reach that length and call again. A terminal result
/// ([`ImageError::UnknownFormat`] for bytes that can never become a supported
/// signature, or any other error from the complete-slice APIs) must never be
/// retried as if more input could change it.
///
/// The reported minimum is exact for fixed signatures and progress-aware for
/// containers that declare their own extent (WebP RIFF chunks and AVIF
/// boxes): it is the total input length the next parse needs before it can
/// either succeed or fail terminally.
///
/// # Errors
///
/// Returns [`ImageError::NeedMoreData`] when `data` is an incomplete prefix,
/// and the terminal [`ImageError::UnknownFormat`] otherwise. Feature state is
/// intentionally not consulted here; detection is feature-independent.
pub fn detect_prefix(data: &[u8]) -> ImageResult<ImageFormat> {
    match detect_prefix_inner(data) {
        Some(Ok(format)) => Ok(format),
        Some(Err(minimum)) => Err(ImageError::NeedMoreData {
            format: None,
            stage: None,
            offset: None,
            identity: None,
            minimum,
        }),
        None => Err(ImageError::UnknownFormat),
    }
}

/// `Some(Ok(format))` identifies a complete signature; `Some(Err(minimum))`
/// is an incomplete prefix needing `minimum` total bytes; `None` is terminal
/// unknown.
fn detect_prefix_inner(data: &[u8]) -> Option<Result<ImageFormat, u64>> {
    let mut minimum = u64::MAX;
    let mut prefix = None;
    for candidate in signature_prefixes(data) {
        match candidate {
            Some(Ok(format)) => return Some(Ok(format)),
            Some(Err(needed)) => {
                minimum = minimum.min(needed);
                prefix = Some(());
            }
            None => {}
        }
    }
    prefix.map(|()| Err(minimum))
}

fn signature_prefixes(data: &[u8]) -> [Option<Result<ImageFormat, u64>>; 8] {
    let jpeg = fixed_signature(data, b"\xff\xd8\xff", ImageFormat::Jpeg);
    let png = fixed_signature(data, b"\x89PNG\r\n\x1a\n", ImageFormat::Png);
    let gif = if data.len() >= 6 {
        (data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a")).then_some(Ok(ImageFormat::Gif))
    } else if b"GIF87a".starts_with(data) || b"GIF89a".starts_with(data) {
        Some(Err(6))
    } else {
        None
    };
    let bmp = fixed_signature(data, b"BM", ImageFormat::Bmp);
    let webp = webp_signature_prefix(data);
    let tiff = any_fixed_signature(
        data,
        &[
            b"MM\x00\x2a",
            b"II\x2a\x00",
            b"MM\x2a\x00",
            b"II\x00\x2a",
            b"MM\x00\x2b",
            b"II\x2b\x00",
        ],
        ImageFormat::Tiff,
    );
    let ico = any_fixed_signature(
        data,
        &[b"\x00\x00\x01\x00", b"\x00\x00\x02\x00"],
        ImageFormat::Ico,
    );
    let avif = avif_signature_prefix(data);
    [jpeg, png, gif, bmp, webp, tiff, ico, avif]
}

fn fixed_signature(
    data: &[u8],
    signature: &[u8],
    format: ImageFormat,
) -> Option<Result<ImageFormat, u64>> {
    if data.len() >= signature.len() {
        data.starts_with(signature).then_some(Ok(format))
    } else if signature.starts_with(data) {
        Some(Err(signature.len() as u64))
    } else {
        None
    }
}

fn any_fixed_signature(
    data: &[u8],
    signatures: &[&[u8]],
    format: ImageFormat,
) -> Option<Result<ImageFormat, u64>> {
    if data.len() >= 4 {
        signatures
            .iter()
            .any(|signature| data.starts_with(signature))
            .then_some(Ok(format))
    } else {
        signatures
            .iter()
            .any(|signature| signature.starts_with(data))
            .then_some(Err(4))
    }
}

fn webp_signature_prefix(data: &[u8]) -> Option<Result<ImageFormat, u64>> {
    if data.len() < 4 {
        return b"RIFF".starts_with(data).then_some(Err(4));
    }
    if &data[..4] != b"RIFF" {
        return None;
    }
    if data.len() < 8 {
        return Some(Err(8));
    }
    if data.len() < 12 {
        return b"WEBP".starts_with(&data[8..]).then_some(Err(12));
    }
    if &data[8..12] != b"WEBP" {
        return None;
    }
    if data.len() < 16 {
        return [
            b"VP8 ".starts_with(&data[12..]),
            b"VP8L".starts_with(&data[12..]),
            b"VP8X".starts_with(&data[12..]),
        ]
        .into_iter()
        .any(|prefix| prefix)
        .then_some(Err(16));
    }
    matches!(&data[12..16], b"VP8 " | b"VP8L" | b"VP8X").then_some(Ok(ImageFormat::WebP))
}

fn avif_signature_prefix(data: &[u8]) -> Option<Result<ImageFormat, u64>> {
    if data.len() < 4 {
        return Some(Err(4));
    }
    let size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as u64;
    if size < 8 {
        return None;
    }
    if data.len() < 12 {
        return Some(Err(12));
    }
    match &data[4..8] {
        b"ftyp" if &data[8..12] == b"avif" || &data[8..12] == b"avis" => {
            Some(Ok(ImageFormat::Avif))
        }
        b"ftyp" if &data[8..12] == b"mif1" || &data[8..12] == b"msf1" => {
            if size < 20 || !size.is_multiple_of(4) {
                return None;
            }
            #[cfg(target_pointer_width = "64")]
            let size = usize::from_ne_bytes(size.to_ne_bytes());
            #[cfg(not(target_pointer_width = "64"))]
            let size = match usize::try_from(size) {
                Ok(size) => size,
                Err(_) => return None,
            };
            if size > data.len() {
                return Some(Err(size as u64));
            }
            data[16..size]
                .chunks_exact(4)
                .any(|brand| matches!(brand, b"avif" | b"avis"))
                .then_some(Ok(ImageFormat::Avif))
        }
        _ => None,
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

fn finish_decoded<T>(
    format: ImageFormat,
    content: T,
    consumed_bytes: Option<usize>,
    input_len: usize,
    stage: ImageErrorStage,
    mut diagnostics: Vec<ImageDiagnostic>,
) -> Decoded<T> {
    if let Some(consumed) = consumed_bytes
        && consumed < input_len
    {
        diagnostics.push(ImageDiagnostic {
            kind: DiagnosticKind::TrailingDataIgnored,
            format,
            stage: Some(stage),
            offset: Some(consumed as u64),
            identity: Some("trailing_input"),
        });
    }
    Decoded::new(format, content, consumed_bytes).with_diagnostics(diagnostics)
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
    policy.check_metadata_bytes(data, format, CodecOperation::StillDecode)?;
    if policy.requires_image_info() {
        let info = codecs::inspect_format(data, format)?;
        policy.check_image_info(&info, CodecOperation::StillDecode)?;
    }
    codecs::decode_format(data, format).map(|(image, consumed_bytes, diagnostics)| {
        finish_decoded(
            format,
            image,
            consumed_bytes,
            data.len(),
            ImageErrorStage::StillDecode,
            diagnostics,
        )
    })
}

/// Incremental still decode for callers that are still receiving input.
///
/// This is the partial-input counterpart of [`decode`]. Detection, inspection,
/// and codec parsing distinguish "the input ends before a structure is
/// complete" from "the data is malformed", and expose the former as the
/// non-terminal [`ImageError::NeedMoreData`] with a total byte `minimum`.
/// Retry only after appending enough bytes to reach
/// [`ImageError::minimum_input`]; every other result is terminal.
///
/// The minimum is exact when the container declares the missing extent (PNG
/// chunks, BMP/ICO pixel spans, TIFF strip/tile spans, WebP RIFF payloads,
/// AVIF boxes) and progress-aware otherwise (at least one more byte or
/// container header is required before the parser can continue).
///
/// # Errors
///
/// Returns the same terminal errors as [`decode`] (unknown signature,
/// disabled feature, malformed payload, invalid decoded buffer), plus the
/// non-terminal [`ImageError::NeedMoreData`] for incomplete input.
pub fn decode_prefix(data: &[u8]) -> ImageResult<Decoded<DecodedImage>> {
    decode_prefix_with_policy(data, &DecodePolicy::default())
}

/// [`decode_prefix`] with an explicit caller-controlled policy.
///
/// The policy limits apply to the current input length on every retry; a
/// prefix shorter than a configured encoded-byte maximum passes the
/// pre-detection check and the limit is re-evaluated as more bytes arrive.
///
/// # Errors
///
/// Returns the same errors as [`decode_prefix`], plus
/// [`ImageError::LimitExceeded`] for the current input length or inspected
/// primary canvas.
pub fn decode_prefix_with_policy(
    data: &[u8],
    policy: &DecodePolicy,
) -> ImageResult<Decoded<DecodedImage>> {
    policy.check_encoded_input(data, CodecOperation::StillDecode)?;
    let format = detect_prefix(data)?;
    policy.check_metadata_bytes(data, format, CodecOperation::StillDecode)?;
    if policy.requires_image_info() {
        let info = codecs::inspect_basic_prefix_format(data, format)?;
        policy.check_image_info(&info, CodecOperation::StillDecode)?;
    }
    codecs::decode_prefix_format(data, format).map(|(image, consumed_bytes, diagnostics)| {
        finish_decoded(
            format,
            image,
            consumed_bytes,
            data.len(),
            ImageErrorStage::StillDecode,
            diagnostics,
        )
    })
}

/// Decode a still image with cooperative cancellation.
///
/// The [`CancellationToken`] is polled at structural checkpoints (chunk
/// boundaries, per-entry selection, pixel-payload work) and the operation
/// stops with [`ImageError::Cancelled`] without publishing partial state.
/// Truncated input reports the same non-terminal
/// [`ImageError::NeedMoreData`] status as [`decode_prefix`].
///
/// # Errors
///
/// Returns the same terminal errors as [`decode`], plus
/// [`ImageError::NeedMoreData`] for incomplete input and
/// [`ImageError::Cancelled`] when the token fires.
pub fn decode_with_token(
    data: &[u8],
    token: &CancellationToken,
) -> ImageResult<Decoded<DecodedImage>> {
    decode_with_token_and_policy(data, &DecodePolicy::default(), token)
}

/// [`decode_with_token`] with an explicit caller-controlled policy.
///
/// # Errors
///
/// Returns the same errors as [`decode_with_token`], plus
/// [`ImageError::LimitExceeded`] for the current input length or inspected
/// primary canvas.
pub fn decode_with_token_and_policy(
    data: &[u8],
    policy: &DecodePolicy,
    token: &CancellationToken,
) -> ImageResult<Decoded<DecodedImage>> {
    policy.check_encoded_input(data, CodecOperation::StillDecode)?;
    let format = detect_prefix(data)?;
    policy.check_metadata_bytes(data, format, CodecOperation::StillDecode)?;
    if policy.requires_image_info() {
        let info = codecs::inspect_basic_prefix_format(data, format)?;
        policy.check_image_info(&info, CodecOperation::StillDecode)?;
    }
    codecs::decode_token_format(data, format, token).map(|(image, consumed_bytes, diagnostics)| {
        finish_decoded(
            format,
            image,
            consumed_bytes,
            data.len(),
            ImageErrorStage::StillDecode,
            diagnostics,
        )
    })
}

/// Decode a still image into an exact-size caller-provided destination.
///
/// The destination must contain exactly [`ImageInfo::decoded_bytes`] bytes
/// for the detected format's inspected mode and canvas. Short, oversized, or
/// layout-incompatible buffers are rejected with [`ImageError::Parameter`]
/// before any bytes are written, so a rejected call never partially
/// overwrites the destination. The returned image remains the authoritative
/// decoded result; the destination receives a byte-identical copy of its
/// pixels.
///
/// # Errors
///
/// Returns the same errors as [`decode`], plus [`ImageError::Parameter`] when
/// the destination length does not match the decoded transfer bytes exactly.
pub fn decode_into(data: &[u8], destination: &mut [u8]) -> ImageResult<Decoded<DecodedImage>> {
    decode_into_with_policy(data, &DecodePolicy::default(), destination)
}

/// [`decode_into`] with an explicit caller-controlled policy.
///
/// # Errors
///
/// Returns the same errors as [`decode_into`], with resource limits applied
/// before the destination length check.
pub fn decode_into_with_policy(
    data: &[u8],
    policy: &DecodePolicy,
    destination: &mut [u8],
) -> ImageResult<Decoded<DecodedImage>> {
    let decoded = decode_with_policy(data, policy)?;
    let expected = decoded.content.pixels.len();
    if destination.len() != expected {
        return Err(ImageError::parameter(format!(
            "decode destination must be exactly {expected} bytes"
        )));
    }
    destination.copy_from_slice(&decoded.content.pixels);
    Ok(decoded)
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
    policy.check_metadata_bytes(data, format, CodecOperation::SequenceDecode)?;
    let mut budget = policy.sequence_budget(format);
    if policy.requires_image_info() {
        let info = codecs::inspect_format(data, format)?;
        policy.check_image_info(&info, CodecOperation::SequenceDecode)?;
        budget.charge_primary(&info)?;
    }
    codecs::decode_sequence_format(data, format, &mut budget).map(
        |(sequence, consumed_bytes, diagnostics)| {
            finish_decoded(
                format,
                sequence,
                consumed_bytes,
                data.len(),
                ImageErrorStage::SequenceDecode,
                diagnostics,
            )
        },
    )
}

/// Incremental sequence decode for callers that are still receiving input.
///
/// This is the partial-input counterpart of [`decode_sequence`], with the
/// same non-terminal [`ImageError::NeedMoreData`] semantics and retry rules
/// as [`decode_prefix`].
///
/// # Errors
///
/// Returns the same terminal errors as [`decode_sequence`], plus the
/// non-terminal [`ImageError::NeedMoreData`] for incomplete input.
pub fn decode_sequence_prefix(data: &[u8]) -> ImageResult<Decoded<DecodedSequence>> {
    decode_sequence_prefix_with_policy(data, &DecodePolicy::default())
}

/// [`decode_sequence_prefix`] with an explicit caller-controlled policy.
///
/// # Errors
///
/// Returns the same errors as [`decode_sequence_prefix`], plus
/// [`ImageError::LimitExceeded`] for the current input length, inspected
/// frame count, or decoded-byte budget.
pub fn decode_sequence_prefix_with_policy(
    data: &[u8],
    policy: &DecodePolicy,
) -> ImageResult<Decoded<DecodedSequence>> {
    policy.check_encoded_input(data, CodecOperation::SequenceDecode)?;
    let format = detect_prefix(data)?;
    policy.check_metadata_bytes(data, format, CodecOperation::SequenceDecode)?;
    let mut budget = policy.sequence_budget(format);
    if policy.requires_image_info() {
        let info = codecs::inspect_basic_prefix_format(data, format)?;
        policy.check_image_info(&info, CodecOperation::SequenceDecode)?;
        budget.charge_primary(&info)?;
    }
    codecs::decode_sequence_prefix_format(data, format, &mut budget).map(
        |(sequence, consumed_bytes, diagnostics)| {
            finish_decoded(
                format,
                sequence,
                consumed_bytes,
                data.len(),
                ImageErrorStage::SequenceDecode,
                diagnostics,
            )
        },
    )
}

/// Decode every retained frame with cooperative cancellation.
///
/// The [`CancellationToken`] is polled at frame/page boundaries and the
/// operation stops with [`ImageError::Cancelled`] without publishing partial
/// state. Truncated input reports the same non-terminal
/// [`ImageError::NeedMoreData`] status as [`decode_sequence_prefix`].
///
/// # Errors
///
/// Returns the same terminal errors as [`decode_sequence`], plus
/// [`ImageError::NeedMoreData`] for incomplete input and
/// [`ImageError::Cancelled`] when the token fires.
pub fn decode_sequence_with_token(
    data: &[u8],
    token: &CancellationToken,
) -> ImageResult<Decoded<DecodedSequence>> {
    decode_sequence_with_token_and_policy(data, &DecodePolicy::default(), token)
}

/// [`decode_sequence_with_token`] with an explicit caller-controlled policy.
///
/// # Errors
///
/// Returns the same errors as [`decode_sequence_with_token`], plus
/// [`ImageError::LimitExceeded`] for the current input length, inspected
/// frame count, or decoded-byte budget.
pub fn decode_sequence_with_token_and_policy(
    data: &[u8],
    policy: &DecodePolicy,
    token: &CancellationToken,
) -> ImageResult<Decoded<DecodedSequence>> {
    policy.check_encoded_input(data, CodecOperation::SequenceDecode)?;
    let format = detect_prefix(data)?;
    policy.check_metadata_bytes(data, format, CodecOperation::SequenceDecode)?;
    let mut budget = policy.sequence_budget(format);
    if policy.requires_image_info() {
        let info = codecs::inspect_basic_prefix_format(data, format)?;
        policy.check_image_info(&info, CodecOperation::SequenceDecode)?;
        budget.charge_primary(&info)?;
    }
    codecs::decode_sequence_token_format(data, format, &mut budget, token).map(
        |(sequence, consumed_bytes, diagnostics)| {
            finish_decoded(
                format,
                sequence,
                consumed_bytes,
                data.len(),
                ImageErrorStage::SequenceDecode,
                diagnostics,
            )
        },
    )
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

/// Inspect encoded headers without counting every frame or page.
///
/// [`inspect_basic`] performs the same header validation as [`inspect`] but
/// may stop after the first proven image, leaving `frame_count` `None` and
/// `frame_count_complete` `false` when the complete count requires a deep
/// traversal (GIF frame descriptors, the TIFF IFD chain, or animated WebP
/// frame chunks). Formats whose inspection is already header-bound (PNG,
/// JPEG, BMP, ICO, and AVIF) return the same result as [`inspect`].
///
/// # Errors
///
/// Returns the same errors as [`inspect`] for unknown signatures, disabled
/// features, and malformed headers.
pub fn inspect_basic(data: &[u8]) -> ImageResult<ImageInfo> {
    let format = detect_format(data)?;
    codecs::inspect_basic_format(data, format)
}

/// Incremental header inspection for callers that are still receiving input.
///
/// This is the partial-input counterpart of [`inspect_basic`]. It returns
/// basic header facts as soon as the detected format can prove them, and
/// [`ImageError::NeedMoreData`] when even the basic header is incomplete. The
/// result distinguishes "basic header known" (an [`ImageInfo`] with
/// `frame_count_complete` describing whether the deep count is provable) from
/// "still receiving input" (the non-terminal status).
///
/// Retry only after appending enough bytes to reach
/// [`ImageError::minimum_input`]. Every other result is terminal and must not
/// be retried.
///
/// # Errors
///
/// Returns the same terminal errors as [`inspect_basic`] (unknown signature,
/// disabled feature, malformed header), plus the non-terminal
/// [`ImageError::NeedMoreData`] for incomplete input.
pub fn inspect_basic_prefix(data: &[u8]) -> ImageResult<ImageInfo> {
    let format = detect_prefix(data)?;
    codecs::inspect_basic_prefix_format(data, format)
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
    policy.check_metadata_bytes(data, format, CodecOperation::Inspection)?;
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
    encode_with_policy(img, format, opts, &EncodePolicy::default())
}

/// Encode a decoded still image under an explicit output-result policy.
///
/// The policy is checked after the selected codec has produced its complete
/// validated buffer and before that buffer is returned. It therefore bounds
/// caller-visible output size. When configured, its cooperative work budget
/// also bounds the number of documented encode checkpoints admitted before
/// the codec continues. Neither field bounds transient allocations or
/// recoverable out-of-memory behavior inside a whole-buffer encoder.
///
/// # Errors
///
/// Returns the same errors as [`encode`], plus
/// [`ImageError::LimitExceeded`] with [`ResourceLimit::EncodedOutputBytes`]
/// when the complete result exceeds `policy`, or with
/// [`ResourceLimit::EncodeWorkUnits`] when its checkpoint budget is exhausted.
pub fn encode_with_policy(
    img: &DecodedImage,
    format: ImageFormat,
    opts: &EncodeOptions,
    policy: &EncodePolicy,
) -> ImageResult<Vec<u8>> {
    let budget_token = work_budget_token(policy, None);
    let encoded = match budget_token.as_ref() {
        Some(token) => codecs::encode_format_with_token(img, format, opts, Some(token))?,
        None => codecs::encode_format(img, format, opts)?,
    };
    policy.check_output(&encoded, format, CodecOperation::StillEncode)?;
    Ok(encoded)
}

/// Encode a decoded still image with cooperative cancellation.
///
/// The token is checked before codec dispatch and after encoding. JPEG also
/// polls between color-conversion rows, sampling rows, quantized block rows,
/// optimized baseline Huffman frequency intervals after each 1,024 AC
/// coefficients, progressive scan block slots after each 1,024 blocks,
/// progressive scan coefficient items after each 1,024 coefficients, and
/// progressive scan-event frequency items after each 1,024 events, entropy
/// rows, after each 1,024 emitted entropy bytes, and
/// progressive event batches; TIFF polls page preparation,
/// row prediction, PackBits/LZW compression checkpoints, and Deflate input-row,
/// level-six matcher, expansion, Huffman, bitstream, stored-block, and checksum
/// intervals; PNG polls row preparation, adaptive-filter and filtered-row
/// subsegments, token-aware stored-block boundaries and 1,024-byte stored-block
/// copy intervals, every zlib-ng level's matcher/expansion/Huffman/bitstream/
/// checksum stages, and structural segments in still and one-frame fallback
/// paths; BMP polls row preparation
/// and structural segments; BMP also polls 1,024-pixel row-conversion
/// subsegments; GIF still encoding also polls block/frame/coalescing/output-assembly
/// checkpoints, RGB/RGBA palette quantization intervals, RGB median-cut
/// hash/order, axis-ordering, split, and partition checkpoints, fixed RGBA
/// FASTOCTREE cell/bucket/lookup and bucket-sort intervals, and LZW input-symbol
/// intervals, and WebP still encoding polls preparation,
/// lossy VP8 RGB/RGBA-to-YUV conversion items, RGBA transparent-area cleanup
/// after each 1,024 scanned or flattened pixels, analysis/mode-selection/
/// probability, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit and 512-bit logical and 16,384-boolean first-partition-bit and
/// 16-bit, 32-bit, 64-bit, 128-bit, 256-bit and 512-bit logical and 16,384-boolean coefficient-bit,
/// 1,024-byte boolean-bitstream output intervals, and bitstream stages, lossless
/// VP8L predictor/cross-color/entropy/transform, bounded backward-reference,
/// histogram/Huffman, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit and 512-bit logical bitstream intervals, 1,024-byte
/// bitstream-output,
/// and token-stream stages, codec-result, and
/// metadata-assembly boundaries; native AVIF still encoding polls preparation,
/// frame, and finalization checkpoints; ICO still encoding polls source-size
/// validation, embedded PNG/BMP work, and directory finalization. The sequence
/// API additionally checks at retained-frame boundaries and codec-specific
/// checkpoints.
///
/// # Errors
///
/// Returns the same errors as [`encode`], plus [`ImageError::Cancelled`] when
/// the token is already cancelled or fires at an implemented checkpoint.
pub fn encode_with_token(
    img: &DecodedImage,
    format: ImageFormat,
    opts: &EncodeOptions,
    token: &CancellationToken,
) -> ImageResult<Vec<u8>> {
    encode_with_token_and_policy(img, format, opts, &EncodePolicy::default(), token)
}

/// [`encode_with_token`] with an explicit output-result policy.
///
/// The policy check runs only after cancellation has been checked by the
/// codec boundary, so a cancelled operation never returns an oversized result.
/// A configured work budget is layered over the caller token and reports a
/// typed limit error separately from caller cancellation.
pub fn encode_with_token_and_policy(
    img: &DecodedImage,
    format: ImageFormat,
    opts: &EncodeOptions,
    policy: &EncodePolicy,
    token: &CancellationToken,
) -> ImageResult<Vec<u8>> {
    let budget_token = work_budget_token(policy, Some(token));
    let effective_token = budget_token.as_ref().unwrap_or(token);
    let encoded = codecs::encode_format_with_token(img, format, opts, Some(effective_token))?;
    policy.check_output(&encoded, format, CodecOperation::StillEncode)?;
    Ok(encoded)
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
    encode_sequence_with_policy(sequence, format, opts, &EncodePolicy::default())
}

/// Encode a still image or animation under an explicit output-result policy.
///
/// The policy is checked after the selected codec has produced its complete
/// validated buffer and before that buffer is returned. It therefore bounds
/// caller-visible output size. When configured, its cooperative work budget
/// also bounds the number of documented encode checkpoints admitted before
/// the codec continues. Neither field bounds transient allocations or
/// recoverable out-of-memory behavior inside a whole-buffer encoder.
///
/// # Errors
///
/// Returns the same errors as [`encode_sequence`], plus
/// [`ImageError::LimitExceeded`] with [`ResourceLimit::EncodedOutputBytes`]
/// when the complete result exceeds `policy`, or with
/// [`ResourceLimit::EncodeWorkUnits`] when its checkpoint budget is exhausted.
pub fn encode_sequence_with_policy(
    sequence: &DecodedSequence,
    format: ImageFormat,
    opts: &EncodeOptions,
    policy: &EncodePolicy,
) -> ImageResult<Vec<u8>> {
    let budget_token = work_budget_token(policy, None);
    let encoded = match budget_token.as_ref() {
        Some(token) => {
            codecs::encode_sequence_format_with_token(sequence, format, opts, Some(token))?
        }
        None => codecs::encode_sequence_format(sequence, format, opts)?,
    };
    policy.check_output(&encoded, format, CodecOperation::SequenceEncode)?;
    Ok(encoded)
}

/// Encode a still image or animation with cooperative cancellation.
///
/// Sequence-capable codecs poll at retained-frame and finalization
/// boundaries. One-frame sequence encodes reuse their still encoder's
/// checkpoints, while their sink paths reuse the corresponding validated
/// structural writers; multi-frame GIF, TIFF, WebP, and native AVIF encodes
/// poll their sequence/container boundaries.
///
/// # Errors
///
/// Returns the same errors as [`encode_sequence`], plus
/// [`ImageError::Cancelled`] when the token is cancelled at an implemented
/// checkpoint.
pub fn encode_sequence_with_token(
    sequence: &DecodedSequence,
    format: ImageFormat,
    opts: &EncodeOptions,
    token: &CancellationToken,
) -> ImageResult<Vec<u8>> {
    encode_sequence_with_token_and_policy(sequence, format, opts, &EncodePolicy::default(), token)
}

/// [`encode_sequence_with_token`] with an explicit output-result policy.
/// A configured work budget is layered over the caller token and reports a
/// typed limit error separately from caller cancellation.
pub fn encode_sequence_with_token_and_policy(
    sequence: &DecodedSequence,
    format: ImageFormat,
    opts: &EncodeOptions,
    policy: &EncodePolicy,
    token: &CancellationToken,
) -> ImageResult<Vec<u8>> {
    let budget_token = work_budget_token(policy, Some(token));
    let effective_token = budget_token.as_ref().unwrap_or(token);
    let encoded =
        codecs::encode_sequence_format_with_token(sequence, format, opts, Some(effective_token))?;
    policy.check_output(&encoded, format, CodecOperation::SequenceEncode)?;
    Ok(encoded)
}

/// Dependency-free destination for encoded output.
///
/// Codecs may retain complete encoded working state before the first write.
/// Current sink writers additionally emit validated container structures
/// through separate writes: JPEG, PNG, GIF, BMP, TIFF, WebP, ICO, and native
/// AVIF still paths, plus one-frame JPEG/PNG/BMP/ICO sequences and supported
/// GIF/TIFF/WebP/AVIF sequences. A sink failure or cancellation after a write
/// may therefore leave a prefix in the destination; `flush` failure likewise
/// does not roll the prefix back. The trait does not provide rollback or
/// short-write recovery.
pub trait OutputSink {
    /// Append one fully accepted encoded segment to this sink.
    ///
    /// # Errors
    ///
    /// Returns a structured error when the destination rejects the write. An
    /// implementation must return `Ok(())` only after accepting the complete
    /// slice; if it accepts a prefix and then returns an error, that prefix is
    /// already delivered and cannot be rolled back by the encoder.
    fn write_all(&mut self, bytes: &[u8]) -> ImageResult<()>;

    /// Finalize delivery after the complete encoded result has been written.
    ///
    /// The default is suitable for in-memory sinks and preserves compatibility
    /// with existing implementations. A buffered or externally owned sink can
    /// override this hook to surface its finalization error. A failure does
    /// not roll back earlier writes.
    fn flush(&mut self) -> ImageResult<()> {
        Ok(())
    }
}

impl OutputSink for Vec<u8> {
    fn write_all(&mut self, bytes: &[u8]) -> ImageResult<()> {
        self.extend_from_slice(bytes);
        Ok(())
    }
}

impl OutputSink for &mut Vec<u8> {
    fn write_all(&mut self, bytes: &[u8]) -> ImageResult<()> {
        self.extend_from_slice(bytes);
        Ok(())
    }
}

/// Encode a still image into a caller-owned output sink.
///
/// The encoded bytes are produced exactly as by [`encode`] and delivered to
/// the sink. Current codec writers validate their container structure and may
/// use multiple writes after their working state is ready; the structure
/// boundaries are format-specific, such as JPEG marker/scan, PNG chunk,
/// GIF block, WebP RIFF/chunk, ICO directory/payload, TIFF page, and AVIF box
/// spans.
///
/// # Errors
///
/// Returns the same errors as [`encode`], plus an [`ImageError::OutputWrite`]
/// when the sink rejects an emitted segment.
pub fn encode_to_sink(
    img: &DecodedImage,
    format: ImageFormat,
    opts: &EncodeOptions,
    sink: &mut impl OutputSink,
) -> ImageResult<usize> {
    encode_to_sink_with_policy(img, format, opts, &EncodePolicy::default(), sink)
}

/// Encode a still image under an output-result policy and deliver it to a
/// caller-owned sink only when the complete result is admitted.
///
/// # Errors
///
/// Returns the same errors as [`encode_with_policy`], plus
/// [`ImageError::OutputWrite`] when the sink rejects an admitted segment, or
/// [`ImageError::LimitExceeded`] when the output or checkpoint budget rejects
/// the operation before delivery.
pub fn encode_to_sink_with_policy(
    img: &DecodedImage,
    format: ImageFormat,
    opts: &EncodeOptions,
    policy: &EncodePolicy,
    sink: &mut impl OutputSink,
) -> ImageResult<usize> {
    encode_to_sink_with_policy_impl(img, format, opts, policy, sink)
}

fn encode_to_sink_with_policy_impl(
    img: &DecodedImage,
    format: ImageFormat,
    opts: &EncodeOptions,
    policy: &EncodePolicy,
    sink: &mut dyn OutputSink,
) -> ImageResult<usize> {
    let budget_token = work_budget_token(policy, None);
    if let Some(written) = codecs::encode_format_to_sink_with_token(
        img,
        format,
        opts,
        *policy,
        budget_token.as_ref(),
        sink,
    )? {
        return finish_sink(sink, format, ImageErrorStage::StillEncode, written);
    }
    let encoded = encode_with_policy(img, format, opts, policy)?;
    write_sink_all(sink, &encoded, format, ImageErrorStage::StillEncode)
}

/// Encode a still image with cooperative cancellation and deliver it to a
/// caller-owned sink. Structural writers may stop after an already-written
/// prefix when the token fires between writes.
pub fn encode_to_sink_with_token(
    img: &DecodedImage,
    format: ImageFormat,
    opts: &EncodeOptions,
    token: &CancellationToken,
    sink: &mut impl OutputSink,
) -> ImageResult<usize> {
    encode_to_sink_with_token_and_policy(img, format, opts, &EncodePolicy::default(), token, sink)
}

/// Encode a still image with cooperative cancellation and an output-result
/// policy before delivering the admitted result to a caller-owned sink.
/// A configured work budget is layered over the caller token and reports a
/// typed limit error separately from caller cancellation.
pub fn encode_to_sink_with_token_and_policy(
    img: &DecodedImage,
    format: ImageFormat,
    opts: &EncodeOptions,
    policy: &EncodePolicy,
    token: &CancellationToken,
    sink: &mut impl OutputSink,
) -> ImageResult<usize> {
    encode_to_sink_with_token_and_policy_impl(img, format, opts, policy, token, sink)
}

fn encode_to_sink_with_token_and_policy_impl(
    img: &DecodedImage,
    format: ImageFormat,
    opts: &EncodeOptions,
    policy: &EncodePolicy,
    token: &CancellationToken,
    sink: &mut dyn OutputSink,
) -> ImageResult<usize> {
    let budget_token = work_budget_token(policy, Some(token));
    let effective_token = budget_token.as_ref().unwrap_or(token);
    if let Some(written) = codecs::encode_format_to_sink_with_token(
        img,
        format,
        opts,
        *policy,
        Some(effective_token),
        sink,
    )? {
        return finish_sink(sink, format, ImageErrorStage::StillEncode, written);
    }
    let encoded = encode_with_token_and_policy(img, format, opts, policy, token)?;
    write_sink_all(sink, &encoded, format, ImageErrorStage::StillEncode)
}

/// Encode a still image or animation into a caller-owned output sink.
///
/// # Errors
///
/// Returns the same errors as [`encode_sequence`], plus an
/// [`ImageError::OutputWrite`] when the sink rejects an emitted segment.
/// Structural writers may leave an already-delivered prefix and check
/// cancellation between their validated structural segments.
pub fn encode_sequence_to_sink(
    sequence: &DecodedSequence,
    format: ImageFormat,
    opts: &EncodeOptions,
    sink: &mut impl OutputSink,
) -> ImageResult<usize> {
    encode_sequence_to_sink_with_policy(sequence, format, opts, &EncodePolicy::default(), sink)
}

/// Encode a still image or animation under an output-result policy and
/// deliver it to a caller-owned sink only when the complete result is
/// admitted.
///
/// # Errors
///
/// Returns the same errors as [`encode_sequence_with_policy`], plus
/// [`ImageError::OutputWrite`] when the sink rejects an admitted segment, or
/// [`ImageError::LimitExceeded`] when the output or checkpoint budget rejects
/// the operation before delivery.
pub fn encode_sequence_to_sink_with_policy(
    sequence: &DecodedSequence,
    format: ImageFormat,
    opts: &EncodeOptions,
    policy: &EncodePolicy,
    sink: &mut impl OutputSink,
) -> ImageResult<usize> {
    encode_sequence_to_sink_with_policy_impl(sequence, format, opts, policy, sink)
}

fn encode_sequence_to_sink_with_policy_impl(
    sequence: &DecodedSequence,
    format: ImageFormat,
    opts: &EncodeOptions,
    policy: &EncodePolicy,
    sink: &mut dyn OutputSink,
) -> ImageResult<usize> {
    let budget_token = work_budget_token(policy, None);
    if let Some(written) = codecs::encode_sequence_to_sink_with_token(
        sequence,
        format,
        opts,
        *policy,
        budget_token.as_ref(),
        sink,
    )? {
        return finish_sink(sink, format, ImageErrorStage::SequenceEncode, written);
    }
    let encoded = encode_sequence_with_policy(sequence, format, opts, policy)?;
    write_sink_all(sink, &encoded, format, ImageErrorStage::SequenceEncode)
}

/// Encode a still image or animation with cooperative cancellation and
/// deliver it to a caller-owned sink. Structural writers may stop after an
/// already-written prefix when the token fires between writes.
pub fn encode_sequence_to_sink_with_token(
    sequence: &DecodedSequence,
    format: ImageFormat,
    opts: &EncodeOptions,
    token: &CancellationToken,
    sink: &mut impl OutputSink,
) -> ImageResult<usize> {
    encode_sequence_to_sink_with_token_and_policy(
        sequence,
        format,
        opts,
        &EncodePolicy::default(),
        token,
        sink,
    )
}

/// Encode a still image or animation with cooperative cancellation and an
/// output-result policy before delivering the admitted result to a sink.
/// A configured work budget is layered over the caller token and reports a
/// typed limit error separately from caller cancellation.
pub fn encode_sequence_to_sink_with_token_and_policy(
    sequence: &DecodedSequence,
    format: ImageFormat,
    opts: &EncodeOptions,
    policy: &EncodePolicy,
    token: &CancellationToken,
    sink: &mut impl OutputSink,
) -> ImageResult<usize> {
    encode_sequence_to_sink_with_token_and_policy_impl(sequence, format, opts, policy, token, sink)
}

fn encode_sequence_to_sink_with_token_and_policy_impl(
    sequence: &DecodedSequence,
    format: ImageFormat,
    opts: &EncodeOptions,
    policy: &EncodePolicy,
    token: &CancellationToken,
    sink: &mut dyn OutputSink,
) -> ImageResult<usize> {
    let budget_token = work_budget_token(policy, Some(token));
    let effective_token = budget_token.as_ref().unwrap_or(token);
    if let Some(written) = codecs::encode_sequence_to_sink_with_token(
        sequence,
        format,
        opts,
        *policy,
        Some(effective_token),
        sink,
    )? {
        return finish_sink(sink, format, ImageErrorStage::SequenceEncode, written);
    }
    let encoded = encode_sequence_with_token_and_policy(sequence, format, opts, policy, token)?;
    write_sink_all(sink, &encoded, format, ImageErrorStage::SequenceEncode)
}

/// Write complete encoded bytes through a trait object so the sink error path
/// has exactly one non-generic coverage instantiation.
fn write_sink_all(
    sink: &mut dyn OutputSink,
    bytes: &[u8],
    format: ImageFormat,
    stage: ImageErrorStage,
) -> ImageResult<usize> {
    sink.write_all(bytes)
        .map_err(|error| ImageError::OutputWrite {
            format: Some(format),
            message: error.to_string(),
            stage: Some(stage),
        })?;
    finish_sink(sink, format, stage, bytes.len())
}

fn finish_sink(
    sink: &mut dyn OutputSink,
    format: ImageFormat,
    stage: ImageErrorStage,
    written: usize,
) -> ImageResult<usize> {
    sink.flush().map_err(|error| ImageError::OutputWrite {
        format: Some(format),
        message: error.to_string(),
        stage: Some(stage),
    })?;
    Ok(written)
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
        ImageErrorStage::StillDecode,
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
        ImageErrorStage::StillDecode,
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
    let fresh = CancellationToken::new();
    assert!(!fresh.is_cancelled());
    let countdown = CancellationToken::new();
    countdown.cancel_after(2);
    assert!(!countdown.is_cancelled());
    assert!(!countdown.is_cancelled());
    assert!(countdown.is_cancelled());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(cancelled.is_cancelled());
    capabilities::__coverage_exercise_private_branches();
    codecs::__coverage_exercise_private_branches();
    decode_policy::__coverage_exercise_private_branches();
    types::__coverage_exercise_private_branches();
}
