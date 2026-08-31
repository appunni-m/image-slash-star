//! Scalar-authoritative, safely vectorized AV1 motion compensation.
//!
//! The checked public boundary operates on immutable retained-plane views and
//! one fallibly allocated tile scratch arena. Integer/edge/scaled predicates
//! are shared by scalar and SIMD paths so optimization cannot change syntax
//! or rounding behavior.
//!
//! The interpolation tables and arithmetic are an altered safe-Rust
//! translation of the pinned dav1d/libaom AV1 reference material. The
//! applicable BSD-2-Clause and patent notices are retained in `NOTICE.md`,
//! `PATENTS`, and `third_party/`.

#![allow(
    dead_code,
    reason = "motion-compensation kernels are wired incrementally by the inter decoder"
)]

use bytemuck::cast;
use wide::{i16x8, i32x8, u16x8};

use super::motion::{InterpolationFilter, MotionVector, ScaleFactors};
use super::sample_depth::SampleDepth;
use super::surface::PlaneView;
use super::{Av1Result, malformed};
use crate::codecs::CodecError;

const MAX_BLOCK_EDGE: usize = 128;
const MAX_BLOCK_SAMPLES: usize = MAX_BLOCK_EDGE * MAX_BLOCK_EDGE;
const HORIZONTAL_ROWS: usize = MAX_BLOCK_EDGE + 7;
const HORIZONTAL_SAMPLES: usize = MAX_BLOCK_EDGE * HORIZONTAL_ROWS;

/// One tile's reusable motion-compensation storage.
pub(super) struct MotionScratch {
    compound: [Vec<i16>; 2],
    predictor: Vec<u16>,
    mask: Vec<u8>,
    horizontal: Vec<i16>,
    edge: Vec<u16>,
    warp: Vec<i16>,
}

impl MotionScratch {
    pub(super) fn new() -> Av1Result<Self> {
        Ok(Self {
            compound: [
                allocate_zeroed(MAX_BLOCK_SAMPLES, "first compound predictor")?,
                allocate_zeroed(MAX_BLOCK_SAMPLES, "second compound predictor")?,
            ],
            predictor: allocate_zeroed(MAX_BLOCK_SAMPLES, "motion predictor")?,
            mask: allocate_zeroed(MAX_BLOCK_SAMPLES, "compound mask")?,
            horizontal: allocate_zeroed(HORIZONTAL_SAMPLES, "motion horizontal ring")?,
            edge: allocate_zeroed(320, "motion edge row")?,
            warp: allocate_zeroed(15 * 8, "motion warp intermediate")?,
        })
    }

    fn length(width: usize, height: usize) -> Av1Result<usize> {
        if width == 0 || height == 0 || width > MAX_BLOCK_EDGE || height > MAX_BLOCK_EDGE {
            return Err(malformed("motion predictor has invalid block geometry"));
        }
        width
            .checked_mul(height)
            .ok_or_else(|| malformed("motion predictor size overflows"))
    }

    pub(super) fn predictor(&self, width: usize, height: usize) -> Av1Result<&[u16]> {
        let length = Self::length(width, height)?;
        self.predictor
            .get(..length)
            .ok_or_else(|| malformed("motion predictor exceeds scratch"))
    }

    pub(super) fn predictor_mut(&mut self, width: usize, height: usize) -> Av1Result<&mut [u16]> {
        let length = Self::length(width, height)?;
        self.predictor
            .get_mut(..length)
            .ok_or_else(|| malformed("motion predictor exceeds scratch"))
    }

    pub(super) fn compound(&self, index: usize, length: usize) -> Av1Result<&[i16]> {
        self.compound
            .get(index)
            .and_then(|values| values.get(..length))
            .ok_or_else(|| malformed("compound predictor exceeds scratch"))
    }

    pub(super) fn mask(&self, length: usize) -> Av1Result<&[u8]> {
        self.mask
            .get(..length)
            .ok_or_else(|| malformed("compound mask exceeds scratch"))
    }

    pub(super) fn put_unscaled(
        &mut self,
        reference: PlaneView<'_>,
        request: PredictionRequest,
    ) -> Av1Result<&[u16]> {
        let geometry = request.geometry()?;
        let length = Self::length(geometry.width, geometry.height)?;
        let output = self
            .predictor
            .get_mut(..length)
            .ok_or_else(|| malformed("motion predictor exceeds scratch"))?;
        put_unscaled_kernel(reference, request, geometry, output)?;
        Ok(output)
    }

    pub(super) fn prep_unscaled(
        &mut self,
        index: usize,
        reference: PlaneView<'_>,
        request: PredictionRequest,
    ) -> Av1Result<&[i16]> {
        let geometry = request.geometry()?;
        let length = Self::length(geometry.width, geometry.height)?;
        let output = self
            .compound
            .get_mut(index)
            .and_then(|values| values.get_mut(..length))
            .ok_or_else(|| malformed("compound predictor exceeds scratch"))?;
        prep_unscaled_kernel(reference, request, geometry, output)?;
        Ok(output)
    }

    pub(super) fn put_scaled(
        &mut self,
        reference: PlaneView<'_>,
        request: PredictionRequest,
        scale: ScaleFactors,
    ) -> Av1Result<&[u16]> {
        let geometry = request.geometry()?;
        let length = Self::length(geometry.width, geometry.height)?;
        let output = self
            .predictor
            .get_mut(..length)
            .ok_or_else(|| malformed("motion predictor exceeds scratch"))?;
        put_scaled_kernel(reference, request, geometry, scale, output)?;
        Ok(output)
    }

    pub(super) fn prep_scaled(
        &mut self,
        index: usize,
        reference: PlaneView<'_>,
        request: PredictionRequest,
        scale: ScaleFactors,
    ) -> Av1Result<&[i16]> {
        let geometry = request.geometry()?;
        let length = Self::length(geometry.width, geometry.height)?;
        let output = self
            .compound
            .get_mut(index)
            .and_then(|values| values.get_mut(..length))
            .ok_or_else(|| malformed("compound predictor exceeds scratch"))?;
        prep_scaled_kernel(reference, request, geometry, scale, output)?;
        Ok(output)
    }

