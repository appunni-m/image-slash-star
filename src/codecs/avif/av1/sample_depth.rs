//! Checked AVIF sample-depth normalization for the public 8-bit boundary.
//!
//! AV1 reconstruction stores samples in `u16` so the same safe plane and
//! canvas types can eventually carry 8-, 10-, and 12-bit pictures. This
//! module owns the narrow conversion from a validated full-range nominal
//! sample to the crate's current 8-bit transfer buffer. It is deliberately
//! independent from AV1 entropy parsing: a future high-bit-depth decoder can
//! prove reconstruction first and then call this boundary without unchecked
//! shifts or narrowing casts.

/// Normalize one full-range nominal AVIF sample to an 8-bit transfer sample.
///
/// The conversion is rounded to nearest over the nominal `[0, 2^depth - 1]`
/// range. A sample outside that range is rejected instead of being silently
/// clipped; clipping belongs to a codec-defined reconstruction stage, not to
/// this public representation boundary.
pub(crate) fn normalize_full_range(value: u16, bit_depth: u32) -> Option<u8> {
    let maximum = maximum_sample(bit_depth)?;
    let value = u32::from(value);
    if value > maximum {
        return None;
    }
    let scaled = value
        .checked_mul(255)?
        .checked_add(maximum / 2)?
        .checked_div(maximum)?;
    u8::try_from(scaled).ok()
}

fn maximum_sample(bit_depth: u32) -> Option<u32> {
    if !(8..=12).contains(&bit_depth) {
        return None;
    }
    1_u32.checked_shl(bit_depth)?.checked_sub(1)
}

#[cfg(test)]
mod tests {
    use super::normalize_full_range;

    #[test]
    fn normalizes_the_endpoints_for_each_avif_depth() {
        for bit_depth in [8, 10, 12] {
            let maximum = (1_u16 << bit_depth) - 1;
            assert_eq!(normalize_full_range(0, bit_depth), Some(0));
            assert_eq!(normalize_full_range(maximum, bit_depth), Some(255));
        }
    }

    #[test]
    fn rounds_a_high_depth_midpoint() {
        assert_eq!(normalize_full_range(512, 10), Some(128));
        assert_eq!(normalize_full_range(2_048, 12), Some(128));
    }

    #[test]
    fn rejects_invalid_depths_and_out_of_range_samples() {
        assert_eq!(normalize_full_range(0, 7), None);
        assert_eq!(normalize_full_range(0, 13), None);
        assert_eq!(normalize_full_range(256, 8), None);
        assert_eq!(normalize_full_range(4_096, 12), None);
    }
}
