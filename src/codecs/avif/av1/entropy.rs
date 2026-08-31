//! Scalar AV1 multi-symbol arithmetic decoding over segmented tile bytes.

use std::ops::Range;

use crate::codecs::CodecError;
#[cfg(coverage)]
use crate::codecs::CodecResult;

use super::bit_reader::SegmentedData;
use super::geometry::{BlockSize, IntraEdgeFlags, PixelLayout, TxSize};
use super::motion::{
    CompoundType, GlobalMotion, GlobalMotionType, InterMode, InterpolationFilter, MotionMode,
    MotionVector, ProjectedTemporalField, ReferenceFrame, ReferenceMvRequest, ReferenceMvTarget,
    ReferencePair, ScaleFactors, SpatialMotionSource, SpatialRefBlock, TemporalMotionField,
    find_reference_mvs, global_motion_vector,
};
use super::restoration::{Plan as RestorationPlan, Unit as RestorationUnit};
use super::surface::FrameSurface;
use super::tile_state::{BlockCoding, BlockCommitMetadata, TileState};
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
    pub(super) segmentation: SegmentationContext,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct SegmentContext {
    pub(super) delta_q: i32,
    pub(super) delta_lf: [i32; 4],
    pub(super) reference: i32,
    pub(super) skip: bool,
    pub(super) global_motion: bool,
    pub(super) qindex: u32,
    pub(super) lossless: bool,
}

impl SegmentContext {
    pub(super) const EMPTY: Self = Self {
        delta_q: 0,
        delta_lf: [0; 4],
        reference: -1,
        skip: false,
        global_motion: false,
        qindex: 0,
        lossless: false,
    };
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct SegmentationContext {
    pub(super) enabled: bool,
    pub(super) update_map: bool,
    pub(super) temporal: bool,
    pub(super) preskip: bool,
    pub(super) last_active_id: i32,
    pub(super) segments: [SegmentContext; 8],
}

impl SegmentationContext {
    pub(super) const DISABLED: Self = Self {
        enabled: false,
        update_map: false,
        temporal: false,
        preskip: false,
        last_active_id: 0,
        segments: [SegmentContext::EMPTY; 8],
    };
}

/// Immutable reference state used by one inter-frame tile decode.  The
/// decoder keeps the owning `Arc<FrameSurface>` in frame state and borrows it
/// only for the duration of entropy/prediction.  This prevents a plane view
/// from outliving the retained slot while still making the hot prediction
/// path allocation-free.
#[derive(Clone, Copy)]
pub(super) struct InterReference<'a> {
    #[allow(
        dead_code,
        reason = "retained for reference-order and temporal dispatch"
    )]
    pub(super) logical: ReferenceFrame,
    pub(super) surface: &'a FrameSurface,
    pub(super) scale: ScaleFactors,
    #[allow(dead_code, reason = "retained for temporal projection dispatch")]
    pub(super) order_hint: u32,
    pub(super) global_motion: GlobalMotion,
    #[allow(dead_code, reason = "retained for sign-biased reference-MV dispatch")]
    pub(super) sign_bias: bool,
    #[allow(dead_code, reason = "retained for temporal reference-MV dispatch")]
    pub(super) temporal: Option<&'a TemporalMotionField>,
}

/// Seven logical references and the frame-level controls shared by every
/// inter block in a tile.  The constructor validates the full mapping before
/// any inter entropy is consumed; an individual block therefore cannot turn
/// a missing slot into a default/DC predictor.
#[derive(Clone, Copy)]
pub(super) struct InterFrameContext<'a> {
    pub(super) references: [InterReference<'a>; 7],
    pub(super) current_order_hint: u32,
    pub(super) order_hint_bits: u32,
    pub(super) reference_order_hints: [u32; 7],
    pub(super) global_motion: [GlobalMotion; 7],
    pub(super) sign_bias: [bool; 7],
    pub(super) force_integer_mv: bool,
    pub(super) high_precision_mv: bool,
    pub(super) use_ref_frame_mvs: bool,
    pub(super) interpolation_filter: u32,
    pub(super) dual_filter: bool,
    pub(super) reference_mode_select: bool,
    pub(super) motion_mode_switchable: bool,
    pub(super) allow_warped_motion: bool,
    pub(super) enable_interintra_compound: bool,
    pub(super) enable_masked_compound: bool,
    pub(super) enable_jnt_comp: bool,
}

impl<'a> InterFrameContext<'a> {
    pub(super) fn reference(self, reference: ReferenceFrame) -> InterReference<'a> {
        self.references[reference.index()]
    }

    pub(super) fn mv_request(
        self,
        target: ReferenceMvTarget,
        block_size: BlockSize,
        local_x_b4: u32,
        local_y_b4: u32,
        absolute_x_b4: u32,
        absolute_y_b4: u32,
        tile_left_b4: u32,
        tile_top_b4: u32,
        tile_right_b4: u32,
        tile_bottom_b4: u32,
        frame_width_b4: u32,
        frame_height_b4: u32,
        top_has_right: bool,
        temporal: Option<&ProjectedTemporalField>,
    ) -> ReferenceMvRequest<'_> {
        ReferenceMvRequest {
            target,
            block_size,
            local_x_b4,
            local_y_b4,
            absolute_x_b4,
            absolute_y_b4,
            tile_left_b4,
            tile_top_b4,
            tile_right_b4,
            tile_bottom_b4,
            frame_width_b4,
            frame_height_b4,
            top_has_right,
            global_motion: self.global_motion,
            force_integer_mv: self.force_integer_mv,
            high_precision_mv: self.high_precision_mv,
            sign_bias: self.sign_bias,
            current_order_hint: self.current_order_hint,
            order_hint_bits: self.order_hint_bits,
            reference_order_hints: self.reference_order_hints,
            use_ref_frame_mvs: self.use_ref_frame_mvs,
            temporal,
        }
    }
}

/// Codec state needed before the first block in one tile.
#[derive(Clone, Copy)]
pub(super) struct FirstBlockContext {
    pub(super) disable_cdf_update: bool,
    /// True only for key and intra-only frames.
    ///
    /// Keeping this admission fact beside the tile entropy state prevents an
    /// error-resilient inter/switch frame with no primary reference from
    /// entering the intra-only block decoder with freshly initialized CDFs.
    pub(super) intra_frame: bool,
    pub(super) level: u32,
    pub(super) block_width: u32,
    pub(super) block_height: u32,
    pub(super) block_x: u32,
    pub(super) block_y: u32,
    /// Absolute tile origin in the frame's padded four-pixel grid.
    pub(super) tile_origin_b4_x: u32,
    pub(super) tile_origin_b4_y: u32,
    /// True only when the frame walker proved that this is the sole tile.
    /// Tile-local `(0, 0)` coordinates alone do not establish that fact.
    pub(super) single_tile: bool,
    pub(super) frame_block_width: u32,
    pub(super) frame_block_height: u32,
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

#[derive(Clone)]
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

const DEFAULT_SEGMENT_ID_CDFS: [[u16; 8]; 3] = [
    [27_146, 24_875, 16_675, 14_535, 4_959, 4_395, 235, 0],
    [18_494, 14_538, 10_211, 7_833, 2_788, 1_917, 424, 0],
    [5_241, 4_281, 4_045, 3_878, 371, 121, 89, 0],
];

/// Complete canonical frame CDF state. The portable block engine consumes the
/// intra/common subset today; retaining exact inter and motion families makes
/// primary-reference publication complete before inter reconstruction lands.
#[derive(Clone)]
pub(super) struct FrameCdfs {
    partition: [[[u16; 10]; 4]; 5],
    restoration: RestorationCdfs,
    block: super::block::BlockCdfState,
    common: super::frame_cdfs::CommonCdfs,
    segment_id: [[u16; 8]; 3],
    #[allow(
        dead_code,
        reason = "intraBC syntax consumes this retained input family in the next block-engine slice"
    )]
    intrabc: super::frame_cdfs::Cdf<2>,
    inter: super::frame_cdfs::InterCdfs,
    motion_vectors: super::frame_cdfs::MvCdfs,
}

impl FrameCdfs {
    pub(super) fn defaults(qindex: u32) -> Av1Result<Self> {
        let block = super::block::BlockCdfState::defaults_for_qindex(qindex)
            .ok_or_else(|| malformed("AV1 qindex has no default CDF state"))?;
        Ok(Self {
            partition: PARTITION_CDFS,
            restoration: RestorationCdfs::defaults(),
            block,
            common: super::frame_cdfs::CommonCdfs::defaults(),
            segment_id: DEFAULT_SEGMENT_ID_CDFS,
            intrabc: super::frame_cdfs::DEFAULT_INTRABC,
            inter: super::frame_cdfs::InterCdfs::defaults(),
            motion_vectors: super::frame_cdfs::MvCdfs::defaults(),
        })
    }

    /// Construct the reference snapshot selected by an intra frame's
    /// context-update tile. Key-intra luma and inter-only state remain the
    /// frame input, while common/coefficient state is selectively published.
    pub(super) fn publish_intra(input: &Self, adapted: &Self) -> Self {
        let mut output = input.clone();
        output.partition = adapted.partition;
        output.restoration = adapted.restoration.clone();
        output.block.publish_intra_from(&adapted.block);
        output.common.publish_from(&adapted.common);
        output.segment_id = adapted.segment_id;
        output.reset_common_counts();
        output
    }

    /// Publish a context-update tile for an inter/switch frame. `intrabc` and
    /// key-frame luma-mode state remain from the frame input exactly as in
    /// scalar dav1d 1.5.3.
    #[allow(
        dead_code,
        reason = "called once the inter block engine can produce an adapted tile snapshot"
    )]
    pub(super) fn publish_inter(input: &Self, adapted: &Self) -> Self {
        let mut output = Self::publish_intra(input, adapted);
        output.inter.publish_from(&adapted.inter);
        output.motion_vectors.publish_from(&adapted.motion_vectors);
        output
    }

    fn reset_common_counts(&mut self) {
        const PARTITION_COUNTS: [usize; 5] = [7, 9, 9, 9, 3];
        for (level, contexts) in self.partition.iter_mut().enumerate() {
            for cdf in contexts {
                cdf[PARTITION_COUNTS[level]] = 0;
            }
        }
        self.restoration.switchable[2] = 0;
        self.restoration.wiener[1] = 0;
        self.restoration.sgr_projection[1] = 0;
        for cdf in &mut self.segment_id {
            cdf[7] = 0;
        }
    }
}

/// AV1's padded current/previous segmentation map, stored in four-pixel
/// units. Stride and height retain the codec's 32-cell padding while all
/// normative reads/writes are clipped to `width`/`height`.
#[derive(Clone)]
pub(super) struct SegmentMap {
    width: u32,
    height: u32,
    stride: usize,
    cells: Vec<u8>,
}

impl SegmentMap {
    pub(super) fn new(width: u32, height: u32) -> Av1Result<Self> {
        let stride_u32 = width
            .checked_add(31)
            .ok_or_else(|| malformed("segment-map stride overflows"))?
            & !31;
        let padded_height = height
            .checked_add(31)
            .ok_or_else(|| malformed("segment-map height overflows"))?
            & !31;
        let stride = usize::try_from(stride_u32)
            .map_err(|_| malformed("segment-map stride exceeds usize"))?;
        let padded_height = usize::try_from(padded_height)
            .map_err(|_| malformed("segment-map height exceeds usize"))?;
        let count = stride
            .checked_mul(padded_height)
            .ok_or_else(|| malformed("segment-map allocation overflows"))?;
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(count)
            .map_err(|_| CodecError::Dimensions("unable to allocate AV1 segment map".to_owned()))?;
        cells.resize(count, 0);
        Ok(Self {
            width,
            height,
            stride,
            cells,
        })
    }

    pub(super) fn compatible_with(&self, width: u32, height: u32) -> bool {
        self.width == width && self.height == height
    }

    fn get(&self, x: u32, y: u32) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let x = usize::try_from(x).ok()?;
        let y = usize::try_from(y).ok()?;
        self.cells
            .get(y.checked_mul(self.stride)?.checked_add(x)?)
            .copied()
    }

    fn fill(&mut self, x: u32, y: u32, width: u32, height: u32, id: u8) -> Av1Result<()> {
        if id > 7 {
            return Err(malformed("segment id exceeds seven"));
        }
        if width == 0 || height == 0 || x >= self.width || y >= self.height {
            return Err(malformed("segment-map write has invalid geometry"));
        }
        let end_x = x.saturating_add(width).min(self.width);
        let end_y = y.saturating_add(height).min(self.height);
        for row in y..end_y {
            let start = usize::try_from(row)
                .ok()
                .and_then(|row| row.checked_mul(self.stride))
                .and_then(|offset| offset.checked_add(usize::try_from(x).ok()?))
                .ok_or_else(|| malformed("segment-map row offset overflows"))?;
            let len = usize::try_from(end_x.saturating_sub(x))
                .map_err(|_| malformed("segment-map span exceeds usize"))?;
            let end = start
                .checked_add(len)
                .ok_or_else(|| malformed("segment-map span overflows"))?;
            self.cells
                .get_mut(start..end)
                .ok_or_else(|| malformed("segment-map span exceeds allocation"))?
                .fill(id);
        }
        Ok(())
    }

    fn minimum(&self, x: u32, y: u32, width: u32, height: u32) -> u8 {
        let end_x = x.saturating_add(width).min(self.width);
        let end_y = y.saturating_add(height).min(self.height);
        let mut minimum = 7_u8;
        for row in y..end_y {
            for column in x..end_x {
                minimum = minimum.min(self.get(column, row).unwrap_or(0));
                if minimum == 0 {
                    return 0;
                }
            }
        }
        minimum
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
) -> RestorationUnit {
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
        RestorationUnitType::None => RestorationUnit::None,
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
            RestorationUnit::Wiener {
                horizontal: [horizontal_zero, horizontal_one, horizontal_two],
                vertical: [vertical_zero, vertical_one, vertical_two],
            }
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
            RestorationUnit::SgrProjection {
                parameter_index: u8::try_from(parameter_index).unwrap_or_default(),
                weights: [first, second],
            }
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
    decode_restoration_prefix_with_cdfs(decoder, context, &mut cdfs)
}

fn decode_restoration_prefix_with_cdfs(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    context: &FirstBlockContext,
    cdfs: &mut RestorationCdfs,
) -> bool {
    let mut references = [RestorationReference::defaults(); 3];
    let plane_count = if context.monochrome { 1 } else { 3 };
    for (plane, reference) in references.iter_mut().enumerate().take(plane_count) {
        let Some(restoration_type) = context.restoration_types[plane] else {
            continue;
        };
        match restoration_unit_starts_at_first_block(context, plane) {
            Some(true) => {
                let _ = decode_restoration_unit(decoder, cdfs, reference, plane, restoration_type);
            }
            Some(false) => {}
            None => return false,
        }
    }
    true
}

/// Decode the single-unit Wiener/SGR profile while retaining the parameters for
/// the post-CDEF/post-superres pixel stage. This is the only restoration
/// decoder that publishes pixel-affecting state; the generic prefix helper
/// above remains a structural consumer for unsupported classes.
fn decode_bounded_restoration_plan(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    context: &FirstBlockContext,
    cdfs: &mut RestorationCdfs,
) -> Option<RestorationPlan> {
    if !context.single_tile || context.block_x != 0 || context.block_y != 0 {
        return None;
    }
    let mut references = [RestorationReference::defaults(); 3];
    let plane_count = if context.monochrome { 1 } else { 3 };
    let mut units = [None; 3];
    for (plane, reference) in references.iter_mut().enumerate().take(plane_count) {
        let Some(restoration_type) = context.restoration_types[plane] else {
            continue;
        };
        if !matches!(
            restoration_type,
            RestorationType::Wiener | RestorationType::SgrProjection
        ) {
            return None;
        }
        units[plane] = Some(decode_restoration_unit(
            decoder,
            cdfs,
            reference,
            plane,
            restoration_type,
        ));
    }
    Some(RestorationPlan { units })
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
    /// Nominal AV1 block width before clipping to the padded frame boundary.
    pub(super) coded_width: u32,
    /// Nominal AV1 block height before clipping to the padded frame boundary.
    pub(super) coded_height: u32,
    /// Normative block-size identity derived from the nominal coded extent.
    pub(super) block_size: BlockSize,
    /// Entropy-visible width clipped to the padded frame boundary.
    pub(super) width: u32,
    /// Entropy-visible height clipped to the padded frame boundary.
    pub(super) height: u32,
    pub(super) context: u8,
    pub(super) kind: PartitionKind,
    /// Partition-tree authorization for top-right and bottom-left intra edge
    /// extensions in each normative pixel layout.
    pub(super) intra_edges: IntraEdgeFlags,
}

fn partition_node_has_chroma(
    context: &FirstBlockContext,
    node: PartitionNode,
    standalone_tiny_frame: bool,
) -> bool {
    if context.monochrome {
        return false;
    }
    if standalone_tiny_frame {
        return true;
    }
    let owns_horizontal = !context.subsampling_x || node.coded_width > 1 || node.x % 2 != 0;
    let owns_vertical = !context.subsampling_y || node.coded_height > 1 || node.y % 2 != 0;
    owns_horizontal && owns_vertical
}

#[expect(
    clippy::too_many_arguments,
    reason = "the following-leaf boundary keeps entropy, tile state, raster, frame geometry, and block tools explicit"
)]
fn decode_complete_following_leaf(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    block_decoder: &mut super::block::Lossy420Decoder,
    tile_state: &TileState,
    canvas: &super::raster::FrameCanvas,
    context: &FirstBlockContext,
    node: PartitionNode,
    width: u32,
    height: u32,
    palette_coded_width: u32,
    palette_coded_height: u32,
    quantization: super::block::LossyQuantization,
    mut tools: super::block::BlockTools,
) -> Av1Result<super::block::PortableResult<super::block::FirstLeaf>> {
    let has_chroma = partition_node_has_chroma(context, node, false);
    let above = match node.y.checked_sub(1) {
        Some(y) => tile_state.neighbor_at_checked(node.x, y)?,
        None => None,
    };
    let left_x = node.x.checked_sub(1);
    let left = match left_x {
        Some(x) => tile_state.neighbor_at_checked(x, node.y)?,
        None => None,
    };
    if above.is_none() && left.is_none() {
        return Ok(Err(super::block::PortableUnavailable));
    }
    tools.skip_context = usize::from(above.is_some_and(|neighbor| neighbor.block_skipped))
        .saturating_add(usize::from(
            left.is_some_and(|neighbor| neighbor.block_skipped),
        ));

    let tx_left = left;
    let above_palette = above
        .and_then(|neighbor| tile_state.block(neighbor.owner))
        .map(|block| block.palette_cache);
    let left_palette = left
        .and_then(|neighbor| tile_state.block(neighbor.owner))
        .map(|block| block.palette_cache);
    tools.palette_context = super::block::PaletteNeighborContext::from_cache_states(
        node.y,
        above_palette,
        left_palette,
    );

    let above_luma_contexts =
        tile_state.luma_contexts_above::<32>(node.x, node.y, node.width.min(32))?;
    let left_luma_contexts =
        tile_state.luma_contexts_left::<32>(node.x, node.y, node.height.min(32))?;

    let chroma_above_y = if context.subsampling_y {
        node.y.saturating_sub(node.y % 2)
    } else {
        node.y
    };
    let chroma_context_x = if context.subsampling_x {
        node.x / 2
    } else {
        node.x
    };
    let chroma_context_y = if context.subsampling_y {
        chroma_above_y / 2
    } else {
        chroma_above_y
    };
    let chroma_context_width = if context.subsampling_x {
        node.width.div_ceil(2)
    } else {
        node.width
    };
    let above_chroma_contexts = [
        tile_state.chroma_contexts_above::<32>(
            0,
            chroma_context_x,
            chroma_context_y,
            chroma_context_width.min(32),
        )?,
        tile_state.chroma_contexts_above::<32>(
            1,
            chroma_context_x,
            chroma_context_y,
            chroma_context_width.min(32),
        )?,
    ];

    let chroma_left_x = if context.subsampling_x {
        node.x.saturating_sub(node.x % 2)
    } else {
        node.x
    };
    let chroma_left_y = if context.subsampling_y {
        node.y.saturating_sub(node.y % 2).saturating_add(1)
    } else {
        node.y
    };
    let chroma_context_left_x = if context.subsampling_x {
        chroma_left_x / 2
    } else {
        chroma_left_x
    };
    let chroma_context_left_y = if context.subsampling_y {
        chroma_left_y / 2
    } else {
        chroma_left_y
    };
    let chroma_context_height = if context.subsampling_y {
        node.height.div_ceil(2)
    } else {
        node.height
    };
    let left_chroma_contexts = [
        tile_state.chroma_contexts_left::<32>(
            0,
            chroma_context_left_x,
            chroma_context_left_y,
            chroma_context_height.min(32),
        )?,
        tile_state.chroma_contexts_left::<32>(
            1,
            chroma_context_left_x,
            chroma_context_left_y,
            chroma_context_height.min(32),
        )?,
    ];

    let smooth_luma = above
        .is_some_and(|neighbor| super::block::is_smooth_luma_predictor(neighbor.luma_predictor))
        || left.is_some_and(|neighbor| {
            super::block::is_smooth_luma_predictor(neighbor.luma_predictor)
        });
    // Match AV1's chroma-base adjustment: the above mode owner is selected
    // from the sampling-aligned base plus one luma unit on a subsampled axis,
    // independent of the current block width.
    let above_chroma_x = if context.subsampling_x {
        chroma_left_x.saturating_add(1)
    } else {
        node.x
    };
    let above_chroma = match chroma_above_y.checked_sub(1) {
        Some(y) => tile_state.chroma_neighbor_at_luma_checked(above_chroma_x, y)?,
        None => None,
    };
    let left_chroma = match chroma_left_x.checked_sub(1) {
        Some(x) => tile_state.chroma_neighbor_at_luma_checked(x, chroma_left_y)?,
        None => None,
    };
    let smooth_chroma = has_chroma
        && (above_chroma.is_some_and(|neighbor| {
            neighbor.has_chroma
                && super::block::is_smooth_chroma_predictor(neighbor.chroma_predictor)
        }) || left_chroma.is_some_and(|neighbor| {
            neighbor.has_chroma
                && super::block::is_smooth_chroma_predictor(neighbor.chroma_predictor)
        }));
    let edges = match canvas.intra_edges(
        node.x,
        node.y,
        palette_coded_width,
        palette_coded_height,
        has_chroma,
        tools.sample_depth,
        node.intra_edges,
        [smooth_luma, smooth_chroma, smooth_chroma],
    ) {
        Ok(edges) => edges,
        Err(_) => return Ok(Err(super::block::PortableUnavailable)),
    };

    let neighbors = super::block::SpatialNeighbors {
        above,
        left,
        tx_left,
        above_luma_contexts,
        left_luma_contexts,
        above_chroma_contexts,
        left_chroma_contexts,
    };
    if super::block::uses_streamed_intra(node.block_size) {
        Ok(block_decoder.decode_large_intra(
            decoder,
            node.block_size,
            width,
            height,
            has_chroma,
            quantization,
            tools,
            super::block::LargeIntraSpatial::Following {
                neighbors,
                edges: &edges,
            },
        ))
    } else {
        Ok(block_decoder.decode_following_from_edges(
            decoder,
            width,
            height,
            has_chroma,
            quantization,
            tools,
            neighbors,
            &edges,
        ))
    }
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
    above: Vec<u8>,
    left: Vec<u8>,
}

impl PartitionContexts {
    fn new(context: &FirstBlockContext) -> Av1Result<Self> {
        let width = context
            .block_width
            .saturating_sub(context.block_x)
            .div_ceil(2);
        let height = context
            .block_height
            .saturating_sub(context.block_y)
            .div_ceil(2);
        let width = usize::try_from(width)
            .map_err(|_| malformed("partition context width exceeds usize"))?;
        let height = usize::try_from(height)
            .map_err(|_| malformed("partition context height exceeds usize"))?;
        let mut above = Vec::new();
        above.try_reserve_exact(width).map_err(|_| {
            CodecError::Dimensions("unable to allocate AV1 partition context".to_owned())
        })?;
        above.resize(width, 0);
        let mut left = Vec::new();
        left.try_reserve_exact(height).map_err(|_| {
            CodecError::Dimensions("unable to allocate AV1 partition context".to_owned())
        })?;
        left.resize(height, 0);
        Ok(Self {
            origin_x: context.block_x,
            origin_y: context.block_y,
            above,
            left,
        })
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
        (width != 0 && height != 0)
            .then_some(())
            .ok_or_else(|| malformed("partition context footprint is empty"))?;
        let (start_x, start_y) = self.cell(x, y)?;
        // Terminal partition syntax retains its nominal coded footprint at a
        // frame edge. Convert the exclusive endpoint without asking `cell`
        // to validate an out-of-frame coordinate, then clip to the context
        // surfaces: those unavailable cells can never be consumed later.
        let end_x = x
            .checked_add(width)
            .and_then(|end| end.checked_sub(self.origin_x))
            .map(|relative| relative.div_ceil(2))
            .and_then(|end| usize::try_from(end).ok())
            .ok_or_else(|| malformed("partition context x endpoint overflows"))?
            .min(self.above.len());
        let end_y = y
            .checked_add(height)
            .and_then(|end| end.checked_sub(self.origin_y))
            .map(|relative| relative.div_ceil(2))
            .and_then(|end| usize::try_from(end).ok())
            .ok_or_else(|| malformed("partition context y endpoint overflows"))?
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

/// Recursive square-node availability used to reproduce AV1's static intra
/// edge tree without allocating or indexing a parallel tree at runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RecursiveIntraEdges {
    top_has_right: bool,
    left_has_bottom: bool,
}

impl RecursiveIntraEdges {
    /// Every superblock root may extend its top edge into the next coded
    /// superblock column, while its left edge may not extend into the next
    /// (not-yet-decoded) superblock row.
    const ROOT: Self = Self {
        top_has_right: true,
        left_has_bottom: false,
    };

    const fn flags(self) -> IntraEdgeFlags {
        IntraEdgeFlags::from_availability(self.top_has_right, self.left_has_bottom)
    }

