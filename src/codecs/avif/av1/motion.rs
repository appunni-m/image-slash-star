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

    pub(super) fn manhattan_distance(self, other: Self) -> u32 {
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

/// Prepared affine parameters used by AV1's 8x8 warped-motion predictor.
///
/// `matrix` remains in the frame-header Q16 luma-coordinate domain.  `abcd`
/// is the four Q6 shear coefficients consumed by the separable 193-phase
/// warped filter.  Keeping both forms avoids re-deriving the source origin for
/// each plane tile while retaining the exact header matrix for coordinate
/// projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PreparedGlobalWarp {
    pub(super) matrix: [i32; 6],
    pub(super) abcd: [i16; 4],
}

// Pinned dav1d 1.5.3 `src/warpmv.c`/libaom 3.13.2 divisor reciprocal table.
// The table is data-only and covered by the existing BSD-2-Clause and patent
// notices retained in NOTICE.md, PATENTS, and third_party/.
const GLOBAL_MOTION_DIV_LUT: [u16; 257] = [
    16384, 16320, 16257, 16194, 16132, 16070, 16009, 15948, 15888, 15828, 15768, 15709, 15650,
    15592, 15534, 15477, 15420, 15364, 15308, 15252, 15197, 15142, 15087, 15033, 14980, 14926,
    14873, 14821, 14769, 14717, 14665, 14614, 14564, 14513, 14463, 14413, 14364, 14315, 14266,
    14218, 14170, 14122, 14075, 14028, 13981, 13935, 13888, 13843, 13797, 13752, 13707, 13662,
    13618, 13574, 13530, 13487, 13443, 13400, 13358, 13315, 13273, 13231, 13190, 13148, 13107,
    13066, 13026, 12985, 12945, 12906, 12866, 12827, 12788, 12749, 12710, 12672, 12633, 12596,
    12558, 12520, 12483, 12446, 12409, 12373, 12336, 12300, 12264, 12228, 12193, 12157, 12122,
    12087, 12053, 12018, 11984, 11950, 11916, 11882, 11848, 11815, 11782, 11749, 11716, 11683,
    11651, 11619, 11586, 11555, 11523, 11491, 11460, 11429, 11398, 11367, 11336, 11305, 11275,
    11245, 11215, 11185, 11155, 11125, 11096, 11067, 11038, 11009, 10980, 10951, 10923, 10894,
    10866, 10838, 10810, 10782, 10755, 10727, 10700, 10673, 10645, 10618, 10592, 10565, 10538,
    10512, 10486, 10460, 10434, 10408, 10382, 10356, 10331, 10305, 10280, 10255, 10230, 10205,
    10180, 10156, 10131, 10107, 10082, 10058, 10034, 10010, 9986, 9963, 9939, 9916, 9892, 9869,
    9846, 9823, 9800, 9777, 9754, 9732, 9709, 9687, 9664, 9642, 9620, 9598, 9576, 9554, 9533, 9511,
    9489, 9468, 9447, 9425, 9404, 9383, 9362, 9341, 9321, 9300, 9279, 9259, 9239, 9218, 9198, 9178,
    9158, 9138, 9118, 9098, 9079, 9059, 9039, 9020, 9001, 8981, 8962, 8943, 8924, 8905, 8886, 8867,
    8849, 8830, 8812, 8793, 8775, 8756, 8738, 8720, 8702, 8684, 8666, 8648, 8630, 8613, 8595, 8577,
    8560, 8542, 8525, 8508, 8490, 8473, 8456, 8439, 8422, 8405, 8389, 8372, 8355, 8339, 8322, 8306,
    8289, 8273, 8257, 8240, 8224, 8208, 8192,
];

fn global_motion_clip_wmp(value: i64) -> Av1Result<i32> {
    let value = value.clamp(i64::from(i16::MIN), i64::from(i16::MAX));
    let magnitude = value
        .unsigned_abs()
        .checked_add(32)
        .ok_or_else(|| malformed("global-motion shear rounding overflows"))?
        >> 6;
    let magnitude = i32::try_from(magnitude)
        .map_err(|_| malformed("global-motion shear magnitude exceeds i32"))?
        .checked_mul(64)
        .ok_or_else(|| malformed("global-motion shear scale overflows"))?;
    Ok(if value < 0 { -magnitude } else { magnitude })
}

fn global_motion_divisor(value: i32) -> Av1Result<(u32, i32)> {
    let value = value.unsigned_abs();
    if value == 0 {
        return Err(malformed("global-motion divisor is zero"));
    }
    let shift = 31_u32.saturating_sub(value.leading_zeros());
    let base = 1_u32
        .checked_shl(shift)
        .ok_or_else(|| malformed("global-motion divisor shift overflows"))?;
    let error = value
        .checked_sub(base)
        .ok_or_else(|| malformed("global-motion divisor normalization underflows"))?;
    let index = if shift > 8 {
        error
            .checked_add(
                1_u32
                    .checked_shl(shift - 9)
                    .ok_or_else(|| malformed("global-motion divisor rounding shift overflows"))?,
            )
            .ok_or_else(|| malformed("global-motion divisor index overflows"))?
            >> (shift - 8)
    } else {
        error
            .checked_shl(8 - shift)
            .ok_or_else(|| malformed("global-motion divisor index shift overflows"))?
    };
    let reciprocal = i32::from(
        *GLOBAL_MOTION_DIV_LUT
            .get(
                usize::try_from(index)
                    .map_err(|_| malformed("global-motion divisor index exceeds usize"))?,
            )
            .ok_or_else(|| malformed("global-motion divisor index exceeds LUT"))?,
    );
    let shift = shift
        .checked_add(14)
        .ok_or_else(|| malformed("global-motion divisor shift overflows"))?;
    Ok((shift, reciprocal))
}

/// Prepare a frame-header ROTZOOM/AFFINE matrix for warped sampling.
///
/// `None` is a normative ordinary-MC fallback for identity/translation or an
/// invalid shear (including a non-positive diagonal).  Arithmetic and matrix
/// representation failures remain hard errors so a malformed frame cannot
/// publish a partial predictor.
pub(super) fn prepare_global_warp(global: GlobalMotion) -> Av1Result<Option<PreparedGlobalWarp>> {
    if !matches!(
        global.kind,
        GlobalMotionType::RotZoom | GlobalMotionType::Affine
    ) {
        return Ok(None);
    }
    if global.kind == GlobalMotionType::RotZoom {
        let neg = global.matrix[3]
            .checked_neg()
            .ok_or_else(|| malformed("rotzoom global-motion matrix overflows"))?;
        if global.matrix[5] != global.matrix[2] || global.matrix[4] != neg {
            return Err(malformed("rotzoom global-motion matrix is inconsistent"));
        }
    }
    if global.matrix[2] <= 0 {
        return Ok(None);
    }
    let alpha = global_motion_clip_wmp(i64::from(global.matrix[2]) - (1_i64 << 16))?;
    let beta = global_motion_clip_wmp(i64::from(global.matrix[3]))?;
    let (shift, reciprocal) = global_motion_divisor(global.matrix[2])?;
    let reciprocal = i64::from(reciprocal);
    let v1 = i64::from(global.matrix[4])
        .checked_mul(1_i64 << 16)
        .and_then(|value| value.checked_mul(reciprocal))
        .ok_or_else(|| malformed("global-motion gamma product overflows"))?;
    let gamma_value = signed_rounded_shift(v1, shift)?;
    let gamma = global_motion_clip_wmp(gamma_value)?;
    let v2 = i64::from(global.matrix[3])
        .checked_mul(i64::from(global.matrix[4]))
        .and_then(|value| value.checked_mul(reciprocal))
        .ok_or_else(|| malformed("global-motion delta product overflows"))?;
    let delta_value = i64::from(global.matrix[5])
        .checked_sub(signed_rounded_shift(v2, shift)?)
        .and_then(|value| value.checked_sub(1_i64 << 16))
        .ok_or_else(|| malformed("global-motion delta product overflows"))?;
    let delta = global_motion_clip_wmp(delta_value)?;
    let alpha = i16::try_from(alpha).map_err(|_| malformed("global-motion alpha exceeds i16"))?;
    let beta = i16::try_from(beta).map_err(|_| malformed("global-motion beta exceeds i16"))?;
    let gamma = i16::try_from(gamma).map_err(|_| malformed("global-motion gamma exceeds i16"))?;
    let delta = i16::try_from(delta).map_err(|_| malformed("global-motion delta exceeds i16"))?;
    if 4_i32
        .checked_mul(i32::from(alpha).abs())
        .and_then(|value| value.checked_add(7_i32.checked_mul(i32::from(beta).abs())?))
        .is_none_or(|value| value >= 1 << 16)
        || 4_i32
            .checked_mul(i32::from(gamma).abs())
            .and_then(|value| value.checked_add(4_i32.checked_mul(i32::from(delta).abs())?))
            .is_none_or(|value| value >= 1 << 16)
    {
        return Ok(None);
    }
    Ok(Some(PreparedGlobalWarp {
        matrix: global.matrix,
        abcd: [alpha, beta, gamma, delta],
    }))
}

