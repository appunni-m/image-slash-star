//! Normative AV1 block and transform geometry.
//!
//! Block footprints and transform footprints deliberately use separate enums:
//! AV1 blocks can reach 128×128 while one transform reaches at most 64×64,
//! and the coded coefficient window is capped at 32×32 even for larger
//! transforms. Keeping those facts separate prevents clipped frame geometry
//! or legacy coefficient carriers from becoming the semantic block model.

/// AV1 pixel layout index used by the maximum-transform table.
#[allow(
    dead_code,
    reason = "the complete layout table is consumed as monochrome and remaining chroma paths migrate to tile state"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PixelLayout {
    Monochrome = 0,
    I420 = 1,
    I422 = 2,
    I444 = 3,
}

#[allow(
    dead_code,
    reason = "the complete layout table is consumed as monochrome and remaining chroma paths migrate to tile state"
)]
impl PixelLayout {
    pub(super) const fn from_sequence(
        monochrome: bool,
        subsampling_x: bool,
        subsampling_y: bool,
    ) -> Option<Self> {
        if monochrome {
            return Some(Self::Monochrome);
        }
        match (subsampling_x, subsampling_y) {
            (true, true) => Some(Self::I420),
            (true, false) => Some(Self::I422),
            (false, false) => Some(Self::I444),
            (false, true) => None,
        }
    }
}

/// Normative AV1 authorization for extending an intra block's top and left
/// reference edges beyond its coded footprint.
///
/// Pixel coverage alone cannot establish these facts: a sample may already
/// be reconstructed but still belong to a partition decoded after the
/// current block in AV1's recursive order. Keeping the three chroma-layout
/// variants beside the block geometry lets the partition walker carry the
/// exact authorization into reconstruction without consulting prior leaves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct IntraEdgeFlags(u8);

impl IntraEdgeFlags {
    const I444_TOP_HAS_RIGHT: u8 = 1 << 0;
    const I422_TOP_HAS_RIGHT: u8 = 1 << 1;
    const I420_TOP_HAS_RIGHT: u8 = 1 << 2;
    const I444_LEFT_HAS_BOTTOM: u8 = 1 << 3;
    const I422_LEFT_HAS_BOTTOM: u8 = 1 << 4;
    const I420_LEFT_HAS_BOTTOM: u8 = 1 << 5;

    pub(super) const NONE: Self = Self(0);
    pub(super) const ALL_TOP_HAS_RIGHT: Self =
        Self(Self::I444_TOP_HAS_RIGHT | Self::I422_TOP_HAS_RIGHT | Self::I420_TOP_HAS_RIGHT);
    pub(super) const ALL_LEFT_HAS_BOTTOM: Self =
        Self(Self::I444_LEFT_HAS_BOTTOM | Self::I422_LEFT_HAS_BOTTOM | Self::I420_LEFT_HAS_BOTTOM);
    pub(super) const ALL: Self = Self(Self::ALL_TOP_HAS_RIGHT.0 | Self::ALL_LEFT_HAS_BOTTOM.0);
    pub(super) const I444_TOP: Self = Self(Self::I444_TOP_HAS_RIGHT);
    pub(super) const I420_TOP: Self = Self(Self::I420_TOP_HAS_RIGHT);
    pub(super) const I422_LEFT: Self = Self(Self::I422_LEFT_HAS_BOTTOM);
    pub(super) const I420_LEFT: Self = Self(Self::I420_LEFT_HAS_BOTTOM);

    pub(super) const fn from_availability(top_has_right: bool, left_has_bottom: bool) -> Self {
        Self(
            (if top_has_right {
                Self::ALL_TOP_HAS_RIGHT.0
            } else {
                0
            }) | (if left_has_bottom {
                Self::ALL_LEFT_HAS_BOTTOM.0
            } else {
                0
            }),
        )
    }

    pub(super) const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub(super) const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub(super) const fn select(self, selected: bool) -> Self {
        if selected { self } else { Self::NONE }
    }

    pub(super) const fn top_has_right(self, layout: PixelLayout) -> bool {
        let flag = match layout {
            PixelLayout::Monochrome | PixelLayout::I444 => Self::I444_TOP_HAS_RIGHT,
            PixelLayout::I422 => Self::I422_TOP_HAS_RIGHT,
            PixelLayout::I420 => Self::I420_TOP_HAS_RIGHT,
        };
        self.0 & flag != 0
    }

    pub(super) const fn left_has_bottom(self, layout: PixelLayout) -> bool {
        let flag = match layout {
            PixelLayout::Monochrome | PixelLayout::I444 => Self::I444_LEFT_HAS_BOTTOM,
            PixelLayout::I422 => Self::I422_LEFT_HAS_BOTTOM,
            PixelLayout::I420 => Self::I420_LEFT_HAS_BOTTOM,
        };
        self.0 & flag != 0
    }
}

