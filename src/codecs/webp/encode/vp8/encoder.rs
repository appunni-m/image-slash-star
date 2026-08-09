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

use crate::codecs::CodecResult;

use super::{
    analysis::{FrameParams, analyze, segment_params},
    frame::{FrameSelectionOptions, select_frame},
    partition::encode_first_partition,
    probability::adapt_coefficients,
    residual::encode_coefficients,
    tokenize::COEFF_PROBS,
};

const OUTPUT_COPY_CHECKPOINT_BYTES: usize = 1_024;

fn extend_with_output_checkpoint(
    output: &mut Vec<u8>,
    source: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    let Some(token) = token else {
        output.extend_from_slice(source);
        return Ok(());
    };
    for chunk in source.chunks(OUTPUT_COPY_CHECKPOINT_BYTES) {
        output.extend_from_slice(chunk);
        if chunk.len() == OUTPUT_COPY_CHECKPOINT_BYTES {
            crate::codecs::error::check_cancelled(Some(token))?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Vp8EncodeContext<'a> {
    quality: u8,
    method: u8,
    token: Option<&'a crate::CancellationToken>,
}

/// Encode an RGB image to a lossy VP8 WebP bitstream.
///
/// Returns the complete RIFF/WEBP container bytes.
pub(crate) fn encode_vp8_lossy(
    rgb: &[u8],
    width: u32,
    height: u32,
    quality: u8,
    method: u8,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    let (y_plane, u_plane, v_plane) = rgb_to_yuv_planes_internal(rgb, width, height, token)?;
    crate::codecs::error::check_cancelled(token)?;
    let vp8_data = encode_vp8_planes(
        y_plane,
        u_plane,
        v_plane,
        width,
        height,
        Vp8EncodeContext {
            quality,
            method,
            token,
        },
    )?;
    crate::codecs::error::check_cancelled(token)?;
    build_webp_container(&vp8_data, width, height, token)
}

pub(crate) fn encode_vp8_lossy_rgba(
    rgba: &[u8],
    width: u32,
    height: u32,
    quality: u8,
    method: u8,
    alpha_chunk: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    let (y_plane, u_plane, v_plane) = rgba_to_yuv_planes_internal(rgba, width, height, token)?;
    crate::codecs::error::check_cancelled(token)?;
    let vp8_data = encode_vp8_planes(
        y_plane,
        u_plane,
        v_plane,
        width,
        height,
        Vp8EncodeContext {
            quality,
            method,
            token,
        },
    )?;
    crate::codecs::error::check_cancelled(token)?;
    build_extended_webp_container(&vp8_data, alpha_chunk, width, height, token)
}

fn encode_vp8_planes(
    y_plane: Vec<u8>,
    u_plane: Vec<u8>,
    v_plane: Vec<u8>,
    width: u32,
    height: u32,
    context: Vp8EncodeContext<'_>,
) -> CodecResult<Vec<u8>> {
    let Vp8EncodeContext {
        quality,
        method,
        token,
    } = context;
    let padded_width = width.div_ceil(16).wrapping_mul(16);
    let padded_height = height.div_ceil(16).wrapping_mul(16);
    let chroma_width = width.div_ceil(2);
    let chroma_height = height.div_ceil(2);
    let padded_chroma_width = padded_width / 2;
    let padded_chroma_height = padded_height / 2;
    let mut padding_items = 0_usize;
    let y_plane = pad_plane_with_token(
        &y_plane,
        width as usize,
        height as usize,
        padded_width as usize,
        padded_height as usize,
        token,
        &mut padding_items,
    )?;
    let u_plane = pad_plane_with_token(
        &u_plane,
        chroma_width as usize,
        chroma_height as usize,
        padded_chroma_width as usize,
        padded_chroma_height as usize,
        token,
        &mut padding_items,
    )?;
    let v_plane = pad_plane_with_token(
        &v_plane,
        chroma_width as usize,
        chroma_height as usize,
        padded_chroma_width as usize,
        padded_chroma_height as usize,
        token,
        &mut padding_items,
    )?;
    crate::codecs::error::check_cancelled(token)?;
    let analysis = analyze(
        [&y_plane, &u_plane, &v_plane],
        (padded_width as usize, padded_height as usize),
        quality,
        method,
        token,
    )?;
    crate::codecs::error::check_cancelled(token)?;
    let mut params = segment_params(&analysis, f64::from(quality));
    crate::codecs::error::check_cancelled(token)?;
    let mut decisions = select_frame(
        [&y_plane, &u_plane, &v_plane],
        (padded_width as usize, padded_height as usize),
        &analysis,
        FrameSelectionOptions {
            quality: f64::from(quality),
            method,
            coefficient_probabilities: &COEFF_PROBS,
            trellis: false,
            token,
        },
    )?;
    crate::codecs::error::check_cancelled(token)?;
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
        token,
    )?;
    crate::codecs::error::check_cancelled(token)?;
    if method >= 6 {
        decisions = select_frame(
            [&y_plane, &u_plane, &v_plane],
            (padded_width as usize, padded_height as usize),
            &analysis,
            FrameSelectionOptions {
                quality: f64::from(quality),
                method,
                coefficient_probabilities: &COEFF_PROBS,
                trellis: true,
                token,
            },
        )?;
        crate::codecs::error::check_cancelled(token)?;
        for decision in &mut decisions {
            decision.segment = segment_map[usize::from(decision.segment)];
        }
        probabilities = adapt_coefficients(&decisions, macroblock_width, true, token)?;
        crate::codecs::error::check_cancelled(token)?;
    }
    let header_data = encode_first_partition(
        &decisions,
        macroblock_width,
        &params,
        &probabilities,
        method >= 3,
        token,
    )?;
    crate::codecs::error::check_cancelled(token)?;
    let coeff_data = encode_coefficients(&decisions, macroblock_width, &probabilities, token)?;
    crate::codecs::error::check_cancelled(token)?;
    let frame_header = build_frame_header(width, height, low_u32(header_data.len()));

    let mut vp8_data = frame_header;
    vp8_data.extend_from_slice(&header_data);
    vp8_data.extend_from_slice(&coeff_data);

    Ok(vp8_data)
}

fn simplify_segments(params: &mut FrameParams) -> [u8; 4] {
    let mut map = [0, 1, 2, 3];
    let mut final_segments = 1_usize;

    for (source, map_entry) in map.iter_mut().enumerate().take(params.num_segments).skip(1) {
        let mut destination = 0_usize;
        while destination < final_segments
            && params.segments[source] != params.segments[destination]
        {
            destination = destination.wrapping_add(1);
        }
        *map_entry = destination.to_le_bytes()[0];
        if destination == final_segments {
            if destination != source {
                params.segments[destination] = params.segments[source];
            }
            final_segments = final_segments.wrapping_add(1);
        }
    }

    params.num_segments = final_segments;
    for segment in final_segments..params.segments.len() {
        params.segments[segment] = params.segments[final_segments.wrapping_sub(1)];
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
    let _ = cleanup_transparent_area(&rgba, 8, 9, &mut y_plane, &mut u_plane, &mut v_plane, None);
}

fn pad_plane(
    input: &[u8],
    width: usize,
    height: usize,
    padded_width: usize,
    padded_height: usize,
) -> Vec<u8> {
    if width == padded_width && height == padded_height {
        return input.to_vec();
    }
    let mut output = vec![0; padded_width.wrapping_mul(padded_height)];
    for y in 0..padded_height {
        let source_y = y.min(height.saturating_sub(1));
        for x in 0..padded_width {
            output[y.wrapping_mul(padded_width).wrapping_add(x)] = input[source_y
                .wrapping_mul(width)
                .wrapping_add(x.min(width.saturating_sub(1)))];
        }
    }
    output
}

// VP8 operates on 16x16 macroblocks, so non-aligned input dimensions require
// an O(padded-pixel) edge-replication pass before analysis. Aligned planes
// need no replication and take the direct-clone path. Keep the ordinary
// encoder on the original tight loop, while the caller-controlled path polls
// the shared Y/U/V padding work at the same 1,024-item granularity as the
// surrounding WebP preparation stages. The allocation itself remains covered
// by the existing no-recoverable-OOM policy.
fn pad_plane_with_token(
    input: &[u8],
    width: usize,
    height: usize,
    padded_width: usize,
    padded_height: usize,
    token: Option<&crate::CancellationToken>,
    padding_items: &mut usize,
) -> CodecResult<Vec<u8>> {
    if width == padded_width && height == padded_height {
        return Ok(input.to_vec());
    }
    let Some(token) = token else {
        return Ok(pad_plane(input, width, height, padded_width, padded_height));
    };
    let mut output = vec![0; padded_width.wrapping_mul(padded_height)];
    for y in 0..padded_height {
        let source_y = y.min(height.saturating_sub(1));
        for x in 0..padded_width {
            output[y.wrapping_mul(padded_width).wrapping_add(x)] = input[source_y
                .wrapping_mul(width)
                .wrapping_add(x.min(width.saturating_sub(1)))];
            *padding_items = (*padding_items).saturating_add(1);
            if (*padding_items).is_multiple_of(PAD_CHECKPOINT_ITEMS) {
                crate::codecs::error::check_cancelled(Some(token))?;
            }
        }
    }
    Ok(output)
}

// ===========================================================================
// Bitstream helpers
// ===========================================================================

const YUV_FIX: i32 = 16;
const YUV_HALF: i32 = 1 << (YUV_FIX - 1);
const GAMMA_FIX: i32 = 12;
const GAMMA_TAB_FIX: i32 = 7;
const GAMMA_TAB_SIZE: usize = 1 << (GAMMA_FIX - GAMMA_TAB_FIX);
const YUV_CHECKPOINT_ITEMS: usize = 1_024;
const PAD_CHECKPOINT_ITEMS: usize = 1_024;
const TRANSPARENT_AREA_CHECKPOINT_PIXELS: usize = 1_024;

trait TransparentAreaCheckpoint {
    fn observe(&mut self) -> CodecResult<()>;

    fn fill_block(
        &mut self,
        plane: &mut [u8],
        stride: usize,
        origin_x: usize,
        origin_y: usize,
        size: usize,
        value: u8,
    ) -> CodecResult<()>;
}

struct NoopTransparentAreaCheckpoint;

impl TransparentAreaCheckpoint for NoopTransparentAreaCheckpoint {
    #[inline(always)]
    fn observe(&mut self) -> CodecResult<()> {
        Ok(())
    }

    #[inline(always)]
    fn fill_block(
        &mut self,
        plane: &mut [u8],
        stride: usize,
        origin_x: usize,
        origin_y: usize,
        size: usize,
        value: u8,
    ) -> CodecResult<()> {
        fill_block(plane, stride, origin_x, origin_y, size, value);
        Ok(())
    }
}

struct TokenTransparentAreaCheckpoint<'a> {
    token: &'a crate::CancellationToken,
    pixels_until_checkpoint: usize,
}

impl<'a> TokenTransparentAreaCheckpoint<'a> {
    fn new(token: &'a crate::CancellationToken) -> Self {
        Self {
            token,
            pixels_until_checkpoint: TRANSPARENT_AREA_CHECKPOINT_PIXELS,
        }
    }
}

impl TransparentAreaCheckpoint for TokenTransparentAreaCheckpoint<'_> {
    #[inline]
    fn observe(&mut self) -> CodecResult<()> {
        self.pixels_until_checkpoint = self.pixels_until_checkpoint.saturating_sub(1);
        if self.pixels_until_checkpoint == 0 {
            crate::codecs::error::check_cancelled(Some(self.token))?;
            self.pixels_until_checkpoint = TRANSPARENT_AREA_CHECKPOINT_PIXELS;
        }
        Ok(())
    }

    #[inline]
    fn fill_block(
        &mut self,
        plane: &mut [u8],
        stride: usize,
        origin_x: usize,
        origin_y: usize,
        size: usize,
        value: u8,
    ) -> CodecResult<()> {
        for y in origin_y..origin_y.saturating_add(size) {
            for x in origin_x..origin_x.saturating_add(size) {
                plane[y.wrapping_mul(stride).wrapping_add(x)] = value;
                self.observe()?;
            }
        }
        Ok(())
    }
}

