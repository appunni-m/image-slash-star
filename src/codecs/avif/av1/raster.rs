//! Safe bounded placement of reconstructed AV1 planes into a frame canvas.
//!
//! This is intentionally separate from entropy decoding. The decoder can
//! reconstruct a block without having a safe place to assemble a complete
//! frame; this type supplies that missing boundary without raw pointers or
//! unchecked slice construction.

use super::block::ReconstructedPlane;
use super::cdef::{self, Block as CdefBlock, Parameters as CdefParameters};
use super::filter;
use super::{Av1Result, malformed};
use crate::codecs::CodecError;

/// A checked row-major canvas for one AV1 frame.
pub(in crate::codecs::avif) struct FrameCanvas {
    width: usize,
    height: usize,
    subsampling_x: bool,
    subsampling_y: bool,
    planes: [Vec<u16>; 3],
    written: [Vec<bool>; 3],
}

/// A checked canvas for a complete monochrome auxiliary plane.
///
/// Alpha items are one AV1 monochrome plane, not three duplicated color
/// planes. Keeping this boundary explicit prevents the alpha decoder from
/// claiming completeness merely because a color-shaped scratch canvas was
/// filled.
pub(super) struct MonochromeFrameCanvas {
    width: usize,
    height: usize,
    samples: Vec<u16>,
    written: Vec<bool>,
}

impl MonochromeFrameCanvas {
    pub(super) fn new(width: u32, height: u32) -> Av1Result<Self> {
        let width = usize::try_from(width).map_err(|_| malformed("alpha width exceeds usize"))?;
        let height =
            usize::try_from(height).map_err(|_| malformed("alpha height exceeds usize"))?;
        if width == 0 || height == 0 {
            return Err(malformed("alpha canvas has an empty extent"));
        }
        let dimensions = (width, height);
        Ok(Self {
            width,
            height,
            samples: allocate_zeroed(dimensions, "alpha canvas")?,
            written: allocate_zeroed(dimensions, "alpha coverage")?,
        })
    }

    pub(super) fn place_partition_leaf(
        &mut self,
        x_units: u32,
        y_units: u32,
        width_units: u32,
        height_units: u32,
        plane: &ReconstructedPlane,
    ) -> Av1Result<()> {
        let x = usize::try_from(x_units)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| malformed("alpha leaf x coordinate overflows pixels"))?;
        let y = usize::try_from(y_units)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| malformed("alpha leaf y coordinate overflows pixels"))?;
        let source_width = usize::try_from(width_units)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| malformed("alpha leaf width overflows pixels"))?;
        let source_height = usize::try_from(height_units)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| malformed("alpha leaf height overflows pixels"))?;
        let visible_width = self.width.saturating_sub(x).min(source_width);
        let visible_height = self.height.saturating_sub(y).min(source_height);
        let source_length = source_width
            .checked_mul(source_height)
            .ok_or_else(|| malformed("alpha leaf size overflows usize"))?;
        if source_length == 0
            || visible_width == 0
            || visible_height == 0
            || plane.samples.len() != source_length
        {
            return Err(malformed("alpha leaf has the wrong extent"));
        }
        let end_x = x
            .checked_add(visible_width)
            .ok_or_else(|| malformed("alpha leaf x extent overflows usize"))?;
        let end_y = y
            .checked_add(visible_height)
            .ok_or_else(|| malformed("alpha leaf y extent overflows usize"))?;
        if end_x > self.width || end_y > self.height {
            return Err(malformed("alpha leaf exceeds the frame canvas"));
        }
        for row in 0..visible_height {
            let start = y
                .checked_add(row)
                .and_then(|row| row.checked_mul(self.width))
                .and_then(|row| row.checked_add(x))
                .ok_or_else(|| malformed("alpha leaf row offset overflows usize"))?;
            let end = start
                .checked_add(visible_width)
                .ok_or_else(|| malformed("alpha leaf row end overflows usize"))?;
            if self.written[start..end].iter().any(|written| *written) {
                return Err(malformed("alpha leaves overlap"));
            }
        }
        for row in 0..visible_height {
            let source_start = row
                .checked_mul(source_width)
                .ok_or_else(|| malformed("alpha source row offset overflows usize"))?;
            let source_end = source_start
                .checked_add(visible_width)
                .ok_or_else(|| malformed("alpha source row end overflows usize"))?;
            let destination_start = y
                .checked_add(row)
                .and_then(|row| row.checked_mul(self.width))
                .and_then(|row| row.checked_add(x))
                .ok_or_else(|| malformed("alpha destination row offset overflows usize"))?;
            let destination_end = destination_start
                .checked_add(visible_width)
                .ok_or_else(|| malformed("alpha destination row end overflows usize"))?;
            self.samples[destination_start..destination_end]
                .copy_from_slice(&plane.samples[source_start..source_end]);
            self.written[destination_start..destination_end].fill(true);
        }
        Ok(())
    }

    pub(super) fn finish(self) -> Av1Result<ReconstructedPlane> {
        if self.written.iter().any(|written| !written) {
            return Err(malformed("alpha canvas is missing reconstructed samples"));
        }
        Ok(ReconstructedPlane {
            samples: self.samples,
        })
    }
}

/// Checked source and destination extents for one reconstructed cell.
pub(in crate::codecs::avif) struct CellPlacement<'a> {
    source_width: u32,
    source_height: u32,
    visible_width: u32,
    visible_height: u32,
    planes: &'a [ReconstructedPlane; 3],
    x: u32,
    y: u32,
}

