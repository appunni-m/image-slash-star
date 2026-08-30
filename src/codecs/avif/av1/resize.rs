//! AV1 normative horizontal super-resolution for retained 4:2:0 pictures.
//!
//! The interpolation table and fixed-point placement follow the pinned
//! libaom `av1/common/resize.c` / `aom_dsp/aom_filter.h` implementation and
//! dav1d `src/decode.c`, `src/mc_tmpl.c`, and `src/tables.c`. The corresponding
//! notices are retained in `NOTICE.md` and `third_party/libaom/`.

use super::block::{FirstLeaf, ReconstructedPlane};
use super::sample_depth::SampleDepth;
use super::{Av1Result, malformed};
use crate::codecs::CodecError;

const RS_SCALE_SUBPEL_SHIFT: u32 = 14;
const RS_SCALE_SUBPEL_BITS: i64 = 14;
const RS_SCALE_SUBPEL_MASK: i64 = (1_i64 << RS_SCALE_SUBPEL_BITS) - 1;
const RS_SCALE_EXTRA_BITS: i64 = 8;
const RS_SCALE_EXTRA_OFF: i64 = 1_i64 << (RS_SCALE_EXTRA_BITS - 1);
const FILTER_BITS: i64 = 7;

// Conventional libaom sign convention: each phase sums to +128 and the
// rounded result is (sum + 64) >> 7. The table is intentionally kept local so
// the scalar path remains the parity authority for any future SIMD kernel.
const RESIZE_FILTER: [[i16; 8]; 64] = [
    [0, 0, 0, 128, 0, 0, 0, 0],
    [0, 0, -1, 128, 2, -1, 0, 0],
    [0, 1, -3, 127, 4, -2, 1, 0],
    [0, 1, -4, 127, 6, -3, 1, 0],
    [0, 2, -6, 126, 8, -3, 1, 0],
    [0, 2, -7, 125, 11, -4, 1, 0],
    [-1, 2, -8, 125, 13, -5, 2, 0],
    [-1, 3, -9, 124, 15, -6, 2, 0],
    [-1, 3, -10, 123, 18, -6, 2, -1],
    [-1, 3, -11, 122, 20, -7, 3, -1],
    [-1, 4, -12, 121, 22, -8, 3, -1],
    [-1, 4, -13, 120, 25, -9, 3, -1],
    [-1, 4, -14, 118, 28, -9, 3, -1],
    [-1, 4, -15, 117, 30, -10, 4, -1],
    [-1, 5, -16, 116, 32, -11, 4, -1],
    [-1, 5, -16, 114, 35, -12, 4, -1],
    [-1, 5, -17, 112, 38, -12, 4, -1],
    [-1, 5, -18, 111, 40, -13, 5, -1],
    [-1, 5, -18, 109, 43, -14, 5, -1],
    [-1, 6, -19, 107, 45, -14, 5, -1],
    [-1, 6, -19, 105, 48, -15, 5, -1],
    [-1, 6, -19, 103, 51, -16, 5, -1],
    [-1, 6, -20, 101, 53, -16, 6, -1],
    [-1, 6, -20, 99, 56, -17, 6, -1],
    [-1, 6, -20, 97, 58, -17, 6, -1],
    [-1, 6, -20, 95, 61, -18, 6, -1],
    [-2, 7, -20, 93, 64, -18, 6, -2],
    [-2, 7, -20, 91, 66, -19, 6, -1],
    [-2, 7, -20, 88, 69, -19, 6, -1],
    [-2, 7, -20, 86, 71, -19, 6, -1],
    [-2, 7, -20, 84, 74, -20, 7, -2],
    [-2, 7, -20, 81, 76, -20, 7, -1],
    [-2, 7, -20, 79, 79, -20, 7, -2],
    [-1, 7, -20, 76, 81, -20, 7, -2],
    [-2, 7, -20, 74, 84, -20, 7, -2],
    [-1, 6, -19, 71, 86, -20, 7, -2],
    [-1, 6, -19, 69, 88, -20, 7, -2],
    [-1, 6, -19, 66, 91, -20, 7, -2],
    [-2, 6, -18, 64, 93, -20, 7, -2],
    [-1, 6, -18, 61, 95, -20, 6, -1],
    [-1, 6, -17, 58, 97, -20, 6, -1],
    [-1, 6, -17, 56, 99, -20, 6, -1],
    [-1, 6, -16, 53, 101, -20, 6, -1],
    [-1, 5, -16, 51, 103, -19, 6, -1],
    [-1, 5, -15, 48, 105, -19, 6, -1],
    [-1, 5, -14, 45, 107, -19, 6, -1],
    [-1, 5, -14, 43, 109, -18, 5, -1],
    [-1, 5, -13, 40, 111, -18, 5, -1],
    [-1, 4, -12, 38, 112, -17, 5, -1],
    [-1, 4, -12, 35, 114, -16, 5, -1],
    [-1, 4, -11, 32, 116, -16, 5, -1],
    [-1, 4, -10, 30, 117, -15, 4, -1],
    [-1, 3, -9, 28, 118, -14, 4, -1],
    [-1, 3, -9, 25, 120, -13, 4, -1],
    [-1, 3, -8, 22, 121, -12, 4, -1],
    [-1, 3, -7, 20, 122, -11, 3, -1],
    [-1, 2, -6, 18, 123, -10, 3, -1],
    [0, 2, -6, 15, 124, -9, 3, -1],
    [0, 2, -5, 13, 125, -8, 2, -1],
    [0, 1, -4, 11, 125, -7, 2, 0],
    [0, 1, -3, 8, 126, -6, 2, 0],
    [0, 1, -3, 6, 127, -4, 1, 0],
    [0, 1, -2, 4, 127, -3, 1, 0],
    [0, 0, -1, 2, 128, -1, 0, 0],
];