fn low_u32(value: usize) -> u32 {
    let bytes = value.to_le_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn pixel_offset(row: usize, width: usize, column: usize, channels: usize) -> usize {
    row.wrapping_mul(width)
        .wrapping_add(column)
        .wrapping_mul(channels)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn rounded_gamma_u16(value: f64) -> u16 {
    value as u16
}

#[allow(clippy::cast_possible_truncation)]
fn rounded_gamma_i32(value: f64) -> i32 {
    value as i32
}

fn rgb_to_y(r: i32, g: i32, b: i32) -> u8 {
    let luma = 16_839_i32
        .wrapping_mul(r)
        .wrapping_add(33_059_i32.wrapping_mul(g))
        .wrapping_add(6_420_i32.wrapping_mul(b));
    luma.wrapping_add(YUV_HALF)
        .wrapping_add(16_i32.wrapping_shl(YUV_FIX.cast_unsigned()))
        .wrapping_shr(YUV_FIX.cast_unsigned())
        .to_le_bytes()[0]
}

fn clip_uv(value: i32) -> u8 {
    let shift = YUV_FIX.wrapping_add(2).cast_unsigned();
    value
        .wrapping_add(YUV_HALF.wrapping_shl(2))
        .wrapping_add(128_i32.wrapping_shl(shift))
        .wrapping_shr(shift)
        .clamp(0, 255)
        .to_le_bytes()[0]
}

fn rgb_to_u(r: i32, g: i32, b: i32) -> u8 {
    clip_uv(
        (-9_719_i32)
            .wrapping_mul(r)
            .wrapping_sub(19_081_i32.wrapping_mul(g))
            .wrapping_add(28_800_i32.wrapping_mul(b)),
    )
}

fn rgb_to_v(r: i32, g: i32, b: i32) -> u8 {
    clip_uv(
        28_800_i32
            .wrapping_mul(r)
            .wrapping_sub(24_116_i32.wrapping_mul(g))
            .wrapping_sub(4_684_i32.wrapping_mul(b)),
    )
}

fn gamma_tables() -> &'static ([u16; 256], [i32; GAMMA_TAB_SIZE + 1]) {
    use std::sync::OnceLock;

    static TABLES: OnceLock<([u16; 256], [i32; GAMMA_TAB_SIZE + 1])> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut gamma_to_linear = [0u16; 256];
        for (value, result) in gamma_to_linear.iter_mut().enumerate() {
            *result = rounded_gamma_u16(
                ((f64::from(value.to_le_bytes()[0]) / 255.0).powf(0.80) * 4_095.0) + 0.5,
            );
        }

        let mut linear_to_gamma = [0i32; GAMMA_TAB_SIZE + 1];
        for (value, result) in linear_to_gamma.iter_mut().enumerate() {
            let scaled = (128.0 * f64::from(value.to_le_bytes()[0])) / 4_095.0;
            *result = rounded_gamma_i32(255.0 * scaled.powf(1.0 / 0.80) + 0.5);
        }
        (gamma_to_linear, linear_to_gamma)
    })
}

