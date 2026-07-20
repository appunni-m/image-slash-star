//! Forward VP8L cross-color transform selection.
//!
//! This is a direct safe-Rust port of libwebp 1.6.0's
//! `VP8LColorSpaceTransform` pipeline (`src/enc/predictor_enc.c:844-1111`)
//! and its scalar color kernels (`src/dsp/lossless_enc.c:471-541`).

#[derive(Clone, Copy, Default)]
struct Multipliers {
    green_to_red: u8,
    green_to_blue: u8,
    red_to_blue: u8,
}

#[inline]
fn signed_byte(value: u32) -> i8 {
    value as u8 as i8
}

#[inline]
fn color_delta(multiplier: u8, color: i8) -> i32 {
    (i32::from(multiplier as i8) * i32::from(color)) >> 5
}

#[inline]
fn transformed_red(multiplier: i32, argb: u32) -> u8 {
    let green = signed_byte(argb >> 8);
    ((argb >> 16) as i32 - color_delta(multiplier as u8, green)) as u8
}

#[inline]
fn transformed_blue(green_multiplier: i32, red_multiplier: i32, argb: u32) -> u8 {
    let green = signed_byte(argb >> 8);
    let red = signed_byte(argb >> 16);
    ((argb & 0xff) as i32
        - color_delta(green_multiplier as u8, green)
        - color_delta(red_multiplier as u8, red)) as u8
}

#[inline]
pub(super) fn slog2(value: u32) -> u64 {
    super::backward_refs::fast_slog(value)
}

pub(super) fn combined_shannon_entropy(counts: &[u32; 256], accumulated: &[u32; 256]) -> u64 {
    let mut entropy_terms = 0_u64;
    let mut count_sum = 0_u32;
    let mut combined_sum = 0_u32;
    for (&count, &previous) in counts.iter().zip(accumulated) {
        if count != 0 {
            let combined = count + previous;
            count_sum += count;
            combined_sum += combined;
            entropy_terms += slog2(count) + slog2(combined);
        } else if previous != 0 {
            combined_sum += previous;
            entropy_terms += slog2(previous);
        }
    }
    slog2(count_sum) + slog2(combined_sum) - entropy_terms
}

#[inline]
fn div_round(value: i64, divisor: i64) -> i64 {
    if (value < 0) == (divisor < 0) {
        (value + divisor / 2) / divisor
    } else {
        (value - divisor / 2) / divisor
    }
}

pub(super) fn prediction_bias(counts: &[u32; 256], zero_weight: u64, mut exponential: u64) -> i64 {
    let mut bits = (zero_weight * u64::from(counts[0])) << 23;
    exponential <<= 23;
    for index in 1..16 {
        bits += div_round(
            (exponential * u64::from(counts[index] + counts[256 - index])) as i64,
            100,
        ) as u64;
        exponential = div_round((6 * exponential) as i64, 10) as u64;
    }
    -div_round(bits as i64, 10)
}

fn prediction_cost(counts: &[u32; 256], accumulated: &[u32; 256]) -> i64 {
    combined_shannon_entropy(counts, accumulated) as i64 + prediction_bias(counts, 3, 240)
}

fn collect_red(
    argb: &[u32],
    stride: usize,
    width: usize,
    height: usize,
    multiplier: i32,
) -> [u32; 256] {
    let mut histogram = [0_u32; 256];
    for row in argb.chunks(stride).take(height) {
        for &pixel in &row[..width] {
            histogram[usize::from(transformed_red(multiplier, pixel))] += 1;
        }
    }
    histogram
}

fn collect_blue(
    argb: &[u32],
    stride: usize,
    width: usize,
    height: usize,
    green_multiplier: i32,
    red_multiplier: i32,
) -> [u32; 256] {
    let mut histogram = [0_u32; 256];
    for row in argb.chunks(stride).take(height) {
        for &pixel in &row[..width] {
            histogram[usize::from(transformed_blue(green_multiplier, red_multiplier, pixel))] += 1;
        }
    }
    histogram
}

fn red_cost(
    argb: &[u32],
    stride: usize,
    width: usize,
    height: usize,
    previous_x: Multipliers,
    previous_y: Multipliers,
    multiplier: i32,
    accumulated: &[u32; 256],
) -> i64 {
    let histogram = collect_red(argb, stride, width, height, multiplier);
    let mut cost = prediction_cost(&histogram, accumulated);
    let multiplier = multiplier as u8;
    if multiplier == previous_x.green_to_red {
        cost -= 3_i64 << 23;
    }
    if multiplier == previous_y.green_to_red {
        cost -= 3_i64 << 23;
    }
    if multiplier == 0 {
        cost -= 3_i64 << 23;
    }
    cost
}

