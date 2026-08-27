//! Scalar AV1 multi-symbol arithmetic decoding over segmented tile bytes.

use std::ops::Range;

#[cfg(coverage)]
use crate::codecs::{CodecError, CodecResult};

use super::bit_reader::SegmentedData;
use super::{Av1Result, malformed};

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
    ) -> Av1Result<Self> {
        if start > end || end > data.len() {
            return Err(malformed("entropy range exceeds the tile payload"));
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
        Ok(decoder)
    }

    #[cfg(coverage)]
    pub(super) fn enable_operation_trace(&mut self) {
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
    pub(super) fn operation_trace(&self) -> Vec<crate::Av1EntropyOperationState> {
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

    // ✅ VERIFIED: dav1d 1.5.3 src/decode.c:2754-2755. A tile is invalid
    // when the symbol coder has consumed past its permitted padding window.
    pub(super) const fn symbol_coder_overread(&self) -> bool {
        self.count <= -15
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
    pub(super) y_strengths: [u32; 4],
    pub(super) uv_strengths: [u32; 4],
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
#[derive(Clone, Copy)]
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
    pub(super) enable_intra_edge_filter: bool,
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
) -> bool {
    let mut cdfs = RestorationCdfs::defaults();
    let mut references = [RestorationReference::defaults(); 3];
    let plane_count = if context.monochrome { 1 } else { 3 };
    for (plane, reference) in references.iter_mut().enumerate().take(plane_count) {
        let Some(restoration_type) = context.restoration_types[plane] else {
            continue;
        };
        match restoration_unit_starts_at_first_block(context, plane) {
            Some(true) => {
                decode_restoration_unit(decoder, &mut cdfs, reference, plane, restoration_type);
            }
            Some(false) => {}
            None => return false,
        }
    }
    true
}

// ✅ VERIFIED: dav1d 1.5.3 src/cdf.c:386-433. The values are dav1d's inverse
// partition CDF for context zero, including the mutable count slot.
const fn square8_partition_cdf() -> ([u16; 10], usize) {
    ([13_636, 7258, 2376, 0, 0, 0, 0, 0, 0, 0], 3)
}

fn default_partition_cdf(level: u32) -> Av1Result<([u16; 10], usize)> {
    match level {
        0 => Ok(([4869, 4549, 4239, 284, 229, 149, 129, 0, 0, 0], 7)),
        1 => Ok((
            [12_631, 11_221, 9690, 3202, 2931, 2507, 2244, 1876, 1044, 0],
            9,
        )),
        2 => Ok((
            [14_306, 11_848, 9644, 5121, 4541, 3719, 3249, 2590, 1224, 0],
            9,
        )),
        3 => Ok((
            [17_171, 11_839, 8197, 6062, 5104, 3947, 3167, 2197, 866, 0],
            9,
        )),
        4 => Ok(square8_partition_cdf()),
        _ => Err(malformed("partition level exceeds four")),
    }
}

/// The ten AV1 partition symbols.  The last two are only legal before the
/// 8x8 block level; the range decoder still returns the same small integer
/// domain for every level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PartitionKind {
    None,
    Horizontal,
    Vertical,
    Split,
    TopSplit,
    BottomSplit,
    LeftSplit,
    RightSplit,
    HorizontalFour,
    VerticalFour,
}

impl PartitionKind {
    fn from_symbol(symbol: u32) -> Av1Result<Self> {
        match symbol {
            0 => Ok(Self::None),
            1 => Ok(Self::Horizontal),
            2 => Ok(Self::Vertical),
            3 => Ok(Self::Split),
            4 => Ok(Self::TopSplit),
            5 => Ok(Self::BottomSplit),
            6 => Ok(Self::LeftSplit),
            7 => Ok(Self::RightSplit),
            8 => Ok(Self::HorizontalFour),
            9 => Ok(Self::VerticalFour),
            _ => Err(malformed("partition symbol exceeds the AV1 domain")),
        }
    }

    const fn symbol(self) -> usize {
        match self {
            Self::None => 0,
            Self::Horizontal => 1,
            Self::Vertical => 2,
            Self::Split => 3,
            Self::TopSplit => 4,
            Self::BottomSplit => 5,
            Self::LeftSplit => 6,
            Self::RightSplit => 7,
            Self::HorizontalFour => 8,
            Self::VerticalFour => 9,
        }
    }

    const fn is_recursive(self) -> bool {
        matches!(self, Self::Split)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PartitionGeometry {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

/// Return the terminal block footprints produced by one non-recursive
/// partition symbol.  AV1 writes the block payloads in this order, so keeping
/// the expansion next to the syntax enum gives the later block decoder one
/// deterministic, allocation-free traversal contract.
fn partition_child_geometries(
    kind: PartitionKind,
    x: u32,
    y: u32,
    half_size: u32,
) -> Av1Result<([PartitionGeometry; 4], usize)> {
    if half_size == 0 {
        return Err(malformed("partition child has zero size"));
    }
    let full_size = half_size
        .checked_mul(2)
        .ok_or_else(|| malformed("partition child size overflows"))?;
    let right = x
        .checked_add(half_size)
        .ok_or_else(|| malformed("partition child x coordinate overflows"))?;
    let bottom = y
        .checked_add(half_size)
        .ok_or_else(|| malformed("partition child y coordinate overflows"))?;
    let mut children = [PartitionGeometry {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    }; 4];
    let count = match kind {
        PartitionKind::None => {
            children[0] = PartitionGeometry {
                x,
                y,
                width: full_size,
                height: full_size,
            };
            1
        }
        PartitionKind::Horizontal => {
            children[0] = PartitionGeometry {
                x,
                y,
                width: full_size,
                height: half_size,
            };
            children[1] = PartitionGeometry {
                x,
                y: bottom,
                width: full_size,
                height: half_size,
            };
            2
        }
        PartitionKind::Vertical => {
            children[0] = PartitionGeometry {
                x,
                y,
                width: half_size,
                height: full_size,
            };
            children[1] = PartitionGeometry {
                x: right,
                y,
                width: half_size,
                height: full_size,
            };
            2
        }
        PartitionKind::Split => {
            children = [
                PartitionGeometry {
                    x,
                    y,
                    width: half_size,
                    height: half_size,
                },
                PartitionGeometry {
                    x: right,
                    y,
                    width: half_size,
                    height: half_size,
                },
                PartitionGeometry {
                    x,
                    y: bottom,
                    width: half_size,
                    height: half_size,
                },
                PartitionGeometry {
                    x: right,
                    y: bottom,
                    width: half_size,
                    height: half_size,
                },
            ];
            4
        }
        PartitionKind::TopSplit => {
            children[0] = PartitionGeometry {
                x,
                y,
                width: half_size,
                height: half_size,
            };
            children[1] = PartitionGeometry {
                x: right,
                y,
                width: half_size,
                height: half_size,
            };
            children[2] = PartitionGeometry {
                x,
                y: bottom,
                width: full_size,
                height: half_size,
            };
            3
        }
        PartitionKind::BottomSplit => {
            children[0] = PartitionGeometry {
                x,
                y,
                width: full_size,
                height: half_size,
            };
            children[1] = PartitionGeometry {
                x,
                y: bottom,
                width: half_size,
                height: half_size,
            };
            children[2] = PartitionGeometry {
                x: right,
                y: bottom,
                width: half_size,
                height: half_size,
            };
            3
        }
        PartitionKind::LeftSplit => {
            children[0] = PartitionGeometry {
                x,
                y,
                width: half_size,
                height: half_size,
            };
            children[1] = PartitionGeometry {
                x,
                y: bottom,
                width: half_size,
                height: half_size,
            };
            children[2] = PartitionGeometry {
                x: right,
                y,
                width: half_size,
                height: full_size,
            };
            3
        }
        PartitionKind::RightSplit => {
            children[0] = PartitionGeometry {
                x,
                y,
                width: half_size,
                height: full_size,
            };
            children[1] = PartitionGeometry {
                x: right,
                y,
                width: half_size,
                height: half_size,
            };
            children[2] = PartitionGeometry {
                x: right,
                y: bottom,
                width: half_size,
                height: half_size,
            };
            3
        }
        PartitionKind::HorizontalFour => {
            let quarter_size = half_size
                .checked_div(2)
                .filter(|size| *size != 0)
                .ok_or_else(|| malformed("horizontal-four partition is too small"))?;
            for (index, child) in children.iter_mut().enumerate() {
                let offset = quarter_size
                    .checked_mul(u32::try_from(index).unwrap_or(u32::MAX))
                    .ok_or_else(|| malformed("horizontal-four y coordinate overflows"))?;
                *child = PartitionGeometry {
                    x,
                    y: y.checked_add(offset)
                        .ok_or_else(|| malformed("horizontal-four y coordinate overflows"))?,
                    width: full_size,
                    height: quarter_size,
                };
            }
            4
        }
        PartitionKind::VerticalFour => {
            let quarter_size = half_size
                .checked_div(2)
                .filter(|size| *size != 0)
                .ok_or_else(|| malformed("vertical-four partition is too small"))?;
            for (index, child) in children.iter_mut().enumerate() {
                let offset = quarter_size
                    .checked_mul(u32::try_from(index).unwrap_or(u32::MAX))
                    .ok_or_else(|| malformed("vertical-four x coordinate overflows"))?;
                *child = PartitionGeometry {
                    x: x.checked_add(offset)
                        .ok_or_else(|| malformed("vertical-four x coordinate overflows"))?,
                    y,
                    width: quarter_size,
                    height: full_size,
                };
            }
            4
        }
    };
    Ok((children, count))
}

fn clip_partition_geometry(
    geometry: PartitionGeometry,
    frame_width: u32,
    frame_height: u32,
) -> Av1Result<Option<PartitionGeometry>> {
    if geometry.x >= frame_width || geometry.y >= frame_height {
        return Ok(None);
    }
    let width = frame_width
        .checked_sub(geometry.x)
        .ok_or_else(|| malformed("partition child escapes the frame horizontally"))?
        .min(geometry.width);
    let height = frame_height
        .checked_sub(geometry.y)
        .ok_or_else(|| malformed("partition child escapes the frame vertically"))?
        .min(geometry.height);
    if width == 0 || height == 0 {
        return Err(malformed("partition child has no visible samples"));
    }
    Ok(Some(PartitionGeometry {
        width,
        height,
        ..geometry
    }))
}

/// One syntax node visited by the bounded partition walker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PartitionNode {
    pub(super) level: u32,
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) context: u8,
    pub(super) kind: PartitionKind,
}

const MAX_PARTITION_NODES: usize = 1_048_576;

/// Controls whether the interleaved partition walker should continue after a
/// terminal block is reached.
///
/// AV1 places a terminal block's prediction/residual syntax between the
/// partition syntax for its siblings.  A caller that has not implemented that
/// block syntax must stop at the terminal node; continuing would interpret
/// block bytes as another partition symbol and would make the result unsound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PartitionVisitControl {
    Continue,
    Stop,
}

// ✅ VERIFIED: dav1d 1.1.0 src/cdf.rs:4277-4313 and src/tables.rs:451-456.
// These are the inverse CDFs used by the scalar decoder above.  The four
// rows are the AV1 above/left partition contexts.  Keeping them in safe Rust
// makes the walker independent from the native oracle while retaining the
// exact adaptive-CDF state transition.
const PARTITION_CDFS: [[[u16; 10]; 4]; 5] = [
    [
        [4869, 4549, 4239, 284, 229, 149, 129, 0, 0, 0],
        [26161, 25778, 24500, 708, 549, 430, 397, 0, 0, 0],
        [27339, 26092, 25646, 741, 541, 237, 186, 0, 0, 0],
        [32057, 31802, 31596, 320, 230, 151, 104, 0, 0, 0],
    ],
    [
        [12631, 11221, 9690, 3202, 2931, 2507, 2244, 1876, 1044, 0],
        [26036, 25278, 23271, 4824, 4518, 4253, 3799, 3138, 2664, 0],
        [26823, 25105, 24420, 4085, 3651, 3019, 2704, 2470, 530, 0],
        [31898, 31556, 31281, 1570, 1374, 1194, 1025, 887, 436, 0],
    ],
    [
        [14306, 11848, 9644, 5121, 4541, 3719, 3249, 2590, 1224, 0],
        [25079, 23708, 20712, 7776, 7108, 6586, 5817, 4727, 3716, 0],
        [26753, 23759, 22706, 8224, 7359, 6223, 5697, 5242, 721, 0],
        [31374, 30560, 29972, 4154, 3707, 3302, 2928, 2583, 869, 0],
    ],
    [
        [17171, 11839, 8197, 6062, 5104, 3947, 3167, 2197, 866, 0],
        [24843, 21725, 15983, 10298, 8797, 7725, 6117, 4067, 2934, 0],
        [27354, 19499, 17657, 12280, 10408, 8268, 7231, 6432, 651, 0],
        [30106, 26406, 24154, 11908, 9715, 7990, 6332, 4939, 1597, 0],
    ],
    [
        [13636, 7258, 2376, 0, 0, 0, 0, 0, 0, 0],
        [18840, 12913, 4228, 0, 0, 0, 0, 0, 0, 0],
        [20246, 9089, 4139, 0, 0, 0, 0, 0, 0, 0],
        [22872, 13985, 6915, 0, 0, 0, 0, 0, 0, 0],
    ],
];

// The values written into dav1d's above and left partition contexts after a
// node. The two tables are edge orientations, not superblock sizes.
const AL_PARTITION_CONTEXT: [[[u8; 10]; 5]; 2] = [
    [
        [0x00, 0x00, 0x10, 0xff, 0x00, 0x10, 0x10, 0x10, 0xff, 0xff],
        [0x10, 0x10, 0x18, 0xff, 0x10, 0x18, 0x18, 0x18, 0x10, 0x1c],
        [0x18, 0x18, 0x1c, 0xff, 0x18, 0x1c, 0x1c, 0x1c, 0x18, 0x1e],
        [0x1c, 0x1c, 0x1e, 0xff, 0x1c, 0x1e, 0x1e, 0x1e, 0x1c, 0x1f],
        [0x1e, 0x1e, 0x1f, 0x1f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
    ],
    [
        [0x00, 0x10, 0x00, 0xff, 0x10, 0x10, 0x00, 0x10, 0xff, 0xff],
        [0x10, 0x18, 0x10, 0xff, 0x18, 0x18, 0x10, 0x18, 0x1c, 0x10],
        [0x18, 0x1c, 0x18, 0xff, 0x1c, 0x1c, 0x18, 0x1c, 0x1e, 0x18],
        [0x1c, 0x1e, 0x1c, 0xff, 0x1e, 0x1e, 0x1c, 0x1e, 0x1f, 0x1c],
        [0x1e, 0x1f, 0x1e, 0x1f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
    ],
];

struct PartitionContexts {
    origin_x: u32,
    origin_y: u32,
    above: [u8; 32],
    left: [u8; 32],
}

impl PartitionContexts {
    fn new(context: &FirstBlockContext) -> Self {
        Self {
            origin_x: context.block_x,
            origin_y: context.block_y,
            above: [0; 32],
            left: [0; 32],
        }
    }

    fn cell(&self, x: u32, y: u32) -> Av1Result<(usize, usize)> {
        let relative_x = x
            .checked_sub(self.origin_x)
            .ok_or_else(|| malformed("partition context escapes the coded superblock"))?
            / 2;
        let relative_y = y
            .checked_sub(self.origin_y)
            .ok_or_else(|| malformed("partition context escapes the coded superblock"))?
            / 2;
        let x = usize::try_from(relative_x)
            .map_err(|_| malformed("partition context coordinate overflows"))?;
        let y = usize::try_from(relative_y)
            .map_err(|_| malformed("partition context coordinate overflows"))?;
        if x >= self.above.len() || y >= self.left.len() {
            return Err(malformed("partition context exceeds the coded superblock"));
        }
        Ok((x, y))
    }

    fn context(&self, level: u32, x: u32, y: u32) -> Av1Result<u8> {
        if level > 4 {
            return Err(malformed("partition level exceeds four"));
        }
        let (cell_x, cell_y) = self.cell(x, y)?;
        let above = self.above[cell_x];
        let left = self.left[cell_y];
        let shift = 4_u32.saturating_sub(level);
        let above = (above >> shift) & 1;
        let left = (left >> shift) & 1;
        Ok(above | left.saturating_mul(2))
    }

    fn record(
        &mut self,
        level: u32,
        kind: PartitionKind,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Av1Result<()> {
        if level > 4 {
            return Ok(());
        }
        let above_value = AL_PARTITION_CONTEXT[0][level as usize][kind.symbol()];
        let left_value = AL_PARTITION_CONTEXT[1][level as usize][kind.symbol()];
        if above_value == 0xff || left_value == 0xff {
            return Err(malformed("partition context has no AV1 value"));
        }
        let start_x = self.cell(x, y)?.0;
        let end_x = self
            .cell(x.saturating_add(width.saturating_sub(1)), y)?
            .0
            .saturating_add(1)
            .min(self.above.len());
        let start_y = self.cell(x, y)?.1;
        let end_y = self
            .cell(x, y.saturating_add(height.saturating_sub(1)))?
            .1
            .saturating_add(1)
            .min(self.left.len());
        self.above[start_x..end_x].fill(above_value);
        self.left[start_y..end_y].fill(left_value);
        Ok(())
    }
}

struct PartitionWalker<'decoder, 'data, 'input, 'spans> {
    decoder: &'decoder mut RangeDecoder<'data, 'input, 'spans>,
    cdfs: [[[u16; 10]; 4]; 5],
    contexts: PartitionContexts,
    frame_width: u32,
    frame_height: u32,
    root_end_x: u32,
    root_end_y: u32,
    monochrome: bool,
    subsampling_x: bool,
    subsampling_y: bool,
    nodes: Vec<PartitionNode>,
}

impl<'decoder, 'data, 'input, 'spans> PartitionWalker<'decoder, 'data, 'input, 'spans> {
    fn new(
        decoder: &'decoder mut RangeDecoder<'data, 'input, 'spans>,
        context: &FirstBlockContext,
    ) -> Self {
        Self {
            decoder,
            cdfs: PARTITION_CDFS,
            contexts: PartitionContexts::new(context),
            frame_width: context.block_width,
            frame_height: context.block_height,
            root_end_x: context.block_width,
            root_end_y: context.block_height,
            monochrome: context.monochrome,
            subsampling_x: context.subsampling_x,
            subsampling_y: context.subsampling_y,
            nodes: Vec::new(),
        }
    }

    fn reset_root(&mut self) {
        // dav1d's above/left partition contexts belong to the tile, not to
        // an individual superblock.  They therefore carry the right and
        // bottom edge state from one root into the next root.  Only the
        // diagnostic node list is per-root; clearing `contexts` here would
        // erase the left neighbor needed by the next horizontal root and the
        // above neighbor needed by the next root row.
        self.nodes.clear();
    }

    fn set_root_bounds(&mut self, x: u32, y: u32, root_size: u32) -> Av1Result<()> {
        self.root_end_x = x
            .checked_add(root_size)
            .ok_or_else(|| malformed("partition root x extent overflows"))?
            .min(self.frame_width);
        self.root_end_y = y
            .checked_add(root_size)
            .ok_or_else(|| malformed("partition root y extent overflows"))?
            .min(self.frame_height);
        if x >= self.root_end_x || y >= self.root_end_y {
            return Err(malformed("partition root has no visible samples"));
        }
        Ok(())
    }

    fn decode_kind(
        &mut self,
        level: u32,
        x: u32,
        y: u32,
        horizontal_split: bool,
        vertical_split: bool,
    ) -> Av1Result<(PartitionKind, u8)> {
        let context = self.contexts.context(level, x, y)?;
        let kind = if horizontal_split && vertical_split {
            let symbol_count_minus_one = match level {
                0 => 7,
                1..=3 => 9,
                4 => 3,
                _ => return Err(malformed("partition level exceeds four")),
            };

            let symbol = self.decoder.adaptive_symbol(
                &mut self.cdfs[level as usize][context as usize],
                symbol_count_minus_one,
            );

            PartitionKind::from_symbol(symbol)?
        } else if horizontal_split {
            let cdf = &self.cdfs[level as usize][context as usize];
            let probability = top_partition_probability(cdf, level);
            if self.decoder.fixed(probability) {
                PartitionKind::Split
            } else {
                PartitionKind::Horizontal
            }
        } else if vertical_split {
            let cdf = &self.cdfs[level as usize][context as usize];
            let probability = left_partition_probability(cdf, level);
            if self.decoder.fixed(probability) {
                PartitionKind::Split
            } else {
                PartitionKind::Vertical
            }
        } else {
            return Err(malformed("partition decoder called without a split edge"));
        };
        if !self.monochrome
            && self.subsampling_x
            && !self.subsampling_y
            && matches!(
                kind,
                PartitionKind::Vertical
                    | PartitionKind::LeftSplit
                    | PartitionKind::RightSplit
                    | PartitionKind::VerticalFour
            )
        {
            return Err(malformed(
                "partition syntax is invalid for vertically unsampled chroma",
            ));
        }

        Ok((kind, context))
    }

    fn push(&mut self, node: PartitionNode) -> Av1Result<()> {
        if self.nodes.len() >= MAX_PARTITION_NODES {
            return Err(malformed("partition tree exceeds the safe node limit"));
        }
        self.nodes.push(node);
        Ok(())
    }

    fn visit_terminal<F>(
        &mut self,
        level: u32,
        kind: PartitionKind,
        x: u32,
        y: u32,
        half_size: u32,
        visit: &mut F,
    ) -> Av1Result<PartitionVisitControl>
    where
        F: FnMut(
            &mut RangeDecoder<'data, 'input, 'spans>,
            PartitionNode,
        ) -> Av1Result<PartitionVisitControl>,
    {
        let (children, count) = partition_child_geometries(kind, x, y, half_size)?;
        let child_level = level.saturating_add(1);
        let context_level = child_level.min(4);
        for geometry in children.into_iter().take(count) {
            let Some(geometry) =
                clip_partition_geometry(geometry, self.root_end_x, self.root_end_y)?
            else {
                continue;
            };
            let context = self
                .contexts
                .context(context_level, geometry.x, geometry.y)?;
            let node = PartitionNode {
                level: child_level,
                x: geometry.x,
                y: geometry.y,
                width: geometry.width,
                height: geometry.height,
                context,
                kind: PartitionKind::None,
            };
            self.contexts.record(
                context_level,
                PartitionKind::None,
                geometry.x,
                geometry.y,
                geometry.width,
                geometry.height,
            )?;
            self.push(node)?;
            if matches!(visit(self.decoder, node)?, PartitionVisitControl::Stop) {
                return Ok(PartitionVisitControl::Stop);
            }
        }
        Ok(PartitionVisitControl::Continue)
    }

    fn walk<F>(
        &mut self,
        level: u32,
        x: u32,
        y: u32,
        visit: &mut F,
    ) -> Av1Result<PartitionVisitControl>
    where
        F: FnMut(
            &mut RangeDecoder<'data, 'input, 'spans>,
            PartitionNode,
        ) -> Av1Result<PartitionVisitControl>,
    {
        let half_size = 16_u32
            .checked_shr(level)
            .ok_or_else(|| malformed("partition level shift overflows"))?;
        if half_size == 0 {
            return Err(malformed("partition block has zero size"));
        }
        if x >= self.root_end_x || y >= self.root_end_y {
            return Ok(PartitionVisitControl::Continue);
        }
        let horizontal_split = self.root_end_x > x.saturating_add(half_size);
        let vertical_split = self.root_end_y > y.saturating_add(half_size);
        if !horizontal_split && !vertical_split {
            if x >= self.frame_width || y >= self.frame_height {
                return Ok(PartitionVisitControl::Continue);
            }
            if level < 4 {
                return self.walk(level.saturating_add(1), x, y, visit);
            }
            let width = self.frame_width.saturating_sub(x).min(half_size);
            let height = self.frame_height.saturating_sub(y).min(half_size);
            let context = self.contexts.context(level, x, y)?;
            let node = PartitionNode {
                level,
                x,
                y,
                width,
                height,
                context,
                kind: PartitionKind::None,
            };
            self.contexts
                .record(level, PartitionKind::None, x, y, width, height)?;
            self.push(node)?;
            return visit(self.decoder, node);
        }

        let (kind, context) = self.decode_kind(level, x, y, horizontal_split, vertical_split)?;
        let full_width = half_size.saturating_mul(2);
        let full_height = half_size.saturating_mul(2);
        let parent = PartitionNode {
            level,
            x,
            y,
            width: full_width,
            height: full_height,
            context,
            kind,
        };
        self.push(parent)?;

        if kind == PartitionKind::None {
            let Some(geometry) = clip_partition_geometry(
                PartitionGeometry {
                    x,
                    y,
                    width: full_width,
                    height: full_height,
                },
                self.root_end_x,
                self.root_end_y,
            )?
            else {
                return Ok(PartitionVisitControl::Continue);
            };
            if let Some(node) = self.nodes.last_mut() {
                node.x = geometry.x;
                node.y = geometry.y;
                node.width = geometry.width;
                node.height = geometry.height;
            }
            self.contexts
                .record(level, kind, x, y, full_width, full_height)?;
            let node = self
                .nodes
                .last()
                .copied()
                .ok_or_else(|| malformed("terminal partition node was not recorded"))?;
            return visit(self.decoder, node);
        }

        if !kind.is_recursive() {
            let control = self.visit_terminal(level, kind, x, y, half_size, visit)?;
            if matches!(control, PartitionVisitControl::Stop) {
                return Ok(PartitionVisitControl::Stop);
            }
            // The neighboring-block state is attached to the whole coded
            // footprint.  H/V/three-way/four-way shapes have several block
            // payloads, but the next partition outside this footprint must
            // see the terminal shape at both its top and bottom/left and
            // right edges.
            self.contexts
                .record(level, kind, x, y, full_width, full_height)?;
            return Ok(PartitionVisitControl::Continue);
        }

        if level == 4 {
            // At 8x8 the split has four implicit 4x4 blocks; no fifth
            // partition CDF exists.  The AV1 context update is still for the
            // 8x8 SPLIT footprint, after all four block payloads have been
            // consumed; recording only the implicit leaves would lose the
            // left-edge bit needed by the next 8x8 partition.
            let control = self.visit_terminal(level, kind, x, y, half_size, visit)?;
            if matches!(control, PartitionVisitControl::Stop) {
                return Ok(PartitionVisitControl::Stop);
            }
            self.contexts
                .record(level, kind, x, y, full_width, full_height)?;
            return Ok(PartitionVisitControl::Continue);
        }

        let next_level = level.saturating_add(1);
        if horizontal_split && vertical_split {
            if matches!(
                self.walk(next_level, x, y, visit)?,
                PartitionVisitControl::Stop
            ) {
                return Ok(PartitionVisitControl::Stop);
            }
            if matches!(
                self.walk(next_level, x.saturating_add(half_size), y, visit)?,
                PartitionVisitControl::Stop
            ) {
                return Ok(PartitionVisitControl::Stop);
            }
            if matches!(
                self.walk(next_level, x, y.saturating_add(half_size), visit)?,
                PartitionVisitControl::Stop
            ) {
                return Ok(PartitionVisitControl::Stop);
            }
            self.walk(
                next_level,
                x.saturating_add(half_size),
                y.saturating_add(half_size),
                visit,
            )
        } else if horizontal_split {
            if matches!(
                self.walk(next_level, x, y, visit)?,
                PartitionVisitControl::Stop
            ) {
                return Ok(PartitionVisitControl::Stop);
            }
            self.walk(next_level, x.saturating_add(half_size), y, visit)
        } else {
            if matches!(
                self.walk(next_level, x, y, visit)?,
                PartitionVisitControl::Stop
            ) {
                return Ok(PartitionVisitControl::Stop);
            }
            self.walk(next_level, x, y.saturating_add(half_size), visit)
        }
    }
}

/// Reach terminal partition footprints in the order in which AV1 places their
/// block payloads, stopping as soon as the caller encounters an unsupported
/// block-syntax class.
///
/// This is the production bridge between partition syntax and block syntax.
/// A callback that returns `Stop` has consumed no bytes on behalf of the block;
/// the decoder therefore never advances into a sibling partition as if block
/// bytes were partition bytes.  The full decoder will replace the stop with a
/// safe block parser as each syntax class is implemented.
fn walk_partition_until_stop<F>(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    context: &FirstBlockContext,
    mut visit: F,
) -> Av1Result<PartitionVisitControl>
where
    F: FnMut(&mut RangeDecoder<'_, '_, '_>, PartitionNode) -> Av1Result<PartitionVisitControl>,
{
    if context.block_width == 0 || context.block_height == 0 {
        return Err(malformed("partition block dimensions are empty"));
    }
    let mut walker = PartitionWalker::new(decoder, context);
    walker.walk(context.level, context.block_x, context.block_y, &mut visit)
}

/// Reconstruct the complete alpha tile for the bounded monochrome class whose
/// block syntax is currently closed: an 8-bit, lossless, intra frame with no
/// restoration or inter-frame tools.
///
/// The callback order is the AV1 block-payload order. Neighbor references are
/// selected from already reconstructed leaves by their checked pixel
/// geometry, so this path does not depend on a fixture-specific partition
/// index table. Unsupported block syntax returns `None`, preserving the
/// explicit pure-Rust gap for broader monochrome and alpha images.
pub(super) fn validate_complete_monochrome_partition(
    data: &SegmentedData<'_, '_>,
    range: Range<usize>,
    context: &FirstBlockContext,
) -> Av1Result<Option<super::block::ReconstructedPlane>> {
    if !complete_monochrome_reconstruction_context(context) {
        return Ok(None);
    }
    let mut decoder = RangeDecoder::new(data, range.start, range.end, context.disable_cdf_update)?;
    if !decode_restoration_prefix(&mut decoder, context) {
        return Ok(None);
    }
    let root_level = context.level;
    let root_size = 32_u32
        .checked_shr(root_level)
        .filter(|&size| size != 0)
        .ok_or_else(|| malformed("monochrome superblock root size is invalid"))?;
    let root_step =
        usize::try_from(root_size).map_err(|_| malformed("monochrome root size exceeds usize"))?;
    let mut walker = PartitionWalker::new(&mut decoder, context);
    let mut block_decoder = super::block::MonochromeLosslessDecoder::new();
    let mut canvas =
        super::raster::MonochromeFrameCanvas::new(context.frame_width, context.frame_height)?;
    let mut leaves = Vec::<super::block::MonochromeLeaf>::new();
    let mut unsupported = false;
    for root_y in (context.block_y..context.block_height).step_by(root_step) {
        for root_x in (context.block_x..context.block_width).step_by(root_step) {
            walker.reset_root();
            walker.set_root_bounds(root_x, root_y, root_size)?;
            let control = walker.walk(root_level, root_x, root_y, &mut |decoder, node| {
                let Some((transform_grid, width, height)) = monochrome_transform_geometry(node)
                else {
                    unsupported = true;
                    return Ok(PartitionVisitControl::Stop);
                };
                let origin_x = node
                    .x
                    .checked_mul(4)
                    .ok_or_else(|| malformed("monochrome leaf x coordinate overflows"))?;
                let origin_y = node
                    .y
                    .checked_mul(4)
                    .ok_or_else(|| malformed("monochrome leaf y coordinate overflows"))?;
                let geometry = super::block::MonochromeBlockGeometry {
                    origin_x,
                    origin_y,
                    width,
                    height,
                    transform_grid,
                };
                let tools = super::block::BlockTools {
                    allow_screen_content_tools: context.allow_screen_content_tools,
                    enable_filter_intra: context.enable_filter_intra,
                    enable_intra_edge_filter: context.enable_intra_edge_filter,
                    transform_mode: context.frame_tools.transform_mode,
                    transform_context: 0,
                    palette_context: Default::default(),
                };
                let decoded = if leaves.is_empty() {
                    block_decoder.decode_origin(decoder, geometry, tools)
                } else {
                    let neighbors = monochrome_neighbors(&leaves, geometry)?;
                    block_decoder.decode_following(decoder, geometry, neighbors, tools)
                };
                let Ok(decoded) = decoded else {
                    unsupported = true;
                    return Ok(PartitionVisitControl::Stop);
                };
                canvas.place_partition_leaf(
                    node.x,
                    node.y,
                    node.width,
                    node.height,
                    decoded.plane(),
                )?;
                leaves.push(decoded);
                Ok(PartitionVisitControl::Continue)
            })?;
            if unsupported || matches!(control, PartitionVisitControl::Stop) {
                return Ok(None);
            }
        }
    }
    if leaves.is_empty() {
        return Ok(None);
    }
    canvas.finish().map(Some)
}

fn complete_monochrome_reconstruction_context(context: &FirstBlockContext) -> bool {
    let dimensions_are_supported = context.frame_width >= 4
        && context.frame_height >= 4
        && context.frame_width <= 128
        && context.frame_height <= 128
        && context.frame_width.is_multiple_of(4)
        && context.frame_height.is_multiple_of(4)
        && context.block_width == context.frame_width / 4
        && context.block_height == context.frame_height / 4
        && context.upscaled_width == context.frame_width;
    context.bit_depth == 8
        && context.monochrome
        && context.all_lossless
        && !context.superres_enabled
        && !context.segmentation_enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.frame_tools.film_grain_present
        && context.block_x == 0
        && context.block_y == 0
        && context.level == 1
        && dimensions_are_supported
        && context.restoration_types == [None; 3]
}

/// Reconstructed 4:2:0 tile plus the frame-filter metadata produced while its
/// tile-local entropy syntax was decoded.
///
/// The pixel planes stay unfiltered until all tiles have been assembled. This
/// is required because CDEF reads neighboring pixels across tile boundaries;
/// applying it to an isolated tile would make the result depend on tile
/// layout.
pub(super) struct Lossy420Reconstruction {
    pub(super) leaf: super::block::FirstLeaf,
    pub(super) subsampling_x: bool,
    pub(super) subsampling_y: bool,
    pub(super) filter_blocks: Vec<super::filter::Block>,
    pub(super) cdef_indices: Vec<Option<usize>>,
    pub(super) cdef_active: Vec<bool>,
    pub(super) loop_parameters: Option<super::filter::Parameters>,
    pub(super) cdef_parameters: Option<super::cdef::FrameParameters>,
}

impl Lossy420Reconstruction {
    /// Apply this tile's filters in isolation for the single-tile path.
    pub(super) fn into_filtered_leaf(self) -> Av1Result<super::block::FirstLeaf> {
        let Lossy420Reconstruction {
            mut leaf,
            subsampling_x,
            subsampling_y,
            filter_blocks,
            cdef_indices,
            cdef_active,
            loop_parameters,
            cdef_parameters,
        } = self;
        let mut canvas =
            super::raster::FrameCanvas::new(leaf.width, leaf.height, subsampling_x, subsampling_y)?;
        canvas.place_planes(leaf.width, leaf.height, &leaf.planes, 0, 0)?;
        leaf.planes = canvas.finish_with_filters(
            loop_parameters,
            &filter_blocks,
            None,
            None,
            cdef_parameters,
            &cdef_indices,
            &cdef_active,
        )?;
        Ok(leaf)
    }
}

/// Reconstruct the complete entropy output for the first general 8-bit 4:2:0
/// frame class.
///
/// The reconstructed planes and filter metadata are returned separately so a
/// multi-tile frame can run the safe frame filters over the fully assembled
/// image. A block syntax class that is not yet supported returns `Ok(None)`
/// rather than exposing a partial canvas.
pub(super) fn validate_complete_lossy_420_partition(
    data: &SegmentedData<'_, '_>,
    range: Range<usize>,
    context: &FirstBlockContext,
) -> Av1Result<Option<Lossy420Reconstruction>> {
    if !complete_lossy_420_reconstruction_context(context) {
        return Ok(None);
    }
    let mut decoder = RangeDecoder::new(data, range.start, range.end, context.disable_cdf_update)?;
    #[cfg(coverage)]
    decoder.enable_operation_trace();
    if !decode_restoration_prefix(&mut decoder, context) {
        return Ok(None);
    }

    let quantization = lossy_quantization_for_context(context);
    let tools = super::block::BlockTools {
        allow_screen_content_tools: context.allow_screen_content_tools,
        enable_filter_intra: context.enable_filter_intra,
        enable_intra_edge_filter: context.enable_intra_edge_filter,
        transform_mode: context.frame_tools.transform_mode,
        transform_context: 0,
        palette_context: Default::default(),
    };
    let root_level = context.level;
    let root_size = 32_u32
        .checked_shr(root_level)
        .filter(|&size| size != 0)
        .ok_or_else(|| malformed("superblock root size is invalid"))?;
    let root_step =
        usize::try_from(root_size).map_err(|_| malformed("superblock root size exceeds usize"))?;
    let mut walker = PartitionWalker::new(&mut decoder, context);
    let Some(mut block_decoder) = super::block::Lossy420Decoder::with_qindex(quantization.qindex)
    else {
        return Ok(None);
    };
    let mut canvas = super::raster::FrameCanvas::new(
        context.frame_width,
        context.frame_height,
        context.subsampling_x,
        context.subsampling_y,
    )?;
    let mut leaves = Vec::<(PartitionNode, super::block::FirstLeaf)>::new();
    let mut filter_blocks = Vec::<super::filter::Block>::new();
    let cdef_region_width = usize::try_from(context.frame_width)
        .map_err(|_| malformed("CDEF frame width exceeds usize"))?
        .div_ceil(64);
    let cdef_region_height = usize::try_from(context.frame_height)
        .map_err(|_| malformed("CDEF frame height exceeds usize"))?
        .div_ceil(64);
    let cdef_active_width = usize::try_from(context.frame_width)
        .map_err(|_| malformed("CDEF frame width exceeds usize"))?
        .div_ceil(8);
    let cdef_active_height = usize::try_from(context.frame_height)
        .map_err(|_| malformed("CDEF frame height exceeds usize"))?
        .div_ceil(8);
    let mut cdef_indices = vec![None; cdef_region_width.saturating_mul(cdef_region_height)];
    let mut cdef_active = vec![false; cdef_active_width.saturating_mul(cdef_active_height)];
    let mut unsupported = false;

    for root_y in (0..context.block_height).step_by(root_step) {
        for root_x in (0..context.block_width).step_by(root_step) {
            let delta_q_mask = if context.level == 0 { 31 } else { 15 };
            let delta_q_at_root =
                context.frame_tools.delta_q_present && ((root_x | root_y) & delta_q_mask) == 0;
            block_decoder.begin_superblock(
                context.frame_tools.cdef.map_or(0, |cdef| cdef.bits),
                delta_q_at_root,
            );
            walker.reset_root();
            walker.set_root_bounds(root_x, root_y, root_size)?;
            let control = walker.walk(root_level, root_x, root_y, &mut |decoder, node| {
                let width = node.width.saturating_mul(4);
                let height = node.height.saturating_mul(4);
                let mut tools = tools;
                tools.palette_context =
                    super::block::PaletteNeighborContext::from_neighbors(node.y, None, None);
                let decoded = if leaves.is_empty() {
                    // A standalone 4x4 image is the one cropped-frame case in
                    // which AV1 codes an 8x8 origin block for a 4x4 visible
                    // result. A 4x4 leaf inside a larger tile is a genuine
                    // 4x4 block and must use the 4x4 syntax sentence.
                    let transform_grid = if context.frame_width == 4
                        && context.frame_height == 4
                        && width == 4
                        && height == 4
                    {
                        super::block::TransformGrid::Square8
                    } else {
                        let Ok(transform_grid) =
                            super::block::TransformGrid::from_luma_dimensions(width, height)
                        else {
                            unsupported = true;
                            return Ok(PartitionVisitControl::Stop);
                        };
                        transform_grid
                    };
                    let standalone_tiny_frame =
                        context.frame_width == 4 && context.frame_height == 4;
                    let has_chroma = if !context.subsampling_x && !context.subsampling_y {
                        !context.monochrome
                    } else {
                        standalone_tiny_frame
                            || (node.width > 1 || node.x % 2 != 0)
                                && (node.height > 1 || node.y % 2 != 0)
                    };
                    if has_chroma {
                        if !context.subsampling_x && !context.subsampling_y {
                            block_decoder.decode_origin_full(
                                decoder,
                                width,
                                height,
                                transform_grid,
                                quantization,
                                tools,
                            )
                        } else {
                            block_decoder.decode_origin_with_grid(
                                decoder,
                                width,
                                height,
                                transform_grid,
                                quantization,
                                tools,
                            )
                        }
                    } else {
                        block_decoder.decode_origin_without_chroma(
                            decoder,
                            width,
                            height,
                            transform_grid,
                            quantization,
                            tools,
                        )
                    }
                } else {
                    let has_chroma = if !context.subsampling_x && !context.subsampling_y {
                        !context.monochrome
                    } else {
                        (node.width > 1 || node.x % 2 != 0) && (node.height > 1 || node.y % 2 != 0)
                    };
                    let above_left = leaves.iter().rev().find(|(prior, _)| {
                        prior.x.saturating_add(prior.width) > node.x
                            && prior.x <= node.x
                            && prior.y.saturating_add(prior.height) == node.y
                    });
                    let above_right = leaves.iter().rev().find(|(prior, _)| {
                        // The angular predictor's top window extends past the
                        // current block, so this is the leaf immediately to
                        // the right of the current covered interval. The
                        // coefficient-context helpers consume only as many
                        // units as the current transform needs and therefore
                        // still use the above-left leaf first.
                        let right_x = if node.width == 1 {
                            node.x.saturating_add(1)
                        } else if node.width == 4 {
                            // A 16x16 leaf is reconstructed as two adjacent
                            // 8x16 contexts. The second top edge starts two
                            // syntax units into the current interval; using
                            // the full interval width skips that adjacent
                            // 8x8 leaf when the partition is split there.
                            node.x.saturating_add(2)
                        } else {
                            node.x.saturating_add(node.width.saturating_sub(1))
                        };
                        prior.x <= right_x
                            && right_x < prior.x.saturating_add(prior.width)
                            && prior.y.saturating_add(prior.height) == node.y
                    });
                    let above_luma_extension = if node.width == 2 {
                        let extension_x = node.x.saturating_add(node.width);
                        leaves.iter().rev().find(|(prior, _)| {
                            prior.x <= extension_x
                                && extension_x < prior.x.saturating_add(prior.width)
                                && prior.y.saturating_add(prior.height) == node.y
                        })
                    } else {
                        None
                    };
                    let full_resolution = !context.subsampling_x && !context.subsampling_y;
                    let chroma_above_y = if full_resolution {
                        node.y
                    } else {
                        node.y.saturating_sub(node.y % 2)
                    };
                    let above_chroma_extension = if node.width == 2 {
                        let extension_x = node.x.saturating_add(node.width);
                        leaves.iter().rev().find(|(prior, _)| {
                            let has_chroma = full_resolution
                                || ((prior.width > 1 || prior.x % 2 != 0)
                                    && (prior.height > 1 || prior.y % 2 != 0));
                            has_chroma
                                && prior.x <= extension_x
                                && extension_x < prior.x.saturating_add(prior.width)
                                && prior.y.saturating_add(prior.height) == chroma_above_y
                        })
                    } else {
                        None
                    };
                    let above_chroma = leaves.iter().rev().find(|(prior, _)| {
                        let has_chroma = full_resolution
                            || ((prior.width > 1 || prior.x % 2 != 0)
                                && (prior.height > 1 || prior.y % 2 != 0));
                        has_chroma
                            && prior.x <= node.x.saturating_add(node.width.saturating_sub(1))
                            && node.x.saturating_add(node.width.saturating_sub(1))
                                < prior.x.saturating_add(prior.width)
                            && prior.y.saturating_add(prior.height) == chroma_above_y
                    });
                    let above_luma_contexts = {
                        let mut candidates: Vec<_> = leaves
                            .iter()
                            .filter(|(prior, _)| {
                                prior.y.saturating_add(prior.height) == node.y
                                    && prior.x < node.x.saturating_add(node.width)
                                    && prior.x.saturating_add(prior.width) > node.x
                            })
                            .collect();
                        candidates.sort_by_key(|(prior, _)| prior.x);
                        let neighbors: Vec<_> = candidates
                            .iter()
                            .map(|&(prior, leaf)| {
                                (
                                    leaf,
                                    usize::try_from(node.x.max(prior.x).saturating_sub(prior.x))
                                        .unwrap_or(0),
                                )
                            })
                            .collect();
                        super::block::combined_luma_edge_contexts_from_positioned_neighbors(
                            &neighbors, 16, true,
                        )
                    };
                    let above_chroma_contexts = if full_resolution {
                        std::array::from_fn(|plane| {
                            let mut candidates: Vec<_> = leaves
                                .iter()
                                .filter(|(prior, _)| {
                                    prior.y.saturating_add(prior.height) == node.y
                                        && prior.x < node.x.saturating_add(node.width)
                                        && prior.x.saturating_add(prior.width) > node.x
                                })
                                .collect();
                            candidates.sort_by_key(|(prior, _)| prior.x);
                            let neighbors: Vec<_> = candidates
                                .iter()
                                .map(|&(prior, leaf)| {
                                    (
                                        leaf,
                                        usize::try_from(
                                            node.x.max(prior.x).saturating_sub(prior.x),
                                        )
                                        .unwrap_or(0),
                                    )
                                })
                                .collect();
                            super::block::combined_full_chroma_edge_contexts_from_positioned_neighbors(
                                &neighbors,
                                plane,
                                usize::try_from(node.width).unwrap_or(0).min(8),
                                true,
                            )
                        })
                    } else if node.width == 4 && node.height == 4 {
                        std::array::from_fn(|plane| {
                            let mut candidates: Vec<_> = leaves
                                .iter()
                                .filter(|(prior, _)| {
                                    prior.y.saturating_add(prior.height) == chroma_above_y
                                        && prior.x < node.x.saturating_add(node.width)
                                        && prior.x.saturating_add(prior.width) > node.x
                                })
                                .collect();
                            candidates.sort_by_key(|(prior, _)| prior.x);
                            let neighbors: Vec<_> = candidates
                                .iter()
                                .map(|&(prior, leaf)| {
                                    (
                                        leaf,
                                        usize::try_from(
                                            node.x.max(prior.x).saturating_sub(prior.x) / 2,
                                        )
                                        .unwrap_or(0),
                                    )
                                })
                                .collect();
                            super::block::combined_chroma_edge_contexts_from_positioned_neighbors(
                                &neighbors,
                                plane,
                                usize::try_from(node.width).unwrap_or(0).div_ceil(2),
                                true,
                            )
                        })
                    } else {
                        std::array::from_fn(|plane| {
                            let neighbors = above_chroma
                                .map(|(_, leaf)| vec![(leaf, 0_usize)])
                                .unwrap_or_default();
                            super::block::combined_chroma_edge_contexts_from_positioned_neighbors(
                                &neighbors,
                                plane,
                                usize::try_from(node.width).unwrap_or(0).div_ceil(2),
                                true,
                            )
                        })
                    };
                    let left_top = leaves.iter().rev().find(|(prior, _)| {
                        prior.x.saturating_add(prior.width) == node.x
                            && prior.y.saturating_add(prior.height) == node.y
                    });
                    let left = leaves.iter().rev().find(|(prior, _)| {
                        let bottom_unit = node.y.saturating_add(node.height.saturating_sub(1));
                        prior.x.saturating_add(prior.width) == node.x
                            && prior.y <= bottom_unit
                            && bottom_unit < prior.y.saturating_add(prior.height)
                    });
                    tools.palette_context =
                        super::block::PaletteNeighborContext::from_neighbors(
                            node.y,
                            above_left.map(|(_, leaf)| leaf),
                            left.map(|(_, leaf)| leaf),
                        );
                    let left_luma_top = leaves.iter().rev().find(|(prior, _)| {
                        prior.x.saturating_add(prior.width) == node.x
                            && prior.y <= node.y
                            && node.y < prior.y.saturating_add(prior.height)
                    });
                    let left_below = leaves.iter().rev().find(|(prior, _)| {
                        let bottom_unit = node.y.saturating_add(node.height);
                        prior.x.saturating_add(prior.width) == node.x
                            && prior.y <= bottom_unit
                            && bottom_unit < prior.y.saturating_add(prior.height)
                    });
                    let chroma_left_x = if full_resolution {
                        node.x
                    } else {
                        node.x.saturating_sub(node.x % 2)
                    };
                    let chroma_left_y = if full_resolution {
                        node.y
                    } else {
                        node.y.saturating_sub(node.y % 2).saturating_add(1)
                    };
                    let left_chroma = leaves.iter().rev().find(|(prior, _)| {
                        prior.x.saturating_add(prior.width) == chroma_left_x
                            && prior.y <= chroma_left_y
                            && chroma_left_y < prior.y.saturating_add(prior.height)
                    });

                    let (
                        left_luma_contexts,
                        luma_top_neighbor,
                        luma_left_edge_8,
                        luma_left_edge_16,
                        luma_left_edge_32,
                        chroma_left_edges_16,
                        chroma_left_edges_8,
                    ) = {
                        let mut candidates: Vec<_> = leaves
                            .iter()
                            .filter(|(prior, _)| {
                                prior.x.saturating_add(prior.width) == node.x
                                    && prior.y < node.y.saturating_add(node.height)
                                    && prior.y.saturating_add(prior.height) > node.y
                            })
                            .collect();
                        candidates.sort_by_key(|(prior, _)| prior.y);
                        let neighbors: Vec<_> = candidates
                            .iter()
                            .map(|&(prior, leaf)| {
                                (
                                    leaf,
                                    usize::try_from(node.y.saturating_sub(prior.y)).unwrap_or(0),
                                )
                            })
                            .collect();
                        let top = neighbors.first().map(|(leaf, _)| *leaf);
                        let contexts =
                            super::block::combined_luma_edge_contexts_from_positioned_neighbors(
                                &neighbors, 16, false,
                            );
                        let luma_edge_8 = if height == 8 && !candidates.is_empty() {
                            let mut complete = true;
                            let edge = std::array::from_fn(|index| {
                                let sample_y = node
                                    .y
                                    .saturating_mul(4)
                                    .saturating_add(u32::try_from(index).unwrap_or(0));
                                candidates
                                    .iter()
                                    .find_map(|candidate| {
                                        let candidate = &**candidate;
                                        let prior = &candidate.0;
                                        let leaf = &candidate.1;
                                        let prior_y = prior.y.saturating_mul(4);
                                        let prior_height = prior.height.saturating_mul(4);
                                        if prior_y <= sample_y
                                            && sample_y < prior_y.saturating_add(prior_height)
                                        {
                                            Some(
                                                super::block::right_edge_at::<1>(
                                                    &leaf.planes[0],
                                                    leaf.width,
                                                    leaf.height,
                                                    sample_y.saturating_sub(prior_y),
                                                )[0],
                                            )
                                        } else {
                                            None
                                        }
                                        })
                                    .unwrap_or_else(|| {
                                        complete = false;
                                        128
                                    })
                            });
                            complete.then_some(edge)
                        } else {
                            None
                        };
                        let luma_edge_16 = if (width == 4 || width == 8 || width == 16)
                            && height == 16
                        {
                            let mut complete = true;
                            let edge = std::array::from_fn(|index| {
                                let sample_y = node
                                    .y
                                    .saturating_mul(4)
                                    .saturating_add(u32::try_from(index).unwrap_or(0));
                                candidates
                                    .iter()
                                    .find_map(|candidate| {
                                        let candidate = &**candidate;
                                        let prior = &candidate.0;
                                        let leaf = &candidate.1;
                                        let prior_y = prior.y.saturating_mul(4);
                                        let prior_height = prior.height.saturating_mul(4);
                                        if prior_y <= sample_y
                                            && sample_y < prior_y.saturating_add(prior_height)
                                        {
                                            Some(
                                                super::block::right_edge_at::<1>(
                                                    &leaf.planes[0],
                                                    leaf.width,
                                                    leaf.height,
                                                    sample_y.saturating_sub(prior_y),
                                                )[0],
                                            )
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or_else(|| {
                                        complete = false;
                                        128
                                    })
                            });

                            complete.then_some(edge)
                        } else {
                            None
                        };
                        let chroma_edges_16 = if full_resolution
                            && (width == 8 || width == 16)
                            && height == 16
                        {
                            std::array::from_fn(|plane| {
                                let mut complete = true;
                                let edge = std::array::from_fn(|index| {
                                    let sample_y = node
                                        .y
                                        .saturating_mul(4)
                                        .saturating_add(u32::try_from(index).unwrap_or(0));
                                    candidates
                                        .iter()
                                        .find_map(|candidate| {
                                            let candidate = &**candidate;
                                            let prior = &candidate.0;
                                            let leaf = &candidate.1;
                                            let prior_y = prior.y.saturating_mul(4);
                                            let prior_height = prior.height.saturating_mul(4);
                                            if prior_y <= sample_y
                                                && sample_y < prior_y.saturating_add(prior_height)
                                            {
                                                Some(
                                                    super::block::right_edge_at::<1>(
                                                        &leaf.planes[plane.saturating_add(1)],
                                                        leaf.width,
                                                        leaf.height,
                                                        sample_y.saturating_sub(prior_y),
                                                    )[0],
                                                )
                                            } else {
                                                None
                                            }
                                        })
                                        .unwrap_or_else(|| {
                                            complete = false;
                                            128
                                        })
                                });
                                complete.then_some(edge)
                            })
                        } else if (width == 4 || (!full_resolution && width == 16))
                            && height == 16
                        {
                            let chroma_left_x = node.x.saturating_sub(node.x % 2);
                            let mut chroma_candidates: Vec<_> = leaves
                                .iter()
                                .filter(|(prior, _)| {
                                    let has_chroma = (prior.width > 1 || prior.x % 2 != 0)
                                        && (prior.height > 1 || prior.y % 2 != 0);
                                    has_chroma
                                        && prior.x.saturating_add(prior.width) == chroma_left_x
                                        && prior.y < node.y.saturating_add(node.height)
                                        && prior.y.saturating_add(prior.height) > node.y
                                })
                                .collect();
                            chroma_candidates.sort_by_key(|(prior, _)| prior.y);

                            std::array::from_fn(|plane| {
                                Some(std::array::from_fn(|index| {
                                    let sample_y = node
                                        .y
                                        .saturating_div(2)
                                        .saturating_mul(4)
                                        .saturating_add(u32::try_from(index).unwrap_or(0));
                                    chroma_candidates
                                        .iter()
                                        .find_map(|candidate| {
                                            let candidate = &**candidate;
                                            let prior = &candidate.0;
                                            let leaf = &candidate.1;
                                            let prior_y =
                                                prior.y.saturating_div(2).saturating_mul(4);
                                            let prior_height = prior
                                                .height
                                                .saturating_div(2)
                                                .max(1)
                                                .saturating_mul(4);
                                            if prior_y <= sample_y
                                                && sample_y < prior_y.saturating_add(prior_height)
                                            {
                                                Some(
                                                    super::block::right_edge_at::<1>(
                                                        &leaf.planes[plane.saturating_add(1)],
                                                        leaf.width.div_ceil(2).max(4),
                                                        leaf.height.div_ceil(2).max(4),
                                                        sample_y.saturating_sub(prior_y),
                                                    )[0],
                                                )
                                            } else {
                                                None
                                            }
                                        })
                                        .unwrap_or(128)
                                }))
                            })
                        } else {
                            [None; 2]
                        };
                        let luma_edge_32 = if width == 32 && height == 32 {
                            let edge = std::array::from_fn(|index| {
                                let sample_y = node
                                    .y
                                    .saturating_mul(4)
                                    .saturating_add(u32::try_from(index).unwrap_or(0));
                                candidates
                                    .iter()
                                    .find_map(|candidate| {
                                        let candidate = &**candidate;
                                        let prior = &candidate.0;
                                        let leaf = &candidate.1;
                                        let prior_y = prior.y.saturating_mul(4);
                                        let prior_height = prior.height.saturating_mul(4);
                                        if prior_y <= sample_y
                                            && sample_y < prior_y.saturating_add(prior_height)
                                        {
                                            Some(
                                                super::block::right_edge_at::<1>(
                                                    &leaf.planes[0],
                                                    leaf.width,
                                                    leaf.height,
                                                    sample_y.saturating_sub(prior_y),
                                                )[0],
                                            )
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or(128)
                            });
                            Some(edge)
                        } else {
                            None
                        };
                        let chroma_edges = if full_resolution && height == 8 {
                            let mut chroma_candidates: Vec<_> = candidates.clone();
                            chroma_candidates.sort_by_key(|(prior, _)| prior.y);

                            std::array::from_fn(|plane| {
                                let mut complete = true;
                                let edge = std::array::from_fn(|index| {
                                    let sample_y = node
                                        .y
                                        .saturating_mul(4)
                                        .saturating_add(u32::try_from(index).unwrap_or(0));
                                    chroma_candidates
                                        .iter()
                                        .find_map(|candidate| {
                                            let candidate = &**candidate;
                                            let prior = &candidate.0;
                                            let leaf = &candidate.1;
                                            let prior_y = prior.y.saturating_mul(4);
                                            let prior_height = prior.height.saturating_mul(4);
                                            if prior_y <= sample_y
                                                && sample_y < prior_y.saturating_add(prior_height)
                                            {
                                                Some(
                                                    super::block::right_edge_at::<1>(
                                                        &leaf.planes[plane.saturating_add(1)],
                                                        leaf.width,
                                                        leaf.height,
                                                        sample_y.saturating_sub(prior_y),
                                                    )[0],
                                                )
                                            } else {
                                                None
                                            }
                                        })
                                        .unwrap_or_else(|| {
                                            complete = false;
                                            128
                                        })
                                });
                                complete.then_some(edge)
                            })
                        } else if (width == 4 || width == 16) && height == 16 {
                            let chroma_left_x = node.x.saturating_sub(node.x % 2);
                            let mut chroma_candidates: Vec<_> = leaves
                                .iter()
                                .filter(|(prior, _)| {
                                    let has_chroma = (prior.width > 1 || prior.x % 2 != 0)
                                        && (prior.height > 1 || prior.y % 2 != 0);
                                    has_chroma
                                        && prior.x.saturating_add(prior.width) == chroma_left_x
                                        && prior.y < node.y.saturating_add(node.height)
                                        && prior.y.saturating_add(prior.height) > node.y
                                })
                                .collect();
                            chroma_candidates.sort_by_key(|(prior, _)| prior.y);

                            std::array::from_fn(|plane| {
                                Some(std::array::from_fn(|index| {
                                    let sample_y = node
                                        .y
                                        .saturating_div(2)
                                        .saturating_mul(4)
                                        .saturating_add(u32::try_from(index).unwrap_or(0));
                                    chroma_candidates
                                        .iter()
                                        .find_map(|candidate| {
                                            let candidate = &**candidate;
                                            let prior = &candidate.0;
                                            let leaf = &candidate.1;
                                            let prior_y =
                                                prior.y.saturating_div(2).saturating_mul(4);
                                            let prior_height = prior
                                                .height
                                                .saturating_div(2)
                                                .max(1)
                                                .saturating_mul(4);
                                            if prior_y <= sample_y
                                                && sample_y < prior_y.saturating_add(prior_height)
                                            {
                                                Some(
                                                    super::block::right_edge_at::<1>(
                                                        &leaf.planes[plane.saturating_add(1)],
                                                        leaf.width.div_ceil(2).max(4),
                                                        leaf.height.div_ceil(2).max(4),
                                                        sample_y.saturating_sub(prior_y),
                                                    )[0],
                                                )
                                            } else {
                                                None
                                            }
                                        })
                                        .unwrap_or(128)
                                }))
                            })
                        } else {
                            [None; 2]
                        };
                        (
                            contexts,
                            top,
                            luma_edge_8,
                        luma_edge_16,
                        luma_edge_32,
                        chroma_edges_16,
                        chroma_edges,
                        )
                    };

                    let left_chroma_contexts = if full_resolution {
                        std::array::from_fn(|plane| {
                            let mut candidates: Vec<_> = leaves
                                .iter()
                                .filter(|(prior, _)| {
                                    prior.x.saturating_add(prior.width) == node.x
                                        && prior.y < node.y.saturating_add(node.height)
                                        && prior.y.saturating_add(prior.height) > node.y
                                })
                                .collect();
                            candidates.sort_by_key(|(prior, _)| prior.y);
                            let neighbors: Vec<_> = candidates
                                .iter()
                                .map(|&(prior, leaf)| {
                                    (
                                        leaf,
                                        usize::try_from(node.y.saturating_sub(prior.y))
                                            .unwrap_or(0),
                                    )
                                })
                                .collect();
                            super::block::combined_full_chroma_edge_contexts_from_positioned_neighbors(
                                &neighbors,
                                plane,
                                usize::try_from(node.height).unwrap_or(0).min(8),
                                false,
                            )
                        })
                    } else {
                        std::array::from_fn(|plane| {
                            let mut candidates: Vec<_> = leaves
                                .iter()
                                .filter(|(prior, _)| {
                                    prior.x.saturating_add(prior.width) == chroma_left_x
                                        && prior.y < node.y.saturating_add(node.height)
                                        && prior.y.saturating_add(prior.height) > chroma_left_y
                                })
                                .collect();
                            candidates.sort_by_key(|(prior, _)| prior.y);
                            let neighbors: Vec<_> = candidates
                                .iter()
                                .map(|&(prior, leaf)| {
                                    (
                                        leaf,
                                        usize::try_from(chroma_left_y.saturating_sub(prior.y) / 2)
                                            .unwrap_or(0),
                                    )
                                })
                                .collect();
                            super::block::combined_chroma_edge_contexts_from_positioned_neighbors(
                                &neighbors,
                                plane,
                                usize::try_from(node.height).unwrap_or(0).div_ceil(2),
                                false,
                            )
                        })
                    };

                    let left_y_offset = left.map_or(0, |(prior, _)| {
                        node.y.saturating_sub(prior.y).saturating_mul(4)
                    });
                    let left_chroma_y_offset = left_chroma.map_or(0, |(prior, _)| {
                        let row_delta = if full_resolution {
                            node.y.saturating_sub(prior.y)
                        } else {
                            node.y
                                .saturating_div(2)
                                .saturating_sub(prior.y.saturating_div(2))
                        };
                        row_delta.saturating_mul(4)
                    });
                    let above_chroma_x_offset = above_chroma.map_or(0, |(prior, _)| {
                        let column_delta = if full_resolution {
                            node.x.saturating_sub(prior.x)
                        } else {
                            node.x
                                .saturating_div(2)
                                .saturating_sub(prior.x.saturating_div(2))
                        };
                        column_delta.saturating_mul(4)
                    });

                    let left_luma_bottom = left_below.and_then(|(prior, leaf)| {
                        super::block::luma_right_edge_after::<8>(
                            leaf,
                            node.y
                                .saturating_add(node.height)
                                .saturating_sub(prior.y)
                                .saturating_mul(4),
                        )
                    });
                    let left_chroma_bottom = std::array::from_fn(|plane| {
                        left_below.and_then(|(prior, leaf)| {
                            let has_chroma = (prior.width > 1 || prior.x % 2 != 0)
                                && (prior.height > 1 || prior.y % 2 != 0);
                            if !has_chroma {
                                return None;
                            }
                            let current_chroma_y = node
                                .y
                                .saturating_add(node.height)
                                .saturating_div(2)
                                .saturating_mul(4);
                            let prior_chroma_y = prior.y.saturating_div(2).saturating_mul(4);
                            let row_offset = current_chroma_y.checked_sub(prior_chroma_y)?;
                            let chroma_height = prior.height.div_ceil(2).max(4);
                            (row_offset.saturating_add(4) <= chroma_height).then(|| {
                                super::block::right_edge_at::<4>(
                                    &leaf.planes[plane.saturating_add(1)],
                                    prior.width.div_ceil(2).max(4),
                                    chroma_height,
                                    row_offset,
                                )
                            })
                        })
                    });
                    let left_full_chroma_bottom_8 = if full_resolution
                        && width == 8
                        && (height == 8 || height == 16)
                    {
                        std::array::from_fn(|plane| {
                            let mut complete = true;
                            let edge = std::array::from_fn(|index| {
                                let sample_y = node
                                    .y
                                    .saturating_add(node.height)
                                    .saturating_mul(4)
                                    .saturating_add(u32::try_from(index).unwrap_or(0));
                                leaves
                                    .iter()
                                    .filter(|(prior, _)| {
                                        prior.x.saturating_add(prior.width) == node.x
                                            && prior.y.saturating_mul(4) <= sample_y
                                            && sample_y
                                                < prior
                                                    .y
                                                    .saturating_add(prior.height)
                                                    .saturating_mul(4)
                                    })
                                    .find_map(|(prior, leaf)| {
                                        let row_offset = sample_y
                                            .checked_sub(prior.y.saturating_mul(4))?;
                                        (row_offset < leaf.height).then(|| {
                                            super::block::right_edge_at::<1>(
                                                &leaf.planes[plane.saturating_add(1)],
                                                leaf.width,
                                                leaf.height,
                                                row_offset,
                                            )[0]
                                        })
                                    })
                                    .unwrap_or_else(|| {
                                        complete = false;
                                        128
                                    })
                            });
                            complete.then_some(edge)
                        })
                    } else {
                        [None; 2]
                    };

                    if let Some((above_prior, above)) = above_left {
                        let above_left_width = above.width;
                        let above_right_width =
                            above_right.map_or(above.width, |(_, leaf)| leaf.width);
                        let above_left_x_offset =
                            node.x.saturating_sub(above_prior.x).saturating_mul(4);
                        let above_right_x_offset = above_right.map_or_else(
                            || node.x.saturating_sub(above_prior.x).saturating_mul(4),
                            |(prior, _)| node.x.saturating_sub(prior.x).saturating_mul(4),
                        );
                        let above_right = above_right.map_or(above, |(_, leaf)| leaf);
                        let neighbors = super::block::VerticalNeighbors {
                            above_left: above,
                            above_right,
                            above_left_width,
                            above_right_width,
                            above_left_x_offset,
                            above_right_x_offset,
                            above_luma_extension: above_luma_extension.map(|(_, leaf)| leaf),
                            above_luma_extension_x_offset: above_luma_extension.map_or(
                                0,
                                |(prior, _)| {
                                    node.x
                                        .saturating_add(node.width)
                                        .saturating_sub(prior.x)
                                        .saturating_mul(4)
                                },
                            ),
                            above_chroma_extension: above_chroma_extension.map(|(_, leaf)| leaf),
                            above_chroma_extension_x_offset: above_chroma_extension.map_or(
                                0,
                                |(prior, _)| {
                                    if full_resolution {
                                        node.x
                                            .saturating_add(node.width)
                                            .saturating_sub(prior.x)
                                            .saturating_mul(4)
                                    } else {
                                        node.x
                                            .saturating_add(node.width)
                                            .saturating_div(2)
                                            .saturating_sub(prior.x.saturating_div(2))
                                            .saturating_mul(4)
                                    }
                                },
                            ),
                            above_luma_contexts,
                            above_chroma_contexts,
                            above_chroma: above_chroma.map(|(_, leaf)| leaf),
                            above_chroma_x_offset,
                            left_top: left_top.map(|(_, leaf)| leaf),
                            left_luma_top: left_luma_top.map(|(_, leaf)| leaf),
                            left: left.map(|(_, leaf)| leaf),
                            left_luma_contexts,
                            left_luma_edge_8: luma_left_edge_8,
                            left_luma_edge_16: luma_left_edge_16,
                            left_luma_edge_32: luma_left_edge_32,
                            left_chroma_contexts,
                            left_chroma: left_chroma.map(|(_, leaf)| leaf),
                            left_chroma_edges_16: chroma_left_edges_16,
                            left_y_offset,
                            left_chroma_y_offset,
                            left_luma_bottom,
                            left_chroma_bottom,
                            left_full_chroma_bottom_8,
                        };
                        if has_chroma {
                            block_decoder.decode_following_vertical(
                                decoder,
                                width,
                                height,
                                quantization,
                                tools,
                                neighbors,
                            )
                        } else {
                            block_decoder.decode_following_vertical_without_chroma(
                                decoder,
                                width,
                                height,
                                quantization,
                                tools,
                                neighbors,
                            )
                        }
                    } else if let Some((_, left)) = left {
                        if has_chroma {
                            block_decoder.decode_following_horizontal_with_chroma(
                                decoder,
                                width,
                                height,
                                quantization,
                                tools,
                                left,
                                luma_top_neighbor,
                                luma_left_edge_8,
                                left_luma_bottom,
                                luma_left_edge_16,
                                chroma_left_edges_16,
                                chroma_left_edges_8,
                                left_full_chroma_bottom_8,
                                left_luma_contexts,
                                left_chroma_contexts,
                            )
                        } else {
                            block_decoder.decode_following_horizontal_without_chroma(
                                decoder,
                                width,
                                height,
                                quantization,
                                tools,
                                left,
                            )
                        }
                    } else {
                        Err(super::block::PortableUnavailable)
                    }
                };
                let decoded = match decoded {
                    Ok(decoded) => decoded,
                    Err(_) => {
                        unsupported = true;
                        return Ok(PartitionVisitControl::Stop);
                    }
                };
                let (cdef_block_active, cdef_index) = block_decoder.cdef_metadata();
                record_cdef_metadata(
                    context.frame_width,
                    context.frame_height,
                    node,
                    cdef_block_active,
                    cdef_index,
                    &mut cdef_indices,
                    &mut cdef_active,
                )?;
                let has_chroma = if !context.subsampling_x && !context.subsampling_y {
                    !context.monochrome
                } else {
                    (context.frame_width == 4 && context.frame_height == 4)
                        || (node.width > 1 || node.x % 2 != 0)
                            && (node.height > 1 || node.y % 2 != 0)
                };
                canvas.place_av1_partition_leaf(
                    node.x,
                    node.y,
                    node.width,
                    node.height,
                    has_chroma,
                    &decoded.planes,
                )?;
                let Some((luma_tx, chroma_tx)) =
                    super::block::filter_transform_dimensions(
                        width,
                        height,
                        &decoded,
                        context.subsampling_x,
                        context.subsampling_y,
                    )
                else {
                    unsupported = true;
                    return Ok(PartitionVisitControl::Stop);
                };
                let x = usize::try_from(node.x)
                    .ok()
                    .and_then(|value| value.checked_mul(4))
                    .ok_or_else(|| malformed("loop-filter x coordinate overflows"))?;
                let y = usize::try_from(node.y)
                    .ok()
                    .and_then(|value| value.checked_mul(4))
                    .ok_or_else(|| malformed("loop-filter y coordinate overflows"))?;
                let width = usize::try_from(width)
                    .map_err(|_| malformed("loop-filter width exceeds usize"))?;
                let height = usize::try_from(height)
                    .map_err(|_| malformed("loop-filter height exceeds usize"))?;
                filter_blocks.push(super::filter::Block {
                    x,
                    y,
                    width,
                    height,
                    luma_tx_width: luma_tx.0,
                    luma_tx_height: luma_tx.1,
                    chroma_tx_width: chroma_tx.0,
                    chroma_tx_height: chroma_tx.1,
                });
                leaves.push((node, decoded));
                Ok(PartitionVisitControl::Continue)
            })?;
            if unsupported || matches!(control, PartitionVisitControl::Stop) {
                return Ok(None);
            }
        }
    }
    if leaves.is_empty() {
        return Ok(None);
    }
    if decoder.symbol_coder_overread() {
        return Ok(None);
    }
    let cdef_frame_parameters = cdef_frame_parameters(context);
    let loop_parameters = loop_filter_parameters(context);
    let planes = canvas.finish()?;
    let leaf = super::block::FirstLeaf {
        width: context.frame_width,
        height: context.frame_height,
        planes,
        luma_predictor: super::block::LumaPredictor::Dc,
        chroma_predictor: None,
        luma_context: 0x40,
        chroma_contexts: [0x40; 2],
        chroma_right_contexts: [[0x40; 8]; 2],
        chroma_bottom_contexts: [[0x40; 8]; 2],
        tx_context_width: 0,
        tx_context_height: 0,
        luma_transform_split: false,
        luma_right_contexts: [0x40; 16],
        luma_bottom_contexts: [0x40; 16],
        palette_cache: Default::default(),
        #[cfg(coverage)]
        entropy_operations: decoder.operation_trace(),
    };
    Ok(Some(Lossy420Reconstruction {
        leaf,
        subsampling_x: context.subsampling_x,
        subsampling_y: context.subsampling_y,
        filter_blocks,
        cdef_indices,
        cdef_active,
        loop_parameters,
        cdef_parameters: cdef_frame_parameters,
    }))
}

fn complete_lossy_420_reconstruction_context(context: &FirstBlockContext) -> bool {
    closed_base_reconstruction_context(context)
        && !context.all_lossless
        && ((context.subsampling_x && context.subsampling_y)
            || (!context.subsampling_x && !context.subsampling_y))
        && context.frame_tools.quantization.is_some()
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && matches!(
            context.frame_tools.cdef,
            None | Some(CdefContext {
                bits: 0..=2,
                y_strength_count: 1..=4,
                uv_strength_count: 1..=4,
                first_y_strength: Some(_),
                first_uv_strength: Some(_),
                ..
            })
        )
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1)
}

fn record_cdef_metadata(
    frame_width: u32,
    frame_height: u32,
    node: PartitionNode,
    active: bool,
    cdef_index: usize,
    cdef_indices: &mut [Option<usize>],
    cdef_active: &mut [bool],
) -> Av1Result<()> {
    let frame_width =
        usize::try_from(frame_width).map_err(|_| malformed("CDEF frame width exceeds usize"))?;
    let frame_height =
        usize::try_from(frame_height).map_err(|_| malformed("CDEF frame height exceeds usize"))?;
    let x = usize::try_from(node.x)
        .ok()
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| malformed("CDEF block x coordinate overflows"))?;
    let y = usize::try_from(node.y)
        .ok()
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| malformed("CDEF block y coordinate overflows"))?;
    let width = usize::try_from(node.width)
        .ok()
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| malformed("CDEF block width overflows"))?;
    let height = usize::try_from(node.height)
        .ok()
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| malformed("CDEF block height overflows"))?;
    let end_x = x.saturating_add(width).min(frame_width);
    let end_y = y.saturating_add(height).min(frame_height);
    if x >= end_x || y >= end_y {
        return Err(malformed("CDEF block is outside the frame"));
    }

    let active_width = frame_width.div_ceil(8);
    let region_width = frame_width.div_ceil(64);
    if active {
        for block_y in (y..end_y).step_by(8) {
            for block_x in (x..end_x).step_by(8) {
                let index = block_y
                    .checked_div(8)
                    .and_then(|row| row.checked_mul(active_width))
                    .and_then(|row| row.checked_add(block_x.checked_div(8)?))
                    .ok_or_else(|| malformed("CDEF active-map index overflows"))?;
                let Some(slot) = cdef_active.get_mut(index) else {
                    return Err(malformed("CDEF active-map index exceeds its frame"));
                };
                *slot = true;
            }
        }

        for region_y in (y..end_y).step_by(64) {
            for region_x in (x..end_x).step_by(64) {
                let index = region_y
                    .checked_div(64)
                    .and_then(|row| row.checked_mul(region_width))
                    .and_then(|row| row.checked_add(region_x.checked_div(64)?))
                    .ok_or_else(|| malformed("CDEF index-map index overflows"))?;
                let Some(slot) = cdef_indices.get_mut(index) else {
                    return Err(malformed("CDEF index-map index exceeds its frame"));
                };
                if slot.is_none() {
                    *slot = Some(cdef_index);
                }
            }
        }
    }
    Ok(())
}

fn cdef_frame_parameters(context: &FirstBlockContext) -> Option<super::cdef::FrameParameters> {
    let cdef = context.frame_tools.cdef?;

    Some(super::cdef::FrameParameters {
        damping: cdef.damping,
        bit_depth: context.bit_depth,
        y_strengths: cdef.y_strengths,
        uv_strengths: cdef.uv_strengths,
        y_strength_count: cdef.y_strength_count,
        uv_strength_count: cdef.uv_strength_count,
    })
}

fn loop_filter_parameters(context: &FirstBlockContext) -> Option<super::filter::Parameters> {
    let loop_filter = context.frame_tools.loop_filter;
    let intra_delta = if loop_filter.delta_enabled {
        loop_filter.reference_deltas[0].saturating_add(loop_filter.mode_deltas[0])
    } else {
        0
    };
    let intra_level = |level: u32| {
        if level == 0 {
            0
        } else {
            let adjusted = i32::try_from(level)
                .unwrap_or(i32::MAX)
                .saturating_add(intra_delta)
                .clamp(0, 63);
            u32::try_from(adjusted).unwrap_or_default()
        }
    };
    (loop_filter.level_y != [0, 0] || loop_filter.level_u != 0 || loop_filter.level_v != 0)
        .then_some(super::filter::Parameters {
            luma_vertical: intra_level(loop_filter.level_y[0]),
            luma_horizontal: intra_level(loop_filter.level_y[1]),
            chroma_u: intra_level(loop_filter.level_u),
            chroma_v: intra_level(loop_filter.level_v),
            sharpness: loop_filter.sharpness,
            bit_depth: context.bit_depth,
        })
}

/// Reconstruct the first complete color-frame class: an 8-bit, lossless,
/// single-tile 64×64 4:4:4 intra frame made from 8×8 terminal blocks.
///
/// The partition walker and the block decoder share one range decoder and one
/// adaptive CDF state. Every block is placed into a checked canvas before the
/// result is published, so an unsupported predictor or a geometry mismatch
/// returns an explicit portable gap without exposing a partial image.
pub(super) fn validate_complete_lossless_444_partition(
    data: &SegmentedData<'_, '_>,
    range: Range<usize>,
    context: &FirstBlockContext,
) -> Av1Result<Option<super::block::FirstLeaf>> {
    if !complete_lossless_444_reconstruction_context(context) {
        return Ok(None);
    }
    let mut decoder = RangeDecoder::new(data, range.start, range.end, context.disable_cdf_update)?;
    #[cfg(coverage)]
    decoder.enable_operation_trace();
    if !decode_restoration_prefix(&mut decoder, context) {
        return Ok(None);
    }
    let mut walker = PartitionWalker::new(&mut decoder, context);
    let sample_depth = super::sample_depth::SampleDepth::new(context.bit_depth)
        .ok_or_else(|| malformed("unsupported lossless 4:4:4 sample depth"))?;
    let mut block_decoder = super::block::Lossless444Decoder::new(sample_depth);
    let mut canvas =
        super::raster::FrameCanvas::new(context.frame_width, context.frame_height, false, false)?;
    let mut leaves = Vec::<super::block::Lossless444Leaf>::new();
    let mut unsupported = false;
    let control = walker.walk(
        context.level,
        context.block_x,
        context.block_y,
        &mut |decoder, node| {
            let (width, height, transform_grid) = match (node.width, node.height) {
                (1, 1) => (4, 4, super::block::TransformGrid::Square4),
                (1, 2) => (4, 8, super::block::TransformGrid::Vertical4x8),
                (2, 2) => (8, 8, super::block::TransformGrid::Square8),
                (4, 4) => (16, 16, super::block::TransformGrid::Square16),
                (4, 1) => (16, 4, super::block::TransformGrid::Horizontal16x4),
                (1, 4) => (4, 16, super::block::TransformGrid::Vertical4x16),
                (2, 1) => (8, 4, super::block::TransformGrid::Horizontal8x4),
                (4, 2) => (16, 8, super::block::TransformGrid::Horizontal16x8),
                (4, 8) => (16, 32, super::block::TransformGrid::Vertical16x32),
                (8, 4) => (32, 16, super::block::TransformGrid::Horizontal32x16),
                (2, 4) => (8, 16, super::block::TransformGrid::Vertical8x16),
                (8, 2) => (32, 8, super::block::TransformGrid::Horizontal32x8),
                (2, 8) => (8, 32, super::block::TransformGrid::Vertical8x32),
                (8, 8) => (32, 32, super::block::TransformGrid::Square32),
                _ => {
                    unsupported = true;
                    return Ok(PartitionVisitControl::Stop);
                }
            };
            let origin_x = node
                .x
                .checked_mul(4)
                .ok_or_else(|| malformed("color leaf x coordinate overflows"))?;
            let origin_y = node
                .y
                .checked_mul(4)
                .ok_or_else(|| malformed("color leaf y coordinate overflows"))?;
            let geometry = super::block::Lossless444BlockGeometry {
                origin_x,
                origin_y,
                width,
                height,
                transform_grid,
            };
            let tools = super::block::BlockTools {
                allow_screen_content_tools: context.allow_screen_content_tools,
                enable_filter_intra: context.enable_filter_intra,
                enable_intra_edge_filter: context.enable_intra_edge_filter,
                transform_mode: context.frame_tools.transform_mode,
                transform_context: 0,
                palette_context: Default::default(),
            };
            let above = std::array::from_fn(|segment| {
                let segment_x = geometry
                    .origin_x
                    .checked_add(u32::try_from(segment.saturating_mul(4)).ok()?)?;
                (segment < usize::try_from(node.width).ok()?).then_some(())?;
                leaves.iter().rev().find(|leaf| {
                    leaf.origin_y().checked_add(leaf.height()) == Some(geometry.origin_y)
                        && segment_x >= leaf.origin_x()
                        && segment_x < leaf.origin_x().saturating_add(leaf.width())
                })
            });
            let above_right = geometry
                .origin_x
                .checked_add(geometry.width)
                .and_then(|segment_x| {
                    leaves.iter().rev().find(|leaf| {
                        leaf.origin_y().checked_add(leaf.height()) == Some(geometry.origin_y)
                            && segment_x >= leaf.origin_x()
                            && segment_x < leaf.origin_x().saturating_add(leaf.width())
                    })
                });
            let left = std::array::from_fn(|segment| {
                let segment_y = geometry
                    .origin_y
                    .checked_add(u32::try_from(segment.saturating_mul(4)).ok()?)?;
                (segment < usize::try_from(node.height).ok()?).then_some(())?;
                leaves.iter().rev().find(|leaf| {
                    leaf.origin_x().checked_add(leaf.width()) == Some(geometry.origin_x)
                        && segment_y >= leaf.origin_y()
                        && segment_y < leaf.origin_y().saturating_add(leaf.height())
                })
            });
            let left_below = std::array::from_fn(|segment| {
                let segment_y = geometry
                    .origin_y
                    .saturating_add(geometry.height)
                    .checked_add(u32::try_from(segment.saturating_mul(4)).ok()?)?;
                leaves.iter().rev().find(|leaf| {
                    leaf.origin_x().checked_add(leaf.width()) == Some(geometry.origin_x)
                        && segment_y >= leaf.origin_y()
                        && segment_y < leaf.origin_y().saturating_add(leaf.height())
                })
            });
            let above_left = leaves.iter().rev().find(|leaf| {
                leaf.origin_x().checked_add(leaf.width()) == Some(geometry.origin_x)
                    && leaf.origin_y().checked_add(leaf.height()) == Some(geometry.origin_y)
            });
            let decoded = if leaves.is_empty() {
                block_decoder.decode_origin(decoder, geometry, tools)
            } else {
                block_decoder.decode_following(
                    decoder,
                    geometry,
                    super::block::Lossless444Neighbors {
                        above_left,
                        above,
                        above_right,
                        left,
                        left_below,
                    },
                    tools,
                )
            };
            let Ok(decoded) = decoded else {
                unsupported = true;
                return Ok(PartitionVisitControl::Stop);
            };
            canvas.place_partition_leaf(
                node.x,
                node.y,
                node.width,
                node.height,
                decoded.planes(),
            )?;
            leaves.push(decoded);
            Ok(PartitionVisitControl::Continue)
        },
    )?;
    if unsupported || matches!(control, PartitionVisitControl::Stop) || leaves.is_empty() {
        return Ok(None);
    }
    let planes = canvas.finish()?;
    Ok(Some(super::block::FirstLeaf {
        width: context.frame_width,
        height: context.frame_height,
        planes,
        luma_predictor: super::block::LumaPredictor::Dc,
        chroma_predictor: None,
        luma_context: 0x40,
        chroma_contexts: [0x40; 2],
        chroma_right_contexts: [[0x40; 8]; 2],
        chroma_bottom_contexts: [[0x40; 8]; 2],
        tx_context_width: 0,
        tx_context_height: 0,
        luma_transform_split: false,
        luma_right_contexts: [0x40; 16],
        luma_bottom_contexts: [0x40; 16],
        palette_cache: Default::default(),
        #[cfg(coverage)]
        entropy_operations: decoder.operation_trace(),
    }))
}

fn complete_lossless_444_reconstruction_context(context: &FirstBlockContext) -> bool {
    let dimensions_are_supported = context.frame_width >= 4
        && context.frame_height >= 4
        && context.frame_width <= 128
        && context.frame_height <= 128
        && context.frame_width.is_multiple_of(4)
        && context.frame_height.is_multiple_of(4)
        && context.block_width == context.frame_width / 4
        && context.block_height == context.frame_height / 4
        && context.upscaled_width == context.frame_width;
    matches!(context.bit_depth, 8 | 10)
        && !context.superres_enabled
        && !context.segmentation_enabled
        && !context.skip_mode_enabled
        && !context.monochrome
        && !context.subsampling_x
        && !context.subsampling_y
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1)
        && dimensions_are_supported
        && context.all_lossless
        && context.restoration_types == [None; 3]
}

fn lossy_quantization_for_context(context: &FirstBlockContext) -> super::block::LossyQuantization {
    let frame_quantization = context
        .frame_tools
        .quantization
        .unwrap_or(QuantizationContext {
            base: context.frame_tools.segment_qindex,
            y_dc_delta: 0,
            u_dc_delta: 0,
            u_ac_delta: 0,
            v_dc_delta: 0,
            v_ac_delta: 0,
            different_uv_delta: false,
            using_matrix: false,
            matrix_y: 0,
            matrix_u: 0,
            matrix_v: 0,
        });
    super::block::LossyQuantization {
        qindex: context.frame_tools.segment_qindex,
        delta_q_present: context.frame_tools.delta_q_present,
        resolution_log2: context.frame_tools.delta_q_resolution_log2,
        y_dc_delta: frame_quantization.y_dc_delta,
        y_ac_delta: 0,
        u_dc_delta: frame_quantization.u_dc_delta,
        u_ac_delta: frame_quantization.u_ac_delta,
        v_dc_delta: frame_quantization.v_dc_delta,
        v_ac_delta: frame_quantization.v_ac_delta,
        using_matrix: frame_quantization.using_matrix,
        matrix_y: frame_quantization.matrix_y,
        matrix_u: frame_quantization.matrix_u,
        matrix_v: frame_quantization.matrix_v,
    }
}

fn monochrome_transform_geometry(
    node: PartitionNode,
) -> Option<(super::block::TransformGrid, u32, u32)> {
    match (node.width, node.height) {
        (1, 1) => Some((super::block::TransformGrid::Square4, 4, 4)),
        (1, 2) => Some((super::block::TransformGrid::Vertical4x8, 4, 8)),
        (1, 4) => Some((super::block::TransformGrid::Vertical4x16, 4, 16)),
        (2, 1) => Some((super::block::TransformGrid::Horizontal8x4, 8, 4)),
        (2, 2) => Some((super::block::TransformGrid::Square8, 8, 8)),
        (4, 1) => Some((super::block::TransformGrid::Horizontal16x4, 16, 4)),
        (4, 2) => Some((super::block::TransformGrid::Horizontal16x8, 16, 8)),
        (2, 4) => Some((super::block::TransformGrid::Vertical8x16, 8, 16)),
        (4, 4) => Some((super::block::TransformGrid::Square16, 16, 16)),
        (4, 8) => Some((super::block::TransformGrid::Vertical16x32, 16, 32)),
        (4, 16) => Some((super::block::TransformGrid::Vertical16x64, 16, 64)),
        (8, 4) => Some((super::block::TransformGrid::Horizontal32x16, 32, 16)),
        (8, 2) => Some((super::block::TransformGrid::Horizontal32x8, 32, 8)),
        (2, 8) => Some((super::block::TransformGrid::Vertical8x32, 8, 32)),
        (8, 8) => Some((super::block::TransformGrid::Square32, 32, 32)),
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct MonochromeNeighborIndices {
    above_left: Option<usize>,
    above: Option<usize>,
    above_right: [Option<usize>; 8],
    left: Option<usize>,
    left_below: Option<usize>,
}

fn monochrome_neighbors<'a>(
    leaves: &'a [super::block::MonochromeLeaf],
    geometry: super::block::MonochromeBlockGeometry,
) -> Av1Result<super::block::MonochromeNeighbors<'a>> {
    let above_left = geometry
        .origin_x
        .checked_sub(1)
        .zip(geometry.origin_y.checked_sub(1))
        .and_then(|(x, y)| {
            leaves
                .iter()
                .enumerate()
                .rev()
                .find(|(_, leaf)| leaf.contains_sample(x, y))
                .map(|(index, _)| index)
        });
    let above = leaves
        .iter()
        .enumerate()
        .rev()
        .find(|(_, leaf)| {
            leaf.origin_y().checked_add(leaf.height()) == Some(geometry.origin_y)
                && leaf.contains_sample(geometry.origin_x, geometry.origin_y.saturating_sub(1))
        })
        .map(|(index, _)| index);
    let mut above_right_candidates = leaves
        .iter()
        .enumerate()
        .filter(|(_, leaf)| {
            leaf.origin_y().checked_add(leaf.height()) == Some(geometry.origin_y)
                && leaf.origin_x() >= geometry.origin_x
        })
        .map(|(index, leaf)| (leaf.origin_x(), index))
        .collect::<Vec<_>>();
    above_right_candidates.sort_unstable_by_key(|(origin_x, _)| *origin_x);
    let mut above_right = [None; 8];
    for (slot, (_, index)) in above_right_candidates.into_iter().take(8).enumerate() {
        above_right[slot] = Some(index);
    }
    let left = leaves
        .iter()
        .enumerate()
        .rev()
        .find(|(_, leaf)| {
            leaf.origin_x().checked_add(leaf.width()) == Some(geometry.origin_x)
                && leaf.contains_sample(geometry.origin_x.saturating_sub(1), geometry.origin_y)
        })
        .map(|(index, _)| index);
    let left_below = leaves
        .iter()
        .enumerate()
        .filter(|(_, leaf)| {
            leaf.origin_x().checked_add(leaf.width()) == Some(geometry.origin_x)
                && leaf.origin_y() > geometry.origin_y
        })
        .min_by_key(|(_, leaf)| leaf.origin_y())
        .map(|(index, _)| index);
    let indices = MonochromeNeighborIndices {
        above_left,
        above,
        above_right,
        left,
        left_below,
    };
    Ok(super::block::MonochromeNeighbors {
        above_left: indices.above_left.and_then(|index| leaves.get(index)),
        above: indices.above.and_then(|index| leaves.get(index)),
        above_right: std::array::from_fn(|slot| {
            indices.above_right[slot].and_then(|index| leaves.get(index))
        }),
        left: indices.left.and_then(|index| leaves.get(index)),
        left_below: indices.left_below.and_then(|index| leaves.get(index)),
    })
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_partition_walker_paths() {
    for symbol in 0..=10 {
        let kind = PartitionKind::from_symbol(symbol);
        if let Ok(kind) = kind {
            let _ = (kind.symbol(), kind.is_recursive());
        }
    }

    let mut contexts = PartitionContexts {
        origin_x: 1,
        origin_y: 1,
        above: [0; 32],
        left: [0; 32],
    };
    let _ = contexts.cell(0, 1);
    let _ = contexts.cell(100, 1);
    let _ = contexts.context(5, 1, 1);
    let _ = contexts.record(0, PartitionKind::Split, 1, 1, 2, 2);
    let _ = contexts.record(1, PartitionKind::None, 1, 1, 2, 2);

    let inputs = [0_u8, 1, 0x3f, 0x55, 0x80, 0xaa, 0xff];
    for (width, height, level, monochrome, subsampling_y) in [
        (32, 32, 0, false, false),
        (32, 8, 1, true, false),
        (8, 32, 1, true, true),
        (8, 8, 1, false, true),
        (4, 4, 4, false, true),
    ] {
        let mut context = coverage_context();
        context.block_width = width;
        context.block_height = height;
        context.level = level;
        context.monochrome = monochrome;
        context.subsampling_y = subsampling_y;
        for fill in inputs {
            let input = [fill; 256];
            let spans = [super::super::samples::ByteSpan {
                start: 0,
                end: input.len(),
            }];
            let data = SegmentedData::new(&input, &spans).unwrap();
            let mut decoder = RangeDecoder::new(&data, 0, input.len(), false).unwrap();
            let _ = walk_partition_until_stop(&mut decoder, &context, |_decoder, _node| {
                Ok(PartitionVisitControl::Continue)
            });
        }
    }

    let mut invalid = coverage_context();
    invalid.level = 5;
    let input = [0_u8; 32];
    let spans = [super::super::samples::ByteSpan {
        start: 0,
        end: input.len(),
    }];
    let data = SegmentedData::new(&input, &spans).unwrap();
    let mut decoder = RangeDecoder::new(&data, 0, input.len(), false).unwrap();
    let _ = walk_partition_until_stop(&mut decoder, &invalid, |_decoder, _node| {
        Ok(PartitionVisitControl::Continue)
    });
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

fn closed_monochrome_reconstruction_context(context: &FirstBlockContext) -> bool {
    (context.bit_depth == 8)
        & !context.superres_enabled
        & !context.segmentation_enabled
        & !context.skip_mode_enabled
        & context.monochrome
        & !context.frame_tools.film_grain_present
        & (context.block_x == 0)
        & (context.block_y == 0)
        & context.all_lossless
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
        y_strengths: [0, 0, 0, 0],
        uv_strengths: [0, 0, 0, 0],
        first_y_strength: Some(0),
        first_uv_strength: Some(0),
    }),
    restoration_present: false,
    transform_mode: 1,
    reduced_transform_set: false,
    film_grain_present: false,
};

fn closed_lossy_420_frame_context(context: &FirstBlockContext) -> bool {
    closed_lossy_420_frame_context_with_delta_q(
        context,
        CLOSED_LOSSY_420_FRAME_TOOLS.delta_q_present,
    )
}

fn closed_lossy_420_frame_context_with_delta_q(
    context: &FirstBlockContext,
    delta_q_present: bool,
) -> bool {
    let mut expected_frame_tools = CLOSED_LOSSY_420_FRAME_TOOLS;
    expected_frame_tools.delta_q_present = delta_q_present;
    closed_base_reconstruction_context(context)
        & !context.all_lossless
        & context.subsampling_x
        & context.subsampling_y
        & !context.disable_cdf_update
        & !context.allow_screen_content_tools
        & (context.restoration_types == [None; 3])
        & (context.restoration_unit_size_log2 == [8; 2])
        & (context.frame_tools == expected_frame_tools)
}

fn closed_lossy_444_16x16_reconstruction_context(context: &FirstBlockContext) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    let complete = complete_lossy_420_reconstruction_context(context);
    let geometry = !context.subsampling_x
        && !context.subsampling_y
        && context.frame_width == 16
        && context.frame_height == 16
        && context.upscaled_width == 16
        && context.block_width == 4
        && context.block_height == 4
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1);
    let no_delta_q = !context.frame_tools.delta_q_present;
    let no_cdef = context.frame_tools.cdef.is_none();
    let zero_loop_filter = context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0;
    let transform_state =
        context.frame_tools.transform_mode == 1 && !context.frame_tools.reduced_transform_set;
    let segment_state = !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base;
    let quantization_state = quantization.base != 0;
    complete
        && geometry
        && no_delta_q
        && no_cdef
        && zero_loop_filter
        && transform_state
        && segment_state
        && quantization_state
}

fn closed_lossy_420_reconstruction_context(context: &FirstBlockContext) -> bool {
    closed_lossy_420_frame_context(context)
        & matches!((context.frame_width, context.frame_height), (4, 4) | (8, 8))
}

fn closed_lossy_420_recursive_split_context(context: &FirstBlockContext) -> bool {
    closed_lossy_420_frame_context(context)
        & (context.frame_width == 16)
        & (context.frame_height == 8)
}

fn closed_lossy_420_16x16_vertical_pair_context(context: &FirstBlockContext) -> bool {
    closed_lossy_420_frame_context(context)
        & (context.level == 3)
        & (context.block_width == 4)
        & (context.block_height == 4)
        & (context.frame_width == 16)
        & (context.frame_height == 16)
}

fn closed_lossy_420_square_split_context(context: &FirstBlockContext) -> bool {
    closed_lossy_420_frame_context(context)
        & (context.frame_width == 16)
        & (context.frame_height == 16)
}

fn closed_lossy_420_square64_split_context(context: &FirstBlockContext) -> bool {
    complete_lossy_420_reconstruction_context(context)
        & (context.level == 0)
        & (context.block_width == 16)
        & (context.block_height == 16)
        & (context.frame_width == 64)
        & (context.frame_height == 64)
        & (context.upscaled_width == 64)
        & !context.frame_tools.delta_q_present
        & (context.frame_tools.transform_mode == 2)
        & !context.frame_tools.reduced_transform_set
        & context.frame_tools.cdef.is_none()
}

fn closed_lossy_420_qcat3_horizontal_four_context(context: &FirstBlockContext) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };

    let base_context = context.bit_depth == 8
        && !context.superres_enabled
        && !context.monochrome
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.disable_cdf_update
        && !context.frame_tools.film_grain_present;
    let geometry = matches!(context.level, 0 | 1)
        && context.block_width == 4
        && context.block_height == 4
        && context.block_x == 0
        && context.block_y == 0
        && context.frame_width == 16
        && context.frame_height == 16
        && context.upscaled_width == 16
        && context.subsampling_x
        && context.subsampling_y;
    let sequence_tools = !context.enable_filter_intra && !context.enable_intra_edge_filter;
    let quantization_state = (121..=255).contains(&quantization.base)
        && context.frame_tools.segment_qindex == quantization.base
        && !context.frame_tools.segment_lossless
        && quantization.y_dc_delta == 0
        && quantization.u_dc_delta == -16
        && quantization.u_ac_delta == -16
        && quantization.v_dc_delta == -16
        && quantization.v_ac_delta == -16
        && quantization.using_matrix
        && quantization.matrix_y == 6
        && quantization.matrix_u == 7
        && quantization.matrix_v == 7;
    let tile_delta_state =
        !context.frame_tools.delta_q_present && !context.frame_tools.delta_lf_present;
    let no_effective_filters = context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context.frame_tools.cdef.is_none()
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && context.restoration_unit_size_log2 == [8; 2];
    let transform_state =
        context.frame_tools.transform_mode == 1 && !context.frame_tools.reduced_transform_set;

    base_context
        && geometry
        && sequence_tools
        && quantization_state
        && tile_delta_state
        && no_effective_filters
        && transform_state
}

fn closed_lossy_420_horizontal_four_split_context(context: &FirstBlockContext) -> bool {
    // The H4 helper does not arm a superblock delta-q sentence. Keep the
    // existing exact class and add only the independently evidenced qcat-three
    // frame-tools class for the predictor-enabled 16x16 witness.
    let legacy_exact = closed_lossy_420_frame_context_with_delta_q(context, false)
        && (context.frame_width == 16)
        && (context.frame_height == 16);
    legacy_exact || closed_lossy_420_qcat3_horizontal_four_context(context)
}

fn decode_closed_leaf(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    context: &FirstBlockContext,
    transform_grid: super::block::TransformGrid,
) -> Av1Result<Option<super::block::FirstLeaf>> {
    let reconstructed = super::block::decode_first_lossless_444_leaf(
        decoder,
        context.frame_width,
        context.frame_height,
        transform_grid,
        super::block::BlockTools {
            allow_screen_content_tools: context.allow_screen_content_tools,
            enable_filter_intra: context.enable_filter_intra,
            enable_intra_edge_filter: context.enable_intra_edge_filter,
            transform_mode: context.frame_tools.transform_mode,
            transform_context: 0,
            palette_context: Default::default(),
        },
    );
    finish_closed_leaf(decoder, reconstructed)
}

fn decode_closed_420_leaf(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    context: &FirstBlockContext,
    transform_grid: super::block::TransformGrid,
) -> Av1Result<Option<super::block::FirstLeaf>> {
    let reconstructed = super::block::decode_first_lossless_420_leaf(
        decoder,
        context.frame_width,
        context.frame_height,
        transform_grid,
        super::block::BlockTools {
            allow_screen_content_tools: context.allow_screen_content_tools,
            enable_filter_intra: context.enable_filter_intra,
            enable_intra_edge_filter: context.enable_intra_edge_filter,
            transform_mode: context.frame_tools.transform_mode,
            transform_context: 0,
            palette_context: Default::default(),
        },
    );
    finish_closed_leaf(decoder, reconstructed)
}

fn decode_closed_lossy_420_leaf(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    context: &FirstBlockContext,
) -> Av1Result<Option<super::block::FirstLeaf>> {
    let quantization = lossy_quantization_for_context(context);
    let reconstructed = super::block::decode_first_lossy_420_leaf(
        decoder,
        context.frame_width,
        context.frame_height,
        quantization,
        super::block::BlockTools {
            allow_screen_content_tools: context.allow_screen_content_tools,
            enable_filter_intra: context.enable_filter_intra,
            enable_intra_edge_filter: context.enable_intra_edge_filter,
            transform_mode: context.frame_tools.transform_mode,
            transform_context: 0,
            palette_context: Default::default(),
        },
    );
    finish_closed_leaf(decoder, reconstructed)
}

fn decode_closed_lossy_444_16x16_leaf(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    context: &FirstBlockContext,
) -> Av1Result<Option<super::block::FirstLeaf>> {
    let quantization = lossy_quantization_for_context(context);
    let reconstructed = super::block::decode_first_lossy_444_16x16_leaf(
        decoder,
        quantization,
        super::block::BlockTools {
            allow_screen_content_tools: context.allow_screen_content_tools,
            enable_filter_intra: context.enable_filter_intra,
            enable_intra_edge_filter: context.enable_intra_edge_filter,
            transform_mode: context.frame_tools.transform_mode,
            transform_context: 0,
            palette_context: Default::default(),
        },
    );
    finish_closed_leaf(decoder, reconstructed)
}

fn finish_closed_leaf(
    _decoder: &RangeDecoder<'_, '_, '_>,
    reconstructed: super::block::PortableResult<super::block::FirstLeaf>,
) -> Av1Result<Option<super::block::FirstLeaf>> {
    if _decoder.symbol_coder_overread() {
        return Ok(None);
    }
    #[expect(
        clippy::manual_ok_err,
        reason = "PortableUnavailable is the explicit pure-Rust unsupported outcome, not an erased AV1 failure"
    )]
    let reconstructed = match reconstructed {
        Ok(leaf) => Some(leaf),
        Err(super::block::PortableUnavailable) => None,
    };
    #[cfg(coverage)]
    let reconstructed = reconstructed.map(|mut leaf| {
        leaf.entropy_operations = _decoder.operation_trace();
        leaf
    });
    Ok(reconstructed)
}

