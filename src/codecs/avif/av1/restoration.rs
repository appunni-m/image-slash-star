//! AV1 loop-restoration kernels for the bounded portable reconstruction path.
//!
//! The syntax-side coefficient reconstruction is owned by `entropy.rs`; this
//! module owns the immutable-plane Wiener operation and its checked geometry.
//! The arithmetic follows the pinned AV1 implementations in dav1d 1.5.3
//! (`src/looprestoration_tmpl.c`, `src/lr_apply_tmpl.c`) and libaom 3.13.1
//! (`av1/common/restoration.c` and `restoration.h`). The repository's existing
//! `NOTICE.md`, `PATENTS`, `third_party/dav1d/COPYING`, and
//! `third_party/libaom/{LICENSE,PATENTS}` retain the required upstream legal
//! notices; this module introduces no separately copied license text.

use super::block::{FirstLeaf, ReconstructedPlane};
use super::sample_depth::SampleDepth;
use super::{Av1Result, malformed};

/// One restoration unit's decoded operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unit {
    /// The frame restoration mode was active, but this unit selected no filter.
    None,
    /// Separable seven-tap Wiener coefficients, excluding the symmetric taps.
    Wiener {
        horizontal: [i32; 3],
        vertical: [i32; 3],
    },
}

/// Restoration parameters for the active planes of one bounded frame.
///
/// `None` in a slot means that the frame-level restoration mode for that plane
/// was disabled. `Some(Unit::None)` is intentionally distinct: the plane had
/// an active mode and its unit entropy selected the no-filter operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Plan {
    pub(super) units: [Option<Unit>; 3],
}

/// Apply the bounded single-unit Wiener plan to an I420 leaf.
///
/// The caller proves the one-unit/single-stripe admission contract before
/// entropy decoding. This function nevertheless revalidates plane extents and
/// all allocations, and consumes only a local leaf, so a failure cannot expose
/// partially restored state.
pub(super) fn restore_i420_leaf(
    mut leaf: FirstLeaf,
    plan: Plan,
    depth: SampleDepth,
) -> Av1Result<FirstLeaf> {
    if leaf.width == 0 || leaf.height == 0 {
        return Err(malformed("restoration leaf dimensions are empty"));
    }
    let chroma_width = leaf
        .width
        .checked_add(1)
        .ok_or_else(|| malformed("restoration chroma width overflows"))?
        / 2;
    let chroma_height = leaf
        .height
        .checked_add(1)
        .ok_or_else(|| malformed("restoration chroma height overflows"))?
        / 2;
    let dimensions = [
        (leaf.width, leaf.height),
        (chroma_width, chroma_height),
        (chroma_width, chroma_height),
    ];
    for (plane, unit) in plan.units.into_iter().enumerate() {
        let Some(unit) = unit else {
            continue;
        };
        let Unit::Wiener {
            horizontal,
            vertical,
        } = unit
        else {
            continue;
        };
        restore_plane(
            &mut leaf.planes[plane],
            dimensions[plane],
            horizontal,
            vertical,
            depth,
        )?;
    }
    Ok(leaf)
}