    /// Recursive SPLIT children in AV1's TL, TR, BL, BR payload order.
    const fn child(self, index: usize) -> Option<Self> {
        match index {
            0 => Some(Self {
                top_has_right: true,
                left_has_bottom: true,
            }),
            1 => Some(Self {
                top_has_right: self.top_has_right,
                left_has_bottom: false,
            }),
            2 => Some(Self {
                top_has_right: true,
                left_has_bottom: self.left_has_bottom,
            }),
            3 => Some(Self {
                top_has_right: false,
                left_has_bottom: false,
            }),
            _ => None,
        }
    }
}

/// Exact edge-extension flags passed to one terminal block payload.
///
/// Levels 0-3 use AV1 branch-node rules. Level 4 uses the 8x8 tip rules,
/// whose 4:2:0 and 4:2:2 exceptions deliberately remain layout-specific.
fn terminal_intra_edges(
    level: u32,
    kind: PartitionKind,
    child: usize,
    recursive: RecursiveIntraEdges,
) -> Option<IntraEdgeFlags> {
    let edge = recursive.flags();
    let all_top = IntraEdgeFlags::ALL_TOP_HAS_RIGHT;
    let all_left = IntraEdgeFlags::ALL_LEFT_HAS_BOTTOM;
    let h0 = edge.union(all_left);
    let h1 = if level == 4 {
        edge.intersection(all_left.union(IntraEdgeFlags::I420_TOP))
    } else {
        edge.intersection(all_left)
    };
    let v0 = edge.union(all_top);
    let v1 = if level == 4 {
        edge.intersection(
            all_top
                .union(IntraEdgeFlags::I420_LEFT)
                .union(IntraEdgeFlags::I422_LEFT),
        )
    } else {
        edge.intersection(all_top)
    };

    match (kind, child) {
        (PartitionKind::None, 0) => Some(edge),
        (PartitionKind::Horizontal, 0) => Some(h0),
        (PartitionKind::Horizontal, 1) => Some(h1),
        (PartitionKind::Vertical, 0) => Some(v0),
        (PartitionKind::Vertical, 1) => Some(v1),
        (PartitionKind::Split, 0) if level == 4 => Some(IntraEdgeFlags::ALL),
        (PartitionKind::Split, 1) if level == 4 => {
            Some(edge.intersection(all_top).union(IntraEdgeFlags::I422_LEFT))
        }
        (PartitionKind::Split, 2) if level == 4 => Some(edge.union(IntraEdgeFlags::I444_TOP)),
        (PartitionKind::Split, 3) if level == 4 => Some(
            edge.intersection(
                IntraEdgeFlags::I420_TOP
                    .union(IntraEdgeFlags::I420_LEFT)
                    .union(IntraEdgeFlags::I422_LEFT),
            ),
        ),
        (PartitionKind::TopSplit, 0) => Some(IntraEdgeFlags::ALL),
        (PartitionKind::TopSplit, 1) => Some(v1),
        (PartitionKind::TopSplit, 2) => Some(h1),
        (PartitionKind::BottomSplit, 0) => Some(h0),
        (PartitionKind::BottomSplit, 1) => Some(v0),
        (PartitionKind::BottomSplit, 2) => Some(IntraEdgeFlags::NONE),
        (PartitionKind::LeftSplit, 0) => Some(IntraEdgeFlags::ALL),
        (PartitionKind::LeftSplit, 1) => Some(h1),
        (PartitionKind::LeftSplit, 2) => Some(v1),
        (PartitionKind::RightSplit, 0) => Some(v0),
        (PartitionKind::RightSplit, 1) => Some(h0),
        (PartitionKind::RightSplit, 2) => Some(IntraEdgeFlags::NONE),
        (PartitionKind::HorizontalFour, 0) => Some(h0),
        (PartitionKind::HorizontalFour, 1) => Some(
            edge.intersection(IntraEdgeFlags::I420_TOP)
                .select(level == 3)
                .union(all_left),
        ),
        (PartitionKind::HorizontalFour, 2) => Some(all_left),
        (PartitionKind::HorizontalFour, 3) => Some(h1),
        (PartitionKind::VerticalFour, 0) => Some(v0),
        (PartitionKind::VerticalFour, 1) => Some(
            edge.intersection(IntraEdgeFlags::I420_LEFT.union(IntraEdgeFlags::I422_LEFT))
                .select(level == 3)
                .union(all_top),
        ),
        (PartitionKind::VerticalFour, 2) => Some(all_top),
        (PartitionKind::VerticalFour, 3) => Some(v1),
        _ => None,
    }
}

impl<'decoder, 'data, 'input, 'spans> PartitionWalker<'decoder, 'data, 'input, 'spans> {
    fn new(
        decoder: &'decoder mut RangeDecoder<'data, 'input, 'spans>,
        context: &FirstBlockContext,
    ) -> Av1Result<Self> {
        Self::with_cdfs(decoder, context, PARTITION_CDFS)
    }

