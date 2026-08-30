//! Compact tile-local ownership and entropy context state.
//!
//! Pixels live in the padded [`super::raster::FrameCanvas`]. This module keeps
//! only stable scalar block metadata and per-4×4-cell state, so decoded leaves
//! can be dropped after placement and neighbor lookup never scans frame
//! history.

use std::num::NonZeroU32;

use super::block::{ChromaPredictor, FirstLeaf, LumaPredictor, PaletteCacheState};
use super::entropy::{PartitionKind, PartitionNode};
use super::geometry::{BlockSize, TxSize};
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

const MAX_EDGE_CELLS: usize = 32;

#[allow(
    dead_code,
    reason = "bounded owner spans are the next compatibility-reader migration boundary"
)]
#[derive(Clone, Copy)]
pub(super) struct OwnerSpan {
    owners: [Option<OwnerId>; MAX_EDGE_CELLS],
    len: usize,
}

#[allow(
    dead_code,
    reason = "bounded owner spans are the next compatibility-reader migration boundary"
)]
impl OwnerSpan {
    pub(super) fn empty() -> Self {
        Self {
            owners: [None; MAX_EDGE_CELLS],
            len: 0,
        }
    }

    pub(super) fn is_empty(self) -> bool {
        self.len == 0
    }

    fn push_unique(&mut self, owner: OwnerId) -> Av1Result<()> {
        if self.len != 0 && self.owners.get(self.len - 1).copied().flatten() == Some(owner) {
            return Ok(());
        }
        let slot = self
            .owners
            .get_mut(self.len)
            .ok_or_else(|| malformed("tile edge crosses too many block owners"))?;
        *slot = Some(owner);
        self.len = self
            .len
            .checked_add(1)
            .ok_or_else(|| malformed("tile edge owner count overflows"))?;
        Ok(())
    }

    pub(super) fn iter(self) -> impl Iterator<Item = OwnerId> {
        self.owners.into_iter().take(self.len).flatten()
    }
}

#[allow(
    dead_code,
    reason = "transform-state readers migrate after owner and coefficient-context readers"
)]
#[derive(Clone, Copy)]
struct TileCell {
    owner: Option<OwnerId>,
    tx_size: Option<TxSize>,
    right_context: u8,
    bottom_context: u8,
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
            tx_size: None,
            right_context: 0x40,
            bottom_context: 0x40,
        }
    }
}

