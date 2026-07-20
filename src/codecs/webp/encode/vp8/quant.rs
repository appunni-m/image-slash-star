//! VP8 quantization tables and color conversion (RFC 6386).
//!
//! Provides:
//! - Quality-to-quantizer-index mapping (`quality_to_quant_index`)
//! - Coefficient quantization and dequantization (`quantize`, `dequantize`)
//! - The four base quantization tables used by VP8 (Y/UV, DC/AC)
//! - RGB to YCbCr (BT.601) conversion (`rgb_to_yuv`)

use super::{
    cost::{bit_cost, level_cost},
    dct::{vp8_fdct_4x4, vp8_idct_add_4x4},
    tokenize::COEFF_BANDS,
};

// ── VP8 quantization step tables ──
//
// These are the exact tables from libvpx (vp8/common/quant_common.c),
// implementing the base quantization step sizes for indices 0..127.

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
        matrix.reciprocal[index] = ((1_u32 << 17) / u32::from(matrix.q[index])) as u16;
        matrix.bias[index] = BIASES[kind][usize::from(index > 0)] << 9;
        matrix.zero_threshold[index] =
            ((1_u32 << 17) - 1 - matrix.bias[index]) / u32::from(matrix.reciprocal[index]);
    }
    for index in 2..16 {
        matrix.reciprocal[index] = matrix.reciprocal[1];
        matrix.bias[index] = matrix.bias[1];
        matrix.zero_threshold[index] = matrix.zero_threshold[1];
    }
    if kind == 0 {
        for (index, sharpen) in matrix.sharpen.iter_mut().enumerate() {
            *sharpen = SHARPENING[index] * matrix.q[index] >> 11;
        }
    }
    let average = (matrix.q.iter().map(|&value| i32::from(value)).sum::<i32>() + 8) >> 4;
    (matrix, average)
}

