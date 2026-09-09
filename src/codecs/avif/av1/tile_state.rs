//! Compact tile-local ownership and entropy context state.
//!
//! Pixels live in the padded [`super::raster::FrameCanvas`]. This module keeps
//! only stable scalar block metadata and per-4×4-cell state, so decoded leaves
//! can be dropped after placement and neighbor lookup never scans frame
//! history.

#![allow(
    dead_code,
    reason = "inter and variable-transform metadata is wired incrementally"
)]

use std::num::NonZeroU32;

use super::block::{ChromaPredictor, FirstLeaf, LumaPredictor, PaletteCacheState};
use super::entropy::PartitionNode;
use super::geometry::{BlockSize, TxSize};
use super::motion::{
    CompoundType, InterMode, InterpolationFilter, MotionMode, MotionVector, ReferenceFrame,
    ReferencePair, RetainedTemporalEntry, SpatialMotionSource, SpatialRefBlock,
};
use super::{Av1Result, malformed};
use crate::codecs::CodecError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OwnerId(NonZeroU32);

impl OwnerId {
    fn from_index(index: usize) -> Av1Result<Self> {
        let value = index
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .and_then(NonZeroU32::new)
            .ok_or_else(|| malformed("tile block owner index exceeds u32"))?;
        Ok(Self(value))
    }

    pub(super) fn index(self) -> Option<usize> {
        usize::try_from(self.0.get().checked_sub(1)?).ok()
    }
}

#[derive(Clone, Copy)]
struct TileCell {
    owner: Option<OwnerId>,
    right_context: u8,
    bottom_context: u8,
    tx_context_width: u8,
    tx_context_height: u8,
    segment_id: u8,
    segment_pred: bool,
    skip: bool,
    skip_mode: bool,
    intra: bool,
}

#[derive(Clone, Copy)]
struct ChromaCell {
    owner: Option<OwnerId>,
    right_contexts: [u8; 2],
    bottom_contexts: [u8; 2],
}

impl Default for ChromaCell {
    fn default() -> Self {
        Self {
            owner: None,
            right_contexts: [0x40; 2],
            bottom_contexts: [0x40; 2],
        }
    }
}

impl Default for TileCell {
    fn default() -> Self {
        Self {
            owner: None,
            right_context: 0x40,
            bottom_context: 0x40,
            tx_context_width: 0,
            tx_context_height: 0,
            segment_id: 0,
            segment_pred: false,
            skip: false,
            skip_mode: false,
            intra: true,
        }
    }
}

/// Stable inter syntax used by MV scans, OBMC, sub-8 chroma, and filtering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InterBlockMeta {
    pub(super) references: ReferencePair,
    pub(super) motion_vectors: [MotionVector; 2],
    pub(super) mode: InterMode,
    pub(super) compound_type: CompoundType,
    pub(super) motion_mode: MotionMode,
    /// Horizontal then vertical filter, matching bitstream syntax order.
    pub(super) filters: [InterpolationFilter; 2],
    pub(super) global: [bool; 2],
    pub(super) new_mv: [bool; 2],
}

/// Reconstruction class published by one complete block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BlockCoding {
    Intra,
    IntraBc { motion_vector: MotionVector },
    Inter(InterBlockMeta),
}

/// Per-MI transform dimensions published by a generalized block commit. The
/// existing intra leaf path uses one uniform value; variable-transform blocks
/// provide one entry for every coded 4x4 cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TxCellUpdate {
    pub(super) width_log2: u8,
    pub(super) height_log2: u8,
}

#[derive(Clone, Copy)]
pub(super) struct BlockCommitMetadata<'a> {
    pub(super) coding: BlockCoding,
    pub(super) skip_mode: bool,
    pub(super) tx_cells: Option<&'a [TxCellUpdate]>,
}

impl BlockCommitMetadata<'_> {
    pub(super) const fn intra() -> Self {
        Self {
            coding: BlockCoding::Intra,
            skip_mode: false,
            tx_cells: None,
        }
    }
}

impl BlockCoding {
    pub(super) const fn is_intra(self) -> bool {
        matches!(self, Self::Intra)
    }

    pub(super) const fn inter(self) -> Option<InterBlockMeta> {
        match self {
            Self::Inter(inter) => Some(inter),
            Self::Intra | Self::IntraBc { .. } => None,
        }
    }
}