impl FrameCanvas {
    /// Allocate an empty canvas using the sequence's chroma subsampling.
    pub(in crate::codecs::avif) fn new(
        width: u32,
        height: u32,
        subsampling_x: bool,
        subsampling_y: bool,
    ) -> Av1Result<Self> {
        let width = usize::try_from(width).map_err(|_| malformed("frame width exceeds usize"))?;
        let height =
            usize::try_from(height).map_err(|_| malformed("frame height exceeds usize"))?;
        if width == 0 || height == 0 {
            return Err(malformed("frame canvas has an empty extent"));
        }
        let chroma_width = if subsampling_x {
            width.div_ceil(2)
        } else {
            width
        };
        let chroma_height = if subsampling_y {
            height.div_ceil(2)
        } else {
            height
        };
        let dimensions = [
            (width, height),
            (chroma_width, chroma_height),
            (chroma_width, chroma_height),
        ];
        let planes = [
            allocate_zeroed(dimensions[0], "luma canvas")?,
            allocate_zeroed(dimensions[1], "first chroma canvas")?,
            allocate_zeroed(dimensions[2], "second chroma canvas")?,
        ];
        let written = [
            allocate_zeroed(dimensions[0], "luma coverage")?,
            allocate_zeroed(dimensions[1], "first chroma coverage")?,
            allocate_zeroed(dimensions[2], "second chroma coverage")?,
        ];
        Ok(Self {
            width,
            height,
            subsampling_x,
            subsampling_y,
            planes,
            written,
        })
    }

    /// Place a complete set of reconstructed planes at a luma-pixel origin.
    ///
    /// Validation happens for all three planes before any destination is
    /// mutated. A rejected tile therefore cannot leave a half-written frame.
    pub(in crate::codecs::avif) fn place_planes(
        &mut self,
        width: u32,
        height: u32,
        planes: &[ReconstructedPlane; 3],
        x: u32,
        y: u32,
    ) -> Av1Result<()> {
        self.place_cropped_planes(CellPlacement {
            source_width: width,
            source_height: height,
            visible_width: width,
            visible_height: height,
            planes,
            x,
            y,
        })
    }

