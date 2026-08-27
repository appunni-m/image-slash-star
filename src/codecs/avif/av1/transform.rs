//! Safe scalar inverse transforms for the portable AV1 decoder.
//!
//! AV1's transform kernels are integer butterflies, not floating-point image
//! processing.  Keeping the arithmetic here explicit gives the decoder a
//! small, reusable boundary for progressively widening coefficient support
//! without importing a native DSP implementation.
//
// The AV1 integer butterfly equations intentionally use fixed-width wrapping
// products and shifts. These two lint exceptions document that arithmetic
// contract locally; they do not permit pointer operations or unchecked memory
// access anywhere in the transform module.
#![allow(clippy::arithmetic_side_effects, clippy::precedence)]

#[inline]
fn clamp_intermediate(value: i32) -> i32 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX))
}

#[inline]
fn rounded_dot(terms: &[(i32, i32)], bits: u32) -> i32 {
    let product = terms.iter().fold(0_i32, |sum, &(weight, input)| {
        sum.wrapping_add(weight.wrapping_mul(input))
    });
    product.wrapping_add(1_i32.wrapping_shl(bits.saturating_sub(1))) >> bits
}

fn inverse_dct4(input: [i32; 4]) -> [i32; 4] {
    // Keep the reduced 181/256 butterflies exactly as dav1d does. Using the
    // equivalent 12-bit cosine constants changes the rounding at this stage.
    let even_sum = input[0]
        .wrapping_add(input[2])
        .wrapping_mul(181)
        .wrapping_add(128)
        >> 8;
    let even_difference = input[0]
        .wrapping_sub(input[2])
        .wrapping_mul(181)
        .wrapping_add(128)
        >> 8;
    let odd_sum = (input[1]
        .wrapping_mul(1567)
        .wrapping_sub(input[3].wrapping_mul(3784 - 4096))
        .wrapping_add(2048)
        >> 12)
        .wrapping_sub(input[3]);
    let odd_difference = (input[1]
        .wrapping_mul(3784 - 4096)
        .wrapping_add(input[3].wrapping_mul(1567))
        .wrapping_add(2048)
        >> 12)
        .wrapping_add(input[1]);
    [
        clamp_intermediate(even_sum.wrapping_add(odd_difference)),
        clamp_intermediate(even_difference.wrapping_add(odd_sum)),
        clamp_intermediate(even_difference.wrapping_sub(odd_sum)),
        clamp_intermediate(even_sum.wrapping_sub(odd_difference)),
    ]
}

fn inverse_dct8(input: [i32; 8]) -> [i32; 8] {
    let even = inverse_dct4([input[0], input[2], input[4], input[6]]);
    let t4a = (input[1]
        .wrapping_mul(799)
        .wrapping_sub(input[7].wrapping_mul(4017 - 4096))
        .wrapping_add(2048)
        >> 12)
        .wrapping_sub(input[7]);
    let t5a = rounded_linear(input[5], 1703, input[3], -1138, 1024, 11);
    let t6a = rounded_linear(input[5], 1138, input[3], 1703, 1024, 11);
    let t7a = (input[1]
        .wrapping_mul(4017 - 4096)
        .wrapping_add(input[7].wrapping_mul(799))
        .wrapping_add(2048)
        >> 12)
        .wrapping_add(input[1]);
    let t4 = clamp_intermediate(t4a.wrapping_add(t5a));
    let t5 = rounded_linear(
        clamp_intermediate(t7a.wrapping_sub(t6a)),
        181,
        clamp_intermediate(t4a.wrapping_sub(t5a)),
        -181,
        128,
        8,
    );
    let t6 = rounded_linear(
        clamp_intermediate(t7a.wrapping_sub(t6a)),
        181,
        clamp_intermediate(t4a.wrapping_sub(t5a)),
        181,
        128,
        8,
    );
    let t7 = clamp_intermediate(t7a.wrapping_add(t6a));
    [
        clamp_intermediate(even[0].wrapping_add(t7)),
        clamp_intermediate(even[1].wrapping_add(t6)),
        clamp_intermediate(even[2].wrapping_add(t5)),
        clamp_intermediate(even[3].wrapping_add(t4)),
        clamp_intermediate(even[3].wrapping_sub(t4)),
        clamp_intermediate(even[2].wrapping_sub(t5)),
        clamp_intermediate(even[1].wrapping_sub(t6)),
        clamp_intermediate(even[0].wrapping_sub(t7)),
    ]
}

/// Apply one AV1 16-point inverse DCT pass.
///
/// The rectangular 8×16 transform uses this pass on each column after its
/// 8-point horizontal pass. Keeping it as a separate scalar primitive makes
/// the rectangular transform boundary testable without pretending that two
/// square transforms are equivalent to one AV1 rectangular transform.
// ✅ VERIFIED: rav1d 1.1.0 `src/itx_1d.rs`,
// `inv_dct16_1d_internal_c`; this is a safe scalar transcription with checked
// fixed-size storage and bounded intermediate values.
#[allow(
    dead_code,
    reason = "the exact rectangular coefficient decoder is a separate planned gap; retain this safe primitive for its integration"
)]
pub(crate) fn inverse_dct16(input: [i32; 16]) -> [i32; 16] {
    let even = inverse_dct8([
        input[0], input[2], input[4], input[6], input[8], input[10], input[12], input[14],
    ]);
    let in1 = input[1];
    let in3 = input[3];
    let in5 = input[5];
    let in7 = input[7];
    let in9 = input[9];
    let in11 = input[11];
    let in13 = input[13];
    let in15 = input[15];

    let t8a = (in1
        .wrapping_mul(401)
        .wrapping_sub(in15.wrapping_mul(4076 - 4096))
        .wrapping_add(2048)
        >> 12)
        .wrapping_sub(in15);
    let t9a = in9
        .wrapping_mul(1583)
        .wrapping_sub(in7.wrapping_mul(1299))
        .wrapping_add(1024)
        >> 11;
    let t10a = (in5
        .wrapping_mul(1931)
        .wrapping_sub(in11.wrapping_mul(3612 - 4096))
        .wrapping_add(2048)
        >> 12)
        .wrapping_sub(in11);
    let t11a = (in13
        .wrapping_mul(3920 - 4096)
        .wrapping_sub(in3.wrapping_mul(1189))
        .wrapping_add(2048)
        >> 12)
        .wrapping_add(in13);
    let t12a = (in13
        .wrapping_mul(1189)
        .wrapping_add(in3.wrapping_mul(3920 - 4096))
        .wrapping_add(2048)
        >> 12)
        .wrapping_add(in3);
    let t13a = (in5
        .wrapping_mul(3612 - 4096)
        .wrapping_add(in11.wrapping_mul(1931))
        .wrapping_add(2048)
        >> 12)
        .wrapping_add(in5);
    let t14a = in9
        .wrapping_mul(1299)
        .wrapping_add(in7.wrapping_mul(1583))
        .wrapping_add(1024)
        >> 11;
    let t15a = (in1
        .wrapping_mul(4076 - 4096)
        .wrapping_add(in15.wrapping_mul(401))
        .wrapping_add(2048)
        >> 12)
        .wrapping_add(in1);

    let t8 = clamp_intermediate(t8a.wrapping_add(t9a));
    let mut t9 = clamp_intermediate(t8a.wrapping_sub(t9a));
    let mut t10 = clamp_intermediate(t11a.wrapping_sub(t10a));
    let t11 = clamp_intermediate(t11a.wrapping_add(t10a));
    let t12 = clamp_intermediate(t12a.wrapping_add(t13a));
    let mut t13 = clamp_intermediate(t12a.wrapping_sub(t13a));
    let mut t14 = clamp_intermediate(t15a.wrapping_sub(t14a));
    let t15 = clamp_intermediate(t15a.wrapping_add(t14a));

    let t9a = (t14
        .wrapping_mul(1567)
        .wrapping_sub(t9.wrapping_mul(3784 - 4096))
        .wrapping_add(2048)
        >> 12)
        .wrapping_sub(t9);
    let t14a = (t14
        .wrapping_mul(3784 - 4096)
        .wrapping_add(t9.wrapping_mul(1567))
        .wrapping_add(2048)
        >> 12)
        .wrapping_add(t14);
    let t10a = (t13
        .wrapping_mul(3784 - 4096)
        .wrapping_add(t10.wrapping_mul(1567))
        .wrapping_neg()
        .wrapping_add(2048)
        >> 12)
        .wrapping_sub(t13);
    let t13a = (t13
        .wrapping_mul(1567)
        .wrapping_sub(t10.wrapping_mul(3784 - 4096))
        .wrapping_add(2048)
        >> 12)
        .wrapping_sub(t10);

    let t8a = clamp_intermediate(t8.wrapping_add(t11));
    t9 = clamp_intermediate(t9a.wrapping_add(t10a));
    t10 = clamp_intermediate(t9a.wrapping_sub(t10a));
    let t11a = clamp_intermediate(t8.wrapping_sub(t11));
    let t12a = clamp_intermediate(t15.wrapping_sub(t12));
    t13 = clamp_intermediate(t14a.wrapping_sub(t13a));
    t14 = clamp_intermediate(t14a.wrapping_add(t13a));
    let t15a = clamp_intermediate(t15.wrapping_add(t12));

    let t10a = (t13.wrapping_sub(t10).wrapping_mul(181).wrapping_add(128)) >> 8;
    let t13a = (t13.wrapping_add(t10).wrapping_mul(181).wrapping_add(128)) >> 8;
    let t11 = (t12a.wrapping_sub(t11a).wrapping_mul(181).wrapping_add(128)) >> 8;
    let t12 = (t12a.wrapping_add(t11a).wrapping_mul(181).wrapping_add(128)) >> 8;

    [
        clamp_intermediate(even[0].wrapping_add(t15a)),
        clamp_intermediate(even[1].wrapping_add(t14)),
        clamp_intermediate(even[2].wrapping_add(t13a)),
        clamp_intermediate(even[3].wrapping_add(t12)),
        clamp_intermediate(even[4].wrapping_add(t11)),
        clamp_intermediate(even[5].wrapping_add(t10a)),
        clamp_intermediate(even[6].wrapping_add(t9)),
        clamp_intermediate(even[7].wrapping_add(t8a)),
        clamp_intermediate(even[7].wrapping_sub(t8a)),
        clamp_intermediate(even[6].wrapping_sub(t9)),
        clamp_intermediate(even[5].wrapping_sub(t10a)),
        clamp_intermediate(even[4].wrapping_sub(t11)),
        clamp_intermediate(even[3].wrapping_sub(t12)),
        clamp_intermediate(even[2].wrapping_sub(t13a)),
        clamp_intermediate(even[1].wrapping_sub(t14)),
        clamp_intermediate(even[0].wrapping_sub(t15a)),
    ]
}

#[inline]
fn rounded_linear(
    first: i32,
    first_weight: i32,
    second: i32,
    second_weight: i32,
    rounding: i32,
    bits: u32,
) -> i32 {
    first
        .wrapping_mul(first_weight)
        .wrapping_add(second.wrapping_mul(second_weight))
        .wrapping_add(rounding)
        >> bits
}

/// Apply one AV1 32-point inverse DCT pass.
// ✅ VERIFIED: rav1d 1.1.0 `src/itx_1d.rs`, `inv_dct32_1d_internal_c`.
// The fixed-array transcription keeps the reference butterfly order while
// making malformed test inputs total through wrapping arithmetic and the AV1
// intermediate clamp.
fn inverse_dct32(input: [i32; 32]) -> [i32; 32] {
    let even = inverse_dct16([
        input[0], input[2], input[4], input[6], input[8], input[10], input[12], input[14],
        input[16], input[18], input[20], input[22], input[24], input[26], input[28], input[30],
    ]);
    let in1 = input[1];
    let in3 = input[3];
    let in5 = input[5];
    let in7 = input[7];
    let in9 = input[9];
    let in11 = input[11];
    let in13 = input[13];
    let in15 = input[15];
    let in17 = input[17];
    let in19 = input[19];
    let in21 = input[21];
    let in23 = input[23];
    let in25 = input[25];
    let in27 = input[27];
    let in29 = input[29];
    let in31 = input[31];

    let t16a = rounded_linear(in1, 201, in31, -(4091 - 4096), 2048, 12).wrapping_sub(in31);
    let t17a = rounded_linear(in17, 3035 - 4096, in15, -2751, 2048, 12).wrapping_add(in17);
    let t18a = rounded_linear(in9, 1751, in23, -(3703 - 4096), 2048, 12).wrapping_sub(in23);
    let t19a = rounded_linear(in25, 3857 - 4096, in7, -1380, 2048, 12).wrapping_add(in25);
    let t20a = rounded_linear(in5, 995, in27, -(3973 - 4096), 2048, 12).wrapping_sub(in27);
    let t21a = rounded_linear(in21, 3513 - 4096, in11, -2106, 2048, 12).wrapping_add(in21);
    let t22a = rounded_linear(in13, 1220, in19, -1645, 1024, 11);
    let t23a = rounded_linear(in29, 4052 - 4096, in3, -601, 2048, 12).wrapping_add(in29);
    let t24a = rounded_linear(in29, 601, in3, 4052 - 4096, 2048, 12).wrapping_add(in3);
    let t25a = rounded_linear(in13, 1645, in19, 1220, 1024, 11);
    let t26a = rounded_linear(in21, 2106, in11, 3513 - 4096, 2048, 12).wrapping_add(in11);
    let t27a = rounded_linear(in5, 3973 - 4096, in27, 995, 2048, 12).wrapping_add(in5);
    let t28a = rounded_linear(in25, 1380, in7, 3857 - 4096, 2048, 12).wrapping_add(in7);
    let t29a = rounded_linear(in9, 3703 - 4096, in23, 1751, 2048, 12).wrapping_add(in9);
    let t30a = rounded_linear(in17, 2751, in15, 3035 - 4096, 2048, 12).wrapping_add(in15);
    let t31a = rounded_linear(in1, 4091 - 4096, in31, 201, 2048, 12).wrapping_add(in1);

    let t16 = clamp_intermediate(t16a.wrapping_add(t17a));
    let mut t17 = clamp_intermediate(t16a.wrapping_sub(t17a));
    let mut t18 = clamp_intermediate(t19a.wrapping_sub(t18a));
    let t19 = clamp_intermediate(t19a.wrapping_add(t18a));
    let t20 = clamp_intermediate(t20a.wrapping_add(t21a));
    let mut t21 = clamp_intermediate(t20a.wrapping_sub(t21a));
    let mut t22 = clamp_intermediate(t23a.wrapping_sub(t22a));
    let t23 = clamp_intermediate(t23a.wrapping_add(t22a));
    let t24 = clamp_intermediate(t24a.wrapping_add(t25a));
    let mut t25 = clamp_intermediate(t24a.wrapping_sub(t25a));
    let mut t26 = clamp_intermediate(t27a.wrapping_sub(t26a));
    let t27 = clamp_intermediate(t27a.wrapping_add(t26a));
    let t28 = clamp_intermediate(t28a.wrapping_add(t29a));
    let mut t29 = clamp_intermediate(t28a.wrapping_sub(t29a));
    let mut t30 = clamp_intermediate(t31a.wrapping_sub(t30a));
    let t31 = clamp_intermediate(t31a.wrapping_add(t30a));

    let t17a = rounded_linear(t30, 799, t17, -(4017 - 4096), 2048, 12).wrapping_sub(t17);
    let t30a = rounded_linear(t30, 4017 - 4096, t17, 799, 2048, 12).wrapping_add(t30);
    let t18a = rounded_linear(t29, -(4017 - 4096), t18, -799, 2048, 12).wrapping_sub(t29);
    let t29a = rounded_linear(t29, 799, t18, -(4017 - 4096), 2048, 12).wrapping_sub(t18);
    let t21a = rounded_linear(t26, 1703, t21, -1138, 1024, 11);
    let t26a = rounded_linear(t26, 1138, t21, 1703, 1024, 11);
    let t22a = rounded_linear(t25, -1138, t22, -1703, 1024, 11);
    let t25a = rounded_linear(t25, 1703, t22, -1138, 1024, 11);

    let t16a = clamp_intermediate(t16.wrapping_add(t19));
    t17 = clamp_intermediate(t17a.wrapping_add(t18a));
    t18 = clamp_intermediate(t17a.wrapping_sub(t18a));
    let t19a = clamp_intermediate(t16.wrapping_sub(t19));
    let t20a = clamp_intermediate(t23.wrapping_sub(t20));
    t21 = clamp_intermediate(t22a.wrapping_sub(t21a));
    t22 = clamp_intermediate(t22a.wrapping_add(t21a));
    let t23a = clamp_intermediate(t23.wrapping_add(t20));
    let t24a = clamp_intermediate(t24.wrapping_add(t27));
    t25 = clamp_intermediate(t25a.wrapping_add(t26a));
    t26 = clamp_intermediate(t25a.wrapping_sub(t26a));
    let t27a = clamp_intermediate(t24.wrapping_sub(t27));
    let t28a = clamp_intermediate(t31.wrapping_sub(t28));
    t29 = clamp_intermediate(t30a.wrapping_sub(t29a));
    t30 = clamp_intermediate(t30a.wrapping_add(t29a));
    let t31a = clamp_intermediate(t31.wrapping_add(t28));

    let t18a = rounded_linear(t29, 1567, t18, -(3784 - 4096), 2048, 12).wrapping_sub(t18);
    let t29a = rounded_linear(t29, 3784 - 4096, t18, 1567, 2048, 12).wrapping_add(t29);
    let t19 = rounded_linear(t28a, 1567, t19a, -(3784 - 4096), 2048, 12).wrapping_sub(t19a);
    let t28 = rounded_linear(t28a, 3784 - 4096, t19a, 1567, 2048, 12).wrapping_add(t28a);
    let t20 = rounded_linear(t27a, -(3784 - 4096), t20a, -1567, 2048, 12).wrapping_sub(t27a);
    let t27 = rounded_linear(t27a, 1567, t20a, -(3784 - 4096), 2048, 12).wrapping_sub(t20a);
    let t21a = rounded_linear(t26, -(3784 - 4096), t21, -1567, 2048, 12).wrapping_sub(t26);
    let t26a = rounded_linear(t26, 1567, t21, -(3784 - 4096), 2048, 12).wrapping_sub(t21);

    let t16 = clamp_intermediate(t16a.wrapping_add(t23a));
    let t17a = clamp_intermediate(t17.wrapping_add(t22));
    let t18 = clamp_intermediate(t18a.wrapping_add(t21a));
    let t19a = clamp_intermediate(t19.wrapping_add(t20));
    let t20a = clamp_intermediate(t19.wrapping_sub(t20));
    let t21 = clamp_intermediate(t18a.wrapping_sub(t21a));
    let t22a = clamp_intermediate(t17.wrapping_sub(t22));
    let t23 = clamp_intermediate(t16a.wrapping_sub(t23a));
    let t24 = clamp_intermediate(t31a.wrapping_sub(t24a));
    let t25a = clamp_intermediate(t30.wrapping_sub(t25));
    let t26 = clamp_intermediate(t29a.wrapping_sub(t26a));
    let t27a = clamp_intermediate(t28.wrapping_sub(t27));
    let t28a = clamp_intermediate(t28.wrapping_add(t27));
    let t29 = clamp_intermediate(t29a.wrapping_add(t26a));
    let t30a = clamp_intermediate(t30.wrapping_add(t25));
    let t31 = clamp_intermediate(t31a.wrapping_add(t24a));

    let t20 = rounded_linear(t27a, 181, t20a, -181, 128, 8);
    let t27 = rounded_linear(t27a, 181, t20a, 181, 128, 8);
    let t21a = rounded_linear(t26, 181, t21, -181, 128, 8);
    let t26a = rounded_linear(t26, 181, t21, 181, 128, 8);
    let t22 = rounded_linear(t25a, 181, t22a, -181, 128, 8);
    let t25 = rounded_linear(t25a, 181, t22a, 181, 128, 8);
    let t23a = rounded_linear(t24, 181, t23, -181, 128, 8);
    let t24a = rounded_linear(t24, 181, t23, 181, 128, 8);

    [
        clamp_intermediate(even[0].wrapping_add(t31)),
        clamp_intermediate(even[1].wrapping_add(t30a)),
        clamp_intermediate(even[2].wrapping_add(t29)),
        clamp_intermediate(even[3].wrapping_add(t28a)),
        clamp_intermediate(even[4].wrapping_add(t27)),
        clamp_intermediate(even[5].wrapping_add(t26a)),
        clamp_intermediate(even[6].wrapping_add(t25)),
        clamp_intermediate(even[7].wrapping_add(t24a)),
        clamp_intermediate(even[8].wrapping_add(t23a)),
        clamp_intermediate(even[9].wrapping_add(t22)),
        clamp_intermediate(even[10].wrapping_add(t21a)),
        clamp_intermediate(even[11].wrapping_add(t20)),
        clamp_intermediate(even[12].wrapping_add(t19a)),
        clamp_intermediate(even[13].wrapping_add(t18)),
        clamp_intermediate(even[14].wrapping_add(t17a)),
        clamp_intermediate(even[15].wrapping_add(t16)),
        clamp_intermediate(even[15].wrapping_sub(t16)),
        clamp_intermediate(even[14].wrapping_sub(t17a)),
        clamp_intermediate(even[13].wrapping_sub(t18)),
        clamp_intermediate(even[12].wrapping_sub(t19a)),
        clamp_intermediate(even[11].wrapping_sub(t20)),
        clamp_intermediate(even[10].wrapping_sub(t21a)),
        clamp_intermediate(even[9].wrapping_sub(t22)),
        clamp_intermediate(even[8].wrapping_sub(t23a)),
        clamp_intermediate(even[7].wrapping_sub(t24a)),
        clamp_intermediate(even[6].wrapping_sub(t25)),
        clamp_intermediate(even[5].wrapping_sub(t26a)),
        clamp_intermediate(even[4].wrapping_sub(t27)),
        clamp_intermediate(even[3].wrapping_sub(t28a)),
        clamp_intermediate(even[2].wrapping_sub(t29)),
        clamp_intermediate(even[1].wrapping_sub(t30a)),
        clamp_intermediate(even[0].wrapping_sub(t31)),
    ]
}

