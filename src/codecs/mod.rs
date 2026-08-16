//! Feature-gated image codec implementations.
//!
//! Each format owns its decoding and encoding implementation so enabling one
//! Cargo feature pulls in only that codec and its private support code.

use crate::SequenceDecodeBudget;
#[cfg(any(
    feature = "jpeg",
    feature = "bmp",
    feature = "gif",
    feature = "png",
    feature = "tiff",
    feature = "webp",
    feature = "avif"
))]
use crate::capabilities::CodecOperation;
use crate::encode_options::EncodeOptions;
use crate::encode_policy::EncodePolicy;
use crate::types::{
    DecodedFrame, DecodedImage, DecodedSequence, FrameDisposal, ImageError, ImageErrorStage,
    ImageFormat, ImageInfo, ImageResult,
};

fn set_diagnostic_stage(
    diagnostics: Vec<crate::ImageDiagnostic>,
    stage: ImageErrorStage,
) -> Vec<crate::ImageDiagnostic> {
    diagnostics
        .into_iter()
        .map(|mut diagnostic| {
            diagnostic.stage = Some(stage);
            diagnostic
        })
        .collect()
}

mod error;
#[cfg(any(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico",
    feature = "avif"
))]
pub(crate) use error::CodecError;
#[cfg(any(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "tiff",
    feature = "webp",
    feature = "ico"
))]
pub(crate) use error::OptionCodecExt;
#[cfg(any(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "webp",
    feature = "ico"
))]
pub(crate) use error::codec_add_end;
#[cfg(feature = "bmp")]
pub(crate) use error::need_from;
#[cfg(any(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico"
))]
pub(crate) use error::need_slice;
#[cfg(any(feature = "webp", feature = "ico"))]
pub(crate) use error::terminalize;
pub(crate) use error::{CodecResult, into_image_result, into_incremental_image_result};

#[cfg(feature = "avif")]
mod avif;
#[cfg(feature = "bmp")]
mod bmp;
#[cfg(feature = "gif")]
mod gif;
#[cfg(feature = "ico")]
mod ico;
#[cfg(feature = "jpeg")]
mod jpeg;
#[cfg(feature = "png")]
mod png;
#[cfg(feature = "tiff")]
mod tiff;
#[cfg(feature = "webp")]
mod webp;

#[cfg(any(feature = "png", feature = "tiff"))]
mod compression;

type DecodedWithDiagnostics<T> = (T, Option<usize>, Vec<crate::ImageDiagnostic>);
type CodecDecodedResult<T> = CodecResult<DecodedWithDiagnostics<T>>;
type ImageDecodedResult<T> = ImageResult<DecodedWithDiagnostics<T>>;

fn decode_codec(
    _data: &[u8],
    format: ImageFormat,
    token: Option<&crate::CancellationToken>,
) -> CodecDecodedResult<DecodedImage> {
    error::check_cancelled(token)?;
    let decoded: CodecDecodedResult<DecodedImage> =
        match format {
            #[cfg(feature = "jpeg")]
            ImageFormat::Jpeg => jpeg::decode::decode(_data, token)
                .map(|(image, consumed)| (image, Some(consumed), Vec::new())),
            #[cfg(not(feature = "jpeg"))]
            ImageFormat::Jpeg => Err(error::CodecError::Malformed(
                "JPEG feature is disabled".to_owned(),
            )),
            #[cfg(feature = "png")]
            ImageFormat::Png => png::decode::decode(_data, token)
                .map(|(image, consumed, diagnostics)| (image, Some(consumed), diagnostics)),
            #[cfg(not(feature = "png"))]
            ImageFormat::Png => Err(error::CodecError::Malformed(
                "PNG feature is disabled".to_owned(),
            )),
            #[cfg(feature = "gif")]
            ImageFormat::Gif => gif::decode::decode(_data, token)
                .map(|(image, consumed, diagnostics)| (image, Some(consumed), diagnostics)),
            #[cfg(not(feature = "gif"))]
            ImageFormat::Gif => Err(error::CodecError::Malformed(
                "GIF feature is disabled".to_owned(),
            )),
            #[cfg(feature = "bmp")]
            ImageFormat::Bmp => bmp::decode::decode(_data, token)
                .map(|(image, consumed)| (image, consumed, Vec::new())),
            #[cfg(not(feature = "bmp"))]
            ImageFormat::Bmp => Err(error::CodecError::Malformed(
                "BMP feature is disabled".to_owned(),
            )),
            #[cfg(feature = "tiff")]
            ImageFormat::Tiff => tiff::decode::decode(_data, token)
                .map(|(image, consumed)| (image, Some(consumed), Vec::new())),
            #[cfg(not(feature = "tiff"))]
            ImageFormat::Tiff => Err(error::CodecError::Malformed(
                "TIFF feature is disabled".to_owned(),
            )),
            #[cfg(feature = "webp")]
            ImageFormat::WebP => webp::decode::decode(_data, token)
                .map(|(image, consumed)| (image, Some(consumed), Vec::new())),
            #[cfg(not(feature = "webp"))]
            ImageFormat::WebP => Err(error::CodecError::Malformed(
                "WebP feature is disabled".to_owned(),
            )),
            #[cfg(feature = "ico")]
            ImageFormat::Ico => ico::decode::decode(_data, token)
                .map(|(image, consumed)| (image, consumed, Vec::new())),
            #[cfg(not(feature = "ico"))]
            ImageFormat::Ico => Err(error::CodecError::Malformed(
                "ICO feature is disabled".to_owned(),
            )),
            #[cfg(feature = "avif")]
            ImageFormat::Avif => avif::decode::decode(_data, token)
                .map(|(image, consumed)| (image, Some(consumed), Vec::new())),
            #[cfg(not(feature = "avif"))]
            ImageFormat::Avif => Err(error::CodecError::Malformed(
                "AVIF feature is disabled".to_owned(),
            )),
        };
    decoded
}

/// Dispatch decoding to the enabled format implementation.
pub(crate) fn decode_format(data: &[u8], format: ImageFormat) -> ImageDecodedResult<DecodedImage> {
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    ensure_available(format)?;
    let (image, consumed, diagnostics) = match decode_codec(data, format, None) {
        Ok(decoded) => decoded,
        Err(error) => {
            return Err(error
                .context("decode")
                .into_image_error(format, ImageErrorStage::StillDecode));
        }
    };
    validate_decoded_image(image).map(|image| {
        (
            image,
            consumed,
            set_diagnostic_stage(diagnostics, ImageErrorStage::StillDecode),
        )
    })
}

/// Dispatch still decode for the incremental-input surface: codec-level
/// truncation is exposed as the non-terminal [`ImageError::NeedMoreData`].
pub(crate) fn decode_prefix_format(
    data: &[u8],
    format: ImageFormat,
) -> ImageDecodedResult<DecodedImage> {
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    ensure_available(format)?;
    let (image, consumed, diagnostics) = match decode_codec(data, format, None) {
        Ok(decoded) => decoded,
        Err(error) => {
            return Err(error
                .context("decode")
                .into_incremental_image_error(format, ImageErrorStage::StillDecode));
        }
    };
    validate_decoded_image(image).map(|image| {
        (
            image,
            consumed,
            set_diagnostic_stage(diagnostics, ImageErrorStage::StillDecode),
        )
    })
}

/// Dispatch still decode with a caller-supplied cooperative cancellation
/// token; cancellation surfaces as [`ImageError::Cancelled`].
pub(crate) fn decode_token_format(
    data: &[u8],
    format: ImageFormat,
    token: &crate::CancellationToken,
) -> ImageDecodedResult<DecodedImage> {
    decode_token_format_with_mode(data, format, token, true)
}

/// Dispatch still decode with a caller-supplied work-budget token while
/// preserving the complete-slice terminal malformed-input contract.
pub(crate) fn decode_format_with_token(
    data: &[u8],
    format: ImageFormat,
    token: &crate::CancellationToken,
) -> ImageDecodedResult<DecodedImage> {
    decode_token_format_with_mode(data, format, token, false)
}

fn decode_token_format_with_mode(
    data: &[u8],
    format: ImageFormat,
    token: &crate::CancellationToken,
    incremental: bool,
) -> ImageDecodedResult<DecodedImage> {
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    ensure_available(format)?;
    let (image, consumed, diagnostics) = match decode_codec(data, format, Some(token)) {
        Ok(decoded) => decoded,
        Err(error) => {
            let error = error.context("decode");
            return if incremental {
                Err(error.into_incremental_image_error(format, ImageErrorStage::StillDecode))
            } else {
                Err(error.into_image_error(format, ImageErrorStage::StillDecode))
            };
        }
    };
    validate_decoded_image(image).map(|image| {
        (
            image,
            consumed,
            set_diagnostic_stage(diagnostics, ImageErrorStage::StillDecode),
        )
    })
}