    pub(super) fn blend_average(
        &mut self,
        width: usize,
        height: usize,
        depth: SampleDepth,
    ) -> Av1Result<&[u16]> {
        let length = Self::length(width, height)?;
        blend_compound(
            &self.compound[0][..length],
            &self.compound[1][..length],
            &mut self.predictor[..length],
            None,
            CompoundBlend::Average,
            depth,
        );
        Ok(&self.predictor[..length])
    }

    pub(super) fn blend_distance(
        &mut self,
        width: usize,
        height: usize,
        weight: u8,
        depth: SampleDepth,
    ) -> Av1Result<&[u16]> {
        let length = Self::length(width, height)?;
        if weight > 16 {
            return Err(malformed("distance compound weight exceeds sixteen"));
        }
        blend_compound(
            &self.compound[0][..length],
            &self.compound[1][..length],
            &mut self.predictor[..length],
            None,
            CompoundBlend::Distance(weight),
            depth,
        );
        Ok(&self.predictor[..length])
    }

    pub(super) fn blend_masked(
        &mut self,
        width: usize,
        height: usize,
        mask: &[u8],
        depth: SampleDepth,
    ) -> Av1Result<&[u16]> {
        let length = Self::length(width, height)?;
        let mask = mask
            .get(..length)
            .ok_or_else(|| malformed("compound mask is shorter than the block"))?;
        if mask.iter().any(|&value| value > 64) {
            return Err(malformed("compound mask sample exceeds sixty-four"));
        }
        blend_compound(
            &self.compound[0][..length],
            &self.compound[1][..length],
            &mut self.predictor[..length],
            Some(mask),
            CompoundBlend::Masked,
            depth,
        );
        Ok(&self.predictor[..length])
    }

    pub(super) fn blend_difference(
        &mut self,
        width: usize,
        height: usize,
        subsampling_x: bool,
        subsampling_y: bool,
        inverted: bool,
        depth: SampleDepth,
    ) -> Av1Result<&[u16]> {
        let length = Self::length(width, height)?;
        difference_mask_and_blend(
            &self.compound[0][..length],
            &self.compound[1][..length],
            &mut self.predictor[..length],
            &mut self.mask,
            width,
            height,
            subsampling_x,
            subsampling_y,
            inverted,
            depth,
        )?;
        Ok(&self.predictor[..length])
    }

    pub(super) fn retained_capacity(&self) -> usize {
        self.horizontal
            .len()
            .saturating_add(self.edge.len())
            .saturating_add(self.warp.len())
    }
}

fn allocate_zeroed<T: Clone + Default>(length: usize, name: &'static str) -> Av1Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| CodecError::Dimensions(format!("unable to allocate AV1 {name} scratch")))?;
    values.resize(length, T::default());
    Ok(values)
}

/// Absolute current-frame geometry for one plane prediction.
#[derive(Clone, Copy, Debug)]
pub(super) struct PredictionRequest {
    pub(super) block_x_b4: u32,
    pub(super) block_y_b4: u32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) subsampling_x: bool,
    pub(super) subsampling_y: bool,
    pub(super) motion: MotionVector,
    /// Horizontal then vertical filter.
    pub(super) filters: [InterpolationFilter; 2],
}

#[derive(Clone, Copy)]
struct PredictionGeometry {
    origin_x: i64,
    origin_y: i64,
    source_x: i64,
    source_y: i64,
    phase_x: usize,
    phase_y: usize,
    width: usize,
    height: usize,
}

impl PredictionRequest {
    fn geometry(self) -> Av1Result<PredictionGeometry> {
        let width = usize::try_from(self.width)
            .map_err(|_| malformed("motion block width exceeds usize"))?;
        let height = usize::try_from(self.height)
            .map_err(|_| malformed("motion block height exceeds usize"))?;
        MotionScratch::length(width, height)?;
        let plane_step_x = 4_u32 >> u32::from(self.subsampling_x);
        let plane_step_y = 4_u32 >> u32::from(self.subsampling_y);
        let origin_x = i64::from(self.block_x_b4)
            .checked_mul(i64::from(plane_step_x))
            .ok_or_else(|| malformed("motion block x origin overflows"))?;
        let origin_y = i64::from(self.block_y_b4)
            .checked_mul(i64::from(plane_step_y))
            .ok_or_else(|| malformed("motion block y origin overflows"))?;
        let divisor_x = 8_i64 << u32::from(self.subsampling_x);
        let divisor_y = 8_i64 << u32::from(self.subsampling_y);
        let motion_x = i64::from(self.motion.x);
        let motion_y = i64::from(self.motion.y);
        let source_x = origin_x
            .checked_add(motion_x.div_euclid(divisor_x))
            .ok_or_else(|| malformed("motion source x overflows"))?;
        let source_y = origin_y
            .checked_add(motion_y.div_euclid(divisor_y))
            .ok_or_else(|| malformed("motion source y overflows"))?;
        let phase_x = usize::try_from(if self.subsampling_x {
            motion_x.rem_euclid(16)
        } else {
            motion_x.rem_euclid(8) * 2
        })
        .map_err(|_| malformed("horizontal subpixel phase exceeds usize"))?;
        let phase_y = usize::try_from(if self.subsampling_y {
            motion_y.rem_euclid(16)
        } else {
            motion_y.rem_euclid(8) * 2
        })
        .map_err(|_| malformed("vertical subpixel phase exceeds usize"))?;
        Ok(PredictionGeometry {
            origin_x,
            origin_y,
            source_x,
            source_y,
            phase_x,
            phase_y,
            width,
            height,
        })
    }
}

const REGULAR: [[i8; 8]; 15] = [
    [0, 1, -3, 63, 4, -1, 0, 0],
    [0, 1, -5, 61, 9, -2, 0, 0],
    [0, 1, -6, 58, 14, -4, 1, 0],
    [0, 1, -7, 55, 19, -5, 1, 0],
    [0, 1, -7, 51, 24, -6, 1, 0],
    [0, 1, -8, 47, 29, -6, 1, 0],
    [0, 1, -7, 42, 33, -6, 1, 0],
    [0, 1, -7, 38, 38, -7, 1, 0],
    [0, 1, -6, 33, 42, -7, 1, 0],
    [0, 1, -6, 29, 47, -8, 1, 0],
    [0, 1, -6, 24, 51, -7, 1, 0],
    [0, 1, -5, 19, 55, -7, 1, 0],
    [0, 1, -4, 14, 58, -6, 1, 0],
    [0, 0, -2, 9, 61, -5, 1, 0],
    [0, 0, -1, 4, 63, -3, 1, 0],
];