fn signed_rounded_shift(value: i64, shift: u32) -> Av1Result<i64> {
    if shift == 0 {
        return Ok(value);
    }
    let rounding_shift = shift
        .checked_sub(1)
        .ok_or_else(|| malformed("global-motion rounding shift underflows"))?;
    let rounding = 1_i64
        .checked_shl(rounding_shift)
        .ok_or_else(|| malformed("global-motion rounding shift overflows"))?;
    let magnitude = value
        .unsigned_abs()
        .checked_add(
            u64::try_from(rounding).map_err(|_| malformed("global-motion rounding exceeds u64"))?,
        )
        .ok_or_else(|| malformed("global-motion rounding overflows"))?
        >> shift;
    let magnitude = i64::try_from(magnitude)
        .map_err(|_| malformed("global-motion rounded value exceeds i64"))?;
    Ok(if value < 0 { -magnitude } else { magnitude })
}

/// Derive the block-centred global MV used by both spatial candidates and the
/// GLOBALMV prediction mode. Matrix products stay in i64 until the Q3 value
/// is validated and precision-corrected.
pub(super) fn global_motion_vector(
    global: GlobalMotion,
    block_x_b4: u32,
    block_y_b4: u32,
    block_width_b4: u32,
    block_height_b4: u32,
    force_integer_mv: bool,
    high_precision_mv: bool,
) -> Av1Result<MotionVector> {
    let block_x = i64::from(block_x_b4);
    let block_y = i64::from(block_y_b4);
    let block_width = i64::from(block_width_b4);
    let block_height = i64::from(block_height_b4);
    let to_motion = |y: i64, x: i64| -> Av1Result<MotionVector> {
        let y = i16::try_from(y).map_err(|_| malformed("global-motion vertical MV exceeds i16"))?;
        let x =
            i16::try_from(x).map_err(|_| malformed("global-motion horizontal MV exceeds i16"))?;
        Ok(MotionVector { y, x }.reduce_precision(force_integer_mv, true))
    };
    match global.kind {
        GlobalMotionType::Identity => Ok(MotionVector::ZERO),
        GlobalMotionType::Translation => {
            let y = i64::from(global.matrix[0]) >> 13;
            let x = i64::from(global.matrix[1]) >> 13;
            to_motion(y, x)
        }
        GlobalMotionType::RotZoom | GlobalMotionType::Affine => {
            if matches!(global.kind, GlobalMotionType::RotZoom) {
                let matrix_3_neg = global.matrix[3]
                    .checked_neg()
                    .ok_or_else(|| malformed("rotzoom global-motion matrix overflows"))?;
                if global.matrix[5] != global.matrix[2] || global.matrix[4] != matrix_3_neg {
                    return Err(malformed("rotzoom global-motion matrix is inconsistent"));
                }
            }
            let x = block_x
                .checked_mul(4)
                .and_then(|value| value.checked_add(block_width.checked_mul(2)?))
                .and_then(|value| value.checked_sub(1))
                .ok_or_else(|| malformed("global-motion x centre overflows"))?;
            let y = block_y
                .checked_mul(4)
                .and_then(|value| value.checked_add(block_height.checked_mul(2)?))
                .and_then(|value| value.checked_sub(1))
                .ok_or_else(|| malformed("global-motion y centre overflows"))?;
            let matrix_x = i64::from(global.matrix[2])
                .checked_sub(1_i64 << 16)
                .ok_or_else(|| malformed("global-motion horizontal matrix overflows"))?;
            let matrix_y = i64::from(global.matrix[5])
                .checked_sub(1_i64 << 16)
                .ok_or_else(|| malformed("global-motion vertical matrix overflows"))?;
            let xc = matrix_x
                .checked_mul(x)
                .and_then(|value| value.checked_add(i64::from(global.matrix[3]).checked_mul(y)?))
                .and_then(|value| value.checked_add(i64::from(global.matrix[0])))
                .ok_or_else(|| malformed("global-motion horizontal product overflows"))?;
            let yc = matrix_y
                .checked_mul(y)
                .and_then(|value| value.checked_add(i64::from(global.matrix[4]).checked_mul(x)?))
                .and_then(|value| value.checked_add(i64::from(global.matrix[1])))
                .ok_or_else(|| malformed("global-motion vertical product overflows"))?;
            let shift = if high_precision_mv { 13 } else { 14 };
            let post_shift = u32::from(!high_precision_mv);
            let y = signed_rounded_shift(yc, shift)?
                .checked_shl(post_shift)
                .ok_or_else(|| malformed("global-motion vertical MV shift overflows"))?;
            let x = signed_rounded_shift(xc, shift)?
                .checked_shl(post_shift)
                .ok_or_else(|| malformed("global-motion horizontal MV shift overflows"))?;
            to_motion(y, x)
        }
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
    coded_count: u8,
    pub(super) context: u8,
}

impl MotionCandidateStack {
    pub(super) const fn new() -> Self {
        Self {
            candidates: [MotionCandidate {
                vectors: [MotionVector::ZERO; 2],
                weight: 0,
            }; 8],
            coded_count: 0,
            context: 0,
        }
    }

    pub(super) const fn len(self) -> usize {
        self.coded_count as usize
    }

    pub(super) fn as_slice(&self) -> &[MotionCandidate] {
        &self.candidates[..self.len()]
    }

    pub(super) fn get(&self, index: usize) -> Option<MotionCandidate> {
        self.candidates
            .get(index)
            .copied()
            .filter(|_| index < self.len())
    }

    /// Read an initialized stack slot. Single-reference stacks retain global
    /// fallbacks in slots zero and one even when `coded_count` is zero or one;
    /// those slots are intentionally distinct from the coded-count range.
    pub(super) fn slot(&self, index: usize) -> Option<MotionCandidate> {
        self.candidates
            .get(index)
            .copied()
            .filter(|_| index < 2 || index < self.len())
    }

    pub(super) const fn coded_count(self) -> usize {
        self.coded_count as usize
    }

    pub(super) fn add_or_weight(&mut self, vectors: [MotionVector; 2], weight: i32) {
        let count = self.len();
        if let Some(candidate) = self
            .candidates
            .iter_mut()
            .take(count)
            .find(|candidate| candidate.vectors == vectors)
        {
            candidate.weight = candidate.weight.saturating_add(weight);
            return;
        }
        let index = self.len();
        if let Some(candidate) = self.candidates.get_mut(index) {
            *candidate = MotionCandidate { vectors, weight };
            self.coded_count = self.coded_count.saturating_add(1);
        }
    }

    /// Insert a minimal extension candidate without changing the weight of an
    /// already-present vector. dav1d's non-self extension pass is a unique
    /// fill operation, whereas direct/temporal scans intentionally add weight
    /// to duplicates.
    fn add_unique(&mut self, vectors: [MotionVector; 2], weight: i32) {
        if self
            .candidates
            .iter()
            .take(self.len())
            .any(|candidate| candidate.vectors == vectors)
        {
            return;
        }
        let index = self.len();
        if let Some(candidate) = self.candidates.get_mut(index) {
            *candidate = MotionCandidate { vectors, weight };
            self.coded_count = self.coded_count.saturating_add(1);
        }
    }

    pub(super) fn sort_range_by_weight(&mut self, start: usize) {
        self.sort_range_by_weight_until(start, self.len());
    }

    pub(super) fn sort_range_by_weight_until(&mut self, start: usize, end: usize) {
        let end = end.min(self.len());
        if start >= end {
            return;
        }
        self.candidates[start..end].sort_by(|left, right| right.weight.cmp(&left.weight));
    }

    fn add_weight_to_range(&mut self, end: usize, weight: i32) {
        let count = self.len();
        for candidate in self.candidates.iter_mut().take(end.min(count)) {
            candidate.weight = candidate.weight.saturating_add(weight);
        }
    }

    fn replace_slot(&mut self, index: usize, candidate: MotionCandidate) -> Av1Result<()> {
        let slot = self
            .candidates
            .get_mut(index)
            .ok_or_else(|| malformed("motion candidate slot exceeds stack capacity"))?;
        *slot = candidate;
        Ok(())
    }

    fn set_coded_count(&mut self, count: usize) -> Av1Result<()> {
        if count > self.candidates.len() {
            return Err(malformed("motion candidate count exceeds stack capacity"));
        }
        self.coded_count =
            u8::try_from(count).map_err(|_| malformed("motion candidate count exceeds u8"))?;
        Ok(())
    }

    pub(super) fn ensure_two(&mut self, fallbacks: [[MotionVector; 2]; 2]) {
        let old_count = self.len();
        for index in old_count..2 {
            // Global fallbacks fill stack positions directly. Equal vectors
            // in slots zero and one are intentional and must not coalesce.
            self.candidates[index] = MotionCandidate {
                vectors: fallbacks[index],
                weight: 0,
            };
        }
        self.coded_count = self.coded_count.max(2);
    }

    pub(super) fn set_single_fallback(&mut self, fallback: MotionVector) {
        if self.coded_count == 0 {
            self.candidates[0].vectors = [fallback, MotionVector::ZERO];
        }
        if self.coded_count < 2 {
            self.candidates[1].vectors = [fallback, MotionVector::ZERO];
        }
    }

    pub(super) fn drl_context(&self, index: usize) -> Option<u8> {
        let current = self.slot(index)?;
        let next = self.slot(index.saturating_add(1))?;
        Some(if current.weight >= 640 {
            u8::from(next.weight < 640)
        } else if next.weight < 640 {
            2
        } else {
            0
        })
    }
}

/// The exact reference sentinels consumed by AV1's spatial ref-MV walker.
/// Zero is intra/invalid, positive values are one-based logical references,
/// and `-1` marks the absent second reference of a single-reference block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SpatialRefBlock {
    pub(super) block_size: BlockSize,
    pub(super) references: [i8; 2],
    pub(super) vectors: [MotionVector; 2],
    pub(super) global_affine: [bool; 2],
    pub(super) new_mv: [bool; 2],
    pub(super) intra_bc: bool,
}