fn best_green_to_red(
    argb: &[u32],
    stride: usize,
    width: usize,
    height: usize,
    previous_x: Multipliers,
    previous_y: Multipliers,
    quality: i32,
    accumulated: &[u32; 256],
) -> u8 {
    let iterations = 4 + ((7 * quality) >> 8);
    let mut best = 0_i32;
    let mut best_cost = red_cost(
        argb,
        stride,
        width,
        height,
        previous_x,
        previous_y,
        best,
        accumulated,
    );
    for iteration in 0..iterations {
        let delta = 32 >> iteration;
        for offset in [-delta, delta] {
            let candidate = best + offset;
            let cost = red_cost(
                argb,
                stride,
                width,
                height,
                previous_x,
                previous_y,
                candidate,
                accumulated,
            );
            if cost < best_cost {
                best_cost = cost;
                best = candidate;
            }
        }
    }
    best as u8
}

#[allow(clippy::too_many_arguments)]
fn blue_cost(
    argb: &[u32],
    stride: usize,
    width: usize,
    height: usize,
    previous_x: Multipliers,
    previous_y: Multipliers,
    green_multiplier: i32,
    red_multiplier: i32,
    accumulated: &[u32; 256],
) -> i64 {
    let histogram = collect_blue(
        argb,
        stride,
        width,
        height,
        green_multiplier,
        red_multiplier,
    );
    let mut cost = prediction_cost(&histogram, accumulated);
    let green_multiplier = green_multiplier as u8;
    let red_multiplier = red_multiplier as u8;
    if green_multiplier == previous_x.green_to_blue {
        cost -= 3_i64 << 23;
    }
    if green_multiplier == previous_y.green_to_blue {
        cost -= 3_i64 << 23;
    }
    if red_multiplier == previous_x.red_to_blue {
        cost -= 3_i64 << 23;
    }
    if red_multiplier == previous_y.red_to_blue {
        cost -= 3_i64 << 23;
    }
    if green_multiplier == 0 {
        cost -= 3_i64 << 23;
    }
    if red_multiplier == 0 {
        cost -= 3_i64 << 23;
    }
    cost
}

#[allow(clippy::too_many_arguments)]
fn best_blue_multipliers(
    argb: &[u32],
    stride: usize,
    width: usize,
    height: usize,
    previous_x: Multipliers,
    previous_y: Multipliers,
    quality: i32,
    accumulated: &[u32; 256],
) -> (u8, u8) {
    const OFFSETS: [(i32, i32); 8] = [
        (0, -1),
        (0, 1),
        (-1, 0),
        (1, 0),
        (-1, -1),
        (-1, 1),
        (1, -1),
        (1, 1),
    ];
    const DELTAS: [i32; 7] = [16, 16, 8, 4, 2, 2, 2];
    let iterations = if quality < 25 {
        1
    } else if quality > 50 {
        DELTAS.len()
    } else {
        4
    };
    let mut best_green = 0_i32;
    let mut best_red = 0_i32;
    let mut best_cost = blue_cost(
        argb,
        stride,
        width,
        height,
        previous_x,
        previous_y,
        best_green,
        best_red,
        accumulated,
    );
    for &delta in &DELTAS[..iterations] {
        for &(green_offset, red_offset) in &OFFSETS {
            let green = best_green + green_offset * delta;
            let red = best_red + red_offset * delta;
            let cost = blue_cost(
                argb,
                stride,
                width,
                height,
                previous_x,
                previous_y,
                green,
                red,
                accumulated,
            );
            if cost < best_cost {
                best_cost = cost;
                best_green = green;
                best_red = red;
            }
        }
        if delta == 2 && best_green == 0 && best_red == 0 {
            break;
        }
    }
    (best_green as u8, best_red as u8)
}

fn transform_tile(
    argb: &mut [u32],
    stride: usize,
    width: usize,
    height: usize,
    multipliers: Multipliers,
) {
    for row in argb.chunks_mut(stride).take(height) {
        for pixel in &mut row[..width] {
            let source = *pixel;
            let green = signed_byte(source >> 8);
            let red = signed_byte(source >> 16);
            let new_red = (i32::from(red) & 0xff) - color_delta(multipliers.green_to_red, green);
            let new_blue = (source & 0xff) as i32
                - color_delta(multipliers.green_to_blue, green)
                - color_delta(multipliers.red_to_blue, red);
            *pixel =
                (source & 0xff00_ff00) | ((new_red as u32 & 0xff) << 16) | (new_blue as u32 & 0xff);
        }
    }
}

