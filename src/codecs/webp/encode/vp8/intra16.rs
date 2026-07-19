//! Exact libwebp-compatible VP8 intra-16 mode evaluation.

#![allow(dead_code)]

use super::{
    cost::{rd_score, residual_cost, spectral_distortion_16x16, squared_error_16x16},
    dct::{vp8_fdct_4x4, vp8_fwht_4x4, vp8_idct_add_4x4, vp8_iwht_4x4},
    quant::{SegmentMatrices, quantize_block},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum Intra16Mode {
    Dc = 0,
    TrueMotion = 1,
    Vertical = 2,
    Horizontal = 3,
}

impl Intra16Mode {
    const ALL: [Self; 4] = [Self::Dc, Self::TrueMotion, Self::Vertical, Self::Horizontal];
}

const FIXED_MODE_COSTS: [u32; 4] = [663, 919, 872, 919];

fn predict(mode: Intra16Mode, top: &[u8; 16], left: &[u8; 16], top_left: u8) -> [u8; 256] {
    let mut output = [0; 256];
    match mode {
        Intra16Mode::Dc => {
            let sum = top
                .iter()
                .chain(left)
                .map(|&value| u32::from(value))
                .sum::<u32>();
            output.fill(((sum + 16) >> 5) as u8);
        }
        Intra16Mode::Vertical => {
            for row in output.chunks_exact_mut(16) {
                row.copy_from_slice(top);
            }
        }
        Intra16Mode::Horizontal => {
            for (row, &value) in output.chunks_exact_mut(16).zip(left) {
                row.fill(value);
            }
        }
        Intra16Mode::TrueMotion => {
            for row in 0..16 {
                for column in 0..16 {
                    output[row * 16 + column] = (i16::from(top[column]) + i16::from(left[row])
                        - i16::from(top_left))
                    .clamp(0, 255) as u8;
                }
            }
        }
    }
    output
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Intra16Candidate {
    pub(super) mode: Intra16Mode,
    pub(super) y2_levels: [i16; 16],
    pub(super) y1_levels: [[i16; 16]; 16],
    pub(super) reconstructed: [u8; 256],
    pub(super) distortion: u32,
    pub(super) spectral_distortion: u32,
    pub(super) header_cost: u32,
    pub(super) rate_cost: u32,
    pub(super) score: u64,
    pub(super) nonzero: u32,
}

#[allow(clippy::too_many_arguments)]
fn evaluate(
    mode: Intra16Mode,
    source: &[u8; 256],
    top: &[u8; 16],
    left: &[u8; 16],
    top_left: u8,
    top_nonzero: [u8; 4],
    left_nonzero: [u8; 4],
    y2_context: usize,
    matrices: &SegmentMatrices,
    lambda_i16: u32,
    texture_lambda: u32,
) -> Intra16Candidate {
    let prediction = predict(mode, top, left, top_left);
    let mut coefficients = [[0i16; 16]; 16];
    for block_y in 0..4 {
        for block_x in 0..4 {
            let block = block_y * 4 + block_x;
            let mut residual = [0i16; 16];
            for row in 0..4 {
                for column in 0..4 {
                    let index = (block_y * 4 + row) * 16 + block_x * 4 + column;
                    residual[row * 4 + column] =
                        i16::from(source[index]) - i16::from(prediction[index]);
                }
            }
            coefficients[block] = vp8_fdct_4x4(&residual);
        }
    }

    let dc = std::array::from_fn(|block| coefficients[block][0]);
    let mut transformed_dc = vp8_fwht_4x4(&dc);
    let mut y2_levels = [0; 16];
    let y2_nonzero = quantize_block(&mut transformed_dc, &mut y2_levels, &matrices.y2);

    let mut y1_levels = [[0; 16]; 16];
    let mut nonzero = u32::from(y2_nonzero) << 24;
    for block in 0..16 {
        coefficients[block][0] = 0;
        if quantize_block(
            &mut coefficients[block],
            &mut y1_levels[block],
            &matrices.y1,
        ) {
            nonzero |= 1 << block;
        }
    }
    let restored_dc = vp8_iwht_4x4(&transformed_dc);
    for block in 0..16 {
        coefficients[block][0] = restored_dc[block];
    }

    let mut reconstructed = [0; 256];
    for block_y in 0..4 {
        for block_x in 0..4 {
            let block = block_y * 4 + block_x;
            let mut prediction_block = [0; 16];
            for row in 0..4 {
                let offset = (block_y * 4 + row) * 16 + block_x * 4;
                prediction_block[row * 4..row * 4 + 4]
                    .copy_from_slice(&prediction[offset..offset + 4]);
            }
            let output = vp8_idct_add_4x4(&prediction_block, &coefficients[block]);
            for row in 0..4 {
                let offset = (block_y * 4 + row) * 16 + block_x * 4;
                reconstructed[offset..offset + 4].copy_from_slice(&output[row * 4..row * 4 + 4]);
            }
        }
    }

    let mut rate = residual_cost(&y2_levels, 0, 1, y2_context);
    let mut top_context = top_nonzero;
    let mut left_context = left_nonzero;
    for block_y in 0..4 {
        for block_x in 0..4 {
            let block = block_y * 4 + block_x;
            let context = usize::from(top_context[block_x] + left_context[block_y]);
            rate += residual_cost(&y1_levels[block], 1, 0, context);
            let block_nonzero = u8::from(y1_levels[block][1..].iter().any(|&level| level != 0));
            top_context[block_x] = block_nonzero;
            left_context[block_y] = block_nonzero;
        }
    }
    let distortion = squared_error_16x16(source, &reconstructed);
    let texture = spectral_distortion_16x16(source, &reconstructed);
    let spectral_distortion = (texture_lambda * texture + 128) >> 8;
    let header = FIXED_MODE_COSTS[mode as usize];
    let score = rd_score(rate, header, distortion + spectral_distortion, lambda_i16);
    Intra16Candidate {
        mode,
        y2_levels,
        y1_levels,
        reconstructed,
        distortion,
        spectral_distortion,
        header_cost: header,
        rate_cost: rate,
        score,
        nonzero,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select(
    source: &[u8; 256],
    top: &[u8; 16],
    left: &[u8; 16],
    top_left: u8,
    top_nonzero: [u8; 4],
    left_nonzero: [u8; 4],
    y2_context: usize,
    matrices: &SegmentMatrices,
    lambda_i16: u32,
    texture_lambda: u32,
) -> Intra16Candidate {
    Intra16Mode::ALL
        .into_iter()
        .map(|mode| {
            evaluate(
                mode,
                source,
                top,
                left,
                top_left,
                top_nonzero,
                left_nonzero,
                y2_context,
                matrices,
                lambda_i16,
                texture_lambda,
            )
        })
        .min_by_key(|candidate| candidate.score)
        .expect("VP8 always has intra16 candidates")
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
        let (luma, _, _) = rgb_to_yuv_planes_internal(rgb, 128, 128);
        let mut source = [0; 256];
        for row in 0..16 {
            source[row * 16..row * 16 + 16].copy_from_slice(&luma[row * 128..row * 128 + 16]);
        }
        let matrices = libwebp_segment_matrices(16, -2, 6);
        let expected = [
            (753, 34, 663, 229_910, 664_943_431),
            (728, 36, 919, 229_925, 665_718_836),
            (750, 30, 872, 229_893, 665_495_175),
            (728, 36, 919, 229_925, 665_718_836),
        ];
        for (index, mode) in Intra16Mode::ALL.into_iter().enumerate() {
            let candidate = evaluate(
                mode, &source, &[127; 16], &[129; 16], 127, [0; 4], [0; 4], 0, &matrices, 2_883, 31,
            );
            let (distortion, spectral, header, rate, score) = expected[index];
            assert_eq!(candidate.distortion, distortion, "D mode {index}");
            assert_eq!(candidate.spectral_distortion, spectral, "SD mode {index}");
            assert_eq!(candidate.header_cost, header, "H mode {index}");
            assert_eq!(candidate.rate_cost, rate, "R mode {index}");
            assert_eq!(candidate.score, score, "score mode {index}");
            assert_eq!(candidate.nonzero, 0x0100_ffff);
        }
        assert_eq!(
            select(
                &source, &[127; 16], &[129; 16], 127, [0; 4], [0; 4], 0, &matrices, 2_883, 31,
            )
            .mode,
            Intra16Mode::Dc
        );
    }
}