/// Decode the first real partition syntax element from one tile.
///
/// `block_width` and `block_height` use dav1d's padded four-pixel units;
/// `block_x` and `block_y` are the tile's first superblock in those units.
pub(super) fn validate_first_partition(
    data: &SegmentedData<'_, '_>,
    range: Range<usize>,
    context: &FirstBlockContext,
) -> Av1Result<Option<super::block::FirstLeaf>> {
    if complete_lossless_444_reconstruction_context(context) {
        return validate_complete_lossless_444_partition(data, range, context);
    }
    if context.frame_width == 64
        && context.frame_height == 64
        && !context.monochrome
        && !context.subsampling_x
        && !context.subsampling_y
    {
        let mut diagnostic_decoder =
            RangeDecoder::new(data, range.start, range.end, context.disable_cdf_update)?;
        if decode_restoration_prefix(&mut diagnostic_decoder, context) {
            let mut visited = 0_u32;
            let _ = walk_partition_until_stop(&mut diagnostic_decoder, context, |decoder, node| {
                if visited == 0 {
                    let _ = super::block::decode_first_lossless_444_leaf(
                        decoder,
                        node.width.saturating_mul(4),
                        node.height.saturating_mul(4),
                        super::block::TransformGrid::Square8,
                        super::block::BlockTools {
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            palette_context: Default::default(),
                        },
                    );
                    visited = visited.saturating_add(1);
                    Ok(PartitionVisitControl::Continue)
                } else {
                    Ok(PartitionVisitControl::Stop)
                }
            });
        }
    }
    let closed_monochrome_class = closed_monochrome_reconstruction_context(context);
    if closed_monochrome_class {
        let mut decoder =
            RangeDecoder::new(data, range.start, range.end, context.disable_cdf_update)?;
        if !decode_restoration_prefix(&mut decoder, context) {
            return Ok(None);
        }
        let mut first_leaf = None;
        let mut first_monochrome = None;
        let mut block_decoder = super::block::MonochromeLosslessDecoder::new();
        let mut visited = 0_u32;
        let _ = walk_partition_until_stop(&mut decoder, context, |decoder, node| {
            let (transform_grid, width, height) = match (node.width, node.height) {
                (2, 2) => (super::block::TransformGrid::Square8, 8, 8),
                (1, 1) => (super::block::TransformGrid::Square4, 4, 4),
                (1, 2) => (super::block::TransformGrid::Vertical4x8, 4, 8),
                (1, 4) => (super::block::TransformGrid::Vertical4x16, 4, 16),
                (2, 1) => (super::block::TransformGrid::Horizontal8x4, 8, 4),
                (4, 4) => (super::block::TransformGrid::Square16, 16, 16),
                (4, 2) => (super::block::TransformGrid::Horizontal16x8, 16, 8),
                (2, 4) => (super::block::TransformGrid::Vertical8x16, 8, 16),
                (8, 2) => (super::block::TransformGrid::Horizontal32x8, 32, 8),
                (2, 8) => (super::block::TransformGrid::Vertical8x32, 8, 32),
                _ => return Ok(PartitionVisitControl::Stop),
            };
            let tools = super::block::BlockTools {
                allow_screen_content_tools: context.allow_screen_content_tools,
                enable_filter_intra: context.enable_filter_intra,
                enable_intra_edge_filter: context.enable_intra_edge_filter,
                transform_mode: context.frame_tools.transform_mode,
                transform_context: 0,
                palette_context: Default::default(),
            };
            let origin_x = node
                .x
                .checked_mul(4)
                .ok_or_else(|| malformed("monochrome leaf x coordinate overflows"))?;
            let origin_y = node
                .y
                .checked_mul(4)
                .ok_or_else(|| malformed("monochrome leaf y coordinate overflows"))?;
            let geometry = super::block::MonochromeBlockGeometry {
                origin_x,
                origin_y,
                width,
                height,
                transform_grid,
            };
            let decoded = if visited == 0 {
                block_decoder.decode_origin(decoder, geometry, tools)
            } else {
                let Some(first) = first_monochrome.as_ref() else {
                    return Ok(PartitionVisitControl::Stop);
                };
                block_decoder.decode_following(
                    decoder,
                    geometry,
                    super::block::MonochromeNeighbors {
                        above_left: None,
                        above: None,
                        above_right: [None; 8],
                        left: Some(first),
                        left_below: None,
                    },
                    tools,
                )
            };
            let Ok(decoded) = decoded else {
                return Ok(PartitionVisitControl::Stop);
            };
            if visited == 0 {
                first_leaf = Some(decoded.clone().into_first_leaf());
                first_monochrome = Some(decoded);
            }
            visited = visited.saturating_add(1);
            Ok(if visited < 2 {
                PartitionVisitControl::Continue
            } else {
                PartitionVisitControl::Stop
            })
        })?;
        return Ok(first_leaf);
    }

    let closed_class = closed_444_reconstruction_context(context)
        || closed_420_reconstruction_context(context)
        || closed_lossy_420_reconstruction_context(context)
        || closed_lossy_420_square64_split_context(context)
        || closed_lossy_420_16x16_vertical_pair_context(context)
        || closed_lossy_420_horizontal_four_split_context(context)
        || closed_lossy_444_16x16_reconstruction_context(context);
    if !closed_class {
        // The old narrow decoder is deliberately not allowed to consume a
        // random prefix and then call a valid larger AV1 frame unsupported.
        // Partition and block syntax are interleaved in AV1, so the safe
        // walker must stop at the first terminal footprint until a matching
        // block parser is available.  Continuing would read block bytes as a
        // sibling partition symbol and would create false validation.
        let mut walker_decoder =
            RangeDecoder::new(data, range.start, range.end, context.disable_cdf_update)?;
        if !decode_restoration_prefix(&mut walker_decoder, context) {
            return Ok(None);
        }
        let mut saw_terminal = false;
        let control =
            walk_partition_until_stop(&mut walker_decoder, context, |_decoder, _node| {
                saw_terminal = true;
                Ok(PartitionVisitControl::Stop)
            })?;
        if !saw_terminal || control != PartitionVisitControl::Stop {
            return Err(malformed("partition walker found no terminal block"));
        }
        return Ok(None);
    }
    let mut decoder = RangeDecoder::new(data, range.start, range.end, context.disable_cdf_update)?;
    #[cfg(coverage)]
    {
        let trace_closed_context = (closed_444_reconstruction_context(context)
            & (closed_leaf_dimensions(context) | rectangular_leaf_dimensions(context)))
            | closed_420_reconstruction_context(context)
            | closed_lossy_420_reconstruction_context(context)
            | closed_lossy_420_square64_split_context(context)
            | closed_lossy_420_16x16_vertical_pair_context(context)
            | closed_lossy_420_horizontal_four_split_context(context)
            | closed_lossy_444_16x16_reconstruction_context(context);
        if trace_closed_context {
            decoder.enable_operation_trace();
        }
    }
    if !decode_restoration_prefix(&mut decoder, context) {
        return Ok(None);
    }
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
                    return Err(malformed(
                        "partition syntax is invalid for vertically unsampled chroma",
                    ));
                }
                let reconstruct_closed_leaf = (partition == 0)
                    & closed_444_reconstruction_context(context)
                    & closed_leaf_dimensions(context)
                    & closed_leaf_level_dimensions(context, level);
                if reconstruct_closed_leaf {
                    // Unsupported syntax ends this deliberately narrow
                    // pure-Rust attempt without pretending partial output is
                    // a complete decode.
                    // The accepted level/dimension pairs above prove a 2x2
                    // transform grid at level 4 and a 4x4 grid at level 3.
                    let transform_grid = if level == 4 {
                        super::block::TransformGrid::Square8
                    } else {
                        super::block::TransformGrid::Square16
                    };
                    return decode_closed_leaf(&mut decoder, context, transform_grid);
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
                    return decode_closed_420_leaf(&mut decoder, context, transform_grid);
                }
                let reconstruct_closed_lossy_420_leaf = (partition == 0)
                    & (level == 4)
                    & closed_lossy_420_reconstruction_context(context);
                if reconstruct_closed_lossy_420_leaf {
                    return decode_closed_lossy_420_leaf(&mut decoder, context);
                }
                let reconstruct_closed_lossy_420_square64_split =
                    (partition == 0) & closed_lossy_420_square64_split_context(context);
                if reconstruct_closed_lossy_420_square64_split {
                    return decode_closed_lossy_420_leaf(&mut decoder, context);
                }
                let reconstruct_closed_lossy_444_16x16_leaf = (partition == 0)
                    & (level == 3)
                    & closed_lossy_444_16x16_reconstruction_context(context);
                if reconstruct_closed_lossy_444_16x16_leaf {
                    return decode_closed_lossy_444_16x16_leaf(&mut decoder, context);
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
                    let first_child_partition =
                        decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one);
                    if first_child_partition != 0 {
                        return Ok(None);
                    }
                    let reconstructed = super::block::decode_four_lossless_444_leaves(
                        &mut decoder,
                        context.frame_width,
                        context.frame_height,
                        super::block::BlockTools {
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            palette_context: Default::default(),
                        },
                        |decoder| {
                            {
                                let partition = decoder
                                    .adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one);
                                partition == 0
                            }
                            .then_some(())
                            .ok_or(super::block::PortableUnavailable)
                        },
                    );
                    return finish_closed_leaf(&decoder, reconstructed);
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
                        return Ok(None);
                    }
                    let reconstructed = super::block::decode_four_lossless_420_leaves(
                        &mut decoder,
                        context.frame_width,
                        context.frame_height,
                        super::block::BlockTools {
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            palette_context: Default::default(),
                        },
                        |decoder| {
                            (decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one)
                                == 0)
                                .then_some(())
                                .ok_or(super::block::PortableUnavailable)
                        },
                    );
                    return finish_closed_leaf(&decoder, reconstructed);
                }
                let reconstruct_lossy_420_square_split = (partition == 3)
                    & closed_lossy_420_square_split_context(context)
                    & (level == 3);
                if reconstruct_lossy_420_square_split {
                    // The safe lossy square path consumes all four terminal
                    // payloads with one adaptive state. It is deliberately
                    // gated to the checked 16x16 frame context until a wider
                    // frame-canvas proof supplies all edge and filter state.
                    let (mut child_cdf, child_symbol_count_minus_one) = square8_partition_cdf();
                    if decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one) != 0 {
                        return Ok(None);
                    }
                    let quantization = lossy_quantization_for_context(context);
                    let reconstructed = super::block::decode_four_lossy_420_leaves(
                        &mut decoder,
                        context.frame_width,
                        context.frame_height,
                        quantization,
                        super::block::BlockTools {
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            palette_context: Default::default(),
                        },
                        |decoder| {
                            (decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one)
                                == 0)
                                .then_some(())
                                .ok_or(super::block::PortableUnavailable)
                        },
                    );
                    return finish_closed_leaf(&decoder, reconstructed);
                }
                let reconstruct_lossy_420_16x16_vertical_pair =
                    (partition == 2) & closed_lossy_420_16x16_vertical_pair_context(context);
                if reconstruct_lossy_420_16x16_vertical_pair {
                    // PARTITION_VERT places two 8x16 leaves side by side.
                    // Unlike PARTITION_SPLIT, the direct two-axis form has no
                    // child partition CDF sentence between the leaf payloads.
                    let quantization = lossy_quantization_for_context(context);
                    let reconstructed = super::block::decode_two_lossy_420_leaves(
                        &mut decoder,
                        context.frame_width,
                        context.frame_height,
                        quantization,
                        super::block::BlockTools {
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            palette_context: Default::default(),
                        },
                        super::block::SplitOrientation::Horizontal,
                        |_| Ok(()),
                    );
                    return finish_closed_leaf(&decoder, reconstructed);
                }
                let reconstruct_lossy_420_horizontal_four_split = (partition == 8)
                    & closed_lossy_420_horizontal_four_split_context(context)
                    & (level == 3);
                if reconstruct_lossy_420_horizontal_four_split {
                    // PARTITION_H4 keeps one 16-pixel luma span and places
                    // four 16x4 leaves vertically. Each child carries the
                    // rectangular transform syntax directly; no child
                    // partition CDF symbol is present between the leaves.
                    let quantization = lossy_quantization_for_context(context);
                    let reconstructed = super::block::decode_four_lossy_420_horizontal_leaves(
                        &mut decoder,
                        context.frame_width,
                        context.frame_height,
                        quantization,
                        super::block::BlockTools {
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            palette_context: Default::default(),
                        },
                        |_| Ok(()),
                    );
                    return finish_closed_leaf(&decoder, reconstructed);
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
                    return Err(malformed(
                        "partition syntax is invalid for vertically unsampled chroma",
                    ));
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
                    return decode_closed_leaf(&mut decoder, context, transform_grid);
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
                    return decode_closed_420_leaf(&mut decoder, context, transform_grid);
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
                        return Ok(None);
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
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            palette_context: Default::default(),
                        },
                        |decoder| {
                            (decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one)
                                == 0)
                                .then_some(())
                                .ok_or(super::block::PortableUnavailable)
                        },
                    );
                    return finish_closed_leaf(&decoder, reconstructed);
                }
                let reconstruct_420_recursive_split = split
                    & closed_420_reconstruction_context(context)
                    & (level == 3)
                    & recursive_split_dimensions(context);
                if reconstruct_420_recursive_split {
                    let (mut child_cdf, child_symbol_count_minus_one) = square8_partition_cdf();
                    if decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one) != 0 {
                        return Ok(None);
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
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            palette_context: Default::default(),
                        },
                        |decoder| {
                            (decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one)
                                == 0)
                                .then_some(())
                                .ok_or(super::block::PortableUnavailable)
                        },
                    );
                    return finish_closed_leaf(&decoder, reconstructed);
                }
                let reconstruct_lossy_420_recursive_split = split
                    & closed_lossy_420_recursive_split_context(context)
                    & horizontal_split
                    & (level == 3);
                if reconstruct_lossy_420_recursive_split {
                    let (mut child_cdf, child_symbol_count_minus_one) = square8_partition_cdf();
                    if decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one) != 0 {
                        return Ok(None);
                    }
                    let quantization = lossy_quantization_for_context(context);
                    let reconstructed = super::block::decode_two_lossy_420_leaves(
                        &mut decoder,
                        context.frame_width,
                        context.frame_height,
                        quantization,
                        super::block::BlockTools {
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            palette_context: Default::default(),
                        },
                        super::block::SplitOrientation::Horizontal,
                        |decoder| {
                            (decoder.adaptive_symbol(&mut child_cdf, child_symbol_count_minus_one)
                                == 0)
                                .then_some(())
                                .ok_or(super::block::PortableUnavailable)
                        },
                    );
                    return finish_closed_leaf(&decoder, reconstructed);
                }
            }
            return Ok(None);
        }
        level = level.wrapping_add(1);
        if level > 4 {
            return Err(malformed("partition recursion exceeds level four"));
        }
    }
}

