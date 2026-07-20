//! 4×4 forward DCT and 4×4 Walsh-Hadamard Transform for VP8 (RFC 6386 Section 14).

#![allow(dead_code)]

// ── 4×4 Forward DCT ──

/// Compute the 4×4 forward DCT.
///
/// Takes a 4×4 block (row-major, 16 i16 values) and returns the DCT coefficients.
/// This is used for both luma and chroma residual blocks.
///
/// The DCT is separable: DCT_2D = DCT_1D_rows ∘ DCT_1D_cols.
/// We use a scaled integer approach with f64 math for precision, then round to i16.
pub fn fdct_4x4(block: &[i16; 16]) -> [i16; 16] {
    let mut coeffs = [0.0f64; 16];

    // Convert to f64
    let input: [f64; 16] = std::array::from_fn(|i| block[i] as f64);

    // 1-D DCT on rows
    let mut rows = [0.0f64; 16];
    for r in 0..4 {
        let offset = r * 4;
        let r0 = input[offset];
        let r1 = input[offset + 1];
        let r2 = input[offset + 2];
        let r3 = input[offset + 3];
        rows[offset] = dct_1d_0(r0, r1, r2, r3);
        rows[offset + 1] = dct_1d_1(r0, r1, r2, r3);
        rows[offset + 2] = dct_1d_2(r0, r1, r2, r3);
        rows[offset + 3] = dct_1d_3(r0, r1, r2, r3);
    }

    // 1-D DCT on columns
    let mut result = [0i16; 16];
    for c in 0..4 {
        let c0 = rows[c];
        let c1 = rows[c + 4];
        let c2 = rows[c + 8];
        let c3 = rows[c + 12];
        coeffs[c] = dct_1d_0(c0, c1, c2, c3);
        coeffs[c + 4] = dct_1d_1(c0, c1, c2, c3);
        coeffs[c + 8] = dct_1d_2(c0, c1, c2, c3);
        coeffs[c + 12] = dct_1d_3(c0, c1, c2, c3);
    }

    for i in 0..16 {
        result[i] = coeffs[i].round() as i16;
    }
    result
}

/// Apply libwebp's scaled integer VP8 forward transform to a 4×4 residual block.
///
/// This is the transform used by libwebp 1.6.0 for susceptibility analysis and
/// coefficient generation (`src/dsp/enc.c`, `FTransform_C`, lines 165–194).
pub fn vp8_fdct_4x4(block: &[i16; 16]) -> [i16; 16] {
    let mut temporary = [0i32; 16];
    for row in 0..4 {
        let offset = row * 4;
        let d0 = i32::from(block[offset]);
        let d1 = i32::from(block[offset + 1]);
        let d2 = i32::from(block[offset + 2]);
        let d3 = i32::from(block[offset + 3]);
        let a0 = d0 + d3;
        let a1 = d1 + d2;
        let a2 = d1 - d2;
        let a3 = d0 - d3;
        temporary[offset] = (a0 + a1) * 8;
        temporary[offset + 1] = (a2 * 2_217 + a3 * 5_352 + 1_812) >> 9;
        temporary[offset + 2] = (a0 - a1) * 8;
        temporary[offset + 3] = (a3 * 2_217 - a2 * 5_352 + 937) >> 9;
    }

    let mut output = [0i16; 16];
    for column in 0..4 {
        let a0 = temporary[column] + temporary[12 + column];
        let a1 = temporary[4 + column] + temporary[8 + column];
        let a2 = temporary[4 + column] - temporary[8 + column];
        let a3 = temporary[column] - temporary[12 + column];
        output[column] = ((a0 + a1 + 7) >> 4) as i16;
        output[4 + column] =
            (((a2 * 2_217 + a3 * 5_352 + 12_000) >> 16) + i32::from(a3 != 0)) as i16;
        output[8 + column] = ((a0 - a1 + 7) >> 4) as i16;
        output[12 + column] = ((a3 * 2_217 - a2 * 5_352 + 51_000) >> 16) as i16;
    }
    output
}

