//! Frame-wide libwebp-compatible VP8 macroblock decisions.

use super::{
    analysis::{FrameAnalysis, segment_params},
    chroma::{self, ChromaCandidate},
    cost::rd_score,
    intra4::{self, Intra4Mode, Intra4Result},
    intra16::{self, Intra16Candidate, Intra16Mode},
    quant::{SegmentMatrices, libwebp_segment_matrices},
};
use crate::codecs::CodecResult;

#[cfg(coverage)]
use std::sync::atomic::{AtomicBool, Ordering};

const STORED_NONZERO_MASK: u32 = (1 << 3)
    | (1 << 7)
    | (1 << 11)
    | (1 << 12)
    | (1 << 13)
    | (1 << 14)
    | (1 << 15)
    | (1 << 17)
    | (1 << 18)
    | (1 << 19)
    | (1 << 21)
    | (1 << 22)
    | (1 << 23)
    | (1 << 24);
// Each macroblock selection evaluates sixteen luma 4×4 blocks. Intra4 mode
// selection has its own token-only checkpoint after each completed block;
// this outer batch still bounds the remaining intra16/chroma and completed
// macroblock work without adding a callback to their no-token paths.
const SELECTION_CHECKPOINT_MACROBLOCKS: usize = 64;

#[cfg(coverage)]
static FORCE_SELECTION_CHECKPOINT_ERROR: AtomicBool = AtomicBool::new(false);

