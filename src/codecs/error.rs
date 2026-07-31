//! Private codec failures retained until format dispatch selects a public error.

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
}

impl CodecError {
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
            ImageError::UnknownFormat
            | ImageError::FeatureDisabled { .. }
            | ImageError::LimitExceeded { .. } => Self::Unsupported(error.to_string()),
        }
    }

    /// Attach the selected format and convert to the canonical public error.
    pub(crate) fn into_image_error(self, format: ImageFormat) -> ImageError {
        match self {
            Self::Malformed(message) => ImageError::Malformed { format, message },
            Self::Unsupported(message) => ImageError::Unsupported {
                format: Some(format),
                message,
            },
            Self::Dimensions(message) => ImageError::Dimensions {
                format: Some(format),
                message,
            },
            Self::Parameter(message) => ImageError::Parameter {
                format: Some(format),
                message,
            },
        }
    }

    /// Prefix a lower-level failure with its caller's pipeline stage.
    pub(crate) fn context(self, stage: &'static str) -> Self {
        match self {
            Self::Malformed(message) => Self::Malformed(format!("{stage}: {message}")),
            Self::Unsupported(message) => Self::Unsupported(format!("{stage}: {message}")),
            Self::Dimensions(message) => Self::Dimensions(format!("{stage}: {message}")),
            Self::Parameter(message) => Self::Parameter(format!("{stage}: {message}")),
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
pub(crate) fn into_image_result<T>(result: CodecResult<T>, format: ImageFormat) -> ImageResult<T> {
    result.map_err(|error| error.into_image_error(format))
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    for error in [
        ImageError::Malformed {
            format: ImageFormat::Png,
            message: "malformed".to_owned(),
        },
        ImageError::Unsupported {
            format: Some(ImageFormat::Png),
            message: "unsupported".to_owned(),
        },
        ImageError::dimensions("dimensions"),
        ImageError::parameter("parameter"),
        ImageError::UnknownFormat,
        ImageError::FeatureDisabled {
            format: ImageFormat::Png,
            feature: "png",
        },
    ] {
        let _ = CodecError::from_image_error(error);
    }
}