/// Applies libwebp's integer VP8 inverse transform to a prediction block.
pub fn vp8_idct_add_4x4(prediction: &[u8; 16], coefficients: &[i16; 16]) -> [u8; 16] {
    fn multiply_one(value: i32) -> i32 {
        ((value * 20_091) >> 16) + value
    }

    fn multiply_two(value: i32) -> i32 {
        (value * 35_468) >> 16
    }

    let mut temporary = [0i32; 16];
    for column in 0..4 {
        let dc = i32::from(coefficients[column]);
        let ac1 = i32::from(coefficients[4 + column]);
        let ac2 = i32::from(coefficients[8 + column]);
        let ac3 = i32::from(coefficients[12 + column]);
        let a = dc + ac2;
        let b = dc - ac2;
        let c = multiply_two(ac1) - multiply_one(ac3);
        let d = multiply_one(ac1) + multiply_two(ac3);
        temporary[column * 4] = a + d;
        temporary[column * 4 + 1] = b + c;
        temporary[column * 4 + 2] = b - c;
        temporary[column * 4 + 3] = a - d;
    }

    let mut output = [0u8; 16];
    for row in 0..4 {
        let dc = temporary[row] + 4;
        let ac1 = temporary[4 + row];
        let ac2 = temporary[8 + row];
        let ac3 = temporary[12 + row];
        let a = dc + ac2;
        let b = dc - ac2;
        let c = multiply_two(ac1) - multiply_one(ac3);
        let d = multiply_one(ac1) + multiply_two(ac3);
        let residuals = [a + d, b + c, b - c, a - d];
        for column in 0..4 {
            output[row * 4 + column] = (i32::from(prediction[row * 4 + column])
                + (residuals[column] >> 3))
                .clamp(0, 255) as u8;
        }
    }
    output
}

/// Compute the inverse 4×4 DCT (for encoder reconstruction loop).
/// Same separable approach in reverse.
pub fn idct_4x4(coeffs: &[i16; 16]) -> [i16; 16] {
    let input: [f64; 16] = std::array::from_fn(|i| coeffs[i] as f64);

    // 1-D IDCT on columns
    let mut cols = [0.0f64; 16];
    for c in 0..4 {
        cols[c] = idct_1d_0(input[c], input[c + 4], input[c + 8], input[c + 12]);
        cols[c + 4] = idct_1d_1(input[c], input[c + 4], input[c + 8], input[c + 12]);
        cols[c + 8] = idct_1d_2(input[c], input[c + 4], input[c + 8], input[c + 12]);
        cols[c + 12] = idct_1d_3(input[c], input[c + 4], input[c + 8], input[c + 12]);
    }

    // 1-D IDCT on rows
    let mut result = [0i16; 16];
    for r in 0..4 {
        let offset = r * 4;
        result[offset] = idct_1d_0(
            cols[offset],
            cols[offset + 1],
            cols[offset + 2],
            cols[offset + 3],
        )
        .round() as i16;
        result[offset + 1] = idct_1d_1(
            cols[offset],
            cols[offset + 1],
            cols[offset + 2],
            cols[offset + 3],
        )
        .round() as i16;
        result[offset + 2] = idct_1d_2(
            cols[offset],
            cols[offset + 1],
            cols[offset + 2],
            cols[offset + 3],
        )
        .round() as i16;
        result[offset + 3] = idct_1d_3(
            cols[offset],
            cols[offset + 1],
            cols[offset + 2],
            cols[offset + 3],
        )
        .round() as i16;
    }
    result
}

// ── DCT basis functions ──
// cos(pi/16) ≈ 0.980785, cos(2pi/16) ≈ 0.923880, cos(3pi/16) ≈ 0.831470
// For k=0: scale factor 1/√2 ≈ 0.707107 is applied

