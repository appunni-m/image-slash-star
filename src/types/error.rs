//! Error types for decoded buffers and codec operations.

use super::ImageFormat;
use std::fmt;

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
        /// Stable high-level diagnostic suitable for logs.
        message: String,
    },
    /// Valid input, options, or output cannot be represented by the selected codec.
    Unsupported {
        /// Selected format when the failure belongs to a codec.
        format: Option<ImageFormat>,
        /// Stable high-level diagnostic suitable for logs.
        message: String,
    },
    /// The operation dimensions are out of bounds or mismatched.
    Dimensions,
    /// A parameter error.
    Parameter(String),
    /// An I/O error.
    IoError(String),
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
            ImageError::Dimensions => write!(f, "image dimensions are out of bounds"),
            ImageError::Parameter(msg) => write!(f, "parameter error: {}", msg),
            ImageError::IoError(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl std::error::Error for ImageError {}

/// A specialized Result type for image operations.
pub type ImageResult<T> = Result<T, ImageError>;

/// A rectangle representing a region of an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// x coordinate of the top-left corner.
    pub x: u32,
    /// y coordinate of the top-left corner.
    pub y: u32,
    /// Width of the rectangle.
    pub width: u32,
    /// Height of the rectangle.
    pub height: u32,
}

impl Rect {
    /// Create a new rectangle.
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }
}
