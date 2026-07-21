//! VP8 lossy encoding pipeline — ties together all VP8 modules.
//!
//! Encodes an RGB image into a VP8 keyframe bitstream within a RIFF/WEBP container.
//!
//! # Bitstream structure (RFC 6386)
//!
//! The VP8 partition 0 consists of two parts:
//! 1. **First partition** (first_partition_size bytes): Bool-encoded frame header +
//!    macroblock mode headers.
//! 2. **Remaining data**: Bool-encoded coefficient tokens (Y2 WHT, luma, chroma).
//!
//! The decoder reads the first partition into the main bool decoder (`self.b`),
//! and the remaining bytes become `self.partitions[0]` for coefficient decoding.

use super::{
    analysis::{FrameParams, analyze, segment_params},
    frame::select_frame,
    partition::encode_first_partition,
    probability::adapt_coefficients,
    residual::encode_coefficients,
    tokenize::COEFF_PROBS,
};

/// Encode an RGB image to a lossy VP8 WebP bitstream.
///
/// Returns the complete RIFF/WEBP container bytes.
pub fn encode_vp8_lossy(rgb: &[u8], width: u32, height: u32, quality: u8, method: u8) -> Vec<u8> {
    let (y_plane, u_plane, v_plane) = rgb_to_yuv_planes_internal(rgb, width, height);
    let vp8_data = encode_vp8_planes(y_plane, u_plane, v_plane, width, height, quality, method);
    build_webp_container(&vp8_data, width, height)
}

pub(crate) fn encode_vp8_lossy_rgba(
    rgba: &[u8],
    width: u32,
    height: u32,
    quality: u8,
    method: u8,
    alpha_chunk: &[u8],
) -> Vec<u8> {
    let (y_plane, u_plane, v_plane) = rgba_to_yuv_planes_internal(rgba, width, height);
    let vp8_data = encode_vp8_planes(y_plane, u_plane, v_plane, width, height, quality, method);
    build_extended_webp_container(&vp8_data, alpha_chunk, width, height)
}

fn encode_vp8_planes(
    y_plane: Vec<u8>,
    u_plane: Vec<u8>,
    v_plane: Vec<u8>,
    width: u32,
    height: u32,
    quality: u8,
    method: u8,
) -> Vec<u8> {
    let padded_width = width.div_ceil(16) * 16;
    let padded_height = height.div_ceil(16) * 16;
    let chroma_width = width.div_ceil(2);
    let chroma_height = height.div_ceil(2);
    let padded_chroma_width = padded_width / 2;
    let padded_chroma_height = padded_height / 2;
    let y_plane = pad_plane(
        &y_plane,
        width as usize,
        height as usize,
        padded_width as usize,
        padded_height as usize,
    );
    let u_plane = pad_plane(
        &u_plane,
        chroma_width as usize,
        chroma_height as usize,
        padded_chroma_width as usize,
        padded_chroma_height as usize,
    );
    let v_plane = pad_plane(
        &v_plane,
        chroma_width as usize,
        chroma_height as usize,
        padded_chroma_width as usize,
        padded_chroma_height as usize,
    );
    let analysis = analyze(
        &y_plane,
        &u_plane,
        &v_plane,
        padded_width as usize,
        padded_height as usize,
        quality,
        method,
    );
    let mut params = segment_params(&analysis, f64::from(quality));
    let mut decisions = select_frame(
        &y_plane,
        &u_plane,
        &v_plane,
        padded_width as usize,
        padded_height as usize,
        f64::from(quality),
        method,
        &COEFF_PROBS,
        false,
    );
    let segment_map = simplify_segments(&mut params);
    for decision in &mut decisions {
        decision.segment = segment_map[usize::from(decision.segment)];
    }
    let macroblock_width = padded_width as usize / 16;
    let statistics_count = if method == 0 {
        decisions.len().min(50)
    } else {
        decisions.len()
    };
    let mut probabilities = adapt_coefficients(
        &decisions[..statistics_count],
        macroblock_width,
        method >= 3,
    );
    if method >= 6 {
        decisions = select_frame(
            &y_plane,
            &u_plane,
            &v_plane,
            padded_width as usize,
            padded_height as usize,
            f64::from(quality),
            method,
            &COEFF_PROBS,
            true,
        );
        for decision in &mut decisions {
            decision.segment = segment_map[usize::from(decision.segment)];
        }
        probabilities = adapt_coefficients(&decisions, macroblock_width, true);
    }
    let header_data = encode_first_partition(
        &decisions,
        macroblock_width,
        &params,
        &probabilities,
        method >= 3,
    );
    let coeff_data = encode_coefficients(&decisions, macroblock_width, &probabilities);
    let frame_header = build_frame_header(width, height, header_data.len() as u32);

    let mut vp8_data = frame_header;
    vp8_data.extend_from_slice(&header_data);
    vp8_data.extend_from_slice(&coeff_data);

    vp8_data
}

