// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

// ── Forward DCT, IJG ISLOW (libjpeg-turbo 3.1.4.1 jfdctint.c) ─────────────
//
// 8×8 forward DCT in scaled fixed-point (CONST_BITS=13, PASS1_BITS=2).
// Input: 64 sample values already level-shifted (sample - 128) in natural
// order.  Output: DCT coefficients in natural order, scaled up by a factor
// of 8 (the factor of 8 is removed by the quantization step, see jcdctmgr.c).

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
