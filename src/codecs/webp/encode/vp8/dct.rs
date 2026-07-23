//! 4×4 forward DCT and 4×4 Walsh-Hadamard Transform for VP8 (RFC 6386 Section 14).

fn low_i16(value: i32) -> i16 {
    let bytes = value.to_le_bytes();
    i16::from_le_bytes([bytes[0], bytes[1]])
}

/// Apply libwebp's scaled integer VP8 forward transform to a 4×4 residual block.
///
/// This is the transform used by libwebp 1.6.0 for susceptibility analysis and
/// coefficient generation (`src/dsp/enc.c`, `FTransform_C`, lines 165–194).
pub fn vp8_fdct_4x4(block: &[i16; 16]) -> [i16; 16] {
    let mut temporary = [0i32; 16];
    for row in 0_usize..4 {
        let offset = row.wrapping_mul(4);
        let d0 = i32::from(block[offset]);
        let d1 = i32::from(block[offset.wrapping_add(1)]);
        let d2 = i32::from(block[offset.wrapping_add(2)]);
        let d3 = i32::from(block[offset.wrapping_add(3)]);
        let a0 = d0.wrapping_add(d3);
        let a1 = d1.wrapping_add(d2);
        let a2 = d1.wrapping_sub(d2);
        let a3 = d0.wrapping_sub(d3);
        temporary[offset] = a0.wrapping_add(a1).wrapping_mul(8);
        temporary[offset.wrapping_add(1)] = a2
            .wrapping_mul(2_217)
            .wrapping_add(a3.wrapping_mul(5_352))
            .wrapping_add(1_812)
            >> 9;
        temporary[offset.wrapping_add(2)] = a0.wrapping_sub(a1).wrapping_mul(8);
        temporary[offset.wrapping_add(3)] = a3
            .wrapping_mul(2_217)
            .wrapping_sub(a2.wrapping_mul(5_352))
            .wrapping_add(937)
            >> 9;
    }

    let mut output = [0i16; 16];
    for column in 0_usize..4 {
        let a0 = temporary[column].wrapping_add(temporary[12_usize.wrapping_add(column)]);
        let a1 = temporary[4_usize.wrapping_add(column)]
            .wrapping_add(temporary[8_usize.wrapping_add(column)]);
        let a2 = temporary[4_usize.wrapping_add(column)]
            .wrapping_sub(temporary[8_usize.wrapping_add(column)]);
        let a3 = temporary[column].wrapping_sub(temporary[12_usize.wrapping_add(column)]);
        output[column] = low_i16(a0.wrapping_add(a1).wrapping_add(7) >> 4);
        output[4_usize.wrapping_add(column)] = low_i16(
            (a2.wrapping_mul(2_217)
                .wrapping_add(a3.wrapping_mul(5_352))
                .wrapping_add(12_000)
                >> 16)
                .wrapping_add(i32::from(a3 != 0)),
        );
        output[8_usize.wrapping_add(column)] = low_i16(a0.wrapping_sub(a1).wrapping_add(7) >> 4);
        output[12_usize.wrapping_add(column)] = low_i16(
            a3.wrapping_mul(2_217)
                .wrapping_sub(a2.wrapping_mul(5_352))
                .wrapping_add(51_000)
                >> 16,
        );
    }
    output
}

/// Applies libwebp's integer VP8 inverse transform to a prediction block.
pub fn vp8_idct_add_4x4(prediction: &[u8; 16], coefficients: &[i16; 16]) -> [u8; 16] {
    fn multiply_one(value: i32) -> i32 {
        (value.wrapping_mul(20_091) >> 16).wrapping_add(value)
    }

    fn multiply_two(value: i32) -> i32 {
        value.wrapping_mul(35_468) >> 16
    }

    let mut temporary = [0i32; 16];
    for column in 0_usize..4 {
        let dc = i32::from(coefficients[column]);
        let ac1 = i32::from(coefficients[4_usize.wrapping_add(column)]);
        let ac2 = i32::from(coefficients[8_usize.wrapping_add(column)]);
        let ac3 = i32::from(coefficients[12_usize.wrapping_add(column)]);
        let a = dc.wrapping_add(ac2);
        let b = dc.wrapping_sub(ac2);
        let c = multiply_two(ac1).wrapping_sub(multiply_one(ac3));
        let d = multiply_one(ac1).wrapping_add(multiply_two(ac3));
        let offset = column.wrapping_mul(4);
        temporary[offset] = a.wrapping_add(d);
        temporary[offset.wrapping_add(1)] = b.wrapping_add(c);
        temporary[offset.wrapping_add(2)] = b.wrapping_sub(c);
        temporary[offset.wrapping_add(3)] = a.wrapping_sub(d);
    }

    let mut output = [0u8; 16];
    for row in 0_usize..4 {
        let dc = temporary[row].wrapping_add(4);
        let ac1 = temporary[4_usize.wrapping_add(row)];
        let ac2 = temporary[8_usize.wrapping_add(row)];
        let ac3 = temporary[12_usize.wrapping_add(row)];
        let a = dc.wrapping_add(ac2);
        let b = dc.wrapping_sub(ac2);
        let c = multiply_two(ac1).wrapping_sub(multiply_one(ac3));
        let d = multiply_one(ac1).wrapping_add(multiply_two(ac3));
        let residuals = [
            a.wrapping_add(d),
            b.wrapping_add(c),
            b.wrapping_sub(c),
            a.wrapping_sub(d),
        ];
        for (column, &residual) in residuals.iter().enumerate() {
            let offset = row.wrapping_mul(4).wrapping_add(column);
            output[offset] = i32::from(prediction[offset])
                .wrapping_add(residual >> 3)
                .clamp(0, 255)
                .to_le_bytes()[0];
        }
    }
    output
}