/// All 22 normative AV1 coded block sizes, in specification order.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BlockSize {
    B4x4 = 0,
    B4x8 = 1,
    B8x4 = 2,
    B8x8 = 3,
    B8x16 = 4,
    B16x8 = 5,
    B16x16 = 6,
    B16x32 = 7,
    B32x16 = 8,
    B32x32 = 9,
    B32x64 = 10,
    B64x32 = 11,
    B64x64 = 12,
    B64x128 = 13,
    B128x64 = 14,
    B128x128 = 15,
    B4x16 = 16,
    B16x4 = 17,
    B8x32 = 18,
    B32x8 = 19,
    B16x64 = 20,
    B64x16 = 21,
}

#[allow(
    dead_code,
    reason = "block-size consumers migrate incrementally from the legacy TransformGrid adapter"
)]
impl BlockSize {
    pub(super) const ALL: [Self; 22] = [
        Self::B4x4,
        Self::B4x8,
        Self::B8x4,
        Self::B8x8,
        Self::B8x16,
        Self::B16x8,
        Self::B16x16,
        Self::B16x32,
        Self::B32x16,
        Self::B32x32,
        Self::B32x64,
        Self::B64x32,
        Self::B64x64,
        Self::B64x128,
        Self::B128x64,
        Self::B128x128,
        Self::B4x16,
        Self::B16x4,
        Self::B8x32,
        Self::B32x8,
        Self::B16x64,
        Self::B64x16,
    ];

    /// Construct from nominal four-pixel units, never from a clipped extent.
    pub(super) const fn from_mi_dimensions(width: u32, height: u32) -> Option<Self> {
        match (width, height) {
            (1, 1) => Some(Self::B4x4),
            (1, 2) => Some(Self::B4x8),
            (2, 1) => Some(Self::B8x4),
            (2, 2) => Some(Self::B8x8),
            (2, 4) => Some(Self::B8x16),
            (4, 2) => Some(Self::B16x8),
            (4, 4) => Some(Self::B16x16),
            (4, 8) => Some(Self::B16x32),
            (8, 4) => Some(Self::B32x16),
            (8, 8) => Some(Self::B32x32),
            (8, 16) => Some(Self::B32x64),
            (16, 8) => Some(Self::B64x32),
            (16, 16) => Some(Self::B64x64),
            (16, 32) => Some(Self::B64x128),
            (32, 16) => Some(Self::B128x64),
            (32, 32) => Some(Self::B128x128),
            (1, 4) => Some(Self::B4x16),
            (4, 1) => Some(Self::B16x4),
            (2, 8) => Some(Self::B8x32),
            (8, 2) => Some(Self::B32x8),
            (4, 16) => Some(Self::B16x64),
            (16, 4) => Some(Self::B64x16),
            _ => None,
        }
    }

    pub(super) const fn mi_dimensions(self) -> (u32, u32) {
        match self {
            Self::B4x4 => (1, 1),
            Self::B4x8 => (1, 2),
            Self::B8x4 => (2, 1),
            Self::B8x8 => (2, 2),
            Self::B8x16 => (2, 4),
            Self::B16x8 => (4, 2),
            Self::B16x16 => (4, 4),
            Self::B16x32 => (4, 8),
            Self::B32x16 => (8, 4),
            Self::B32x32 => (8, 8),
            Self::B32x64 => (8, 16),
            Self::B64x32 => (16, 8),
            Self::B64x64 => (16, 16),
            Self::B64x128 => (16, 32),
            Self::B128x64 => (32, 16),
            Self::B128x128 => (32, 32),
            Self::B4x16 => (1, 4),
            Self::B16x4 => (4, 1),
            Self::B8x32 => (2, 8),
            Self::B32x8 => (8, 2),
            Self::B16x64 => (4, 16),
            Self::B64x16 => (16, 4),
        }
    }

    pub(super) const fn pixel_dimensions(self) -> (u32, u32) {
        let (width, height) = self.mi_dimensions();
        (width.wrapping_mul(4), height.wrapping_mul(4))
    }

    /// dav1d's reverse block-size index used by block-size CDF tables.
    pub(super) const fn cdf_index(self) -> usize {
        match self {
            Self::B128x128 => 0,
            Self::B128x64 => 1,
            Self::B64x128 => 2,
            Self::B64x64 => 3,
            Self::B64x32 => 4,
            Self::B64x16 => 5,
            Self::B32x64 => 6,
            Self::B32x32 => 7,
            Self::B32x16 => 8,
            Self::B32x8 => 9,
            Self::B16x64 => 10,
            Self::B16x32 => 11,
            Self::B16x16 => 12,
            Self::B16x8 => 13,
            Self::B16x4 => 14,
            Self::B8x32 => 15,
            Self::B8x16 => 16,
            Self::B8x8 => 17,
            Self::B8x4 => 18,
            Self::B4x16 => 19,
            Self::B4x8 => 20,
            Self::B4x4 => 21,
        }
    }