fn linear_to_gamma(base_value: u32) -> i32 {
    let (_, linear_to_gamma) = gamma_tables();
    let tab_position = (base_value >> (GAMMA_TAB_FIX + 2)) as usize;
    let fraction = (base_value & ((1 << (GAMMA_TAB_FIX + 2)) - 1)) as i32;
    let span: i32 = 1_i32.wrapping_shl(GAMMA_TAB_FIX.wrapping_add(2).cast_unsigned());
    let interpolated = linear_to_gamma[tab_position]
        .wrapping_mul(span.wrapping_sub(fraction))
        .wrapping_add(linear_to_gamma[tab_position.wrapping_add(1)].wrapping_mul(fraction));
    interpolated
        .wrapping_add(1_i32.wrapping_shl(GAMMA_TAB_FIX.wrapping_sub(1).cast_unsigned()))
        .wrapping_shr(GAMMA_TAB_FIX.cast_unsigned())
}

/// Convert RGB bytes to the YUV420 planes produced by libwebp's regular import path.
pub(super) fn rgb_to_yuv_planes_internal(
    rgb: &[u8],
    width: u32,
    height: u32,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let w = width as usize;
    let h = height as usize;
    let mut y_plane = vec![0u8; w.wrapping_mul(h)];
    let uv_w = w.div_ceil(2);
    let uv_h = h.div_ceil(2);
    let mut u_plane = vec![0u8; uv_w.wrapping_mul(uv_h)];
    let mut v_plane = vec![0u8; uv_w.wrapping_mul(uv_h)];

    let mut conversion_items = 0usize;
    for row in 0..h {
        for col in 0..w {
            let idx = pixel_offset(row, w, col, 3);
            y_plane[row.wrapping_mul(w).wrapping_add(col)] = rgb_to_y(
                i32::from(rgb[idx]),
                i32::from(rgb[idx.wrapping_add(1)]),
                i32::from(rgb[idx.wrapping_add(2)]),
            );
            conversion_items = conversion_items.saturating_add(1);
            if conversion_items.is_multiple_of(YUV_CHECKPOINT_ITEMS) {
                crate::codecs::error::check_cancelled(token)?;
            }
        }
    }

    for row in 0..uv_h {
        for col in 0..uv_w {
            let r0 = row.wrapping_mul(2);
            let c0 = col.wrapping_mul(2);
            let r1 = r0.wrapping_add(1).min(h.saturating_sub(1));
            let c1 = c0.wrapping_add(1).min(w.saturating_sub(1));

            let p00 = pixel_offset(r0, w, c0, 3);
            let p01 = pixel_offset(r0, w, c1, 3);
            let p10 = pixel_offset(r1, w, c0, 3);
            let p11 = pixel_offset(r1, w, c1, 3);

            let (gamma_to_linear, _) = gamma_tables();
            let gamma_sum = |channel: usize| {
                u32::from(gamma_to_linear[usize::from(rgb[p00.wrapping_add(channel)])])
                    .wrapping_add(u32::from(
                        gamma_to_linear[usize::from(rgb[p01.wrapping_add(channel)])],
                    ))
                    .wrapping_add(u32::from(
                        gamma_to_linear[usize::from(rgb[p10.wrapping_add(channel)])],
                    ))
                    .wrapping_add(u32::from(
                        gamma_to_linear[usize::from(rgb[p11.wrapping_add(channel)])],
                    ))
            };
            let r = linear_to_gamma(gamma_sum(0));
            let g = linear_to_gamma(gamma_sum(1));
            let b = linear_to_gamma(gamma_sum(2));
            let uv_idx = row.wrapping_mul(uv_w).wrapping_add(col);
            u_plane[uv_idx] = rgb_to_u(r, g, b);
            v_plane[uv_idx] = rgb_to_v(r, g, b);
            conversion_items = conversion_items.saturating_add(1);
            if conversion_items.is_multiple_of(YUV_CHECKPOINT_ITEMS) {
                crate::codecs::error::check_cancelled(token)?;
            }
        }
    }

    Ok((y_plane, u_plane, v_plane))
}

