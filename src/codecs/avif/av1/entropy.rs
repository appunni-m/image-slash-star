//! Scalar AV1 multi-symbol arithmetic decoding over segmented tile bytes.

use std::ops::Range;

use super::bit_reader::SegmentedData;

const WINDOW_BITS: i32 = 64;
const WINDOW_OUTPUT_BITS: u32 = 16;
const MIN_PROBABILITY: u32 = 4;
const PROBABILITY_SHIFT: u32 = 6;

/// Scalar range-decoder state for one AV1 tile.
pub(super) struct RangeDecoder<'data, 'input, 'spans> {
    data: &'data SegmentedData<'input, 'spans>,
    #[cfg(coverage)]
    start: usize,
    position: usize,
    end: usize,
    difference: u64,
    range: u32,
    count: i32,
    allow_update_cdf: bool,
    #[cfg(coverage)]
    operations: Option<Vec<crate::Av1EntropyOperationState>>,
}

impl<'data, 'input, 'spans> RangeDecoder<'data, 'input, 'spans> {
    // ✅ VERIFIED: dav1d 1.5.3 src/msac.c:204-219 (`dav1d_msac_init`) and
    // src/msac.c:41-58 (`ctx_refill`). The fixed u64 window intentionally
    // preserves the pinned 64-bit C state on both native and wasm32 targets.
    pub(super) fn new(
        data: &'data SegmentedData<'input, 'spans>,
        start: usize,
        end: usize,
        disable_cdf_update: bool,
    ) -> Option<Self> {
        if start > end || end > data.len() {
            return None;
        }
        let mut decoder = Self {
            data,
            #[cfg(coverage)]
            start,
            position: start,
            end,
            difference: 0,
            range: 0x8000,
            count: -15,
            allow_update_cdf: !disable_cdf_update,
            #[cfg(coverage)]
            operations: None,
        };
        decoder.refill();
        Some(decoder)
    }

    #[cfg(coverage)]
    fn enable_operation_trace(&mut self) {
        self.operations = Some(Vec::new());
        self.record_operation("init", 0, -1, &[]);
    }