impl SpatialRefBlock {
    pub(super) const fn ordinary_intra(block_size: BlockSize) -> Self {
        Self {
            block_size,
            references: [0, -1],
            vectors: [MotionVector::ZERO; 2],
            global_affine: [false; 2],
            new_mv: [false; 2],
            intra_bc: false,
        }
    }

    pub(super) const fn intra_bc(block_size: BlockSize, vector: MotionVector) -> Self {
        Self {
            block_size,
            references: [0, -1],
            vectors: [vector, MotionVector::ZERO],
            global_affine: [false; 2],
            new_mv: [false; 2],
            intra_bc: true,
        }
    }

    pub(super) const fn single(
        block_size: BlockSize,
        reference: ReferenceFrame,
        vector: MotionVector,
        global_affine: bool,
        new_mv: bool,
    ) -> Self {
        Self {
            block_size,
            references: [reference.index() as i8 + 1, -1],
            vectors: [vector, MotionVector::ZERO],
            global_affine: [global_affine, false],
            new_mv: [new_mv, false],
            intra_bc: false,
        }
    }

    pub(super) const fn interintra(
        block_size: BlockSize,
        reference: ReferenceFrame,
        vector: MotionVector,
        global_affine: bool,
        new_mv: bool,
    ) -> Self {
        Self {
            block_size,
            references: [reference.index() as i8 + 1, 0],
            vectors: [vector, MotionVector::ZERO],
            global_affine: [global_affine, false],
            new_mv: [new_mv, false],
            intra_bc: false,
        }
    }

    pub(super) const fn compound(
        block_size: BlockSize,
        first: ReferenceFrame,
        second: ReferenceFrame,
        vectors: [MotionVector; 2],
        global_affine: [bool; 2],
        new_mv: [bool; 2],
    ) -> Self {
        Self {
            block_size,
            references: [first.index() as i8 + 1, second.index() as i8 + 1],
            vectors,
            global_affine,
            new_mv,
            intra_bc: false,
        }
    }

    pub(super) const fn is_intra(self) -> bool {
        self.references[0] == 0 && !self.intra_bc
    }

    pub(super) const fn is_compound(self) -> bool {
        self.references[1] >= 0
    }
}

/// Safe read-only source for the tile-local four-pixel motion grid.
pub(super) trait SpatialMotionSource {
    fn spatial_block(&self, x_b4: u32, y_b4: u32) -> Av1Result<Option<SpatialRefBlock>>;
}

/// A target branch for reference-MV discovery. IntraBC uses the pseudo
/// reference and therefore must not match ordinary inter neighbours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReferenceMvTarget {
    IntraBc,
    Single(ReferenceFrame),
    Compound(ReferencePair),
}

/// Absolute and tile-local geometry used by the scalar reference-MV walker.
#[derive(Clone, Copy)]
pub(super) struct ReferenceMvRequest<'a> {
    pub(super) target: ReferenceMvTarget,
    pub(super) block_size: BlockSize,
    pub(super) local_x_b4: u32,
    pub(super) local_y_b4: u32,
    pub(super) absolute_x_b4: u32,
    pub(super) absolute_y_b4: u32,
    pub(super) tile_left_b4: u32,
    pub(super) tile_top_b4: u32,
    pub(super) tile_right_b4: u32,
    pub(super) tile_bottom_b4: u32,
    pub(super) frame_width_b4: u32,
    pub(super) frame_height_b4: u32,
    pub(super) top_has_right: bool,
    pub(super) global_motion: [GlobalMotion; 7],
    pub(super) force_integer_mv: bool,
    pub(super) high_precision_mv: bool,
    pub(super) sign_bias: [bool; 7],
    pub(super) current_order_hint: u32,
    pub(super) order_hint_bits: u32,
    pub(super) reference_order_hints: [u32; 7],
    pub(super) use_ref_frame_mvs: bool,
    pub(super) temporal: Option<&'a ProjectedTemporalField>,
}

impl ReferenceMvRequest<'_> {
    fn local_extent(self) -> Av1Result<(u32, u32)> {
        let (width, height) = self.block_size.mi_dimensions();
        let right = self
            .tile_right_b4
            .checked_sub(self.local_x_b4)
            .ok_or_else(|| malformed("reference-MV block starts past tile right edge"))?;
        let bottom = self
            .tile_bottom_b4
            .checked_sub(self.local_y_b4)
            .ok_or_else(|| malformed("reference-MV block starts past tile bottom edge"))?;
        Ok((width.min(16).min(right), height.min(16).min(bottom)))
    }
}

fn target_references(target: ReferenceMvTarget) -> [i8; 2] {
    match target {
        ReferenceMvTarget::IntraBc => [0, -1],
        ReferenceMvTarget::Single(reference) => [reference.index() as i8 + 1, -1],
        ReferenceMvTarget::Compound(pair) => [
            pair.first.index() as i8 + 1,
            pair.second
                .map_or(0, |reference| reference.index() as i8 + 1),
        ],
    }
}

fn target_global_vectors(
    request: ReferenceMvRequest<'_>,
    width_b4: u32,
    height_b4: u32,
) -> Av1Result<([MotionVector; 2], [Option<MotionVector>; 2])> {
    let mut temporal = [MotionVector::ZERO; 2];
    let mut affine = [None; 2];
    let references = target_references(request.target);
    for (index, encoded) in references.into_iter().enumerate() {
        if encoded <= 0 {
            continue;
        }
        let reference = usize::try_from(encoded - 1)
            .map_err(|_| malformed("global-motion reference index is invalid"))?;
        let global = *request
            .global_motion
            .get(reference)
            .ok_or_else(|| malformed("global-motion reference exceeds seven slots"))?;
        temporal[index] = global_motion_vector(
            global,
            request.absolute_x_b4,
            request.absolute_y_b4,
            width_b4,
            height_b4,
            request.force_integer_mv,
            request.high_precision_mv,
        )?;
        if matches!(
            global.kind,
            GlobalMotionType::RotZoom | GlobalMotionType::Affine
        ) {
            affine[index] = Some(temporal[index]);
        }
    }
    Ok((temporal, affine))
}

fn add_spatial_candidate(
    stack: &mut MotionCandidateStack,
    block: SpatialRefBlock,
    target: ReferenceMvTarget,
    affine: [Option<MotionVector>; 2],
    have_new_mv: &mut i32,
    have_ref_mv: &mut i32,
    weight: i32,
) {
    if block.is_intra() {
        return;
    }
    let wanted = target_references(target);
    if matches!(target, ReferenceMvTarget::IntraBc) && !block.intra_bc {
        return;
    }
    if !matches!(target, ReferenceMvTarget::IntraBc) && block.intra_bc {
        return;
    }
    if block.references != wanted {
        return;
    }
    let mut vectors = block.vectors;
    for index in 0..2 {
        if block.global_affine[index]
            && let Some(global) = affine[index]
        {
            vectors[index] = global;
        }
    }
    *have_ref_mv = 1;
    *have_new_mv |= i32::from(block.new_mv[0]) | (i32::from(block.new_mv[1]) << 1);
    stack.add_or_weight(vectors, weight);
}