fn validate_decoded_image(image: DecodedImage) -> ImageResult<DecodedImage> {
    image.validate()?;
    Ok(image)
}

/// Dispatch header inspection to the enabled format implementation.
pub(crate) fn inspect_format(_data: &[u8], format: ImageFormat) -> ImageResult<ImageInfo> {
    #[cfg(not(all(
        feature = "jpeg",
        feature = "png",
        feature = "gif",
        feature = "bmp",
        feature = "tiff",
        feature = "webp",
        feature = "ico",
        feature = "avif"
    )))]
    ensure_inspection_available(format)?;

    let inspected: CodecResult<ImageInfo> = match format {
        #[cfg(feature = "jpeg")]
        ImageFormat::Jpeg => jpeg::inspect::inspect(_data),
        #[cfg(not(feature = "jpeg"))]
        ImageFormat::Jpeg => Err(error::CodecError::Unsupported(
            "JPEG metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "png")]
        ImageFormat::Png => png::inspect::inspect(_data),
        #[cfg(not(feature = "png"))]
        ImageFormat::Png => Err(error::CodecError::Unsupported(
            "PNG metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "gif")]
        ImageFormat::Gif => gif::inspect::inspect(_data),
        #[cfg(not(feature = "gif"))]
        ImageFormat::Gif => Err(error::CodecError::Unsupported(
            "GIF metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "bmp")]
        ImageFormat::Bmp => bmp::inspect::inspect(_data),
        #[cfg(not(feature = "bmp"))]
        ImageFormat::Bmp => Err(error::CodecError::Unsupported(
            "BMP metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "tiff")]
        ImageFormat::Tiff => tiff::inspect::inspect(_data),
        #[cfg(not(feature = "tiff"))]
        ImageFormat::Tiff => Err(error::CodecError::Unsupported(
            "TIFF metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "webp")]
        ImageFormat::WebP => webp::inspect::inspect(_data),
        #[cfg(not(feature = "webp"))]
        ImageFormat::WebP => Err(error::CodecError::Unsupported(
            "WebP metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "ico")]
        ImageFormat::Ico => ico::inspect::inspect(_data),
        #[cfg(not(feature = "ico"))]
        ImageFormat::Ico => Err(error::CodecError::Unsupported(
            "ICO metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "avif")]
        ImageFormat::Avif => avif::inspect::inspect(_data),
        #[cfg(not(feature = "avif"))]
        ImageFormat::Avif => Err(error::CodecError::Unsupported(
            "AVIF metadata inspection is unavailable".to_owned(),
        )),
    };
    into_image_result(
        inspected.map_err(|error| error.context("inspect")),
        format,
        ImageErrorStage::Inspection,
    )
}

/// Invoke the format's header-only inspector below the public dispatcher.
fn inspect_basic_codec(_data: &[u8], format: ImageFormat) -> error::CodecResult<ImageInfo> {
    let inspected: CodecResult<ImageInfo> = match format {
        #[cfg(feature = "jpeg")]
        ImageFormat::Jpeg => jpeg::inspect::inspect(_data),
        #[cfg(not(feature = "jpeg"))]
        ImageFormat::Jpeg => Err(error::CodecError::Unsupported(
            "JPEG metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "png")]
        ImageFormat::Png => png::inspect::inspect(_data),
        #[cfg(not(feature = "png"))]
        ImageFormat::Png => Err(error::CodecError::Unsupported(
            "PNG metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "gif")]
        ImageFormat::Gif => gif::inspect::inspect_basic(_data),
        #[cfg(not(feature = "gif"))]
        ImageFormat::Gif => Err(error::CodecError::Unsupported(
            "GIF metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "bmp")]
        ImageFormat::Bmp => bmp::inspect::inspect(_data),
        #[cfg(not(feature = "bmp"))]
        ImageFormat::Bmp => Err(error::CodecError::Unsupported(
            "BMP metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "tiff")]
        ImageFormat::Tiff => tiff::inspect::inspect_basic(_data),
        #[cfg(not(feature = "tiff"))]
        ImageFormat::Tiff => Err(error::CodecError::Unsupported(
            "TIFF metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "webp")]
        ImageFormat::WebP => webp::inspect::inspect_basic(_data),
        #[cfg(not(feature = "webp"))]
        ImageFormat::WebP => Err(error::CodecError::Unsupported(
            "WebP metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "ico")]
        ImageFormat::Ico => ico::inspect::inspect(_data),
        #[cfg(not(feature = "ico"))]
        ImageFormat::Ico => Err(error::CodecError::Unsupported(
            "ICO metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "avif")]
        ImageFormat::Avif => avif::inspect::inspect(_data),
        #[cfg(not(feature = "avif"))]
        ImageFormat::Avif => Err(error::CodecError::Unsupported(
            "AVIF metadata inspection is unavailable".to_owned(),
        )),
    };
    inspected
}

/// Dispatch header-only inspection to the enabled format implementation.
pub(crate) fn inspect_basic_format(data: &[u8], format: ImageFormat) -> ImageResult<ImageInfo> {
    #[cfg(not(all(
        feature = "jpeg",
        feature = "png",
        feature = "gif",
        feature = "bmp",
        feature = "tiff",
        feature = "webp",
        feature = "ico",
        feature = "avif"
    )))]
    ensure_inspection_available(format)?;

    into_image_result(
        inspect_basic_codec(data, format).map_err(|error| error.context("inspect basic")),
        format,
        ImageErrorStage::Inspection,
    )
}

/// Dispatch header-only inspection for incremental input: short reads surface
/// as the non-terminal [`ImageError::NeedMoreData`] status.
pub(crate) fn inspect_basic_prefix_format(
    data: &[u8],
    format: ImageFormat,
) -> ImageResult<ImageInfo> {
    #[cfg(not(all(
        feature = "jpeg",
        feature = "png",
        feature = "gif",
        feature = "bmp",
        feature = "tiff",
        feature = "webp",
        feature = "ico",
        feature = "avif"
    )))]
    ensure_inspection_available(format)?;

    into_incremental_image_result(
        inspect_basic_codec(data, format).map_err(|error| error.context("inspect basic")),
        format,
        ImageErrorStage::Inspection,
    )
}

/// Apply the pinned Pillow oracle's codec-specific verification contract.
pub(crate) fn verify_format(_data: &[u8], format: ImageFormat) -> ImageResult<()> {
    #[cfg(not(all(
        feature = "jpeg",
        feature = "png",
        feature = "gif",
        feature = "bmp",
        feature = "tiff",
        feature = "webp",
        feature = "ico",
        feature = "avif"
    )))]
    ensure_inspection_available(format)?;

    let verified: CodecResult<()> = match format {
        #[cfg(feature = "png")]
        ImageFormat::Png => png::decode::verify(_data),
        #[cfg(feature = "webp")]
        ImageFormat::WebP => webp::decode::verify(_data),
        #[cfg(feature = "jpeg")]
        ImageFormat::Jpeg => jpeg::inspect::verify(_data),
        // Formats without a manifest-proven structural verifier retain
        // Pillow's metadata-open verification behavior. EncodedImage
        // construction has already performed that inspection.
        _ => Ok(()),
    };
    into_image_result(
        verified.map_err(|error| error.context("verify")),
        format,
        ImageErrorStage::Verification,
    )
}

#[cfg_attr(
    not(any(
        feature = "gif",
        feature = "png",
        feature = "webp",
        feature = "tiff",
        feature = "avif"
    )),
    allow(unused_variables)
)]
fn decode_sequence_codec(
    data: &[u8],
    format: ImageFormat,
    budget: &mut SequenceDecodeBudget,
    token: Option<&crate::CancellationToken>,
) -> Option<CodecDecodedResult<DecodedSequence>> {
    if let Err(error) = error::check_cancelled(token) {
        return Some(Err(error));
    }
    #[cfg(feature = "gif")]
    if format == ImageFormat::Gif {
        return Some(
            gif::decode::decode_sequence(data, budget, token)
                .map(|(sequence, consumed, diagnostics)| (sequence, Some(consumed), diagnostics)),
        );
    }

    #[cfg(feature = "png")]
    if format == ImageFormat::Png {
        return Some(
            png::decode::decode_sequence(data, budget, token)
                .map(|(sequence, consumed, diagnostics)| (sequence, Some(consumed), diagnostics)),
        );
    }

    #[cfg(feature = "webp")]
    if format == ImageFormat::WebP {
        return Some(
            webp::decode::decode_sequence(data, budget, token)
                .map(|(sequence, consumed)| (sequence, Some(consumed), Vec::new())),
        );
    }

    #[cfg(feature = "tiff")]
    if format == ImageFormat::Tiff {
        return Some(
            tiff::decode::decode_sequence(data, budget, token)
                .map(|(sequence, consumed)| (sequence, Some(consumed), Vec::new())),
        );
    }

    #[cfg(feature = "avif")]
    if format == ImageFormat::Avif {
        return Some(
            avif::decode::decode_sequence(data, budget, token)
                .map(|(sequence, consumed)| (sequence, Some(consumed), Vec::new())),
        );
    }

    None
}

fn still_to_sequence(
    mut image: DecodedImage,
    consumed: Option<usize>,
    diagnostics: Vec<crate::ImageDiagnostic>,
) -> DecodedWithDiagnostics<DecodedSequence> {
    let opaque_blocks = std::mem::take(&mut image.opaque_blocks);
    let metadata = std::mem::take(&mut image.metadata);
    let source_color = std::mem::take(&mut image.source_color);
    let mut sequence = DecodedSequence::from_image(image);
    sequence.opaque_blocks = opaque_blocks;
    sequence.metadata = metadata;
    sequence.source_color = source_color;
    (sequence, consumed, diagnostics)
}

/// Dispatch decoding while retaining every frame and its presentation data.
pub(crate) fn decode_sequence_format(
    data: &[u8],
    format: ImageFormat,
    budget: &mut SequenceDecodeBudget,
) -> ImageDecodedResult<DecodedSequence> {
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    ensure_available(format)?;
    if let Some(decoded) = decode_sequence_codec(data, format, budget, None) {
        return into_image_result(
            decoded.map_err(|error| error.context("decode sequence")),
            format,
            ImageErrorStage::SequenceDecode,
        )
        .map(|(sequence, consumed, diagnostics)| {
            (
                sequence,
                consumed,
                set_diagnostic_stage(diagnostics, ImageErrorStage::SequenceDecode),
            )
        });
    }
    decode_format(data, format)
        .map(|(image, consumed, diagnostics)| still_to_sequence(image, consumed, diagnostics))
}

/// Dispatch sequence decode for the incremental-input surface.
pub(crate) fn decode_sequence_prefix_format(
    data: &[u8],
    format: ImageFormat,
    budget: &mut SequenceDecodeBudget,
) -> ImageDecodedResult<DecodedSequence> {
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    ensure_available(format)?;
    if let Some(decoded) = decode_sequence_codec(data, format, budget, None) {
        return into_incremental_image_result(
            decoded.map_err(|error| error.context("decode sequence")),
            format,
            ImageErrorStage::SequenceDecode,
        )
        .map(|(sequence, consumed, diagnostics)| {
            (
                sequence,
                consumed,
                set_diagnostic_stage(diagnostics, ImageErrorStage::SequenceDecode),
            )
        });
    }
    decode_prefix_format(data, format)
        .map(|(image, consumed, diagnostics)| still_to_sequence(image, consumed, diagnostics))
}

/// Dispatch sequence decode with a caller-supplied cancellation token.
pub(crate) fn decode_sequence_token_format(
    data: &[u8],
    format: ImageFormat,
    budget: &mut SequenceDecodeBudget,
    token: &crate::CancellationToken,
) -> ImageDecodedResult<DecodedSequence> {
    decode_sequence_token_format_with_mode(data, format, budget, token, true)
}

/// Dispatch sequence decode with a caller-supplied work-budget token while
/// preserving the complete-slice terminal malformed-input contract.
pub(crate) fn decode_sequence_format_with_token(
    data: &[u8],
    format: ImageFormat,
    budget: &mut SequenceDecodeBudget,
    token: &crate::CancellationToken,
) -> ImageDecodedResult<DecodedSequence> {
    decode_sequence_token_format_with_mode(data, format, budget, token, false)
}

fn decode_sequence_token_format_with_mode(
    data: &[u8],
    format: ImageFormat,
    budget: &mut SequenceDecodeBudget,
    token: &crate::CancellationToken,
    incremental: bool,
) -> ImageDecodedResult<DecodedSequence> {
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    ensure_available(format)?;
    if let Some(decoded) = decode_sequence_codec(data, format, budget, Some(token)) {
        let decoded = decoded.map_err(|error| error.context("decode sequence"));
        let decoded = if incremental {
            into_incremental_image_result(decoded, format, ImageErrorStage::SequenceDecode)
        } else {
            into_image_result(decoded, format, ImageErrorStage::SequenceDecode)
        }?;
        return Ok((
            decoded.0,
            decoded.1,
            set_diagnostic_stage(decoded.2, ImageErrorStage::SequenceDecode),
        ));
    }
    if incremental {
        return decode_token_format(data, format, token)
            .map(|(image, consumed, diagnostics)| still_to_sequence(image, consumed, diagnostics));
    }
    decode_format_with_token(data, format, token)
        .map(|(image, consumed, diagnostics)| still_to_sequence(image, consumed, diagnostics))
}

/// Dispatch one-frame decode to the enabled format implementation.
///
/// TIFF decodes only the selected page's IFD; every other sequence format
/// currently decodes the full sequence and returns the indexed frame, so the
/// public contract is uniform while the eager fallback is documented.
#[cfg_attr(not(feature = "tiff"), allow(unused_variables))]
pub(crate) fn decode_frame_format(
    data: &[u8],
    format: ImageFormat,
    index: u32,
) -> ImageResult<DecodedFrame> {
    #[cfg(feature = "tiff")]
    if format == ImageFormat::Tiff {
        return into_image_result(
            tiff::decode::decode_page(data, index, None).map(|(image, _)| {
                DecodedFrame::source_rectangle(
                    image,
                    0,
                    0,
                    crate::types::FrameDuration::ZERO,
                    crate::types::FrameDisposal::Unspecified,
                    crate::types::FrameBlend::Unspecified,
                    false,
                )
            }),
            format,
            ImageErrorStage::SequenceDecode,
        );
    }
    let sequence = crate::decode_sequence(data)?.into_inner();
    sequence
        .frames
        .get(index as usize)
        .cloned()
        .ok_or_else(|| ImageError::parameter(format!("frame index {index} is out of range")))
}

/// Dispatch encoding to the enabled format implementation.
pub(crate) fn encode_format(
    image: &DecodedImage,
    format: ImageFormat,
    options: &EncodeOptions,
) -> ImageResult<Vec<u8>> {
    encode_format_with_token(image, format, options, None)
}

/// Dispatch encoding with an optional cooperative cancellation token.
pub(crate) fn encode_format_with_token(
    _image: &DecodedImage,
    format: ImageFormat,
    _options: &EncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> ImageResult<Vec<u8>> {
    into_image_result(
        error::check_cancelled(token),
        format,
        ImageErrorStage::StillEncode,
    )?;
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    ensure_available(format)?;
    _image
        .validate()
        .map_err(|error| error.with_format(format))?;
    #[cfg(any(
        feature = "jpeg",
        feature = "png",
        feature = "gif",
        feature = "bmp",
        feature = "tiff",
        feature = "webp",
        feature = "ico",
        feature = "avif"
    ))]
    {
        let encoded: CodecResult<Vec<u8>> = match (format, _options) {
            #[cfg(feature = "jpeg")]
            (ImageFormat::Jpeg, EncodeOptions::Jpeg(options)) => {
                jpeg::encode::encode_with_token(_image, options, token)
            }
            #[cfg(feature = "png")]
            (ImageFormat::Png, EncodeOptions::Png(options)) => match token {
                Some(token) => png::encode::encode_with_token(_image, options, Some(token)),
                None => png::encode::encode(_image, options),
            },
            #[cfg(feature = "gif")]
            (ImageFormat::Gif, EncodeOptions::Gif(options)) => match token {
                Some(token) => gif::encode::encode_with_token(_image, options, Some(token)),
                None => gif::encode::encode(_image, options),
            },
            #[cfg(feature = "bmp")]
            (ImageFormat::Bmp, EncodeOptions::Bmp(options)) => match token {
                Some(token) => bmp::encode::encode_with_token(_image, options, Some(token)),
                None => bmp::encode::encode(_image, options),
            },
            #[cfg(feature = "tiff")]
            (ImageFormat::Tiff, EncodeOptions::Tiff(options)) => match token {
                Some(token) => tiff::encode::encode_with_token(_image, options, Some(token)),
                None => tiff::encode::encode(_image, options),
            },
            #[cfg(feature = "webp")]
            (ImageFormat::WebP, EncodeOptions::WebP(options)) => match token {
                Some(token) => webp::encode::encode_with_token(_image, options, Some(token)),
                None => webp::encode::encode(_image, options),
            },
            #[cfg(feature = "ico")]
            (ImageFormat::Ico, EncodeOptions::Ico(options)) => match token {
                Some(token) => ico::encode::encode_with_token(_image, options, Some(token)),
                None => ico::encode::encode(_image, options),
            },
            #[cfg(feature = "avif")]
            (ImageFormat::Avif, EncodeOptions::Avif(options)) => match token {
                Some(token) => avif::encode::encode_with_token(_image, options, Some(token)),
                None => avif::encode::encode(_image, options),
            },
            _ => {
                return Err(option_format_mismatch(
                    format,
                    _options,
                    ImageErrorStage::StillEncode,
                ));
            }
        };
        let encoded = into_image_result(
            encoded.map_err(|error| error.context("encode")),
            format,
            ImageErrorStage::StillEncode,
        )?;
        into_image_result(
            error::check_cancelled(token),
            format,
            ImageErrorStage::StillEncode,
        )?;
        Ok(encoded)
    }
    #[cfg(not(any(
        feature = "jpeg",
        feature = "png",
        feature = "gif",
        feature = "bmp",
        feature = "tiff",
        feature = "webp",
        feature = "ico",
        feature = "avif"
    )))]
    Err(option_format_mismatch(
        format,
        _options,
        ImageErrorStage::StillEncode,
    ))
}

/// Try the first format-specific structural writer without changing the
/// whole-buffer fallback used by other codecs.
#[cfg_attr(
    not(any(
        feature = "jpeg",
        feature = "bmp",
        feature = "gif",
        feature = "ico",
        feature = "png",
        feature = "tiff",
        feature = "webp",
        feature = "avif"
    )),
    allow(unused_variables)
)]
pub(crate) fn encode_format_to_sink_with_token(
    image: &DecodedImage,
    format: ImageFormat,
    options: &EncodeOptions,
    policy: EncodePolicy,
    token: Option<&crate::CancellationToken>,
    sink: &mut dyn crate::OutputSink,
) -> ImageResult<Option<usize>> {
    if format == ImageFormat::Avif {
        #[cfg(not(feature = "avif"))]
        {
            return Err(ImageError::FeatureDisabled {
                format,
                feature: "avif",
            });
        }
        #[cfg(feature = "avif")]
        {
            #[cfg(any(
                not(all(
                    feature = "jpeg",
                    feature = "png",
                    feature = "gif",
                    feature = "bmp",
                    feature = "tiff",
                    feature = "webp",
                    feature = "ico",
                    feature = "avif"
                )),
                target_arch = "wasm32"
            ))]
            ensure_available(format)?;
            image
                .validate()
                .map_err(|error| error.with_format(format))?;
            let EncodeOptions::Avif(options) = options else {
                return Err(option_format_mismatch(
                    format,
                    options,
                    ImageErrorStage::StillEncode,
                ));
            };
            let encoded = avif::encode::encode_to_sink(
                image,
                options,
                policy,
                CodecOperation::StillEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode")),
                format,
                ImageErrorStage::StillEncode,
            )
            .map(Some);
        }
    }

    if format == ImageFormat::Jpeg {
        #[cfg(not(feature = "jpeg"))]
        {
            return Err(ImageError::FeatureDisabled {
                format,
                feature: "jpeg",
            });
        }
        #[cfg(feature = "jpeg")]
        {
            #[cfg(any(
                not(all(
                    feature = "jpeg",
                    feature = "png",
                    feature = "gif",
                    feature = "bmp",
                    feature = "tiff",
                    feature = "webp",
                    feature = "ico",
                    feature = "avif"
                )),
                target_arch = "wasm32"
            ))]
            ensure_available(format)?;
            image
                .validate()
                .map_err(|error| error.with_format(format))?;
            let EncodeOptions::Jpeg(options) = options else {
                return Err(option_format_mismatch(
                    format,
                    options,
                    ImageErrorStage::StillEncode,
                ));
            };
            let encoded = jpeg::encode::encode_to_sink(
                image,
                options,
                policy,
                CodecOperation::StillEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode")),
                format,
                ImageErrorStage::StillEncode,
            )
            .map(Some);
        }
    }

    if format == ImageFormat::Png {
        #[cfg(not(feature = "png"))]
        {
            return Err(ImageError::FeatureDisabled {
                format,
                feature: "png",
            });
        }
        #[cfg(feature = "png")]
        {
            #[cfg(any(
                not(all(
                    feature = "jpeg",
                    feature = "png",
                    feature = "gif",
                    feature = "bmp",
                    feature = "tiff",
                    feature = "webp",
                    feature = "ico",
                    feature = "avif"
                )),
                target_arch = "wasm32"
            ))]
            ensure_available(format)?;
            image
                .validate()
                .map_err(|error| error.with_format(format))?;
            let EncodeOptions::Png(options) = options else {
                return Err(option_format_mismatch(
                    format,
                    options,
                    ImageErrorStage::StillEncode,
                ));
            };
            let encoded = png::encode::encode_to_sink(
                image,
                options,
                policy,
                CodecOperation::StillEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode")),
                format,
                ImageErrorStage::StillEncode,
            )
            .map(Some);
        }
    }

    if format == ImageFormat::Gif {
        #[cfg(not(feature = "gif"))]
        {
            return Err(ImageError::FeatureDisabled {
                format,
                feature: "gif",
            });
        }
        #[cfg(feature = "gif")]
        {
            #[cfg(any(
                not(all(
                    feature = "jpeg",
                    feature = "png",
                    feature = "gif",
                    feature = "bmp",
                    feature = "tiff",
                    feature = "webp",
                    feature = "ico",
                    feature = "avif"
                )),
                target_arch = "wasm32"
            ))]
            ensure_available(format)?;
            image
                .validate()
                .map_err(|error| error.with_format(format))?;
            let EncodeOptions::Gif(options) = options else {
                return Err(option_format_mismatch(
                    format,
                    options,
                    ImageErrorStage::StillEncode,
                ));
            };
            let encoded = gif::encode::encode_to_sink(
                image,
                options,
                policy,
                CodecOperation::StillEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode")),
                format,
                ImageErrorStage::StillEncode,
            )
            .map(Some);
        }
    }

    if format == ImageFormat::Bmp {
        #[cfg(not(feature = "bmp"))]
        {
            return Err(ImageError::FeatureDisabled {
                format,
                feature: "bmp",
            });
        }
        #[cfg(feature = "bmp")]
        {
            #[cfg(any(
                not(all(
                    feature = "jpeg",
                    feature = "png",
                    feature = "gif",
                    feature = "bmp",
                    feature = "tiff",
                    feature = "webp",
                    feature = "ico",
                    feature = "avif"
                )),
                target_arch = "wasm32"
            ))]
            ensure_available(format)?;
            image
                .validate()
                .map_err(|error| error.with_format(format))?;
            let EncodeOptions::Bmp(options) = options else {
                return Err(option_format_mismatch(
                    format,
                    options,
                    ImageErrorStage::StillEncode,
                ));
            };
            let encoded = bmp::encode::encode_to_sink(
                image,
                options,
                policy,
                CodecOperation::StillEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode")),
                format,
                ImageErrorStage::StillEncode,
            )
            .map(Some);
        }
    }

    if format == ImageFormat::Tiff {
        #[cfg(not(feature = "tiff"))]
        {
            return Err(ImageError::FeatureDisabled {
                format,
                feature: "tiff",
            });
        }
        #[cfg(feature = "tiff")]
        {
            #[cfg(any(
                not(all(
                    feature = "jpeg",
                    feature = "png",
                    feature = "gif",
                    feature = "bmp",
                    feature = "tiff",
                    feature = "webp",
                    feature = "ico",
                    feature = "avif"
                )),
                target_arch = "wasm32"
            ))]
            ensure_available(format)?;
            image
                .validate()
                .map_err(|error| error.with_format(format))?;
            let options = match options {
                EncodeOptions::Tiff(options) => options,
                options => {
                    return Err(option_format_mismatch(
                        format,
                        options,
                        ImageErrorStage::StillEncode,
                    ));
                }
            };
            let encoded = tiff::encode::encode_to_sink(
                image,
                options,
                policy,
                CodecOperation::StillEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode")),
                format,
                ImageErrorStage::StillEncode,
            )
            .map(Some);
        }
    }

    if format == ImageFormat::WebP {
        #[cfg(not(feature = "webp"))]
        {
            return Err(ImageError::FeatureDisabled {
                format,
                feature: "webp",
            });
        }
        #[cfg(feature = "webp")]
        {
            #[cfg(any(
                not(all(
                    feature = "jpeg",
                    feature = "png",
                    feature = "gif",
                    feature = "bmp",
                    feature = "tiff",
                    feature = "webp",
                    feature = "ico",
                    feature = "avif"
                )),
                target_arch = "wasm32"
            ))]
            ensure_available(format)?;
            image
                .validate()
                .map_err(|error| error.with_format(format))?;
            let EncodeOptions::WebP(options) = options else {
                return Err(option_format_mismatch(
                    format,
                    options,
                    ImageErrorStage::StillEncode,
                ));
            };
            let encoded = webp::encode::encode_to_sink(
                image,
                options,
                policy,
                CodecOperation::StillEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode")),
                format,
                ImageErrorStage::StillEncode,
            )
            .map(Some);
        }
    }

    // Every supported still format above returns, so reaching this tail means
    // the only remaining enum variant is ICO.  Document that invariant;
    // the old final `if` created an impossible false branch in all-features
    // coverage runs.
    #[cfg(not(feature = "ico"))]
    {
        Err(ImageError::FeatureDisabled {
            format,
            feature: "ico",
        })
    }
    #[cfg(feature = "ico")]
    {
        #[cfg(any(
            not(all(
                feature = "jpeg",
                feature = "png",
                feature = "gif",
                feature = "bmp",
                feature = "tiff",
                feature = "webp",
                feature = "ico",
                feature = "avif"
            )),
            target_arch = "wasm32"
        ))]
        ensure_available(format)?;
        let EncodeOptions::Ico(options) = options else {
            return Err(option_format_mismatch(
                format,
                options,
                ImageErrorStage::StillEncode,
            ));
        };
        let encoded = ico::encode::encode_to_sink(
            image,
            options,
            policy,
            CodecOperation::StillEncode,
            token,
            sink,
        );
        into_image_result(
            encoded.map_err(|error| error.context("encode")),
            format,
            ImageErrorStage::StillEncode,
        )
        .map(Some)
    }
}

