//! Safe AV1 motion-vector, reference, scaling, and temporal-field state.
//!
//! Entropy decoding deliberately lives in `entropy`; this module contains
//! only typed values and bounded arithmetic shared by reference-MV discovery,
//! motion compensation, retained surfaces, and frame-state publication.
//!
//! Reference-MV projection follows an altered safe-Rust translation of the
//! pinned dav1d AV1 implementation. Its BSD-2-Clause and patent provenance is
//! recorded in `NOTICE.md`, `PATENTS`, and `third_party/`.

#![allow(
    dead_code,
    reason = "motion foundations are wired incrementally by the complete inter decoder"
)]

use super::geometry::BlockSize;
use super::{Av1Result, malformed};
use crate::codecs::CodecError;

/// One AV1 motion vector in signed one-eighth luma-pixel units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct MotionVector {
    pub(super) y: i16,
    pub(super) x: i16,
}

impl MotionVector {
    pub(super) const ZERO: Self = Self { y: 0, x: 0 };

    /// Apply the sign-correct precision reduction used by scalar dav1d.
    pub(super) const fn reduce_precision(self, force_integer: bool, high_precision: bool) -> Self {
        const fn component(value: i16, force_integer: bool, high_precision: bool) -> i16 {
            let value = value as i32;
            let corrected = if force_integer {
                (value - (value >> 15) + 3) & !7
            } else if !high_precision {
                (value - (value >> 15)) & !1
            } else {
                value
            };
            corrected as i16
        }
        Self {
            y: component(self.y, force_integer, high_precision),
            x: component(self.x, force_integer, high_precision),
        }
    }

    pub(super) fn checked_add(self, residual: Self) -> Av1Result<Self> {
        let y = i32::from(self.y)
            .checked_add(i32::from(residual.y))
            .ok_or_else(|| malformed("motion-vector vertical component overflows"))?;
        let x = i32::from(self.x)
            .checked_add(i32::from(residual.x))
            .ok_or_else(|| malformed("motion-vector horizontal component overflows"))?;
        Ok(Self {
            y: i16::try_from(y)
                .map_err(|_| malformed("motion-vector vertical component exceeds i16"))?,
            x: i16::try_from(x)
                .map_err(|_| malformed("motion-vector horizontal component exceeds i16"))?,
        })
    }

    pub(super) const fn manhattan_distance(self, other: Self) -> u32 {
        u32::from(self.y.abs_diff(other.y)).saturating_add(u32::from(self.x.abs_diff(other.x)))
    }

    pub(super) const fn projectable(self) -> bool {
        self.y > -4096 && self.y < 4096 && self.x > -4096 && self.x < 4096
    }

    /// Clamp a candidate to AV1's frame-relative reference-MV bounds.
    pub(super) fn clamp_to_block(
        self,
        block_x: u32,
        block_y: u32,
        block_size: BlockSize,
        frame_width_b4: u32,
        frame_height_b4: u32,
    ) -> Av1Result<Self> {
        let (width, height) = block_size.mi_dimensions();
        let block_x = i64::from(block_x);
        let block_y = i64::from(block_y);
        let width = i64::from(width);
        let height = i64::from(height);
        let frame_width = i64::from(frame_width_b4);
        let frame_height = i64::from(frame_height_b4);
        if width == 0
            || height == 0
            || block_x < 0
            || block_y < 0
            || block_x >= frame_width
            || block_y >= frame_height
        {
            return Err(malformed("motion-vector block geometry is invalid"));
        }
        // The ref-MV clamp includes four extra 4x4 cells on each side. The
        // equations below convert that sixteen-pixel border to Q3 units.
        let minimum_x = -((block_x + width + 4) * 32);
        let maximum_x = (frame_width - block_x + 4) * 32;
        let minimum_y = -((block_y + height + 4) * 32);
        let maximum_y = (frame_height - block_y + 4) * 32;
        let x = i64::from(self.x).clamp(minimum_x, maximum_x);
        let y = i64::from(self.y).clamp(minimum_y, maximum_y);
        Ok(Self {
            y: i16::try_from(y).map_err(|_| malformed("clamped vertical MV exceeds i16"))?,
            x: i16::try_from(x).map_err(|_| malformed("clamped horizontal MV exceeds i16"))?,
        })
    }
}