fn inverse_dct4_tx64(input: [i32; 4]) -> [i32; 4] {
    let mut c = input.map(clamp_intermediate);
    let clip = |v: i32| clamp_intermediate(v);
    let in0 = c[0];
    let in1 = c[1];

    let t1 = in0 * 181 + 128 >> 8;
    let t0 = t1;
    let t2 = in1 * 1567 + 2048 >> 12;
    let t3 = in1 * 3784 + 2048 >> 12;

    c[0] = clip(t0 + t3);
    c[1] = clip(t1 + t2);
    c[2] = clip(t1 - t2);
    c[3] = clip(t0 - t3);

    c
}

fn inverse_dct8_tx64(input: [i32; 8]) -> [i32; 8] {
    let clip = |v: i32| clamp_intermediate(v);
    let even = inverse_dct4_tx64(std::array::from_fn(|index| input[index * 2]));
    let mut c = input.map(clamp_intermediate);
    for (index, value) in even.into_iter().enumerate() {
        c[index * 2] = value;
    }

    let in1 = c[1];
    let in3 = c[3];

    let t4a = in1 * 799 + 2048 >> 12;
    let mut t5a = in3 * -2276 + 2048 >> 12;
    let mut t6a = in3 * 3406 + 2048 >> 12;
    let t7a = in1 * 4017 + 2048 >> 12;

    let t4 = clip(t4a + t5a);
    t5a = clip(t4a - t5a);
    let t7 = clip(t7a + t6a);
    t6a = clip(t7a - t6a);

    let t5 = (t6a - t5a) * 181 + 128 >> 8;
    let t6 = (t6a + t5a) * 181 + 128 >> 8;

    let t0 = c[0];
    let t1 = c[2];
    let t2 = c[4];
    let t3 = c[6];

    c[0] = clip(t0 + t7);
    c[1] = clip(t1 + t6);
    c[2] = clip(t2 + t5);
    c[3] = clip(t3 + t4);
    c[4] = clip(t3 - t4);
    c[5] = clip(t2 - t5);
    c[6] = clip(t1 - t6);
    c[7] = clip(t0 - t7);

    c
}

fn inverse_dct16_tx64(input: [i32; 16]) -> [i32; 16] {
    let clip = |v: i32| clamp_intermediate(v);
    let even = inverse_dct8_tx64(std::array::from_fn(|index| input[index * 2]));
    let mut c = input.map(clamp_intermediate);
    for (index, value) in even.into_iter().enumerate() {
        c[index * 2] = value;
    }

    let in1 = c[1];
    let in3 = c[3];
    let in5 = c[5];
    let in7 = c[7];

    let mut t8a = in1 * 401 + 2048 >> 12;
    let mut t9a = in7 * -2598 + 2048 >> 12;
    let mut t10a = in5 * 1931 + 2048 >> 12;
    let mut t11a = in3 * -1189 + 2048 >> 12;
    let mut t12a = in3 * 3920 + 2048 >> 12;
    let mut t13a = in5 * 3612 + 2048 >> 12;
    let mut t14a = in7 * 3166 + 2048 >> 12;
    let mut t15a = in1 * 4076 + 2048 >> 12;

    let t8 = clip(t8a + t9a);
    let mut t9 = clip(t8a - t9a);
    let mut t10 = clip(t11a - t10a);
    let mut t11 = clip(t11a + t10a);
    let mut t12 = clip(t12a + t13a);
    let mut t13 = clip(t12a - t13a);
    let mut t14 = clip(t15a - t14a);
    let t15 = clip(t15a + t14a);

    t9a = (t14 * 1567 - t9 * (3784 - 4096) + 2048 >> 12) - t9;
    t14a = (t14 * (3784 - 4096) + t9 * 1567 + 2048 >> 12) + t14;
    t10a = (-(t13 * (3784 - 4096) + t10 * 1567) + 2048 >> 12) - t13;
    t13a = (t13 * 1567 - t10 * (3784 - 4096) + 2048 >> 12) - t10;
    t8a = clip(t8 + t11);
    t9 = clip(t9a + t10a);
    t10 = clip(t9a - t10a);
    t11a = clip(t8 - t11);
    t12a = clip(t15 - t12);
    t13 = clip(t14a - t13a);
    t14 = clip(t14a + t13a);
    t15a = clip(t15 + t12);

    t10a = (t13 - t10) * 181 + 128 >> 8;
    t13a = (t13 + t10) * 181 + 128 >> 8;
    t11 = (t12a - t11a) * 181 + 128 >> 8;
    t12 = (t12a + t11a) * 181 + 128 >> 8;

    let t0 = c[0];
    let t1 = c[2];
    let t2 = c[4];
    let t3 = c[6];
    let t4 = c[8];
    let t5 = c[10];
    let t6 = c[12];
    let t7 = c[14];

    c[0] = clip(t0 + t15a);
    c[1] = clip(t1 + t14);
    c[2] = clip(t2 + t13a);
    c[3] = clip(t3 + t12);
    c[4] = clip(t4 + t11);
    c[5] = clip(t5 + t10a);
    c[6] = clip(t6 + t9);
    c[7] = clip(t7 + t8a);
    c[8] = clip(t7 - t8a);
    c[9] = clip(t6 - t9);
    c[10] = clip(t5 - t10a);
    c[11] = clip(t4 - t11);
    c[12] = clip(t3 - t12);
    c[13] = clip(t2 - t13a);
    c[14] = clip(t1 - t14);
    c[15] = clip(t0 - t15a);

    c
}

fn inverse_dct32_tx64(input: [i32; 32]) -> [i32; 32] {
    let clip = |v: i32| clamp_intermediate(v);
    let even = inverse_dct16_tx64(std::array::from_fn(|index| input[index * 2]));
    let mut c = input.map(clamp_intermediate);
    for (index, value) in even.into_iter().enumerate() {
        c[index * 2] = value;
    }

    let in1 = c[1];
    let in3 = c[3];
    let in5 = c[5];
    let in7 = c[7];
    let in9 = c[9];
    let in11 = c[11];
    let in13 = c[13];
    let in15 = c[15];

    let mut t16a = in1 * 201 + 2048 >> 12;
    let mut t17a = in15 * -2751 + 2048 >> 12;
    let mut t18a = in9 * 1751 + 2048 >> 12;
    let mut t19a = in7 * -1380 + 2048 >> 12;
    let mut t20a = in5 * 995 + 2048 >> 12;
    let mut t21a = in11 * -2106 + 2048 >> 12;
    let mut t22a = in13 * 2440 + 2048 >> 12;
    let mut t23a = in3 * -601 + 2048 >> 12;
    let mut t24a = in3 * 4052 + 2048 >> 12;
    let mut t25a = in13 * 3290 + 2048 >> 12;
    let mut t26a = in11 * 3513 + 2048 >> 12;
    let mut t27a = in5 * 3973 + 2048 >> 12;
    let mut t28a = in7 * 3857 + 2048 >> 12;
    let mut t29a = in9 * 3703 + 2048 >> 12;
    let mut t30a = in15 * 3035 + 2048 >> 12;
    let mut t31a = in1 * 4091 + 2048 >> 12;

    let mut t16 = clip(t16a + t17a);
    let mut t17 = clip(t16a - t17a);
    let mut t18 = clip(t19a - t18a);
    let mut t19 = clip(t19a + t18a);
    let mut t20 = clip(t20a + t21a);
    let mut t21 = clip(t20a - t21a);
    let mut t22 = clip(t23a - t22a);
    let mut t23 = clip(t23a + t22a);
    let mut t24 = clip(t24a + t25a);
    let mut t25 = clip(t24a - t25a);
    let mut t26 = clip(t27a - t26a);
    let mut t27 = clip(t27a + t26a);
    let mut t28 = clip(t28a + t29a);
    let mut t29 = clip(t28a - t29a);
    let mut t30 = clip(t31a - t30a);
    let mut t31 = clip(t31a + t30a);

    t17a = (t30 * 799 - t17 * (4017 - 4096) + 2048 >> 12) - t17;
    t30a = (t30 * (4017 - 4096) + t17 * 799 + 2048 >> 12) + t30;
    t18a = (-(t29 * (4017 - 4096) + t18 * 799) + 2048 >> 12) - t29;
    t29a = (t29 * 799 - t18 * (4017 - 4096) + 2048 >> 12) - t18;
    t21a = t26 * 1703 - t21 * 1138 + 1024 >> 11;
    t26a = t26 * 1138 + t21 * 1703 + 1024 >> 11;
    t22a = -(t25 * 1138 + t22 * 1703) + 1024 >> 11;
    t25a = t25 * 1703 - t22 * 1138 + 1024 >> 11;

    t16a = clip(t16 + t19);
    t17 = clip(t17a + t18a);
    t18 = clip(t17a - t18a);
    t19a = clip(t16 - t19);
    t20a = clip(t23 - t20);
    t21 = clip(t22a - t21a);
    t22 = clip(t22a + t21a);
    t23a = clip(t23 + t20);
    t24a = clip(t24 + t27);
    t25 = clip(t25a + t26a);
    t26 = clip(t25a - t26a);
    t27a = clip(t24 - t27);
    t28a = clip(t31 - t28);
    t29 = clip(t30a - t29a);
    t30 = clip(t30a + t29a);
    t31a = clip(t31 + t28);

    t18a = (t29 * 1567 - t18 * (3784 - 4096) + 2048 >> 12) - t18;
    t29a = (t29 * (3784 - 4096) + t18 * 1567 + 2048 >> 12) + t29;
    t19 = (t28a * 1567 - t19a * (3784 - 4096) + 2048 >> 12) - t19a;
    t28 = (t28a * (3784 - 4096) + t19a * 1567 + 2048 >> 12) + t28a;
    t20 = (-(t27a * (3784 - 4096) + t20a * 1567) + 2048 >> 12) - t27a;
    t27 = (t27a * 1567 - t20a * (3784 - 4096) + 2048 >> 12) - t20a;
    t21a = (-(t26 * (3784 - 4096) + t21 * 1567) + 2048 >> 12) - t26;
    t26a = (t26 * 1567 - t21 * (3784 - 4096) + 2048 >> 12) - t21;

    t16 = clip(t16a + t23a);
    t17a = clip(t17 + t22);
    t18 = clip(t18a + t21a);
    t19a = clip(t19 + t20);
    t20a = clip(t19 - t20);
    t21 = clip(t18a - t21a);
    t22a = clip(t17 - t22);
    t23 = clip(t16a - t23a);
    t24 = clip(t31a - t24a);
    t25a = clip(t30 - t25);
    t26 = clip(t29a - t26a);
    t27a = clip(t28 - t27);
    t28a = clip(t28 + t27);
    t29 = clip(t29a + t26a);
    t30a = clip(t30 + t25);
    t31 = clip(t31a + t24a);

    t20 = (t27a - t20a) * 181 + 128 >> 8;
    t27 = (t27a + t20a) * 181 + 128 >> 8;
    t21a = (t26 - t21) * 181 + 128 >> 8;
    t26a = (t26 + t21) * 181 + 128 >> 8;
    t22 = (t25a - t22a) * 181 + 128 >> 8;
    t25 = (t25a + t22a) * 181 + 128 >> 8;
    t23a = (t24 - t23) * 181 + 128 >> 8;
    t24a = (t24 + t23) * 181 + 128 >> 8;

    let t0 = c[0];
    let t1 = c[2];
    let t2 = c[4];
    let t3 = c[6];
    let t4 = c[8];
    let t5 = c[10];
    let t6 = c[12];
    let t7 = c[14];
    let t8 = c[16];
    let t9 = c[18];
    let t10 = c[20];
    let t11 = c[22];
    let t12 = c[24];
    let t13 = c[26];
    let t14 = c[28];
    let t15 = c[30];

    c[0] = clip(t0 + t31);
    c[1] = clip(t1 + t30a);
    c[2] = clip(t2 + t29);
    c[3] = clip(t3 + t28a);
    c[4] = clip(t4 + t27);
    c[5] = clip(t5 + t26a);
    c[6] = clip(t6 + t25);
    c[7] = clip(t7 + t24a);
    c[8] = clip(t8 + t23a);
    c[9] = clip(t9 + t22);
    c[10] = clip(t10 + t21a);
    c[11] = clip(t11 + t20);
    c[12] = clip(t12 + t19a);
    c[13] = clip(t13 + t18);
    c[14] = clip(t14 + t17a);
    c[15] = clip(t15 + t16);
    c[16] = clip(t15 - t16);
    c[17] = clip(t14 - t17a);
    c[18] = clip(t13 - t18);
    c[19] = clip(t12 - t19a);
    c[20] = clip(t11 - t20);
    c[21] = clip(t10 - t21a);
    c[22] = clip(t9 - t22);
    c[23] = clip(t8 - t23a);
    c[24] = clip(t7 - t24a);
    c[25] = clip(t6 - t25);
    c[26] = clip(t5 - t26a);
    c[27] = clip(t4 - t27);
    c[28] = clip(t3 - t28a);
    c[29] = clip(t2 - t29);
    c[30] = clip(t1 - t30a);
    c[31] = clip(t0 - t31);

    c
}