    fn with_cdfs(
        decoder: &'decoder mut RangeDecoder<'data, 'input, 'spans>,
        context: &FirstBlockContext,
        cdfs: [[[u16; 10]; 4]; 5],
    ) -> Av1Result<Self> {
        Ok(Self {
            decoder,
            cdfs,
            contexts: PartitionContexts::new(context)?,
            frame_width: context.block_width,
            frame_height: context.block_height,
            root_end_x: context.block_width,
            root_end_y: context.block_height,
            monochrome: context.monochrome,
            subsampling_x: context.subsampling_x,
            subsampling_y: context.subsampling_y,
            nodes: Vec::new(),
        })
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
        self.nodes.try_reserve(1).map_err(|_| {
            CodecError::Dimensions("unable to allocate AV1 partition nodes".to_owned())
        })?;
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
        intra_edges: RecursiveIntraEdges,
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
        for (child, coded_geometry) in children.into_iter().take(count).enumerate() {
            let Some(geometry) =
                clip_partition_geometry(coded_geometry, self.root_end_x, self.root_end_y)?
            else {
                continue;
            };
            let context = self
                .contexts
                .context(context_level, geometry.x, geometry.y)?;
            let block_size =
                BlockSize::from_mi_dimensions(coded_geometry.width, coded_geometry.height)
                    .ok_or_else(|| malformed("partition child has a non-normative block size"))?;
            let intra_edges = terminal_intra_edges(level, kind, child, intra_edges)
                .ok_or_else(|| malformed("partition child has invalid intra edge state"))?;
            let node = PartitionNode {
                level: child_level,
                x: geometry.x,
                y: geometry.y,
                coded_width: coded_geometry.width,
                coded_height: coded_geometry.height,
                block_size,
                width: geometry.width,
                height: geometry.height,
                context,
                kind: PartitionKind::None,
                intra_edges,
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
        self.walk_with_edges(level, x, y, RecursiveIntraEdges::ROOT, visit)
    }

    fn walk_with_edges<F>(
        &mut self,
        level: u32,
        x: u32,
        y: u32,
        intra_edges: RecursiveIntraEdges,
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
                let child_edges = intra_edges
                    .child(0)
                    .ok_or_else(|| malformed("partition edge child index exceeds four"))?;
                return self.walk_with_edges(level.saturating_add(1), x, y, child_edges, visit);
            }
            let width = self.frame_width.saturating_sub(x).min(half_size);
            let height = self.frame_height.saturating_sub(y).min(half_size);
            let context = self.contexts.context(level, x, y)?;
            let block_size = BlockSize::from_mi_dimensions(half_size, half_size)
                .ok_or_else(|| malformed("boundary leaf has a non-normative block size"))?;
            let node = PartitionNode {
                level,
                x,
                y,
                coded_width: half_size,
                coded_height: half_size,
                block_size,
                width,
                height,
                context,
                kind: PartitionKind::None,
                intra_edges: intra_edges.flags(),
            };
            self.contexts
                .record(level, PartitionKind::None, x, y, width, height)?;
            self.push(node)?;
            return visit(self.decoder, node);
        }

        let (kind, context) = self.decode_kind(level, x, y, horizontal_split, vertical_split)?;
        let full_width = half_size.saturating_mul(2);
        let full_height = half_size.saturating_mul(2);
        let block_size = BlockSize::from_mi_dimensions(full_width, full_height)
            .ok_or_else(|| malformed("partition parent has a non-normative block size"))?;
        let parent = PartitionNode {
            level,
            x,
            y,
            coded_width: full_width,
            coded_height: full_height,
            block_size,
            width: full_width,
            height: full_height,
            context,
            kind,
            intra_edges: intra_edges.flags(),
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
            let control = self.visit_terminal(level, kind, x, y, half_size, intra_edges, visit)?;
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
            let control = self.visit_terminal(level, kind, x, y, half_size, intra_edges, visit)?;
            if matches!(control, PartitionVisitControl::Stop) {
                return Ok(PartitionVisitControl::Stop);
            }
            self.contexts
                .record(level, kind, x, y, full_width, full_height)?;
            return Ok(PartitionVisitControl::Continue);
        }

        let next_level = level.saturating_add(1);
        if horizontal_split && vertical_split {
            let child_0 = intra_edges
                .child(0)
                .ok_or_else(|| malformed("partition edge child index exceeds four"))?;
            if matches!(
                self.walk_with_edges(next_level, x, y, child_0, visit)?,
                PartitionVisitControl::Stop
            ) {
                return Ok(PartitionVisitControl::Stop);
            }
            let child_1 = intra_edges
                .child(1)
                .ok_or_else(|| malformed("partition edge child index exceeds four"))?;
            if matches!(
                self.walk_with_edges(next_level, x.saturating_add(half_size), y, child_1, visit,)?,
                PartitionVisitControl::Stop
            ) {
                return Ok(PartitionVisitControl::Stop);
            }
            let child_2 = intra_edges
                .child(2)
                .ok_or_else(|| malformed("partition edge child index exceeds four"))?;
            if matches!(
                self.walk_with_edges(next_level, x, y.saturating_add(half_size), child_2, visit,)?,
                PartitionVisitControl::Stop
            ) {
                return Ok(PartitionVisitControl::Stop);
            }
            let child_3 = intra_edges
                .child(3)
                .ok_or_else(|| malformed("partition edge child index exceeds four"))?;
            self.walk_with_edges(
                next_level,
                x.saturating_add(half_size),
                y.saturating_add(half_size),
                child_3,
                visit,
            )
        } else if horizontal_split {
            let child_0 = intra_edges
                .child(0)
                .ok_or_else(|| malformed("partition edge child index exceeds four"))?;
            if matches!(
                self.walk_with_edges(next_level, x, y, child_0, visit)?,
                PartitionVisitControl::Stop
            ) {
                return Ok(PartitionVisitControl::Stop);
            }
            let child_1 = intra_edges
                .child(1)
                .ok_or_else(|| malformed("partition edge child index exceeds four"))?;
            self.walk_with_edges(next_level, x.saturating_add(half_size), y, child_1, visit)
        } else {
            let child_0 = intra_edges
                .child(0)
                .ok_or_else(|| malformed("partition edge child index exceeds four"))?;
            if matches!(
                self.walk_with_edges(next_level, x, y, child_0, visit)?,
                PartitionVisitControl::Stop
            ) {
                return Ok(PartitionVisitControl::Stop);
            }
            let child_2 = intra_edges
                .child(2)
                .ok_or_else(|| malformed("partition edge child index exceeds four"))?;
            self.walk_with_edges(next_level, x, y.saturating_add(half_size), child_2, visit)
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
    let mut walker = PartitionWalker::new(decoder, context)?;
    walker.walk(context.level, context.block_x, context.block_y, &mut visit)
}

/// Reconstruct the complete alpha tile for the bounded monochrome class whose
/// block syntax is currently closed: an 8/10/12-bit, lossless, intra frame
/// with no restoration or inter-frame tools.
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
    let mut walker = PartitionWalker::new(&mut decoder, context)?;
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
                    sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                        .ok_or_else(|| malformed("AV1 block sample depth is unsupported"))?,
                    allow_screen_content_tools: context.allow_screen_content_tools,
                    enable_filter_intra: context.enable_filter_intra,
                    enable_intra_edge_filter: context.enable_intra_edge_filter,
                    transform_mode: context.frame_tools.transform_mode,
                    transform_context: 0,
                    skip_context: 0,
                    suppress_delta_q_when_skipped: false,
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
    let sample_depth = super::sample_depth::SampleDepth::new(context.bit_depth)
        .ok_or_else(|| malformed("AV1 alpha sample depth is unsupported"))?;
    canvas.finish(sample_depth).map(Some)
}

fn no_unsupported_film_grain(context: &FirstBlockContext) -> bool {
    !context.frame_tools.film_grain_present
        || (context.bit_depth == 8
            && context.subsampling_x
            && context.subsampling_y
            && !context.monochrome)
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
    context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && context.monochrome
        && context.all_lossless
        && !context.superres_enabled
        && !context.segmentation_enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && no_unsupported_film_grain(context)
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
    pub(super) monochrome: bool,
    pub(super) subsampling_x: bool,
    pub(super) subsampling_y: bool,
    pub(super) filter_blocks: Vec<super::filter::Block>,
    pub(super) cdef_indices: Vec<Option<usize>>,
    pub(super) cdef_active: Vec<bool>,
    pub(super) loop_parameters: Option<super::filter::Parameters>,
    pub(super) cdef_parameters: Option<super::cdef::FrameParameters>,
    pub(super) restoration: Option<RestorationPlan>,
    pub(super) cdfs: Option<FrameCdfs>,
}

#[derive(Clone, Copy)]
struct SegmentMapUpdate {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    id: u8,
}

fn negative_deinterleave(diff: u8, predicted: u8, maximum: u8) -> u8 {
    if predicted == 0 {
        diff
    } else if predicted.saturating_add(1) >= maximum {
        maximum.wrapping_sub(diff.wrapping_add(1))
    } else if predicted.saturating_mul(2) < maximum {
        if diff <= predicted.saturating_mul(2) {
            if diff & 1 != 0 {
                predicted.saturating_add(diff.wrapping_add(1) >> 1)
            } else {
                predicted.saturating_sub(diff >> 1)
            }
        } else {
            diff
        }
    } else if diff
        <= maximum
            .saturating_sub(predicted)
            .saturating_sub(1)
            .saturating_mul(2)
    {
        if diff & 1 != 0 {
            predicted.saturating_add(diff.wrapping_add(1) >> 1)
        } else {
            predicted.saturating_sub(diff >> 1)
        }
    } else {
        maximum.wrapping_sub(diff.wrapping_add(1))
    }
}

fn spatial_segment_prediction(tile_state: &TileState, node: PartitionNode) -> (u8, usize) {
    let have_left = node.x != 0;
    let have_top = node.y != 0;
    match (have_left, have_top) {
        (true, true) => {
            let left = tile_state
                .segment_at(node.x.saturating_sub(1), node.y)
                .map_or(0, |(id, _)| id);
            let above = tile_state
                .segment_at(node.x, node.y.saturating_sub(1))
                .map_or(0, |(id, _)| id);
            let above_left = tile_state
                .segment_at(node.x.saturating_sub(1), node.y.saturating_sub(1))
                .map_or(0, |(id, _)| id);
            let context = if left == above && above_left == left {
                2
            } else if left == above || above_left == left || above == above_left {
                1
            } else {
                0
            };
            (if above == above_left { above } else { left }, context)
        }
        (true, false) => (
            tile_state
                .segment_at(node.x.saturating_sub(1), node.y)
                .map_or(0, |(id, _)| id),
            0,
        ),
        (false, true) => (
            tile_state
                .segment_at(node.x, node.y.saturating_sub(1))
                .map_or(0, |(id, _)| id),
            0,
        ),
        (false, false) => (0, 0),
    }
}

fn previous_segment_id(
    previous: Option<&SegmentMap>,
    context: &FirstBlockContext,
    node: PartitionNode,
) -> u8 {
    previous.map_or(0, |map| {
        map.minimum(
            context.tile_origin_b4_x.saturating_add(node.x),
            context.tile_origin_b4_y.saturating_add(node.y),
            node.width,
            node.height,
        )
    })
}

/// Resolve one segment ID at either the pre-skip (`skip == None`) or
/// post-skip (`skip == Some`) syntax position.
fn decode_segment_id(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut FrameCdfs,
    context: &FirstBlockContext,
    node: PartitionNode,
    tile_state: &TileState,
    previous: Option<&SegmentMap>,
    skip: Option<bool>,
) -> Av1Result<(u8, bool)> {
    let segmentation = context.frame_tools.segmentation;
    if !segmentation.enabled {
        return Ok((0, false));
    }
    if !segmentation.update_map {
        return Ok((previous_segment_id(previous, context, node), false));
    }
    let above_pred = node
        .y
        .checked_sub(1)
        .and_then(|y| tile_state.segment_at(node.x, y))
        .is_some_and(|(_, predicted)| predicted);
    let left_pred = node
        .x
        .checked_sub(1)
        .and_then(|x| tile_state.segment_at(x, node.y))
        .is_some_and(|(_, predicted)| predicted);
    let temporal_allowed = segmentation.temporal && skip != Some(true);
    let predicted_from_previous = temporal_allowed
        && decoder.adaptive_bool(
            &mut cdfs.inter.segment_prediction[usize::from(above_pred) + usize::from(left_pred)].0,
        );
    if predicted_from_previous {
        return Ok((previous_segment_id(previous, context, node), true));
    }
    let (predicted, spatial_context) = spatial_segment_prediction(tile_state, node);
    if skip == Some(true) {
        return Ok((predicted, false));
    }
    let difference = decoder.adaptive_symbol(&mut cdfs.segment_id[spatial_context], 7);
    let difference =
        u8::try_from(difference).map_err(|_| malformed("segment-id difference exceeds u8"))?;
    let active_count = u8::try_from(segmentation.last_active_id.saturating_add(1).clamp(0, 8))
        .map_err(|_| malformed("active segment count exceeds u8"))?;
    let decoded = negative_deinterleave(difference, predicted, active_count);
    if decoded >= active_count {
        return Err(malformed("decoded segment id exceeds the active range"));
    }
    Ok((decoded, false))
}

fn skip_context_for_node(tile_state: &TileState, node: PartitionNode) -> Av1Result<usize> {
    let above = match node.y.checked_sub(1) {
        Some(y) => tile_state.neighbor_at_checked(node.x, y)?,
        None => None,
    };
    let left = match node.x.checked_sub(1) {
        Some(x) => tile_state.neighbor_at_checked(x, node.y)?,
        None => None,
    };
    Ok(
        usize::from(above.is_some_and(|neighbor| neighbor.block_skipped))
            + usize::from(left.is_some_and(|neighbor| neighbor.block_skipped)),
    )
}

fn inter_neighbors(
    tile_state: &TileState,
    node: PartitionNode,
) -> Av1Result<[Option<SpatialRefBlock>; 2]> {
    let above = node
        .y
        .checked_sub(1)
        .map(|y| tile_state.spatial_block(node.x, y))
        .transpose()?
        .flatten();
    let left = node
        .x
        .checked_sub(1)
        .map(|x| tile_state.spatial_block(x, node.y))
        .transpose()?
        .flatten();
    Ok([above, left])
}

fn inter_intra_context(neighbors: [Option<SpatialRefBlock>; 2]) -> usize {
    let count = neighbors
        .into_iter()
        .flatten()
        .filter(|block| block.is_intra())
        .count();
    match count {
        2 => 3,
        1 => 2,
        _ => 0,
    }
}

fn compare_reference_counts(left: u32, right: u32) -> u8 {
    match left.cmp(&right) {
        std::cmp::Ordering::Less => 0,
        std::cmp::Ordering::Equal => 1,
        std::cmp::Ordering::Greater => 2,
    }
}

fn inter_reference_context(neighbors: [Option<SpatialRefBlock>; 2]) -> u8 {
    let mut forward = 0_u32;
    let mut backward = 0_u32;
    for block in neighbors
        .into_iter()
        .flatten()
        .filter(|block| !block.is_intra())
    {
        for reference in block.references {
            if reference < 0 {
                continue;
            }
            if reference >= 5 {
                backward = backward.saturating_add(1);
            } else if reference > 0 {
                forward = forward.saturating_add(1);
            }
        }
    }
    compare_reference_counts(forward, backward)
}

fn interintra_allowed(block_size: BlockSize) -> bool {
    matches!(
        block_size,
        BlockSize::B8x8
            | BlockSize::B8x16
            | BlockSize::B16x8
            | BlockSize::B16x16
            | BlockSize::B16x32
            | BlockSize::B32x16
            | BlockSize::B32x32
    )
}

fn interintra_size_group(block_size: BlockSize) -> usize {
    let (width, height) = block_size.mi_dimensions();
    usize::try_from(width.ilog2().min(height.ilog2()).min(3)).unwrap_or(3)
}

/// Consume the inter-intra sentence even though this reconstruction tranche
/// deliberately rejects the blended predictor. Keeping the exact CDF rows
/// and wedge context means the private decoder never mistakes an inter-intra
/// block for a residual-bearing translation block.
fn decode_interintra_syntax(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut FrameCdfs,
    inter_context: &InterFrameContext<'_>,
    block_size: BlockSize,
    compound: bool,
    skip_mode: bool,
) -> Av1Result<bool> {
    if !inter_context.enable_interintra_compound
        || compound
        || skip_mode
        || !interintra_allowed(block_size)
    {
        return Ok(false);
    }
    let size_group = interintra_size_group(block_size);
    if !decoder.adaptive_bool(&mut cdfs.inter.interintra_for(size_group).0) {
        return Ok(false);
    }
    let mode = decoder.adaptive_symbol(&mut cdfs.inter.interintra_mode_for(size_group).0, 3);
    if mode > 3 {
        return Err(malformed("inter-intra mode symbol is invalid"));
    }
    let use_wedge = decoder.adaptive_bool(&mut cdfs.inter.interintra_wedge_for(block_size).0);
    if use_wedge {
        let wedge = decoder.adaptive_symbol(&mut cdfs.inter.wedge_index_for(block_size).0, 15);
        if wedge > 15 {
            return Err(malformed("inter-intra wedge index is invalid"));
        }
    }
    Ok(true)
}

fn has_overlappable_neighbor(tile_state: &TileState, node: PartitionNode) -> Av1Result<bool> {
    let (width, height) = node.block_size.mi_dimensions();
    if width.min(height) < 2 {
        return Ok(false);
    }
    if let Some(y) = node.y.checked_sub(1) {
        for offset in 0..width {
            let x = node
                .x
                .checked_add(offset)
                .ok_or_else(|| malformed("overlappable-neighbor x overflows"))?;
            if tile_state
                .neighbor_at_checked(x, y)?
                .is_some_and(|neighbor| neighbor.coding.inter().is_some())
            {
                return Ok(true);
            }
        }
    }
    if let Some(x) = node.x.checked_sub(1) {
        for offset in 0..height {
            let y = node
                .y
                .checked_add(offset)
                .ok_or_else(|| malformed("overlappable-neighbor y overflows"))?;
            if tile_state
                .neighbor_at_checked(x, y)?
                .is_some_and(|neighbor| neighbor.coding.inter().is_some())
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn interpolation_filter_symbol(filter: InterpolationFilter) -> usize {
    match filter {
        InterpolationFilter::Regular => 0,
        InterpolationFilter::Smooth => 1,
        InterpolationFilter::Sharp => 2,
        // Bilinear is not one of the switchable symbols; treating it as
        // unknown matches the reference context derivation.
        InterpolationFilter::Bilinear => 3,
    }
}

fn neighbor_interpolation_filter(
    tile_state: &TileState,
    x: u32,
    y: u32,
    reference: ReferenceFrame,
    direction: usize,
) -> Av1Result<usize> {
    let Some(neighbor) = tile_state.neighbor_at_checked(x, y)? else {
        return Ok(3);
    };
    let Some(inter) = neighbor.coding.inter() else {
        return Ok(3);
    };
    if inter.references.first != reference && inter.references.second != Some(reference) {
        return Ok(3);
    }
    Ok(interpolation_filter_symbol(
        inter.filters[if direction == 0 { 1 } else { 0 }],
    ))
}

fn switchable_interpolation_context(
    tile_state: &TileState,
    node: PartitionNode,
    reference: ReferenceFrame,
    compound: bool,
    direction: usize,
) -> Av1Result<usize> {
    let left = node
        .x
        .checked_sub(1)
        .map(|x| neighbor_interpolation_filter(tile_state, x, node.y, reference, direction))
        .transpose()?
        .unwrap_or(3);
    let above = node
        .y
        .checked_sub(1)
        .map(|y| neighbor_interpolation_filter(tile_state, node.x, y, reference, direction))
        .transpose()?
        .unwrap_or(3);
    let context = if left == above {
        left
    } else if left == 3 {
        above
    } else if above == 3 {
        left
    } else {
        3
    };
    Ok(context.saturating_add(usize::from(compound) * 4))
}

fn inter_forward_reference_context(neighbors: [Option<SpatialRefBlock>; 2]) -> (u8, u8, u8) {
    let mut broad = [0_u32; 4];
    let mut first = [0_u32; 2];
    let mut second = [0_u32; 2];
    for block in neighbors
        .into_iter()
        .flatten()
        .filter(|block| !block.is_intra())
    {
        for reference in block.references {
            if reference <= 0 || reference >= 5 {
                continue;
            }
            let reference = usize::try_from(reference - 1).unwrap_or(usize::MAX);
            if let Some(count) = broad.get_mut(reference) {
                *count = count.saturating_add(1);
            }
            if reference < 2 {
                first[reference] = first[reference].saturating_add(1);
            } else if let Some(count) = second.get_mut(reference - 2) {
                *count = count.saturating_add(1);
            }
        }
    }
    (
        compare_reference_counts(
            broad[0].saturating_add(broad[1]),
            broad[2].saturating_add(broad[3]),
        ),
        compare_reference_counts(first[0], first[1]),
        compare_reference_counts(second[0], second[1]),
    )
}

fn inter_backward_reference_context(neighbors: [Option<SpatialRefBlock>; 2]) -> (u8, u8) {
    let mut counts = [0_u32; 3];
    for block in neighbors
        .into_iter()
        .flatten()
        .filter(|block| !block.is_intra())
    {
        for reference in block.references {
            if reference >= 5 {
                let index = usize::try_from(reference - 5).unwrap_or(usize::MAX);
                if let Some(count) = counts.get_mut(index) {
                    *count = count.saturating_add(1);
                }
            }
        }
    }
    (
        compare_reference_counts(counts[0].saturating_add(counts[1]), counts[2]),
        compare_reference_counts(counts[0], counts[1]),
    )
}

fn reference_type_from_sentinel(value: i8) -> Option<ReferenceFrame> {
    (value > 0)
        .then(|| ReferenceFrame::from_index(usize::try_from(value - 1).ok()?))
        .flatten()
}

fn neighbor_reference_types(
    block: SpatialRefBlock,
) -> (Option<ReferenceFrame>, Option<ReferenceFrame>) {
    if block.is_intra() {
        return (None, None);
    }
    (
        reference_type_from_sentinel(block.references[0]),
        reference_type_from_sentinel(block.references[1]),
    )
}

fn is_backward_reference(reference: Option<ReferenceFrame>) -> bool {
    reference.is_some_and(|reference| !reference.is_forward())
}

fn is_unidirectional_compound(
    references: (Option<ReferenceFrame>, Option<ReferenceFrame>),
) -> bool {
    references
        .0
        .zip(references.1)
        .is_some_and(|(first, second)| first.is_forward() == second.is_forward())
}

/// Context for the reference-mode (single versus compound) symbol.  AV1
/// treats intra neighbours as an absent reference rather than as a synthetic
/// forward slot; keeping that distinction is required for the 2/3/4 rows.
fn inter_compound_context(neighbors: [Option<SpatialRefBlock>; 2]) -> usize {
    let above = neighbors[0].map(neighbor_reference_types);
    let left = neighbors[1].map(neighbor_reference_types);
    match (above, left) {
        (Some(above), Some(left)) => match (above.1.is_some(), left.1.is_some()) {
            (false, false) => {
                usize::from(is_backward_reference(above.0))
                    ^ usize::from(is_backward_reference(left.0))
            }
            (false, true) => 2 + usize::from(above.0.is_none() || is_backward_reference(above.0)),
            (true, false) => 2 + usize::from(left.0.is_none() || is_backward_reference(left.0)),
            (true, true) => 4,
        },
        (Some(reference), None) | (None, Some(reference)) => {
            if reference.1.is_some() {
                3
            } else {
                usize::from(is_backward_reference(reference.0))
            }
        }
        (None, None) => 1,
    }
}

/// Context for the bidirectional/unidirectional compound reference tree.
fn inter_compound_reference_type_context(neighbors: [Option<SpatialRefBlock>; 2]) -> usize {
    let above = neighbors[0].map(neighbor_reference_types);
    let left = neighbors[1].map(neighbor_reference_types);
    match (above, left) {
        (Some(above), Some(left)) => {
            let above_intra = above.0.is_none();
            let left_intra = left.0.is_none();
            if above_intra && left_intra {
                2
            } else if above_intra || left_intra {
                let inter = if above_intra { left } else { above };
                if inter.1.is_none() {
                    2
                } else {
                    1 + 2 * usize::from(is_unidirectional_compound(inter))
                }
            } else {
                let above_single = above.1.is_none();
                let left_single = left.1.is_none();
                if above_single && left_single {
                    1 + 2 * usize::from(
                        is_backward_reference(above.0) == is_backward_reference(left.0),
                    )
                } else if above_single || left_single {
                    let compound = if above_single { left } else { above };
                    if !is_unidirectional_compound(compound) {
                        1
                    } else {
                        3 + usize::from(
                            is_backward_reference(above.0) == is_backward_reference(left.0),
                        )
                    }
                } else {
                    let above_uni = is_unidirectional_compound(above);
                    let left_uni = is_unidirectional_compound(left);
                    if !above_uni && !left_uni {
                        0
                    } else if above_uni != left_uni {
                        2
                    } else {
                        3 + usize::from(
                            is_backward_reference(above.0) == is_backward_reference(left.0),
                        )
                    }
                }
            }
        }
        (Some(reference), None) | (None, Some(reference)) => {
            if reference.0.is_none() || reference.1.is_none() {
                2
            } else {
                4 * usize::from(is_unidirectional_compound(reference))
            }
        }
        (None, None) => 2,
    }
}

fn inter_compound_reference_contexts(neighbors: [Option<SpatialRefBlock>; 2]) -> [usize; 8] {
    let mut counts = [0_u32; 7];
    for block in neighbors
        .into_iter()
        .flatten()
        .filter(|block| !block.is_intra())
    {
        for reference in block.references {
            let Some(reference) = reference_type_from_sentinel(reference) else {
                continue;
            };
            if let Some(count) = counts.get_mut(reference.index()) {
                *count = count.saturating_add(1);
            }
        }
    }
    [
        usize::from(compare_reference_counts(
            counts[0].saturating_add(counts[1]),
            counts[2].saturating_add(counts[3]),
        )),
        usize::from(compare_reference_counts(counts[0], counts[1])),
        usize::from(compare_reference_counts(counts[2], counts[3])),
        usize::from(compare_reference_counts(
            counts[4].saturating_add(counts[5]),
            counts[6],
        )),
        usize::from(compare_reference_counts(counts[4], counts[5])),
        usize::from(compare_reference_counts(
            counts[0]
                .saturating_add(counts[1])
                .saturating_add(counts[2])
                .saturating_add(counts[3]),
            counts[4]
                .saturating_add(counts[5])
                .saturating_add(counts[6]),
        )),
        usize::from(compare_reference_counts(
            counts[1],
            counts[2].saturating_add(counts[3]),
        )),
        usize::from(compare_reference_counts(counts[2], counts[3])),
    ]
}

fn compound_mode_context(mode_context: u8) -> usize {
    const COMPOUND_MODE_CONTEXT_MAP: [[usize; 5]; 3] =
        [[0, 1, 1, 1, 1], [1, 2, 3, 4, 4], [4, 4, 5, 6, 7]];
    let new_mv_context = usize::from(mode_context & 7).min(4);
    let ref_mv_context = usize::from((mode_context >> 4) & 7).min(5);
    COMPOUND_MODE_CONTEXT_MAP[ref_mv_context >> 1][new_mv_context]
}

fn decode_inter_compound_references(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut FrameCdfs,
    neighbors: [Option<SpatialRefBlock>; 2],
) -> Av1Result<ReferencePair> {
    let type_context = inter_compound_reference_type_context(neighbors).min(4);
    let contexts = inter_compound_reference_contexts(neighbors);
    let bidirectional = decoder.adaptive_bool(&mut cdfs.inter.compound_direction[type_context].0);
    if bidirectional {
        let first_group =
            decoder.adaptive_bool(&mut cdfs.inter.compound_forward_reference[contexts[0]][0].0);
        let first = if first_group {
            if decoder.adaptive_bool(&mut cdfs.inter.compound_forward_reference[contexts[2]][2].0) {
                ReferenceFrame::Golden
            } else {
                ReferenceFrame::Last3
            }
        } else if decoder
            .adaptive_bool(&mut cdfs.inter.compound_forward_reference[contexts[1]][1].0)
        {
            ReferenceFrame::Last2
        } else {
            ReferenceFrame::Last
        };
        let second_group =
            decoder.adaptive_bool(&mut cdfs.inter.compound_backward_reference[contexts[3]][0].0);
        let second = if second_group {
            ReferenceFrame::Alt
        } else if decoder
            .adaptive_bool(&mut cdfs.inter.compound_backward_reference[contexts[4]][1].0)
        {
            ReferenceFrame::Alt2
        } else {
            ReferenceFrame::Backward
        };
        Ok(ReferencePair::compound(first, second))
    } else {
        let backward = decoder
            .adaptive_bool(&mut cdfs.inter.compound_unidirectional_reference[contexts[5]][0].0);
        if backward {
            return Ok(ReferencePair::compound(
                ReferenceFrame::Backward,
                ReferenceFrame::Alt,
            ));
        }
        let late_forward = decoder
            .adaptive_bool(&mut cdfs.inter.compound_unidirectional_reference[contexts[6]][1].0);
        if late_forward {
            return if decoder
                .adaptive_bool(&mut cdfs.inter.compound_unidirectional_reference[contexts[7]][2].0)
            {
                Ok(ReferencePair::compound(
                    ReferenceFrame::Last,
                    ReferenceFrame::Golden,
                ))
            } else {
                Ok(ReferencePair::compound(
                    ReferenceFrame::Last,
                    ReferenceFrame::Last3,
                ))
            };
        }
        Ok(ReferencePair::compound(
            ReferenceFrame::Last,
            ReferenceFrame::Last2,
        ))
    }
}

fn decode_inter_single_reference(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut FrameCdfs,
    neighbors: [Option<SpatialRefBlock>; 2],
) -> Av1Result<ReferenceFrame> {
    let ctx = usize::from(inter_reference_context(neighbors));
    let first = decoder.adaptive_bool(
        &mut cdfs
            .inter
            .reference
            .get_mut(0)
            .and_then(|row| row.get_mut(ctx))
            .ok_or_else(|| malformed("single-reference CDF context exceeds three"))?
            .0,
    );
    if first {
        let (backward_ctx, backward_one_ctx) = inter_backward_reference_context(neighbors);
        if decoder.adaptive_bool(
            &mut cdfs
                .inter
                .reference
                .get_mut(1)
                .and_then(|row| row.get_mut(usize::from(backward_ctx)))
                .ok_or_else(|| malformed("backward-reference CDF context exceeds three"))?
                .0,
        ) {
            return Ok(ReferenceFrame::Alt);
        }
        return ReferenceFrame::from_index(
            4 + usize::from(
                decoder.adaptive_bool(
                    &mut cdfs
                        .inter
                        .reference
                        .get_mut(3)
                        .and_then(|row| row.get_mut(usize::from(backward_one_ctx)))
                        .ok_or_else(|| malformed("backward-reference leaf CDF is unavailable"))?
                        .0,
                ),
            ),
        )
        .ok_or_else(|| malformed("decoded backward reference exceeds seven"));
    }
    let (forward_ctx, forward_one_ctx, forward_two_ctx) =
        inter_forward_reference_context(neighbors);
    if decoder.adaptive_bool(
        &mut cdfs
            .inter
            .reference
            .get_mut(2)
            .and_then(|row| row.get_mut(usize::from(forward_ctx)))
            .ok_or_else(|| malformed("forward-reference CDF context exceeds three"))?
            .0,
    ) {
        return ReferenceFrame::from_index(
            2 + usize::from(
                decoder.adaptive_bool(
                    &mut cdfs
                        .inter
                        .reference
                        .get_mut(4)
                        .and_then(|row| row.get_mut(usize::from(forward_two_ctx)))
                        .ok_or_else(|| malformed("forward-reference leaf CDF is unavailable"))?
                        .0,
                ),
            ),
        )
        .ok_or_else(|| malformed("decoded forward reference exceeds seven"));
    }
    ReferenceFrame::from_index(usize::from(
        decoder.adaptive_bool(
            &mut cdfs
                .inter
                .reference
                .get_mut(3)
                .and_then(|row| row.get_mut(usize::from(forward_one_ctx)))
                .ok_or_else(|| malformed("forward-reference leaf CDF is unavailable"))?
                .0,
        ),
    ))
    .ok_or_else(|| malformed("decoded forward reference exceeds seven"))
}

fn decode_mv_component(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdf: &mut super::frame_cdfs::MvComponentCdfs,
    mv_precision: i32,
) -> Av1Result<i16> {
    let negative = decoder.adaptive_bool(&mut cdf.sign.0);
    let class = decoder.adaptive_symbol(&mut cdf.classes.0, 10);
    let (up, fractional, high_precision) = if class == 0 {
        let up = u16::from(decoder.adaptive_bool(&mut cdf.class_zero.0));
        let fractional = if mv_precision >= 0 {
            decoder.adaptive_symbol(&mut cdf.class_zero_fractional[usize::from(up)].0, 3)
        } else {
            3
        };
        let high_precision = if mv_precision > 0 {
            u16::from(decoder.adaptive_bool(&mut cdf.class_zero_high_precision.0))
        } else {
            1
        };
        (up, fractional, high_precision)
    } else {
        let mut up = 1_u16
            .checked_shl(class)
            .ok_or_else(|| malformed("motion-vector class shift overflows"))?;
        for bit in 0..class {
            let value = u16::from(
                decoder.adaptive_bool(
                    &mut cdf
                        .class_n_bits
                        .get_mut(
                            usize::try_from(bit)
                                .map_err(|_| malformed("motion-vector class index exceeds ten"))?,
                        )
                        .ok_or_else(|| malformed("motion-vector class index exceeds ten"))?
                        .0,
                ),
            );
            up |= value
                .checked_shl(bit)
                .ok_or_else(|| malformed("motion-vector class bit shift overflows"))?;
        }
        let fractional = if mv_precision >= 0 {
            decoder.adaptive_symbol(&mut cdf.class_n_fractional.0, 3)
        } else {
            3
        };
        let high_precision = if mv_precision > 0 {
            u16::from(decoder.adaptive_bool(&mut cdf.class_n_high_precision.0))
        } else {
            1
        };
        (up, fractional, high_precision)
    };
    let magnitude = u32::from(up)
        .checked_shl(3)
        .and_then(|value| value.checked_add(fractional << 1))
        .and_then(|value| value.checked_add(u32::from(high_precision)))
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| malformed("motion-vector magnitude overflows"))?;
    let magnitude =
        i32::try_from(magnitude).map_err(|_| malformed("motion-vector magnitude exceeds i32"))?;
    let value = if negative { -magnitude } else { magnitude };
    i16::try_from(value).map_err(|_| malformed("motion-vector residual exceeds i16"))
}

fn decode_mv_residual(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut super::frame_cdfs::MvCdfs,
    vector: &mut MotionVector,
    force_integer_mv: bool,
    high_precision_mv: bool,
) -> Av1Result<()> {
    let joint = decoder.adaptive_symbol(&mut cdfs.joint.0, 3);
    let precision = i32::from(high_precision_mv) - i32::from(force_integer_mv);
    if joint == 2 || joint == 3 {
        let residual = decode_mv_component(decoder, &mut cdfs.component[0], precision)?;
        *vector = vector.checked_add(MotionVector { y: residual, x: 0 })?;
    }
    if joint == 1 || joint == 3 {
        let residual = decode_mv_component(decoder, &mut cdfs.component[1], precision)?;
        *vector = vector.checked_add(MotionVector { y: 0, x: residual })?;
    }
    Ok(())
}

fn inter_transform_from_symbol(
    offset: usize,
    symbol: u32,
) -> Option<super::block::Av1TransformType> {
    let index = offset.checked_add(usize::try_from(symbol).ok()?)?;
    Some(match index {
        12 => super::block::Av1TransformType::IdentityIdentity,
        13 => super::block::Av1TransformType::VerticalDct,
        14 => super::block::Av1TransformType::HorizontalDct,
        15 => super::block::Av1TransformType::VerticalAdst,
        16 => super::block::Av1TransformType::HorizontalAdst,
        17 => super::block::Av1TransformType::VerticalFlipAdst,
        18 => super::block::Av1TransformType::HorizontalFlipAdst,
        19 => super::block::Av1TransformType::DctDct,
        20 => super::block::Av1TransformType::AdstDct,
        21 => super::block::Av1TransformType::DctAdst,
        22 => super::block::Av1TransformType::FlipAdstDct,
        23 => super::block::Av1TransformType::DctFlipAdst,
        24 => super::block::Av1TransformType::IdentityIdentity,
        25 => super::block::Av1TransformType::VerticalDct,
        26 => super::block::Av1TransformType::HorizontalDct,
        27 => super::block::Av1TransformType::VerticalAdst,
        28 => super::block::Av1TransformType::HorizontalAdst,
        29 => super::block::Av1TransformType::VerticalFlipAdst,
        30 => super::block::Av1TransformType::HorizontalFlipAdst,
        31 => super::block::Av1TransformType::DctDct,
        32 => super::block::Av1TransformType::AdstDct,
        33 => super::block::Av1TransformType::DctAdst,
        34 => super::block::Av1TransformType::FlipAdstDct,
        35 => super::block::Av1TransformType::DctFlipAdst,
        36 => super::block::Av1TransformType::AdstAdst,
        37 => super::block::Av1TransformType::FlipAdstFlipAdst,
        38 => super::block::Av1TransformType::AdstFlipAdst,
        39 => super::block::Av1TransformType::FlipAdstAdst,
        _ => return None,
    })
}

fn decode_inter_transform_type(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut FrameCdfs,
    tx_size: TxSize,
    reduced_transform_set: bool,
    segment_lossless: bool,
) -> Av1Result<super::block::Av1TransformType> {
    if segment_lossless
        || tx_size
            .pixel_dimensions()
            .0
            .max(tx_size.pixel_dimensions().1)
            >= 64
    {
        return Ok(super::block::Av1TransformType::DctDct);
    }
    if reduced_transform_set
        || tx_size
            .pixel_dimensions()
            .0
            .max(tx_size.pixel_dimensions().1)
            == 32
    {
        return Ok(
            if decoder
                .adaptive_bool(&mut cdfs.common.inter_transform_3[tx_size.minimum_context()].0)
            {
                super::block::Av1TransformType::DctDct
            } else {
                super::block::Av1TransformType::IdentityIdentity
            },
        );
    }
    if tx_size.minimum_context() == 2 {
        let symbol = decoder.adaptive_symbol(&mut cdfs.common.inter_transform_2.0, 11);
        return inter_transform_from_symbol(12, symbol)
            .ok_or_else(|| malformed("inter transform symbol is invalid"));
    }
    let context = tx_size.minimum_context().min(1);
    let symbol = decoder.adaptive_symbol(&mut cdfs.common.inter_transform_1[context].0, 15);
    inter_transform_from_symbol(24, symbol)
        .ok_or_else(|| malformed("inter transform symbol is invalid"))
}

fn decode_inter_transform_size(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut FrameCdfs,
    block_size: BlockSize,
    transform_mode: u32,
) -> super::block::PortableResult<TxSize> {
    let mut tx_size = if transform_mode == 0 {
        TxSize::Tx4x4
    } else {
        block_size.maximum_luma_tx()
    };
    if transform_mode != 2 {
        return Ok(tx_size);
    }
    let max_depth = block_size.transform_size_max().min(2);
    for depth in 0..max_depth {
        if tx_size == TxSize::Tx4x4 {
            break;
        }
        let max_axis = tx_size
            .pixel_dimensions()
            .0
            .max(tx_size.pixel_dimensions().1);
        let max_index = max_axis.ilog2().saturating_sub(2);
        let category = 2_u32
            .saturating_mul(4_u32.saturating_sub(max_index))
            .saturating_sub(u32::try_from(depth).unwrap_or(2));
        let category =
            usize::try_from(category.min(6)).map_err(|_| super::block::PortableUnavailable)?;
        let context = 0_usize;
        let split =
            decoder.adaptive_bool(&mut cdfs.common.transform_partition[category][context].0);
        if !split {
            return Ok(tx_size);
        }
        tx_size = tx_size.sub_size();
    }
    Err(super::block::PortableUnavailable)
}

#[derive(Clone)]
struct DecodedInterLeaf {
    leaf: super::block::FirstLeaf,
    metadata: super::tile_state::InterBlockMeta,
}

#[expect(
    clippy::too_many_arguments,
    reason = "the inter leaf boundary carries syntax, tile state, frame references, geometry, and block tools explicitly"
)]
fn decode_inter_leaf(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut FrameCdfs,
    block_decoder: &mut super::block::Lossy420Decoder,
    tile_state: &TileState,
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
    node: PartitionNode,
    visible_width: u32,
    visible_height: u32,
    selected_segment: SegmentContext,
    block_skipped: bool,
    prepared_quantization: super::block::PreparedInterQuantization,
    tools: super::block::BlockTools,
) -> Av1Result<super::block::PortableResult<DecodedInterLeaf>> {
    if matches!(context.bit_depth, 10 | 12) {
        let (block_width, block_height) = node.block_size.pixel_dimensions();
        let (minimum, maximum) = if context.monochrome { (4, 64) } else { (8, 32) };
        if !(minimum..=maximum).contains(&block_width)
            || !(minimum..=maximum).contains(&block_height)
        {
            return Ok(Err(super::block::PortableUnavailable));
        }
    }
    if selected_segment.reference == 0 {
        return Ok(Err(super::block::PortableUnavailable));
    }
    let neighbors = inter_neighbors(tile_state, node)?;
    let intra_context = inter_intra_context(neighbors);
    let inter_flag = if selected_segment.reference > 0 || selected_segment.global_motion {
        true
    } else {
        decoder.adaptive_bool(&mut cdfs.inter.intra[intra_context].0)
    };
    if !inter_flag {
        return Ok(Err(super::block::PortableUnavailable));
    }
    let (block_width_b4, block_height_b4) = node.block_size.mi_dimensions();
    // Frame-level skip mode is rejected by the current admission gate. Keep
    // the per-block value explicit so compound/inter-intra syntax cannot be
    // accidentally tied to residual skipping as that gate widens.
    let skip_mode = false;
    let forced_global = selected_segment.global_motion || selected_segment.skip;
    let forced_reference = selected_segment.reference > 0 || forced_global;
    let compound_allowed = inter_context.reference_mode_select
        && block_width_b4.min(block_height_b4) > 1
        && !skip_mode
        && !forced_reference;
    let compound = if compound_allowed {
        decoder.adaptive_bool(&mut cdfs.inter.compound[inter_compound_context(neighbors)].0)
    } else {
        false
    };
    let (references, motions, mode, global, new_mv) = if compound {
        // The average-only tranche deliberately closes the joint/masked
        // compound tools at the block boundary. Frames may still contain
        // supported single-reference blocks when those sequence flags are on.
        if inter_context.enable_masked_compound || inter_context.enable_jnt_comp {
            return Ok(Err(super::block::PortableUnavailable));
        }
        let references = decode_inter_compound_references(decoder, cdfs, neighbors)?;
        let second = references
            .second
            .ok_or_else(|| malformed("compound reference pair omits its second reference"))?;
        let request = inter_context.mv_request(
            ReferenceMvTarget::Compound(references),
            node.block_size,
            node.x,
            node.y,
            context
                .tile_origin_b4_x
                .checked_add(node.x)
                .ok_or_else(|| malformed("compound MV x coordinate overflows"))?,
            context
                .tile_origin_b4_y
                .checked_add(node.y)
                .ok_or_else(|| malformed("compound MV y coordinate overflows"))?,
            0,
            0,
            context.block_width,
            context.block_height,
            context.frame_block_width,
            context.frame_block_height,
            node.x.saturating_add(block_width_b4) < context.block_width,
            None,
        );
        let stack = find_reference_mvs(tile_state, request)?;
        let mode_symbol = decoder.adaptive_symbol(
            &mut cdfs.inter.compound_mode[compound_mode_context(stack.context)].0,
            7,
        );
        let mode = match mode_symbol {
            0 => InterMode::NearestNearest,
            1 => InterMode::NearNear,
            2 => InterMode::NearestNew,
            3 => InterMode::NewNearest,
            4 => InterMode::NearNew,
            5 => InterMode::NewNear,
            6 => InterMode::GlobalGlobal,
            7 => InterMode::NewNew,
            _ => return Err(malformed("compound inter mode symbol is invalid")),
        };
        let drl_start = match mode {
            InterMode::NearNear | InterMode::NearNew | InterMode::NewNear => Some(1_usize),
            InterMode::NewNew => Some(0_usize),
            _ => None,
        };
        let mut drl_index = 0_usize;
        if let Some(drl_start) = drl_start {
            drl_index = drl_start;
            if stack.len() > drl_start.saturating_add(1) {
                let context = stack
                    .drl_context(drl_start)
                    .ok_or_else(|| malformed("compound DRL context is unavailable"))?;
                if decoder.adaptive_bool(&mut cdfs.inter.drl_bit[usize::from(context)].0) {
                    drl_index = drl_start.saturating_add(1);
                    if stack.len() > drl_index.saturating_add(1) {
                        let context = stack
                            .drl_context(drl_index)
                            .ok_or_else(|| malformed("compound DRL context is unavailable"))?;
                        if decoder.adaptive_bool(&mut cdfs.inter.drl_bit[usize::from(context)].0) {
                            drl_index = drl_index.saturating_add(1);
                        }
                    }
                }
            }
        }
        let nearest = stack
            .slot(0)
            .ok_or_else(|| malformed("compound MV stack has no nearest slot"))?;
        let selected = stack
            .slot(drl_index)
            .ok_or_else(|| malformed("compound MV stack has no selected slot"))?;
        let mut motions = [MotionVector::ZERO; 2];
        if mode == InterMode::GlobalGlobal {
            let first_state = inter_context.reference(references.first);
            let second_state = inter_context.reference(second);
            motions[0] = global_motion_vector(
                first_state.global_motion,
                context.tile_origin_b4_x.saturating_add(node.x),
                context.tile_origin_b4_y.saturating_add(node.y),
                block_width_b4,
                block_height_b4,
                inter_context.force_integer_mv,
                inter_context.high_precision_mv,
            )?;
            motions[1] = global_motion_vector(
                second_state.global_motion,
                context.tile_origin_b4_x.saturating_add(node.x),
                context.tile_origin_b4_y.saturating_add(node.y),
                block_width_b4,
                block_height_b4,
                inter_context.force_integer_mv,
                inter_context.high_precision_mv,
            )?;
            if !matches!(
                first_state.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            ) || !matches!(
                second_state.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            ) {
                return Ok(Err(super::block::PortableUnavailable));
            }
        } else {
            motions[0] = if mode.uses_new_mv(0) {
                let predictor = if matches!(mode, InterMode::NewNearest) {
                    nearest.vectors[0]
                } else {
                    selected.vectors[0]
                };
                let mut value = predictor;
                decode_mv_residual(
                    decoder,
                    &mut cdfs.motion_vectors,
                    &mut value,
                    inter_context.force_integer_mv,
                    inter_context.high_precision_mv,
                )?;
                value
            } else {
                selected.vectors[0]
            };
            motions[1] = if mode.uses_new_mv(1) {
                let predictor = if matches!(mode, InterMode::NearestNew) {
                    nearest.vectors[1]
                } else {
                    selected.vectors[1]
                };
                let mut value = predictor;
                decode_mv_residual(
                    decoder,
                    &mut cdfs.motion_vectors,
                    &mut value,
                    inter_context.force_integer_mv,
                    inter_context.high_precision_mv,
                )?;
                value
            } else {
                selected.vectors[1]
            };
            for (index, motion) in motions.iter_mut().enumerate() {
                if !mode.uses_new_mv(index) {
                    *motion = motion.reduce_precision(
                        inter_context.force_integer_mv,
                        inter_context.high_precision_mv,
                    );
                }
            }
        }
        (
            references,
            motions,
            mode,
            [mode == InterMode::GlobalGlobal; 2],
            [mode.uses_new_mv(0), mode.uses_new_mv(1)],
        )
    } else {
        let reference = if selected_segment.reference > 0 {
            ReferenceFrame::from_segment_feature(selected_segment.reference)
                .ok_or_else(|| malformed("segment reference exceeds seven"))?
        } else if forced_global {
            ReferenceFrame::Last
        } else {
            decode_inter_single_reference(decoder, cdfs, neighbors)?
        };
        let reference_state = inter_context.reference(reference);
        let request = inter_context.mv_request(
            ReferenceMvTarget::Single(reference),
            node.block_size,
            node.x,
            node.y,
            context
                .tile_origin_b4_x
                .checked_add(node.x)
                .ok_or_else(|| malformed("single-reference MV x coordinate overflows"))?,
            context
                .tile_origin_b4_y
                .checked_add(node.y)
                .ok_or_else(|| malformed("single-reference MV y coordinate overflows"))?,
            0,
            0,
            context.block_width,
            context.block_height,
            context.frame_block_width,
            context.frame_block_height,
            node.x.saturating_add(block_width_b4) < context.block_width,
            None,
        );
        let stack = find_reference_mvs(tile_state, request)?;
        let stack_context = stack.context;
        let (mode, drl_index) = if forced_global {
            (InterMode::Global, 0_usize)
        } else if decoder.adaptive_bool(&mut cdfs.inter.new_mv[usize::from(stack_context & 7)].0) {
            if !decoder
                .adaptive_bool(&mut cdfs.inter.global_mv[usize::from((stack_context >> 3) & 1)].0)
            {
                (InterMode::Global, 0)
            } else if decoder.adaptive_bool(
                &mut cdfs.inter.reference_mv[usize::from((stack_context >> 4) & 15)].0,
            ) {
                let mut index = 1;
                if stack.len() > 2 {
                    let first_context = stack
                        .drl_context(1)
                        .ok_or_else(|| malformed("near MV context is unavailable"))?;
                    if decoder.adaptive_bool(&mut cdfs.inter.drl_bit[usize::from(first_context)].0)
                    {
                        index = 2;
                        if stack.len() > 3 {
                            let second_context = stack
                                .drl_context(2)
                                .ok_or_else(|| malformed("near MV context is unavailable"))?;
                            if decoder.adaptive_bool(
                                &mut cdfs.inter.drl_bit[usize::from(second_context)].0,
                            ) {
                                index = 3;
                            }
                        }
                    }
                }
                (InterMode::Near, index)
            } else {
                (InterMode::Nearest, 0)
            }
        } else {
            let mut index = 0;
            if stack.len() > 1 {
                let first_context = stack
                    .drl_context(0)
                    .ok_or_else(|| malformed("new MV context is unavailable"))?;
                if decoder.adaptive_bool(&mut cdfs.inter.drl_bit[usize::from(first_context)].0) {
                    index = 1;
                    if stack.len() > 2 {
                        let second_context = stack
                            .drl_context(1)
                            .ok_or_else(|| malformed("new MV context is unavailable"))?;
                        if decoder
                            .adaptive_bool(&mut cdfs.inter.drl_bit[usize::from(second_context)].0)
                        {
                            index = 2;
                        }
                    }
                }
            }
            (InterMode::New, index)
        };
        let mut motion = match mode {
            InterMode::Global => global_motion_vector(
                reference_state.global_motion,
                context.tile_origin_b4_x.saturating_add(node.x),
                context.tile_origin_b4_y.saturating_add(node.y),
                block_width_b4,
                block_height_b4,
                inter_context.force_integer_mv,
                inter_context.high_precision_mv,
            )?,
            _ => {
                stack
                    .slot(drl_index)
                    .ok_or_else(|| malformed("MV candidate stack has no selected slot"))?
                    .vectors[0]
            }
        };
        if matches!(mode, InterMode::Nearest | InterMode::Near) && drl_index < 2 {
            motion = motion.reduce_precision(
                inter_context.force_integer_mv,
                inter_context.high_precision_mv,
            );
        }
        if matches!(mode, InterMode::New) {
            if stack.len() <= 1 {
                motion = motion.reduce_precision(
                    inter_context.force_integer_mv,
                    inter_context.high_precision_mv,
                );
            }
            decode_mv_residual(
                decoder,
                &mut cdfs.motion_vectors,
                &mut motion,
                inter_context.force_integer_mv,
                inter_context.high_precision_mv,
            )?;
        }
        if matches!(mode, InterMode::Global)
            && !matches!(
                reference_state.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
        {
            return Ok(Err(super::block::PortableUnavailable));
        }
        (
            ReferencePair::single(reference),
            [motion, MotionVector::ZERO],
            mode,
            [mode == InterMode::Global, false],
            [mode == InterMode::New, false],
        )
    };
    let first_state = inter_context.reference(references.first);
    let second_state = references
        .second
        .map(|reference| inter_context.reference(reference));
    let interintra = if compound {
        false
    } else {
        decode_interintra_syntax(
            decoder,
            cdfs,
            inter_context,
            node.block_size,
            false,
            skip_mode,
        )?
    };
    if interintra {
        return Ok(Err(super::block::PortableUnavailable));
    }
    let motion_mode = if compound {
        MotionMode::Translation
    } else if !skip_mode
        && inter_context.motion_mode_switchable
        && block_width_b4.min(block_height_b4) >= 2
        && has_overlappable_neighbor(tile_state, node)?
    {
        // Local-warp sample discovery is not part of this private
        // translation tranche. Refuse the potentially three-symbol sentence
        // while retaining the frame flag, rather than consuming an OBMC CDF
        // for a block whose bitstream may select local warp.
        if inter_context.allow_warped_motion {
            return Ok(Err(super::block::PortableUnavailable));
        }
        let symbol = decoder.adaptive_symbol(&mut cdfs.inter.obmc_for(node.block_size).0, 1);
        match symbol {
            0 => MotionMode::Translation,
            1 => return Ok(Err(super::block::PortableUnavailable)),
            _ => return Err(malformed("OBMC motion-mode symbol is invalid")),
        }
    } else {
        MotionMode::Translation
    };
    let interpolation_needed = match mode {
        InterMode::Global => matches!(
            first_state.global_motion.kind,
            GlobalMotionType::Translation
        ),
        InterMode::GlobalGlobal => second_state.is_some_and(|second| {
            matches!(
                first_state.global_motion.kind,
                GlobalMotionType::Translation
            ) || matches!(second.global_motion.kind, GlobalMotionType::Translation)
        }),
        _ => !skip_mode && motion_mode == MotionMode::Translation,
    };
    let filters = if interpolation_needed && inter_context.interpolation_filter == 4 {
        // AV1 reads vertical first and horizontal second; the motion kernel
        // stores horizontal then vertical.
        let vertical_context =
            switchable_interpolation_context(tile_state, node, references.first, compound, 0)?;
        let vertical = decoder.adaptive_symbol(
            &mut cdfs.inter.interpolation_filter[0][vertical_context].0,
            2,
        );
        let vertical = InterpolationFilter::from_symbol(vertical)
            .ok_or_else(|| malformed("vertical interpolation filter symbol is invalid"))?;
        let horizontal = if inter_context.dual_filter {
            let horizontal_context =
                switchable_interpolation_context(tile_state, node, references.first, compound, 1)?;
            let symbol = decoder.adaptive_symbol(
                &mut cdfs.inter.interpolation_filter[1][horizontal_context].0,
                2,
            );
            InterpolationFilter::from_symbol(symbol)
                .ok_or_else(|| malformed("horizontal interpolation filter symbol is invalid"))?
        } else {
            vertical
        };
        [horizontal, vertical]
    } else if interpolation_needed {
        let filter = InterpolationFilter::from_symbol(inter_context.interpolation_filter.min(3))
            .ok_or_else(|| malformed("fixed interpolation filter is invalid"))?;
        [filter; 2]
    } else {
        [InterpolationFilter::Regular; 2]
    };
    let tx_size = decode_inter_transform_size(
        decoder,
        cdfs,
        node.block_size,
        context.frame_tools.transform_mode,
    )
    .map_err(|_| malformed("variable inter transform is unavailable"))?;
    let quantization = prepared_quantization.quantization;
    let (tx_width, tx_height) = tx_size.pixel_dimensions();
    let mut coefficient_contexts = super::block::InterCoefficientContexts {
        above: [[0x40; 32]; 3],
        left: [[0x40; 32]; 3],
    };
    let luma_context_width = tx_width
        .checked_div(4)
        .ok_or_else(|| malformed("inter luma transform width is not four-aligned"))?;
    let luma_context_height = tx_height
        .checked_div(4)
        .ok_or_else(|| malformed("inter luma transform height is not four-aligned"))?;
    let luma_context_width_usize = usize::try_from(luma_context_width)
        .map_err(|_| malformed("inter luma context width exceeds scratch"))?;
    let luma_context_height_usize = usize::try_from(luma_context_height)
        .map_err(|_| malformed("inter luma context height exceeds scratch"))?;
    (luma_context_width_usize <= coefficient_contexts.above[0].len()
        && luma_context_height_usize <= coefficient_contexts.left[0].len())
    .then_some(())
    .ok_or_else(|| malformed("inter luma context exceeds scratch"))?;
    coefficient_contexts.above[0][..luma_context_width_usize].copy_from_slice(
        &tile_state.luma_contexts_above::<32>(node.x, node.y, luma_context_width)?
            [..luma_context_width_usize],
    );
    coefficient_contexts.left[0][..luma_context_height_usize].copy_from_slice(
        &tile_state.luma_contexts_left::<32>(node.x, node.y, luma_context_height)?
            [..luma_context_height_usize],
    );

    let chroma_tx = if context.monochrome {
        None
    } else {
        let chroma_sampling = super::block::ChromaSampling::from_subsampling(
            context.subsampling_x,
            context.subsampling_y,
        )
        .ok_or_else(|| malformed("inter chroma sampling is invalid"))?;
        let chroma_layout = match chroma_sampling {
            super::block::ChromaSampling::Full => PixelLayout::I444,
            super::block::ChromaSampling::Subsampled420 => PixelLayout::I420,
            super::block::ChromaSampling::Subsampled422 => PixelLayout::I422,
            super::block::ChromaSampling::Monochrome => PixelLayout::Monochrome,
        };
        Some(
            node.block_size
                .maximum_chroma_tx(chroma_layout)
                .ok_or_else(|| malformed("inter chroma transform is unavailable"))?,
        )
    };
    if let Some(chroma_tx) = chroma_tx {
        let (chroma_tx_width, chroma_tx_height) = chroma_tx.pixel_dimensions();
        let chroma_context_width = chroma_tx_width
            .checked_div(4)
            .ok_or_else(|| malformed("inter chroma transform width is not four-aligned"))?;
        let chroma_context_height = chroma_tx_height
            .checked_div(4)
            .ok_or_else(|| malformed("inter chroma transform height is not four-aligned"))?;
        let chroma_context_width_usize = usize::try_from(chroma_context_width)
            .map_err(|_| malformed("inter chroma context width exceeds scratch"))?;
        let chroma_context_height_usize = usize::try_from(chroma_context_height)
            .map_err(|_| malformed("inter chroma context height exceeds scratch"))?;
        (chroma_context_width_usize <= coefficient_contexts.above[1].len()
            && chroma_context_height_usize <= coefficient_contexts.left[1].len())
        .then_some(())
        .ok_or_else(|| malformed("inter chroma context exceeds scratch"))?;
        let chroma_x = if context.subsampling_x {
            node.x / 2
        } else {
            node.x
        };
        let chroma_y = if context.subsampling_y {
            node.y / 2
        } else {
            node.y
        };
        for plane in 0..2 {
            coefficient_contexts.above[plane + 1][..chroma_context_width_usize].copy_from_slice(
                &tile_state.chroma_contexts_above::<32>(
                    plane,
                    chroma_x,
                    chroma_y,
                    chroma_context_width,
                )?[..chroma_context_width_usize],
            );
            coefficient_contexts.left[plane + 1][..chroma_context_height_usize].copy_from_slice(
                &tile_state.chroma_contexts_left::<32>(
                    plane,
                    chroma_x,
                    chroma_y,
                    chroma_context_height,
                )?[..chroma_context_height_usize],
            );
        }
    }

    let luma_block_width = usize::try_from(node.block_size.pixel_dimensions().0)
        .map_err(|_| malformed("inter luma block width exceeds usize"))?;
    let luma_block_height = usize::try_from(node.block_size.pixel_dimensions().1)
        .map_err(|_| malformed("inter luma block height exceeds usize"))?;
    let luma_txb_skipped = block_decoder
        .decode_inter_txb_skip(
            decoder,
            0,
            tx_size,
            luma_block_width,
            luma_block_height,
            &coefficient_contexts.above[0][..luma_context_width_usize],
            &coefficient_contexts.left[0][..luma_context_height_usize],
            block_skipped,
        )
        .map_err(|_| malformed("inter luma coefficient-skip sentence is unavailable"))?;
    let transform = if luma_txb_skipped {
        super::block::Av1TransformType::DctDct
    } else {
        decode_inter_transform_type(
            decoder,
            cdfs,
            tx_size,
            context.frame_tools.reduced_transform_set,
            quantization.segment_lossless,
        )?
    };
    let leaf = match if compound {
        let second = second_state
            .ok_or_else(|| malformed("compound reconstruction omits second reference"))?;
        block_decoder.decode_inter_compound_translation(
            decoder,
            node.block_size,
            visible_width,
            visible_height,
            !context.monochrome,
            prepared_quantization,
            tools,
            [first_state.surface, second.surface],
            [first_state.scale, second.scale],
            context.tile_origin_b4_x.saturating_add(node.x),
            context.tile_origin_b4_y.saturating_add(node.y),
            motions,
            filters,
            block_skipped,
            luma_txb_skipped,
            tx_size,
            transform,
            coefficient_contexts,
        )
    } else {
        block_decoder.decode_inter_translation(
            decoder,
            node.block_size,
            visible_width,
            visible_height,
            !context.monochrome,
            prepared_quantization,
            tools,
            first_state.surface,
            first_state.scale,
            context.tile_origin_b4_x.saturating_add(node.x),
            context.tile_origin_b4_y.saturating_add(node.y),
            motions[0],
            filters,
            block_skipped,
            luma_txb_skipped,
            tx_size,
            transform,
            coefficient_contexts,
        )
    } {
        Ok(leaf) => leaf,
        Err(_) => return Ok(Err(super::block::PortableUnavailable)),
    };
    Ok(Ok(DecodedInterLeaf {
        leaf,
        metadata: super::tile_state::InterBlockMeta {
            references,
            motion_vectors: motions,
            mode,
            compound_type: CompoundType::Average,
            motion_mode,
            filters,
            global,
            new_mv,
        },
    }))
}

#[derive(Clone, Copy)]
enum LoopFilterClass {
    Intra,
    Inter {
        reference: ReferenceFrame,
        mode: InterMode,
    },
}

fn effective_loop_levels(
    context: &FirstBlockContext,
    segment: SegmentContext,
    dynamic_delta_lf: [i32; 4],
    class: LoopFilterClass,
) -> [u8; 4] {
    let frame = context.frame_tools.loop_filter;
    let bases = [
        frame.level_y[0],
        frame.level_y[1],
        frame.level_u,
        frame.level_v,
    ];
    let luma_disabled = frame.level_y == [0; 2];
    let mut levels = [0_u8; 4];
    for (index, level) in levels.iter_mut().enumerate() {
        let header_disabled = (index >= 2 && bases[index] == 0) || (index < 2 && luma_disabled);
        if header_disabled {
            continue;
        }
        let dynamic_base = i64::from(bases[index])
            .saturating_add(i64::from(dynamic_delta_lf[index]))
            .clamp(0, 63);
        let base = dynamic_base
            .saturating_add(i64::from(segment.delta_lf[index]))
            .clamp(0, 63);
        let adjusted = if frame.delta_enabled {
            let scale = if base >= 32 { 2_i64 } else { 1 };
            let delta = match class {
                LoopFilterClass::Intra => i64::from(frame.reference_deltas[0]),
                LoopFilterClass::Inter { reference, mode } => {
                    let reference_delta = frame
                        .reference_deltas
                        .get(reference.index().saturating_add(1))
                        .copied()
                        .unwrap_or_default();
                    let mode_index =
                        usize::from(!matches!(mode, InterMode::Global | InterMode::GlobalGlobal));
                    let mode_delta = frame
                        .mode_deltas
                        .get(mode_index)
                        .copied()
                        .unwrap_or_default();
                    i64::from(reference_delta).saturating_add(i64::from(mode_delta))
                }
            };
            base.saturating_add(delta.saturating_mul(scale))
                .clamp(0, 63)
        } else {
            base
        };
        *level = u8::try_from(adjusted).unwrap_or_default();
    }
    levels
}

impl Lossy420Reconstruction {
    /// Apply this tile's filters in isolation for the single-tile path.
    pub(super) fn into_filtered_leaf(self) -> Av1Result<super::block::FirstLeaf> {
        let Lossy420Reconstruction {
            mut leaf,
            monochrome,
            subsampling_x,
            subsampling_y,
            filter_blocks,
            cdef_indices,
            cdef_active,
            loop_parameters,
            cdef_parameters,
            restoration: _,
            cdfs: _,
        } = self;
        if monochrome {
            return Err(malformed(
                "monochrome reconstruction cannot enter the color filter path",
            ));
        }
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

    /// Extract a complete monochrome plane from the shared walker result.
    ///
    /// The plane is moved out of the private three-carrier leaf. The caller
    /// applies any admitted frame-level CDEF/restoration stages before
    /// publishing the resulting surface.
    pub(super) fn into_monochrome_plane(self) -> Av1Result<super::block::ReconstructedPlane> {
        let Lossy420Reconstruction {
            leaf, monochrome, ..
        } = self;
        if !monochrome {
            return Err(malformed(
                "color reconstruction cannot enter the monochrome surface path",
            ));
        }
        let [plane, _, _] = leaf.planes;
        Ok(plane)
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
    input_cdfs: &FrameCdfs,
    mut current_segment_map: Option<&mut SegmentMap>,
    previous_segment_map: Option<&SegmentMap>,
    inter_context: Option<&InterFrameContext<'_>>,
) -> Av1Result<Option<Lossy420Reconstruction>> {
    let bounded_intra_restoration =
        complete_bounded_restoration_intra_420_reconstruction_context(context);
    let bounded_inter_restoration =
        complete_bounded_restoration_inter_420_reconstruction_context(context);
    let monochrome_intra_reconstruction =
        complete_monochrome_lossy_intra_reconstruction_context(context);
    let monochrome_postfilter = complete_monochrome_postfilter_reconstruction_context(context);
    let bounded_i444_inter_geometry = inter_context.and_then(|inter_context| {
        bounded_i444_inter_reconstruction_geometry(context, inter_context)
    });
    let bounded_i444_inter = bounded_i444_inter_geometry.is_some();
    let bounded_i444_restoration = inter_context.is_some_and(|inter_context| {
        complete_bounded_i444_restoration_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i422_intra_restoration =
        complete_bounded_i422_restoration_intra_reconstruction_context(context);
    let bounded_i422_inter_restoration = inter_context.is_some_and(|inter_context| {
        complete_bounded_i422_restoration_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i422_restoration = bounded_i422_intra_restoration || bounded_i422_inter_restoration;
    let bounded_i420_intra_restoration =
        complete_bounded_i420_restoration_intra_reconstruction_context(context);
    let bounded_i420_inter_restoration = inter_context.is_some_and(|inter_context| {
        complete_bounded_i420_restoration_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i420_restoration = bounded_i420_intra_restoration || bounded_i420_inter_restoration;
    let bounded_i420_intra_cdef = complete_bounded_i420_cdef_intra_reconstruction_context(context);
    let bounded_i420_inter_cdef = inter_context.is_some_and(|inter_context| {
        complete_bounded_i420_cdef_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i420_cdef = bounded_i420_intra_cdef || bounded_i420_inter_cdef;
    let bounded_i422_intra_cdef = complete_bounded_i422_cdef_intra_reconstruction_context(context);
    let bounded_i422_inter_cdef = inter_context.is_some_and(|inter_context| {
        complete_bounded_i422_cdef_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i422_cdef = bounded_i422_intra_cdef || bounded_i422_inter_cdef;
    let bounded_i420_intra_cdef_restoration =
        complete_bounded_i420_cdef_restoration_intra_reconstruction_context(context);
    let bounded_i420_inter_cdef_restoration = inter_context.is_some_and(|inter_context| {
        complete_bounded_i420_cdef_restoration_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i420_cdef_restoration =
        bounded_i420_intra_cdef_restoration || bounded_i420_inter_cdef_restoration;
    let bounded_i422_intra_cdef_restoration =
        complete_bounded_i422_cdef_restoration_intra_reconstruction_context(context);
    let bounded_i422_inter_cdef_restoration = inter_context.is_some_and(|inter_context| {
        complete_bounded_i422_cdef_restoration_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i422_cdef_restoration =
        bounded_i422_intra_cdef_restoration || bounded_i422_inter_cdef_restoration;
    let bounded_i420_intra_loop_postfilters =
        complete_bounded_i420_loop_postfilters_intra_reconstruction_context(context);
    let bounded_i420_inter_loop_postfilters = inter_context.and_then(|inter_context| {
        complete_bounded_i420_loop_postfilters_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i420_loop_postfilters = bounded_i420_intra_loop_postfilters
        .or(bounded_i420_inter_loop_postfilters)
        .is_some();
    let bounded_i420_loop_postfilter_restoration = bounded_i420_intra_loop_postfilters
        .is_some_and(|profile| profile.restoration)
        || bounded_i420_inter_loop_postfilters.is_some_and(|profile| profile.restoration);
    let bounded_i422_intra_loop_postfilters =
        complete_bounded_i422_loop_postfilters_intra_reconstruction_context(context);
    let bounded_i422_inter_loop_postfilters = inter_context.and_then(|inter_context| {
        complete_bounded_i422_loop_postfilters_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i422_loop_postfilters = bounded_i422_intra_loop_postfilters
        .or(bounded_i422_inter_loop_postfilters)
        .is_some();
    let bounded_i422_loop_postfilter_restoration = bounded_i422_intra_loop_postfilters
        .is_some_and(|profile| profile.restoration)
        || bounded_i422_inter_loop_postfilters.is_some_and(|profile| profile.restoration);
    let bounded_i422_intra_rect_loop_geometry =
        complete_bounded_i422_rect_loop_intra_reconstruction_context(context);
    let bounded_i422_inter_rect_loop_geometry = inter_context.and_then(|inter_context| {
        complete_bounded_i422_rect_loop_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i422_rect_loop_geometry =
        bounded_i422_intra_rect_loop_geometry.or(bounded_i422_inter_rect_loop_geometry);
    let bounded_i422_rect_loop = bounded_i422_rect_loop_geometry.is_some();
    let bounded_i422_intra_rect_cdef_geometry =
        complete_bounded_i422_rect_cdef_intra_reconstruction_context(context);
    let bounded_i422_inter_rect_cdef_geometry = inter_context.and_then(|inter_context| {
        complete_bounded_i422_rect_cdef_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i422_rect_cdef_geometry =
        bounded_i422_intra_rect_cdef_geometry.or(bounded_i422_inter_rect_cdef_geometry);
    let bounded_i422_rect_cdef = bounded_i422_rect_cdef_geometry.is_some();
    let bounded_i422_intra_rect_restoration_geometry =
        complete_bounded_i422_rect_restoration_intra_reconstruction_context(context);
    let bounded_i422_inter_rect_restoration_geometry = inter_context.and_then(|inter_context| {
        complete_bounded_i422_rect_restoration_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i422_rect_restoration_geometry = bounded_i422_intra_rect_restoration_geometry
        .or(bounded_i422_inter_rect_restoration_geometry);
    let bounded_i422_rect_restoration = bounded_i422_rect_restoration_geometry.is_some();
    let bounded_i422_rect_geometry = bounded_i422_rect_loop_geometry
        .or(bounded_i422_rect_cdef_geometry)
        .or(bounded_i422_rect_restoration_geometry);
    let bounded_i420_intra_rect_loop_geometry =
        complete_bounded_i420_rect_loop_intra_reconstruction_context(context);
    let bounded_i420_inter_rect_loop_geometry = inter_context.and_then(|inter_context| {
        complete_bounded_i420_rect_loop_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i420_rect_loop_geometry =
        bounded_i420_intra_rect_loop_geometry.or(bounded_i420_inter_rect_loop_geometry);
    let bounded_i420_rect_loop = bounded_i420_rect_loop_geometry.is_some();
    let bounded_i420_intra_rect_cdef_geometry =
        complete_bounded_i420_rect_cdef_intra_reconstruction_context(context);
    let bounded_i420_inter_rect_cdef_geometry = inter_context.and_then(|inter_context| {
        complete_bounded_i420_rect_cdef_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i420_rect_cdef_geometry =
        bounded_i420_intra_rect_cdef_geometry.or(bounded_i420_inter_rect_cdef_geometry);
    let bounded_i420_rect_cdef = bounded_i420_rect_cdef_geometry.is_some();
    let bounded_i420_intra_rect_restoration_geometry =
        complete_bounded_i420_rect_restoration_intra_reconstruction_context(context);
    let bounded_i420_inter_rect_restoration_geometry = inter_context.and_then(|inter_context| {
        complete_bounded_i420_rect_restoration_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i420_rect_restoration_geometry = bounded_i420_intra_rect_restoration_geometry
        .or(bounded_i420_inter_rect_restoration_geometry);
    let bounded_i420_rect_restoration = bounded_i420_rect_restoration_geometry.is_some();
    let bounded_i420_intra_rect_cdef_restoration_geometry =
        complete_bounded_i420_rect_cdef_restoration_intra_reconstruction_context(context);
    let bounded_i420_inter_rect_cdef_restoration_geometry =
        inter_context.and_then(|inter_context| {
            complete_bounded_i420_rect_cdef_restoration_inter_reconstruction_context(
                context,
                inter_context,
            )
        });
    let bounded_i420_rect_cdef_restoration_geometry =
        bounded_i420_intra_rect_cdef_restoration_geometry
            .or(bounded_i420_inter_rect_cdef_restoration_geometry);
    let bounded_i420_rect_cdef_restoration = bounded_i420_rect_cdef_restoration_geometry.is_some();
    let bounded_i422_intra_rect_cdef_restoration_geometry =
        complete_bounded_i422_rect_cdef_restoration_intra_reconstruction_context(context);
    let bounded_i422_inter_rect_cdef_restoration_geometry =
        inter_context.and_then(|inter_context| {
            complete_bounded_i422_rect_cdef_restoration_inter_reconstruction_context(
                context,
                inter_context,
            )
        });
    let bounded_i422_rect_cdef_restoration_geometry =
        bounded_i422_intra_rect_cdef_restoration_geometry
            .or(bounded_i422_inter_rect_cdef_restoration_geometry);
    let bounded_i422_rect_cdef_restoration = bounded_i422_rect_cdef_restoration_geometry.is_some();
    let bounded_i420_intra_rect_loop_postfilters =
        complete_bounded_i420_rect_loop_postfilters_intra_reconstruction_context(context);
    let bounded_i420_inter_rect_loop_postfilters = inter_context.and_then(|inter_context| {
        complete_bounded_i420_rect_loop_postfilters_inter_reconstruction_context(
            context,
            inter_context,
        )
    });
    let bounded_i420_rect_loop_postfilters =
        bounded_i420_intra_rect_loop_postfilters.or(bounded_i420_inter_rect_loop_postfilters);
    let bounded_i420_rect_loop_postfilters_restoration =
        bounded_i420_rect_loop_postfilters.is_some_and(|profile| profile.restoration);
    let bounded_i422_intra_rect_loop_postfilters =
        complete_bounded_i422_rect_loop_postfilters_intra_reconstruction_context(context);
    let bounded_i422_inter_rect_loop_postfilters = inter_context.and_then(|inter_context| {
        complete_bounded_i422_rect_loop_postfilters_inter_reconstruction_context(
            context,
            inter_context,
        )
    });
    let bounded_i422_rect_loop_postfilters =
        bounded_i422_intra_rect_loop_postfilters.or(bounded_i422_inter_rect_loop_postfilters);
    let bounded_i422_rect_loop_postfilters_restoration =
        bounded_i422_rect_loop_postfilters.is_some_and(|profile| profile.restoration);
    let bounded_i420_rect_geometry = bounded_i420_rect_loop_geometry
        .or(bounded_i420_rect_cdef_geometry)
        .or(bounded_i420_rect_restoration_geometry)
        .or(bounded_i420_rect_cdef_restoration_geometry)
        .or(bounded_i420_rect_loop_postfilters.map(|profile| profile.geometry));
    let bounded_i420_rect = bounded_i420_rect_geometry.is_some();
    let bounded_i422_rect_geometry = bounded_i422_rect_geometry
        .or(bounded_i422_rect_cdef_restoration_geometry)
        .or(bounded_i422_rect_loop_postfilters.map(|profile| profile.geometry));
    let bounded_subsampled_rect_geometry =
        bounded_i420_rect_geometry.or(bounded_i422_rect_geometry);
    let bounded_subsampled_rect = bounded_subsampled_rect_geometry.is_some();
    let bounded_i420_intra_loop = complete_bounded_i420_loop_intra_reconstruction_context(context);
    let bounded_i420_inter_loop = inter_context.is_some_and(|inter_context| {
        complete_bounded_i420_loop_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i420_loop = bounded_i420_intra_loop || bounded_i420_inter_loop;
    let bounded_i422_intra_loop = complete_bounded_i422_loop_intra_reconstruction_context(context);
    let bounded_i422_inter_loop = inter_context.is_some_and(|inter_context| {
        complete_bounded_i422_loop_inter_reconstruction_context(context, inter_context)
    });
    let bounded_i422_loop = bounded_i422_intra_loop || bounded_i422_inter_loop;
    let bounded_i444_intra_restoration_geometry = bounded_i444_intra_restoration_geometry(context);
    let bounded_i444_intra_restoration = bounded_i444_intra_restoration_geometry.is_some();
    let bounded_i444_intra_cdef_geometry = bounded_i444_intra_cdef_geometry(context);
    let bounded_i444_intra_cdef = bounded_i444_intra_cdef_geometry.is_some();
    let bounded_i444_intra_loop_geometry = bounded_i444_intra_loop_geometry(context);
    let bounded_i444_intra_loop = bounded_i444_intra_loop_geometry.is_some();
    let bounded_i444_geometry = if bounded_i444_inter {
        bounded_i444_inter_geometry
    } else if bounded_i444_restoration {
        bounded_i444_geometry_for_context(context)
    } else if bounded_i444_intra_restoration {
        bounded_i444_intra_restoration_geometry
    } else if bounded_i444_intra_cdef {
        bounded_i444_intra_cdef_geometry
    } else if bounded_i444_intra_loop {
        bounded_i444_intra_loop_geometry
    } else {
        None
    };
    let inter_reconstruction = inter_context.is_some_and(|inter_context| {
        complete_inter_420_reconstruction_context(context)
            || complete_high_depth_inter_reconstruction_context(context, inter_context)
            || bounded_i444_inter
            || complete_monochrome_lossy_inter_reconstruction_context(context, inter_context)
            || (!context.intra_frame
                && monochrome_postfilter
                && complete_monochrome_references(context, inter_context))
            || bounded_inter_restoration
            || bounded_i444_restoration
            || bounded_i422_inter_restoration
            || bounded_i420_inter_restoration
            || bounded_i420_inter_cdef
            || bounded_i422_inter_cdef
            || bounded_i420_inter_cdef_restoration
            || bounded_i422_inter_cdef_restoration
            || bounded_i420_loop_postfilters
            || bounded_i422_loop_postfilters
            || bounded_i420_rect_loop
            || bounded_i420_rect_cdef
            || bounded_i420_rect_restoration
            || bounded_i420_rect_cdef_restoration
            || bounded_i420_rect_loop_postfilters.is_some()
            || bounded_i422_rect_loop
            || bounded_i422_rect_cdef
            || bounded_i422_rect_restoration
            || bounded_i422_rect_cdef_restoration
            || bounded_i422_rect_loop_postfilters.is_some()
            || bounded_i420_inter_loop
            || bounded_i422_inter_loop
    });
    let intra_reconstruction = complete_lossy_420_reconstruction_context(context)
        || complete_superres_lossy_420_reconstruction_context(context)
        || monochrome_intra_reconstruction
        || (context.intra_frame && monochrome_postfilter)
        || bounded_intra_restoration
        || bounded_i444_intra_restoration
        || bounded_i422_intra_restoration
        || bounded_i420_intra_restoration
        || bounded_i420_intra_cdef
        || bounded_i422_intra_cdef
        || bounded_i420_intra_cdef_restoration
        || bounded_i422_intra_cdef_restoration
        || bounded_i420_loop_postfilters
        || bounded_i422_loop_postfilters
        || bounded_i420_rect_loop
        || bounded_i420_rect_cdef
        || bounded_i420_rect_restoration
        || bounded_i420_rect_cdef_restoration
        || bounded_i420_rect_loop_postfilters.is_some()
        || bounded_i422_rect_loop
        || bounded_i422_rect_cdef
        || bounded_i422_rect_restoration
        || bounded_i422_rect_cdef_restoration
        || bounded_i422_rect_loop_postfilters.is_some()
        || bounded_i420_intra_loop
        || bounded_i422_intra_loop
        || bounded_i444_intra_cdef
        || bounded_i444_intra_loop;
    let segmentation = context.frame_tools.segmentation;
    let intra_segment_features_supported = !segmentation.enabled
        || segmentation
            .segments
            .iter()
            .all(|segment| segment.reference <= 0 && !segment.global_motion);
    if (!intra_reconstruction
        && !complete_streamed_lossless_color_context(context)
        && !inter_reconstruction)
        || (context.intra_frame && !intra_segment_features_supported)
    {
        return Ok(None);
    }
    let chroma_sampling = if context.monochrome {
        super::block::ChromaSampling::Monochrome
    } else {
        super::block::ChromaSampling::from_subsampling(context.subsampling_x, context.subsampling_y)
            .ok_or_else(|| malformed("unsupported AV1 chroma subsampling"))?
    };
    let mut decoder = RangeDecoder::new(data, range.start, range.end, context.disable_cdf_update)?;
    let mut tile_cdfs = input_cdfs.clone();
    if context.frame_tools.segmentation.enabled {
        let current = current_segment_map
            .as_deref()
            .ok_or_else(|| malformed("enabled segmentation omits current map"))?;
        if !current.compatible_with(context.frame_block_width, context.frame_block_height) {
            return Err(malformed("current segment map has incompatible dimensions"));
        }
    }
    #[cfg(coverage)]
    decoder.enable_operation_trace();
    let restoration_plan = if bounded_intra_restoration
        || bounded_inter_restoration
        || bounded_i444_restoration
        || bounded_i444_intra_restoration
        || bounded_i422_restoration
        || bounded_i420_restoration
        || bounded_i420_cdef_restoration
        || bounded_i422_cdef_restoration
        || bounded_i420_loop_postfilter_restoration
        || bounded_i422_loop_postfilter_restoration
        || bounded_i420_rect_restoration
        || bounded_i420_rect_cdef_restoration
        || bounded_i420_rect_loop_postfilters_restoration
        || bounded_i422_rect_restoration
        || bounded_i422_rect_cdef_restoration
        || bounded_i422_rect_loop_postfilters_restoration
        || (monochrome_postfilter && context.restoration_types[0].is_some())
    {
        let Some(plan) =
            decode_bounded_restoration_plan(&mut decoder, context, &mut tile_cdfs.restoration)
        else {
            return Ok(None);
        };
        Some(plan)
    } else {
        if !decode_restoration_prefix_with_cdfs(&mut decoder, context, &mut tile_cdfs.restoration) {
            return Ok(None);
        }
        None
    };

    let quantization = lossy_quantization_for_context(context)?;
    let tools = super::block::BlockTools {
        sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
            .ok_or_else(|| malformed("AV1 block sample depth is unsupported"))?,
        allow_screen_content_tools: context.allow_screen_content_tools,
        enable_filter_intra: context.enable_filter_intra,
        enable_intra_edge_filter: context.enable_intra_edge_filter,
        transform_mode: context.frame_tools.transform_mode,
        transform_context: 0,
        skip_context: 0,
        suppress_delta_q_when_skipped: false,
        palette_context: Default::default(),
    };
    let root_level = context.level;
    let root_size = 32_u32
        .checked_shr(root_level)
        .filter(|&size| size != 0)
        .ok_or_else(|| malformed("superblock root size is invalid"))?;
    let root_step =
        usize::try_from(root_size).map_err(|_| malformed("superblock root size exceeds usize"))?;
    let mut walker = PartitionWalker::with_cdfs(&mut decoder, context, tile_cdfs.partition)?;
    let Some(mut block_decoder) = super::block::Lossy420Decoder::with_cdf_state(
        quantization.qindex,
        chroma_sampling,
        &tile_cdfs.block,
    ) else {
        return Ok(None);
    };
    block_decoder.configure_delta_lf(
        context.frame_tools.delta_lf_present,
        context.frame_tools.delta_lf_resolution_log2,
        context.frame_tools.delta_lf_multi,
        context.monochrome,
    );
    let padded_width = context
        .block_width
        .checked_mul(4)
        .ok_or_else(|| malformed("padded tile width overflows pixels"))?;
    let padded_height = context
        .block_height
        .checked_mul(4)
        .ok_or_else(|| malformed("padded tile height overflows pixels"))?;
    let mut canvas = super::raster::FrameCanvas::new_padded(
        context.frame_width,
        context.frame_height,
        padded_width,
        padded_height,
        context.subsampling_x,
        context.subsampling_y,
    )?;
    let mut tile_state = TileState::new(
        context.block_width,
        context.block_height,
        context.subsampling_x,
        context.subsampling_y,
    )?;
    let collect_loop_filter = context.frame_tools.loop_filter.level_y != [0; 2]
        || context.frame_tools.loop_filter.level_u != 0
        || context.frame_tools.loop_filter.level_v != 0;
    let collect_cdef = context.frame_tools.cdef.is_some();
    let mut filter_blocks = Vec::<super::filter::Block>::new();
    let bounded_i444_filter_reserve = bounded_i444_geometry
        .filter(|_| collect_loop_filter)
        .map(|geometry| geometry.expected_leaf_count())
        .unwrap_or(0);
    let bounded_subsampled_filter_reserve = [
        bounded_i422_rect_loop_geometry,
        bounded_i420_rect_loop_geometry,
        bounded_i420_rect_loop_postfilters.map(|profile| profile.geometry),
        bounded_i422_rect_loop_postfilters.map(|profile| profile.geometry),
    ]
    .into_iter()
    .flatten()
    .map(BoundedSubsampledRectGeometry::expected_leaf_count)
    .max()
    .unwrap_or(0);
    let filter_reserve = bounded_i444_filter_reserve.max(bounded_subsampled_filter_reserve);
    if filter_reserve != 0 {
        filter_blocks
            .try_reserve_exact(filter_reserve)
            .map_err(|_| {
                CodecError::Dimensions("unable to allocate AV1 loop-filter metadata".to_owned())
            })?;
    }
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
    let mut cdef_indices = if collect_cdef {
        let count = cdef_region_width
            .checked_mul(cdef_region_height)
            .ok_or_else(|| malformed("CDEF region map allocation overflows"))?;
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| {
            CodecError::Dimensions("unable to allocate AV1 CDEF region map".to_owned())
        })?;
        values.resize(count, None);
        values
    } else {
        Vec::new()
    };
    let mut cdef_active = if collect_cdef {
        let count = cdef_active_width
            .checked_mul(cdef_active_height)
            .ok_or_else(|| malformed("CDEF active map allocation overflows"))?;
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| {
            CodecError::Dimensions("unable to allocate AV1 CDEF active map".to_owned())
        })?;
        values.resize(count, false);
        values
    } else {
        Vec::new()
    };
    let mut segment_updates = Vec::<SegmentMapUpdate>::new();
    let mut unsupported = false;

    for root_y in (0..context.block_height).step_by(root_step) {
        for root_x in (0..context.block_width).step_by(root_step) {
            let mut bounded_i444_leaf_count = 0usize;
            let mut bounded_subsampled_rect_leaf_count = 0usize;
            let delta_q_at_root = context.frame_tools.delta_q_present;
            block_decoder.begin_superblock(
                context.frame_tools.cdef.map_or(0, |cdef| cdef.bits),
                delta_q_at_root,
                root_x,
                root_y,
                root_size,
            );
            walker.reset_root();
            walker.set_root_bounds(root_x, root_y, root_size)?;
            let control = walker.walk(root_level, root_x, root_y, &mut |decoder, node| {
                let width = node.width.saturating_mul(4);
                let height = node.height.saturating_mul(4);
                // A standalone 4x4 image is cropped from one coded 8x8
                // block. All other leaves retain their nominal pre-clipping
                // partition dimensions for block and transform syntax.
                let syntax_block_size = if context.frame_width == 4
                    && context.frame_height == 4
                    && width == 4
                    && height == 4
                {
                    BlockSize::B8x8
                } else {
                    node.block_size
                };
                if complete_high_depth_422_intra_reconstruction_context(context)
                    || bounded_i444_inter
                    || bounded_i444_restoration
                    || bounded_i444_intra_restoration
                    || bounded_i422_restoration
                    || bounded_i420_restoration
                    || bounded_i420_cdef
                    || bounded_i422_cdef
                    || bounded_i420_cdef_restoration
                    || bounded_i422_cdef_restoration
                    || bounded_i420_loop_postfilters
                    || bounded_i422_loop_postfilters
                    || bounded_i420_rect
                    || bounded_subsampled_rect
                    || bounded_i444_intra_cdef
                    || bounded_i444_intra_loop
                    || bounded_i420_loop
                    || bounded_i422_loop
                {
                    if let Some(geometry) = bounded_subsampled_rect_geometry {
                        if !bounded_subsampled_expected_rect_terminal(
                            geometry,
                            bounded_subsampled_rect_leaf_count,
                            node,
                        ) {
                            unsupported = true;
                            return Ok(PartitionVisitControl::Stop);
                        }
                        bounded_subsampled_rect_leaf_count = bounded_subsampled_rect_leaf_count
                            .checked_add(1)
                            .ok_or_else(|| {
                                malformed("bounded subsampled rectangle leaf count overflows")
                            })?;
                    }
                    if let Some(geometry) = bounded_i444_geometry {
                        if !bounded_i444_expected_terminal(geometry, bounded_i444_leaf_count, node)
                        {
                            // The bounded full-resolution inter profiles admit
                            // only the exact normalized terminal sequence. A
                            // clipped B32x16, a deeper split, or a misplaced
                            // sibling is rejected before skip/CDEF/
                            // quantization syntax can mutate state.
                            unsupported = true;
                            return Ok(PartitionVisitControl::Stop);
                        }
                        bounded_i444_leaf_count = bounded_i444_leaf_count
                            .checked_add(1)
                            .ok_or_else(|| malformed("bounded I444 leaf count overflows"))?;
                    }
                    if syntax_block_size != BlockSize::B16x16 {
                        // These bounded inter/intra tranches are proved only
                        // for one normalized B16x16/Square16 terminal. Reject
                        // a partition before any block skip, quantization, or
                        // coefficient CDF mutates.
                        unsupported = true;
                        return Ok(PartitionVisitControl::Stop);
                    }
                }
                let streamed_large = super::block::uses_streamed_intra(syntax_block_size);
                let transform_grid = if streamed_large {
                    None
                } else {
                    let Ok(transform_grid) =
                        super::block::TransformGrid::from_block_size(syntax_block_size)
                    else {
                        unsupported = true;
                        return Ok(PartitionVisitControl::Stop);
                    };
                    Some(transform_grid)
                };
                let mut tools = tools;
                tools.suppress_delta_q_when_skipped = delta_q_at_root
                    && node.x == root_x
                    && node.y == root_y
                    && node.coded_width == root_size
                    && node.coded_height == root_size;
                tools.palette_context =
                    super::block::PaletteNeighborContext::from_neighbors(node.y, None, None);
                let legacy_full_large = !streamed_large
                    && !context.subsampling_x
                    && !context.subsampling_y
                    && matches!(
                        (node.coded_width, node.coded_height),
                        (4, 16) | (8, 4) | (16, 4) | (16, 16)
                    );
                if legacy_full_large {
                    let fully_visible = node
                        .x
                        .checked_add(node.coded_width)
                        .and_then(|right| right.checked_mul(4))
                        .is_some_and(|right| right <= context.frame_width)
                        && node
                            .y
                            .checked_add(node.coded_height)
                            .and_then(|bottom| bottom.checked_mul(4))
                            .is_some_and(|bottom| bottom <= context.frame_height);
                    if !fully_visible {
                        unsupported = true;
                        return Ok(PartitionVisitControl::Stop);
                    }
                }
                let (coded_mi_width, coded_mi_height) = syntax_block_size.mi_dimensions();
                let (palette_coded_width, palette_coded_height) =
                    syntax_block_size.pixel_dimensions();
                let palette_entropy_width = palette_coded_width
                    .min(context.block_width.saturating_sub(node.x).saturating_mul(4));
                let palette_entropy_height = palette_coded_height.min(
                    context
                        .block_height
                        .saturating_sub(node.y)
                        .saturating_mul(4),
                );
                tools.skip_context = skip_context_for_node(&tile_state, node)?;
                let segmentation = context.frame_tools.segmentation;
                let (segment_id, segment_pred, selected_segment, block_skipped) = if !segmentation
                    .update_map
                    || segmentation.preskip
                {
                    let (segment_id, segment_pred) = decode_segment_id(
                        decoder,
                        &mut tile_cdfs,
                        context,
                        node,
                        &tile_state,
                        previous_segment_map,
                        None,
                    )?;
                    let segment = segmentation.segments[usize::from(segment_id)];
                    block_decoder.select_segment(segment.delta_q, segment.qindex, segment.lossless);
                    let block_skipped = match block_decoder.decode_skip(
                        decoder,
                        tools.skip_context,
                        segment.skip,
                    ) {
                        Ok(skip) => skip,
                        Err(_) => {
                            unsupported = true;
                            return Ok(PartitionVisitControl::Stop);
                        }
                    };
                    (segment_id, segment_pred, segment, block_skipped)
                } else {
                    let skip = match block_decoder.decode_skip(decoder, tools.skip_context, false) {
                        Ok(skip) => skip,
                        Err(_) => {
                            unsupported = true;
                            return Ok(PartitionVisitControl::Stop);
                        }
                    };
                    let (segment_id, segment_pred) = decode_segment_id(
                        decoder,
                        &mut tile_cdfs,
                        context,
                        node,
                        &tile_state,
                        previous_segment_map,
                        Some(skip),
                    )?;
                    let segment = segmentation.segments[usize::from(segment_id)];
                    block_decoder.select_segment(segment.delta_q, segment.qindex, segment.lossless);
                    (segment_id, segment_pred, segment, skip)
                };
                block_decoder.begin_block(
                    syntax_block_size,
                    palette_entropy_width,
                    palette_entropy_height,
                    node.x,
                    node.y,
                );
                // The value is consulted only in the `!streamed_large`
                // branch, where construction above proved `Some`. Keeping a
                // concrete copy avoids a panic-only unwrap in codec code.
                let legacy_transform_grid =
                    transform_grid.unwrap_or(super::block::TransformGrid::Square4);
                let mut inter_metadata = None;
                let decoded = if !context.intra_frame {
                    let Some(inter_context) = inter_context else {
                        unsupported = true;
                        return Ok(PartitionVisitControl::Stop);
                    };
                    if context.frame_tools.cdef.is_some()
                        && block_decoder
                            .decode_inter_cdef(decoder, block_skipped)
                            .is_err()
                    {
                        unsupported = true;
                        return Ok(PartitionVisitControl::Stop);
                    }
                    let prepared_quantization = match block_decoder.decode_inter_quantization(
                        decoder,
                        quantization,
                        block_skipped,
                    ) {
                        Ok(prepared) => prepared,
                        Err(_) => {
                            unsupported = true;
                            return Ok(PartitionVisitControl::Stop);
                        }
                    };
                    match decode_inter_leaf(
                        decoder,
                        &mut tile_cdfs,
                        &mut block_decoder,
                        &tile_state,
                        context,
                        inter_context,
                        node,
                        width,
                        height,
                        selected_segment,
                        block_skipped,
                        prepared_quantization,
                        tools,
                    )? {
                        Ok(inter) => {
                            inter_metadata = Some(inter.metadata);
                            Ok(inter.leaf)
                        }
                        Err(error) => Err(error),
                    }
                } else if tile_state.is_empty() {
                    let standalone_tiny_frame =
                        context.frame_width == 4 && context.frame_height == 4;
                    let has_chroma =
                        partition_node_has_chroma(context, node, standalone_tiny_frame);
                    if streamed_large {
                        block_decoder.decode_large_intra(
                            decoder,
                            syntax_block_size,
                            width,
                            height,
                            has_chroma,
                            quantization,
                            tools,
                            super::block::LargeIntraSpatial::Origin,
                        )
                    } else if has_chroma {
                        if !context.subsampling_x && !context.subsampling_y {
                            block_decoder.decode_origin_full(
                                decoder,
                                width,
                                height,
                                legacy_transform_grid,
                                quantization,
                                tools,
                            )
                        } else {
                            block_decoder.decode_origin_with_grid(
                                decoder,
                                width,
                                height,
                                legacy_transform_grid,
                                quantization,
                                tools,
                            )
                        }
                    } else {
                        block_decoder.decode_origin_without_chroma(
                            decoder,
                            width,
                            height,
                            legacy_transform_grid,
                            quantization,
                            tools,
                        )
                    }
                } else {
                    decode_complete_following_leaf(
                        decoder,
                        &mut block_decoder,
                        &tile_state,
                        &canvas,
                        context,
                        node,
                        width,
                        height,
                        coded_mi_width,
                        coded_mi_height,
                        quantization,
                        tools,
                    )?
                };
                let decoded = match decoded {
                    Ok(decoded) => decoded,
                    Err(_) => {
                        unsupported = true;
                        return Ok(PartitionVisitControl::Stop);
                    }
                };
                if collect_cdef {
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
                }
                let has_chroma = partition_node_has_chroma(
                    context,
                    node,
                    context.frame_width == 4 && context.frame_height == 4,
                );
                canvas.place_av1_partition_leaf(
                    node.x,
                    node.y,
                    node.width,
                    node.height,
                    has_chroma,
                    &decoded.planes,
                )?;
                if collect_loop_filter {
                    let Some((luma_tx, chroma_tx)) =
                        super::block::filter_transform_dimensions_for_block(
                            syntax_block_size,
                            transform_grid,
                            &decoded,
                            context.subsampling_x,
                            context.subsampling_y,
                            selected_segment.lossless,
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
                    filter_blocks.try_reserve(1).map_err(|_| {
                        CodecError::Dimensions(
                            "unable to allocate AV1 loop-filter metadata".to_owned(),
                        )
                    })?;
                    filter_blocks.push(super::filter::Block {
                        x,
                        y,
                        width,
                        height,
                        has_chroma,
                        luma_tx_width: luma_tx.0,
                        luma_tx_height: luma_tx.1,
                        chroma_tx_width: chroma_tx.0,
                        chroma_tx_height: chroma_tx.1,
                        skip_internal_edges: block_skipped && inter_metadata.is_some(),
                        levels: effective_loop_levels(
                            context,
                            selected_segment,
                            block_decoder.loop_delta_lf(),
                            inter_metadata
                                .as_ref()
                                .map_or(LoopFilterClass::Intra, |metadata| {
                                    LoopFilterClass::Inter {
                                        reference: metadata.references.first,
                                        mode: metadata.mode,
                                    }
                                }),
                        ),
                    });
                }
                if segmentation.update_map {
                    segment_updates.push(SegmentMapUpdate {
                        x: context.tile_origin_b4_x.saturating_add(node.x),
                        y: context.tile_origin_b4_y.saturating_add(node.y),
                        width: node.width,
                        height: node.height,
                        id: segment_id,
                    });
                }
                let commit_metadata =
                    inter_metadata.map_or_else(BlockCommitMetadata::intra, |metadata| {
                        BlockCommitMetadata {
                            coding: BlockCoding::Inter(metadata),
                            skip_mode: false,
                            tx_cells: None,
                        }
                    });
                tile_state.commit_with_metadata(
                    node,
                    has_chroma,
                    &decoded,
                    segment_id,
                    segment_pred,
                    commit_metadata,
                )?;
                Ok(PartitionVisitControl::Continue)
            })?;
            if unsupported || matches!(control, PartitionVisitControl::Stop) {
                return Ok(None);
            }
            if let Some(geometry) = bounded_i444_geometry {
                if bounded_i444_leaf_count != geometry.expected_leaf_count() {
                    return Ok(None);
                }
            }
            if let Some(geometry) = bounded_subsampled_rect_geometry {
                if bounded_subsampled_rect_leaf_count != geometry.expected_leaf_count() {
                    return Ok(None);
                }
            }
        }
    }
    if tile_state.is_empty() {
        return Ok(None);
    }
    tile_cdfs.partition = walker.cdfs;
    drop(walker);
    tile_cdfs.block = block_decoder.cdf_state();
    if decoder.symbol_coder_overread() {
        return Err(malformed("entropy symbol coder overread the tile padding"));
    }
    let cdef_frame_parameters = cdef_frame_parameters(context);
    let loop_parameters = loop_filter_parameters(context);
    let (planes, monochrome) = if context.monochrome {
        let plane = if monochrome_postfilter {
            let depth = super::sample_depth::SampleDepth::new(context.bit_depth)
                .ok_or_else(|| malformed("monochrome CDEF sample depth is unsupported"))?;
            canvas.finish_monochrome_with_cdef(
                cdef_frame_parameters,
                &cdef_indices,
                &cdef_active,
                depth,
            )?
        } else {
            canvas.finish_monochrome()?
        };
        let planes = [plane.clone(), plane.clone(), plane];
        (planes, true)
    } else {
        (canvas.finish()?, false)
    };
    if !segment_updates.is_empty() {
        let map = current_segment_map
            .as_deref_mut()
            .ok_or_else(|| malformed("segment updates omit their current map"))?;
        for update in segment_updates {
            map.fill(update.x, update.y, update.width, update.height, update.id)?;
        }
    }
    let leaf = super::block::FirstLeaf {
        width: context.frame_width,
        height: context.frame_height,
        block_skipped: false,
        planes,
        luma_predictor: super::block::LumaPredictor::Dc,
        chroma_predictor: None,
        luma_context: 0x40,
        chroma_contexts: [0x40; 2],
        chroma_right_contexts: [[0x40; 16]; 2],
        chroma_bottom_contexts: [[0x40; 16]; 2],
        tx_context_width: 0,
        tx_context_height: 0,
        luma_transform_split: false,
        luma_right_contexts: [0x40; 16],
        luma_bottom_contexts: [0x40; 16],
        wide_coefficient_contexts: None,
        palette_cache: Default::default(),
        #[cfg(coverage)]
        entropy_operations: decoder.operation_trace(),
    };
    Ok(Some(Lossy420Reconstruction {
        leaf,
        monochrome,
        subsampling_x: context.subsampling_x,
        subsampling_y: context.subsampling_y,
        filter_blocks,
        cdef_indices,
        cdef_active,
        loop_parameters,
        cdef_parameters: cdef_frame_parameters,
        restoration: restoration_plan,
        cdfs: Some(tile_cdfs),
    }))
}

fn complete_lossy_420_reconstruction_context(context: &FirstBlockContext) -> bool {
    let high_depth_full = complete_high_depth_full_reconstruction_context(context);
    let high_depth_420 = complete_high_depth_420_reconstruction_context(context);
    let high_depth_422 = complete_high_depth_422_intra_reconstruction_context(context);
    let simple_422 = context.subsampling_x
        && !context.subsampling_y
        && context.frame_width == 16
        && context.frame_height == 16
        && context.upscaled_width == 16
        && context.block_width == 4
        && context.block_height == 4
        && context.block_x == 0
        && context.block_y == 0
        && context.frame_tools.cdef.is_none()
        && !context.frame_tools.delta_q_present
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0;
    (closed_base_reconstruction_context(context)
        || high_depth_full
        || high_depth_420
        || high_depth_422)
        && !context.all_lossless
        && if high_depth_full {
            !context.subsampling_x && !context.subsampling_y
        } else if high_depth_420 {
            context.subsampling_x && context.subsampling_y
        } else if high_depth_422 {
            context.subsampling_x && !context.subsampling_y
        } else {
            (context.subsampling_x && context.subsampling_y)
                || (!context.subsampling_x && !context.subsampling_y)
                || simple_422
        }
        && context.frame_tools.quantization.is_some()
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

fn bounded_restoration_geometry(context: &FirstBlockContext) -> bool {
    if !context.single_tile
        || context.frame_height == 0
        || context.frame_height > 56
        || context.restoration_types == [None; 3]
        || !context.restoration_types.iter().all(|restoration_type| {
            restoration_type.is_none_or(|kind| {
                matches!(
                    kind,
                    RestorationType::Wiener | RestorationType::SgrProjection
                )
            })
        })
    {
        return false;
    }
    let chroma_width = context.upscaled_width.checked_add(1).map(|width| width / 2);
    let chroma_height = context.frame_height.checked_add(1).map(|height| height / 2);
    let Some(chroma_width) = chroma_width else {
        return false;
    };
    let Some(chroma_height) = chroma_height else {
        return false;
    };
    let dimensions = [
        (context.upscaled_width, context.frame_height, 0_usize),
        (chroma_width, chroma_height, 1_usize),
        (chroma_width, chroma_height, 1_usize),
    ];
    for restoration_type in context.restoration_types.iter().enumerate() {
        let (plane, restoration_type) = restoration_type;
        if restoration_type.is_none() {
            continue;
        }
        let (width, height, unit_index) = dimensions[plane];
        let unit_size_log2 = context.restoration_unit_size_log2[unit_index];
        let Some(unit_size) = 1_u32.checked_shl(unit_size_log2) else {
            return false;
        };
        let Some(width_with_half) = width.checked_add(unit_size / 2) else {
            return false;
        };
        let Some(height_with_half) = height.checked_add(unit_size / 2) else {
            return false;
        };
        let units_x = width_with_half >> unit_size_log2;
        let units_y = height_with_half >> unit_size_log2;
        if units_x.max(1) != 1 || units_y.max(1) != 1 {
            return false;
        }
    }
    true
}

fn bounded_restoration_common(context: &FirstBlockContext) -> bool {
    context.bit_depth == 8
        && context.subsampling_x
        && context.subsampling_y
        && !context.monochrome
        && !context.all_lossless
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && context.frame_tools.quantization.is_some()
        && no_unsupported_film_grain(context)
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1)
        && bounded_restoration_geometry(context)
}

fn bounded_restoration_cdef_supported(context: &FirstBlockContext) -> bool {
    matches!(
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
}

fn complete_bounded_restoration_intra_420_reconstruction_context(
    context: &FirstBlockContext,
) -> bool {
    context.intra_frame
        && bounded_restoration_common(context)
        && bounded_restoration_cdef_supported(context)
}

fn complete_bounded_restoration_inter_420_reconstruction_context(
    context: &FirstBlockContext,
) -> bool {
    !context.intra_frame
        && bounded_restoration_common(context)
        && context.frame_tools.transform_mode != 2
        && !context.segmentation_enabled
        && inter_cdef_supported(context)
}

/// First inter reconstruction tranche: 8-bit 4:2:0 translation blocks with
/// loop filtering and the bounded CDEF profile enabled. Single-reference and
/// average compound prediction share the checked MC boundary; joint/masked
/// compound, inter-intra, warped, and variable-transform branches are still
/// rejected before a block publishes neighbor metadata.
fn inter_cdef_supported(context: &FirstBlockContext) -> bool {
    let Some(cdef) = context.frame_tools.cdef else {
        return true;
    };
    let count = match cdef.bits {
        0 => 1,
        1 => 2,
        2 => 4,
        _ => return false,
    };
    context.frame_width.is_multiple_of(8)
        && context.frame_height.is_multiple_of(8)
        && cdef.y_strength_count == count
        && cdef.uv_strength_count == count
        && cdef.first_y_strength.is_some()
        && cdef.first_uv_strength.is_some()
}

fn complete_inter_420_reconstruction_context(context: &FirstBlockContext) -> bool {
    !context.intra_frame
        && context.bit_depth == 8
        && context.subsampling_x
        && context.subsampling_y
        && (context.superres_enabled || context.upscaled_width == context.frame_width)
        && !context.monochrome
        && !context.all_lossless
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && context.frame_tools.transform_mode != 2
        && inter_cdef_supported(context)
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && no_unsupported_film_grain(context)
        && !context.segmentation_enabled
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1)
}

/// The same bounded intra profile as the general lossy 4:2:0 decoder, with
/// AV1's horizontal super-resolution flag admitted. Reconstruction remains in
/// coded coordinates; the completed leaf is resized only after deblock/CDEF.
/// High-depth, lossless, restoration, and film-grain classes stay explicitly
/// outside this first super-resolution tranche.
fn complete_superres_lossy_420_reconstruction_context(context: &FirstBlockContext) -> bool {
    context.intra_frame
        && context.bit_depth == 8
        && context.superres_enabled
        && context.subsampling_x
        && context.subsampling_y
        && !context.monochrome
        && !context.all_lossless
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && context.frame_tools.quantization.is_some()
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && no_unsupported_film_grain(context)
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

/// Exact high-depth 4:2:0/4:2:2 inter tranche admitted by the depth-parametric
/// motion-compensation core. The block engine retains samples in `u16`, but
/// its inter path is intentionally limited to whole 8..=32-pixel transforms
/// and a closed tool profile until the remaining AV1 syntax families publish
/// their high-depth state. I422 additionally requires skipped residuals and
/// DCT-DCT chroma, enforced at the block boundary. Every retained reference is
/// checked up front so a later reference choice cannot narrow the path back to
/// eight-bit geometry.
fn complete_high_depth_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> bool {
    let i420 = context.subsampling_x && context.subsampling_y;
    let i422 = context.subsampling_x && !context.subsampling_y;
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.depth.bits() == context.bit_depth
            && (reference.surface.layout == PixelLayout::I420 && i420
                || reference.surface.layout == PixelLayout::I422 && i422)
            && !reference.scale.scaled
    });
    !context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && (i420 || i422)
        && !context.superres_enabled
        && context.upscaled_width == context.frame_width
        && !context.monochrome
        && !context.all_lossless
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.segmentation_enabled
        && context.frame_tools.transform_mode == 1
        && !context.frame_tools.reduced_transform_set
        && context
            .frame_tools
            .quantization
            .is_some_and(|quantization| !quantization.using_matrix)
        && !context.frame_tools.delta_lf_present
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context.frame_tools.cdef.is_none()
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && !inter_context.reference_mode_select
        && no_unsupported_film_grain(context)
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1)
        && references_match
}

/// Common frame-level proof for the first I422 restoration tranche. The
/// high-depth I422 block path is already limited to one 16x16 B16x16 leaf;
/// keep restoration on that same exact geometry and leave loop/CDEF inactive
/// until their I422 boundary metadata has independent parity evidence.
fn bounded_i422_restoration_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
) -> bool {
    context.subsampling_x
        && !context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.frame_width == 16
        && context.frame_height == 16
        && context.upscaled_width == 16
        && context.block_width == 4
        && context.block_height == 4
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1)
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context.frame_tools.cdef.is_none()
        && context.frame_tools.restoration_present
        && context.restoration_types.iter().any(Option::is_some)
        && context.restoration_types.iter().all(|restoration_type| {
            restoration_type.is_none_or(|kind| {
                matches!(
                    kind,
                    RestorationType::Wiener | RestorationType::SgrProjection
                )
            })
        })
        && bounded_i422_restoration_units_supported(context)
}

fn complete_bounded_i422_restoration_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && bounded_i422_restoration_common(context, quantization)
}

fn complete_bounded_i422_restoration_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I422
            && reference.surface.coded_width == 16
            && reference.surface.upscaled_width == 16
            && reference.surface.frame_height == 16
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    !context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && bounded_i422_restoration_common(context, quantization)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match
}

fn bounded_i422_restoration_units_supported(context: &FirstBlockContext) -> bool {
    bounded_i422_restoration_units_supported_for_dimensions(context, 16, 16)
}

fn bounded_i422_restoration_units_supported_for_dimensions(
    context: &FirstBlockContext,
    luma_width: u32,
    luma_height: u32,
) -> bool {
    let unit_log2 = context.restoration_unit_size_log2;
    let minimum_log2 = match context.level {
        0 => 7,
        1 => 6,
        _ => return false,
    };
    if unit_log2[0] != unit_log2[1] || !(minimum_log2..=8).contains(&unit_log2[0]) {
        return false;
    }
    let Some(unit_size) = 1_u32.checked_shl(unit_log2[0]) else {
        return false;
    };
    let Some(chroma_width) = luma_width.checked_add(1).map(|width| width / 2) else {
        return false;
    };
    for (width, height) in [
        (luma_width, luma_height),
        (chroma_width, luma_height),
        (chroma_width, luma_height),
    ] {
        let Some(width_with_half) = width.checked_add(unit_size / 2) else {
            return false;
        };
        let Some(height_with_half) = height.checked_add(unit_size / 2) else {
            return false;
        };
        let units_x = (width_with_half >> unit_log2[0]).max(1);
        let units_y = (height_with_half >> unit_log2[0]).max(1);
        if units_x != 1 || units_y != 1 {
            return false;
        }
    }
    true
}

/// Common frame-level proof for the first high-depth I420 restoration
/// tranche. I420's chroma restoration exponent can be one step smaller than
/// luma, so its unit validation remains separate from the I422/I444 helpers.
fn bounded_i420_restoration_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
) -> bool {
    context.subsampling_x
        && context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.frame_width == 16
        && context.frame_height == 16
        && context.upscaled_width == 16
        && context.block_width == 4
        && context.block_height == 4
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1)
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context.frame_tools.cdef.is_none()
        && context.frame_tools.restoration_present
        && context.restoration_types.iter().any(Option::is_some)
        && context.restoration_types.iter().all(|restoration_type| {
            restoration_type.is_none_or(|kind| {
                matches!(
                    kind,
                    RestorationType::Wiener | RestorationType::SgrProjection
                )
            })
        })
        && bounded_i420_restoration_units_supported(context)
}

fn complete_bounded_i420_restoration_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && bounded_i420_restoration_common(context, quantization)
}

fn complete_bounded_i420_restoration_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I420
            && reference.surface.coded_width == 16
            && reference.surface.upscaled_width == 16
            && reference.surface.frame_height == 16
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    !context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && bounded_i420_restoration_common(context, quantization)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match
}

fn bounded_i420_restoration_units_supported(context: &FirstBlockContext) -> bool {
    bounded_i420_restoration_units_supported_for_dimensions(context, 16, 16)
}

fn bounded_i420_restoration_units_supported_for_dimensions(
    context: &FirstBlockContext,
    luma_width: u32,
    luma_height: u32,
) -> bool {
    let luma_log2 = context.restoration_unit_size_log2[0];
    let chroma_log2 = context.restoration_unit_size_log2[1];
    let minimum_luma = match context.level {
        0 => 7,
        1 => 6,
        _ => return false,
    };
    if !(minimum_luma..=8).contains(&luma_log2) {
        return false;
    }
    let chroma_active =
        context.restoration_types[1].is_some() || context.restoration_types[2].is_some();
    if chroma_log2 > luma_log2 || luma_log2.saturating_sub(chroma_log2) > u32::from(chroma_active) {
        return false;
    }
    let Some(chroma_width) = luma_width.checked_add(1).map(|width| width / 2) else {
        return false;
    };
    let Some(chroma_height) = luma_height.checked_add(1).map(|height| height / 2) else {
        return false;
    };
    for (plane, (width, height)) in [
        (luma_width, luma_height),
        (chroma_width, chroma_height),
        (chroma_width, chroma_height),
    ]
    .into_iter()
    .enumerate()
    {
        if context.restoration_types[plane].is_none() {
            continue;
        }
        let unit_log2 = if plane == 0 { luma_log2 } else { chroma_log2 };
        let Some(unit_size) = 1_u32.checked_shl(unit_log2) else {
            return false;
        };
        let Some(width_with_half) = width.checked_add(unit_size / 2) else {
            return false;
        };
        let Some(height_with_half) = height.checked_add(unit_size / 2) else {
            return false;
        };
        let units_x = (width_with_half >> unit_log2).max(1);
        let units_y = (height_with_half >> unit_log2).max(1);
        if units_x != 1 || units_y != 1 {
            return false;
        }
    }
    true
}

/// Common frame-level proof for the first high-depth I420 CDEF tranche. CDEF
/// is staged after the one B16x16 reconstruction while loop filtering and
/// restoration remain inactive, keeping the filter order and metadata maps
/// within the already checked single-tile path.
fn bounded_i420_cdef_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
) -> bool {
    context.subsampling_x
        && context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.frame_width == 16
        && context.frame_height == 16
        && context.upscaled_width == 16
        && context.block_width == 4
        && context.block_height == 4
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1)
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && context
            .frame_tools
            .cdef
            .is_some_and(|_| bounded_i444_cdef_supported(context))
}

fn complete_bounded_i420_cdef_intra_reconstruction_context(context: &FirstBlockContext) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && bounded_i420_cdef_common(context, quantization)
}

fn complete_bounded_i420_cdef_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I420
            && reference.surface.coded_width == 16
            && reference.surface.upscaled_width == 16
            && reference.surface.frame_height == 16
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    !context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && bounded_i420_cdef_common(context, quantization)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match
}

/// Common frame-level proof for the first bounded I422 CDEF tranche. The
/// luma plane remains 16x16 while horizontally subsampled chroma is 8x16;
/// the checked CDEF raster already owns that independent plane geometry.
fn bounded_i422_cdef_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
) -> bool {
    context.subsampling_x
        && !context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.frame_width == 16
        && context.frame_height == 16
        && context.upscaled_width == 16
        && context.block_width == 4
        && context.block_height == 4
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1)
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && context
            .frame_tools
            .cdef
            .is_some_and(|_| bounded_i444_cdef_supported(context))
}

fn complete_bounded_i422_cdef_intra_reconstruction_context(context: &FirstBlockContext) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && bounded_i422_cdef_common(context, quantization)
}

fn complete_bounded_i422_cdef_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I422
            && reference.surface.coded_width == 16
            && reference.surface.upscaled_width == 16
            && reference.surface.frame_height == 16
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    !context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && bounded_i422_cdef_common(context, quantization)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match
}

fn bounded_i422_rect_cdef_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
    geometry: BoundedSubsampledRectGeometry,
) -> bool {
    let (frame_width, frame_height) = geometry.dimensions();
    context.subsampling_x
        && !context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.frame_width == frame_width
        && context.frame_height == frame_height
        && context.upscaled_width == frame_width
        && context.block_x == 0
        && context.block_y == 0
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && context
            .frame_tools
            .cdef
            .is_some_and(|_| bounded_i444_cdef_supported(context))
}

fn complete_bounded_i422_rect_cdef_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    (context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && bounded_i422_rect_cdef_common(context, quantization, geometry))
    .then_some(geometry)
}

fn complete_bounded_i422_rect_cdef_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    let (reference_width, reference_height) = geometry.dimensions();
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I422
            && reference.surface.coded_width == reference_width
            && reference.surface.upscaled_width == reference_width
            && reference.surface.frame_height == reference_height
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    (!context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && bounded_i422_rect_cdef_common(context, quantization, geometry)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match)
        .then_some(geometry)
}

fn bounded_i422_rect_restoration_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
    geometry: BoundedSubsampledRectGeometry,
) -> bool {
    let (frame_width, frame_height) = geometry.dimensions();
    context.subsampling_x
        && !context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.frame_width == frame_width
        && context.frame_height == frame_height
        && context.upscaled_width == frame_width
        && context.block_x == 0
        && context.block_y == 0
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context.frame_tools.cdef.is_none()
        && context.frame_tools.restoration_present
        && context.restoration_types.iter().any(Option::is_some)
        && context.restoration_types.iter().all(|restoration_type| {
            restoration_type.is_none_or(|kind| {
                matches!(
                    kind,
                    RestorationType::Wiener | RestorationType::SgrProjection
                )
            })
        })
        && geometry.restoration_supported()
        && bounded_i422_restoration_units_supported_for_dimensions(
            context,
            frame_width,
            frame_height,
        )
}

fn complete_bounded_i422_rect_restoration_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    (context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && bounded_i422_rect_restoration_common(context, quantization, geometry))
    .then_some(geometry)
}

fn complete_bounded_i422_rect_restoration_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    let (reference_width, reference_height) = geometry.dimensions();
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I422
            && reference.surface.coded_width == reference_width
            && reference.surface.upscaled_width == reference_width
            && reference.surface.frame_height == reference_height
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    (!context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && bounded_i422_rect_restoration_common(context, quantization, geometry)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match)
        .then_some(geometry)
}

/// Common frame-level proof for the bounded high-depth I420 rectangular loop
/// tranche. Each admitted partition is exactly two B16x16 leaves in a 32x16
/// or 16x32 frame, with only the luma edge that exists in that orientation
/// enabled; CDEF and restoration remain inactive.
fn bounded_i420_rect_loop_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
    geometry: BoundedSubsampledRectGeometry,
) -> bool {
    let (frame_width, frame_height) = geometry.dimensions();
    context.subsampling_x
        && context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.frame_width == frame_width
        && context.frame_height == frame_height
        && context.upscaled_width == frame_width
        && context.block_x == 0
        && context.block_y == 0
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.cdef.is_none()
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && bounded_subsampled_rect_loop_filter_supported(context, geometry)
}

fn complete_bounded_i420_rect_loop_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    (context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && bounded_i420_rect_loop_common(context, quantization, geometry))
    .then_some(geometry)
}

fn complete_bounded_i420_rect_loop_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    let (reference_width, reference_height) = geometry.dimensions();
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I420
            && reference.surface.coded_width == reference_width
            && reference.surface.upscaled_width == reference_width
            && reference.surface.frame_height == reference_height
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    (!context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && bounded_i420_rect_loop_common(context, quantization, geometry)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match)
        .then_some(geometry)
}

/// Common frame-level proof for the bounded high-depth I420 rectangular CDEF
/// tranche. The two-leaf geometry is shared with the loop path while both
/// luma edge levels and chroma levels stay disabled.
fn bounded_i420_rect_cdef_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
    geometry: BoundedSubsampledRectGeometry,
) -> bool {
    let (frame_width, frame_height) = geometry.dimensions();
    context.subsampling_x
        && context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.frame_width == frame_width
        && context.frame_height == frame_height
        && context.upscaled_width == frame_width
        && context.block_x == 0
        && context.block_y == 0
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && context
            .frame_tools
            .cdef
            .is_some_and(|_| bounded_i444_cdef_supported(context))
}

fn complete_bounded_i420_rect_cdef_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    (context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && bounded_i420_rect_cdef_common(context, quantization, geometry))
    .then_some(geometry)
}

fn complete_bounded_i420_rect_cdef_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    let (reference_width, reference_height) = geometry.dimensions();
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I420
            && reference.surface.coded_width == reference_width
            && reference.surface.upscaled_width == reference_width
            && reference.surface.frame_height == reference_height
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    (!context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && bounded_i420_rect_cdef_common(context, quantization, geometry)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match)
        .then_some(geometry)
}

fn bounded_i420_rect_restoration_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
    geometry: BoundedSubsampledRectGeometry,
) -> bool {
    let (frame_width, frame_height) = geometry.dimensions();
    context.subsampling_x
        && context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.frame_width == frame_width
        && context.frame_height == frame_height
        && context.upscaled_width == frame_width
        && context.block_x == 0
        && context.block_y == 0
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context.frame_tools.cdef.is_none()
        && context.frame_tools.restoration_present
        && context.restoration_types.iter().any(Option::is_some)
        && context.restoration_types.iter().all(|restoration_type| {
            restoration_type.is_none_or(|kind| {
                matches!(
                    kind,
                    RestorationType::Wiener | RestorationType::SgrProjection
                )
            })
        })
        && geometry.restoration_supported()
        && bounded_i420_restoration_units_supported_for_dimensions(
            context,
            frame_width,
            frame_height,
        )
}

fn complete_bounded_i420_rect_restoration_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    (context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && bounded_i420_rect_restoration_common(context, quantization, geometry))
    .then_some(geometry)
}

fn complete_bounded_i420_rect_restoration_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    let (reference_width, reference_height) = geometry.dimensions();
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I420
            && reference.surface.coded_width == reference_width
            && reference.surface.upscaled_width == reference_width
            && reference.surface.frame_height == reference_height
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    (!context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && bounded_i420_rect_restoration_common(context, quantization, geometry)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match)
        .then_some(geometry)
}

/// Common frame-level proof for the combined bounded I420/I422 CDEF and
/// restoration tranche. The standalone predicates stay intentionally narrow;
/// this separate class admits the legal filter order while retaining the same
/// one-B16 geometry and checked one-unit restoration bounds.
fn bounded_subsampled_cdef_restoration_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
) -> bool {
    let i420 = context.subsampling_x && context.subsampling_y;
    let i422 = context.subsampling_x && !context.subsampling_y;
    let restoration_units_supported = if i420 {
        bounded_i420_restoration_units_supported(context)
    } else if i422 {
        bounded_i422_restoration_units_supported(context)
    } else {
        false
    };
    (i420 || i422)
        && !context.monochrome
        && context.single_tile
        && context.frame_width == 16
        && context.frame_height == 16
        && context.upscaled_width == 16
        && context.block_width == 4
        && context.block_height == 4
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1)
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context
            .frame_tools
            .cdef
            .is_some_and(|_| bounded_i444_cdef_supported(context))
        && context.frame_tools.restoration_present
        && context.restoration_types.iter().any(Option::is_some)
        && context.restoration_types.iter().all(|restoration_type| {
            restoration_type.is_none_or(|kind| {
                matches!(
                    kind,
                    RestorationType::Wiener | RestorationType::SgrProjection
                )
            })
        })
        && restoration_units_supported
}

fn complete_bounded_i420_cdef_restoration_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && context.subsampling_x
        && context.subsampling_y
        && bounded_subsampled_cdef_restoration_common(context, quantization)
}