/// Stable scalar metadata for one successfully reconstructed block.
#[derive(Clone, Copy)]
pub(super) struct DecodedBlockMeta {
    pub(super) origin_x: u32,
    pub(super) origin_y: u32,
    pub(super) block_size: BlockSize,
    pub(super) entropy_width: u32,
    pub(super) entropy_height: u32,
    pub(super) luma_predictor: LumaPredictor,
    pub(super) chroma_predictor: Option<ChromaPredictor>,
    pub(super) palette_cache: PaletteCacheState,
    pub(super) has_chroma: bool,
    pub(super) tx_context_width: u8,
    pub(super) tx_context_height: u8,
    pub(super) block_skipped: bool,
    pub(super) skip_mode: bool,
    pub(super) coding: BlockCoding,
}

/// Copy-only neighbor facts consumed while decoding a following block.
///
/// Pixel samples remain exclusively in `FrameCanvas`; the owner identity is
/// retained so equality is structural and never depends on allocation or
/// pointer reuse.
#[derive(Clone, Copy)]
pub(super) struct NeighborMeta {
    pub(super) owner: OwnerId,
    pub(super) origin_x: u32,
    pub(super) origin_y: u32,
    pub(super) block_size: BlockSize,
    pub(super) pixel_width: u32,
    pub(super) pixel_height: u32,
    pub(super) luma_predictor: LumaPredictor,
    pub(super) chroma_predictor: Option<ChromaPredictor>,
    pub(super) has_chroma: bool,
    pub(super) tx_context_width: u8,
    pub(super) tx_context_height: u8,
    pub(super) block_skipped: bool,
    pub(super) skip_mode: bool,
    pub(super) coding: BlockCoding,
}

impl NeighborMeta {
    fn from_block(owner: OwnerId, block: &DecodedBlockMeta) -> Av1Result<Self> {
        Ok(Self {
            owner,
            origin_x: block.origin_x,
            origin_y: block.origin_y,
            block_size: block.block_size,
            pixel_width: block
                .entropy_width
                .checked_mul(4)
                .ok_or_else(|| malformed("neighbor pixel width overflows"))?,
            pixel_height: block
                .entropy_height
                .checked_mul(4)
                .ok_or_else(|| malformed("neighbor pixel height overflows"))?,
            luma_predictor: block.luma_predictor,
            chroma_predictor: block.chroma_predictor,
            has_chroma: block.has_chroma,
            tx_context_width: block.tx_context_width,
            tx_context_height: block.tx_context_height,
            block_skipped: block.block_skipped,
            skip_mode: block.skip_mode,
            coding: block.coding,
        })
    }
}

/// Per-cell contexts that cannot be reconstructed from one block-wide value
/// after a variable-transform tree splits differently along an edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CellContexts {
    pub(super) tx_width: u8,
    pub(super) tx_height: u8,
    pub(super) skip: bool,
    pub(super) skip_mode: bool,
    pub(super) intra: bool,
}

/// Bounded O(1)-indexed state for one tile's padded four-pixel grid.
pub(super) struct TileState {
    width: usize,
    height: usize,
    subsampling_x: bool,
    subsampling_y: bool,
    cells: Vec<TileCell>,
    chroma_width: usize,
    chroma_height: usize,
    chroma_cells: Vec<ChromaCell>,
    blocks: Vec<DecodedBlockMeta>,
}

impl TileState {
    pub(super) fn new(
        width: u32,
        height: u32,
        subsampling_x: bool,
        subsampling_y: bool,
    ) -> Av1Result<Self> {
        let width = usize::try_from(width).map_err(|_| malformed("tile width exceeds usize"))?;
        let height = usize::try_from(height).map_err(|_| malformed("tile height exceeds usize"))?;
        let cell_count = width
            .checked_mul(height)
            .filter(|&count| count != 0)
            .ok_or_else(|| malformed("tile state has an invalid extent"))?;
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(cell_count)
            .map_err(|_| CodecError::Dimensions("unable to allocate AV1 tile cells".to_owned()))?;
        cells.resize(cell_count, TileCell::default());
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
        let chroma_cell_count = chroma_width
            .checked_mul(chroma_height)
            .filter(|&count| count != 0)
            .ok_or_else(|| malformed("tile chroma state has an invalid extent"))?;
        let mut chroma_cells = Vec::new();
        chroma_cells
            .try_reserve_exact(chroma_cell_count)
            .map_err(|_| {
                CodecError::Dimensions("unable to allocate AV1 chroma tile cells".to_owned())
            })?;
        chroma_cells.resize(chroma_cell_count, ChromaCell::default());
        Ok(Self {
            width,
            height,
            subsampling_x,
            subsampling_y,
            cells,
            chroma_width,
            chroma_height,
            chroma_cells,
            blocks: Vec::new(),
        })
    }

