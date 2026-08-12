//! Safe, fixed-shape JPEG color kernels.
//!
//! The vectorization workspace identified color conversion as a good first
//! batch boundary: every pixel is independent, while row tails and malformed
//! input remain the caller's responsibility. These kernels deliberately use
//! only safe slices and fixed-size batches. LLVM may auto-vectorize the
//! regular loop in an optimized build, but the codec never relies on a target
//! feature or on undefined behavior for correctness.

use wide::i32x4;

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

/// Convert one RGB sample using the exact libjpeg fixed-point coefficients.
#[inline(always)]
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
#[cfg(any(test, feature = "jpeg-wide-color"))]
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

    let mut pixel = 0usize;
    while pixel.saturating_add(8) <= available {
        convert_eight(pixels, pixel, &mut y, &mut cb, &mut cr);
        pixel = pixel.saturating_add(8);
    }
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

#[cfg(any(test, feature = "jpeg-wide-color"))]
#[inline(always)]
fn convert_eight(pixels: &[u8], start: usize, y: &mut [u8], cb: &mut [u8], cr: &mut [u8]) {
    convert_rgb_to_ycbcr_four(pixels, start, y, cb, cr);
    convert_rgb_to_ycbcr_four(pixels, start.saturating_add(4), y, cb, cr);
}

#[cfg(any(test, feature = "jpeg-wide-color"))]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::too_many_arguments,
    reason = "the wide arithmetic is bounded by the u8 JPEG sample contract"
)]
#[inline(always)]
fn convert_rgb_to_ycbcr_four(
    pixels: &[u8],
    start: usize,
    y: &mut [u8],
    cb: &mut [u8],
    cr: &mut [u8],
) {
    let mut red = [0i32; 4];
    let mut green = [0i32; 4];
    let mut blue = [0i32; 4];
    let mut lane = 0usize;
    while lane < 4 {
        let pixel = start.saturating_add(lane);
        let source = pixel.saturating_mul(3);
        red[lane] = i32::from(pixels[source]);
        green[lane] = i32::from(pixels[source.saturating_add(1)]);
        blue[lane] = i32::from(pixels[source.saturating_add(2)]);
        lane = lane.saturating_add(1);
    }

    let red = i32x4::new(red);
    let green = i32x4::new(green);
    let blue = i32x4::new(blue);
    let y_values = fixed_point_wide(red, Y_R, green, Y_G, blue, Y_B, 32_768);
    let cb_values = fixed_point_wide(red, CB_R, green, CB_G, blue, CB_B, CHROMA_BIAS);
    let cr_values = fixed_point_wide(red, CR_R, green, CR_G, blue, CR_B, CHROMA_BIAS);
    write_four(y, start, y_values);
    write_four(cb, start, cb_values);
    write_four(cr, start, cr_values);
}

#[cfg(any(test, feature = "jpeg-wide-color"))]
#[allow(
    clippy::arithmetic_side_effects,
    reason = "the wide arithmetic is bounded by the u8 JPEG sample contract"
)]
#[inline(always)]
fn fixed_point_wide(
    first: i32x4,
    first_weight: i32,
    second: i32x4,
    second_weight: i32,
    third: i32x4,
    third_weight: i32,
    bias: i32,
) -> i32x4 {
    (first * first_weight + second * second_weight + third * third_weight + bias)
        .unbounded_shr_scalar(16)
}

#[cfg(any(test, feature = "jpeg-wide-color"))]
#[inline(always)]
fn write_four(output: &mut [u8], start: usize, values: i32x4) {
    let values = values.to_array();
    let mut lane = 0usize;
    while lane < 4 {
        output[start.saturating_add(lane)] = values[lane].to_le_bytes()[0];
        lane = lane.saturating_add(1);
    }
}

#[inline(always)]
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

/// Convert one row of YCbCr samples into interleaved RGB output in eight-pixel
/// batches. The input slices may be shorter than a full row only when the
/// caller intentionally supplies a tail; output is sized for the samples that
/// are present.
#[allow(
    clippy::too_many_arguments,
    reason = "the fixed-point tables are the explicit state of the color kernel"
)]
#[inline(never)]
pub(crate) fn ycc_to_rgb_batch(
    y: &[u8],
    cb: &[u8],
    cr: &[u8],
    cr_r_tab: &[i32; 256],
    cb_b_tab: &[i32; 256],
    cr_g_tab: &[i32; 256],
    cb_g_tab: &[i32; 256],
    output: &mut [u8],
) {
    let sample_count = y
        .len()
        .min(cb.len())
        .min(cr.len())
        .min(output.len().div_euclid(3));
    let mut pixel = 0usize;
    while pixel.saturating_add(8) <= sample_count {
        convert_rgb_eight(
            y, cb, cr, cr_r_tab, cb_b_tab, cr_g_tab, cb_g_tab, pixel, output,
        );
        pixel = pixel.saturating_add(8);
    }
    while pixel < sample_count {
        let (red, green, blue) = ycc_to_rgb_pixel(
            y[pixel], cb[pixel], cr[pixel], cr_r_tab, cb_b_tab, cr_g_tab, cb_g_tab,
        );
        let destination = pixel.saturating_mul(3);
        output[destination] = red;
        output[destination.saturating_add(1)] = green;
        output[destination.saturating_add(2)] = blue;
        pixel = pixel.saturating_add(1);
    }
}

