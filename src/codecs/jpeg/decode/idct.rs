// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
use wide::bytemuck::{cast, cast_slice, pod_read_unaligned};
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
use wide::{i16x8, i32x4, i32x8, u8x16};

// ── IDCT Constants (matching IJG jidctint.c) ──────────────────────────────

pub(crate) const CONST_BITS: i32 = 13;
pub(crate) const PASS1_BITS: i32 = 2;

// FIX(x) = (i32)(x * (1 << CONST_BITS) + 0.5)
pub(crate) const FIX_0_298631336: i32 = 2446;
pub(crate) const FIX_0_390180644: i32 = 3196;
pub(crate) const FIX_0_541196100: i32 = 4433;
pub(crate) const FIX_0_765366865: i32 = 6270;
pub(crate) const FIX_0_899976223: i32 = 7373;
pub(crate) const FIX_1_175875602: i32 = 9633;
pub(crate) const FIX_1_501321110: i32 = 12299;
pub(crate) const FIX_1_847759065: i32 = 15137;
pub(crate) const FIX_1_961570560: i32 = 16069;
pub(crate) const FIX_2_053119869: i32 = 16819;
pub(crate) const FIX_2_562915447: i32 = 20995;
pub(crate) const FIX_3_072711026: i32 = 25172;

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
const VDESCALE_11_BIAS: i32x4 = i32x4::new([1 << 10; 4]);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
const VDESCALE_18_BIAS: i32x4 = i32x4::new([1 << 17; 4]);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
const VRANGE_CENTER: i32x4 = i32x4::new([128; 4]);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
const VRANGE_MAX: i32x4 = i32x4::new([255; 4]);

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
const AVX2_VDESCALE_11_BIAS: i32x8 = i32x8::new([1 << 10; 8]);
#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
const AVX2_VDESCALE_18_BIAS: i32x8 = i32x8::new([1 << 17; 8]);
#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
const AVX2_VRANGE_CENTER: i32x8 = i32x8::new([128; 8]);
#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
const AVX2_VRANGE_MAX: i32x8 = i32x8::new([255; 8]);

pub(crate) const DCTSIZE: usize = 8;
pub(crate) const DCTSIZE2: usize = 64;

/// Full-precision multiply matching IJG's MULTIPLY macro (no premature descale).
/// Returns v * c at CONST_BITS (2^13) scale.
#[inline(always)]
pub(crate) fn mpy(v: i32, c: i32) -> i32 {
    low_i32(i64::from(v).saturating_mul(i64::from(c)))
}

#[inline(always)]
pub(crate) fn descale(x: i32, shift: i32) -> i32 {
    add(
        x,
        1i32.wrapping_shl(shift.saturating_sub(1).cast_unsigned()),
    )
    .wrapping_shr(shift.cast_unsigned())
}