    pub(super) fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub(super) fn block(&self, owner: OwnerId) -> Option<&DecodedBlockMeta> {
        self.blocks.get(owner.index()?)
    }

    pub(super) fn owner_at(&self, x: u32, y: u32) -> Option<OwnerId> {
        let x = usize::try_from(x).ok()?;
        let y = usize::try_from(y).ok()?;
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = y.checked_mul(self.width)?.checked_add(x)?;
        self.cells.get(index)?.owner
    }

    pub(super) fn segment_at(&self, x: u32, y: u32) -> Option<(u8, bool)> {
        let x = usize::try_from(x).ok()?;
        let y = usize::try_from(y).ok()?;
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = y.checked_mul(self.width)?.checked_add(x)?;
        self.cells
            .get(index)
            .map(|cell| (cell.segment_id, cell.segment_pred))
    }

    pub(super) fn contexts_at(&self, x: u32, y: u32) -> Option<CellContexts> {
        let x = usize::try_from(x).ok()?;
        let y = usize::try_from(y).ok()?;
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = y.checked_mul(self.width)?.checked_add(x)?;
        let cell = self.cells.get(index)?;
        cell.owner?;
        Some(CellContexts {
            tx_width: cell.tx_context_width,
            tx_height: cell.tx_context_height,
            skip: cell.skip,
            skip_mode: cell.skip_mode,
            intra: cell.intra,
        })
    }

    /// Return transform-size context dimensions for a neighboring cell.
    ///
    /// A skipped inter block suppresses its transform syntax, so its
    /// transform context is the block's normative maximum rather than a
    /// stale per-cell value left by a smaller terminal or a clipped grid.
    /// Intra and non-skipped cells retain the exact dimensions published by
    /// their decoded transform plan.
    pub(super) fn transform_contexts_at(&self, x: u32, y: u32) -> Option<(u8, u8)> {
        let x = usize::try_from(x).ok()?;
        let y = usize::try_from(y).ok()?;
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = y.checked_mul(self.width)?.checked_add(x)?;
        let cell = self.cells.get(index)?;
        let owner = cell.owner?;
        if cell.skip && !cell.intra {
            return Some(
                self.block(owner)?
                    .block_size
                    .maximum_luma_tx()
                    .context_dimensions(),
            );
        }
        Some((cell.tx_context_width, cell.tx_context_height))
    }

    pub(super) fn block_at(&self, x: u32, y: u32) -> Option<(OwnerId, &DecodedBlockMeta)> {
        let owner = self.owner_at(x, y)?;
        Some((owner, self.block(owner)?))
    }

    pub(super) fn block_at_checked(
        &self,
        x: u32,
        y: u32,
    ) -> Av1Result<Option<(OwnerId, &DecodedBlockMeta)>> {
        let Some(owner) = self.owner_at(x, y) else {
            return Ok(None);
        };
        let block = self
            .block(owner)
            .ok_or_else(|| malformed("tile owner has no block metadata"))?;
        Ok(Some((owner, block)))
    }

    pub(super) fn neighbor_at_checked(&self, x: u32, y: u32) -> Av1Result<Option<NeighborMeta>> {
        self.block_at_checked(x, y)?
            .map(|(owner, block)| NeighborMeta::from_block(owner, block))
            .transpose()
    }

    pub(super) fn chroma_owner_at_luma(&self, x: u32, y: u32) -> Option<OwnerId> {
        let x = usize::try_from(x).ok()?;
        let y = usize::try_from(y).ok()?;
        if x >= self.width || y >= self.height {
            return None;
        }
        let chroma_x = if self.subsampling_x { x / 2 } else { x };
        let chroma_y = if self.subsampling_y { y / 2 } else { y };
        let index = chroma_y
            .checked_mul(self.chroma_width)?
            .checked_add(chroma_x)?;
        self.chroma_cells.get(index)?.owner
    }