/// Try the structural writer for supported JPEG, PNG, BMP, GIF, WebP, TIFF,
/// ICO, or AVIF sequence delivery.
#[cfg_attr(
    not(any(
        feature = "avif",
        feature = "bmp",
        feature = "gif",
        feature = "ico",
        feature = "jpeg",
        feature = "png",
        feature = "tiff",
        feature = "webp"
    )),
    allow(unused_variables)
)]
pub(crate) fn encode_sequence_to_sink_with_token(
    sequence: &DecodedSequence,
    format: ImageFormat,
    options: &EncodeOptions,
    policy: EncodePolicy,
    token: Option<&crate::CancellationToken>,
    sink: &mut dyn crate::OutputSink,
) -> ImageResult<Option<usize>> {
    if format == ImageFormat::Jpeg {
        #[cfg(not(feature = "jpeg"))]
        {
            return Err(ImageError::FeatureDisabled {
                format,
                feature: "jpeg",
            });
        }
        #[cfg(feature = "jpeg")]
        {
            #[cfg(any(
                not(all(
                    feature = "jpeg",
                    feature = "png",
                    feature = "gif",
                    feature = "bmp",
                    feature = "tiff",
                    feature = "webp",
                    feature = "ico",
                    feature = "avif"
                )),
                target_arch = "wasm32"
            ))]
            ensure_available(format)?;
            let frame = single_frame_for_sink(sequence, format)?;
            let EncodeOptions::Jpeg(options) = options else {
                return Err(option_format_mismatch(
                    format,
                    options,
                    ImageErrorStage::SequenceEncode,
                ));
            };
            let encoded = jpeg::encode::encode_to_sink(
                &frame.image,
                options,
                policy,
                CodecOperation::SequenceEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            )
            .map(Some);
        }
    }

    if format == ImageFormat::Avif {
        #[cfg(not(feature = "avif"))]
        {
            return Err(ImageError::FeatureDisabled {
                format,
                feature: "avif",
            });
        }
        #[cfg(feature = "avif")]
        {
            #[cfg(any(
                not(all(
                    feature = "jpeg",
                    feature = "png",
                    feature = "gif",
                    feature = "bmp",
                    feature = "tiff",
                    feature = "webp",
                    feature = "ico",
                    feature = "avif"
                )),
                target_arch = "wasm32"
            ))]
            ensure_available(format)?;
            let EncodeOptions::Avif(options) = options else {
                return Err(option_format_mismatch(
                    format,
                    options,
                    ImageErrorStage::SequenceEncode,
                ));
            };
            let encoded = avif::encode::encode_sequence_to_sink(
                sequence,
                options,
                policy,
                CodecOperation::SequenceEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            )
            .map(Some);
        }
    }

    if format == ImageFormat::Gif {
        #[cfg(not(feature = "gif"))]
        {
            return Err(ImageError::FeatureDisabled {
                format,
                feature: "gif",
            });
        }
        #[cfg(feature = "gif")]
        {
            #[cfg(any(
                not(all(
                    feature = "jpeg",
                    feature = "png",
                    feature = "gif",
                    feature = "bmp",
                    feature = "tiff",
                    feature = "webp",
                    feature = "ico",
                    feature = "avif"
                )),
                target_arch = "wasm32"
            ))]
            ensure_available(format)?;
            let EncodeOptions::Gif(options) = options else {
                return Err(option_format_mismatch(
                    format,
                    options,
                    ImageErrorStage::SequenceEncode,
                ));
            };
            let encoded = gif::encode::encode_sequence_to_sink(
                sequence,
                options,
                policy,
                CodecOperation::SequenceEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            )
            .map(Some);
        }
    }

    if format == ImageFormat::Png {
        #[cfg(not(feature = "png"))]
        {
            return Err(ImageError::FeatureDisabled {
                format,
                feature: "png",
            });
        }
        #[cfg(feature = "png")]
        {
            #[cfg(any(
                not(all(
                    feature = "jpeg",
                    feature = "png",
                    feature = "gif",
                    feature = "bmp",
                    feature = "tiff",
                    feature = "webp",
                    feature = "ico",
                    feature = "avif"
                )),
                target_arch = "wasm32"
            ))]
            ensure_available(format)?;
            let frame = single_frame_for_sink(sequence, format)?;
            let EncodeOptions::Png(options) = options else {
                return Err(option_format_mismatch(
                    format,
                    options,
                    ImageErrorStage::SequenceEncode,
                ));
            };
            let encoded = png::encode::encode_to_sink(
                &frame.image,
                options,
                policy,
                CodecOperation::SequenceEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            )
            .map(Some);
        }
    }

    if format == ImageFormat::Bmp {
        #[cfg(not(feature = "bmp"))]
        {
            return Err(ImageError::FeatureDisabled {
                format,
                feature: "bmp",
            });
        }
        #[cfg(feature = "bmp")]
        {
            #[cfg(any(
                not(all(
                    feature = "jpeg",
                    feature = "png",
                    feature = "gif",
                    feature = "bmp",
                    feature = "tiff",
                    feature = "webp",
                    feature = "ico",
                    feature = "avif"
                )),
                target_arch = "wasm32"
            ))]
            ensure_available(format)?;
            let frame = single_frame_for_sink(sequence, format)?;
            let EncodeOptions::Bmp(options) = options else {
                return Err(option_format_mismatch(
                    format,
                    options,
                    ImageErrorStage::SequenceEncode,
                ));
            };
            let encoded = bmp::encode::encode_to_sink(
                &frame.image,
                options,
                policy,
                CodecOperation::SequenceEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            )
            .map(Some);
        }
    }

    if format == ImageFormat::Tiff {
        #[cfg(not(feature = "tiff"))]
        {
            return Err(ImageError::FeatureDisabled {
                format,
                feature: "tiff",
            });
        }
        #[cfg(feature = "tiff")]
        {
            #[cfg(any(
                not(all(
                    feature = "jpeg",
                    feature = "png",
                    feature = "gif",
                    feature = "bmp",
                    feature = "tiff",
                    feature = "webp",
                    feature = "ico",
                    feature = "avif"
                )),
                target_arch = "wasm32"
            ))]
            ensure_available(format)?;
            let options = match options {
                EncodeOptions::Tiff(options) => options,
                options => {
                    return Err(option_format_mismatch(
                        format,
                        options,
                        ImageErrorStage::SequenceEncode,
                    ));
                }
            };
            let encoded = tiff::encode::encode_sequence_to_sink(
                sequence,
                options,
                policy,
                CodecOperation::SequenceEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            )
            .map(Some);
        }
    }

    if format == ImageFormat::Ico {
        #[cfg(not(feature = "ico"))]
        {
            return Err(ImageError::FeatureDisabled {
                format,
                feature: "ico",
            });
        }
        #[cfg(feature = "ico")]
        {
            #[cfg(any(
                not(all(
                    feature = "jpeg",
                    feature = "png",
                    feature = "gif",
                    feature = "bmp",
                    feature = "tiff",
                    feature = "webp",
                    feature = "ico",
                    feature = "avif"
                )),
                target_arch = "wasm32"
            ))]
            ensure_available(format)?;
            let frame = single_frame_for_sink(sequence, format)?;
            let EncodeOptions::Ico(options) = options else {
                return Err(option_format_mismatch(
                    format,
                    options,
                    ImageErrorStage::SequenceEncode,
                ));
            };
            let encoded = ico::encode::encode_to_sink(
                &frame.image,
                options,
                policy,
                CodecOperation::SequenceEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            )
            .map(Some);
        }
    }

    // All earlier sequence formats return, so this tail is the WebP variant.
    // The frame-count split remains real API behavior; only the old outer
    // format check was an impossible false branch in the all-features build.
    #[cfg(not(feature = "webp"))]
    {
        Err(ImageError::FeatureDisabled {
            format,
            feature: "webp",
        })
    }
    #[cfg(feature = "webp")]
    {
        #[cfg(any(
            not(all(
                feature = "jpeg",
                feature = "png",
                feature = "gif",
                feature = "bmp",
                feature = "tiff",
                feature = "webp",
                feature = "ico",
                feature = "avif"
            )),
            target_arch = "wasm32"
        ))]
        ensure_available(format)?;
        if sequence.frames.len() > 1 {
            let EncodeOptions::WebP(options) = options else {
                return Err(option_format_mismatch(
                    format,
                    options,
                    ImageErrorStage::SequenceEncode,
                ));
            };
            let encoded = webp::encode::encode_sequence_to_sink(
                sequence,
                options,
                policy,
                CodecOperation::SequenceEncode,
                token,
                sink,
            );
            return into_image_result(
                encoded.map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            )
            .map(Some);
        }
        if sequence.frames.len() != 1 {
            return Ok(None);
        }
        let frame = single_frame_for_sink(sequence, format)?;
        let EncodeOptions::WebP(options) = options else {
            return Err(option_format_mismatch(
                format,
                options,
                ImageErrorStage::SequenceEncode,
            ));
        };
        let encoded = webp::encode::encode_to_sink(
            &frame.image,
            options,
            policy,
            CodecOperation::SequenceEncode,
            token,
            sink,
        );
        into_image_result(
            encoded.map_err(|error| error.context("encode sequence")),
            format,
            ImageErrorStage::SequenceEncode,
        )
        .map(Some)
    }
}

