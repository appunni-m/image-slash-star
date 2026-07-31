//! Caller-selected policy for encoded image inspection and decoding.

use crate::{
    CodecOperation, ImageError, ImageFormat, ImageInfo, ImageMode, ImageResult, ResourceLimit,
};

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
    max_frame_decoded_bytes: Option<u64>,
    max_sequence_decoded_bytes: Option<u64>,
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
            max_frame_decoded_bytes: None,
            max_sequence_decoded_bytes: None,
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

    /// Return the maximum accepted decoded byte length of one later frame.
    #[must_use]
    pub const fn max_frame_decoded_bytes(self) -> Option<u64> {
        self.max_frame_decoded_bytes
    }

    /// Set the maximum accepted decoded byte length of one later frame.
    #[must_use]
    pub const fn with_max_frame_decoded_bytes(mut self, maximum: u64) -> Self {
        self.max_frame_decoded_bytes = Some(maximum);
        self
    }

    /// Return the maximum accepted cumulative decoded byte length of every
    /// retained frame or page in a sequence.
    #[must_use]
    pub const fn max_sequence_decoded_bytes(self) -> Option<u64> {
        self.max_sequence_decoded_bytes
    }

    /// Set the maximum accepted cumulative decoded byte length of every
    /// retained frame or page in a sequence.
    #[must_use]
    pub const fn with_max_sequence_decoded_bytes(mut self, maximum: u64) -> Self {
        self.max_sequence_decoded_bytes = Some(maximum);
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

    /// Set the maximum accepted decoded byte length of one later frame.
    #[must_use]
    pub const fn with_max_frame_decoded_bytes(mut self, maximum: u64) -> Self {
        self.limits = self.limits.with_max_frame_decoded_bytes(maximum);
        self
    }

    /// Set the maximum accepted cumulative decoded byte length of every
    /// retained frame or page in a sequence.
    #[must_use]
    pub const fn with_max_sequence_decoded_bytes(mut self, maximum: u64) -> Self {
        self.limits = self.limits.with_max_sequence_decoded_bytes(maximum);
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
            || self.limits.max_sequence_decoded_bytes.is_some()
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

    /// Create the sequence materialization budget for one detected format.
    pub(crate) fn sequence_budget(self, format: ImageFormat) -> SequenceDecodeBudget {
        SequenceDecodeBudget {
            format,
            max_frame_bytes: self.limits.max_frame_decoded_bytes,
            remaining_sequence_bytes: None,
            total_sequence_max: self.limits.max_sequence_decoded_bytes,
        }
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

/// Crate-internal budget passed to sequence decoders so later-frame and
/// cumulative decoded-byte limits reject before the next frame allocation.
#[derive(Debug, Clone)]
pub(crate) struct SequenceDecodeBudget {
    format: ImageFormat,
    #[cfg_attr(
        all(
            not(any(feature = "gif", feature = "png", feature = "webp", feature = "tiff")),
            any(not(feature = "avif"), target_arch = "wasm32")
        ),
        allow(dead_code)
    )]
    max_frame_bytes: Option<u64>,
    remaining_sequence_bytes: Option<u64>,
    total_sequence_max: Option<u64>,
}

impl SequenceDecodeBudget {
    /// Create the unlimited budget used by convenience and still paths.
    #[cfg(any(feature = "gif", coverage))]
    pub(crate) fn default_for(format: ImageFormat) -> Self {
        Self {
            format,
            max_frame_bytes: None,
            remaining_sequence_bytes: None,
            total_sequence_max: None,
        }
    }

    /// Charge the inspected primary frame against the cumulative limit before
    /// sequence materialization begins.
    pub(crate) fn charge_primary(&mut self, info: &ImageInfo) -> ImageResult<()> {
        let Some(maximum) = self.total_sequence_max else {
            return Ok(());
        };
        let primary = info
            .mode
            .expected_bytes(info.width, info.height)
            .map_err(|error| error.with_format(info.format))? as u64;
        if primary > maximum {
            return Err(ImageError::LimitExceeded {
                format: Some(self.format),
                operation: CodecOperation::SequenceDecode,
                resource: ResourceLimit::SequenceDecodedBytes,
                maximum,
                observed: primary,
            });
        }
        // The guard above proves primary <= maximum.
        #[allow(clippy::arithmetic_side_effects)]
        let remaining = maximum - primary;
        self.remaining_sequence_bytes = Some(remaining);
        Ok(())
    }

    /// Reserve one later frame's decoded byte length before its pixel work.
    #[cfg_attr(
        all(
            not(any(feature = "gif", feature = "png", feature = "webp", feature = "tiff")),
            any(not(feature = "avif"), target_arch = "wasm32")
        ),
        allow(dead_code)
    )]
    pub(crate) fn reserve_later_frame(
        &mut self,
        mode: ImageMode,
        width: u32,
        height: u32,
    ) -> ImageResult<()> {
        let bytes = mode
            .expected_bytes(width, height)
            .map_err(|error| error.with_format(self.format))? as u64;
        if let Some(maximum) = self.max_frame_bytes
            && bytes > maximum
        {
            return Err(ImageError::LimitExceeded {
                format: Some(self.format),
                operation: CodecOperation::SequenceDecode,
                resource: ResourceLimit::FrameDecodedBytes,
                maximum,
                observed: bytes,
            });
        }
        if let Some(remaining) = self.remaining_sequence_bytes {
            if bytes > remaining {
                let maximum = self.total_sequence_max.unwrap_or_default();
                let consumed_before = maximum.saturating_sub(remaining);
                return Err(ImageError::LimitExceeded {
                    format: Some(self.format),
                    operation: CodecOperation::SequenceDecode,
                    resource: ResourceLimit::SequenceDecodedBytes,
                    maximum,
                    observed: consumed_before.saturating_add(bytes),
                });
            }
            // The guard above proves bytes <= remaining.
            #[allow(clippy::arithmetic_side_effects)]
            let remaining_after = remaining - bytes;
            self.remaining_sequence_bytes = Some(remaining_after);
        }
        Ok(())
    }
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

    // Exercise the unrepresentable-transfer-length error paths that fixtures
    // cannot reach: the layout-overflow mutation rejects during inspection,
    // before either budget method runs.
    let mut overflow_budget = DecodePolicy::default()
        .with_max_frame_decoded_bytes(u64::MAX)
        .with_max_sequence_decoded_bytes(u64::MAX)
        .sequence_budget(ImageFormat::Png);
    let overflow_info = ImageInfo {
        format: ImageFormat::Png,
        width: u32::MAX,
        height: u32::MAX,
        mode: ImageMode::Rgb8,
        bit_depth: 8,
        palette: None,
        is_animated: false,
        frame_count: Some(1),
        cursor_hotspot: None,
        source: SourceDescriptor::new(),
    };
    let _ = overflow_budget.charge_primary(&overflow_info);
    let _ = overflow_budget.reserve_later_frame(ImageMode::Rgb8, u32::MAX, u32::MAX);
}
