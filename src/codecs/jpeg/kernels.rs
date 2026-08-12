//! Safe, fixed-shape JPEG color kernels.
//!
//! The vectorization workspace identified color conversion as a good first
//! batch boundary: every pixel is independent, while row tails and malformed
//! input remain the caller's responsibility. These kernels deliberately use
//! only safe slices and fixed-size batches. LLVM may auto-vectorize the
//! regular loop in an optimized build, but the codec never relies on a target
//! feature or on undefined behavior for correctness.

use wide::bytemuck::{cast, pod_read_unaligned};
use wide::{i16x8, i32x8, u8x16, u16x8};

const Y_R: i32 = 19_595;
const Y_G: i32 = 38_470;
const Y_B: i32 = 7_471;
const CB_R: i32 = -11_059;
const CB_G: i32 = -21_709;
const CB_B: i32 = 32_768;
const CR_R: i32 = 32_768;
const CR_G: i32 = -27_439;
const CR_B: i32 = -5_329;
const CHROMA_BIAS: i32 = (128i32 << 16) + 32_767;

// Two overlapping safe 16-byte loads cover each 24-byte RGB batch. These
// masks deinterleave eight channels; high-bit mask entries become zero and
// the two partial vectors can therefore be joined with bitwise OR.
const RED_FIRST: u8x16 = u8x16::new([
    0, 3, 6, 9, 12, 15, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
]);
const RED_SECOND: u8x16 = u8x16::new([
    0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 10, 13, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
]);
const GREEN_FIRST: u8x16 = u8x16::new([
    1, 4, 7, 10, 13, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
]);
const GREEN_SECOND: u8x16 = u8x16::new([
    0x80, 0x80, 0x80, 0x80, 0x80, 8, 11, 14, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
]);
const BLUE_FIRST: u8x16 = u8x16::new([
    2, 5, 8, 11, 14, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
]);
const BLUE_SECOND: u8x16 = u8x16::new([
    0x80, 0x80, 0x80, 0x80, 0x80, 9, 12, 15, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
]);

// The three rows of the RGB->YCbCr matrix sum to 65,536, 0, and 0. Expressing
// them relative to G, B, and R respectively reduces each output from three
// widening multiplies to two, and every remaining coefficient fits i16.
const Y_RG_I16: i16x8 = i16x8::new([19_595; 8]);
const Y_BG_I16: i16x8 = i16x8::new([7_471; 8]);
const CB_RB_I16: i16x8 = i16x8::new([-11_059; 8]);
const CB_GB_I16: i16x8 = i16x8::new([-21_709; 8]);
const CR_RG_I16: i16x8 = i16x8::new([27_439; 8]);
const CR_RB_I16: i16x8 = i16x8::new([5_329; 8]);
const LUMA_BIAS_I32: i32x8 = i32x8::new([32_768; 8]);
const CHROMA_BIAS_I32: i32x8 = i32x8::new([CHROMA_BIAS; 8]);
const PAIR_EVEN: u8x16 = u8x16::new([
    0, 2, 4, 6, 8, 10, 12, 14, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
]);
const PAIR_ODD: u8x16 = u8x16::new([
    1, 3, 5, 7, 9, 11, 13, 15, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
]);
const H2V1_BIAS: u16x8 = u16x8::new([0, 1, 0, 1, 0, 1, 0, 1]);
const H2V2_BIAS: u16x8 = u16x8::new([1, 2, 1, 2, 1, 2, 1, 2]);
const YCC_CENTER_I16: i16x8 = i16x8::new([-128; 8]);
const YCC_ONE_I16: i16x8 = i16x8::new([1; 8]);
const YCC_HALF_I32: i32x8 = i32x8::new([32_768; 8]);
// Coefficients outside i16 are split into a signed low half and an exact
// power-of-two correction. This keeps the safe SIMD implementation on the
// widening-multiply path that maps directly to AArch64 smull/smlal.
const YCC_CR_R_LOW_I16: i16x8 = i16x8::new([26_345; 8]);
const YCC_CB_B_LOW_I16: i16x8 = i16x8::new([-14_942; 8]);
const YCC_CR_G_LOW_I16: i16x8 = i16x8::new([18_734; 8]);
const YCC_CB_G_I16: i16x8 = i16x8::new([-22_554; 8]);
const YCC_RED_FIRST: u8x16 = u8x16::new([
    0, 0x80, 0x80, 1, 0x80, 0x80, 2, 0x80, 0x80, 3, 0x80, 0x80, 4, 0x80, 0x80, 5,
]);
const YCC_GREEN_FIRST: u8x16 = u8x16::new([
    0x80, 0, 0x80, 0x80, 1, 0x80, 0x80, 2, 0x80, 0x80, 3, 0x80, 0x80, 4, 0x80, 0x80,
]);
const YCC_BLUE_FIRST: u8x16 = u8x16::new([
    0x80, 0x80, 0, 0x80, 0x80, 1, 0x80, 0x80, 2, 0x80, 0x80, 3, 0x80, 0x80, 4, 0x80,
]);
const YCC_RED_SECOND: u8x16 = u8x16::new([
    0x80, 0x80, 6, 0x80, 0x80, 7, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
]);
const YCC_GREEN_SECOND: u8x16 = u8x16::new([
    5, 0x80, 0x80, 6, 0x80, 0x80, 7, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
]);
const YCC_BLUE_SECOND: u8x16 = u8x16::new([
    0x80, 5, 0x80, 0x80, 6, 0x80, 0x80, 7, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
]);