/// Stable scalar metadata for one successfully reconstructed block.
#[allow(
    dead_code,
    reason = "metadata consumers migrate incrementally from retained reconstructed leaves"
)]
#[derive(Clone, Copy)]
pub(super) struct DecodedBlockMeta {
    pub(super) partition_level: u32,
    pub(super) partition_kind: PartitionKind,
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
    pub(super) luma_transform_split: bool,
    pub(super) luma_context: u8,
    pub(super) chroma_contexts: [u8; 2],
    pub(super) luma_right_contexts: [u8; 16],
    pub(super) luma_bottom_contexts: [u8; 16],
    pub(super) chroma_right_contexts: [[u8; 16]; 2],
    pub(super) chroma_bottom_contexts: [[u8; 16]; 2],
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

#[allow(
    dead_code,
    reason = "queries migrate incrementally while the state is shadow-written beside the legacy leaf walker"
)]
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
        let mut blocks = Vec::new();
        blocks.try_reserve_exact(cell_count).map_err(|_| {
            CodecError::Dimensions("unable to allocate AV1 tile block metadata".to_owned())
        })?;
        Ok(Self {
            width,
            height,
            subsampling_x,
            subsampling_y,
            cells,
            chroma_width,
            chroma_height,
            chroma_cells,
            blocks,
        })
    }

    pub(super) fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub(super) fn decoded_block_count(&self) -> usize {
        self.blocks.len()
    }

    pub(super) fn block(&self, owner: OwnerId) -> Option<&DecodedBlockMeta> {
        self.blocks.get(owner.index()?)
    }

    pub(super) fn block_by_index(&self, index: usize) -> Option<&DecodedBlockMeta> {
        self.blocks.get(index)
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

    pub(super) fn owners_in_row(&self, y: u32, start_x: u32, end_x: u32) -> Av1Result<OwnerSpan> {
        if start_x >= end_x {
            return Err(malformed("tile row query has an empty extent"));
        }
        let y = usize::try_from(y).map_err(|_| malformed("tile row exceeds usize"))?;
        let start_x =
            usize::try_from(start_x).map_err(|_| malformed("tile row start exceeds usize"))?;
        let end_x = usize::try_from(end_x).map_err(|_| malformed("tile row end exceeds usize"))?;
        if y >= self.height || end_x > self.width {
            return Err(malformed("tile row query exceeds state grid"));
        }
        let mut owners = OwnerSpan::empty();
        for x in start_x..end_x {
            let index = y
                .checked_mul(self.width)
                .and_then(|row| row.checked_add(x))
                .ok_or_else(|| malformed("tile row query index overflows"))?;
            if let Some(owner) = self.cells.get(index).and_then(|cell| cell.owner) {
                owners.push_unique(owner)?;
            }
        }
        Ok(owners)
    }

    pub(super) fn owners_in_column(
        &self,
        x: u32,
        start_y: u32,
        end_y: u32,
    ) -> Av1Result<OwnerSpan> {
        if start_y >= end_y {
            return Err(malformed("tile column query has an empty extent"));
        }
        let x = usize::try_from(x).map_err(|_| malformed("tile column exceeds usize"))?;
        let start_y =
            usize::try_from(start_y).map_err(|_| malformed("tile column start exceeds usize"))?;
        let end_y =
            usize::try_from(end_y).map_err(|_| malformed("tile column end exceeds usize"))?;
        if x >= self.width || end_y > self.height {
            return Err(malformed("tile column query exceeds state grid"));
        }
        let mut owners = OwnerSpan::empty();
        for y in start_y..end_y {
            let index = y
                .checked_mul(self.width)
                .and_then(|row| row.checked_add(x))
                .ok_or_else(|| malformed("tile column query index overflows"))?;
            if let Some(owner) = self.cells.get(index).and_then(|cell| cell.owner) {
                owners.push_unique(owner)?;
            }
        }
        Ok(owners)
    }

    /// Publish one block only after entropy, reconstruction, validation, and
    /// padded-canvas placement have all succeeded.
    pub(super) fn commit(
        &mut self,
        node: PartitionNode,
        has_chroma: bool,
        leaf: &FirstLeaf,
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
        let tx_size = TxSize::from_mi_dimensions(tx_width, tx_height)
            .ok_or_else(|| malformed("leaf publishes a non-normative transform size"))?;

        self.blocks.push(DecodedBlockMeta {
            partition_level: node.level,
            partition_kind: node.kind,
            origin_x: node.x,
            origin_y: node.y,
            block_size: node.block_size,
            entropy_width: node.width,
            entropy_height: node.height,
            luma_predictor: leaf.luma_predictor,
            chroma_predictor: leaf.chroma_predictor,
            palette_cache: leaf.palette_cache,
            has_chroma,
            tx_context_width: leaf.tx_context_width,
            tx_context_height: leaf.tx_context_height,
            luma_transform_split: leaf.luma_transform_split,
            luma_context: leaf.luma_context,
            chroma_contexts: leaf.chroma_contexts,
            luma_right_contexts: leaf.luma_right_contexts,
            luma_bottom_contexts: leaf.luma_bottom_contexts,
            chroma_right_contexts: leaf.chroma_right_contexts,
            chroma_bottom_contexts: leaf.chroma_bottom_contexts,
        });

        let split = leaf.luma_transform_split;
        let row_context = |y: u32, contexts: &[u8; 16], origin: u32| {
            if !split {
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
                let bottom_context = if split {
                    leaf.luma_bottom_contexts
                        .get(column.min(leaf.luma_bottom_contexts.len().saturating_sub(1)))
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
                cell.tx_size = Some(tx_size);
                cell.right_context = row_context(y, &leaf.luma_right_contexts, node.y);
                cell.bottom_context = bottom_context;
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
                        cell.right_contexts[plane] = leaf.chroma_right_contexts[plane]
                            .get(row.min(leaf.chroma_right_contexts[plane].len().saturating_sub(1)))
                            .copied()
                            .unwrap_or(leaf.chroma_contexts[plane]);
                        cell.bottom_contexts[plane] =
                            leaf.chroma_bottom_contexts[plane]
                                .get(column.min(
                                    leaf.chroma_bottom_contexts[plane].len().saturating_sub(1),
                                ))
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
}
