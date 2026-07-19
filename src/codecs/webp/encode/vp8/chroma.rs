//! Exact libwebp-compatible VP8 chroma mode evaluation.

#![allow(dead_code)]

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

fn predict(mode: ChromaMode, top: &[u8; 8], left: &[u8; 8], top_left: u8) -> [u8; 64] {
    let mut output = [0; 64];
    match mode {
        ChromaMode::Dc => {
            let sum = top
                .iter()
                .chain(left)
                .map(|&value| u32::from(value))
                .sum::<u32>();
            output.fill(((sum + 8) >> 4) as u8);
        }
        ChromaMode::TrueMotion => {
            for row in 0..8 {
                for column in 0..8 {
                    output[row * 8 + column] = (i16::from(top[column]) + i16::from(left[row])
                        - i16::from(top_left))
                    .clamp(0, 255) as u8;
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
        let quantized = ((magnitude * u32::from(matrix.reciprocal[0]) + matrix.bias[0]) >> 17)
            * u32::from(matrix.q[0]);
        let error = magnitude as i32 - quantized as i32;
        *value = if negative {
            -(quantized as i16)
        } else {
            quantized as i16
        };
        (if negative { -error } else { error } >> 1) as i8
    } else {
        *value = 0;
        (if negative {
            -(magnitude as i32)
        } else {
            magnitude as i32
        } >> 1) as i8
    }
}

fn correct_dc(
    coefficients: &mut [[i16; 16]; 4],
    matrix: &QuantMatrix,
    top_errors: [i8; 2],
    left_errors: [i8; 2],
) -> [i8; 3] {
    coefficients[0][0] +=
        ((7 * i16::from(top_errors[0]) + 8 * i16::from(left_errors[0])) >> 3) as i16;
    let error0 = quantize_single(&mut coefficients[0][0], matrix);
    coefficients[1][0] += ((7 * i16::from(top_errors[1]) + 8 * i16::from(error0)) >> 3) as i16;
    let error1 = quantize_single(&mut coefficients[1][0], matrix);
    coefficients[2][0] += ((7 * i16::from(error0) + 8 * i16::from(left_errors[1])) >> 3) as i16;
    let error2 = quantize_single(&mut coefficients[2][0], matrix);
    coefficients[3][0] += ((7 * i16::from(error1) + 8 * i16::from(error2)) >> 3) as i16;
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
    top_nonzero: [u8; 4],
    left_nonzero: [u8; 4],
    top_errors: [[i8; 2]; 2],
    left_errors: [[i8; 2]; 2],
    matrices: &SegmentMatrices,
    lambda_uv: u32,
) -> ChromaCandidate {
    let predictions = [
        predict(mode, top_u, left_u, top_left_u),
        predict(mode, top_v, left_v, top_left_v),
    ];
    let sources = [source_u, source_v];
    let mut levels = [[0; 16]; 8];
    let mut reconstructed = [[0; 64]; 2];
    let mut errors = [[0; 3]; 2];
    let mut nonzero = 0u32;

    for plane in 0..2 {
        let mut coefficients = [[0i16; 16]; 4];
        for block_y in 0..2 {
            for block_x in 0..2 {
                let block = block_y * 2 + block_x;
                let mut residual = [0i16; 16];
                for row in 0..4 {
                    for column in 0..4 {
                        let index = (block_y * 4 + row) * 8 + block_x * 4 + column;
                        residual[row * 4 + column] =
                            i16::from(sources[plane][index]) - i16::from(predictions[plane][index]);
                    }
                }
                coefficients[block] = vp8_fdct_4x4(&residual);
            }
        }
        errors[plane] = correct_dc(
            &mut coefficients,
            &matrices.uv,
            top_errors[plane],
            left_errors[plane],
        );
        for block_y in 0..2 {
            for block_x in 0..2 {
                let block = block_y * 2 + block_x;
                let level_index = plane * 4 + block;
                if quantize_block(
                    &mut coefficients[block],
                    &mut levels[level_index],
                    &matrices.uv,
                ) {
                    nonzero |= 1 << (16 + level_index);
                }
                let mut prediction_block = [0; 16];
                for row in 0..4 {
                    let offset = (block_y * 4 + row) * 8 + block_x * 4;
                    prediction_block[row * 4..row * 4 + 4]
                        .copy_from_slice(&predictions[plane][offset..offset + 4]);
                }
                let output = vp8_idct_add_4x4(&prediction_block, &coefficients[block]);
                for row in 0..4 {
                    let offset = (block_y * 4 + row) * 8 + block_x * 4;
                    reconstructed[plane][offset..offset + 4]
                        .copy_from_slice(&output[row * 4..row * 4 + 4]);
                }
            }
        }
    }

    let mut top_context = top_nonzero;
    let mut left_context = left_nonzero;
    let mut rate = 0;
    for plane in 0..2 {
        for block_y in 0..2 {
            for block_x in 0..2 {
                let level_index = plane * 4 + block_y * 2 + block_x;
                let context_index = plane * 2 + block_x;
                let context =
                    usize::from(top_context[context_index] + left_context[plane * 2 + block_y]);
                rate += residual_cost(&levels[level_index], 0, 2, context);
                let block_nonzero = u8::from(levels[level_index].iter().any(|&level| level != 0));
                top_context[context_index] = block_nonzero;
                left_context[plane * 2 + block_y] = block_nonzero;
            }
        }
    }
    if mode != ChromaMode::Dc
        && levels
            .iter()
            .flat_map(|block| &block[1..])
            .filter(|&&level| level != 0)
            .count()
            <= 6
    {
        rate += 1_120;
    }
    let distortion = sources
        .iter()
        .zip(&reconstructed)
        .map(|(source, output)| {
            source
                .iter()
                .zip(output)
                .map(|(&source, &output)| {
                    let difference = i32::from(source) - i32::from(output);
                    (difference * difference) as u32
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
    top_nonzero: [u8; 4],
    left_nonzero: [u8; 4],
    top_errors: [[i8; 2]; 2],
    left_errors: [[i8; 2]; 2],
    matrices: &SegmentMatrices,
    lambda_uv: u32,
) -> ChromaCandidate {
    ChromaMode::ALL
        .into_iter()
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
                top_nonzero,
                left_nonzero,
                top_errors,
                left_errors,
                matrices,
                lambda_uv,
            )
        })
        .min_by_key(|candidate| candidate.score)
        .expect("VP8 always has chroma candidates")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codecs::webp::encode::vp8::{
        encoder::rgb_to_yuv_planes_internal, quant::libwebp_segment_matrices,
    };

    #[test]
    fn first_q80_candidates_match_libwebp_1_6_0() {
        let rgb = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/outputs/raws/Decode.webp_lossless_webp.bin"
        ));
        let (_, u, v) = rgb_to_yuv_planes_internal(rgb, 128, 128);
        let mut source_u = [0; 64];
        let mut source_v = [0; 64];
        for row in 0..8 {
            source_u[row * 8..row * 8 + 8].copy_from_slice(&u[row * 64..row * 64 + 8]);
            source_v[row * 8..row * 8 + 8].copy_from_slice(&v[row * 64..row * 64 + 8]);
        }
        let matrices = libwebp_segment_matrices(16, -2, 6);
        let expected = [
            (319, 302, 32_452, 1_031_530, [-2, -3, 0, 0, 3, -2]),
            (305, 984, 32_497, 1_049_029, [0, 0, -2, 3, -3, 1]),
            (297, 439, 33_535, 1_061_278, [0, -1, -3, 2, -4, -1]),
            (305, 642, 32_497, 1_039_111, [0, 0, -2, 3, -3, 1]),
        ];
        for (index, mode) in ChromaMode::ALL.into_iter().enumerate() {
            let candidate = evaluate(
                mode,
                &source_u,
                &source_v,
                &[127; 8],
                &[127; 8],
                &[129; 8],
                &[129; 8],
                127,
                127,
                [0; 4],
                [0; 4],
                [[0; 2]; 2],
                [[0; 2]; 2],
                &matrices,
                29,
            );
            let (distortion, header, rate, score, errors) = expected[index];
            assert_eq!(candidate.distortion, distortion, "D mode {index}");
            assert_eq!(candidate.header_cost, header, "H mode {index}");
            assert_eq!(candidate.rate_cost, rate, "R mode {index}");
            assert_eq!(candidate.score, score, "score mode {index}");
            assert_eq!(candidate.errors.concat(), errors, "errors mode {index}");
            assert_eq!(candidate.nonzero, 0x00ff_0000);
        }
        assert_eq!(
            select(
                &source_u,
                &source_v,
                &[127; 8],
                &[127; 8],
                &[129; 8],
                &[129; 8],
                127,
                127,
                [0; 4],
                [0; 4],
                [[0; 2]; 2],
                [[0; 2]; 2],
                &matrices,
                29,
            )
            .mode,
            ChromaMode::Dc
        );
    }
}
