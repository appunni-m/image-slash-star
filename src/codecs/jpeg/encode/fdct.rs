// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

// ── Forward DCT, IJG ISLOW (libjpeg-turbo 3.1.4.1 jfdctint.c) ─────────────
//
// 8×8 forward DCT in scaled fixed-point (CONST_BITS=13, PASS1_BITS=2).
// Input: 64 sample values already level-shifted (sample - 128) in natural
// order.  Output: DCT coefficients in natural order, scaled up by a factor
// of 8 (the factor of 8 is removed by the quantization step, see jcdctmgr.c).

use wide::i32x4;

const CONST_BITS: i32 = 13;
const PASS1_BITS: i32 = 2;

const FIX_0_298631336: i32 = 2446;
const FIX_0_390180644: i32 = 3196;
const FIX_0_541196100: i32 = 4433;
const FIX_0_765366865: i32 = 6270;
const FIX_0_899976223: i32 = 7373;
const FIX_1_175875602: i32 = 9633;
const FIX_1_501321110: i32 = 12299;
const FIX_1_847759065: i32 = 15137;
const FIX_1_961570560: i32 = 16069;
const FIX_2_053119869: i32 = 16819;
const FIX_2_562915447: i32 = 20995;
const FIX_3_072711026: i32 = 25172;

const FOUR: i32x4 = i32x4::new([4; 4]);
const FIX_0_298631336_X4: i32x4 = i32x4::new([FIX_0_298631336; 4]);
const FIX_0_390180644_X4: i32x4 = i32x4::new([FIX_0_390180644; 4]);
const FIX_0_541196100_X4: i32x4 = i32x4::new([FIX_0_541196100; 4]);
const FIX_0_765366865_X4: i32x4 = i32x4::new([FIX_0_765366865; 4]);
const FIX_0_899976223_X4: i32x4 = i32x4::new([FIX_0_899976223; 4]);
const FIX_1_175875602_X4: i32x4 = i32x4::new([FIX_1_175875602; 4]);
const FIX_1_501321110_X4: i32x4 = i32x4::new([FIX_1_501321110; 4]);
const FIX_1_847759065_X4: i32x4 = i32x4::new([FIX_1_847759065; 4]);
const FIX_1_961570560_X4: i32x4 = i32x4::new([FIX_1_961570560; 4]);
const FIX_2_053119869_X4: i32x4 = i32x4::new([FIX_2_053119869; 4]);
const FIX_2_562915447_X4: i32x4 = i32x4::new([FIX_2_562915447; 4]);
const FIX_3_072711026_X4: i32x4 = i32x4::new([FIX_3_072711026; 4]);
const NEG_FIX_0_390180644_X4: i32x4 = i32x4::new([-FIX_0_390180644; 4]);
const NEG_FIX_0_899976223_X4: i32x4 = i32x4::new([-FIX_0_899976223; 4]);
const NEG_FIX_1_847759065_X4: i32x4 = i32x4::new([-FIX_1_847759065; 4]);
const NEG_FIX_1_961570560_X4: i32x4 = i32x4::new([-FIX_1_961570560; 4]);
const NEG_FIX_2_562915447_X4: i32x4 = i32x4::new([-FIX_2_562915447; 4]);
const DESCALE_BIAS_2_X4: i32x4 = i32x4::new([2; 4]);
const DESCALE_BIAS_11_X4: i32x4 = i32x4::new([1_024; 4]);
const DESCALE_BIAS_15_X4: i32x4 = i32x4::new([16_384; 4]);

#[inline(always)]
fn descale(x: i32, n: i32) -> i32 {
    // IJG DESCALE: round-to-nearest arithmetic right shift.
    add(x, 1i32.wrapping_shl(n.saturating_sub(1).cast_unsigned())).wrapping_shr(n.cast_unsigned())
}

fn add(left: i32, right: i32) -> i32 {
    left.saturating_add(right)
}

fn add3(first: i32, second: i32, third: i32) -> i32 {
    add(add(first, second), third)
}

fn sub(left: i32, right: i32) -> i32 {
    left.saturating_sub(right)
}

fn mul(left: i32, right: i32) -> i32 {
    left.saturating_mul(right)
}

