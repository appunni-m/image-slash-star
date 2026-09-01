//! Safe scalar AV1 loop filtering.
//!
//! The implementation follows the scalar dav1d 1.5.3 reference kernels. It
//! deliberately represents filter edges as bounded masks instead of exposing
//! padded pointers or aliasing slices. SIMD can be added behind this same
//! checked boundary later without changing the decoder's safety model.

#![expect(
    clippy::arithmetic_side_effects,
    reason = "AV1 loop-filter formulas operate on validated 16-bit samples and bounded filter widths"
)]

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Block {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) has_chroma: bool,
    pub(crate) luma_tx_width: usize,
    pub(crate) luma_tx_height: usize,
    /// Optional per-4x4 luma transform dimensions for mixed variable-
    /// transform trees. Entries are `(width_log2, height_log2)` in row-major
    /// block order; when absent the scalar dimensions above apply.
    pub(crate) luma_tx_cells: Option<Vec<(u8, u8)>>,
    pub(crate) chroma_tx_width: usize,
    pub(crate) chroma_tx_height: usize,
    /// Skipped inter prediction units have no internal transform edges.
    /// Their outer prediction-unit boundaries remain filterable.
    pub(crate) skip_internal_edges: bool,
    /// Effective loop-filter levels for Y-vertical, Y-horizontal, U, and V.
    pub(crate) levels: [u8; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Parameters {
    pub(crate) sharpness: u32,
    pub(crate) bit_depth: u32,
}

const NO_EDGE: u8 = u8::MAX;

/// Apply AV1's vertical and horizontal deblocking passes to complete planes.
///
/// The block list contains the decoded intra-block geometry and final
/// transform sizes. It is used only to construct the same edge masks as the
/// scalar AV1 loop-filter path; all pixel reads and writes remain bounds
/// checked.
pub(crate) fn apply(
    planes: &mut [Vec<u16>; 3],
    dimensions: [(usize, usize); 3],
    blocks: &[Block],
    parameters: Parameters,
    subsampling_x: bool,
    subsampling_y: bool,
) -> Option<()> {
    if !(8..=16).contains(&parameters.bit_depth)
        || dimensions
            .iter()
            .zip(planes.iter())
            .any(|(&(width, height), samples)| {
                samples.len() != width.checked_mul(height).unwrap_or(0)
            })
    {
        return None;
    }

    let luma_mask = build_masks(dimensions[0], blocks, false, false, false)?;
    let chroma_mask = build_masks(dimensions[1], blocks, true, subsampling_x, subsampling_y)?;
    let threshold_lut: [(i32, i32, i32); 64] = std::array::from_fn(|level| {
        thresholds(
            u32::try_from(level).unwrap_or_default(),
            parameters.sharpness,
        )
    });
    apply_vertical(
        &mut planes[0],
        dimensions[0],
        &luma_mask,
        &threshold_lut,
        0,
        false,
        parameters.bit_depth,
    )?;
    apply_horizontal(
        &mut planes[0],
        dimensions[0],
        &luma_mask,
        &threshold_lut,
        1,
        false,
        parameters.bit_depth,
    )?;
    apply_vertical(
        &mut planes[1],
        dimensions[1],
        &chroma_mask,
        &threshold_lut,
        2,
        true,
        parameters.bit_depth,
    )?;
    apply_horizontal(
        &mut planes[1],
        dimensions[1],
        &chroma_mask,
        &threshold_lut,
        2,
        true,
        parameters.bit_depth,
    )?;
    apply_vertical(
        &mut planes[2],
        dimensions[1],
        &chroma_mask,
        &threshold_lut,
        3,
        true,
        parameters.bit_depth,
    )?;
    apply_horizontal(
        &mut planes[2],
        dimensions[1],
        &chroma_mask,
        &threshold_lut,
        3,
        true,
        parameters.bit_depth,
    )?;
    Some(())
}

struct Masks {
    vertical: Vec<u8>,
    horizontal: Vec<u8>,
    width_units: usize,
    height_units: usize,
    levels: Vec<[u8; 4]>,
}

fn fallible_filled_vec<T: Clone>(length: usize, value: T) -> Option<Vec<T>> {
    let mut values = Vec::new();
    values.try_reserve_exact(length).ok()?;
    values.resize(length, value);
    Some(values)
}

fn build_masks(
    dimensions: (usize, usize),
    blocks: &[Block],
    chroma: bool,
    subsampling_x: bool,
    subsampling_y: bool,
) -> Option<Masks> {
    let (width, height) = dimensions;
    let width_units = width.div_ceil(4);
    let height_units = height.div_ceil(4);
    let mask_width = width_units.checked_add(1)?;
    let mask_height = height_units.checked_add(1)?;
    let vertical_len = mask_width.checked_mul(height_units)?;
    let horizontal_len = mask_height.checked_mul(width_units)?;
    let levels_len = width_units.checked_mul(height_units)?;
    let mut masks = Masks {
        vertical: fallible_filled_vec(vertical_len, NO_EDGE)?,
        horizontal: fallible_filled_vec(horizontal_len, NO_EDGE)?,
        width_units,
        height_units,
        levels: fallible_filled_vec(levels_len, [0; 4])?,
    };

    for block in blocks {
        if chroma && !block.has_chroma {
            continue;
        }
        let (x, y, block_width, block_height, tx_width, tx_height) = if chroma {
            (
                if subsampling_x { block.x / 2 } else { block.x },
                if subsampling_y { block.y / 2 } else { block.y },
                if subsampling_x {
                    block.width.div_ceil(2)
                } else {
                    block.width
                },
                if subsampling_y {
                    block.height.div_ceil(2)
                } else {
                    block.height
                },
                block.chroma_tx_width,
                block.chroma_tx_height,
            )
        } else {
            (
                block.x,
                block.y,
                block.width,
                block.height,
                block.luma_tx_width,
                block.luma_tx_height,
            )
        };
        let x_units = x / 4;
        let y_units = y / 4;
        let block_width_units = block_width.div_ceil(4);
        let block_height_units = block_height.div_ceil(4);
        if x_units >= masks.width_units
            || y_units >= masks.height_units
            || block_width_units == 0
            || block_height_units == 0
        {
            continue;
        }
        let end_x = x_units
            .saturating_add(block_width_units)
            .min(masks.width_units);
        let end_y = y_units
            .saturating_add(block_height_units)
            .min(masks.height_units);
        let horizontal_tx = transform_units(tx_width);
        let vertical_tx = transform_units(tx_height);
        let variable_luma = !chroma && block.luma_tx_cells.is_some();
        for row in y_units..end_y {
            for column in x_units..end_x {
                let index = row.checked_mul(masks.width_units)?.checked_add(column)?;
                *masks.levels.get_mut(index)? = block.levels;
            }
        }
        if variable_luma {
            let cells = block.luma_tx_cells.as_deref()?;
            let expected = block_width_units.checked_mul(block_height_units)?;
            if cells.len() != expected {
                return None;
            }
            let mut visited = fallible_filled_vec(expected, false)?;
            for local_row in 0..block_height_units {
                for local_column in 0..block_width_units {
                    let cell_index = local_row
                        .checked_mul(block_width_units)?
                        .checked_add(local_column)?;
                    if *visited.get(cell_index)? {
                        continue;
                    }
                    let &(width_log2, height_log2) = cells.get(cell_index)?;
                    if !(2..=4).contains(&width_log2) || !(2..=4).contains(&height_log2) {
                        return None;
                    }
                    let tx_width_units = 1usize.checked_shl(u32::from(width_log2))?;
                    let tx_height_units = 1usize.checked_shl(u32::from(height_log2))?;
                    let end_column = local_column.checked_add(tx_width_units)?;
                    let end_row = local_row.checked_add(tx_height_units)?;
                    if end_column > block_width_units || end_row > block_height_units {
                        return None;
                    }
                    for row in local_row..end_row {
                        for column in local_column..end_column {
                            let index = row.checked_mul(block_width_units)?.checked_add(column)?;
                            if *visited.get(index)?
                                || cells.get(index)? != &(width_log2, height_log2)
                            {
                                return None;
                            }
                            *visited.get_mut(index)? = true;
                        }
                    }
                    let absolute_left = x_units.checked_add(local_column)?;
                    let absolute_right = x_units.checked_add(end_column)?;
                    let absolute_top = y_units.checked_add(local_row)?;
                    let absolute_bottom = y_units.checked_add(end_row)?;
                    let vertical_index = edge_index(tx_width_units);
                    let horizontal_index = edge_index(tx_height_units);
                    if absolute_left > 0 && (!block.skip_internal_edges || local_column == 0) {
                        for segment in
                            y_units.checked_add(local_row)?..y_units.checked_add(end_row)?
                        {
                            set_min(
                                &mut masks.vertical,
                                segment
                                    .checked_mul(mask_width)?
                                    .checked_add(absolute_left)?,
                                vertical_index,
                            );
                        }
                    }
                    if absolute_right <= masks.width_units
                        && (!block.skip_internal_edges || end_column == block_width_units)
                    {
                        for segment in
                            y_units.checked_add(local_row)?..y_units.checked_add(end_row)?
                        {
                            set_min(
                                &mut masks.vertical,
                                segment
                                    .checked_mul(mask_width)?
                                    .checked_add(absolute_right)?,
                                vertical_index,
                            );
                        }
                    }
                    if absolute_top > 0 && (!block.skip_internal_edges || local_row == 0) {
                        for segment in
                            x_units.checked_add(local_column)?..x_units.checked_add(end_column)?
                        {
                            set_min(
                                &mut masks.horizontal,
                                absolute_top
                                    .checked_mul(masks.width_units)?
                                    .checked_add(segment)?,
                                horizontal_index,
                            );
                        }
                    }
                    if absolute_bottom <= masks.height_units
                        && (!block.skip_internal_edges || end_row == block_height_units)
                    {
                        for segment in
                            x_units.checked_add(local_column)?..x_units.checked_add(end_column)?
                        {
                            set_min(
                                &mut masks.horizontal,
                                absolute_bottom
                                    .checked_mul(masks.width_units)?
                                    .checked_add(segment)?,
                                horizontal_index,
                            );
                        }
                    }
                }
            }
            if visited.iter().any(|&cell| !cell) {
                return None;
            }
        } else {
            if x_units > 0 {
                for segment in y_units..end_y {
                    set_min(
                        &mut masks.vertical,
                        segment.checked_mul(mask_width)?.checked_add(x_units)?,
                        edge_index(horizontal_tx),
                    );
                }
            }
            if end_x <= masks.width_units {
                for segment in y_units..end_y {
                    set_min(
                        &mut masks.vertical,
                        segment.checked_mul(mask_width)?.checked_add(end_x)?,
                        edge_index(horizontal_tx),
                    );
                }
            }
            if y_units > 0 {
                for segment in x_units..end_x {
                    set_min(
                        &mut masks.horizontal,
                        y_units
                            .checked_mul(masks.width_units)?
                            .checked_add(segment)?,
                        edge_index(vertical_tx),
                    );
                }
            }
            if end_y <= masks.height_units {
                for segment in x_units..end_x {
                    set_min(
                        &mut masks.horizontal,
                        end_y.checked_mul(masks.width_units)?.checked_add(segment)?,
                        edge_index(vertical_tx),
                    );
                }
            }
        }

        if !block.skip_internal_edges {
            if variable_luma {
                // Terminal rectangles above already emitted every applicable
                // side. Avoid adding a uniform or one-sided internal path.
            } else if !chroma {
                if let Some(cells) = block.luma_tx_cells.as_deref() {
                    let expected = block_width_units.checked_mul(block_height_units)?;
                    if cells.len() != expected {
                        return None;
                    }
                    for local_row in 0..block_height_units {
                        for local_column in 0..block_width_units {
                            let cell_index = local_row
                                .checked_mul(block_width_units)?
                                .checked_add(local_column)?;
                            let &(width_log2, height_log2) = cells.get(cell_index)?;
                            if !(2..=4).contains(&width_log2) || !(2..=4).contains(&height_log2) {
                                return None;
                            }
                            let tx_width_units = 1usize.checked_shl(u32::from(width_log2))?;
                            let tx_height_units = 1usize.checked_shl(u32::from(height_log2))?;
                            if local_column > 0
                                && local_column.is_multiple_of(tx_width_units)
                                && local_row.is_multiple_of(tx_height_units)
                            {
                                let edge = x_units.checked_add(local_column)?;
                                let end_row = local_row
                                    .checked_add(tx_height_units)?
                                    .min(block_height_units);
                                for segment in
                                    y_units.checked_add(local_row)?..y_units.checked_add(end_row)?
                                {
                                    set_min(
                                        &mut masks.vertical,
                                        segment.checked_mul(mask_width)?.checked_add(edge)?,
                                        edge_index(tx_width_units),
                                    );
                                }
                            }
                            if local_row > 0
                                && local_row.is_multiple_of(tx_height_units)
                                && local_column.is_multiple_of(tx_width_units)
                            {
                                let edge = y_units.checked_add(local_row)?;
                                let end_column = local_column
                                    .checked_add(tx_width_units)?
                                    .min(block_width_units);
                                for segment in x_units.checked_add(local_column)?
                                    ..x_units.checked_add(end_column)?
                                {
                                    set_min(
                                        &mut masks.horizontal,
                                        edge.checked_mul(masks.width_units)?
                                            .checked_add(segment)?,
                                        edge_index(tx_height_units),
                                    );
                                }
                            }
                        }
                    }
                } else {
                    let mut edge = x_units.saturating_add(horizontal_tx);
                    while edge < end_x {
                        for segment in y_units..end_y {
                            set_min(
                                &mut masks.vertical,
                                segment.checked_mul(mask_width)?.checked_add(edge)?,
                                edge_index(horizontal_tx),
                            );
                        }
                        edge = edge.saturating_add(horizontal_tx);
                    }
                    let mut edge = y_units.saturating_add(vertical_tx);
                    while edge < end_y {
                        for segment in x_units..end_x {
                            set_min(
                                &mut masks.horizontal,
                                edge.checked_mul(masks.width_units)?.checked_add(segment)?,
                                edge_index(vertical_tx),
                            );
                        }
                        edge = edge.saturating_add(vertical_tx);
                    }
                }
            } else {
                let mut edge = x_units.saturating_add(horizontal_tx);
                while edge < end_x {
                    for segment in y_units..end_y {
                        set_min(
                            &mut masks.vertical,
                            segment.checked_mul(mask_width)?.checked_add(edge)?,
                            edge_index(horizontal_tx),
                        );
                    }
                    edge = edge.saturating_add(horizontal_tx);
                }
                let mut edge = y_units.saturating_add(vertical_tx);
                while edge < end_y {
                    for segment in x_units..end_x {
                        set_min(
                            &mut masks.horizontal,
                            edge.checked_mul(masks.width_units)?.checked_add(segment)?,
                            edge_index(vertical_tx),
                        );
                    }
                    edge = edge.saturating_add(vertical_tx);
                }
            }
        }
    }
    Some(masks)
}

fn transform_units(size: usize) -> usize {
    size.max(4).div_ceil(4).next_power_of_two().min(16)
}

fn edge_index(units: usize) -> u8 {
    match units {
        0 | 1 => 0,
        2 => 1,
        _ => 2,
    }
}

fn set_min(mask: &mut [u8], index: usize, value: u8) {
    if let Some(slot) = mask.get_mut(index)
        && (*slot == NO_EDGE || value < *slot)
    {
        *slot = value;
    }
}

fn thresholds(level: u32, sharpness: u32) -> (i32, i32, i32) {
    let level = level.min(63);
    let mut limit = level;
    if sharpness > 0 {
        limit >>= (sharpness.saturating_add(3)) >> 2;
        limit = limit.min(9_u32.saturating_sub(sharpness));
    }
    limit = limit.max(1);
    let level = i32::try_from(level).unwrap_or_default();
    let limit = i32::try_from(limit).unwrap_or_default();
    (2 * (level + 2) + limit, limit, level >> 4)
}

fn apply_vertical(
    plane: &mut [u16],
    dimensions: (usize, usize),
    masks: &Masks,
    threshold_lut: &[(i32, i32, i32); 64],
    level_index: usize,
    chroma: bool,
    bit_depth: u32,
) -> Option<()> {
    let (width, height) = dimensions;
    let width_units = width.div_ceil(4);
    let height_units = height.div_ceil(4);
    let mask_width = width_units.checked_add(1)?;
    for y_unit in 0..height_units {
        for x_unit in 1..width_units {
            let index = y_unit.checked_mul(mask_width)?.checked_add(x_unit)?;
            let edge = *masks.vertical.get(index)?;
            if edge == NO_EDGE {
                continue;
            }
            let level = edge_level(masks, y_unit, x_unit, level_index, true)?;
            if level == 0 {
                continue;
            }
            let thresholds = *threshold_lut.get(level)?;
            let x = x_unit.checked_mul(4)?;
            let y = y_unit.checked_mul(4)?;
            let rows = height.saturating_sub(y).min(4);
            for row in 0..rows {
                filter_line(
                    plane,
                    dimensions,
                    x,
                    y.checked_add(row)?,
                    true,
                    edge,
                    thresholds,
                    chroma,
                    bit_depth,
                )?;
            }
        }
    }
    Some(())
}

fn apply_horizontal(
    plane: &mut [u16],
    dimensions: (usize, usize),
    masks: &Masks,
    threshold_lut: &[(i32, i32, i32); 64],
    level_index: usize,
    chroma: bool,
    bit_depth: u32,
) -> Option<()> {
    let (width, height) = dimensions;
    let width_units = width.div_ceil(4);
    let height_units = height.div_ceil(4);
    for y_unit in 1..height_units {
        for x_unit in 0..width_units {
            let index = y_unit.checked_mul(width_units)?.checked_add(x_unit)?;
            let edge = *masks.horizontal.get(index)?;
            if edge == NO_EDGE {
                continue;
            }
            let level = edge_level(masks, y_unit, x_unit, level_index, false)?;
            if level == 0 {
                continue;
            }
            let thresholds = *threshold_lut.get(level)?;
            let x = x_unit.checked_mul(4)?;
            let y = y_unit.checked_mul(4)?;
            let columns = width.saturating_sub(x).min(4);
            for column in 0..columns {
                filter_line(
                    plane,
                    dimensions,
                    x.checked_add(column)?,
                    y,
                    false,
                    edge,
                    thresholds,
                    chroma,
                    bit_depth,
                )?;
            }
        }
    }
    Some(())
}

/// Resolve the level on a prediction-unit edge. AV1 carries the current
/// non-zero level to the edge; when that side is disabled, the level from the
/// previous cell still enables filtering across the boundary.
fn edge_level(
    masks: &Masks,
    y_unit: usize,
    x_unit: usize,
    level_index: usize,
    vertical: bool,
) -> Option<usize> {
    let current_index = y_unit
        .checked_mul(masks.width_units)?
        .checked_add(x_unit.min(masks.width_units.saturating_sub(1)))?;
    let current = usize::from(*masks.levels.get(current_index)?.get(level_index)?);
    if current != 0 {
        return Some(current);
    }
    let previous_index = if vertical {
        x_unit.checked_sub(1)?
    } else {
        y_unit.checked_sub(1)?
    };
    let previous_index = if vertical {
        y_unit
            .checked_mul(masks.width_units)?
            .checked_add(previous_index)?
    } else {
        previous_index
            .checked_mul(masks.width_units)?
            .checked_add(x_unit)?
    };
    Some(usize::from(
        *masks.levels.get(previous_index)?.get(level_index)?,
    ))
}

struct ReplacementBuffer {
    entries: [(isize, i32); 12],
    length: usize,
}

impl ReplacementBuffer {
    const fn new() -> Self {
        Self {
            entries: [(0, 0); 12],
            length: 0,
        }
    }

    fn extend<const N: usize>(&mut self, values: [(isize, i32); N]) -> Option<()> {
        let end = self.length.checked_add(N)?;
        let destination = self.entries.get_mut(self.length..end)?;
        destination.copy_from_slice(&values);
        self.length = end;
        Some(())
    }

    fn as_slice(&self) -> &[(isize, i32)] {
        &self.entries[..self.length]
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the scalar kernel keeps the validated plane, edge geometry, thresholds, and sample depth explicit"
)]
fn filter_line(
    plane: &mut [u16],
    dimensions: (usize, usize),
    coordinate: usize,
    secondary_coordinate: usize,
    vertical: bool,
    edge: u8,
    thresholds: (i32, i32, i32),
    chroma: bool,
    bit_depth: u32,
) -> Option<()> {
    let (width, height) = dimensions;
    let (mut e, mut i, mut h) = thresholds;
    let shift = bit_depth.saturating_sub(8);
    let scale = 1_i32.checked_shl(shift)?;
    e = e.checked_mul(scale)?;
    i = i.checked_mul(scale)?;
    h = h.checked_mul(scale)?;
    let width_filter = if chroma {
        if edge >= 1 { 6 } else { 4 }
    } else {
        4 << edge.min(2)
    };
    let required_left = if width_filter >= 16 {
        7
    } else if width_filter >= 8 {
        3
    } else {
        2
    };
    let required_right = if width_filter >= 16 {
        6
    } else if width_filter >= 8 {
        3
    } else {
        2
    };

    let sample = |offset: isize| -> Option<i32> {
        let index = if vertical {
            let x = shift_coordinate(coordinate, offset)?;
            secondary_coordinate.checked_mul(width)?.checked_add(x)?
        } else {
            let y = shift_coordinate(secondary_coordinate, offset)?;
            y.checked_mul(width)?.checked_add(coordinate)?
        };
        plane.get(index).copied().map(i32::from)
    };
    let edge_coordinate = if vertical {
        coordinate
    } else {
        secondary_coordinate
    };
    if shift_coordinate(edge_coordinate, -(required_left as isize)).is_none()
        || shift_coordinate(edge_coordinate, required_right as isize).is_none()
    {
        return Some(());
    }
    if vertical && secondary_coordinate >= height || !vertical && secondary_coordinate >= width {
        return Some(());
    }

    let p0 = sample(-1)?;
    let p1 = sample(-2)?;
    let q0 = sample(0)?;
    let q1 = sample(1)?;
    let mut filter_mask = (p1 - p0).abs() <= i
        && (q1 - q0).abs() <= i
        && (p0 - q0).abs().saturating_mul(2) + ((p1 - q1).abs() >> 1) <= e;
    let p2 = if width_filter > 4 {
        Some(sample(-3)?)
    } else {
        None
    };
    let q2 = if width_filter > 4 {
        Some(sample(2)?)
    } else {
        None
    };
    if let (Some(p2), Some(q2)) = (p2, q2) {
        filter_mask &= (p2 - p1).abs() <= i && (q2 - q1).abs() <= i;
    }
    let p3 = if width_filter > 6 {
        Some(sample(-4)?)
    } else {
        None
    };
    let q3 = if width_filter > 6 {
        Some(sample(3)?)
    } else {
        None
    };
    if let (Some(p3), Some(q3)) = (p3, q3) {
        filter_mask &= (p3 - p2?).abs() <= i && (q3 - q2?).abs() <= i;
    }
    if !filter_mask {
        return Some(());
    }
    let p4 = if width_filter >= 16 {
        Some(sample(-5)?)
    } else {
        None
    };
    let q4 = if width_filter >= 16 {
        Some(sample(4)?)
    } else {
        None
    };
    let p5 = if width_filter >= 16 {
        Some(sample(-6)?)
    } else {
        None
    };
    let q5 = if width_filter >= 16 {
        Some(sample(5)?)
    } else {
        None
    };
    let p6 = if width_filter >= 16 {
        Some(sample(-7)?)
    } else {
        None
    };
    let q6 = if width_filter >= 16 {
        Some(sample(6)?)
    } else {
        None
    };
    let flat8in = if width_filter >= 6 {
        (p2? - p0).abs() <= scale
            && (p1 - p0).abs() <= scale
            && (q1 - q0).abs() <= scale
            && (q2? - q0).abs() <= scale
            && (width_filter < 8 || (p3? - p0).abs() <= scale && (q3? - q0).abs() <= scale)
    } else {
        false
    };
    let flat8out = width_filter >= 16
        && (p6? - p0).abs() <= scale
        && (p5? - p0).abs() <= scale
        && (p4? - p0).abs() <= scale
        && (q4? - q0).abs() <= scale
        && (q5? - q0).abs() <= scale
        && (q6? - q0).abs() <= scale;
    let mut replacements = ReplacementBuffer::new();
    if width_filter >= 16 && flat8in && flat8out {
        replacements.extend([
            (
                -6,
                (p6? + p6?
                    + p6?
                    + p6?
                    + p6?
                    + 2 * p6?
                    + 2 * p5?
                    + 2 * p4?
                    + p3?
                    + p2?
                    + p1
                    + p0
                    + q0
                    + 8)
                    >> 4,
            ),
            (
                -5,
                (5 * p6? + 2 * p5? + 2 * p4? + 2 * p3? + p2? + p1 + p0 + q0 + q1 + 8) >> 4,
            ),
            (
                -4,
                (4 * p6? + p5? + 2 * p4? + 2 * p3? + 2 * p2? + p1 + p0 + q0 + q1 + q2? + 8) >> 4,
            ),
            (
                -3,
                (3 * p6? + p5? + p4? + 2 * p3? + 2 * p2? + 2 * p1 + p0 + q0 + q1 + q2? + q3? + 8)
                    >> 4,
            ),
            (
                -2,
                (p6? + p6?
                    + p5?
                    + p4?
                    + p3?
                    + 2 * p2?
                    + 2 * p1
                    + 2 * p0
                    + q0
                    + q1
                    + q2?
                    + q3?
                    + q4?
                    + 8)
                    >> 4,
            ),
            (
                -1,
                (p6? + p5?
                    + p4?
                    + p3?
                    + p2?
                    + 2 * p1
                    + 2 * p0
                    + 2 * q0
                    + q1
                    + q2?
                    + q3?
                    + q4?
                    + q5?
                    + 8)
                    >> 4,
            ),
            (
                0,
                (p5? + p4?
                    + p3?
                    + p2?
                    + p1
                    + 2 * p0
                    + 2 * q0
                    + 2 * q1
                    + q2?
                    + q3?
                    + q4?
                    + q5?
                    + q6?
                    + 8)
                    >> 4,
            ),
            (
                1,
                (p4? + p3?
                    + p2?
                    + p1
                    + p0
                    + 2 * q0
                    + 2 * q1
                    + 2 * q2?
                    + q3?
                    + q4?
                    + q5?
                    + q6?
                    + q6?
                    + 8)
                    >> 4,
            ),
            (
                2,
                (p3? + p2?
                    + p1
                    + p0
                    + q0
                    + 2 * q1
                    + 2 * q2?
                    + 2 * q3?
                    + q4?
                    + q5?
                    + q6?
                    + q6?
                    + q6?
                    + 8)
                    >> 4,
            ),
            (
                3,
                (p2? + p1
                    + p0
                    + q0
                    + q1
                    + 2 * q2?
                    + 2 * q3?
                    + 2 * q4?
                    + q5?
                    + q6?
                    + q6?
                    + q6?
                    + q6?
                    + 8)
                    >> 4,
            ),
            (
                4,
                (p1 + p0
                    + q0
                    + q1
                    + q2?
                    + 2 * q3?
                    + 2 * q4?
                    + 2 * q5?
                    + q6?
                    + q6?
                    + q6?
                    + q6?
                    + q6?
                    + 8)
                    >> 4,
            ),
            (
                5,
                (p0 + q0
                    + q1
                    + q2?
                    + q3?
                    + 2 * q4?
                    + 2 * q5?
                    + 2 * q6?
                    + q6?
                    + q6?
                    + q6?
                    + q6?
                    + q6?
                    + 8)
                    >> 4,
            ),
        ])?;
    } else if width_filter >= 8 && flat8in {
        replacements.extend([
            (
                -(3_isize),
                (p3? + p3? + p3? + 2 * p2? + p1 + p0 + q0 + 4) >> 3,
            ),
            (
                -(2_isize),
                (p3? + p3? + p2? + 2 * p1 + p0 + q0 + q1 + 4) >> 3,
            ),
            (
                -(1_isize),
                (p3? + p2? + p1 + 2 * p0 + q0 + q1 + q2? + 4) >> 3,
            ),
            (0, (p2? + p1 + p0 + 2 * q0 + q1 + q2? + q3? + 4) >> 3),
            (1, (p1 + p0 + q0 + 2 * q1 + q2? + q3? + q3? + 4) >> 3),
            (2, (p0 + q0 + q1 + 2 * q2? + q3? + q3? + q3? + 4) >> 3),
        ])?;
    } else if width_filter == 6 && flat8in {
        replacements.extend([
            (-2, (p2? + 2 * p2? + 2 * p1 + 2 * p0 + q0 + 4) >> 3),
            (-1, (p2? + 2 * p1 + 2 * p0 + 2 * q0 + q1 + 4) >> 3),
            (0, (p1 + 2 * p0 + 2 * q0 + 2 * q1 + q2? + 4) >> 3),
            (1, (p0 + 2 * q0 + 2 * q1 + 2 * q2? + q2? + 4) >> 3),
        ])?;
    } else {
        let hev = (p1 - p0).abs() > h || (q1 - q0).abs() > h;
        let mut delta = 3 * (q0 - p0);
        if hev {
            let side_delta = (p1 - q1).clamp(-128 * scale, 128 * scale - 1);
            delta += side_delta;
            delta = delta.clamp(-128 * scale, 128 * scale - 1);
        } else {
            delta = delta.clamp(-128 * scale, 128 * scale - 1);
        }
        let f1 = (delta + 4).min(128 * scale - 1) >> 3;
        let f2 = (delta + 3).min(128 * scale - 1) >> 3;
        replacements.extend([(-1, p0 + f2), (0, q0 - f1)])?;
        if !hev {
            let correction = (f1 + 1) >> 1;
            replacements.extend([(-2, p1 + correction), (1, q1 - correction)])?;
        }
    }

    for &(offset, value) in replacements.as_slice() {
        let index = if vertical {
            let x = shift_coordinate(coordinate, offset)?;
            secondary_coordinate.checked_mul(width)?.checked_add(x)?
        } else {
            let y = shift_coordinate(secondary_coordinate, offset)?;
            y.checked_mul(width)?.checked_add(coordinate)?
        };
        let value = value.clamp(0, i32::from(maximum_sample(bit_depth)?));
        *plane.get_mut(index)? = u16::try_from(value).ok()?;
    }
    Some(())
}

fn maximum_sample(bit_depth: u32) -> Option<u16> {
    u16::try_from(1_u32.checked_shl(bit_depth)?.saturating_sub(1)).ok()
}

fn shift_coordinate(value: usize, offset: isize) -> Option<usize> {
    if offset.is_negative() {
        value.checked_sub(offset.unsigned_abs())
    } else {
        value.checked_add(usize::try_from(offset).ok()?)
    }
}

#[cfg(test)]
mod tests {
    use super::{Block, Parameters, apply};

    #[test]
    fn flat_planes_are_unchanged() {
        let mut planes = [vec![128; 64], vec![64; 16], vec![64; 16]];
        let blocks = [Block {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
            has_chroma: true,
            luma_tx_width: 8,
            luma_tx_height: 8,
            luma_tx_cells: None,
            chroma_tx_width: 4,
            chroma_tx_height: 4,
            skip_internal_edges: false,
            levels: [9; 4],
        }];
        assert!(
            apply(
                &mut planes,
                [(8, 8), (4, 4), (4, 4)],
                &blocks,
                Parameters {
                    sharpness: 0,
                    bit_depth: 8,
                },
                true,
                true,
            )
            .is_some()
        );
        assert_eq!(planes, [vec![128; 64], vec![64; 16], vec![64; 16]]);
    }
}
