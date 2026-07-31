//! Caller-selected policy for encoded image inspection and decoding.

use crate::{CodecOperation, ImageError, ImageInfo, ImageResult, ResourceLimit};

const _: () = assert!(usize::BITS <= u64::BITS);

/// Resource limits applied before or during inspection and decoding.
///
/// An absent limit is unlimited for backward compatibility. A maximum of zero
/// is valid and rejects any non-empty encoded input.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DecodeLimits {
    max_encoded_bytes: Option<u64>,
    max_width: Option<u32>,
    max_height: Option<u32>,
    max_pixels: Option<u64>,
    max_primary_decoded_bytes: Option<u64>,
    max_frames: Option<u32>,
}

impl DecodeLimits {
    /// Create an unlimited resource-limit set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_encoded_bytes: None,
            max_width: None,
            max_height: None,
            max_pixels: None,
            max_primary_decoded_bytes: None,
            max_frames: None,
        }
    }

    /// Return the maximum accepted encoded-input length.
    #[must_use]
    pub const fn max_encoded_bytes(self) -> Option<u64> {
        self.max_encoded_bytes
    }

    /// Set the maximum accepted encoded-input length.
    #[must_use]
    pub const fn with_max_encoded_bytes(mut self, maximum: u64) -> Self {
        self.max_encoded_bytes = Some(maximum);
        self
    }

    /// Return the maximum accepted inspected canvas width.
    #[must_use]
    pub const fn max_width(self) -> Option<u32> {
        self.max_width
    }

    /// Set the maximum accepted inspected canvas width.
    #[must_use]
    pub const fn with_max_width(mut self, maximum: u32) -> Self {
        self.max_width = Some(maximum);
        self
    }

    /// Return the maximum accepted inspected canvas height.
    #[must_use]
    pub const fn max_height(self) -> Option<u32> {
        self.max_height
    }

    /// Set the maximum accepted inspected canvas height.
    #[must_use]
    pub const fn with_max_height(mut self, maximum: u32) -> Self {
        self.max_height = Some(maximum);
        self
    }

    /// Return the maximum accepted inspected canvas pixel count.
    #[must_use]
    pub const fn max_pixels(self) -> Option<u64> {
        self.max_pixels
    }

    /// Set the maximum accepted inspected canvas pixel count.
    #[must_use]
    pub const fn with_max_pixels(mut self, maximum: u64) -> Self {
        self.max_pixels = Some(maximum);
        self
    }

    /// Return the maximum accepted decoded byte length for the primary image.
    #[must_use]
    pub const fn max_primary_decoded_bytes(self) -> Option<u64> {
        self.max_primary_decoded_bytes
    }

    /// Set the maximum accepted decoded byte length for the primary image.
    #[must_use]
    pub const fn with_max_primary_decoded_bytes(mut self, maximum: u64) -> Self {
        self.max_primary_decoded_bytes = Some(maximum);
        self
    }

    /// Return the maximum accepted frame or page count.
    #[must_use]
    pub const fn max_frames(self) -> Option<u32> {
        self.max_frames
    }

    /// Set the maximum accepted frame or page count.
    #[must_use]
    pub const fn with_max_frames(mut self, maximum: u32) -> Self {
        self.max_frames = Some(maximum);
        self
    }
}

/// Policy shared by encoded-image inspection and decoding.
///
/// [`Default`] preserves the original unlimited behavior. New limits are
/// added here rather than as codec-specific knobs so every canonical entry
/// point has one precedence and error contract.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DecodePolicy {
    limits: DecodeLimits,
}