/// IJG-style range_limit: clamps (x + 128) to [0, 255].
#[inline(always)]
pub(crate) fn range_limit(x: i32) -> u8 {
    let x = x.saturating_add(128);
    if x < 0 {
        0
    } else if x > 255 {
        255
    } else {
        x.to_le_bytes()[0]
    }
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

fn low_i32(value: i64) -> i32 {
    let [a, b, c, d, ..] = value.to_le_bytes();
    i32::from_le_bytes([a, b, c, d])
}

// ── IJG jpeg_idct_islow — in-place on 8×8 block ─────────────────────────

pub(crate) fn jpeg_idct_islow(block: &mut [i32; DCTSIZE2], workspace: &mut [i32; DCTSIZE2]) {
    // Pass 1: columns
    for c in 0..DCTSIZE {
        if block[c.saturating_add(DCTSIZE)] == 0
            && block[c.saturating_add(DCTSIZE.saturating_mul(2))] == 0
            && block[c.saturating_add(DCTSIZE.saturating_mul(3))] == 0
            && block[c.saturating_add(DCTSIZE.saturating_mul(4))] == 0
            && block[c.saturating_add(DCTSIZE.saturating_mul(5))] == 0
            && block[c.saturating_add(DCTSIZE.saturating_mul(6))] == 0
            && block[c.saturating_add(DCTSIZE.saturating_mul(7))] == 0
        {
            let dcval = block[c].wrapping_shl(PASS1_BITS.cast_unsigned());
            for r in 0..DCTSIZE {
                workspace[r.saturating_mul(DCTSIZE).saturating_add(c)] = dcval;
            }
            continue;
        }

        let z2 = block[c.saturating_add(DCTSIZE.saturating_mul(2))];
        let z3 = block[c.saturating_add(DCTSIZE.saturating_mul(6))];
        let z1 = mpy(add(z2, z3), FIX_0_541196100);
        let tmp2 = add(z1, mpy(z3, -FIX_1_847759065));
        let tmp3 = add(z1, mpy(z2, FIX_0_765366865));

        let z2 = block[c];
        let z3 = block[c.saturating_add(DCTSIZE.saturating_mul(4))];
        let tmp0 = add(z2, z3).wrapping_shl(CONST_BITS.cast_unsigned());
        let tmp1 = sub(z2, z3).wrapping_shl(CONST_BITS.cast_unsigned());

        let tmp10 = add(tmp0, tmp3);
        let tmp13 = sub(tmp0, tmp3);
        let tmp11 = add(tmp1, tmp2);
        let tmp12 = sub(tmp1, tmp2);

        // Odd part — Figure 8
        let v0 = block[c.saturating_add(DCTSIZE.saturating_mul(7))];
        let v1 = block[c.saturating_add(DCTSIZE.saturating_mul(5))];
        let v2 = block[c.saturating_add(DCTSIZE.saturating_mul(3))];
        let v3 = block[c.saturating_add(DCTSIZE)];
        let z1 = add(v0, v3);
        let z2 = add(v1, v2);
        let z3 = add(v0, v2);
        let z4 = add(v1, v3);
        let z5 = mpy(add(z3, z4), FIX_1_175875602);

        let t0 = mpy(v0, FIX_0_298631336);
        let t1 = mpy(v1, FIX_2_053119869);
        let t2 = mpy(v2, FIX_3_072711026);
        let t3 = mpy(v3, FIX_1_501321110);
        let z1 = mpy(z1, -FIX_0_899976223);
        let z2 = mpy(z2, -FIX_2_562915447);
        let z3 = mpy(z3, -FIX_1_961570560);
        let z4 = mpy(z4, -FIX_0_390180644);
        let z3 = add(z3, z5);
        let z4 = add(z4, z5);

        let o0 = add3(t0, z1, z3);
        let o1 = add3(t1, z2, z4);
        let o2 = add3(t2, z2, z3);
        let o3 = add3(t3, z1, z4);

        let scale = CONST_BITS.saturating_sub(PASS1_BITS);
        workspace[c] = descale(add(tmp10, o3), scale);
        workspace[c.saturating_add(DCTSIZE.saturating_mul(7))] = descale(sub(tmp10, o3), scale);
        workspace[c.saturating_add(DCTSIZE)] = descale(add(tmp11, o2), scale);
        workspace[c.saturating_add(DCTSIZE.saturating_mul(6))] = descale(sub(tmp11, o2), scale);
        workspace[c.saturating_add(DCTSIZE.saturating_mul(2))] = descale(add(tmp12, o1), scale);
        workspace[c.saturating_add(DCTSIZE.saturating_mul(5))] = descale(sub(tmp12, o1), scale);
        workspace[c.saturating_add(DCTSIZE.saturating_mul(3))] = descale(add(tmp13, o0), scale);
        workspace[c.saturating_add(DCTSIZE.saturating_mul(4))] = descale(sub(tmp13, o0), scale);
    }

    // Pass 2: rows from workspace → block (in-place with range limiting)
    const FS: i32 = 18;

    for r in 0..DCTSIZE {
        let row = r.saturating_mul(DCTSIZE);
        if workspace[row.saturating_add(1)] == 0
            && workspace[row.saturating_add(2)] == 0
            && workspace[row.saturating_add(3)] == 0
            && workspace[row.saturating_add(4)] == 0
            && workspace[row.saturating_add(5)] == 0
            && workspace[row.saturating_add(6)] == 0
            && workspace[row.saturating_add(7)] == 0
        {
            let sample = i32::from(range_limit(descale(
                workspace[row],
                PASS1_BITS.saturating_add(3),
            )));
            block[row..row.saturating_add(DCTSIZE)].fill(sample);
            continue;
        }

        let z2 = workspace[row.saturating_add(2)];
        let z3 = workspace[row.saturating_add(6)];
        let z1 = mpy(add(z2, z3), FIX_0_541196100);
        let tmp2 = add(z1, mpy(z3, -FIX_1_847759065));
        let tmp3 = add(z1, mpy(z2, FIX_0_765366865));

        let z2 = workspace[row];
        let z3 = workspace[row.saturating_add(4)];
        let tmp0 = add(z2, z3).wrapping_shl(CONST_BITS.cast_unsigned());
        let tmp1 = sub(z2, z3).wrapping_shl(CONST_BITS.cast_unsigned());

        let tmp10 = add(tmp0, tmp3);
        let tmp13 = sub(tmp0, tmp3);
        let tmp11 = add(tmp1, tmp2);
        let tmp12 = sub(tmp1, tmp2);

        let v0 = workspace[row.saturating_add(7)];
        let v1 = workspace[row.saturating_add(5)];
        let v2 = workspace[row.saturating_add(3)];
        let v3 = workspace[row.saturating_add(1)];

        let z1 = add(v0, v3);
        let z2 = add(v1, v2);
        let z3 = add(v0, v2);
        let z4 = add(v1, v3);
        let z5 = mpy(add(z3, z4), FIX_1_175875602);

        let t0 = mpy(v0, FIX_0_298631336);
        let t1 = mpy(v1, FIX_2_053119869);
        let t2 = mpy(v2, FIX_3_072711026);
        let t3 = mpy(v3, FIX_1_501321110);
        let z1 = mpy(z1, -FIX_0_899976223);
        let z2 = mpy(z2, -FIX_2_562915447);
        let z3 = mpy(z3, -FIX_1_961570560);
        let z4 = mpy(z4, -FIX_0_390180644);
        let z3 = add(z3, z5);
        let z4 = add(z4, z5);

        let o0 = add3(t0, z1, z3);
        let o1 = add3(t1, z2, z4);
        let o2 = add3(t2, z2, z3);
        let o3 = add3(t3, z1, z4);

        block[row] = i32::from(range_limit(descale(add(tmp10, o3), FS)));
        block[row.saturating_add(7)] = i32::from(range_limit(descale(sub(tmp10, o3), FS)));
        block[row.saturating_add(1)] = i32::from(range_limit(descale(add(tmp11, o2), FS)));
        block[row.saturating_add(6)] = i32::from(range_limit(descale(sub(tmp11, o2), FS)));
        block[row.saturating_add(2)] = i32::from(range_limit(descale(add(tmp12, o1), FS)));
        block[row.saturating_add(5)] = i32::from(range_limit(descale(sub(tmp12, o1), FS)));
        block[row.saturating_add(3)] = i32::from(range_limit(descale(add(tmp13, o0), FS)));
        block[row.saturating_add(4)] = i32::from(range_limit(descale(sub(tmp13, o0), FS)));
    }
}

/// Safe vectorized baseline transform that writes range-limited bytes directly
/// into a component plane. Pass 1 keeps the scalar saturating contract while
/// laying out four adjacent rows for the safe `wide` pass-2 vectors.
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[allow(
    clippy::too_many_arguments,
    reason = "the direct transform receives explicit component-plane coordinates"
)]
#[inline(never)]
pub(crate) fn jpeg_idct_islow_to_u8_safe(
    block: &[i32; DCTSIZE2],
    workspace: &mut [i32; DCTSIZE2],
    output: &mut [u8],
    output_stride: usize,
    output_x: usize,
    output_y: usize,
) {
    debug_assert!(output_x.saturating_add(DCTSIZE) <= output_stride);
    debug_assert!(
        output_y
            .saturating_add(DCTSIZE.saturating_sub(1))
            .saturating_mul(output_stride)
            .saturating_add(output_x)
            .saturating_add(DCTSIZE)
            <= output.len()
    );

    for column in 0..DCTSIZE {
        let values = idct_pass1_column_values(block, column);
        let group_base = column.div_euclid(4).saturating_mul(16);
        let lane_base = column.rem_euclid(4).saturating_mul(4);
        let low = group_base.saturating_add(lane_base);
        let high = 32usize.saturating_add(low);
        workspace[low] = values[0];
        workspace[low.saturating_add(1)] = values[1];
        workspace[low.saturating_add(2)] = values[2];
        workspace[low.saturating_add(3)] = values[3];
        workspace[high] = values[4];
        workspace[high.saturating_add(1)] = values[5];
        workspace[high.saturating_add(2)] = values[6];
        workspace[high.saturating_add(3)] = values[7];
    }

    for group in 0..2usize {
        idct_pass2_four_rows_safe(
            workspace,
            output,
            output_stride,
            output_x,
            output_y.saturating_add(group.saturating_mul(4)),
            group.saturating_mul(32),
            false,
        );
    }
}

/// Safe native transform that retains signed-word dequantization before the
/// fixed-point butterfly and writes final bytes to the component plane.
/// The caller checks the cumulative DC product before selecting this path.
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[allow(
    clippy::too_many_arguments,
    reason = "the fused transform receives its quantizer and explicit component-plane coordinates"
)]
#[inline(never)]
pub(crate) fn jpeg_idct_islow_dequantized_to_u8_safe(
    block: &[i32; DCTSIZE2],
    quant_table: &[i32; DCTSIZE2],
    workspace: &mut [i32; DCTSIZE2],
    output: &mut [u8],
    output_stride: usize,
    output_x: usize,
    output_y: usize,
    high_horizontal_nonzero: bool,
) {
    debug_assert!(block[0].checked_mul(quant_table[0]).is_some());
    debug_assert!(output_x.saturating_add(DCTSIZE) <= output_stride);
    debug_assert!(
        output_y
            .saturating_add(DCTSIZE.saturating_sub(1))
            .saturating_mul(output_stride)
            .saturating_add(output_x)
            .saturating_add(DCTSIZE)
            <= output.len()
    );

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    if high_horizontal_nonzero {
        idct_pass1_eight_columns_avx2(block, quant_table, workspace);
        idct_pass2_eight_rows_avx2(workspace, output, output_stride, output_x, output_y);
        return;
    }

    idct_pass1_four_columns_safe(block, quant_table, workspace, 0);
    if high_horizontal_nonzero {
        idct_pass1_four_columns_safe(block, quant_table, workspace, 4);
    }
    idct_pass2_four_rows_safe(
        workspace,
        output,
        output_stride,
        output_x,
        output_y,
        0,
        !high_horizontal_nonzero,
    );
    idct_pass2_four_rows_safe(
        workspace,
        output,
        output_stride,
        output_x,
        output_y.saturating_add(4),
        32,
        !high_horizontal_nonzero,
    );
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[inline(always)]
fn idct_pass1_column_values(block: &[i32; DCTSIZE2], column: usize) -> [i32; DCTSIZE] {
    let z2 = block[column.saturating_add(DCTSIZE.saturating_mul(2))];
    let z3 = block[column.saturating_add(DCTSIZE.saturating_mul(6))];
    let z1 = mpy(add(z2, z3), FIX_0_541196100);
    let tmp2 = add(z1, mpy(z3, -FIX_1_847759065));
    let tmp3 = add(z1, mpy(z2, FIX_0_765366865));

    let z2 = block[column];
    let z3 = block[column.saturating_add(DCTSIZE.saturating_mul(4))];
    let tmp0 = add(z2, z3).wrapping_shl(CONST_BITS.cast_unsigned());
    let tmp1 = sub(z2, z3).wrapping_shl(CONST_BITS.cast_unsigned());

    let tmp10 = add(tmp0, tmp3);
    let tmp13 = sub(tmp0, tmp3);
    let tmp11 = add(tmp1, tmp2);
    let tmp12 = sub(tmp1, tmp2);

    let v0 = block[column.saturating_add(DCTSIZE.saturating_mul(7))];
    let v1 = block[column.saturating_add(DCTSIZE.saturating_mul(5))];
    let v2 = block[column.saturating_add(DCTSIZE.saturating_mul(3))];
    let v3 = block[column.saturating_add(DCTSIZE)];
    let z1 = add(v0, v3);
    let z2 = add(v1, v2);
    let z3 = add(v0, v2);
    let z4 = add(v1, v3);
    let z5 = mpy(add(z3, z4), FIX_1_175875602);

    let t0 = mpy(v0, FIX_0_298631336);
    let t1 = mpy(v1, FIX_2_053119869);
    let t2 = mpy(v2, FIX_3_072711026);
    let t3 = mpy(v3, FIX_1_501321110);
    let z1 = mpy(z1, -FIX_0_899976223);
    let z2 = mpy(z2, -FIX_2_562915447);
    let z3 = add(mpy(z3, -FIX_1_961570560), z5);
    let z4 = add(mpy(z4, -FIX_0_390180644), z5);

    let o0 = add3(t0, z1, z3);
    let o1 = add3(t1, z2, z4);
    let o2 = add3(t2, z2, z3);
    let o3 = add3(t3, z1, z4);
    let scale = CONST_BITS.saturating_sub(PASS1_BITS);
    [
        descale(add(tmp10, o3), scale),
        descale(add(tmp11, o2), scale),
        descale(add(tmp12, o1), scale),
        descale(add(tmp13, o0), scale),
        descale(sub(tmp13, o0), scale),
        descale(sub(tmp12, o1), scale),
        descale(sub(tmp11, o2), scale),
        descale(sub(tmp10, o3), scale),
    ]
}

// The upstream SIMD butterflies add the even pair and two odd pairs in
// signed 16-bit lanes, then widen for the factored fixed-point products.
// With signed-word inputs, each final sum is bounded by 64_570 * 32_768,
// which fits i32, including either descale bias. The safe vector operators
// also retain the upstream wrapping lane semantics for defensive states.
// Keep broadcasts in a const context: constructing them from a closure
// parameter otherwise materializes temporary arrays in the release kernel.
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
macro_rules! idct_fixed_word {
    ($value:expr) => {
        const {
            let [low, high, ..] = ($value).to_le_bytes();
            i16x8::new([i16::from_le_bytes([low, high]); DCTSIZE])
        }
    };
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
macro_rules! native_idct_butterfly {
    ($name:ident, $vector:ty, $word:path, $widen:path, $multiply:path) => {
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the native signed-word IDCT factorization bounds its fixed-point vector arithmetic within i32"
        )]
        #[inline(always)]
        fn $name(r: [$vector; DCTSIZE]) -> [$vector; DCTSIZE] {
            let r = [
                $word(r[0]), $word(r[1]), $word(r[2]), $word(r[3]),
                $word(r[4]), $word(r[5]), $word(r[6]), $word(r[7]),
            ];
            let tmp2 = $multiply(r[2], idct_fixed_word!(FIX_0_541196100))
                + $multiply(r[6], idct_fixed_word!(FIX_0_541196100 - FIX_1_847759065));
            let tmp3 = $multiply(r[2], idct_fixed_word!(FIX_0_541196100 + FIX_0_765366865))
                + $multiply(r[6], idct_fixed_word!(FIX_0_541196100));
            let tmp0 = $widen(r[0] + r[4]).unbounded_shl_scalar(CONST_BITS.cast_unsigned());
            let tmp1 = $widen(r[0] - r[4]).unbounded_shl_scalar(CONST_BITS.cast_unsigned());
            let tmp10 = tmp0 + tmp3;
            let tmp13 = tmp0 - tmp3;
            let tmp11 = tmp1 + tmp2;
            let tmp12 = tmp1 - tmp2;

            let pair_73 = r[7] + r[3];
            let pair_51 = r[5] + r[1];
            let z3 = $multiply(pair_73, idct_fixed_word!(FIX_1_175875602 - FIX_1_961570560))
                + $multiply(pair_51, idct_fixed_word!(FIX_1_175875602));
            let z4 = $multiply(pair_73, idct_fixed_word!(FIX_1_175875602))
                + $multiply(pair_51, idct_fixed_word!(FIX_1_175875602 - FIX_0_390180644));
            let odd0 = $multiply(r[7], idct_fixed_word!(FIX_0_298631336 - FIX_0_899976223))
                - $multiply(r[1], idct_fixed_word!(FIX_0_899976223)) + z3;
            let odd1 = $multiply(r[5], idct_fixed_word!(FIX_2_053119869 - FIX_2_562915447))
                - $multiply(r[3], idct_fixed_word!(FIX_2_562915447)) + z4;
            let odd2 = $multiply(r[3], idct_fixed_word!(FIX_3_072711026 - FIX_2_562915447))
                - $multiply(r[5], idct_fixed_word!(FIX_2_562915447)) + z3;
            let odd3 = $multiply(r[1], idct_fixed_word!(FIX_1_501321110 - FIX_0_899976223))
                - $multiply(r[7], idct_fixed_word!(FIX_0_899976223)) + z4;
            [
                tmp10 + odd3,
                tmp11 + odd2,
                tmp12 + odd1,
                tmp13 + odd0,
                tmp13 - odd0,
                tmp12 - odd1,
                tmp11 - odd2,
                tmp10 - odd3,
            ]
        }
    };
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[inline(always)]
fn idct_word_four(value: i32x4) -> i16x8 {
    i16x8::from_i32x8_truncate(cast::<[i32x4; 2], i32x8>([value, i32x4::ZERO]))
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[inline(always)]
fn idct_widen_four(value: i16x8) -> i32x4 {
    let [low, _] = cast::<i32x8, [i32x4; 2]>(i32x8::from_i16x8(value));
    low
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[inline(always)]
fn idct_multiply_four(left: i16x8, right: i16x8) -> i32x4 {
    let [low, _] = cast::<i32x8, [i32x4; 2]>(left.widening_mul(right));
    low
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
native_idct_butterfly!(
    idct_butterfly_four,
    i32x4,
    idct_word_four,
    idct_widen_four,
    idct_multiply_four
);
#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
native_idct_butterfly!(
    idct_butterfly_eight,
    i32x8,
    i16x8::from_i32x8_truncate,
    i32x8::from_i16x8,
    i16x8::widening_mul
);

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[inline(always)]
fn load_idct_four(workspace: &[i32; DCTSIZE2], offset: usize) -> i32x4 {
    pod_read_unaligned(cast_slice(&workspace[offset..offset.saturating_add(4)]))
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[inline(always)]
fn idct_vector_descale_11(value: i32x4) -> i32x4 {
    let scaled = value
        .saturating_add(VDESCALE_11_BIAS)
        .unbounded_shr_scalar(11);
    // The pinned Pillow oracle truncates its NEON workspace to signed words.
    // Preserve that boundary on every SIMD target, including overflowing input.
    scaled.unbounded_shl_scalar(16).unbounded_shr_scalar(16)
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[inline(always)]
fn store_idct_four(workspace: &mut [i32; DCTSIZE2], offset: usize, value: i32x4) {
    workspace[offset..offset.saturating_add(4)].copy_from_slice(value.as_array());
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[inline(never)]
fn store_zero_vertical_columns(
    block: &[i32; DCTSIZE2],
    workspace: &mut [i32; DCTSIZE2],
    column: usize,
    dc: i32x4,
) -> bool {
    // Preserve the pinned oracle's four-column shortcut on every target.
    // Test raw coefficients: zero quantizers must not alter path selection.
    let vertical_ac_zero = (1usize..8).all(|row| {
        load_idct_four(block, row.saturating_mul(DCTSIZE).saturating_add(column)) == i32x4::ZERO
    });
    if !vertical_ac_zero {
        return false;
    }
    let group_base = column.div_euclid(4).saturating_mul(16);
    for (lane, value) in dc.to_array().into_iter().enumerate() {
        let offset = group_base.saturating_add(lane.saturating_mul(4));
        let repeated = i32x4::new([value; 4]);
        store_idct_four(workspace, offset, repeated);
        store_idct_four(workspace, offset.saturating_add(32), repeated);
    }
    true
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "JPEG ISLOW fixed-point lanes are bounded before the safe vector multiplies"
)]
#[inline(always)]
fn idct_pass1_four_columns_safe(
    block: &[i32; DCTSIZE2],
    quant_table: &[i32; DCTSIZE2],
    workspace: &mut [i32; DCTSIZE2],
    column: usize,
) {
    let load = |row: usize| {
        let offset = row.saturating_mul(DCTSIZE).saturating_add(column);
        // Upstream NEON and SSE2 multiply into signed 16-bit lanes before
        // widening for the transform. Large quantizers retain the low word.
        let product = load_idct_four(block, offset) * load_idct_four(quant_table, offset);
        product.unbounded_shl_scalar(16).unbounded_shr_scalar(16)
    };
    let r0 = load(0);
    let scaled_dc = r0.unbounded_shl_scalar(PASS1_BITS.cast_unsigned());
    let narrowed_dc = scaled_dc.unbounded_shl_scalar(16).unbounded_shr_scalar(16);
    // The ordinary transform already agrees when the shortcut's workspace
    // fits i16. Outline the rare bitmap check and word-sized wrap.
    if scaled_dc != narrowed_dc
        && store_zero_vertical_columns(block, workspace, column, narrowed_dc)
    {
        return;
    }
    let r1 = load(1);
    let r2 = load(2);
    let r3 = load(3);
    let r4 = load(4);
    let r5 = load(5);
    let r6 = load(6);
    let r7 = load(7);

    let rows = idct_butterfly_four([r0, r1, r2, r3, r4, r5, r6, r7]);
    let rows = [
        idct_vector_descale_11(rows[0]),
        idct_vector_descale_11(rows[1]),
        idct_vector_descale_11(rows[2]),
        idct_vector_descale_11(rows[3]),
        idct_vector_descale_11(rows[4]),
        idct_vector_descale_11(rows[5]),
        idct_vector_descale_11(rows[6]),
        idct_vector_descale_11(rows[7]),
    ];
    let low = transpose_idct_four([rows[0], rows[1], rows[2], rows[3]]);
    let high = transpose_idct_four([rows[4], rows[5], rows[6], rows[7]]);
    let group_base = column.div_euclid(4).saturating_mul(16);
    for lane in 0..4usize {
        store_idct_four(
            workspace,
            group_base.saturating_add(lane.saturating_mul(4)),
            low[lane],
        );
        store_idct_four(
            workspace,
            32usize
                .saturating_add(group_base)
                .saturating_add(lane.saturating_mul(4)),
            high[lane],
        );
    }
}

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "bounded JPEG ISLOW coefficients and quantizers prove the AVX2 lane multiplies fit"
)]
#[inline(always)]
fn idct_pass1_eight_columns_avx2(
    block: &[i32; DCTSIZE2],
    quant_table: &[i32; DCTSIZE2],
    workspace: &mut [i32; DCTSIZE2],
) {
    let load = |frequency_row: usize| {
        let offset = frequency_row.saturating_mul(DCTSIZE);
        let product = load_idct_eight(block, offset) * load_idct_eight(quant_table, offset);
        product.unbounded_shl_scalar(16).unbounded_shr_scalar(16)
    };
    let r0 = load(0);
    let scaled_dc = r0.unbounded_shl_scalar(PASS1_BITS.cast_unsigned());
    let narrowed_dc = scaled_dc.unbounded_shl_scalar(16).unbounded_shr_scalar(16);
    if scaled_dc != narrowed_dc {
        // A wrapped DC workspace must retain independent four-column shortcut
        // decisions, even when the other half has nonzero vertical AC.
        idct_pass1_wrapped_dc_avx2(block, quant_table, workspace);
        return;
    }
    let r1 = load(1);
    let r2 = load(2);
    let r3 = load(3);
    let r4 = load(4);
    let r5 = load(5);
    let r6 = load(6);
    let r7 = load(7);

    let rows = idct_butterfly_eight([r0, r1, r2, r3, r4, r5, r6, r7]);
    let rows = [
        avx2_idct_descale_11(rows[0]),
        avx2_idct_descale_11(rows[1]),
        avx2_idct_descale_11(rows[2]),
        avx2_idct_descale_11(rows[3]),
        avx2_idct_descale_11(rows[4]),
        avx2_idct_descale_11(rows[5]),
        avx2_idct_descale_11(rows[6]),
        avx2_idct_descale_11(rows[7]),
    ];
    store_idct_pass1_rows_avx2(workspace, rows);
}

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
#[inline(always)]
fn load_idct_eight(values: &[i32; DCTSIZE2], offset: usize) -> i32x8 {
    pod_read_unaligned(cast_slice(&values[offset..offset.saturating_add(8)]))
}

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
#[inline(never)]
fn idct_pass1_wrapped_dc_avx2(
    block: &[i32; DCTSIZE2],
    quant_table: &[i32; DCTSIZE2],
    workspace: &mut [i32; DCTSIZE2],
) {
    // Keep the rare complete four-column transforms outside the eight-lane
    // kernel so ordinary blocks do not carry their additional code size.
    idct_pass1_four_columns_safe(block, quant_table, workspace, 0);
    idct_pass1_four_columns_safe(block, quant_table, workspace, 4);
}

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
#[inline(always)]
fn store_idct_pass1_rows_avx2(workspace: &mut [i32; DCTSIZE2], rows: [i32x8; DCTSIZE]) {
    let rows = rows.map(i32x8::to_array);
    let columns: [[i32; DCTSIZE]; DCTSIZE] =
        std::array::from_fn(|column| std::array::from_fn(|row| rows[row][column]));
    for (column, values) in columns.into_iter().enumerate() {
        let column_values = i32x8::new(values);
        let [top, bottom] = cast::<i32x8, [i32x4; 2]>(column_values);
        let column_group = column.div_euclid(4).saturating_mul(16);
        let column_offset = column_group.saturating_add(column.rem_euclid(4).saturating_mul(4));
        store_idct_four(workspace, column_offset, top);
        store_idct_four(workspace, column_offset.saturating_add(32), bottom);
    }
}

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
#[allow(
    clippy::too_many_arguments,
    reason = "the AVX2 row transform writes one complete fixed-size component block"
)]
#[inline(always)]
fn idct_pass2_eight_rows_avx2(
    workspace: &[i32; DCTSIZE2],
    output: &mut [u8],
    output_stride: usize,
    output_x: usize,
    output_y: usize,
) {
    let left = [
        load_idct_eight_rows(workspace, 0, 0),
        load_idct_eight_rows(workspace, 0, 1),
        load_idct_eight_rows(workspace, 0, 2),
        load_idct_eight_rows(workspace, 0, 3),
    ];
    let right = [
        load_idct_eight_rows(workspace, 16, 0),
        load_idct_eight_rows(workspace, 16, 1),
        load_idct_eight_rows(workspace, 16, 2),
        load_idct_eight_rows(workspace, 16, 3),
    ];

    let rows = idct_butterfly_eight([
        left[0], left[1], left[2], left[3], right[0], right[1], right[2], right[3],
    ]);
    store_idct_eight_rows(
        output,
        output_stride,
        output_x,
        output_y,
        [
            avx2_idct_range_limit(avx2_idct_descale_18(rows[0])),
            avx2_idct_range_limit(avx2_idct_descale_18(rows[1])),
            avx2_idct_range_limit(avx2_idct_descale_18(rows[2])),
            avx2_idct_range_limit(avx2_idct_descale_18(rows[3])),
            avx2_idct_range_limit(avx2_idct_descale_18(rows[4])),
            avx2_idct_range_limit(avx2_idct_descale_18(rows[5])),
            avx2_idct_range_limit(avx2_idct_descale_18(rows[6])),
            avx2_idct_range_limit(avx2_idct_descale_18(rows[7])),
        ],
    );
}

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
#[inline(always)]
fn load_idct_eight_rows(workspace: &[i32; DCTSIZE2], column_group: usize, column: usize) -> i32x8 {
    let offset = column_group.saturating_add(column.saturating_mul(4));
    cast::<[i32x4; 2], i32x8>([
        load_idct_four(workspace, offset),
        load_idct_four(workspace, offset.saturating_add(32)),
    ])
}

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
#[inline(always)]
fn avx2_idct_descale_11(value: i32x8) -> i32x8 {
    value
        .saturating_add(AVX2_VDESCALE_11_BIAS)
        .unbounded_shr_scalar(11)
        .unbounded_shl_scalar(16)
        .unbounded_shr_scalar(16)
}

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
#[inline(always)]
fn avx2_idct_descale_18(value: i32x8) -> i32x8 {
    value
        .saturating_add(AVX2_VDESCALE_18_BIAS)
        .unbounded_shr_scalar(18)
}

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
#[inline(always)]
fn avx2_idct_range_limit(value: i32x8) -> i32x8 {
    value
        .saturating_add(AVX2_VRANGE_CENTER)
        .max(i32x8::ZERO)
        .min(AVX2_VRANGE_MAX)
}

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
#[allow(
    clippy::too_many_arguments,
    reason = "the AVX2 store receives the full transformed block and component-plane coordinates"
)]
#[inline(always)]
fn store_idct_eight_rows(
    output: &mut [u8],
    output_stride: usize,
    output_x: usize,
    output_y: usize,
    columns: [i32x8; 8],
) {
    let columns = columns.map(i32x8::to_array);
    let rows: [[i32; DCTSIZE]; DCTSIZE] =
        std::array::from_fn(|row| std::array::from_fn(|column| columns[column][row]));
    for (row, values) in rows.iter().enumerate() {
        let values = i32x8::new(*values);
        let packed =
            u8x16::narrow_i16x8(i16x8::from_i32x8_saturate(values), i16x8::ZERO).to_array();
        let start = output_y
            .saturating_add(row)
            .saturating_mul(output_stride)
            .saturating_add(output_x);
        output[start..start.saturating_add(DCTSIZE)].copy_from_slice(&packed[..DCTSIZE]);
    }
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[inline(always)]
fn range_limit_four(value: i32x4) -> i32x4 {
    value
        .saturating_add(VDESCALE_18_BIAS)
        .unbounded_shr_scalar(18)
        .saturating_add(VRANGE_CENTER)
        .max(i32x4::ZERO)
        .min(VRANGE_MAX)
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[inline(always)]
fn transpose_idct_four(columns: [i32x4; 4]) -> [i32x4; 4] {
    let first = columns[0].to_array();
    let second = columns[1].to_array();
    let third = columns[2].to_array();
    let fourth = columns[3].to_array();
    [
        i32x4::new([first[0], second[0], third[0], fourth[0]]),
        i32x4::new([first[1], second[1], third[1], fourth[1]]),
        i32x4::new([first[2], second[2], third[2], fourth[2]]),
        i32x4::new([first[3], second[3], third[3], fourth[3]]),
    ]
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[allow(
    clippy::too_many_arguments,
    reason = "the four-row store receives both transformed column halves and plane coordinates"
)]
#[inline(always)]
fn store_idct_four_rows(
    output: &mut [u8],
    output_stride: usize,
    output_x: usize,
    output_y: usize,
    left: [i32x4; 4],
    right: [i32x4; 4],
) {
    let left = transpose_idct_four(left);
    let right = transpose_idct_four(right);
    for row in 0..4usize {
        let values = cast::<[i32x4; 2], i32x8>([left[row], right[row]]);
        let packed =
            u8x16::narrow_i16x8(i16x8::from_i32x8_saturate(values), i16x8::ZERO).to_array();
        let start = output_y
            .saturating_add(row)
            .saturating_mul(output_stride)
            .saturating_add(output_x);
        output[start..start.saturating_add(8)].copy_from_slice(&packed[..8]);
    }
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[allow(
    clippy::too_many_arguments,
    reason = "the vector pass receives its transposed workspace and plane coordinates"
)]
#[inline(always)]
fn idct_pass2_four_rows_safe(
    workspace: &[i32; DCTSIZE2],
    output: &mut [u8],
    output_stride: usize,
    output_x: usize,
    output_y: usize,
    base: usize,
    skip_right: bool,
) {
    let left = [
        load_idct_four(workspace, base),
        load_idct_four(workspace, base.saturating_add(4)),
        load_idct_four(workspace, base.saturating_add(8)),
        load_idct_four(workspace, base.saturating_add(12)),
    ];
    let right = if skip_right {
        [i32x4::ZERO; 4]
    } else {
        [
            load_idct_four(workspace, base.saturating_add(16)),
            load_idct_four(workspace, base.saturating_add(20)),
            load_idct_four(workspace, base.saturating_add(24)),
            load_idct_four(workspace, base.saturating_add(28)),
        ]
    };

    let rows = idct_butterfly_four([
        left[0], left[1], left[2], left[3], right[0], right[1], right[2], right[3],
    ]);
    let rows = [
        range_limit_four(rows[0]),
        range_limit_four(rows[1]),
        range_limit_four(rows[2]),
        range_limit_four(rows[3]),
        range_limit_four(rows[4]),
        range_limit_four(rows[5]),
        range_limit_four(rows[6]),
        range_limit_four(rows[7]),
    ];
    store_idct_four_rows(
        output,
        output_stride,
        output_x,
        output_y,
        [rows[0], rows[1], rows[2], rows[3]],
        [rows[4], rows[5], rows[6], rows[7]],
    );
}

// ── JPEG Utilities ────────────────────────────────────────────────────────

/// Exact output of [`jpeg_idct_islow`] when only the natural-order DC
/// coefficient is nonzero.
#[inline(always)]
pub(crate) fn dc_only_output(dc: i32) -> u8 {
    range_limit(descale(dc, 3))
}

/// `jpeg_natural_order` maps zigzag index to natural (row-major) position.
pub(crate) const JPEG_NATURAL_ORDER: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// Sign extension for DC/AC coefficient additional bits (Figure F.12).
#[inline(always)]
pub(crate) fn extend(value: u32, size: u8) -> i32 {
    debug_assert!(size > 0);
    let threshold = 1u32.wrapping_shl(u32::from(size.saturating_sub(1)));
    if value < threshold {
        value
            .cast_signed()
            .saturating_sub(1i32.wrapping_shl(u32::from(size)).saturating_sub(1))
    } else {
        value.cast_signed()
    }
}

/// YCbCr -> RGB conversion matching libjpeg's jdcolor.c.
pub(super) struct YccColorConverter {
    cr_r_tab: [i32; 256],
    cb_b_tab: [i32; 256],
    cr_g_tab: [i32; 256],
    cb_g_tab: [i32; 256],
}

impl YccColorConverter {
    pub(crate) fn shared() -> &'static Self {
        static CONVERTER: std::sync::OnceLock<YccColorConverter> = std::sync::OnceLock::new();
        CONVERTER.get_or_init(Self::new)
    }

    pub(crate) fn new() -> Self {
        let mut cr_r_tab = [0i32; 256];
        let mut cb_b_tab = [0i32; 256];
        let mut cr_g_tab = [0i32; 256];
        let mut cb_g_tab = [0i32; 256];

        for i in 0usize..256 {
            let x = i32::from(i.to_le_bytes()[0]).saturating_sub(128);
            cr_r_tab[i] = low_i32(
                91_881i64
                    .saturating_mul(i64::from(x))
                    .saturating_add(32_768)
                    .wrapping_shr(16),
            );
            cb_b_tab[i] = low_i32(
                116_130i64
                    .saturating_mul(i64::from(x))
                    .saturating_add(32_768)
                    .wrapping_shr(16),
            );
            cr_g_tab[i] = low_i32((-46_802i64).saturating_mul(i64::from(x)));
            cb_g_tab[i] = low_i32(
                (-22_554i64)
                    .saturating_mul(i64::from(x))
                    .saturating_add(32_768),
            );
        }

        YccColorConverter {
            cr_r_tab,
            cb_b_tab,
            cr_g_tab,
            cb_g_tab,
        }
    }

    #[allow(
        dead_code,
        reason = "the scalar pixel reference remains useful for focused kernel tests"
    )]
    #[inline(always)]
    pub(crate) fn ycc_to_rgb(&self, y: u8, cb: u8, cr: u8) -> (u8, u8, u8) {
        crate::codecs::jpeg::kernels::ycc_to_rgb_pixel(
            y,
            cb,
            cr,
            &self.cr_r_tab,
            &self.cb_b_tab,
            &self.cr_g_tab,
            &self.cb_g_tab,
        )
    }

    /// Convert one row of YCbCr samples into interleaved RGB pixels in safe
    /// fixed-size batches.
    pub(crate) fn ycc_to_rgb_batch(&self, y: &[u8], cb: &[u8], cr: &[u8], output: &mut [u8]) {
        crate::codecs::jpeg::kernels::ycc_to_rgb_batch(y, cb, cr, output);
    }
}