#[cfg(coverage)]
#[coverage(off)]
fn coverage_cancel_selection_checkpoint(token: Option<&crate::CancellationToken>) {
    if FORCE_SELECTION_CHECKPOINT_ERROR.swap(false, Ordering::Relaxed) {
        if let Some(token) = token {
            token.cancel();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum LumaDecision {
    Intra4(Intra4Result),
    Intra16(Intra16Candidate),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MacroblockDecision {
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) segment: u8,
    /// Best I16 mode, retained even when the macroblock selects I4.
    pub(super) intra16_mode: Intra16Mode,
    pub(super) luma: LumaDecision,
    pub(super) chroma: ChromaCandidate,
    pub(super) distortion: u32,
    pub(super) spectral_distortion: u32,
    pub(super) header_cost: u32,
    pub(super) rate_cost: u32,
    pub(super) score: u64,
    pub(super) nonzero: u32,
}

#[derive(Clone, Copy)]
pub(super) struct FrameSelectionOptions<'a> {
    pub(super) quality: f64,
    pub(super) method: u8,
    pub(super) coefficient_probabilities: &'a [[[[u8; 11]; 3]; 8]; 4],
    pub(super) trellis: bool,
    pub(super) token: Option<&'a crate::CancellationToken>,
}

fn bit(value: u32, index: usize) -> u8 {
    value
        .wrapping_shr(index.to_le_bytes()[0].into())
        .to_le_bytes()[0]
        & 1
}

fn extract_16(plane: &[u8], stride: usize, x: usize, y: usize) -> [u8; 256] {
    let mut block = [0; 256];
    for row in 0_usize..16 {
        let destination_start = row.wrapping_mul(16);
        let source_start = y
            .wrapping_mul(16)
            .wrapping_add(row)
            .wrapping_mul(stride)
            .wrapping_add(x.wrapping_mul(16));
        block[destination_start..destination_start.wrapping_add(16)]
            .copy_from_slice(&plane[source_start..][..16]);
    }
    block
}

fn extract_8(plane: &[u8], stride: usize, x: usize, y: usize) -> [u8; 64] {
    let mut block = [0; 64];
    for row in 0_usize..8 {
        let destination_start = row.wrapping_mul(8);
        let source_start = y
            .wrapping_mul(8)
            .wrapping_add(row)
            .wrapping_mul(stride)
            .wrapping_add(x.wrapping_mul(8));
        block[destination_start..destination_start.wrapping_add(8)]
            .copy_from_slice(&plane[source_start..][..8]);
    }
    block
}

fn intra16_to_intra4(mode: Intra16Mode) -> Intra4Mode {
    match mode {
        Intra16Mode::Dc => Intra4Mode::Dc,
        Intra16Mode::TrueMotion => Intra4Mode::TrueMotion,
        Intra16Mode::Vertical => Intra4Mode::Vertical,
        Intra16Mode::Horizontal => Intra4Mode::Horizontal,
    }
}

fn store_diffusion_errors(errors: [[i8; 3]; 2], top: &mut [[i8; 2]; 2]) -> [[i8; 2]; 2] {
    let mut left = [[0; 2]; 2];
    for plane in 0..2 {
        left[plane][0] = errors[plane][0];
        let weighted = 3_i16
            .wrapping_mul(i16::from(errors[plane][2]))
            .wrapping_shr(2);
        left[plane][1] = i8::from_le_bytes([weighted.to_le_bytes()[0]]);
        top[plane][0] = errors[plane][1];
        top[plane][1] = errors[plane][2].wrapping_sub(left[plane][1]);
    }
    left
}

pub(super) fn select_frame(
    planes: [&[u8]; 3],
    dimensions: (usize, usize),
    analysis: &FrameAnalysis,
    options: FrameSelectionOptions<'_>,
) -> CodecResult<Vec<MacroblockDecision>> {
    let FrameSelectionOptions {
        quality,
        method,
        coefficient_probabilities,
        trellis,
        token,
    } = options;
    let [y_plane, u_plane, v_plane] = planes;
    let (width, height) = dimensions;
    assert_eq!(width % 16, 0);
    assert_eq!(height % 16, 0);
    let macroblock_width = width / 16;
    let macroblock_height = height / 16;
    let chroma_stride = width / 2;
    let params = segment_params(analysis, quality);
    let matrices: [SegmentMatrices; 4] = std::array::from_fn(|segment| {
        libwebp_segment_matrices(
            params.segments[segment].quantizer,
            params.chroma_dc_delta,
            params.chroma_ac_delta,
        )
    });
    let mut y_top = vec![127; width];
    let mut u_top = vec![127; width / 2];
    let mut v_top = vec![127; width / 2];
    let mut packed_nonzero = vec![0u32; macroblock_width];
    let mode_stride = macroblock_width.wrapping_mul(4);
    let mut mode_grid =
        vec![Intra4Mode::Dc; mode_stride.wrapping_mul(macroblock_height).wrapping_mul(4)];
    let mut top_errors = vec![[[0i8; 2]; 2]; macroblock_width];
    let mut decisions = Vec::with_capacity(macroblock_width.wrapping_mul(macroblock_height));
    let mut selection_items = 0usize;

    for macroblock_y in 0..macroblock_height {
        let mut y_left = [129; 16];
        let mut u_left = [129; 8];
        let mut v_left = [129; 8];
        let mut y_top_left = if macroblock_y == 0 { 127 } else { 129 };
        let mut u_top_left = y_top_left;
        let mut v_top_left = y_top_left;
        let mut left_packed = 0u32;
        let mut left_y2_nonzero = 0u8;
        let mut left_errors = [[0i8; 2]; 2];

        for macroblock_x in 0..macroblock_width {
            let block_index = macroblock_y
                .wrapping_mul(macroblock_width)
                .wrapping_add(macroblock_x);
            let segment = analysis.macroblocks[block_index].segment;
            let matrix = &matrices[usize::from(segment)];
            let source_y = extract_16(y_plane, width, macroblock_x, macroblock_y);
            let source_u = extract_8(u_plane, chroma_stride, macroblock_x, macroblock_y);
            let source_v = extract_8(v_plane, chroma_stride, macroblock_x, macroblock_y);
            let y_offset = macroblock_x.wrapping_mul(16);
            let uv_offset = macroblock_x.wrapping_mul(8);
            let top_y = std::array::from_fn(|index| y_top[y_offset.wrapping_add(index)]);
            let top_u = std::array::from_fn(|index| u_top[uv_offset.wrapping_add(index)]);
            let top_v = std::array::from_fn(|index| v_top[uv_offset.wrapping_add(index)]);
            let top_y_i4: [u8; 20] = std::array::from_fn(|index| {
                y_top[y_offset.wrapping_add(index).min(width.saturating_sub(1))]
            });
            let top_packed = packed_nonzero[macroblock_x];
            let top_y_nonzero = [
                bit(top_packed, 12),
                bit(top_packed, 13),
                bit(top_packed, 14),
                bit(top_packed, 15),
            ];
            let left_y_nonzero = [
                bit(left_packed, 3),
                bit(left_packed, 7),
                bit(left_packed, 11),
                bit(left_packed, 15),
            ];
            let top_chroma_nonzero = [
                bit(top_packed, 18),
                bit(top_packed, 19),
                bit(top_packed, 22),
                bit(top_packed, 23),
            ];
            let left_chroma_nonzero = [
                bit(left_packed, 17),
                bit(left_packed, 19),
                bit(left_packed, 21),
                bit(left_packed, 23),
            ];
            let top_modes = std::array::from_fn(|block_x| {
                if macroblock_y == 0 {
                    Intra4Mode::Dc
                } else {
                    let row = macroblock_y.wrapping_mul(4).wrapping_sub(1);
                    mode_grid[row
                        .wrapping_mul(mode_stride)
                        .wrapping_add(macroblock_x.wrapping_mul(4))
                        .wrapping_add(block_x)]
                }
            });
            let neighboring_left_modes = std::array::from_fn(|block_y| {
                if macroblock_x == 0 {
                    Intra4Mode::Dc
                } else {
                    let row = macroblock_y.wrapping_mul(4).wrapping_add(block_y);
                    mode_grid[row
                        .wrapping_mul(mode_stride)
                        .wrapping_add(macroblock_x.wrapping_mul(4))
                        .wrapping_sub(1)]
                }
            });

            let intra16 = intra16::select(
                &source_y,
                &top_y,
                &y_left,
                y_top_left,
                macroblock_y != 0,
                macroblock_x != 0,
                top_y_nonzero,
                left_y_nonzero,
                usize::from(bit(top_packed, 24).wrapping_add(left_y2_nonzero)),
                matrix,
                matrix.lambda_i16.cast_unsigned(),
                matrix.texture_lambda.cast_unsigned(),
                None,
                method <= 1,
                coefficient_probabilities,
                trellis,
            );
            let intra16_mode_score = rd_score(
                intra16.rate_cost,
                intra16.header_cost,
                intra16.distortion.wrapping_add(intra16.spectral_distortion),
                matrix.lambda_mode.cast_unsigned(),
            );
            let intra16_mode = intra16.mode;
            let intra4 = if let Some(token) = token {
                intra4::select_macroblock_with_token(
                    &source_y,
                    &top_y_i4,
                    &y_left,
                    y_top_left,
                    &top_modes,
                    &neighboring_left_modes,
                    top_y_nonzero,
                    left_y_nonzero,
                    matrix,
                    matrix.lambda_i4.cast_unsigned(),
                    matrix.lambda_mode.cast_unsigned(),
                    matrix.texture_lambda.cast_unsigned(),
                    method <= 1,
                    coefficient_probabilities,
                    trellis,
                    token,
                )?
            } else {
                intra4::select_macroblock(
                    &source_y,
                    &top_y_i4,
                    &y_left,
                    y_top_left,
                    &top_modes,
                    &neighboring_left_modes,
                    top_y_nonzero,
                    left_y_nonzero,
                    matrix,
                    matrix.lambda_i4.cast_unsigned(),
                    matrix.lambda_mode.cast_unsigned(),
                    matrix.texture_lambda.cast_unsigned(),
                    method <= 1,
                    coefficient_probabilities,
                    trellis,
                )
            };
            let luma = if method <= 1 {
                if analysis.macroblocks[block_index].use_intra4 {
                    LumaDecision::Intra4(intra4)
                } else {
                    LumaDecision::Intra16(intra16)
                }
            } else if intra4.score < intra16_mode_score {
                LumaDecision::Intra4(intra4)
            } else {
                LumaDecision::Intra16(intra16)
            };
            let (reconstructed_y, luma_d, luma_sd, luma_h, luma_r, luma_score, luma_nz) =
                match &luma {
                    LumaDecision::Intra4(result) => (
                        result.reconstructed,
                        result.distortion,
                        result.spectral_distortion,
                        result.header_cost,
                        result.rate_cost,
                        result.score,
                        result.nonzero,
                    ),
                    LumaDecision::Intra16(result) => (
                        result.reconstructed,
                        result.distortion,
                        result.spectral_distortion,
                        result.header_cost,
                        result.rate_cost,
                        intra16_mode_score,
                        result.nonzero,
                    ),
                };
            let chroma = chroma::select(
                &source_u,
                &source_v,
                &top_u,
                &top_v,
                &u_left,
                &v_left,
                u_top_left,
                v_top_left,
                macroblock_y != 0,
                macroblock_x != 0,
                top_chroma_nonzero,
                left_chroma_nonzero,
                top_errors[macroblock_x],
                left_errors,
                quality < 98.0,
                matrix,
                matrix.lambda_uv.cast_unsigned(),
                (method == 0).then(|| {
                    let mode = analysis.macroblocks[block_index].chroma_mode;
                    debug_assert!(mode <= 1);
                    if mode == 0 {
                        chroma::ChromaMode::Dc
                    } else {
                        chroma::ChromaMode::TrueMotion
                    }
                }),
                coefficient_probabilities,
            );
            let nonzero = luma_nz | chroma.nonzero;
            decisions.push(MacroblockDecision {
                x: macroblock_x,
                y: macroblock_y,
                segment,
                intra16_mode,
                luma,
                distortion: luma_d.wrapping_add(chroma.distortion),
                spectral_distortion: luma_sd,
                header_cost: luma_h.wrapping_add(chroma.header_cost),
                rate_cost: luma_r.wrapping_add(chroma.rate_cost),
                score: luma_score.wrapping_add(chroma.score),
                nonzero,
                chroma,
            });

            let decision = &decisions[decisions.len().saturating_sub(1)];
            for block_y in 0..4 {
                for block_x in 0..4 {
                    let mode_index = macroblock_y
                        .wrapping_mul(4)
                        .wrapping_add(block_y)
                        .wrapping_mul(mode_stride)
                        .wrapping_add(macroblock_x.wrapping_mul(4))
                        .wrapping_add(block_x);
                    mode_grid[mode_index] = match &decision.luma {
                        LumaDecision::Intra4(result) => {
                            result.modes[block_y.wrapping_mul(4).wrapping_add(block_x)]
                        }
                        LumaDecision::Intra16(result) => intra16_to_intra4(result.mode),
                    };
                }
            }

            let next_y_top_left = y_top[y_offset.wrapping_add(15)];
            let next_u_top_left = u_top[uv_offset.wrapping_add(7)];
            let next_v_top_left = v_top[uv_offset.wrapping_add(7)];
            for row in 0_usize..16 {
                y_left[row] = reconstructed_y[row.wrapping_mul(16).wrapping_add(15)];
            }
            for row in 0_usize..8 {
                let right_index = row.wrapping_mul(8).wrapping_add(7);
                u_left[row] = decision.chroma.reconstructed_u[right_index];
                v_left[row] = decision.chroma.reconstructed_v[right_index];
            }
            y_top[y_offset..y_offset.wrapping_add(16)].copy_from_slice(&reconstructed_y[240..256]);
            u_top[uv_offset..uv_offset.wrapping_add(8)]
                .copy_from_slice(&decision.chroma.reconstructed_u[56..64]);
            v_top[uv_offset..uv_offset.wrapping_add(8)]
                .copy_from_slice(&decision.chroma.reconstructed_v[56..64]);
            y_top_left = next_y_top_left;
            u_top_left = next_u_top_left;
            v_top_left = next_v_top_left;
            left_packed = nonzero & STORED_NONZERO_MASK;
            if matches!(decision.luma, LumaDecision::Intra4(_)) {
                // I4 has no Y2 block. libwebp preserves the incoming top Y2
                // context in the packed column state and leaves the separate
                // left Y2 context untouched.
                left_packed |= top_packed & (1 << 24);
            } else {
                left_y2_nonzero = bit(nonzero, 24);
            }
            packed_nonzero[macroblock_x] = left_packed;
            if method >= 3 {
                left_errors =
                    store_diffusion_errors(decision.chroma.errors, &mut top_errors[macroblock_x]);
            }
            selection_items = selection_items.saturating_add(1);
            if selection_items.is_multiple_of(SELECTION_CHECKPOINT_MACROBLOCKS) {
                #[cfg(coverage)]
                coverage_cancel_selection_checkpoint(token);
                crate::codecs::error::check_cancelled(token)?;
            }
        }
    }
    Ok(decisions)
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let width = 16 * SELECTION_CHECKPOINT_MACROBLOCKS;
    let height = 16;
    let y_plane = vec![0_u8; width * height];
    let u_plane = vec![128_u8; (width / 2) * (height / 2)];
    let v_plane = u_plane.clone();
    let analysis = FrameAnalysis {
        alpha: 0,
        chroma_alpha: 0,
        macroblocks: vec![
            super::analysis::MacroblockAnalysis::default();
            SELECTION_CHECKPOINT_MACROBLOCKS
        ],
        segments: [super::analysis::SegmentAnalysis::default(); 4],
    };
    let token = crate::CancellationToken::new();
    FORCE_SELECTION_CHECKPOINT_ERROR.store(true, Ordering::Relaxed);
    let _ = select_frame(
        [&y_plane, &u_plane, &v_plane],
        (width, height),
        &analysis,
        FrameSelectionOptions {
            quality: 75.0,
            method: 0,
            coefficient_probabilities: &super::tokenize::COEFF_PROBS,
            trellis: false,
            token: Some(&token),
        },
    );
}
