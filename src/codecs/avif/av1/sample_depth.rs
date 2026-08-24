//! Checked AVIF sample-depth normalization for the public 8-bit boundary.
//!
//! AV1 reconstruction stores samples in `u16` so the same safe plane and
//! canvas types can eventually carry 8-, 10-, and 12-bit pictures. This
//! module owns the narrow conversion from a validated full-range nominal
//! sample to the crate's current 8-bit transfer buffer. It is deliberately
//! independent from AV1 entropy parsing: a future high-bit-depth decoder can
//! prove reconstruction first and then call this boundary without unchecked
//! shifts or narrowing casts.

/// Truncate one validated AVIF sample to the crate's 8-bit transfer boundary.
///
/// High-bit-depth AVIF conversion for the target Pillow-compatible path is a
/// bit truncation, not a rounded full-range rescale. Keeping that distinction
/// explicit prevents a later decoder stage from accidentally changing the
/// requested conversion semantics while still rejecting samples outside the
/// declared nominal range.
pub(crate) fn truncate_to_u8(value: u16, bit_depth: u32) -> Option<u8> {
    let maximum = maximum_sample(bit_depth)?;
    let value = u32::from(value);
    if value > maximum {
        return None;
    }
    let shift = bit_depth.checked_sub(8)?;
    u8::try_from(value.checked_shr(shift)?).ok()
}

fn maximum_sample(bit_depth: u32) -> Option<u32> {
    if !(8..=12).contains(&bit_depth) {
        return None;
    }
    1_u32.checked_shl(bit_depth)?.checked_sub(1)
}

#[cfg(test)]
mod tests {
    use super::truncate_to_u8;

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
