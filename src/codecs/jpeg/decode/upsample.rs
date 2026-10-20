// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

use crate::codecs::{CodecError, CodecResult};

// ── Chroma Upsampling (libjpeg-exact triangle filter) ─────────────────────

/// 2x1 fancy upsampling — exact match of IJG libjpeg h2v1_fancy_upsample.
pub(super) fn h2v1_fancy_upsample(src: &[u8], src_w: usize, src_h: usize) -> Vec<u8> {
    let dst_w = src_w.saturating_mul(2);
    let mut out = vec![0u8; dst_w.saturating_mul(src_h)];
    for y in 0..src_h {
        let in_row = y.saturating_mul(src_w);
        let out_row = y.saturating_mul(dst_w);

        let mut invalue = i32::from(src[in_row]);
        out[out_row] = low_u8(invalue);
        if src_w > 1 {
            out[out_row.saturating_add(1)] =
                interpolate(invalue, i32::from(src[in_row.saturating_add(1)]), 3, 2, 2);
        } else {
            out[out_row.saturating_add(1)] = low_u8(invalue);
        }

        for col in 1..src_w.saturating_sub(1) {
            invalue = i32::from(src[in_row.saturating_add(col)]).saturating_mul(3);
            let destination = out_row.saturating_add(col.saturating_mul(2));
            out[destination] = weighted_sum(
                invalue,
                i32::from(src[in_row.saturating_add(col.saturating_sub(1))]),
                1,
                2,
            );
            out[destination.saturating_add(1)] = weighted_sum(
                invalue,
                i32::from(src[in_row.saturating_add(col.saturating_add(1))]),
                2,
                2,
            );
        }

        if src_w > 1 {
            let last = src_w.saturating_sub(1);
            invalue = i32::from(src[in_row.saturating_add(last)]);
            let destination = out_row.saturating_add(last.saturating_mul(2));
            out[destination] = interpolate(
                invalue,
                i32::from(src[in_row.saturating_add(src_w.saturating_sub(2))]),
                3,
                1,
                2,
            );
            out[destination.saturating_add(1)] = low_u8(invalue);
        }
    }
    out
}

