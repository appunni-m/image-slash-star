//! Exact libwebp-compatible VP8 chroma mode evaluation.

use super::{
    cost::{rd_score, residual_cost},
    dct::{vp8_fdct_4x4, vp8_idct_add_4x4},
    quant::{QuantMatrix, SegmentMatrices, quantize_block},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum ChromaMode {
    Dc = 0,
    TrueMotion = 1,
    Vertical = 2,
    Horizontal = 3,
}

impl ChromaMode {
    const ALL: [Self; 4] = [Self::Dc, Self::TrueMotion, Self::Vertical, Self::Horizontal];
}

const FIXED_MODE_COSTS: [u32; 4] = [302, 984, 439, 642];

fn predict(
    mode: ChromaMode,
    top: &[u8; 8],
    left: &[u8; 8],
    top_left: u8,
    has_top: bool,
    has_left: bool,
) -> [u8; 64] {
    let mut output = [0; 64];
    match mode {
        ChromaMode::Dc => {
            let dc = match (has_top, has_left) {
                (true, true) => (top
                    .iter()
                    .chain(left)
                    .map(|&value| u32::from(value))
                    .sum::<u32>()
                    .saturating_add(8))
                .wrapping_shr(4),
                (true, false) => top
                    .iter()
                    .map(|&value| u32::from(value))
                    .sum::<u32>()
                    .saturating_add(4)
                    .wrapping_shr(3),
                (false, true) => left
                    .iter()
                    .map(|&value| u32::from(value))
                    .sum::<u32>()
                    .saturating_add(4)
                    .wrapping_shr(3),
                (false, false) => 128,
            };
            output.fill(dc.to_le_bytes()[0]);
        }
        ChromaMode::TrueMotion => {
            for row in 0usize..8 {
                for column in 0usize..8 {
                    output[row.saturating_mul(8).saturating_add(column)] = i16::from(top[column])
                        .saturating_add(i16::from(left[row]))
                        .saturating_sub(i16::from(top_left))
                        .clamp(0, 255)
                        .to_le_bytes()[0];
                }
            }
        }
        ChromaMode::Vertical => {
            for row in output.chunks_exact_mut(8) {
                row.copy_from_slice(top);
            }
        }
        ChromaMode::Horizontal => {
            for (row, &value) in output.chunks_exact_mut(8).zip(left) {
                row.fill(value);
            }
        }
    }
    output
}

fn quantize_single(value: &mut i16, matrix: &QuantMatrix) -> i8 {
    let signed = i32::from(*value);
    let negative = signed < 0;
    let magnitude = signed.unsigned_abs();
    if magnitude > matrix.zero_threshold[0] {
        let quantized = magnitude
            .saturating_mul(u32::from(matrix.reciprocal[0]))
            .saturating_add(matrix.bias[0])
            .wrapping_shr(17)
            .saturating_mul(u32::from(matrix.q[0]));
        let quantized_i32 = i32::from_le_bytes(quantized.to_le_bytes());
        let error = i32::from_le_bytes(magnitude.to_le_bytes()).saturating_sub(quantized_i32);
        let [a, b, ..] = quantized.to_le_bytes();
        let quantized_i16 = i16::from_le_bytes([a, b]);
        *value = if negative {
            quantized_i16.saturating_neg()
        } else {
            quantized_i16
        };
        let error = if negative {
            error.saturating_neg()
        } else {
            error
        }
        .wrapping_shr(1);
        i8::from_le_bytes([error.to_le_bytes()[0]])
    } else {
        *value = 0;
        let error = if negative {
            i32::from_le_bytes(magnitude.to_le_bytes()).saturating_neg()
        } else {
            i32::from_le_bytes(magnitude.to_le_bytes())
        }
        .wrapping_shr(1);
        i8::from_le_bytes([error.to_le_bytes()[0]])
    }
}

fn correct_dc(
    coefficients: &mut [[i16; 16]; 4],
    matrix: &QuantMatrix,
    top_errors: [i8; 2],
    left_errors: [i8; 2],
) -> [i8; 3] {
    coefficients[0][0] = coefficients[0][0].saturating_add(
        7i16.saturating_mul(i16::from(top_errors[0]))
            .saturating_add(8i16.saturating_mul(i16::from(left_errors[0])))
            .wrapping_shr(3),
    );
    let error0 = quantize_single(&mut coefficients[0][0], matrix);
    coefficients[1][0] = coefficients[1][0].saturating_add(
        7i16.saturating_mul(i16::from(top_errors[1]))
            .saturating_add(8i16.saturating_mul(i16::from(error0)))
            .wrapping_shr(3),
    );
    let error1 = quantize_single(&mut coefficients[1][0], matrix);
    coefficients[2][0] = coefficients[2][0].saturating_add(
        7i16.saturating_mul(i16::from(error0))
            .saturating_add(8i16.saturating_mul(i16::from(left_errors[1])))
            .wrapping_shr(3),
    );
    let error2 = quantize_single(&mut coefficients[2][0], matrix);
    coefficients[3][0] = coefficients[3][0].saturating_add(
        7i16.saturating_mul(i16::from(error1))
            .saturating_add(8i16.saturating_mul(i16::from(error2)))
            .wrapping_shr(3),
    );
    let error3 = quantize_single(&mut coefficients[3][0], matrix);
    [error1, error2, error3]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ChromaCandidate {
    pub(super) mode: ChromaMode,
    pub(super) levels: [[i16; 16]; 8],
    pub(super) reconstructed_u: [u8; 64],
    pub(super) reconstructed_v: [u8; 64],
    pub(super) errors: [[i8; 3]; 2],
    pub(super) distortion: u32,
    pub(super) header_cost: u32,
    pub(super) rate_cost: u32,
    pub(super) score: u64,
    pub(super) nonzero: u32,
}

#[allow(clippy::too_many_arguments)]
fn evaluate(
    mode: ChromaMode,
    source_u: &[u8; 64],
    source_v: &[u8; 64],
    top_u: &[u8; 8],
    top_v: &[u8; 8],
    left_u: &[u8; 8],
    left_v: &[u8; 8],
    top_left_u: u8,
    top_left_v: u8,
    has_top: bool,
    has_left: bool,
    top_nonzero: [u8; 4],
    left_nonzero: [u8; 4],
    top_errors: [[i8; 2]; 2],
    left_errors: [[i8; 2]; 2],
    error_diffusion: bool,
    matrices: &SegmentMatrices,
    lambda_uv: u32,
    coefficient_probabilities: &[[[[u8; 11]; 3]; 8]; 4],
) -> ChromaCandidate {
    let predictions = [
        predict(mode, top_u, left_u, top_left_u, has_top, has_left),
        predict(mode, top_v, left_v, top_left_v, has_top, has_left),
    ];
    let sources = [source_u, source_v];
    let mut levels = [[0; 16]; 8];
    let mut reconstructed = [[0; 64]; 2];
    let mut errors = [[0; 3]; 2];
    let mut nonzero = 0u32;

    for plane in 0usize..2 {
        let mut coefficients = [[0i16; 16]; 4];
        for block_y in 0usize..2 {
            for block_x in 0usize..2 {
                let block = block_y.saturating_mul(2).saturating_add(block_x);
                let mut residual = [0i16; 16];
                for row in 0usize..4 {
                    for column in 0usize..4 {
                        let index = block_y
                            .saturating_mul(4)
                            .saturating_add(row)
                            .saturating_mul(8)
                            .saturating_add(block_x.saturating_mul(4))
                            .saturating_add(column);
                        residual[row.saturating_mul(4).saturating_add(column)] =
                            i16::from(sources[plane][index])
                                .saturating_sub(i16::from(predictions[plane][index]));
                    }
                }
                coefficients[block] = vp8_fdct_4x4(&residual);
            }
        }
        if error_diffusion {
            errors[plane] = correct_dc(
                &mut coefficients,
                &matrices.uv,
                top_errors[plane],
                left_errors[plane],
            );
        }
        for block_y in 0usize..2 {
            for block_x in 0usize..2 {
                let block = block_y.saturating_mul(2).saturating_add(block_x);
                let level_index = plane.saturating_mul(4).saturating_add(block);
                if quantize_block(
                    &mut coefficients[block],
                    &mut levels[level_index],
                    &matrices.uv,
                ) {
                    nonzero |= 1u32
                        .wrapping_shl(16usize.saturating_add(level_index).to_le_bytes()[0].into());
                }
                let mut prediction_block = [0; 16];
                for row in 0usize..4 {
                    let offset = block_y
                        .saturating_mul(4)
                        .saturating_add(row)
                        .saturating_mul(8)
                        .saturating_add(block_x.saturating_mul(4));
                    let row_start = row.saturating_mul(4);
                    prediction_block[row_start..row_start.saturating_add(4)]
                        .copy_from_slice(&predictions[plane][offset..offset.saturating_add(4)]);
                }
                let output = vp8_idct_add_4x4(&prediction_block, &coefficients[block]);
                for row in 0usize..4 {
                    let offset = block_y
                        .saturating_mul(4)
                        .saturating_add(row)
                        .saturating_mul(8)
                        .saturating_add(block_x.saturating_mul(4));
                    let row_start = row.saturating_mul(4);
                    reconstructed[plane][offset..offset.saturating_add(4)]
                        .copy_from_slice(&output[row_start..row_start.saturating_add(4)]);
                }
            }
        }
    }

    let mut top_context = top_nonzero;
    let mut left_context = left_nonzero;
    let mut rate = 0u32;
    for plane in 0usize..2 {
        for block_y in 0usize..2 {
            for block_x in 0usize..2 {
                let level_index = plane
                    .saturating_mul(4)
                    .saturating_add(block_y.saturating_mul(2))
                    .saturating_add(block_x);
                let context_index = plane.saturating_mul(2).saturating_add(block_x);
                let left_index = plane.saturating_mul(2).saturating_add(block_y);
                let context = usize::from(
                    top_context[context_index].saturating_add(left_context[left_index]),
                );
                rate = rate.saturating_add(residual_cost(
                    &levels[level_index],
                    0,
                    2,
                    context,
                    coefficient_probabilities,
                ));
                let block_nonzero = u8::from(levels[level_index].iter().any(|&level| level != 0));
                top_context[context_index] = block_nonzero;
                left_context[left_index] = block_nonzero;
            }
        }
    }
    if mode != ChromaMode::Dc
        && levels
            .iter()
            .flat_map(|block| &block[1..])
            .filter(|&&level| level != 0)
            .count()
            <= 2
    {
        rate = rate.saturating_add(1_120);
    }
    let distortion = sources
        .iter()
        .zip(&reconstructed)
        .map(|(source, output)| {
            source
                .iter()
                .zip(output)
                .map(|(&source, &output)| {
                    let difference = i32::from(source).saturating_sub(i32::from(output));
                    difference.saturating_mul(difference).cast_unsigned()
                })
                .sum::<u32>()
        })
        .sum();
    let header = FIXED_MODE_COSTS[mode as usize];
    let score = rd_score(rate, header, distortion, lambda_uv);
    ChromaCandidate {
        mode,
        levels,
        reconstructed_u: reconstructed[0],
        reconstructed_v: reconstructed[1],
        errors,
        distortion,
        header_cost: header,
        rate_cost: rate,
        score,
        nonzero,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select(
    source_u: &[u8; 64],
    source_v: &[u8; 64],
    top_u: &[u8; 8],
    top_v: &[u8; 8],
    left_u: &[u8; 8],
    left_v: &[u8; 8],
    top_left_u: u8,
    top_left_v: u8,
    has_top: bool,
    has_left: bool,
    top_nonzero: [u8; 4],
    left_nonzero: [u8; 4],
    top_errors: [[i8; 2]; 2],
    left_errors: [[i8; 2]; 2],
    error_diffusion: bool,
    matrices: &SegmentMatrices,
    lambda_uv: u32,
    fixed_mode: Option<ChromaMode>,
    coefficient_probabilities: &[[[[u8; 11]; 3]; 8]; 4],
) -> ChromaCandidate {
    // `fixed_mode`, when present, is itself a member of this non-empty enum set.
    #[allow(clippy::expect_used)]
    ChromaMode::ALL
        .into_iter()
        .filter(|&mode| fixed_mode.is_none_or(|fixed| fixed == mode))
        .map(|mode| {
            evaluate(
                mode,
                source_u,
                source_v,
                top_u,
                top_v,
                left_u,
                left_v,
                top_left_u,
                top_left_v,
                has_top,
                has_left,
                top_nonzero,
                left_nonzero,
                top_errors,
                left_errors,
                error_diffusion,
                matrices,
                lambda_uv,
                coefficient_probabilities,
            )
        })
        .min_by_key(|candidate| candidate.score)
        .expect("VP8 always has chroma candidates")
}