    #[cfg(coverage)]
    fn record_operation(
        &mut self,
        operation: &'static str,
        parameter: i32,
        value: i32,
        cdf: &[u16],
    ) {
        let step = self
            .operations
            .as_ref()
            .map_or(0, |operations| operations.len());
        #[expect(
            clippy::cast_possible_truncation,
            reason = "one bounded AV1 tile cannot contain u32::MAX scalar trace operations"
        )]
        let step = step as u32;
        let state = crate::Av1EntropyOperationState {
            operation,
            parameter,
            step,
            value,
            byte_position: self.position.saturating_sub(self.start),
            difference: self.difference,
            range: self.range,
            count: self.count,
            cdf: cdf.to_vec(),
        };
        if let Some(operations) = &mut self.operations {
            operations.push(state);
        }
    }

    #[cfg(coverage)]
    #[coverage(off)]
    fn operation_trace(&self) -> Vec<crate::Av1EntropyOperationState> {
        self.operations.clone().unwrap_or_default()
    }

    // ✅ VERIFIED: dav1d 1.5.3 src/msac.c:41-58 (`ctx_refill`).
    fn refill(&mut self) {
        // `count` is initialized to -15 and normalized/refilled only within
        // dav1d's -15..=41 state range, so every shift below is representable.
        let mut shift = WINDOW_BITS.wrapping_sub(self.count).wrapping_sub(24);
        loop {
            #[expect(
                clippy::cast_sign_loss,
                reason = "the refill loop reaches this conversion only while shift is nonnegative"
            )]
            let shift_width = shift as u32;
            if self.position >= self.end {
                self.difference |= !((!u64::from(u8::MAX)) << shift_width);
                break;
            }
            let byte = self.data.validated_byte(self.position) ^ u8::MAX;
            self.difference |= u64::from(byte) << shift_width;
            // `position < end <= data.len()` proves this increment cannot wrap.
            self.position = self.position.wrapping_add(1);
            shift = shift.wrapping_sub(8);
            if shift < 0 {
                break;
            }
        }
        self.count = WINDOW_BITS.wrapping_sub(shift).wrapping_sub(24);
    }

    // ✅ VERIFIED: dav1d 1.5.3 src/msac.c:80-97 (`ctx_norm`).
    fn normalize(&mut self, difference: u64, range: u32) {
        // Every caller derives a nonzero at-most-16-bit partition range from
        // the current normalized state, exactly as dav1d's `ctx_norm` does.
        let shift = 15 ^ (31 ^ range.leading_zeros());
        let previous_count = self.count;
        self.difference = difference.wrapping_shl(shift);
        self.range = range.wrapping_shl(shift);
        #[expect(
            clippy::cast_possible_wrap,
            reason = "a normalized 16-bit range shifts by at most fourteen bits"
        )]
        let signed_shift = shift as i32;
        self.count = previous_count.wrapping_sub(signed_shift);
        #[expect(
            clippy::cast_sign_loss,
            reason = "the conversion is guarded by previous_count >= 0"
        )]
        if previous_count >= 0 && (previous_count as u32) < shift {
            self.refill();
        }
    }

    // ✅ VERIFIED: dav1d 1.5.3 src/msac.c:99-112
    // (`dav1d_msac_decode_bool_equi_c`).
    pub(super) fn equal(&mut self) -> bool {
        let current_range = self.range;
        let mut difference = self.difference;
        let mut candidate = ((current_range >> 8) << 7).wrapping_add(MIN_PROBABILITY);
        let scaled = u64::from(candidate) << (u64::BITS - WINDOW_OUTPUT_BITS);
        let upper_partition = difference >= scaled;
        if upper_partition {
            difference = difference.wrapping_sub(scaled);
            // ✅ FIX: dav1d writes `v += r - 2 * v` with unsigned
            // intermediates. Its bounded final value is exactly `r - v`;
            // evaluating the C intermediate with checked Rust arithmetic
            // incorrectly rejects cases where `2 * v > r`.
            candidate = current_range.wrapping_sub(candidate);
        }
        self.normalize(difference, candidate);
        let value = !upper_partition;
        #[cfg(coverage)]
        self.record_operation("equal", -1, i32::from(value), &[]);
        value
    }

    // ✅ VERIFIED: dav1d 1.5.3 src/msac.c:117-128
    // (`dav1d_msac_decode_bool_c`).
    pub(super) fn fixed(&mut self, probability: u32) -> bool {
        let current_range = self.range;
        let mut difference = self.difference;
        let scaled_probability = (current_range >> 8)
            .wrapping_mul(probability >> PROBABILITY_SHIFT)
            >> (7 - PROBABILITY_SHIFT);
        let mut candidate = scaled_probability.wrapping_add(MIN_PROBABILITY);
        let scaled = u64::from(candidate) << (u64::BITS - WINDOW_OUTPUT_BITS);
        let upper_partition = difference >= scaled;
        if upper_partition {
            difference = difference.wrapping_sub(scaled);
            // ✅ FIX: preserve the final value of dav1d's wrapping unsigned
            // `v += r - 2 * v` without manufacturing an intermediate
            // underflow failure.
            candidate = current_range.wrapping_sub(candidate);
        }
        self.normalize(difference, candidate);
        let value = !upper_partition;
        #[cfg(coverage)]
        self.record_operation(
            "fixed",
            i32::try_from(probability).unwrap_or(i32::MAX),
            i32::from(value),
            &[],
        );
        value
    }

    // ✅ VERIFIED: dav1d 1.5.3 src/msac.c:132-166
    // (`dav1d_msac_decode_symbol_adapt_c`).
    pub(super) fn adaptive_symbol(
        &mut self,
        cdf: &mut [u16],
        symbol_count_minus_one: usize,
    ) -> u32 {
        // Callers supply AV1 tables with 1..=15 symbols, descending
        // probabilities no greater than 32768, and a count slot at `n`.
        let code = (self.difference >> (u64::BITS - WINDOW_OUTPUT_BITS)) as u32;
        let scaled_range = self.range >> 8;
        let mut upper = self.range;
        let mut value = 0_usize;
        let lower = loop {
            let probability = u32::from(cdf[value]) >> PROBABILITY_SHIFT;
            let mut lower = scaled_range.wrapping_mul(probability) >> (7 - PROBABILITY_SHIFT);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the symbol count is validated to at most fifteen"
            )]
            let remaining_symbols = symbol_count_minus_one.wrapping_sub(value) as u32;
            lower = lower.wrapping_add(MIN_PROBABILITY.wrapping_mul(remaining_symbols));
            if code >= lower {
                break lower;
            }
            upper = lower;
            value = value.wrapping_add(1);
        };
        let difference = self
            .difference
            .wrapping_sub(u64::from(lower) << (u64::BITS - WINDOW_OUTPUT_BITS));
        self.normalize(difference, upper.wrapping_sub(lower));

        if self.allow_update_cdf {
            let count = cdf[symbol_count_minus_one];
            let rate = 4_u32
                .wrapping_add(u32::from(count >> 4))
                .wrapping_add(u32::from(symbol_count_minus_one > 2));
            for probability in &mut cdf[..value] {
                let increase = 32_768_u32.wrapping_sub(u32::from(*probability)) >> rate;
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a valid AV1 CDF update increases a u16 probability by at most 2048"
                )]
                let increase = increase as u16;
                *probability = probability.wrapping_add(increase);
            }
            for probability in &mut cdf[value..symbol_count_minus_one] {
                *probability = probability.wrapping_sub(*probability >> rate);
            }
            cdf[symbol_count_minus_one] = count.wrapping_add(u16::from(count < 32));
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the decoded symbol is bounded to at most fifteen"
        )]
        let value = value as u32;
        #[cfg(coverage)]
        self.record_operation(
            "adaptive_symbol",
            i32::try_from(symbol_count_minus_one).unwrap_or(i32::MAX),
            i32::try_from(value).unwrap_or(i32::MAX),
            &cdf[..=symbol_count_minus_one],
        );
        value
    }

    // ✅ VERIFIED: dav1d 1.5.3 src/msac.c:168-185
    // (`dav1d_msac_decode_bool_adapt_c`).
    pub(super) fn adaptive_bool(&mut self, cdf: &mut [u16; 2]) -> bool {
        let bit = self.fixed(u32::from(cdf[0]));
        if self.allow_update_cdf {
            let count = cdf[1];
            let rate = 4_u32.wrapping_add(u32::from(count >> 4));
            if bit {
                let increase = 32_768_u32.wrapping_sub(u32::from(cdf[0])) >> rate;
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a valid AV1 CDF update increases a u16 probability by at most 2048"
                )]
                let increase = increase as u16;
                cdf[0] = cdf[0].wrapping_add(increase);
            } else {
                cdf[0] = cdf[0].wrapping_sub(cdf[0] >> rate);
            }
            cdf[1] = count.wrapping_add(u16::from(count < 32));
        }
        #[cfg(coverage)]
        self.record_operation("adaptive_bool", 1, i32::from(bit), cdf);
        bit
    }

    // ✅ VERIFIED: dav1d 1.5.3 src/msac.c:187-201
    // (`dav1d_msac_decode_hi_tok_c`).
    pub(super) fn high_token(&mut self, cdf: &mut [u16; 4]) -> u32 {
        let mut branch = self.adaptive_symbol(cdf, 3);
        let mut token = 3_u32.wrapping_add(branch);
        if branch == 3 {
            branch = self.adaptive_symbol(cdf, 3);
            token = 6_u32.wrapping_add(branch);
            if branch == 3 {
                branch = self.adaptive_symbol(cdf, 3);
                token = 9_u32.wrapping_add(branch);
                if branch == 3 {
                    token = 12_u32.wrapping_add(self.adaptive_symbol(cdf, 3));
                }
            }
        }
        #[cfg(coverage)]
        self.record_operation(
            "high_token",
            3,
            i32::try_from(token).unwrap_or(i32::MAX),
            cdf,
        );
        token
    }

    // ✅ VERIFIED: dav1d 1.5.3 src/msac.h:94-99
    // (`dav1d_msac_decode_bools`).
    pub(super) fn bits(&mut self, count: u32) -> u32 {
        let mut value = 0_u32;
        for _ in 0..count {
            value = value.wrapping_shl(1).wrapping_add(u32::from(self.equal()));
        }
        value
    }

    // ✅ VERIFIED: dav1d 1.5.3 src/msac.h:101-108
    // (`dav1d_msac_decode_uniform`).
    #[cfg(coverage)]
    pub(super) fn uniform(&mut self, count: u32) -> u32 {
        let bit_width = u32::BITS.wrapping_sub(count.leading_zeros());
        let boundary = (1_u64 << bit_width).wrapping_sub(u64::from(count));
        let value = u64::from(self.bits(bit_width.wrapping_sub(1)));
        let decoded = if value < boundary {
            value
        } else {
            value
                .wrapping_shl(1)
                .wrapping_sub(boundary)
                .wrapping_add(u64::from(self.equal()))
        };
        #[expect(
            clippy::cast_possible_truncation,
            reason = "uniform decoding returns a value strictly below its u32 count"
        )]
        let decoded = decoded as u32;
        decoded
    }

    // ✅ VERIFIED: dav1d 1.5.3 src/msac.c:60-74
    // (`dav1d_msac_decode_subexp`) and include/common/intops.h:75-81.
    pub(super) fn subexponential(&mut self, reference: i32, count: i32, mut bit_width: u32) -> i32 {
        // All call sites use dav1d's valid `(reference, count, bit_width)`
        // tuples where `count >> bit_width == 8`.
        let mut offset = 0_u32;
        if self.equal() {
            if self.equal() {
                bit_width = bit_width
                    .wrapping_add(u32::from(self.equal()))
                    .wrapping_add(1);
            }
            offset = 1_u32.wrapping_shl(bit_width);
        }
        let value = self.bits(bit_width).wrapping_add(offset);
        let reference = reference.unsigned_abs();
        let count = count.unsigned_abs();
        let decoded = if reference.wrapping_mul(2) <= count {
            inverse_recenter(reference, value)
        } else {
            let maximum = count.wrapping_sub(1);
            maximum.wrapping_sub(inverse_recenter(maximum.wrapping_sub(reference), value))
        };
        #[expect(
            clippy::cast_possible_wrap,
            reason = "subexponential output is strictly below its positive i32 count"
        )]
        let decoded = decoded as i32;
        decoded
    }

    #[cfg(coverage)]
    fn trace_state(
        &self,
        case: &'static str,
        step: u32,
        value: i32,
        cdf: &[u16],
    ) -> crate::Av1EntropyTraceState {
        crate::Av1EntropyTraceState {
            case,
            step,
            value,
            byte_position: self.position.saturating_sub(self.start),
            difference: self.difference,
            range: self.range,
            count: self.count,
            cdf: cdf.to_vec(),
        }
    }
}

fn inverse_recenter(reference: u32, value: u32) -> u32 {
    if value > reference.wrapping_mul(2) {
        value
    } else if value.is_multiple_of(2) {
        reference.wrapping_add(value >> 1)
    } else {
        reference.wrapping_sub(value.wrapping_add(1) >> 1)
    }
}

/// Active loop-restoration mode declared by an AV1 frame header.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RestorationType {
    Switchable,
    Wiener,
    SgrProjection,
}