/// Forward DCT on one 8×8 block.  `data` is natural-order samples in/out.
pub(crate) fn fdct_islow(data: &mut [i32; 64]) {
    // Pass 1: process rows.  Results scaled up by 2^PASS1_BITS.
    for ctr in 0usize..8 {
        let row = ctr.saturating_mul(8);
        let (tmp0, tmp7) = (
            add(data[row], data[row.saturating_add(7)]),
            sub(data[row], data[row.saturating_add(7)]),
        );
        let (tmp1, tmp6) = (
            add(data[row.saturating_add(1)], data[row.saturating_add(6)]),
            sub(data[row.saturating_add(1)], data[row.saturating_add(6)]),
        );
        let (tmp2, tmp5) = (
            add(data[row.saturating_add(2)], data[row.saturating_add(5)]),
            sub(data[row.saturating_add(2)], data[row.saturating_add(5)]),
        );
        let (tmp3, tmp4) = (
            add(data[row.saturating_add(3)], data[row.saturating_add(4)]),
            sub(data[row.saturating_add(3)], data[row.saturating_add(4)]),
        );

        let tmp10 = add(tmp0, tmp3);
        let tmp13 = sub(tmp0, tmp3);
        let tmp11 = add(tmp1, tmp2);
        let tmp12 = sub(tmp1, tmp2);

        data[row] = add(tmp10, tmp11).wrapping_shl(PASS1_BITS.cast_unsigned());
        data[row.saturating_add(4)] = sub(tmp10, tmp11).wrapping_shl(PASS1_BITS.cast_unsigned());

        let z1 = mul(add(tmp12, tmp13), FIX_0_541196100);
        data[row.saturating_add(2)] = descale(
            add(z1, mul(tmp13, FIX_0_765366865)),
            CONST_BITS.saturating_sub(PASS1_BITS),
        );
        data[row.saturating_add(6)] = descale(
            add(z1, mul(tmp12, -FIX_1_847759065)),
            CONST_BITS.saturating_sub(PASS1_BITS),
        );

        let z1 = add(tmp4, tmp7);
        let z2 = add(tmp5, tmp6);
        let z3 = add(tmp4, tmp6);
        let z4 = add(tmp5, tmp7);
        let z5 = mul(add(z3, z4), FIX_1_175875602);

        let t4 = mul(tmp4, FIX_0_298631336);
        let t5 = mul(tmp5, FIX_2_053119869);
        let t6 = mul(tmp6, FIX_3_072711026);
        let t7 = mul(tmp7, FIX_1_501321110);
        let z1 = mul(z1, -FIX_0_899976223);
        let z2 = mul(z2, -FIX_2_562915447);
        let z3 = add(mul(z3, -FIX_1_961570560), z5);
        let z4 = add(mul(z4, -FIX_0_390180644), z5);

        let scale = CONST_BITS.saturating_sub(PASS1_BITS);
        data[row.saturating_add(7)] = descale(add3(t4, z1, z3), scale);
        data[row.saturating_add(5)] = descale(add3(t5, z2, z4), scale);
        data[row.saturating_add(3)] = descale(add3(t6, z2, z3), scale);
        data[row.saturating_add(1)] = descale(add3(t7, z1, z4), scale);
    }

    // Pass 2: process columns.  Remove PASS1_BITS scaling; results scaled by 8.
    for ctr in 0usize..8 {
        let col = ctr;
        let (tmp0, tmp7) = (
            add(data[col], data[col.saturating_add(56)]),
            sub(data[col], data[col.saturating_add(56)]),
        );
        let (tmp1, tmp6) = (
            add(data[col.saturating_add(8)], data[col.saturating_add(48)]),
            sub(data[col.saturating_add(8)], data[col.saturating_add(48)]),
        );
        let (tmp2, tmp5) = (
            add(data[col.saturating_add(16)], data[col.saturating_add(40)]),
            sub(data[col.saturating_add(16)], data[col.saturating_add(40)]),
        );
        let (tmp3, tmp4) = (
            add(data[col.saturating_add(24)], data[col.saturating_add(32)]),
            sub(data[col.saturating_add(24)], data[col.saturating_add(32)]),
        );

        let tmp10 = add(tmp0, tmp3);
        let tmp13 = sub(tmp0, tmp3);
        let tmp11 = add(tmp1, tmp2);
        let tmp12 = sub(tmp1, tmp2);

        data[col] = descale(add(tmp10, tmp11), PASS1_BITS);
        data[col.saturating_add(32)] = descale(sub(tmp10, tmp11), PASS1_BITS);

        let z1 = mul(add(tmp12, tmp13), FIX_0_541196100);
        let scale = CONST_BITS.saturating_add(PASS1_BITS);
        data[col.saturating_add(16)] = descale(add(z1, mul(tmp13, FIX_0_765366865)), scale);
        data[col.saturating_add(48)] = descale(add(z1, mul(tmp12, -FIX_1_847759065)), scale);

        let z1 = add(tmp4, tmp7);
        let z2 = add(tmp5, tmp6);
        let z3 = add(tmp4, tmp6);
        let z4 = add(tmp5, tmp7);
        let z5 = mul(add(z3, z4), FIX_1_175875602);

        let t4 = mul(tmp4, FIX_0_298631336);
        let t5 = mul(tmp5, FIX_2_053119869);
        let t6 = mul(tmp6, FIX_3_072711026);
        let t7 = mul(tmp7, FIX_1_501321110);
        let z1 = mul(z1, -FIX_0_899976223);
        let z2 = mul(z2, -FIX_2_562915447);
        let z3 = add(mul(z3, -FIX_1_961570560), z5);
        let z4 = add(mul(z4, -FIX_0_390180644), z5);

        data[col.saturating_add(56)] = descale(add3(t4, z1, z3), scale);
        data[col.saturating_add(40)] = descale(add3(t5, z2, z4), scale);
        data[col.saturating_add(24)] = descale(add3(t6, z2, z3), scale);
        data[col.saturating_add(8)] = descale(add3(t7, z1, z4), scale);
    }
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "valid level-shifted JPEG samples keep every four-lane FDCT intermediate in i32 range"
)]
#[inline(always)]
fn fdct_line_four(values: [i32x4; 8], first_pass: bool) -> [i32x4; 8] {
    let (tmp0, tmp7) = (values[0] + values[7], values[0] - values[7]);
    let (tmp1, tmp6) = (values[1] + values[6], values[1] - values[6]);
    let (tmp2, tmp5) = (values[2] + values[5], values[2] - values[5]);
    let (tmp3, tmp4) = (values[3] + values[4], values[3] - values[4]);

    let tmp10 = tmp0 + tmp3;
    let tmp13 = tmp0 - tmp3;
    let tmp11 = tmp1 + tmp2;
    let tmp12 = tmp1 - tmp2;

    let dc = tmp10 + tmp11;
    let fourth = tmp10 - tmp11;
    let output0 = if first_pass {
        dc * FOUR
    } else {
        fdct_descale_four(dc, 2)
    };
    let output4 = if first_pass {
        fourth * FOUR
    } else {
        fdct_descale_four(fourth, 2)
    };

    let z1 = (tmp12 + tmp13) * FIX_0_541196100_X4;
    let even_scale = if first_pass { 11 } else { 15 };
    let output2 = fdct_descale_four(z1 + tmp13 * FIX_0_765366865_X4, even_scale);
    let output6 = fdct_descale_four(z1 + tmp12 * NEG_FIX_1_847759065_X4, even_scale);

    let z1 = tmp4 + tmp7;
    let z2 = tmp5 + tmp6;
    let z3 = tmp4 + tmp6;
    let z4 = tmp5 + tmp7;
    let z5 = (z3 + z4) * FIX_1_175875602_X4;

    let t4 = tmp4 * FIX_0_298631336_X4;
    let t5 = tmp5 * FIX_2_053119869_X4;
    let t6 = tmp6 * FIX_3_072711026_X4;
    let t7 = tmp7 * FIX_1_501321110_X4;
    let z1 = z1 * NEG_FIX_0_899976223_X4;
    let z2 = z2 * NEG_FIX_2_562915447_X4;
    let z3 = z3 * NEG_FIX_1_961570560_X4 + z5;
    let z4 = z4 * NEG_FIX_0_390180644_X4 + z5;

    let output7 = fdct_descale_four(t4 + z1 + z3, even_scale);
    let output5 = fdct_descale_four(t5 + z2 + z4, even_scale);
    let output3 = fdct_descale_four(t6 + z2 + z3, even_scale);
    let output1 = fdct_descale_four(t7 + z1 + z4, even_scale);
    [
        output0, output1, output2, output3, output4, output5, output6, output7,
    ]
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "the fixed JPEG descale shifts are 2, 11, or 15 and intermediates are in range"
)]
#[inline(always)]
fn fdct_descale_four(value: i32x4, shift: u32) -> i32x4 {
    let bias = match shift {
        2 => DESCALE_BIAS_2_X4,
        11 => DESCALE_BIAS_11_X4,
        15 => DESCALE_BIAS_15_X4,
        _ => i32x4::ZERO,
    };
    (value + bias).unbounded_shr_scalar(shift)
}

