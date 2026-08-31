//! AV1 loop-restoration kernels for the bounded portable reconstruction path.
//!
//! The syntax-side coefficient reconstruction is owned by `entropy.rs`; this
//! module owns the immutable-plane Wiener/SGR operations and their checked geometry.
//! The arithmetic follows the pinned AV1 implementations in dav1d 1.5.3
//! (`src/looprestoration_tmpl.c`, `src/lr_apply_tmpl.c`) and libaom 3.13.2
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
    /// Self-guided restoration parameter index and decoded projection values.
    SgrProjection {
        parameter_index: u8,
        weights: [i32; 2],
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

/// Apply the bounded single-unit Wiener/SGR plan to an I420 leaf.
///
/// The caller proves the one-unit/single-stripe admission contract before
/// entropy decoding. This function nevertheless revalidates plane extents and
/// all allocations, and consumes only a local leaf, so a failure cannot expose
/// partially restored state.
pub(super) fn restore_i420_leaf(
    leaf: FirstLeaf,
    plan: Plan,
    depth: SampleDepth,
) -> Av1Result<FirstLeaf> {
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
    restore_leaf_with_dimensions(leaf, plan, depth, dimensions)
}

/// Apply the bounded single-unit Wiener/SGR plan to an I422 leaf.
///
/// Horizontal subsampling halves the chroma width, while the vertical extent
/// remains full-height. The caller proves the one-unit/single-stripe profile;
/// this boundary still revalidates the exact plane buffers before mutation.
pub(super) fn restore_i422_leaf(
    leaf: FirstLeaf,
    plan: Plan,
    depth: SampleDepth,
) -> Av1Result<FirstLeaf> {
    let chroma_width = leaf
        .width
        .checked_add(1)
        .ok_or_else(|| malformed("restoration chroma width overflows"))?
        / 2;
    let dimensions = [
        (leaf.width, leaf.height),
        (chroma_width, leaf.height),
        (chroma_width, leaf.height),
    ];
    restore_leaf_with_dimensions(leaf, plan, depth, dimensions)
}

/// Apply the bounded single-unit restoration plan to a full-resolution I444
/// leaf. Unlike 4:2:0, all three planes retain the visible frame dimensions.
pub(super) fn restore_i444_leaf(
    leaf: FirstLeaf,
    plan: Plan,
    depth: SampleDepth,
) -> Av1Result<FirstLeaf> {
    let dimensions = [(leaf.width, leaf.height); 3];
    restore_leaf_with_dimensions(leaf, plan, depth, dimensions)
}

fn restore_leaf_with_dimensions(
    mut leaf: FirstLeaf,
    plan: Plan,
    depth: SampleDepth,
    dimensions: [(u32, u32); 3],
) -> Av1Result<FirstLeaf> {
    if leaf.width == 0 || leaf.height == 0 {
        return Err(malformed("restoration leaf dimensions are empty"));
    }
    for (plane, &(width, height)) in dimensions.iter().enumerate() {
        let width = usize::try_from(width)
            .map_err(|_| malformed("restoration plane width exceeds usize"))?;
        let height = usize::try_from(height)
            .map_err(|_| malformed("restoration plane height exceeds usize"))?;
        let sample_count = width
            .checked_mul(height)
            .ok_or_else(|| malformed("restoration plane sample count overflows"))?;
        if width == 0 || height == 0 || leaf.planes[plane].samples.len() != sample_count {
            return Err(malformed("restoration plane extent is invalid"));
        }
    }
    for (plane, unit) in plan.units.into_iter().enumerate() {
        let Some(unit) = unit else {
            continue;
        };
        match unit {
            Unit::None => {}
            Unit::Wiener {
                horizontal,
                vertical,
            } => restore_wiener_plane(
                &mut leaf.planes[plane],
                dimensions[plane],
                horizontal,
                vertical,
                depth,
            )?,
            Unit::SgrProjection {
                parameter_index,
                weights,
            } => restore_sgr_plane(
                &mut leaf.planes[plane],
                dimensions[plane],
                parameter_index,
                weights,
                depth,
            )?,
        }
    }
    Ok(leaf)
}

/// Apply the bounded single-unit restoration plan to one monochrome frame.
///
/// The entropy admission proves that only plane zero is active and that the
/// frame fits inside one restoration unit. This boundary still checks the
/// dimensions and sample buffer before invoking the depth-aware kernels, and
/// it consumes a private plane so a failure cannot publish partial state.
pub(super) fn restore_monochrome_plane(
    mut plane: ReconstructedPlane,
    width: u32,
    height: u32,
    plan: Plan,
    depth: SampleDepth,
) -> Av1Result<ReconstructedPlane> {
    if plan.units[1].is_some() || plan.units[2].is_some() {
        return Err(malformed("monochrome restoration carries a chroma unit"));
    }
    let unit = plan.units[0];
    if unit.is_none() {
        return Ok(plane);
    }
    match unit.ok_or_else(|| malformed("monochrome restoration unit is missing"))? {
        Unit::None => {}
        Unit::Wiener {
            horizontal,
            vertical,
        } => restore_wiener_plane(&mut plane, (width, height), horizontal, vertical, depth)?,
        Unit::SgrProjection {
            parameter_index,
            weights,
        } => restore_sgr_plane(&mut plane, (width, height), parameter_index, weights, depth)?,
    }
    Ok(plane)
}

fn restore_wiener_plane(
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

#[derive(Clone, Copy)]
struct SgrParameters {
    radius: [u8; 2],
    scale: [u32; 2],
}

// ✅ VERIFIED: dav1d 1.5.3 src/tables.c:419-424 and libaom 3.13.2
// av1/common/restoration.c:37-46. The first pass is the 5x5/radius-2
// filter; the second is the 3x3/radius-1 filter.
const SGR_PARAMETERS: [SgrParameters; 16] = [
    SgrParameters {
        radius: [2, 1],
        scale: [140, 3236],
    },
    SgrParameters {
        radius: [2, 1],
        scale: [112, 2158],
    },
    SgrParameters {
        radius: [2, 1],
        scale: [93, 1618],
    },
    SgrParameters {
        radius: [2, 1],
        scale: [80, 1438],
    },
    SgrParameters {
        radius: [2, 1],
        scale: [70, 1295],
    },
    SgrParameters {
        radius: [2, 1],
        scale: [58, 1177],
    },
    SgrParameters {
        radius: [2, 1],
        scale: [47, 1079],
    },
    SgrParameters {
        radius: [2, 1],
        scale: [37, 996],
    },
    SgrParameters {
        radius: [2, 1],
        scale: [30, 925],
    },
    SgrParameters {
        radius: [2, 1],
        scale: [25, 863],
    },
    SgrParameters {
        radius: [0, 1],
        scale: [0, 2589],
    },
    SgrParameters {
        radius: [0, 1],
        scale: [0, 1618],
    },
    SgrParameters {
        radius: [0, 1],
        scale: [0, 1177],
    },
    SgrParameters {
        radius: [0, 1],
        scale: [0, 925],
    },
    SgrParameters {
        radius: [2, 0],
        scale: [56, 0],
    },
    SgrParameters {
        radius: [2, 0],
        scale: [22, 0],
    },
];

// ✅ VERIFIED: dav1d 1.5.3 src/tables.c:426-445. This is intentionally a
// literal table: index zero maps to 255 and index 255 maps to zero, and the
// endpoint behavior is part of the AV1 fixed-point definition.
const SGR_X_BY_X: [u8; 256] = [
    255, 128, 85, 64, 51, 43, 37, 32, 28, 26, 23, 21, 20, 18, 17, 16, 15, 14, 13, 13, 12, 12, 11,
    11, 10, 10, 9, 9, 9, 9, 8, 8, 8, 8, 7, 7, 7, 7, 7, 6, 6, 6, 6, 6, 6, 6, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 0,
];

#[derive(Clone, Copy)]
struct SgrIntermediate {
    mean: i64,
    gain: i64,
}

fn restore_sgr_plane(
    plane: &mut ReconstructedPlane,
    (width, height): (u32, u32),
    parameter_index: u8,
    weights: [i32; 2],
    depth: SampleDepth,
) -> Av1Result<()> {
    let width = usize::try_from(width).map_err(|_| malformed("SGR plane width exceeds usize"))?;
    let height =
        usize::try_from(height).map_err(|_| malformed("SGR plane height exceeds usize"))?;
    let sample_count = width
        .checked_mul(height)
        .ok_or_else(|| malformed("SGR plane sample count overflows"))?;
    if width == 0 || height == 0 || plane.samples.len() != sample_count {
        return Err(malformed("SGR plane extent is invalid"));
    }
    let parameter = SGR_PARAMETERS
        .get(usize::from(parameter_index))
        .copied()
        .ok_or_else(|| malformed("SGR parameter index exceeds sixteen"))?;
    if parameter.radius == [0, 0] {
        return Err(malformed("SGR parameter disables both radii"));
    }

    let mut source = Vec::<u16>::new();
    source
        .try_reserve(sample_count)
        .map_err(|_| malformed("unable to allocate SGR source"))?;
    source.extend_from_slice(&plane.samples);
    let mut radius_two = None;
    if parameter.radius[0] != 0 {
        radius_two = Some(sgr_intermediates(
            &source,
            width,
            height,
            2,
            parameter.scale[0],
            depth,
        )?);
    }
    let mut radius_one = None;
    if parameter.radius[1] != 0 {
        radius_one = Some(sgr_intermediates(
            &source,
            width,
            height,
            1,
            parameter.scale[1],
            depth,
        )?);
    }

    let weight_two = i64::from(weights[0]);
    let weight_one = 128_i64
        .checked_sub(i64::from(weights[0]))
        .and_then(|value| value.checked_sub(i64::from(weights[1])))
        .ok_or_else(|| malformed("SGR projection weight overflows"))?;
    let mut restored = Vec::<u16>::new();
    restored
        .try_reserve(sample_count)
        .map_err(|_| malformed("unable to allocate SGR output"))?;
    restored.resize(sample_count, 0);
    let maximum = i64::from(depth.maximum());
    let extended_width = width
        .checked_add(2)
        .ok_or_else(|| malformed("SGR intermediate width overflows"))?;
    for y in 0..height {
        let row = y
            .checked_mul(width)
            .ok_or_else(|| malformed("SGR output row overflows"))?;
        for x in 0..width {
            let source_index = row
                .checked_add(x)
                .ok_or_else(|| malformed("SGR source index overflows"))?;
            let source_sample = i64::from(
                *source
                    .get(source_index)
                    .ok_or_else(|| malformed("SGR source sample is missing"))?,
            );
            let mut correction = 0_i64;
            if let Some(intermediates) = radius_two.as_deref() {
                correction = correction
                    .checked_add(
                        weight_two
                            .checked_mul(sgr_radius_two_residual(
                                intermediates,
                                extended_width,
                                width,
                                height,
                                x,
                                y,
                                source_sample,
                            )?)
                            .ok_or_else(|| malformed("SGR radius-two product overflows"))?,
                    )
                    .ok_or_else(|| malformed("SGR correction overflows"))?;
            }
            if let Some(intermediates) = radius_one.as_deref() {
                correction = correction
                    .checked_add(
                        weight_one
                            .checked_mul(sgr_radius_one_residual(
                                intermediates,
                                extended_width,
                                width,
                                height,
                                x,
                                y,
                                source_sample,
                            )?)
                            .ok_or_else(|| malformed("SGR radius-one product overflows"))?,
                    )
                    .ok_or_else(|| malformed("SGR correction overflows"))?;
            }
            let corrected = correction
                .checked_add(1024)
                .ok_or_else(|| malformed("SGR projection rounding overflows"))?
                >> 11;
            let output = source_sample
                .checked_add(corrected)
                .ok_or_else(|| malformed("SGR output overflows"))?
                .clamp(0, maximum);
            let output =
                u16::try_from(output).map_err(|_| malformed("SGR output exceeds sample depth"))?;
            *restored
                .get_mut(source_index)
                .ok_or_else(|| malformed("SGR output sample is missing"))? = output;
        }
    }
    plane.samples = restored;
    Ok(())
}

fn sgr_intermediates(
    source: &[u16],
    width: usize,
    height: usize,
    radius: u8,
    scale: u32,
    depth: SampleDepth,
) -> Av1Result<Vec<SgrIntermediate>> {
    let radius = usize::from(radius);
    if !matches!(radius, 1 | 2) {
        return Err(malformed("SGR radius is unsupported"));
    }
    let extended_width = width
        .checked_add(2)
        .ok_or_else(|| malformed("SGR intermediate width overflows"))?;
    let extended_height = height
        .checked_add(2)
        .ok_or_else(|| malformed("SGR intermediate height overflows"))?;
    let extended_count = extended_width
        .checked_mul(extended_height)
        .ok_or_else(|| malformed("SGR intermediate sample count overflows"))?;
    let mut output = Vec::<SgrIntermediate>::new();
    output
        .try_reserve(extended_count)
        .map_err(|_| malformed("unable to allocate SGR intermediates"))?;
    output.resize(extended_count, SgrIntermediate { mean: 0, gain: 0 });
    let radius_signed =
        isize::try_from(radius).map_err(|_| malformed("SGR radius exceeds isize"))?;
    let diameter = radius
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| malformed("SGR neighborhood diameter overflows"))?;
    let neighborhood = diameter
        .checked_mul(diameter)
        .ok_or_else(|| malformed("SGR neighborhood size overflows"))?;
    let n =
        u64::try_from(neighborhood).map_err(|_| malformed("SGR neighborhood size exceeds u64"))?;
    let reciprocal = match n {
        9 => 455_u64,
        25 => 164_u64,
        _ => return Err(malformed("SGR neighborhood size is invalid")),
    };
    let depth_shift = depth.bits().saturating_sub(8);
    let square_shift = depth_shift.saturating_mul(2);
    for extended_y in 0..extended_height {
        let center_y = isize::try_from(extended_y)
            .map_err(|_| malformed("SGR intermediate y exceeds isize"))?
            .checked_sub(1)
            .ok_or_else(|| malformed("SGR intermediate y underflows"))?;
        for extended_x in 0..extended_width {
            let center_x = isize::try_from(extended_x)
                .map_err(|_| malformed("SGR intermediate x exceeds isize"))?
                .checked_sub(1)
                .ok_or_else(|| malformed("SGR intermediate x underflows"))?;
            let mut sum = 0_u64;
            let mut sumsq = 0_u64;
            let mut dy = -radius_signed;
            while dy <= radius_signed {
                let mut dx = -radius_signed;
                while dx <= radius_signed {
                    let sample_x = center_x
                        .checked_add(dx)
                        .ok_or_else(|| malformed("SGR sample x overflows"))?;
                    let sample_y = center_y
                        .checked_add(dy)
                        .ok_or_else(|| malformed("SGR sample y overflows"))?;
                    let sample_x = clamp_signed(sample_x, width);
                    let sample_y = clamp_signed(sample_y, height);
                    let source_index = sample_y
                        .checked_mul(width)
                        .and_then(|row| row.checked_add(sample_x))
                        .ok_or_else(|| malformed("SGR sample index overflows"))?;
                    let sample = u64::from(
                        *source
                            .get(source_index)
                            .ok_or_else(|| malformed("SGR source sample is missing"))?,
                    );
                    sum = sum
                        .checked_add(sample)
                        .ok_or_else(|| malformed("SGR sum overflows"))?;
                    let square = sample
                        .checked_mul(sample)
                        .ok_or_else(|| malformed("SGR square overflows"))?;
                    sumsq = sumsq
                        .checked_add(square)
                        .ok_or_else(|| malformed("SGR sumsq overflows"))?;
                    dx = dx
                        .checked_add(1)
                        .ok_or_else(|| malformed("SGR x loop overflows"))?;
                }
                dy = dy
                    .checked_add(1)
                    .ok_or_else(|| malformed("SGR y loop overflows"))?;
            }
            let normalized_sumsq = round_unsigned(sumsq, square_shift)?;
            let normalized_sum = round_unsigned(sum, depth_shift)?;
            let variance_left = normalized_sumsq
                .checked_mul(n)
                .ok_or_else(|| malformed("SGR variance product overflows"))?;
            let variance_right = normalized_sum
                .checked_mul(normalized_sum)
                .ok_or_else(|| malformed("SGR variance square overflows"))?;
            let variance = variance_left.saturating_sub(variance_right);
            let scaled = variance
                .checked_mul(u64::from(scale))
                .ok_or_else(|| malformed("SGR scaled variance overflows"))?;
            let z = (scaled
                .checked_add(1_u64 << 19)
                .ok_or_else(|| malformed("SGR variance rounding overflows"))?)
                >> 20;
            let lookup = usize::try_from(z.min(255))
                .map_err(|_| malformed("SGR lookup index exceeds usize"))?;
            let gain = u64::from(SGR_X_BY_X[lookup]);
            let mean = gain
                .checked_mul(normalized_sum)
                .and_then(|value| value.checked_mul(reciprocal))
                .and_then(|value| value.checked_add(1_u64 << 11))
                .ok_or_else(|| malformed("SGR mean product overflows"))?
                >> 12;
            let index = extended_y
                .checked_mul(extended_width)
                .and_then(|row| row.checked_add(extended_x))
                .ok_or_else(|| malformed("SGR intermediate index overflows"))?;
            *output
                .get_mut(index)
                .ok_or_else(|| malformed("SGR intermediate is missing"))? = SgrIntermediate {
                mean: i64::try_from(mean).map_err(|_| malformed("SGR mean exceeds i64"))?,
                gain: i64::try_from(gain).map_err(|_| malformed("SGR gain exceeds i64"))?,
            };
        }
    }
    Ok(output)
}

fn sgr_radius_one_residual(
    intermediates: &[SgrIntermediate],
    extended_width: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    source: i64,
) -> Av1Result<i64> {
    let mut gain_cross = 0_i64;
    let mut mean_cross = 0_i64;
    for (dx, dy) in [(0_i32, 0_i32), (-1, 0), (1, 0), (0, -1), (0, 1)] {
        let value = sgr_ab_at(intermediates, extended_width, width, height, x, y, dx, dy)?;
        gain_cross = gain_cross
            .checked_add(value.gain)
            .ok_or_else(|| malformed("SGR cross gain overflows"))?;
        mean_cross = mean_cross
            .checked_add(value.mean)
            .ok_or_else(|| malformed("SGR cross mean overflows"))?;
    }
    let mut gain_corners = 0_i64;
    let mut mean_corners = 0_i64;
    for (dx, dy) in [(-1_i32, -1_i32), (1, -1), (-1, 1), (1, 1)] {
        let value = sgr_ab_at(intermediates, extended_width, width, height, x, y, dx, dy)?;
        gain_corners = gain_corners
            .checked_add(value.gain)
            .ok_or_else(|| malformed("SGR corner gain overflows"))?;
        mean_corners = mean_corners
            .checked_add(value.mean)
            .ok_or_else(|| malformed("SGR corner mean overflows"))?;
    }
    let gain = gain_cross
        .checked_mul(4)
        .and_then(|value| value.checked_add(gain_corners.checked_mul(3)?))
        .ok_or_else(|| malformed("SGR weighted gain overflows"))?;
    let mean = mean_cross
        .checked_mul(4)
        .and_then(|value| value.checked_add(mean_corners.checked_mul(3)?))
        .ok_or_else(|| malformed("SGR weighted mean overflows"))?;
    mean.checked_sub(
        gain.checked_mul(source)
            .ok_or_else(|| malformed("SGR residual product overflows"))?,
    )
    .and_then(|value| value.checked_add(256))
    .map(|value| value >> 9)
    .ok_or_else(|| malformed("SGR residual overflows"))
}

fn sgr_radius_two_residual(
    intermediates: &[SgrIntermediate],
    extended_width: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    source: i64,
) -> Av1Result<i64> {
    let (gain, mean) = if y.is_multiple_of(2) {
        let upper = sgr_radius_two_row(intermediates, extended_width, width, height, x, y, -1)?;
        let lower = sgr_radius_two_row(intermediates, extended_width, width, height, x, y, 1)?;
        (
            upper
                .0
                .checked_add(lower.0)
                .ok_or_else(|| malformed("SGR radius-two gain overflows"))?,
            upper
                .1
                .checked_add(lower.1)
                .ok_or_else(|| malformed("SGR radius-two mean overflows"))?,
        )
    } else {
        sgr_radius_two_row(intermediates, extended_width, width, height, x, y, 0)?
    };
    mean.checked_sub(
        gain.checked_mul(source)
            .ok_or_else(|| malformed("SGR radius-two product overflows"))?,
    )
    .and_then(|value| value.checked_add(if y.is_multiple_of(2) { 256 } else { 128 }))
    .map(|value| value >> if y.is_multiple_of(2) { 9 } else { 8 })
    .ok_or_else(|| malformed("SGR radius-two residual overflows"))
}

fn sgr_radius_two_row(
    intermediates: &[SgrIntermediate],
    extended_width: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    dy: i32,
) -> Av1Result<(i64, i64)> {
    let center = sgr_ab_at(intermediates, extended_width, width, height, x, y, 0, dy)?;
    let left = sgr_ab_at(intermediates, extended_width, width, height, x, y, -1, dy)?;
    let right = sgr_ab_at(intermediates, extended_width, width, height, x, y, 1, dy)?;
    let gain = center
        .gain
        .checked_mul(6)
        .and_then(|value| {
            left.gain
                .checked_add(right.gain)
                .and_then(|sides| sides.checked_mul(5))
                .and_then(|sides| value.checked_add(sides))
        })
        .ok_or_else(|| malformed("SGR radius-two row gain overflows"))?;
    let mean = center
        .mean
        .checked_mul(6)
        .and_then(|value| {
            left.mean
                .checked_add(right.mean)
                .and_then(|sides| sides.checked_mul(5))
                .and_then(|sides| value.checked_add(sides))
        })
        .ok_or_else(|| malformed("SGR radius-two row mean overflows"))?;
    Ok((gain, mean))
}

fn sgr_ab_at(
    intermediates: &[SgrIntermediate],
    extended_width: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    dx: i32,
    dy: i32,
) -> Av1Result<SgrIntermediate> {
    let x = isize::try_from(x)
        .map_err(|_| malformed("SGR x exceeds isize"))?
        .checked_add(isize::try_from(dx).map_err(|_| malformed("SGR x delta exceeds isize"))?)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| malformed("SGR x coordinate overflows"))?;
    let y = isize::try_from(y)
        .map_err(|_| malformed("SGR y exceeds isize"))?
        .checked_add(isize::try_from(dy).map_err(|_| malformed("SGR y delta exceeds isize"))?)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| malformed("SGR y coordinate overflows"))?;
    let x = clamp_signed(
        x,
        width
            .checked_add(2)
            .ok_or_else(|| malformed("SGR extended width overflows"))?,
    );
    let y = clamp_signed(
        y,
        height
            .checked_add(2)
            .ok_or_else(|| malformed("SGR extended height overflows"))?,
    );
    let index = y
        .checked_mul(extended_width)
        .and_then(|row| row.checked_add(x))
        .ok_or_else(|| malformed("SGR intermediate lookup overflows"))?;
    intermediates
        .get(index)
        .copied()
        .ok_or_else(|| malformed("SGR intermediate lookup is missing"))
}

fn round_unsigned(value: u64, shift: u32) -> Av1Result<u64> {
    if shift == 0 {
        return Ok(value);
    }
    let bias = 1_u64
        .checked_shl(shift.saturating_sub(1))
        .ok_or_else(|| malformed("SGR rounding shift overflows"))?;
    value
        .checked_add(bias)
        .map(|value| value >> shift)
        .ok_or_else(|| malformed("SGR rounded value overflows"))
}

fn clamp_signed(value: isize, extent: usize) -> usize {
    if value <= 0 {
        0
    } else {
        usize::try_from(value).map_or(extent.saturating_sub(1), |value| {
            value.min(extent.saturating_sub(1))
        })
    }
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