/// AV1's seven logical inter references in bitstream order.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ReferenceFrame {
    Last = 0,
    Last2 = 1,
    Last3 = 2,
    Golden = 3,
    Backward = 4,
    Alt2 = 5,
    Alt = 6,
}

impl ReferenceFrame {
    pub(super) const ALL: [Self; 7] = [
        Self::Last,
        Self::Last2,
        Self::Last3,
        Self::Golden,
        Self::Backward,
        Self::Alt2,
        Self::Alt,
    ];

    pub(super) const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Last),
            1 => Some(Self::Last2),
            2 => Some(Self::Last3),
            3 => Some(Self::Golden),
            4 => Some(Self::Backward),
            5 => Some(Self::Alt2),
            6 => Some(Self::Alt),
            _ => None,
        }
    }

    /// A segmentation reference feature stores zero for intra and 1..=7 for
    /// the seven logical inter references.
    pub(super) const fn from_segment_feature(value: i32) -> Option<Self> {
        if value <= 0 {
            return None;
        }
        Self::from_index(value.saturating_sub(1) as usize)
    }

    pub(super) const fn index(self) -> usize {
        self as usize
    }

    pub(super) const fn is_forward(self) -> bool {
        matches!(self, Self::Last | Self::Last2 | Self::Last3 | Self::Golden)
    }
}

/// One single- or compound-reference selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ReferencePair {
    pub(super) first: ReferenceFrame,
    pub(super) second: Option<ReferenceFrame>,
}

impl ReferencePair {
    pub(super) const fn single(first: ReferenceFrame) -> Self {
        Self {
            first,
            second: None,
        }
    }

    pub(super) const fn compound(first: ReferenceFrame, second: ReferenceFrame) -> Self {
        Self {
            first,
            second: Some(second),
        }
    }

    pub(super) const fn is_compound(self) -> bool {
        self.second.is_some()
    }

    pub(super) const fn as_array(self) -> [Option<ReferenceFrame>; 2] {
        [Some(self.first), self.second]
    }
}

/// AV1 inter prediction mode after the single/compound mode trees resolve.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum InterMode {
    #[default]
    Nearest,
    Near,
    Global,
    New,
    NearestNearest,
    NearNear,
    NearestNew,
    NewNearest,
    NearNew,
    NewNear,
    GlobalGlobal,
    NewNew,
}

impl InterMode {
    pub(super) const fn uses_new_mv(self, index: usize) -> bool {
        match (self, index) {
            (Self::New | Self::NewNew | Self::NewNearest | Self::NewNear, 0) => true,
            (Self::NewNew | Self::NearestNew | Self::NearNew, 1) => true,
            _ => false,
        }
    }

    pub(super) const fn uses_global_mv(self, index: usize) -> bool {
        matches!(
            (self, index),
            (Self::Global, 0) | (Self::GlobalGlobal, 0 | 1)
        )
    }

    pub(super) const fn is_near(self) -> bool {
        matches!(
            self,
            Self::Near | Self::NearNear | Self::NearNew | Self::NewNear
        )
    }
}

/// The prediction-domain compound operation for one inter block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum CompoundType {
    #[default]
    Average,
    Distance,
    Difference {
        inverted: bool,
    },
    Wedge {
        index: u8,
        inverted: bool,
    },
}

/// Motion model selected for one single-reference block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum MotionMode {
    #[default]
    Translation,
    Obmc,
    LocalWarp,
}

/// Per-axis interpolation filter selected for translation prediction.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum InterpolationFilter {
    #[default]
    Regular = 0,
    Smooth = 1,
    Sharp = 2,
    Bilinear = 3,
}

impl InterpolationFilter {
    pub(super) const fn from_symbol(symbol: u32) -> Option<Self> {
        match symbol {
            0 => Some(Self::Regular),
            1 => Some(Self::Smooth),
            2 => Some(Self::Sharp),
            3 => Some(Self::Bilinear),
            _ => None,
        }
    }
}

/// Parsed AV1 global-motion model in the frame header's fixed-point domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GlobalMotionType {
    Identity,
    Translation,
    RotZoom,
    Affine,
}

/// Six-parameter AV1 global-motion matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GlobalMotion {
    pub(super) kind: GlobalMotionType,
    pub(super) matrix: [i32; 6],
}

impl GlobalMotion {
    pub(super) const fn identity() -> Self {
        Self {
            kind: GlobalMotionType::Identity,
            matrix: [0, 0, 1 << 16, 0, 0, 1 << 16],
        }
    }