fn inverse_dct64(input: [i32; 64]) -> [i32; 64] {
    let clip = |v: i32| clamp_intermediate(v);
    let even = inverse_dct32_tx64(std::array::from_fn(|index| input[index * 2]));
    let mut c = input.map(clamp_intermediate);
    for (index, value) in even.into_iter().enumerate() {
        c[index * 2] = value;
    }

    let in1 = c[1];
    let in3 = c[3];
    let in5 = c[5];
    let in7 = c[7];
    let in9 = c[9];
    let in11 = c[11];
    let in13 = c[13];
    let in15 = c[15];
    let in17 = c[17];
    let in19 = c[19];
    let in21 = c[21];
    let in23 = c[23];
    let in25 = c[25];
    let in27 = c[27];
    let in29 = c[29];
    let in31 = c[31];

    let mut t32a = in1 * 101 + 2048 >> 12;
    let mut t33a = in31 * -2824 + 2048 >> 12;
    let mut t34a = in17 * 1660 + 2048 >> 12;
    let mut t35a = in15 * -1474 + 2048 >> 12;
    let mut t36a = in9 * 897 + 2048 >> 12;
    let mut t37a = in23 * -2191 + 2048 >> 12;
    let mut t38a = in25 * 2359 + 2048 >> 12;
    let mut t39a = in7 * -700 + 2048 >> 12;
    let mut t40a = in5 * 501 + 2048 >> 12;
    let mut t41a = in27 * -2520 + 2048 >> 12;
    let mut t42a = in21 * 2019 + 2048 >> 12;
    let mut t43a = in11 * -1092 + 2048 >> 12;
    let mut t44a = in13 * 1285 + 2048 >> 12;
    let mut t45a = in19 * -1842 + 2048 >> 12;
    let mut t46a = in29 * 2675 + 2048 >> 12;
    let mut t47a = in3 * -301 + 2048 >> 12;
    let mut t48a = in3 * 4085 + 2048 >> 12;
    let mut t49a = in29 * 3102 + 2048 >> 12;
    let mut t50a = in19 * 3659 + 2048 >> 12;
    let mut t51a = in13 * 3889 + 2048 >> 12;
    let mut t52a = in11 * 3948 + 2048 >> 12;
    let mut t53a = in21 * 3564 + 2048 >> 12;
    let mut t54a = in27 * 3229 + 2048 >> 12;
    let mut t55a = in5 * 4065 + 2048 >> 12;
    let mut t56a = in7 * 4036 + 2048 >> 12;
    let mut t57a = in25 * 3349 + 2048 >> 12;
    let mut t58a = in23 * 3461 + 2048 >> 12;
    let mut t59a = in9 * 3996 + 2048 >> 12;
    let mut t60a = in15 * 3822 + 2048 >> 12;
    let mut t61a = in17 * 3745 + 2048 >> 12;
    let mut t62a = in31 * 2967 + 2048 >> 12;
    let mut t63a = in1 * 4095 + 2048 >> 12;

    let mut t32 = clip(t32a + t33a);
    let mut t33 = clip(t32a - t33a);
    let mut t34 = clip(t35a - t34a);
    let mut t35 = clip(t35a + t34a);
    let mut t36 = clip(t36a + t37a);
    let mut t37 = clip(t36a - t37a);
    let mut t38 = clip(t39a - t38a);
    let mut t39 = clip(t39a + t38a);
    let mut t40 = clip(t40a + t41a);
    let mut t41 = clip(t40a - t41a);
    let mut t42 = clip(t43a - t42a);
    let mut t43 = clip(t43a + t42a);
    let mut t44 = clip(t44a + t45a);
    let mut t45 = clip(t44a - t45a);
    let mut t46 = clip(t47a - t46a);
    let mut t47 = clip(t47a + t46a);
    let mut t48 = clip(t48a + t49a);
    let mut t49 = clip(t48a - t49a);
    let mut t50 = clip(t51a - t50a);
    let mut t51 = clip(t51a + t50a);
    let mut t52 = clip(t52a + t53a);
    let mut t53 = clip(t52a - t53a);
    let mut t54 = clip(t55a - t54a);
    let mut t55 = clip(t55a + t54a);
    let mut t56 = clip(t56a + t57a);
    let mut t57 = clip(t56a - t57a);
    let mut t58 = clip(t59a - t58a);
    let mut t59 = clip(t59a + t58a);
    let mut t60 = clip(t60a + t61a);
    let mut t61 = clip(t60a - t61a);
    let mut t62 = clip(t63a - t62a);
    let mut t63 = clip(t63a + t62a);

    t33a = (t33 * (4096 - 4076) + t62 * 401 + 2048 >> 12) - t33;
    t34a = (t34 * -401 + t61 * (4096 - 4076) + 2048 >> 12) - t61;
    t37a = t37 * -1299 + t58 * 1583 + 1024 >> 11;
    t38a = t38 * -1583 + t57 * -1299 + 1024 >> 11;
    t41a = (t41 * (4096 - 3612) + t54 * 1931 + 2048 >> 12) - t41;
    t42a = (t42 * -1931 + t53 * (4096 - 3612) + 2048 >> 12) - t53;
    t45a = (t45 * -1189 + t50 * (3920 - 4096) + 2048 >> 12) + t50;
    t46a = (t46 * (4096 - 3920) + t49 * -1189 + 2048 >> 12) - t46;
    t49a = (t46 * -1189 + t49 * (3920 - 4096) + 2048 >> 12) + t49;
    t50a = (t45 * (3920 - 4096) + t50 * 1189 + 2048 >> 12) + t45;
    t53a = (t42 * (4096 - 3612) + t53 * 1931 + 2048 >> 12) - t42;
    t54a = (t41 * 1931 + t54 * (3612 - 4096) + 2048 >> 12) + t54;
    t57a = t38 * -1299 + t57 * 1583 + 1024 >> 11;
    t58a = t37 * 1583 + t58 * 1299 + 1024 >> 11;
    t61a = (t34 * (4096 - 4076) + t61 * 401 + 2048 >> 12) - t34;
    t62a = (t33 * 401 + t62 * (4076 - 4096) + 2048 >> 12) + t62;

    t32a = clip(t32 + t35);
    t33 = clip(t33a + t34a);
    t34 = clip(t33a - t34a);
    t35a = clip(t32 - t35);
    t36a = clip(t39 - t36);
    t37 = clip(t38a - t37a);
    t38 = clip(t38a + t37a);
    t39a = clip(t39 + t36);
    t40a = clip(t40 + t43);
    t41 = clip(t41a + t42a);
    t42 = clip(t41a - t42a);
    t43a = clip(t40 - t43);
    t44a = clip(t47 - t44);
    t45 = clip(t46a - t45a);
    t46 = clip(t46a + t45a);
    t47a = clip(t47 + t44);
    t48a = clip(t48 + t51);
    t49 = clip(t49a + t50a);
    t50 = clip(t49a - t50a);
    t51a = clip(t48 - t51);
    t52a = clip(t55 - t52);
    t53 = clip(t54a - t53a);
    t54 = clip(t54a + t53a);
    t55a = clip(t55 + t52);
    t56a = clip(t56 + t59);
    t57 = clip(t57a + t58a);
    t58 = clip(t57a - t58a);
    t59a = clip(t56 - t59);
    t60a = clip(t63 - t60);
    t61 = clip(t62a - t61a);
    t62 = clip(t62a + t61a);
    t63a = clip(t63 + t60);

    t34a = (t34 * (4096 - 4017) + t61 * 799 + 2048 >> 12) - t34;
    t35 = (t35a * (4096 - 4017) + t60a * 799 + 2048 >> 12) - t35a;
    t36 = (t36a * -799 + t59a * (4096 - 4017) + 2048 >> 12) - t59a;
    t37a = (t37 * -799 + t58 * (4096 - 4017) + 2048 >> 12) - t58;
    t42a = t42 * -1138 + t53 * 1703 + 1024 >> 11;
    t43 = t43a * -1138 + t52a * 1703 + 1024 >> 11;
    t44 = t44a * -1703 + t51a * -1138 + 1024 >> 11;
    t45a = t45 * -1703 + t50 * -1138 + 1024 >> 11;
    t50a = t45 * -1138 + t50 * 1703 + 1024 >> 11;
    t51 = t44a * -1138 + t51a * 1703 + 1024 >> 11;
    t52 = t43a * 1703 + t52a * 1138 + 1024 >> 11;
    t53a = t42 * 1703 + t53 * 1138 + 1024 >> 11;
    t58a = (t37 * (4096 - 4017) + t58 * 799 + 2048 >> 12) - t37;
    t59 = (t36a * (4096 - 4017) + t59a * 799 + 2048 >> 12) - t36a;
    t60 = (t35a * 799 + t60a * (4017 - 4096) + 2048 >> 12) + t60a;
    t61a = (t34 * 799 + t61 * (4017 - 4096) + 2048 >> 12) + t61;

    t32 = clip(t32a + t39a);
    t33a = clip(t33 + t38);
    t34 = clip(t34a + t37a);
    t35a = clip(t35 + t36);
    t36a = clip(t35 - t36);
    t37 = clip(t34a - t37a);
    t38a = clip(t33 - t38);
    t39 = clip(t32a - t39a);
    t40 = clip(t47a - t40a);
    t41a = clip(t46 - t41);
    t42 = clip(t45a - t42a);
    t43a = clip(t44 - t43);
    t44a = clip(t44 + t43);
    t45 = clip(t45a + t42a);
    t46a = clip(t46 + t41);
    t47 = clip(t47a + t40a);
    t48 = clip(t48a + t55a);
    t49a = clip(t49 + t54);
    t50 = clip(t50a + t53a);
    t51a = clip(t51 + t52);
    t52a = clip(t51 - t52);
    t53 = clip(t50a - t53a);
    t54a = clip(t49 - t54);
    t55 = clip(t48a - t55a);
    t56 = clip(t63a - t56a);
    t57a = clip(t62 - t57);
    t58 = clip(t61a - t58a);
    t59a = clip(t60 - t59);
    t60a = clip(t60 + t59);
    t61 = clip(t61a + t58a);
    t62a = clip(t62 + t57);
    t63 = clip(t63a + t56a);

    t36 = (t36a * (4096 - 3784) + t59a * 1567 + 2048 >> 12) - t36a;
    t37a = (t37 * (4096 - 3784) + t58 * 1567 + 2048 >> 12) - t37;
    t38 = (t38a * (4096 - 3784) + t57a * 1567 + 2048 >> 12) - t38a;
    t39a = (t39 * (4096 - 3784) + t56 * 1567 + 2048 >> 12) - t39;
    t40a = (t40 * -1567 + t55 * (4096 - 3784) + 2048 >> 12) - t55;
    t41 = (t41a * -1567 + t54a * (4096 - 3784) + 2048 >> 12) - t54a;
    t42a = (t42 * -1567 + t53 * (4096 - 3784) + 2048 >> 12) - t53;
    t43 = (t43a * -1567 + t52a * (4096 - 3784) + 2048 >> 12) - t52a;
    t52 = (t43a * (4096 - 3784) + t52a * 1567 + 2048 >> 12) - t43a;
    t53a = (t42 * (4096 - 3784) + t53 * 1567 + 2048 >> 12) - t42;
    t54 = (t41a * (4096 - 3784) + t54a * 1567 + 2048 >> 12) - t41a;
    t55a = (t40 * (4096 - 3784) + t55 * 1567 + 2048 >> 12) - t40;
    t56a = (t39 * 1567 + t56 * (3784 - 4096) + 2048 >> 12) + t56;
    t57 = (t38a * 1567 + t57a * (3784 - 4096) + 2048 >> 12) + t57a;
    t58a = (t37 * 1567 + t58 * (3784 - 4096) + 2048 >> 12) + t58;
    t59 = (t36a * 1567 + t59a * (3784 - 4096) + 2048 >> 12) + t59a;

    t32a = clip(t32 + t47);
    t33 = clip(t33a + t46a);
    t34a = clip(t34 + t45);
    t35 = clip(t35a + t44a);
    t36a = clip(t36 + t43);
    t37 = clip(t37a + t42a);
    t38a = clip(t38 + t41);
    t39 = clip(t39a + t40a);
    t40 = clip(t39a - t40a);
    t41a = clip(t38 - t41);
    t42 = clip(t37a - t42a);
    t43a = clip(t36 - t43);
    t44 = clip(t35a - t44a);
    t45a = clip(t34 - t45);
    t46 = clip(t33a - t46a);
    t47a = clip(t32 - t47);
    t48a = clip(t63 - t48);
    t49 = clip(t62a - t49a);
    t50a = clip(t61 - t50);
    t51 = clip(t60a - t51a);
    t52a = clip(t59 - t52);
    t53 = clip(t58a - t53a);
    t54a = clip(t57 - t54);
    t55 = clip(t56a - t55a);
    t56 = clip(t56a + t55a);
    t57a = clip(t57 + t54);
    t58 = clip(t58a + t53a);
    t59a = clip(t59 + t52);
    t60 = clip(t60a + t51a);
    t61a = clip(t61 + t50);
    t62 = clip(t62a + t49a);
    t63a = clip(t63 + t48);

    t40a = (t55 - t40) * 181 + 128 >> 8;
    t41 = (t54a - t41a) * 181 + 128 >> 8;
    t42a = (t53 - t42) * 181 + 128 >> 8;
    t43 = (t52a - t43a) * 181 + 128 >> 8;
    t44a = (t51 - t44) * 181 + 128 >> 8;
    t45 = (t50a - t45a) * 181 + 128 >> 8;
    t46a = (t49 - t46) * 181 + 128 >> 8;
    t47 = (t48a - t47a) * 181 + 128 >> 8;
    t48 = (t47a + t48a) * 181 + 128 >> 8;
    t49a = (t46 + t49) * 181 + 128 >> 8;
    t50 = (t45a + t50a) * 181 + 128 >> 8;
    t51a = (t44 + t51) * 181 + 128 >> 8;
    t52 = (t43a + t52a) * 181 + 128 >> 8;
    t53a = (t42 + t53) * 181 + 128 >> 8;
    t54 = (t41a + t54a) * 181 + 128 >> 8;
    t55a = (t40 + t55) * 181 + 128 >> 8;

    let t0 = c[0];
    let t1 = c[2];
    let t2 = c[4];
    let t3 = c[6];
    let t4 = c[8];
    let t5 = c[10];
    let t6 = c[12];
    let t7 = c[14];
    let t8 = c[16];
    let t9 = c[18];
    let t10 = c[20];
    let t11 = c[22];
    let t12 = c[24];
    let t13 = c[26];
    let t14 = c[28];
    let t15 = c[30];
    let t16 = c[32];
    let t17 = c[34];
    let t18 = c[36];
    let t19 = c[38];
    let t20 = c[40];
    let t21 = c[42];
    let t22 = c[44];
    let t23 = c[46];
    let t24 = c[48];
    let t25 = c[50];
    let t26 = c[52];
    let t27 = c[54];
    let t28 = c[56];
    let t29 = c[58];
    let t30 = c[60];
    let t31 = c[62];

    c[0] = clip(t0 + t63a);
    c[1] = clip(t1 + t62);
    c[2] = clip(t2 + t61a);
    c[3] = clip(t3 + t60);
    c[4] = clip(t4 + t59a);
    c[5] = clip(t5 + t58);
    c[6] = clip(t6 + t57a);
    c[7] = clip(t7 + t56);
    c[8] = clip(t8 + t55a);
    c[9] = clip(t9 + t54);
    c[10] = clip(t10 + t53a);
    c[11] = clip(t11 + t52);
    c[12] = clip(t12 + t51a);
    c[13] = clip(t13 + t50);
    c[14] = clip(t14 + t49a);
    c[15] = clip(t15 + t48);
    c[16] = clip(t16 + t47);
    c[17] = clip(t17 + t46a);
    c[18] = clip(t18 + t45);
    c[19] = clip(t19 + t44a);
    c[20] = clip(t20 + t43);
    c[21] = clip(t21 + t42a);
    c[22] = clip(t22 + t41);
    c[23] = clip(t23 + t40a);
    c[24] = clip(t24 + t39);
    c[25] = clip(t25 + t38a);
    c[26] = clip(t26 + t37);
    c[27] = clip(t27 + t36a);
    c[28] = clip(t28 + t35);
    c[29] = clip(t29 + t34a);
    c[30] = clip(t30 + t33);
    c[31] = clip(t31 + t32a);
    c[32] = clip(t31 - t32a);
    c[33] = clip(t30 - t33);
    c[34] = clip(t29 - t34a);
    c[35] = clip(t28 - t35);
    c[36] = clip(t27 - t36a);
    c[37] = clip(t26 - t37);
    c[38] = clip(t25 - t38a);
    c[39] = clip(t24 - t39);
    c[40] = clip(t23 - t40a);
    c[41] = clip(t22 - t41);
    c[42] = clip(t21 - t42a);
    c[43] = clip(t20 - t43);
    c[44] = clip(t19 - t44a);
    c[45] = clip(t18 - t45);
    c[46] = clip(t17 - t46a);
    c[47] = clip(t16 - t47);
    c[48] = clip(t15 - t48);
    c[49] = clip(t14 - t49a);
    c[50] = clip(t13 - t50);
    c[51] = clip(t12 - t51a);
    c[52] = clip(t11 - t52);
    c[53] = clip(t10 - t53a);
    c[54] = clip(t9 - t54);
    c[55] = clip(t8 - t55a);
    c[56] = clip(t7 - t56);
    c[57] = clip(t6 - t57a);
    c[58] = clip(t5 - t58);
    c[59] = clip(t4 - t59a);
    c[60] = clip(t3 - t60);
    c[61] = clip(t2 - t61a);
    c[62] = clip(t1 - t62);
    c[63] = clip(t0 - t63a);

    c
}

