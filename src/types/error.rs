//! Error types for decoded buffers and codec operations.

use super::ImageFormat;
use crate::CodecOperation;
use std::fmt;

/// Caller-controlled resource whose configured maximum was exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResourceLimit {
    /// Complete encoded input length in bytes.
    EncodedBytes,
    /// Inspected image canvas width in pixels.
    Width,
    /// Inspected image canvas height in pixels.
    Height,
    /// Inspected image canvas area in pixels.
    Pixels,
    /// Decoded transfer-byte length of the inspected primary image.
    PrimaryDecodedBytes,
    /// Number of frames or pages the policy permits to be inspected or
    /// materialized.
    Frames,
}

/// Stable category of an [`ImageError`].
///
/// Error messages provide useful diagnostics but may become more specific over
/// time. Match this category, and optionally [`ImageError::format`], when
/// implementing recovery policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ImageErrorKind {
    /// No encoded-image signature was recognized.
    UnknownFormat,
    /// The detected format's Cargo feature is disabled.
    FeatureDisabled,
    /// Encoded bytes violate the selected format.
    Malformed,
    /// The format cannot represent the requested input or operation.
    Unsupported,
    /// Dimensions, bounds, or represented byte lengths are invalid.
    Dimensions,
    /// An option or represented image property is invalid.
    Parameter,
    /// A caller-configured resource maximum was exceeded.
    LimitExceeded,
}

/// Failure returned by image validation, format detection, and codec operations.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImageError {
    /// The input does not have a recognized encoded-image signature.
    UnknownFormat,
    /// The input format is known, but its Cargo feature is not enabled.
    FeatureDisabled {
        /// Format requiring the disabled codec.
        format: ImageFormat,
        /// Cargo feature that enables the codec.
        feature: &'static str,
    },
    /// Encoded bytes were rejected by the selected decoder.
    Malformed {
        /// Detected or explicitly selected format.
        format: ImageFormat,
        /// High-level diagnostic suitable for logs.
        message: String,
    },
    /// Valid input, options, or output cannot be represented by the selected codec.
    Unsupported {
        /// Selected format when the failure belongs to a codec.
        format: Option<ImageFormat>,
        /// High-level diagnostic suitable for logs.
        message: String,
    },
    /// The operation dimensions are out of bounds or mismatched.
    Dimensions {
        /// Selected format when the failure belongs to a codec.
        format: Option<ImageFormat>,
        /// High-level diagnostic suitable for logs.
        message: String,
    },
    /// A parameter error.
    Parameter {
        /// Selected format when the failure belongs to a codec.
        format: Option<ImageFormat>,
        /// High-level diagnostic suitable for logs.
        message: String,
    },
    /// A caller-configured resource maximum was exceeded.
    LimitExceeded {
        /// Detected format, or `None` when rejection precedes detection.
        format: Option<ImageFormat>,
        /// Public operation rejected by the limit.
        operation: CodecOperation,
        /// Resource whose maximum was exceeded.
        resource: ResourceLimit,
        /// Configured inclusive maximum.
        maximum: u64,
        /// Observed value that exceeded the maximum.
        observed: u64,
    },
}

impl ImageError {
    /// Return the stable category used for programmatic recovery.
    #[must_use]
    pub const fn kind(&self) -> ImageErrorKind {
        match self {
            Self::UnknownFormat => ImageErrorKind::UnknownFormat,
            Self::FeatureDisabled { .. } => ImageErrorKind::FeatureDisabled,
            Self::Malformed { .. } => ImageErrorKind::Malformed,
            Self::Unsupported { .. } => ImageErrorKind::Unsupported,
            Self::Dimensions { .. } => ImageErrorKind::Dimensions,
            Self::Parameter { .. } => ImageErrorKind::Parameter,
            Self::LimitExceeded { .. } => ImageErrorKind::LimitExceeded,
        }
    }

    /// Return the encoded or output format associated with the failure.
    ///
    /// This is `None` for unknown-format failures and validation errors that
    /// occur before a codec is selected.
    #[must_use]
    pub const fn format(&self) -> Option<ImageFormat> {
        match self {
            Self::UnknownFormat => None,
            Self::FeatureDisabled { format, .. } | Self::Malformed { format, .. } => Some(*format),
            Self::Unsupported { format, .. }
            | Self::Dimensions { format, .. }
            | Self::Parameter { format, .. } => *format,
            Self::LimitExceeded { format, .. } => *format,
        }
    }

    /// Return the stable high-level diagnostic retained by the failure.
    ///
    /// The error kind and format are the compatibility surface for recovery;
    /// this message is intended for logs and troubleshooting.
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::UnknownFormat | Self::FeatureDisabled { .. } | Self::LimitExceeded { .. } => None,
            Self::Malformed { message, .. }
            | Self::Unsupported { message, .. }
            | Self::Dimensions { message, .. }
            | Self::Parameter { message, .. } => Some(message),
        }
    }

    pub(crate) fn dimensions(message: impl Into<String>) -> Self {
        Self::Dimensions {
            format: None,
            message: message.into(),
        }
    }

    pub(crate) fn parameter(message: impl Into<String>) -> Self {
        Self::Parameter {
            format: None,
            message: message.into(),
        }
    }

    pub(crate) fn with_format(self, selected: ImageFormat) -> Self {
        match self {
            Self::Dimensions {
                format: None,
                message,
            } => Self::Dimensions {
                format: Some(selected),
                message,
            },
            Self::Parameter {
                format: None,
                message,
            } => Self::Parameter {
                format: Some(selected),
                message,
            },
            error => error,
        }
    }
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::UnknownFormat => write!(f, "unknown image format"),
            ImageError::FeatureDisabled { format, feature } => {
                write!(f, "codec feature `{feature}` is disabled for {format:?}")
            }
            ImageError::Malformed { format, message } => {
                write!(f, "malformed {format:?} image data: {message}")
            }
            ImageError::Unsupported { format, message } => match format {
                Some(format) => write!(f, "unsupported {format:?}: {message}"),
                None => write!(f, "unsupported: {message}"),
            },
            ImageError::Dimensions { format, message } => match format {
                Some(format) => write!(f, "invalid {format:?} dimensions: {message}"),
                None => write!(f, "invalid image dimensions: {message}"),
            },
            ImageError::Parameter { format, message } => match format {
                Some(format) => write!(f, "invalid {format:?} parameter: {message}"),
                None => write!(f, "invalid image parameter: {message}"),
            },
            ImageError::LimitExceeded {
                format,
                operation,
                resource,
                maximum,
                observed,
            } => match format {
                Some(format) => write!(
                    f,
                    "{format:?} {operation:?} exceeded {resource:?} limit: observed {observed}, maximum {maximum}"
                ),
                None => write!(
                    f,
                    "{operation:?} exceeded {resource:?} limit: observed {observed}, maximum {maximum}"
                ),
            },
        }
    }
}

impl std::error::Error for ImageError {}

/// A specialized result type for codec operations.
pub type ImageResult<T> = Result<T, ImageError>;
