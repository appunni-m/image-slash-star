//! Forward VP8L spatial predictor selection.
//!
//! This is the lossless method-four path through libwebp 1.6.0's
//! `VP8LResidualImage` (`src/enc/predictor_enc.c:34-840`). The pinned Pillow
//! profile has one initial sampling (`bits = 3`), reducing the general routine
//! to per-tile mode selection plus uniform-map sampling optimization.

use super::cross_color::{combined_shannon_entropy, optimize_sampling, prediction_bias};

const ARGB_BLACK: u32 = 0xff00_0000;
const MODE_COUNT: usize = 14;
const HISTOGRAM_SIZE: usize = 4 * 256;

#[inline]
fn average2(a: u32, b: u32) -> u32 {
    (((a ^ b) & 0xfefe_fefe) >> 1).wrapping_add(a & b)
}

#[inline]
fn average3(a: u32, b: u32, c: u32) -> u32 {
    average2(average2(a, c), b)
}

#[inline]
fn average4(a: u32, b: u32, c: u32, d: u32) -> u32 {
    average2(average2(a, b), average2(c, d))
}

#[inline]
fn clip(value: i32) -> u8 {
    value.clamp(0, 255) as u8
}

fn clamped_add_subtract_full(left: u32, top: u32, top_left: u32) -> u32 {
    let left = left.to_le_bytes();
    let top = top.to_le_bytes();
    let top_left = top_left.to_le_bytes();
    u32::from_le_bytes(core::array::from_fn(|index| {
        clip(i32::from(left[index]) + i32::from(top[index]) - i32::from(top_left[index]))
    }))
}

fn clamped_add_subtract_half(left: u32, top: u32, top_left: u32) -> u32 {
    let average = average2(left, top).to_le_bytes();
    let top_left = top_left.to_le_bytes();
    u32::from_le_bytes(core::array::from_fn(|index| {
        let average = i32::from(average[index]);
        clip(average + (average - i32::from(top_left[index])) / 2)
    }))
}

fn select(top: u32, left: u32, top_left: u32) -> u32 {
    let top_bytes = top.to_le_bytes();
    let left_bytes = left.to_le_bytes();
    let top_left_bytes = top_left.to_le_bytes();
    let score = (0..4).fold(0_i32, |score, index| {
        let center = i32::from(top_left_bytes[index]);
        score + (i32::from(left_bytes[index]) - center).abs()
            - (i32::from(top_bytes[index]) - center).abs()
    });
    if score <= 0 { top } else { left }
}

#[inline]
fn predict(mode: usize, left: u32, top_left: u32, top: u32, top_right: u32) -> u32 {
    match mode {
        1 => left,
        2 => top,
        3 => top_right,
        4 => top_left,
        5 => average3(left, top, top_right),
        6 => average2(left, top_left),
        7 => average2(left, top),
        8 => average2(top_left, top),
        9 => average2(top, top_right),
        10 => average4(left, top_left, top, top_right),
        11 => select(top, left, top_left),
        12 => clamped_add_subtract_full(left, top, top_left),
        13 => clamped_add_subtract_half(left, top, top_left),
        _ => ARGB_BLACK,
    }
}

#[inline]
fn subtract_pixels(pixel: u32, prediction: u32) -> u32 {
    let pixel = pixel.to_le_bytes();
    let prediction = prediction.to_le_bytes();
    u32::from_le_bytes(core::array::from_fn(|index| {
        pixel[index].wrapping_sub(prediction[index])
    }))
}

#[inline]
fn update_histogram(histogram: &mut [u32; HISTOGRAM_SIZE], pixel: u32) {
    histogram[(pixel >> 24) as usize] += 1;
    histogram[256 + ((pixel >> 16) & 0xff) as usize] += 1;
    histogram[512 + ((pixel >> 8) & 0xff) as usize] += 1;
    histogram[768 + (pixel & 0xff) as usize] += 1;
}

#[allow(clippy::too_many_arguments)]
fn tile_histogram(
    source: &[u32],
    width: usize,
    height: usize,
    start_x: usize,
    start_y: usize,
    tile_size: usize,
    mode: usize,
) -> [u32; HISTOGRAM_SIZE] {
    let end_x = (start_x + tile_size).min(width);
    let end_y = (start_y + tile_size).min(height);
    let mut histogram = [0_u32; HISTOGRAM_SIZE];
    let mut upper = vec![0_u32; width + 1];
    let mut current = vec![0_u32; width + 1];
    if start_y > 0 {
        upper[..width].copy_from_slice(&source[(start_y - 1) * width..start_y * width]);
        if start_y < height {
            upper[width] = source[start_y * width];
        }
    }
    for y in start_y..end_y {
        current[..width].copy_from_slice(&source[y * width..(y + 1) * width]);
        current[width] = if y + 1 < height {
            source[(y + 1) * width]
        } else {
            0
        };
        for x in start_x..end_x {
            let prediction = if y == 0 {
                if x == 0 { ARGB_BLACK } else { current[x - 1] }
            } else if x == 0 {
                upper[0]
            } else {
                predict(mode, current[x - 1], upper[x - 1], upper[x], upper[x + 1])
            };
            let mut residual = subtract_pixels(current[x], prediction);
            if current[x] >> 24 == 0 {
                residual &= 0xff00_0000;
                current[x] = prediction & 0x00ff_ffff;
                if x == 0 && y != 0 {
                    upper[width] = current[0];
                }
            }
            update_histogram(&mut histogram, residual);
        }
        std::mem::swap(&mut upper, &mut current);
    }
    histogram
}