#[inline]
fn subsample_size(size: usize, bits: u8) -> usize {
    (size + (1_usize << bits) - 1) >> bits
}

pub(super) fn optimize_sampling(
    image: &mut [u32],
    full_width: usize,
    full_height: usize,
    bits: u8,
) -> u8 {
    const MAX_BITS: u8 = 9;
    let original_width = subsample_size(full_width, bits);
    let original_height = subsample_size(full_height, bits);
    let mut best_bits = bits;
    while best_bits < MAX_BITS {
        let next_square = 1_usize << (best_bits + 1 - bits);
        let square = 1_usize << (best_bits - bits);
        let rows_equal = (0..original_height.saturating_sub(square))
            .step_by(next_square)
            .all(|y| {
                image[y * original_width..(y + 1) * original_width]
                    == image[(y + square) * original_width..(y + square + 1) * original_width]
            });
        if !rows_equal {
            break;
        }
        best_bits += 1;
    }
    while best_bits > bits {
        let square = 1_usize << (best_bits - bits);
        let columns_equal = (0..original_height).all(|y| {
            (0..original_width).step_by(square).all(|x| {
                image[y * original_width + x..y * original_width + (x + square).min(original_width)]
                    .iter()
                    .all(|&value| value == image[y * original_width + x])
            })
        });
        if columns_equal {
            break;
        }
        best_bits -= 1;
    }
    if best_bits == bits {
        return bits;
    }
    let square = 1_usize << (best_bits - bits);
    let width = subsample_size(full_width, best_bits);
    let height = subsample_size(full_height, best_bits);
    for y in 0..height {
        for x in 0..width {
            image[y * width + x] = image[square * (y * original_width + x)];
        }
    }
    best_bits
}

/// Selects and applies libwebp's cross-color transform.
#[allow(dead_code)]
pub(crate) fn select_and_apply(
    argb: &mut [u32],
    width: usize,
    height: usize,
    bits: u8,
    quality: i32,
) -> (Vec<u32>, u8) {
    let tile_width = subsample_size(width, bits);
    let tile_height = subsample_size(height, bits);
    let tile_size = 1_usize << bits;
    let mut image = vec![0_u32; tile_width * tile_height];
    let mut accumulated_red = [0_u32; 256];
    let mut accumulated_blue = [0_u32; 256];
    let mut previous_y;
    let mut previous_x = Multipliers::default();
    for tile_y in 0..tile_height {
        previous_y = Multipliers::default();
        for tile_x in 0..tile_width {
            if tile_y != 0 {
                let color = image[(tile_y - 1) * tile_width + tile_x];
                previous_y = Multipliers {
                    green_to_red: color as u8,
                    green_to_blue: (color >> 8) as u8,
                    red_to_blue: (color >> 16) as u8,
                };
            }
            let x = tile_x * tile_size;
            let y = tile_y * tile_size;
            let current_width = tile_size.min(width - x);
            let current_height = tile_size.min(height - y);
            let tile = &argb[y * width + x..];
            let green_to_red = best_green_to_red(
                tile,
                width,
                current_width,
                current_height,
                previous_x,
                previous_y,
                quality,
                &accumulated_red,
            );
            let (green_to_blue, red_to_blue) = best_blue_multipliers(
                tile,
                width,
                current_width,
                current_height,
                previous_x,
                previous_y,
                quality,
                &accumulated_blue,
            );
            previous_x = Multipliers {
                green_to_red,
                green_to_blue,
                red_to_blue,
            };
            image[tile_y * tile_width + tile_x] = 0xff00_0000
                | (u32::from(red_to_blue) << 16)
                | (u32::from(green_to_blue) << 8)
                | u32::from(green_to_red);
            transform_tile(
                &mut argb[y * width + x..],
                width,
                current_width,
                current_height,
                previous_x,
            );
            for row_y in y..y + current_height {
                let row_start = row_y * width + x;
                for index in row_start..row_start + current_width {
                    let pixel = argb[index];
                    if index >= 2 && pixel == argb[index - 2] && pixel == argb[index - 1] {
                        continue;
                    }
                    if index >= width + 2
                        && argb[index - 2] == argb[index - width - 2]
                        && argb[index - 1] == argb[index - width - 1]
                        && pixel == argb[index - width]
                    {
                        continue;
                    }
                    accumulated_red[((pixel >> 16) & 0xff) as usize] += 1;
                    accumulated_blue[(pixel & 0xff) as usize] += 1;
                }
            }
        }
    }
    let best_bits = optimize_sampling(&mut image, width, height, bits);
    image.truncate(subsample_size(width, best_bits) * subsample_size(height, best_bits));
    (image, best_bits)
}
