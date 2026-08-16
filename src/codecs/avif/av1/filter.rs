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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Block {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) luma_tx_width: usize,
    pub(crate) luma_tx_height: usize,
    pub(crate) chroma_tx_width: usize,
    pub(crate) chroma_tx_height: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Parameters {
    pub(crate) luma_vertical: u32,
    pub(crate) luma_horizontal: u32,
    pub(crate) chroma_u: u32,
    pub(crate) chroma_v: u32,
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

    let luma_mask = build_masks(dimensions[0], blocks, false)?;
    let chroma_mask = build_masks(dimensions[1], blocks, true)?;
    let luma_lut = thresholds(parameters.luma_vertical, parameters.sharpness);
    let luma_horizontal_lut = thresholds(parameters.luma_horizontal, parameters.sharpness);
    let chroma_u_lut = thresholds(parameters.chroma_u, parameters.sharpness);
    let chroma_v_lut = thresholds(parameters.chroma_v, parameters.sharpness);

    apply_vertical(
        &mut planes[0],
        dimensions[0],
        &luma_mask.vertical,
        luma_lut,
        false,
        parameters.bit_depth,
    )?;
    apply_horizontal(
        &mut planes[0],
        dimensions[0],
        &luma_mask.horizontal,
        luma_horizontal_lut,
        false,
        parameters.bit_depth,
    )?;
    apply_vertical(
        &mut planes[1],
        dimensions[1],
        &chroma_mask.vertical,
        chroma_u_lut,
        true,
        parameters.bit_depth,
    )?;
    apply_horizontal(
        &mut planes[1],
        dimensions[1],
        &chroma_mask.horizontal,
        chroma_u_lut,
        true,
        parameters.bit_depth,
    )?;
    apply_vertical(
        &mut planes[2],
        dimensions[1],
        &chroma_mask.vertical,
        chroma_v_lut,
        true,
        parameters.bit_depth,
    )?;
    apply_horizontal(
        &mut planes[2],
        dimensions[1],
        &chroma_mask.horizontal,
        chroma_v_lut,
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
}

fn build_masks(dimensions: (usize, usize), blocks: &[Block], chroma: bool) -> Option<Masks> {
    let (width, height) = dimensions;
    let width_units = width.div_ceil(4);
    let height_units = height.div_ceil(4);
    let mask_width = width_units.checked_add(1)?;
    let mask_height = height_units.checked_add(1)?;
    let mut masks = Masks {
        vertical: vec![NO_EDGE; mask_width.checked_mul(height_units)?],
        horizontal: vec![NO_EDGE; mask_height.checked_mul(width_units)?],
        width_units,
        height_units,
    };

    for block in blocks {
        let (x, y, block_width, block_height, tx_width, tx_height) = if chroma {
            (
                block.x / 2,
                block.y / 2,
                block.width.div_ceil(2),
                block.height.div_ceil(2),
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
    mask: &[u8],
    thresholds: (i32, i32, i32),
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
            let edge = *mask.get(index)?;
            if edge == NO_EDGE {
                continue;
            }
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
    mask: &[u8],
    thresholds: (i32, i32, i32),
    chroma: bool,
    bit_depth: u32,
) -> Option<()> {
    let (width, height) = dimensions;
    let width_units = width.div_ceil(4);
    let height_units = height.div_ceil(4);
    for y_unit in 1..height_units {
        for x_unit in 0..width_units {
            let index = y_unit.checked_mul(width_units)?.checked_add(x_unit)?;
            let edge = *mask.get(index)?;
            if edge == NO_EDGE {
                continue;
            }
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
    let mut replacements = Vec::new();
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
        ]);
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
        ]);
    } else if width_filter == 6 && flat8in {
        replacements.extend([
            (-2, (p2? + 2 * p2? + 2 * p1 + 2 * p0 + q0 + 4) >> 3),
            (-1, (p2? + 2 * p1 + 2 * p0 + 2 * q0 + q1 + 4) >> 3),
            (0, (p1 + 2 * p0 + 2 * q0 + 2 * q1 + q2? + 4) >> 3),
            (1, (p0 + 2 * q0 + 2 * q1 + 2 * q2? + q2? + 4) >> 3),
        ]);
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
        replacements.extend([(-1, p0 + f2), (0, q0 - f1)]);
        if !hev {
            let correction = (f1 + 1) >> 1;
            replacements.extend([(-2, p1 + correction), (1, q1 - correction)]);
        }
    }

    for (offset, value) in replacements {
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
            luma_tx_width: 8,
            luma_tx_height: 8,
            chroma_tx_width: 4,
            chroma_tx_height: 4,
        }];
        assert!(
            apply(
                &mut planes,
                [(8, 8), (4, 4), (4, 4)],
                &blocks,
                Parameters {
                    luma_vertical: 9,
                    luma_horizontal: 9,
                    chroma_u: 9,
                    chroma_v: 9,
                    sharpness: 0,
                    bit_depth: 8,
                },
            )
            .is_some()
        );
        assert_eq!(planes, [vec![128; 64], vec![64; 16], vec![64; 16]]);
    }
}