/// Apply AV1's horizontal super-resolution step to a complete I420 leaf.
///
/// Reconstruction and all frame filters must already have run at coded width.
/// The transform consumes that private candidate, so allocation or geometry
/// failures cannot publish a partial surface or mutate pending frame state.
pub(super) fn upscale_i420_leaf(
    mut leaf: FirstLeaf,
    coded_width: u32,
    upscaled_width: u32,
    frame_height: u32,
    superres_denominator: u32,
    depth: SampleDepth,
) -> Av1Result<FirstLeaf> {
    validate_header_geometry(
        leaf.width,
        leaf.height,
        coded_width,
        upscaled_width,
        frame_height,
        superres_denominator,
    )?;
    if upscaled_width == coded_width {
        validate_plane(&leaf.planes[0], coded_width, frame_height, depth)?;
        let chroma_width = coded_width.div_ceil(2);
        let chroma_height = frame_height.div_ceil(2);
        validate_plane(&leaf.planes[1], chroma_width, chroma_height, depth)?;
        validate_plane(&leaf.planes[2], chroma_width, chroma_height, depth)?;
        return Ok(leaf);
    }

    let chroma_coded_width = coded_width.div_ceil(2);
    let chroma_upscaled_width = upscaled_width.div_ceil(2);
    let chroma_height = frame_height.div_ceil(2);
    let [luma, chroma_u, chroma_v] = leaf.planes;
    let planes = [
        resize_plane(luma, coded_width, upscaled_width, frame_height, depth)?,
        resize_plane(
            chroma_u,
            chroma_coded_width,
            chroma_upscaled_width,
            chroma_height,
            depth,
        )?,
        resize_plane(
            chroma_v,
            chroma_coded_width,
            chroma_upscaled_width,
            chroma_height,
            depth,
        )?,
    ];
    leaf.width = upscaled_width;
    leaf.planes = planes;
    Ok(leaf)
}

