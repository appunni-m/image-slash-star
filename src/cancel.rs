//! Dependency-free cooperative cancellation.
//!
//! A [`CancellationToken`] is a cheap handle that callers cancel from their
//! own control flow. Token-aware operations poll it at documented structural
//! and codec-internal checkpoints (frame/page boundaries, chunk boundaries,
//! strip/tile loops, TIFF Deflate level-six matcher positions/candidate chains,
//! Deflate expansion/Huffman/bitstream/checksum stages, PNG stored-block copy
//! intervals, lossy WebP VP8
//! RGB/RGBA-to-YUV conversion items, analysis, 4,096-bit logical and
//! 16,384-boolean first-partition bit intervals, 4,096-bit logical and
//! 16,384-boolean coefficient-bit intervals, 1,024-byte boolean-bitstream output
//! intervals, and bitstream stages, and lossless WebP VP8L
//! predictor/cross-color/entropy/transform, histogram/Huffman, 4,096-bit
//! logical bitstream, 1,024-byte bitstream-output, and bounded
//! backward-reference/token-stream intervals, plus GIF RGB/RGBA palette
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