fn simplify_segments(params: &mut FrameParams) -> [u8; 4] {
    let mut map = [0, 1, 2, 3];
    let mut final_segments = 1;

    for source in 1..params.num_segments {
        let mut destination = 0;
        while destination < final_segments
            && params.segments[source] != params.segments[destination]
        {
            destination += 1;
        }
        map[source] = destination as u8;
        if destination == final_segments {
            if destination != source {
                params.segments[destination] = params.segments[source];
            }
            final_segments += 1;
        }
    }

    params.num_segments = final_segments;
    for segment in final_segments..params.segments.len() {
        params.segments[segment] = params.segments[final_segments - 1];
    }
    map
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let mut params = FrameParams {
        segments: std::array::from_fn(|_| super::analysis::SegmentParams {
            quantizer: 20,
            filter_strength: 10,
        }),
        num_segments: 3,
        chroma_dc_delta: 0,
        chroma_ac_delta: 0,
    };
    params.num_segments = 3;
    params.segments[1] = params.segments[0].clone();
    params.segments[2].quantizer = params.segments[0].quantizer.saturating_add(1);
    let map = simplify_segments(&mut params);
    assert_eq!(map[2], 1);

    let rgba = vec![0u8; 8 * 9 * 4];
    let mut y_plane = vec![0u8; 8 * 9];
    let mut u_plane = vec![0u8; 5 * 5];
    let mut v_plane = vec![0u8; 5 * 5];
    cleanup_transparent_area(&rgba, 8, 9, &mut y_plane, &mut u_plane, &mut v_plane);
}

fn pad_plane(
    input: &[u8],
    width: usize,
    height: usize,
    padded_width: usize,
    padded_height: usize,
) -> Vec<u8> {
    let mut output = vec![0; padded_width * padded_height];
    for y in 0..padded_height {
        let source_y = y.min(height - 1);
        for x in 0..padded_width {
            output[y * padded_width + x] = input[source_y * width + x.min(width - 1)];
        }
    }
    output
}

// ===========================================================================
// Bitstream helpers
// ===========================================================================

const YUV_FIX: i32 = 16;
const YUV_HALF: i32 = 1 << (YUV_FIX - 1);
const GAMMA_FIX: i32 = 12;
const GAMMA_TAB_FIX: i32 = 7;
const GAMMA_TAB_SIZE: usize = 1 << (GAMMA_FIX - GAMMA_TAB_FIX);

fn rgb_to_y(r: i32, g: i32, b: i32) -> u8 {
    let luma = 16_839 * r + 33_059 * g + 6_420 * b;
    ((luma + YUV_HALF + (16 << YUV_FIX)) >> YUV_FIX) as u8
}

fn clip_uv(value: i32) -> u8 {
    let value = (value + (YUV_HALF << 2) + (128 << (YUV_FIX + 2))) >> (YUV_FIX + 2);
    value.clamp(0, 255) as u8
}

fn rgb_to_u(r: i32, g: i32, b: i32) -> u8 {
    clip_uv(-9_719 * r - 19_081 * g + 28_800 * b)
}

fn rgb_to_v(r: i32, g: i32, b: i32) -> u8 {
    clip_uv(28_800 * r - 24_116 * g - 4_684 * b)
}