impl RestorationType {
    /// Convert the two-bit AV1 frame-restoration syntax into an active mode.
    pub(super) const fn from_bits(value: u32) -> Option<Self> {
        match value {
            0 => None,
            1 => Some(Self::Switchable),
            2 => Some(Self::Wiener),
            _ => Some(Self::SgrProjection),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct QuantizationContext {
    pub(super) base: u32,
    pub(super) y_dc_delta: i32,
    pub(super) u_dc_delta: i32,
    pub(super) u_ac_delta: i32,
    pub(super) v_dc_delta: i32,
    pub(super) v_ac_delta: i32,
    pub(super) different_uv_delta: bool,
    pub(super) using_matrix: bool,
    pub(super) matrix_y: u32,
    pub(super) matrix_u: u32,
    pub(super) matrix_v: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct LoopFilterContext {
    pub(super) level_y: [u32; 2],
    pub(super) level_u: u32,
    pub(super) level_v: u32,
    pub(super) sharpness: u32,
    pub(super) delta_enabled: bool,
    pub(super) delta_update: bool,
    pub(super) reference_deltas: [i32; 8],
    pub(super) mode_deltas: [i32; 2],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct CdefContext {
    pub(super) damping: u32,
    pub(super) bits: u32,
    pub(super) y_strength_count: usize,
    pub(super) uv_strength_count: usize,
    pub(super) first_y_strength: Option<u32>,
    pub(super) first_uv_strength: Option<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct FrameToolsContext {
    pub(super) quantization: Option<QuantizationContext>,
    pub(super) segment_qindex: u32,
    pub(super) segment_lossless: bool,
    pub(super) delta_q_present: bool,
    pub(super) delta_q_resolution_log2: u32,
    pub(super) delta_lf_present: bool,
    pub(super) delta_lf_resolution_log2: u32,
    pub(super) delta_lf_multi: bool,
    pub(super) loop_filter: LoopFilterContext,
    pub(super) cdef: Option<CdefContext>,
    pub(super) restoration_present: bool,
    pub(super) transform_mode: u32,
    pub(super) reduced_transform_set: bool,
    pub(super) film_grain_present: bool,
}

/// Codec state needed before the first block in one tile.
pub(super) struct FirstBlockContext {
    pub(super) disable_cdf_update: bool,
    pub(super) level: u32,
    pub(super) block_width: u32,
    pub(super) block_height: u32,
    pub(super) block_x: u32,
    pub(super) block_y: u32,
    pub(super) frame_width: u32,
    pub(super) frame_height: u32,
    pub(super) upscaled_width: u32,
    pub(super) superres_enabled: bool,
    pub(super) monochrome: bool,
    pub(super) subsampling_x: bool,
    pub(super) subsampling_y: bool,
    pub(super) restoration_types: [Option<RestorationType>; 3],
    pub(super) restoration_unit_size_log2: [u32; 2],
    pub(super) bit_depth: u32,
    pub(super) all_lossless: bool,
    pub(super) segmentation_enabled: bool,
    pub(super) skip_mode_enabled: bool,
    pub(super) allow_intrabc: bool,
    pub(super) allow_screen_content_tools: bool,
    pub(super) enable_filter_intra: bool,
    pub(super) frame_tools: FrameToolsContext,
}

#[derive(Clone, Copy)]
struct RestorationReference {
    filter_vertical: [i32; 3],
    filter_horizontal: [i32; 3],
    sgr_weights: [i32; 2],
}

impl RestorationReference {
    const fn defaults() -> Self {
        Self {
            filter_vertical: [3, -7, 15],
            filter_horizontal: [3, -7, 15],
            sgr_weights: [-32, 31],
        }
    }
}

struct RestorationCdfs {
    switchable: [u16; 3],
    wiener: [u16; 2],
    sgr_projection: [u16; 2],
}

#[derive(Clone, Copy)]
enum RestorationUnitType {
    None,
    Wiener,
    SgrProjection,
}

impl RestorationCdfs {
    const fn defaults() -> Self {
        Self {
            switchable: [23_355, 10_187, 0],
            wiener: [21_198, 0],
            sgr_projection: [15_913, 0],
        }
    }
}

const SGR_PARAMETER_ACTIVITY: [[bool; 2]; 16] = [
    [true, true],
    [true, true],
    [true, true],
    [true, true],
    [true, true],
    [true, true],
    [true, true],
    [true, true],
    [true, true],
    [true, true],
    [false, true],
    [false, true],
    [false, true],
    [false, true],
    [true, false],
    [true, false],
];

// ✅ VERIFIED: dav1d 1.5.3 src/decode.c:2511-2578
// (`read_restoration_info`) and src/cdf.c:451-458.
fn decode_restoration_unit(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut RestorationCdfs,
    reference: &mut RestorationReference,
    plane: usize,
    frame_type: RestorationType,
) {
    let unit_type = match frame_type {
        RestorationType::Switchable => match decoder.adaptive_symbol(&mut cdfs.switchable, 2) {
            0 => RestorationUnitType::None,
            1 => RestorationUnitType::Wiener,
            _ => RestorationUnitType::SgrProjection,
        },
        RestorationType::Wiener => {
            if decoder.adaptive_bool(&mut cdfs.wiener) {
                RestorationUnitType::Wiener
            } else {
                RestorationUnitType::None
            }
        }
        RestorationType::SgrProjection => {
            if decoder.adaptive_bool(&mut cdfs.sgr_projection) {
                RestorationUnitType::SgrProjection
            } else {
                RestorationUnitType::None
            }
        }
    };

    match unit_type {
        RestorationUnitType::None => {}
        RestorationUnitType::Wiener => {
            let vertical_zero = if plane == 0 {
                decoder
                    .subexponential(reference.filter_vertical[0].wrapping_add(5), 16, 1)
                    .wrapping_sub(5)
            } else {
                0
            };
            let vertical_one = decoder
                .subexponential(reference.filter_vertical[1].wrapping_add(23), 32, 2)
                .wrapping_sub(23);
            let vertical_two = decoder
                .subexponential(reference.filter_vertical[2].wrapping_add(17), 64, 3)
                .wrapping_sub(17);
            let horizontal_zero = if plane == 0 {
                decoder
                    .subexponential(reference.filter_horizontal[0].wrapping_add(5), 16, 1)
                    .wrapping_sub(5)
            } else {
                0
            };
            let horizontal_one = decoder
                .subexponential(reference.filter_horizontal[1].wrapping_add(23), 32, 2)
                .wrapping_sub(23);
            let horizontal_two = decoder
                .subexponential(reference.filter_horizontal[2].wrapping_add(17), 64, 3)
                .wrapping_sub(17);
            reference.filter_vertical = [vertical_zero, vertical_one, vertical_two];
            reference.filter_horizontal = [horizontal_zero, horizontal_one, horizontal_two];
        }
        RestorationUnitType::SgrProjection => {
            let parameter_index = decoder.bits(4) as usize;
            let activity = SGR_PARAMETER_ACTIVITY[parameter_index];
            let first = if activity[0] {
                decoder
                    .subexponential(reference.sgr_weights[0].wrapping_add(96), 128, 4)
                    .wrapping_sub(96)
            } else {
                0
            };
            let second = if activity[1] {
                decoder
                    .subexponential(reference.sgr_weights[1].wrapping_add(32), 128, 4)
                    .wrapping_sub(32)
            } else {
                95
            };
            reference.sgr_weights = [first, second];
        }
    }
}

fn restoration_unit_starts_at_first_block(
    context: &FirstBlockContext,
    plane: usize,
) -> Option<bool> {
    let chroma = plane != 0;
    let horizontal_shift = u32::from(chroma && context.subsampling_x);
    let vertical_shift = u32::from(chroma && context.subsampling_y);
    let unit_size_log2 = context.restoration_unit_size_log2[usize::from(chroma)];
    // Parsed AV1 restoration-unit exponents are 5..=8. The wrapping forms
    // encode that parser invariant without introducing unreachable failures
    // into this private syntax helper.
    let unit_size = 1_u32.wrapping_shl(unit_size_log2);
    let mask = unit_size.wrapping_sub(1);
    let y = context.block_y.wrapping_mul(4).wrapping_shr(vertical_shift);
    let height = context
        .frame_height
        .wrapping_add(vertical_shift)
        .wrapping_shr(vertical_shift);
    // An aligned `y` leaves at least `mask` representable values, so adding
    // half a restoration unit cannot wrap.
    if y & mask != 0 || (y != 0 && y.wrapping_add(unit_size >> 1) > height) {
        return Some(false);
    }
    if context.frame_width != context.upscaled_width {
        // The super-resolution path may cover more than one restoration unit
        // before the first partition and is implemented with reconstruction.
        return None;
    }
    let x = context
        .block_x
        .wrapping_mul(4)
        .wrapping_shr(horizontal_shift);
    let width = context
        .frame_width
        .wrapping_add(horizontal_shift)
        .wrapping_shr(horizontal_shift);
    // The same alignment invariant proves this half-unit addition is exact.
    Some(x & mask == 0 && (x == 0 || x.wrapping_add(unit_size >> 1) <= width))
}

fn decode_restoration_prefix(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    context: &FirstBlockContext,
) -> Option<()> {
    let mut cdfs = RestorationCdfs::defaults();
    let mut references = [RestorationReference::defaults(); 3];
    let plane_count = if context.monochrome { 1 } else { 3 };
    for (plane, reference) in references.iter_mut().enumerate().take(plane_count) {
        let Some(restoration_type) = context.restoration_types[plane] else {
            continue;
        };
        if restoration_unit_starts_at_first_block(context, plane)? {
            decode_restoration_unit(decoder, &mut cdfs, reference, plane, restoration_type);
        }
    }
    Some(())
}

// ✅ VERIFIED: dav1d 1.5.3 src/cdf.c:386-433. The values are dav1d's inverse
// partition CDF for context zero, including the mutable count slot.
const fn square8_partition_cdf() -> ([u16; 10], usize) {
    ([13_636, 7258, 2376, 0, 0, 0, 0, 0, 0, 0], 3)
}

fn default_partition_cdf(level: u32) -> Option<([u16; 10], usize)> {
    match level {
        0 => Some(([4869, 4549, 4239, 284, 229, 149, 129, 0, 0, 0], 7)),
        1 => Some((
            [12_631, 11_221, 9690, 3202, 2931, 2507, 2244, 1876, 1044, 0],
            9,
        )),
        2 => Some((
            [14_306, 11_848, 9644, 5121, 4541, 3719, 3249, 2590, 1224, 0],
            9,
        )),
        3 => Some((
            [17_171, 11_839, 8197, 6062, 5104, 3947, 3167, 2197, 866, 0],
            9,
        )),
        4 => Some(square8_partition_cdf()),
        _ => None,
    }
}

// ✅ VERIFIED: dav1d 1.5.3 src/env.h:93-121.
fn left_partition_probability(cdf: &[u16; 10], level: u32) -> u32 {
    let mut probability = u32::from(cdf[0])
        .wrapping_sub(u32::from(cdf[1]))
        .wrapping_add(u32::from(cdf[2]).wrapping_sub(u32::from(cdf[6])));
    if level != 0 {
        probability = probability.wrapping_add(u32::from(cdf[7]).wrapping_sub(u32::from(cdf[8])));
    }
    probability
}

// ✅ VERIFIED: dav1d 1.5.3 src/env.h:103-121.
fn top_partition_probability(cdf: &[u16; 10], level: u32) -> u32 {
    let mut probability = u32::from(cdf[1])
        .wrapping_sub(u32::from(cdf[4]))
        .wrapping_add(u32::from(cdf[5]));
    if level != 0 {
        probability = probability.wrapping_add(u32::from(cdf[8]).wrapping_sub(u32::from(cdf[7])));
    }
    probability
}

fn closed_leaf_dimensions(context: &FirstBlockContext) -> bool {
    matches!(
        (context.frame_width, context.frame_height),
        (4, 4) | (4, 8) | (8, 4) | (8, 8) | (12, 12) | (12, 16) | (16, 12) | (16, 16)
    )
}

fn rectangular_leaf_dimensions(context: &FirstBlockContext) -> bool {
    matches!(
        (context.frame_width, context.frame_height),
        (12, 4) | (12, 8) | (16, 4) | (16, 8) | (4, 12) | (8, 12) | (4, 16) | (8, 16)
    )
}

fn closed_leaf_level_dimensions(context: &FirstBlockContext, level: u32) -> bool {
    matches!(
        (level, context.frame_width, context.frame_height),
        (4, 4, 4)
            | (4, 4, 8)
            | (4, 8, 4)
            | (4, 8, 8)
            | (3, 12, 12)
            | (3, 12, 16)
            | (3, 16, 12)
            | (3, 16, 16)
    )
}

fn recursive_split_dimensions(context: &FirstBlockContext) -> bool {
    matches!(
        (context.frame_width, context.frame_height),
        (12, 4) | (16, 4) | (12, 8) | (16, 8) | (4, 12) | (4, 16) | (8, 12) | (8, 16)
    )
}

fn square_recursive_split_dimensions(context: &FirstBlockContext) -> bool {
    matches!(
        (context.frame_width, context.frame_height),
        (12, 12) | (16, 16)
    )
}

fn closed_base_reconstruction_context(context: &FirstBlockContext) -> bool {
    (context.bit_depth == 8)
        & !context.superres_enabled
        & !context.segmentation_enabled
        & !context.skip_mode_enabled
        & !context.allow_intrabc
        & !context.monochrome
        & !context.frame_tools.film_grain_present
        & (context.block_x == 0)
        & (context.block_y == 0)
}

fn closed_reconstruction_context(context: &FirstBlockContext) -> bool {
    closed_base_reconstruction_context(context) & context.all_lossless
}

fn closed_444_reconstruction_context(context: &FirstBlockContext) -> bool {
    closed_reconstruction_context(context) & !context.subsampling_x & !context.subsampling_y
}

fn closed_420_reconstruction_context(context: &FirstBlockContext) -> bool {
    closed_reconstruction_context(context)
        & context.subsampling_x
        & context.subsampling_y
        & (closed_leaf_dimensions(context) | rectangular_leaf_dimensions(context))
}

const CLOSED_LOSSY_420_FRAME_TOOLS: FrameToolsContext = FrameToolsContext {
    quantization: Some(QuantizationContext {
        base: 4,
        y_dc_delta: 0,
        u_dc_delta: 0,
        u_ac_delta: 0,
        v_dc_delta: 0,
        v_ac_delta: 0,
        different_uv_delta: false,
        using_matrix: true,
        matrix_y: 10,
        matrix_u: 10,
        matrix_v: 10,
    }),
    segment_qindex: 4,
    segment_lossless: false,
    delta_q_present: true,
    delta_q_resolution_log2: 0,
    delta_lf_present: false,
    delta_lf_resolution_log2: 0,
    delta_lf_multi: false,
    loop_filter: LoopFilterContext {
        level_y: [0; 2],
        level_u: 0,
        level_v: 0,
        sharpness: 7,
        delta_enabled: true,
        delta_update: false,
        reference_deltas: [1, 0, 0, 0, -1, 0, -1, -1],
        mode_deltas: [0; 2],
    },
    cdef: Some(CdefContext {
        damping: 4,
        bits: 0,
        y_strength_count: 1,
        uv_strength_count: 1,
        first_y_strength: Some(0),
        first_uv_strength: Some(0),
    }),
    restoration_present: false,
    transform_mode: 1,
    reduced_transform_set: false,
    film_grain_present: false,
};

fn closed_lossy_420_reconstruction_context(context: &FirstBlockContext) -> bool {
    closed_base_reconstruction_context(context)
        & !context.all_lossless
        & context.subsampling_x
        & context.subsampling_y
        & matches!((context.frame_width, context.frame_height), (4, 4) | (8, 8))
        & !context.disable_cdf_update
        & !context.allow_screen_content_tools
        & context.enable_filter_intra
        & (context.restoration_types == [None; 3])
        & (context.restoration_unit_size_log2 == [8; 2])
        & (context.frame_tools == CLOSED_LOSSY_420_FRAME_TOOLS)
}

fn decode_closed_leaf(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    context: &FirstBlockContext,
    transform_grid: super::block::TransformGrid,
) -> Option<super::block::FirstLeaf> {
    let reconstructed = super::block::decode_first_lossless_444_leaf(
        decoder,
        context.frame_width,
        context.frame_height,
        transform_grid,
        super::block::BlockTools {
            allow_screen_content_tools: context.allow_screen_content_tools,
            enable_filter_intra: context.enable_filter_intra,
        },
    );
    finish_closed_leaf(decoder, reconstructed)
}

fn decode_closed_420_leaf(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    context: &FirstBlockContext,
    transform_grid: super::block::TransformGrid,
) -> Option<super::block::FirstLeaf> {
    let reconstructed = super::block::decode_first_lossless_420_leaf(
        decoder,
        context.frame_width,
        context.frame_height,
        transform_grid,
        super::block::BlockTools {
            allow_screen_content_tools: context.allow_screen_content_tools,
            enable_filter_intra: context.enable_filter_intra,
        },
    );
    finish_closed_leaf(decoder, reconstructed)
}

fn decode_closed_lossy_420_leaf(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    context: &FirstBlockContext,
) -> Option<super::block::FirstLeaf> {
    let reconstructed = super::block::decode_first_lossy_420_leaf(
        decoder,
        context.frame_width,
        context.frame_height,
        context.frame_tools.segment_qindex,
        context.frame_tools.delta_q_resolution_log2,
        super::block::BlockTools {
            allow_screen_content_tools: context.allow_screen_content_tools,
            enable_filter_intra: context.enable_filter_intra,
        },
    );
    finish_closed_leaf(decoder, reconstructed)
}

fn finish_closed_leaf(
    _decoder: &RangeDecoder<'_, '_, '_>,
    reconstructed: Option<super::block::FirstLeaf>,
) -> Option<super::block::FirstLeaf> {
    #[cfg(coverage)]
    let reconstructed = reconstructed.map(|mut leaf| {
        leaf.entropy_operations = _decoder.operation_trace();
        leaf
    });
    reconstructed
}

/// Decode the first real partition syntax element from one tile.
///
/// `block_width` and `block_height` use dav1d's padded four-pixel units;
/// `block_x` and `block_y` are the tile's first superblock in those units.
pub(super) fn validate_first_partition(
    data: &SegmentedData<'_, '_>,
    range: Range<usize>,
    context: &FirstBlockContext,
) -> Option<Option<super::block::FirstLeaf>> {
    let mut decoder = RangeDecoder::new(data, range.start, range.end, context.disable_cdf_update)?;
    #[cfg(coverage)]
    {
        let trace_closed_context = (closed_444_reconstruction_context(context)
            & (closed_leaf_dimensions(context) | rectangular_leaf_dimensions(context)))
            | closed_420_reconstruction_context(context)
            | closed_lossy_420_reconstruction_context(context);
        if trace_closed_context {
            decoder.enable_operation_trace();
        }
    }
    decode_restoration_prefix(&mut decoder, context)?;
    let mut level = context.level;
    loop {
        // Parsed root levels are 0 or 1. Later iterations are capped below, so
        // neither the shift nor these four-pixel-unit additions can overflow.
        let half_size = 16_u32.wrapping_shr(level);
        let horizontal_split = context.block_width > context.block_x.wrapping_add(half_size);
        let vertical_split = context.block_height > context.block_y.wrapping_add(half_size);
        if horizontal_split || vertical_split {
            let (mut cdf, symbol_count_minus_one) = default_partition_cdf(level)?;
            if horizontal_split && vertical_split {
                let partition = decoder.adaptive_symbol(&mut cdf, symbol_count_minus_one);
                if !context.monochrome
                    && context.subsampling_x
                    && !context.subsampling_y
                    && matches!(partition, 2 | 6 | 7 | 9)
                {
                    return None;
                }
                let reconstruct_closed_leaf = (partition == 0)
                    & closed_444_reconstruction_context(context)
                    & closed_leaf_dimensions(context)
                    & closed_leaf_level_dimensions(context, level);
                if reconstruct_closed_leaf {
                    // Unsupported syntax ends this deliberately narrow
                    // portable attempt without narrowing the native fallback.
                    // The accepted level/dimension pairs above prove a 2x2
                    // transform grid at level 4 and a 4x4 grid at level 3.
                    let transform_grid = if level == 4 {
                        super::block::TransformGrid::Square8
                    } else {
                        super::block::TransformGrid::Square16
                    };
                    return Some(decode_closed_leaf(&mut decoder, context, transform_grid));
                }
                let reconstruct_closed_420_leaf = (partition == 0)
                    & closed_420_reconstruction_context(context)
                    & closed_leaf_dimensions(context)
                    & closed_leaf_level_dimensions(context, level);
                if reconstruct_closed_420_leaf {
                    let transform_grid = if level == 4 {
                        super::block::TransformGrid::Square8
                    } else {
                        super::block::TransformGrid::Square16
                    };
                    return Some(decode_closed_420_leaf(
                        &mut decoder,
                        context,
                        transform_grid,
                    ));
                }
                let reconstruct_closed_lossy_420_leaf = (partition == 0)
                    & (level == 4)
                    & closed_lossy_420_reconstruction_context(context);
                if reconstruct_closed_lossy_420_leaf {
                    return Some(decode_closed_lossy_420_leaf(&mut decoder, context));
                }
                let reconstruct_square_split = (partition == 3)
                    & closed_444_reconstruction_context(context)
                    & (level == 3)
                    & square_recursive_split_dimensions(context);
                if reconstruct_square_split {
                    // ✅ VERIFIED: dav1d 1.5.3 src/decode.c:2117-2380 and
                    // the pinned Slice 18 scalar traces. The four level-4
                    // child symbols are interleaved with their leaf syntax and
                    // mutate one shared partition CDF.
                    let (mut child_cdf, child_symbol_count_minus_one) = square8_partition_cdf();
                    if decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one) != 0 {
                        return Some(None);
                    }
                    let reconstructed = super::block::decode_four_lossless_444_leaves(
                        &mut decoder,
                        context.frame_width,
                        context.frame_height,
                        super::block::BlockTools {
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                        },
                        |decoder| {
                            (decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one)
                                == 0)
                                .then_some(())
                        },
                    );
                    return Some(finish_closed_leaf(&decoder, reconstructed));
                }
                let reconstruct_420_square_split = (partition == 3)
                    & closed_420_reconstruction_context(context)
                    & (level == 3)
                    & matches!((context.frame_width, context.frame_height), (16, 16));
                if reconstruct_420_square_split {
                    // ✅ VERIFIED: dav1d 1.5.3 src/decode.c:2117-2380 and
                    // the pinned Slice 33 scalar traces. The four 8x8 luma
                    // children carry matching 4x4 4:2:0 chroma children and
                    // mutate one shared partition CDF between leaf payloads.
                    let (mut child_cdf, child_symbol_count_minus_one) = square8_partition_cdf();
                    if decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one) != 0 {
                        return Some(None);
                    }
                    let reconstructed = super::block::decode_four_lossless_420_leaves(
                        &mut decoder,
                        context.frame_width,
                        context.frame_height,
                        super::block::BlockTools {
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                        },
                        |decoder| {
                            (decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one)
                                == 0)
                                .then_some(())
                        },
                    );
                    return Some(finish_closed_leaf(&decoder, reconstructed));
                }
            } else {
                let probability = if horizontal_split {
                    top_partition_probability(&cdf, level)
                } else {
                    left_partition_probability(&cdf, level)
                };
                let split = decoder.fixed(probability);
                if !context.monochrome
                    && context.subsampling_x
                    && !context.subsampling_y
                    && vertical_split
                    && !split
                {
                    return None;
                }
                let reconstruct_rectangular_leaf = !split
                    & closed_444_reconstruction_context(context)
                    & (level == 3)
                    & rectangular_leaf_dimensions(context);
                if reconstruct_rectangular_leaf {
                    let transform_grid = if horizontal_split {
                        super::block::TransformGrid::Horizontal16x8
                    } else {
                        super::block::TransformGrid::Vertical8x16
                    };
                    return Some(decode_closed_leaf(&mut decoder, context, transform_grid));
                }
                let reconstruct_420_rectangular_leaf = !split
                    & closed_420_reconstruction_context(context)
                    & (level == 3)
                    & rectangular_leaf_dimensions(context);
                if reconstruct_420_rectangular_leaf {
                    let transform_grid = if horizontal_split {
                        super::block::TransformGrid::Horizontal16x8
                    } else {
                        super::block::TransformGrid::Vertical8x16
                    };
                    return Some(decode_closed_420_leaf(
                        &mut decoder,
                        context,
                        transform_grid,
                    ));
                }
                let reconstruct_recursive_split = split
                    & closed_444_reconstruction_context(context)
                    & (level == 3)
                    & recursive_split_dimensions(context);
                if reconstruct_recursive_split {
                    // ✅ VERIFIED: dav1d 1.5.3 src/decode.c:2117-2380 and
                    // the pinned Slice 15 scalar traces. Both 8x8 children
                    // decode PARTITION_NONE through one shared level-4 CDF,
                    // with the second symbol occurring after the first leaf.
                    let (mut child_cdf, child_symbol_count_minus_one) = square8_partition_cdf();
                    if decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one) != 0 {
                        return Some(None);
                    }
                    let orientation = if horizontal_split {
                        super::block::SplitOrientation::Horizontal
                    } else {
                        super::block::SplitOrientation::Vertical
                    };
                    let reconstructed = super::block::decode_two_lossless_444_leaves(
                        &mut decoder,
                        context.frame_width,
                        context.frame_height,
                        orientation,
                        super::block::BlockTools {
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                        },
                        |decoder| {
                            (decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one)
                                == 0)
                                .then_some(())
                        },
                    );
                    return Some(finish_closed_leaf(&decoder, reconstructed));
                }
                let reconstruct_420_recursive_split = split
                    & closed_420_reconstruction_context(context)
                    & (level == 3)
                    & recursive_split_dimensions(context);
                if reconstruct_420_recursive_split {
                    let (mut child_cdf, child_symbol_count_minus_one) = square8_partition_cdf();
                    if decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one) != 0 {
                        return Some(None);
                    }
                    let orientation = if horizontal_split {
                        super::block::SplitOrientation::Horizontal
                    } else {
                        super::block::SplitOrientation::Vertical
                    };
                    let reconstructed = super::block::decode_two_lossless_420_leaves(
                        &mut decoder,
                        context.frame_width,
                        context.frame_height,
                        orientation,
                        super::block::BlockTools {
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                        },
                        |decoder| {
                            (decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one)
                                == 0)
                                .then_some(())
                        },
                    );
                    return Some(finish_closed_leaf(&decoder, reconstructed));
                }
            }
            return Some(None);
        }
        level = level.wrapping_add(1);
        if level > 4 {
            return None;
        }
    }
}