    pub(super) const fn is_identity(self) -> bool {
        matches!(self.kind, GlobalMotionType::Identity)
    }

    pub(super) const fn is_translation(self) -> bool {
        matches!(
            self.kind,
            GlobalMotionType::Identity | GlobalMotionType::Translation
        )
    }
}

/// Fixed-point reference scaling for one retained picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ScaleFactors {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) step_x: i32,
    pub(super) step_y: i32,
    pub(super) scaled: bool,
}

impl ScaleFactors {
    const UNITY: i32 = 1 << 14;

    pub(super) fn new(
        current_coded_width: u32,
        current_height: u32,
        reference_upscaled_width: u32,
        reference_height: u32,
    ) -> Av1Result<Self> {
        if current_coded_width == 0
            || current_height == 0
            || reference_upscaled_width == 0
            || reference_height == 0
            || current_coded_width
                .checked_mul(2)
                .is_none_or(|value| value < reference_upscaled_width)
            || current_height
                .checked_mul(2)
                .is_none_or(|value| value < reference_height)
            || reference_upscaled_width
                .checked_mul(16)
                .is_none_or(|value| current_coded_width > value)
            || reference_height
                .checked_mul(16)
                .is_none_or(|value| current_height > value)
        {
            return Err(malformed("AV1 reference scaling ratio is invalid"));
        }
        let scale = |reference: u32, current: u32| -> Av1Result<i32> {
            let numerator = i64::from(reference)
                .checked_shl(14)
                .and_then(|value| value.checked_add(i64::from(current / 2)))
                .ok_or_else(|| malformed("reference scale numerator overflows"))?;
            let scale = numerator.div_euclid(i64::from(current));
            i32::try_from(scale).map_err(|_| malformed("reference scale exceeds i32"))
        };
        let x = scale(reference_upscaled_width, current_coded_width)?;
        let y = scale(reference_height, current_height)?;
        Ok(Self {
            x,
            y,
            step_x: x
                .checked_add(8)
                .ok_or_else(|| malformed("horizontal scale step overflows"))?
                >> 4,
            step_y: y
                .checked_add(8)
                .ok_or_else(|| malformed("vertical scale step overflows"))?
                >> 4,
            // Path selection is dimensional, not based on the rounded Q14
            // factor: distinct large dimensions can round to the same value.
            scaled: reference_upscaled_width != current_coded_width
                || reference_height != current_height,
        })
    }

    /// Map one coded-plane coordinate plus a Q3 luma MV into the scaled MC
    /// domain (ten fractional bits), preserving negative-coordinate rounding.
    pub(super) fn position(
        scale: i32,
        block_origin: i32,
        motion: i16,
        subsampled: bool,
    ) -> Av1Result<i32> {
        let motion_scale = 1_i64 << u32::from(!subsampled);
        let origin = i64::from(block_origin)
            .checked_mul(16)
            .and_then(|value| value.checked_add(i64::from(motion).checked_mul(motion_scale)?))
            .ok_or_else(|| malformed("scaled MC origin overflows"))?;
        let scale = i64::from(scale);
        let temporary = origin
            .checked_mul(scale)
            .and_then(|value| value.checked_add((scale - 0x4000).checked_mul(8)?))
            .ok_or_else(|| malformed("scaled MC position overflows"))?;
        let rounded = temporary
            .unsigned_abs()
            .checked_add(128)
            .ok_or_else(|| malformed("scaled MC rounding overflows"))?
            >> 8;
        let signed = if temporary < 0 {
            -(i64::try_from(rounded).map_err(|_| malformed("scaled MC coordinate exceeds i64"))?)
        } else {
            i64::try_from(rounded).map_err(|_| malformed("scaled MC coordinate exceeds i64"))?
        };
        i32::try_from(
            signed
                .checked_add(32)
                .ok_or_else(|| malformed("scaled MC coordinate overflows"))?,
        )
        .map_err(|_| malformed("scaled MC coordinate exceeds i32"))
    }
}

/// One bounded spatial reference-MV candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MotionCandidate {
    pub(super) vectors: [MotionVector; 2],
    pub(super) weight: i32,
}

impl Default for MotionCandidate {
    fn default() -> Self {
        Self {
            vectors: [MotionVector::ZERO; 2],
            weight: 0,
        }
    }
}

