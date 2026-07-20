//! 4×4 forward DCT and 4×4 Walsh-Hadamard Transform for VP8 (RFC 6386 Section 14).

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