fn complete_bounded_i420_cdef_restoration_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I420
            && reference.surface.coded_width == 16
            && reference.surface.upscaled_width == 16
            && reference.surface.frame_height == 16
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    !context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && bounded_subsampled_cdef_restoration_common(context, quantization)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match
}

fn complete_bounded_i422_cdef_restoration_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && context.subsampling_x
        && !context.subsampling_y
        && bounded_subsampled_cdef_restoration_common(context, quantization)
}

fn complete_bounded_i422_cdef_restoration_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I422
            && reference.surface.coded_width == 16
            && reference.surface.upscaled_width == 16
            && reference.surface.frame_height == 16
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    !context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && bounded_subsampled_cdef_restoration_common(context, quantization)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match
}

/// Shared rectangular proof for the bounded CDEF+restoration profiles. The
/// layout-specific wrappers below add the I420/I422 geometry and restoration
/// exponent rules without widening the one-block predicates above.
fn bounded_subsampled_rect_cdef_restoration_base(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
    geometry: BoundedSubsampledRectGeometry,
) -> bool {
    let (frame_width, frame_height) = geometry.dimensions();
    context.single_tile
        && context.frame_width == frame_width
        && context.frame_height == frame_height
        && context.upscaled_width == frame_width
        && context.block_x == 0
        && context.block_y == 0
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context
            .frame_tools
            .cdef
            .is_some_and(|_| bounded_i444_cdef_supported(context))
        && context.frame_tools.restoration_present
        && context.restoration_types.iter().any(Option::is_some)
        && context.restoration_types.iter().all(|restoration_type| {
            restoration_type.is_none_or(|kind| {
                matches!(
                    kind,
                    RestorationType::Wiener | RestorationType::SgrProjection
                )
            })
        })
}