/// Convert one RGB sample using the exact libjpeg fixed-point coefficients.
#[cfg_attr(not(coverage), inline(always))]
pub(crate) fn rgb_to_ycbcr_pixel(red: u8, green: u8, blue: u8) -> (u8, u8, u8) {
    let red = i32::from(red);
    let green = i32::from(green);
    let blue = i32::from(blue);
    (
        fixed_point(Y_R, red, Y_G, green, Y_B, blue, 32_768),
        fixed_point(CB_R, red, CB_G, green, CB_B, blue, CHROMA_BIAS),
        fixed_point(CR_R, red, CR_G, green, CR_B, blue, CHROMA_BIAS),
    )
}

/// Convert an RGB image into planar Y, Cb, and Cr samples in eight-pixel
/// batches, with a scalar tail for the final partial batch.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "validated sample counts make the vector batch and scalar tail bounds explicit"
)]
#[inline(never)]
pub(crate) fn rgb_to_ycbcr_batch(
    pixels: &[u8],
    width: usize,
    height: usize,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let sample_count = width.saturating_mul(height);
    let available = sample_count.min(pixels.len().div_euclid(3));
    let mut y = vec![0u8; sample_count];
    let mut cb = vec![0u8; sample_count];
    let mut cr = vec![0u8; sample_count];

    let batched_pixels = available - available % 8;
    let batched_bytes = batched_pixels.saturating_mul(3);
    let sources = pixels[..batched_bytes].chunks_exact(24);
    let y_destinations = y[..batched_pixels].chunks_exact_mut(8);
    let cb_destinations = cb[..batched_pixels].chunks_exact_mut(8);
    let cr_destinations = cr[..batched_pixels].chunks_exact_mut(8);
    for (((source, y_destination), cb_destination), cr_destination) in sources
        .zip(y_destinations)
        .zip(cb_destinations)
        .zip(cr_destinations)
    {
        let source: &[u8; 24] = source
            .try_into()
            .unwrap_or_else(|_| unreachable!("chunks_exact returned a non-24-byte RGB batch"));
        let y_destination: &mut [u8; 8] = y_destination
            .try_into()
            .unwrap_or_else(|_| unreachable!("chunks_exact returned a non-eight-byte Y batch"));
        let cb_destination: &mut [u8; 8] = cb_destination
            .try_into()
            .unwrap_or_else(|_| unreachable!("chunks_exact returned a non-eight-byte Cb batch"));
        let cr_destination: &mut [u8; 8] = cr_destination
            .try_into()
            .unwrap_or_else(|_| unreachable!("chunks_exact returned a non-eight-byte Cr batch"));
        convert_eight(source, y_destination, cb_destination, cr_destination);
    }
    let mut pixel = batched_pixels;
    while pixel < available {
        let source = pixel.saturating_mul(3);
        let (y_sample, cb_sample, cr_sample) = rgb_to_ycbcr_pixel(
            pixels[source],
            pixels[source.saturating_add(1)],
            pixels[source.saturating_add(2)],
        );
        y[pixel] = y_sample;
        cb[pixel] = cb_sample;
        cr[pixel] = cr_sample;
        pixel = pixel.saturating_add(1);
    }
    (y, cb, cr)
}