#[cfg(any(
    feature = "bmp",
    feature = "ico",
    feature = "jpeg",
    feature = "png",
    feature = "webp"
))]
fn single_frame_for_sink(
    sequence: &DecodedSequence,
    format: ImageFormat,
) -> ImageResult<&crate::types::DecodedFrame> {
    sequence
        .validate()
        .map_err(|error| error.with_format(format))?;
    if sequence.frames.len() != 1 {
        return Err(ImageError::Unsupported {
            format: Some(format),
            message: "format cannot encode multiple retained frames".to_owned(),
            stage: Some(ImageErrorStage::SequenceEncode),
            reason: Some(crate::UnsupportedReason::NotImplemented),
            offset: None,
            identity: None,
        });
    }
    let frame = &sequence.frames[0];
    if !has_plain_still_semantics(sequence, frame) {
        return Err(ImageError::Unsupported {
            format: Some(format),
            message: "still-image format cannot represent retained sequence metadata".to_owned(),
            stage: Some(ImageErrorStage::SequenceEncode),
            reason: None,
            offset: None,
            identity: None,
        });
    }
    Ok(frame)
}

/// Dispatch encoding without collapsing an animation to its first frame.
pub(crate) fn encode_sequence_format(
    sequence: &DecodedSequence,
    format: ImageFormat,
    options: &EncodeOptions,
) -> ImageResult<Vec<u8>> {
    encode_sequence_format_with_token(sequence, format, options, None)
}