#[cfg(coverage)]
#[coverage(off)]
pub(super) fn reference_trace() -> Result<Vec<crate::Av1EntropyTraceState>, &'static str> {
    const INPUT: [u8; 32] = [
        0x00, 0xff, 0x81, 0x7e, 0x55, 0xaa, 0x13, 0xec, 0x42, 0xbd, 0x99, 0x66, 0x01, 0x80, 0xfe,
        0x24, 0xdb, 0x10, 0xef, 0x73, 0x8c, 0x31, 0xce, 0x5a, 0xa5, 0x0f, 0xf0, 0x69, 0x96, 0x3c,
        0xc3, 0x7f,
    ];
    let spans = [super::super::samples::ByteSpan {
        start: 0,
        end: INPUT.len(),
    }];
    let data = SegmentedData::new(&INPUT, &spans).ok_or("segmented input")?;
    let mut records = Vec::with_capacity(103);

    let mut decoder =
        RangeDecoder::new(&data, 0, INPUT.len(), true).ok_or("equal initialization")?;
    records.push(decoder.trace_state("equal", 0, -1, &[]));
    for step in 1..=16 {
        let value = i32::from(decoder.equal());
        records.push(decoder.trace_state("equal", step, value, &[]));
    }

    let mut decoder =
        RangeDecoder::new(&data, 0, INPUT.len(), true).ok_or("fixed initialization")?;
    records.push(decoder.trace_state("fixed", 0, -1, &[]));
    for (index, probability) in [0, 1, 4096, 8192, 16_384, 24_576, 32_767]
        .into_iter()
        .enumerate()
    {
        let value = i32::from(decoder.fixed(probability));
        let step = u32::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or("fixed step")?;
        records.push(decoder.trace_state("fixed", step, value, &[]));
    }

    let mut cdf = [16_384, 0];
    let mut decoder =
        RangeDecoder::new(&data, 0, INPUT.len(), false).ok_or("adaptive bool initialization")?;
    records.push(decoder.trace_state("adaptive_bool", 0, -1, &cdf));
    for step in 1..=16 {
        let value = i32::from(decoder.adaptive_bool(&mut cdf));
        records.push(decoder.trace_state("adaptive_bool", step, value, &cdf));
    }

    let mut cdf = [24_576, 16_384, 8192, 0];
    let mut decoder =
        RangeDecoder::new(&data, 0, INPUT.len(), false).ok_or("adaptive symbol initialization")?;
    records.push(decoder.trace_state("adaptive_symbol", 0, -1, &cdf));
    for step in 1..=16 {
        let value = i32::try_from(decoder.adaptive_symbol(&mut cdf, 3))
            .map_err(|_| "adaptive symbol value")?;
        records.push(decoder.trace_state("adaptive_symbol", step, value, &cdf));
    }

    let mut cdf = [24_576, 16_384, 8192, 0];
    let mut decoder =
        RangeDecoder::new(&data, 0, INPUT.len(), true).ok_or("frozen symbol initialization")?;
    records.push(decoder.trace_state("frozen_symbol", 0, -1, &cdf));
    for step in 1..=8 {
        let value = i32::try_from(decoder.adaptive_symbol(&mut cdf, 3))
            .map_err(|_| "frozen symbol value")?;
        records.push(decoder.trace_state("frozen_symbol", step, value, &cdf));
    }

    let mut cdf = [24_576, 16_384, 8192, 0];
    let mut decoder =
        RangeDecoder::new(&data, 0, INPUT.len(), false).ok_or("high token initialization")?;
    records.push(decoder.trace_state("high_token", 0, -1, &cdf));
    for step in 1..=8 {
        let value = i32::try_from(decoder.high_token(&mut cdf)).map_err(|_| "high token value")?;
        records.push(decoder.trace_state("high_token", step, value, &cdf));
    }

    let mut decoder =
        RangeDecoder::new(&data, 0, INPUT.len(), true).ok_or("uniform initialization")?;
    records.push(decoder.trace_state("uniform", 0, -1, &[]));
    for (index, count) in [2, 3, 5, 17, 255].into_iter().enumerate() {
        let value = i32::try_from(decoder.uniform(count)).map_err(|_| "uniform value")?;
        let step = u32::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or("uniform step")?;
        records.push(decoder.trace_state("uniform", step, value, &[]));
    }

    let mut decoder =
        RangeDecoder::new(&data, 0, INPUT.len(), true).ok_or("subexponential initialization")?;
    records.push(decoder.trace_state("subexponential", 0, -1, &[]));
    for (index, reference) in [0, 63, 127, 200].into_iter().enumerate() {
        let value = decoder.subexponential(reference, 256, 5);
        let step = u32::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or("subexponential step")?;
        records.push(decoder.trace_state("subexponential", step, value, &[]));
    }

    const PARTITION_STILL: [u8; 14] = [
        0x00, 0xe2, 0x34, 0xfe, 0x35, 0xf6, 0xba, 0x40, 0x26, 0xa9, 0xe0, 0xb7, 0x7e, 0x80,
    ];
    const PARTITION_FRAME_2: [u8; 13] = [
        0x0a, 0x05, 0x77, 0x97, 0xa7, 0xa0, 0x58, 0x37, 0xfe, 0xb1, 0x1c, 0x88, 0x87,
    ];
    for (case, input) in [
        ("partition_422_still", PARTITION_STILL.as_slice()),
        ("partition_422_frame_2", PARTITION_FRAME_2.as_slice()),
    ] {
        let spans = [super::super::samples::ByteSpan {
            start: 0,
            end: input.len(),
        }];
        let data = SegmentedData::new(input, &spans).ok_or("partition segmented input")?;
        let mut decoder =
            RangeDecoder::new(&data, 0, input.len(), false).ok_or("partition initialization")?;
        let (mut cdf, symbol_count_minus_one) = default_partition_cdf(1).ok_or("partition CDF")?;
        records.push(decoder.trace_state(case, 0, -1, &cdf));
        let value = i32::try_from(decoder.adaptive_symbol(&mut cdf, symbol_count_minus_one))
            .map_err(|_| "partition value")?;
        records.push(decoder.trace_state(case, 1, value, &cdf));
    }

    const RESTORATION_FRAME_3: [u8; 77] = [
        0xf8, 0x3f, 0x9f, 0xfd, 0x73, 0xc0, 0x2f, 0xa5, 0x59, 0x48, 0xfa, 0xc5, 0xe5, 0x74, 0x87,
        0x85, 0xca, 0xc6, 0x00, 0x81, 0x5d, 0xa5, 0x3a, 0x6e, 0xfa, 0xf3, 0x7c, 0x24, 0x18, 0x0b,
        0xfc, 0x69, 0x2c, 0x41, 0x07, 0x3b, 0x72, 0x2e, 0xcf, 0xff, 0xb0, 0x2a, 0x3b, 0x55, 0x45,
        0x22, 0x47, 0xbb, 0x8c, 0x3c, 0x03, 0xb2, 0x19, 0xe9, 0xdf, 0x68, 0xca, 0xf0, 0x15, 0x6e,
        0xc0, 0xe7, 0x9d, 0x21, 0xff, 0x54, 0xf6, 0xce, 0x30, 0x93, 0x63, 0x6f, 0x59, 0x97, 0x89,
        0xba, 0x72,
    ];
    let spans = [super::super::samples::ByteSpan {
        start: 0,
        end: RESTORATION_FRAME_3.len(),
    }];
    let data =
        SegmentedData::new(&RESTORATION_FRAME_3, &spans).ok_or("restoration segmented input")?;
    let mut decoder = RangeDecoder::new(&data, 0, RESTORATION_FRAME_3.len(), false)
        .ok_or("restoration initialization")?;
    let mut sgr_cdf = [15_913, 0];
    let case = "restoration_422_frame_3";
    let mut step = 0_u32;
    records.push(decoder.trace_state(case, step, -1, &sgr_cdf));
    step = step.checked_add(1).ok_or("restoration step")?;
    for _plane in 0..3 {
        let enabled = decoder.adaptive_bool(&mut sgr_cdf);
        records.push(decoder.trace_state(case, step, i32::from(enabled), &sgr_cdf));
        step = step.checked_add(1).ok_or("restoration step")?;
        if !enabled {
            continue;
        }
        let parameter_index = decoder.bits(4);
        let parameter_index_value =
            i32::try_from(parameter_index).map_err(|_| "restoration parameter value")?;
        records.push(decoder.trace_state(case, step, parameter_index_value, &[]));
        step = step.checked_add(1).ok_or("restoration step")?;
        let activity = *SGR_PARAMETER_ACTIVITY
            .get(usize::try_from(parameter_index).map_err(|_| "restoration parameter index")?)
            .ok_or("restoration activity")?;
        if activity[0] {
            let weight = decoder
                .subexponential(64, 128, 4)
                .checked_sub(96)
                .ok_or("restoration first weight value")?;
            records.push(decoder.trace_state(case, step, weight, &[]));
            step = step.checked_add(1).ok_or("restoration step")?;
        }
        if activity[1] {
            let weight = decoder
                .subexponential(63, 128, 4)
                .checked_sub(32)
                .ok_or("restoration second weight value")?;
            records.push(decoder.trace_state(case, step, weight, &[]));
            step = step.checked_add(1).ok_or("restoration step")?;
        }
    }
    let (mut partition_cdf, symbol_count_minus_one) =
        default_partition_cdf(1).ok_or("restoration partition CDF")?;
    let partition =
        i32::try_from(decoder.adaptive_symbol(&mut partition_cdf, symbol_count_minus_one))
            .map_err(|_| "restoration partition value")?;
    records.push(decoder.trace_state(case, step, partition, &partition_cdf));

    Ok(records)
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_context() -> FirstBlockContext {
    FirstBlockContext {
        disable_cdf_update: false,
        level: 1,
        block_width: 16,
        block_height: 16,
        block_x: 0,
        block_y: 0,
        frame_width: 64,
        frame_height: 64,
        upscaled_width: 64,
        superres_enabled: false,
        monochrome: false,
        subsampling_x: true,
        subsampling_y: false,
        restoration_types: [None; 3],
        restoration_unit_size_log2: [8; 2],
        bit_depth: 8,
        all_lossless: false,
        segmentation_enabled: false,
        skip_mode_enabled: false,
        allow_intrabc: false,
        allow_screen_content_tools: false,
        enable_filter_intra: true,
        frame_tools: FrameToolsContext {
            quantization: None,
            segment_qindex: 0,
            segment_lossless: false,
            delta_q_present: false,
            delta_q_resolution_log2: 0,
            delta_lf_present: false,
            delta_lf_resolution_log2: 0,
            delta_lf_multi: false,
            loop_filter: LoopFilterContext {
                level_y: [0; 2],
                level_u: 0,
                level_v: 0,
                sharpness: 0,
                delta_enabled: true,
                delta_update: true,
                reference_deltas: [1, 0, 0, 0, -1, 0, -1, -1],
                mode_deltas: [0; 2],
            },
            cdef: None,
            restoration_present: false,
            transform_mode: 0,
            reduced_transform_set: false,
            film_grain_present: false,
        },
    }
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_restoration_and_partition_paths() {
    let input = [0_u8; 64];
    let spans = [super::super::samples::ByteSpan {
        start: 0,
        end: input.len(),
    }];
    let data = SegmentedData::new(&input, &spans).unwrap();

    let mut maximum_token = 0;
    for fill in 0..=u8::MAX {
        let input = [fill; 64];
        let spans = [super::super::samples::ByteSpan {
            start: 0,
            end: input.len(),
        }];
        let data = SegmentedData::new(&input, &spans).unwrap();
        let mut decoder = RangeDecoder::new(&data, 0, input.len(), false).unwrap();
        let mut cdf = [24_576, 16_384, 8192, 0];
        maximum_token = maximum_token.max(decoder.high_token(&mut cdf));
        for frame_type in [
            RestorationType::Switchable,
            RestorationType::Wiener,
            RestorationType::SgrProjection,
        ] {
            for plane in 0..=1 {
                let mut decoder = RangeDecoder::new(&data, 0, input.len(), false).unwrap();
                let mut cdfs = RestorationCdfs::defaults();
                let mut reference = RestorationReference::defaults();
                let _ = decode_restoration_unit(
                    &mut decoder,
                    &mut cdfs,
                    &mut reference,
                    plane,
                    frame_type,
                );
            }
        }
    }
    assert!(maximum_token >= 12);

    let mut context = coverage_context();
    context.restoration_types[0] = Some(RestorationType::Wiener);
    context.restoration_unit_size_log2[0] = 3;
    context.block_y = 1;
    assert_eq!(
        restoration_unit_starts_at_first_block(&context, 0),
        Some(false)
    );
    context.restoration_unit_size_log2[0] = 3;
    context.block_y = 2;
    context.frame_height = 8;
    assert_eq!(
        restoration_unit_starts_at_first_block(&context, 0),
        Some(false)
    );
    context.restoration_unit_size_log2[0] = 3;
    context.block_y = 2;
    context.frame_height = 64;
    assert_eq!(
        restoration_unit_starts_at_first_block(&context, 0),
        Some(true)
    );
    context.block_y = 0;
    context.frame_height = 64;
    context.upscaled_width = 65;
    assert_eq!(restoration_unit_starts_at_first_block(&context, 0), None);
    context.upscaled_width = 64;
    context.restoration_unit_size_log2[0] = 3;
    context.block_x = 1;
    assert_eq!(
        restoration_unit_starts_at_first_block(&context, 0),
        Some(false)
    );
    context.restoration_unit_size_log2[0] = 4;
    context.block_x = 2;
    context.frame_width = 8;
    context.upscaled_width = 8;
    assert_eq!(
        restoration_unit_starts_at_first_block(&context, 0),
        Some(false)
    );
    context.restoration_unit_size_log2[0] = 3;
    context.block_x = 2;
    context.frame_width = 64;
    context.upscaled_width = 64;
    assert_eq!(
        restoration_unit_starts_at_first_block(&context, 0),
        Some(true)
    );
    context.block_x = 0;
    context.frame_width = 64;
    context.upscaled_width = 64;
    assert_eq!(
        restoration_unit_starts_at_first_block(&context, 0),
        Some(true)
    );
    let mut active_prefix = coverage_context();
    active_prefix.restoration_types[0] = Some(RestorationType::Wiener);
    active_prefix.restoration_unit_size_log2[0] = 3;
    let mut decoder = RangeDecoder::new(&data, 0, input.len(), false).unwrap();
    assert_eq!(
        decode_restoration_prefix(&mut decoder, &active_prefix),
        Some(())
    );
    active_prefix.block_y = 1;
    let mut decoder = RangeDecoder::new(&data, 0, input.len(), false).unwrap();
    assert_eq!(
        decode_restoration_prefix(&mut decoder, &active_prefix),
        Some(())
    );
    active_prefix.block_y = 0;
    active_prefix.upscaled_width = 65;
    let mut decoder = RangeDecoder::new(&data, 0, input.len(), false).unwrap();
    assert_eq!(
        decode_restoration_prefix(&mut decoder, &active_prefix),
        None
    );
    assert_eq!(
        validate_first_partition(&data, 0..input.len(), &active_prefix),
        None
    );

    assert!(default_partition_cdf(5).is_none());
    let cdf = default_partition_cdf(0).unwrap().0;
    let _ = left_partition_probability(&cdf, 0);
    let _ = top_partition_probability(&cdf, 0);

    const FORBIDDEN_422: [u8; 77] = [
        0xf8, 0x3f, 0x9f, 0xfd, 0x73, 0xc0, 0x2f, 0xa5, 0x59, 0x48, 0xfa, 0xc5, 0xe5, 0x74, 0x87,
        0x85, 0xca, 0xc6, 0x00, 0x81, 0x5d, 0xa5, 0x3a, 0x6e, 0xfa, 0xf3, 0x7c, 0x24, 0x18, 0x0b,
        0xfc, 0x69, 0x2c, 0x41, 0x07, 0x3b, 0x72, 0x2e, 0xcf, 0xff, 0xb0, 0x2a, 0x3b, 0x55, 0x45,
        0x22, 0x47, 0xbb, 0x8c, 0x3c, 0x03, 0xb2, 0x19, 0xe9, 0xdf, 0x68, 0xca, 0xf0, 0x15, 0x6e,
        0xc0, 0xe7, 0x9d, 0x21, 0xff, 0x54, 0xf6, 0xce, 0x30, 0x93, 0x63, 0x6f, 0x59, 0x97, 0x89,
        0xba, 0x72,
    ];
    let spans = [super::super::samples::ByteSpan {
        start: 0,
        end: FORBIDDEN_422.len(),
    }];
    let forbidden_data = SegmentedData::new(&FORBIDDEN_422, &spans).unwrap();
    assert_eq!(
        validate_first_partition(&forbidden_data, 0..FORBIDDEN_422.len(), &coverage_context(),),
        None
    );

    let mut horizontal_only = coverage_context();
    horizontal_only.block_height = 8;
    horizontal_only.monochrome = true;
    let mut vertical_only = coverage_context();
    vertical_only.block_width = 8;
    let mut horizontal_422 = coverage_context();
    horizontal_422.block_height = 8;
    let mut vertical_444 = coverage_context();
    vertical_444.block_width = 8;
    vertical_444.subsampling_x = false;
    let mut vertical_420 = coverage_context();
    vertical_420.block_width = 8;
    vertical_420.subsampling_y = true;
    let mut accepted_vertical = false;
    let mut rejected_vertical = false;
    for fill in 0..=u8::MAX {
        let input = [fill; 64];
        let spans = [super::super::samples::ByteSpan {
            start: 0,
            end: input.len(),
        }];
        let data = SegmentedData::new(&input, &spans).unwrap();
        let _ = validate_first_partition(&data, 0..input.len(), &horizontal_only);
        let _ = validate_first_partition(&data, 0..input.len(), &horizontal_422);
        let _ = validate_first_partition(&data, 0..input.len(), &vertical_444);
        let _ = validate_first_partition(&data, 0..input.len(), &vertical_420);
        if validate_first_partition(&data, 0..input.len(), &vertical_only).is_some() {
            accepted_vertical = true;
        } else {
            rejected_vertical = true;
        }
    }
    assert!(accepted_vertical && rejected_vertical);

    let mut no_partition = coverage_context();
    no_partition.level = 4;
    no_partition.block_width = 0;
    no_partition.block_height = 0;
    assert_eq!(
        validate_first_partition(&data, 0..input.len(), &no_partition),
        None
    );
    let mut invalid_level = coverage_context();
    invalid_level.level = 5;
    let _ = validate_first_partition(&data, 0..input.len(), &invalid_level);
}

#[cfg(coverage)]
#[coverage(off)]
pub(super) fn __coverage_exercise_private_branches() {
    let empty_spans = [];
    let empty = SegmentedData::new(&[], &empty_spans).unwrap();
    let _ = RangeDecoder::new(&empty, 1, 0, false);
    let _ = RangeDecoder::new(&empty, 0, 1, false);
    let _ = validate_first_partition(&empty, 1..0, &coverage_context());
    let mut frozen = RangeDecoder::new(&empty, 0, 0, true).unwrap();
    let mut bool_cdf = [16_384, 0];
    let _ = frozen.adaptive_bool(&mut bool_cdf);
    let _ = inverse_recenter(1, 3);
    for value in 0..=3 {
        let _ = RestorationType::from_bits(value);
    }
    coverage_restoration_and_partition_paths();
}