fn scan_row<S: SpatialMotionSource>(
    source: &S,
    stack: &mut MotionCandidateStack,
    target: ReferenceMvTarget,
    affine: [Option<MotionVector>; 2],
    start_x: u32,
    y: u32,
    block_width: u32,
    visible_width: u32,
    max_rows: u32,
    step: u32,
    have_new_mv: &mut i32,
    have_row_mvs: &mut i32,
) -> Av1Result<u32> {
    let Some(first) = source.spatial_block(start_x, y)? else {
        return Ok(0);
    };
    let (candidate_width, candidate_height) = first.block_size.mi_dimensions();
    let mut len = step.max(block_width.min(candidate_width));
    if block_width <= candidate_width {
        let weight = if block_width == 1 {
            2
        } else {
            2_u32.max(
                max_rows
                    .checked_mul(2)
                    .ok_or_else(|| malformed("reference-MV row weight overflows"))?
                    .min(candidate_height),
            )
        };
        add_spatial_candidate(
            stack,
            first,
            target,
            affine,
            have_new_mv,
            have_row_mvs,
            i32::try_from(
                len.checked_mul(weight)
                    .ok_or_else(|| malformed("reference-MV row weight overflows"))?,
            )
            .map_err(|_| malformed("reference-MV row weight exceeds i32"))?,
        );
        return Ok(weight / 2);
    }
    let mut x = 0_u32;
    loop {
        let coordinate = start_x
            .checked_add(x)
            .ok_or_else(|| malformed("reference-MV top scan coordinate overflows"))?;
        if let Some(block) = source.spatial_block(coordinate, y)? {
            let advance = len;
            add_spatial_candidate(
                stack,
                block,
                target,
                affine,
                have_new_mv,
                have_row_mvs,
                i32::try_from(
                    len.checked_mul(2)
                        .ok_or_else(|| malformed("reference-MV row weight overflows"))?,
                )
                .map_err(|_| malformed("reference-MV row weight exceeds i32"))?,
            );
            x = x
                .checked_add(advance)
                .ok_or_else(|| malformed("reference-MV top scan span overflows"))?;
            if x >= visible_width {
                return Ok(1);
            }
            let next_x = start_x
                .checked_add(x)
                .ok_or_else(|| malformed("reference-MV top scan next coordinate overflows"))?;
            let Some(next) = source.spatial_block(next_x, y)? else {
                return Ok(1);
            };
            let (next_width, _) = next.block_size.mi_dimensions();
            len = step.max(next_width);
        } else {
            return Ok(1);
        }
        // The next iteration's block width is read above; the source grid is
        // tile-bounded, so no speculative coordinate is exposed.
    }
}

fn scan_col<S: SpatialMotionSource>(
    source: &S,
    stack: &mut MotionCandidateStack,
    target: ReferenceMvTarget,
    affine: [Option<MotionVector>; 2],
    x: u32,
    start_y: u32,
    block_height: u32,
    visible_height: u32,
    max_cols: u32,
    step: u32,
    have_new_mv: &mut i32,
    have_col_mvs: &mut i32,
) -> Av1Result<u32> {
    let Some(first) = source.spatial_block(x, start_y)? else {
        return Ok(0);
    };
    let (candidate_width, candidate_height) = first.block_size.mi_dimensions();
    let mut len = step.max(block_height.min(candidate_height));
    if block_height <= candidate_height {
        let weight = if block_height == 1 {
            2
        } else {
            2_u32.max(
                max_cols
                    .checked_mul(2)
                    .ok_or_else(|| malformed("reference-MV column weight overflows"))?
                    .min(candidate_width),
            )
        };
        add_spatial_candidate(
            stack,
            first,
            target,
            affine,
            have_new_mv,
            have_col_mvs,
            i32::try_from(
                len.checked_mul(weight)
                    .ok_or_else(|| malformed("reference-MV column weight overflows"))?,
            )
            .map_err(|_| malformed("reference-MV column weight exceeds i32"))?,
        );
        return Ok(weight / 2);
    }
    let mut y = 0_u32;
    loop {
        let coordinate = start_y
            .checked_add(y)
            .ok_or_else(|| malformed("reference-MV left scan coordinate overflows"))?;
        if let Some(block) = source.spatial_block(x, coordinate)? {
            let advance = len;
            add_spatial_candidate(
                stack,
                block,
                target,
                affine,
                have_new_mv,
                have_col_mvs,
                i32::try_from(
                    len.checked_mul(2)
                        .ok_or_else(|| malformed("reference-MV column weight overflows"))?,
                )
                .map_err(|_| malformed("reference-MV column weight exceeds i32"))?,
            );
            y = y
                .checked_add(advance)
                .ok_or_else(|| malformed("reference-MV left scan span overflows"))?;
            if y >= visible_height {
                return Ok(1);
            }
            let next_y = start_y
                .checked_add(y)
                .ok_or_else(|| malformed("reference-MV left scan next coordinate overflows"))?;
            let Some(next) = source.spatial_block(x, next_y)? else {
                return Ok(1);
            };
            let (_, next_height) = next.block_size.mi_dimensions();
            len = step.max(next_height);
        } else {
            return Ok(1);
        }
    }
}

fn add_compound_extended_candidate(
    same: &mut [[Option<MotionVector>; 2]; 2],
    different: &mut [[Option<MotionVector>; 2]; 2],
    counts: &mut [[u8; 2]; 2],
    block: SpatialRefBlock,
    wanted: [i8; 2],
    signs: [bool; 2],
    sign_bias: [bool; 7],
) {
    for index in 0..2 {
        let candidate_ref = block.references[index];
        if candidate_ref <= 0 {
            break;
        }
        let Ok(candidate_index) = usize::try_from(candidate_ref - 1) else {
            continue;
        };
        let candidate_sign = sign_bias.get(candidate_index).copied().unwrap_or(false);
        let candidate_mv = block.vectors[index];
        if candidate_ref == wanted[0] {
            if counts[0][0] < 2 {
                same[0][usize::from(counts[0][0])] = Some(candidate_mv);
                counts[0][0] = counts[0][0].saturating_add(1);
            }
            if counts[1][1] < 2 {
                let value = if signs[1] != candidate_sign {
                    MotionVector {
                        y: candidate_mv.y.saturating_neg(),
                        x: candidate_mv.x.saturating_neg(),
                    }
                } else {
                    candidate_mv
                };
                different[1][usize::from(counts[1][1])] = Some(value);
                counts[1][1] = counts[1][1].saturating_add(1);
            }
        } else if candidate_ref == wanted[1] {
            if counts[0][1] < 2 {
                same[1][usize::from(counts[0][1])] = Some(candidate_mv);
                counts[0][1] = counts[0][1].saturating_add(1);
            }
            if counts[1][0] < 2 {
                let value = if signs[0] != candidate_sign {
                    MotionVector {
                        y: candidate_mv.y.saturating_neg(),
                        x: candidate_mv.x.saturating_neg(),
                    }
                } else {
                    candidate_mv
                };
                different[0][usize::from(counts[1][0])] = Some(value);
                counts[1][0] = counts[1][0].saturating_add(1);
            }
        } else {
            let inverted = MotionVector {
                y: candidate_mv.y.saturating_neg(),
                x: candidate_mv.x.saturating_neg(),
            };
            for target_index in 0..2 {
                if counts[1][target_index] >= 2 {
                    continue;
                }
                let value = if signs[target_index] != candidate_sign {
                    inverted
                } else {
                    candidate_mv
                };
                different[target_index][usize::from(counts[1][target_index])] = Some(value);
                counts[1][target_index] = counts[1][target_index].saturating_add(1);
            }
        }
    }
}

fn add_single_extended_candidate(
    stack: &mut MotionCandidateStack,
    block: SpatialRefBlock,
    _target: ReferenceFrame,
    target_sign: bool,
    sign_bias: [bool; 7],
) {
    for index in 0..2 {
        let candidate_ref = block.references[index];
        if candidate_ref <= 0 {
            break;
        }
        let Ok(candidate_index) = usize::try_from(candidate_ref - 1) else {
            continue;
        };
        let mut vector = block.vectors[index];
        if target_sign != sign_bias.get(candidate_index).copied().unwrap_or(false) {
            vector = MotionVector {
                y: vector.y.saturating_neg(),
                x: vector.x.saturating_neg(),
            };
        }
        stack.add_unique([vector, MotionVector::ZERO], 2);
        if stack.coded_count() >= 2 {
            return;
        }
    }
}