/// Dispatch sequence encoding with an optional cooperative cancellation token.
pub(crate) fn encode_sequence_format_with_token(
    sequence: &DecodedSequence,
    format: ImageFormat,
    options: &EncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> ImageResult<Vec<u8>> {
    into_image_result(
        error::check_cancelled(token),
        format,
        ImageErrorStage::SequenceEncode,
    )?;
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    ensure_available(format)?;
    sequence
        .validate()
        .map_err(|error| error.with_format(format))?;
    match (format, options) {
        #[cfg(feature = "gif")]
        (ImageFormat::Gif, EncodeOptions::Gif(options)) => {
            let encoded = match token {
                Some(token) => {
                    gif::encode::encode_sequence_with_token(sequence, options, Some(token))
                }
                None => gif::encode::encode_sequence(sequence, options),
            };
            return into_image_result(
                encoded.map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            );
        }
        #[cfg(feature = "avif")]
        (ImageFormat::Avif, EncodeOptions::Avif(options)) => {
            let encoded = match token {
                Some(token) => {
                    avif::encode::encode_sequence_with_token(sequence, options, Some(token))
                }
                None => avif::encode::encode_sequence(sequence, options),
            };
            return into_image_result(
                encoded.map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            );
        }
        #[cfg(feature = "tiff")]
        (ImageFormat::Tiff, EncodeOptions::Tiff(options)) => {
            let encoded = match token {
                Some(token) => {
                    tiff::encode::encode_sequence_with_token(sequence, options, Some(token))
                }
                None => tiff::encode::encode_sequence(sequence, options),
            };
            return into_image_result(
                encoded.map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            );
        }
        #[cfg(feature = "webp")]
        (ImageFormat::WebP, EncodeOptions::WebP(options)) if sequence.frames.len() > 1 => {
            let encoded = match token {
                Some(token) => {
                    webp::encode::encode_sequence_with_token(sequence, options, Some(token))
                }
                None => webp::encode::encode_sequence(sequence, options),
            };
            return into_image_result(
                encoded.map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            );
        }
        (_, supplied) if supplied.format() != format => {
            return Err(option_format_mismatch(
                format,
                supplied,
                ImageErrorStage::SequenceEncode,
            ));
        }
        _ => {}
    }

    if sequence.frames.len() != 1 {
        return Err(ImageError::Unsupported {
            format: Some(format),
            message: "format cannot encode multiple retained frames".to_owned(),
            stage: Some(ImageErrorStage::SequenceEncode),
            reason: Some(crate::UnsupportedReason::NotImplemented),
            offset: None,
            identity: None,
        });
    }
    let frame = &sequence.frames[0];
    if !has_plain_still_semantics(sequence, frame) {
        return Err(ImageError::Unsupported {
            format: Some(format),
            message: "still-image format cannot represent retained sequence metadata".to_owned(),
            stage: Some(ImageErrorStage::SequenceEncode),
            reason: None,
            offset: None,
            identity: None,
        });
    }
    encode_format_with_token(&frame.image, format, options, token)
}

fn option_format_mismatch(
    format: ImageFormat,
    options: &EncodeOptions,
    stage: ImageErrorStage,
) -> ImageError {
    ImageError::Parameter {
        format: Some(format),
        message: format!(
            "{} options cannot be used to encode {}",
            options.format(),
            format
        ),
        stage: Some(stage),
        offset: None,
        identity: None,
    }
}

/// Measure the encoded metadata extent of one detected format.
///
/// The metadata extent is the container-defined consumed length minus the
/// encoded bytes of primary pixel payload data. It is computed without
/// decompressing pixels so `max_metadata_bytes` can reject before codec work.
pub(crate) fn metadata_bytes_format(data: &[u8], format: ImageFormat) -> ImageResult<u64> {
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    ensure_available(format)?;
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    let _ = data;
    let metadata: CodecResult<u64> = match format {
        #[cfg(feature = "jpeg")]
        ImageFormat::Jpeg => jpeg::decode::metadata_bytes(data),
        #[cfg(not(feature = "jpeg"))]
        ImageFormat::Jpeg => Err(error::CodecError::Malformed(
            "JPEG feature is disabled".to_owned(),
        )),
        #[cfg(feature = "png")]
        ImageFormat::Png => png::decode::metadata_bytes(data),
        #[cfg(not(feature = "png"))]
        ImageFormat::Png => Err(error::CodecError::Malformed(
            "PNG feature is disabled".to_owned(),
        )),
        #[cfg(feature = "gif")]
        ImageFormat::Gif => gif::decode::metadata_bytes(data),
        #[cfg(not(feature = "gif"))]
        ImageFormat::Gif => Err(error::CodecError::Malformed(
            "GIF feature is disabled".to_owned(),
        )),
        #[cfg(feature = "bmp")]
        ImageFormat::Bmp => bmp::decode::metadata_bytes(data),
        #[cfg(not(feature = "bmp"))]
        ImageFormat::Bmp => Err(error::CodecError::Malformed(
            "BMP feature is disabled".to_owned(),
        )),
        #[cfg(feature = "tiff")]
        ImageFormat::Tiff => tiff::decode::metadata_bytes(data),
        #[cfg(not(feature = "tiff"))]
        ImageFormat::Tiff => Err(error::CodecError::Malformed(
            "TIFF feature is disabled".to_owned(),
        )),
        #[cfg(feature = "webp")]
        ImageFormat::WebP => webp::decode::metadata_bytes(data),
        #[cfg(not(feature = "webp"))]
        ImageFormat::WebP => Err(error::CodecError::Malformed(
            "WebP feature is disabled".to_owned(),
        )),
        #[cfg(feature = "ico")]
        ImageFormat::Ico => ico::decode::metadata_bytes(data),
        #[cfg(not(feature = "ico"))]
        ImageFormat::Ico => Err(error::CodecError::Malformed(
            "ICO feature is disabled".to_owned(),
        )),
        #[cfg(feature = "avif")]
        ImageFormat::Avif => avif::decode::metadata_bytes(data),
        #[cfg(not(feature = "avif"))]
        ImageFormat::Avif => Err(error::CodecError::Malformed(
            "AVIF feature is disabled".to_owned(),
        )),
    };
    into_image_result(
        metadata.map_err(|error| error.context("metadata measure")),
        format,
        ImageErrorStage::Inspection,
    )
}

fn has_plain_still_semantics(
    sequence: &DecodedSequence,
    frame: &crate::types::DecodedFrame,
) -> bool {
    if frame.source.rect.left != 0 {
        return false;
    }
    if frame.source.rect.top != 0 {
        return false;
    }
    if frame.source.rect.width != sequence.width || frame.source.rect.height != sequence.height {
        return false;
    }
    if frame.source.duration.numerator != 0 {
        return false;
    }
    if frame.source.disposal != FrameDisposal::Unspecified {
        return false;
    }
    if frame.source.blend != crate::types::FrameBlend::Unspecified
        || frame.source.interlaced
        || frame.source.is_default_image
    {
        return false;
    }
    if sequence.loop_count.is_some() {
        return false;
    }
    if sequence.background.is_some() {
        return false;
    }
    true
}

#[cfg(all(
    target_arch = "wasm32",
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico",
    feature = "avif"
))]
fn ensure_available(_format: ImageFormat) -> ImageResult<()> {
    Ok(())
}

