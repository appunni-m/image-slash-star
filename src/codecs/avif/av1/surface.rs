//! Immutable validated AV1 frame surfaces retained by reference slots.
//!
//! Stored planes are post-super-resolution and post-restoration but never
//! film-grained. Motion and entropy coordinates remain in the separate coded
//! width carried by the surface and its temporal motion field.

#![allow(
    dead_code,
    reason = "checked plane views are wired incrementally by the complete inter decoder"
)]

use super::block::ReconstructedPlane;
use super::geometry::PixelLayout;
use super::motion::TemporalMotionField;
use super::sample_depth::SampleDepth;
use super::{Av1Result, malformed};
use crate::codecs::CodecError;

/// A checked immutable row-major view over one retained plane.
#[derive(Clone, Copy)]
pub(super) struct PlaneView<'a> {
    width: usize,
    height: usize,
    stride: usize,
    depth: SampleDepth,
    samples: &'a [u16],
}

impl<'a> PlaneView<'a> {
    pub(super) const fn width(self) -> usize {
        self.width
    }

    pub(super) const fn height(self) -> usize {
        self.height
    }

    pub(super) const fn stride(self) -> usize {
        self.stride
    }

    pub(super) const fn depth(self) -> SampleDepth {
        self.depth
    }

    pub(super) const fn samples(self) -> &'a [u16] {
        self.samples
    }

    pub(super) fn row(self, y: usize) -> Option<&'a [u16]> {
        if y >= self.height {
            return None;
        }
        let start = y.checked_mul(self.stride)?;
        let end = start.checked_add(self.width)?;
        self.samples.get(start..end)
    }

    /// Edge-emulated sample access for normative interpolation support.
    pub(super) fn replicated(self, x: i64, y: i64) -> u16 {
        let maximum_x = self.width.saturating_sub(1);
        let maximum_y = self.height.saturating_sub(1);
        let x = if x <= 0 {
            0
        } else {
            usize::try_from(x).map_or(maximum_x, |x| x.min(maximum_x))
        };
        let y = if y <= 0 {
            0
        } else {
            usize::try_from(y).map_or(maximum_y, |y| y.min(maximum_y))
        };
        let index = y
            .checked_mul(self.stride)
            .and_then(|row| row.checked_add(x));
        index
            .and_then(|index| self.samples.get(index))
            .copied()
            .unwrap_or(0)
    }
}

/// One checked owned plane of an AV1 decoded frame.
#[derive(Clone)]
pub(super) struct FramePlane {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) stride: usize,
    pub(super) samples: Vec<u16>,
}

impl FramePlane {
    pub(super) fn validate_reconstructed(
        source: &ReconstructedPlane,
        width: u32,
        height: u32,
        depth: SampleDepth,
    ) -> Av1Result<usize> {
        let stride =
            usize::try_from(width).map_err(|_| malformed("reference plane width exceeds usize"))?;
        let height_usize = usize::try_from(height)
            .map_err(|_| malformed("reference plane height exceeds usize"))?;
        let length = stride
            .checked_mul(height_usize)
            .ok_or_else(|| malformed("reference plane size overflows usize"))?;
        if width == 0
            || height == 0
            || source.samples.len() != length
            || source
                .samples
                .iter()
                .any(|&sample| depth.validate(sample).is_none())
        {
            return Err(malformed("reference plane has invalid samples or extent"));
        }
        Ok(stride)
    }

    pub(super) fn from_validated(
        source: ReconstructedPlane,
        width: u32,
        height: u32,
        stride: usize,
    ) -> Self {
        Self {
            width,
            height,
            stride,
            samples: source.samples,
        }
    }