#[inline(always)]
#[allow(
    clippy::too_many_arguments,
    reason = "the fixed-point tables are the explicit state of the color kernel"
)]
fn convert_rgb_eight(
    y: &[u8],
    cb: &[u8],
    cr: &[u8],
    cr_r_tab: &[i32; 256],
    cb_b_tab: &[i32; 256],
    cr_g_tab: &[i32; 256],
    cb_g_tab: &[i32; 256],
    start: usize,
    output: &mut [u8],
) {
    convert_ycc_to_rgb_four(
        y, cb, cr, cr_r_tab, cb_b_tab, cr_g_tab, cb_g_tab, start, output,
    );
    convert_ycc_to_rgb_four(
        y,
        cb,
        cr,
        cr_r_tab,
        cb_b_tab,
        cr_g_tab,
        cb_g_tab,
        start.saturating_add(4),
        output,
    );
}

#[allow(
    clippy::arithmetic_side_effects,
    clippy::too_many_arguments,
    reason = "the wide arithmetic is bounded by the u8 JPEG sample contract"
)]
#[inline(always)]
fn convert_ycc_to_rgb_four(
    y: &[u8],
    cb: &[u8],
    cr: &[u8],
    cr_r_tab: &[i32; 256],
    cb_b_tab: &[i32; 256],
    cr_g_tab: &[i32; 256],
    cb_g_tab: &[i32; 256],
    start: usize,
    output: &mut [u8],
) {
    let mut y_values = [0i32; 4];
    let mut lane = 0usize;
    while lane < 4 {
        let pixel = start.saturating_add(lane);
        y_values[lane] = i32::from(y[pixel]);
        lane = lane.saturating_add(1);
    }
    let y_values = i32x4::new(y_values);
    let cb_g = i32x4::new([
        cb_g_tab[usize::from(cb[start])],
        cb_g_tab[usize::from(cb[start.saturating_add(1)])],
        cb_g_tab[usize::from(cb[start.saturating_add(2)])],
        cb_g_tab[usize::from(cb[start.saturating_add(3)])],
    ]);
    let cr_g = i32x4::new([
        cr_g_tab[usize::from(cr[start])],
        cr_g_tab[usize::from(cr[start.saturating_add(1)])],
        cr_g_tab[usize::from(cr[start.saturating_add(2)])],
        cr_g_tab[usize::from(cr[start.saturating_add(3)])],
    ]);
    let red_offset = i32x4::new([
        cr_r_tab[usize::from(cr[start])],
        cr_r_tab[usize::from(cr[start.saturating_add(1)])],
        cr_r_tab[usize::from(cr[start.saturating_add(2)])],
        cr_r_tab[usize::from(cr[start.saturating_add(3)])],
    ]);
    let blue_offset = i32x4::new([
        cb_b_tab[usize::from(cb[start])],
        cb_b_tab[usize::from(cb[start.saturating_add(1)])],
        cb_b_tab[usize::from(cb[start.saturating_add(2)])],
        cb_b_tab[usize::from(cb[start.saturating_add(3)])],
    ]);
    let clamp_min = i32x4::ZERO;
    let clamp_max = i32x4::splat(255);
    let red = (y_values + red_offset)
        .clamp(clamp_min, clamp_max)
        .to_array();
    let green = (y_values + (cb_g + cr_g).unbounded_shr_scalar(16))
        .clamp(clamp_min, clamp_max)
        .to_array();
    let blue = (y_values + blue_offset)
        .clamp(clamp_min, clamp_max)
        .to_array();
    let mut lane = 0usize;
    while lane < 4 {
        let destination = start.saturating_add(lane).saturating_mul(3);
        output[destination] = red[lane].to_le_bytes()[0];
        output[destination.saturating_add(1)] = green[lane].to_le_bytes()[0];
        output[destination.saturating_add(2)] = blue[lane].to_le_bytes()[0];
        lane = lane.saturating_add(1);
    }
}

#[inline(always)]
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

#[cfg(test)]
mod tests {
    use super::{rgb_to_ycbcr_batch, rgb_to_ycbcr_pixel, ycc_to_rgb_batch, ycc_to_rgb_pixel};

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
    fn ycc_batch_matches_pixel_reference_with_tail() {
        let mut cr_r_tab = [0i32; 256];
        let mut cb_b_tab = [0i32; 256];
        let mut cr_g_tab = [0i32; 256];
        let mut cb_g_tab = [0i32; 256];
        for index in 0..256 {
            let centered = i32::try_from(index).unwrap_or(0).saturating_sub(128);
            cr_r_tab[index] = centered.saturating_mul(2);
            cb_b_tab[index] = centered.saturating_mul(3);
            cr_g_tab[index] = centered.saturating_mul(4);
            cb_g_tab[index] = centered.saturating_mul(5);
        }

        let y: Vec<u8> = (0u8..=23).collect();
        let cb: Vec<u8> = (32u8..=55).collect();
        let cr: Vec<u8> = (64u8..=87).collect();
        let mut actual = vec![0u8; y.len().saturating_mul(3)];
        ycc_to_rgb_batch(
            &y,
            &cb,
            &cr,
            &cr_r_tab,
            &cb_b_tab,
            &cr_g_tab,
            &cb_g_tab,
            &mut actual,
        );

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
