//! Checked AVIF sample-depth normalization for the public 8-bit boundary.
//!
//! AV1 reconstruction stores samples in `u16` so the same safe plane and
//! canvas types can eventually carry 8-, 10-, and 12-bit pictures. This
//! module owns the narrow conversion from a validated full-range nominal
//! sample to the crate's current 8-bit transfer buffer. It is deliberately
//! independent from AV1 entropy parsing: a future high-bit-depth decoder can
//! prove reconstruction first and then call this boundary without unchecked
//! shifts or narrowing casts.

/// A validated nominal AVIF sample depth.
///
/// This type keeps sample-domain invariants together. It is intentionally
/// independent from entropy parsing: a future high-bit-depth decoder can
/// prove reconstruction first and then use the same checked boundary for
/// sample validation, midpoint prediction, and public 8-bit conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SampleDepth {
    bits: u32,
    maximum: u16,
}

impl SampleDepth {
    /// Construct a depth supported by the checked AVIF sample domain.
    pub(crate) fn new(bits: u32) -> Option<Self> {
        if !matches!(bits, 8 | 10 | 12) {
            return None;
        }
        let maximum = 1_u32.checked_shl(bits)?.checked_sub(1)?;
        Some(Self {
            bits,
            maximum: u16::try_from(maximum).ok()?,
        })
    }

    /// Return the validated number of nominal sample bits.
    pub(crate) const fn bits(self) -> u32 {
        self.bits
    }

    /// Return the greatest representable sample for this depth.
    pub(crate) const fn maximum(self) -> u16 {
        self.maximum
    }

    /// Return the AV1 midpoint used by the lossless predictor defaults.
    pub(crate) const fn midpoint(self) -> u16 {
        1_u16 << self.bits.saturating_sub(1)
    }

    /// Return the AV1 missing-top-edge default for this sample depth.
    pub(crate) const fn top_edge_default(self) -> u16 {
        self.midpoint().saturating_sub(1)
    }

    /// Return the AV1 missing-left-edge default for this sample depth.
    pub(crate) const fn left_edge_default(self) -> u16 {
        self.midpoint().saturating_add(1)
    }

    /// Validate one reconstructed sample against this depth's nominal range.
    pub(crate) fn validate(self, value: u16) -> Option<u16> {
        (value <= self.maximum()).then_some(value)
    }

    /// Truncate one validated sample to the crate's 8-bit transfer boundary.
    ///
    /// High-bit-depth AVIF conversion for the target Pillow-compatible path is
    /// a bit truncation, not a rounded full-range rescale. Keeping that
    /// distinction explicit prevents a later decoder stage from accidentally
    /// changing the requested conversion semantics.
    pub(crate) fn truncate_to_u8(self, value: u16) -> Option<u8> {
        let value = u32::from(self.validate(value)?);
        let shift = self.bits().checked_sub(8)?;
        u8::try_from(value.checked_shr(shift)?).ok()
    }
}

/// Truncate one validated AVIF sample to the crate's 8-bit transfer boundary.
pub(crate) fn truncate_to_u8(value: u16, bit_depth: u32) -> Option<u8> {
    SampleDepth::new(bit_depth)?.truncate_to_u8(value)
}

#[cfg(test)]
mod tests {
    use super::{SampleDepth, truncate_to_u8};

    #[test]
    fn validates_supported_depths_and_sample_domain() -> Result<(), &'static str> {
        assert_eq!(SampleDepth::new(7), None);
        assert_eq!(SampleDepth::new(9), None);
        assert_eq!(SampleDepth::new(11), None);
        assert_eq!(SampleDepth::new(13), None);
        let depth = SampleDepth::new(12).ok_or("12-bit AVIF depth is supported")?;
        assert_eq!(depth.bits(), 12);
        assert_eq!(depth.maximum(), 4_095);
        assert_eq!(depth.midpoint(), 2_048);
        assert_eq!(depth.top_edge_default(), 2_047);
        assert_eq!(depth.left_edge_default(), 2_049);
        assert_eq!(depth.validate(4_095), Some(4_095));
        assert_eq!(depth.validate(4_096), None);
        Ok(())
    }

    #[test]
    fn truncates_the_endpoints_for_each_avif_depth() {
        for bit_depth in [8, 10, 12] {
            let maximum = (1_u16 << bit_depth) - 1;
            assert_eq!(truncate_to_u8(0, bit_depth), Some(0));
            assert_eq!(truncate_to_u8(maximum, bit_depth), Some(255));
        }
    }

    #[test]
    fn truncates_high_depth_samples_at_the_declared_boundary() {
        assert_eq!(truncate_to_u8(0, 10), Some(0));
        assert_eq!(truncate_to_u8(1_023, 10), Some(255));
        assert_eq!(truncate_to_u8(2_048, 12), Some(128));
        assert_eq!(truncate_to_u8(4_095, 12), Some(255));
        assert_eq!(truncate_to_u8(255, 8), Some(255));
    }

    #[test]
    fn rejects_invalid_depths_and_out_of_range_samples() {
        assert_eq!(truncate_to_u8(0, 7), None);
        assert_eq!(truncate_to_u8(0, 13), None);
        assert_eq!(truncate_to_u8(256, 8), None);
        assert_eq!(truncate_to_u8(4_096, 12), None);
    }
}
