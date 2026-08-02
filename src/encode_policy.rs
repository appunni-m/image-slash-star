//! Caller-selected policy for encoded-output admission.

use crate::{CodecOperation, ImageError, ImageFormat, ImageResult, ResourceLimit};

/// Policy applied to complete encoded output before it is returned or written.
///
/// An absent limit is unlimited for backward compatibility. The output-result
/// bound does not account for transient allocations or recoverable out-of-
/// memory failure. `max_work_units` counts the deterministic cooperative
/// checkpoints reached by an encode, including TIFF Deflate input-row,
/// level-six matcher, expansion, Huffman, bitstream, and checksum intervals,
/// the PNG adaptive-filter and filtered-row checkpoints charged after each
/// 1,024 row bytes, BMP row-conversion checkpoints charged after each 1,024
/// pixels, the lossy WebP VP8 analysis/partition stages, and the lossless WebP VP8L
/// predictor/cross-color/entropy/transform, bounded backward-reference,
/// histogram/Huffman, bitstream, and token-stream stages, and GIF LZW
/// input-symbol intervals; it is a
/// work-control bound, not a CPU-time or allocation guarantee.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EncodePolicy {
    max_output_bytes: Option<u64>,
    max_work_units: Option<u64>,
}

impl EncodePolicy {
    /// Create the unlimited compatibility policy.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_output_bytes: None,
            max_work_units: None,
        }
    }

    /// Return the maximum admitted encoded-output length.
    #[must_use]
    pub const fn max_output_bytes(self) -> Option<u64> {
        self.max_output_bytes
    }

    /// Set the inclusive maximum admitted encoded-output length.
    #[must_use]
    pub const fn with_max_output_bytes(mut self, maximum: u64) -> Self {
        self.max_output_bytes = Some(maximum);
        self
    }

    /// Return the maximum number of cooperative encode checkpoints admitted.
    #[must_use]
    pub const fn max_work_units(self) -> Option<u64> {
        self.max_work_units
    }

    /// Set the inclusive maximum number of cooperative encode checkpoints.
    ///
    /// One work unit is charged at each documented cancellation checkpoint.
    /// PNG long-row filtering charges additional checkpoints after each 1,024
    /// filtered bytes while adaptive candidates are scored or emitted. BMP
    /// row conversion charges additional checkpoints after each 1,024 pixels.
    /// TIFF
    /// Deflate charges input-row and level-six matcher candidate, insertion,
    /// fizzle, window, and position intervals plus expansion, Huffman,
    /// bitstream, stored-block, and checksum intervals. Lossy
    /// WebP VP8 encoding charges checkpoints between its major analysis,
    /// mode-selection, probability, and bitstream stages; VP8L encoding
    /// charges checkpoints around predictor, cross-color, entropy, transform,
    /// bounded backward-reference, histogram/Huffman, bitstream, and
    /// token-stream intervals. GIF LZW encoding charges an interval for each
    /// input symbol considered by its dictionary pass.
    /// Exhaustion returns [`ImageError::LimitExceeded`] before that checkpoint
    /// performs further codec work.
    #[must_use]
    pub const fn with_max_work_units(mut self, maximum: u64) -> Self {
        self.max_work_units = Some(maximum);
        self
    }

    pub(crate) fn check_output(
        self,
        bytes: &[u8],
        format: ImageFormat,
        operation: CodecOperation,
    ) -> ImageResult<()> {
        self.check_output_len(bytes.len(), format, operation)
    }

    pub(crate) fn check_output_len(
        self,
        observed_length: usize,
        format: ImageFormat,
        operation: CodecOperation,
    ) -> ImageResult<()> {
        let Some(maximum) = self.max_output_bytes else {
            return Ok(());
        };
        let observed = u64::try_from(observed_length).unwrap_or(u64::MAX);
        if observed > maximum {
            return Err(ImageError::LimitExceeded {
                format: Some(format),
                operation,
                resource: ResourceLimit::EncodedOutputBytes,
                maximum,
                observed,
            });
        }
        Ok(())
    }
}