fn spatial_cost(
    accumulated: &[u32; HISTOGRAM_SIZE],
    tile: &[u32; HISTOGRAM_SIZE],
    mode: usize,
    left_mode: Option<usize>,
    above_mode: Option<usize>,
) -> i64 {
    let mut cost = 0_i64;
    for plane in 0..4 {
        let tile_plane: &[u32; 256] = tile[plane * 256..(plane + 1) * 256].try_into().unwrap();
        let accumulated_plane: &[u32; 256] = accumulated[plane * 256..(plane + 1) * 256]
            .try_into()
            .unwrap();
        cost += prediction_bias(tile_plane, 1, 94);
        cost += combined_shannon_entropy(tile_plane, accumulated_plane) as i64;
    }
    if left_mode == Some(mode) {
        cost -= 15_i64 << 23;
    }
    if above_mode == Some(mode) {
        cost -= 15_i64 << 23;
    }
    cost
}

fn apply_modes(source: &mut [u32], width: usize, height: usize, bits: u8, modes: &[u32]) {
    let tiles_per_row = (width + (1_usize << bits) - 1) >> bits;
    let original = source.to_vec();
    let mut upper = vec![0_u32; width + 1];
    let mut current = vec![0_u32; width + 1];
    for y in 0..height {
        current[..width].copy_from_slice(&original[y * width..(y + 1) * width]);
        current[width] = if y + 1 < height {
            original[(y + 1) * width]
        } else {
            0
        };
        for x in 0..width {
            let mode = ((modes[(y >> bits) * tiles_per_row + (x >> bits)] >> 8) & 0xff) as usize;
            let prediction = if y == 0 {
                if x == 0 { ARGB_BLACK } else { current[x - 1] }
            } else if x == 0 {
                upper[0]
            } else {
                predict(mode, current[x - 1], upper[x - 1], upper[x], upper[x + 1])
            };
            let mut residual = subtract_pixels(current[x], prediction);
            if current[x] >> 24 == 0 {
                residual &= 0xff00_0000;
                current[x] = prediction & 0x00ff_ffff;
                if x == 0 && y != 0 {
                    upper[width] = current[0];
                }
            }
            source[y * width + x] = residual;
        }
        std::mem::swap(&mut upper, &mut current);
    }
}

/// Selects and applies libwebp's predictor transform for Pillow's method-four
/// lossless profile.
#[allow(dead_code)]
pub(crate) fn select_and_apply(
    source: &mut [u32],
    width: usize,
    height: usize,
    bits: u8,
) -> (Vec<u32>, u8) {
    let tile_size = 1_usize << bits;
    let tiles_per_row = (width + tile_size - 1) >> bits;
    let tiles_per_column = (height + tile_size - 1) >> bits;
    let mut modes = vec![ARGB_BLACK; tiles_per_row * tiles_per_column];
    let mut accumulated = [0_u32; HISTOGRAM_SIZE];
    for tile_y in 0..tiles_per_column {
        for tile_x in 0..tiles_per_row {
            let left_mode = (tile_x > 0)
                .then(|| ((modes[tile_y * tiles_per_row + tile_x - 1] >> 8) & 0xff) as usize);
            let above_mode = (tile_y > 0)
                .then(|| ((modes[(tile_y - 1) * tiles_per_row + tile_x] >> 8) & 0xff) as usize);
            let mut best_mode = 0;
            let mut best_cost = i64::MAX;
            let mut best_histogram = [0_u32; HISTOGRAM_SIZE];
            for mode in 0..MODE_COUNT {
                let histogram = tile_histogram(
                    source,
                    width,
                    height,
                    tile_x * tile_size,
                    tile_y * tile_size,
                    tile_size,
                    mode,
                );
                let cost = spatial_cost(&accumulated, &histogram, mode, left_mode, above_mode);
                if cost < best_cost {
                    best_cost = cost;
                    best_mode = mode;
                    best_histogram = histogram;
                }
            }
            for (total, selected) in accumulated.iter_mut().zip(best_histogram) {
                *total += selected;
            }
            modes[tile_y * tiles_per_row + tile_x] = ARGB_BLACK | ((best_mode as u32) << 8);
        }
    }
    apply_modes(source, width, height, bits, &modes);
    let best_bits = optimize_sampling(&mut modes, width, height, bits);
    modes.truncate(
        ((width + (1 << best_bits) - 1) >> best_bits)
            * ((height + (1 << best_bits) - 1) >> best_bits),
    );
    (modes, best_bits)
}

pub(crate) fn apply_fixed(
    source: &mut [u32],
    width: usize,
    height: usize,
    bits: u8,
    mode: usize,
) -> (Vec<u32>, u8) {
    let tile_size = 1_usize << bits;
    let tiles_per_row = (width + tile_size - 1) >> bits;
    let tiles_per_column = (height + tile_size - 1) >> bits;
    let mut modes = vec![ARGB_BLACK | ((mode as u32) << 8); tiles_per_row * tiles_per_column];
    apply_modes(source, width, height, bits, &modes);
    let best_bits = optimize_sampling(&mut modes, width, height, bits);
    modes.truncate(
        ((width + (1 << best_bits) - 1) >> best_bits)
            * ((height + (1 << best_bits) - 1) >> best_bits),
    );
    (modes, best_bits)
}