impl DecodePolicy {
    /// Create the unlimited compatibility policy.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            limits: DecodeLimits::new(),
        }
    }

    /// Create a policy from an explicit limit set.
    #[must_use]
    pub const fn with_limits(limits: DecodeLimits) -> Self {
        Self { limits }
    }

    /// Return this policy's resource limits.
    #[must_use]
    pub const fn limits(self) -> DecodeLimits {
        self.limits
    }

    /// Set the maximum accepted encoded-input length.
    #[must_use]
    pub const fn with_max_encoded_bytes(mut self, maximum: u64) -> Self {
        self.limits = self.limits.with_max_encoded_bytes(maximum);
        self
    }

    /// Set the maximum accepted inspected canvas width.
    #[must_use]
    pub const fn with_max_width(mut self, maximum: u32) -> Self {
        self.limits = self.limits.with_max_width(maximum);
        self
    }

    /// Set the maximum accepted inspected canvas height.
    #[must_use]
    pub const fn with_max_height(mut self, maximum: u32) -> Self {
        self.limits = self.limits.with_max_height(maximum);
        self
    }

    /// Set the maximum accepted inspected canvas pixel count.
    #[must_use]
    pub const fn with_max_pixels(mut self, maximum: u64) -> Self {
        self.limits = self.limits.with_max_pixels(maximum);
        self
    }

    /// Set the maximum accepted decoded byte length for the primary image.
    #[must_use]
    pub const fn with_max_primary_decoded_bytes(mut self, maximum: u64) -> Self {
        self.limits = self.limits.with_max_primary_decoded_bytes(maximum);
        self
    }

    /// Set the maximum accepted frame or page count.
    #[must_use]
    pub const fn with_max_frames(mut self, maximum: u32) -> Self {
        self.limits = self.limits.with_max_frames(maximum);
        self
    }

    pub(crate) fn check_encoded_input(
        self,
        data: &[u8],
        operation: CodecOperation,
    ) -> ImageResult<()> {
        let observed = data.len() as u64;
        if let Some(maximum) = self.limits.max_encoded_bytes
            && observed > maximum
        {
            return Err(ImageError::LimitExceeded {
                format: None,
                operation,
                resource: ResourceLimit::EncodedBytes,
                maximum,
                observed,
            });
        }
        Ok(())
    }

    pub(crate) const fn requires_image_info(self) -> bool {
        self.limits.max_width.is_some()
            || self.limits.max_height.is_some()
            || self.limits.max_pixels.is_some()
            || self.limits.max_primary_decoded_bytes.is_some()
            || self.limits.max_frames.is_some()
    }

    pub(crate) fn check_image_info(
        self,
        info: &ImageInfo,
        operation: CodecOperation,
    ) -> ImageResult<()> {
        check_limit(
            self.limits.max_width.map(u64::from),
            u64::from(info.width),
            info,
            operation,
            ResourceLimit::Width,
        )?;
        check_limit(
            self.limits.max_height.map(u64::from),
            u64::from(info.height),
            info,
            operation,
            ResourceLimit::Height,
        )?;
        check_limit(
            self.limits.max_pixels,
            u64::from(info.width).saturating_mul(u64::from(info.height)),
            info,
            operation,
            ResourceLimit::Pixels,
        )?;
        if let Some(maximum) = self.limits.max_primary_decoded_bytes {
            let primary_decoded_bytes =
                info.mode
                    .expected_bytes(info.width, info.height)
                    .map_err(|error| error.with_format(info.format))? as u64;
            check_limit(
                Some(maximum),
                primary_decoded_bytes,
                info,
                operation,
                ResourceLimit::PrimaryDecodedBytes,
            )?;
        }
        self.check_frame_count(info, operation)?;
        Ok(())
    }

    pub(crate) fn check_frame_count(
        self,
        info: &ImageInfo,
        operation: CodecOperation,
    ) -> ImageResult<()> {
        let Some(maximum) = self.limits.max_frames else {
            return Ok(());
        };
        let observed = match operation {
            CodecOperation::Inspection | CodecOperation::SequenceDecode => {
                info.frame_count.map(u64::from)
            }
            // Still decode and lazy still materialization retain exactly one
            // frame, so only a zero maximum can reject them.
            CodecOperation::StillDecode => Some(1),
            _ => None,
        };
        if let Some(observed) = observed {
            check_limit(
                Some(u64::from(maximum)),
                observed,
                info,
                operation,
                ResourceLimit::Frames,
            )?;
        }
        Ok(())
    }
}

fn check_limit(
    maximum: Option<u64>,
    observed: u64,
    info: &ImageInfo,
    operation: CodecOperation,
    resource: ResourceLimit,
) -> ImageResult<()> {
    if let Some(maximum) = maximum
        && observed > maximum
    {
        return Err(ImageError::LimitExceeded {
            format: Some(info.format),
            operation,
            resource,
            maximum,
            observed,
        });
    }
    Ok(())
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    use crate::{ImageFormat, ImageMode, SourceDescriptor};

    let info = ImageInfo {
        format: ImageFormat::Png,
        width: 1,
        height: 1,
        mode: ImageMode::Rgb8,
        bit_depth: 8,
        palette: None,
        is_animated: false,
        frame_count: Some(1),
        cursor_hotspot: None,
        source: SourceDescriptor::new(),
    };
    let policy = DecodePolicy::default().with_max_frames(0);
    // Operations outside the decode-policy call sites cannot observe a frame
    // count and must remain unlimited.
    for operation in [
        CodecOperation::Detection,
        CodecOperation::StillEncode,
        CodecOperation::SequenceEncode,
    ] {
        let _ = policy.check_frame_count(&info, operation);
    }
    let mut unknown_count = info;
    unknown_count.frame_count = None;
    let _ = policy.check_frame_count(&unknown_count, CodecOperation::SequenceDecode);
}