fn add_temporal_candidate(
    stack: &mut MotionCandidateStack,
    entry: ProjectedTemporalEntry,
    target: ReferenceMvTarget,
    request: ReferenceMvRequest<'_>,
    global_vectors: [MotionVector; 2],
    globalmv_context: Option<&mut i32>,
) {
    let target_refs = target_references(target);
    if target_refs[0] <= 0 {
        return;
    }
    let mut vectors = [MotionVector::ZERO; 2];
    for index in 0..2 {
        let encoded = target_refs[index];
        if encoded <= 0 {
            return;
        }
        let Some(reference_index) = usize::try_from(encoded - 1).ok() else {
            return;
        };
        let target_distance = relative_distance(
            request.order_hint_bits,
            request.current_order_hint,
            request.reference_order_hints[reference_index],
        );
        let Some(mut vector) = project_vector(entry.vector, entry.denominator, target_distance)
        else {
            return;
        };
        vector = vector.reduce_precision(request.force_integer_mv, request.high_precision_mv);
        vectors[index] = vector;
    }
    // dav1d derives the global-MV context only for the single-reference
    // temporal candidate. Compound candidates carry two projected vectors
    // and do not consume the single-reference global context bit.
    if matches!(target, ReferenceMvTarget::Single(_))
        && let Some(context) = globalmv_context
    {
        *context = i32::from(
            (vectors[0].x.abs_diff(global_vectors[0].x)
                | vectors[0].y.abs_diff(global_vectors[0].y))
                >= 16,
        );
    }
    stack.add_or_weight(vectors, 2);
}