/// Convert RGB pixels while producing 4:2:0 chroma at its final resolution.
///
/// The ordinary conversion path materializes full-size Cb and Cr planes and
/// reads both back during downsampling. This exact fused form retains only two
/// 16-sample chroma packets at a time. Right and bottom edges repeat the last
/// source sample, matching libjpeg's h2v2 expansion and rounding rules.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "validated image dimensions bound the fused 4:2:0 packet arithmetic"
)]
pub(crate) fn rgb_to_ycbcr_420_batch(
    pixels: &[u8],
    width: usize,
    height: usize,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let sample_count = width.saturating_mul(height);
    let chroma_width = width.div_ceil(16).saturating_mul(8);
    let chroma_height = height.div_ceil(2);
    let mut y = vec![0u8; sample_count];
    let mut cb = vec![0u8; chroma_width.saturating_mul(chroma_height)];
    let mut cr = vec![0u8; chroma_width.saturating_mul(chroma_height)];
    if width == 0 || height == 0 {
        return (y, cb, cr);
    }
    debug_assert!(pixels.len() >= sample_count.saturating_mul(3));

    // The production baseline fast path is MCU aligned. Convert both source
    // rows of each h2v2 group together so their temporary chroma packets can
    // be downsampled immediately, without allocating and rereading two
    // full-width Cb/Cr row buffers.
    if width.is_multiple_of(16) && height.is_multiple_of(2) {
        for source_y in (0usize..height).step_by(2) {
            let first_row_start = source_y.saturating_mul(width);
            let second_row_start = first_row_start.saturating_add(width);
            let chroma_row_start = source_y.div_euclid(2).saturating_mul(chroma_width);
            for x in (0usize..width).step_by(16) {
                let first_pixel = first_row_start.saturating_add(x);
                let second_pixel = second_row_start.saturating_add(x);
                let first_source = first_pixel.saturating_mul(3);
                let second_source = second_pixel.saturating_mul(3);
                let first_rgb0: &[u8; 24] = pixels[first_source..first_source.saturating_add(24)]
                    .try_into()
                    .unwrap_or_else(|_| unreachable!("validated RGB packet is not 24 bytes"));
                let first_rgb1: &[u8; 24] = pixels
                    [first_source.saturating_add(24)..first_source.saturating_add(48)]
                    .try_into()
                    .unwrap_or_else(|_| unreachable!("validated RGB packet is not 24 bytes"));
                let second_rgb0: &[u8; 24] = pixels
                    [second_source..second_source.saturating_add(24)]
                    .try_into()
                    .unwrap_or_else(|_| unreachable!("validated RGB packet is not 24 bytes"));
                let second_rgb1: &[u8; 24] = pixels
                    [second_source.saturating_add(24)..second_source.saturating_add(48)]
                    .try_into()
                    .unwrap_or_else(|_| unreachable!("validated RGB packet is not 24 bytes"));

                let (y00, cb00, cr00) = convert_eight_values(first_rgb0);
                let (y01, cb01, cr01) = convert_eight_values(first_rgb1);
                let (y10, cb10, cr10) = convert_eight_values(second_rgb0);
                let (y11, cb11, cr11) = convert_eight_values(second_rgb1);
                let y_first = pack_two_eight(y00, y01).to_array();
                let y_second = pack_two_eight(y10, y11).to_array();
                y[first_pixel..first_pixel.saturating_add(16)].copy_from_slice(&y_first);
                y[second_pixel..second_pixel.saturating_add(16)].copy_from_slice(&y_second);

                let cb_first = pack_two_eight(cb00, cb01);
                let cr_first = pack_two_eight(cr00, cr01);
                let cb_second = pack_two_eight(cb10, cb11);
                let cr_second = pack_two_eight(cr10, cr11);

                let chroma_start = chroma_row_start.saturating_add(x.div_euclid(2));
                cb[chroma_start..chroma_start.saturating_add(8)]
                    .copy_from_slice(&downsample_h2v2_vectors(cb_first, cb_second));
                cr[chroma_start..chroma_start.saturating_add(8)]
                    .copy_from_slice(&downsample_h2v2_vectors(cr_first, cr_second));
            }
        }
        return (y, cb, cr);
    }

    let mut cb_rows = vec![0u8; width.saturating_mul(2)];
    let mut cr_rows = vec![0u8; width.saturating_mul(2)];
    let (cb_even, cb_odd) = cb_rows.split_at_mut(width);
    let (cr_even, cr_odd) = cr_rows.split_at_mut(width);

    for source_y in 0usize..height {
        let row_start = source_y.saturating_mul(width);
        let y_row = &mut y[row_start..row_start.saturating_add(width)];
        let (cb_row, cr_row) = if source_y.is_multiple_of(2) {
            (&mut cb_even[..], &mut cr_even[..])
        } else {
            (&mut cb_odd[..], &mut cr_odd[..])
        };
        let complete = width - width % 8;
        for x in (0usize..complete).step_by(8) {
            let source_start = row_start.saturating_add(x).saturating_mul(3);
            let source: &[u8; 24] = pixels[source_start..source_start.saturating_add(24)]
                .try_into()
                .unwrap_or_else(|_| unreachable!("validated RGB packet is not 24 bytes"));
            let y_output: &mut [u8; 8] = (&mut y_row[x..x.saturating_add(8)])
                .try_into()
                .unwrap_or_else(|_| unreachable!("validated Y packet is not eight bytes"));
            let cb_output: &mut [u8; 8] = (&mut cb_row[x..x.saturating_add(8)])
                .try_into()
                .unwrap_or_else(|_| unreachable!("validated Cb packet is not eight bytes"));
            let cr_output: &mut [u8; 8] = (&mut cr_row[x..x.saturating_add(8)])
                .try_into()
                .unwrap_or_else(|_| unreachable!("validated Cr packet is not eight bytes"));
            convert_eight(source, y_output, cb_output, cr_output);
        }

        let remainder = width.saturating_sub(complete);
        if remainder != 0 {
            let source_start = row_start.saturating_add(complete).saturating_mul(3);
            let mut padded_rgb = [0u8; 24];
            let source_bytes = remainder.saturating_mul(3);
            padded_rgb[..source_bytes]
                .copy_from_slice(&pixels[source_start..source_start.saturating_add(source_bytes)]);
            let final_pixel = remainder.saturating_sub(1).saturating_mul(3);
            let repeated = [
                padded_rgb[final_pixel],
                padded_rgb[final_pixel.saturating_add(1)],
                padded_rgb[final_pixel.saturating_add(2)],
            ];
            for lane in remainder..8 {
                let destination = lane.saturating_mul(3);
                padded_rgb[destination..destination.saturating_add(3)].copy_from_slice(&repeated);
            }
            let mut padded_y = [0u8; 8];
            let mut padded_cb = [0u8; 8];
            let mut padded_cr = [0u8; 8];
            convert_eight(&padded_rgb, &mut padded_y, &mut padded_cb, &mut padded_cr);
            y_row[complete..].copy_from_slice(&padded_y[..remainder]);
            cb_row[complete..].copy_from_slice(&padded_cb[..remainder]);
            cr_row[complete..].copy_from_slice(&padded_cr[..remainder]);
        }

        if !source_y.is_multiple_of(2) {
            let output_start = source_y.div_euclid(2).saturating_mul(chroma_width);
            downsample_420_rows(cb_even, cb_odd, chroma_width, &mut cb, output_start);
            downsample_420_rows(cr_even, cr_odd, chroma_width, &mut cr, output_start);
        }
    }

    if !height.is_multiple_of(2) {
        let output_start = height.div_euclid(2).saturating_mul(chroma_width);
        downsample_420_rows(cb_even, cb_even, chroma_width, &mut cb, output_start);
        downsample_420_rows(cr_even, cr_even, chroma_width, &mut cr, output_start);
    }
    (y, cb, cr)
}