fn bounded_i420_rect_cdef_restoration_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
    geometry: BoundedSubsampledRectGeometry,
) -> bool {
    let (frame_width, frame_height) = geometry.dimensions();
    context.subsampling_x
        && context.subsampling_y
        && !context.monochrome
        && bounded_subsampled_rect_cdef_restoration_base(context, quantization, geometry)
        && geometry.restoration_supported()
        && bounded_i420_restoration_units_supported_for_dimensions(
            context,
            frame_width,
            frame_height,
        )
}

fn complete_bounded_i420_rect_cdef_restoration_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    (context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && bounded_i420_rect_cdef_restoration_common(context, quantization, geometry))
    .then_some(geometry)
}

fn complete_bounded_i420_rect_cdef_restoration_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    let (reference_width, reference_height) = geometry.dimensions();
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I420
            && reference.surface.coded_width == reference_width
            && reference.surface.upscaled_width == reference_width
            && reference.surface.frame_height == reference_height
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    (!context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && bounded_i420_rect_cdef_restoration_common(context, quantization, geometry)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match)
        .then_some(geometry)
}

fn bounded_i422_rect_cdef_restoration_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
    geometry: BoundedSubsampledRectGeometry,
) -> bool {
    let (frame_width, frame_height) = geometry.dimensions();
    context.subsampling_x
        && !context.subsampling_y
        && !context.monochrome
        && bounded_subsampled_rect_cdef_restoration_base(context, quantization, geometry)
        && geometry.restoration_supported()
        && bounded_i422_restoration_units_supported_for_dimensions(
            context,
            frame_width,
            frame_height,
        )
}

