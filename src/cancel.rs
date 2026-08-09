//! Dependency-free cooperative cancellation.
//!
//! A [`CancellationToken`] is a cheap handle that callers cancel from their
//! own control flow. Token-aware operations poll it at documented structural
//! and codec-internal checkpoints (frame/page boundaries, chunk boundaries,
//! strip/tile loops, TIFF Deflate level-six matcher positions/candidate chains,
//! Deflate expansion/Huffman/bitstream/checksum stages, PNG stored-block copy
//! intervals, JPEG RGB-to-YCbCr conversion items and chroma-downsample output
//! pixels after each 1,024 converted or produced pixels, JPEG baseline entropy
//! coding after each 1,024 MCUs, and entropy-output intervals after each 1,024
//! emitted entropy bytes, lossy WebP VP8
//! RGB/RGBA-to-YUV conversion items, padded-plane edge replication when
//! padding is required, filter-edge adjustment, RGBA transparent-area cleanup
//! after each 1,024 scanned or flattened pixels, RGBA alpha-palette source
//! collection and index packing after each 1,024 source pixels, lossy WebP
//! VP8/ALPH RIFF payload and alpha-stream compressed/raw buffer copies plus
//! lossless VP8L candidate-trial suffix and RIFF frame copies, each after
//! 1,024 output bytes,
//! analysis
//! histogram construction after each 64 completed 4×4 blocks, analysis after
//! each 1,024 macroblocks, segment-clustering alpha-domain chunks after each
//! 64 values, segment-assignment macroblocks after each 1,024 items, and
//! mode-selection batches after each 64 completed macroblocks (roughly 1,024
//! luma blocks),
//! 8-bit, 16-bit, 32-bit,
//! 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit,
//! 32,768-bit, 65,536-bit, 131,072-bit, and 262,144-bit logical and
//! 16,384-boolean first-partition bit intervals, 8-bit, 16-bit, 32-bit, 64-bit,
//! 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit,
//! 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical and 16,384-boolean coefficient-bit
//! intervals, 1,024-byte boolean-bitstream output intervals (including pending
//! VP8 boolean-output runs drained in those same chunks), JPEG optimized-Huffman
//! frequency coefficients after each 1,024
//! coefficients, progressive scan block slots after each 1,024 blocks,
//! progressive scan coefficient items after each 1,024 coefficients, and
//! progressive scan-event frequency items after each 1,024 events,
//! forward/inverse-DCT row/column subpasses, non-trellis quantization
//! coefficients, method-6 trellis-quantization coefficient candidates and
//! path-reconstruction nodes, squared-error pixels, spectral-distortion
//! weighted-transform row/column passes, residual-cost coefficients, and
//! bitstream stages, and
//! WebP L1/P8/L8/La8/CMYK source-mode preparation and RGBA alpha/RGB extraction
//! after each 1,024 source pixels, lossless WebP VP8L RGB/RGBA source-pixel materialization, predictor
//! source-snapshot copying, image-palette construction, and palette-mode index
//! packing after each 1,024 source pixels,
//! RGB-equal grayscale preparation, predictor image-width tile-row copies,
//! tile scans, mode application, and
//! subtract-green transforms after
//! each 1,024 pixels,
//! RGBA hidden-RGB cleanup after each 1,024 scanned pixels,
//! cross-color multiplier search/transform and sampling scans/compaction,
//! including VP8L meta-histogram row/column comparisons and symbol compaction
//! after each 1,024 symbols,
//! palette-index lookup candidate scans after each 64 palette entries,
//! entropy, histogram-clustering populated-tile collection, min/max, and
//! bin-assignment scans after each 64 tile histograms, histogram/Huffman,
//! 8-bit, 16-bit, 32-bit, 64-bit, 128-bit,
//! 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, 1,048,576-bit, and 2,097,152-bit logical bitstream intervals, 1,024-byte bitstream-output, and bounded
//! backward-reference cost/length-table initialization and length-cost table
//! and equal-cost interval setup after each 1,024 entries, token-aware
//! cost-manager interval-update and cleanup
//! scans after each 256 cumulative interval entries, saturated cost-interval
//! fallback scans after each 1,024 entries, repeated-run hash-chain insertion
//! after each 256 pixels, long backward-reference result backfills after each
//! 256 entries, palette-mode box-chain candidate offsets after each 64 completed
//! offsets, plus cost/Huffman scans
//! after each 1,024 tokens or 64 symbols,
//! copy-token cache population after each 256 pixels,
//! Huffman-tree simple-tree symbol discovery scans after each 64 code-length
//! slots, Huffman RLE preparation and in-run code-length scans after each 64
//! code-length symbols, canonical-code assignment scans after each 64 symbols,
//! and Huffman-tree ordering comparisons and insertion scans
//! after each 64 comparisons or candidate nodes, Huffman-tree code-length-token frequency scans after each 16
//! compressed token entries, trailing zero-repeat token trim scans after each
//! 16 compressed token entries, Huffman code-length emission after each 16
//! compressed token entries, token-stream intervals, plus
//! GIF RGB/RGBA palette
//! quantization, RGB median-cut hash/order, axis-ordering, split, and 1,024-item
//! partition intervals, fixed 1,024-cell RGBA FASTOCTREE copy/subtraction/lookup
//! and bucket-sort intervals, and LZW input-symbol intervals and BMP
//! row-conversion subsegments) and stop with
//! [`ImageError::Cancelled`] without publishing partial state.
//! The crate remains single-threaded by design, so the token uses `Rc<Cell>`
//! and adds no synchronization overhead on native or WASM targets.