fn inverse_identity16(input: [i32; 16]) -> [i32; 16] {
    input.map(|value| {
        value
            .wrapping_mul(2)
            .wrapping_add(value.wrapping_mul(1697).wrapping_add(1024) >> 11)
    })
}

fn inverse_identity4(input: [i32; 4]) -> [i32; 4] {
    input.map(|value| value.wrapping_add(value.wrapping_mul(1697).wrapping_add(2048) >> 12))
}

fn inverse_identity8(input: [i32; 8]) -> [i32; 8] {
    input.map(|value| value.wrapping_mul(2))
}

fn inverse_rectangular_16x4(
    coefficients: &[i32; 64],
    horizontal: fn([i32; 16]) -> [i32; 16],
    vertical: fn([i32; 4]) -> [i32; 4],
) -> [i32; 64] {
    // R16x4 is not one of AV1's 1:2 rectangular transforms, so its
    // coefficients are not pre-scaled by inverse-sqrt(2). The first pass is
    // clipped and shifted by one bit; the second pass uses the usual final
    // four-bit output shift.
    let mut rows = [0_i32; 64];
    for row in 0_usize..4 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(4))]
        });
        let output = horizontal(input);
        let row_start = row.saturating_mul(16);
        let row_end = row_start.saturating_add(16);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(1) >> 1);
    }

    let mut output = [0_i32; 64];
    for column in 0_usize..16 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(16).saturating_add(column)]);
        let transformed = vertical(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(16).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

fn inverse_rectangular_16x8(
    coefficients: &[i32; 128],
    horizontal: fn([i32; 16]) -> [i32; 16],
    vertical: fn([i32; 8]) -> [i32; 8],
) -> [i32; 128] {
    // R16x8 is a 1:2 rectangular transform. AV1 applies the inverse-sqrt(2)
    // coefficient scale before the first pass, shifts the intermediate by
    // one bit, and applies the common four-bit output shift after the second
    // pass.
    let mut rows = [0_i32; 128];
    for row in 0_usize..8 {
        let input = std::array::from_fn(|column| {
            let coefficient = coefficients[row.saturating_add(column.saturating_mul(8))];
            coefficient.wrapping_mul(181).wrapping_add(128) >> 8
        });
        let transformed = horizontal(input);
        let row_start = row.saturating_mul(16);
        rows[row_start..row_start.saturating_add(16)].copy_from_slice(&transformed);
    }

    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(1) >> 1);
    }

    let mut output = [0_i32; 128];
    for column in 0_usize..16 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(16).saturating_add(column)]);
        let transformed = vertical(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(16).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 R16x8 identity-identity inverse transform.
pub(super) fn inverse_identity16x8(coefficients: &[i32; 128]) -> [i32; 128] {
    inverse_rectangular_16x8(coefficients, inverse_identity16, inverse_identity8)
}

/// Apply the AV1 R16x8 identity/DCT inverse transform.
pub(super) fn inverse_identity_dct16x8(coefficients: &[i32; 128]) -> [i32; 128] {
    inverse_rectangular_16x8(coefficients, inverse_identity16, inverse_dct8)
}

/// Apply the AV1 R16x8 DCT/identity inverse transform.
pub(super) fn inverse_dct_identity16x8(coefficients: &[i32; 128]) -> [i32; 128] {
    inverse_rectangular_16x8(coefficients, inverse_dct16, inverse_identity8)
}

/// Apply the AV1 R16x8 DCT-DCT inverse transform.
pub(super) fn inverse_dct16x8(coefficients: &[i32; 128]) -> [i32; 128] {
    inverse_rectangular_16x8(coefficients, inverse_dct16, inverse_dct8)
}

/// Apply the AV1 R16x8 DCT-ADST inverse transform.
pub(super) fn inverse_dct_adst16x8(coefficients: &[i32; 128]) -> [i32; 128] {
    inverse_rectangular_16x8(coefficients, inverse_adst16, inverse_dct8)
}

/// Apply the AV1 R16x8 ADST-DCT inverse transform.
pub(super) fn inverse_adst_dct16x8(coefficients: &[i32; 128]) -> [i32; 128] {
    inverse_rectangular_16x8(coefficients, inverse_dct16, inverse_adst8)
}

/// Apply the AV1 R16x8 ADST-ADST inverse transform.
pub(super) fn inverse_adst_adst16x8(coefficients: &[i32; 128]) -> [i32; 128] {
    inverse_rectangular_16x8(coefficients, inverse_adst16, inverse_adst8)
}

/// Apply the AV1 16×4 identity-identity inverse transform.
pub(super) fn inverse_identity16x4(coefficients: &[i32; 64]) -> [i32; 64] {
    inverse_rectangular_16x4(coefficients, inverse_identity16, inverse_identity4)
}

/// Apply the AV1 16×4 identity/DCT inverse transform.
pub(super) fn inverse_identity_dct16x4(coefficients: &[i32; 64]) -> [i32; 64] {
    inverse_rectangular_16x4(coefficients, inverse_identity16, inverse_dct4)
}

/// Apply the AV1 16×4 DCT/identity inverse transform.
pub(super) fn inverse_dct_identity16x4(coefficients: &[i32; 64]) -> [i32; 64] {
    inverse_rectangular_16x4(coefficients, inverse_dct16, inverse_identity4)
}

/// Apply the AV1 16×4 DCT-DCT inverse transform.
pub(super) fn inverse_dct16x4(coefficients: &[i32; 64]) -> [i32; 64] {
    inverse_rectangular_16x4(coefficients, inverse_dct16, inverse_dct4)
}

/// Apply the AV1 16×4 DCT-ADST inverse transform.
pub(super) fn inverse_dct_adst16x4(coefficients: &[i32; 64]) -> [i32; 64] {
    inverse_rectangular_16x4(coefficients, inverse_dct16, inverse_adst4)
}

/// Apply the AV1 16×4 ADST-DCT inverse transform.
pub(super) fn inverse_adst_dct16x4(coefficients: &[i32; 64]) -> [i32; 64] {
    inverse_rectangular_16x4(coefficients, inverse_adst16, inverse_dct4)
}

/// Apply the AV1 16×4 ADST-ADST inverse transform.
pub(super) fn inverse_adst_adst16x4(coefficients: &[i32; 64]) -> [i32; 64] {
    inverse_rectangular_16x4(coefficients, inverse_adst16, inverse_adst4)
}

/// Apply the AV1 16×16 identity inverse transform.
///
/// The two one-dimensional identity passes are separated by the 16×16
/// intermediate shift and use the same final four-bit output shift as the
/// other two-dimensional inverse transforms.
pub(super) fn inverse_identity16x16(coefficients: &[i32; 256]) -> [i32; 256] {
    let mut rows = [0_i32; 256];
    for row in 0_usize..16 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(16))]
        });
        let output = inverse_identity16(input);
        let row_start = row.saturating_mul(16);
        let row_end = row_start.saturating_add(16);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(2) >> 2);
    }

    let mut output = [0_i32; 256];
    for column in 0_usize..16 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(16).saturating_add(column)]);
        let transformed = inverse_identity16(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(16).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 16×16 DCT-DCT inverse transform.
pub(super) fn inverse_dct16x16(coefficients: &[i32; 256]) -> [i32; 256] {
    let mut rows = [0_i32; 256];
    for row in 0_usize..16 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(16))]
        });
        let transformed = inverse_dct16(input);
        let row_start = row.saturating_mul(16);
        rows[row_start..row_start.saturating_add(16)].copy_from_slice(&transformed);
    }
    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(2) >> 2);
    }

    let mut output = [0_i32; 256];
    for column in 0_usize..16 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(16).saturating_add(column)]);
        let transformed = inverse_dct16(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(16).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

fn inverse_square16x16(
    coefficients: &[i32; 256],
    horizontal: fn([i32; 16]) -> [i32; 16],
    vertical: fn([i32; 16]) -> [i32; 16],
) -> [i32; 256] {
    let mut rows = [0_i32; 256];
    for row in 0_usize..16 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(16))]
        });
        let transformed = horizontal(input);
        let row_start = row.saturating_mul(16);
        rows[row_start..row_start.saturating_add(16)].copy_from_slice(&transformed);
    }
    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(2) >> 2);
    }

    let mut output = [0_i32; 256];
    for column in 0_usize..16 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(16).saturating_add(column)]);
        let transformed = vertical(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(16).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 16×16 ADST-ADST inverse transform.
pub(super) fn inverse_adst_adst16x16(coefficients: &[i32; 256]) -> [i32; 256] {
    inverse_square16x16(coefficients, inverse_adst16, inverse_adst16)
}

/// Apply the AV1 16×16 ADST-DCT inverse transform.
pub(super) fn inverse_adst_dct16x16(coefficients: &[i32; 256]) -> [i32; 256] {
    inverse_square16x16(coefficients, inverse_adst16, inverse_dct16)
}

/// Apply the AV1 16×16 DCT-ADST inverse transform.
pub(super) fn inverse_dct_adst16x16(coefficients: &[i32; 256]) -> [i32; 256] {
    inverse_square16x16(coefficients, inverse_dct16, inverse_adst16)
}

/// Apply the AV1 R8x32 DCT-DCT inverse transform.
pub(super) fn inverse_dct8x32(coefficients: &[i32; 256]) -> [i32; 256] {
    let mut rows = [0_i32; 256];
    for row in 0_usize..32 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(32))]
        });
        let transformed = inverse_dct8(input);
        let row_start = row.saturating_mul(8);
        rows[row_start..row_start.saturating_add(8)].copy_from_slice(&transformed);
    }
    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(2) >> 2);
    }

    let mut output = [0_i32; 256];
    for column in 0_usize..8 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(8).saturating_add(column)]);
        let transformed = inverse_dct32(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(8).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 32×32 DCT-DCT inverse transform.
pub(super) fn inverse_dct32x32(coefficients: &[i32; 1024]) -> [i32; 1024] {
    let mut rows = [0_i32; 1024];
    for row in 0_usize..32 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(32))]
        });
        let transformed = inverse_dct32(input);
        let row_start = row.saturating_mul(32);
        rows[row_start..row_start.saturating_add(32)].copy_from_slice(&transformed);
    }
    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(2) >> 2);
    }

    let mut output = [0_i32; 1024];
    for column in 0_usize..32 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(32).saturating_add(column)]);
        let transformed = inverse_dct32(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(32).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 S64x64 DCT-DCT inverse transform.
///
/// AV1 stores only a 32×32 coefficient window for a 64×64 transform. The
/// missing high-frequency half is implicitly zero, while both one-dimensional
/// passes still run at 64 points. This mirrors the safe scalar fallback's
/// `sh = sw = 32` coefficient window and its two-bit intermediate shift.
pub(super) fn inverse_dct64x64(coefficients: &[i32; 1024]) -> [i32; 4096] {
    let mut rows = [0_i32; 4096];
    for row in 0_usize..32 {
        let input = std::array::from_fn(|column| {
            if column < 32 {
                coefficients[row.saturating_add(column.saturating_mul(32))]
            } else {
                0
            }
        });
        let transformed = inverse_dct64(input);
        let row_start = row.saturating_mul(64);
        rows[row_start..row_start.saturating_add(64)].copy_from_slice(&transformed);
    }

    for value in &mut rows[..64 * 32] {
        *value = clamp_intermediate(value.wrapping_add(2) >> 2);
    }

    let mut output = [0_i32; 4096];
    for column in 0_usize..64 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(64).saturating_add(column)]);
        let transformed = inverse_dct64(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(64).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 R16x64 DCT-DCT inverse transform.
pub(super) fn inverse_dct16x64(coefficients: &[i32; 512]) -> [i32; 1024] {
    let mut rows = [0_i32; 1024];
    for row in 0_usize..32 {
        let input = std::array::from_fn(|column| {
            let coefficient = coefficients[row.saturating_add(column.saturating_mul(32))];
            coefficient.wrapping_mul(181).wrapping_add(128) >> 8
        });
        let transformed = inverse_dct16(input);
        let row_start = row.saturating_mul(16);
        rows[row_start..row_start.saturating_add(16)].copy_from_slice(&transformed);
    }

    for value in &mut rows[..16 * 32] {
        *value = clamp_intermediate(value.wrapping_add(2) >> 2);
    }

    let mut output = [0_i32; 1024];
    for column in 0_usize..16 {
        let input = std::array::from_fn(|row| {
            if row < 32 {
                rows[row.saturating_mul(16).saturating_add(column)]
            } else {
                0
            }
        });
        let transformed = inverse_dct64(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(16).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

fn inverse_rectangular_4x16(
    coefficients: &[i32; 64],
    horizontal: fn([i32; 4]) -> [i32; 4],
    vertical: fn([i32; 16]) -> [i32; 16],
) -> [i32; 64] {
    let mut rows = [0_i32; 64];
    for row in 0_usize..16 {
        let input = std::array::from_fn(|column| {
            // R4x16 applies the inverse-sqrt(2) normalization through the
            // four-point inverse transform itself. Unlike the other
            // rectangular families, its first pass does not pre-scale the
            // coefficient before calling the 4-point kernel.
            coefficients[row.saturating_add(column.saturating_mul(16))]
        });
        let transformed = horizontal(input);
        let row_start = row.saturating_mul(4);
        rows[row_start..row_start.saturating_add(4)].copy_from_slice(&transformed);
    }
    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(1) >> 1);
    }

    let mut output = [0_i32; 64];
    for column in 0_usize..4 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(4).saturating_add(column)]);
        let transformed = vertical(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(4).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 R4x16 identity-identity inverse transform.
pub(super) fn inverse_identity4x16(coefficients: &[i32; 64]) -> [i32; 64] {
    inverse_rectangular_4x16(coefficients, inverse_identity4, inverse_identity16)
}

/// Apply the AV1 R4x16 identity/DCT inverse transform.
pub(super) fn inverse_identity_dct4x16(coefficients: &[i32; 64]) -> [i32; 64] {
    inverse_rectangular_4x16(coefficients, inverse_identity4, inverse_dct16)
}

/// Apply the AV1 R4x16 DCT/identity inverse transform.
pub(super) fn inverse_dct_identity4x16(coefficients: &[i32; 64]) -> [i32; 64] {
    inverse_rectangular_4x16(coefficients, inverse_dct4, inverse_identity16)
}

/// Apply the AV1 R4x16 DCT-DCT inverse transform.
pub(super) fn inverse_dct4x16(coefficients: &[i32; 64]) -> [i32; 64] {
    inverse_rectangular_4x16(coefficients, inverse_dct4, inverse_dct16)
}

/// Apply the AV1 R4x16 DCT-ADST inverse transform.
pub(super) fn inverse_dct_adst4x16(coefficients: &[i32; 64]) -> [i32; 64] {
    inverse_rectangular_4x16(coefficients, inverse_dct4, inverse_adst16)
}

/// Apply the AV1 R4x16 ADST-DCT inverse transform.
pub(super) fn inverse_adst_dct4x16(coefficients: &[i32; 64]) -> [i32; 64] {
    inverse_rectangular_4x16(coefficients, inverse_adst4, inverse_dct16)
}

/// Apply the AV1 R4x16 ADST-ADST inverse transform.
pub(super) fn inverse_adst_adst4x16(coefficients: &[i32; 64]) -> [i32; 64] {
    inverse_rectangular_4x16(coefficients, inverse_adst4, inverse_adst16)
}

fn inverse_rectangular_8x16(
    coefficients: &[i32; 128],
    horizontal: fn([i32; 8]) -> [i32; 8],
    vertical: fn([i32; 16]) -> [i32; 16],
) -> [i32; 128] {
    const WIDTH: usize = 8;
    const HEIGHT: usize = 16;

    let mut rows = [0_i32; WIDTH * HEIGHT];
    for row in 0..HEIGHT {
        let input = std::array::from_fn(|column| {
            let coefficient = coefficients[row.saturating_add(column.saturating_mul(HEIGHT))];
            coefficient.wrapping_mul(181).wrapping_add(128) >> 8
        });
        let output = horizontal(input);
        let row_start = row.saturating_mul(WIDTH);
        let row_end = row_start.saturating_add(WIDTH);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    // For an 8×16 rectangle the first-pass result is clipped and shifted by
    // one bit before the 16-point vertical pass.
    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(1) >> 1);
    }

    let mut output = [0_i32; WIDTH * HEIGHT];
    for column in 0..WIDTH {
        let input =
            std::array::from_fn(|row| rows[row.saturating_mul(WIDTH).saturating_add(column)]);
        let transformed = vertical(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(WIDTH).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

#[allow(
    dead_code,
    reason = "the safe R32x16 primitive is test-backed before walker integration"
)]
fn inverse_rectangular_32x16(
    coefficients: &[i32; 512],
    horizontal: fn([i32; 32]) -> [i32; 32],
    vertical: fn([i32; 16]) -> [i32; 16],
) -> [i32; 512] {
    const WIDTH: usize = 32;
    const HEIGHT: usize = 16;

    let mut rows = [0_i32; WIDTH * HEIGHT];
    for row in 0..HEIGHT {
        let input = std::array::from_fn(|column| {
            let coefficient = coefficients[row.saturating_add(column.saturating_mul(HEIGHT))];
            coefficient.wrapping_mul(181).wrapping_add(128) >> 8
        });
        let output = horizontal(input);
        let row_start = row.saturating_mul(WIDTH);
        let row_end = row_start.saturating_add(WIDTH);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    // R32x16 uses AV1's one-bit rectangular intermediate shift with the
    // reference rounding bias `rnd = (1 << 1) >> 1`.
    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(1) >> 1);
    }

    let mut output = [0_i32; WIDTH * HEIGHT];
    for column in 0..WIDTH {
        let input =
            std::array::from_fn(|row| rows[row.saturating_mul(WIDTH).saturating_add(column)]);
        let transformed = vertical(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(WIDTH).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 R8x16 DCT/identity inverse transform.
pub(super) fn inverse_dct_identity8x16(coefficients: &[i32; 128]) -> [i32; 128] {
    inverse_rectangular_8x16(coefficients, inverse_dct8, inverse_identity16)
}

/// Apply the AV1 R8x16 identity/identity inverse transform.
pub(super) fn inverse_identity8x16(coefficients: &[i32; 128]) -> [i32; 128] {
    inverse_rectangular_8x16(coefficients, inverse_identity8, inverse_identity16)
}

/// Apply the AV1 R8x16 identity/DCT inverse transform.
pub(super) fn inverse_identity_dct8x16(coefficients: &[i32; 128]) -> [i32; 128] {
    inverse_rectangular_8x16(coefficients, inverse_identity8, inverse_dct16)
}

/// Apply the AV1 R8x16 DCT-DCT inverse transform.
pub(super) fn inverse_dct8x16(coefficients: &[i32; 128]) -> [i32; 128] {
    inverse_rectangular_8x16(coefficients, inverse_dct8, inverse_dct16)
}

/// Apply the AV1 R8x16 DCT-ADST inverse transform.
pub(super) fn inverse_dct_adst8x16(coefficients: &[i32; 128]) -> [i32; 128] {
    inverse_rectangular_8x16(coefficients, inverse_dct8, inverse_adst16)
}

/// Apply the AV1 R8x16 ADST-DCT inverse transform.
pub(super) fn inverse_adst_dct8x16(coefficients: &[i32; 128]) -> [i32; 128] {
    inverse_rectangular_8x16(coefficients, inverse_adst8, inverse_dct16)
}

/// Apply the AV1 R8x16 ADST-ADST inverse transform.
pub(super) fn inverse_adst_adst8x16(coefficients: &[i32; 128]) -> [i32; 128] {
    inverse_rectangular_8x16(coefficients, inverse_adst8, inverse_adst16)
}

/// Apply the AV1 R32x16 DCT-DCT inverse transform.
#[allow(
    dead_code,
    reason = "the safe R32x16 primitive is test-backed before walker integration"
)]
pub(super) fn inverse_dct32x16(coefficients: &[i32; 512]) -> [i32; 512] {
    inverse_rectangular_32x16(coefficients, inverse_dct32, inverse_dct16)
}

/// Apply the AV1 R16x32 DCT-DCT inverse transform.
fn inverse_rectangular_16x32(
    coefficients: &[i32; 512],
    horizontal: fn([i32; 16]) -> [i32; 16],
    vertical: fn([i32; 32]) -> [i32; 32],
) -> [i32; 512] {
    const WIDTH: usize = 16;
    const HEIGHT: usize = 32;

    let mut rows = [0_i32; WIDTH * HEIGHT];
    for row in 0..HEIGHT {
        let input = std::array::from_fn(|column| {
            let coefficient = coefficients[row.saturating_add(column.saturating_mul(HEIGHT))];
            coefficient.wrapping_mul(181).wrapping_add(128) >> 8
        });
        let transformed = horizontal(input);
        let row_start = row.saturating_mul(WIDTH);
        rows[row_start..row_start.saturating_add(WIDTH)].copy_from_slice(&transformed);
    }

    // Rectangular transforms apply the inverse-sqrt(2) scale between passes.
    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(1) >> 1);
    }

    let mut output = [0_i32; WIDTH * HEIGHT];
    for column in 0..WIDTH {
        let input =
            std::array::from_fn(|row| rows[row.saturating_mul(WIDTH).saturating_add(column)]);
        let transformed = vertical(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(WIDTH).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 R16x32 DCT-DCT inverse transform.
pub(super) fn inverse_dct16x32(coefficients: &[i32; 512]) -> [i32; 512] {
    inverse_rectangular_16x32(coefficients, inverse_dct16, inverse_dct32)
}

/// Apply the AV1 R16x32 ADST-DCT inverse transform.
pub(super) fn inverse_adst_dct16x32(coefficients: &[i32; 512]) -> [i32; 512] {
    inverse_rectangular_16x32(coefficients, inverse_adst16, inverse_dct32)
}

/// Apply the AV1 R32x8 DCT-DCT inverse transform.
fn inverse_rectangular_32x8(
    coefficients: &[i32; 256],
    horizontal: fn([i32; 32]) -> [i32; 32],
    vertical: fn([i32; 8]) -> [i32; 8],
) -> [i32; 256] {
    const WIDTH: usize = 32;
    const HEIGHT: usize = 8;

    // ✅ VERIFIED: dav1d 1.5.3 src/itx_tmpl.c:71-121 and the
    // `inv_txfm_fn32(R, 32, 8, 2)` declaration. R32x8 is a 1:4 rectangle,
    // so it does not receive the inverse-sqrt(2) coefficient scale used by
    // R16x8. Its first-pass intermediate uses shift=2 before the final
    // four-bit output shift.
    let mut rows = [0_i32; WIDTH * HEIGHT];
    for row in 0..HEIGHT {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(HEIGHT))]
        });
        let transformed = horizontal(input);
        let row_start = row.saturating_mul(WIDTH);
        rows[row_start..row_start.saturating_add(WIDTH)].copy_from_slice(&transformed);
    }

    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(2) >> 2);
    }

    let mut output = [0_i32; WIDTH * HEIGHT];
    for column in 0..WIDTH {
        let input =
            std::array::from_fn(|row| rows[row.saturating_mul(WIDTH).saturating_add(column)]);
        let transformed = vertical(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(WIDTH).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

pub(super) fn inverse_dct32x8(coefficients: &[i32; 256]) -> [i32; 256] {
    inverse_rectangular_32x8(coefficients, inverse_dct32, inverse_dct8)
}

/// Apply the AV1 R32x8 DCT-ADST inverse transform.
pub(super) fn inverse_dct_adst32x8(coefficients: &[i32; 256]) -> [i32; 256] {
    inverse_rectangular_32x8(coefficients, inverse_dct32, inverse_adst8)
}

/// Apply the AV1 R64x16 DCT-DCT inverse transform.
///
/// AV1 stores only the 32×16 low-frequency coefficient window for this
/// transform. The horizontal 64-point pass therefore receives zeroes for
/// columns 32 through 63, while the vertical pass remains a full 16-point
/// transform. This 4:1 rectangle does not use the inverse-square-root-two
/// coefficient scaling used by the 2:1 rectangular transforms.
pub(super) fn inverse_dct64x16(coefficients: &[i32; 512]) -> [i32; 1024] {
    const WIDTH: usize = 64;
    const HEIGHT: usize = 16;
    const COEFFICIENT_WIDTH: usize = 32;

    let mut rows = [0_i32; WIDTH * HEIGHT];
    for row in 0..HEIGHT {
        let input = std::array::from_fn(|column| {
            if column < COEFFICIENT_WIDTH {
                coefficients[row.saturating_add(column.saturating_mul(HEIGHT))]
            } else {
                0
            }
        });
        let transformed = inverse_dct64(input);
        let row_start = row.saturating_mul(WIDTH);
        rows[row_start..row_start.saturating_add(WIDTH)].copy_from_slice(&transformed);
    }

    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(2) >> 2);
    }

    let mut output = [0_i32; WIDTH * HEIGHT];
    for column in 0..WIDTH {
        let input =
            std::array::from_fn(|row| rows[row.saturating_mul(WIDTH).saturating_add(column)]);
        let transformed = inverse_dct16(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(WIDTH).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

// ✅ VERIFIED: rav1d 1.1.0 src/itx_1d.rs:830-873. These are the scalar AV1
// inverse ADST kernels transcribed into total, allocation-free Rust. The
// inputs reaching this module are already bounded by the coefficient decoder;
// wrapping products keep the helper total even when unit tests exercise the
// public(crate) transform boundary with arbitrary i32 values.
fn inverse_adst4(input: [i32; 4]) -> [i32; 4] {
    let [input0, input1, input2, input3] = input;
    [
        rounded_dot(
            &[
                (1321, input0),
                (-293, input2),
                (-1614, input3),
                (-752, input1),
            ],
            12,
        )
        .wrapping_add(input2)
        .wrapping_add(input3)
        .wrapping_add(input1),
        rounded_dot(
            &[
                (-1614, input0),
                (-1321, input2),
                (293, input3),
                (-752, input1),
            ],
            12,
        )
        .wrapping_add(input0)
        .wrapping_sub(input3)
        .wrapping_add(input1),
        rounded_dot(&[(209, input0), (-209, input2), (209, input3)], 8),
        rounded_dot(
            &[
                (-293, input0),
                (-1614, input2),
                (-1321, input3),
                (752, input1),
            ],
            12,
        )
        .wrapping_add(input0)
        .wrapping_add(input2)
        .wrapping_sub(input1),
    ]
}

fn inverse_adst8(input: [i32; 8]) -> [i32; 8] {
    let [
        input0,
        input1,
        input2,
        input3,
        input4,
        input5,
        input6,
        input7,
    ] = input;
    let t0a = rounded_dot(&[(-20, input7), (401, input0)], 12).wrapping_add(input7);
    let t1a = rounded_dot(&[(401, input7), (20, input0)], 12).wrapping_sub(input0);
    let t2a = rounded_dot(&[(-484, input5), (1931, input2)], 12).wrapping_add(input5);
    let t3a = rounded_dot(&[(1931, input5), (484, input2)], 12).wrapping_sub(input2);
    let mut t4a = rounded_dot(&[(1299, input3), (1583, input4)], 11);
    let mut t5a = rounded_dot(&[(1583, input3), (-1299, input4)], 11);
    let mut t6a = rounded_dot(&[(1189, input1), (-176, input6)], 12).wrapping_add(input6);
    let mut t7a = rounded_dot(&[(-176, input1), (-1189, input6)], 12).wrapping_add(input1);

    let t0 = clamp_intermediate(t0a.wrapping_add(t4a));
    let t1 = clamp_intermediate(t1a.wrapping_add(t5a));
    let mut t2 = clamp_intermediate(t2a.wrapping_add(t6a));
    let mut t3 = clamp_intermediate(t3a.wrapping_add(t7a));
    let t4 = clamp_intermediate(t0a.wrapping_sub(t4a));
    let t5 = clamp_intermediate(t1a.wrapping_sub(t5a));
    let mut t6 = clamp_intermediate(t2a.wrapping_sub(t6a));
    let mut t7 = clamp_intermediate(t3a.wrapping_sub(t7a));

    t4a = rounded_dot(&[(-312, t4), (1567, t5)], 12).wrapping_add(t4);
    t5a = rounded_dot(&[(1567, t4), (312, t5)], 12).wrapping_sub(t5);
    t6a = rounded_dot(&[(-312, t7), (-1567, t6)], 12).wrapping_add(t7);
    t7a = rounded_dot(&[(1567, t7), (-312, t6)], 12).wrapping_add(t6);

    let output0 = clamp_intermediate(t0.wrapping_add(t2));
    let output7 = clamp_intermediate(t1.wrapping_add(t3)).wrapping_neg();
    t2 = clamp_intermediate(t0.wrapping_sub(t2));
    t3 = clamp_intermediate(t1.wrapping_sub(t3));
    let output1 = clamp_intermediate(t4a.wrapping_add(t6a)).wrapping_neg();
    let output6 = clamp_intermediate(t5a.wrapping_add(t7a));
    t6 = clamp_intermediate(t4a.wrapping_sub(t6a));
    t7 = clamp_intermediate(t5a.wrapping_sub(t7a));

    let output3 = rounded_dot(&[(181, t2), (181, t3)], 8).wrapping_neg();
    let output4 = rounded_dot(&[(181, t2), (-181, t3)], 8);
    let output2 = rounded_dot(&[(181, t6), (181, t7)], 8);
    let output5 = rounded_dot(&[(181, t6), (-181, t7)], 8).wrapping_neg();
    [
        output0, output1, output2, output3, output4, output5, output6, output7,
    ]
}

// ✅ VERIFIED: rav1d 1.1.0 src/itx_1d.rs:943-1085. The checked scalar
// transcription keeps the reference's clipping points and fixed-point
// rounding while using fixed-size arrays and wrapping arithmetic for totality.
fn inverse_adst16(input: [i32; 16]) -> [i32; 16] {
    let [
        in0,
        in1,
        in2,
        in3,
        in4,
        in5,
        in6,
        in7,
        in8,
        in9,
        in10,
        in11,
        in12,
        in13,
        in14,
        in15,
    ] = input;

    let mut t0 = rounded_dot(&[(4091 - 4096, in15), (201, in0)], 12).wrapping_add(in15);
    let mut t1 = rounded_dot(&[(201, in15), (-(4091 - 4096), in0)], 12).wrapping_sub(in0);
    let mut t2 = rounded_dot(&[(3973 - 4096, in13), (995, in2)], 12).wrapping_add(in13);
    let mut t3 = rounded_dot(&[(995, in13), (-(3973 - 4096), in2)], 12).wrapping_sub(in2);
    let mut t4 = rounded_dot(&[(3703 - 4096, in11), (1751, in4)], 12).wrapping_add(in11);
    let mut t5 = rounded_dot(&[(1751, in11), (-(3703 - 4096), in4)], 12).wrapping_sub(in4);
    let mut t6 = rounded_dot(&[(1645, in9), (1220, in6)], 11);
    let mut t7 = rounded_dot(&[(1220, in9), (-1645, in6)], 11);
    let mut t8 = rounded_dot(&[(2751, in7), (3035 - 4096, in8)], 12).wrapping_add(in8);
    let mut t9 = rounded_dot(&[(3035 - 4096, in7), (-2751, in8)], 12).wrapping_add(in7);
    let mut t10 = rounded_dot(&[(2106, in5), (3513 - 4096, in10)], 12).wrapping_add(in10);
    let mut t11 = rounded_dot(&[(3513 - 4096, in5), (-2106, in10)], 12).wrapping_add(in5);
    let mut t12 = rounded_dot(&[(1380, in3), (3857 - 4096, in12)], 12).wrapping_add(in12);
    let mut t13 = rounded_dot(&[(3857 - 4096, in3), (-1380, in12)], 12).wrapping_add(in3);
    let mut t14 = rounded_dot(&[(601, in1), (4052 - 4096, in14)], 12).wrapping_add(in14);
    let mut t15 = rounded_dot(&[(4052 - 4096, in1), (-601, in14)], 12).wrapping_add(in1);

    let t0a = clamp_intermediate(t0.wrapping_add(t8));
    let t1a = clamp_intermediate(t1.wrapping_add(t9));
    let mut t2a = clamp_intermediate(t2.wrapping_add(t10));
    let mut t3a = clamp_intermediate(t3.wrapping_add(t11));
    let mut t4a = clamp_intermediate(t4.wrapping_add(t12));
    let mut t5a = clamp_intermediate(t5.wrapping_add(t13));
    let mut t6a = clamp_intermediate(t6.wrapping_add(t14));
    let mut t7a = clamp_intermediate(t7.wrapping_add(t15));
    let mut t8a = clamp_intermediate(t0.wrapping_sub(t8));
    let mut t9a = clamp_intermediate(t1.wrapping_sub(t9));
    let mut t10a = clamp_intermediate(t2.wrapping_sub(t10));
    let mut t11a = clamp_intermediate(t3.wrapping_sub(t11));
    let mut t12a = clamp_intermediate(t4.wrapping_sub(t12));
    let mut t13a = clamp_intermediate(t5.wrapping_sub(t13));
    let mut t14a = clamp_intermediate(t6.wrapping_sub(t14));
    let mut t15a = clamp_intermediate(t7.wrapping_sub(t15));

    t8 = rounded_dot(&[(4017 - 4096, t8a), (799, t9a)], 12).wrapping_add(t8a);
    t9 = rounded_dot(&[(799, t8a), (-(4017 - 4096), t9a)], 12).wrapping_sub(t9a);
    t10 = rounded_dot(&[(2276, t10a), (3406 - 4096, t11a)], 12).wrapping_add(t11a);
    t11 = rounded_dot(&[(3406 - 4096, t10a), (-2276, t11a)], 12).wrapping_add(t10a);
    t12 = rounded_dot(&[(4017 - 4096, t13a), (-799, t12a)], 12).wrapping_add(t13a);
    t13 = rounded_dot(&[(799, t13a), (4017 - 4096, t12a)], 12).wrapping_add(t12a);
    t14 = rounded_dot(&[(2276, t15a), (-(3406 - 4096), t14a)], 12).wrapping_sub(t14a);
    t15 = rounded_dot(&[(3406 - 4096, t15a), (2276, t14a)], 12).wrapping_add(t15a);

    t0 = clamp_intermediate(t0a.wrapping_add(t4a));
    t1 = clamp_intermediate(t1a.wrapping_add(t5a));
    t2 = clamp_intermediate(t2a.wrapping_add(t6a));
    t3 = clamp_intermediate(t3a.wrapping_add(t7a));
    t4 = clamp_intermediate(t0a.wrapping_sub(t4a));
    t5 = clamp_intermediate(t1a.wrapping_sub(t5a));
    t6 = clamp_intermediate(t2a.wrapping_sub(t6a));
    t7 = clamp_intermediate(t3a.wrapping_sub(t7a));
    t8a = clamp_intermediate(t8.wrapping_add(t12));
    t9a = clamp_intermediate(t9.wrapping_add(t13));
    t10a = clamp_intermediate(t10.wrapping_add(t14));
    t11a = clamp_intermediate(t11.wrapping_add(t15));
    t12a = clamp_intermediate(t8.wrapping_sub(t12));
    t13a = clamp_intermediate(t9.wrapping_sub(t13));
    t14a = clamp_intermediate(t10.wrapping_sub(t14));
    t15a = clamp_intermediate(t11.wrapping_sub(t15));

    t4a = rounded_dot(&[(3784 - 4096, t4), (1567, t5)], 12).wrapping_add(t4);
    t5a = rounded_dot(&[(1567, t4), (-(3784 - 4096), t5)], 12).wrapping_sub(t5);
    t6a = rounded_dot(&[(3784 - 4096, t7), (-1567, t6)], 12).wrapping_add(t7);
    t7a = rounded_dot(&[(1567, t7), (3784 - 4096, t6)], 12).wrapping_add(t6);
    t12 = rounded_dot(&[(3784 - 4096, t12a), (1567, t13a)], 12).wrapping_add(t12a);
    t13 = rounded_dot(&[(1567, t12a), (-(3784 - 4096), t13a)], 12).wrapping_sub(t13a);
    t14 = rounded_dot(&[(3784 - 4096, t15a), (-1567, t14a)], 12).wrapping_add(t15a);
    t15 = rounded_dot(&[(1567, t15a), (3784 - 4096, t14a)], 12).wrapping_add(t14a);

    let output0 = clamp_intermediate(t0.wrapping_add(t2));
    let output15 = clamp_intermediate(t1.wrapping_add(t3)).wrapping_neg();
    t2a = clamp_intermediate(t0.wrapping_sub(t2));
    t3a = clamp_intermediate(t1.wrapping_sub(t3));
    let output3 = clamp_intermediate(t4a.wrapping_add(t6a)).wrapping_neg();
    let output12 = clamp_intermediate(t5a.wrapping_add(t7a));
    t6 = clamp_intermediate(t4a.wrapping_sub(t6a));
    t7 = clamp_intermediate(t5a.wrapping_sub(t7a));
    let output1 = clamp_intermediate(t8a.wrapping_add(t10a)).wrapping_neg();
    let output14 = clamp_intermediate(t9a.wrapping_add(t11a));
    t10 = clamp_intermediate(t8a.wrapping_sub(t10a));
    t11 = clamp_intermediate(t9a.wrapping_sub(t11a));
    let output2 = clamp_intermediate(t12.wrapping_add(t14));
    let output13 = clamp_intermediate(t13.wrapping_add(t15)).wrapping_neg();
    t14a = clamp_intermediate(t12.wrapping_sub(t14));
    t15a = clamp_intermediate(t13.wrapping_sub(t15));

    [
        output0,
        output1,
        output2,
        output3,
        rounded_dot(&[(181, t6), (181, t7)], 8),
        rounded_dot(&[(181, t14a), (181, t15a)], 8).wrapping_neg(),
        rounded_dot(&[(181, t10), (181, t11)], 8),
        rounded_dot(&[(181, t2a), (181, t3a)], 8).wrapping_neg(),
        rounded_dot(&[(181, t2a), (-181, t3a)], 8),
        rounded_dot(&[(181, t10), (-181, t11)], 8).wrapping_neg(),
        rounded_dot(&[(181, t14a), (-181, t15a)], 8),
        rounded_dot(&[(181, t6), (-181, t7)], 8).wrapping_neg(),
        output12,
        output13,
        output14,
        output15,
    ]
}

fn inverse_rectangular_4x8(
    coefficients: &[i32; 32],
    horizontal: fn([i32; 4]) -> [i32; 4],
    vertical: fn([i32; 8]) -> [i32; 8],
) -> [i32; 32] {
    // AV1 applies the inverse square-root-of-two scale to coefficients of a
    // 1:2 rectangular transform before the first one-dimensional pass. The
    // R4X8 transform has no intermediate shift between its 4- and 8-point
    // passes; both passes use the ordinary final four-bit output shift.
    const INV_SQRT2: i32 = 2_896;
    const SQRT2_BITS: u32 = 12;
    let mut rows = [0_i32; 32];
    for row in 0_usize..8 {
        let input = std::array::from_fn(|column| {
            let coefficient = coefficients[row.saturating_add(column.saturating_mul(8))];
            coefficient
                .wrapping_mul(INV_SQRT2)
                .wrapping_add(1_i32 << (SQRT2_BITS - 1))
                >> SQRT2_BITS
        });
        let output = horizontal(input);
        let row_start = row.saturating_mul(4);
        let row_end = row_start.saturating_add(4);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    let mut output = [0_i32; 32];
    for column in 0_usize..4 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(4).saturating_add(column)]);
        let transformed = vertical(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(4).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

fn inverse_rectangular_8x4(
    coefficients: &[i32; 32],
    horizontal: fn([i32; 8]) -> [i32; 8],
    vertical: fn([i32; 4]) -> [i32; 4],
) -> [i32; 32] {
    // The R8X4 transform is the transposed 1:2 rectangular class. AV1
    // applies the same inverse-square-root-of-two coefficient scale before
    // the horizontal eight-point and vertical four-point passes.
    const INV_SQRT2: i32 = 2_896;
    const SQRT2_BITS: u32 = 12;
    let mut rows = [0_i32; 32];
    for row in 0_usize..4 {
        let input = std::array::from_fn(|column| {
            let coefficient = coefficients[row.saturating_add(column.saturating_mul(4))];
            coefficient
                .wrapping_mul(INV_SQRT2)
                .wrapping_add(1_i32 << (SQRT2_BITS - 1))
                >> SQRT2_BITS
        });
        let output = horizontal(input);
        let row_start = row.saturating_mul(8);
        let row_end = row_start.saturating_add(8);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    let mut output = [0_i32; 32];
    for column in 0_usize..8 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(8).saturating_add(column)]);
        let transformed = vertical(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(8).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 8×4 identity-identity inverse transform.
pub(super) fn inverse_identity8x4(coefficients: &[i32; 32]) -> [i32; 32] {
    const INV_SQRT2: i32 = 2_896;
    let mut output = [0_i32; 32];
    for row in 0_usize..4 {
        for column in 0_usize..8 {
            let coefficient = coefficients[row.saturating_add(column.saturating_mul(4))];
            let scaled = coefficient.wrapping_mul(INV_SQRT2).wrapping_add(2_048) >> 12;
            output[row.saturating_mul(8).saturating_add(column)] =
                (scaled.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 8×4 horizontal identity/vertical DCT inverse transform.
pub(super) fn inverse_identity_dct8x4(coefficients: &[i32; 32]) -> [i32; 32] {
    inverse_rectangular_8x4(coefficients, inverse_identity8, inverse_dct4)
}

/// Apply the AV1 8×4 horizontal DCT/vertical identity inverse transform.
pub(super) fn inverse_dct_identity8x4(coefficients: &[i32; 32]) -> [i32; 32] {
    inverse_rectangular_8x4(coefficients, inverse_dct8, inverse_identity4)
}

/// Apply the AV1 8×4 DCT-DCT inverse transform.
pub(super) fn inverse_dct8x4(coefficients: &[i32; 32]) -> [i32; 32] {
    inverse_rectangular_8x4(coefficients, inverse_dct8, inverse_dct4)
}

/// Apply the AV1 8×4 DCT-ADST inverse transform.
pub(super) fn inverse_dct_adst8x4(coefficients: &[i32; 32]) -> [i32; 32] {
    inverse_rectangular_8x4(coefficients, inverse_dct8, inverse_adst4)
}

/// Apply the AV1 8×4 ADST-DCT inverse transform.
pub(super) fn inverse_adst_dct8x4(coefficients: &[i32; 32]) -> [i32; 32] {
    inverse_rectangular_8x4(coefficients, inverse_adst8, inverse_dct4)
}

/// Apply the AV1 8×4 ADST-ADST inverse transform.
pub(super) fn inverse_adst_adst8x4(coefficients: &[i32; 32]) -> [i32; 32] {
    inverse_rectangular_8x4(coefficients, inverse_adst8, inverse_adst4)
}

/// Apply AV1's rectangular coefficient scale and leave both axes in the
/// identity domain.
pub(super) fn inverse_identity4x8(coefficients: &[i32; 32]) -> [i32; 32] {
    const INV_SQRT2: i32 = 2_896;
    let mut output = [0_i32; 32];
    for row in 0_usize..8 {
        for column in 0_usize..4 {
            let coefficient = coefficients[row.saturating_add(column.saturating_mul(8))];
            let scaled = coefficient.wrapping_mul(INV_SQRT2).wrapping_add(2_048) >> 12;
            output[row.saturating_mul(4).saturating_add(column)] =
                (scaled.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply AV1's rectangular coefficient scale, an identity horizontal pass,
/// and an eight-point vertical DCT.
pub(super) fn inverse_identity_dct4x8(coefficients: &[i32; 32]) -> [i32; 32] {
    const INV_SQRT2: i32 = 2_896;
    let mut rows = [0_i32; 32];
    for row in 0_usize..8 {
        for column in 0_usize..4 {
            let coefficient = coefficients[row.saturating_add(column.saturating_mul(8))];
            rows[row.saturating_mul(4).saturating_add(column)] =
                coefficient.wrapping_mul(INV_SQRT2).wrapping_add(2_048) >> 12;
        }
    }
    let mut output = [0_i32; 32];
    for column in 0_usize..4 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(4).saturating_add(column)]);
        let transformed = inverse_dct8(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(4).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 4×4 DCT-DCT inverse transform.
///
/// As with [`inverse_dct8x8`], coefficients use AV1's column-major order and
/// the result is row-major residual data after the scalar decoder's final
/// `>> 4` scaling step.  This is the transform used by a subsampled 4:2:0
/// chroma block inside the small lossy leaf class.
pub(super) fn inverse_dct4x4(coefficients: &[i32; 16]) -> [i32; 16] {
    let mut rows = [0_i32; 16];
    for row in 0_usize..4 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(4))]
        });
        let output = inverse_dct4(input);
        let row_start = row.saturating_mul(4);
        let row_end = row_start.saturating_add(4);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    let mut output = [0_i32; 16];
    for column in 0_usize..4 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(4).saturating_add(column)]);
        let transformed = inverse_dct4(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(4).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 4×4 identity/identity inverse transform.
pub(super) fn inverse_identity4x4(coefficients: &[i32; 16]) -> [i32; 16] {
    inverse_4x4_with_passes(coefficients, inverse_identity4, inverse_identity4)
}

/// Apply the AV1 4×4 horizontal-identity/vertical-DCT inverse transform.
pub(super) fn inverse_identity_dct4x4(coefficients: &[i32; 16]) -> [i32; 16] {
    inverse_4x4_with_passes(coefficients, inverse_identity4, inverse_dct4)
}

/// Apply the AV1 4×4 horizontal-DCT/vertical-identity inverse transform.
pub(super) fn inverse_dct_identity4x4(coefficients: &[i32; 16]) -> [i32; 16] {
    inverse_4x4_with_passes(coefficients, inverse_dct4, inverse_identity4)
}

fn inverse_4x4_with_passes(
    coefficients: &[i32; 16],
    horizontal: fn([i32; 4]) -> [i32; 4],
    vertical: fn([i32; 4]) -> [i32; 4],
) -> [i32; 16] {
    let mut rows = [0_i32; 16];
    for row in 0_usize..4 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(4))]
        });
        let output = horizontal(input);
        let row_start = row.saturating_mul(4);
        let row_end = row_start.saturating_add(4);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    let mut output = [0_i32; 16];
    for column in 0_usize..4 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(4).saturating_add(column)]);
        let transformed = vertical(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(4).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 4×8 DCT-DCT inverse transform.
pub(super) fn inverse_dct4x8(coefficients: &[i32; 32]) -> [i32; 32] {
    inverse_rectangular_4x8(coefficients, inverse_dct4, inverse_dct8)
}

/// Apply the AV1 4×8 DCT-ADST inverse transform.
pub(super) fn inverse_dct_adst4x8(coefficients: &[i32; 32]) -> [i32; 32] {
    inverse_rectangular_4x8(coefficients, inverse_adst4, inverse_dct8)
}

/// Apply the AV1 4×8 ADST-DCT inverse transform.
pub(super) fn inverse_adst_dct4x8(coefficients: &[i32; 32]) -> [i32; 32] {
    inverse_rectangular_4x8(coefficients, inverse_dct4, inverse_adst8)
}

/// Apply the AV1 4×8 ADST-ADST inverse transform.
pub(super) fn inverse_adst_adst4x8(coefficients: &[i32; 32]) -> [i32; 32] {
    inverse_rectangular_4x8(coefficients, inverse_adst4, inverse_adst8)
}

/// Apply the AV1 4×8 horizontal DCT/vertical identity inverse transform.
pub(super) fn inverse_dct_identity4x8(coefficients: &[i32; 32]) -> [i32; 32] {
    const INV_SQRT2: i32 = 2_896;
    let mut output = [0_i32; 32];
    for row in 0_usize..8 {
        let input = std::array::from_fn(|column| {
            let coefficient = coefficients[row.saturating_add(column.saturating_mul(8))];
            coefficient.wrapping_mul(INV_SQRT2).wrapping_add(2_048) >> 12
        });
        let transformed = inverse_dct4(input);
        for (column, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(4).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 8×8 DCT-DCT inverse transform.
///
/// Coefficients use AV1's column-major transform order (`y + 8 * x`).  The
/// returned samples are row-major residuals after the 8×8 transform's final
/// scaling step.  The function is total for every `i32` input: intermediate
/// products use wrapping arithmetic, while the codec's prescribed 16-bit
/// intermediate clamps prevent malformed input from affecting indexing or
/// allocation decisions.
pub(super) fn inverse_dct8x8(coefficients: &[i32; 64]) -> [i32; 64] {
    let mut rows = [0_i32; 64];
    for row in 0_usize..8 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(8))]
        });
        let output = inverse_dct8(input);
        let row_start = row.saturating_mul(8);
        let row_end = row_start.saturating_add(8);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    // An 8×8 transform uses a one-bit intermediate shift between the two
    // one-dimensional passes.  `INTERMEDIATE_BITS` is the AV1 8-bit
    // row/column clipping range used by the scalar decoder.
    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(1) >> 1);
    }

    let mut output = [0_i32; 64];
    for column in 0_usize..8 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(8).saturating_add(column)]);
        let transformed = inverse_dct8(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(8).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 8×8 identity/identity inverse transform.
pub(super) fn inverse_identity8x8(coefficients: &[i32; 64]) -> [i32; 64] {
    let mut output = [0_i32; 64];
    for row in 0_usize..8 {
        for column in 0_usize..8 {
            let coefficient = coefficients[row.saturating_add(column.saturating_mul(8))];
            output[row.saturating_mul(8).saturating_add(column)] =
                clamp_intermediate(coefficient.wrapping_mul(2));
        }
    }
    for value in &mut output {
        *value = clamp_intermediate(value.wrapping_add(1) >> 1);
        *value = value.wrapping_mul(2);
    }
    output.map(|value| (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX)))
}

/// Apply the AV1 8×8 vertical-DCT inverse transform.
pub(super) fn inverse_identity_dct8x8(coefficients: &[i32; 64]) -> [i32; 64] {
    let mut rows = [0_i32; 64];
    for row in 0_usize..8 {
        for column in 0_usize..8 {
            let coefficient = coefficients[row.saturating_add(column.saturating_mul(8))];
            rows[row.saturating_mul(8).saturating_add(column)] =
                clamp_intermediate(coefficient.wrapping_mul(2));
        }
    }
    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(1) >> 1);
    }

    let mut output = [0_i32; 64];
    for column in 0_usize..8 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(8).saturating_add(column)]);
        let transformed = inverse_dct8(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(8).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 4×4 DCT-ADST inverse transform.
///
/// AV1 names the horizontal ADST/vertical DCT combination `DCT_ADST`; this
/// ordering follows the reference transform dispatcher rather than the
/// visual direction implied by the name.
pub(super) fn inverse_dct_adst4x4(coefficients: &[i32; 16]) -> [i32; 16] {
    let mut rows = [0_i32; 16];
    for row in 0_usize..4 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(4))]
        });
        let output = inverse_adst4(input);
        let row_start = row.saturating_mul(4);
        let row_end = row_start.saturating_add(4);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    let mut output = [0_i32; 16];
    for column in 0_usize..4 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(4).saturating_add(column)]);
        let transformed = inverse_dct4(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(4).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 4×4 ADST-DCT inverse transform.
pub(super) fn inverse_adst_dct4x4(coefficients: &[i32; 16]) -> [i32; 16] {
    let mut rows = [0_i32; 16];
    for row in 0_usize..4 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(4))]
        });
        let output = inverse_dct4(input);
        let row_start = row.saturating_mul(4);
        let row_end = row_start.saturating_add(4);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    let mut output = [0_i32; 16];
    for column in 0_usize..4 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(4).saturating_add(column)]);
        let transformed = inverse_adst4(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(4).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 4×4 ADST-ADST inverse transform.
///
/// The two one-dimensional ADST passes use the same column-major input and
/// row-major output convention as the other scalar transform helpers.
pub(super) fn inverse_adst_adst4x4(coefficients: &[i32; 16]) -> [i32; 16] {
    let mut rows = [0_i32; 16];
    for row in 0_usize..4 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(4))]
        });
        let output = inverse_adst4(input);
        let row_start = row.saturating_mul(4);
        let row_end = row_start.saturating_add(4);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    let mut output = [0_i32; 16];
    for column in 0_usize..4 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(4).saturating_add(column)]);
        let transformed = inverse_adst4(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(4).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 8×8 DCT-ADST inverse transform.
///
/// Coefficients use AV1's column-major transform order and the result is
/// row-major residual data after the prescribed intermediate and final
/// scaling steps.
pub(super) fn inverse_dct_adst8x8(coefficients: &[i32; 64]) -> [i32; 64] {
    let mut rows = [0_i32; 64];
    for row in 0_usize..8 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(8))]
        });
        let output = inverse_adst8(input);
        let row_start = row.saturating_mul(8);
        let row_end = row_start.saturating_add(8);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(1) >> 1);
    }

    let mut output = [0_i32; 64];
    for column in 0_usize..8 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(8).saturating_add(column)]);
        let transformed = inverse_dct8(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(8).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 8×8 ADST-DCT inverse transform.
///
/// The AV1 name describes the vertical ADST and horizontal DCT combination;
/// the scalar pass order is therefore DCT across each coded row followed by
/// ADST down each column.
pub(super) fn inverse_adst_dct8x8(coefficients: &[i32; 64]) -> [i32; 64] {
    let mut rows = [0_i32; 64];
    for row in 0_usize..8 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(8))]
        });
        let output = inverse_dct8(input);
        let row_start = row.saturating_mul(8);
        let row_end = row_start.saturating_add(8);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(1) >> 1);
    }

    let mut output = [0_i32; 64];
    for column in 0_usize..8 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(8).saturating_add(column)]);
        let transformed = inverse_adst8(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(8).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

/// Apply the AV1 8×8 H-DCT inverse transform.
///
/// `H_DCT` is DCT across rows followed by the identity transform down
/// columns. It uses the same 8×8 intermediate shift and final scaling as the
/// two-dimensional transforms.
pub(super) fn inverse_dct_identity8x8(coefficients: &[i32; 64]) -> [i32; 64] {
    let mut rows = [0_i32; 64];
    for row in 0_usize..8 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(8))]
        });
        let output = inverse_dct8(input);
        let row_start = row.saturating_mul(8);
        let row_end = row_start.saturating_add(8);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(1) >> 1);
        *value = value.wrapping_mul(2);
    }

    rows.map(|value| (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX)))
}