const SMOOTH: [[i8; 8]; 15] = [
    [0, 1, 14, 31, 17, 1, 0, 0],
    [0, 0, 13, 31, 18, 2, 0, 0],
    [0, 0, 11, 31, 20, 2, 0, 0],
    [0, 0, 10, 30, 21, 3, 0, 0],
    [0, 0, 9, 29, 22, 4, 0, 0],
    [0, 0, 8, 28, 23, 5, 0, 0],
    [0, -1, 8, 27, 24, 6, 0, 0],
    [0, -1, 7, 26, 26, 7, -1, 0],
    [0, 0, 6, 24, 27, 8, -1, 0],
    [0, 0, 5, 23, 28, 8, 0, 0],
    [0, 0, 4, 22, 29, 9, 0, 0],
    [0, 0, 3, 21, 30, 10, 0, 0],
    [0, 0, 2, 20, 31, 11, 0, 0],
    [0, 0, 2, 18, 31, 13, 0, 0],
    [0, 0, 1, 17, 31, 14, 1, 0],
];

const SHARP: [[i8; 8]; 15] = [
    [-1, 1, -3, 63, 4, -1, 1, 0],
    [-1, 3, -6, 62, 8, -3, 2, -1],
    [-1, 4, -9, 60, 13, -5, 3, -1],
    [-2, 5, -11, 58, 19, -7, 3, -1],
    [-2, 5, -11, 54, 24, -9, 4, -1],
    [-2, 5, -12, 50, 30, -10, 4, -1],
    [-2, 5, -12, 45, 35, -11, 5, -1],
    [-2, 6, -12, 40, 40, -12, 6, -2],
    [-1, 5, -11, 35, 45, -12, 5, -2],
    [-1, 4, -10, 30, 50, -12, 5, -2],
    [-1, 4, -9, 24, 54, -11, 5, -2],
    [-1, 3, -7, 19, 58, -11, 5, -2],
    [-1, 3, -5, 13, 60, -9, 4, -1],
    [-1, 2, -3, 8, 62, -6, 3, -1],
    [0, 1, -1, 4, 63, -3, 1, -1],
];

const REDUCED_REGULAR: [[i8; 8]; 15] = [
    [0, 0, -2, 63, 4, -1, 0, 0],
    [0, 0, -4, 61, 9, -2, 0, 0],
    [0, 0, -5, 58, 14, -3, 0, 0],
    [0, 0, -6, 55, 19, -4, 0, 0],
    [0, 0, -6, 51, 24, -5, 0, 0],
    [0, 0, -7, 47, 29, -5, 0, 0],
    [0, 0, -6, 42, 33, -5, 0, 0],
    [0, 0, -6, 38, 38, -6, 0, 0],
    [0, 0, -5, 33, 42, -6, 0, 0],
    [0, 0, -5, 29, 47, -7, 0, 0],
    [0, 0, -5, 24, 51, -6, 0, 0],
    [0, 0, -4, 19, 55, -6, 0, 0],
    [0, 0, -3, 14, 58, -5, 0, 0],
    [0, 0, -2, 9, 61, -4, 0, 0],
    [0, 0, -1, 4, 63, -2, 0, 0],
];

const REDUCED_SMOOTH: [[i8; 8]; 15] = [
    [0, 0, 15, 31, 17, 1, 0, 0],
    [0, 0, 13, 31, 18, 2, 0, 0],
    [0, 0, 11, 31, 20, 2, 0, 0],
    [0, 0, 10, 30, 21, 3, 0, 0],
    [0, 0, 9, 29, 22, 4, 0, 0],
    [0, 0, 8, 28, 23, 5, 0, 0],
    [0, 0, 7, 27, 24, 6, 0, 0],
    [0, 0, 6, 26, 26, 6, 0, 0],
    [0, 0, 6, 24, 27, 7, 0, 0],
    [0, 0, 5, 23, 28, 8, 0, 0],
    [0, 0, 4, 22, 29, 9, 0, 0],
    [0, 0, 3, 21, 30, 10, 0, 0],
    [0, 0, 2, 20, 31, 11, 0, 0],
    [0, 0, 2, 18, 31, 13, 0, 0],
    [0, 0, 1, 17, 31, 15, 0, 0],
];

const SCALED_BILINEAR: [[i8; 8]; 15] = [
    [0, 0, 0, 60, 4, 0, 0, 0],
    [0, 0, 0, 56, 8, 0, 0, 0],
    [0, 0, 0, 52, 12, 0, 0, 0],
    [0, 0, 0, 48, 16, 0, 0, 0],
    [0, 0, 0, 44, 20, 0, 0, 0],
    [0, 0, 0, 40, 24, 0, 0, 0],
    [0, 0, 0, 36, 28, 0, 0, 0],
    [0, 0, 0, 32, 32, 0, 0, 0],
    [0, 0, 0, 28, 36, 0, 0, 0],
    [0, 0, 0, 24, 40, 0, 0, 0],
    [0, 0, 0, 20, 44, 0, 0, 0],
    [0, 0, 0, 16, 48, 0, 0, 0],
    [0, 0, 0, 12, 52, 0, 0, 0],
    [0, 0, 0, 8, 56, 0, 0, 0],
    [0, 0, 0, 4, 60, 0, 0, 0],
];