/// Applies libwebp's encoder-side VP8 Walsh-Hadamard transform to luma DCs.
pub fn vp8_fwht_4x4(block: &[i16; 16]) -> [i16; 16] {
    let mut temporary = [0i32; 16];
    for row in 0_usize..4 {
        let offset = row.wrapping_mul(4);
        let a0 = i32::from(block[offset]).wrapping_add(i32::from(block[offset.wrapping_add(2)]));
        let a1 = i32::from(block[offset.wrapping_add(1)])
            .wrapping_add(i32::from(block[offset.wrapping_add(3)]));
        let a2 = i32::from(block[offset.wrapping_add(1)])
            .wrapping_sub(i32::from(block[offset.wrapping_add(3)]));
        let a3 = i32::from(block[offset]).wrapping_sub(i32::from(block[offset.wrapping_add(2)]));
        temporary[offset] = a0.wrapping_add(a1);
        temporary[offset.wrapping_add(1)] = a3.wrapping_add(a2);
        temporary[offset.wrapping_add(2)] = a3.wrapping_sub(a2);
        temporary[offset.wrapping_add(3)] = a0.wrapping_sub(a1);
    }

    let mut output = [0i16; 16];
    for column in 0_usize..4 {
        let a0 = temporary[column].wrapping_add(temporary[8_usize.wrapping_add(column)]);
        let a1 = temporary[4_usize.wrapping_add(column)]
            .wrapping_add(temporary[12_usize.wrapping_add(column)]);
        let a2 = temporary[4_usize.wrapping_add(column)]
            .wrapping_sub(temporary[12_usize.wrapping_add(column)]);
        let a3 = temporary[column].wrapping_sub(temporary[8_usize.wrapping_add(column)]);
        output[column] = low_i16(a0.wrapping_add(a1) >> 1);
        output[4_usize.wrapping_add(column)] = low_i16(a3.wrapping_add(a2) >> 1);
        output[8_usize.wrapping_add(column)] = low_i16(a3.wrapping_sub(a2) >> 1);
        output[12_usize.wrapping_add(column)] = low_i16(a0.wrapping_sub(a1) >> 1);
    }
    output
}

/// Applies libwebp's decoder-side inverse VP8 Walsh-Hadamard transform.
pub fn vp8_iwht_4x4(block: &[i16; 16]) -> [i16; 16] {
    let mut temporary = [0i32; 16];
    for column in 0_usize..4 {
        let a0 =
            i32::from(block[column]).wrapping_add(i32::from(block[12_usize.wrapping_add(column)]));
        let a1 = i32::from(block[4_usize.wrapping_add(column)])
            .wrapping_add(i32::from(block[8_usize.wrapping_add(column)]));
        let a2 = i32::from(block[4_usize.wrapping_add(column)])
            .wrapping_sub(i32::from(block[8_usize.wrapping_add(column)]));
        let a3 =
            i32::from(block[column]).wrapping_sub(i32::from(block[12_usize.wrapping_add(column)]));
        temporary[column] = a0.wrapping_add(a1);
        temporary[8_usize.wrapping_add(column)] = a0.wrapping_sub(a1);
        temporary[4_usize.wrapping_add(column)] = a3.wrapping_add(a2);
        temporary[12_usize.wrapping_add(column)] = a3.wrapping_sub(a2);
    }

    let mut output = [0i16; 16];
    for row in 0_usize..4 {
        let offset = row.wrapping_mul(4);
        let dc = temporary[offset].wrapping_add(3);
        let a0 = dc.wrapping_add(temporary[offset.wrapping_add(3)]);
        let a1 = temporary[offset.wrapping_add(1)].wrapping_add(temporary[offset.wrapping_add(2)]);
        let a2 = temporary[offset.wrapping_add(1)].wrapping_sub(temporary[offset.wrapping_add(2)]);
        let a3 = dc.wrapping_sub(temporary[offset.wrapping_add(3)]);
        output[offset] = low_i16(a0.wrapping_add(a1) >> 3);
        output[offset.wrapping_add(1)] = low_i16(a3.wrapping_add(a2) >> 3);
        output[offset.wrapping_add(2)] = low_i16(a0.wrapping_sub(a1) >> 3);
        output[offset.wrapping_add(3)] = low_i16(a3.wrapping_sub(a2) >> 3);
    }
    output
}
