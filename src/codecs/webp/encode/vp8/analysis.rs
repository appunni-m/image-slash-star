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
    let mut output = vec![0; SIZE * SIZE];
    match mode {
        0 => {
            let value = match (top, left) {
                (Some(top), Some(left)) => {
                    let sum: u32 = top
                        .iter()
                        .take(SIZE)
                        .chain(left.iter().take(SIZE))
                        .map(|&v| u32::from(v))
                        .sum();
                    ((sum + SIZE as u32) / (2 * SIZE) as u32) as u8
                }
                (Some(top), None) => {
                    let sum: u32 = top.iter().take(SIZE).map(|&v| u32::from(v)).sum();
                    ((2 * sum + SIZE as u32) / (2 * SIZE) as u32) as u8
                }
                (None, Some(left)) => {
                    let sum: u32 = left.iter().take(SIZE).map(|&v| u32::from(v)).sum();
                    ((2 * sum + SIZE as u32) / (2 * SIZE) as u32) as u8
                }
                (None, None) => 128,
            };
            output.fill(value);
        }
        1 => match (top, left) {
            (Some(top), Some(left)) => {
                for row in 0..SIZE {
                    for column in 0..SIZE {
                        output[row * SIZE + column] =
                            (i16::from(top[column]) + i16::from(left[row]) - i16::from(top_left))
                                .clamp(0, 255) as u8;
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
        },
        _ => unreachable!("analysis only evaluates DC and true-motion modes"),
    }
    output
}

fn collect_histogram(blocks: &[(&[u8], &[u8], usize)]) -> Histogram {
    let mut distribution = [0i32; MAX_COEFFICIENT_THRESHOLD + 1];
    for &(source, prediction, stride) in blocks {
        for block_y in 0..stride / 4 {
            for block_x in 0..stride / 4 {
                let mut residual = [0i16; 16];
                for row in 0..4 {
                    for column in 0..4 {
                        let index = (block_y * 4 + row) * stride + block_x * 4 + column;
                        residual[row * 4 + column] =
                            i16::from(source[index]) - i16::from(prediction[index]);
                    }
                }
                for coefficient in vp8_fdct_4x4(&residual) {
                    let bin = (usize::from(coefficient.unsigned_abs()) >> 3)
                        .min(MAX_COEFFICIENT_THRESHOLD);
                    distribution[bin] += 1;
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
            histogram.last_non_zero = bin as i32;
        }
    }
    histogram
}

fn histogram_alpha(histogram: Histogram) -> i32 {
    if histogram.max_value > 1 {
        510 * histogram.last_non_zero / histogram.max_value
    } else {
        0
    }
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
    let mut output = vec![0; size * size];
    for row in 0..size {
        let source_y = (origin_y + row).min(height - 1);
        for column in 0..size {
            let source_x = (origin_x + column).min(width - 1);
            output[row * size + column] = plane[source_y * stride + source_x];
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
            .map(|column| plane[(origin_y - 1) * stride + (origin_x + column).min(width - 1)])
            .collect()
    });
    let left = (origin_x > 0).then(|| {
        (0..size)
            .map(|row| plane[(origin_y + row).min(height - 1) * stride + origin_x - 1])
            .collect()
    });
    let top_left = if origin_x > 0 && origin_y > 0 {
        plane[(origin_y - 1) * stride + origin_x - 1]
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
    let range = maximum - minimum;
    let mut centers = [0i32; NUM_SEGMENTS];
    for (index, center) in centers.iter_mut().enumerate() {
        *center = minimum as i32 + ((2 * index + 1) * range / (2 * NUM_SEGMENTS)) as i32;
    }

    let mut map = [0u8; MAX_ALPHA + 1];
    let mut weighted_average = 0;
    for _ in 0..MAX_K_MEANS_ITERATIONS {
        let mut accumulations = [0i32; NUM_SEGMENTS];
        let mut distance_accumulations = [0i32; NUM_SEGMENTS];
        let mut nearest = 0;
        for alpha in minimum..=maximum {
            let count = alpha_counts[alpha];
            if count != 0 {
                while nearest + 1 < NUM_SEGMENTS
                    && (alpha as i32 - centers[nearest + 1]).abs()
                        < (alpha as i32 - centers[nearest]).abs()
                {
                    nearest += 1;
                }
                map[alpha] = nearest as u8;
                distance_accumulations[nearest] += alpha as i32 * count;
                accumulations[nearest] += count;
            }
        }

        let mut displaced = 0;
        let mut weighted_sum = 0;
        let mut total_weight = 0;
        for index in 0..NUM_SEGMENTS {
            if accumulations[index] != 0 {
                let center = (distance_accumulations[index] + accumulations[index] / 2)
                    / accumulations[index];
                displaced += (centers[index] - center).abs();
                centers[index] = center;
                weighted_sum += center * accumulations[index];
                total_weight += accumulations[index];
            }
        }
        weighted_average = (weighted_sum + total_weight / 2) / total_weight;
        if displaced < 5 {
            break;
        }
    }

    for macroblock in macroblocks {
        let segment = map[macroblock.alpha as usize];
        macroblock.segment = segment;
        macroblock.alpha = centers[segment as usize] as u8;
    }

    let minimum_center = *centers.iter().min().unwrap_or(&0);
    let mut maximum_center = *centers.iter().max().unwrap_or(&0);
    if maximum_center == minimum_center {
        maximum_center = minimum_center + 1;
    }
    std::array::from_fn(|index| SegmentAnalysis {
        alpha: (255 * (centers[index] - weighted_average) / (maximum_center - minimum_center))
            .clamp(-127, 127),
        beta: (255 * (centers[index] - minimum_center) / (maximum_center - minimum_center))
            .clamp(0, 255),
    })
}

pub(super) fn analyze(
    y_plane: &[u8],
    u_plane: &[u8],
    v_plane: &[u8],
    width: usize,
    height: usize,
) -> FrameAnalysis {
    let chroma_width = width.div_ceil(2);
    let chroma_height = height.div_ceil(2);
    let macroblock_width = width.div_ceil(16);
    let macroblock_height = height.div_ceil(16);
    let mut macroblocks = Vec::with_capacity(macroblock_width * macroblock_height);
    let mut alpha_counts = [0i32; MAX_ALPHA + 1];
    let mut alpha_sum = 0;
    let mut chroma_alpha_sum = 0;

    for macroblock_y in 0..macroblock_height {
        for macroblock_x in 0..macroblock_width {
            let y_x = macroblock_x * 16;
            let y_y = macroblock_y * 16;
            let y_block = extract_block(y_plane, width, width, height, y_x, y_y, 16);
            let (y_top, y_left, y_top_left) = boundary(y_plane, width, width, height, y_x, y_y, 16);

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

            let uv_x = macroblock_x * 8;
            let uv_y = macroblock_y * 8;
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

            let mixed_alpha =
                (255 - ((3 * best_luma_alpha + best_chroma_alpha + 2) >> 2)).clamp(0, 255);
            alpha_counts[mixed_alpha as usize] += 1;
            alpha_sum += mixed_alpha;
            chroma_alpha_sum += best_chroma_alpha;
            macroblocks.push(MacroblockAnalysis {
                alpha: mixed_alpha as u8,
                segment: 0,
                luma_mode,
                chroma_mode,
            });
        }
    }

    let macroblock_count = macroblocks.len() as i32;
    let alpha = alpha_sum / macroblock_count;
    let chroma_alpha = chroma_alpha_sum / macroblock_count;
    let segments = assign_segments(&mut macroblocks, &alpha_counts);
    FrameAnalysis {
        alpha,
        chroma_alpha,
        macroblocks,
        segments,
    }
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
        let quantizer = (127.0 * (1.0 - compression.powf(exponent))) as i32;
        let quantizer = quantizer.clamp(0, 127) as u8;
        let quantizer_step = Y_AC_QUANT[quantizer as usize] >> 2;
        let strength = i32::from(quantizer_step) * 300 / (256 + analysis.segments[index].beta);
        let filter_strength = if strength < 2 {
            0
        } else {
            strength.min(63) as u8
        };
        SegmentParams {
            quantizer,
            filter_strength,
        }
    });
    let chroma_ac_delta = (((analysis.chroma_alpha - 64) * 10 / 70) * 50 / 100).clamp(-4, 6) as i8;
    let chroma_dc_delta = (-4_i32 * 50 / 100).clamp(-15, 15) as i8;
    FrameParams {
        segments,
        chroma_dc_delta,
        chroma_ac_delta,
    }
}