/// Convert one 16-pixel pair of RGB rows to two luma rows and one 4:2:0
/// chroma row. The fixed packet is the safe hand-off used by the streaming
/// encoder; callers own right/bottom sample replication.
pub(crate) fn rgb_to_ycbcr_420_packet(
    first_row: &[u8; 48],
    second_row: &[u8; 48],
) -> ([u8; 16], [u8; 16], [u8; 8], [u8; 8]) {
    let first_rgb0: &[u8; 24] = first_row[..24]
        .try_into()
        .unwrap_or_else(|_| unreachable!("fixed RGB half-packet is 24 bytes"));
    let first_rgb1: &[u8; 24] = first_row[24..]
        .try_into()
        .unwrap_or_else(|_| unreachable!("fixed RGB half-packet is 24 bytes"));
    let second_rgb0: &[u8; 24] = second_row[..24]
        .try_into()
        .unwrap_or_else(|_| unreachable!("fixed RGB half-packet is 24 bytes"));
    let second_rgb1: &[u8; 24] = second_row[24..]
        .try_into()
        .unwrap_or_else(|_| unreachable!("fixed RGB half-packet is 24 bytes"));

    let (y00, cb00, cr00) = convert_eight_values(first_rgb0);
    let (y01, cb01, cr01) = convert_eight_values(first_rgb1);
    let (y10, cb10, cr10) = convert_eight_values(second_rgb0);
    let (y11, cb11, cr11) = convert_eight_values(second_rgb1);
    let y_first = pack_two_eight(y00, y01).to_array();
    let y_second = pack_two_eight(y10, y11).to_array();
    let cb = downsample_h2v2_vectors(pack_two_eight(cb00, cb01), pack_two_eight(cb10, cb11));
    let cr = downsample_h2v2_vectors(pack_two_eight(cr00, cr01), pack_two_eight(cr10, cr11));
    (y_first, y_second, cb, cr)
}

#[expect(
    clippy::arithmetic_side_effects,
    reason = "validated row geometry bounds the safe edge-replication indices"
)]
#[inline(never)]
fn downsample_420_rows(
    row0: &[u8],
    row1: &[u8],
    destination_width: usize,
    output: &mut [u8],
    output_start: usize,
) {
    debug_assert!(!row0.is_empty());
    debug_assert_eq!(row0.len(), row1.len());
    for output_x in (0usize..destination_width).step_by(8) {
        let source_x = output_x.saturating_mul(2);
        let averaged = if source_x.saturating_add(16) <= row0.len() {
            let source0: &[u8; 16] = row0[source_x..source_x.saturating_add(16)]
                .try_into()
                .unwrap_or_else(|_| unreachable!("validated chroma row packet is not 16 bytes"));
            let source1: &[u8; 16] = row1[source_x..source_x.saturating_add(16)]
                .try_into()
                .unwrap_or_else(|_| unreachable!("validated chroma row packet is not 16 bytes"));
            downsample_h2v2_eight(source0, source1)
        } else {
            let mut source0 = [0u8; 16];
            let mut source1 = [0u8; 16];
            for lane in 0usize..16 {
                let source = source_x.saturating_add(lane).min(row0.len() - 1);
                source0[lane] = row0[source];
                source1[lane] = row1[source];
            }
            downsample_h2v2_eight(&source0, &source1)
        };
        let destination = output_start.saturating_add(output_x);
        output[destination..destination.saturating_add(8)].copy_from_slice(&averaged);
    }
}

#[cfg_attr(not(coverage), inline(always))]
fn convert_eight(pixels: &[u8; 24], y: &mut [u8; 8], cb: &mut [u8; 8], cr: &mut [u8; 8]) {
    let (y_values, cb_values, cr_values) = convert_eight_values(pixels);
    *y = narrow_eight(y_values);
    *cb = narrow_eight(cb_values);
    *cr = narrow_eight(cr_values);
}