pub(super) fn libwebp_segment_matrices(
    quantizer: u8,
    chroma_dc_delta: i8,
    chroma_ac_delta: i8,
) -> SegmentMatrices {
    let quantizer = usize::from(quantizer);
    let (y1, q_i4) = expand_matrix(Y_DC_QUANT[quantizer], Y_AC_QUANT[quantizer], 0);
    let (y2, q_i16) = expand_matrix(Y_DC_QUANT[quantizer] * 2, Y2_AC_QUANT[quantizer], 1);
    let uv_dc_index = (quantizer as i32 + i32::from(chroma_dc_delta)).clamp(0, 117) as usize;
    let uv_ac_index = (quantizer as i32 + i32::from(chroma_ac_delta)).clamp(0, 127) as usize;
    let (uv, q_uv) = expand_matrix(Y_DC_QUANT[uv_dc_index], Y_AC_QUANT[uv_ac_index], 2);
    SegmentMatrices {
        y1,
        y2,
        uv,
        lambda_i4: ((3 * q_i4 * q_i4) >> 7).max(1),
        lambda_i16: (3 * q_i16 * q_i16).max(1),
        lambda_uv: ((3 * q_uv * q_uv) >> 6).max(1),
        lambda_mode: ((q_i4 * q_i4) >> 7).max(1),
        texture_lambda: ((50 * q_i4) >> 5).max(1),
        lambda_trellis_i4: ((7 * q_i4 * q_i4) >> 3).max(1),
        lambda_trellis_i16: ((q_i16 * q_i16) >> 2).max(1),
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
        if coefficient * coefficient > threshold {
            last = position;
            break;
        }
    }
    if last == usize::MAX {
        last = first;
    } else if last < 15 {
        last += 1;
    }

    let initial =
        &probabilities[coefficient_type][usize::from(COEFF_BANDS[first])][initial_context];
    let mut best_score = i64::from(lambda) * i64::from(bit_cost(false, initial[0]));
    let mut best_path: Option<(usize, usize, usize)> = None;
    let mut nodes = [[TrellisNode {
        previous: 0,
        negative: false,
        level: 0,
    }; 2]; 16];
    let entry_score = if initial_context == 0 {
        i64::from(lambda) * i64::from(bit_cost(true, initial[0]))
    } else {
        0
    };
    let mut previous_scores = [entry_score; 2];
    let mut previous_contexts = [initial_context; 2];

    for position in first..=last {
        let coefficient_index = ZIGZAG[position];
        let signed = i32::from(coefficients[coefficient_index]);
        let negative = signed < 0;
        let coefficient = signed.unsigned_abs() + u32::from(matrix.sharpen[coefficient_index]);
        let reciprocal = u32::from(matrix.reciprocal[coefficient_index]);
        let level0 = ((coefficient * reciprocal) >> 17).min(MAX_LEVEL);
        let threshold_level = ((coefficient * reciprocal + (0x80 << 9)) >> 17).min(MAX_LEVEL);
        let mut current_scores = [MAX_SCORE; 2];
        let mut current_contexts = [0; 2];
        for delta in 0..2 {
            let level = level0 + delta as u32;
            if level > threshold_level {
                continue;
            }
            let quantized_error =
                i64::from(coefficient) - i64::from(level) * i64::from(matrix.q[coefficient_index]);
            let original_error = i64::from(coefficient);
            let distortion_delta = WEIGHTS[coefficient_index]
                * (quantized_error * quantized_error - original_error * original_error);
            let mut selected_score = MAX_SCORE;
            let mut selected_previous = 0;
            for previous in 0..2 {
                let probs = &probabilities[coefficient_type][usize::from(COEFF_BANDS[position])]
                    [previous_contexts[previous]];
                let score = previous_scores[previous]
                    + i64::from(lambda)
                        * i64::from(level_cost(
                            level as usize,
                            probs,
                            previous_contexts[previous],
                        ));
                if score < selected_score {
                    selected_score = score;
                    selected_previous = previous;
                }
            }
            selected_score += 256 * distortion_delta;
            nodes[position][delta] = TrellisNode {
                previous: selected_previous as u8,
                negative,
                level: level as i16,
            };
            current_scores[delta] = selected_score;
            current_contexts[delta] = (level > 2).then_some(2).unwrap_or(level as usize);
            if level != 0 && selected_score < best_score {
                let terminal = if position < 15 {
                    let probs = &probabilities[coefficient_type]
                        [usize::from(COEFF_BANDS[position + 1])][current_contexts[delta]];
                    i64::from(lambda) * i64::from(bit_cost(false, probs[0]))
                } else {
                    0
                };
                let score = selected_score + terminal;
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
    nodes[position][node].previous = terminal_previous as u8;
    loop {
        let selected = nodes[position][node];
        let signed_level = if selected.negative {
            -selected.level
        } else {
            selected.level
        };
        levels[position] = signed_level;
        coefficients[ZIGZAG[position]] = signed_level * matrix.q[ZIGZAG[position]] as i16;
        node = usize::from(selected.previous);
        if position == first {
            break;
        }
        position -= 1;
    }
    true
}

/// Quantizes one transform block using libwebp's lossy VP8 scalar quantizer.
///
/// `coefficients` are replaced with their dequantized reconstruction values,
/// while the returned levels use VP8 zigzag order.
pub(super) fn quantize_block(
    coefficients: &mut [i16; 16],
    levels: &mut [i16; 16],
    matrix: &QuantMatrix,
) -> bool {
    const MAX_LEVEL: u32 = 2_047;

    let mut nonzero = false;
    for (zigzag_index, &coefficient_index) in ZIGZAG.iter().enumerate() {
        let signed_coefficient = i32::from(coefficients[coefficient_index]);
        let negative = signed_coefficient < 0;
        let coefficient =
            signed_coefficient.unsigned_abs() + u32::from(matrix.sharpen[coefficient_index]);
        if coefficient > matrix.zero_threshold[coefficient_index] {
            let mut level = ((coefficient * u32::from(matrix.reciprocal[coefficient_index])
                + matrix.bias[coefficient_index])
                >> 17)
                .min(MAX_LEVEL) as i32;
            if negative {
                level = -level;
            }
            coefficients[coefficient_index] =
                (level * i32::from(matrix.q[coefficient_index])) as i16;
            levels[zigzag_index] = level as i16;
            nonzero |= level != 0;
        } else {
            coefficients[coefficient_index] = 0;
            levels[zigzag_index] = 0;
        }
    }
    nonzero
}

/// Transforms, quantizes, and reconstructs one predicted 4×4 block.
pub(super) fn quantize_reconstruct_block(
    source: &[u8; 16],
    prediction: &[u8; 16],
    matrix: &QuantMatrix,
) -> (bool, [i16; 16], [u8; 16]) {
    let residual =
        std::array::from_fn(|index| i16::from(source[index]) - i16::from(prediction[index]));
    let mut coefficients = vp8_fdct_4x4(&residual);
    let mut levels = [0; 16];
    let nonzero = quantize_block(&mut coefficients, &mut levels, matrix);
    let reconstructed = vp8_idct_add_4x4(prediction, &coefficients);
    (nonzero, levels, reconstructed)
}
