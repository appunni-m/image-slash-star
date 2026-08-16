//! Safe scalar AV1 constrained directional enhancement filtering.
//!
//! CDEF runs after block reconstruction and before the later restoration
//! stages.  This module deliberately works on checked slices and returns one
//! bounded block, so a future frame walker can decide when a neighboring
//! block is available without relying on padded pointers or aliasing.

#![expect(
    clippy::arithmetic_side_effects,
    reason = "AV1 CDEF cost and tap arithmetic is bounded by the validated 8x8 block and 16-bit samples"
)]

const DIRECTIONS: [[(isize, isize); 2]; 8] = [
    [(-1, 1), (-2, 2)],
    [(0, 1), (-1, 2)],
    [(0, 1), (0, 2)],
    [(0, 1), (1, 2)],
    [(1, 1), (2, 2)],
    [(1, 0), (2, 1)],
    [(1, 0), (2, 0)],
    [(1, 0), (2, -1)],
];

const PRIMARY_TAPS: [[i32; 2]; 2] = [[4, 2], [3, 3]];
const SECONDARY_TAPS: [[i32; 2]; 2] = [[2, 1], [2, 1]];

#[derive(Clone, Copy)]
pub(crate) struct Block {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct Parameters {
    pub(crate) primary_strength: u32,
    pub(crate) secondary_strength: u32,
    pub(crate) direction: usize,
    pub(crate) damping: u32,
    pub(crate) bit_depth: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct FrameParameters {
    pub(crate) damping: u32,
    pub(crate) bit_depth: u32,
    pub(crate) y_strengths: [u32; 4],
    pub(crate) uv_strengths: [u32; 4],
    pub(crate) y_strength_count: usize,
    pub(crate) uv_strength_count: usize,
}

/// Find AV1's constrained-direction for one complete 8×8 luma block.
///
/// The returned variance is the value used to adjust the primary strength.
/// A partial edge block is intentionally rejected: the frame-level caller can
/// safely leave such a block unchanged until it has the padded edge context
/// required by the AV1 CDEF syntax.
pub(crate) fn direction_for_block(
    source: &[u16],
    dimensions: (usize, usize),
    block: Block,
    bit_depth: u32,
) -> Option<(usize, u32)> {
    let (width, height) = dimensions;
    if block.width != 8
        || block.height != 8
        || block.x.checked_add(8)? > width
        || block.y.checked_add(8)? > height
        || source.len() != width.checked_mul(height)?
        || !(8..=16).contains(&bit_depth)
    {
        return None;
    }

    let shift = bit_depth.saturating_sub(8);
    let mut partial_sum_hv = [[0_i64; 8]; 2];
    let mut partial_sum_diag = [[0_i64; 15]; 2];
    let mut partial_sum_alt = [[0_i64; 11]; 4];
    for row in 0..8 {
        for column in 0..8 {
            let index = block
                .y
                .checked_add(row)?
                .checked_mul(width)?
                .checked_add(block.x.checked_add(column)?)?;
            let sample = i64::from(*source.get(index)? >> shift) - 128;
            partial_sum_diag[0][row + column] += sample;
            partial_sum_alt[0][row + (column >> 1)] += sample;
            partial_sum_hv[0][row] += sample;
            partial_sum_alt[1][3 + row - (column >> 1)] += sample;
            partial_sum_diag[1][7 + row - column] += sample;
            partial_sum_alt[2][3 - (row >> 1) + column] += sample;
            partial_sum_hv[1][column] += sample;
            partial_sum_alt[3][(row >> 1) + column] += sample;
        }
    }

    let mut cost = [0_u64; 8];
    for value in partial_sum_hv[0] {
        cost[2] = cost[2].saturating_add(square(value));
    }
    for value in partial_sum_hv[1] {
        cost[6] = cost[6].saturating_add(square(value));
    }
    cost[2] = cost[2].saturating_mul(105);
    cost[6] = cost[6].saturating_mul(105);

    const DIV_TABLE: [u64; 7] = [840, 420, 280, 210, 168, 140, 120];
    for index in 0..7 {
        let diagonal_cost = square(partial_sum_diag[0][index])
            .saturating_add(square(partial_sum_diag[0][14 - index]));
        let other_diagonal_cost = square(partial_sum_diag[1][index])
            .saturating_add(square(partial_sum_diag[1][14 - index]));
        cost[0] = cost[0].saturating_add(diagonal_cost.saturating_mul(DIV_TABLE[index]));
        cost[4] = cost[4].saturating_add(other_diagonal_cost.saturating_mul(DIV_TABLE[index]));
    }
    cost[0] = cost[0].saturating_add(square(partial_sum_diag[0][7]).saturating_mul(105));
    cost[4] = cost[4].saturating_add(square(partial_sum_diag[1][7]).saturating_mul(105));

    for (index, partial) in partial_sum_alt.iter().enumerate() {
        let cost_index = index * 2 + 1;
        for middle in 0..5 {
            cost[cost_index] = cost[cost_index].saturating_add(square(partial[3 + middle]));
        }
        cost[cost_index] = cost[cost_index].saturating_mul(105);
        for middle in 0..3 {
            let divisor = DIV_TABLE[2 * middle + 1];
            cost[cost_index] = cost[cost_index].saturating_add(
                square(partial[middle])
                    .saturating_add(square(partial[10 - middle]))
                    .saturating_mul(divisor),
            );
        }
    }

    let mut best_direction = 0;
    let mut best_cost = cost[0];
    for (direction, candidate) in cost.iter().copied().enumerate().skip(1) {
        if candidate > best_cost {
            best_cost = candidate;
            best_direction = direction;
        }
    }
    let variance = best_cost.saturating_sub(cost[best_direction ^ 4]) >> 10;
    Some((best_direction, u32::try_from(variance).ok()?))
}

pub(crate) fn adjust_primary_strength(strength: u32, variance: u32) -> u32 {
    if variance == 0 {
        return 0;
    }
    let logarithm = if variance >> 6 == 0 {
        0
    } else {
        (u32::BITS
            .saturating_sub((variance >> 6).leading_zeros())
            .saturating_sub(1))
        .min(12)
    };
    strength
        .saturating_mul(4_u32.saturating_add(logarithm))
        .saturating_add(8)
        >> 4
}

fn square(value: i64) -> u64 {
    value.unsigned_abs().saturating_mul(value.unsigned_abs())
}

/// Filter one reconstructed block with the scalar AV1 CDEF kernel.
///
/// `source` is a complete row-major plane. `x` and `y` identify the block's
/// top-left sample, while `block_width` and `block_height` are each at most
/// eight (four for a horizontally or vertically subsampled 4:2:0 plane).
/// Neighbors outside the plane are treated as unavailable padding, matching
/// the AV1 edge rule without constructing a padded allocation.
///
/// Strengths use the six-bit AV1 frame-header values. The result is a newly
/// owned block in row-major order; callers can validate the whole frame before
/// copying it into a canvas.
pub(crate) fn filter_block(
    source: &[u16],
    dimensions: (usize, usize),
    block: Block,
    parameters: Parameters,
) -> Option<Vec<u16>> {
    let (width, height) = dimensions;
    let Block {
        x,
        y,
        width: block_width,
        height: block_height,
    } = block;
    let Parameters {
        primary_strength,
        secondary_strength,
        direction,
        damping,
        bit_depth,
    } = parameters;
    let source_len = width.checked_mul(height)?;
    if source.len() != source_len
        || direction >= DIRECTIONS.len()
        || block_width == 0
        || block_width > 8
        || block_height == 0
        || block_height > 8
        || x.checked_add(block_width)? > width
        || y.checked_add(block_height)? > height
    {
        return None;
    }
    let maximum_sample = maximum_sample(bit_depth)?;
    let tap_set = usize::try_from(primary_strength >> bit_depth.checked_sub(8)?).ok()? & 1;
    let primary_strength = i32::try_from(primary_strength).ok()?;
    let secondary_strength = i32::try_from(secondary_strength).ok()?;
    let primary_taps = PRIMARY_TAPS[tap_set];
    let secondary_taps = SECONDARY_TAPS[tap_set];
    let damping = i32::try_from(damping).ok()?;
    let mut output = vec![0_u16; block_width.checked_mul(block_height)?];

    for row in 0..block_height {
        for column in 0..block_width {
            let source_row = y.saturating_add(row);
            let source_column = x.saturating_add(column);
            let origin = i32::from(
                *source.get(
                    source_row
                        .saturating_mul(width)
                        .saturating_add(source_column),
                )?,
            );
            let mut sum = 0_i32;
            let mut minimum = origin;
            let mut maximum = origin;

            // `DIRECTIONS` is indexed by AV1's canonical direction number.
            // The dav1d table stores two sentinel entries before direction 0,
            // so its `dir + 2`, `dir + 4`, and `dir` accesses correspond to
            // canonical `dir`, `dir + 2`, and `dir - 2` respectively.
            let primary_direction = direction;
            let secondary_directions = [
                direction.saturating_add(2) & 7,
                direction.saturating_add(6) & 7,
            ];
            for tap_index in 0..2_usize {
                let primary_offset = DIRECTIONS[primary_direction][tap_index];
                for offset in [primary_offset, negate_offset(primary_offset)] {
                    if let Some(sample) =
                        sample_at(source, width, height, source_row, source_column, offset)
                    {
                        minimum = minimum.min(sample);
                        maximum = maximum.max(sample);
                        let difference = sample.wrapping_sub(origin);
                        let constrained = constrain(difference, primary_strength, damping);
                        sum =
                            sum.saturating_add(primary_taps[tap_index].saturating_mul(constrained));
                    }
                }

                for secondary_direction in secondary_directions {
                    let secondary_offset = DIRECTIONS[secondary_direction][tap_index];
                    for offset in [secondary_offset, negate_offset(secondary_offset)] {
                        if let Some(sample) =
                            sample_at(source, width, height, source_row, source_column, offset)
                        {
                            minimum = minimum.min(sample);
                            maximum = maximum.max(sample);
                            let difference = sample.wrapping_sub(origin);
                            let constrained = constrain(difference, secondary_strength, damping);
                            sum = sum.saturating_add(
                                secondary_taps[tap_index].saturating_mul(constrained),
                            );
                        }
                    }
                }
            }

            let correction = sum.saturating_add(8).saturating_sub(i32::from(sum < 0)) >> 4;
            let filtered = origin.saturating_add(correction).clamp(minimum, maximum);
            let filtered = filtered.clamp(0, i32::from(maximum_sample));
            let output_index = row.saturating_mul(block_width).saturating_add(column);
            *output.get_mut(output_index)? = u16::try_from(filtered).ok()?;
        }
    }
    Some(output)
}

fn maximum_sample(bit_depth: u32) -> Option<u16> {
    if !(8..=16).contains(&bit_depth) {
        return None;
    }
    let maximum = 1_u32.checked_shl(bit_depth)?.saturating_sub(1);
    u16::try_from(maximum).ok()
}

fn negate_offset(offset: (isize, isize)) -> (isize, isize) {
    (offset.0.wrapping_neg(), offset.1.wrapping_neg())
}

fn sample_at(
    source: &[u16],
    width: usize,
    height: usize,
    row: usize,
    column: usize,
    offset: (isize, isize),
) -> Option<i32> {
    if width == 0 || height == 0 {
        return None;
    }
    let Some(row) = shift_coordinate(row, offset.0) else {
        return Some(i32::from(i16::MIN));
    };
    let Some(column) = shift_coordinate(column, offset.1) else {
        return Some(i32::from(i16::MIN));
    };
    if row >= height || column >= width {
        // dav1d's padded CDEF window uses INT16_MIN for an unavailable
        // frame edge. Keeping the sentinel in the tap set matters for the
        // clamp range even though its constrained contribution is zero.
        return Some(i32::from(i16::MIN));
    }
    let index = row.checked_mul(width)?.checked_add(column)?;
    source.get(index).copied().map(i32::from)
}

fn shift_coordinate(value: usize, offset: isize) -> Option<usize> {
    if offset.is_negative() {
        value.checked_sub(offset.unsigned_abs())
    } else {
        value.checked_add(usize::try_from(offset).ok()?)
    }
}

fn constrain(diff: i32, threshold: i32, damping: i32) -> i32 {
    if threshold == 0 {
        return 0;
    }
    let shift = damping
        .saturating_sub(most_significant_bit(threshold))
        .max(0);
    let shifted = diff
        .unsigned_abs()
        .wrapping_shr(u32::try_from(shift).unwrap_or(31));
    let shifted = i32::try_from(shifted).unwrap_or(i32::MAX);
    let difference = i32::try_from(diff.unsigned_abs()).unwrap_or(i32::MAX);
    let magnitude = threshold.saturating_sub(shifted).clamp(0, difference);
    if diff < 0 {
        magnitude.wrapping_neg()
    } else {
        magnitude
    }
}

fn most_significant_bit(value: i32) -> i32 {
    i32::try_from(
        i32::BITS
            .saturating_sub(value.leading_zeros())
            .saturating_sub(1),
    )
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{Block, Parameters, direction_for_block, filter_block};

    #[test]
    fn direction_matches_scalar_reference_cost_order() {
        let source = vec![
            23, 22, 20, 33, 26, 26, 19, 25, 23, 22, 20, 33, 26, 26, 19, 25, 23, 22, 20, 33, 26, 26,
            19, 25, 23, 22, 20, 33, 26, 26, 19, 25, 23, 22, 20, 33, 26, 26, 19, 25, 23, 22, 20, 33,
            26, 26, 19, 25, 23, 22, 20, 33, 26, 26, 19, 25, 23, 24, 20, 33, 26, 26, 19, 25,
        ];
        assert_eq!(
            direction_for_block(
                &source,
                (8, 8),
                Block {
                    x: 0,
                    y: 0,
                    width: 8,
                    height: 8,
                },
                8,
            ),
            Some((6, 881))
        );
    }

    #[test]
    fn zero_strength_is_identity() {
        let source: Vec<_> = (0_u16..64).collect();
        let filtered = filter_block(
            &source,
            (8, 8),
            Block {
                x: 0,
                y: 0,
                width: 8,
                height: 8,
            },
            Parameters {
                primary_strength: 0,
                secondary_strength: 0,
                direction: 0,
                damping: 3,
                bit_depth: 8,
            },
        );
        assert_eq!(filtered, Some(source));
    }

    #[test]
    fn flat_block_is_identity_with_active_filter() {
        let source = vec![128_u16; 64];
        let filtered = filter_block(
            &source,
            (8, 8),
            Block {
                x: 0,
                y: 0,
                width: 8,
                height: 8,
            },
            Parameters {
                primary_strength: 4,
                secondary_strength: 3,
                direction: 2,
                damping: 3,
                bit_depth: 8,
            },
        );
        assert_eq!(filtered, Some(source));
    }

    #[test]
    fn output_is_bounded_and_deterministic_at_edges() {
        let source: Vec<_> = (0_u16..64).map(|sample| sample.saturating_mul(4)).collect();
        let block = Block {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
        };
        let parameters = Parameters {
            primary_strength: 63,
            secondary_strength: 63,
            direction: 7,
            damping: 6,
            bit_depth: 8,
        };
        let first = filter_block(&source, (8, 8), block, parameters);
        let second = filter_block(&source, (8, 8), block, parameters);
        assert_eq!(first, second);
        assert!(first.is_some_and(|block| block.iter().all(|&sample| sample <= 255)));
    }

    #[test]
    fn malformed_geometry_is_rejected_without_indexing() {
        let source = vec![0_u16; 64];
        let block = Block {
            x: 7,
            y: 0,
            width: 2,
            height: 1,
        };
        let parameters = Parameters {
            primary_strength: 0,
            secondary_strength: 0,
            direction: 0,
            damping: 3,
            bit_depth: 8,
        };
        assert!(filter_block(&source, (8, 8), block, parameters).is_none());
        assert!(
            filter_block(
                &source,
                (8, 8),
                Block {
                    x: 0,
                    y: 0,
                    width: 8,
                    height: 8,
                },
                Parameters {
                    direction: 8,
                    ..parameters
                },
            )
            .is_none()
        );
        assert!(
            filter_block(
                &source,
                (8, 8),
                Block {
                    x: 0,
                    y: 0,
                    width: 8,
                    height: 8,
                },
                Parameters {
                    bit_depth: 7,
                    ..parameters
                },
            )
            .is_none()
        );
    }
}