use std::cell::Cell;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkBudget {
    maximum: u64,
    consumed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PollResult {
    Continue,
    Cancelled,
    WorkBudgetExceeded { maximum: u64, observed: u64 },
}

/// Cooperative cancellation handle for token-aware operations.
///
/// Clones share the same cancellation state. The token is neither `Send` nor
/// `Sync`, matching the crate's single-threaded execution model.
#[derive(Clone, Default)]
pub struct CancellationToken {
    cancelled: Rc<Cell<bool>>,
    work_budget: Rc<Cell<Option<WorkBudget>>>,
    #[cfg(coverage)]
    cancel_after: Rc<Cell<Option<usize>>>,
}

impl CancellationToken {
    /// Create an uncancelled token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancel the token; every clone observes the cancellation.
    pub fn cancel(&self) {
        self.cancelled.set(true);
    }

    /// Return whether the token has been cancelled.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        #[cfg(coverage)]
        {
            if self.cancelled.get() {
                return true;
            }
            match self.cancel_after.get() {
                Some(0) => {
                    self.cancelled.set(true);
                    true
                }
                Some(remaining) => {
                    self.cancel_after.set(Some(remaining.saturating_sub(1)));
                    false
                }
                None => false,
            }
        }
        #[cfg(not(coverage))]
        {
            self.cancelled.get()
        }
    }

    pub(crate) fn with_work_budget(maximum: u64) -> Self {
        Self {
            work_budget: Rc::new(Cell::new(Some(WorkBudget {
                maximum,
                consumed: 0,
            }))),
            ..Self::new()
        }
    }

    pub(crate) fn with_work_budget_from(source: &Self, maximum: u64) -> Self {
        Self {
            cancelled: source.cancelled.clone(),
            work_budget: Rc::new(Cell::new(Some(WorkBudget {
                maximum,
                consumed: 0,
            }))),
            #[cfg(coverage)]
            cancel_after: source.cancel_after.clone(),
        }
    }

    pub(crate) fn poll(&self) -> PollResult {
        if self.cancelled.get() {
            return PollResult::Cancelled;
        }
        #[cfg(coverage)]
        {
            match self.cancel_after.get() {
                Some(0) => {
                    self.cancelled.set(true);
                    return PollResult::Cancelled;
                }
                Some(remaining) => {
                    self.cancel_after.set(Some(remaining.saturating_sub(1)));
                }
                None => {}
            }
        }
        let Some(mut budget) = self.work_budget.get() else {
            return PollResult::Continue;
        };
        if budget.consumed >= budget.maximum {
            return PollResult::WorkBudgetExceeded {
                maximum: budget.maximum,
                observed: budget.consumed.saturating_add(1),
            };
        }
        budget.consumed = budget.consumed.saturating_add(1);
        self.work_budget.set(Some(budget));
        PollResult::Continue
    }

    /// Coverage-only hook: automatically cancel after `checks` more polls.
    ///
    /// This lets the coverage drills deterministically hit each structural
    /// checkpoint inside a single call. It is compiled out of production
    /// builds and has no effect on the public contract.
    #[cfg(coverage)]
    pub(crate) fn cancel_after(&self, checks: usize) {
        self.cancel_after.set(Some(checks));
    }
}
