//! Error types for decoded buffers and codec operations.

use super::ImageFormat;
use crate::CodecOperation;
use std::fmt;

/// Public operation that produced a structured error, when known.
///
/// Codec-dispatch failures attach the operation that was executing when the
/// failure escaped; caller-built validation failures and option-construction
/// errors may not belong to one operation and remain `None`. This is a stable
/// recovery field, unlike the diagnostic prose in [`ImageError::message`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ImageErrorStage {
    /// Header inspection without pixel materialization.
    Inspection,
    /// Still or first-image decode.
    StillDecode,
    /// Still-image encode.
    StillEncode,
    /// Multi-image sequence decode.
    SequenceDecode,
    /// Multi-image sequence encode.
    SequenceEncode,
    /// Format-specific verification.
    Verification,
}

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
    /// Decoded transfer-byte length of one later frame or page.
    FrameDecodedBytes,
    /// Cumulative decoded transfer-byte length of every retained frame or
    /// page in a sequence.
    SequenceDecodedBytes,
    /// Encoded bytes of container structures that are not primary pixel
    /// payload data.
    MetadataBytes,
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
    /// The operation was cooperatively cancelled by a caller token.
    Cancelled,
    /// More encoded input is required before the operation can continue.
    ///
    /// This is the non-terminal incremental-input status: callers should
    /// provide at least [`ImageError::minimum_input`] total bytes and retry.
    /// It is never returned by the complete-slice APIs; those retain the
    /// legacy terminal [`ImageError::Malformed`] classification.
    NeedMoreData,
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
        /// Public operation that produced the failure.
        stage: Option<ImageErrorStage>,
        /// Byte offset in the encoded input where the failing container
        /// structure begins, when the codec parser can name it.
        offset: Option<u64>,
        /// Stable container-structure identity (for example `png_chunk`,
        /// `jpeg_marker`, or `tiff_ifd`), when the codec parser can name it.
        identity: Option<&'static str>,
    },
    /// Valid input, options, or output cannot be represented by the selected codec.
    Unsupported {
        /// Selected format when the failure belongs to a codec.
        format: Option<ImageFormat>,
        /// High-level diagnostic suitable for logs.
        message: String,
        /// Public operation that produced the failure.
        stage: Option<ImageErrorStage>,
        /// Byte offset in the encoded input where the failing container
        /// structure begins, when the codec parser can name it.
        offset: Option<u64>,
        /// Stable container-structure identity, when the codec parser can
        /// name it.
        identity: Option<&'static str>,
    },
    /// The operation dimensions are out of bounds or mismatched.
    Dimensions {
        /// Selected format when the failure belongs to a codec.
        format: Option<ImageFormat>,
        /// High-level diagnostic suitable for logs.
        message: String,
        /// Public operation that produced the failure.
        stage: Option<ImageErrorStage>,
        /// Byte offset in the encoded input where the failing container
        /// structure begins, when the codec parser can name it.
        offset: Option<u64>,
        /// Stable container-structure identity, when the codec parser can
        /// name it.
        identity: Option<&'static str>,
    },
    /// A parameter error.
    Parameter {
        /// Selected format when the failure belongs to a codec.
        format: Option<ImageFormat>,
        /// High-level diagnostic suitable for logs.
        message: String,
        /// Public operation that produced the failure.
        stage: Option<ImageErrorStage>,
        /// Byte offset in the encoded input where the failing container
        /// structure begins, when the codec parser can name it.
        offset: Option<u64>,
        /// Stable container-structure identity, when the codec parser can
        /// name it.
        identity: Option<&'static str>,
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
    /// The input is an incomplete prefix of a supported container.
    ///
    /// Retry only after appending enough bytes to reach [`Self::minimum_input`].
    /// Terminal results ([`Self::UnknownFormat`], [`Self::Malformed`], and
    /// every other variant) must never be turned into an implicit retry loop.
    NeedMoreData {
        /// Detected or partially identified format, when the prefix already
        /// names one.
        format: Option<ImageFormat>,
        /// Public operation that produced the status, when known.
        stage: Option<ImageErrorStage>,
        /// Byte offset in the encoded input where the incomplete structure
        /// begins, when the parser can name it.
        offset: Option<u64>,
        /// Stable container-structure identity, when the parser can name it.
        identity: Option<&'static str>,
        /// Total encoded-input length (in bytes) the caller should provide
        /// before retrying.
        minimum: u64,
    },
    /// The operation stopped at a cooperative checkpoint because the
    /// caller's [`crate::CancellationToken`] was cancelled.
    ///
    /// No partial result is published: token-aware operations either return
    /// their complete validated result or this error. Retrying after
    /// cancelling is pointless; create a fresh token for a new operation.
    Cancelled {
        /// Detected or selected format, when the operation reached a codec.
        format: Option<ImageFormat>,
        /// Public operation that was cancelled.
        stage: Option<ImageErrorStage>,
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
            Self::NeedMoreData { .. } => ImageErrorKind::NeedMoreData,
            Self::Cancelled { .. } => ImageErrorKind::Cancelled,
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
            Self::NeedMoreData { format, .. } => *format,
            Self::Cancelled { format, .. } => *format,
        }
    }

    /// Return the stable high-level diagnostic retained by the failure.
    ///
    /// The error kind and format are the compatibility surface for recovery;
    /// this message is intended for logs and troubleshooting.
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::UnknownFormat
            | Self::FeatureDisabled { .. }
            | Self::LimitExceeded { .. }
            | Self::NeedMoreData { .. }
            | Self::Cancelled { .. } => None,
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
            stage: None,
            offset: None,
            identity: None,
        }
    }

    pub(crate) fn parameter(message: impl Into<String>) -> Self {
        Self::Parameter {
            format: None,
            message: message.into(),
            stage: None,
            offset: None,
            identity: None,
        }
    }

    /// Return the public operation that produced this failure, when known.
    #[must_use]
    pub const fn stage(&self) -> Option<ImageErrorStage> {
        match self {
            Self::Malformed { stage, .. }
            | Self::Unsupported { stage, .. }
            | Self::Dimensions { stage, .. }
            | Self::Parameter { stage, .. }
            | Self::NeedMoreData { stage, .. }
            | Self::Cancelled { stage, .. } => *stage,
            Self::UnknownFormat | Self::FeatureDisabled { .. } | Self::LimitExceeded { .. } => None,
        }
    }

    /// Return the encoded-input byte offset of the failing container
    /// structure, when the codec parser can name it.
    #[must_use]
    pub const fn offset(&self) -> Option<u64> {
        match self {
            Self::Malformed { offset, .. }
            | Self::Unsupported { offset, .. }
            | Self::Dimensions { offset, .. }
            | Self::Parameter { offset, .. }
            | Self::NeedMoreData { offset, .. } => *offset,
            Self::UnknownFormat
            | Self::FeatureDisabled { .. }
            | Self::LimitExceeded { .. }
            | Self::Cancelled { .. } => None,
        }
    }

    /// Return the stable container-structure identity of the failing parse
    /// site, when the codec parser can name it.
    #[must_use]
    pub const fn identity(&self) -> Option<&'static str> {
        match self {
            Self::Malformed { identity, .. }
            | Self::Unsupported { identity, .. }
            | Self::Dimensions { identity, .. }
            | Self::Parameter { identity, .. }
            | Self::NeedMoreData { identity, .. } => *identity,
            Self::UnknownFormat
            | Self::FeatureDisabled { .. }
            | Self::LimitExceeded { .. }
            | Self::Cancelled { .. } => None,
        }
    }

    /// Return the total input length required before an incremental retry,
    /// or `None` for terminal results.
    ///
    /// Only [`ImageErrorKind::NeedMoreData`] carries a retry minimum. Every
    /// other result is terminal: feeding more bytes and calling again is
    /// either pointless or explicitly forbidden.
    #[must_use]
    pub const fn minimum_input(&self) -> Option<u64> {
        match self {
            Self::NeedMoreData { minimum, .. } => Some(*minimum),
            Self::UnknownFormat
            | Self::FeatureDisabled { .. }
            | Self::Malformed { .. }
            | Self::Unsupported { .. }
            | Self::Dimensions { .. }
            | Self::Parameter { .. }
            | Self::LimitExceeded { .. }
            | Self::Cancelled { .. } => None,
        }
    }

    pub(crate) fn with_format(self, selected: ImageFormat) -> Self {
        match self {
            Self::Dimensions {
                format: None,
                message,
                stage,
                offset,
                identity,
            } => Self::Dimensions {
                format: Some(selected),
                message,
                stage,
                offset,
                identity,
            },
            Self::Parameter {
                format: None,
                message,
                stage,
                offset,
                identity,
            } => Self::Parameter {
                format: Some(selected),
                message,
                stage,
                offset,
                identity,
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
            ImageError::Malformed {
                format, message, ..
            } => {
                write!(f, "malformed {format:?} image data: {message}")
            }
            ImageError::Unsupported {
                format, message, ..
            } => match format {
                Some(format) => write!(f, "unsupported {format:?}: {message}"),
                None => write!(f, "unsupported: {message}"),
            },
            ImageError::Dimensions {
                format, message, ..
            } => match format {
                Some(format) => write!(f, "invalid {format:?} dimensions: {message}"),
                None => write!(f, "invalid image dimensions: {message}"),
            },
            ImageError::Parameter {
                format, message, ..
            } => match format {
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
            ImageError::NeedMoreData {
                format, minimum, ..
            } => match format {
                Some(format) => {
                    write!(f, "need at least {minimum} bytes of {format:?} input")
                }
                None => write!(f, "need at least {minimum} bytes of input"),
            },
            ImageError::Cancelled { format, .. } => match format {
                Some(format) => write!(f, "cancelled {format:?} operation"),
                None => write!(f, "cancelled operation"),
            },
        }
    }
}

impl std::error::Error for ImageError {}

/// A specialized result type for codec operations.
pub type ImageResult<T> = Result<T, ImageError>;