/// 1x2 fancy upsampling — exact match of IJG libjpeg h1v2_fancy_upsample.
pub(super) fn h1v2_fancy_upsample(src: &[u8], src_w: usize, src_h: usize) -> CodecResult<Vec<u8>> {
    let invalid_geometry =
        || CodecError::Malformed("invalid JPEG component upsampling geometry".to_owned());
    #[cfg(coverage)]
    let src_w = if crate::coverage_support::take_fault_point(
        crate::coverage_support::CoverageFaultPoint::JpegH1v2UpsampleZeroWidth,
    ) {
        0
    } else {
        src_w
    };
    #[cfg(coverage)]
    let src_h = if crate::coverage_support::take_fault_point(
        crate::coverage_support::CoverageFaultPoint::JpegH1v2UpsampleEmptyGeometry,
    ) {
        0
    } else {
        src_h
    };
    if src_w == 0 || src_h == 0 {
        return Err(invalid_geometry());
    }
    let source_length = src_w.checked_mul(src_h).ok_or_else(invalid_geometry)?;
    #[cfg(coverage)]
    let available_source_length = if crate::coverage_support::take_fault_point(
        crate::coverage_support::CoverageFaultPoint::JpegH1v2UpsampleSourceLengthMismatch,
    ) {
        src.len().saturating_sub(1)
    } else {
        src.len()
    };
    #[cfg(not(coverage))]
    let available_source_length = src.len();
    if available_source_length < source_length {
        return Err(invalid_geometry());
    }
    let dst_h = src_h.checked_mul(2).ok_or_else(invalid_geometry)?;
    let output_length = src_w.checked_mul(dst_h).ok_or_else(invalid_geometry)?;
    let mut out = Vec::new();
    #[cfg(coverage)]
    let output_reservation = if crate::coverage_support::take_fault_point(
        crate::coverage_support::CoverageFaultPoint::JpegH1v2UpsamplePlaneReservation,
    ) {
        usize::MAX
    } else {
        output_length
    };
    #[cfg(not(coverage))]
    let output_reservation = output_length;
    out.try_reserve_exact(output_reservation).map_err(|_| {
        CodecError::Dimensions("unable to allocate JPEG upsampled component".to_owned())
    })?;
    out.resize(output_length, 0);

    for source_y in 0..src_h {
        let source_start = source_y.checked_mul(src_w).ok_or_else(invalid_geometry)?;
        let source_end = source_start
            .checked_add(src_w)
            .ok_or_else(invalid_geometry)?;
        let current = src
            .get(source_start..source_end)
            .ok_or_else(invalid_geometry)?;
        let previous_y = source_y.saturating_sub(1);
        let previous_start = previous_y.checked_mul(src_w).ok_or_else(invalid_geometry)?;
        let previous_end = previous_start
            .checked_add(src_w)
            .ok_or_else(invalid_geometry)?;
        let previous = src
            .get(previous_start..previous_end)
            .ok_or_else(invalid_geometry)?;
        let next_y = source_y
            .checked_add(1)
            .ok_or_else(invalid_geometry)?
            .min(src_h.saturating_sub(1));
        let next_start = next_y.checked_mul(src_w).ok_or_else(invalid_geometry)?;
        let next_end = next_start.checked_add(src_w).ok_or_else(invalid_geometry)?;
        let next = src.get(next_start..next_end).ok_or_else(invalid_geometry)?;

        let top_start = source_y
            .checked_mul(2)
            .and_then(|row| row.checked_mul(src_w))
            .ok_or_else(invalid_geometry)?;
        let top_end = top_start.checked_add(src_w).ok_or_else(invalid_geometry)?;
        let top = out
            .get_mut(top_start..top_end)
            .ok_or_else(invalid_geometry)?;
        upsample_h1v2_row(current, previous, top, 1);
        let bottom_start = top_end;
        let bottom_end = bottom_start
            .checked_add(src_w)
            .ok_or_else(invalid_geometry)?;
        let bottom = out
            .get_mut(bottom_start..bottom_end)
            .ok_or_else(invalid_geometry)?;
        upsample_h1v2_row(current, next, bottom, 2);
    }

    Ok(out)
}

/// 2x2 fancy upsampling — exact match of IJG libjpeg h2v2_fancy_upsample.
pub(super) fn h2v2_fancy_upsample(src: &[u8], src_w: usize, src_h: usize) -> Vec<u8> {
    let dst_w = src_w.saturating_mul(2);
    let dst_h = src_h.saturating_mul(2);
    let mut out = vec![0u8; dst_w.saturating_mul(dst_h)];
    let mut inrow = 0usize;
    let mut outrow = 0usize;

    while outrow < dst_h {
        for v in 0usize..2 {
            let inptr0 = &src[inrow.saturating_mul(src_w)..];
            let inptr1 = if v == 0 {
                if inrow > 0 {
                    &src[inrow.saturating_sub(1).saturating_mul(src_w)..]
                } else {
                    &src[inrow.saturating_mul(src_w)..]
                }
            } else {
                if inrow.saturating_add(1) < src_h {
                    &src[inrow.saturating_add(1).saturating_mul(src_w)..]
                } else {
                    &src[inrow.saturating_mul(src_w)..]
                }
            };

            let out_row = outrow.saturating_mul(dst_w);

            let mut thiscolsum = vertical_sum(inptr0[0], inptr1[0]);
            let mut nextcolsum = if src_w > 1 {
                vertical_sum(inptr0[1], inptr1[1])
            } else {
                thiscolsum
            };
            out[out_row] = interpolate(thiscolsum, 0, 4, 8, 4);
            out[out_row.saturating_add(1)] = interpolate(thiscolsum, nextcolsum, 3, 7, 4);
            let mut lastcolsum = thiscolsum;
            thiscolsum = nextcolsum;

            for col in 1..src_w.saturating_sub(1) {
                let next = col.saturating_add(1);
                nextcolsum = vertical_sum(inptr0[next], inptr1[next]);
                let destination = out_row.saturating_add(col.saturating_mul(2));
                out[destination] = interpolate(thiscolsum, lastcolsum, 3, 8, 4);
                out[destination.saturating_add(1)] = interpolate(thiscolsum, nextcolsum, 3, 7, 4);
                lastcolsum = thiscolsum;
                thiscolsum = nextcolsum;
            }

            if src_w > 1 {
                let destination = out_row.saturating_add(src_w.saturating_sub(1).saturating_mul(2));
                out[destination] = interpolate(thiscolsum, lastcolsum, 3, 8, 4);
                out[destination.saturating_add(1)] = interpolate(thiscolsum, 0, 4, 7, 4);
            } else {
                out[out_row] = interpolate(thiscolsum, 0, 4, 8, 4);
                out[out_row.saturating_add(1)] = interpolate(thiscolsum, 0, 4, 7, 4);
            }

            outrow = outrow.saturating_add(1);
        }
        inrow = inrow.saturating_add(1);
    }
    out
}

