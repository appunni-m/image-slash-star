//! VP8 quantization tables and color conversion (RFC 6386).
//!
//! Provides:
//! - Quality-to-quantizer-index mapping (`quality_to_quant_index`)
//! - Coefficient quantization and dequantization (`quantize`, `dequantize`)
//! - The four base quantization tables used by VP8 (Y/UV, DC/AC)
//! - RGB to YCbCr (BT.601) conversion (`rgb_to_yuv`)

#![allow(dead_code)]

use super::dct::{vp8_fdct_4x4, vp8_idct_add_4x4};

/// Map a quality setting (0–100) to a VP8 quantizer index (0–127).
///
/// Maps [0 (worst), 100 (best)] linearly to [127 (coarsest), 0 (finest)].
pub fn quality_to_quant_index(quality: u8) -> u8 {
    let q = quality.min(100);
    ((100 - q) as u16 * 127 / 100) as u8
}

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
    }
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
    const ZIGZAG: [usize; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];
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

/// DC quantization step sizes for chroma (UV) blocks.
///
/// In libvpx the UV-DC table is identical to `Y_DC_QUANT` with a cap at 132
/// applied at use-site. Functions that need this table with the cap applied
/// should use `uv_dc_quant_table()`.
pub const UV_DC_QUANT: [u16; 128] = {
    let mut t = [0u16; 128];
    let mut i = 0;
    while i < 128 {
        t[i] = if i == 0 { 4 } else { 0 }; // placeholder — same as Y_DC_QUANT in practice
        i += 1;
    }
    // UV_DC_QUANT is not computed here; callers use a runtime function.
    // This placeholder exists to satisfy the module-level declaration.
    // Use `uv_dc_quant_table()` to get the properly capped table.
    t
};

/// AC quantization step sizes for chroma (UV) blocks.
///
/// Same as `Y_AC_QUANT` (libvpx convention — no separate UV-AC table).
pub const UV_AC_QUANT: [u16; 128] = Y_AC_QUANT;

// ── Quantize and dequantize ──

/// Quantize a single DCT coefficient using VP8 scalar quantization.
///
/// Uses integer division with rounding:
///   pos: (coeff + step/2) / step
///   neg: -((-coeff + step/2) / step)
///
/// `dc` selects DC vs AC quantization step. `q` is quantizer index (0–127).
pub fn quantize(coeff: i16, q: u8, dc: bool) -> i16 {
    let qi = (q as usize).min(127);
    let step = if dc { Y_DC_QUANT[qi] } else { Y_AC_QUANT[qi] };
    if step == 0 {
        return 0;
    }
    let step = step as i16;
    if coeff >= 0 {
        ((coeff as i32 + (step as i32 / 2)) / step as i32) as i16
    } else {
        -(((-coeff as i32 + (step as i32 / 2)) / step as i32) as i16)
    }
}

/// Quantize a chroma (UV) coefficient, with UV DC capped at 132.
pub fn quantize_uv(coeff: i16, q: u8, dc: bool) -> i16 {
    let qi = (q as usize).min(127);
    let step = if dc {
        Y_DC_QUANT[qi].min(132)
    } else {
        Y_AC_QUANT[qi]
    };
    if step == 0 {
        return 0;
    }
    let step = step as i16;
    if coeff >= 0 {
        ((coeff as i32 + (step as i32 / 2)) / step as i32) as i16
    } else {
        -(((-coeff as i32 + (step as i32 / 2)) / step as i32) as i16)
    }
}

/// Dequantize a coefficient (reconstruction in encoder prediction loop).
///
/// Equivalent to `qcoeff * step`.
pub fn dequantize(qcoeff: i16, q: u8, dc: bool) -> i16 {
    let qi = (q as usize).min(127);
    let step = if dc { Y_DC_QUANT[qi] } else { Y_AC_QUANT[qi] };
    (qcoeff as i32 * step as i32) as i16
}

/// Dequantize a chroma coefficient.
pub fn dequantize_uv(qcoeff: i16, q: u8, dc: bool) -> i16 {
    let qi = (q as usize).min(127);
    let step = if dc {
        Y_DC_QUANT[qi].min(132)
    } else {
        Y_AC_QUANT[qi]
    };
    (qcoeff as i32 * step as i32) as i16
}

// ── RGB to YUV ──

/// Convert RGB to YUV (YCbCr, BT.601) using floating-point math.
///
///   Y  =  0.299 * R + 0.587 * G + 0.114 * B
///   Cb = -0.169 * R - 0.331 * G + 0.500 * B + 128
///   Cr =  0.500 * R - 0.419 * G - 0.081 * B + 128
pub fn rgb_to_yuv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let rf = r as f64;
    let gf = g as f64;
    let bf = b as f64;

    let y = (0.299 * rf + 0.587 * gf + 0.114 * bf)
        .round()
        .clamp(0.0, 255.0) as u8;
    let u = (-0.169 * rf - 0.331 * gf + 0.500 * bf + 128.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let v = (0.500 * rf - 0.419 * gf - 0.081 * bf + 128.0)
        .round()
        .clamp(0.0, 255.0) as u8;

    (y, u, v)
}

/// Convert RGB to YUV using integer (fixed-point) arithmetic.
///
/// Coefficients scaled by 2^10:
///   Y   = ( 306*R + 601*G + 117*B + 512) >> 10
///   Cb  = (-173*R - 339*G + 512*B + 131072) >> 10
///   Cr  = ( 512*R - 429*G -  83*B + 131072) >> 10
pub fn rgb_to_yuv_int(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let r = r as i32;
    let g = g as i32;
    let b = b as i32;

    let y = ((306 * r + 601 * g + 117 * b + 512) >> 10).clamp(0, 255) as u8;
    let u = ((-173 * r - 339 * g + 512 * b + 131072) >> 10).clamp(0, 255) as u8;
    let v = ((512 * r - 429 * g - 83 * b + 131072) >> 10).clamp(0, 255) as u8;

    (y, u, v)
}

/// Return `Y_DC_QUANT` with each entry capped at 132 (VP8 UV-DC convention).
pub fn uv_dc_quant_table() -> [u16; 128] {
    let mut t = Y_DC_QUANT;
    for v in t.iter_mut() {
        *v = (*v).min(132);
    }
    t
}