    pub(super) fn view(&self, depth: SampleDepth) -> Av1Result<PlaneView<'_>> {
        let width = usize::try_from(self.width)
            .map_err(|_| malformed("retained plane width exceeds usize"))?;
        let height = usize::try_from(self.height)
            .map_err(|_| malformed("retained plane height exceeds usize"))?;
        let length = self
            .stride
            .checked_mul(height)
            .ok_or_else(|| malformed("retained plane size overflows usize"))?;
        if width == 0 || height == 0 || self.stride < width || self.samples.len() != length {
            return Err(malformed("retained plane has invalid geometry"));
        }
        Ok(PlaneView {
            width,
            height,
            stride: self.stride,
            depth,
            samples: &self.samples,
        })
    }

    pub(super) fn reconstructed_copy(&self) -> Av1Result<ReconstructedPlane> {
        let width = usize::try_from(self.width)
            .map_err(|_| malformed("display plane width exceeds usize"))?;
        let height = usize::try_from(self.height)
            .map_err(|_| malformed("display plane height exceeds usize"))?;
        let retained_length = self
            .stride
            .checked_mul(height)
            .ok_or_else(|| malformed("retained display plane size overflows usize"))?;
        let length = width
            .checked_mul(height)
            .ok_or_else(|| malformed("display plane size overflows usize"))?;
        if width == 0 || height == 0 || self.stride < width || self.samples.len() != retained_length
        {
            return Err(malformed("retained display plane has invalid geometry"));
        }
        let mut samples = Vec::new();
        samples.try_reserve_exact(length).map_err(|_| {
            CodecError::Dimensions("unable to allocate AV1 display plane".to_owned())
        })?;
        for row in 0..height {
            let start = row
                .checked_mul(self.stride)
                .ok_or_else(|| malformed("display plane row offset overflows"))?;
            let end = start
                .checked_add(width)
                .ok_or_else(|| malformed("display plane row extent overflows"))?;
            samples.extend_from_slice(
                self.samples
                    .get(start..end)
                    .ok_or_else(|| malformed("display plane row exceeds retained samples"))?,
            );
        }
        Ok(ReconstructedPlane { samples })
    }
}

/// Restored, upscaled, ungrained AV1 picture retained by reference slots.
/// Render dimensions are presentation metadata and never redefine storage.
pub(super) struct FrameSurface {
    pub(super) depth: SampleDepth,
    pub(super) layout: PixelLayout,
    pub(super) coded_width: u32,
    pub(super) upscaled_width: u32,
    /// Authoritative syntax marker selecting the full-resolution versus
    /// super-resolution film-grain admission policy. This must not be inferred
    /// from widths because the AV1 minimum coded-width clamp permits an
    /// enabled super-resolution frame to have equal coded and upscaled widths.
    pub(super) superres_enabled: bool,
    pub(super) frame_height: u32,
    pub(super) render_width: u32,
    pub(super) render_height: u32,
    pub(super) planes: [Option<FramePlane>; 3],
    pub(super) motion: TemporalMotionField,
    #[cfg(coverage)]
    pub(super) entropy_operations: Vec<crate::Av1EntropyOperationState>,
}

impl FrameSurface {
    pub(super) const fn plane_dimensions(
        layout: PixelLayout,
        width: u32,
        height: u32,
        plane: usize,
    ) -> Option<(u32, u32)> {
        if plane == 0 {
            return Some((width, height));
        }
        match layout {
            PixelLayout::Monochrome => None,
            PixelLayout::I420 => Some((width.div_ceil(2), height.div_ceil(2))),
            PixelLayout::I422 => Some((width.div_ceil(2), height)),
            PixelLayout::I444 => Some((width, height)),
        }
    }

    pub(super) fn plane(&self, plane: usize) -> Av1Result<Option<PlaneView<'_>>> {
        if plane >= self.planes.len() {
            return Err(malformed("reference plane index exceeds three"));
        }
        self.planes[plane]
            .as_ref()
            .map(|retained| retained.view(self.depth))
            .transpose()
    }

    pub(super) fn validate(&self) -> Av1Result<()> {
        if !matches!(self.depth.bits(), 8 | 10 | 12)
            || self.coded_width == 0
            || self.coded_width > self.upscaled_width
            || self.upscaled_width == 0
            || self.frame_height == 0
            || self.render_width == 0
            || self.render_height == 0
            || (!self.superres_enabled && self.coded_width != self.upscaled_width)
            || self.motion.coded_width() != self.coded_width
            || self.motion.frame_height() != self.frame_height
        {
            return Err(malformed("retained frame surface has invalid metadata"));
        }
        for (plane, retained) in self.planes.iter().enumerate() {
            let expected =
                Self::plane_dimensions(self.layout, self.upscaled_width, self.frame_height, plane);
            match (expected, retained) {
                (None, None) => {}
                (Some((width, height)), Some(retained)) => {
                    let view = retained.view(self.depth)?;
                    let expected_width = usize::try_from(width)
                        .map_err(|_| malformed("retained plane width exceeds usize"))?;
                    let expected_height = usize::try_from(height)
                        .map_err(|_| malformed("retained plane height exceeds usize"))?;
                    if retained.width != width
                        || retained.height != height
                        || view.width() != expected_width
                        || view.height() != expected_height
                    {
                        return Err(malformed("retained frame plane has invalid geometry"));
                    }
                }
                _ => return Err(malformed("retained frame surface has invalid plane layout")),
            }
        }
        Ok(())
    }
}