#[cfg(not(all(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico",
    feature = "avif"
)))]
fn ensure_available(format: ImageFormat) -> ImageResult<()> {
    ensure_enabled(format)
}

#[cfg(not(all(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico",
    feature = "avif"
)))]
fn ensure_inspection_available(format: ImageFormat) -> ImageResult<()> {
    ensure_enabled(format)
}

#[cfg(not(all(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico",
    feature = "avif"
)))]
fn ensure_enabled(format: ImageFormat) -> ImageResult<()> {
    #[cfg(not(feature = "jpeg"))]
    if format == ImageFormat::Jpeg {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "jpeg",
        });
    }
    #[cfg(not(feature = "png"))]
    if format == ImageFormat::Png {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "png",
        });
    }
    #[cfg(not(feature = "gif"))]
    if format == ImageFormat::Gif {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "gif",
        });
    }
    #[cfg(not(feature = "bmp"))]
    if format == ImageFormat::Bmp {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "bmp",
        });
    }
    #[cfg(not(feature = "tiff"))]
    if format == ImageFormat::Tiff {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "tiff",
        });
    }
    #[cfg(not(feature = "webp"))]
    if format == ImageFormat::WebP {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "webp",
        });
    }
    #[cfg(not(feature = "ico"))]
    if format == ImageFormat::Ico {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "ico",
        });
    }
    #[cfg(not(feature = "avif"))]
    if format == ImageFormat::Avif {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "avif",
        });
    }
    Ok(())
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    error::__coverage_exercise_private_branches();

    // The eager frame-decode fallback's sequence-error path is exercised
    // with bytes whose signature never reaches a codec.
    let _ = decode_frame_format(b"not an image", ImageFormat::Gif, 0);

    let invalid_sequence = DecodedSequence {
        width: 0,
        height: 1,
        frames: Vec::new(),
        loop_count: None,
        background: None,
        kind: crate::types::SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = encode_sequence_format(
        &invalid_sequence,
        ImageFormat::Png,
        &EncodeOptions::for_format(ImageFormat::Png),
    );

    let luma = DecodedImage::new(1, 1, vec![0], crate::types::ColorType::L8);
    let rgb = DecodedImage::new(1, 1, vec![0, 0, 0], crate::types::ColorType::Rgb8);

    // Exercise the public dispatch matrix for every enabled target.  Each
    // format gets its successful structural writer, its typed option-mismatch
    // error, and both still and one-frame sequence delivery.  These are real
    // caller routes; Pillow parity cannot observe the Rust sink boundary or
    // the format-qualified options error.
    let dispatch_formats = [
        ImageFormat::Jpeg,
        ImageFormat::Png,
        ImageFormat::Gif,
        ImageFormat::Bmp,
        ImageFormat::Tiff,
        ImageFormat::WebP,
        ImageFormat::Ico,
        ImageFormat::Avif,
    ];
    let one_frame_sequence = DecodedSequence::from_image(rgb.clone());
    for format in dispatch_formats {
        let options = EncodeOptions::for_format(format);
        let wrong_format = if format == ImageFormat::Png {
            ImageFormat::Jpeg
        } else {
            ImageFormat::Png
        };
        let wrong_options = EncodeOptions::for_format(wrong_format);
        let token = crate::CancellationToken::new();
        let mut still_sink = Vec::new();
        let _ = encode_format_to_sink_with_token(
            &rgb,
            format,
            &options,
            EncodePolicy::default(),
            Some(&token),
            &mut still_sink,
        );
        let mut still_mismatch_sink = Vec::new();
        let _ = encode_format_to_sink_with_token(
            &rgb,
            format,
            &wrong_options,
            EncodePolicy::default(),
            None,
            &mut still_mismatch_sink,
        );
        let mut sequence_sink = Vec::new();
        let _ = encode_sequence_to_sink_with_token(
            &one_frame_sequence,
            format,
            &options,
            EncodePolicy::default(),
            Some(&token),
            &mut sequence_sink,
        );
        let mut sequence_mismatch_sink = Vec::new();
        let _ = encode_sequence_to_sink_with_token(
            &one_frame_sequence,
            format,
            &wrong_options,
            EncodePolicy::default(),
            None,
            &mut sequence_mismatch_sink,
        );
        let _ = encode_sequence_format_with_token(&one_frame_sequence, format, &options, None);
        let _ = encode_sequence_format_with_token(
            &one_frame_sequence,
            format,
            &options,
            Some(&crate::CancellationToken::new()),
        );
    }

    // A single retained frame can still carry sequence-only presentation
    // metadata.  Still-image sink formats must reject that rather than
    // silently discarding it.
    let mut metadata_sequence = DecodedSequence::from_image(rgb.clone());
    metadata_sequence.background = Some(crate::types::AnimationBackground::Rgba([0, 0, 0, 0]));
    let mut metadata_sink = Vec::new();
    let _ = encode_sequence_to_sink_with_token(
        &metadata_sequence,
        ImageFormat::Png,
        &EncodeOptions::for_format(ImageFormat::Png),
        EncodePolicy::default(),
        None,
        &mut metadata_sink,
    );
    let mut metadata_webp_sink = Vec::new();
    let _ = encode_sequence_to_sink_with_token(
        &metadata_sequence,
        ImageFormat::WebP,
        &EncodeOptions::for_format(ImageFormat::WebP),
        EncodePolicy::default(),
        None,
        &mut metadata_webp_sink,
    );

    // Still codecs are currently whole-buffer operations.  The coverage-only
    // hook makes the post-codec public-boundary cancellation checkpoint
    // deterministic; Pillow has no equivalent caller token to exercise this
    // Rust-only path.
    // Token-aware PNG work adds internal polls before the dispatcher performs
    // its final public-boundary check. Sweep a bounded count so the
    // dispatcher-level cancellation edge is reached as well as the earlier
    // codec-local edges; Pillow has no equivalent caller token.
    let probe = crate::CancellationToken::new();
    probe.cancel_after(usize::MAX);
    let _ = encode_format_with_token(
        &luma,
        ImageFormat::Png,
        &EncodeOptions::for_format(ImageFormat::Png),
        Some(&probe),
    );
    let calls = usize::MAX - probe.coverage_remaining_checks().unwrap_or(usize::MAX);
    for checks in 0..=calls {
        let post_codec_cancel = crate::CancellationToken::new();
        post_codec_cancel.cancel_after(checks);
        let _ = encode_format_with_token(
            &luma,
            ImageFormat::Png,
            &EncodeOptions::for_format(ImageFormat::Png),
            Some(&post_codec_cancel),
        );
    }
    let successful_post_codec_token = crate::CancellationToken::new();
    let _ = encode_format_with_token(
        &rgb,
        ImageFormat::Png,
        &EncodeOptions::for_format(ImageFormat::Png),
        Some(&successful_post_codec_token),
    );

    let two_frame_sequence = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![
            crate::types::DecodedFrame::rendered_canvas(
                luma.clone(),
                crate::types::FrameRect {
                    left: 0,
                    top: 0,
                    width: 1,
                    height: 1,
                },
                crate::types::FrameDuration::ZERO,
                crate::types::FrameDisposal::Unspecified,
                crate::types::FrameBlend::Unspecified,
            ),
            crate::types::DecodedFrame::rendered_canvas(
                luma.clone(),
                crate::types::FrameRect {
                    left: 0,
                    top: 0,
                    width: 1,
                    height: 1,
                },
                crate::types::FrameDuration::ZERO,
                crate::types::FrameDisposal::Unspecified,
                crate::types::FrameBlend::Unspecified,
            ),
        ],
        loop_count: None,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = encode_sequence_format(
        &two_frame_sequence,
        ImageFormat::Png,
        &EncodeOptions::for_format(ImageFormat::Png),
    );
    for format in [
        ImageFormat::Jpeg,
        ImageFormat::Png,
        ImageFormat::Bmp,
        ImageFormat::Ico,
    ] {
        let mut sink = Vec::new();
        let _ = encode_sequence_to_sink_with_token(
            &two_frame_sequence,
            format,
            &EncodeOptions::for_format(format),
            EncodePolicy::default(),
            None,
            &mut sink,
        );
    }
    let mut webp_sequence_sink = Vec::new();
    let _ = encode_sequence_to_sink_with_token(
        &two_frame_sequence,
        ImageFormat::WebP,
        &EncodeOptions::for_format(ImageFormat::WebP),
        EncodePolicy::default(),
        None,
        &mut webp_sequence_sink,
    );
    let mut webp_sequence_mismatch_sink = Vec::new();
    let _ = encode_sequence_to_sink_with_token(
        &two_frame_sequence,
        ImageFormat::WebP,
        &EncodeOptions::for_format(ImageFormat::Png),
        EncodePolicy::default(),
        None,
        &mut webp_sequence_mismatch_sink,
    );
    let mut empty_webp_sink = Vec::new();
    let _ = encode_sequence_to_sink_with_token(
        &invalid_sequence,
        ImageFormat::WebP,
        &EncodeOptions::for_format(ImageFormat::WebP),
        EncodePolicy::default(),
        None,
        &mut empty_webp_sink,
    );

    // These are Rust-only token-dispatch checks.  CancellationToken and the
    // caller-owned output contract are not represented by Pillow parity rows.
    #[cfg(feature = "gif")]
    let _ = encode_sequence_format_with_token(
        &two_frame_sequence,
        ImageFormat::Gif,
        &EncodeOptions::for_format(ImageFormat::Gif),
        Some(&crate::CancellationToken::new()),
    );
    #[cfg(feature = "avif")]
    let _ = encode_sequence_format_with_token(
        &two_frame_sequence,
        ImageFormat::Avif,
        &EncodeOptions::for_format(ImageFormat::Avif),
        Some(&crate::CancellationToken::new()),
    );
    #[cfg(feature = "avif")]
    {
        // The public dispatcher consumes the first poll before entering the
        // AVIF still encoder. Cancel after that poll to exercise the codec's
        // own pre-work checkpoint without adding a parity row.
        let avif_still_cancel = crate::CancellationToken::new();
        avif_still_cancel.cancel_after(1);
        let _ = encode_format_with_token(
            &luma,
            ImageFormat::Avif,
            &EncodeOptions::for_format(ImageFormat::Avif),
            Some(&avif_still_cancel),
        );
    }
    #[cfg(feature = "tiff")]
    let _ = encode_sequence_format_with_token(
        &two_frame_sequence,
        ImageFormat::Tiff,
        &EncodeOptions::for_format(ImageFormat::Tiff),
        Some(&crate::CancellationToken::new()),
    );
    #[cfg(feature = "webp")]
    let _ = encode_sequence_format_with_token(
        &two_frame_sequence,
        ImageFormat::WebP,
        &EncodeOptions::for_format(ImageFormat::WebP),
        Some(&crate::CancellationToken::new()),
    );

    let invalid_image = DecodedImage::new(1, 1, Vec::new(), crate::types::ColorType::Rgb8);
    let _ = validate_decoded_image(invalid_image);
    for format in [
        ImageFormat::Avif,
        ImageFormat::Gif,
        ImageFormat::Tiff,
        ImageFormat::WebP,
    ] {
        let mut sink = Vec::new();
        let invalid_image = DecodedImage::new(1, 1, Vec::new(), crate::types::ColorType::Rgb8);
        let _ = encode_format_to_sink_with_token(
            &invalid_image,
            format,
            &EncodeOptions::for_format(format),
            EncodePolicy::default(),
            None,
            &mut sink,
        );
    }

    // The manifest currently has no malformed AVIF container fixture. Keep the
    // dispatch-only error conversion covered without weakening AVIF parity rows.
    #[cfg(feature = "avif")]
    let _ = decode_sequence_format(
        b"not an AVIF container",
        ImageFormat::Avif,
        &mut SequenceDecodeBudget::default_for(ImageFormat::Avif),
    );

    compression::__coverage_exercise_private_branches();
    #[cfg(feature = "avif")]
    avif::__coverage_exercise_private_branches();
    #[cfg(feature = "bmp")]
    bmp::__coverage_exercise_private_branches();
    #[cfg(feature = "gif")]
    gif::__coverage_exercise_private_branches();
    #[cfg(feature = "ico")]
    ico::__coverage_exercise_private_branches();
    #[cfg(feature = "jpeg")]
    jpeg::__coverage_exercise_private_branches();
    #[cfg(feature = "png")]
    png::__coverage_exercise_private_branches();
    #[cfg(feature = "tiff")]
    tiff::__coverage_exercise_private_branches();
    #[cfg(feature = "webp")]
    webp::__coverage_exercise_private_branches();
}

#[cfg(all(coverage, feature = "avif"))]
pub(crate) fn __coverage_av1_entropy_reference_trace()
-> CodecResult<Vec<crate::Av1EntropyTraceState>> {
    avif::__coverage_entropy_reference_trace()
}

#[cfg(all(coverage, feature = "avif"))]
pub(crate) fn __coverage_av1_reconstruction(
    data: &[u8],
) -> CodecResult<Option<crate::Av1ReconstructionTrace>> {
    avif::__coverage_reconstruction(data)
}

#[cfg(all(coverage, feature = "avif"))]
pub(crate) fn __coverage_sweep_av1_first_leaf(data: &[u8]) {
    avif::__coverage_sweep_first_leaf(data);
}