#[cfg(coverage)]
#[coverage(off)]
pub(super) fn reference_trace() -> CodecResult<Vec<crate::Av1EntropyTraceState>> {
    const INPUT: [u8; 32] = [
        0x00, 0xff, 0x81, 0x7e, 0x55, 0xaa, 0x13, 0xec, 0x42, 0xbd, 0x99, 0x66, 0x01, 0x80, 0xfe,
        0x24, 0xdb, 0x10, 0xef, 0x73, 0x8c, 0x31, 0xce, 0x5a, 0xa5, 0x0f, 0xf0, 0x69, 0x96, 0x3c,
        0xc3, 0x7f,
    ];
    let spans = [super::super::samples::ByteSpan {
        start: 0,
        end: INPUT.len(),
    }];
    let data = SegmentedData::new(&INPUT, &spans)?;
    let mut records = Vec::with_capacity(103);

    let mut decoder = RangeDecoder::new(&data, 0, INPUT.len(), true)?;
    records.push(decoder.trace_state("equal", 0, -1, &[]));
    for step in 1..=16 {
        let value = i32::from(decoder.equal());
        records.push(decoder.trace_state("equal", step, value, &[]));
    }

    let mut decoder = RangeDecoder::new(&data, 0, INPUT.len(), true)?;
    records.push(decoder.trace_state("fixed", 0, -1, &[]));
    for (index, probability) in [0, 1, 4096, 8192, 16_384, 24_576, 32_767]
        .into_iter()
        .enumerate()
    {
        let value = i32::from(decoder.fixed(probability));
        let step = u32::try_from(index)
            .map_err(|error| CodecError::Dimensions(format!("fixed trace step: {error}")))?
            .checked_add(1)
            .ok_or_else(|| malformed("fixed trace step overflows"))?;
        records.push(decoder.trace_state("fixed", step, value, &[]));
    }

    let mut cdf = [16_384, 0];
    let mut decoder = RangeDecoder::new(&data, 0, INPUT.len(), false)?;
    records.push(decoder.trace_state("adaptive_bool", 0, -1, &cdf));
    for step in 1..=16 {
        let value = i32::from(decoder.adaptive_bool(&mut cdf));
        records.push(decoder.trace_state("adaptive_bool", step, value, &cdf));
    }

    let mut cdf = [24_576, 16_384, 8192, 0];
    let mut decoder = RangeDecoder::new(&data, 0, INPUT.len(), false)?;
    records.push(decoder.trace_state("adaptive_symbol", 0, -1, &cdf));
    for step in 1..=16 {
        let value = i32::try_from(decoder.adaptive_symbol(&mut cdf, 3))
            .map_err(|error| CodecError::Dimensions(format!("adaptive symbol value: {error}")))?;
        records.push(decoder.trace_state("adaptive_symbol", step, value, &cdf));
    }

    let mut cdf = [24_576, 16_384, 8192, 0];
    let mut decoder = RangeDecoder::new(&data, 0, INPUT.len(), true)?;
    records.push(decoder.trace_state("frozen_symbol", 0, -1, &cdf));
    for step in 1..=8 {
        let value = i32::try_from(decoder.adaptive_symbol(&mut cdf, 3))
            .map_err(|error| CodecError::Dimensions(format!("frozen symbol value: {error}")))?;
        records.push(decoder.trace_state("frozen_symbol", step, value, &cdf));
    }

    let mut cdf = [24_576, 16_384, 8192, 0];
    let mut decoder = RangeDecoder::new(&data, 0, INPUT.len(), false)?;
    records.push(decoder.trace_state("high_token", 0, -1, &cdf));
    for step in 1..=8 {
        let value = i32::try_from(decoder.high_token(&mut cdf))
            .map_err(|error| CodecError::Dimensions(format!("high token value: {error}")))?;
        records.push(decoder.trace_state("high_token", step, value, &cdf));
    }

    let mut decoder = RangeDecoder::new(&data, 0, INPUT.len(), true)?;
    records.push(decoder.trace_state("uniform", 0, -1, &[]));
    for (index, count) in [2, 3, 5, 17, 255].into_iter().enumerate() {
        let value = i32::try_from(decoder.uniform(count))
            .map_err(|error| CodecError::Dimensions(format!("uniform value: {error}")))?;
        let step = u32::try_from(index)
            .map_err(|error| CodecError::Dimensions(format!("uniform trace step: {error}")))?
            .checked_add(1)
            .ok_or_else(|| malformed("uniform trace step overflows"))?;
        records.push(decoder.trace_state("uniform", step, value, &[]));
    }

    let mut decoder = RangeDecoder::new(&data, 0, INPUT.len(), true)?;
    records.push(decoder.trace_state("subexponential", 0, -1, &[]));
    for (index, reference) in [0, 63, 127, 200].into_iter().enumerate() {
        let value = decoder.subexponential(reference, 256, 5);
        let step = u32::try_from(index)
            .map_err(|error| CodecError::Dimensions(format!("subexponential trace step: {error}")))?
            .checked_add(1)
            .ok_or_else(|| malformed("subexponential trace step overflows"))?;
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
        let data = SegmentedData::new(input, &spans)?;
        let mut decoder = RangeDecoder::new(&data, 0, input.len(), false)?;
        let (mut cdf, symbol_count_minus_one) = default_partition_cdf(1)?;
        records.push(decoder.trace_state(case, 0, -1, &cdf));
        let value = i32::try_from(decoder.adaptive_symbol(&mut cdf, symbol_count_minus_one))
            .map_err(|error| CodecError::Dimensions(format!("partition value: {error}")))?;
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
    let data = SegmentedData::new(&RESTORATION_FRAME_3, &spans)?;
    let mut decoder = RangeDecoder::new(&data, 0, RESTORATION_FRAME_3.len(), false)?;
    let mut sgr_cdf = [15_913, 0];
    let case = "restoration_422_frame_3";
    let mut step = 0_u32;
    records.push(decoder.trace_state(case, step, -1, &sgr_cdf));
    step = step
        .checked_add(1)
        .ok_or_else(|| malformed("restoration trace step overflows"))?;
    for _plane in 0..3 {
        let enabled = decoder.adaptive_bool(&mut sgr_cdf);
        records.push(decoder.trace_state(case, step, i32::from(enabled), &sgr_cdf));
        step = step
            .checked_add(1)
            .ok_or_else(|| malformed("restoration trace step overflows"))?;
        if !enabled {
            continue;
        }
        let parameter_index = decoder.bits(4);
        let parameter_index_value = i32::try_from(parameter_index).map_err(|error| {
            CodecError::Dimensions(format!("restoration parameter value: {error}"))
        })?;
        records.push(decoder.trace_state(case, step, parameter_index_value, &[]));
        step = step
            .checked_add(1)
            .ok_or_else(|| malformed("restoration trace step overflows"))?;
        let activity = *SGR_PARAMETER_ACTIVITY
            .get(usize::try_from(parameter_index).map_err(|error| {
                CodecError::Dimensions(format!("restoration parameter index: {error}"))
            })?)
            .ok_or_else(|| malformed("restoration activity is unavailable"))?;
        if activity[0] {
            let weight = decoder
                .subexponential(64, 128, 4)
                .checked_sub(96)
                .ok_or_else(|| malformed("restoration first weight underflows"))?;
            records.push(decoder.trace_state(case, step, weight, &[]));
            step = step
                .checked_add(1)
                .ok_or_else(|| malformed("restoration trace step overflows"))?;
        }
        if activity[1] {
            let weight = decoder
                .subexponential(63, 128, 4)
                .checked_sub(32)
                .ok_or_else(|| malformed("restoration second weight underflows"))?;
            records.push(decoder.trace_state(case, step, weight, &[]));
            step = step
                .checked_add(1)
                .ok_or_else(|| malformed("restoration trace step overflows"))?;
        }
    }
    let (mut partition_cdf, symbol_count_minus_one) = default_partition_cdf(1)?;
    let partition =
        i32::try_from(decoder.adaptive_symbol(&mut partition_cdf, symbol_count_minus_one))
            .map_err(|error| {
                CodecError::Dimensions(format!("restoration partition value: {error}"))
            })?;
    records.push(decoder.trace_state(case, step, partition, &partition_cdf));

    Ok(records)
}

#[cfg(any(test, coverage))]
#[cfg_attr(coverage, coverage(off))]
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
        enable_intra_edge_filter: true,
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
        let spans = [crate::codecs::avif::samples::ByteSpan {
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
        true
    );
    active_prefix.block_y = 1;
    let mut decoder = RangeDecoder::new(&data, 0, input.len(), false).unwrap();
    assert_eq!(
        decode_restoration_prefix(&mut decoder, &active_prefix),
        true
    );
    active_prefix.block_y = 0;
    active_prefix.upscaled_width = 65;
    let mut decoder = RangeDecoder::new(&data, 0, input.len(), false).unwrap();
    assert_eq!(
        decode_restoration_prefix(&mut decoder, &active_prefix),
        false
    );
    assert_eq!(
        validate_first_partition(&data, 0..input.len(), &active_prefix),
        Ok(None)
    );

    assert!(default_partition_cdf(5).is_err());
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
    assert!(
        validate_first_partition(&forbidden_data, 0..FORBIDDEN_422.len(), &coverage_context(),)
            .is_err()
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
        if validate_first_partition(&data, 0..input.len(), &vertical_only).is_ok() {
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
    assert!(validate_first_partition(&data, 0..input.len(), &no_partition).is_err());
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
    coverage_partition_walker_paths();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_shapes_expand_in_payload_order() {
        let expected_counts = [1, 2, 2, 4, 3, 3, 3, 3, 4, 4];
        for (symbol, expected_count) in expected_counts.into_iter().enumerate() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the test array contains only the ten AV1 partition symbols"
            )]
            let kind = PartitionKind::from_symbol(symbol as u32).unwrap_or(PartitionKind::None);
            let (children, count) = partition_child_geometries(kind, 8, 12, 4).unwrap_or((
                [PartitionGeometry {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                }; 4],
                0,
            ));
            assert_eq!(count, expected_count, "symbol {symbol}");
            assert!(
                children[..count]
                    .iter()
                    .all(|child| child.width != 0 && child.height != 0)
            );

            let area = children[..count]
                .iter()
                .map(|child| child.width.saturating_mul(child.height))
                .sum::<u32>();
            assert_eq!(area, 64, "symbol {symbol} must cover the parent once");
        }
    }

    #[test]
    fn partition_shapes_reject_impossible_four_way_geometry() {
        assert!(partition_child_geometries(PartitionKind::HorizontalFour, 0, 0, 1).is_err());
        assert!(partition_child_geometries(PartitionKind::VerticalFour, 0, 0, 1).is_err());
        assert!(partition_child_geometries(PartitionKind::Split, 0, 0, 0).is_err());
    }

    #[test]
    fn partition_geometry_clips_only_the_visible_edge() -> Av1Result<()> {
        let geometry = clip_partition_geometry(
            PartitionGeometry {
                x: 8,
                y: 4,
                width: 8,
                height: 8,
            },
            12,
            10,
        )?
        .ok_or_else(|| malformed("geometry is outside the frame"))?;
        assert_eq!(
            geometry,
            PartitionGeometry {
                x: 8,
                y: 4,
                width: 4,
                height: 6,
            }
        );
        let clipped = clip_partition_geometry(
            PartitionGeometry {
                x: 12,
                y: 0,
                width: 4,
                height: 4,
            },
            12,
            4,
        )?;
        assert!(clipped.is_none());
        Ok(())
    }

    #[test]
    fn partition_walker_stops_before_a_sibling_payload() -> Av1Result<()> {
        let context = coverage_context();
        let input = [0_u8; 64];
        let spans = [crate::codecs::avif::samples::ByteSpan {
            start: 0,
            end: input.len(),
        }];
        let data = SegmentedData::new(&input, &spans)?;
        let mut decoder = RangeDecoder::new(&data, 0, input.len(), false)?;
        let mut visited = Vec::new();
        let control = walk_partition_until_stop(&mut decoder, &context, |_decoder, node| {
            visited.push(node);
            Ok(PartitionVisitControl::Stop)
        })?;

        assert_eq!(control, PartitionVisitControl::Stop);
        assert_eq!(visited.len(), 1);
        assert_eq!(visited[0].kind, PartitionKind::None);
        Ok(())
    }

    #[test]
    fn alpha_auxiliary_monochrome_first_leaf_is_bounded() -> Av1Result<()> {
        let bytes = include_bytes!("../../../../tests/fixtures/input/images/avif/alpha.avif");
        let extracted = crate::codecs::avif::samples::validated(bytes)?;
        let sample = &extracted
            .still
            .as_ref()
            .ok_or_else(|| malformed("alpha fixture has no still payload"))?
            .alpha
            .as_ref()
            .ok_or_else(|| malformed("alpha fixture has no auxiliary sample"))?
            .samples[0];
        let data = SegmentedData::new(bytes, &sample.spans)?;
        assert_eq!(data.len(), 270);
        let mut context = coverage_context();
        context.frame_width = 64;
        context.frame_height = 64;
        context.upscaled_width = 64;
        context.monochrome = true;
        context.subsampling_x = true;
        context.subsampling_y = true;
        context.all_lossless = true;
        let leaf = validate_first_partition(&data, 15..data.len(), &context)?
            .ok_or_else(|| malformed("alpha monochrome first leaf was not reconstructed"))?;
        assert_eq!((leaf.width, leaf.height), (16, 16));
        assert_eq!(leaf.planes[0].samples.len(), 256);
        assert_eq!(leaf.planes[1].samples.len(), 256);
        assert_eq!(leaf.planes[2].samples.len(), 256);
        assert!(leaf.planes[0].samples.iter().any(|&sample| sample != 0));
        Ok(())
    }

    #[test]
    fn alpha_auxiliary_monochrome_partition_reconstructs_canvas() -> Av1Result<()> {
        let bytes = include_bytes!("../../../../tests/fixtures/input/images/avif/alpha.avif");
        let extracted = crate::codecs::avif::samples::validated(bytes)?;
        let sample = &extracted
            .still
            .as_ref()
            .ok_or_else(|| malformed("alpha fixture has no still payload"))?
            .alpha
            .as_ref()
            .ok_or_else(|| malformed("alpha fixture has no auxiliary sample"))?
            .samples[0];
        let data = SegmentedData::new(bytes, &sample.spans)?;
        let mut context = coverage_context();
        context.frame_width = 64;
        context.frame_height = 64;
        context.upscaled_width = 64;
        context.monochrome = true;
        context.subsampling_x = true;
        context.subsampling_y = true;
        context.all_lossless = true;
        let plane = validate_complete_monochrome_partition(&data, 15..data.len(), &context)?
            .ok_or_else(|| malformed("alpha monochrome canvas was not reconstructed"))?;
        assert_eq!(plane.samples.len(), 64 * 64);
        assert!(plane.samples.iter().any(|&sample| sample != 0));
        Ok(())
    }

    #[test]
    fn alpha_auxiliary_monochrome_partition_reconstructs_canvas_with_explicit_neighbors()
    -> Av1Result<()> {
        let bytes = include_bytes!("../../../../tests/fixtures/input/images/avif/alpha.avif");
        let extracted = crate::codecs::avif::samples::validated(bytes)?;
        let sample = &extracted
            .still
            .as_ref()
            .ok_or_else(|| malformed("alpha fixture has no still payload"))?
            .alpha
            .as_ref()
            .ok_or_else(|| malformed("alpha fixture has no auxiliary sample"))?
            .samples[0];
        let data = SegmentedData::new(bytes, &sample.spans)?;
        let mut context = coverage_context();
        context.frame_width = 64;
        context.frame_height = 64;
        context.upscaled_width = 64;
        context.monochrome = true;
        context.subsampling_x = true;
        context.subsampling_y = true;
        context.all_lossless = true;
        let mut decoder = RangeDecoder::new(&data, 15, data.len(), false)?;
        let mut walker = PartitionWalker::new(&mut decoder, &context);
        let mut block_decoder = super::super::block::MonochromeLosslessDecoder::new();
        let mut canvas = super::super::raster::MonochromeFrameCanvas::new(
            context.frame_width,
            context.frame_height,
        )?;
        let mut leaves = Vec::new();
        let control = walker.walk(1, 0, 0, &mut |decoder, node| {
            let Some((transform_grid, width, height)) = monochrome_transform_geometry(node) else {
                return Err(malformed("alpha auxiliary terminal geometry"));
            };
            let origin_x = node
                .x
                .checked_mul(4)
                .ok_or_else(|| malformed("alpha auxiliary x origin overflow"))?;
            let origin_y = node
                .y
                .checked_mul(4)
                .ok_or_else(|| malformed("alpha auxiliary y origin overflow"))?;
            let geometry = super::super::block::MonochromeBlockGeometry {
                origin_x,
                origin_y,
                width,
                height,
                transform_grid,
            };
            let tools = super::super::block::BlockTools {
                allow_screen_content_tools: false,
                enable_filter_intra: true,
                enable_intra_edge_filter: true,
                transform_mode: 0,
                transform_context: 0,
                palette_context: Default::default(),
            };
            let decoded = if leaves.is_empty() {
                block_decoder
                    .decode_origin(decoder, geometry, tools)
                    .map_err(|_| malformed("alpha monochrome origin syntax rejected"))?
            } else {
                let neighbors = monochrome_neighbors(&leaves, geometry)?;
                block_decoder
                    .decode_following(decoder, geometry, neighbors, tools)
                    .map_err(|_| malformed("alpha monochrome following syntax rejected"))?
            };
            canvas.place_partition_leaf(
                node.x,
                node.y,
                node.width,
                node.height,
                decoded.plane(),
            )?;
            leaves.push(decoded);
            Ok(PartitionVisitControl::Continue)
        })?;
        assert_eq!(control, PartitionVisitControl::Continue);
        assert!(!leaves.is_empty());
        let plane = canvas.finish()?;
        assert_eq!(plane.samples.len(), 64 * 64);
        assert!(plane.samples.iter().any(|&sample| sample != 0));
        Ok(())
    }

    #[test]
    fn baseline_first_terminal_lossy_syntax_is_consumed() -> Av1Result<()> {
        let bytes = include_bytes!("../../../../tests/fixtures/input/images/avif/baseline.avif");
        let tile = &bytes[307..307 + 2770];
        let spans = [crate::codecs::avif::samples::ByteSpan {
            start: 0,
            end: tile.len(),
        }];
        let data = SegmentedData::new(tile, &spans)?;
        let mut decoder = RangeDecoder::new(&data, 0, tile.len(), false)?;
        let mut context = coverage_context();
        context.level = 1;
        context.block_width = 64;
        context.block_height = 64;
        context.frame_width = 128;
        context.frame_height = 128;
        context.upscaled_width = 128;
        context.subsampling_x = true;
        context.subsampling_y = true;
        let mut visited = Vec::new();
        let control = walk_partition_until_stop(&mut decoder, &context, |decoder, node| {
            visited.push(node);
            let reconstructed = crate::codecs::avif::av1::block::decode_first_lossy_420_leaf(
                decoder,
                8,
                8,
                crate::codecs::avif::av1::block::LossyQuantization {
                    qindex: 120,
                    delta_q_present: false,
                    resolution_log2: 0,
                    y_dc_delta: 0,
                    y_ac_delta: 0,
                    u_dc_delta: 0,
                    u_ac_delta: 0,
                    v_dc_delta: 0,
                    v_ac_delta: 0,
                    using_matrix: false,
                    matrix_y: 0,
                    matrix_u: 0,
                    matrix_v: 0,
                },
                crate::codecs::avif::av1::block::BlockTools {
                    allow_screen_content_tools: false,
                    enable_filter_intra: true,
                    enable_intra_edge_filter: true,
                    transform_mode: 1,
                    transform_context: 0,
                    palette_context: Default::default(),
                },
            );
            let reconstructed =
                reconstructed.map_err(|_| malformed("baseline first terminal syntax rejected"))?;
            assert_eq!(reconstructed.width, 8);
            assert_eq!(reconstructed.height, 8);
            assert_eq!(reconstructed.planes[0].samples.len(), 64);
            assert_eq!(reconstructed.planes[1].samples.len(), 16);
            assert_eq!(reconstructed.planes[2].samples.len(), 16);
            Ok(PartitionVisitControl::Stop)
        })?;
        assert_eq!(control, PartitionVisitControl::Stop);
        assert_eq!(visited.len(), 1);
        Ok(())
    }

    #[test]
    fn baseline_frame_first_leaf_remains_an_explicit_gap() -> Av1Result<()> {
        let bytes = include_bytes!("../../../../tests/fixtures/input/images/avif/baseline.avif");
        let tile = &bytes[307..307 + 2770];
        let spans = [crate::codecs::avif::samples::ByteSpan {
            start: 0,
            end: tile.len(),
        }];
        let data = SegmentedData::new(tile, &spans)?;
        let mut context = coverage_context();
        context.level = 1;
        context.block_width = 32;
        context.block_height = 32;
        context.frame_width = 128;
        context.frame_height = 128;
        context.upscaled_width = 128;
        context.subsampling_x = true;
        context.subsampling_y = true;

        assert!(validate_first_partition(&data, 0..tile.len(), &context)?.is_none());
        Ok(())
    }

    #[test]
    fn baseline_partition_prefix_stops_before_unsupported_block() -> Av1Result<()> {
        let bytes = include_bytes!("../../../../tests/fixtures/input/images/avif/baseline.avif");
        let tile = &bytes[307..307 + 2770];
        let spans = [crate::codecs::avif::samples::ByteSpan {
            start: 0,
            end: tile.len(),
        }];
        let data = SegmentedData::new(tile, &spans)?;
        let mut decoder = RangeDecoder::new(&data, 0, tile.len(), false)?;
        let mut context = coverage_context();
        context.level = 1;
        context.block_width = 32;
        context.block_height = 32;
        context.frame_width = 128;
        context.frame_height = 128;
        context.upscaled_width = 128;
        context.subsampling_x = true;
        context.subsampling_y = true;
        let mut visited = 0_u32;
        let control = walk_partition_until_stop(&mut decoder, &context, |_decoder, _node| {
            visited = visited.saturating_add(1);
            Ok(PartitionVisitControl::Stop)
        })?;

        assert_eq!(control, PartitionVisitControl::Stop);
        assert_eq!(visited, 1);
        Ok(())
    }

    #[test]
    fn baseline_full_frame_does_not_publish_a_partial_canvas() -> Av1Result<()> {
        let bytes = include_bytes!("../../../../tests/fixtures/input/images/avif/baseline.avif");
        let tile = &bytes[307..307 + 2770];
        let spans = [crate::codecs::avif::samples::ByteSpan {
            start: 0,
            end: tile.len(),
        }];
        let data = SegmentedData::new(tile, &spans)?;
        let mut context = coverage_context();
        context.block_width = 32;
        context.block_height = 32;
        context.frame_width = 128;
        context.frame_height = 128;
        context.upscaled_width = 128;
        context.subsampling_x = true;
        context.subsampling_y = true;

        assert!(validate_first_partition(&data, 0..tile.len(), &context)?.is_none());
        Ok(())
    }
}