fn complete_bounded_i422_rect_cdef_restoration_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    (context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && bounded_i422_rect_cdef_restoration_common(context, quantization, geometry))
    .then_some(geometry)
}

fn complete_bounded_i422_rect_cdef_restoration_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    let (reference_width, reference_height) = geometry.dimensions();
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I422
            && reference.surface.coded_width == reference_width
            && reference.surface.upscaled_width == reference_width
            && reference.surface.frame_height == reference_height
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    (!context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && bounded_i422_rect_cdef_restoration_common(context, quantization, geometry)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match)
        .then_some(geometry)
}

#[derive(Clone, Copy)]
struct BoundedRectLoopPostfilters {
    geometry: BoundedSubsampledRectGeometry,
    restoration: bool,
}

/// Common rectangular proof for loop filtering followed by CDEF and/or
/// restoration. Loop-only rectangles are admitted by their dedicated
/// predicates; this helper requires at least one later post-filter so the
/// restoration decoder is selected only for profiles that actually need it.
fn bounded_subsampled_rect_loop_postfilters_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
    geometry: BoundedSubsampledRectGeometry,
) -> Option<BoundedRectLoopPostfilters> {
    let cdef = context.frame_tools.cdef.is_some();
    if cdef && !bounded_i444_cdef_supported(context) {
        return None;
    }
    let restoration = if context.frame_tools.restoration_present {
        if !context.restoration_types.iter().any(Option::is_some)
            || !context.restoration_types.iter().all(|restoration_type| {
                restoration_type.is_none_or(|kind| {
                    matches!(
                        kind,
                        RestorationType::Wiener | RestorationType::SgrProjection
                    )
                })
            })
        {
            return None;
        }
        if !geometry.restoration_supported() {
            return None;
        }
        let supported = if context.subsampling_x && context.subsampling_y {
            let (frame_width, frame_height) = geometry.dimensions();
            bounded_i420_restoration_units_supported_for_dimensions(
                context,
                frame_width,
                frame_height,
            )
        } else if context.subsampling_x && !context.subsampling_y {
            let (frame_width, frame_height) = geometry.dimensions();
            bounded_i422_restoration_units_supported_for_dimensions(
                context,
                frame_width,
                frame_height,
            )
        } else {
            false
        };
        if !supported {
            return None;
        }
        true
    } else {
        if context.restoration_types != [None; 3] {
            return None;
        }
        false
    };
    if !cdef && !restoration {
        return None;
    }
    let (frame_width, frame_height) = geometry.dimensions();
    (context.single_tile
        && context.frame_width == frame_width
        && context.frame_height == frame_height
        && context.upscaled_width == frame_width
        && context.block_x == 0
        && context.block_y == 0
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && bounded_subsampled_rect_loop_filter_supported(context, geometry))
    .then_some(BoundedRectLoopPostfilters {
        geometry,
        restoration,
    })
}

fn complete_bounded_i420_rect_loop_postfilters_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> Option<BoundedRectLoopPostfilters> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    (context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && context.subsampling_x
        && context.subsampling_y)
        .then_some(())
        .and_then(|_| {
            bounded_subsampled_rect_loop_postfilters_common(context, quantization, geometry)
        })
}

fn complete_bounded_i420_rect_loop_postfilters_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> Option<BoundedRectLoopPostfilters> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    let (reference_width, reference_height) = geometry.dimensions();
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I420
            && reference.surface.coded_width == reference_width
            && reference.surface.upscaled_width == reference_width
            && reference.surface.frame_height == reference_height
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    (!context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && context.subsampling_x
        && context.subsampling_y
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match)
        .then_some(())
        .and_then(|_| {
            bounded_subsampled_rect_loop_postfilters_common(context, quantization, geometry)
        })
}

fn complete_bounded_i422_rect_loop_postfilters_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> Option<BoundedRectLoopPostfilters> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    (context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && context.subsampling_x
        && !context.subsampling_y)
        .then_some(())
        .and_then(|_| {
            bounded_subsampled_rect_loop_postfilters_common(context, quantization, geometry)
        })
}