    pub(super) fn chroma_block_at_luma_checked(
        &self,
        x: u32,
        y: u32,
    ) -> Av1Result<Option<(OwnerId, &DecodedBlockMeta)>> {
        let Some(owner) = self.chroma_owner_at_luma(x, y) else {
            return Ok(None);
        };
        let block = self
            .block(owner)
            .ok_or_else(|| malformed("tile chroma owner has no block metadata"))?;
        if !block.has_chroma {
            return Err(malformed("tile chroma owner does not publish chroma state"));
        }
        Ok(Some((owner, block)))
    }

    pub(super) fn chroma_neighbor_at_luma_checked(
        &self,
        x: u32,
        y: u32,
    ) -> Av1Result<Option<NeighborMeta>> {
        self.chroma_block_at_luma_checked(x, y)?
            .map(|(owner, block)| NeighborMeta::from_block(owner, block))
            .transpose()
    }

    pub(super) fn chroma_contexts_above<const COUNT: usize>(
        &self,
        plane: usize,
        x: u32,
        y: u32,
        needed: u32,
    ) -> Av1Result<[u8; COUNT]> {
        let mut contexts = [0x40; COUNT];
        if plane >= 2 {
            return Err(malformed("chroma above-context plane exceeds two"));
        }
        let needed = usize::try_from(needed)
            .map_err(|_| malformed("chroma above-context extent exceeds usize"))?;
        if needed > COUNT {
            return Err(malformed("chroma above-context extent exceeds scratch"));
        }
        let x =
            usize::try_from(x).map_err(|_| malformed("chroma above-context x exceeds usize"))?;
        let y =
            usize::try_from(y).map_err(|_| malformed("chroma above-context y exceeds usize"))?;
        if y > self.chroma_height
            || x.checked_add(needed)
                .is_none_or(|end_x| end_x > self.chroma_width)
        {
            return Err(malformed("chroma above-context query exceeds state grid"));
        }
        let Some(above_y) = y.checked_sub(1) else {
            return Ok(contexts);
        };
        let row = above_y
            .checked_mul(self.chroma_width)
            .ok_or_else(|| malformed("chroma above-context row overflows"))?;
        for (offset, context) in contexts.iter_mut().take(needed).enumerate() {
            let index = row
                .checked_add(x)
                .and_then(|index| index.checked_add(offset))
                .ok_or_else(|| malformed("chroma above-context index overflows"))?;
            if let Some(cell) = self.chroma_cells.get(index)
                && cell.owner.is_some()
            {
                *context = cell.bottom_contexts[plane];
            }
        }
        Ok(contexts)
    }

    pub(super) fn chroma_contexts_left<const COUNT: usize>(
        &self,
        plane: usize,
        x: u32,
        y: u32,
        needed: u32,
    ) -> Av1Result<[u8; COUNT]> {
        let mut contexts = [0x40; COUNT];
        if plane >= 2 {
            return Err(malformed("chroma left-context plane exceeds two"));
        }
        let needed = usize::try_from(needed)
            .map_err(|_| malformed("chroma left-context extent exceeds usize"))?;
        if needed > COUNT {
            return Err(malformed("chroma left-context extent exceeds scratch"));
        }
        let x = usize::try_from(x).map_err(|_| malformed("chroma left-context x exceeds usize"))?;
        let y = usize::try_from(y).map_err(|_| malformed("chroma left-context y exceeds usize"))?;
        if x > self.chroma_width
            || y.checked_add(needed)
                .is_none_or(|end_y| end_y > self.chroma_height)
        {
            return Err(malformed("chroma left-context query exceeds state grid"));
        }
        let Some(left_x) = x.checked_sub(1) else {
            return Ok(contexts);
        };
        for (offset, context) in contexts.iter_mut().take(needed).enumerate() {
            let index = y
                .checked_add(offset)
                .and_then(|row| row.checked_mul(self.chroma_width))
                .and_then(|row| row.checked_add(left_x))
                .ok_or_else(|| malformed("chroma left-context index overflows"))?;
            if let Some(cell) = self.chroma_cells.get(index)
                && cell.owner.is_some()
            {
                *context = cell.right_contexts[plane];
            }
        }
        Ok(contexts)
    }

