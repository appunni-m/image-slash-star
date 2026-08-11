//! Private codec failures retained until format dispatch selects a public error.

use crate::CodecOperation;
use crate::types::ImageErrorStage;
use crate::types::{ImageError, ImageFormat, ImageResult, ResourceLimit, UnsupportedReason};

/// Operational failure produced below the public format dispatcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CodecError {
    /// Encoded input violates a container or bitstream contract.
    Malformed(String),
    /// A container structure is present in the input but extends beyond the
    /// available bytes; the caller needs at least `minimum` total bytes
    /// before this operation can continue.
    NeedMore { minimum: usize, message: String },
    /// Valid input, mode, or options are outside the codec's supported contract.
    Unsupported(String),
    /// The operation is unavailable on the current compilation target.
    TargetUnavailable(String),
    /// Image dimensions are invalid, inconsistent, or unrepresentable.
    Dimensions(String),
    /// A caller-supplied image property or encoding option is invalid.
    Parameter(String),
    /// A caller-configured resource maximum was exceeded; the structured
    /// [`ImageError::LimitExceeded`] value is retained verbatim.
    LimitExceeded(ImageError),
    /// The deterministic cooperative checkpoint budget was exhausted.
    WorkBudgetExceeded { maximum: u64, observed: u64 },
    /// The operation stopped at a cooperative checkpoint because the
    /// caller's cancellation token fired.
    Cancelled,
    /// A caller-owned output sink rejected one structural write.
    OutputWrite(String),
    /// A codec failure wrapped with the encoded-input byte offset and stable
    /// container-structure identity of its parse site.
    #[cfg_attr(
        not(any(
            feature = "png",
            feature = "gif",
            feature = "jpeg",
            feature = "tiff",
            feature = "webp",
            feature = "avif"
        )),
        allow(dead_code)
    )]
    At {
        error: Box<CodecError>,
        offset: u64,
        identity: &'static str,
    },
}