/// Build the exact single/compound reference-MV stack for one block. This is
/// deliberately pure and scalar; entropy integration supplies the typed
/// target only after all downstream inter branches exist.
pub(super) fn find_reference_mvs<S: SpatialMotionSource>(
    source: &S,
    request: ReferenceMvRequest<'_>,
) -> Av1Result<MotionCandidateStack> {
    let origin_x_b4 = request
        .absolute_x_b4
        .checked_sub(request.local_x_b4)
        .ok_or_else(|| malformed("reference-MV absolute x precedes local block"))?;
    let origin_y_b4 = request
        .absolute_y_b4
        .checked_sub(request.local_y_b4)
        .ok_or_else(|| malformed("reference-MV absolute y precedes local block"))?;
    let tile_left_abs_b4 = origin_x_b4
        .checked_add(request.tile_left_b4)
        .ok_or_else(|| malformed("reference-MV absolute tile left overflows"))?;
    let tile_top_abs_b4 = origin_y_b4
        .checked_add(request.tile_top_b4)
        .ok_or_else(|| malformed("reference-MV absolute tile top overflows"))?;
    let tile_right_abs_b4 = origin_x_b4
        .checked_add(request.tile_right_b4)
        .ok_or_else(|| malformed("reference-MV absolute tile right overflows"))?;
    let tile_bottom_abs_b4 = origin_y_b4
        .checked_add(request.tile_bottom_b4)
        .ok_or_else(|| malformed("reference-MV absolute tile bottom overflows"))?;
    if request.tile_left_b4 >= request.tile_right_b4
        || request.tile_top_b4 >= request.tile_bottom_b4
        || request.frame_width_b4 == 0
        || request.frame_height_b4 == 0
        || tile_left_abs_b4 >= tile_right_abs_b4
        || tile_top_abs_b4 >= tile_bottom_abs_b4
        || tile_right_abs_b4 > request.frame_width_b4
        || tile_bottom_abs_b4 > request.frame_height_b4
        || request.local_x_b4 < request.tile_left_b4
        || request.local_y_b4 < request.tile_top_b4
    {
        return Err(malformed("reference-MV tile/block geometry is invalid"));
    }
    if matches!(request.target, ReferenceMvTarget::Compound(pair) if pair.second.is_none()) {
        return Err(malformed(
            "compound reference-MV target omits its second reference",
        ));
    }
    let (block_width, block_height) = request.block_size.mi_dimensions();
    let (width, height) = request.local_extent()?;
    if width == 0 || height == 0 {
        return Err(malformed("reference-MV block has an empty tile extent"));
    }
    let (temporal_vectors, affine_vectors) =
        target_global_vectors(request, block_width, block_height)?;
    let mut stack = MotionCandidateStack::new();
    let mut have_new_mv = 0_i32;
    let mut have_row_mvs = 0_i32;
    let mut have_col_mvs = 0_i32;
    let mut max_rows = 0_u32;
    let mut max_cols = 0_u32;
    let mut n_rows = None;
    let mut n_cols = None;

    if request.local_y_b4 > request.tile_top_b4 {
        max_rows = ((request.local_y_b4 - request.tile_top_b4 + 1) >> 1)
            .min(2 + u32::from(block_height > 1));
        n_rows = Some(scan_row(
            source,
            &mut stack,
            request.target,
            affine_vectors,
            request.local_x_b4,
            request.local_y_b4 - 1,
            block_width,
            width,
            max_rows,
            if block_width >= 16 { 4 } else { 1 },
            &mut have_new_mv,
            &mut have_row_mvs,
        )?);
    }
    if request.local_x_b4 > request.tile_left_b4 {
        max_cols = ((request.local_x_b4 - request.tile_left_b4 + 1) >> 1)
            .min(2 + u32::from(block_width > 1));
        n_cols = Some(scan_col(
            source,
            &mut stack,
            request.target,
            affine_vectors,
            request.local_x_b4 - 1,
            request.local_y_b4,
            block_height,
            height,
            max_cols,
            if block_height >= 16 { 4 } else { 1 },
            &mut have_new_mv,
            &mut have_col_mvs,
        )?);
    }
    if n_rows.is_some()
        && request.top_has_right
        && block_width.max(block_height) <= 16
        && request.local_x_b4.saturating_add(block_width) < request.tile_right_b4
    {
        if let Some(block) = source.spatial_block(
            request.local_x_b4.saturating_add(block_width),
            request.local_y_b4.saturating_sub(1),
        )? {
            add_spatial_candidate(
                &mut stack,
                block,
                request.target,
                affine_vectors,
                &mut have_new_mv,
                &mut have_row_mvs,
                4,
            );
        }
    }

    let nearest_match = have_col_mvs + have_row_mvs;
    let nearest_count = stack.coded_count();
    stack.add_weight_to_range(nearest_count, 640);

    let mut globalmv_context = i32::from(request.use_ref_frame_mvs);
    if request.use_ref_frame_mvs {
        let temporal = request
            .temporal
            .ok_or_else(|| malformed("reference-MV temporal field is unavailable"))?;
        let base_x = request.absolute_x_b4 / 2;
        let base_y = request.absolute_y_b4 / 2;
        let step_h = if block_width >= 16 { 2 } else { 1 };
        let step_v = if block_height >= 16 { 2 } else { 1 };
        let temporal_width = ((width + 1) / 2).min(8);
        let temporal_height = ((height + 1) / 2).min(8);
        for y in (0..temporal_height).step_by(usize::try_from(step_v).unwrap_or(1)) {
            for x in (0..temporal_width).step_by(usize::try_from(step_h).unwrap_or(1)) {
                let x8 = base_x.saturating_add(x);
                let y8 = base_y.saturating_add(y);
                if let Some(entry) = temporal.get(x8, y8) {
                    let context = if x == 0 && y == 0 {
                        Some(&mut globalmv_context)
                    } else {
                        None
                    };
                    add_temporal_candidate(
                        &mut stack,
                        entry,
                        request.target,
                        request,
                        temporal_vectors,
                        context,
                    );
                }
            }
        }
        if block_width.min(block_height) >= 2 && block_width.max(block_height) < 16 {
            let block_width8 = block_width / 2;
            let block_height8 = block_height / 2;
            let has_bottom = request
                .absolute_y_b4
                .checked_add(block_height)
                .is_some_and(|end| end < tile_bottom_abs_b4)
                && base_y.saturating_add(block_height8)
                    < ((base_y & !7) + 8).min(tile_bottom_abs_b4 / 2);
            if has_bottom
                && base_x > tile_left_abs_b4 / 2
                && let Some(entry) = temporal.get(base_x, base_y.saturating_add(block_height8))
            {
                add_temporal_candidate(
                    &mut stack,
                    entry,
                    request.target,
                    request,
                    temporal_vectors,
                    None,
                );
            }
            if base_x.saturating_add(block_width8) < ((base_x & !7) + 8).min(tile_right_abs_b4 / 2)
            {
                if has_bottom
                    && let Some(entry) = temporal.get(
                        base_x.saturating_add(block_width8),
                        base_y.saturating_add(block_height8),
                    )
                {
                    add_temporal_candidate(
                        &mut stack,
                        entry,
                        request.target,
                        request,
                        temporal_vectors,
                        None,
                    );
                }
                if base_y.saturating_add(block_height8).saturating_sub(1)
                    < ((base_y & !7) + 8).min(tile_bottom_abs_b4 / 2)
                    && let Some(entry) = temporal.get(
                        base_x.saturating_add(block_width8),
                        base_y.saturating_add(block_height8).saturating_sub(1),
                    )
                {
                    add_temporal_candidate(
                        &mut stack,
                        entry,
                        request.target,
                        request,
                        temporal_vectors,
                        None,
                    );
                }
            }
        }
    }

    if n_rows.is_some() && n_cols.is_some() {
        if let Some(block) = source.spatial_block(request.local_x_b4 - 1, request.local_y_b4 - 1)? {
            let mut dummy = 0_i32;
            add_spatial_candidate(
                &mut stack,
                block,
                request.target,
                affine_vectors,
                &mut dummy,
                &mut have_row_mvs,
                4,
            );
        }
    }

    let mut scanned_rows = n_rows.unwrap_or(u32::MAX);
    let mut scanned_cols = n_cols.unwrap_or(u32::MAX);
    let mut dummy_new_mv = 0_i32;
    for offset in 2..=3_u32 {
        if offset > scanned_rows && offset <= max_rows {
            let row = request
                .local_y_b4
                .checked_sub(2 * offset)
                .unwrap_or(0)
                .saturating_add(1)
                | 1;
            let start_x = request.local_x_b4 | 1;
            scanned_rows = scanned_rows.saturating_add(scan_row(
                source,
                &mut stack,
                request.target,
                affine_vectors,
                start_x,
                row,
                block_width,
                width,
                1 + max_rows - offset,
                if block_width >= 16 { 4 } else { 2 },
                &mut dummy_new_mv,
                &mut have_row_mvs,
            )?);
        }
        if offset > scanned_cols && offset <= max_cols {
            let column = request
                .local_x_b4
                .checked_sub(2 * offset)
                .unwrap_or(0)
                .saturating_add(1)
                | 1;
            let start_y = request.local_y_b4 | 1;
            scanned_cols = scanned_cols.saturating_add(scan_col(
                source,
                &mut stack,
                request.target,
                affine_vectors,
                column,
                start_y,
                block_height,
                height,
                1 + max_cols - offset,
                if block_height >= 16 { 4 } else { 2 },
                &mut dummy_new_mv,
                &mut have_col_mvs,
            )?);
        }
    }

    let ref_match_count = have_col_mvs + have_row_mvs;
    let (reference_context, new_mv_context) = match nearest_match {
        0 => (2.min(ref_match_count), i32::from(ref_match_count > 0)),
        1 => ((3 * ref_match_count).min(4), 3 - have_new_mv),
        2 => (5, 5 - have_new_mv),
        _ => (0, 0),
    };
    stack.sort_range_by_weight_until(0, nearest_count);
    stack.sort_range_by_weight(nearest_count);

    let wanted = target_references(request.target);
    if matches!(request.target, ReferenceMvTarget::Compound(_)) {
        if stack.coded_count() < 2 {
            let mut same = [[None; 2]; 2];
            let mut different = [[None; 2]; 2];
            let mut counts = [[0_u8; 2]; 2];
            let signs = [
                request
                    .sign_bias
                    .get(usize::try_from(wanted[0] - 1).unwrap_or(0))
                    .copied()
                    .unwrap_or(false),
                request
                    .sign_bias
                    .get(usize::try_from(wanted[1] - 1).unwrap_or(0))
                    .copied()
                    .unwrap_or(false),
            ];
            let size = width.min(height);
            if n_rows.is_some() {
                let mut x = 0_u32;
                while x < size {
                    if let Some(block) = source.spatial_block(
                        request.local_x_b4.saturating_add(x),
                        request.local_y_b4.saturating_sub(1),
                    )? {
                        let (candidate_width, _) = block.block_size.mi_dimensions();
                        add_compound_extended_candidate(
                            &mut same,
                            &mut different,
                            &mut counts,
                            block,
                            wanted,
                            signs,
                            request.sign_bias,
                        );
                        x = x.saturating_add(candidate_width.max(1));
                    } else {
                        break;
                    }
                }
            }
            if n_cols.is_some() {
                let mut y = 0_u32;
                while y < size {
                    if let Some(block) = source.spatial_block(
                        request.local_x_b4.saturating_sub(1),
                        request.local_y_b4.saturating_add(y),
                    )? {
                        let (_, candidate_height) = block.block_size.mi_dimensions();
                        add_compound_extended_candidate(
                            &mut same,
                            &mut different,
                            &mut counts,
                            block,
                            wanted,
                            signs,
                            request.sign_bias,
                        );
                        y = y.saturating_add(candidate_height.max(1));
                    } else {
                        break;
                    }
                }
            }
            let mut component = [[MotionVector::ZERO; 2]; 2];
            for index in 0..2 {
                let mut filled = 0_usize;
                for value in same[index]
                    .into_iter()
                    .chain(different[index].into_iter())
                    .flatten()
                {
                    if filled >= 2 {
                        break;
                    }
                    component[index][filled] = value;
                    filled += 1;
                }
                while filled < 2 {
                    component[index][filled] = temporal_vectors[index];
                    filled += 1;
                }
            }
            let extension = [
                MotionCandidate {
                    vectors: [component[0][0], component[1][0]],
                    weight: 2,
                },
                MotionCandidate {
                    vectors: [component[0][1], component[1][1]],
                    weight: 2,
                },
            ];
            // When the first merged extension duplicates the existing
            // nearest candidate, dav1d replaces it with the first
            // "different-reference" pair (the candidate held in stack slot
            // two before the merge), not with the second same-reference
            // component. Preserve that distinction explicitly.
            let different_pair = match (different[0][0], different[1][0]) {
                (Some(first), Some(second)) => MotionCandidate {
                    vectors: [first, second],
                    weight: 2,
                },
                _ => extension[1],
            };
            let coded_count = stack.coded_count();
            if coded_count == 1
                && stack
                    .slot(0)
                    .is_some_and(|candidate| candidate.vectors == extension[0].vectors)
            {
                stack.replace_slot(1, different_pair)?;
            } else {
                stack.replace_slot(coded_count, extension[0])?;
                if coded_count == 0 {
                    stack.replace_slot(1, extension[1])?;
                }
            }
            stack.set_coded_count(2)?;
        }
        stack.context = match reference_context >> 1 {
            0 => u8::try_from(new_mv_context.min(1)).unwrap_or(0),
            1 => u8::try_from(1 + new_mv_context.min(3)).unwrap_or(0),
            2 => u8::try_from((3 + new_mv_context).clamp(4, 7)).unwrap_or(0),
            _ => stack.context,
        };
        let coded_count = stack.coded_count();
        for candidate in &mut stack.candidates[..coded_count] {
            candidate.vectors[0] = candidate.vectors[0].clamp_to_block(
                request.absolute_x_b4,
                request.absolute_y_b4,
                request.block_size,
                request.frame_width_b4,
                request.frame_height_b4,
            )?;
            candidate.vectors[1] = candidate.vectors[1].clamp_to_block(
                request.absolute_x_b4,
                request.absolute_y_b4,
                request.block_size,
                request.frame_width_b4,
                request.frame_height_b4,
            )?;
        }
        return Ok(stack);
    }

    if let ReferenceMvTarget::Single(target_reference) = request.target
        && stack.coded_count() < 2
    {
        if n_rows.is_some() {
            let mut x = 0_u32;
            let size = width.min(height);
            while x < size && stack.coded_count() < 2 {
                if let Some(block) = source.spatial_block(
                    request.local_x_b4.saturating_add(x),
                    request.local_y_b4.saturating_sub(1),
                )? {
                    let (candidate_width, _) = block.block_size.mi_dimensions();
                    add_single_extended_candidate(
                        &mut stack,
                        block,
                        target_reference,
                        request.sign_bias[target_reference.index()],
                        request.sign_bias,
                    );
                    x = x.saturating_add(candidate_width.max(1));
                } else {
                    break;
                }
            }
        }
        if n_cols.is_some() {
            let mut y = 0_u32;
            let size = width.min(height);
            while y < size && stack.coded_count() < 2 {
                if let Some(block) = source.spatial_block(
                    request.local_x_b4.saturating_sub(1),
                    request.local_y_b4.saturating_add(y),
                )? {
                    let (_, candidate_height) = block.block_size.mi_dimensions();
                    add_single_extended_candidate(
                        &mut stack,
                        block,
                        target_reference,
                        request.sign_bias[target_reference.index()],
                        request.sign_bias,
                    );
                    y = y.saturating_add(candidate_height.max(1));
                } else {
                    break;
                }
            }
        }
    }
    let coded_count = stack.coded_count();
    for candidate in &mut stack.candidates[..coded_count] {
        candidate.vectors[0] = candidate.vectors[0].clamp_to_block(
            request.absolute_x_b4,
            request.absolute_y_b4,
            request.block_size,
            request.frame_width_b4,
            request.frame_height_b4,
        )?;
    }
    stack.set_single_fallback(temporal_vectors[0]);
    stack.context =
        u8::try_from((reference_context << 4) | (globalmv_context << 3) | new_mv_context)
            .map_err(|_| malformed("single-reference MV context exceeds u8"))?;
    Ok(stack)
}