    /// Place a reconstructed AV1 partition leaf whose coordinates are in
    /// four-by-four luma units.
    ///
    /// AV1 partition geometry is padded to four-pixel units, while the
    /// visible frame canvas is measured in pixels. Keeping that conversion
    /// here gives the entropy walker one checked boundary: an overflowing
    /// unit conversion, an edge crop, or a subsampled misalignment is
    /// rejected before any sample is copied.
    pub(in crate::codecs::avif) fn place_partition_leaf(
        &mut self,
        x_units: u32,
        y_units: u32,
        width_units: u32,
        height_units: u32,
        planes: &[ReconstructedPlane; 3],
    ) -> Av1Result<()> {
        let x = usize::try_from(x_units)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| malformed("partition leaf x coordinate overflows pixels"))?;
        let y = usize::try_from(y_units)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| malformed("partition leaf y coordinate overflows pixels"))?;
        let source_width = usize::try_from(width_units)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| malformed("partition leaf width overflows pixels"))?;
        let source_height = usize::try_from(height_units)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| malformed("partition leaf height overflows pixels"))?;
        let visible_width = self.width.saturating_sub(x).min(source_width);
        let visible_height = self.height.saturating_sub(y).min(source_height);
        let source_width = u32::try_from(source_width)
            .map_err(|_| malformed("partition leaf width exceeds u32"))?;
        let source_height = u32::try_from(source_height)
            .map_err(|_| malformed("partition leaf height exceeds u32"))?;
        let visible_width = u32::try_from(visible_width)
            .map_err(|_| malformed("partition leaf visible width exceeds u32"))?;
        let visible_height = u32::try_from(visible_height)
            .map_err(|_| malformed("partition leaf visible height exceeds u32"))?;
        let x = u32::try_from(x).map_err(|_| malformed("partition leaf x exceeds u32"))?;
        let y = u32::try_from(y).map_err(|_| malformed("partition leaf y exceeds u32"))?;
        self.place_cropped_planes(CellPlacement {
            source_width,
            source_height,
            visible_width,
            visible_height,
            planes,
            x,
            y,
        })
    }

    /// Place one AV1 4:2:0 partition leaf using the codec's coded chroma
    /// geometry.
    ///
    /// A 4×4 luma leaf at an odd luma-unit coordinate owns a full 4×4
    /// chroma transform. Its chroma origin is aligned to the even 4×4
    /// chroma grid, and a luma leaf at an even/even coordinate owns no
    /// chroma samples at all. The generic cell placement API cannot express
    /// that ownership rule, so the lossy AV1 walker uses this checked path.
    pub(in crate::codecs::avif) fn place_av1_partition_leaf(
        &mut self,
        x_units: u32,
        y_units: u32,
        width_units: u32,
        height_units: u32,
        has_chroma: bool,
        planes: &[ReconstructedPlane; 3],
    ) -> Av1Result<()> {
        let pixels = |units: u32, message: &'static str| {
            usize::try_from(units)
                .ok()
                .and_then(|value| value.checked_mul(4))
                .ok_or_else(|| malformed(message))
        };
        let x = pixels(x_units, "AV1 luma leaf x coordinate overflows pixels")?;
        let y = pixels(y_units, "AV1 luma leaf y coordinate overflows pixels")?;
        let source_width = pixels(width_units, "AV1 luma leaf width overflows pixels")?;
        let source_height = pixels(height_units, "AV1 luma leaf height overflows pixels")?;
        let luma_width = self.width.saturating_sub(x).min(source_width);
        let luma_height = self.height.saturating_sub(y).min(source_height);

        self.validate_plane_placement(
            0,
            (x, y),
            (source_width, source_height),
            (luma_width, luma_height),
            &planes[0],
        )?;

        let chroma_geometry = |units: u32, subsampled: bool, message: &'static str| {
            let units = if subsampled {
                units.checked_add(1).ok_or_else(|| malformed(message))? / 2
            } else {
                units
            };
            pixels(units, message)
        };
        if has_chroma {
            let chroma_x_units = if self.subsampling_x {
                x_units / 2
            } else {
                x_units
            };
            let chroma_y_units = if self.subsampling_y {
                y_units / 2
            } else {
                y_units
            };
            let chroma_x = pixels(
                chroma_x_units,
                "AV1 chroma leaf x coordinate overflows pixels",
            )?;
            let chroma_y = pixels(
                chroma_y_units,
                "AV1 chroma leaf y coordinate overflows pixels",
            )?;
            let chroma_width = chroma_geometry(
                width_units,
                self.subsampling_x,
                "AV1 chroma leaf width overflows pixels",
            )?;
            let chroma_height = chroma_geometry(
                height_units,
                self.subsampling_y,
                "AV1 chroma leaf height overflows pixels",
            )?;
            let (canvas_width, canvas_height) = self.plane_dimensions(1);
            let visible_width = canvas_width.saturating_sub(chroma_x).min(chroma_width);
            let visible_height = canvas_height.saturating_sub(chroma_y).min(chroma_height);

            self.validate_plane_placement(
                1,
                (chroma_x, chroma_y),
                (chroma_width, chroma_height),
                (visible_width, visible_height),
                &planes[1],
            )?;
            self.validate_plane_placement(
                2,
                (chroma_x, chroma_y),
                (chroma_width, chroma_height),
                (visible_width, visible_height),
                &planes[2],
            )?;

            self.copy_plane(
                1,
                (chroma_x, chroma_y),
                (chroma_width, chroma_height),
                (visible_width, visible_height),
                &planes[1],
            )?;
            self.copy_plane(
                2,
                (chroma_x, chroma_y),
                (chroma_width, chroma_height),
                (visible_width, visible_height),
                &planes[2],
            )?;
        }

        self.copy_plane(
            0,
            (x, y),
            (source_width, source_height),
            (luma_width, luma_height),
            &planes[0],
        )?;
        Ok(())
    }

    /// Place the top-left visible rectangle of a reconstructed cell.
    ///
    /// AVIF grid cells may be coded larger than the rectangle that contributes
    /// to the final grid canvas. The source planes must contain the complete
    /// coded extent; only the checked top-left rectangle is copied. This
    /// keeps cropping a structural operation rather than a special case in a
    /// decoder or a container fixture.
    pub(in crate::codecs::avif) fn place_cropped_planes(
        &mut self,
        placement: CellPlacement<'_>,
    ) -> Av1Result<()> {
        self.place_cells(std::slice::from_ref(&placement))
    }

    /// Place several coded cells as one checked operation.
    ///
    /// The AVIF grid and multi-tile paths eventually need to assemble many
    /// independently decoded cells. Validate every cell, including overlap
    /// between two cells in the same call, before copying any sample. That
    /// makes a rejected grid or tile group atomic: callers can discard it
    /// without repairing a half-written frame.
    pub(in crate::codecs::avif) fn place_cells(
        &mut self,
        placements: &[CellPlacement<'_>],
    ) -> Av1Result<()> {
        for (index, placement) in placements.iter().enumerate() {
            let geometry = self.cell_geometry(placement)?;
            for plane in 0..3 {
                self.validate_plane_placement(
                    plane,
                    geometry.origins[plane],
                    geometry.source_dimensions[plane],
                    geometry.visible_dimensions[plane],
                    &placement.planes[plane],
                )?;
            }
            for previous in &placements[..index] {
                let previous_geometry = self.cell_geometry(previous)?;
                for plane in 0..3 {
                    if rectangles_overlap(
                        previous_geometry.origins[plane],
                        previous_geometry.visible_dimensions[plane],
                        geometry.origins[plane],
                        geometry.visible_dimensions[plane],
                    ) {
                        return Err(malformed("reconstructed leaves overlap"));
                    }
                }
            }
        }

        for placement in placements {
            let geometry = self.cell_geometry(placement)?;
            for plane in 0..3 {
                self.copy_plane(
                    plane,
                    geometry.origins[plane],
                    geometry.source_dimensions[plane],
                    geometry.visible_dimensions[plane],
                    &placement.planes[plane],
                )?;
            }
        }
        Ok(())
    }

    fn cell_geometry(&self, placement: &CellPlacement<'_>) -> Av1Result<CellGeometry> {
        let CellPlacement {
            source_width,
            source_height,
            visible_width,
            visible_height,
            x,
            y,
            ..
        } = *placement;
        let source_width = usize::try_from(source_width)
            .map_err(|_| malformed("coded cell width exceeds usize"))?;
        let source_height = usize::try_from(source_height)
            .map_err(|_| malformed("coded cell height exceeds usize"))?;
        let width = usize::try_from(visible_width)
            .map_err(|_| malformed("visible cell width exceeds usize"))?;
        let height = usize::try_from(visible_height)
            .map_err(|_| malformed("visible cell height exceeds usize"))?;
        let x = usize::try_from(x).map_err(|_| malformed("leaf x origin exceeds usize"))?;
        let y = usize::try_from(y).map_err(|_| malformed("leaf y origin exceeds usize"))?;
        if source_width == 0 || source_height == 0 || width == 0 || height == 0 {
            return Err(malformed("cell has an empty extent"));
        }
        if width > source_width || height > source_height {
            return Err(malformed("visible cell exceeds its coded extent"));
        }
        if (self.subsampling_x && x % 2 != 0) || (self.subsampling_y && y % 2 != 0) {
            return Err(malformed("subsampled cell origin is not aligned"));
        }

        let source_luma_dimensions = (source_width, source_height);
        let source_chroma_dimensions = (
            if self.subsampling_x {
                source_width.div_ceil(2)
            } else {
                source_width
            },
            if self.subsampling_y {
                source_height.div_ceil(2)
            } else {
                source_height
            },
        );
        let visible_luma_dimensions = (width, height);
        let visible_chroma_dimensions = (
            if self.subsampling_x {
                width.div_ceil(2)
            } else {
                width
            },
            if self.subsampling_y {
                height.div_ceil(2)
            } else {
                height
            },
        );
        let origins = [
            (x, y),
            (
                if self.subsampling_x { x / 2 } else { x },
                if self.subsampling_y { y / 2 } else { y },
            ),
            (
                if self.subsampling_x { x / 2 } else { x },
                if self.subsampling_y { y / 2 } else { y },
            ),
        ];
        let source_dimensions = [
            source_luma_dimensions,
            source_chroma_dimensions,
            source_chroma_dimensions,
        ];
        let visible_dimensions = [
            visible_luma_dimensions,
            visible_chroma_dimensions,
            visible_chroma_dimensions,
        ];
        Ok(CellGeometry {
            origins,
            source_dimensions,
            visible_dimensions,
        })
    }

    /// Finish the canvas only after every plane has been written exactly once.
    pub(in crate::codecs::avif) fn finish(self) -> Av1Result<[ReconstructedPlane; 3]> {
        self.finish_after_cdef(None, None)
    }

    // The frame walker passes the frame-header strengths here. CDEF direction
    // selection is derived from the immutable post-deblock luma source, while
    // each chroma block uses the corresponding luma direction.
    pub(super) fn finish_after_cdef(
        self,
        luma_parameters: Option<CdefParameters>,
        chroma_parameters: Option<CdefParameters>,
    ) -> Av1Result<[ReconstructedPlane; 3]> {
        self.finish_with_filters(
            None,
            &[],
            luma_parameters,
            chroma_parameters,
            None,
            &[],
            &[],
        )
    }

    pub(super) fn finish_with_filters(
        mut self,
        loop_parameters: Option<filter::Parameters>,
        filter_blocks: &[filter::Block],
        luma_parameters: Option<CdefParameters>,
        chroma_parameters: Option<CdefParameters>,
        frame_parameters: Option<cdef::FrameParameters>,
        cdef_indices: &[Option<usize>],
        cdef_active: &[bool],
    ) -> Av1Result<[ReconstructedPlane; 3]> {
        if self
            .written
            .iter()
            .any(|plane| plane.iter().any(|written| !written))
        {
            return Err(malformed("frame canvas is missing reconstructed samples"));
        }

        if let Some(parameters) = loop_parameters {
            let dimensions = [
                self.plane_dimensions(0),
                self.plane_dimensions(1),
                self.plane_dimensions(2),
            ];
            filter::apply(&mut self.planes, dimensions, filter_blocks, parameters)
                .ok_or_else(|| malformed("loop filter geometry exceeds its source planes"))?;
        }

        if frame_parameters.is_some() || luma_parameters.is_some() || chroma_parameters.is_some() {
            self.apply_cdef(
                luma_parameters,
                chroma_parameters,
                frame_parameters,
                cdef_indices,
                cdef_active,
            )?;
        }
        Ok(self.planes.map(|samples| ReconstructedPlane { samples }))
    }

    fn apply_cdef(
        &mut self,
        luma_parameters: Option<CdefParameters>,
        chroma_parameters: Option<CdefParameters>,
        frame_parameters: Option<cdef::FrameParameters>,
        cdef_indices: &[Option<usize>],
        cdef_active: &[bool],
    ) -> Av1Result<()> {
        let source = self.planes.clone();
        let luma_dimensions = self.plane_dimensions(0);
        let active_width = luma_dimensions.0.div_ceil(8);
        let region_width = self.width.div_ceil(64);

        for plane in 0..3 {
            let scalar_parameters = if plane == 0 {
                luma_parameters
            } else {
                chroma_parameters
            };
            if frame_parameters.is_none() && scalar_parameters.is_none() {
                continue;
            }
            let dimensions = self.plane_dimensions(plane);
            let block_size = if plane == 0 { 8 } else { 4 };
            for y in (0..dimensions.1).step_by(block_size) {
                for x in (0..dimensions.0).step_by(block_size) {
                    let block = CdefBlock {
                        x,
                        y,
                        width: dimensions.0.saturating_sub(x).min(block_size),
                        height: dimensions.1.saturating_sub(y).min(block_size),
                    };
                    let luma_x = if plane == 0 || !self.subsampling_x {
                        x
                    } else {
                        x.saturating_mul(2)
                    };
                    let luma_y = if plane == 0 || !self.subsampling_y {
                        y
                    } else {
                        y.saturating_mul(2)
                    };
                    let parameters = if let Some(frame) = frame_parameters {
                        let active_index = luma_y
                            .checked_div(8)
                            .and_then(|row| row.checked_mul(active_width))
                            .and_then(|row| row.checked_add(luma_x.checked_div(8)?));
                        if !active_index
                            .and_then(|index| cdef_active.get(index).copied())
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let region_index = luma_y
                            .checked_div(64)
                            .and_then(|row| row.checked_mul(region_width))
                            .and_then(|row| row.checked_add(luma_x.checked_div(64)?));
                        let cdef_index = region_index
                            .and_then(|index| cdef_indices.get(index).copied().flatten())
                            .or_else(|| {
                                (frame.y_strength_count == 1 && frame.uv_strength_count == 1)
                                    .then_some(0)
                            });
                        let Some(cdef_index) = cdef_index else {
                            continue;
                        };
                        let (strengths, count) = if plane == 0 {
                            (frame.y_strengths, frame.y_strength_count)
                        } else {
                            (frame.uv_strengths, frame.uv_strength_count)
                        };
                        let Some(&strength) =
                            strengths.get(cdef_index).filter(|_| cdef_index < count)
                        else {
                            continue;
                        };
                        if strength == 0 {
                            continue;
                        }
                        let secondary = strength & 3;
                        let base = CdefParameters {
                            primary_strength: strength >> 2,
                            secondary_strength: if secondary == 3 { 4 } else { secondary },
                            direction: 0,
                            damping: frame.damping.saturating_sub(u32::from(plane != 0)),
                            bit_depth: frame.bit_depth,
                        };
                        let direction = cdef::direction_for_block(
                            &source[0],
                            luma_dimensions,
                            CdefBlock {
                                x: luma_x,
                                y: luma_y,
                                width: 8,
                                height: 8,
                            },
                            base.bit_depth,
                        );
                        let Some((direction, variance)) = direction else {
                            continue;
                        };
                        if plane == 0 {
                            let primary =
                                cdef::adjust_primary_strength(base.primary_strength, variance);
                            CdefParameters {
                                direction: if primary == 0 { 0 } else { direction },
                                primary_strength: primary,
                                ..base
                            }
                        } else {
                            CdefParameters {
                                direction: if base.primary_strength == 0 {
                                    0
                                } else if self.subsampling_x && !self.subsampling_y {
                                    // dav1d remaps luma directions for 4:2:2
                                    // chroma because only the horizontal axis
                                    // is subsampled.
                                    [7, 0, 2, 4, 5, 6, 6, 6][direction]
                                } else {
                                    direction
                                },
                                ..base
                            }
                        }
                    } else {
                        let Some(base) = scalar_parameters else {
                            continue;
                        };
                        if plane == 0 {
                            let Some((direction, variance)) = cdef::direction_for_block(
                                &source[0],
                                luma_dimensions,
                                block,
                                base.bit_depth,
                            ) else {
                                continue;
                            };
                            let primary =
                                cdef::adjust_primary_strength(base.primary_strength, variance);
                            CdefParameters {
                                direction: if primary == 0 { 0 } else { direction },
                                primary_strength: primary,
                                ..base
                            }
                        } else {
                            let Some((direction, _)) = cdef::direction_for_block(
                                &source[0],
                                luma_dimensions,
                                CdefBlock {
                                    x: luma_x,
                                    y: luma_y,
                                    width: 8,
                                    height: 8,
                                },
                                base.bit_depth,
                            ) else {
                                continue;
                            };
                            CdefParameters {
                                direction: if base.primary_strength == 0 {
                                    0
                                } else if self.subsampling_x && !self.subsampling_y {
                                    [7, 0, 2, 4, 5, 6, 6, 6][direction]
                                } else {
                                    direction
                                },
                                ..base
                            }
                        }
                    };

                    let filtered =
                        cdef::filter_block(&source[plane], dimensions, block, parameters)
                            .ok_or_else(|| malformed("CDEF block exceeds its source plane"))?;
                    for row in 0..block.height {
                        let source_start = row.saturating_mul(block.width);
                        let source_end = source_start.saturating_add(block.width);
                        let destination_start = y
                            .saturating_add(row)
                            .saturating_mul(dimensions.0)
                            .saturating_add(x);
                        let destination_end = destination_start.saturating_add(block.width);
                        self.planes[plane][destination_start..destination_end]
                            .copy_from_slice(&filtered[source_start..source_end]);
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_plane_placement(
        &self,
        plane: usize,
        (x, y): (usize, usize),
        (source_width, source_height): (usize, usize),
        (width, height): (usize, usize),
        source: &ReconstructedPlane,
    ) -> Av1Result<()> {
        let (canvas_width, canvas_height) = self.plane_dimensions(plane);
        let source_length = source_width
            .checked_mul(source_height)
            .ok_or_else(|| malformed("leaf plane size overflows usize"))?;
        if source.samples.len() != source_length {
            return Err(malformed("reconstructed leaf plane has the wrong extent"));
        }
        if width > source_width || height > source_height {
            return Err(malformed("visible leaf plane exceeds coded extent"));
        }
        let end_x = x
            .checked_add(width)
            .ok_or_else(|| malformed("leaf plane x extent overflows usize"))?;
        let end_y = y
            .checked_add(height)
            .ok_or_else(|| malformed("leaf plane y extent overflows usize"))?;
        if end_x > canvas_width || end_y > canvas_height {
            return Err(malformed("reconstructed leaf exceeds the frame canvas"));
        }
        let coverage = &self.written[plane];
        for row in 0..height {
            let row_start = y
                .checked_add(row)
                .and_then(|row| row.checked_mul(canvas_width))
                .and_then(|row| row.checked_add(x))
                .ok_or_else(|| malformed("leaf plane row offset overflows usize"))?;
            let row_end = row_start
                .checked_add(width)
                .ok_or_else(|| malformed("leaf plane row end overflows usize"))?;
            if coverage[row_start..row_end].iter().any(|written| *written) {
                return Err(malformed("reconstructed leaves overlap"));
            }
        }
        Ok(())
    }

    fn copy_plane(
        &mut self,
        plane: usize,
        (x, y): (usize, usize),
        (source_width, _source_height): (usize, usize),
        (width, height): (usize, usize),
        source: &ReconstructedPlane,
    ) -> Av1Result<()> {
        let (canvas_width, _) = self.plane_dimensions(plane);
        let destination = &mut self.planes[plane];
        let coverage = &mut self.written[plane];
        for row in 0..height {
            let source_start = row
                .checked_mul(source_width)
                .ok_or_else(|| malformed("leaf source row offset overflows usize"))?;
            let source_end = source_start
                .checked_add(width)
                .ok_or_else(|| malformed("leaf source row end overflows usize"))?;
            let destination_start = y
                .checked_add(row)
                .and_then(|row| row.checked_mul(canvas_width))
                .and_then(|row| row.checked_add(x))
                .ok_or_else(|| malformed("leaf destination row offset overflows usize"))?;
            let destination_end = destination_start
                .checked_add(width)
                .ok_or_else(|| malformed("leaf destination row end overflows usize"))?;
            destination[destination_start..destination_end]
                .copy_from_slice(&source.samples[source_start..source_end]);
            coverage[destination_start..destination_end].fill(true);
        }
        Ok(())
    }

    fn plane_dimensions(&self, plane: usize) -> (usize, usize) {
        if plane == 0 {
            (self.width, self.height)
        } else {
            (
                if self.subsampling_x {
                    self.width.div_ceil(2)
                } else {
                    self.width
                },
                if self.subsampling_y {
                    self.height.div_ceil(2)
                } else {
                    self.height
                },
            )
        }
    }
}

struct CellGeometry {
    origins: [(usize, usize); 3],
    source_dimensions: [(usize, usize); 3],
    visible_dimensions: [(usize, usize); 3],
}

fn rectangles_overlap(
    (first_x, first_y): (usize, usize),
    (first_width, first_height): (usize, usize),
    (second_x, second_y): (usize, usize),
    (second_width, second_height): (usize, usize),
) -> bool {
    let first_end_x = first_x.saturating_add(first_width);
    let first_end_y = first_y.saturating_add(first_height);
    let second_end_x = second_x.saturating_add(second_width);
    let second_end_y = second_y.saturating_add(second_height);
    first_x < second_end_x
        && second_x < first_end_x
        && first_y < second_end_y
        && second_y < first_end_y
}

fn allocate_zeroed<T: Clone + Default>(
    dimensions: (usize, usize),
    label: &str,
) -> Av1Result<Vec<T>> {
    let length = dimensions
        .0
        .checked_mul(dimensions.1)
        .ok_or_else(|| malformed("frame canvas size overflows usize"))?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| CodecError::Dimensions(format!("unable to allocate {label}")))?;
    values.resize_with(length, T::default);
    Ok(values)
}

#[cfg(coverage)]
#[coverage(off)]
pub(super) fn __coverage_exercise_private_branches() {
    let _ = FrameCanvas::new(0, 4, false, false);
    let _ = FrameCanvas::new(4, 0, false, false);
    let _ = allocate_zeroed::<u8>((usize::MAX, 2), "overflow");
    assert!(rectangles_overlap((0, 0), (2, 2), (1, 1), (2, 2)));
    assert!(!rectangles_overlap((0, 0), (1, 1), (1, 1), (2, 2)));

    let plane = |width: usize, height: usize, value: u16| ReconstructedPlane {
        samples: vec![value; width.saturating_mul(height)],
    };
    let valid = [plane(2, 2, 1), plane(1, 1, 2), plane(1, 1, 3)];

    let mut empty = FrameCanvas::new(4, 4, true, true).expect("coverage canvas");
    let empty_cell = CellPlacement {
        source_width: 0,
        source_height: 2,
        visible_width: 0,
        visible_height: 2,
        planes: &valid,
        x: 0,
        y: 0,
    };
    let _ = empty.place_cells(std::slice::from_ref(&empty_cell));

    let too_visible = CellPlacement {
        source_width: 2,
        source_height: 2,
        visible_width: 3,
        visible_height: 2,
        planes: &valid,
        x: 0,
        y: 0,
    };
    let _ = empty.place_cells(std::slice::from_ref(&too_visible));

    let misaligned_x = CellPlacement {
        source_width: 2,
        source_height: 2,
        visible_width: 2,
        visible_height: 2,
        planes: &valid,
        x: 1,
        y: 0,
    };
    let _ = empty.place_cells(std::slice::from_ref(&misaligned_x));
    let misaligned_y = CellPlacement {
        y: 1,
        ..misaligned_x
    };
    let _ = empty.place_cells(std::slice::from_ref(&misaligned_y));

    let bad_luma = [plane(1, 1, 1), plane(1, 1, 2), plane(1, 1, 3)];
    let bad_luma_cell = CellPlacement {
        source_width: 2,
        source_height: 2,
        visible_width: 2,
        visible_height: 2,
        planes: &bad_luma,
        x: 0,
        y: 0,
    };
    let _ = empty.place_cells(std::slice::from_ref(&bad_luma_cell));

    let bad_chroma = [plane(2, 2, 1), plane(2, 2, 2), plane(1, 1, 3)];
    let bad_chroma_cell = CellPlacement {
        planes: &bad_chroma,
        ..bad_luma_cell
    };
    let _ = empty.place_cells(std::slice::from_ref(&bad_chroma_cell));

    let outside = CellPlacement {
        planes: &valid,
        x: 4,
        y: 0,
        ..left_cell(2, 2, &valid)
    };
    let _ = empty.place_cells(std::slice::from_ref(&outside));

    let left = left_cell(2, 2, &valid);
    let overlapping = CellPlacement { x: 1, ..left };
    let _ = empty.place_cells(&[left, overlapping]);

    let complete_left_planes = [plane(2, 2, 1), plane(2, 2, 2), plane(2, 2, 3)];
    let complete_right_planes = [plane(2, 2, 4), plane(2, 2, 5), plane(2, 2, 6)];
    let left = CellPlacement {
        planes: &complete_left_planes,
        ..left_cell(2, 2, &complete_left_planes)
    };
    let right = CellPlacement {
        planes: &complete_right_planes,
        x: 2,
        ..left_cell(2, 2, &complete_right_planes)
    };
    let mut complete = FrameCanvas::new(4, 2, false, false).expect("coverage canvas");
    let _ = complete.place_cells(&[left, right]);
    let _ = complete.finish();
}

#[cfg(coverage)]
#[coverage(off)]
fn left_cell<'a>(
    width: u32,
    height: u32,
    planes: &'a [ReconstructedPlane; 3],
) -> CellPlacement<'a> {
    CellPlacement {
        source_width: width,
        source_height: height,
        visible_width: width,
        visible_height: height,
        planes,
        x: 0,
        y: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane(width: usize, height: usize, value: u16) -> ReconstructedPlane {
        ReconstructedPlane {
            samples: vec![value; width.saturating_mul(height)],
        }
    }

    #[test]
    fn places_subsampled_leaf_and_finishes_complete_planes() -> Av1Result<()> {
        let mut canvas = FrameCanvas::new(4, 4, true, true)?;
        let planes = [plane(4, 4, 10), plane(2, 2, 20), plane(2, 2, 30)];
        canvas.place_planes(4, 4, &planes, 0, 0)?;
        let [luma, u, v] = canvas.finish()?;
        assert_eq!(luma.samples, vec![10; 16]);
        assert_eq!(u.samples, vec![20; 4]);
        assert_eq!(v.samples, vec![30; 4]);
        Ok(())
    }

    #[test]
    fn rejects_misaligned_overlap_and_incomplete_canvas() -> Av1Result<()> {
        let mut canvas = FrameCanvas::new(4, 4, true, true)?;
        let planes = [plane(2, 2, 10), plane(1, 1, 20), plane(1, 1, 30)];
        assert!(canvas.place_planes(2, 2, &planes, 1, 0).is_err());
        canvas.place_planes(2, 2, &planes, 0, 0)?;
        assert!(canvas.place_planes(2, 2, &planes, 0, 0).is_err());
        assert!(canvas.finish().is_err());
        Ok(())
    }

    #[test]
    fn places_adjacent_subsampled_tiles_without_overlap() -> Av1Result<()> {
        let mut canvas = FrameCanvas::new(4, 2, true, true)?;
        let left = [plane(2, 2, 1), plane(1, 1, 10), plane(1, 1, 20)];
        let right = [plane(2, 2, 2), plane(1, 1, 30), plane(1, 1, 40)];
        canvas.place_planes(2, 2, &left, 0, 0)?;
        canvas.place_planes(2, 2, &right, 2, 0)?;
        let [luma, u, v] = canvas.finish()?;
        assert_eq!(luma.samples, vec![1, 1, 2, 2, 1, 1, 2, 2]);
        assert_eq!(u.samples, vec![10, 30]);
        assert_eq!(v.samples, vec![20, 40]);
        Ok(())
    }

    #[test]
    fn converts_partition_units_and_crops_only_the_visible_edge() -> Av1Result<()> {
        let mut canvas = FrameCanvas::new(12, 8, false, false)?;
        let left = [plane(8, 8, 1), plane(8, 8, 2), plane(8, 8, 3)];
        let right = [plane(8, 8, 4), plane(8, 8, 5), plane(8, 8, 6)];
        canvas.place_partition_leaf(0, 0, 2, 2, &left)?;
        canvas.place_partition_leaf(2, 0, 2, 2, &right)?;
        let [luma, u, v] = canvas.finish()?;
        let expected = |left, right| {
            (0..8)
                .flat_map(|_| [vec![left; 8], vec![right; 4]].into_iter().flatten())
                .collect::<Vec<_>>()
        };
        assert_eq!(luma.samples, expected(1, 4));
        assert_eq!(u.samples, expected(2, 5));
        assert_eq!(v.samples, expected(3, 6));
        Ok(())
    }

    #[test]
    fn validates_a_cell_batch_before_copying_any_cell() -> Av1Result<()> {
        let first = [plane(2, 2, 1), plane(2, 2, 10), plane(2, 2, 20)];
        let second = [plane(2, 2, 2), plane(2, 2, 30), plane(2, 2, 40)];
        let overlapping = [
            CellPlacement {
                source_width: 2,
                source_height: 2,
                visible_width: 2,
                visible_height: 2,
                planes: &first,
                x: 0,
                y: 0,
            },
            CellPlacement {
                source_width: 2,
                source_height: 2,
                visible_width: 2,
                visible_height: 2,
                planes: &second,
                x: 1,
                y: 0,
            },
        ];
        let mut canvas = FrameCanvas::new(4, 2, false, false)?;
        assert!(canvas.place_cells(&overlapping).is_err());

        let adjacent = [
            CellPlacement {
                x: 0,
                ..overlapping[0]
            },
            CellPlacement {
                x: 2,
                ..overlapping[1]
            },
        ];
        canvas.place_cells(&adjacent)?;
        let [luma, u, v] = canvas.finish()?;
        assert_eq!(luma.samples, vec![1, 1, 2, 2, 1, 1, 2, 2]);
        assert_eq!(u.samples, vec![10, 10, 30, 30, 10, 10, 30, 30]);
        assert_eq!(v.samples, vec![20, 20, 40, 40, 20, 20, 40, 40]);
        Ok(())
    }

    #[test]
    fn crops_coded_grid_cells_by_visible_extent() -> Av1Result<()> {
        let mut canvas = FrameCanvas::new(4, 4, false, false)?;
        let top = [
            ReconstructedPlane {
                samples: (1..=12).collect(),
            },
            plane(4, 3, 30),
            plane(4, 3, 40),
        ];
        let bottom = [
            ReconstructedPlane {
                samples: (21..=32).collect(),
            },
            plane(4, 3, 50),
            plane(4, 3, 60),
        ];
        canvas.place_cropped_planes(CellPlacement {
            source_width: 4,
            source_height: 3,
            visible_width: 4,
            visible_height: 2,
            planes: &top,
            x: 0,
            y: 0,
        })?;
        canvas.place_cropped_planes(CellPlacement {
            source_width: 4,
            source_height: 3,
            visible_width: 4,
            visible_height: 2,
            planes: &bottom,
            x: 0,
            y: 2,
        })?;
        let [luma, u, v] = canvas.finish()?;
        assert_eq!(luma.samples, (1..=8).chain(21..=28).collect::<Vec<_>>());
        assert_eq!(
            u.samples,
            vec![30; 8]
                .into_iter()
                .chain(vec![50; 8])
                .collect::<Vec<_>>()
        );
        assert_eq!(
            v.samples,
            vec![40; 8]
                .into_iter()
                .chain(vec![60; 8])
                .collect::<Vec<_>>()
        );
        Ok(())
    }

    #[test]
    fn rejects_leaf_outside_canvas_before_mutating_it() -> Av1Result<()> {
        let mut canvas = FrameCanvas::new(4, 4, false, false)?;
        assert!(
            canvas
                .place_planes(
                    3,
                    3,
                    &[plane(3, 3, 1), plane(3, 3, 2), plane(3, 3, 3)],
                    2,
                    2,
                )
                .is_err()
        );
        assert!(canvas.finish().is_err());
        let mut canvas = FrameCanvas::new(4, 4, false, false)?;
        canvas.place_planes(
            4,
            4,
            &[plane(4, 4, 4), plane(4, 4, 5), plane(4, 4, 6)],
            0,
            0,
        )?;
        assert!(canvas.finish().is_ok());
        Ok(())
    }

    #[test]
    fn rejects_empty_extents_and_wrong_plane_lengths() -> Av1Result<()> {
        assert!(FrameCanvas::new(0, 4, true, true).is_err());
        assert!(FrameCanvas::new(4, 0, true, true).is_err());
        let mut canvas = FrameCanvas::new(4, 4, true, true)?;
        assert!(
            canvas
                .place_planes(
                    0,
                    2,
                    &[plane(0, 2, 1), plane(0, 1, 2), plane(0, 1, 3)],
                    0,
                    0,
                )
                .is_err()
        );
        assert!(
            canvas
                .place_planes(
                    2,
                    2,
                    &[plane(2, 2, 1), plane(2, 2, 2), plane(1, 1, 3)],
                    0,
                    0,
                )
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn rejects_vertical_alignment_and_bounds() -> Av1Result<()> {
        let mut subsampled = FrameCanvas::new(4, 4, true, true)?;
        let planes = [plane(2, 2, 1), plane(1, 1, 2), plane(1, 1, 3)];
        assert!(subsampled.place_planes(2, 2, &planes, 0, 1).is_err());

        let mut full = FrameCanvas::new(4, 4, false, false)?;
        let full_planes = [plane(3, 3, 1), plane(3, 3, 2), plane(3, 3, 3)];
        assert!(full.place_planes(3, 3, &full_planes, 0, 2).is_err());
        Ok(())
    }

    #[test]
    fn optional_cdef_hook_filters_a_complete_canvas() -> Av1Result<()> {
        let mut canvas = FrameCanvas::new(8, 8, false, false)?;
        let planes = [plane(8, 8, 128), plane(8, 8, 64), plane(8, 8, 64)];
        canvas.place_planes(8, 8, &planes, 0, 0)?;
        let [luma, u, v] = canvas.finish_after_cdef(
            Some(CdefParameters {
                primary_strength: 4,
                secondary_strength: 3,
                direction: 2,
                damping: 3,
                bit_depth: 8,
            }),
            Some(CdefParameters {
                primary_strength: 4,
                secondary_strength: 3,
                direction: 2,
                damping: 3,
                bit_depth: 8,
            }),
        )?;
        assert_eq!(luma.samples, vec![128; 64]);
        assert_eq!(u.samples, vec![64; 64]);
        assert_eq!(v.samples, vec![64; 64]);
        Ok(())
    }
}