/// Apply the AV1 8×8 ADST-ADST inverse transform.
pub(super) fn inverse_adst_adst8x8(coefficients: &[i32; 64]) -> [i32; 64] {
    let mut rows = [0_i32; 64];
    for row in 0_usize..8 {
        let input = std::array::from_fn(|column| {
            coefficients[row.saturating_add(column.saturating_mul(8))]
        });
        let output = inverse_adst8(input);
        let row_start = row.saturating_mul(8);
        let row_end = row_start.saturating_add(8);
        rows[row_start..row_end].copy_from_slice(&output);
    }

    for value in &mut rows {
        *value = clamp_intermediate(value.wrapping_add(1) >> 1);
    }

    let mut output = [0_i32; 64];
    for column in 0_usize..8 {
        let input = std::array::from_fn(|row| rows[row.saturating_mul(8).saturating_add(column)]);
        let transformed = inverse_adst8(input);
        for (row, value) in transformed.into_iter().enumerate() {
            output[row.saturating_mul(8).saturating_add(column)] =
                (value.wrapping_add(8) >> 4).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        }
    }
    output
}

#[cfg(any(test, coverage))]
fn assert_sparse<const N: usize>(actual: [i32; N], expected: &[(usize, i32)]) {
    let mut expected_output = [0_i32; N];
    for &(index, value) in expected {
        assert!(index < N);
        assert_eq!(expected_output[index], 0);
        expected_output[index] = value;
    }
    assert_eq!(actual, expected_output);
}

