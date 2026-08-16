//! AVIF encoding boundary for the pure-Rust implementation.
//!
//! The container writer and AV1 encoder are deliberately not represented by a
//! foreign-codec fallback. Until the Rust encoder is complete, every target reports a
//! stable unsupported result after normal input validation and cancellation
//! checks. Keeping this boundary in place lets the public API, capability
//! table, and future encoder share one target-independent contract.

use crate::codecs::{CodecError, CodecResult};
use crate::encode_options::AvifEncodeOptions;
use crate::encode_policy::EncodePolicy;
use crate::types::{DecodedImage, DecodedSequence};
use crate::{CodecOperation, OutputSink};

const PURE_RUST_ENCODER_UNAVAILABLE: &str =
    "AVIF encoding is not implemented in the pure-Rust backend";

/// Encode one image with the pure-Rust AVIF backend.
///
/// The validation and cancellation behavior is already part of the codec
/// boundary. Actual ISO-BMFF and AV1 emission is tracked as the next Rust-only
/// implementation stage rather than delegated to a C library.
pub fn encode(image: &DecodedImage, options: &AvifEncodeOptions) -> CodecResult<Vec<u8>> {
    encode_with_token(image, options, None)
}

/// Encode one image while polling an optional cooperative cancellation token.
pub fn encode_with_token(
    image: &DecodedImage,
    _options: &AvifEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    image.validate().map_err(CodecError::from_image_error)?;
    Err(CodecError::NotImplemented(
        PURE_RUST_ENCODER_UNAVAILABLE.to_owned(),
    ))
}

/// Encode one image into a caller-owned sink.
pub(crate) fn encode_to_sink(
    image: &DecodedImage,
    options: &AvifEncodeOptions,
    _policy: EncodePolicy,
    _operation: CodecOperation,
    token: Option<&crate::CancellationToken>,
    _sink: &mut dyn OutputSink,
) -> CodecResult<usize> {
    encode_with_token(image, options, token).map(|encoded| encoded.len())
}

/// Encode an AVIF sequence with the pure-Rust backend.
pub fn encode_sequence(
    sequence: &DecodedSequence,
    options: &AvifEncodeOptions,
) -> CodecResult<Vec<u8>> {
    encode_sequence_with_token(sequence, options, None)
}

/// Encode an AVIF sequence while polling an optional cooperative cancellation
/// token.
pub fn encode_sequence_with_token(
    sequence: &DecodedSequence,
    _options: &AvifEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    sequence.validate().map_err(CodecError::from_image_error)?;
    Err(CodecError::NotImplemented(
        PURE_RUST_ENCODER_UNAVAILABLE.to_owned(),
    ))
}

/// Encode an AVIF sequence into a caller-owned sink.
pub(crate) fn encode_sequence_to_sink(
    sequence: &DecodedSequence,
    options: &AvifEncodeOptions,
    _policy: EncodePolicy,
    _operation: CodecOperation,
    token: Option<&crate::CancellationToken>,
    _sink: &mut dyn OutputSink,
) -> CodecResult<usize> {
    encode_sequence_with_token(sequence, options, token).map(|encoded| encoded.len())
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    use crate::types::{ColorType, DecodedSequence};

    let image = DecodedImage::new(1, 1, vec![0], ColorType::L8);
    let options = AvifEncodeOptions::default();
    let _ = encode(&image, &options);
    let token = crate::CancellationToken::new();
    token.cancel_after(0);
    let _ = encode_with_token(&image, &options, Some(&token));
    let _ = encode_to_sink(
        &image,
        &options,
        EncodePolicy::default(),
        CodecOperation::StillEncode,
        None,
        &mut Vec::new(),
    );

    let sequence = DecodedSequence::from_image(image.clone());
    let _ = encode_sequence(&sequence, &options);
    let _ = encode_sequence_to_sink(
        &sequence,
        &options,
        EncodePolicy::default(),
        CodecOperation::SequenceEncode,
        None,
        &mut Vec::new(),
    );
    let invalid_image = DecodedImage::new(1, 1, Vec::new(), ColorType::L8);
    let _ = encode(&invalid_image, &options);
    let invalid_sequence = DecodedSequence {
        frames: Vec::new(),
        ..sequence
    };
    let _ = encode_sequence(&invalid_sequence, &options);
}