fn filter_coefficients(
    filter: InterpolationFilter,
    reduced: bool,
    phase: usize,
) -> Option<&'static [i8; 8]> {
    let phase = phase.checked_sub(1)?;
    match (filter, reduced) {
        (InterpolationFilter::Regular | InterpolationFilter::Sharp, true) => {
            REDUCED_REGULAR.get(phase)
        }
        (InterpolationFilter::Smooth, true) => REDUCED_SMOOTH.get(phase),
        (InterpolationFilter::Bilinear, _) => SCALED_BILINEAR.get(phase),
        (InterpolationFilter::Regular, false) => REGULAR.get(phase),
        (InterpolationFilter::Smooth, false) => SMOOTH.get(phase),
        (InterpolationFilter::Sharp, false) => SHARP.get(phase),
    }
}

fn intermediate_bits(depth: SampleDepth) -> i32 {
    match depth.bits() {
        8 | 10 => 4,
        12 => 2,
        _ => 0,
    }
}

fn preparation_bias(depth: SampleDepth) -> i32 {
    if depth.bits() == 8 { 0 } else { 8192 }
}

fn rounded_shift(value: i32, shift: i32) -> i32 {
    if shift <= 0 {
        return value;
    }
    value.saturating_add(1_i32 << u32::try_from(shift.saturating_sub(1)).unwrap_or(0))
        >> u32::try_from(shift).unwrap_or(0)
}

fn sample_filter(
    reference: PlaneView<'_>,
    x: i64,
    y: i64,
    horizontal: bool,
    coefficients: &[i8; 8],
) -> i32 {
    coefficients
        .iter()
        .enumerate()
        .fold(0_i32, |sum, (tap, &coefficient)| {
            let offset = i64::try_from(tap).map_or(0, |tap| tap.saturating_sub(3));
            let sample = if horizontal {
                reference.replicated(x.saturating_add(offset), y)
            } else {
                reference.replicated(x, y.saturating_add(offset))
            };
            sum.saturating_add(i32::from(sample).saturating_mul(i32::from(coefficient)))
        })
}

fn horizontal_intermediate(
    reference: PlaneView<'_>,
    x: i64,
    y: i64,
    filter: Option<&[i8; 8]>,
    bits: i32,
) -> i32 {
    filter.map_or_else(
        || i32::from(reference.replicated(x, y)) << u32::try_from(bits).unwrap_or(0),
        |filter| rounded_shift(sample_filter(reference, x, y, true, filter), 6 - bits),
    )
}

fn put_unscaled_kernel(
    reference: PlaneView<'_>,
    request: PredictionRequest,
    geometry: PredictionGeometry,
    output: &mut [u16],
) -> Av1Result<()> {
    let depth = reference.depth();
    let bits = intermediate_bits(depth);
    let maximum = i32::from(depth.maximum());
    let horizontal = filter_coefficients(request.filters[0], geometry.width <= 4, geometry.phase_x);
    let vertical = filter_coefficients(request.filters[1], geometry.height <= 4, geometry.phase_y);
    for y in 0..geometry.height {
        let source_y = geometry
            .source_y
            .saturating_add(i64::try_from(y).unwrap_or(0));
        for x in 0..geometry.width {
            let source_x = geometry
                .source_x
                .saturating_add(i64::try_from(x).unwrap_or(0));
            let value = match (horizontal, vertical) {
                (None, None) => i32::from(reference.replicated(source_x, source_y)),
                (Some(filter), None) => {
                    let sum = sample_filter(reference, source_x, source_y, true, filter);
                    let extra = 1_i32 << u32::try_from(5 - bits).unwrap_or(0);
                    sum.saturating_add(32).saturating_add(extra) >> 6
                }
                (None, Some(filter)) => rounded_shift(
                    sample_filter(reference, source_x, source_y, false, filter),
                    6,
                ),
                (horizontal, Some(vertical)) => {
                    let mut sum = 0_i32;
                    for (tap, &coefficient) in vertical.iter().enumerate() {
                        let offset = i64::try_from(tap).map_or(0, |tap| tap.saturating_sub(3));
                        let value = horizontal_intermediate(
                            reference,
                            source_x,
                            source_y.saturating_add(offset),
                            horizontal,
                            bits,
                        );
                        sum = sum.saturating_add(value.saturating_mul(i32::from(coefficient)));
                    }
                    rounded_shift(sum, 6 + bits)
                }
            };
            let index = y
                .checked_mul(geometry.width)
                .and_then(|row| row.checked_add(x))
                .ok_or_else(|| malformed("motion predictor index overflows"))?;
            output[index] = u16::try_from(value.clamp(0, maximum))
                .map_err(|_| malformed("motion predictor sample exceeds u16"))?;
        }
    }
    Ok(())
}

fn prep_unscaled_kernel(
    reference: PlaneView<'_>,
    request: PredictionRequest,
    geometry: PredictionGeometry,
    output: &mut [i16],
) -> Av1Result<()> {
    let depth = reference.depth();
    let bits = intermediate_bits(depth);
    let bias = preparation_bias(depth);
    let horizontal = filter_coefficients(request.filters[0], geometry.width <= 4, geometry.phase_x);
    let vertical = filter_coefficients(request.filters[1], geometry.height <= 4, geometry.phase_y);
    for y in 0..geometry.height {
        let source_y = geometry
            .source_y
            .saturating_add(i64::try_from(y).unwrap_or(0));
        for x in 0..geometry.width {
            let source_x = geometry
                .source_x
                .saturating_add(i64::try_from(x).unwrap_or(0));
            let value = match (horizontal, vertical) {
                (None, None) => (i32::from(reference.replicated(source_x, source_y))
                    << u32::try_from(bits).unwrap_or(0))
                .saturating_sub(bias),
                (Some(filter), None) => rounded_shift(
                    sample_filter(reference, source_x, source_y, true, filter),
                    6 - bits,
                )
                .saturating_sub(bias),
                (None, Some(filter)) => rounded_shift(
                    sample_filter(reference, source_x, source_y, false, filter),
                    6 - bits,
                )
                .saturating_sub(bias),
                (horizontal, Some(vertical)) => {
                    let mut sum = 0_i32;
                    for (tap, &coefficient) in vertical.iter().enumerate() {
                        let offset = i64::try_from(tap).map_or(0, |tap| tap.saturating_sub(3));
                        let value = horizontal_intermediate(
                            reference,
                            source_x,
                            source_y.saturating_add(offset),
                            horizontal,
                            bits,
                        );
                        sum = sum.saturating_add(value.saturating_mul(i32::from(coefficient)));
                    }
                    rounded_shift(sum, 6).saturating_sub(bias)
                }
            };
            let index = y
                .checked_mul(geometry.width)
                .and_then(|row| row.checked_add(x))
                .ok_or_else(|| malformed("compound predictor index overflows"))?;
            output[index] = i16::try_from(value)
                .map_err(|_| malformed("compound predictor sample exceeds i16"))?;
        }
    }
    Ok(())
}