#[cfg(any(test, coverage))]
fn assert_i16_bounded<const N: usize>(output: [i32; N]) {
    assert!(
        output
            .iter()
            .all(|&sample| (i32::from(i16::MIN)..=i32::from(i16::MAX)).contains(&sample))
    );
}

#[cfg(any(test, coverage))]
#[cfg_attr(coverage, coverage(off))]
fn assert_rectangular_wrapper_conformance() {
    // These checks are shared by the normal unit suite and the managed
    // integration coverage hook. The expected values are checked-in output
    // from dav1d 1.5.3's scalar inv_txfm_add_c path (BITDEPTH=8,
    // HAVE_ASM=0), with the neutral destination bias removed.
    assert_eq!(inverse_identity16x8(&[0; 128]), [0; 128]);
    assert_eq!(inverse_identity_dct16x8(&[0; 128]), [0; 128]);
    assert_eq!(inverse_identity16x4(&[0; 64]), [0; 64]);
    assert_eq!(inverse_identity_dct16x4(&[0; 64]), [0; 64]);
    assert_eq!(inverse_dct_identity16x4(&[0; 64]), [0; 64]);
    assert_eq!(inverse_adst_adst16x4(&[0; 64]), [0; 64]);
    assert_eq!(inverse_adst_dct16x16(&[0; 256]), [0; 256]);

    let mut identity_r16x8 = [0_i32; 128];
    // Coefficients use y + x * height; output uses y * width + x.
    identity_r16x8[30] = 4_096; // (x, y) = (3, 6)
    assert_sparse(inverse_identity16x8(&identity_r16x8), &[(99, 512)]);

    let mut identity_r16x4 = [0_i32; 64];
    identity_r16x4[22] = -4_096; // (x, y) = (5, 2)
    assert_sparse(inverse_identity16x4(&identity_r16x4), &[(37, -512)]);

    let mut r16x8 = [0_i32; 128];
    r16x8[0] = 93;
    r16x8[8] = -112;
    r16x8[1] = 66;
    r16x8[46] = -79;
    assert_sparse(
        inverse_identity_dct16x8(&r16x8),
        &[
            (0, 8),
            (1, -5),
            (5, -2),
            (16, 8),
            (17, -5),
            (21, 5),
            (32, 6),
            (33, -5),
            (37, -5),
            (48, 5),
            (49, -5),
            (53, 2),
            (64, 3),
            (65, -5),
            (69, 2),
            (80, 2),
            (81, -5),
            (85, -5),
            (96, 1),
            (97, -5),
            (101, 5),
            (113, -5),
            (117, -2),
        ],
    );

    let mut r16x4 = [0_i32; 64];
    r16x4[0] = 93;
    r16x4[4] = -112;
    r16x4[1] = 66;
    r16x4[22] = -79;
    assert_sparse(
        inverse_identity_dct16x4(&r16x4),
        &[
            (0, 11),
            (1, -7),
            (5, -5),
            (16, 8),
            (17, -7),
            (21, 5),
            (32, 4),
            (33, -7),
            (37, 5),
            (49, -7),
            (53, -5),
        ],
    );
    let dct_identity = inverse_dct_identity16x4(&r16x4);
    assert_eq!(
        dct_identity.as_chunks::<16>().0,
        &[
            [-2, -2, -1, -1, 0, 1, 2, 3, 3, 4, 5, 6, 7, 7, 8, 8],
            [2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2],
            [-3, 0, 3, 3, 1, -2, -3, -2, 2, 4, 2, -1, -3, -3, 0, 3],
            [0; 16],
        ]
    );
    let adst_adst = inverse_adst_adst16x4(&r16x4);
    assert_eq!(
        adst_adst.as_chunks::<16>().0,
        &[
            [-1, -2, -1, 1, 2, 1, -1, -1, 1, 4, 4, 3, 1, 1, 4, 6],
            [0, 0, 0, -1, -1, -1, 1, 1, 2, 2, 2, 4, 5, 6, 5, 5],
            [1, 1, 0, -3, -4, -2, 0, 1, 1, -1, 0, 3, 5, 6, 5, 4],
            [-1, -3, -3, -1, -1, -2, -3, -3, -2, 1, 2, 2, 1, 2, 4, 6],
        ]
    );
    assert_ne!(inverse_identity_dct16x4(&r16x4), dct_identity);
    assert_ne!(inverse_identity_dct16x4(&r16x4), adst_adst);
    assert_ne!(dct_identity, adst_adst);

    let mut square = [0_i32; 256];
    square[0] = 93;
    square[16] = -112;
    square[1] = 66;
    square[35] = -79;
    assert_eq!(
        inverse_adst_dct16x16(&square).as_chunks::<16>().0,
        &[
            [0, -1, -1, -1, -1, -1, 0, 1, 2, 2, 3, 3, 3, 3, 2, 2],
            [0, -1, -1, -1, -1, 0, 0, 1, 1, 2, 2, 3, 3, 3, 3, 3],
            [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 2, 3, 3, 3],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 2, 3, 3, 4],
            [0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 4],
            [0, 1, 1, 1, 0, 0, 0, -1, -1, 0, 0, 1, 2, 3, 4, 4],
            [0, 0, 0, 0, 0, 0, 0, -1, 0, 0, 0, 1, 2, 2, 3, 3],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 3, 3],
            [0, -1, -1, -1, -1, -1, -1, 0, 0, 1, 1, 1, 2, 2, 2, 2],
            [0, -1, -2, -2, -2, -1, -1, 0, 1, 1, 1, 2, 1, 1, 1, 1],
            [0, -1, -2, -2, -2, -1, -1, 0, 1, 1, 2, 2, 1, 1, 1, 1],
            [0, -1, -2, -2, -2, -2, -1, 0, 1, 1, 1, 1, 1, 1, 1, 1],
            [0, -1, -1, -1, -2, -1, -1, -1, 0, 0, 1, 1, 1, 1, 1, 1],
            [0, 0, -1, -1, -1, -1, -1, -1, -1, 0, 0, 0, 1, 1, 1, 1],
            [0, 0, 0, 0, -1, -1, -1, -1, -1, -1, -1, 0, 0, 1, 2, 2],
            [0, 0, 0, 0, 0, -1, -1, -2, -2, -2, -1, 0, 0, 1, 2, 2],
        ]
    );

    assert_i16_bounded(inverse_identity16x8(&[i32::MAX; 128]));
    assert_i16_bounded(inverse_identity_dct16x8(&[i32::MAX; 128]));
    assert_i16_bounded(inverse_identity16x4(&[i32::MAX; 64]));
    assert_i16_bounded(inverse_identity_dct16x4(&[i32::MAX; 64]));
    assert_i16_bounded(inverse_dct_identity16x4(&[i32::MAX; 64]));
    assert_i16_bounded(inverse_adst_adst16x4(&[i32::MAX; 64]));
    assert_i16_bounded(inverse_adst_dct16x16(&[i32::MAX; 256]));
}