    /// Whether this nominal coded block carries AV1 palette syntax.
    ///
    /// The predicate is intentionally independent of the visible crop: a
    /// standalone 4x4 image is coded as B8x8 and therefore remains eligible.
    pub(super) const fn palette_allowed(self) -> bool {
        let (width, height) = self.pixel_dimensions();
        width >= 8 && width <= 64 && height >= 8 && height <= 64
    }

    /// Palette-size CDF row for one palette-eligible coded block.
    pub(super) const fn palette_size_context(self) -> Option<usize> {
        if !self.palette_allowed() {
            return None;
        }
        let (width, height) = self.mi_dimensions();
        Some(
            width
                .ilog2()
                .saturating_add(height.ilog2())
                .saturating_sub(2) as usize,
        )
    }

    /// Normative maximum luma transform.
    pub(super) const fn maximum_luma_tx(self) -> TxSize {
        MAX_TX_FOR_BLOCK[self as usize][PixelLayout::Monochrome as usize]
    }

    /// Normative maximum chroma transform for a color pixel layout.
    pub(super) const fn maximum_chroma_tx(self, layout: PixelLayout) -> Option<TxSize> {
        match layout {
            PixelLayout::Monochrome => None,
            PixelLayout::I420 | PixelLayout::I422 | PixelLayout::I444 => {
                Some(MAX_TX_FOR_BLOCK[self as usize][layout as usize])
            }
        }
    }
}

/// All 19 normative AV1 transform sizes.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TxSize {
    Tx4x4 = 0,
    Tx8x8 = 1,
    Tx16x16 = 2,
    Tx32x32 = 3,
    Tx64x64 = 4,
    Tx4x8 = 5,
    Tx8x4 = 6,
    Tx8x16 = 7,
    Tx16x8 = 8,
    Tx16x32 = 9,
    Tx32x16 = 10,
    Tx32x64 = 11,
    Tx64x32 = 12,
    Tx4x16 = 13,
    Tx16x4 = 14,
    Tx8x32 = 15,
    Tx32x8 = 16,
    Tx16x64 = 17,
    Tx64x16 = 18,
}

#[allow(
    dead_code,
    reason = "semantic transform state is consumed incrementally while legacy coefficient carriers remain compatible"
)]
impl TxSize {
    pub(super) const ALL: [Self; 19] = [
        Self::Tx4x4,
        Self::Tx8x8,
        Self::Tx16x16,
        Self::Tx32x32,
        Self::Tx64x64,
        Self::Tx4x8,
        Self::Tx8x4,
        Self::Tx8x16,
        Self::Tx16x8,
        Self::Tx16x32,
        Self::Tx32x16,
        Self::Tx32x64,
        Self::Tx64x32,
        Self::Tx4x16,
        Self::Tx16x4,
        Self::Tx8x32,
        Self::Tx32x8,
        Self::Tx16x64,
        Self::Tx64x16,
    ];

    pub(super) const fn from_mi_dimensions(width: u32, height: u32) -> Option<Self> {
        match (width, height) {
            (1, 1) => Some(Self::Tx4x4),
            (2, 2) => Some(Self::Tx8x8),
            (4, 4) => Some(Self::Tx16x16),
            (8, 8) => Some(Self::Tx32x32),
            (16, 16) => Some(Self::Tx64x64),
            (1, 2) => Some(Self::Tx4x8),
            (2, 1) => Some(Self::Tx8x4),
            (2, 4) => Some(Self::Tx8x16),
            (4, 2) => Some(Self::Tx16x8),
            (4, 8) => Some(Self::Tx16x32),
            (8, 4) => Some(Self::Tx32x16),
            (8, 16) => Some(Self::Tx32x64),
            (16, 8) => Some(Self::Tx64x32),
            (1, 4) => Some(Self::Tx4x16),
            (4, 1) => Some(Self::Tx16x4),
            (2, 8) => Some(Self::Tx8x32),
            (8, 2) => Some(Self::Tx32x8),
            (4, 16) => Some(Self::Tx16x64),
            (16, 4) => Some(Self::Tx64x16),
            _ => None,
        }
    }

    pub(super) const fn mi_dimensions(self) -> (u32, u32) {
        match self {
            Self::Tx4x4 => (1, 1),
            Self::Tx8x8 => (2, 2),
            Self::Tx16x16 => (4, 4),
            Self::Tx32x32 => (8, 8),
            Self::Tx64x64 => (16, 16),
            Self::Tx4x8 => (1, 2),
            Self::Tx8x4 => (2, 1),
            Self::Tx8x16 => (2, 4),
            Self::Tx16x8 => (4, 2),
            Self::Tx16x32 => (4, 8),
            Self::Tx32x16 => (8, 4),
            Self::Tx32x64 => (8, 16),
            Self::Tx64x32 => (16, 8),
            Self::Tx4x16 => (1, 4),
            Self::Tx16x4 => (4, 1),
            Self::Tx8x32 => (2, 8),
            Self::Tx32x8 => (8, 2),
            Self::Tx16x64 => (4, 16),
            Self::Tx64x16 => (16, 4),
        }
    }

