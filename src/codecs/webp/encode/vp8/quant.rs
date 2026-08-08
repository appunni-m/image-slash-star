//! VP8 quantization tables and color conversion (RFC 6386).
//!
//! Provides:
//! - Quality-to-quantizer-index mapping (`quality_to_quant_index`)
//! - Coefficient quantization and dequantization (`quantize`, `dequantize`)
//! - The four base quantization tables used by VP8 (Y/UV, DC/AC)
//! - RGB to YCbCr (BT.601) conversion (`rgb_to_yuv`)

use super::{
    cost::{bit_cost, level_cost},
    tokenize::COEFF_BANDS,
};
use crate::codecs::CodecResult;

// ── VP8 quantization step tables ──
//
// These are the exact tables from libvpx 1.15.2
// (vp8/common/quant_common.c), implementing the base quantization step sizes
// for indices 0..127. The fixed source and legal terms are retained under
// third_party/libvpx/.

/// DC quantization step sizes for luma (Y) blocks. Indexed 0..127.
pub const Y_DC_QUANT: [u16; 128] = [
    4, 5, 6, 7, 8, 9, 10, 10, 11, 12, 13, 14, 15, 16, 17, 17, 18, 19, 20, 20, 21, 21, 22, 22, 23,
    23, 24, 25, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 37, 38, 39, 40, 41, 42, 43, 44,
    45, 46, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
    68, 69, 70, 71, 72, 73, 74, 75, 76, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 91,
    93, 95, 96, 98, 100, 101, 102, 104, 106, 108, 110, 112, 114, 116, 118, 122, 124, 126, 128, 130,
    132, 134, 136, 138, 140, 143, 145, 148, 151, 154, 157,
];

/// AC quantization step sizes for luma (Y) blocks. Indexed 0..127.
pub const Y_AC_QUANT: [u16; 128] = [
    4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28,
    29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52,
    53, 54, 55, 56, 57, 58, 60, 62, 64, 66, 68, 70, 72, 74, 76, 78, 80, 82, 84, 86, 88, 90, 92, 94,
    96, 98, 100, 102, 104, 106, 108, 110, 112, 114, 116, 119, 122, 125, 128, 131, 134, 137, 140,
    143, 146, 149, 152, 155, 158, 161, 164, 167, 170, 173, 177, 181, 185, 189, 193, 197, 201, 205,
    209, 213, 217, 221, 225, 229, 234, 239, 245, 249, 254, 259, 264, 269, 274, 279, 284,
];