/// Crop a component buffer to the valid image-derived dimensions.
///
/// The component buffer is padded to MCU-aligned boundaries. Chroma data
/// beyond the actual image area must not be fed into the upsampler,
/// or the triangle filter blends garbage padding values at image edges.
pub(super) fn crop_component(
    buf: &[u8],
    buf_w: usize,
    buf_h: usize,
    crop_w: usize,
    crop_h: usize,
) -> Vec<u8> {
    debug_assert!(crop_w <= buf_w);
    debug_assert!(crop_h <= buf_h);
    let mut out = Vec::with_capacity(crop_w.saturating_mul(crop_h));
    for y in 0..crop_h {
        let src_off = y.saturating_mul(buf_w);
        out.extend_from_slice(&buf[src_off..src_off.saturating_add(crop_w)]);
    }
    out
}

/// Dispatch to libjpeg-exact chroma upsampling based on ratios.
pub(super) fn fancy_upsample(
    src: &[u8],
    src_w: usize,
    src_h: usize,
    h_ratio: usize,
    v_ratio: usize,
    _dst_w: usize,
    _dst_h: usize,
) -> CodecResult<Vec<u8>> {
    match (h_ratio, v_ratio) {
        (1, 1) => {
            let mut out = Vec::with_capacity(src_w.saturating_mul(src_h));
            for y in 0..src_h {
                let row = y.saturating_mul(src_w);
                for x in 0..src_w {
                    out.push(src[row.saturating_add(x)]);
                }
            }
            Ok(out)
        }
        (1, 2) => h1v2_fancy_upsample(src, src_w, src_h),
        (2, 1) => Ok(h2v1_fancy_upsample(src, src_w, src_h)),
        (2, 2) => Ok(h2v2_fancy_upsample(src, src_w, src_h)),
        _ => {
            // Integer-only nearest-neighbor fallback for other ratios
            let out_w = src_w.saturating_mul(h_ratio);
            let out_h = src_h.saturating_mul(v_ratio);
            let mut out = vec![0u8; out_w.saturating_mul(out_h)];
            for y in 0..out_h {
                let sy = y.div_euclid(v_ratio);
                for x in 0..out_w {
                    let sx = x.div_euclid(h_ratio);
                    out[y.saturating_mul(out_w).saturating_add(x)] =
                        src[sy.saturating_mul(src_w).saturating_add(sx)];
                }
            }
            Ok(out)
        }
    }
}

/// Upsample one destination row without materializing the full component plane.
///
/// The h1v2, h2v1, and h2v2 branches preserve the same IJG triangle filter and
/// edge rounding as [`fancy_upsample`]. Other integer ratios use the same
/// nearest neighbor mapping as its fallback branch.
pub(super) struct UpsampleComponent<'a> {
    pub(super) samples: &'a [u8],
    pub(super) stride: usize,
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) horizontal_ratio: usize,
    pub(super) vertical_ratio: usize,
}

