//! Optimized alpha blending routines based on libwebp
//!
//! <https://github.com/webmproject/libwebp/blob/e4f7a9f0c7c9fbfae1568bc7fa5c94b989b50872/src/demux/anim_decode.c#L215-L267>

#![warn(clippy::all)]
#![deny(
    clippy::clone_on_copy,
    clippy::expect_used,
    clippy::large_enum_variant,
    clippy::map_unwrap_or,
    clippy::needless_borrow,
    clippy::needless_collect,
    clippy::needless_range_loop,
    clippy::redundant_clone,
    clippy::todo,
    clippy::unnecessary_cast,
    clippy::unnecessary_to_owned,
    clippy::unwrap_in_result,
    clippy::unwrap_used
)]
#![warn(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

const fn channel_shift(i: u32) -> u32 {
    i.wrapping_mul(8)
}

/// Blend a single channel of `src` over `dst`, given their alpha channel values.
/// `src` and `dst` are assumed to be NOT pre-multiplied by alpha.
fn blend_channel_nonpremult(
    src: u32,
    src_a: u8,
    dst: u32,
    dst_a: u8,
    scale: u32,
    shift: u32,
) -> u8 {
    let src_channel = src.wrapping_shr(shift).to_le_bytes()[0];
    let dst_channel = dst.wrapping_shr(shift).to_le_bytes()[0];
    // Each product is at most 255² and their sum fits comfortably in u32.
    // Wrapping operations state the ported unsigned-C arithmetic explicitly.
    let blend_unscaled = u32::from(src_channel)
        .wrapping_mul(u32::from(src_a))
        .wrapping_add(u32::from(dst_channel).wrapping_mul(u32::from(dst_a)));
    // libwebp's reciprocal scale keeps the shifted result in one byte.
    blend_unscaled
        .wrapping_mul(scale)
        .wrapping_shr(channel_shift(3))
        .to_le_bytes()[0]
}

/// Blend `src` over `dst` assuming they are NOT pre-multiplied by alpha.
fn blend_pixel_nonpremult(src: u32, dst: u32) -> u32 {
    let src_a = ((src >> channel_shift(3)) & 0xff) as u8;

    if src_a == 0 {
        dst
    } else {
        let dst_a = ((dst >> channel_shift(3)) & 0xff) as u8;
        if dst_a == 0 {
            return src;
        }
        // Match libwebp's approximate integer arithmetic for:
        // dst_factor_a = (dst_a * (255 - src_a)) / 255.
        let dst_factor_a = u32::from(dst_a)
            .wrapping_mul(256u32.wrapping_sub(u32::from(src_a)))
            .wrapping_shr(8);
        let blend_a = u32::from(src_a).wrapping_add(dst_factor_a);
        // `src_a != 0`, so the divisor is nonzero.
        let scale = {
            #[allow(clippy::arithmetic_side_effects)]
            {
                1u32.wrapping_shl(24) / blend_a
            }
        };
        let dst_factor_a = dst_factor_a.to_le_bytes()[0];

        let blend_r =
            blend_channel_nonpremult(src, src_a, dst, dst_factor_a, scale, channel_shift(0));
        let blend_g =
            blend_channel_nonpremult(src, src_a, dst, dst_factor_a, scale, channel_shift(1));
        let blend_b =
            blend_channel_nonpremult(src, src_a, dst, dst_factor_a, scale, channel_shift(2));
        debug_assert!(u32::from(src_a).wrapping_add(u32::from(dst_factor_a)) < 256);

        (u32::from(blend_r) << channel_shift(0))
            | (u32::from(blend_g) << channel_shift(1))
            | (u32::from(blend_b) << channel_shift(2))
            | (blend_a << channel_shift(3))
    }
}

pub(crate) fn do_alpha_blending(buffer: [u8; 4], canvas: [u8; 4]) -> [u8; 4] {
    // libwebp 1.6.0 anim_decode.c:245-251 bypasses reciprocal blending for
    // fully opaque source pixels, preserving their channels byte-for-byte.
    if buffer[3] == 255 {
        return buffer;
    }
    // The original C code contained different shift functions for different endianness,
    // but they didn't work when ported to Rust directly (and probably didn't work in C either).
    // So instead we reverse the order of bytes on big-endian here, at the interface.
    // `from_le_bytes` is a no-op on little endian (most systems) and a cheap shuffle on big endian.
    blend_pixel_nonpremult(u32::from_le_bytes(buffer), u32::from_le_bytes(canvas)).to_le_bytes()
}