    pub(super) fn luma_contexts_above<const COUNT: usize>(
        &self,
        x: u32,
        y: u32,
        needed: u32,
    ) -> Av1Result<[u8; COUNT]> {
        let mut contexts = [0x40; COUNT];
        let needed = usize::try_from(needed)
            .map_err(|_| malformed("luma above-context extent exceeds usize"))?;
        if needed > COUNT {
            return Err(malformed("luma above-context extent exceeds scratch"));
        }
        let x = usize::try_from(x).map_err(|_| malformed("luma above-context x exceeds usize"))?;
        let y = usize::try_from(y).map_err(|_| malformed("luma above-context y exceeds usize"))?;
        if y > self.height || x.checked_add(needed).is_none_or(|end_x| end_x > self.width) {
            return Err(malformed("luma above-context query exceeds state grid"));
        }
        let Some(above_y) = y.checked_sub(1) else {
            return Ok(contexts);
        };
        let row = above_y
            .checked_mul(self.width)
            .ok_or_else(|| malformed("luma above-context row overflows"))?;
        for (offset, context) in contexts.iter_mut().take(needed).enumerate() {
            let index = row
                .checked_add(x)
                .and_then(|index| index.checked_add(offset))
                .ok_or_else(|| malformed("luma above-context index overflows"))?;
            if let Some(cell) = self.cells.get(index)
                && cell.owner.is_some()
            {
                *context = cell.bottom_context;
            }
        }
        Ok(contexts)
    }

    pub(super) fn luma_contexts_left<const COUNT: usize>(
        &self,
        x: u32,
        y: u32,
        needed: u32,
    ) -> Av1Result<[u8; COUNT]> {
        let mut contexts = [0x40; COUNT];
        let needed = usize::try_from(needed)
            .map_err(|_| malformed("luma left-context extent exceeds usize"))?;
        if needed > COUNT {
            return Err(malformed("luma left-context extent exceeds scratch"));
        }
        let x = usize::try_from(x).map_err(|_| malformed("luma left-context x exceeds usize"))?;
        let y = usize::try_from(y).map_err(|_| malformed("luma left-context y exceeds usize"))?;
        if x > self.width
            || y.checked_add(needed)
                .is_none_or(|end_y| end_y > self.height)
        {
            return Err(malformed("luma left-context query exceeds state grid"));
        }
        let Some(left_x) = x.checked_sub(1) else {
            return Ok(contexts);
        };
        for (offset, context) in contexts.iter_mut().take(needed).enumerate() {
            let row = y
                .checked_add(offset)
                .and_then(|row| row.checked_mul(self.width))
                .ok_or_else(|| malformed("luma left-context row overflows"))?;
            let index = row
                .checked_add(left_x)
                .ok_or_else(|| malformed("luma left-context index overflows"))?;
            if let Some(cell) = self.cells.get(index)
                && cell.owner.is_some()
            {
                *context = cell.right_context;
            }
        }
        Ok(contexts)
    }

    /// Publish one block only after entropy, reconstruction, validation, and
    /// padded-canvas placement have all succeeded.
    pub(super) fn commit(
        &mut self,
        node: PartitionNode,
        has_chroma: bool,
        leaf: &FirstLeaf,
        segment_id: u8,
        segment_pred: bool,
    ) -> Av1Result<OwnerId> {
        self.commit_with_metadata(
            node,
            has_chroma,
            leaf,
            segment_id,
            segment_pred,
            BlockCommitMetadata::intra(),
        )
    }