fn gamma_tables() -> &'static ([u16; 256], [i32; GAMMA_TAB_SIZE + 1]) {
    use std::sync::OnceLock;

    static TABLES: OnceLock<([u16; 256], [i32; GAMMA_TAB_SIZE + 1])> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut gamma_to_linear = [0u16; 256];
        for (value, result) in gamma_to_linear.iter_mut().enumerate() {
            *result = (((value as f64 / 255.0).powf(0.80) * 4_095.0) + 0.5) as u16;
        }

        let mut linear_to_gamma = [0i32; GAMMA_TAB_SIZE + 1];
        for (value, result) in linear_to_gamma.iter_mut().enumerate() {
            let scaled = (128.0 * value as f64) / 4_095.0;
            *result = (255.0 * scaled.powf(1.0 / 0.80) + 0.5) as i32;
        }
        (gamma_to_linear, linear_to_gamma)
    })
}

fn linear_to_gamma(base_value: u32) -> i32 {
    let (_, linear_to_gamma) = gamma_tables();
    let tab_position = (base_value >> (GAMMA_TAB_FIX + 2)) as usize;
    let fraction = (base_value & ((1 << (GAMMA_TAB_FIX + 2)) - 1)) as i32;
    let span = 1 << (GAMMA_TAB_FIX + 2);
    let interpolated = linear_to_gamma[tab_position] * (span - fraction)
        + linear_to_gamma[tab_position + 1] * fraction;
    (interpolated + (1 << (GAMMA_TAB_FIX - 1))) >> GAMMA_TAB_FIX
}

/// Convert RGB bytes to the YUV420 planes produced by libwebp's regular import path.
pub(super) fn rgb_to_yuv_planes_internal(
    rgb: &[u8],
    width: u32,
    height: u32,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let w = width as usize;
    let h = height as usize;
    let mut y_plane = vec![0u8; w * h];
    let uv_w = (w + 1) / 2;
    let uv_h = (h + 1) / 2;
    let mut u_plane = vec![0u8; uv_w * uv_h];
    let mut v_plane = vec![0u8; uv_w * uv_h];

    for row in 0..h {
        for col in 0..w {
            let idx = (row * w + col) * 3;
            y_plane[row * w + col] = rgb_to_y(
                i32::from(rgb[idx]),
                i32::from(rgb[idx + 1]),
                i32::from(rgb[idx + 2]),
            );
        }
    }

    for row in 0..uv_h {
        for col in 0..uv_w {
            let r0 = row * 2;
            let c0 = col * 2;
            let r1 = (r0 + 1).min(h - 1);
            let c1 = (c0 + 1).min(w - 1);

            let p00 = (r0 * w + c0) * 3;
            let p01 = (r0 * w + c1) * 3;
            let p10 = (r1 * w + c0) * 3;
            let p11 = (r1 * w + c1) * 3;

            let (gamma_to_linear, _) = gamma_tables();
            let gamma_sum = |channel: usize| {
                u32::from(gamma_to_linear[rgb[p00 + channel] as usize])
                    + u32::from(gamma_to_linear[rgb[p01 + channel] as usize])
                    + u32::from(gamma_to_linear[rgb[p10 + channel] as usize])
                    + u32::from(gamma_to_linear[rgb[p11 + channel] as usize])
            };
            let r = linear_to_gamma(gamma_sum(0));
            let g = linear_to_gamma(gamma_sum(1));
            let b = linear_to_gamma(gamma_sum(2));
            let uv_idx = row * uv_w + col;
            u_plane[uv_idx] = rgb_to_u(r, g, b);
            v_plane[uv_idx] = rgb_to_v(r, g, b);
        }
    }

    (y_plane, u_plane, v_plane)
}

fn smoothen_transparent_luma(
    rgba: &[u8],
    image_width: usize,
    y_plane: &mut [u8],
    origin_x: usize,
    origin_y: usize,
    width: usize,
    height: usize,
) -> bool {
    let mut sum = 0usize;
    let mut count = 0usize;
    for y in origin_y..origin_y + height {
        for x in origin_x..origin_x + width {
            if rgba[(y * image_width + x) * 4 + 3] != 0 {
                count += 1;
                sum += usize::from(y_plane[y * image_width + x]);
            }
        }
    }
    if count > 0 && count < width * height {
        let average = (sum / count) as u8;
        for y in origin_y..origin_y + height {
            for x in origin_x..origin_x + width {
                if rgba[(y * image_width + x) * 4 + 3] == 0 {
                    y_plane[y * image_width + x] = average;
                }
            }
        }
    }
    count == 0
}

