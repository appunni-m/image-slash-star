// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

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

// ── JPEG Utilities ────────────────────────────────────────────────────────

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
        crate::codecs::jpeg::kernels::ycc_to_rgb_batch(
            y,
            cb,
            cr,
            &self.cr_r_tab,
            &self.cb_b_tab,
            &self.cr_g_tab,
            &self.cb_g_tab,
            output,
        );
    }
}
