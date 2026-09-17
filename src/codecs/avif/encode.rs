//! AVIF encoding boundary for the pure-Rust implementation.
//!
//! The private still-container writer follows pinned native fixtures. Its AV1
//! compressor remains a missing dependency, without a foreign-codec fallback.
//! Until the Rust encoder is complete, every target reports a
//! stable unsupported result after normal input validation and cancellation
//! checks. Keeping this boundary in place lets the public API, capability
//! table, and future encoder share one target-independent contract.

use crate::codecs::{CodecError, CodecResult};
use crate::encode_options::AvifEncodeOptions;
use crate::encode_policy::EncodePolicy;
use crate::types::{DecodedImage, DecodedSequence};
use crate::{CodecOperation, OutputSink};

mod container;

const PURE_RUST_ENCODER_UNAVAILABLE: &str =
    "AVIF encoding is not implemented in the pure-Rust backend";

/// Encode one image with the pure-Rust AVIF backend.
///
/// The validation and cancellation behavior is already part of the codec
/// boundary. The private still-container writer awaits the pure-Rust AV1
/// compressor; public encoding remains unavailable until that dependency is ready.
pub fn encode(image: &DecodedImage, options: &AvifEncodeOptions) -> CodecResult<Vec<u8>> {
    encode_with_token(image, options, None)
}

/// Encode one image while polling an optional cooperative cancellation token.
pub fn encode_with_token(
    image: &DecodedImage,
    options: &AvifEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    encode_with_policy(
        image,
        options,
        EncodePolicy::default(),
        CodecOperation::StillEncode,
        token,
    )
}

fn encode_with_policy(
    image: &DecodedImage,
    options: &AvifEncodeOptions,
    policy: EncodePolicy,
    operation: CodecOperation,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    image.validate().map_err(CodecError::from_image_error)?;
    let prepared = prepare_still(image, options)?;
    let image = container::StillImage {
        color: container::Sample {
            bytes: &prepared.color,
            configuration: prepared.color_configuration,
        },
        alpha: prepared
            .alpha
            .as_ref()
            .map(|(bytes, configuration)| container::Sample {
                bytes,
                configuration: *configuration,
            }),
        metadata: prepared.metadata,
    };
    container::write_still(&image, policy, operation, token)
}

struct PreparedStill<'a> {
    color: Vec<u8>,
    color_configuration: [u8; 4],
    alpha: Option<(Vec<u8>, [u8; 4])>,
    metadata: container::Metadata<'a>,
}

/// The future compressor must establish sample/header agreement and prepare
/// metadata before the container writer is reachable. Keeping the existing
/// failure here preserves public validation and cancellation precedence.
fn prepare_still<'a>(
    _image: &DecodedImage,
    _options: &'a AvifEncodeOptions,
) -> CodecResult<PreparedStill<'a>> {
    Err(CodecError::NotImplemented(
        PURE_RUST_ENCODER_UNAVAILABLE.to_owned(),
    ))
}

#[cfg(coverage)]
pub(crate) fn mux_size_trace(length: u64) -> CodecResult<u32> {
    container::checked_length(length)
}

#[cfg(coverage)]
pub(crate) fn mux_still_trace(
    input: &crate::__coverage_avif_mux::Input<'_>,
    policy: EncodePolicy,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    let sample = |(bytes, configuration)| container::Sample {
        bytes,
        configuration,
    };
    container::write_still(
        &container::StillImage {
            color: sample(input.color),
            alpha: input.alpha.map(sample),
            metadata: container::Metadata {
                dimensions: input.dimensions,
                cicp: input.cicp,
                full_range: input.full_range,
                premultiplied: input.premultiplied,
                icc: input.icc,
                exif: input.exif,
                xmp: input.xmp,
                rotation: input.rotation,
                mirror: input.mirror,
            },
        },
        policy,
        CodecOperation::StillEncode,
        token,
    )
}

/// Encode one image into a caller-owned sink.
pub(crate) fn encode_to_sink(
    image: &DecodedImage,
    options: &AvifEncodeOptions,
    policy: EncodePolicy,
    operation: CodecOperation,
    token: Option<&crate::CancellationToken>,
    sink: &mut dyn OutputSink,
) -> CodecResult<usize> {
    let encoded = encode_with_policy(image, options, policy, operation, token)?;
    crate::codecs::error::check_cancelled(token)?;
    sink.write_all(&encoded)
        .map_err(|error| CodecError::OutputWrite(error.to_string()))?;
    Ok(encoded.len())
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

    let sequence = DecodedSequence::from_image(image);
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
