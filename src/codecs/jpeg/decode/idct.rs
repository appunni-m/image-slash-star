// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

#[cfg(target_arch = "aarch64")]
use wide::bytemuck::{cast, cast_slice, pod_read_unaligned};
#[cfg(target_arch = "aarch64")]
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

#[cfg(target_arch = "aarch64")]
const VFIX_0_298631336: i32x4 = i32x4::new([FIX_0_298631336; 4]);
#[cfg(target_arch = "aarch64")]
const VFIX_0_390180644_NEG: i32x4 = i32x4::new([-FIX_0_390180644; 4]);
#[cfg(target_arch = "aarch64")]
const VFIX_0_541196100: i32x4 = i32x4::new([FIX_0_541196100; 4]);
#[cfg(target_arch = "aarch64")]
const VFIX_0_765366865: i32x4 = i32x4::new([FIX_0_765366865; 4]);
#[cfg(target_arch = "aarch64")]
const VFIX_0_899976223_NEG: i32x4 = i32x4::new([-FIX_0_899976223; 4]);
#[cfg(target_arch = "aarch64")]
const VFIX_1_175875602: i32x4 = i32x4::new([FIX_1_175875602; 4]);
#[cfg(target_arch = "aarch64")]
const VFIX_1_501321110: i32x4 = i32x4::new([FIX_1_501321110; 4]);
#[cfg(target_arch = "aarch64")]
const VFIX_1_847759065_NEG: i32x4 = i32x4::new([-FIX_1_847759065; 4]);
#[cfg(target_arch = "aarch64")]
const VFIX_1_961570560_NEG: i32x4 = i32x4::new([-FIX_1_961570560; 4]);
#[cfg(target_arch = "aarch64")]
const VFIX_2_053119869: i32x4 = i32x4::new([FIX_2_053119869; 4]);
#[cfg(target_arch = "aarch64")]
const VFIX_2_562915447_NEG: i32x4 = i32x4::new([-FIX_2_562915447; 4]);
#[cfg(target_arch = "aarch64")]
const VFIX_3_072711026: i32x4 = i32x4::new([FIX_3_072711026; 4]);
#[cfg(target_arch = "aarch64")]
const VDESCALE_11_BIAS: i32x4 = i32x4::new([1 << 10; 4]);
#[cfg(target_arch = "aarch64")]
const VDESCALE_18_BIAS: i32x4 = i32x4::new([1 << 17; 4]);
#[cfg(target_arch = "aarch64")]
const VRANGE_CENTER: i32x4 = i32x4::new([128; 4]);
#[cfg(target_arch = "aarch64")]
const VRANGE_MAX: i32x4 = i32x4::new([255; 4]);

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

/// Safe AArch64 baseline transform that writes range-limited bytes directly
/// into a component plane. Pass 1 keeps the scalar saturating contract while
/// laying out four adjacent rows for the safe `wide` pass-2 vectors.
#[cfg(target_arch = "aarch64")]
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

/// Safe AArch64 baseline transform that folds exact dequantization into the
/// first fixed-width pass and writes final bytes directly to the component
/// plane. Baseline AC magnitudes always fit after dequantization; the caller
/// checks the only cumulative lane, the DC predictor, before selecting it.
#[cfg(target_arch = "aarch64")]
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