const C1: f64 = 0.9807852804032304; // cos(π/16)
const C2: f64 = 0.9238795325112867; // cos(2π/16)
const C3: f64 = 0.8314696123025452; // cos(3π/16)
const S2: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// 1-D DCT for 4 points, coefficient k (output index).
fn dct_1d_k(a: f64, b: f64, c: f64, d: f64, k: usize) -> f64 {
    let scale = if k == 0 { S2 } else { 1.0 };
    let sum = a * cos_dct(0, k) + b * cos_dct(1, k) + c * cos_dct(2, k) + d * cos_dct(3, k);
    scale * 0.5 * sum
}

fn cos_dct(n: usize, k: usize) -> f64 {
    let angle = std::f64::consts::PI * (2 * n + 1) as f64 * k as f64 / 16.0;
    angle.cos()
}

fn dct_1d_0(a: f64, b: f64, c: f64, d: f64) -> f64 {
    dct_1d_k(a, b, c, d, 0)
}
fn dct_1d_1(a: f64, b: f64, c: f64, d: f64) -> f64 {
    dct_1d_k(a, b, c, d, 1)
}
fn dct_1d_2(a: f64, b: f64, c: f64, d: f64) -> f64 {
    dct_1d_k(a, b, c, d, 2)
}
fn dct_1d_3(a: f64, b: f64, c: f64, d: f64) -> f64 {
    dct_1d_k(a, b, c, d, 3)
}

// ── 1-D IDCT ──
// X[n] = sum_{k=0..3} scale_k * Y[k] * cos((2n+1)*k*pi/16)
// where scale_0 = 1/√2, scale_k = 1 for k>0

fn idct_1d_k(y0: f64, y1: f64, y2: f64, y3: f64, n: usize) -> f64 {
    0.5 * (S2 * y0 * cos_dct(n, 0) + y1 * cos_dct(n, 1) + y2 * cos_dct(n, 2) + y3 * cos_dct(n, 3))
}

fn idct_1d_0(y0: f64, y1: f64, y2: f64, y3: f64) -> f64 {
    idct_1d_k(y0, y1, y2, y3, 0)
}
fn idct_1d_1(y0: f64, y1: f64, y2: f64, y3: f64) -> f64 {
    idct_1d_k(y0, y1, y2, y3, 1)
}
fn idct_1d_2(y0: f64, y1: f64, y2: f64, y3: f64) -> f64 {
    idct_1d_k(y0, y1, y2, y3, 2)
}
fn idct_1d_3(y0: f64, y1: f64, y2: f64, y3: f64) -> f64 {
    idct_1d_k(y0, y1, y2, y3, 3)
}

// ── 4×4 Walsh-Hadamard Transform ──

/// Compute the 4×4 Walsh-Hadamard Transform on luma DC coefficients.
///
/// Applied to the 4×4 block of DC coefficients gathered from the 16 4×4 sub-blocks
/// within a 16×16 luma macroblock. The WHT provides additional energy compaction
/// for the DC components.
///
/// 1-D WHT with normalization:
///   out[0] = (in[0] + in[1] + in[2] + in[3]) / 2
///   out[1] = (in[0] + in[1] - in[2] - in[3]) / 2
///   out[2] = (in[0] - in[1] - in[2] + in[3]) / 2
///   out[3] = (in[0] - in[1] + in[2] - in[3]) / 2
pub fn wht_4x4(block: &[i16; 16]) -> [i16; 16] {
    let mut temp = [0i32; 16];

    // 1-D WHT on rows
    for r in 0..4 {
        let offset = r * 4;
        let a = block[offset] as i32;
        let b = block[offset + 1] as i32;
        let c = block[offset + 2] as i32;
        let d = block[offset + 3] as i32;
        temp[offset] = (a + b + c + d) >> 1;
        temp[offset + 1] = (a + b - c - d) >> 1;
        temp[offset + 2] = (a - b - c + d) >> 1;
        temp[offset + 3] = (a - b + c - d) >> 1;
    }

    // 1-D WHT on columns
    let mut result = [0i16; 16];
    for c in 0..4 {
        let a = temp[c];
        let b = temp[c + 4];
        let cc = temp[c + 8];
        let d = temp[c + 12];
        result[c] = ((a + b + cc + d) >> 1) as i16;
        result[c + 4] = ((a + b - cc - d) >> 1) as i16;
        result[c + 8] = ((a - b - cc + d) >> 1) as i16;
        result[c + 12] = ((a - b + cc - d) >> 1) as i16;
    }
    result
}