#[inline(always)]
fn load_four(data: &[[i32; 64]; 4], coefficient: usize) -> i32x4 {
    i32x4::new([
        data[0][coefficient],
        data[1][coefficient],
        data[2][coefficient],
        data[3][coefficient],
    ])
}

#[inline(always)]
fn store_four(data: &mut [[i32; 64]; 4], coefficient: usize, value: i32x4) {
    let lanes = value.to_array();
    data[0][coefficient] = lanes[0];
    data[1][coefficient] = lanes[1];
    data[2][coefficient] = lanes[2];
    data[3][coefficient] = lanes[3];
}

/// Run the exact IJG ISLOW transform for four independent blocks in parallel.
/// Each vector lane reads and writes one of four complete block arrays. This
/// is safe portable SIMD: fixed-size arrays define every load and store.
pub(crate) fn fdct_islow_four(data: &mut [[i32; 64]; 4]) {
    let transformed = fdct_islow_four_coefficient_major(data);
    for (coefficient, value) in transformed.into_iter().enumerate() {
        store_four(data, coefficient, value);
    }
}

/// Run four transforms while retaining their native coefficient-major SIMD
/// layout. The returned vector at index `k` contains coefficient `k` for the
/// four input blocks; an entropy producer can consume those lanes directly
/// without a block-major transpose.
pub(crate) fn fdct_islow_four_coefficient_major(data: &mut [[i32; 64]; 4]) -> [i32x4; 64] {
    debug_assert!(
        data.iter()
            .flatten()
            .all(|sample| (-128..=127).contains(sample))
    );

    for row in 0usize..8 {
        let base = row.saturating_mul(8);
        let transformed = fdct_line_four(
            [
                load_four(data, base),
                load_four(data, base.saturating_add(1)),
                load_four(data, base.saturating_add(2)),
                load_four(data, base.saturating_add(3)),
                load_four(data, base.saturating_add(4)),
                load_four(data, base.saturating_add(5)),
                load_four(data, base.saturating_add(6)),
                load_four(data, base.saturating_add(7)),
            ],
            true,
        );
        for (column, value) in transformed.into_iter().enumerate() {
            store_four(data, base.saturating_add(column), value);
        }
    }

    let mut output = [i32x4::ZERO; 64];
    for column in 0usize..8 {
        let transformed = fdct_line_four(
            [
                load_four(data, column),
                load_four(data, column.saturating_add(8)),
                load_four(data, column.saturating_add(16)),
                load_four(data, column.saturating_add(24)),
                load_four(data, column.saturating_add(32)),
                load_four(data, column.saturating_add(40)),
                load_four(data, column.saturating_add(48)),
                load_four(data, column.saturating_add(56)),
            ],
            false,
        );
        for (row, value) in transformed.into_iter().enumerate() {
            output[column.saturating_add(row.saturating_mul(8))] = value;
        }
    }
    output
}

