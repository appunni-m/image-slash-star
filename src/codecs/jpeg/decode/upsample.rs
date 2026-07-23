// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

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
    _buf_h: usize,
    crop_w: usize,
    crop_h: usize,
) -> Vec<u8> {
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
) -> Vec<u8> {
    match (h_ratio, v_ratio) {
        (1, 1) => {
            let mut out = Vec::with_capacity(src_w.saturating_mul(src_h));
            for y in 0..src_h {
                let row = y.saturating_mul(src_w);
                for x in 0..src_w {
                    out.push(src[row.saturating_add(x)]);
                }
            }
            out
        }
        (2, 1) => h2v1_fancy_upsample(src, src_w, src_h),
        (2, 2) => h2v2_fancy_upsample(src, src_w, src_h),
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
            out
        }
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