/// Inverse 4×4 WHT: same as forward WHT since it's its own inverse (up to scaling).
/// The normalization accumulates: forward applies two >>1, inverse applies two >>1.
/// Total: input × 0.25. For full reconstruction, add 2 (rounding) after each step.
pub fn iwht_4x4(block: &[i16; 16]) -> [i16; 16] {
    // Identical to forward WHT
    wht_4x4(block)
}

/// Applies libwebp's encoder-side VP8 Walsh-Hadamard transform to luma DCs.
pub fn vp8_fwht_4x4(block: &[i16; 16]) -> [i16; 16] {
    let mut temporary = [0i32; 16];
    for row in 0..4 {
        let offset = row * 4;
        let a0 = i32::from(block[offset]) + i32::from(block[offset + 2]);
        let a1 = i32::from(block[offset + 1]) + i32::from(block[offset + 3]);
        let a2 = i32::from(block[offset + 1]) - i32::from(block[offset + 3]);
        let a3 = i32::from(block[offset]) - i32::from(block[offset + 2]);
        temporary[offset] = a0 + a1;
        temporary[offset + 1] = a3 + a2;
        temporary[offset + 2] = a3 - a2;
        temporary[offset + 3] = a0 - a1;
    }

    let mut output = [0i16; 16];
    for column in 0..4 {
        let a0 = temporary[column] + temporary[8 + column];
        let a1 = temporary[4 + column] + temporary[12 + column];
        let a2 = temporary[4 + column] - temporary[12 + column];
        let a3 = temporary[column] - temporary[8 + column];
        output[column] = ((a0 + a1) >> 1) as i16;
        output[4 + column] = ((a3 + a2) >> 1) as i16;
        output[8 + column] = ((a3 - a2) >> 1) as i16;
        output[12 + column] = ((a0 - a1) >> 1) as i16;
    }
    output
}

/// Applies libwebp's decoder-side inverse VP8 Walsh-Hadamard transform.
pub fn vp8_iwht_4x4(block: &[i16; 16]) -> [i16; 16] {
    let mut temporary = [0i32; 16];
    for column in 0..4 {
        let a0 = i32::from(block[column]) + i32::from(block[12 + column]);
        let a1 = i32::from(block[4 + column]) + i32::from(block[8 + column]);
        let a2 = i32::from(block[4 + column]) - i32::from(block[8 + column]);
        let a3 = i32::from(block[column]) - i32::from(block[12 + column]);
        temporary[column] = a0 + a1;
        temporary[8 + column] = a0 - a1;
        temporary[4 + column] = a3 + a2;
        temporary[12 + column] = a3 - a2;
    }

    let mut output = [0i16; 16];
    for row in 0..4 {
        let offset = row * 4;
        let dc = temporary[offset] + 3;
        let a0 = dc + temporary[offset + 3];
        let a1 = temporary[offset + 1] + temporary[offset + 2];
        let a2 = temporary[offset + 1] - temporary[offset + 2];
        let a3 = dc - temporary[offset + 3];
        output[offset] = ((a0 + a1) >> 3) as i16;
        output[offset + 1] = ((a3 + a2) >> 3) as i16;
        output[offset + 2] = ((a0 - a1) >> 3) as i16;
        output[offset + 3] = ((a3 - a2) >> 3) as i16;
    }
    output
}