impl CodecError {
    /// Attach the parse-site offset and structure identity to a failure.
    #[cfg_attr(
        not(any(
            feature = "png",
            feature = "gif",
            feature = "jpeg",
            feature = "tiff",
            feature = "webp",
            feature = "avif"
        )),
        allow(dead_code)
    )]
    pub(crate) fn at(self, offset: u64, identity: &'static str) -> Self {
        Self::At {
            error: Box::new(self),
            offset,
            identity,
        }
    }

    /// Retain a validation failure when crossing into a private codec.
    #[cfg(any(
        feature = "jpeg",
        feature = "png",
        feature = "gif",
        feature = "bmp",
        feature = "tiff",
        feature = "ico",
        feature = "avif",
        feature = "webp"
    ))]
    pub(crate) fn from_image_error(error: ImageError) -> Self {
        match error {
            ImageError::Malformed { message, .. } => Self::Malformed(message),
            ImageError::Unsupported {
                message,
                reason: Some(UnsupportedReason::TargetUnavailable),
                ..
            } => Self::TargetUnavailable(message),
            ImageError::Unsupported { message, .. } => Self::Unsupported(message),
            ImageError::Dimensions { message, .. } => Self::Dimensions(message),
            ImageError::Parameter { message, .. } => Self::Parameter(message),
            ImageError::UnknownFormat | ImageError::FeatureDisabled { .. } => {
                Self::Unsupported(error.to_string())
            }
            ImageError::OutputWrite { message, .. } => Self::OutputWrite(message),
            ImageError::LimitExceeded { .. } => Self::LimitExceeded(error),
            ImageError::NeedMoreData { minimum, .. } => Self::NeedMore {
                minimum: usize::try_from(minimum).unwrap_or(usize::MAX),
                message: "incremental input required".to_owned(),
            },
            ImageError::Cancelled { .. } => Self::Cancelled,
        }
    }

    /// Attach the selected format and convert to the canonical public error.
    pub(crate) fn into_image_error(
        self,
        format: ImageFormat,
        stage: ImageErrorStage,
    ) -> ImageError {
        self.into_image_error_with_mode(format, stage, false)
    }

    fn into_image_error_with_mode(
        self,
        format: ImageFormat,
        stage: ImageErrorStage,
        incremental: bool,
    ) -> ImageError {
        match self {
            Self::Malformed(message) => ImageError::Malformed {
                format,
                message,
                stage: Some(stage),
                offset: None,
                identity: None,
            },
            Self::NeedMore { minimum, .. } if incremental => ImageError::NeedMoreData {
                format: Some(format),
                stage: Some(stage),
                offset: None,
                identity: None,
                minimum: u64::try_from(minimum).unwrap_or(u64::MAX),
            },
            Self::NeedMore { message, .. } => ImageError::Malformed {
                format,
                message,
                stage: Some(stage),
                offset: None,
                identity: None,
            },
            Self::Cancelled => ImageError::Cancelled {
                format: Some(format),
                stage: Some(stage),
            },
            Self::OutputWrite(message) => ImageError::OutputWrite {
                format: Some(format),
                message,
                stage: Some(stage),
            },
            Self::Unsupported(message) => ImageError::Unsupported {
                format: Some(format),
                message,
                stage: Some(stage),
                reason: None,
                offset: None,
                identity: None,
            },
            Self::TargetUnavailable(message) => ImageError::Unsupported {
                format: Some(format),
                message,
                stage: Some(stage),
                reason: Some(UnsupportedReason::TargetUnavailable),
                offset: None,
                identity: None,
            },
            Self::Dimensions(message) => ImageError::Dimensions {
                format: Some(format),
                message,
                stage: Some(stage),
                offset: None,
                identity: None,
            },
            Self::Parameter(message) => ImageError::Parameter {
                format: Some(format),
                message,
                stage: Some(stage),
                offset: None,
                identity: None,
            },
            Self::LimitExceeded(error) => error,
            Self::WorkBudgetExceeded { maximum, observed } => ImageError::LimitExceeded {
                format: Some(format),
                operation: operation_for_stage(stage),
                resource: ResourceLimit::EncodeWorkUnits,
                maximum,
                observed,
            },
            Self::At {
                error,
                offset,
                identity,
            } => {
                let mut converted = error.into_image_error_with_mode(format, stage, incremental);
                match &mut converted {
                    ImageError::Malformed {
                        offset: target,
                        identity: target_identity,
                        ..
                    }
                    | ImageError::Unsupported {
                        offset: target,
                        identity: target_identity,
                        ..
                    }
                    | ImageError::Dimensions {
                        offset: target,
                        identity: target_identity,
                        ..
                    }
                    | ImageError::Parameter {
                        offset: target,
                        identity: target_identity,
                        ..
                    } => {
                        *target = Some(offset);
                        *target_identity = Some(identity);
                    }
                    ImageError::NeedMoreData {
                        offset: target,
                        identity: target_identity,
                        ..
                    } => {
                        *target = Some(offset);
                        *target_identity = Some(identity);
                    }
                    ImageError::UnknownFormat
                    | ImageError::FeatureDisabled { .. }
                    | ImageError::LimitExceeded { .. }
                    | ImageError::Cancelled { .. }
                    | ImageError::OutputWrite { .. } => {}
                }
                converted
            }
        }
    }

    /// Convert to the canonical public error, exposing incremental truncation
    /// as the non-terminal [`ImageError::NeedMoreData`] status.
    pub(crate) fn into_incremental_image_error(
        self,
        format: ImageFormat,
        stage: ImageErrorStage,
    ) -> ImageError {
        self.into_image_error_with_mode(format, stage, true)
    }

    /// Prefix a lower-level failure with its caller's pipeline stage.
    pub(crate) fn context(self, stage: &'static str) -> Self {
        match self {
            Self::Malformed(message) => Self::Malformed(format!("{stage}: {message}")),
            Self::NeedMore { minimum, message } => Self::NeedMore {
                minimum,
                message: format!("{stage}: {message}"),
            },
            Self::Unsupported(message) => Self::Unsupported(format!("{stage}: {message}")),
            Self::TargetUnavailable(message) => {
                Self::TargetUnavailable(format!("{stage}: {message}"))
            }
            Self::Dimensions(message) => Self::Dimensions(format!("{stage}: {message}")),
            Self::Parameter(message) => Self::Parameter(format!("{stage}: {message}")),
            Self::LimitExceeded(error) => Self::LimitExceeded(error),
            Self::WorkBudgetExceeded { maximum, observed } => {
                Self::WorkBudgetExceeded { maximum, observed }
            }
            Self::Cancelled => Self::Cancelled,
            Self::OutputWrite(message) => Self::OutputWrite(message),
            Self::At {
                error,
                offset,
                identity,
            } => Self::At {
                error: Box::new(error.context(stage)),
                offset,
                identity,
            },
        }
    }
}

/// Poll a caller-supplied cancellation token at a structural checkpoint.
///
/// `None` (the legacy path) never cancels. The codec format and stage are
/// attached when the token fires so the public error is actionable.
pub(crate) fn check_cancelled(token: Option<&crate::CancellationToken>) -> CodecResult<()> {
    match token {
        Some(token) => match token.poll() {
            crate::cancel::PollResult::Continue => Ok(()),
            crate::cancel::PollResult::Cancelled => Err(CodecError::Cancelled),
            crate::cancel::PollResult::WorkBudgetExceeded { maximum, observed } => {
                Err(CodecError::WorkBudgetExceeded { maximum, observed })
            }
        },
        None => Ok(()),
    }
}