fn complete_bounded_i422_rect_loop_postfilters_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> Option<BoundedRectLoopPostfilters> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    let (reference_width, reference_height) = geometry.dimensions();
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I422
            && reference.surface.coded_width == reference_width
            && reference.surface.upscaled_width == reference_width
            && reference.surface.frame_height == reference_height
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    (!context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && context.subsampling_x
        && !context.subsampling_y
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match)
        .then_some(())
        .and_then(|_| {
            bounded_subsampled_rect_loop_postfilters_common(context, quantization, geometry)
        })
}

#[derive(Clone, Copy)]
struct BoundedLoopPostfilters {
    restoration: bool,
}

/// Common frame-level proof for bounded I420/I422 loop filtering combined
/// with one or both later post-filters. A separate profile keeps the existing
/// loop-only, CDEF-only, restoration-only, and CDEF+restoration admissions
/// narrow while selecting the restoration decoder only when restoration is
/// actually active in the frame header.
fn bounded_subsampled_loop_postfilters_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
) -> Option<BoundedLoopPostfilters> {
    let i420 = context.subsampling_x && context.subsampling_y;
    let i422 = context.subsampling_x && !context.subsampling_y;
    let cdef = context.frame_tools.cdef.is_some();
    if cdef && !bounded_i444_cdef_supported(context) {
        return None;
    }
    let restoration = if context.frame_tools.restoration_present {
        if !context.restoration_types.iter().any(Option::is_some)
            || !context.restoration_types.iter().all(|restoration_type| {
                restoration_type.is_none_or(|kind| {
                    matches!(
                        kind,
                        RestorationType::Wiener | RestorationType::SgrProjection
                    )
                })
            })
        {
            return None;
        }
        if i420 {
            if !bounded_i420_restoration_units_supported(context) {
                return None;
            }
        } else if i422 {
            if !bounded_i422_restoration_units_supported(context) {
                return None;
            }
        } else {
            return None;
        }
        true
    } else {
        if context.restoration_types != [None; 3] {
            return None;
        }
        false
    };
    if !cdef && !restoration {
        return None;
    }
    (i420 || i422)
        .then_some(())
        .filter(|_| {
            !context.monochrome
                && context.single_tile
                && context.frame_width == 16
                && context.frame_height == 16
                && context.upscaled_width == 16
                && context.block_width == 4
                && context.block_height == 4
                && context.block_x == 0
                && context.block_y == 0
                && matches!(context.level, 0 | 1)
                && !context.superres_enabled
                && !context.all_lossless
                && !context.segmentation_enabled
                && !context.frame_tools.segmentation.enabled
                && !context.skip_mode_enabled
                && !context.allow_intrabc
                && !context.allow_screen_content_tools
                && !context.frame_tools.film_grain_present
                && !context.frame_tools.delta_q_present
                && !context.frame_tools.delta_lf_present
                && !context.frame_tools.segment_lossless
                && context.frame_tools.segment_qindex == quantization.base
                && quantization.base != 0
                && !quantization.using_matrix
                && !context.frame_tools.reduced_transform_set
                && context.frame_tools.transform_mode == 1
                && context.frame_tools.loop_filter.sharpness <= 7
                && context
                    .frame_tools
                    .loop_filter
                    .level_y
                    .iter()
                    .all(|&level| level <= 63)
                && context.frame_tools.loop_filter.level_u <= 63
                && context.frame_tools.loop_filter.level_v <= 63
                && (context.frame_tools.loop_filter.level_y != [0; 2]
                    || context.frame_tools.loop_filter.level_u != 0
                    || context.frame_tools.loop_filter.level_v != 0)
        })
        .map(|_| BoundedLoopPostfilters { restoration })
}

fn complete_bounded_i420_loop_postfilters_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> Option<BoundedLoopPostfilters> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    (context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && context.subsampling_x
        && context.subsampling_y)
        .then_some(())
        .and_then(|_| bounded_subsampled_loop_postfilters_common(context, quantization))
}

fn complete_bounded_i420_loop_postfilters_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> Option<BoundedLoopPostfilters> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I420
            && reference.surface.coded_width == 16
            && reference.surface.upscaled_width == 16
            && reference.surface.frame_height == 16
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    (!context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && context.subsampling_x
        && context.subsampling_y
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match)
        .then_some(())
        .and_then(|_| bounded_subsampled_loop_postfilters_common(context, quantization))
}

fn complete_bounded_i422_loop_postfilters_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> Option<BoundedLoopPostfilters> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    (context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && context.subsampling_x
        && !context.subsampling_y)
        .then_some(())
        .and_then(|_| bounded_subsampled_loop_postfilters_common(context, quantization))
}

fn complete_bounded_i422_loop_postfilters_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> Option<BoundedLoopPostfilters> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I422
            && reference.surface.coded_width == 16
            && reference.surface.upscaled_width == 16
            && reference.surface.frame_height == 16
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    (!context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && context.subsampling_x
        && !context.subsampling_y
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match)
        .then_some(())
        .and_then(|_| bounded_subsampled_loop_postfilters_common(context, quantization))
}

/// Common frame-level proof for the first bounded I420/I422 loop-filter
/// tranche. The one B16x16 leaf has no internal edge on either subsampled
/// layout, but retaining the complete bounded header still keeps every
/// nonzero level and sharpness value within the checked AV1 domain. Filter
/// metadata is therefore accepted without widening the partition or tool
/// surface that the block path can safely publish.
fn bounded_subsampled_loop_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
) -> bool {
    let i420 = context.subsampling_x && context.subsampling_y;
    let i422 = context.subsampling_x && !context.subsampling_y;
    let loop_filter = context.frame_tools.loop_filter;
    (i420 || i422)
        && !context.monochrome
        && context.single_tile
        && context.frame_width == 16
        && context.frame_height == 16
        && context.upscaled_width == 16
        && context.block_width == 4
        && context.block_height == 4
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1)
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.cdef.is_none()
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && loop_filter.sharpness <= 7
        && loop_filter.level_y.iter().all(|&level| level <= 63)
        && loop_filter.level_u <= 63
        && loop_filter.level_v <= 63
        && (loop_filter.level_y != [0; 2] || loop_filter.level_u != 0 || loop_filter.level_v != 0)
}

fn complete_bounded_i420_loop_intra_reconstruction_context(context: &FirstBlockContext) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && context.subsampling_x
        && context.subsampling_y
        && bounded_subsampled_loop_common(context, quantization)
}

fn complete_bounded_i420_loop_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I420
            && reference.surface.coded_width == 16
            && reference.surface.upscaled_width == 16
            && reference.surface.frame_height == 16
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    !context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && bounded_subsampled_loop_common(context, quantization)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match
}

fn complete_bounded_i422_loop_intra_reconstruction_context(context: &FirstBlockContext) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && context.subsampling_x
        && !context.subsampling_y
        && bounded_subsampled_loop_common(context, quantization)
}

fn complete_bounded_i422_loop_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I422
            && reference.surface.coded_width == 16
            && reference.surface.upscaled_width == 16
            && reference.surface.frame_height == 16
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    !context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && bounded_subsampled_loop_common(context, quantization)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoundedSubsampledRectGeometry {
    /// A level-2 split followed by two level-3 B16x16 leaves side by side.
    TwoHorizontal,
    /// A level-2 split followed by two level-3 B16x16 leaves stacked.
    TwoVertical,
    /// A level-2 split followed by four level-3 B16x16 leaves in raster order.
    FourSquare,
    /// A level-1 split followed by four level-2 splits and sixteen level-3
    /// B16x16 leaves in AV1's depth-first quadrant order.
    SixteenSquare,
    /// A level-0 root with two 32x32 quadrants across eight level-3 leaves.
    EightHorizontal,
    /// A level-0 root with two 32x32 quadrants down eight level-3 leaves.
    EightVertical,
}

impl BoundedSubsampledRectGeometry {
    const fn dimensions(self) -> (u32, u32) {
        match self {
            Self::TwoHorizontal => (32, 16),
            Self::TwoVertical => (16, 32),
            Self::FourSquare => (32, 32),
            Self::SixteenSquare => (64, 64),
            Self::EightHorizontal => (64, 32),
            Self::EightVertical => (32, 64),
        }
    }

    const fn expected_leaf_count(self) -> usize {
        match self {
            Self::TwoHorizontal | Self::TwoVertical => 2,
            Self::FourSquare => 4,
            Self::SixteenSquare => 16,
            Self::EightHorizontal | Self::EightVertical => 8,
        }
    }

    const fn restoration_supported(self) -> bool {
        !matches!(self, Self::EightVertical | Self::SixteenSquare)
    }
}

fn bounded_subsampled_rect_geometry_for_context(
    context: &FirstBlockContext,
) -> Option<BoundedSubsampledRectGeometry> {
    match (
        context.frame_width,
        context.frame_height,
        context.block_width,
        context.block_height,
        context.level,
    ) {
        (32, 16, 8, 4, 1) => Some(BoundedSubsampledRectGeometry::TwoHorizontal),
        (16, 32, 4, 8, 1) => Some(BoundedSubsampledRectGeometry::TwoVertical),
        (32, 32, 8, 8, 1) => Some(BoundedSubsampledRectGeometry::FourSquare),
        (64, 64, 16, 16, 0) => Some(BoundedSubsampledRectGeometry::SixteenSquare),
        (64, 32, 16, 8, 0) => Some(BoundedSubsampledRectGeometry::EightHorizontal),
        (32, 64, 8, 16, 0) => Some(BoundedSubsampledRectGeometry::EightVertical),
        _ => None,
    }
}

fn bounded_subsampled_rect_loop_filter_supported(
    context: &FirstBlockContext,
    geometry: BoundedSubsampledRectGeometry,
) -> bool {
    let loop_filter = context.frame_tools.loop_filter;
    if loop_filter.sharpness > 7
        || loop_filter.level_y.iter().any(|&level| level > 63)
        || loop_filter.level_u > 63
        || loop_filter.level_v > 63
        || loop_filter.level_u != 0
        || loop_filter.level_v != 0
    {
        return false;
    }
    match geometry {
        BoundedSubsampledRectGeometry::TwoHorizontal => loop_filter.level_y[0] != 0,
        BoundedSubsampledRectGeometry::TwoVertical => loop_filter.level_y[1] != 0,
        BoundedSubsampledRectGeometry::FourSquare => loop_filter.level_y != [0; 2],
        BoundedSubsampledRectGeometry::SixteenSquare => loop_filter.level_y != [0; 2],
        BoundedSubsampledRectGeometry::EightHorizontal => loop_filter.level_y != [0; 2],
        BoundedSubsampledRectGeometry::EightVertical => loop_filter.level_y != [0; 2],
    }
}

fn bounded_subsampled_expected_rect_terminal(
    geometry: BoundedSubsampledRectGeometry,
    leaf_index: usize,
    node: PartitionNode,
) -> bool {
    let expected = match geometry {
        BoundedSubsampledRectGeometry::TwoHorizontal => match leaf_index {
            0 => (0, 0),
            1 => (4, 0),
            _ => return false,
        },
        BoundedSubsampledRectGeometry::TwoVertical => match leaf_index {
            0 => (0, 0),
            1 => (0, 4),
            _ => return false,
        },
        BoundedSubsampledRectGeometry::FourSquare => match leaf_index {
            0 => (0, 0),
            1 => (4, 0),
            2 => (0, 4),
            3 => (4, 4),
            _ => return false,
        },
        BoundedSubsampledRectGeometry::SixteenSquare => match leaf_index {
            0 => (0, 0),
            1 => (4, 0),
            2 => (0, 4),
            3 => (4, 4),
            4 => (8, 0),
            5 => (12, 0),
            6 => (8, 4),
            7 => (12, 4),
            8 => (0, 8),
            9 => (4, 8),
            10 => (0, 12),
            11 => (4, 12),
            12 => (8, 8),
            13 => (12, 8),
            14 => (8, 12),
            15 => (12, 12),
            _ => return false,
        },
        BoundedSubsampledRectGeometry::EightHorizontal => match leaf_index {
            0 => (0, 0),
            1 => (4, 0),
            2 => (0, 4),
            3 => (4, 4),
            4 => (8, 0),
            5 => (12, 0),
            6 => (8, 4),
            7 => (12, 4),
            _ => return false,
        },
        BoundedSubsampledRectGeometry::EightVertical => match leaf_index {
            0 => (0, 0),
            1 => (4, 0),
            2 => (0, 4),
            3 => (4, 4),
            4 => (0, 8),
            5 => (4, 8),
            6 => (0, 12),
            7 => (4, 12),
            _ => return false,
        },
    };
    node.level == 3
        && node.x == expected.0
        && node.y == expected.1
        && node.coded_width == 4
        && node.coded_height == 4
        && node.width == 4
        && node.height == 4
        && node.block_size == BlockSize::B16x16
        && node.kind == PartitionKind::None
}

fn bounded_i422_rect_loop_common(
    context: &FirstBlockContext,
    quantization: QuantizationContext,
    geometry: BoundedSubsampledRectGeometry,
) -> bool {
    let (frame_width, frame_height) = geometry.dimensions();
    context.subsampling_x
        && !context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.frame_width == frame_width
        && context.frame_height == frame_height
        && context.upscaled_width == frame_width
        && context.block_x == 0
        && context.block_y == 0
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.cdef.is_none()
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && bounded_subsampled_rect_loop_filter_supported(context, geometry)
}

fn complete_bounded_i422_rect_loop_intra_reconstruction_context(
    context: &FirstBlockContext,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    (context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && bounded_i422_rect_loop_common(context, quantization, geometry))
    .then_some(geometry)
}

fn complete_bounded_i422_rect_loop_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> Option<BoundedSubsampledRectGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_subsampled_rect_geometry_for_context(context)?;
    let (reference_width, reference_height) = geometry.dimensions();
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I422
            && reference.surface.coded_width == reference_width
            && reference.surface.upscaled_width == reference_width
            && reference.surface.frame_height == reference_height
            && !reference.scale.scaled
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
    });
    (!context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && bounded_i422_rect_loop_common(context, quantization, geometry)
        && !inter_context.reference_mode_select
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match)
        .then_some(geometry)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoundedI444InterGeometry {
    /// One normalized B16x16/Square16 terminal in a 16x16 frame.
    OneBlock,
    /// A level-2 SPLIT followed by two level-3 NONE B16x16 terminals in a
    /// 32x16 frame. The leaves are visited left-to-right at x=0 and x=4 MI.
    TwoHorizontal,
    /// A level-2 SPLIT followed by two level-3 NONE B16x16 terminals in a
    /// 16x32 frame. The leaves are visited top-to-bottom at y=0 and y=4 MI.
    TwoVertical,
    /// A level-2 SPLIT followed by four level-3 NONE B16x16 terminals in a
    /// 32x32 frame. The leaves are visited in raster order.
    FourSquare,
    /// A level-1 SPLIT followed by four level-2 splits and sixteen level-3
    /// NONE B16x16 terminals in AV1's depth-first quadrant order.
    SixteenSquare,
    /// A level-0 root with two 32x32 quadrants across eight level-3 leaves.
    EightHorizontal,
    /// A level-0 root with two 32x32 quadrants down eight level-3 leaves.
    EightVertical,
}

impl BoundedI444InterGeometry {
    const fn dimensions(self) -> (u32, u32) {
        match self {
            Self::OneBlock => (16, 16),
            Self::TwoHorizontal => (32, 16),
            Self::TwoVertical => (16, 32),
            Self::FourSquare => (32, 32),
            Self::SixteenSquare => (64, 64),
            Self::EightHorizontal => (64, 32),
            Self::EightVertical => (32, 64),
        }
    }

    const fn expected_leaf_count(self) -> usize {
        match self {
            Self::OneBlock => 1,
            Self::TwoHorizontal | Self::TwoVertical => 2,
            Self::FourSquare => 4,
            Self::SixteenSquare => 16,
            Self::EightHorizontal | Self::EightVertical => 8,
        }
    }

    const fn restoration_supported(self) -> bool {
        !matches!(self, Self::EightVertical | Self::SixteenSquare)
    }
}

/// Validate one exact terminal in the bounded full-resolution inter profiles.
///
/// `PartitionWalker` only calls the block callback for terminal footprints. For
/// the rectangular profiles, the frame edge makes level 2 a one-axis node, so
/// the only alternatives are SPLIT or the corresponding HORIZONTAL/VERTICAL
/// terminal. Requiring the two B16x16 footprints below therefore forces the
/// normative level-2 SPLIT and level-3 NONE sequence before any block syntax is
/// consumed.
fn bounded_i444_expected_terminal(
    geometry: BoundedI444InterGeometry,
    leaf_index: usize,
    node: PartitionNode,
) -> bool {
    let expected = match geometry {
        BoundedI444InterGeometry::OneBlock => (0, 0),
        BoundedI444InterGeometry::TwoHorizontal => match leaf_index {
            0 => (0, 0),
            1 => (4, 0),
            _ => return false,
        },
        BoundedI444InterGeometry::TwoVertical => match leaf_index {
            0 => (0, 0),
            1 => (0, 4),
            _ => return false,
        },
        BoundedI444InterGeometry::FourSquare => match leaf_index {
            0 => (0, 0),
            1 => (4, 0),
            2 => (0, 4),
            3 => (4, 4),
            _ => return false,
        },
        BoundedI444InterGeometry::SixteenSquare => match leaf_index {
            0 => (0, 0),
            1 => (4, 0),
            2 => (0, 4),
            3 => (4, 4),
            4 => (8, 0),
            5 => (12, 0),
            6 => (8, 4),
            7 => (12, 4),
            8 => (0, 8),
            9 => (4, 8),
            10 => (0, 12),
            11 => (4, 12),
            12 => (8, 8),
            13 => (12, 8),
            14 => (8, 12),
            15 => (12, 12),
            _ => return false,
        },
        BoundedI444InterGeometry::EightHorizontal => match leaf_index {
            0 => (0, 0),
            1 => (4, 0),
            2 => (0, 4),
            3 => (4, 4),
            4 => (8, 0),
            5 => (12, 0),
            6 => (8, 4),
            7 => (12, 4),
            _ => return false,
        },
        BoundedI444InterGeometry::EightVertical => match leaf_index {
            0 => (0, 0),
            1 => (4, 0),
            2 => (0, 4),
            3 => (4, 4),
            4 => (0, 8),
            5 => (4, 8),
            6 => (0, 12),
            7 => (4, 12),
            _ => return false,
        },
    };
    node.level == 3
        && node.x == expected.0
        && node.y == expected.1
        && node.coded_width == 4
        && node.coded_height == 4
        && node.width == 4
        && node.height == 4
        && node.block_size == BlockSize::B16x16
        && node.kind == PartitionKind::None
}

/// Exact bounded 4:4:4 inter tranche admitted by the full-resolution
/// translation path. The inter block engine carries depth-parametric
/// predictors and residuals for all three full-resolution planes, including
/// the average compound predictor. Keep each admitted geometry as an explicit
/// profile so a clipped partition cannot inherit the narrower 4:2:0/4:2:2
/// admission or skip child-local context publication.
fn bounded_i444_geometry_for_context(
    context: &FirstBlockContext,
) -> Option<BoundedI444InterGeometry> {
    match (
        context.frame_width,
        context.frame_height,
        context.block_width,
        context.block_height,
        context.level,
    ) {
        (16, 16, 4, 4, 0 | 1) => Some(BoundedI444InterGeometry::OneBlock),
        (32, 16, 8, 4, 1) => Some(BoundedI444InterGeometry::TwoHorizontal),
        (16, 32, 4, 8, 1) => Some(BoundedI444InterGeometry::TwoVertical),
        (32, 32, 8, 8, 1) => Some(BoundedI444InterGeometry::FourSquare),
        (64, 64, 16, 16, 0) => Some(BoundedI444InterGeometry::SixteenSquare),
        (64, 32, 16, 8, 0) => Some(BoundedI444InterGeometry::EightHorizontal),
        (32, 64, 8, 16, 0) => Some(BoundedI444InterGeometry::EightVertical),
        _ => None,
    }
}

fn bounded_i444_film_grain_supported(context: &FirstBlockContext) -> bool {
    !context.frame_tools.film_grain_present
        || (context.bit_depth == 8
            && matches!(
                (context.frame_width, context.frame_height),
                (16, 16) | (32, 16) | (16, 32)
            ))
}

/// Exact bounded 4:4:4 intra tranche with one Wiener/SGR unit per plane.
///
/// The generic intra block engine already carries full-resolution spatial
/// edges, mode CDFs, and coefficient state for the three normalized geometries
/// returned by `bounded_i444_geometry_for_context`. Keep this admission
/// separate from the inter restoration predicate so an intra tile never
/// inherits reference or motion requirements.
fn bounded_i444_intra_restoration_geometry(
    context: &FirstBlockContext,
) -> Option<BoundedI444InterGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_i444_geometry_for_context(context)?;
    (context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && !context.subsampling_x
        && !context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.upscaled_width == context.frame_width
        && context.block_x == 0
        && context.block_y == 0
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && bounded_i444_film_grain_supported(context)
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && (bounded_i444_loop_filter_supported(context, geometry)
            || bounded_i444_loop_filter_inactive(context))
        && bounded_i444_cdef_supported(context)
        && context.frame_tools.restoration_present
        && context.restoration_types.iter().any(Option::is_some)
        && context.restoration_types.iter().all(|restoration_type| {
            restoration_type.is_none_or(|kind| {
                matches!(
                    kind,
                    RestorationType::Wiener | RestorationType::SgrProjection
                )
            })
        })
        && bounded_i444_restoration_units_supported(context, geometry))
    .then_some(geometry)
}

/// Exact bounded high-depth 4:4:4 intra tranche with CDEF enabled and the
/// later restoration stage disabled. The geometry is shared with the bounded
/// inter/restoration profiles so the terminal sequence and CDEF region maps
/// remain one-to-one for both rectangular two-leaf shapes.
fn bounded_i444_intra_cdef_geometry(
    context: &FirstBlockContext,
) -> Option<BoundedI444InterGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_i444_geometry_for_context(context)?;
    (context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && !context.subsampling_x
        && !context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.upscaled_width == context.frame_width
        && context.block_x == 0
        && context.block_y == 0
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && context
            .frame_tools
            .cdef
            .is_some_and(|_| bounded_i444_cdef_supported(context)))
    .then_some(geometry)
}

/// Exact bounded high-depth 4:4:4 intra tranche with luma deblocking on the
/// internal edge of a rectangular two-leaf frame. Chroma loop levels remain
/// zero until their full-resolution edge ownership is independently proven.
fn bounded_i444_intra_loop_geometry(
    context: &FirstBlockContext,
) -> Option<BoundedI444InterGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_i444_geometry_for_context(context)?;
    (context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && !context.subsampling_x
        && !context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.upscaled_width == context.frame_width
        && context.block_x == 0
        && context.block_y == 0
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && bounded_i444_loop_filter_supported(context, geometry)
        && context.frame_tools.cdef.is_none()
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3])
        .then_some(geometry)
}

fn bounded_i444_inter_reconstruction_geometry(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> Option<BoundedI444InterGeometry> {
    let Some(quantization) = context.frame_tools.quantization else {
        return None;
    };
    let geometry = bounded_i444_geometry_for_context(context)?;
    let (reference_width, reference_height) = geometry.dimensions();
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I444
            && reference.surface.coded_width == reference_width
            && reference.surface.upscaled_width == reference_width
            && reference.surface.frame_height == reference_height
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
            && !reference.scale.scaled
    });
    (!context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && !context.subsampling_x
        && !context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.upscaled_width == context.frame_width
        && context.block_x == 0
        && context.block_y == 0
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && bounded_i444_film_grain_supported(context)
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && (bounded_i444_loop_filter_supported(context, geometry)
            || bounded_i444_loop_filter_inactive(context))
        && bounded_i444_cdef_supported(context)
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.allow_warped_motion
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match)
        .then_some(geometry)
}

fn complete_bounded_i444_restoration_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    let Some(geometry) = bounded_i444_geometry_for_context(context) else {
        return false;
    };
    let (reference_width, reference_height) = geometry.dimensions();
    let references_match = inter_context.references.iter().all(|reference| {
        reference.surface.validate().is_ok()
            && reference.surface.depth.bits() == context.bit_depth
            && reference.surface.layout == PixelLayout::I444
            && reference.surface.coded_width == reference_width
            && reference.surface.upscaled_width == reference_width
            && reference.surface.frame_height == reference_height
            && matches!(
                reference.global_motion.kind,
                GlobalMotionType::Identity | GlobalMotionType::Translation
            )
            && !reference.scale.scaled
    });
    !context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && !context.subsampling_x
        && !context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.upscaled_width == context.frame_width
        && context.block_x == 0
        && context.block_y == 0
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && bounded_i444_film_grain_supported(context)
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && (bounded_i444_loop_filter_supported(context, geometry)
            || bounded_i444_loop_filter_inactive(context))
        && bounded_i444_cdef_supported(context)
        && context.frame_tools.restoration_present
        && context.restoration_types.iter().any(Option::is_some)
        && context.restoration_types.iter().all(|restoration_type| {
            restoration_type.is_none_or(|kind| {
                matches!(
                    kind,
                    RestorationType::Wiener | RestorationType::SgrProjection
                )
            })
        })
        && bounded_i444_restoration_units_supported(context, geometry)
        && !inter_context.use_ref_frame_mvs
        && !inter_context.motion_mode_switchable
        && !inter_context.enable_interintra_compound
        && !inter_context.enable_masked_compound
        && !inter_context.enable_jnt_comp
        && references_match
}

fn bounded_i444_restoration_units_supported(
    context: &FirstBlockContext,
    geometry: BoundedI444InterGeometry,
) -> bool {
    if !geometry.restoration_supported() {
        return false;
    }
    let unit_log2 = context.restoration_unit_size_log2;
    if unit_log2[0] != unit_log2[1]
        || !match context.level {
            0 => (7..=8).contains(&unit_log2[0]),
            1 => (6..=8).contains(&unit_log2[0]),
            _ => false,
        }
    {
        return false;
    }
    let Some(unit_size) = 1_u32.checked_shl(unit_log2[0]) else {
        return false;
    };
    let (width, height) = geometry.dimensions();
    let Some(width_with_half) = width.checked_add(unit_size / 2) else {
        return false;
    };
    let Some(height_with_half) = height.checked_add(unit_size / 2) else {
        return false;
    };
    let units_x = (width_with_half >> unit_log2[0]).max(1);
    let units_y = (height_with_half >> unit_log2[0]).max(1);
    units_x == 1 && units_y == 1
}

fn bounded_i444_loop_filter_supported(
    context: &FirstBlockContext,
    geometry: BoundedI444InterGeometry,
) -> bool {
    let loop_filter = context.frame_tools.loop_filter;
    if loop_filter.sharpness > 7
        || loop_filter.level_y.iter().any(|&level| level > 63)
        || loop_filter.level_u > 63
        || loop_filter.level_v > 63
    {
        return false;
    }
    match geometry {
        BoundedI444InterGeometry::OneBlock => {
            loop_filter.level_y == [0; 2] && loop_filter.level_u == 0 && loop_filter.level_v == 0
        }
        BoundedI444InterGeometry::TwoHorizontal => {
            loop_filter.level_y[0] != 0 && loop_filter.level_u == 0 && loop_filter.level_v == 0
        }
        BoundedI444InterGeometry::TwoVertical => {
            loop_filter.level_y[1] != 0 && loop_filter.level_u == 0 && loop_filter.level_v == 0
        }
        BoundedI444InterGeometry::FourSquare => {
            loop_filter.level_y != [0; 2] && loop_filter.level_u == 0 && loop_filter.level_v == 0
        }
        BoundedI444InterGeometry::SixteenSquare => {
            loop_filter.level_y != [0; 2] && loop_filter.level_u == 0 && loop_filter.level_v == 0
        }
        BoundedI444InterGeometry::EightHorizontal | BoundedI444InterGeometry::EightVertical => {
            loop_filter.level_y != [0; 2] && loop_filter.level_u == 0 && loop_filter.level_v == 0
        }
    }
}