#[allow(clippy::too_many_arguments)]
fn smoothen_transparent_luma<C: TransparentAreaCheckpoint>(
    rgba: &[u8],
    image_width: usize,
    y_plane: &mut [u8],
    origin_x: usize,
    origin_y: usize,
    width: usize,
    height: usize,
    checkpoint: &mut C,
) -> CodecResult<bool> {
    let mut sum = 0usize;
    let mut count = 0usize;
    for y in origin_y..origin_y.saturating_add(height) {
        for x in origin_x..origin_x.saturating_add(width) {
            let rgba_offset = pixel_offset(y, image_width, x, 4);
            let plane_offset = y.wrapping_mul(image_width).wrapping_add(x);
            if rgba[rgba_offset.wrapping_add(3)] != 0 {
                count = count.wrapping_add(1);
                sum = sum.wrapping_add(usize::from(y_plane[plane_offset]));
            }
            checkpoint.observe()?;
        }
    }
    if count > 0 && count < width.wrapping_mul(height) {
        let average = sum.checked_div(count).unwrap_or_default().to_le_bytes()[0];
        for y in origin_y..origin_y.saturating_add(height) {
            for x in origin_x..origin_x.saturating_add(width) {
                let rgba_offset = pixel_offset(y, image_width, x, 4);
                let plane_offset = y.wrapping_mul(image_width).wrapping_add(x);
                if rgba[rgba_offset.wrapping_add(3)] == 0 {
                    y_plane[plane_offset] = average;
                }
                checkpoint.observe()?;
            }
        }
    }
    Ok(count == 0)
}