pub(super) fn fancy_upsample_row(
    component: &UpsampleComponent<'_>,
    dst_y: usize,
    output: &mut [u8],
) -> CodecResult<()> {
    let invalid_geometry =
        || CodecError::Malformed("invalid JPEG component upsampling geometry".to_owned());
    let src = component.samples;
    let src_stride = component.stride;
    let src_w = component.width;
    let src_h = component.height;
    let h_ratio = component.horizontal_ratio;
    let v_ratio = component.vertical_ratio;
    #[cfg(coverage)]
    let src_w = if crate::coverage_support::take_fault_point(
        crate::coverage_support::CoverageFaultPoint::JpegFancyUpsampleRowInvalidGeometry,
    ) {
        0
    } else {
        src_w
    };
    if src_w == 0 || src_h == 0 || src_stride < src_w || h_ratio == 0 || v_ratio == 0 {
        return Err(invalid_geometry());
    }
    let Some(last_row_start) = src_stride.checked_mul(src_h.saturating_sub(1)) else {
        return Err(invalid_geometry());
    };
    let Some(required_source_len) = last_row_start.checked_add(src_w) else {
        return Err(invalid_geometry());
    };
    let Some(destination_height) = src_h.checked_mul(v_ratio) else {
        return Err(invalid_geometry());
    };
    let Some(destination_width) = src_w.checked_mul(h_ratio) else {
        return Err(invalid_geometry());
    };
    if src.len() < required_source_len
        || dst_y >= destination_height
        || output.len() > destination_width
    {
        return Err(invalid_geometry());
    }

    let source_y = dst_y.checked_div(v_ratio).ok_or_else(invalid_geometry)?;
    let source_row_start = src_stride
        .checked_mul(source_y)
        .ok_or_else(invalid_geometry)?;
    let source_row_end = source_row_start
        .checked_add(src_w)
        .ok_or_else(invalid_geometry)?;
    let source_row = src
        .get(source_row_start..source_row_end)
        .ok_or_else(invalid_geometry)?;
    match (h_ratio, v_ratio) {
        (1, 1) => output.copy_from_slice(&source_row[..output.len()]),
        (1, 2) => {
            let adjacent_y = if dst_y.is_multiple_of(2) {
                source_y.saturating_sub(1)
            } else {
                source_y.saturating_add(1).min(src_h.saturating_sub(1))
            };
            let adjacent_start = src_stride
                .checked_mul(adjacent_y)
                .ok_or_else(invalid_geometry)?;
            let adjacent_end = adjacent_start
                .checked_add(src_w)
                .ok_or_else(invalid_geometry)?;
            let adjacent_row = src
                .get(adjacent_start..adjacent_end)
                .ok_or_else(invalid_geometry)?;
            let bias = if dst_y.is_multiple_of(2) { 1 } else { 2 };
            upsample_h1v2_row(source_row, adjacent_row, output, bias);
        }
        (2, 1) => upsample_h2v1_row(source_row, output),
        (2, 2) => {
            let adjacent_y = if dst_y.is_multiple_of(2) {
                source_y.saturating_sub(1)
            } else {
                source_y.saturating_add(1).min(src_h.saturating_sub(1))
            };
            let adjacent_start = src_stride
                .checked_mul(adjacent_y)
                .ok_or_else(invalid_geometry)?;
            let adjacent_end = adjacent_start
                .checked_add(src_w)
                .ok_or_else(invalid_geometry)?;
            let adjacent_row = src
                .get(adjacent_start..adjacent_end)
                .ok_or_else(invalid_geometry)?;
            upsample_h2v2_row(source_row, adjacent_row, output);
        }
        _ => {
            for (x, sample) in output.iter_mut().enumerate() {
                let source_x = x.checked_div(h_ratio).ok_or_else(invalid_geometry)?;
                *sample = source_row[source_x];
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "Validated equal nonempty rows and bounded output length prove each h1v2 sample index is in range."
)]
fn upsample_h1v2_row(current: &[u8], adjacent: &[u8], output: &mut [u8], bias: i32) {
    for (column, sample) in output.iter_mut().enumerate() {
        *sample = weighted_sum(
            i32::from(current[column]).saturating_mul(3),
            i32::from(adjacent[column]),
            bias,
            2,
        );
    }
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "Loop ranges and output bounds prove each h2v1 source and destination index is in range."
)]
fn upsample_h2v1_row(src: &[u8], output: &mut [u8]) {
    if output.is_empty() {
        return;
    }
    let mut invalue = i32::from(src[0]);
    output[0] = low_u8(invalue);
    if output.len() > 1 {
        output[1] = if src.len() > 1 {
            interpolate(invalue, i32::from(src[1]), 3, 2, 2)
        } else {
            low_u8(invalue)
        };
    }

    for col in 1..src.len().saturating_sub(1) {
        invalue = i32::from(src[col]).saturating_mul(3);
        let destination = col.saturating_mul(2);
        if destination < output.len() {
            output[destination] = weighted_sum(invalue, i32::from(src[col - 1]), 1, 2);
        }
        if destination.saturating_add(1) < output.len() {
            output[destination + 1] = weighted_sum(invalue, i32::from(src[col + 1]), 2, 2);
        }
    }

    if src.len() > 1 {
        let last = src.len() - 1;
        let destination = last.saturating_mul(2);
        invalue = i32::from(src[last]);
        if destination < output.len() {
            output[destination] = interpolate(invalue, i32::from(src[last - 1]), 3, 1, 2);
        }
        if destination.saturating_add(1) < output.len() {
            output[destination + 1] = low_u8(invalue);
        }
    }
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "Validated equal nonempty rows and bounded loop ranges prove each h2v2 index is in range."
)]
fn upsample_h2v2_row(top: &[u8], adjacent: &[u8], output: &mut [u8]) {
    if output.is_empty() {
        return;
    }
    let mut this_col_sum = vertical_sum(top[0], adjacent[0]);
    let mut next_col_sum = if top.len() > 1 {
        vertical_sum(top[1], adjacent[1])
    } else {
        this_col_sum
    };
    output[0] = interpolate(this_col_sum, 0, 4, 8, 4);
    if output.len() > 1 {
        output[1] = interpolate(this_col_sum, next_col_sum, 3, 7, 4);
    }
    let mut last_col_sum = this_col_sum;
    this_col_sum = next_col_sum;

    for col in 1..top.len().saturating_sub(1) {
        let next = col + 1;
        next_col_sum = vertical_sum(top[next], adjacent[next]);
        let destination = col.saturating_mul(2);
        if destination < output.len() {
            output[destination] = interpolate(this_col_sum, last_col_sum, 3, 8, 4);
        }
        if destination.saturating_add(1) < output.len() {
            output[destination + 1] = interpolate(this_col_sum, next_col_sum, 3, 7, 4);
        }
        last_col_sum = this_col_sum;
        this_col_sum = next_col_sum;
    }

    if top.len() > 1 {
        let last = top.len() - 1;
        let destination = last.saturating_mul(2);
        if destination < output.len() {
            output[destination] = interpolate(this_col_sum, last_col_sum, 3, 8, 4);
        }
        if destination.saturating_add(1) < output.len() {
            output[destination + 1] = interpolate(this_col_sum, 0, 4, 7, 4);
        }
    } else if output.len() > 1 {
        output[0] = interpolate(this_col_sum, 0, 4, 8, 4);
        output[1] = interpolate(this_col_sum, 0, 4, 7, 4);
    }
}

fn vertical_sum(primary: u8, adjacent: u8) -> i32 {
    i32::from(primary)
        .saturating_mul(3)
        .saturating_add(i32::from(adjacent))
}

fn interpolate(center: i32, adjacent: i32, weight: i32, bias: i32, shift: u32) -> u8 {
    weighted_sum(center.saturating_mul(weight), adjacent, bias, shift)
}

fn weighted_sum(weighted_center: i32, adjacent: i32, bias: i32, shift: u32) -> u8 {
    low_u8(
        weighted_center
            .saturating_add(adjacent)
            .saturating_add(bias)
            .wrapping_shr(shift),
    )
}

fn low_u8(value: i32) -> u8 {
    value.to_le_bytes()[0]
}