/// Fixed-capacity AV1 reference-MV stack.
#[derive(Clone, Copy, Debug)]
pub(super) struct MotionCandidateStack {
    candidates: [MotionCandidate; 8],
    len: u8,
    pub(super) context: u8,
}

impl MotionCandidateStack {
    pub(super) const fn new() -> Self {
        Self {
            candidates: [MotionCandidate {
                vectors: [MotionVector::ZERO; 2],
                weight: 0,
            }; 8],
            len: 0,
            context: 0,
        }
    }

    pub(super) const fn len(self) -> usize {
        self.len as usize
    }

    pub(super) fn as_slice(&self) -> &[MotionCandidate] {
        &self.candidates[..self.len()]
    }

    pub(super) fn get(&self, index: usize) -> Option<MotionCandidate> {
        self.as_slice().get(index).copied()
    }

    pub(super) fn add_or_weight(&mut self, vectors: [MotionVector; 2], weight: i32) {
        if let Some(candidate) = self
            .candidates
            .iter_mut()
            .take(usize::from(self.len))
            .find(|candidate| candidate.vectors == vectors)
        {
            candidate.weight = candidate.weight.saturating_add(weight);
            return;
        }
        let index = usize::from(self.len);
        if let Some(candidate) = self.candidates.get_mut(index) {
            *candidate = MotionCandidate { vectors, weight };
            self.len = self.len.saturating_add(1);
        }
    }

    pub(super) fn sort_range_by_weight(&mut self, start: usize) {
        let end = self.len();
        if start >= end {
            return;
        }
        self.candidates[start..end].sort_by(|left, right| right.weight.cmp(&left.weight));
    }

    pub(super) fn ensure_two(&mut self, fallbacks: [[MotionVector; 2]; 2]) {
        while self.len() < 2 {
            let index = self.len();
            // Global fallbacks fill stack positions directly. Equal vectors
            // in slots zero and one are intentional and must not coalesce.
            self.candidates[index] = MotionCandidate {
                vectors: fallbacks[index],
                weight: 0,
            };
            self.len = self.len.saturating_add(1);
        }
    }
}

/// One projectable temporal-MV entry retained for an 8×8 coded region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TemporalMotion {
    pub(super) vector: MotionVector,
    pub(super) reference: ReferenceFrame,
}

/// Motion state inseparable from one retained decoded frame surface.
#[derive(Clone)]
pub(super) struct TemporalMotionField {
    coded_width: u32,
    frame_height: u32,
    stride: usize,
    cells: Vec<Option<TemporalMotion>>,
    order_hint: u32,
    order_hint_bits: u32,
    reference_order_hints: [u32; 7],
}

impl TemporalMotionField {
    pub(super) fn new(
        coded_width: u32,
        frame_height: u32,
        order_hint: u32,
        order_hint_bits: u32,
        reference_order_hints: [u32; 7],
    ) -> Av1Result<Self> {
        if coded_width == 0 || frame_height == 0 || order_hint_bits > 8 {
            return Err(malformed("temporal motion field has invalid geometry"));
        }
        let width = coded_width.div_ceil(8);
        let height = frame_height.div_ceil(8);
        let stride = usize::try_from(width)
            .map_err(|_| malformed("temporal motion stride exceeds usize"))?;
        let height = usize::try_from(height)
            .map_err(|_| malformed("temporal motion height exceeds usize"))?;
        let length = stride
            .checked_mul(height)
            .ok_or_else(|| malformed("temporal motion allocation overflows"))?;
        let mut cells = Vec::new();
        cells.try_reserve_exact(length).map_err(|_| {
            CodecError::Dimensions("unable to allocate AV1 temporal motion field".to_owned())
        })?;
        cells.resize(length, None);
        Ok(Self {
            coded_width,
            frame_height,
            stride,
            cells,
            order_hint,
            order_hint_bits,
            reference_order_hints,
        })
    }

    pub(super) const fn coded_width(&self) -> u32 {
        self.coded_width
    }

    pub(super) const fn frame_height(&self) -> u32 {
        self.frame_height
    }

    pub(super) const fn order_hint(&self) -> u32 {
        self.order_hint
    }

    pub(super) const fn reference_order_hints(&self) -> [u32; 7] {
        self.reference_order_hints
    }

    fn index(&self, x8: u32, y8: u32) -> Option<usize> {
        if x8 >= self.coded_width.div_ceil(8) || y8 >= self.frame_height.div_ceil(8) {
            return None;
        }
        usize::try_from(y8)
            .ok()?
            .checked_mul(self.stride)?
            .checked_add(usize::try_from(x8).ok()?)
    }