fn fill_block(
    plane: &mut [u8],
    stride: usize,
    origin_x: usize,
    origin_y: usize,
    size: usize,
    value: u8,
) {
    for y in origin_y..origin_y + size {
        plane[y * stride + origin_x..y * stride + origin_x + size].fill(value);
    }
}

fn cleanup_transparent_area(
    rgba: &[u8],
    width: usize,
    height: usize,
    y_plane: &mut [u8],
    u_plane: &mut [u8],
    v_plane: &mut [u8],
) {
    const BLOCK: usize = 8;
    let uv_width = width.div_ceil(2);
    let full_width = width / BLOCK * BLOCK;
    let full_height = height / BLOCK * BLOCK;

    for origin_y in (0..full_height).step_by(BLOCK) {
        let mut flattened_values = None;
        for origin_x in (0..full_width).step_by(BLOCK) {
            if smoothen_transparent_luma(rgba, width, y_plane, origin_x, origin_y, BLOCK, BLOCK) {
                let values = *flattened_values.get_or_insert_with(|| {
                    [
                        y_plane[origin_y * width + origin_x],
                        u_plane[(origin_y / 2) * uv_width + origin_x / 2],
                        v_plane[(origin_y / 2) * uv_width + origin_x / 2],
                    ]
                });
                fill_block(y_plane, width, origin_x, origin_y, BLOCK, values[0]);
                fill_block(
                    u_plane,
                    uv_width,
                    origin_x / 2,
                    origin_y / 2,
                    BLOCK / 2,
                    values[1],
                );
                fill_block(
                    v_plane,
                    uv_width,
                    origin_x / 2,
                    origin_y / 2,
                    BLOCK / 2,
                    values[2],
                );
            } else {
                flattened_values = None;
            }
        }
        if full_width < width {
            smoothen_transparent_luma(
                rgba,
                width,
                y_plane,
                full_width,
                origin_y,
                width - full_width,
                BLOCK,
            );
        }
    }
    if full_height < height {
        for origin_x in (0..full_width).step_by(BLOCK) {
            smoothen_transparent_luma(
                rgba,
                width,
                y_plane,
                origin_x,
                full_height,
                BLOCK,
                height - full_height,
            );
        }
        if full_width < width {
            smoothen_transparent_luma(
                rgba,
                width,
                y_plane,
                full_width,
                full_height,
                width - full_width,
                height - full_height,
            );
        }
    }
}

fn rgba_to_yuv_planes_internal(
    rgba: &[u8],
    width: u32,
    height: u32,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let w = width as usize;
    let h = height as usize;
    let mut y_plane = vec![0u8; w * h];
    let uv_w = w.div_ceil(2);
    let uv_h = h.div_ceil(2);
    let mut u_plane = vec![0u8; uv_w * uv_h];
    let mut v_plane = vec![0u8; uv_w * uv_h];

    for row in 0..h {
        for col in 0..w {
            let index = (row * w + col) * 4;
            y_plane[row * w + col] = rgb_to_y(
                i32::from(rgba[index]),
                i32::from(rgba[index + 1]),
                i32::from(rgba[index + 2]),
            );
        }
    }

    let (gamma_to_linear, _) = gamma_tables();
    for row in 0..uv_h {
        for col in 0..uv_w {
            let y0 = row * 2;
            let x0 = col * 2;
            let y1 = (y0 + 1).min(h - 1);
            let x1 = (x0 + 1).min(w - 1);
            let indices = [
                (y0 * w + x0) * 4,
                (y0 * w + x1) * 4,
                (y1 * w + x0) * 4,
                (y1 * w + x1) * 4,
            ];
            let total_alpha = indices
                .iter()
                .map(|&index| u32::from(rgba[index + 3]))
                .sum::<u32>();
            let channel_sum = |channel: usize| {
                if matches!(total_alpha, 0 | 1020) {
                    indices
                        .iter()
                        .map(|&index| u32::from(gamma_to_linear[rgba[index + channel] as usize]))
                        .sum()
                } else {
                    let weighted = indices
                        .iter()
                        .map(|&index| {
                            u32::from(rgba[index + 3])
                                * u32::from(gamma_to_linear[rgba[index + channel] as usize])
                        })
                        .sum::<u32>();
                    weighted * ((1 << 19) / total_alpha) >> 17
                }
            };
            let r = linear_to_gamma(channel_sum(0));
            let g = linear_to_gamma(channel_sum(1));
            let b = linear_to_gamma(channel_sum(2));
            let uv_index = row * uv_w + col;
            u_plane[uv_index] = rgb_to_u(r, g, b);
            v_plane[uv_index] = rgb_to_v(r, g, b);
        }
    }

    cleanup_transparent_area(rgba, w, h, &mut y_plane, &mut u_plane, &mut v_plane);
    (y_plane, u_plane, v_plane)
}