/// Run four transforms from an already coefficient-major SIMD workspace.
/// Keeping the workspace in vectors removes the scalar lane scatter/gather
/// between sample loading and both FDCT passes.
pub(crate) fn fdct_islow_four_coefficient_major_packed(data: &mut [i32x4; 64]) {
    debug_assert!(
        data.iter()
            .flat_map(|values| values.to_array())
            .all(|sample| (-128..=127).contains(&sample))
    );

    for row in 0usize..8 {
        let base = row.saturating_mul(8);
        let [first, second, third, fourth, fifth, sixth, seventh, eighth] = fdct_line_four(
            [
                data[base],
                data[base.saturating_add(1)],
                data[base.saturating_add(2)],
                data[base.saturating_add(3)],
                data[base.saturating_add(4)],
                data[base.saturating_add(5)],
                data[base.saturating_add(6)],
                data[base.saturating_add(7)],
            ],
            true,
        );
        data[base] = first;
        data[base.saturating_add(1)] = second;
        data[base.saturating_add(2)] = third;
        data[base.saturating_add(3)] = fourth;
        data[base.saturating_add(4)] = fifth;
        data[base.saturating_add(5)] = sixth;
        data[base.saturating_add(6)] = seventh;
        data[base.saturating_add(7)] = eighth;
    }

    for column in 0usize..8 {
        let [first, second, third, fourth, fifth, sixth, seventh, eighth] = fdct_line_four(
            [
                data[column],
                data[column.saturating_add(8)],
                data[column.saturating_add(16)],
                data[column.saturating_add(24)],
                data[column.saturating_add(32)],
                data[column.saturating_add(40)],
                data[column.saturating_add(48)],
                data[column.saturating_add(56)],
            ],
            false,
        );
        data[column] = first;
        data[column.saturating_add(8)] = second;
        data[column.saturating_add(16)] = third;
        data[column.saturating_add(24)] = fourth;
        data[column.saturating_add(32)] = fifth;
        data[column.saturating_add(40)] = sixth;
        data[column.saturating_add(48)] = seventh;
        data[column.saturating_add(56)] = eighth;
    }
}