    pub(super) fn commit_with_metadata(
        &mut self,
        node: PartitionNode,
        has_chroma: bool,
        leaf: &FirstLeaf,
        segment_id: u8,
        segment_pred: bool,
        metadata: BlockCommitMetadata<'_>,
    ) -> Av1Result<OwnerId> {
        let (coded_width, coded_height) = node.block_size.mi_dimensions();
        if coded_width != node.coded_width
            || coded_height != node.coded_height
            || node.width == 0
            || node.height == 0
            || node.width > coded_width
            || node.height > coded_height
        {
            return Err(malformed("tile block geometry is inconsistent"));
        }
        let end_x = node
            .x
            .checked_add(node.width)
            .ok_or_else(|| malformed("tile block x extent overflows"))?;
        let end_y = node
            .y
            .checked_add(node.height)
            .ok_or_else(|| malformed("tile block y extent overflows"))?;
        let end_x_usize =
            usize::try_from(end_x).map_err(|_| malformed("tile block x extent exceeds usize"))?;
        let end_y_usize =
            usize::try_from(end_y).map_err(|_| malformed("tile block y extent exceeds usize"))?;
        if end_x_usize > self.width || end_y_usize > self.height {
            return Err(malformed("tile block exceeds the padded tile grid"));
        }

        let owner = OwnerId::from_index(self.blocks.len())?;
        for y in node.y..end_y {
            for x in node.x..end_x {
                let index = self.cell_index(x, y)?;
                if self
                    .cells
                    .get(index)
                    .is_none_or(|cell| cell.owner.is_some())
                {
                    return Err(malformed("tile blocks overlap"));
                }
            }
        }

        let chroma_geometry = if has_chroma {
            let chroma_x = if self.subsampling_x {
                node.x / 2
            } else {
                node.x
            };
            let chroma_y = if self.subsampling_y {
                node.y / 2
            } else {
                node.y
            };
            let chroma_width = if self.subsampling_x {
                node.width.div_ceil(2)
            } else {
                node.width
            };
            let chroma_height = if self.subsampling_y {
                node.height.div_ceil(2)
            } else {
                node.height
            };
            let chroma_end_x = chroma_x
                .checked_add(chroma_width)
                .ok_or_else(|| malformed("tile chroma block x extent overflows"))?;
            let chroma_end_y = chroma_y
                .checked_add(chroma_height)
                .ok_or_else(|| malformed("tile chroma block y extent overflows"))?;
            let chroma_end_x_usize = usize::try_from(chroma_end_x)
                .map_err(|_| malformed("tile chroma block x extent exceeds usize"))?;
            let chroma_end_y_usize = usize::try_from(chroma_end_y)
                .map_err(|_| malformed("tile chroma block y extent exceeds usize"))?;
            if chroma_end_x_usize > self.chroma_width || chroma_end_y_usize > self.chroma_height {
                return Err(malformed("tile chroma block exceeds the padded state grid"));
            }
            for y in chroma_y..chroma_end_y {
                for x in chroma_x..chroma_end_x {
                    let index = self.chroma_cell_index(x, y)?;
                    if self
                        .chroma_cells
                        .get(index)
                        .is_none_or(|cell| cell.owner.is_some())
                    {
                        return Err(malformed("tile chroma blocks overlap"));
                    }
                }
            }
            Some((chroma_x, chroma_y, chroma_end_x, chroma_end_y))
        } else {
            None
        };

        let tx_width = 1_u32
            .checked_shl(u32::from(leaf.tx_context_width))
            .ok_or_else(|| malformed("luma transform width context overflows"))?;
        let tx_height = 1_u32
            .checked_shl(u32::from(leaf.tx_context_height))
            .ok_or_else(|| malformed("luma transform height context overflows"))?;
        TxSize::from_mi_dimensions(tx_width, tx_height)
            .ok_or_else(|| malformed("leaf publishes a non-normative transform size"))?;
        let cell_count = usize::try_from(node.width)
            .ok()
            .and_then(|width| {
                usize::try_from(node.height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .ok_or_else(|| malformed("block transform cell count overflows"))?;
        if let Some(tx_cells) = metadata.tx_cells {
            if tx_cells.is_empty() {
                return Err(malformed("variable-transform cell plan is empty"));
            }
            if tx_cells.len() != cell_count {
                return Err(malformed(
                    "variable-transform cell plan has the wrong extent",
                ));
            }
            for update in tx_cells {
                let width = 1_u32
                    .checked_shl(u32::from(update.width_log2))
                    .ok_or_else(|| malformed("variable-transform width context overflows"))?;
                let height = 1_u32
                    .checked_shl(u32::from(update.height_log2))
                    .ok_or_else(|| malformed("variable-transform height context overflows"))?;
                TxSize::from_mi_dimensions(width, height)
                    .ok_or_else(|| malformed("variable-transform cell size is invalid"))?;
            }
        }
        let first_tx = metadata.tx_cells.map_or(
            TxCellUpdate {
                width_log2: leaf.tx_context_width,
                height_log2: leaf.tx_context_height,
            },
            |cells| cells[0],
        );

        self.blocks.try_reserve(1).map_err(|_| {
            CodecError::Dimensions("unable to allocate AV1 tile block metadata".to_owned())
        })?;
        self.blocks.push(DecodedBlockMeta {
            origin_x: node.x,
            origin_y: node.y,
            block_size: node.block_size,
            entropy_width: node.width,
            entropy_height: node.height,
            luma_predictor: leaf.luma_predictor,
            chroma_predictor: leaf.chroma_predictor,
            palette_cache: leaf.palette_cache,
            has_chroma,
            tx_context_width: first_tx.width_log2,
            tx_context_height: first_tx.height_log2,
            block_skipped: leaf.block_skipped,
            skip_mode: metadata.skip_mode,
            coding: metadata.coding,
        });

        let edge_contextual = leaf.luma_transform_split || leaf.wide_coefficient_contexts.is_some();
        let luma_right_contexts = leaf
            .wide_coefficient_contexts
            .as_ref()
            .map_or(leaf.luma_right_contexts.as_slice(), |contexts| {
                contexts.luma_right.as_slice()
            });
        let luma_bottom_contexts = leaf
            .wide_coefficient_contexts
            .as_ref()
            .map_or(leaf.luma_bottom_contexts.as_slice(), |contexts| {
                contexts.luma_bottom.as_slice()
            });
        let row_context = |y: u32, contexts: &[u8], origin: u32| {
            if !edge_contextual {
                return leaf.luma_context;
            }
            usize::try_from(y.saturating_sub(origin))
                .ok()
                .and_then(|offset| contexts.get(offset.min(contexts.len().saturating_sub(1))))
                .copied()
                .unwrap_or(0x40)
        };
        for y in node.y..end_y {
            for x in node.x..end_x {
                let index = self.cell_index(x, y)?;
                let column = usize::try_from(x.saturating_sub(node.x)).unwrap_or(usize::MAX);
                let bottom_context = if edge_contextual {
                    luma_bottom_contexts
                        .get(column.min(luma_bottom_contexts.len().saturating_sub(1)))
                        .copied()
                        .unwrap_or(0x40)
                } else {
                    leaf.luma_context
                };
                let cell = self
                    .cells
                    .get_mut(index)
                    .ok_or_else(|| malformed("tile cell disappeared during block state commit"))?;
                cell.owner = Some(owner);
                cell.right_context = row_context(y, luma_right_contexts, node.y);
                cell.bottom_context = bottom_context;
                let offset = usize::try_from(y.saturating_sub(node.y))
                    .ok()
                    .and_then(|row| {
                        usize::try_from(node.width)
                            .ok()
                            .and_then(|width| row.checked_mul(width))
                    })
                    .and_then(|row| {
                        usize::try_from(x.saturating_sub(node.x))
                            .ok()
                            .and_then(|column| row.checked_add(column))
                    })
                    .ok_or_else(|| malformed("transform cell offset overflows"))?;
                let tx = metadata.tx_cells.map_or(
                    TxCellUpdate {
                        width_log2: leaf.tx_context_width,
                        height_log2: leaf.tx_context_height,
                    },
                    |cells| cells[offset],
                );
                cell.tx_context_width = tx.width_log2;
                cell.tx_context_height = tx.height_log2;
                cell.segment_id = segment_id;
                cell.segment_pred = segment_pred;
                cell.skip = leaf.block_skipped;
                cell.skip_mode = metadata.skip_mode;
                cell.intra = metadata.coding.is_intra();
            }
        }
        if let Some((chroma_x, chroma_y, chroma_end_x, chroma_end_y)) = chroma_geometry {
            for y in chroma_y..chroma_end_y {
                let row = usize::try_from(y.saturating_sub(chroma_y)).unwrap_or(usize::MAX);
                for x in chroma_x..chroma_end_x {
                    let column = usize::try_from(x.saturating_sub(chroma_x)).unwrap_or(usize::MAX);
                    let index = self.chroma_cell_index(x, y)?;
                    let cell = self.chroma_cells.get_mut(index).ok_or_else(|| {
                        malformed("tile chroma cell disappeared during block state commit")
                    })?;
                    cell.owner = Some(owner);
                    for plane in 0..2 {
                        let chroma_right_contexts = leaf
                            .wide_coefficient_contexts
                            .as_ref()
                            .map_or(leaf.chroma_right_contexts[plane].as_slice(), |contexts| {
                                contexts.chroma_right[plane].as_slice()
                            });
                        let chroma_bottom_contexts =
                            leaf.wide_coefficient_contexts.as_ref().map_or(
                                leaf.chroma_bottom_contexts[plane].as_slice(),
                                |contexts| contexts.chroma_bottom[plane].as_slice(),
                            );
                        cell.right_contexts[plane] = chroma_right_contexts
                            .get(row.min(chroma_right_contexts.len().saturating_sub(1)))
                            .copied()
                            .unwrap_or(leaf.chroma_contexts[plane]);
                        cell.bottom_contexts[plane] = chroma_bottom_contexts
                            .get(column.min(chroma_bottom_contexts.len().saturating_sub(1)))
                            .copied()
                            .unwrap_or(leaf.chroma_contexts[plane]);
                    }
                }
            }
        }
        Ok(owner)
    }

    fn cell_index(&self, x: u32, y: u32) -> Av1Result<usize> {
        let x = usize::try_from(x).map_err(|_| malformed("tile x coordinate exceeds usize"))?;
        let y = usize::try_from(y).map_err(|_| malformed("tile y coordinate exceeds usize"))?;
        y.checked_mul(self.width)
            .and_then(|row| row.checked_add(x))
            .filter(|&index| index < self.cells.len())
            .ok_or_else(|| malformed("tile cell coordinate exceeds state grid"))
    }

    fn chroma_cell_index(&self, x: u32, y: u32) -> Av1Result<usize> {
        let x = usize::try_from(x).map_err(|_| malformed("tile chroma x exceeds usize"))?;
        let y = usize::try_from(y).map_err(|_| malformed("tile chroma y exceeds usize"))?;
        if x >= self.chroma_width || y >= self.chroma_height {
            return Err(malformed("tile chroma cell coordinate exceeds state grid"));
        }
        y.checked_mul(self.chroma_width)
            .and_then(|row| row.checked_add(x))
            .filter(|&index| index < self.chroma_cells.len())
            .ok_or_else(|| malformed("tile chroma cell index overflows"))
    }

    /// Select the temporal-MV entry dav1d saves for one local 8x8 cell. The
    /// source sample is intentionally the top-right 4x4 cell `(2*x8+1,2*y8)`
    /// and the second future-facing reference has priority over the first.
    pub(super) fn temporal_entry_at(
        &self,
        x8: u32,
        y8: u32,
        sign_bias: [bool; 7],
    ) -> Av1Result<Option<RetainedTemporalEntry>> {
        let x4 = x8
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| malformed("temporal sample x coordinate overflows"))?;
        let y4 = y8
            .checked_mul(2)
            .ok_or_else(|| malformed("temporal sample y coordinate overflows"))?;
        let Some(block) = self.spatial_block(x4, y4)? else {
            return Ok(None);
        };
        for index in [1_usize, 0] {
            let encoded = block.references[index];
            if encoded <= 0 {
                continue;
            }
            let encoded = encoded
                .checked_sub(1)
                .ok_or_else(|| malformed("temporal sample reference underflows"))?;
            let reference_index = usize::try_from(encoded)
                .map_err(|_| malformed("temporal sample reference exceeds seven"))?;
            if !sign_bias.get(reference_index).copied().unwrap_or(false) {
                continue;
            }
            let vector = block.vectors[index];
            if vector.y > -4096 && vector.y < 4096 && vector.x > -4096 && vector.x < 4096 {
                let reference = ReferenceFrame::from_index(reference_index)
                    .ok_or_else(|| malformed("temporal sample reference is invalid"))?;
                return Ok(Some(RetainedTemporalEntry { vector, reference }));
            }
        }
        Ok(None)
    }
}

impl SpatialMotionSource for TileState {
    fn spatial_block(&self, x_b4: u32, y_b4: u32) -> Av1Result<Option<SpatialRefBlock>> {
        let Some((_owner, block)) = self.block_at_checked(x_b4, y_b4)? else {
            return Ok(None);
        };
        let spatial = match block.coding {
            BlockCoding::Intra => SpatialRefBlock::ordinary_intra(block.block_size),
            BlockCoding::IntraBc { motion_vector } => {
                SpatialRefBlock::intra_bc(block.block_size, motion_vector)
            }
            BlockCoding::Inter(inter) => match inter.references.second {
                Some(second) => SpatialRefBlock::compound(
                    block.block_size,
                    inter.references.first,
                    second,
                    inter.motion_vectors,
                    inter.global,
                    inter.new_mv,
                ),
                None => SpatialRefBlock::single(
                    block.block_size,
                    inter.references.first,
                    inter.motion_vectors[0],
                    inter.global[0],
                    inter.new_mv[0],
                ),
            },
        };
        Ok(Some(spatial))
    }
}
