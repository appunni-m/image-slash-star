//! Private codec failures retained until format dispatch selects a public error.

use crate::types::ImageErrorStage;
use crate::types::{ImageError, ImageFormat, ImageResult};

/// Operational failure produced below the public format dispatcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CodecError {
    /// Encoded input violates a container or bitstream contract.
    Malformed(String),
    /// Valid input, mode, or options are outside the codec's supported contract.
    Unsupported(String),
    /// Image dimensions are invalid, inconsistent, or unrepresentable.
    Dimensions(String),
    /// A caller-supplied image property or encoding option is invalid.
    Parameter(String),
    /// A caller-configured resource maximum was exceeded; the structured
    /// [`ImageError::LimitExceeded`] value is retained verbatim.
    LimitExceeded(ImageError),
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
        feature = "avif"
    ))]
    pub(crate) fn from_image_error(error: ImageError) -> Self {
        match error {
            ImageError::Malformed { message, .. } => Self::Malformed(message),
            ImageError::Unsupported { message, .. } => Self::Unsupported(message),
            ImageError::Dimensions { message, .. } => Self::Dimensions(message),
            ImageError::Parameter { message, .. } => Self::Parameter(message),
            ImageError::UnknownFormat | ImageError::FeatureDisabled { .. } => {
                Self::Unsupported(error.to_string())
            }
            ImageError::LimitExceeded { .. } => Self::LimitExceeded(error),
        }
    }

    /// Attach the selected format and convert to the canonical public error.
    pub(crate) fn into_image_error(
        self,
        format: ImageFormat,
        stage: ImageErrorStage,
    ) -> ImageError {
        match self {
            Self::Malformed(message) => ImageError::Malformed {
                format,
                message,
                stage: Some(stage),
                offset: None,
                identity: None,
            },
            Self::Unsupported(message) => ImageError::Unsupported {
                format: Some(format),
                message,
                stage: Some(stage),
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
            Self::At {
                error,
                offset,
                identity,
            } => {
                let mut converted = error.into_image_error(format, stage);
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
                    ImageError::UnknownFormat
                    | ImageError::FeatureDisabled { .. }
                    | ImageError::LimitExceeded { .. } => {}
                }
                converted
            }
        }
    }

    /// Prefix a lower-level failure with its caller's pipeline stage.
    pub(crate) fn context(self, stage: &'static str) -> Self {
        match self {
            Self::Malformed(message) => Self::Malformed(format!("{stage}: {message}")),
            Self::Unsupported(message) => Self::Unsupported(format!("{stage}: {message}")),
            Self::Dimensions(message) => Self::Dimensions(format!("{stage}: {message}")),
            Self::Parameter(message) => Self::Parameter(format!("{stage}: {message}")),
            Self::LimitExceeded(error) => Self::LimitExceeded(error),
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

/// Result retained by private codec implementations.
pub(crate) type CodecResult<T> = Result<T, CodecError>;

/// Convert an optional lookup into an explicitly classified codec failure.
#[cfg(any(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico"
))]
pub(crate) trait OptionCodecExt<T> {
    /// Require a value whose absence means malformed encoded input.
    fn malformed(self, message: &'static str) -> CodecResult<T>;

    /// Require a value whose absence means dimensions are unrepresentable.
    #[cfg(any(feature = "png", feature = "tiff"))]
    fn dimensions(self, message: &'static str) -> CodecResult<T>;
}

#[cfg(any(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
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

    #[cfg(any(feature = "png", feature = "tiff"))]
    fn dimensions(self, message: &'static str) -> CodecResult<T> {
        match self {
            Some(value) => Ok(value),
            None => Err(CodecError::Dimensions(message.to_owned())),
        }
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

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    use crate::{CodecOperation, ResourceLimit};

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
    ] {
        let _ = CodecError::from_image_error(error);
    }
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
    let _ = limit.context("decode sequence");
    let at = CodecError::Malformed("at".to_owned()).at(12, "png_chunk");
    let _ = at
        .clone()
        .into_image_error(ImageFormat::Png, ImageErrorStage::StillDecode);
    let _ = at.context("decode");
    let _ = CodecError::Unsupported("at".to_owned())
        .at(12, "webp_chunk")
        .into_image_error(ImageFormat::WebP, ImageErrorStage::StillDecode);
    let _ = CodecError::Dimensions("at".to_owned())
        .at(12, "tiff_ifd")
        .into_image_error(ImageFormat::Tiff, ImageErrorStage::StillDecode);
    let _ = CodecError::Parameter("at".to_owned())
        .at(12, "png_chunk")
        .into_image_error(ImageFormat::Png, ImageErrorStage::StillEncode);
}