/// Build the uncompressed VP8 keyframe header (NOT bool-encoded).
fn build_frame_header(width: u32, height: u32, partition0_size: u32) -> Vec<u8> {
    let mut hdr = Vec::new();

    // Frame tag: 3 bytes
    //   Bit 0: frame type (0 = KEYFRAME)
    //   Bits 1-3: version (0)
    //   Bit 4: show_frame (1)
    //   Bits 5-23: first_partition_size (19 bits)
    let p0 = partition0_size & 0x7FFFF;
    let tag_byte0: u8 = 0x10 | (((p0 & 0x07) as u8) << 5);
    let tag_byte1: u8 = ((p0 >> 3) & 0xFF) as u8;
    let tag_byte2: u8 = ((p0 >> 11) & 0xFF) as u8;
    hdr.push(tag_byte0);
    hdr.push(tag_byte1);
    hdr.push(tag_byte2);

    // Start-of-frame marker
    hdr.push(0x9D);
    hdr.push(0x01);
    hdr.push(0x2A);

    // Horizontal size code: 14-bit width + 2-bit scale (0)
    let w = (width & 0x3FFF) as u16;
    hdr.extend_from_slice(&w.to_le_bytes());

    // Vertical size code: 14-bit height + 2-bit scale (0)
    let h = (height & 0x3FFF) as u16;
    hdr.extend_from_slice(&h.to_le_bytes());

    hdr
}

/// Build RIFF/WEBP/VP8 container.
fn build_webp_container(vp8_data: &[u8], _width: u32, _height: u32) -> Vec<u8> {
    let vp8_chunk_size = (vp8_data.len() + (vp8_data.len() & 1)) as u32;
    let riff_size: u32 = 4 + 4 + 4 + vp8_chunk_size;

    let mut out = Vec::with_capacity(12 + 8 + vp8_data.len() + 1);

    // RIFF header
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WEBP");

    // VP8 chunk header
    out.extend_from_slice(b"VP8 ");
    out.extend_from_slice(&vp8_chunk_size.to_le_bytes());

    // VP8 data (includes frame header + bool-encoded data)
    out.extend_from_slice(vp8_data);

    // Pad to even length (RIFF requirement)
    if vp8_data.len() & 1 != 0 {
        out.push(0);
    }

    out
}

fn append_chunk(output: &mut Vec<u8>, name: &[u8; 4], data: &[u8]) {
    output.extend_from_slice(name);
    output.extend_from_slice(&(data.len() as u32).to_le_bytes());
    output.extend_from_slice(data);
    if data.len() & 1 != 0 {
        output.push(0);
    }
}

fn build_extended_webp_container(
    vp8_data: &[u8],
    alpha_chunk: &[u8],
    width: u32,
    height: u32,
) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&[0; 4]);
    output.extend_from_slice(b"WEBP");

    let mut vp8x = Vec::with_capacity(10);
    vp8x.extend_from_slice(&[0x10, 0, 0, 0]);
    vp8x.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
    vp8x.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
    append_chunk(&mut output, b"VP8X", &vp8x);
    append_chunk(&mut output, b"ALPH", alpha_chunk);
    append_chunk(&mut output, b"VP8 ", vp8_data);

    let riff_size = (output.len() - 8) as u32;
    output[4..8].copy_from_slice(&riff_size.to_le_bytes());
    output
}