fn bounded_i444_loop_filter_inactive(context: &FirstBlockContext) -> bool {
    let loop_filter = context.frame_tools.loop_filter;
    loop_filter.level_y == [0; 2] && loop_filter.level_u == 0 && loop_filter.level_v == 0
}

fn bounded_i444_cdef_supported(context: &FirstBlockContext) -> bool {
    context.frame_tools.cdef.is_none_or(|cdef| {
        let count = match cdef.bits {
            0 => 1,
            1 => 2,
            2 => 4,
            _ => return false,
        };
        (3..=6).contains(&cdef.damping)
            && cdef.y_strength_count == count
            && cdef.uv_strength_count == count
            && cdef.first_y_strength == cdef.y_strengths.first().copied()
            && cdef.first_uv_strength == cdef.uv_strengths.first().copied()
            && cdef.y_strengths[..count]
                .iter()
                .all(|&strength| strength <= 63)
            && cdef.uv_strengths[..count]
                .iter()
                .all(|&strength| strength <= 63)
    })
}

/// Common admission for the first luma-only monochrome lossy tranche.
///
/// A monochrome sequence still carries the canonical `subsampling_x/y` bits
/// in AV1C, but it has no U/V block syntax or post-filter planes. Keep this
/// profile deliberately narrow until those independent carriers are wired:
/// one tile, largest-transform blocks, no active filtering, and no frame-level
/// state that would require a second plane or a separate publication path.
fn complete_monochrome_lossy_common(context: &FirstBlockContext) -> bool {
    complete_monochrome_lossy_base(context)
        && complete_monochrome_cdef_inactive(context)
        && context.restoration_types == [None; 3]
}

fn complete_monochrome_lossy_base(context: &FirstBlockContext) -> bool {
    let dimensions_are_supported = context.frame_width >= 4
        && context.frame_height >= 4
        && context.frame_width <= 64
        && context.frame_height <= 64
        && context.frame_width.is_multiple_of(4)
        && context.frame_height.is_multiple_of(4)
        && context.block_width == context.frame_width / 4
        && context.block_height == context.frame_height / 4
        && context.upscaled_width == context.frame_width;
    context.single_tile
        && context.monochrome
        && matches!(context.bit_depth, 8 | 10 | 12)
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && context.frame_tools.transform_mode == 1
        && context
            .frame_tools
            .quantization
            .is_some_and(|quantization| !quantization.using_matrix)
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && !context.frame_tools.film_grain_present
        && context.block_x == 0
        && context.block_y == 0
        && context.level == 1
        && dimensions_are_supported
}

fn complete_monochrome_cdef_inactive(context: &FirstBlockContext) -> bool {
    let cdef_is_inactive = context.frame_tools.cdef.is_none_or(|cdef| {
        cdef.bits == 0
            && cdef.y_strength_count == 1
            && cdef.first_y_strength == Some(0)
            && cdef.y_strengths[0] == 0
            && cdef.uv_strength_count == 0
            && cdef.first_uv_strength.is_none()
    });
    cdef_is_inactive
}

fn complete_monochrome_cdef_supported(context: &FirstBlockContext) -> bool {
    context.frame_tools.cdef.is_none_or(|cdef| {
        let expected_count = match cdef.bits {
            0 => 1,
            1 => 2,
            2 => 4,
            _ => return false,
        };
        cdef.damping == cdef.damping.clamp(3, 6)
            && cdef.y_strength_count == expected_count
            && cdef.first_y_strength.is_some()
            && cdef.uv_strength_count == 0
            && cdef.first_uv_strength.is_none()
    })
}

fn complete_monochrome_restoration_supported(context: &FirstBlockContext) -> bool {
    if context.restoration_types[1].is_some() || context.restoration_types[2].is_some() {
        return false;
    }
    let Some(restoration_type) = context.restoration_types[0] else {
        return true;
    };
    if !matches!(
        restoration_type,
        RestorationType::Wiener | RestorationType::SgrProjection
    ) || !(6..=8).contains(&context.restoration_unit_size_log2[0])
        || context.frame_height > 56
    {
        return false;
    }
    let Some(unit_size) = 1_u32.checked_shl(context.restoration_unit_size_log2[0]) else {
        return false;
    };
    let Some(width_with_half) = context.frame_width.checked_add(unit_size / 2) else {
        return false;
    };
    let Some(height_with_half) = context.frame_height.checked_add(unit_size / 2) else {
        return false;
    };
    (width_with_half >> context.restoration_unit_size_log2[0]).max(1) == 1
        && (height_with_half >> context.restoration_unit_size_log2[0]).max(1) == 1
}

fn complete_monochrome_postfilter_reconstruction_context(context: &FirstBlockContext) -> bool {
    complete_monochrome_lossy_base(context)
        && context.frame_width >= 8
        && context.frame_height >= 8
        && context.frame_width.is_multiple_of(8)
        && context.frame_height.is_multiple_of(8)
        && complete_monochrome_cdef_supported(context)
        && complete_monochrome_restoration_supported(context)
}

fn complete_monochrome_lossy_intra_reconstruction_context(context: &FirstBlockContext) -> bool {
    context.intra_frame && complete_monochrome_lossy_common(context)
}

/// Luma-only inter admission. Block-level parsing still consumes the normal
/// reference/MV sentence and rejects intra, compound, warped, or unsupported
/// transform branches; the frame gate only proves that a successful leaf can
/// be published as a persistent monochrome surface.
fn complete_monochrome_lossy_inter_reconstruction_context(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> bool {
    !context.intra_frame
        && complete_monochrome_lossy_common(context)
        && !inter_context.reference_mode_select
        && complete_monochrome_references(context, inter_context)
}

fn complete_monochrome_references(
    context: &FirstBlockContext,
    inter_context: &InterFrameContext<'_>,
) -> bool {
    inter_context.references.iter().all(|reference| {
        reference.surface.layout == PixelLayout::Monochrome
            && reference.surface.depth.bits() == context.bit_depth
    })
}

/// Color all-lossless frames share the canonical streamed coefficient state
/// with lossy intra. Keeping this gate beside the lossy admission makes the
/// tile walker, context publication, and private reconstruction raster common
/// across all 22 block sizes without mixing the legacy lossless CDF copies.
fn complete_streamed_lossless_color_context(context: &FirstBlockContext) -> bool {
    let layout_supported = context.subsampling_x || !context.subsampling_y;
    let padded_block_width = (context.frame_width.saturating_add(7) >> 3).wrapping_shl(1);
    let padded_block_height = (context.frame_height.saturating_add(7) >> 3).wrapping_shl(1);
    let dimensions_are_supported = context.frame_width != 0
        && context.frame_height != 0
        && context.block_width == padded_block_width
        && context.block_height == padded_block_height
        && context.upscaled_width == context.frame_width;
    context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && context.all_lossless
        && context.frame_tools.segment_lossless
        && !context.monochrome
        && layout_supported
        && !context.superres_enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.restoration_present
        && no_unsupported_film_grain(context)
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context.frame_tools.cdef.is_none()
        && context.restoration_types == [None; 3]
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1)
        && dimensions_are_supported
}

/// Exact high-depth Full-resolution tranche admitted by the generic unsplit
/// reconstruction core. Optional post-filters and large implicit transform
/// tilings stay closed until their high-depth arithmetic/state is connected;
/// this gate therefore cannot silently route them through eight-bit code.
fn complete_high_depth_full_reconstruction_context(context: &FirstBlockContext) -> bool {
    context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && !context.superres_enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.monochrome
        && !context.subsampling_x
        && !context.subsampling_y
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.reduced_transform_set
        && !context
            .frame_tools
            .quantization
            .is_some_and(|quantization| quantization.using_matrix)
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context.frame_tools.cdef.is_none()
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && context.block_x == 0
        && context.block_y == 0
}

/// Exact high-depth 4:2:0 tranche admitted by the normalized reconstruction
/// core. The older geometry-specific 4:2:0 reconstructors narrow samples to
/// eight-bit arithmetic, so high-depth I420 must remain on the generic
/// edge-aware path until those carriers are depth-parametric. Keep the same
/// conservative tool profile as the high-depth 4:4:4 tranche, while rejecting
/// quantization matrices here because the 4:2:0 coefficient path does not yet
/// carry their plane-specific high-depth proof.
fn complete_high_depth_420_reconstruction_context(context: &FirstBlockContext) -> bool {
    context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && !context.superres_enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.monochrome
        && context.subsampling_x
        && context.subsampling_y
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.reduced_transform_set
        && !context
            .frame_tools
            .quantization
            .is_some_and(|quantization| quantization.using_matrix)
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context.frame_tools.cdef.is_none()
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && context.block_x == 0
        && context.block_y == 0
}

/// Exact high-depth 4:2:2 intra tranche admitted by the normalized streamed
/// leaf.  The live walker has complete coefficient and predictor carriers for
/// one B16x16/Square16 terminal (Y 16x16, U/V 8x16); other partition or
/// transform geometries remain outside this proof until their independent
/// context grids are wired.
fn complete_high_depth_422_intra_reconstruction_context(context: &FirstBlockContext) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };
    context.intra_frame
        && matches!(context.bit_depth, 10 | 12)
        && context.subsampling_x
        && !context.subsampling_y
        && !context.monochrome
        && context.single_tile
        && context.frame_width == 16
        && context.frame_height == 16
        && context.upscaled_width == 16
        && context.block_width == 4
        && context.block_height == 4
        && context.block_x == 0
        && context.block_y == 0
        && !context.superres_enabled
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && !context.frame_tools.film_grain_present
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && quantization.base != 0
        && !quantization.using_matrix
        && !context.frame_tools.reduced_transform_set
        && context.frame_tools.transform_mode == 1
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context.frame_tools.cdef.is_none()
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
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
        for block_y in y / 8..end_y.div_ceil(8) {
            for block_x in x / 8..end_x.div_ceil(8) {
                let index = block_y
                    .checked_mul(active_width)
                    .and_then(|row| row.checked_add(block_x))
                    .ok_or_else(|| malformed("CDEF active-map index overflows"))?;
                let Some(slot) = cdef_active.get_mut(index) else {
                    return Err(malformed("CDEF active-map index exceeds its frame"));
                };
                *slot = true;
            }
        }

        for region_y in y / 64..end_y.div_ceil(64) {
            for region_x in x / 64..end_x.div_ceil(64) {
                let index = region_y
                    .checked_mul(region_width)
                    .and_then(|row| row.checked_add(region_x))
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
    (loop_filter.level_y != [0, 0] || loop_filter.level_u != 0 || loop_filter.level_v != 0)
        .then_some(super::filter::Parameters {
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
    let mut walker = PartitionWalker::new(&mut decoder, context)?;
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
                sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                    .ok_or_else(|| malformed("AV1 block sample depth is unsupported"))?,
                allow_screen_content_tools: context.allow_screen_content_tools,
                enable_filter_intra: context.enable_filter_intra,
                enable_intra_edge_filter: context.enable_intra_edge_filter,
                transform_mode: context.frame_tools.transform_mode,
                transform_context: 0,
                skip_context: 0,
                suppress_delta_q_when_skipped: false,
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
        block_skipped: false,
        planes,
        luma_predictor: super::block::LumaPredictor::Dc,
        chroma_predictor: None,
        luma_context: 0x40,
        chroma_contexts: [0x40; 2],
        chroma_right_contexts: [[0x40; 16]; 2],
        chroma_bottom_contexts: [[0x40; 16]; 2],
        tx_context_width: 0,
        tx_context_height: 0,
        luma_transform_split: false,
        luma_right_contexts: [0x40; 16],
        luma_bottom_contexts: [0x40; 16],
        wide_coefficient_contexts: None,
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
    context.intra_frame
        && matches!(context.bit_depth, 8 | 10 | 12)
        && !context.superres_enabled
        && !context.segmentation_enabled
        && !context.skip_mode_enabled
        && !context.monochrome
        && !context.subsampling_x
        && !context.subsampling_y
        && !context.allow_intrabc
        && !context.allow_screen_content_tools
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1)
        && dimensions_are_supported
        && context.all_lossless
        && context.restoration_types == [None; 3]
        && (!context.frame_tools.film_grain_present
            || exact_lossless_i444_film_grain_profile(context))
}

fn exact_lossless_i444_film_grain_profile(context: &FirstBlockContext) -> bool {
    context.bit_depth == 8
        && context.frame_width == 16
        && context.frame_height == 16
        && context.upscaled_width == 16
        && context.block_width == 4
        && context.block_height == 4
        && context.block_x == 0
        && context.block_y == 0
        && matches!(context.level, 0 | 1)
        && context.single_tile
        && !context.frame_tools.segmentation.enabled
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && context.frame_tools.cdef.is_none()
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && !context.frame_tools.restoration_present
}

fn lossy_quantization_for_context(
    context: &FirstBlockContext,
) -> Av1Result<super::block::LossyQuantization> {
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
    let sample_depth = super::sample_depth::SampleDepth::new(context.bit_depth)
        .ok_or_else(|| malformed("AV1 lossy sample depth is unsupported"))?;
    Ok(super::block::LossyQuantization {
        sample_depth,
        qindex: frame_quantization.base,
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
        reduced_transform_set: context.frame_tools.reduced_transform_set,
        segment_qindex: context.frame_tools.segment_qindex,
        segment_lossless: context.frame_tools.segment_lossless,
    })
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
    context.intra_frame
        & (context.bit_depth == 8)
        & !context.superres_enabled
        & !context.skip_mode_enabled
        & !context.allow_intrabc
        & !context.monochrome
        & no_unsupported_film_grain(context)
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
    context.intra_frame
        & (context.bit_depth == 8)
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
    segmentation: SegmentationContext {
        segments: [SegmentContext {
            qindex: 4,
            ..SegmentContext::EMPTY
        }; 8],
        ..SegmentationContext::DISABLED
    },
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
    let complete = context.intra_frame
        && context.bit_depth == 8
        && !context.superres_enabled
        && !context.skip_mode_enabled
        && !context.allow_intrabc
        && !context.monochrome
        && !context.subsampling_x
        && !context.subsampling_y
        && context.block_x == 0
        && context.block_y == 0
        && !context.all_lossless
        && !context.segmentation_enabled
        && !context.frame_tools.segmentation.enabled
        && !context.frame_tools.delta_q_present
        && !context.frame_tools.delta_lf_present
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && context.frame_tools.cdef.is_none()
        && context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context.frame_tools.transform_mode == 1
        && !context.frame_tools.reduced_transform_set
        && !quantization.using_matrix
        && quantization.base != 0
        && !context.frame_tools.segment_lossless
        && context.frame_tools.segment_qindex == quantization.base
        && matches!(context.level, 0 | 1);
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
    complete && geometry && bounded_i444_film_grain_supported(context)
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
    let transform_state = matches!(context.frame_tools.transform_mode, 1 | 2)
        && !context.frame_tools.reduced_transform_set;

    base_context
        && geometry
        && sequence_tools
        && quantization_state
        && tile_delta_state
        && no_effective_filters
        && transform_state
}

fn closed_lossy_420_qcat2_horizontal_four_context(context: &FirstBlockContext) -> bool {
    let Some(quantization) = context.frame_tools.quantization else {
        return false;
    };

    // This is the exact qcat-two frame-tools class exercised by the pinned
    // predictor-enabled H16x4 witness.  Keeping the admission predicate
    // geometry- and matrix-specific prevents the narrow H4 decoder from
    // consuming arbitrary qcat-two rectangular streams before their CDF and
    // reconstruction coverage is independently established.
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
    let quantization_state = (61..=120).contains(&quantization.base)
        && context.frame_tools.segment_qindex == quantization.base
        && !context.frame_tools.segment_lossless
        && quantization.y_dc_delta == 0
        && quantization.u_dc_delta == -16
        && quantization.u_ac_delta == -16
        && quantization.v_dc_delta == -16
        && quantization.v_ac_delta == -16
        && quantization.using_matrix
        && quantization.matrix_y == 9
        && quantization.matrix_u == 9
        && quantization.matrix_v == 9;
    let tile_delta_state =
        !context.frame_tools.delta_q_present && !context.frame_tools.delta_lf_present;
    let no_effective_filters = context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context.frame_tools.cdef.is_none()
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && context.restoration_unit_size_log2 == [8; 2];
    let transform_state = matches!(context.frame_tools.transform_mode, 1 | 2)
        && !context.frame_tools.reduced_transform_set;

    base_context
        && geometry
        && sequence_tools
        && quantization_state
        && tile_delta_state
        && no_effective_filters
        && transform_state
}

fn closed_lossy_420_qcat3_horizontal_rect_context(context: &FirstBlockContext) -> bool {
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
        && context.block_height == 2
        && context.block_x == 0
        && context.block_y == 0
        && context.frame_width == 16
        && context.frame_height == 8
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
        && quantization.matrix_y == 7
        && quantization.matrix_u == 8
        && quantization.matrix_v == 8;
    let tile_delta_state =
        !context.frame_tools.delta_q_present && !context.frame_tools.delta_lf_present;
    let no_effective_filters = context.frame_tools.loop_filter.level_y == [0; 2]
        && context.frame_tools.loop_filter.level_u == 0
        && context.frame_tools.loop_filter.level_v == 0
        && context.frame_tools.cdef.is_none()
        && !context.frame_tools.restoration_present
        && context.restoration_types == [None; 3]
        && context.restoration_unit_size_log2 == [8; 2];
    let transform_state = matches!(context.frame_tools.transform_mode, 1 | 2)
        && !context.frame_tools.reduced_transform_set;

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
    // existing exact class and add only the independently evidenced qcat-two
    // and qcat-three frame-tools classes for predictor-enabled 16x16 witnesses.
    let legacy_exact = closed_lossy_420_frame_context_with_delta_q(context, false)
        && (context.frame_width == 16)
        && (context.frame_height == 16);
    legacy_exact
        || closed_lossy_420_qcat2_horizontal_four_context(context)
        || closed_lossy_420_qcat3_horizontal_four_context(context)
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
            sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                .ok_or_else(|| malformed("AV1 block sample depth is unsupported"))?,
            allow_screen_content_tools: context.allow_screen_content_tools,
            enable_filter_intra: context.enable_filter_intra,
            enable_intra_edge_filter: context.enable_intra_edge_filter,
            transform_mode: context.frame_tools.transform_mode,
            transform_context: 0,
            skip_context: 0,
            suppress_delta_q_when_skipped: false,
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
            sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                .ok_or_else(|| malformed("AV1 block sample depth is unsupported"))?,
            allow_screen_content_tools: context.allow_screen_content_tools,
            enable_filter_intra: context.enable_filter_intra,
            enable_intra_edge_filter: context.enable_intra_edge_filter,
            transform_mode: context.frame_tools.transform_mode,
            transform_context: 0,
            skip_context: 0,
            suppress_delta_q_when_skipped: false,
            palette_context: Default::default(),
        },
    );
    finish_closed_leaf(decoder, reconstructed)
}

fn decode_closed_lossy_420_leaf(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    context: &FirstBlockContext,
) -> Av1Result<Option<super::block::FirstLeaf>> {
    let quantization = lossy_quantization_for_context(context)?;
    let reconstructed = super::block::decode_first_lossy_420_leaf(
        decoder,
        context.frame_width,
        context.frame_height,
        quantization,
        super::block::BlockTools {
            sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                .ok_or_else(|| malformed("AV1 block sample depth is unsupported"))?,
            allow_screen_content_tools: context.allow_screen_content_tools,
            enable_filter_intra: context.enable_filter_intra,
            enable_intra_edge_filter: context.enable_intra_edge_filter,
            transform_mode: context.frame_tools.transform_mode,
            transform_context: 0,
            skip_context: 0,
            suppress_delta_q_when_skipped: false,
            palette_context: Default::default(),
        },
    );
    finish_closed_leaf(decoder, reconstructed)
}

fn decode_closed_lossy_444_16x16_leaf(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    context: &FirstBlockContext,
) -> Av1Result<Option<super::block::FirstLeaf>> {
    let quantization = lossy_quantization_for_context(context)?;
    let reconstructed = super::block::decode_first_lossy_444_16x16_leaf(
        decoder,
        quantization,
        super::block::BlockTools {
            sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                .ok_or_else(|| malformed("AV1 block sample depth is unsupported"))?,
            allow_screen_content_tools: context.allow_screen_content_tools,
            enable_filter_intra: context.enable_filter_intra,
            enable_intra_edge_filter: context.enable_intra_edge_filter,
            transform_mode: context.frame_tools.transform_mode,
            transform_context: 0,
            skip_context: 0,
            suppress_delta_q_when_skipped: false,
            palette_context: Default::default(),
        },
    );
    finish_closed_leaf(decoder, reconstructed)
}

fn finish_closed_leaf(
    _decoder: &RangeDecoder<'_, '_, '_>,
    reconstructed: super::block::PortableResult<super::block::FirstLeaf>,
) -> Av1Result<Option<super::block::FirstLeaf>> {
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
    // Inter/switch frames require retained reference surfaces, inherited CDFs,
    // motion fields, and inter prediction. In particular, primary-ref-none is
    // legal for error-resilient inter frames, so that header field alone is
    // not an intra admission test. Reject before consuming tile entropy.
    if !context.intra_frame {
        return Ok(None);
    }
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
                            sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                                .ok_or_else(|| {
                                    malformed("AV1 block sample depth is unsupported")
                                })?,
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            skip_context: 0,
                            suppress_delta_q_when_skipped: false,
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
                sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                    .ok_or_else(|| malformed("AV1 block sample depth is unsupported"))?,
                allow_screen_content_tools: context.allow_screen_content_tools,
                enable_filter_intra: context.enable_filter_intra,
                enable_intra_edge_filter: context.enable_intra_edge_filter,
                transform_mode: context.frame_tools.transform_mode,
                transform_context: 0,
                skip_context: 0,
                suppress_delta_q_when_skipped: false,
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
        || closed_lossy_420_qcat3_horizontal_rect_context(context)
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
            | closed_lossy_420_qcat3_horizontal_rect_context(context)
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
                            sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                                .ok_or_else(|| {
                                    malformed("AV1 block sample depth is unsupported")
                                })?,
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            skip_context: 0,
                            suppress_delta_q_when_skipped: false,
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
                            sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                                .ok_or_else(|| {
                                    malformed("AV1 block sample depth is unsupported")
                                })?,
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            skip_context: 0,
                            suppress_delta_q_when_skipped: false,
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
                    let quantization = lossy_quantization_for_context(context)?;
                    let reconstructed = super::block::decode_four_lossy_420_leaves(
                        &mut decoder,
                        context.frame_width,
                        context.frame_height,
                        quantization,
                        super::block::BlockTools {
                            sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                                .ok_or_else(|| {
                                    malformed("AV1 block sample depth is unsupported")
                                })?,
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            skip_context: 0,
                            suppress_delta_q_when_skipped: false,
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
                    let quantization = lossy_quantization_for_context(context)?;
                    let reconstructed = super::block::decode_two_lossy_420_leaves(
                        &mut decoder,
                        context.frame_width,
                        context.frame_height,
                        quantization,
                        super::block::BlockTools {
                            sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                                .ok_or_else(|| {
                                    malformed("AV1 block sample depth is unsupported")
                                })?,
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            skip_context: 0,
                            suppress_delta_q_when_skipped: false,
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
                    let quantization = lossy_quantization_for_context(context)?;
                    let reconstructed = super::block::decode_four_lossy_420_horizontal_leaves(
                        &mut decoder,
                        context.frame_width,
                        context.frame_height,
                        quantization,
                        super::block::BlockTools {
                            sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                                .ok_or_else(|| {
                                    malformed("AV1 block sample depth is unsupported")
                                })?,
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            skip_context: 0,
                            suppress_delta_q_when_skipped: false,
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
                let reconstruct_lossy_420_qcat3_horizontal_rectangular_leaf = !split
                    & closed_lossy_420_qcat3_horizontal_rect_context(context)
                    & horizontal_split
                    & (level == 3);
                if reconstruct_lossy_420_qcat3_horizontal_rectangular_leaf {
                    let reconstructed = decode_closed_lossy_420_leaf(&mut decoder, context);
                    return reconstructed;
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
                            sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                                .ok_or_else(|| {
                                    malformed("AV1 block sample depth is unsupported")
                                })?,
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            skip_context: 0,
                            suppress_delta_q_when_skipped: false,
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
                            sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                                .ok_or_else(|| {
                                    malformed("AV1 block sample depth is unsupported")
                                })?,
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            skip_context: 0,
                            suppress_delta_q_when_skipped: false,
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
                    let quantization = lossy_quantization_for_context(context)?;
                    let reconstructed = super::block::decode_two_lossy_420_leaves(
                        &mut decoder,
                        context.frame_width,
                        context.frame_height,
                        quantization,
                        super::block::BlockTools {
                            sample_depth: super::sample_depth::SampleDepth::new(context.bit_depth)
                                .ok_or_else(|| {
                                    malformed("AV1 block sample depth is unsupported")
                                })?,
                            allow_screen_content_tools: context.allow_screen_content_tools,
                            enable_filter_intra: context.enable_filter_intra,
                            enable_intra_edge_filter: context.enable_intra_edge_filter,
                            transform_mode: context.frame_tools.transform_mode,
                            transform_context: 0,
                            skip_context: 0,
                            suppress_delta_q_when_skipped: false,
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
        intra_frame: true,
        level: 1,
        block_width: 16,
        block_height: 16,
        block_x: 0,
        block_y: 0,
        tile_origin_b4_x: 0,
        tile_origin_b4_y: 0,
        single_tile: true,
        frame_block_width: 16,
        frame_block_height: 16,
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
            segmentation: SegmentationContext::DISABLED,
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
        let mut walker = PartitionWalker::new(&mut decoder, &context)?;
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
                sample_depth: super::super::sample_depth::SampleDepth::new(8)
                    .ok_or_else(|| malformed("eight-bit alpha sample depth is unsupported"))?,
                allow_screen_content_tools: false,
                enable_filter_intra: true,
                enable_intra_edge_filter: true,
                transform_mode: 0,
                transform_context: 0,
                skip_context: 0,
                suppress_delta_q_when_skipped: false,
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
        let plane = canvas.finish(super::super::sample_depth::SampleDepth::EIGHT)?;
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
                    sample_depth: crate::codecs::avif::av1::sample_depth::SampleDepth::new(8)
                        .ok_or_else(|| {
                            malformed("eight-bit coverage sample depth is unsupported")
                        })?,
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
                    reduced_transform_set: false,
                    segment_qindex: 120,
                    segment_lossless: false,
                },
                crate::codecs::avif::av1::block::BlockTools {
                    sample_depth: crate::codecs::avif::av1::sample_depth::SampleDepth::new(8)
                        .ok_or_else(|| {
                            malformed("eight-bit coverage sample depth is unsupported")
                        })?,
                    allow_screen_content_tools: false,
                    enable_filter_intra: true,
                    enable_intra_edge_filter: true,
                    transform_mode: 1,
                    transform_context: 0,
                    skip_context: 0,
                    suppress_delta_q_when_skipped: false,
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