#[cfg(coverage)]
#[coverage(off)]
pub(super) fn __coverage_exercise_private_branches() {
    assert_rectangular_wrapper_conformance();
}

#[cfg(test)]
mod tests {
    use super::{
        assert_rectangular_wrapper_conformance, inverse_adst_adst4x4, inverse_adst_adst4x8,
        inverse_adst_adst8x8, inverse_adst_adst16x8, inverse_adst_dct4x4, inverse_adst_dct4x8,
        inverse_adst_dct4x16, inverse_adst_dct8x8, inverse_adst_dct16x16, inverse_adst16,
        inverse_dct_adst4x4, inverse_dct_adst4x8, inverse_dct_adst8x8, inverse_dct_adst8x16,
        inverse_dct_identity4x8, inverse_dct_identity8x8, inverse_dct_identity16x4, inverse_dct4x4,
        inverse_dct4x8, inverse_dct8x8, inverse_dct16x64, inverse_dct32x16, inverse_dct32x32,
        inverse_dct64x64, inverse_identity_dct16x4, inverse_identity_dct16x8, inverse_identity16x4,
        inverse_identity16x8,
    };

    fn assert_sparse<const N: usize>(actual: [i32; N], expected: &[(usize, i32)]) {
        let mut expected_output = [0_i32; N];
        for &(index, value) in expected {
            assert!(index < N);
            assert_eq!(expected_output[index], 0);
            expected_output[index] = value;
        }
        assert_eq!(actual, expected_output);
    }

    fn assert_i16_bounded<const N: usize>(output: [i32; N]) {
        assert!(
            output
                .iter()
                .all(|&sample| (i32::from(i16::MIN)..=i32::from(i16::MAX)).contains(&sample))
        );
    }

    #[test]
    fn dc_only_4x4_transform_is_constant() {
        let mut coefficients = [0_i32; 16];
        coefficients[0] = 4_096;
        let output = inverse_dct4x4(&coefficients);
        assert!(output.iter().all(|&sample| sample == 128));
    }

    #[test]
    fn odd_basis_4x4_has_expected_dct_symmetry() {
        let mut coefficients = [0_i32; 16];
        coefficients[1] = 2_048;
        let output = inverse_dct4x4(&coefficients);
        for column in 0..4 {
            assert_eq!(output[column], -output[12 + column]);
            assert_eq!(output[4 + column], -output[8 + column]);
        }
    }

    #[test]
    fn dc_only_transform_is_constant() {
        let mut coefficients = [0_i32; 64];
        coefficients[0] = 4_096;
        let output = inverse_dct8x8(&coefficients);
        assert!(output.iter().all(|&sample| sample == 64));
    }

    #[test]
    fn odd_basis_has_expected_dct_symmetry() {
        let mut coefficients = [0_i32; 64];
        coefficients[1] = 2_048;
        let output = inverse_dct8x8(&coefficients);
        for column in 0..8 {
            assert_eq!(output[column], -output[56 + column]);
            assert_eq!(output[8 + column], -output[48 + column]);
            assert_eq!(output[16 + column], -output[40 + column]);
            assert_eq!(output[24 + column], -output[32 + column]);
        }
    }

    #[test]
    fn extreme_coefficients_remain_bounded() {
        let coefficients = [i32::MAX; 64];
        let output = inverse_dct8x8(&coefficients);
        assert!(
            output
                .iter()
                .all(|&sample| (-32_768..=32_767).contains(&sample))
        );
    }

    #[test]
    fn large_square_dc_is_constant_and_bounded() {
        let mut coefficients = [0_i32; 1024];
        coefficients[0] = 4_096;
        let output = inverse_dct64x64(&coefficients);
        assert!(output.iter().all(|&sample| sample == output[0]));
        assert_ne!(output[0], 0);
        assert!(
            output
                .iter()
                .all(|&sample| (-32_768..=32_767).contains(&sample))
        );
    }

    #[test]
    fn rectangular_sixteen_by_sixty_four_dc_is_constant_and_bounded() {
        let mut coefficients = [0_i32; 512];
        coefficients[0] = 4_096;
        let output = inverse_dct16x64(&coefficients);
        assert!(output.iter().all(|&sample| sample == output[0]));
        assert_ne!(output[0], 0);
        assert!(
            output
                .iter()
                .all(|&sample| (-32_768..=32_767).contains(&sample))
        );
    }

