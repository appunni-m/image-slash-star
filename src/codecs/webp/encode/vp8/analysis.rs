//! Macroblock susceptibility analysis used by libwebp's lossy VP8 encoder.

use super::dct::vp8_fdct_4x4;
use super::quant::Y_AC_QUANT;

const MAX_ALPHA: usize = 255;
const NUM_SEGMENTS: usize = 4;
const MAX_K_MEANS_ITERATIONS: usize = 6;
const MAX_COEFFICIENT_THRESHOLD: usize = 31;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct MacroblockAnalysis {
    pub(super) alpha: u8,
    pub(super) segment: u8,
    pub(super) use_intra4: bool,
    pub(super) luma_mode: u8,
    pub(super) chroma_mode: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SegmentAnalysis {
    pub(super) alpha: i32,
    pub(super) beta: i32,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct FrameAnalysis {
    pub(super) alpha: i32,
    pub(super) chroma_alpha: i32,
    pub(super) macroblocks: Vec<MacroblockAnalysis>,
    pub(super) segments: [SegmentAnalysis; NUM_SEGMENTS],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SegmentParams {
    pub(super) quantizer: u8,
    pub(super) filter_strength: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct FrameParams {
    pub(super) segments: [SegmentParams; NUM_SEGMENTS],
    pub(super) num_segments: usize,
    pub(super) chroma_dc_delta: i8,
    pub(super) chroma_ac_delta: i8,
}

#[derive(Clone, Copy)]
struct Histogram {
    max_value: i32,
    last_non_zero: i32,
}

fn predict_block<const SIZE: usize>(
    top: Option<&[u8]>,
    left: Option<&[u8]>,
    top_left: u8,
    mode: u8,
) -> Vec<u8> {
    let mut output = vec![0; SIZE.wrapping_mul(SIZE)];
    let size_u32 = u32::from(SIZE.to_le_bytes()[0]);
    let denominator = size_u32.wrapping_mul(2);
    match mode {
        0 => {
            let value = match (top, left) {
                (Some(top), Some(left)) => {
                    let sum: u32 = top
                        .iter()
                        .take(SIZE)
                        .chain(left.iter().take(SIZE))
                        .fold(0_u32, |sum, &value| sum.wrapping_add(u32::from(value)));
                    sum.wrapping_add(size_u32)
                        .checked_div(denominator)
                        .unwrap_or_default()
                        .to_le_bytes()[0]
                }
                (Some(top), None) => {
                    let sum = top
                        .iter()
                        .take(SIZE)
                        .fold(0_u32, |sum, &value| sum.wrapping_add(u32::from(value)));
                    sum.wrapping_mul(2)
                        .wrapping_add(size_u32)
                        .checked_div(denominator)
                        .unwrap_or_default()
                        .to_le_bytes()[0]
                }
                (None, Some(left)) => {
                    let sum = left
                        .iter()
                        .take(SIZE)
                        .fold(0_u32, |sum, &value| sum.wrapping_add(u32::from(value)));
                    sum.wrapping_mul(2)
                        .wrapping_add(size_u32)
                        .checked_div(denominator)
                        .unwrap_or_default()
                        .to_le_bytes()[0]
                }
                (None, None) => 128,
            };
            output.fill(value);
        }
        _ => {
            debug_assert_eq!(mode, 1);
            match (top, left) {
                (Some(top), Some(left)) => {
                    for row in 0..SIZE {
                        for column in 0..SIZE {
                            output[row.wrapping_mul(SIZE).wrapping_add(column)] =
                                i16::from(top[column])
                                    .wrapping_add(i16::from(left[row]))
                                    .wrapping_sub(i16::from(top_left))
                                    .clamp(0, 255)
                                    .to_le_bytes()[0];
                        }
                    }
                }
                (Some(top), None) => {
                    for row in output.chunks_exact_mut(SIZE) {
                        row.copy_from_slice(&top[..SIZE]);
                    }
                }
                (None, Some(left)) => {
                    for (row, &value) in output.chunks_exact_mut(SIZE).zip(left.iter()) {
                        row.fill(value);
                    }
                }
                (None, None) => output.fill(129),
            }
        }
    }
    output
}

fn collect_histogram(blocks: &[(&[u8], &[u8], usize)]) -> Histogram {
    let mut distribution = [0i32; MAX_COEFFICIENT_THRESHOLD + 1];
    for &(source, prediction, stride) in blocks {
        for block_y in 0..stride / 4 {
            for block_x in 0..stride / 4 {
                let mut residual = [0i16; 16];
                for row in 0_usize..4 {
                    for column in 0_usize..4 {
                        let index = block_y
                            .wrapping_mul(4)
                            .wrapping_add(row)
                            .wrapping_mul(stride)
                            .wrapping_add(block_x.wrapping_mul(4))
                            .wrapping_add(column);
                        residual[row.wrapping_mul(4).wrapping_add(column)] =
                            i16::from(source[index]).wrapping_sub(i16::from(prediction[index]));
                    }
                }
                for coefficient in vp8_fdct_4x4(&residual) {
                    let bin = (usize::from(coefficient.unsigned_abs()) >> 3)
                        .min(MAX_COEFFICIENT_THRESHOLD);
                    distribution[bin] = distribution[bin].wrapping_add(1);
                }
            }
        }
    }

    let mut histogram = Histogram {
        max_value: 0,
        last_non_zero: 1,
    };
    for (bin, &count) in distribution.iter().enumerate() {
        if count > 0 {
            histogram.max_value = histogram.max_value.max(count);
            histogram.last_non_zero = i32::from(bin.to_le_bytes()[0]);
        }
    }
    histogram
}

fn histogram_alpha(histogram: Histogram) -> i32 {
    debug_assert!(histogram.max_value > 1);
    510_i32
        .wrapping_mul(histogram.last_non_zero)
        .checked_div(histogram.max_value)
        .unwrap_or_default()
}

fn extract_block(
    plane: &[u8],
    stride: usize,
    width: usize,
    height: usize,
    origin_x: usize,
    origin_y: usize,
    size: usize,
) -> Vec<u8> {
    let mut output = vec![0; size.wrapping_mul(size)];
    for row in 0..size {
        let source_y = origin_y.wrapping_add(row).min(height.saturating_sub(1));
        for column in 0..size {
            let source_x = origin_x.wrapping_add(column).min(width.saturating_sub(1));
            output[row.wrapping_mul(size).wrapping_add(column)] =
                plane[source_y.wrapping_mul(stride).wrapping_add(source_x)];
        }
    }
    output
}

fn boundary(
    plane: &[u8],
    stride: usize,
    width: usize,
    height: usize,
    origin_x: usize,
    origin_y: usize,
    size: usize,
) -> (Option<Vec<u8>>, Option<Vec<u8>>, u8) {
    let top = (origin_y > 0).then(|| {
        (0..size)
            .map(|column| {
                plane[origin_y
                    .wrapping_sub(1)
                    .wrapping_mul(stride)
                    .wrapping_add(origin_x.wrapping_add(column).min(width.saturating_sub(1)))]
            })
            .collect()
    });
    let left = (origin_x > 0).then(|| {
        (0..size)
            .map(|row| {
                plane[origin_y
                    .wrapping_add(row)
                    .min(height.saturating_sub(1))
                    .wrapping_mul(stride)
                    .wrapping_add(origin_x)
                    .wrapping_sub(1)]
            })
            .collect()
    });
    let top_left = if origin_x > 0 && origin_y > 0 {
        plane[origin_y
            .wrapping_sub(1)
            .wrapping_mul(stride)
            .wrapping_add(origin_x)
            .wrapping_sub(1)]
    } else if origin_y > 0 {
        129
    } else {
        127
    };
    (top, left, top_left)
}

fn assign_segments(
    macroblocks: &mut [MacroblockAnalysis],
    alpha_counts: &[i32; MAX_ALPHA + 1],
) -> [SegmentAnalysis; NUM_SEGMENTS] {
    let minimum = alpha_counts
        .iter()
        .position(|&count| count != 0)
        .unwrap_or(0);
    let maximum = alpha_counts
        .iter()
        .rposition(|&count| count != 0)
        .unwrap_or(minimum);
    let range = maximum.saturating_sub(minimum);
    let mut centers = [0i32; NUM_SEGMENTS];
    for (index, center) in centers.iter_mut().enumerate() {
        let numerator = index.wrapping_mul(2).wrapping_add(1).wrapping_mul(range);
        let denominator = NUM_SEGMENTS.wrapping_mul(2);
        let offset = numerator.checked_div(denominator).unwrap_or_default();
        *center =
            i32::from(minimum.to_le_bytes()[0]).wrapping_add(i32::from(offset.to_le_bytes()[0]));
    }

    let mut map = [0u8; MAX_ALPHA + 1];
    let mut weighted_average = 0_i32;
    for _ in 0..MAX_K_MEANS_ITERATIONS {
        let mut accumulations = [0i32; NUM_SEGMENTS];
        let mut distance_accumulations = [0i32; NUM_SEGMENTS];
        let mut nearest = 0_usize;
        for alpha in minimum..=maximum {
            let count = alpha_counts[alpha];
            if count != 0 {
                let alpha_i32 = i32::from(alpha.to_le_bytes()[0]);
                while nearest.wrapping_add(1) < NUM_SEGMENTS
                    && alpha_i32
                        .wrapping_sub(centers[nearest.wrapping_add(1)])
                        .abs()
                        < alpha_i32.wrapping_sub(centers[nearest]).abs()
                {
                    nearest = nearest.wrapping_add(1);
                }
                map[alpha] = nearest.to_le_bytes()[0];
                distance_accumulations[nearest] =
                    distance_accumulations[nearest].wrapping_add(alpha_i32.wrapping_mul(count));
                accumulations[nearest] = accumulations[nearest].wrapping_add(count);
            }
        }

        let mut displaced = 0_i32;
        let mut weighted_sum = 0_i32;
        let mut total_weight = 0_i32;
        for index in 0..NUM_SEGMENTS {
            if accumulations[index] != 0 {
                let center = distance_accumulations[index]
                    .wrapping_add(accumulations[index] / 2)
                    .checked_div(accumulations[index])
                    .unwrap_or_default();
                displaced = displaced.wrapping_add(centers[index].wrapping_sub(center).abs());
                centers[index] = center;
                weighted_sum = weighted_sum.wrapping_add(center.wrapping_mul(accumulations[index]));
                total_weight = total_weight.wrapping_add(accumulations[index]);
            }
        }
        weighted_average = weighted_sum
            .wrapping_add(total_weight / 2)
            .checked_div(total_weight)
            .unwrap_or_default();
        if displaced < 5 {
            break;
        }
    }

    for macroblock in macroblocks {
        let segment = map[macroblock.alpha as usize];
        macroblock.segment = segment;
        macroblock.alpha = centers[usize::from(segment)].to_le_bytes()[0];
    }

    let minimum_center = *centers.iter().min().unwrap_or(&0);
    let mut maximum_center = *centers.iter().max().unwrap_or(&0);
    if maximum_center == minimum_center {
        maximum_center = minimum_center.wrapping_add(1);
    }
    let center_range = maximum_center.wrapping_sub(minimum_center);
    std::array::from_fn(|index| {
        let alpha = 255_i32
            .wrapping_mul(centers[index].wrapping_sub(weighted_average))
            .checked_div(center_range)
            .unwrap_or_default()
            .clamp(-127, 127);
        let beta = 255_i32
            .wrapping_mul(centers[index].wrapping_sub(minimum_center))
            .checked_div(center_range)
            .unwrap_or_default()
            .clamp(0, 255);
        SegmentAnalysis { alpha, beta }
    })
}

pub(super) fn analyze(
    y_plane: &[u8],
    u_plane: &[u8],
    v_plane: &[u8],
    width: usize,
    height: usize,
    quality: u8,
    method: u8,
) -> FrameAnalysis {
    let chroma_width = width.div_ceil(2);
    let chroma_height = height.div_ceil(2);
    let macroblock_width = width.div_ceil(16);
    let macroblock_height = height.div_ceil(16);
    let mut macroblocks = Vec::with_capacity(macroblock_width.wrapping_mul(macroblock_height));
    let mut alpha_counts = [0i32; MAX_ALPHA + 1];
    let mut alpha_sum = 0_i32;
    let mut chroma_alpha_sum = 0_i32;

    for macroblock_y in 0..macroblock_height {
        for macroblock_x in 0..macroblock_width {
            let y_x = macroblock_x.wrapping_mul(16);
            let y_y = macroblock_y.wrapping_mul(16);
            let y_block = extract_block(y_plane, width, width, height, y_x, y_y, 16);
            let (y_top, y_left, y_top_left) = boundary(y_plane, width, width, height, y_x, y_y, 16);

            let (best_luma_alpha, luma_mode, use_intra4) = if method <= 1 {
                let strip_sums = std::array::from_fn::<_, 16, _>(|strip| {
                    let block_x = strip % 4;
                    let block_y = strip / 4;
                    (0..4)
                        .flat_map(|row| {
                            let offset = block_y
                                .wrapping_mul(4)
                                .wrapping_add(row)
                                .wrapping_mul(16)
                                .wrapping_add(block_x.wrapping_mul(4));
                            &y_block[offset..][..4]
                        })
                        .map(|&value| u32::from(value))
                        .sum::<u32>()
                });
                let mean = strip_sums.iter().sum::<u32>();
                let squared_mean = strip_sums.iter().fold(0_u32, |sum, &value| {
                    sum.wrapping_add(value.wrapping_mul(value))
                });
                let threshold =
                    8_u32.wrapping_add(9_u32.wrapping_mul(u32::from(quality)).wrapping_div(100));
                (
                    0,
                    0,
                    threshold.wrapping_mul(squared_mean) >= mean.wrapping_mul(mean),
                )
            } else {
                let mut best_luma_alpha = -1;
                let mut luma_mode = 0;
                for mode in 0..2 {
                    let prediction =
                        predict_block::<16>(y_top.as_deref(), y_left.as_deref(), y_top_left, mode);
                    let alpha = histogram_alpha(collect_histogram(&[(&y_block, &prediction, 16)]));
                    if alpha > best_luma_alpha {
                        best_luma_alpha = alpha;
                        luma_mode = mode;
                    }
                }
                (best_luma_alpha, luma_mode, false)
            };

            let uv_x = macroblock_x.wrapping_mul(8);
            let uv_y = macroblock_y.wrapping_mul(8);
            let u_block = extract_block(
                u_plane,
                chroma_width,
                chroma_width,
                chroma_height,
                uv_x,
                uv_y,
                8,
            );
            let v_block = extract_block(
                v_plane,
                chroma_width,
                chroma_width,
                chroma_height,
                uv_x,
                uv_y,
                8,
            );
            let (u_top, u_left, u_top_left) = boundary(
                u_plane,
                chroma_width,
                chroma_width,
                chroma_height,
                uv_x,
                uv_y,
                8,
            );
            let (v_top, v_left, v_top_left) = boundary(
                v_plane,
                chroma_width,
                chroma_width,
                chroma_height,
                uv_x,
                uv_y,
                8,
            );
            let mut best_chroma_alpha = -1;
            let mut smallest_chroma_alpha = 0;
            let mut chroma_mode = 0;
            for mode in 0..2 {
                let u_prediction =
                    predict_block::<8>(u_top.as_deref(), u_left.as_deref(), u_top_left, mode);
                let v_prediction =
                    predict_block::<8>(v_top.as_deref(), v_left.as_deref(), v_top_left, mode);
                let alpha = histogram_alpha(collect_histogram(&[
                    (&u_block, &u_prediction, 8),
                    (&v_block, &v_prediction, 8),
                ]));
                best_chroma_alpha = best_chroma_alpha.max(alpha);
                if mode == 0 || alpha < smallest_chroma_alpha {
                    smallest_chroma_alpha = alpha;
                    chroma_mode = mode;
                }
            }

            let mixed_alpha = 255_i32
                .wrapping_sub(
                    3_i32
                        .wrapping_mul(best_luma_alpha)
                        .wrapping_add(best_chroma_alpha)
                        .wrapping_add(2)
                        .wrapping_shr(2),
                )
                .clamp(0, 255);
            let mixed_alpha_index = usize::from(mixed_alpha.to_le_bytes()[0]);
            alpha_counts[mixed_alpha_index] = alpha_counts[mixed_alpha_index].wrapping_add(1);
            alpha_sum = alpha_sum.wrapping_add(mixed_alpha);
            chroma_alpha_sum = chroma_alpha_sum.wrapping_add(best_chroma_alpha);
            macroblocks.push(MacroblockAnalysis {
                alpha: mixed_alpha.to_le_bytes()[0],
                segment: 0,
                use_intra4,
                luma_mode,
                chroma_mode,
            });
        }
    }

    let count_bytes = macroblocks.len().to_le_bytes();
    let macroblock_count = i32::from_le_bytes([
        count_bytes[0],
        count_bytes[1],
        count_bytes[2],
        count_bytes[3],
    ]);
    let alpha = alpha_sum.checked_div(macroblock_count).unwrap_or_default();
    let chroma_alpha = chroma_alpha_sum
        .checked_div(macroblock_count)
        .unwrap_or_default();
    let segments = assign_segments(&mut macroblocks, &alpha_counts);
    FrameAnalysis {
        alpha,
        chroma_alpha,
        macroblocks,
        segments,
    }
}

/// Converts the bounded libwebp quantizer expression with Rust's truncation
/// semantics after the floating-point quality transform.
#[allow(clippy::cast_possible_truncation)]
fn trunc_quantizer(value: f64) -> i32 {
    value as i32
}

pub(super) fn segment_params(analysis: &FrameAnalysis, quality: f64) -> FrameParams {
    let compression = {
        let quality = quality / 100.0;
        let linear = if quality < 0.75 {
            quality * (2.0 / 3.0)
        } else {
            2.0 * quality - 1.0
        };
        linear.powf(1.0 / 3.0)
    };
    let segments = std::array::from_fn(|index| {
        let exponent = 1.0 - (0.9 * 50.0 / 100.0 / 128.0) * analysis.segments[index].alpha as f64;
        let quantizer = trunc_quantizer(127.0 * (1.0 - compression.powf(exponent)));
        let quantizer = quantizer.clamp(0, 127).to_le_bytes()[0];
        let quantizer_step = Y_AC_QUANT[quantizer as usize] >> 2;
        let strength = i32::from(quantizer_step)
            .wrapping_mul(300)
            .checked_div(256_i32.wrapping_add(analysis.segments[index].beta))
            .unwrap_or_default();
        let filter_strength = if strength < 2 {
            0
        } else {
            strength.min(63).to_le_bytes()[0]
        };
        SegmentParams {
            quantizer,
            filter_strength,
        }
    });
    let chroma_ac_value = analysis
        .chroma_alpha
        .wrapping_sub(64)
        .wrapping_mul(10)
        .wrapping_div(70)
        .wrapping_mul(50)
        .wrapping_div(100)
        .clamp(-4, 6);
    let chroma_ac_delta = i8::from_le_bytes([chroma_ac_value.to_le_bytes()[0]]);
    let chroma_dc_value = (-4_i32).wrapping_mul(50).wrapping_div(100).clamp(-15, 15);
    let chroma_dc_delta = i8::from_le_bytes([chroma_dc_value.to_le_bytes()[0]]);
    FrameParams {
        segments,
        num_segments: NUM_SEGMENTS,
        chroma_dc_delta,
        chroma_ac_delta,
    }
}