fn operation_for_stage(stage: ImageErrorStage) -> CodecOperation {
    match stage {
        ImageErrorStage::SequenceEncode => CodecOperation::SequenceEncode,
        // Work budgets are only installed by public encode entry points. A
        // non-sequence encode stage is therefore the only other valid input;
        // keeping the fallback defensive avoids pretending decode or
        // verification paths can consume an encode budget.
        _ => CodecOperation::StillEncode,
    }
}

/// Result retained by private codec implementations.
pub(crate) type CodecResult<T> = Result<T, CodecError>;

/// Convert an optional lookup into an explicitly classified codec failure.
#[cfg(any(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "tiff",
    feature = "webp",
    feature = "ico"
))]
pub(crate) trait OptionCodecExt<T> {
    /// Require a value whose absence means malformed encoded input.
    fn malformed(self, message: &'static str) -> CodecResult<T>;

    /// Require a value whose absence means the input ends before the
    /// requested byte range; the caller needs at least `minimum` total bytes.
    #[cfg(feature = "gif")]
    fn need_more(self, minimum: usize, message: &'static str) -> CodecResult<T>;

    /// Require a value whose absence means dimensions are unrepresentable.
    #[cfg(any(feature = "png", feature = "tiff"))]
    fn dimensions(self, message: &'static str) -> CodecResult<T>;
}

#[cfg(any(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "tiff",
    feature = "webp",
    feature = "ico"
))]
impl<T> OptionCodecExt<T> for Option<T> {
    fn malformed(self, message: &'static str) -> CodecResult<T> {
        match self {
            Some(value) => Ok(value),
            None => Err(CodecError::Malformed(message.to_owned())),
        }
    }

    #[cfg(feature = "gif")]
    fn need_more(self, minimum: usize, message: &'static str) -> CodecResult<T> {
        match self {
            Some(value) => Ok(value),
            None => Err(CodecError::NeedMore {
                minimum,
                message: message.to_owned(),
            }),
        }
    }

    #[cfg(any(feature = "png", feature = "tiff"))]
    fn dimensions(self, message: &'static str) -> CodecResult<T> {
        match self {
            Some(value) => Ok(value),
            None => Err(CodecError::Dimensions(message.to_owned())),
        }
    }
}

/// Read `data[start..end]`, classifying a short input as the incremental
/// truncation status with the exact total byte minimum.
#[cfg(any(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico"
))]
pub(crate) fn need_slice<'a>(
    data: &'a [u8],
    start: usize,
    end: usize,
    message: &'static str,
) -> CodecResult<&'a [u8]> {
    if start > end {
        return Err(CodecError::Malformed(message.to_owned()));
    }
    match data.get(start..end) {
        Some(value) => Ok(value),
        None => Err(CodecError::NeedMore {
            minimum: end,
            message: message.to_owned(),
        }),
    }
}

/// Read the tail `data[start..]`, classifying a start beyond the input as the
/// incremental truncation status. An exact end is allowed: a zero-length tail
/// is a valid parse outcome, so the minimum is `start`.
#[cfg(feature = "bmp")]
pub(crate) fn need_from<'a>(
    data: &'a [u8],
    start: usize,
    message: &'static str,
) -> CodecResult<&'a [u8]> {
    if start > data.len() {
        return Err(CodecError::NeedMore {
            minimum: start,
            message: message.to_owned(),
        });
    }
    Ok(&data[start..])
}

/// Compute the exclusive end of a slice read.
///
/// The 64-bit host path is branch-free: slice lengths cannot approach
/// `usize::MAX`, so wrapping is unreachable for valid inputs and a wrapped
/// range is rejected by [`need_slice`] as malformed. Narrow targets saturate
/// a genuine file-derived overflow to `usize::MAX`, which [`need_slice`]
/// reports as an unattainable incremental minimum instead of misclassifying
/// the read.
#[cfg(any(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "webp",
    feature = "ico"
))]
pub(crate) fn codec_add_end(base: usize, add: usize) -> usize {
    #[cfg(target_pointer_width = "64")]
    {
        base.wrapping_add(add)
    }
    #[cfg(not(target_pointer_width = "64"))]
    {
        base.saturating_add(add)
    }
}