#[expect(
    clippy::arithmetic_side_effects,
    reason = "8-bit RGB samples and libjpeg fixed-point coefficients stay within i32 lanes"
)]
#[cfg_attr(not(coverage), inline(always))]
fn convert_eight_values(pixels: &[u8; 24]) -> (i32x8, i32x8, i32x8) {
    let first = pod_read_unaligned::<u8x16>(&pixels[..16]);
    let second = pod_read_unaligned::<u8x16>(&pixels[8..]);
    let red = first.swizzle_relaxed(RED_FIRST) | second.swizzle_relaxed(RED_SECOND);
    let green = first.swizzle_relaxed(GREEN_FIRST) | second.swizzle_relaxed(GREEN_SECOND);
    let blue = first.swizzle_relaxed(BLUE_FIRST) | second.swizzle_relaxed(BLUE_SECOND);
    // Widen as unsigned first, then reinterpret the 0..=255 lanes as signed.
    // This is mathematically identical for byte samples and maps to the native
    // AArch64 widening instruction without the scalar lane expansion used by
    // the signed convenience constructor.
    let red = cast::<u16x8, i16x8>(u16x8::from_u8x16_low(red));
    let green = cast::<u16x8, i16x8>(u16x8::from_u8x16_low(green));
    let blue = cast::<u16x8, i16x8>(u16x8::from_u8x16_low(blue));
    let green_i32 = i32x8::from(green);
    let red_minus_green = red - green;
    let blue_minus_green = blue - green;
    let red_minus_blue = red - blue;
    let green_minus_blue = green - blue;

    let y_values = (red_minus_green.widening_mul(Y_RG_I16)
        + blue_minus_green.widening_mul(Y_BG_I16)
        + green_i32.unbounded_shl_scalar(16)
        + LUMA_BIAS_I32)
        .unbounded_shr_scalar(16);
    let cb_values = (red_minus_blue.widening_mul(CB_RB_I16)
        + green_minus_blue.widening_mul(CB_GB_I16)
        + CHROMA_BIAS_I32)
        .unbounded_shr_scalar(16);
    let cr_values = (red_minus_green.widening_mul(CR_RG_I16)
        + red_minus_blue.widening_mul(CR_RB_I16)
        + CHROMA_BIAS_I32)
        .unbounded_shr_scalar(16);
    (y_values, cb_values, cr_values)
}

#[cfg_attr(not(coverage), inline(always))]
fn pack_two_eight(first: i32x8, second: i32x8) -> u8x16 {
    u8x16::narrow_i16x8(
        i16x8::from_i32x8_saturate(first),
        i16x8::from_i32x8_saturate(second),
    )
}

#[cfg_attr(not(coverage), inline(always))]
fn narrow_eight(values: i32x8) -> [u8; 8] {
    let packed = u8x16::narrow_i16x8(i16x8::from_i32x8_saturate(values), i16x8::ZERO).to_array();
    let [a, b, c, d, e, f, g, h, ..] = packed;
    [a, b, c, d, e, f, g, h]
}

#[cfg_attr(not(coverage), inline(always))]
fn fixed_point(
    first_weight: i32,
    first: i32,
    second_weight: i32,
    second: i32,
    third_weight: i32,
    third: i32,
    bias: i32,
) -> u8 {
    first_weight
        .saturating_mul(first)
        .saturating_add(second_weight.saturating_mul(second))
        .saturating_add(third_weight.saturating_mul(third))
        .saturating_add(bias)
        .wrapping_shr(16)
        .to_le_bytes()[0]
}

/// Average eight adjacent horizontal pairs from one row using libjpeg's
/// alternating h2v1 rounding bias.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "8-bit chroma pairs and their rounding bias fit within u16 lanes"
)]
#[cfg_attr(not(coverage), inline(always))]
pub(crate) fn downsample_h2v1_eight(row: &[u8; 16]) -> [u8; 8] {
    let samples = pod_read_unaligned::<u8x16>(row);
    let sum = widen_pair_half(samples, PAIR_EVEN) + widen_pair_half(samples, PAIR_ODD);
    narrow_u16_eight((sum + H2V1_BIAS).unbounded_shr_scalar(1))
}

/// Average eight 2x2 sample boxes using libjpeg's alternating h2v2 rounding
/// bias.
#[cfg_attr(not(coverage), inline(always))]
pub(crate) fn downsample_h2v2_eight(row0: &[u8; 16], row1: &[u8; 16]) -> [u8; 8] {
    let row0 = pod_read_unaligned::<u8x16>(row0);
    let row1 = pod_read_unaligned::<u8x16>(row1);
    downsample_h2v2_vectors(row0, row1)
}

#[cfg_attr(not(coverage), inline(always))]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "8-bit chroma boxes and their rounding bias fit within u16 lanes"
)]
fn downsample_h2v2_vectors(row0: u8x16, row1: u8x16) -> [u8; 8] {
    let sum = widen_pair_half(row0, PAIR_EVEN)
        + widen_pair_half(row0, PAIR_ODD)
        + widen_pair_half(row1, PAIR_EVEN)
        + widen_pair_half(row1, PAIR_ODD);
    narrow_u16_eight((sum + H2V2_BIAS).unbounded_shr_scalar(2))
}