/// Geometry needed to legalize an intraBC source rectangle against tile and
/// superblock causality. All positions are luma pixels after conversion from
/// the four-pixel coded grid.
#[derive(Clone, Copy, Debug)]
pub(super) struct IntrabcLegalityInput {
    pub(super) block_x_b4: u32,
    pub(super) block_y_b4: u32,
    pub(super) block_width_b4: u32,
    pub(super) block_height_b4: u32,
    pub(super) tile_left_b4: u32,
    pub(super) tile_top_b4: u32,
    pub(super) tile_right_b4: u32,
    pub(super) tile_bottom_b4: u32,
    pub(super) has_chroma: bool,
    pub(super) subsampling_x: bool,
    pub(super) subsampling_y: bool,
    pub(super) sb128: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct IntrabcSource {
    pub(super) motion_vector: MotionVector,
    pub(super) left: i64,
    pub(super) top: i64,
    pub(super) right: i64,
    pub(super) bottom: i64,
}

/// Apply dav1d's tile-border relocation and current-superblock overlap rules
/// to a decoded intraBC MV. The returned rectangle is still in the coded
/// pre-superres canvas and is safe to pass to `FrameCanvas::stage_written_rect`.
pub(super) fn relocate_intrabc_source(
    input: IntrabcLegalityInput,
    decoded_mv: MotionVector,
) -> Av1Result<IntrabcSource> {
    if input.block_width_b4 == 0
        || input.block_height_b4 == 0
        || input.tile_left_b4 >= input.tile_right_b4
        || input.tile_top_b4 >= input.tile_bottom_b4
    {
        return Err(malformed("intraBC legality geometry is invalid"));
    }
    let block_x = i64::from(input.block_x_b4);
    let block_y = i64::from(input.block_y_b4);
    let block_width = i64::from(input.block_width_b4);
    let block_height = i64::from(input.block_height_b4);
    let tile_left = i64::from(input.tile_left_b4)
        .checked_mul(4)
        .ok_or_else(|| malformed("intraBC tile-left coordinate overflows"))?;
    let tile_top = i64::from(input.tile_top_b4)
        .checked_mul(4)
        .ok_or_else(|| malformed("intraBC tile-top coordinate overflows"))?;
    let tile_right_b4 = i64::from(input.tile_right_b4)
        .checked_add(block_width - 1)
        .ok_or_else(|| malformed("intraBC tile-right coordinate overflows"))?;
    let border_right = (tile_right_b4 & !(block_width - 1))
        .checked_mul(4)
        .ok_or_else(|| malformed("intraBC right border overflows"))?;
    let block_left = block_x
        .checked_mul(4)
        .and_then(|value| value.checked_add(i64::from(decoded_mv.x) >> 3))
        .ok_or_else(|| malformed("intraBC source-left coordinate overflows"))?;
    let block_top = block_y
        .checked_mul(4)
        .and_then(|value| value.checked_add(i64::from(decoded_mv.y) >> 3))
        .ok_or_else(|| malformed("intraBC source-top coordinate overflows"))?;
    let source_width = block_width
        .checked_mul(4)
        .ok_or_else(|| malformed("intraBC source width overflows"))?;
    let source_height = block_height
        .checked_mul(4)
        .ok_or_else(|| malformed("intraBC source height overflows"))?;
    let mut left = block_left;
    let mut top = block_top;
    let mut right = left
        .checked_add(source_width)
        .ok_or_else(|| malformed("intraBC source-right coordinate overflows"))?;
    let mut bottom = top
        .checked_add(source_height)
        .ok_or_else(|| malformed("intraBC source-bottom coordinate overflows"))?;
    let border_left = tile_left
        .checked_add(
            if input.has_chroma && input.subsampling_x && input.block_width_b4 < 2 {
                4
            } else {
                0
            },
        )
        .ok_or_else(|| malformed("intraBC left border overflows"))?;
    let border_top = tile_top
        .checked_add(
            if input.has_chroma && input.subsampling_y && input.block_height_b4 < 2 {
                4
            } else {
                0
            },
        )
        .ok_or_else(|| malformed("intraBC top border overflows"))?;
    if left < border_left {
        let shift = border_left - left;
        left = left
            .checked_add(shift)
            .ok_or_else(|| malformed("intraBC left relocation overflows"))?;
        right = right
            .checked_add(shift)
            .ok_or_else(|| malformed("intraBC right relocation overflows"))?;
    } else if right > border_right {
        let shift = right - border_right;
        left = left
            .checked_sub(shift)
            .ok_or_else(|| malformed("intraBC left relocation underflows"))?;
        right = right
            .checked_sub(shift)
            .ok_or_else(|| malformed("intraBC right relocation underflows"))?;
    }
    if top < border_top {
        let shift = border_top - top;
        top = top
            .checked_add(shift)
            .ok_or_else(|| malformed("intraBC top relocation overflows"))?;
        bottom = bottom
            .checked_add(shift)
            .ok_or_else(|| malformed("intraBC bottom relocation overflows"))?;
    }

    let sb_width_b4 = if input.sb128 { 32_i64 } else { 16_i64 };
    let sb_size = if input.sb128 { 128_i64 } else { 64_i64 };
    let sb_left = (block_x / sb_width_b4)
        .checked_mul(sb_size)
        .ok_or_else(|| malformed("intraBC superblock x coordinate overflows"))?;
    let sb_top = (block_y / sb_width_b4)
        .checked_mul(sb_size)
        .ok_or_else(|| malformed("intraBC superblock y coordinate overflows"))?;
    if bottom > sb_top && right > sb_left {
        if top - border_top >= bottom - sb_top {
            let shift = bottom - sb_top;
            top -= shift;
            bottom -= shift;
        } else if left - border_left >= right - sb_left {
            let shift = right - sb_left;
            left -= shift;
            right -= shift;
        }
    }
    if bottom > sb_top + sb_size {
        let shift = bottom - (sb_top + sb_size);
        top -= shift;
        bottom -= shift;
    }
    if bottom > sb_top && right > sb_left {
        return Err(malformed("intraBC source overlaps current superblock"));
    }
    let motion_x = left
        .checked_sub(block_x * 4)
        .and_then(|value| value.checked_mul(8))
        .ok_or_else(|| malformed("intraBC horizontal MV conversion overflows"))?;
    let motion_y = top
        .checked_sub(block_y * 4)
        .and_then(|value| value.checked_mul(8))
        .ok_or_else(|| malformed("intraBC vertical MV conversion overflows"))?;
    Ok(IntrabcSource {
        motion_vector: MotionVector {
            x: i16::try_from(motion_x)
                .map_err(|_| malformed("intraBC horizontal MV exceeds i16"))?,
            y: i16::try_from(motion_y).map_err(|_| malformed("intraBC vertical MV exceeds i16"))?,
        },
        left,
        top,
        right,
        bottom,
    })
}

/// One projectable temporal-MV entry retained for an 8×8 coded region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RetainedTemporalEntry {
    pub(super) vector: MotionVector,
    pub(super) reference: ReferenceFrame,
}

pub(super) type TemporalMotion = RetainedTemporalEntry;

/// One sparse retained temporal-MV sample in the current frame's absolute
/// 8x8 coded grid.  Empty cells are intentionally omitted so tile assembly
/// can carry only the samples that may be projected by a later frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RetainedTemporalSample {
    pub(super) x8: u32,
    pub(super) y8: u32,
    pub(super) entry: RetainedTemporalEntry,
}

/// A temporal entry after source-frame relocation. The vector remains the
/// source-frame MV; `denominator` is the validated source-to-entry order
/// distance used by the later target-reference projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ProjectedTemporalEntry {
    pub(super) vector: MotionVector,
    pub(super) denominator: i32,
}

/// Tile/frame-local projected temporal-MV grid. Its row stride is padded to
/// the 128-pixel coded superblock granularity used by dav1d, while `get`
/// exposes only the logical coded width.
#[derive(Clone)]
pub(super) struct ProjectedTemporalField {
    coded_width: u32,
    frame_height: u32,
    stride: usize,
    logical_width: u32,
    cells: Vec<Option<ProjectedTemporalEntry>>,
}