fn scaled_positions(
    request: PredictionRequest,
    geometry: PredictionGeometry,
    scale: ScaleFactors,
) -> Av1Result<(i32, i32)> {
    let origin_x = i32::try_from(geometry.origin_x)
        .map_err(|_| malformed("scaled horizontal origin exceeds i32"))?;
    let origin_y = i32::try_from(geometry.origin_y)
        .map_err(|_| malformed("scaled vertical origin exceeds i32"))?;
    Ok((
        ScaleFactors::position(scale.x, origin_x, request.motion.x, request.subsampling_x)?,
        ScaleFactors::position(scale.y, origin_y, request.motion.y, request.subsampling_y)?,
    ))
}

fn scaled_horizontal(
    reference: PlaneView<'_>,
    x_position: i32,
    y: i64,
    filter: InterpolationFilter,
    reduced: bool,
    bits: i32,
) -> i32 {
    let source_x = i64::from(x_position.div_euclid(1024));
    let phase = usize::try_from(x_position.rem_euclid(1024) >> 6).unwrap_or(0);
    let coefficients = filter_coefficients(filter, reduced, phase);
    horizontal_intermediate(reference, source_x, y, coefficients, bits)
}

fn put_scaled_kernel(
    reference: PlaneView<'_>,
    request: PredictionRequest,
    geometry: PredictionGeometry,
    scale: ScaleFactors,
    output: &mut [u16],
) -> Av1Result<()> {
    let depth = reference.depth();
    let bits = intermediate_bits(depth);
    let maximum = i32::from(depth.maximum());
    let (start_x, start_y) = scaled_positions(request, geometry, scale)?;
    for y in 0..geometry.height {
        let y_position = i64::from(start_y)
            .checked_add(
                i64::try_from(y)
                    .map_err(|_| malformed("scaled row exceeds i64"))?
                    .checked_mul(i64::from(scale.step_y))
                    .ok_or_else(|| malformed("scaled row position overflows"))?,
            )
            .ok_or_else(|| malformed("scaled row position overflows"))?;
        let source_y = y_position.div_euclid(1024);
        let phase_y = usize::try_from(y_position.rem_euclid(1024) >> 6)
            .map_err(|_| malformed("scaled vertical phase exceeds usize"))?;
        let vertical = filter_coefficients(request.filters[1], geometry.height <= 4, phase_y);
        for x in 0..geometry.width {
            let x_position = i64::from(start_x)
                .checked_add(
                    i64::try_from(x)
                        .map_err(|_| malformed("scaled column exceeds i64"))?
                        .checked_mul(i64::from(scale.step_x))
                        .ok_or_else(|| malformed("scaled column position overflows"))?,
                )
                .ok_or_else(|| malformed("scaled column position overflows"))?;
            let x_position = i32::try_from(x_position)
                .map_err(|_| malformed("scaled column position exceeds i32"))?;
            let value = if let Some(vertical) = vertical {
                let mut sum = 0_i32;
                for (tap, &coefficient) in vertical.iter().enumerate() {
                    let offset = i64::try_from(tap).map_or(0, |tap| tap.saturating_sub(3));
                    let horizontal = scaled_horizontal(
                        reference,
                        x_position,
                        source_y.saturating_add(offset),
                        request.filters[0],
                        geometry.width <= 4,
                        bits,
                    );
                    sum = sum.saturating_add(horizontal.saturating_mul(i32::from(coefficient)));
                }
                rounded_shift(sum, 6 + bits)
            } else {
                let horizontal = scaled_horizontal(
                    reference,
                    x_position,
                    source_y,
                    request.filters[0],
                    geometry.width <= 4,
                    bits,
                );
                rounded_shift(horizontal, bits)
            };
            let index = y
                .checked_mul(geometry.width)
                .and_then(|row| row.checked_add(x))
                .ok_or_else(|| malformed("scaled predictor index overflows"))?;
            output[index] = u16::try_from(value.clamp(0, maximum))
                .map_err(|_| malformed("scaled predictor sample exceeds u16"))?;
        }
    }
    Ok(())
}