#[cfg_attr(not(coverage), inline(always))]
fn widen_pair_half(samples: u8x16, mask: u8x16) -> u16x8 {
    u16x8::from_u8x16_low(samples.swizzle_relaxed(mask))
}

#[cfg_attr(not(coverage), inline(always))]
fn narrow_u16_eight(values: u16x8) -> [u8; 8] {
    let packed = u8x16::narrow_i16x8(cast(values), i16x8::ZERO).to_array();
    let [a, b, c, d, e, f, g, h, ..] = packed;
    [a, b, c, d, e, f, g, h]
}

/// Convert one row of YCbCr samples into interleaved RGB output in eight-pixel
/// batches. The input slices may be shorter than a full row only when the
/// caller intentionally supplies a tail; output is sized for the samples that
/// are present.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "validated row lengths bound the vector batches and padded tail"
)]
#[inline(never)]
pub(crate) fn ycc_to_rgb_batch(y: &[u8], cb: &[u8], cr: &[u8], output: &mut [u8]) {
    let sample_count = y
        .len()
        .min(cb.len())
        .min(cr.len())
        .min(output.len().div_euclid(3));
    let full_samples = sample_count - sample_count % 8;
    let input_samples = y.len().min(cb.len()).min(cr.len());
    let padded_samples = full_samples.min(
        input_samples
            .saturating_sub(8)
            .div_euclid(8)
            .saturating_mul(8),
    );
    for start in (0..padded_samples).step_by(8) {
        let end = start.saturating_add(16);
        let y_batch: &[u8; 16] = y[start..end]
            .try_into()
            .unwrap_or_else(|_| unreachable!("padded Y batch has invalid length"));
        let cb_batch: &[u8; 16] = cb[start..end]
            .try_into()
            .unwrap_or_else(|_| unreachable!("padded Cb batch has invalid length"));
        let cr_batch: &[u8; 16] = cr[start..end]
            .try_into()
            .unwrap_or_else(|_| unreachable!("padded Cr batch has invalid length"));
        let output_start = start.saturating_mul(3);
        let output_batch: &mut [u8; 24] = (&mut output
            [output_start..output_start.saturating_add(24)])
            .try_into()
            .unwrap_or_else(|_| unreachable!("eight-sample RGB batch has invalid length"));
        convert_ycc_to_rgb_eight_padded(y_batch, cb_batch, cr_batch, output_batch);
    }
    if full_samples > padded_samples {
        let y_batch: &[u8; 8] = y[padded_samples..full_samples]
            .try_into()
            .unwrap_or_else(|_| unreachable!("eight-sample Y tail has invalid length"));
        let cb_batch: &[u8; 8] = cb[padded_samples..full_samples]
            .try_into()
            .unwrap_or_else(|_| unreachable!("eight-sample Cb tail has invalid length"));
        let cr_batch: &[u8; 8] = cr[padded_samples..full_samples]
            .try_into()
            .unwrap_or_else(|_| unreachable!("eight-sample Cr tail has invalid length"));
        let output_start = padded_samples.saturating_mul(3);
        let output_batch: &mut [u8; 24] = (&mut output
            [output_start..output_start.saturating_add(24)])
            .try_into()
            .unwrap_or_else(|_| unreachable!("eight-sample RGB tail has invalid length"));
        convert_ycc_to_rgb_eight(y_batch, cb_batch, cr_batch, output_batch);
    }
    let mut pixel = full_samples;
    while pixel < sample_count {
        let (red, green, blue) = ycc_to_rgb_standard_pixel(y[pixel], cb[pixel], cr[pixel]);
        let destination = pixel.saturating_mul(3);
        output[destination] = red;
        output[destination.saturating_add(1)] = green;
        output[destination.saturating_add(2)] = blue;
        pixel = pixel.saturating_add(1);
    }
}

#[cfg_attr(not(coverage), inline(always))]
fn convert_ycc_to_rgb_eight(y: &[u8; 8], cb: &[u8; 8], cr: &[u8; 8], output: &mut [u8; 24]) {
    let y_values = load_eight_u8_as_i16(y);
    let cb_values = load_eight_u8_as_i16(cb);
    let cr_values = load_eight_u8_as_i16(cr);
    *output = convert_ycc_vectors_to_rgb(y_values, cb_values, cr_values);
}

#[cfg_attr(not(coverage), inline(always))]
fn convert_ycc_to_rgb_eight_padded(
    y: &[u8; 16],
    cb: &[u8; 16],
    cr: &[u8; 16],
    output: &mut [u8; 24],
) {
    let y = pod_read_unaligned::<u8x16>(y);
    let cb = pod_read_unaligned::<u8x16>(cb);
    let cr = pod_read_unaligned::<u8x16>(cr);
    *output = convert_ycc_vectors_to_rgb(
        cast(u16x8::from_u8x16_low(y)),
        cast(u16x8::from_u8x16_low(cb)),
        cast(u16x8::from_u8x16_low(cr)),
    );
}