/// AC quantization steps for the second-order luma transform.
pub const Y2_AC_QUANT: [u16; 128] = [
    8, 8, 9, 10, 12, 13, 15, 17, 18, 20, 21, 23, 24, 26, 27, 29, 31, 32, 34, 35, 37, 38, 40, 41,
    43, 44, 46, 48, 49, 51, 52, 54, 55, 57, 58, 60, 62, 63, 65, 66, 68, 69, 71, 72, 74, 75, 77, 79,
    80, 82, 83, 85, 86, 88, 89, 93, 96, 99, 102, 105, 108, 111, 114, 117, 120, 124, 127, 130, 133,
    136, 139, 142, 145, 148, 151, 155, 158, 161, 164, 167, 170, 173, 176, 179, 184, 189, 193, 198,
    203, 207, 212, 217, 221, 226, 230, 235, 240, 244, 249, 254, 258, 263, 268, 274, 280, 286, 292,
    299, 305, 311, 317, 323, 330, 336, 342, 348, 354, 362, 370, 379, 385, 393, 401, 409, 416, 424,
    432, 440,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct QuantMatrix {
    pub(super) q: [u16; 16],
    pub(super) reciprocal: [u16; 16],
    pub(super) bias: [u32; 16],
    pub(super) zero_threshold: [u32; 16],
    pub(super) sharpen: [u16; 16],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SegmentMatrices {
    pub(super) y1: QuantMatrix,
    pub(super) y2: QuantMatrix,
    pub(super) uv: QuantMatrix,
    pub(super) lambda_i4: i32,
    pub(super) lambda_i16: i32,
    pub(super) lambda_uv: i32,
    pub(super) lambda_mode: i32,
    pub(super) texture_lambda: i32,
    pub(super) lambda_trellis_i4: i32,
    pub(super) lambda_trellis_i16: i32,
}

fn expand_matrix(dc: u16, ac: u16, kind: usize) -> (QuantMatrix, i32) {
    const BIASES: [[u32; 2]; 3] = [[96, 110], [96, 108], [110, 115]];
    const SHARPENING: [u16; 16] = [
        0, 30, 60, 90, 30, 60, 90, 90, 60, 90, 90, 90, 90, 90, 90, 90,
    ];
    let mut matrix = QuantMatrix {
        q: [ac; 16],
        reciprocal: [0; 16],
        bias: [0; 16],
        zero_threshold: [0; 16],
        sharpen: [0; 16],
    };
    matrix.q[0] = dc;
    for index in 0..2 {
        let reciprocal = 131_072_u32
            .checked_div(u32::from(matrix.q[index]))
            .unwrap_or_default();
        matrix.reciprocal[index] =
            u16::from_le_bytes([reciprocal.to_le_bytes()[0], reciprocal.to_le_bytes()[1]]);
        matrix.bias[index] = BIASES[kind][usize::from(index > 0)] << 9;
        matrix.zero_threshold[index] = 131_071_u32
            .wrapping_sub(matrix.bias[index])
            .checked_div(u32::from(matrix.reciprocal[index]))
            .unwrap_or_default();
    }
    for index in 2..16 {
        matrix.reciprocal[index] = matrix.reciprocal[1];
        matrix.bias[index] = matrix.bias[1];
        matrix.zero_threshold[index] = matrix.zero_threshold[1];
    }
    if kind == 0 {
        for (index, sharpen) in matrix.sharpen.iter_mut().enumerate() {
            *sharpen = SHARPENING[index].wrapping_mul(matrix.q[index]) >> 11;
        }
    }
    let average = matrix
        .q
        .iter()
        .fold(0_i32, |sum, &value| sum.wrapping_add(i32::from(value)))
        .wrapping_add(8)
        >> 4;
    (matrix, average)
}

pub(super) fn libwebp_segment_matrices(
    quantizer: u8,
    chroma_dc_delta: i8,
    chroma_ac_delta: i8,
) -> SegmentMatrices {
    let quantizer = usize::from(quantizer);
    let (y1, q_i4) = expand_matrix(Y_DC_QUANT[quantizer], Y_AC_QUANT[quantizer], 0);
    let (y2, q_i16) = expand_matrix(
        Y_DC_QUANT[quantizer].wrapping_mul(2),
        Y2_AC_QUANT[quantizer],
        1,
    );
    let quantizer_i32 = i32::from(quantizer.to_le_bytes()[0]);
    let uv_dc_value = quantizer_i32
        .wrapping_add(i32::from(chroma_dc_delta))
        .clamp(0, 117);
    let uv_ac_value = quantizer_i32
        .wrapping_add(i32::from(chroma_ac_delta))
        .clamp(0, 127);
    let uv_dc_index = usize::from(uv_dc_value.to_le_bytes()[0]);
    let uv_ac_index = usize::from(uv_ac_value.to_le_bytes()[0]);
    let (uv, q_uv) = expand_matrix(Y_DC_QUANT[uv_dc_index], Y_AC_QUANT[uv_ac_index], 2);
    SegmentMatrices {
        y1,
        y2,
        uv,
        lambda_i4: 3_i32
            .wrapping_mul(q_i4)
            .wrapping_mul(q_i4)
            .wrapping_shr(7)
            .max(1),
        lambda_i16: 3_i32.wrapping_mul(q_i16).wrapping_mul(q_i16).max(1),
        lambda_uv: 3_i32
            .wrapping_mul(q_uv)
            .wrapping_mul(q_uv)
            .wrapping_shr(6)
            .max(1),
        lambda_mode: q_i4.wrapping_mul(q_i4).wrapping_shr(7).max(1),
        texture_lambda: 50_i32.wrapping_mul(q_i4).wrapping_shr(5).max(1),
        lambda_trellis_i4: 7_i32
            .wrapping_mul(q_i4)
            .wrapping_mul(q_i4)
            .wrapping_shr(3)
            .max(1),
        lambda_trellis_i16: q_i16.wrapping_mul(q_i16).wrapping_shr(2).max(1),
    }
}

const ZIGZAG: [usize; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];

#[derive(Clone, Copy)]
struct TrellisNode {
    previous: u8,
    negative: bool,
    level: i16,
}

/// Trellis quantization used by libwebp's method 6 luma search.
pub(super) fn trellis_quantize_block(
    coefficients: &mut [i16; 16],
    levels: &mut [i16; 16],
    initial_context: usize,
    coefficient_type: usize,
    matrix: &QuantMatrix,
    lambda: i32,
    probabilities: &[[[[u8; 11]; 3]; 8]; 4],
) -> bool {
    const WEIGHTS: [i64; 16] = [30, 27, 19, 11, 27, 24, 17, 10, 19, 17, 12, 8, 11, 10, 8, 6];
    const MAX_LEVEL: u32 = 2_047;
    const MAX_SCORE: i64 = i64::MAX / 4;
    let first = usize::from(coefficient_type == 0);
    let threshold = i32::from(matrix.q[1]).pow(2) / 4;
    let mut last = first.wrapping_sub(1);
    for position in (first..16).rev() {
        let coefficient = i32::from(coefficients[ZIGZAG[position]]);
        if coefficient.wrapping_mul(coefficient) > threshold {
            last = position;
            break;
        }
    }
    if last == usize::MAX {
        last = first;
    } else if last < 15 {
        last = last.wrapping_add(1);
    }

    let initial =
        &probabilities[coefficient_type][usize::from(COEFF_BANDS[first])][initial_context];
    let mut best_score = i64::from(lambda).wrapping_mul(i64::from(bit_cost(false, initial[0])));
    let mut best_path: Option<(usize, usize, usize)> = None;
    let mut nodes = [[TrellisNode {
        previous: 0,
        negative: false,
        level: 0,
    }; 2]; 16];
    let entry_score = if initial_context == 0 {
        i64::from(lambda).wrapping_mul(i64::from(bit_cost(true, initial[0])))
    } else {
        0
    };
    let mut previous_scores = [entry_score; 2];
    let mut previous_contexts = [initial_context; 2];

    for position in first..=last {
        let coefficient_index = ZIGZAG[position];
        let signed = i32::from(coefficients[coefficient_index]);
        let negative = signed < 0;
        let coefficient = signed
            .unsigned_abs()
            .wrapping_add(u32::from(matrix.sharpen[coefficient_index]));
        let reciprocal = u32::from(matrix.reciprocal[coefficient_index]);
        let product = coefficient.wrapping_mul(reciprocal);
        let level0 = product.wrapping_shr(17).min(MAX_LEVEL);
        let threshold_level = product
            .wrapping_add(0x80_u32.wrapping_shl(9))
            .wrapping_shr(17)
            .min(MAX_LEVEL);
        let mut current_scores = [MAX_SCORE; 2];
        let mut current_contexts = [0; 2];
        for delta in 0_usize..2 {
            let level = level0.wrapping_add(delta.to_le_bytes()[0].into());
            if level > threshold_level {
                continue;
            }
            let quantized_error = i64::from(coefficient).wrapping_sub(
                i64::from(level).wrapping_mul(i64::from(matrix.q[coefficient_index])),
            );
            let original_error = i64::from(coefficient);
            let distortion_delta = WEIGHTS[coefficient_index].wrapping_mul(
                quantized_error
                    .wrapping_mul(quantized_error)
                    .wrapping_sub(original_error.wrapping_mul(original_error)),
            );
            let mut selected_score = MAX_SCORE;
            let mut selected_previous = 0;
            for previous in 0..2 {
                let probs = &probabilities[coefficient_type][usize::from(COEFF_BANDS[position])]
                    [previous_contexts[previous]];
                let score = previous_scores[previous].wrapping_add(i64::from(lambda).wrapping_mul(
                    i64::from(level_cost(
                        level as usize,
                        probs,
                        previous_contexts[previous],
                    )),
                ));
                if score < selected_score {
                    selected_score = score;
                    selected_previous = previous;
                }
            }
            selected_score = selected_score.wrapping_add(256_i64.wrapping_mul(distortion_delta));
            nodes[position][delta] = TrellisNode {
                previous: selected_previous.to_le_bytes()[0],
                negative,
                level: i16::from_le_bytes([level.to_le_bytes()[0], level.to_le_bytes()[1]]),
            };
            current_scores[delta] = selected_score;
            current_contexts[delta] = if level > 2 {
                2
            } else {
                usize::from(level.to_le_bytes()[0])
            };
            if level != 0 && selected_score < best_score {
                let terminal = if position < 15 {
                    let probs = &probabilities[coefficient_type]
                        [usize::from(COEFF_BANDS[position.wrapping_add(1)])]
                        [current_contexts[delta]];
                    i64::from(lambda).wrapping_mul(i64::from(bit_cost(false, probs[0])))
                } else {
                    0
                };
                let score = selected_score.wrapping_add(terminal);
                if score < best_score {
                    best_score = score;
                    best_path = Some((position, delta, selected_previous));
                }
            }
        }
        previous_scores = current_scores;
        previous_contexts = current_contexts;
    }

    let clear_from = first;
    for position in clear_from..16 {
        coefficients[ZIGZAG[position]] = 0;
        levels[position] = 0;
    }
    let Some((mut position, mut node, terminal_previous)) = best_path else {
        return false;
    };
    nodes[position][node].previous = terminal_previous.to_le_bytes()[0];
    loop {
        let selected = nodes[position][node];
        let signed_level = if selected.negative {
            selected.level.wrapping_neg()
        } else {
            selected.level
        };
        levels[position] = signed_level;
        let q = matrix.q[ZIGZAG[position]];
        coefficients[ZIGZAG[position]] =
            signed_level.wrapping_mul(i16::from_le_bytes(q.to_le_bytes()));
        node = usize::from(selected.previous);
        if position == first {
            break;
        }
        position = position.wrapping_sub(1);
    }
    true
}

/// Quantizes one transform block using libwebp's lossy VP8 scalar quantizer.
///
/// `coefficients` are replaced with their dequantized reconstruction values,
/// while the returned levels use VP8 zigzag order.
#[inline(always)]
pub(super) fn quantize_block(
    coefficients: &mut [i16; 16],
    levels: &mut [i16; 16],
    matrix: &QuantMatrix,
) -> bool {
    quantize_block_with_control(coefficients, levels, matrix, || Ok(())).unwrap_or_default()
}

/// Quantizes one transform block while polling after each coefficient.
pub(super) fn quantize_block_with_control<F>(
    coefficients: &mut [i16; 16],
    levels: &mut [i16; 16],
    matrix: &QuantMatrix,
    mut checkpoint: F,
) -> CodecResult<bool>
where
    F: FnMut() -> CodecResult<()>,
{
    const MAX_LEVEL: u32 = 2_047;

    let mut nonzero = false;
    for (zigzag_index, &coefficient_index) in ZIGZAG.iter().enumerate() {
        let signed_coefficient = i32::from(coefficients[coefficient_index]);
        let negative = signed_coefficient < 0;
        let coefficient = signed_coefficient
            .unsigned_abs()
            .wrapping_add(u32::from(matrix.sharpen[coefficient_index]));
        if coefficient > matrix.zero_threshold[coefficient_index] {
            let level_u32 = coefficient
                .wrapping_mul(u32::from(matrix.reciprocal[coefficient_index]))
                .wrapping_add(matrix.bias[coefficient_index])
                .wrapping_shr(17)
                .min(MAX_LEVEL);
            let mut level = i32::from_le_bytes(level_u32.to_le_bytes());
            if negative {
                level = level.wrapping_neg();
            }
            let reconstructed = level.wrapping_mul(i32::from(matrix.q[coefficient_index]));
            coefficients[coefficient_index] = i16::from_le_bytes([
                reconstructed.to_le_bytes()[0],
                reconstructed.to_le_bytes()[1],
            ]);
            levels[zigzag_index] =
                i16::from_le_bytes([level.to_le_bytes()[0], level.to_le_bytes()[1]]);
            nonzero |= level != 0;
        } else {
            coefficients[coefficient_index] = 0;
            levels[zigzag_index] = 0;
        }
        checkpoint()?;
    }
    Ok(nonzero)
}