fn restore_plane(
    plane: &mut ReconstructedPlane,
    (width, height): (u32, u32),
    horizontal: [i32; 3],
    vertical: [i32; 3],
    depth: SampleDepth,
) -> Av1Result<()> {
    let width =
        usize::try_from(width).map_err(|_| malformed("restoration plane width exceeds usize"))?;
    let height =
        usize::try_from(height).map_err(|_| malformed("restoration plane height exceeds usize"))?;
    let sample_count = width
        .checked_mul(height)
        .ok_or_else(|| malformed("restoration plane sample count overflows"))?;
    if width == 0 || height == 0 || plane.samples.len() != sample_count {
        return Err(malformed("restoration plane extent is invalid"));
    }

    let horizontal_filter = full_filter(horizontal);
    let vertical_filter = full_filter(vertical);
    let mut intermediate = Vec::<i32>::new();
    intermediate
        .try_reserve(sample_count)
        .map_err(|_| malformed("unable to allocate restoration intermediate"))?;
    intermediate.resize(sample_count, 0);

    // AV1's 8/10-bit and 12-bit Wiener stages use different split-rounding
    // schedules. The current admission is 8-bit, but keeping the exact depth
    // table here makes the kernel safe for the later high-depth tranche.
    let round_h: u32 = if depth.bits() == 12 { 5 } else { 3 };
    let round_v: u32 = if depth.bits() == 12 { 9 } else { 11 };
    let horizontal_bias = 1_i64
        .checked_shl(depth.bits().saturating_add(6))
        .ok_or_else(|| malformed("restoration horizontal bias overflows"))?;
    let horizontal_round = 1_i64
        .checked_shl(round_h.saturating_sub(1))
        .ok_or_else(|| malformed("restoration horizontal rounding overflows"))?;
    let horizontal_max = 1_i64
        .checked_shl(depth.bits().saturating_add(8).saturating_sub(round_h))
        .and_then(|value| value.checked_sub(1))
        .ok_or_else(|| malformed("restoration horizontal range overflows"))?;

    for y in 0..height {
        let row = y
            .checked_mul(width)
            .ok_or_else(|| malformed("restoration horizontal row overflows"))?;
        for x in 0..width {
            let mut sum = horizontal_bias;
            for (tap, &coefficient) in horizontal_filter.iter().enumerate() {
                let offset = isize::try_from(tap)
                    .ok()
                    .and_then(|tap| tap.checked_sub(3))
                    .ok_or_else(|| malformed("restoration horizontal tap overflows"))?;
                let source_x = clamp_offset(x, offset, width);
                let source_index = row
                    .checked_add(source_x)
                    .ok_or_else(|| malformed("restoration horizontal index overflows"))?;
                let sample = i64::from(
                    *plane
                        .samples
                        .get(source_index)
                        .ok_or_else(|| malformed("restoration horizontal source is missing"))?,
                );
                sum = sum
                    .checked_add(
                        sample
                            .checked_mul(i64::from(coefficient))
                            .ok_or_else(|| malformed("restoration horizontal product overflows"))?,
                    )
                    .ok_or_else(|| malformed("restoration horizontal sum overflows"))?;
            }
            let value = (sum
                .checked_add(horizontal_round)
                .ok_or_else(|| malformed("restoration horizontal rounding overflows"))?)
                >> round_h;
            let value = value.clamp(0, horizontal_max);
            let value = i32::try_from(value)
                .map_err(|_| malformed("restoration horizontal sample exceeds i32"))?;
            let destination = row
                .checked_add(x)
                .ok_or_else(|| malformed("restoration horizontal destination overflows"))?;
            *intermediate
                .get_mut(destination)
                .ok_or_else(|| malformed("restoration horizontal destination is missing"))? = value;
        }
    }

    let vertical_bias = -(1_i64
        .checked_shl(depth.bits().saturating_add(round_v).saturating_sub(1))
        .ok_or_else(|| malformed("restoration vertical bias overflows"))?);
    let vertical_round = 1_i64
        .checked_shl(round_v.saturating_sub(1))
        .ok_or_else(|| malformed("restoration vertical rounding overflows"))?;
    let maximum = i64::from(depth.maximum());
    let mut restored = Vec::<u16>::new();
    restored
        .try_reserve(sample_count)
        .map_err(|_| malformed("unable to allocate restoration output"))?;
    restored.resize(sample_count, 0);
    for y in 0..height {
        let row = y
            .checked_mul(width)
            .ok_or_else(|| malformed("restoration vertical row overflows"))?;
        for x in 0..width {
            let mut sum = vertical_bias;
            for (tap, &coefficient) in vertical_filter.iter().enumerate() {
                let offset = isize::try_from(tap)
                    .ok()
                    .and_then(|tap| tap.checked_sub(3))
                    .ok_or_else(|| malformed("restoration vertical tap overflows"))?;
                let source_y = clamp_offset(y, offset, height);
                let source_row = source_y
                    .checked_mul(width)
                    .ok_or_else(|| malformed("restoration vertical source row overflows"))?;
                let source_index = source_row
                    .checked_add(x)
                    .ok_or_else(|| malformed("restoration vertical index overflows"))?;
                let sample = i64::from(
                    *intermediate
                        .get(source_index)
                        .ok_or_else(|| malformed("restoration vertical source is missing"))?,
                );
                sum = sum
                    .checked_add(
                        sample
                            .checked_mul(i64::from(coefficient))
                            .ok_or_else(|| malformed("restoration vertical product overflows"))?,
                    )
                    .ok_or_else(|| malformed("restoration vertical sum overflows"))?;
            }
            let value = (sum
                .checked_add(vertical_round)
                .ok_or_else(|| malformed("restoration vertical rounding overflows"))?)
                >> round_v;
            let value = value.clamp(0, maximum);
            let value = u16::try_from(value)
                .map_err(|_| malformed("restoration output exceeds sample depth"))?;
            let destination = row
                .checked_add(x)
                .ok_or_else(|| malformed("restoration output index overflows"))?;
            *restored
                .get_mut(destination)
                .ok_or_else(|| malformed("restoration output is missing"))? = value;
        }
    }
    plane.samples = restored;
    Ok(())
}

fn full_filter(side: [i32; 3]) -> [i32; 7] {
    let sum = side[0] + side[1] + side[2];
    let center = 128_i32 - 2 * sum;
    [side[0], side[1], side[2], center, side[2], side[1], side[0]]
}

fn clamp_offset(index: usize, offset: isize, extent: usize) -> usize {
    let shifted = if offset.is_negative() {
        index.saturating_sub(offset.unsigned_abs())
    } else {
        index.saturating_add(offset as usize)
    };
    shifted.min(extent.saturating_sub(1))
}
