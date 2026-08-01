//! Caller-selected policy for encoded-output admission.

use crate::{CodecOperation, ImageError, ImageFormat, ImageResult, ResourceLimit};

/// Policy applied to complete encoded output before it is returned or written.
///
/// An absent limit is unlimited for backward compatibility. The current
/// whole-buffer encoders still allocate their internal result before this
/// policy can measure it, so this is an output-result bound rather than a
/// recoverable out-of-memory guarantee. Incremental structural writing and
/// transient internal-allocation accounting remain separate roadmap work.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EncodePolicy {
    max_output_bytes: Option<u64>,
}

impl EncodePolicy {
    /// Create the unlimited compatibility policy.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_output_bytes: None,
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