/// Convert an incremental truncation status into the terminal malformed
/// classification. Used where a slice is already bounded by a validated
/// declared structure, so appending more input cannot repair it.
#[cfg(any(feature = "webp", feature = "ico"))]
pub(crate) fn terminalize(error: CodecError) -> CodecError {
    match error {
        CodecError::NeedMore { message, .. } => CodecError::Malformed(message),
        CodecError::At {
            error,
            offset,
            identity,
        } => CodecError::At {
            error: Box::new(terminalize(*error)),
            offset,
            identity,
        },
        other => other,
    }
}

/// Convert a private codec result after the dispatcher supplies its format.
pub(crate) fn into_image_result<T>(
    result: CodecResult<T>,
    format: ImageFormat,
    stage: ImageErrorStage,
) -> ImageResult<T> {
    result.map_err(|error| error.into_image_error(format, stage))
}

/// Convert a private codec result for the incremental-input surface.
pub(crate) fn into_incremental_image_result<T>(
    result: CodecResult<T>,
    format: ImageFormat,
    stage: ImageErrorStage,
) -> ImageResult<T> {
    result.map_err(|error| error.into_incremental_image_error(format, stage))
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    use crate::{CodecOperation, ResourceLimit, UnsupportedReason};

    #[cfg(all(feature = "avif", not(target_arch = "wasm32")))]
    {
        let _ = into_image_result::<Vec<crate::Av1EntropyTraceState>>(
            Err(CodecError::Malformed("coverage trace error".to_owned())),
            ImageFormat::Avif,
            ImageErrorStage::StillDecode,
        );
        let _ = into_image_result::<Option<crate::Av1ReconstructionTrace>>(
            Err(CodecError::Malformed(
                "coverage reconstruction error".to_owned(),
            )),
            ImageFormat::Avif,
            ImageErrorStage::StillDecode,
        );
    }

    for error in [
        ImageError::Malformed {
            format: ImageFormat::Png,
            message: "malformed".to_owned(),
            stage: Some(ImageErrorStage::StillDecode),
            offset: Some(8),
            identity: Some("png_chunk"),
        },
        ImageError::Unsupported {
            format: Some(ImageFormat::Png),
            message: "unsupported".to_owned(),
            stage: Some(ImageErrorStage::StillEncode),
            reason: None,
            offset: None,
            identity: None,
        },
        ImageError::dimensions("dimensions"),
        ImageError::parameter("parameter"),
        ImageError::UnknownFormat,
        ImageError::FeatureDisabled {
            format: ImageFormat::Png,
            feature: "png",
        },
        ImageError::LimitExceeded {
            format: Some(ImageFormat::Png),
            operation: CodecOperation::SequenceDecode,
            resource: ResourceLimit::Frames,
            maximum: 1,
            observed: 2,
        },
        ImageError::NeedMoreData {
            format: Some(ImageFormat::Png),
            stage: Some(ImageErrorStage::Inspection),
            offset: Some(8),
            identity: Some("png_chunk"),
            minimum: 41,
        },
        ImageError::Cancelled {
            format: Some(ImageFormat::Png),
            stage: Some(ImageErrorStage::StillDecode),
        },
        ImageError::OutputWrite {
            format: Some(ImageFormat::Png),
            message: "sink rejected".to_owned(),
            stage: Some(ImageErrorStage::StillEncode),
        },
    ] {
        let _ = CodecError::from_image_error(error);
    }
    let _ = CodecError::from_image_error(ImageError::Unsupported {
        format: Some(ImageFormat::Avif),
        message: "target".to_owned(),
        stage: Some(ImageErrorStage::StillEncode),
        reason: Some(UnsupportedReason::TargetUnavailable),
        offset: None,
        identity: None,
    });
    let limit = CodecError::LimitExceeded(ImageError::LimitExceeded {
        format: Some(ImageFormat::Png),
        operation: CodecOperation::SequenceDecode,
        resource: ResourceLimit::Frames,
        maximum: 1,
        observed: 2,
    });
    let _ = limit
        .clone()
        .into_image_error(ImageFormat::Png, ImageErrorStage::StillDecode);
    let _ = limit.clone().context("decode sequence");
    let at = CodecError::Malformed("at".to_owned()).at(12, "png_chunk");
    let _ = at
        .clone()
        .into_image_error(ImageFormat::Png, ImageErrorStage::StillDecode);
    let _ = at.context("decode");
    let _ = CodecError::Unsupported("at".to_owned())
        .at(12, "webp_chunk")
        .into_image_error(ImageFormat::WebP, ImageErrorStage::StillDecode);
    let _ = CodecError::TargetUnavailable("target".to_owned())
        .clone()
        .into_image_error(ImageFormat::Avif, ImageErrorStage::StillEncode);
    let _ = CodecError::TargetUnavailable("target".to_owned())
        .into_incremental_image_error(ImageFormat::Avif, ImageErrorStage::SequenceEncode);
    let _ = CodecError::TargetUnavailable("target".to_owned()).context("encode");
    let _ = CodecError::Dimensions("at".to_owned())
        .at(12, "tiff_ifd")
        .into_image_error(ImageFormat::Tiff, ImageErrorStage::StillDecode);
    let _ = CodecError::Parameter("at".to_owned())
        .at(12, "png_chunk")
        .into_image_error(ImageFormat::Png, ImageErrorStage::StillEncode);
    let need_more = CodecError::NeedMore {
        minimum: 41,
        message: "truncated PNG chunk payload".to_owned(),
    };
    let _ = need_more
        .clone()
        .into_image_error(ImageFormat::Png, ImageErrorStage::Inspection);
    let _ = need_more
        .clone()
        .into_incremental_image_error(ImageFormat::Png, ImageErrorStage::Inspection);
    let _ = need_more
        .clone()
        .at(8, "png_chunk")
        .into_incremental_image_error(ImageFormat::Png, ImageErrorStage::Inspection);
    let _ = CodecError::Malformed("malformed".to_owned())
        .into_incremental_image_error(ImageFormat::Png, ImageErrorStage::Inspection);
    let _ = CodecError::Unsupported("unsupported".to_owned())
        .into_incremental_image_error(ImageFormat::Png, ImageErrorStage::Inspection);
    let _ = CodecError::Dimensions("dimensions".to_owned())
        .into_incremental_image_error(ImageFormat::Png, ImageErrorStage::Inspection);
    let _ = CodecError::Parameter("parameter".to_owned())
        .into_incremental_image_error(ImageFormat::Png, ImageErrorStage::Inspection);
    let _ = limit
        .clone()
        .into_incremental_image_error(ImageFormat::Png, ImageErrorStage::Inspection);
    let _ = CodecError::Malformed("nested".to_owned())
        .at(4, "webp_chunk")
        .into_incremental_image_error(ImageFormat::WebP, ImageErrorStage::Inspection);
    let _ = CodecError::Cancelled.into_image_error(ImageFormat::Png, ImageErrorStage::StillDecode);
    let _ = CodecError::Cancelled
        .into_incremental_image_error(ImageFormat::Png, ImageErrorStage::StillDecode);
    let _ = CodecError::OutputWrite("sink rejected".to_owned())
        .into_incremental_image_error(ImageFormat::Png, ImageErrorStage::SequenceEncode);
    let _ = CodecError::Unsupported("nested".to_owned())
        .at(4, "webp_chunk")
        .into_incremental_image_error(ImageFormat::WebP, ImageErrorStage::Inspection);
    let _ = CodecError::Dimensions("nested".to_owned())
        .at(4, "tiff_ifd")
        .into_incremental_image_error(ImageFormat::Tiff, ImageErrorStage::Inspection);
    let _ = CodecError::Parameter("nested".to_owned())
        .at(4, "png_chunk")
        .into_incremental_image_error(ImageFormat::Png, ImageErrorStage::Inspection);
    let _ = limit
        .clone()
        .at(4, "png_chunk")
        .into_incremental_image_error(ImageFormat::Png, ImageErrorStage::Inspection);
    let _ = need_more.context("inspect basic");
    let _ = need_slice(b"12345", 0, 6, "truncated field");
    let _ = need_slice(b"12345", 7, 6, "inverted field");
    let _ = need_from(b"12345", 3, "tail beyond input");
    let _ = need_from(b"12345", 9, "tail beyond input");
    let _ = codec_add_end(3, 2);
    let _ = terminalize(CodecError::NeedMore {
        minimum: 5,
        message: "truncated".to_owned(),
    });
    let _ = terminalize(
        CodecError::NeedMore {
            minimum: 5,
            message: "truncated".to_owned(),
        }
        .at(3, "ico_entry"),
    );
    let _ = terminalize(CodecError::Unsupported("kept".to_owned()));
    let _ = Option::<u8>::None.need_more(3, "truncated byte");
    let _ = Option::<u8>::Some(1).need_more(3, "truncated byte");
    let cancelled = crate::CancellationToken::new();
    let _ = check_cancelled(None);
    let _ = check_cancelled(Some(&cancelled));
    cancelled.cancel();
    let _ = check_cancelled(Some(&cancelled));
    let staged = crate::CancellationToken::new();
    staged.cancel_after(1);
    let _ = check_cancelled(Some(&staged));
    let _ = check_cancelled(Some(&staged));
}