impl ProjectedTemporalField {
    pub(super) fn new(coded_width: u32, frame_height: u32) -> Av1Result<Self> {
        if coded_width == 0 || frame_height == 0 {
            return Err(malformed("projected temporal field has invalid geometry"));
        }
        let logical_width = coded_width.div_ceil(8);
        let padded_width = coded_width
            .checked_add(127)
            .ok_or_else(|| malformed("projected temporal stride overflows"))?
            & !127;
        let stride = usize::try_from(padded_width / 8)
            .map_err(|_| malformed("projected temporal stride exceeds usize"))?;
        let height = usize::try_from(frame_height.div_ceil(8))
            .map_err(|_| malformed("projected temporal height exceeds usize"))?;
        let length = stride
            .checked_mul(height)
            .ok_or_else(|| malformed("projected temporal allocation overflows"))?;
        let mut cells = Vec::new();
        cells.try_reserve_exact(length).map_err(|_| {
            CodecError::Dimensions("unable to allocate AV1 projected temporal field".to_owned())
        })?;
        cells.resize(length, None);
        Ok(Self {
            coded_width,
            frame_height,
            stride,
            logical_width,
            cells,
        })
    }

    fn padded_index(&self, x8: u32, y8: u32) -> Option<usize> {
        let stride = u32::try_from(self.stride).ok()?;
        if x8 >= stride || y8 >= self.frame_height.div_ceil(8) {
            return None;
        }
        usize::try_from(y8)
            .ok()?
            .checked_mul(self.stride)?
            .checked_add(usize::try_from(x8).ok()?)
    }

    fn logical_index(&self, x8: u32, y8: u32) -> Option<usize> {
        if x8 >= self.logical_width {
            return None;
        }
        self.padded_index(x8, y8)
    }

    pub(super) fn get(&self, x8: u32, y8: u32) -> Option<ProjectedTemporalEntry> {
        self.cells
            .get(self.logical_index(x8, y8)?)?
            .as_ref()
            .copied()
    }

    fn get_padded(&self, x8: u32, y8: u32) -> Option<ProjectedTemporalEntry> {
        self.cells
            .get(self.padded_index(x8, y8)?)?
            .as_ref()
            .copied()
    }

    fn set_padded(
        &mut self,
        x8: u32,
        y8: u32,
        value: Option<ProjectedTemporalEntry>,
    ) -> Av1Result<()> {
        let index = self
            .padded_index(x8, y8)
            .ok_or_else(|| malformed("projected temporal write exceeds padded field"))?;
        let slot = self
            .cells
            .get_mut(index)
            .ok_or_else(|| malformed("projected temporal index exceeds allocation"))?;
        *slot = value;
        Ok(())
    }

    pub(super) fn clear_region(
        &mut self,
        col_start8: u32,
        col_end8: u32,
        row_start8: u32,
        row_end8: u32,
    ) -> Av1Result<()> {
        let end_x = col_end8.min(self.logical_width);
        let end_y = row_end8.min(self.frame_height.div_ceil(8));
        if col_start8 > end_x || row_start8 > end_y {
            return Err(malformed("projected temporal clear range is invalid"));
        }
        for y in row_start8..end_y {
            for x in col_start8..end_x {
                self.set_padded(x, y, None)?;
            }
        }
        Ok(())
    }

    pub(super) const fn coded_width(&self) -> u32 {
        self.coded_width
    }
}

/// Motion state inseparable from one retained decoded frame surface.
#[derive(Clone)]
pub(super) struct TemporalMotionField {
    coded_width: u32,
    frame_height: u32,
    stride: usize,
    cells: Vec<Option<RetainedTemporalEntry>>,
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
        let _width = coded_width.div_ceil(8);
        let height = frame_height.div_ceil(8);
        let padded_width = coded_width
            .checked_add(127)
            .ok_or_else(|| malformed("temporal motion stride overflows"))?
            & !127;
        let stride = usize::try_from(padded_width / 8)
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

    pub(super) fn set(
        &mut self,
        x8: u32,
        y8: u32,
        value: Option<RetainedTemporalEntry>,
    ) -> Av1Result<()> {
        let index = self
            .index(x8, y8)
            .ok_or_else(|| malformed("temporal motion write exceeds logical field"))?;
        let slot = self
            .cells
            .get_mut(index)
            .ok_or_else(|| malformed("temporal motion index exceeds allocation"))?;
        *slot = value;
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

fn apply_sign(value: i32, sign: i32) -> i32 {
    if sign < 0 { -value } else { value }
}

/// Load retained temporal fields into the current frame's projected 8x8
/// grid. This mirrors dav1d's load_tmvs relocation pass and deliberately
/// keeps the source MV plus its positive `ref2ref` denominator for the later
/// target-reference projection.
pub(super) fn load_projected_temporal_field(
    current_width: u32,
    current_height: u32,
    current_order_hint: u32,
    order_hint_bits: u32,
    reference_order_hints: [u32; 7],
    retained: [Option<&TemporalMotionField>; 7],
    col_start8: u32,
    col_end8: u32,
    row_start8: u32,
    row_end8: u32,
) -> Av1Result<ProjectedTemporalField> {
    let mut projected = ProjectedTemporalField::new(current_width, current_height)?;
    projected.clear_region(col_start8, col_end8, row_start8, row_end8)?;
    if order_hint_bits == 0 {
        return Ok(projected);
    }
    let mut selected = [0_usize; 3];
    let mut selected_count = 0_usize;
    let mut total = 2_usize;
    fn select_source(
        selected: &mut [usize; 3],
        selected_count: &mut usize,
        index: usize,
        total_limit: &mut usize,
    ) {
        if *selected_count < selected.len() {
            selected[*selected_count] = index;
            *selected_count += 1;
            *total_limit = (*total_limit).max(*selected_count);
        }
    }
    if retained[0].is_some()
        && retained[0]
            .is_some_and(|field| field.reference_order_hints()[6] != reference_order_hints[3])
    {
        total = 3;
        select_source(&mut selected, &mut selected_count, 0, &mut total);
    }
    for index in [4_usize, 5, 6] {
        if selected_count >= total {
            break;
        }
        if retained[index].is_some()
            && relative_distance(
                order_hint_bits,
                reference_order_hints[index],
                current_order_hint,
            ) > 0
        {
            select_source(&mut selected, &mut selected_count, index, &mut total);
        }
    }
    if selected_count < total && retained[1].is_some() {
        select_source(&mut selected, &mut selected_count, 1, &mut total);
    }

    let logical_width8 = current_width.div_ceil(8);
    let logical_height8 = current_height.div_ceil(8);
    let row_end8 = row_end8.min(logical_height8);
    let col_end8 = col_end8.min(logical_width8);
    let source_col_start = col_start8.saturating_sub(8);
    let source_col_end = col_end8.saturating_add(8).min(logical_width8);
    for source_index in selected.into_iter().take(selected_count) {
        let Some(source) = retained[source_index] else {
            continue;
        };
        let diff1 = relative_distance(order_hint_bits, source.order_hint(), current_order_hint);
        if diff1.unsigned_abs() > 31 {
            continue;
        }
        let ref2cur = if source_index < 4 { -diff1 } else { diff1 };
        let ref_sign = i32::try_from(source_index).unwrap_or(0) - 4;
        for y in row_start8..row_end8 {
            let y_sb_align = y & !7;
            let y_project_start = y_sb_align.max(row_start8);
            let y_project_end = (y_sb_align + 8).min(row_end8);
            let mut x = source_col_start;
            while x < source_col_end {
                let Some(retained_entry) = source.get(x, y) else {
                    x = x.saturating_add(1);
                    continue;
                };
                let diff2 = relative_distance(
                    order_hint_bits,
                    source.order_hint(),
                    source.reference_order_hints()[retained_entry.reference.index()],
                );
                if !(1..=31).contains(&diff2) {
                    x = x.saturating_add(1);
                    continue;
                }
                let Some(offset) = project_vector(retained_entry.vector, diff2, ref2cur) else {
                    x = x.saturating_add(1);
                    continue;
                };
                let pos_x = i64::from(x)
                    .checked_add(i64::from(apply_sign(
                        i32::from(offset.x).unsigned_abs() as i32 >> 6,
                        i32::from(offset.x) ^ ref_sign,
                    )))
                    .and_then(|value| u32::try_from(value).ok());
                let pos_y = i64::from(y)
                    .checked_add(i64::from(apply_sign(
                        i32::from(offset.y).unsigned_abs() as i32 >> 6,
                        i32::from(offset.y) ^ ref_sign,
                    )))
                    .and_then(|value| u32::try_from(value).ok());
                if let (Some(pos_x), Some(pos_y)) = (pos_x, pos_y)
                    && pos_y >= y_project_start
                    && pos_y < y_project_end
                {
                    let x_sb_align = x & !7;
                    let window_start = x_sb_align.saturating_sub(8).max(col_start8);
                    let window_end = (x_sb_align + 16).min(col_end8);
                    if pos_x >= window_start && pos_x < window_end {
                        projected.set_padded(
                            pos_x,
                            pos_y,
                            Some(ProjectedTemporalEntry {
                                vector: retained_entry.vector,
                                denominator: diff2,
                            }),
                        )?;
                    }
                }
                x = x.saturating_add(1);
            }
        }
    }
    Ok(projected)
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