fn validate_header_geometry(
    leaf_width: u32,
    leaf_height: u32,
    coded_width: u32,
    upscaled_width: u32,
    frame_height: u32,
    superres_denominator: u32,
) -> Av1Result<()> {
    if coded_width == 0
        || upscaled_width == 0
        || frame_height == 0
        || coded_width > upscaled_width
        || leaf_width != coded_width
        || leaf_height != frame_height
        || !(9..=16).contains(&superres_denominator)
    {
        return Err(malformed("super-resolution geometry is inconsistent"));
    }
    let numerator = i64::from(upscaled_width)
        .checked_mul(8)
        .and_then(|value| value.checked_add(i64::from(superres_denominator >> 1)))
        .ok_or_else(|| malformed("super-resolution width overflows"))?;
    let expected = numerator
        .checked_div(i64::from(superres_denominator))
        .ok_or_else(|| malformed("super-resolution denominator is zero"))?;
    let minimum = i64::from(upscaled_width.min(16));
    let expected = expected.max(minimum);
    if i64::from(coded_width) != expected {
        return Err(malformed(
            "super-resolution width disagrees with its denominator",
        ));
    }
    Ok(())
}

fn validate_plane(
    plane: &ReconstructedPlane,
    width: u32,
    height: u32,
    depth: SampleDepth,
) -> Av1Result<()> {
    let width =
        usize::try_from(width).map_err(|_| malformed("resize plane width exceeds usize"))?;
    let height =
        usize::try_from(height).map_err(|_| malformed("resize plane height exceeds usize"))?;
    let length = width
        .checked_mul(height)
        .ok_or_else(|| malformed("resize plane extent overflows usize"))?;
    if width == 0
        || height == 0
        || plane.samples.len() != length
        || plane
            .samples
            .iter()
            .any(|&sample| depth.validate(sample).is_none())
    {
        return Err(malformed(
            "resize source plane has invalid samples or extent",
        ));
    }
    Ok(())
}