#[cfg(target_arch = "aarch64")]
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

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn idct_vector_add(left: i32x4, right: i32x4) -> i32x4 {
    left.saturating_add(right)
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn idct_vector_sub(left: i32x4, right: i32x4) -> i32x4 {
    left.saturating_sub(right)
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn load_idct_four(workspace: &[i32; DCTSIZE2], offset: usize) -> i32x4 {
    pod_read_unaligned(cast_slice(&workspace[offset..offset.saturating_add(4)]))
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn idct_vector_descale_11(value: i32x4) -> i32x4 {
    value
        .saturating_add(VDESCALE_11_BIAS)
        .unbounded_shr_scalar(11)
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn store_idct_four(workspace: &mut [i32; DCTSIZE2], offset: usize, value: i32x4) {
    workspace[offset..offset.saturating_add(4)].copy_from_slice(value.as_array());
}

#[cfg(target_arch = "aarch64")]
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
        load_idct_four(block, offset) * load_idct_four(quant_table, offset)
    };
    let r0 = load(0);
    let r1 = load(1);
    let r2 = load(2);
    let r3 = load(3);
    let r4 = load(4);
    let r5 = load(5);
    let r6 = load(6);
    let r7 = load(7);

    let z1 = idct_vector_add(r2, r6) * VFIX_0_541196100;
    let tmp2 = idct_vector_add(z1, r6 * VFIX_1_847759065_NEG);
    let tmp3 = idct_vector_add(z1, r2 * VFIX_0_765366865);
    let tmp0 = idct_vector_add(r0, r4).unbounded_shl_scalar(CONST_BITS.cast_unsigned());
    let tmp1 = idct_vector_sub(r0, r4).unbounded_shl_scalar(CONST_BITS.cast_unsigned());

    let tmp10 = idct_vector_add(tmp0, tmp3);
    let tmp13 = idct_vector_sub(tmp0, tmp3);
    let tmp11 = idct_vector_add(tmp1, tmp2);
    let tmp12 = idct_vector_sub(tmp1, tmp2);

    let z1 = idct_vector_add(r7, r1);
    let z2 = idct_vector_add(r5, r3);
    let z3 = idct_vector_add(r7, r3);
    let z4 = idct_vector_add(r5, r1);
    let z5 = idct_vector_add(z3, z4) * VFIX_1_175875602;

    let t0 = r7 * VFIX_0_298631336;
    let t1 = r5 * VFIX_2_053119869;
    let t2 = r3 * VFIX_3_072711026;
    let t3 = r1 * VFIX_1_501321110;
    let z1 = z1 * VFIX_0_899976223_NEG;
    let z2 = z2 * VFIX_2_562915447_NEG;
    let z3 = idct_vector_add(z3 * VFIX_1_961570560_NEG, z5);
    let z4 = idct_vector_add(z4 * VFIX_0_390180644_NEG, z5);

    let o0 = idct_vector_add(idct_vector_add(t0, z1), z3);
    let o1 = idct_vector_add(idct_vector_add(t1, z2), z4);
    let o2 = idct_vector_add(idct_vector_add(t2, z2), z3);
    let o3 = idct_vector_add(idct_vector_add(t3, z1), z4);
    let low = transpose_idct_four([
        idct_vector_descale_11(idct_vector_add(tmp10, o3)),
        idct_vector_descale_11(idct_vector_add(tmp11, o2)),
        idct_vector_descale_11(idct_vector_add(tmp12, o1)),
        idct_vector_descale_11(idct_vector_add(tmp13, o0)),
    ]);
    let high = transpose_idct_four([
        idct_vector_descale_11(idct_vector_sub(tmp13, o0)),
        idct_vector_descale_11(idct_vector_sub(tmp12, o1)),
        idct_vector_descale_11(idct_vector_sub(tmp11, o2)),
        idct_vector_descale_11(idct_vector_sub(tmp10, o3)),
    ]);
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

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn range_limit_four(value: i32x4) -> i32x4 {
    value
        .saturating_add(VDESCALE_18_BIAS)
        .unbounded_shr_scalar(18)
        .saturating_add(VRANGE_CENTER)
        .max(i32x4::ZERO)
        .min(VRANGE_MAX)
}

#[cfg(target_arch = "aarch64")]
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

#[cfg(target_arch = "aarch64")]
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

#[cfg(target_arch = "aarch64")]
#[allow(
    clippy::too_many_arguments,
    reason = "the vector pass receives its transposed workspace and plane coordinates"
)]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "JPEG ISLOW fixed-point lanes are bounded before the safe vector multiplies"
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

    let z2 = left[2];
    let z3 = right[2];
    let z1 = idct_vector_add(z2, z3) * VFIX_0_541196100;
    let tmp2 = idct_vector_add(z1, z3 * VFIX_1_847759065_NEG);
    let tmp3 = idct_vector_add(z1, z2 * VFIX_0_765366865);

    let z2 = left[0];
    let z3 = right[0];
    let tmp0 = idct_vector_add(z2, z3).unbounded_shl_scalar(CONST_BITS.cast_unsigned());
    let tmp1 = idct_vector_sub(z2, z3).unbounded_shl_scalar(CONST_BITS.cast_unsigned());

    let tmp10 = idct_vector_add(tmp0, tmp3);
    let tmp13 = idct_vector_sub(tmp0, tmp3);
    let tmp11 = idct_vector_add(tmp1, tmp2);
    let tmp12 = idct_vector_sub(tmp1, tmp2);

    let v0 = right[3];
    let v1 = right[1];
    let v2 = left[3];
    let v3 = left[1];
    let z1 = idct_vector_add(v0, v3);
    let z2 = idct_vector_add(v1, v2);
    let z3 = idct_vector_add(v0, v2);
    let z4 = idct_vector_add(v1, v3);
    let z5 = idct_vector_add(z3, z4) * VFIX_1_175875602;

    let t0 = v0 * VFIX_0_298631336;
    let t1 = v1 * VFIX_2_053119869;
    let t2 = v2 * VFIX_3_072711026;
    let t3 = v3 * VFIX_1_501321110;
    let z1 = z1 * VFIX_0_899976223_NEG;
    let z2 = z2 * VFIX_2_562915447_NEG;
    let z3 = idct_vector_add(z3 * VFIX_1_961570560_NEG, z5);
    let z4 = idct_vector_add(z4 * VFIX_0_390180644_NEG, z5);

    let o0 = idct_vector_add(idct_vector_add(t0, z1), z3);
    let o1 = idct_vector_add(idct_vector_add(t1, z2), z4);
    let o2 = idct_vector_add(idct_vector_add(t2, z2), z3);
    let o3 = idct_vector_add(idct_vector_add(t3, z1), z4);

    store_idct_four_rows(
        output,
        output_stride,
        output_x,
        output_y,
        [
            range_limit_four(idct_vector_add(tmp10, o3)),
            range_limit_four(idct_vector_add(tmp11, o2)),
            range_limit_four(idct_vector_add(tmp12, o1)),
            range_limit_four(idct_vector_add(tmp13, o0)),
        ],
        [
            range_limit_four(idct_vector_sub(tmp13, o0)),
            range_limit_four(idct_vector_sub(tmp12, o1)),
            range_limit_four(idct_vector_sub(tmp11, o2)),
            range_limit_four(idct_vector_sub(tmp10, o3)),
        ],
    );
}