#[expect(
    clippy::arithmetic_side_effects,
    reason = "YCbCr samples and the fixed-point inverse matrix fit within i32 lanes"
)]
#[cfg_attr(not(coverage), inline(always))]
fn convert_ycc_vectors_to_rgb(y_values: i16x8, cb_values: i16x8, cr_values: i16x8) -> [u8; 24] {
    let y_values = y_values.widening_mul(YCC_ONE_I16);
    let cb_values = cb_values + YCC_CENTER_I16;
    let cr_values = cr_values + YCC_CENTER_I16;
    let cb_wide = cb_values.widening_mul(YCC_ONE_I16);
    let cr_wide = cr_values.widening_mul(YCC_ONE_I16);

    let red = narrow_eight_vector(
        y_values
            + (cr_values.widening_mul(YCC_CR_R_LOW_I16)
                + cr_wide.unbounded_shl_scalar(16)
                + YCC_HALF_I32)
                .unbounded_shr_scalar(16),
    );
    let green = narrow_eight_vector(
        y_values
            + (cb_values.widening_mul(YCC_CB_G_I16) + cr_values.widening_mul(YCC_CR_G_LOW_I16)
                - cr_wide.unbounded_shl_scalar(16)
                + YCC_HALF_I32)
                .unbounded_shr_scalar(16),
    );
    let blue = narrow_eight_vector(
        y_values
            + (cb_values.widening_mul(YCC_CB_B_LOW_I16)
                + cb_wide.unbounded_shl_scalar(17)
                + YCC_HALF_I32)
                .unbounded_shr_scalar(16),
    );

    let first = (red.swizzle_relaxed(YCC_RED_FIRST)
        | green.swizzle_relaxed(YCC_GREEN_FIRST)
        | blue.swizzle_relaxed(YCC_BLUE_FIRST))
    .to_array();
    let second = (red.swizzle_relaxed(YCC_RED_SECOND)
        | green.swizzle_relaxed(YCC_GREEN_SECOND)
        | blue.swizzle_relaxed(YCC_BLUE_SECOND))
    .to_array();
    let mut output = [0u8; 24];
    output[..16].copy_from_slice(&first);
    output[16..].copy_from_slice(&second[..8]);
    output
}

#[cfg_attr(not(coverage), inline(always))]
fn narrow_eight_vector(values: i32x8) -> u8x16 {
    u8x16::narrow_i16x8(i16x8::from_i32x8_saturate(values), i16x8::ZERO)
}

#[cfg_attr(not(coverage), inline(always))]
fn load_eight_u8_as_i16(samples: &[u8; 8]) -> i16x8 {
    let packed = u64::from_ne_bytes(*samples);
    cast(u16x8::from_u8x16_low(cast::<[u64; 2], u8x16>([packed, 0])))
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "the fixed-point products are bounded by centered u8 JPEG samples"
)]
#[cfg_attr(not(coverage), inline(always))]
fn ycc_to_rgb_standard_pixel(y: u8, cb: u8, cr: u8) -> (u8, u8, u8) {
    let y = i32::from(y);
    let cb = i32::from(cb) - 128;
    let cr = i32::from(cr) - 128;
    let red = (y + ((91_881 * cr + 32_768) >> 16)).clamp(0, 255);
    let green = (y + ((-22_554 * cb - 46_802 * cr + 32_768) >> 16)).clamp(0, 255);
    let blue = (y + ((116_130 * cb + 32_768) >> 16)).clamp(0, 255);
    (
        red.to_le_bytes()[0],
        green.to_le_bytes()[0],
        blue.to_le_bytes()[0],
    )
}

#[cfg_attr(not(coverage), inline(always))]
pub(crate) fn ycc_to_rgb_pixel(
    y: u8,
    cb: u8,
    cr: u8,
    cr_r_tab: &[i32; 256],
    cb_b_tab: &[i32; 256],
    cr_g_tab: &[i32; 256],
    cb_g_tab: &[i32; 256],
) -> (u8, u8, u8) {
    let y = i32::from(y);
    let red = y.saturating_add(cr_r_tab[usize::from(cr)]).clamp(0, 255);
    let green = y
        .saturating_add(
            cb_g_tab[usize::from(cb)]
                .saturating_add(cr_g_tab[usize::from(cr)])
                .wrapping_shr(16),
        )
        .clamp(0, 255);
    let blue = y.saturating_add(cb_b_tab[usize::from(cb)]).clamp(0, 255);
    (
        red.to_le_bytes()[0],
        green.to_le_bytes()[0],
        blue.to_le_bytes()[0],
    )
}

/// Exercise the empty-input and partially aligned tails of the fused 4:2:0
/// color kernel. These are internal buffer contracts: public image validation
/// rejects zero-sized images, while the kernel itself remains total for safe
/// callers that use it as a row-building primitive.
#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let (y, cb, cr) = rgb_to_ycbcr_420_batch(&[], 0, 0);
    assert_eq!(y.len(), 0);
    assert_eq!(cb.len(), 0);
    assert_eq!(cr.len(), 0);

    let _ = rgb_to_ycbcr_420_batch(&[], 1, 0);
    let _ = rgb_to_ycbcr_420_batch(&[], 0, 1);

    let pixels = vec![0u8; 16 * 3];
    let (y, cb, cr) = rgb_to_ycbcr_420_batch(&pixels, 16, 1);
    assert_eq!(y.len(), 16);
    assert_eq!(cb.len(), 8);
    assert_eq!(cr.len(), 8);
}