fn resize_plane(
    source: ReconstructedPlane,
    input_width: u32,
    output_width: u32,
    height: u32,
    depth: SampleDepth,
) -> Av1Result<ReconstructedPlane> {
    validate_plane(&source, input_width, height, depth)?;
    if input_width == output_width {
        return Ok(source);
    }
    if input_width == 0 || output_width == 0 || input_width > output_width {
        return Err(malformed("resize plane has invalid horizontal geometry"));
    }
    let input_width =
        usize::try_from(input_width).map_err(|_| malformed("resize input width exceeds usize"))?;
    let output_width = usize::try_from(output_width)
        .map_err(|_| malformed("resize output width exceeds usize"))?;
    let height = usize::try_from(height).map_err(|_| malformed("resize height exceeds usize"))?;
    let length = output_width
        .checked_mul(height)
        .ok_or_else(|| malformed("resize output extent overflows usize"))?;
    let (step, x0) = resize_position(input_width, output_width)?;
    let mut positions = Vec::new();
    positions.try_reserve_exact(output_width).map_err(|_| {
        CodecError::Dimensions("unable to allocate AV1 super-resolution positions".to_owned())
    })?;
    for output_x in 0..output_width {
        let output_x = i64::try_from(output_x)
            .map_err(|_| malformed("resize output coordinate exceeds i64"))?;
        let fixed = x0
            .checked_add(
                output_x
                    .checked_mul(step)
                    .ok_or_else(|| malformed("resize position overflows i64"))?,
            )
            .ok_or_else(|| malformed("resize position overflows i64"))?;
        let center = fixed >> RS_SCALE_SUBPEL_BITS;
        let phase = usize::try_from((fixed & RS_SCALE_SUBPEL_MASK) >> RS_SCALE_EXTRA_BITS)
            .map_err(|_| malformed("resize phase exceeds filter table"))?;
        if phase >= RESIZE_FILTER.len() {
            return Err(malformed("resize phase exceeds filter table"));
        }
        positions.push((center, phase));
    }

    let mut samples = Vec::new();
    samples.try_reserve_exact(length).map_err(|_| {
        CodecError::Dimensions("unable to allocate AV1 super-resolution plane".to_owned())
    })?;
    for row in 0..height {
        let source_row = row
            .checked_mul(input_width)
            .ok_or_else(|| malformed("resize source row offset overflows"))?;
        for &(center, phase) in &positions {
            let mut sum = 0_i64;
            for (tap, &coefficient) in RESIZE_FILTER[phase].iter().enumerate() {
                let tap =
                    i64::try_from(tap).map_err(|_| malformed("resize tap index exceeds i64"))?;
                let sample_x = center
                    // `av1_upscale_normative_rows` passes the source one
                    // sample before the convolution origin; the convolver
                    // then backs up by `taps / 2 - 1` more samples. The
                    // effective source span is therefore center-4..center+3.
                    .checked_sub(4)
                    .and_then(|value| value.checked_add(tap))
                    .ok_or_else(|| malformed("resize sample coordinate overflows"))?;
                let sample_x = if sample_x <= 0 {
                    0
                } else {
                    usize::try_from(sample_x).map_or(input_width.saturating_sub(1), |value| {
                        value.min(input_width.saturating_sub(1))
                    })
                };
                let index = source_row
                    .checked_add(sample_x)
                    .ok_or_else(|| malformed("resize source index overflows"))?;
                let sample = source
                    .samples
                    .get(index)
                    .copied()
                    .ok_or_else(|| malformed("resize source index exceeds plane"))?;
                let product = i64::from(coefficient)
                    .checked_mul(i64::from(sample))
                    .ok_or_else(|| malformed("resize filter product overflows"))?;
                sum = sum
                    .checked_add(product)
                    .ok_or_else(|| malformed("resize filter sum overflows"))?;
            }
            let rounded = sum
                .checked_add(1_i64 << (FILTER_BITS - 1))
                .ok_or_else(|| malformed("resize rounding overflows"))?
                >> FILTER_BITS;
            let rounded =
                i32::try_from(rounded).map_err(|_| malformed("resize result exceeds i32"))?;
            samples.push(depth.clip_i32(rounded));
        }
    }
    Ok(ReconstructedPlane { samples })
}

fn resize_position(input_width: usize, output_width: usize) -> Av1Result<(i64, i64)> {
    let input_width =
        i64::try_from(input_width).map_err(|_| malformed("resize input width exceeds i64"))?;
    let output_width =
        i64::try_from(output_width).map_err(|_| malformed("resize output width exceeds i64"))?;
    let input_fixed = input_width
        .checked_shl(RS_SCALE_SUBPEL_SHIFT)
        .ok_or_else(|| malformed("resize input fixed-point width overflows"))?;
    let step = input_fixed
        .checked_add(output_width / 2)
        .and_then(|value| value.checked_div(output_width))
        .ok_or_else(|| malformed("resize step overflows"))?;
    let width_delta = output_width
        .checked_sub(input_width)
        .ok_or_else(|| malformed("resize output is narrower than input"))?;
    let offset_numerator = width_delta
        .checked_shl(RS_SCALE_SUBPEL_SHIFT - 1)
        .and_then(|value| value.checked_neg())
        .and_then(|value| value.checked_add(output_width / 2))
        .ok_or_else(|| malformed("resize offset overflows"))?;
    let offset = offset_numerator
        .checked_div(output_width)
        .ok_or_else(|| malformed("resize offset denominator is zero"))?;
    let error = output_width
        .checked_mul(step)
        .and_then(|value| value.checked_sub(input_fixed))
        .ok_or_else(|| malformed("resize step error overflows"))?;
    let x0 = offset
        .checked_add(RS_SCALE_EXTRA_OFF)
        .and_then(|value| value.checked_sub(error / 2))
        .ok_or_else(|| malformed("resize initial position overflows"))?
        & RS_SCALE_SUBPEL_MASK;
    Ok((step, x0))
}