fn prep_scaled_kernel(
    reference: PlaneView<'_>,
    request: PredictionRequest,
    geometry: PredictionGeometry,
    scale: ScaleFactors,
    output: &mut [i16],
) -> Av1Result<()> {
    let depth = reference.depth();
    let bits = intermediate_bits(depth);
    let bias = preparation_bias(depth);
    let (start_x, start_y) = scaled_positions(request, geometry, scale)?;
    for y in 0..geometry.height {
        let y_position = i64::from(start_y)
            .checked_add(
                i64::try_from(y)
                    .map_err(|_| malformed("scaled row exceeds i64"))?
                    .checked_mul(i64::from(scale.step_y))
                    .ok_or_else(|| malformed("scaled row position overflows"))?,
            )
            .ok_or_else(|| malformed("scaled row position overflows"))?;
        let source_y = y_position.div_euclid(1024);
        let phase_y = usize::try_from(y_position.rem_euclid(1024) >> 6)
            .map_err(|_| malformed("scaled vertical phase exceeds usize"))?;
        let vertical = filter_coefficients(request.filters[1], geometry.height <= 4, phase_y);
        for x in 0..geometry.width {
            let x_position = i64::from(start_x)
                .checked_add(
                    i64::try_from(x)
                        .map_err(|_| malformed("scaled column exceeds i64"))?
                        .checked_mul(i64::from(scale.step_x))
                        .ok_or_else(|| malformed("scaled column position overflows"))?,
                )
                .ok_or_else(|| malformed("scaled column position overflows"))?;
            let x_position = i32::try_from(x_position)
                .map_err(|_| malformed("scaled column position exceeds i32"))?;
            let value = if let Some(vertical) = vertical {
                let mut sum = 0_i32;
                for (tap, &coefficient) in vertical.iter().enumerate() {
                    let offset = i64::try_from(tap).map_or(0, |tap| tap.saturating_sub(3));
                    let horizontal = scaled_horizontal(
                        reference,
                        x_position,
                        source_y.saturating_add(offset),
                        request.filters[0],
                        geometry.width <= 4,
                        bits,
                    );
                    sum = sum.saturating_add(horizontal.saturating_mul(i32::from(coefficient)));
                }
                rounded_shift(sum, 6).saturating_sub(bias)
            } else {
                scaled_horizontal(
                    reference,
                    x_position,
                    source_y,
                    request.filters[0],
                    geometry.width <= 4,
                    bits,
                )
                .saturating_sub(bias)
            };
            let index = y
                .checked_mul(geometry.width)
                .and_then(|row| row.checked_add(x))
                .ok_or_else(|| malformed("scaled compound index overflows"))?;
            output[index] = i16::try_from(value)
                .map_err(|_| malformed("scaled compound sample exceeds i16"))?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum CompoundBlend {
    Average,
    Distance(u8),
    Masked,
}

fn narrow_samples(values: i32x8, maximum: i32) -> [u16; 8] {
    let clamped = values.max(i32x8::ZERO).min(i32x8::new([maximum; 8]));
    cast::<i16x8, u16x8>(i16x8::from_i32x8_saturate(clamped)).to_array()
}

fn blend_compound(
    first: &[i16],
    second: &[i16],
    output: &mut [u16],
    mask: Option<&[u8]>,
    blend: CompoundBlend,
    depth: SampleDepth,
) {
    let length = first.len().min(second.len()).min(output.len());
    let bits = intermediate_bits(depth);
    let bias = preparation_bias(depth);
    let maximum = i32::from(depth.maximum());
    let vector_length = length / 8 * 8;
    for offset in (0..vector_length).step_by(8) {
        let first = i32x8::from_i16x8(i16x8::new(std::array::from_fn(|lane| first[offset + lane])));
        let second = i32x8::from_i16x8(i16x8::new(std::array::from_fn(|lane| {
            second[offset + lane]
        })));
        let value = match blend {
            CompoundBlend::Average => (first + second + i32x8::new([(1 << bits) + 2 * bias; 8]))
                .unbounded_shr_scalar(u32::try_from(bits + 1).unwrap_or(0)),
            CompoundBlend::Distance(weight) => {
                let weight = i32::from(weight);
                (first * i32x8::new([weight; 8])
                    + second * i32x8::new([16 - weight; 8])
                    + i32x8::new([(8 << bits) + 16 * bias; 8]))
                .unbounded_shr_scalar(u32::try_from(bits + 4).unwrap_or(0))
            }
            CompoundBlend::Masked => {
                let masks = i32x8::new(std::array::from_fn(|lane| {
                    mask.and_then(|mask| mask.get(offset + lane))
                        .copied()
                        .map_or(32, i32::from)
                }));
                (first * masks
                    + second * (i32x8::new([64; 8]) - masks)
                    + i32x8::new([(32 << bits) + 64 * bias; 8]))
                .unbounded_shr_scalar(u32::try_from(bits + 6).unwrap_or(0))
            }
        };
        output[offset..offset + 8].copy_from_slice(&narrow_samples(value, maximum));
    }
    for index in vector_length..length {
        let first = i32::from(first[index]);
        let second = i32::from(second[index]);
        let value = match blend {
            CompoundBlend::Average => (first + second + (1 << bits) + 2 * bias) >> (bits + 1),
            CompoundBlend::Distance(weight) => {
                let weight = i32::from(weight);
                (first * weight + second * (16 - weight) + (8 << bits) + 16 * bias) >> (bits + 4)
            }
            CompoundBlend::Masked => {
                let mask = mask
                    .and_then(|mask| mask.get(index))
                    .copied()
                    .map_or(32, i32::from);
                (first * mask + second * (64 - mask) + (32 << bits) + 64 * bias) >> (bits + 6)
            }
        };
        output[index] = u16::try_from(value.clamp(0, maximum)).unwrap_or(depth.maximum());
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "difference-mask geometry and chroma reduction are explicit correctness inputs"
)]
fn difference_mask_and_blend(
    first: &[i16],
    second: &[i16],
    output: &mut [u16],
    mask: &mut [u8],
    width: usize,
    height: usize,
    subsampling_x: bool,
    subsampling_y: bool,
    inverted: bool,
    depth: SampleDepth,
) -> Av1Result<()> {
    let length = width
        .checked_mul(height)
        .ok_or_else(|| malformed("difference compound size overflows"))?;
    if first.len() < length || second.len() < length || output.len() < length {
        return Err(malformed("difference compound buffers are too short"));
    }
    let mask_width = if subsampling_x {
        width.div_ceil(2)
    } else {
        width
    };
    let mask_height = if subsampling_y {
        height.div_ceil(2)
    } else {
        height
    };
    let mask_length = mask_width
        .checked_mul(mask_height)
        .ok_or_else(|| malformed("difference mask size overflows"))?;
    if mask.len() < mask_length {
        return Err(malformed("difference mask exceeds scratch"));
    }
    let bits = intermediate_bits(depth);
    let bias = preparation_bias(depth);
    let maximum = i32::from(depth.maximum());
    let depth_bits =
        i32::try_from(depth.bits()).map_err(|_| malformed("sample depth exceeds i32"))?;
    let mask_shift = depth_bits + bits - 4;
    let mask_round = 1_i32 << u32::try_from(mask_shift - 5).unwrap_or(0);
    let mut full_row = [0_u8; MAX_BLOCK_EDGE];
    let mut previous_pair_sums = [0_u16; MAX_BLOCK_EDGE / 2];
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            let difference = i32::from(first[index]).saturating_sub(i32::from(second[index]));
            let magnitude = difference
                .unsigned_abs()
                .saturating_add(u32::try_from(mask_round).unwrap_or(0))
                .checked_shr(u32::try_from(mask_shift).unwrap_or(0))
                .unwrap_or(0);
            let magnitude = i32::try_from(magnitude)
                .map_err(|_| malformed("difference mask magnitude exceeds i32"))?;
            let raw_mask = 38_i32.saturating_add(magnitude).min(64);
            let blend_mask = if inverted { 64 - raw_mask } else { raw_mask };
            full_row[x] = u8::try_from(raw_mask)
                .map_err(|_| malformed("difference mask sample exceeds u8"))?;
            let value = (difference * blend_mask
                + i32::from(second[index]) * 64
                + (32 << bits)
                + bias * 64)
                >> (bits + 6);
            output[index] = u16::try_from(value.clamp(0, maximum))
                .map_err(|_| malformed("difference compound sample exceeds u16"))?;
        }
        if subsampling_x {
            for x in 0..mask_width {
                let first_x = x * 2;
                let second_x = first_x.saturating_add(1).min(width.saturating_sub(1));
                let pair_sum = u16::from(full_row[first_x]) + u16::from(full_row[second_x]);
                if subsampling_y && y % 2 == 0 && y + 1 < height {
                    previous_pair_sums[x] = pair_sum;
                } else {
                    let mask_y = y / (usize::from(subsampling_y) + 1);
                    let destination = mask_y
                        .checked_mul(mask_width)
                        .and_then(|row| row.checked_add(x))
                        .ok_or_else(|| malformed("difference mask index overflows"))?;
                    let sign = u16::from(inverted);
                    let reduced = if subsampling_y && y % 2 == 1 {
                        (previous_pair_sums[x] + pair_sum + 2 - sign) >> 2
                    } else {
                        (pair_sum + 1 - sign) >> 1
                    };
                    mask[destination] = u8::try_from(reduced)
                        .map_err(|_| malformed("reduced difference mask exceeds u8"))?;
                }
            }
        } else {
            let mask_y = y / (usize::from(subsampling_y) + 1);
            if !subsampling_y || y % 2 == 1 || y + 1 == height {
                for x in 0..width {
                    let destination = mask_y
                        .checked_mul(mask_width)
                        .and_then(|row| row.checked_add(x))
                        .ok_or_else(|| malformed("difference mask index overflows"))?;
                    mask[destination] = full_row[x];
                }
            }
        }
    }
    Ok(())
}

/// Exact AV1 distance-compound weight for two reference order distances.
pub(super) fn distance_weight(first: i32, second: i32) -> u8 {
    const THRESHOLDS: [[u32; 2]; 3] = [[2, 3], [2, 5], [2, 7]];
    const WEIGHTS: [[u8; 2]; 4] = [[9, 7], [11, 5], [12, 4], [13, 3]];
    // dav1d names these in reverse reference order: d1 is ref0 and d0 is
    // ref1. Preserve that ordering because it also selects the returned
    // weight column.
    let d1 = first.unsigned_abs().min(31);
    let d0 = second.unsigned_abs().min(31);
    let order = usize::from(d0 <= d1);
    let mut index = THRESHOLDS.len();
    for (candidate, threshold) in THRESHOLDS.iter().enumerate() {
        let c0 = threshold[order];
        let c1 = threshold[usize::from(order == 0)];
        let d0_c0 = d0.saturating_mul(c0);
        let d1_c1 = d1.saturating_mul(c1);
        if (d0 > d1 && d0_c0 < d1_c1) || (d0 <= d1 && d0_c0 > d1_c1) {
            index = candidate;
            break;
        }
    }
    WEIGHTS[index][order]
}

const OBMC_MASKS: [u8; 64] = [
    0, 0, 19, 0, 25, 14, 5, 0, 28, 22, 16, 11, 7, 3, 0, 0, 30, 27, 24, 21, 18, 15, 12, 10, 8, 6, 4,
    3, 0, 0, 0, 0, 31, 29, 28, 26, 24, 23, 21, 20, 19, 17, 16, 14, 13, 12, 11, 9, 8, 7, 6, 5, 4, 4,
    3, 2, 0, 0, 0, 0, 0, 0, 0, 0,
];

fn validate_obmc_extent(extent: usize) -> Av1Result<usize> {
    if !matches!(extent, 2 | 4 | 8 | 16 | 32) {
        return Err(malformed("OBMC overlap has a non-normative extent"));
    }
    Ok(extent.saturating_mul(3) >> 2)
}

fn blend_obmc_row(destination: &mut [u16], neighbor: &[u16], mask: u8) -> Av1Result<()> {
    if destination.len() != neighbor.len() {
        return Err(malformed("OBMC row buffers have different lengths"));
    }
    let mask = i32x8::new([i32::from(mask); 8]);
    let complement = i32x8::new([64; 8]) - mask;
    let vector_length = destination.len() / 8 * 8;
    for offset in (0..vector_length).step_by(8) {
        let current = i32x8::new(std::array::from_fn(|lane| {
            i32::from(destination[offset + lane])
        }));
        let staged = i32x8::new(std::array::from_fn(|lane| {
            i32::from(neighbor[offset + lane])
        }));
        let value =
            (current * complement + staged * mask + i32x8::new([32; 8])).unbounded_shr_scalar(6);
        destination[offset..offset + 8]
            .copy_from_slice(&narrow_samples(value, i32::from(u16::MAX)));
    }
    for (current, staged) in destination[vector_length..]
        .iter_mut()
        .zip(&neighbor[vector_length..])
    {
        let mask = i32::from(mask.to_array()[0]);
        let value = (i32::from(*current) * (64 - mask) + i32::from(*staged) * mask + 32) >> 6;
        *current = u16::try_from(value).map_err(|_| malformed("OBMC blend sample exceeds u16"))?;
    }
    Ok(())
}

/// Blend a top-neighbor predictor into the first three quarters of a full
/// overlap-height strip. The neighbor predictor may contain one extra rounded
/// source row; AV1 deliberately leaves that row unused by the mask.
pub(super) fn blend_obmc_top(
    destination: &mut [u16],
    dst_stride: usize,
    x_offset: usize,
    neighbor: &[u16],
    overlap_width: usize,
    overlap_height: usize,
) -> Av1Result<()> {
    let active_height = validate_obmc_extent(overlap_height)?;
    let destination_end = x_offset
        .checked_add(overlap_width)
        .ok_or_else(|| malformed("OBMC top destination span overflows"))?;
    if overlap_width == 0 || destination_end > dst_stride {
        return Err(malformed("OBMC top destination span is invalid"));
    }
    let neighbor_rows = neighbor
        .len()
        .checked_div(overlap_width)
        .ok_or_else(|| malformed("OBMC top neighbor width is invalid"))?;
    if neighbor_rows < active_height {
        return Err(malformed("OBMC top neighbor rows are too short"));
    }
    let destination_rows = destination
        .len()
        .checked_div(dst_stride)
        .ok_or_else(|| malformed("OBMC top destination stride is invalid"))?;
    if destination_rows < active_height {
        return Err(malformed("OBMC top destination rows are too short"));
    }
    for row in 0..active_height {
        let destination_start = row
            .checked_mul(dst_stride)
            .and_then(|offset| offset.checked_add(x_offset))
            .ok_or_else(|| malformed("OBMC top destination row overflows"))?;
        let destination_end = destination_start
            .checked_add(overlap_width)
            .ok_or_else(|| malformed("OBMC top destination row exceeds buffer"))?;
        let neighbor_start = row
            .checked_mul(overlap_width)
            .ok_or_else(|| malformed("OBMC top neighbor row overflows"))?;
        let neighbor_end = neighbor_start
            .checked_add(overlap_width)
            .ok_or_else(|| malformed("OBMC top neighbor row exceeds buffer"))?;
        let mask = *OBMC_MASKS
            .get(overlap_height + row)
            .ok_or_else(|| malformed("OBMC top mask index exceeds table"))?;
        blend_obmc_row(
            destination
                .get_mut(destination_start..destination_end)
                .ok_or_else(|| malformed("OBMC top destination row is unavailable"))?,
            neighbor
                .get(neighbor_start..neighbor_end)
                .ok_or_else(|| malformed("OBMC top neighbor row is unavailable"))?,
            mask,
        )?;
    }
    Ok(())
}

/// Blend a left-neighbor predictor into the first three quarters of a full
/// overlap-width strip. The top pass is applied before this pass, so the
/// corner receives AV1's intentional two-stage blend.
pub(super) fn blend_obmc_left(
    destination: &mut [u16],
    dst_stride: usize,
    y_offset: usize,
    neighbor: &[u16],
    overlap_width: usize,
    overlap_height: usize,
) -> Av1Result<()> {
    let active_width = validate_obmc_extent(overlap_width)?;
    if overlap_height == 0 || overlap_width == 0 {
        return Err(malformed("OBMC left overlap has zero extent"));
    }
    let neighbor_length = overlap_width
        .checked_mul(overlap_height)
        .ok_or_else(|| malformed("OBMC left neighbor size overflows"))?;
    if neighbor.len() < neighbor_length {
        return Err(malformed("OBMC left neighbor rows are too short"));
    }
    if dst_stride == 0 || active_width > dst_stride {
        return Err(malformed("OBMC left destination stride is invalid"));
    }
    let destination_start = y_offset
        .checked_mul(dst_stride)
        .ok_or_else(|| malformed("OBMC left destination offset overflows"))?;
    let destination_last = y_offset
        .checked_add(overlap_height.saturating_sub(1))
        .and_then(|row| row.checked_mul(dst_stride))
        .and_then(|row| row.checked_add(active_width))
        .ok_or_else(|| malformed("OBMC left destination span overflows"))?;
    if destination_start > destination.len() || destination_last > destination.len() {
        return Err(malformed("OBMC left destination span is invalid"));
    }
    for row in 0..overlap_height {
        let destination_row = y_offset
            .checked_add(row)
            .and_then(|value| value.checked_mul(dst_stride))
            .ok_or_else(|| malformed("OBMC left destination row overflows"))?;
        let neighbor_row = row
            .checked_mul(overlap_width)
            .ok_or_else(|| malformed("OBMC left neighbor row overflows"))?;
        let vector_length = active_width / 8 * 8;
        for offset in (0..vector_length).step_by(8) {
            let current = i32x8::new(std::array::from_fn(|lane| {
                i32::from(destination[destination_row + offset + lane])
            }));
            let staged = i32x8::new(std::array::from_fn(|lane| {
                i32::from(neighbor[neighbor_row + offset + lane])
            }));
            let masks = i32x8::new(std::array::from_fn(|lane| {
                i32::from(OBMC_MASKS[overlap_width + offset + lane])
            }));
            let value =
                (current * (i32x8::new([64; 8]) - masks) + staged * masks + i32x8::new([32; 8]))
                    .unbounded_shr_scalar(6);
            destination[destination_row + offset..destination_row + offset + 8]
                .copy_from_slice(&narrow_samples(value, i32::from(u16::MAX)));
        }
        for offset in vector_length..active_width {
            let destination_index = destination_row
                .checked_add(offset)
                .ok_or_else(|| malformed("OBMC left destination index overflows"))?;
            let neighbor_index = neighbor_row
                .checked_add(offset)
                .ok_or_else(|| malformed("OBMC left neighbor index overflows"))?;
            let mask = i32::from(
                *OBMC_MASKS
                    .get(overlap_width + offset)
                    .ok_or_else(|| malformed("OBMC left mask index exceeds table"))?,
            );
            let value = (i32::from(
                *destination
                    .get(destination_index)
                    .ok_or_else(|| malformed("OBMC left destination sample is unavailable"))?,
            ) * (64 - mask)
                + i32::from(
                    *neighbor
                        .get(neighbor_index)
                        .ok_or_else(|| malformed("OBMC left neighbor sample is unavailable"))?,
                ) * mask
                + 32)
                >> 6;
            *destination
                .get_mut(destination_index)
                .ok_or_else(|| malformed("OBMC left destination sample is unavailable"))? =
                u16::try_from(value).map_err(|_| malformed("OBMC blend sample exceeds u16"))?;
        }
    }
    Ok(())
}