#[cfg(test)]
mod tests {
    use super::{
        rgb_to_ycbcr_420_batch, rgb_to_ycbcr_batch, rgb_to_ycbcr_pixel, ycc_to_rgb_batch,
        ycc_to_rgb_pixel,
    };

    #[test]
    fn rgb_batch_matches_pixel_reference_with_tail() {
        let pixels: Vec<u8> = (0u8..=71).collect();
        let (y, cb, cr) = rgb_to_ycbcr_batch(&pixels, 8, 3);
        for pixel in 0..24 {
            let source = pixel * 3;
            assert_eq!(
                (y[pixel], cb[pixel], cr[pixel]),
                rgb_to_ycbcr_pixel(pixels[source], pixels[source + 1], pixels[source + 2])
            );
        }
    }

    #[test]
    fn fused_rgb_420_matches_split_reference_with_edges() {
        for (width, height) in [(1usize, 1usize), (17, 13), (32, 16)] {
            let pixels: Vec<u8> = (0..width.saturating_mul(height).saturating_mul(3))
                .map(|index| index.wrapping_mul(37).wrapping_add(11).to_le_bytes()[0])
                .collect();
            let (reference_y, reference_cb, reference_cr) =
                rgb_to_ycbcr_batch(&pixels, width, height);
            let (actual_y, actual_cb, actual_cr) = rgb_to_ycbcr_420_batch(&pixels, width, height);
            assert_eq!(actual_y, reference_y);

            let chroma_width = width.div_ceil(16).saturating_mul(8);
            let chroma_height = height.div_ceil(2);
            let mut expected_cb = Vec::with_capacity(chroma_width.saturating_mul(chroma_height));
            let mut expected_cr = Vec::with_capacity(chroma_width.saturating_mul(chroma_height));
            for chroma_y in 0..chroma_height {
                let row0 = chroma_y.saturating_mul(2).min(height - 1);
                let row1 = row0.saturating_add(1).min(height - 1);
                for chroma_x in 0..chroma_width {
                    let x0 = chroma_x.saturating_mul(2).min(width - 1);
                    let x1 = x0.saturating_add(1).min(width - 1);
                    let bias = u32::from(chroma_x.to_le_bytes()[0] & 1).saturating_add(1);
                    expected_cb.push(
                        (u32::from(reference_cb[row0 * width + x0])
                            + u32::from(reference_cb[row0 * width + x1])
                            + u32::from(reference_cb[row1 * width + x0])
                            + u32::from(reference_cb[row1 * width + x1])
                            + bias)
                            .wrapping_shr(2)
                            .to_le_bytes()[0],
                    );
                    expected_cr.push(
                        (u32::from(reference_cr[row0 * width + x0])
                            + u32::from(reference_cr[row0 * width + x1])
                            + u32::from(reference_cr[row1 * width + x0])
                            + u32::from(reference_cr[row1 * width + x1])
                            + bias)
                            .wrapping_shr(2)
                            .to_le_bytes()[0],
                    );
                }
            }
            assert_eq!(actual_cb, expected_cb);
            assert_eq!(actual_cr, expected_cr);
        }
    }

    #[test]
    fn ycc_batch_matches_pixel_reference_with_tail() {
        let mut cr_r_tab = [0i32; 256];
        let mut cb_b_tab = [0i32; 256];
        let mut cr_g_tab = [0i32; 256];
        let mut cb_g_tab = [0i32; 256];
        for index in 0..256 {
            let centered = i32::try_from(index).unwrap_or(0).saturating_sub(128);
            cr_r_tab[index] = 91_881i32
                .saturating_mul(centered)
                .saturating_add(32_768)
                .wrapping_shr(16);
            cb_b_tab[index] = 116_130i32
                .saturating_mul(centered)
                .saturating_add(32_768)
                .wrapping_shr(16);
            cr_g_tab[index] = (-46_802i32).saturating_mul(centered);
            cb_g_tab[index] = (-22_554i32).saturating_mul(centered).saturating_add(32_768);
        }

        let y: Vec<u8> = (0u8..=23).collect();
        let cb: Vec<u8> = (32u8..=55).collect();
        let cr: Vec<u8> = (64u8..=87).collect();
        let mut actual = vec![0u8; y.len().saturating_mul(3)];
        ycc_to_rgb_batch(&y, &cb, &cr, &mut actual);

        for pixel in 0..y.len() {
            let expected = ycc_to_rgb_pixel(
                y[pixel], cb[pixel], cr[pixel], &cr_r_tab, &cb_b_tab, &cr_g_tab, &cb_g_tab,
            );
            let destination = pixel.saturating_mul(3);
            assert_eq!(
                &actual[destination..destination.saturating_add(3)],
                &[expected.0, expected.1, expected.2]
            );
        }
    }
}