fn fill_block(
    plane: &mut [u8],
    stride: usize,
    origin_x: usize,
    origin_y: usize,
    size: usize,
    value: u8,
) {
    for y in origin_y..origin_y.saturating_add(size) {
        let start = y.wrapping_mul(stride).wrapping_add(origin_x);
        let end = start.saturating_add(size);
        plane[start..end].fill(value);
    }
}

fn cleanup_transparent_area(
    rgba: &[u8],
    width: usize,
    height: usize,
    y_plane: &mut [u8],
    u_plane: &mut [u8],
    v_plane: &mut [u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    match token {
        Some(token) => {
            let mut checkpoint = TokenTransparentAreaCheckpoint::new(token);
            cleanup_transparent_area_with_checkpoint(
                rgba,
                width,
                height,
                y_plane,
                u_plane,
                v_plane,
                &mut checkpoint,
            )
        }
        None => {
            let mut checkpoint = NoopTransparentAreaCheckpoint;
            cleanup_transparent_area_with_checkpoint(
                rgba,
                width,
                height,
                y_plane,
                u_plane,
                v_plane,
                &mut checkpoint,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn cleanup_transparent_area_with_checkpoint<C: TransparentAreaCheckpoint>(
    rgba: &[u8],
    width: usize,
    height: usize,
    y_plane: &mut [u8],
    u_plane: &mut [u8],
    v_plane: &mut [u8],
    checkpoint: &mut C,
) -> CodecResult<()> {
    const BLOCK: usize = 8;
    let uv_width = width.div_ceil(2);
    let full_width = width.wrapping_div(BLOCK).wrapping_mul(BLOCK);
    let full_height = height.wrapping_div(BLOCK).wrapping_mul(BLOCK);

    for origin_y in (0..full_height).step_by(BLOCK) {
        let mut flattened_values = None;
        for origin_x in (0..full_width).step_by(BLOCK) {
            if smoothen_transparent_luma(
                rgba, width, y_plane, origin_x, origin_y, BLOCK, BLOCK, checkpoint,
            )? {
                let values = *flattened_values.get_or_insert_with(|| {
                    [
                        y_plane[origin_y.wrapping_mul(width).wrapping_add(origin_x)],
                        u_plane[(origin_y / 2)
                            .wrapping_mul(uv_width)
                            .wrapping_add(origin_x / 2)],
                        v_plane[(origin_y / 2)
                            .wrapping_mul(uv_width)
                            .wrapping_add(origin_x / 2)],
                    ]
                });
                checkpoint.fill_block(y_plane, width, origin_x, origin_y, BLOCK, values[0])?;
                checkpoint.fill_block(
                    u_plane,
                    uv_width,
                    origin_x / 2,
                    origin_y / 2,
                    BLOCK / 2,
                    values[1],
                )?;
                checkpoint.fill_block(
                    v_plane,
                    uv_width,
                    origin_x / 2,
                    origin_y / 2,
                    BLOCK / 2,
                    values[2],
                )?;
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
                width.saturating_sub(full_width),
                BLOCK,
                checkpoint,
            )?;
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
                height.saturating_sub(full_height),
                checkpoint,
            )?;
        }
        if full_width < width {
            smoothen_transparent_luma(
                rgba,
                width,
                y_plane,
                full_width,
                full_height,
                width.saturating_sub(full_width),
                height.saturating_sub(full_height),
                checkpoint,
            )?;
        }
    }
    Ok(())
}

fn rgba_to_yuv_planes_internal(
    rgba: &[u8],
    width: u32,
    height: u32,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let w = width as usize;
    let h = height as usize;
    let mut y_plane = vec![0u8; w.wrapping_mul(h)];
    let uv_w = w.div_ceil(2);
    let uv_h = h.div_ceil(2);
    let mut u_plane = vec![0u8; uv_w.wrapping_mul(uv_h)];
    let mut v_plane = vec![0u8; uv_w.wrapping_mul(uv_h)];

    let mut conversion_items = 0usize;
    for row in 0..h {
        for col in 0..w {
            let index = pixel_offset(row, w, col, 4);
            y_plane[row.wrapping_mul(w).wrapping_add(col)] = rgb_to_y(
                i32::from(rgba[index]),
                i32::from(rgba[index.wrapping_add(1)]),
                i32::from(rgba[index.wrapping_add(2)]),
            );
            conversion_items = conversion_items.saturating_add(1);
            if conversion_items.is_multiple_of(YUV_CHECKPOINT_ITEMS) {
                crate::codecs::error::check_cancelled(token)?;
            }
        }
    }

    let (gamma_to_linear, _) = gamma_tables();
    for row in 0..uv_h {
        for col in 0..uv_w {
            let y0 = row.wrapping_mul(2);
            let x0 = col.wrapping_mul(2);
            let y1 = y0.wrapping_add(1).min(h.saturating_sub(1));
            let x1 = x0.wrapping_add(1).min(w.saturating_sub(1));
            let indices = [
                pixel_offset(y0, w, x0, 4),
                pixel_offset(y0, w, x1, 4),
                pixel_offset(y1, w, x0, 4),
                pixel_offset(y1, w, x1, 4),
            ];
            let total_alpha = indices.iter().fold(0_u32, |sum, &index| {
                sum.wrapping_add(u32::from(rgba[index.wrapping_add(3)]))
            });
            let channel_sum = |channel: usize| {
                if matches!(total_alpha, 0 | 1020) {
                    indices.iter().fold(0_u32, |sum, &index| {
                        sum.wrapping_add(u32::from(
                            gamma_to_linear[usize::from(rgba[index.wrapping_add(channel)])],
                        ))
                    })
                } else {
                    let weighted = indices.iter().fold(0_u32, |sum, &index| {
                        sum.wrapping_add(u32::from(rgba[index.wrapping_add(3)]).wrapping_mul(
                            u32::from(
                                gamma_to_linear[usize::from(rgba[index.wrapping_add(channel)])],
                            ),
                        ))
                    });
                    weighted
                        .wrapping_mul(524_288_u32.wrapping_div(total_alpha))
                        .wrapping_shr(17)
                }
            };
            let r = linear_to_gamma(channel_sum(0));
            let g = linear_to_gamma(channel_sum(1));
            let b = linear_to_gamma(channel_sum(2));
            let uv_index = row.wrapping_mul(uv_w).wrapping_add(col);
            u_plane[uv_index] = rgb_to_u(r, g, b);
            v_plane[uv_index] = rgb_to_v(r, g, b);
            conversion_items = conversion_items.saturating_add(1);
            if conversion_items.is_multiple_of(YUV_CHECKPOINT_ITEMS) {
                crate::codecs::error::check_cancelled(token)?;
            }
        }
    }

    cleanup_transparent_area(rgba, w, h, &mut y_plane, &mut u_plane, &mut v_plane, token)?;
    Ok((y_plane, u_plane, v_plane))
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
fn build_webp_container(
    vp8_data: &[u8],
    _width: u32,
    _height: u32,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    let vp8_chunk_size = low_u32(vp8_data.len().wrapping_add(vp8_data.len() & 1));
    let riff_size = 12_u32.wrapping_add(vp8_chunk_size);

    let mut out = Vec::with_capacity(21_usize.saturating_add(vp8_data.len()));

    // RIFF header
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WEBP");

    // VP8 chunk header
    out.extend_from_slice(b"VP8 ");
    out.extend_from_slice(&vp8_chunk_size.to_le_bytes());

    // VP8 data (includes frame header + bool-encoded data)
    extend_with_output_checkpoint(&mut out, vp8_data, token)?;

    // Pad to even length (RIFF requirement)
    if vp8_data.len() & 1 != 0 {
        out.push(0);
    }

    Ok(out)
}

fn append_chunk(
    output: &mut Vec<u8>,
    name: &[u8; 4],
    data: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    output.extend_from_slice(name);
    output.extend_from_slice(&low_u32(data.len()).to_le_bytes());
    extend_with_output_checkpoint(output, data, token)?;
    if data.len() & 1 != 0 {
        output.push(0);
    }
    Ok(())
}

fn append_vp8_chunk(
    output: &mut Vec<u8>,
    data: &[u8],
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    let padded_length = data.len().saturating_add(data.len() & 1);
    output.extend_from_slice(b"VP8 ");
    output.extend_from_slice(&low_u32(padded_length).to_le_bytes());
    extend_with_output_checkpoint(output, data, token)?;
    if data.len() & 1 != 0 {
        output.push(0);
    }
    Ok(())
}

fn build_extended_webp_container(
    vp8_data: &[u8],
    alpha_chunk: &[u8],
    width: u32,
    height: u32,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    let mut output = Vec::new();
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&[0; 4]);
    output.extend_from_slice(b"WEBP");

    let mut vp8x = Vec::with_capacity(10);
    vp8x.extend_from_slice(&[0x10, 0, 0, 0]);
    vp8x.extend_from_slice(&width.wrapping_sub(1).to_le_bytes()[..3]);
    vp8x.extend_from_slice(&height.wrapping_sub(1).to_le_bytes()[..3]);
    append_chunk(&mut output, b"VP8X", &vp8x, token)?;
    append_chunk(&mut output, b"ALPH", alpha_chunk, token)?;
    append_vp8_chunk(&mut output, vp8_data, token)?;

    let riff_size = low_u32(output.len().saturating_sub(8));
    output[4..8].copy_from_slice(&riff_size.to_le_bytes());
    Ok(output)
}