    pub(super) const fn pixel_dimensions(self) -> (u32, u32) {
        let (width, height) = self.mi_dimensions();
        (width.wrapping_mul(4), height.wrapping_mul(4))
    }

    /// Compact coefficient window; AV1 caps each coded axis at 32 samples.
    pub(super) const fn coefficient_dimensions(self) -> (u32, u32) {
        let (width, height) = self.pixel_dimensions();
        (
            if width < 32 { width } else { 32 },
            if height < 32 { height } else { 32 },
        )
    }

    /// Transform exposed by one normative subdivision step.
    pub(super) const fn sub_size(self) -> Self {
        match self {
            Self::Tx4x4 => Self::Tx4x4,
            Self::Tx8x8 => Self::Tx4x4,
            Self::Tx16x16 => Self::Tx8x8,
            Self::Tx32x32 => Self::Tx16x16,
            Self::Tx64x64 => Self::Tx32x32,
            Self::Tx4x8 | Self::Tx8x4 => Self::Tx4x4,
            Self::Tx8x16 | Self::Tx16x8 => Self::Tx8x8,
            Self::Tx16x32 | Self::Tx32x16 => Self::Tx16x16,
            Self::Tx32x64 | Self::Tx64x32 => Self::Tx32x32,
            Self::Tx4x16 => Self::Tx4x8,
            Self::Tx16x4 => Self::Tx8x4,
            Self::Tx8x32 => Self::Tx8x16,
            Self::Tx32x8 => Self::Tx16x8,
            Self::Tx16x64 => Self::Tx16x32,
            Self::Tx64x16 => Self::Tx32x16,
        }
    }

    /// Width/height transform contexts stored on the four-pixel tile grid.
    pub(super) const fn context_dimensions(self) -> (u8, u8) {
        let (width, height) = self.mi_dimensions();
        (context_dimension(width), context_dimension(height))
    }
}

const fn context_dimension(units: u32) -> u8 {
    match units {
        1 => 0,
        2 => 1,
        4 => 2,
        8 => 3,
        16 => 4,
        _ => 0,
    }
}

use TxSize::{
    Tx4x4, Tx4x8, Tx4x16, Tx8x4, Tx8x8, Tx8x16, Tx8x32, Tx16x4, Tx16x8, Tx16x16, Tx16x32, Tx16x64,
    Tx32x8, Tx32x16, Tx32x32, Tx32x64, Tx64x16, Tx64x32, Tx64x64,
};

/// dav1d 0.5.7 `dav1d_max_txfm_size_for_bs`, reordered to AV1 block IDs.
const MAX_TX_FOR_BLOCK: [[TxSize; 4]; 22] = [
    [Tx4x4, Tx4x4, Tx4x4, Tx4x4],
    [Tx4x8, Tx4x4, Tx4x4, Tx4x8],
    [Tx8x4, Tx4x4, Tx4x4, Tx8x4],
    [Tx8x8, Tx4x4, Tx4x8, Tx8x8],
    [Tx8x16, Tx4x8, Tx4x4, Tx8x16],
    [Tx16x8, Tx8x4, Tx8x8, Tx16x8],
    [Tx16x16, Tx8x8, Tx8x16, Tx16x16],
    [Tx16x32, Tx8x16, Tx4x4, Tx16x32],
    [Tx32x16, Tx16x8, Tx16x16, Tx32x16],
    [Tx32x32, Tx16x16, Tx16x32, Tx32x32],
    [Tx32x64, Tx16x32, Tx4x4, Tx32x32],
    [Tx64x32, Tx32x16, Tx32x32, Tx32x32],
    [Tx64x64, Tx32x32, Tx32x32, Tx32x32],
    [Tx64x64, Tx32x32, Tx4x4, Tx32x32],
    [Tx64x64, Tx32x32, Tx32x32, Tx32x32],
    [Tx64x64, Tx32x32, Tx32x32, Tx32x32],
    [Tx4x16, Tx4x8, Tx4x4, Tx4x16],
    [Tx16x4, Tx8x4, Tx8x4, Tx16x4],
    [Tx8x32, Tx4x16, Tx4x4, Tx8x32],
    [Tx32x8, Tx16x4, Tx16x8, Tx32x8],
    [Tx16x64, Tx8x32, Tx4x4, Tx16x32],
    [Tx64x16, Tx32x8, Tx32x16, Tx32x16],
];