/// Exercise the safe vector transform's fixed-shape contract in the managed
/// coverage build. These inputs model valid initialized DCT blocks and cover
/// both the low-frequency-only and full horizontal-pass forms; they are not a
/// production escape hatch or a public codec input.
#[cfg(all(coverage, target_arch = "aarch64"))]
pub(crate) fn __coverage_exercise_private_branches() {
    let block = [0i32; DCTSIZE2];
    let quant_table = [1i32; DCTSIZE2];
    let mut workspace = [0i32; DCTSIZE2];
    let mut output = vec![0u8; DCTSIZE2];

    jpeg_idct_islow_to_u8_safe(&block, &mut workspace, &mut output, DCTSIZE, 0, 0);
    assert!(output.iter().all(|&value| value == 128));

    jpeg_idct_islow_dequantized_to_u8_safe(
        &block,
        &quant_table,
        &mut workspace,
        &mut output,
        DCTSIZE,
        0,
        0,
        false,
    );
    jpeg_idct_islow_dequantized_to_u8_safe(
        &block,
        &quant_table,
        &mut workspace,
        &mut output,
        DCTSIZE,
        0,
        0,
        true,
    );
    assert!(output.iter().all(|&value| value == 128));
}

// ── JPEG Utilities ────────────────────────────────────────────────────────

/// Exact output of [`jpeg_idct_islow`] when only the natural-order DC
/// coefficient is nonzero.
#[inline(always)]
pub(crate) fn dc_only_output(dc: i32) -> u8 {
    let pass1 = descale(
        dc.wrapping_shl(CONST_BITS.cast_unsigned()),
        CONST_BITS.saturating_sub(PASS1_BITS),
    );
    range_limit(descale(pass1.wrapping_shl(CONST_BITS.cast_unsigned()), 18))
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