    #[test]
    fn large_transforms_remain_bounded_for_extreme_coefficients() {
        let square = inverse_dct64x64(&[i32::MAX; 1024]);
        let rectangular = inverse_dct16x64(&[i32::MAX; 512]);
        assert!(
            square
                .iter()
                .all(|&sample| (-32_768..=32_767).contains(&sample))
        );
        assert!(
            rectangular
                .iter()
                .all(|&sample| (-32_768..=32_767).contains(&sample))
        );
    }

    #[test]
    fn square_thirty_two_transform_stays_separate_from_sixty_four() {
        let mut coefficients = [0_i32; 1024];
        coefficients[1] = 2_048;
        let square32 = inverse_dct32x32(&coefficients);
        let square64 = inverse_dct64x64(&coefficients);
        assert_ne!(&square32[..64], &square64[..64]);
        assert!(square64.iter().any(|&sample| sample != 0));
    }

    #[test]
    fn zero_rectangular_transform_is_zero() {
        assert_eq!(inverse_dct_adst8x16(&[0; 128]), [0; 128]);
        assert_eq!(inverse_dct4x8(&[0; 32]), [0; 32]);
    }

    #[test]
    fn r4x16_dct_adst_matches_dav1d_vertical4x16_witness() {
        // Dequantized coefficients dumped by dav1d 1.5.3 for the third
        // horizontal sibling of the pure-Rust AVIF coverage witness
        // `coverage_v4x16_predictor_adst_adst_01.avif`. The four 16-value
        // rows are AV1's column-major coefficient storage.
        let coefficients = [
            -3360, -104, 104, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let expected = [
            -24, -45, -61, -69, -24, -45, -61, -69, -24, -46, -61, -70, -25, -46, -62, -71, -25,
            -47, -63, -71, -25, -47, -63, -72, -25, -47, -63, -72, -25, -47, -63, -72, -25, -47,
            -63, -72, -24, -46, -62, -71, -24, -45, -61, -69, -23, -44, -59, -68, -23, -43, -58,
            -66, -22, -42, -57, -65, -22, -41, -56, -63, -22, -41, -55, -63,
        ];

        // AV1 spells this transform in vertical-then-horizontal order:
        // DCT_ADST is horizontal ADST followed by vertical DCT.
        assert_eq!(inverse_adst_dct4x16(&coefficients), expected);
    }

    #[test]
    fn uncovered_transform_wrappers_preserve_zero_input() {
        assert_eq!(inverse_identity16x8(&[0; 128]), [0; 128]);
        assert_eq!(inverse_identity_dct16x8(&[0; 128]), [0; 128]);
        assert_eq!(inverse_identity16x4(&[0; 64]), [0; 64]);
        assert_eq!(inverse_identity_dct16x4(&[0; 64]), [0; 64]);
        assert_eq!(inverse_dct_identity16x4(&[0; 64]), [0; 64]);
        assert_eq!(inverse_adst_dct16x16(&[0; 256]), [0; 256]);
    }

    #[test]
    fn identity_rectangular_transforms_preserve_coefficient_layout() {
        let mut r16x8 = [0_i32; 128];
        // Coefficients use y + x * height; output uses y * width + x.
        r16x8[30] = 4_096; // (x, y) = (3, 6)
        assert_sparse(inverse_identity16x8(&r16x8), &[(99, 512)]);

        let mut r16x4 = [0_i32; 64];
        r16x4[22] = -4_096; // (x, y) = (5, 2)
        assert_sparse(inverse_identity16x4(&r16x4), &[(37, -512)]);
    }

    #[test]
    fn rectangular_transform_variants_match_dav1d_scalar_vectors() {
        // These expected residuals were generated once from dav1d 1.5.3's
        // scalar `inv_txfm_add_c` path with BITDEPTH=8 and HAVE_ASM=0. The
        // destination was initialized to 128 and the bias was subtracted,
        // leaving the signed residual after dav1d's final shift. They are
        // checked-in constants rather than values computed by this module.
        let mut r16x8 = [0_i32; 128];
        r16x8[0] = 93;
        r16x8[8] = -112;
        r16x8[1] = 66;
        r16x8[46] = -79;
        assert_sparse(
            inverse_identity_dct16x8(&r16x8),
            &[
                (0, 8),
                (1, -5),
                (5, -2),
                (16, 8),
                (17, -5),
                (21, 5),
                (32, 6),
                (33, -5),
                (37, -5),
                (48, 5),
                (49, -5),
                (53, 2),
                (64, 3),
                (65, -5),
                (69, 2),
                (80, 2),
                (81, -5),
                (85, -5),
                (96, 1),
                (97, -5),
                (101, 5),
                (113, -5),
                (117, -2),
            ],
        );

        let mut r16x4 = [0_i32; 64];
        r16x4[0] = 93;
        r16x4[4] = -112;
        r16x4[1] = 66;
        r16x4[22] = -79;
        assert_sparse(
            inverse_identity_dct16x4(&r16x4),
            &[
                (0, 11),
                (1, -7),
                (5, -5),
                (16, 8),
                (17, -7),
                (21, 5),
                (32, 4),
                (33, -7),
                (37, 5),
                (49, -7),
                (53, -5),
            ],
        );
        let dct_identity = inverse_dct_identity16x4(&r16x4);
        let expected = [
            [-2, -2, -1, -1, 0, 1, 2, 3, 3, 4, 5, 6, 7, 7, 8, 8],
            [2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2],
            [-3, 0, 3, 3, 1, -2, -3, -2, 2, 4, 2, -1, -3, -3, 0, 3],
            [0; 16],
        ];
        assert_eq!(dct_identity.as_chunks::<16>().0, &expected);
    }

    #[test]
    fn adst_dct16x16_matches_dav1d_scalar_vector() {
        // The input is asymmetric in both axes. This catches an accidental
        // ADST/DCT axis swap that a DC-only vector would not expose.
        let mut coefficients = [0_i32; 256];
        coefficients[0] = 93;
        coefficients[16] = -112;
        coefficients[1] = 66;
        coefficients[35] = -79;
        let output = inverse_adst_dct16x16(&coefficients);
        let expected = [
            [0, -1, -1, -1, -1, -1, 0, 1, 2, 2, 3, 3, 3, 3, 2, 2],
            [0, -1, -1, -1, -1, 0, 0, 1, 1, 2, 2, 3, 3, 3, 3, 3],
            [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 2, 3, 3, 3],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 2, 3, 3, 4],
            [0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 4],
            [0, 1, 1, 1, 0, 0, 0, -1, -1, 0, 0, 1, 2, 3, 4, 4],
            [0, 0, 0, 0, 0, 0, 0, -1, 0, 0, 0, 1, 2, 2, 3, 3],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 3, 3],
            [0, -1, -1, -1, -1, -1, -1, 0, 0, 1, 1, 1, 2, 2, 2, 2],
            [0, -1, -2, -2, -2, -1, -1, 0, 1, 1, 1, 2, 1, 1, 1, 1],
            [0, -1, -2, -2, -2, -1, -1, 0, 1, 1, 2, 2, 1, 1, 1, 1],
            [0, -1, -2, -2, -2, -2, -1, 0, 1, 1, 1, 1, 1, 1, 1, 1],
            [0, -1, -1, -1, -2, -1, -1, -1, 0, 0, 1, 1, 1, 1, 1, 1],
            [0, 0, -1, -1, -1, -1, -1, -1, -1, 0, 0, 0, 1, 1, 1, 1],
            [0, 0, 0, 0, -1, -1, -1, -1, -1, -1, -1, 0, 0, 1, 2, 2],
            [0, 0, 0, 0, 0, -1, -1, -2, -2, -2, -1, 0, 0, 1, 2, 2],
        ];
        assert_eq!(output.as_chunks::<16>().0, &expected);
    }

    #[test]
    fn uncovered_transform_wrappers_bound_extreme_coefficients() {
        assert_i16_bounded(inverse_identity16x8(&[i32::MAX; 128]));
        assert_i16_bounded(inverse_identity_dct16x8(&[i32::MAX; 128]));
        assert_i16_bounded(inverse_identity16x4(&[i32::MAX; 64]));
        assert_i16_bounded(inverse_identity_dct16x4(&[i32::MAX; 64]));
        assert_i16_bounded(inverse_dct_identity16x4(&[i32::MAX; 64]));
        assert_i16_bounded(inverse_adst_dct16x16(&[i32::MAX; 256]));
    }

    #[test]
    fn rectangular_transform_wrappers_match_dav1d_and_invariants() {
        assert_rectangular_wrapper_conformance();
    }

    #[test]
    fn adst16_matches_dav1d_sparse_reference_vector() {
        // Reference: dav1d 1.5.3 inverse ADST16 butterfly in src/itx_1d.c
        // for input [66, -79, 0, ...].
        let input = [66, -79, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(
            inverse_adst16(input),
            [
                -9, -23, -38, -45, -48, -45, -35, -19, 2, 25, 54, 78, 103, 122, 137, 144
            ]
        );
    }

    #[test]
    fn adst_adst16x8_matches_dav1d_sparse_reference_vector() {
        // Reference: dav1d 1.5.3 rectangular 16x8 inverse-transform path
        // for coefficients [0] = 93 and [8] = -112.
        let mut coefficients = [0_i32; 128];
        coefficients[0] = 93;
        coefficients[8] = -112;
        assert_eq!(
            inverse_adst_adst16x8(&coefficients),
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1,
                1, 1, 1, 1, 0, 0, -1, -1, -1, -1, 0, 0, 0, 0, 1, 1, 2, 2, 2, 2, 0, 0, -1, -1, -1,
                -1, -1, 0, 0, 1, 1, 2, 2, 2, 3, 3, 0, 0, -1, -1, -1, -1, -1, 0, 0, 1, 1, 2, 3, 3,
                3, 4, 0, -1, -1, -1, -1, -1, -1, 0, 0, 1, 2, 2, 3, 3, 4, 4, 0, -1, -1, -1, -1, -1,
                -1, -1, 0, 1, 2, 2, 3, 4, 4, 4, 0, -1, -1, -1, -1, -1, -1, -1, 0, 1, 2, 2, 3, 4, 4,
                5
            ]
        );
    }

    #[test]
    fn four_by_eight_dc_is_constant_and_bounded() {
        let mut coefficients = [0_i32; 32];
        coefficients[0] = 4_096;
        let output = inverse_dct4x8(&coefficients);
        assert!(output.iter().all(|&sample| sample == output[0]));
        assert!(output[0] != 0);
        assert!(
            output
                .iter()
                .all(|&sample| (-32_768..=32_767).contains(&sample))
        );
    }

    #[test]
    fn four_by_eight_transform_families_are_distinct() {
        let mut coefficients = [0_i32; 32];
        coefficients[1] = 2_048;
        let dct = inverse_dct4x8(&coefficients);
        let h_dct = inverse_dct_identity4x8(&coefficients);
        let dct_adst = inverse_dct_adst4x8(&coefficients);
        let adst_dct = inverse_adst_dct4x8(&coefficients);
        let adst_adst = inverse_adst_adst4x8(&coefficients);
        assert_ne!(dct, h_dct);
        assert_ne!(dct, dct_adst);
        assert_ne!(dct, adst_dct);
        assert_ne!(dct, adst_adst);
        assert!(
            dct.into_iter()
                .chain(h_dct)
                .chain(dct_adst)
                .chain(adst_dct)
                .chain(adst_adst)
                .all(|sample| (-32_768..=32_767).contains(&sample))
        );
    }

    #[test]
    fn rectangular_transform_has_a_real_sixteen_point_vertical_pass() {
        let mut coefficients = [0_i32; 128];
        // Column-major `y + 16 * x`: this is the first vertical AC basis.
        coefficients[1] = 2_048;
        let output = inverse_dct_adst8x16(&coefficients);

        assert!(output.iter().any(|&sample| sample != 0));
        assert_ne!(&output[..8], &output[120..]);
        assert!(
            output
                .iter()
                .all(|&sample| (-32_768..=32_767).contains(&sample))
        );
    }

    #[test]
    fn rectangular_adst_dc_has_distinct_vertical_rows() {
        let mut coefficients = [0_i32; 128];
        coefficients[0] = 4_096;
        let output = inverse_dct_adst8x16(&coefficients);
        assert_eq!(output.len(), 128);
        assert!(output.iter().any(|&sample| sample != 0));
        assert_ne!(&output[..64], &output[64..]);
    }

    #[test]
    fn thirty_two_by_sixteen_dct_has_the_expected_extent_and_passes() {
        let mut coefficients = [0_i32; 512];
        coefficients[1] = 2_048;
        let output = inverse_dct32x16(&coefficients);
        assert_eq!(output.len(), 512);
        assert!(output.iter().any(|&sample| sample != 0));
        assert_ne!(&output[..32], &output[480..]);
        assert!(
            output
                .iter()
                .all(|&sample| (-32_768..=32_767).contains(&sample))
        );
    }

    #[test]
    fn zero_adst_transforms_are_zero() {
        assert_eq!(inverse_dct_adst4x4(&[0; 16]), [0; 16]);
        assert_eq!(inverse_dct_adst8x8(&[0; 64]), [0; 64]);
    }

    #[test]
    fn adst_transforms_are_distinct_from_dct_for_odd_basis() {
        let mut coefficients4 = [0_i32; 16];
        coefficients4[1] = 2_048;
        assert_ne!(
            inverse_dct_adst4x4(&coefficients4),
            inverse_dct4x4(&coefficients4)
        );

        let mut coefficients8 = [0_i32; 64];
        coefficients8[1] = 2_048;
        assert_ne!(
            inverse_dct_adst8x8(&coefficients8),
            inverse_dct8x8(&coefficients8)
        );
    }

    #[test]
    fn extreme_adst_coefficients_remain_bounded() {
        let output4 = inverse_dct_adst4x4(&[i32::MAX; 16]);
        assert!(
            output4
                .iter()
                .all(|&sample| (-32_768..=32_767).contains(&sample))
        );
        let output8 = inverse_dct_adst8x8(&[i32::MAX; 64]);
        assert!(
            output8
                .iter()
                .all(|&sample| (-32_768..=32_767).contains(&sample))
        );
    }

    #[test]
    fn adst_adst_transform_is_distinct_from_dct_dct_for_odd_basis() {
        let mut coefficients = [0_i32; 16];
        coefficients[1] = 256;
        let dct = inverse_dct4x4(&coefficients);
        let adst = inverse_adst_adst4x4(&coefficients);
        assert_ne!(dct, adst);
        assert!(
            adst.iter()
                .all(|sample| (-32_768..=32_767).contains(sample))
        );

        let mut coefficients8 = [0_i32; 64];
        coefficients8[1] = 256;
        let dct8 = inverse_dct8x8(&coefficients8);
        let adst8 = inverse_adst_adst8x8(&coefficients8);
        assert_ne!(dct8, adst8);
        assert!(
            adst8
                .iter()
                .all(|sample| (-32_768..=32_767).contains(sample))
        );
    }

    #[test]
    fn one_dimensional_transforms_are_distinct_and_bounded() {
        let mut coefficients = [0_i32; 64];
        coefficients[1] = 2_048;
        let dct = inverse_dct8x8(&coefficients);
        let h_dct = inverse_dct_identity8x8(&coefficients);
        let adst_dct = inverse_adst_dct8x8(&coefficients);
        assert_ne!(h_dct, dct);
        assert_ne!(adst_dct, dct);
        assert!(
            h_dct
                .into_iter()
                .chain(adst_dct)
                .all(|sample| (-32_768..=32_767).contains(&sample))
        );
    }

    #[test]
    fn adst_dct_4x4_is_distinct_and_bounded() {
        let mut coefficients = [0_i32; 16];
        coefficients[1] = 2_048;
        let dct = inverse_dct4x4(&coefficients);
        let adst_dct = inverse_adst_dct4x4(&coefficients);
        assert_ne!(adst_dct, dct);
        assert!(
            adst_dct
                .iter()
                .all(|sample| (-32_768..=32_767).contains(sample))
        );
    }

    #[test]
    fn dct_dct16x8_matches_dav1d_chroma_vectors() {
        let cases = [
            (
                [(0, -54), (8, 427), (24, -177), (40, 65), (56, -77)],
                [3, 5, 5, 5, 5, 6, 6, 2, -3, -7, -7, -6, -6, -7, -6, -5],
            ),
            (
                [(0, -54), (8, -366), (24, 118), (40, -65), (56, 77)],
                [-4, -6, -6, -5, -4, -5, -6, -3, 2, 5, 4, 3, 4, 5, 4, 3],
            ),
        ];
        for (nonzero, expected_row) in cases {
            let mut coefficients = [0_i32; 128];
            for (index, value) in nonzero {
                coefficients[index] = value;
            }
            let residual = super::inverse_dct16x8(&coefficients);
            assert_eq!(&residual[..16], &expected_row);
            assert!(
                residual
                    .as_chunks::<16>()
                    .0
                    .iter()
                    .all(|row| *row == expected_row)
            );
        }
    }
}