    pub(super) fn get(&self, x8: u32, y8: u32) -> Option<TemporalMotion> {
        self.cells.get(self.index(x8, y8)?)?.as_ref().copied()
    }

    /// Publish a selected block MV to every covered 8×8 cell. Callers stage
    /// this field privately until the complete frame succeeds.
    pub(super) fn fill_block(
        &mut self,
        x_b4: u32,
        y_b4: u32,
        width_b4: u32,
        height_b4: u32,
        value: Option<TemporalMotion>,
    ) -> Av1Result<()> {
        if width_b4 == 0 || height_b4 == 0 {
            return Err(malformed("temporal motion block has an empty extent"));
        }
        let start_x = x_b4 / 2;
        let start_y = y_b4 / 2;
        let end_x = x_b4
            .checked_add(width_b4)
            .ok_or_else(|| malformed("temporal motion block x extent overflows"))?
            .div_ceil(2)
            .min(self.coded_width.div_ceil(8));
        let end_y = y_b4
            .checked_add(height_b4)
            .ok_or_else(|| malformed("temporal motion block y extent overflows"))?
            .div_ceil(2)
            .min(self.frame_height.div_ceil(8));
        if start_x >= end_x || start_y >= end_y {
            return Err(malformed("temporal motion block exceeds the coded field"));
        }
        for y in start_y..end_y {
            for x in start_x..end_x {
                let index = self
                    .index(x, y)
                    .ok_or_else(|| malformed("temporal motion write exceeds allocation"))?;
                self.cells[index] = value;
            }
        }
        Ok(())
    }

    /// Project one retained temporal entry into the current frame. AV1's
    /// projection divisor is bounded by order-hint distance and produces a
    /// clamped signed Q3 MV.
    pub(super) fn project(
        &self,
        motion: TemporalMotion,
        current_order_hint: u32,
        target_reference_hint: u32,
    ) -> Option<MotionVector> {
        if self.order_hint_bits == 0 {
            return None;
        }
        let source_hint = self.reference_order_hints[motion.reference.index()];
        let source_distance = relative_distance(self.order_hint_bits, self.order_hint, source_hint);
        let target_distance = relative_distance(
            self.order_hint_bits,
            current_order_hint,
            target_reference_hint,
        );
        project_vector(motion.vector, source_distance, target_distance)
    }
}

/// Signed AV1 order-hint distance with modular wraparound.
pub(super) fn relative_distance(bits: u32, first: u32, second: u32) -> i32 {
    if bits == 0 {
        return 0;
    }
    let sign = 1_i64 << bits.saturating_sub(1);
    let difference = i64::from(first).saturating_sub(i64::from(second));
    let distance = (difference & sign.saturating_sub(1)).saturating_sub(difference & sign);
    match i32::try_from(distance) {
        Ok(distance) => distance,
        Err(_) => 0,
    }
}

/// AV1 temporal MV projection using the normative reciprocal approximation.
pub(super) fn project_vector(
    vector: MotionVector,
    denominator_distance: i32,
    numerator_distance: i32,
) -> Option<MotionVector> {
    const DIV_MULT: [i32; 32] = [
        0, 16_384, 8_192, 5_461, 4_096, 3_276, 2_730, 2_340, 2_048, 1_820, 1_638, 1_489, 1_365,
        1_260, 1_170, 1_092, 1_024, 963, 910, 862, 819, 780, 744, 712, 682, 655, 630, 606, 585,
        564, 546, 528,
    ];

    if !(1..=31).contains(&denominator_distance)
        || !(-31..=31).contains(&numerator_distance)
        || !vector.projectable()
    {
        return None;
    }
    let denominator = usize::try_from(denominator_distance).ok()?;
    let reciprocal = *DIV_MULT.get(denominator)?;
    let fraction = numerator_distance.checked_mul(reciprocal)?;
    let project = |component: i16| -> Option<i16> {
        let product = i32::from(component).checked_mul(fraction)?;
        let rounded = product.checked_add(8_192)?.checked_add(product >> 31)? >> 14;
        i16::try_from(rounded.clamp(-16_383, 16_383)).ok()
    };
    Some(MotionVector {
        y: project(vector.y)?,
        x: project(vector.x)?,
    })
}
